/*

`async-winit` is free software: you can redistribute it and/or modify it under the terms of one of
the following licenses:

- The GNU Affero General Public License as published by the Free Software Foundation, either version
  3 of the License, or (at your option) any later version.
- The Patron License at https://github.com/notgull/async-winit/blob/main/LICENSE-PATRON.md, for
  sponsors and contributors, who can ignore the copyleft provisions of the GNU AGPL for this project.

`async-winit` is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even
the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General
Public License and the Patron License for more details.

You should have received a copy of the GNU Affero General Public License and the corresponding Patron
License along with `async-winit`. If not, see <https://www.gnu.org/licenses/>.

*/

//! The shared reactor used by the runtime.

use crate::filter::ReactorWaker;
use crate::handler::Handler;
use crate::oneoff::Complete;
use crate::window::registration::Registration as WinRegistration;
use crate::window::WindowBuilder;

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Waker;
use std::time::{Duration, Instant};

use async_channel::{Receiver, Sender};
use concurrent_queue::ConcurrentQueue;
use once_cell::sync::OnceCell as OnceLock;

use winit::dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use winit::error::{ExternalError, NotSupportedError, OsError};
use winit::event_loop::DeviceEventFilter;
use winit::monitor::MonitorHandle;
use winit::window::{
    CursorGrabMode, CursorIcon, Fullscreen, Icon, ImePurpose, ResizeDirection, Theme,
    UserAttentionType, Window, WindowId, WindowLevel,
};

const NEEDS_EXIT: i64 = 0x1;
const EXIT_CODE_SHIFT: u32 = 1;

pub(crate) struct Reactor {
    /// The exit code to exit with, if any.
    exit_code: AtomicI64,

    /// The channel used to send event loop operation requests.
    evl_ops: (Sender<EventLoopOp>, Receiver<EventLoopOp>),

    /// The list of windows.
    windows: Mutex<HashMap<WindowId, Arc<WinRegistration>>>,

    /// The event loop proxy.
    ///
    /// Used to wake up the event loop.
    proxy: OnceLock<Arc<ReactorWaker>>,

    /// The timer wheel.
    timers: Mutex<BTreeMap<(Instant, usize), Waker>>,

    /// Queue of timer operations.
    timer_op_queue: ConcurrentQueue<TimerOp>,

    /// The last timer ID we used.
    timer_id: AtomicUsize,

    /// Registration for event loop events.
    pub(crate) evl_registration: GlobalRegistration,
}

enum TimerOp {
    /// Add a new timer.
    InsertTimer(Instant, usize, Waker),

    /// Delete an existing timer.
    RemoveTimer(Instant, usize),
}

impl Reactor {
    /// Get the global instance of the `Reactor`.
    ///
    /// Since there can only be one instance of `EventLoop`, we can also have only one instance of a `Reactor`.
    /// If `winit` is ever updated so that `EventLoopBuilder::build()` doesn't panic if it's called more than
    /// once, remove this!
    ///
    /// Relevant winit code:
    /// <https://github.com/rust-windowing/winit/blob/2486f0f1a1d00ac9e5936a5222b2cfe90ceeca02/src/event_loop.rs#L114-L117>
    pub(crate) fn get() -> &'static Self {
        static REACTOR: OnceLock<Reactor> = OnceLock::new();

