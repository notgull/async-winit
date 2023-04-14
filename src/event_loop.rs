//! The [`EventLoop`] and associated structures.

use crate::handler::Handler;
use crate::reactor::Reactor;

use std::cell::RefCell;
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
    inner: RefCell<Option<winit::event_loop::EventLoop<Wakeup>>>,

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
}

/// Object that allows for building the [`EventLoop`].
pub struct EventLoopBuilder {
    /// The underlying builder.
    inner: winit::event_loop::EventLoopBuilder<Wakeup>,
}

impl Clone for EventLoopWindowTarget {
    fn clone(&self) -> Self {
        Self {
            reactor: self.reactor,
            proxy: self.proxy.clone(),
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
            },
            inner: RefCell::new(Some(inner)),
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
    pub fn block_on(&self, future: impl Future<Output = Infallible> + 'static) -> ! {
        let inner_loop = self
            .inner
            .borrow_mut()
            .take()
            .expect("Event loop already blocked on");
        let reactor = self.window_target.reactor;

        let mut timeout = None;
        let mut wakers = vec![];

        // Create a waker to wake us up.
        let notifier = Arc::new(ReactorWaker {
            proxy: Mutex::new(inner_loop.create_proxy()),
            notified: AtomicBool::new(true),
            awake: AtomicBool::new(false),
        });
        let notifier_waker = Waker::from(notifier.clone());
        reactor.set_proxy(notifier.clone());

        // We have to allocate the future on the heap to make it movable.
        let mut future = Box::pin(future);

        // Function for polling the future once.
        let mut poll_once = move || {
            let mut cx = Context::from_waker(&notifier_waker);
            if let Poll::Ready(i) = future.as_mut().poll(&mut cx) {
                match i {}
            }
        };

        // Poll once before starting to set up event handlers et al.
        poll_once();

        inner_loop.run(move |event, elwt, flow| {
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

            // Drain the queue of incoming requests.
            // TODO: Drain wakers to "wakers" and wake them all up at once.
            reactor.drain_loop_queue(elwt);
            reactor.post_event(event);

            if falling_asleep {
                // Enter the sleeping state.
                notifier.awake.store(false, Ordering::SeqCst);
            }

            // Check the notification.
            if notifier.notified.swap(false, Ordering::SeqCst) {
                // We were notified, so we should poll the future.
                poll_once();
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
