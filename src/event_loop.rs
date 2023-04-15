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

//! The [`EventLoop`] and associated structures.

use crate::handler::Handler;
use crate::reactor::{EventLoopOp, Reactor};

use futures_lite::prelude::*;

use std::convert::Infallible;
use std::future::Future;
use std::ops;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

use winit::event::Event;
use winit::event_loop::EventLoopProxy;

#[doc(inline)]
pub use winit::event_loop::{ControlFlow, DeviceEventFilter, EventLoopClosed};

/// Used to indicate that we need to wake up the event loop.
pub(crate) struct Wakeup;

/// Provides a way to retrieve events from the system and from the windows that were registered to
/// the events loop.
pub struct EventLoop {
    /// The underlying event loop.
    pub(crate) inner: winit::event_loop::EventLoop<Wakeup>,

    /// The window target.
    window_target: EventLoopWindowTarget,
}

/// A reference to the `EventLoop` that allows the user to create windows, among other things.
///
/// Unlike in `winit`, this type is cheaply clonable.
pub struct EventLoopWindowTarget {
    /// The associated reactor, cached for convenience.
    reactor: &'static Reactor,

    /// The event loop proxy.
    proxy: EventLoopProxy<Wakeup>,

    /// Is this using wayland?
    #[cfg(all(
        unix,
        not(any(target_os = "android", target_os = "macos", target_os = "ios")),
    ))]
    pub(crate) is_wayland: bool,
}

/// Object that allows for building the [`EventLoop`].
pub struct EventLoopBuilder {
    /// The underlying builder.
    pub(crate) inner: winit::event_loop::EventLoopBuilder<Wakeup>,
}

impl Clone for EventLoopWindowTarget {
    fn clone(&self) -> Self {
        Self {
            reactor: self.reactor,
            proxy: self.proxy.clone(),
            #[cfg(all(
                unix,
                not(any(target_os = "android", target_os = "macos", target_os = "ios")),
            ))]
            is_wayland: self.is_wayland,
        }
    }
}

impl EventLoopBuilder {
    /// Create a new [`EventLoopBuilder`].
    pub fn new() -> Self {
        Self {
            inner: winit::event_loop::EventLoopBuilder::with_user_event(),
        }
    }

    /// Builds a new event loop.
    pub fn build(&mut self) -> EventLoop {
        let inner = self.inner.build();
        EventLoop {
            window_target: EventLoopWindowTarget {
                reactor: Reactor::get(),
                proxy: inner.create_proxy(),
                #[cfg(all(
                    unix,
                    not(any(target_os = "android", target_os = "macos", target_os = "ios",)),
                ))]
                is_wayland: {
                    cfg_if::cfg_if! {
                        if #[cfg(feature = "x11")] {
                            use winit::platform::x11::EventLoopWindowTargetExtX11;
                            !inner.is_x11()
                        } else if #[cfg(feature = "wayland")] {
                            use winit::platform::wayland::EventLoopWindowTargetExtWayland;
                            inner.is_wayland()
                        } else {
                            false
                        }
                    }
                },
            },
            inner,
        }
    }
}

impl Default for EventLoopBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLoop {
    /// Alias for [`EventLoopBuilder::new().build()`].
    ///
    /// [`EventLoopBuilder::new().build()`]: EventLoopBuilder::build
    #[inline]
    pub fn new() -> EventLoop {
        EventLoopBuilder::new().build()
    }
}

impl Default for EventLoop {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl EventLoopWindowTarget {
    /// Request that the event loop exit as soon as possible.
    #[inline]
    pub fn exit(&self) {
        self.reactor.request_exit();
    }

    /// Get the handler for the `Resumed` event.
    #[inline]
    pub fn resumed(&self) -> &Handler<()> {
        &self.reactor.evl_registration.resumed
    }

    /// Get the handler for the `Suspended` event.
    #[inline]
    pub fn suspended(&self) -> &Handler<()> {
        &self.reactor.evl_registration.suspended
    }

    /// Get the primary monitor.
    #[inline]
    pub async fn primary_monitor(&self) -> Option<winit::monitor::MonitorHandle> {
        let (tx, rx) = crate::oneoff::oneoff();
        self.reactor
            .push_event_loop_op(EventLoopOp::PrimaryMonitor(tx))
            .await;
        rx.recv().await
    }

    /// Get the available monitors.
    #[inline]
    pub async fn available_monitors(&self) -> impl Iterator<Item = winit::monitor::MonitorHandle> {
        let (tx, rx) = crate::oneoff::oneoff();
        self.reactor
            .push_event_loop_op(EventLoopOp::AvailableMonitors(tx))
            .await;
        rx.recv().await.into_iter()
    }

    /// Set the device event filter.
    #[inline]
    pub async fn set_device_event_filter(&self, filter: DeviceEventFilter) {
        let (tx, rx) = crate::oneoff::oneoff();
        self.reactor
            .push_event_loop_op(EventLoopOp::SetDeviceFilter { filter, waker: tx })
            .await;

        // Wait for the filter to be set.
        rx.recv().await;
    }
}

impl EventLoop {
    /// Manually get a reference to the event loop's window target.
    #[inline]
    pub fn window_target(&self) -> &EventLoopWindowTarget {
        &self.window_target
    }