        REACTOR.get_or_init(|| Reactor {
            exit_code: AtomicI64::new(0),
            proxy: OnceLock::new(),
            evl_ops: async_channel::bounded(1024),
            windows: Mutex::new(HashMap::new()),
            timers: BTreeMap::new().into(),
            timer_op_queue: ConcurrentQueue::bounded(1024),
            timer_id: AtomicUsize::new(1),
            evl_registration: GlobalRegistration::new(),
        })
    }

    /// Set the event loop proxy.
    pub(crate) fn set_proxy(&self, proxy: Arc<ReactorWaker>) {
        self.proxy.set(proxy).ok();
    }

    /// Get whether or not we need to exit, and the code as well.
    pub(crate) fn exit_requested(&self) -> Option<i32> {
        let value = self.exit_code.load(Ordering::SeqCst);
        if value & NEEDS_EXIT != 0 {
            Some((value >> EXIT_CODE_SHIFT) as i32)
        } else {
            None
        }
    }

    /// Request that the event loop exit.
    pub(crate) fn request_exit(&self, code: i32) {
        let value = NEEDS_EXIT | (code as i64) << EXIT_CODE_SHIFT;

        // Set the exit code.
        self.exit_code.store(value, Ordering::SeqCst);

        // Wake up the event loop.
        self.notify();
    }

    /// Insert a new timer into the timer wheel.
    pub(crate) fn insert_timer(&self, deadline: Instant, waker: &Waker) -> usize {
        // Generate a new ID.
        let id = self.timer_id.fetch_add(1, Ordering::Relaxed);

        // Insert the timer into the timer wheel.
        let mut op = TimerOp::InsertTimer(deadline, id, waker.clone());
        while let Err(e) = self.timer_op_queue.push(op) {
            // Process incoming timer operations.
            let mut timers = self.timers.lock().unwrap();
            self.process_timer_ops(&mut timers);
            op = e.into_inner();
        }

        // Notify that we have new timers.
        self.notify();

        // Return the ID.
        id
    }

    /// Remove a timer from the timer wheel.
    pub(crate) fn remove_timer(&self, deadline: Instant, id: usize) {
        let mut op = TimerOp::RemoveTimer(deadline, id);
        while let Err(e) = self.timer_op_queue.push(op) {
            // Process incoming timer operations.
            let mut timers = self.timers.lock().unwrap();
            self.process_timer_ops(&mut timers);
            op = e.into_inner();
        }
    }

    /// Insert a window into the window list.
    pub(crate) fn insert_window(&self, id: WindowId) -> Arc<WinRegistration> {
        let mut windows = self.windows.lock().unwrap();
        let registration = Arc::new(WinRegistration::new());
        windows.insert(id, registration.clone());
        registration
    }

    /// Remove a window from the window list.
    pub(crate) fn remove_window(&self, id: WindowId) {
        let mut windows = self.windows.lock().unwrap();
        windows.remove(&id);
    }

    /// Process pending timer operations.
    fn process_timer_ops(&self, timers: &mut BTreeMap<(Instant, usize), Waker>) {
        // Limit the number of operations we process at once to avoid starving other tasks.
        let limit = self.timer_op_queue.capacity().unwrap();

        self.timer_op_queue
            .try_iter()
            .take(limit)
            .for_each(|op| match op {
                TimerOp::InsertTimer(deadline, id, waker) => {
                    timers.insert((deadline, id), waker);
                }
                TimerOp::RemoveTimer(deadline, id) => {
                    if let Some(waker) = timers.remove(&(deadline, id)) {
                        // Don't let a waker that panics on drop blow everything up.
                        std::panic::catch_unwind(|| drop(waker)).ok();
                    }
                }
            });
    }

    /// Process timers and return the amount of time to wait.
    pub(crate) fn process_timers(&self, wakers: &mut Vec<Waker>) -> Option<Duration> {
        // Process incoming timer operations.
        let mut timers = self.timers.lock().unwrap();
        self.process_timer_ops(&mut timers);

        let now = Instant::now();

        // Split timers into pending and ready timers.
        let pending = timers.split_off(&(now + Duration::from_nanos(1), 0));
        let ready = std::mem::replace(&mut *timers, pending);

        // Figure out how long it will be until the next timer is ready.
        let timeout = if ready.is_empty() {
            timers
                .keys()
                .next()
                .map(|(deadline, _)| deadline.saturating_duration_since(now))
        } else {
            // There are timers ready to fire now.
            Some(Duration::ZERO)
        };

        drop(timers);

        // Push wakers for ready timers.
        wakers.extend(ready.into_values());

        timeout
    }

    /// Wake up the event loop.
    pub(crate) fn notify(&self) {
        if let Some(proxy) = self.proxy.get() {
            proxy.notify();
        }
    }

    /// Push an event loop operation.
    pub(crate) async fn push_event_loop_op(&self, op: EventLoopOp) {
        self.evl_ops.0.send(op).await.unwrap();

        // Notify the event loop that there is a new operation.
        self.notify();
    }

    /// Drain the event loop operation queue.
    pub(crate) fn drain_loop_queue<T: 'static>(
        &self,
        elwt: &winit::event_loop::EventLoopWindowTarget<T>,
    ) {
        for _ in 0..self.evl_ops.1.capacity().unwrap() {
            if let Ok(op) = self.evl_ops.1.try_recv() {
                op.run(elwt);
            } else {
                break;
            }
        }
    }

    /// Post an event to the reactor.
    pub(crate) async fn post_event<T: 'static>(&self, event: winit::event::Event<'_, T>) {
        use winit::event::Event;

        match event {
            Event::WindowEvent { window_id, event } => {
                let registration = {
                    let windows = self.windows.lock().unwrap();
                    windows.get(&window_id).cloned()
                };

                if let Some(registration) = registration {
                    registration.signal(event).await;
                }
            }
            Event::Resumed => {
                self.evl_registration.resumed.run_with(&mut ()).await;
            }
            Event::Suspended => self.evl_registration.suspended.run_with(&mut ()).await,
            Event::RedrawRequested(id) => {
                let registration = {
                    let windows = self.windows.lock().unwrap();
                    windows.get(&id).cloned()
                };

                if let Some(registration) = registration {
                    registration.redraw_requested.run_with(&mut ()).await;
                }
            }
            _ => {}
        }
    }
}