    /// Block on a future forever.
    ///
    /// This function can only be called once per event loop, despite taking `&self`. Calling this
    /// function twice will result in a panic.
    #[inline]
    pub fn block_on(self, future: impl Future<Output = Infallible> + 'static) -> ! {
        let Self {
            inner,
            window_target,
        } = self;
        let reactor = window_target.reactor;

        let mut timeout = None;
        let mut wakers = vec![];

        // Parker/unparker pair.
        let (parker, unparker) = parking::pair();

        // Create a waker to wake us up.
        let notifier = Arc::new(ReactorWaker {
            proxy: Mutex::new(inner.create_proxy()),
            notified: AtomicBool::new(true),
            awake: AtomicBool::new(false),
        });
        let notifier_waker = Waker::from(notifier.clone());
        reactor.set_proxy(notifier.clone());

        // Create another waker to hold us in the holding pattern.
        let holding_waker = Waker::from(Arc::new(HoldingPattern {
            reactor_waker: notifier.clone(),
            unparker,
        }));

        // We have to allocate the future on the heap to make it movable.
        let mut future = Box::pin(future);

        // Function for polling the future once.
        macro_rules! poll_once {
            () => {
                let mut cx = Context::from_waker(&notifier_waker);
                if let Poll::Ready(i) = future.as_mut().poll(&mut cx) {
                    match i {}
                }
            };
        }

        // Poll once before starting to set up event handlers et al.
        poll_once!();

        inner.run(move |event, elwt, flow| {
            // Function for blocking on holding.
            macro_rules! block_on {
                ($fut:expr) => {{
                    let fut = $fut;
                    futures_lite::pin!(fut);
                    let mut cx = Context::from_waker(&holding_waker);

                    loop {
                        notifier.awake.store(true, Ordering::SeqCst);

                        // Drain the incoming queue of requests.
                        // TODO: Poll timers as well?
                        reactor.drain_loop_queue(elwt);

                        if let Poll::Ready(i) = fut.as_mut().poll(&mut cx) {
                            break i;
                        }

                        notifier.awake.store(false, Ordering::SeqCst);
                        parker.park();
                    }
                }};
            }

            let mut falling_asleep = false;

            match &event {
                Event::NewEvents(_) => {
                    // We are now awake.
                    notifier.awake.store(true, Ordering::SeqCst);

                    // Figure out how long we should wait for.
                    timeout = reactor.process_timers(&mut wakers);
                }

                Event::MainEventsCleared => {
                    falling_asleep = true;
                }

                _ => {}
            }

            if falling_asleep {
                for waker in wakers.drain(..) {
                    // Don't let a panicking waker blow everything up.
                    std::panic::catch_unwind(|| waker.wake()).ok();
                }
            }

            // Post the event, block on it and poll the future at the same time.
            let posting = reactor.post_event(event).or({
                let future = future.as_mut();

                async move { match future.await {} }
            });

            block_on!(posting);

            if falling_asleep {
                // Enter the sleeping state.
                notifier.awake.store(false, Ordering::SeqCst);
            }

            // Check the notification.
            if notifier.notified.swap(false, Ordering::SeqCst) {
                // We were notified, so we should poll the future.
                poll_once!();
            }

            // Set the control flow.
            if reactor.exit_requested() {
                flow.set_exit()
            } else {
                match timeout {
                    Some(timeout) => flow.set_wait_timeout(timeout),
                    None => flow.set_wait(),
                }
            }
        })
    }
}

impl ops::Deref for EventLoop {
    type Target = EventLoopWindowTarget;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.window_target
    }
}

impl ops::DerefMut for EventLoop {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.window_target
    }
}

pub(crate) struct ReactorWaker {
    /// The proxy used to wake up the event loop.
    proxy: Mutex<EventLoopProxy<Wakeup>>,

    /// Whether or not we are already notified.
    notified: AtomicBool,

    /// Whether or not the reactor is awake.
    awake: AtomicBool,
}

impl ReactorWaker {
    pub(crate) fn notify(&self) {
        // If we are already notified, don't notify again.
        if self.notified.swap(true, Ordering::SeqCst) {
            return;
        }

        // If we are currently polling the event loop, don't notify.
        if self.awake.load(Ordering::SeqCst) {
            return;
        }

        // Wake up the reactor.
        self.proxy.lock().unwrap().send_event(Wakeup).ok();
    }
}

impl Wake for ReactorWaker {
    fn wake(self: Arc<Self>) {
        self.notify()
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.notify()
    }
}

struct HoldingPattern {
    reactor_waker: Arc<ReactorWaker>,
    unparker: parking::Unparker,
}

impl Wake for HoldingPattern {
    fn wake(self: Arc<Self>) {
        self.reactor_waker.notify();
        self.unparker.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.reactor_waker.notify();
        self.unparker.unpark();
    }
}