/// An operation to run in the main event loop thread.
pub(crate) enum EventLoopOp {
    /// Build a window.
    BuildWindow {
        /// The window builder to build.
        builder: Box<WindowBuilder>,

        /// The window has been built.
        waker: Complete<Result<winit::window::Window, OsError>>,
    },

    /// Get the primary monitor.
    PrimaryMonitor(Complete<Option<MonitorHandle>>),

    /// Get the list of monitors.
    AvailableMonitors(Complete<Vec<MonitorHandle>>),

    /// Set the device filter.
    SetDeviceFilter {
        /// The device filter.
        filter: DeviceEventFilter,

        /// The device filter has been set.
        waker: Complete<()>,
    },

    /// Get the inner position of the window.
    InnerPosition {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<Result<PhysicalPosition<i32>, NotSupportedError>>,
    },

    /// Get the outer position of the window.
    OuterPosition {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<Result<PhysicalPosition<i32>, NotSupportedError>>,
    },

    /// Set the outer position.
    SetOuterPosition {
        /// The window.
        window: Arc<Window>,

        /// The position.
        position: Position,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Get the inner size.
    InnerSize {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<PhysicalSize<u32>>,
    },

    /// Set the inner size.
    SetInnerSize {
        /// The window.
        window: Arc<Window>,

        /// The size.
        size: Size,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Get the outer size.
    OuterSize {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<PhysicalSize<u32>>,
    },

    /// Set the minimum inner size.
    SetMinInnerSize {
        /// The window.
        window: Arc<Window>,

        /// The size.
        size: Option<Size>,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Set the maximum inner size.
    SetMaxInnerSize {
        /// The window.
        window: Arc<Window>,

        /// The size.
        size: Option<Size>,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Get the resize increments.
    ResizeIncrements {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<Option<PhysicalSize<u32>>>,
    },

    /// Set the resize increments.
    SetResizeIncrements {
        /// The window.
        window: Arc<Window>,

        /// The size.
        size: Option<Size>,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Set the title.
    SetTitle {
        /// The window.
        window: Arc<Window>,

        /// The title.
        title: String,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Set whether the window is transparent.
    SetTransparent {
        /// The window.
        window: Arc<Window>,

        /// Whether the window is transparent.
        transparent: bool,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Set whether or not the window is resizable.
    SetResizable {
        /// The window.
        window: Arc<Window>,

        /// Whether or not the window is resizable.
        resizable: bool,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Set whether the window is visible.
    SetVisible {
        /// The window.
        window: Arc<Window>,

        /// Whether the window is visible.
        visible: bool,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Get whether the window is resizable.
    Resizable {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<bool>,
    },

    /// Get whether the window is visible.
    Visible {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<Option<bool>>,
    },

    /// Set whether the window is minimized.
    SetMinimized {
        /// The window.
        window: Arc<Window>,

        /// Whether the window is minimized.
        minimized: bool,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Get whether the window is minimized.
    Minimized {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<Option<bool>>,
    },

    /// Set whether the window is maximized.
    SetMaximized {
        /// The window.
        window: Arc<Window>,

        /// Whether the window is maximized.
        maximized: bool,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Get whether the window is maximized.
    Maximized {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<bool>,
    },

    /// Set whether the window is fullscreen.
    SetFullscreen {
        /// The window.
        window: Arc<Window>,

        /// Whether the window is fullscreen.
        fullscreen: Option<Fullscreen>,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Get whether the window is fullscreen.
    Fullscreen {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<Option<Fullscreen>>,
    },

    /// Set whether the window is decorated.
    SetDecorated {
        /// The window.
        window: Arc<Window>,

        /// Whether the window is decorated.
        decorated: bool,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Get whether the window is decorated.
    Decorated {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<bool>,
    },

    /// Set the window level.
    SetWindowLevel {
        /// The window.
        window: Arc<Window>,

        /// The window level.
        level: WindowLevel,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Set the window icon.
    SetWindowIcon {
        /// The window.
        window: Arc<Window>,

        /// The window icon.
        icon: Option<Icon>,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Set the IME position.
    SetImePosition {
        /// The window.
        window: Arc<Window>,

        /// The IME position.
        position: Position,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Set whether IME is allowed.
    SetImeAllowed {
        /// The window.
        window: Arc<Window>,

        /// Whether IME is allowed.
        allowed: bool,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Set the IME purpose.
    SetImePurpose {
        /// The window.
        window: Arc<Window>,

        /// The IME purpose.
        purpose: ImePurpose,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Focus the window.
    FocusWindow {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Tell whether or not the window is focused.
    Focused {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<bool>,
    },

    /// Request user attention.
    RequestUserAttention {
        /// The window.
        window: Arc<Window>,

        /// The request.
        request_type: Option<UserAttentionType>,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Set the theme of the window.
    SetTheme {
        /// The window.
        window: Arc<Window>,

        /// The theme.
        theme: Option<Theme>,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Get the theme of the window.
    Theme {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<Option<Theme>>,
    },

    /// Set whether the content is protected.
    SetProtectedContent {
        /// The window.
        window: Arc<Window>,

        /// Whether the content is protected.
        protected: bool,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Get the title.
    Title {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<String>,
    },

    /// Set the cursor icon.
    SetCursorIcon {
        /// The window.
        window: Arc<Window>,

        /// The cursor icon.
        icon: CursorIcon,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Set the cursor position.
    SetCursorPosition {
        /// The window.
        window: Arc<Window>,

        /// The cursor position.
        position: Position,

        /// Wake up the task.
        waker: Complete<Result<(), ExternalError>>,
    },

    /// Set the cursor grab.
    SetCursorGrab {
        /// The window.
        window: Arc<Window>,

        /// The mode to grab the cursor.
        mode: CursorGrabMode,

        /// Wake up the task.
        waker: Complete<Result<(), ExternalError>>,
    },

    /// Set whether the cursor is visible.
    SetCursorVisible {
        /// The window.
        window: Arc<Window>,

        /// Whether the cursor is visible.
        visible: bool,

        /// Wake up the task.
        waker: Complete<()>,
    },

    /// Drag the window.
    DragWindow {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<Result<(), ExternalError>>,
    },

    /// Drag-resize the window.
    DragResizeWindow {
        /// The window.
        window: Arc<Window>,

        direction: ResizeDirection,

        /// Wake up the task.
        waker: Complete<Result<(), ExternalError>>,
    },

    /// Set the cursor hit test.
    SetCursorHitTest {
        /// The window.
        window: Arc<Window>,

        /// The cursor hit test.
        hit_test: bool,

        /// Wake up the task.
        waker: Complete<Result<(), ExternalError>>,
    },

    /// Get the current monitor.
    CurrentMonitor {
        /// The window.
        window: Arc<Window>,

        /// Wake up the task.
        waker: Complete<Option<MonitorHandle>>,
    },
}

impl EventLoopOp {
    /// Run this event loop operation on a window target.
    fn run<T: 'static>(self, target: &winit::event_loop::EventLoopWindowTarget<T>) {
        match self {
            EventLoopOp::BuildWindow { builder, waker } => {
                waker.send(builder.into_winit_builder().build(target));
            }

            EventLoopOp::PrimaryMonitor(waker) => {
                waker.send(target.primary_monitor());
            }

            EventLoopOp::AvailableMonitors(waker) => {
                waker.send(target.available_monitors().collect());
            }

            EventLoopOp::SetDeviceFilter { filter, waker } => {
                target.set_device_event_filter(filter);
                waker.send(());
            }

            EventLoopOp::InnerPosition { window, waker } => {
                waker.send(window.inner_position());
            }

            EventLoopOp::OuterPosition { window, waker } => {
                waker.send(window.outer_position());
            }

            EventLoopOp::SetOuterPosition {
                window,
                position,
                waker,
            } => {
                window.set_outer_position(position);
                waker.send(());
            }

            EventLoopOp::InnerSize { window, waker } => {
                waker.send(window.inner_size());
            }

            EventLoopOp::SetInnerSize {
                window,
                size,
                waker,
            } => {
                window.set_inner_size(size);
                waker.send(());
            }

            EventLoopOp::OuterSize { window, waker } => {
                waker.send(window.outer_size());
            }

            EventLoopOp::SetMinInnerSize {
                window,
                size,
                waker,
            } => {
                window.set_min_inner_size(size);
                waker.send(());
            }

            EventLoopOp::SetMaxInnerSize {
                window,
                size,
                waker,
            } => {
                window.set_max_inner_size(size);
                waker.send(());
            }

            EventLoopOp::ResizeIncrements { window, waker } => {
                waker.send(window.resize_increments());
            }

            EventLoopOp::SetResizeIncrements {
                window,
                size,
                waker,
            } => {
                window.set_resize_increments(size);
                waker.send(());
            }

            EventLoopOp::SetTitle {
                window,
                title,
                waker,
            } => {
                window.set_title(&title);
                waker.send(());
            }

            EventLoopOp::SetWindowIcon {
                window,
                icon,
                waker,
            } => {
                window.set_window_icon(icon);
                waker.send(());
            }

            EventLoopOp::Fullscreen { window, waker } => {
                waker.send(window.fullscreen());
            }

            EventLoopOp::SetFullscreen {
                window,
                fullscreen,
                waker,
            } => {
                window.set_fullscreen(fullscreen);
                waker.send(());
            }

            EventLoopOp::Maximized { window, waker } => {
                waker.send(window.is_maximized());
            }

            EventLoopOp::SetMaximized {
                window,
                maximized,
                waker,
            } => {
                window.set_maximized(maximized);
                waker.send(());
            }

            EventLoopOp::Minimized { window, waker } => {
                waker.send(window.is_minimized());
            }

            EventLoopOp::SetMinimized {
                window,
                minimized,
                waker,
            } => {
                window.set_minimized(minimized);
                waker.send(());
            }

            EventLoopOp::Visible { window, waker } => {
                waker.send(window.is_visible());
            }

            EventLoopOp::SetVisible {
                window,
                visible,
                waker,
            } => {
                window.set_visible(visible);
                waker.send(());
            }

            EventLoopOp::Decorated { window, waker } => {
                waker.send(window.is_decorated());
            }

            EventLoopOp::SetDecorated {
                window,
                decorated,
                waker,
            } => {
                window.set_decorations(decorated);
                waker.send(());
            }

            EventLoopOp::SetWindowLevel {
                window,
                level,
                waker,
            } => {
                window.set_window_level(level);
                waker.send(());
            }

            EventLoopOp::SetImePosition {
                window,
                position,
                waker,
            } => {
                window.set_ime_position(position);
                waker.send(());
            }

            EventLoopOp::SetImeAllowed {
                window,
                allowed,
                waker,
            } => {
                window.set_ime_allowed(allowed);
                waker.send(());
            }

            EventLoopOp::SetImePurpose {
                window,
                purpose,
                waker,
            } => {
                window.set_ime_purpose(purpose);
                waker.send(());
            }

            EventLoopOp::FocusWindow { window, waker } => {
                window.focus_window();
                waker.send(());
            }

            EventLoopOp::Focused { window, waker } => {
                waker.send(window.has_focus());
            }

            EventLoopOp::RequestUserAttention {
                window,
                request_type,
                waker,
            } => {
                window.request_user_attention(request_type);
                waker.send(());
            }

            EventLoopOp::SetTheme {
                window,
                theme,
                waker,
            } => {
                window.set_theme(theme);
                waker.send(());
            }

            EventLoopOp::Theme { window, waker } => {
                waker.send(window.theme());
            }

            EventLoopOp::SetProtectedContent {
                window,
                protected,
                waker,
            } => {
                window.set_content_protected(protected);
                waker.send(());
            }

            EventLoopOp::Title { window, waker } => {
                waker.send(window.title());
            }

            EventLoopOp::SetCursorIcon {
                window,
                icon,
                waker,
            } => {
                window.set_cursor_icon(icon);
                waker.send(());
            }

            EventLoopOp::SetCursorGrab {
                window,
                mode,
                waker,
            } => {
                waker.send(window.set_cursor_grab(mode));
            }

            EventLoopOp::SetCursorVisible {
                window,
                visible,
                waker,
            } => {
                window.set_cursor_visible(visible);
                waker.send(());
            }

            EventLoopOp::DragWindow { window, waker } => {
                waker.send(window.drag_window());
            }

            EventLoopOp::DragResizeWindow {
                window,
                direction,
                waker,
            } => {
                waker.send(window.drag_resize_window(direction));
            }

            EventLoopOp::SetCursorHitTest {
                window,
                hit_test,
                waker,
            } => {
                waker.send(window.set_cursor_hittest(hit_test));
            }

            EventLoopOp::CurrentMonitor { window, waker } => {
                waker.send(window.current_monitor());
            }

            EventLoopOp::SetTransparent {
                window,
                transparent,
                waker,
            } => {
                window.set_transparent(transparent);
                waker.send(());
            }

            EventLoopOp::SetResizable {
                window,
                resizable,
                waker,
            } => {
                window.set_resizable(resizable);
                waker.send(());
            }

            EventLoopOp::Resizable { window, waker } => {
                waker.send(window.is_resizable());
            }

            EventLoopOp::SetCursorPosition {
                window,
                position,
                waker,
            } => {
                waker.send(window.set_cursor_position(position));
            }
        }
    }
}

pub(crate) struct GlobalRegistration {
    pub(crate) resumed: Handler<()>,
    pub(crate) suspended: Handler<()>,
}

impl GlobalRegistration {
    pub(crate) fn new() -> Self {
        Self {
            resumed: Handler::new(),
            suspended: Handler::new(),
        }
    }
}
