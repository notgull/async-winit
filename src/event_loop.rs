//! The [`EventLoop`] and associated structures.

use crate::handler::Handler;
use crate::reactor::{Proxy, Reactor};

use std::cell::RefCell;
use std::convert::Infallible;
use std::future::Future;
use std::ops;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

use winit::event::Event;
#[doc(inline)]
pub use winit::event_loop::{ControlFlow, DeviceEventFilter, EventLoopClosed};

pub(crate) mod registration;

pub(crate) enum Message<T> {
    User(T),
    Wakeup,
}

/// Provides a way to retrieve events from the system and from the windows that were registered to
/// the events loop.
pub struct EventLoop<T: 'static> {
    /// The underlying event loop.
    inner: RefCell<Option<winit::event_loop::EventLoop<Message<T>>>>,

    /// The window target.
    window_target: EventLoopWindowTarget<T>,
}

/// A reference to the `EventLoop` that allows the user to create windows, among other things.
///
/// Unlike in `winit`, this type is cheaply clonable.
pub struct EventLoopWindowTarget<T: 'static> {
    /// The associated reactor, cached for convenience.
    reactor: &'static Reactor,

    /// The event loop proxy.
    proxy: EventLoopProxy<T>,
}

/// Object that allows for building the [`EventLoop`].
pub struct EventLoopBuilder<T: 'static> {
    /// The underlying builder.
    inner: winit::event_loop::EventLoopBuilder<Message<T>>,
}

/// Used to send custom events to [`EventLoop`].
pub struct EventLoopProxy<T: 'static> {
    /// The underlying proxy.
    inner: winit::event_loop::EventLoopProxy<Message<T>>,
}

impl<T: 'static> Clone for EventLoopWindowTarget<T> {
    fn clone(&self) -> Self {
        Self {
            reactor: self.reactor,
            proxy: self.proxy.clone(),
        }
    }
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl EventLoopBuilder<()> {
    /// Create a new [`EventLoopBuilder`] with no user event.
    pub fn new() -> Self {
        Self::with_user_event()
    }
}

impl<T: 'static> Default for EventLoopBuilder<T> {
    fn default() -> Self {
        Self::with_user_event()
    }
}

impl<T: 'static> EventLoopBuilder<T> {
    /// Create a new [`EventLoopBuilder`] with a new user event.
    pub fn with_user_event() -> Self {
        Self {
            inner: winit::event_loop::EventLoopBuilder::with_user_event(),
        }
    }

    /// Builds a new event loop.
    pub fn build(&mut self) -> EventLoop<T> {
        let inner = self.inner.build();
        EventLoop {
            window_target: EventLoopWindowTarget {
                reactor: Reactor::get(),
                proxy: EventLoopProxy {
                    inner: inner.create_proxy(),
                },
            },
            inner: RefCell::new(Some(inner)),
        }
    }
}

impl EventLoop<()> {
    /// Alias for [`EventLoopBuilder::new().build()`].
    ///
    /// [`EventLoopBuilder::new().build()`]: EventLoopBuilder::build
    #[inline]
    pub fn new() -> EventLoop<()> {
        EventLoopBuilder::new().build()
    }
}

impl Default for EventLoop<()> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> EventLoopWindowTarget<T> {
    /// Create a proxy that can be used to send custom events to the event loop.
    #[inline]
    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        self.proxy.clone()
    }

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

impl<T: 'static> EventLoop<T> {
    /// Manually get a reference to the event loop's window target.
    #[inline]
    pub fn window_target(&self) -> &EventLoopWindowTarget<T> {
        &self.window_target
    }

    /// Block on a future forever.
    ///
    /// This function can only be called once per event loop, despite taking `&self`. Calling this
    /// function twice will result in a panic.
    #[inline]
    pub fn block_on(&self, future: impl Future<Output = Infallible> + 'static) -> !
    where
        T: Send,
    {
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
            proxy: Mutex::new(self.create_proxy()),
            notified: AtomicBool::new(true),
            awake: AtomicBool::new(false),
        });
        let notifier_waker = Waker::from(notifier.clone());
        reactor.set_proxy(notifier.clone());

        // We have to allocate the future on the heap to make it movable.
        let mut future = Box::pin(future);

        inner_loop.run(move |event, elwt, flow| {
            let mut wake = false;

            match &event {
                Event::NewEvents(_) => {
                    // We are now awake.
                    notifier.awake.store(true, Ordering::SeqCst);

                    // Figure out how long we should wait for.
                    timeout = reactor.process_timers(&mut wakers);
                }

                Event::MainEventsCleared => {
                    wake = true;
                }

                _ => {}
            }

            if wake {
                for waker in wakers.drain(..) {
                    // Don't let a panicking waker blow everything up.
                    std::panic::catch_unwind(|| waker.wake()).ok();
                }
            }

            // Drain the queue of incoming requests.
            // TODO: Drain wakers to "wakers" and wake them all up at once.
            reactor.drain_loop_queue(elwt);
            reactor.post_event(event);

            if wake {
                // Enter the sleeping state.
                notifier.awake.store(false, Ordering::SeqCst);

                // Check the notification.
                if notifier.notified.swap(false, Ordering::SeqCst) {
                    // We were notified, so we should poll the future.
                    let mut cx = Context::from_waker(&notifier_waker);
                    match future.as_mut().poll(&mut cx) {
                        Poll::Ready(i) => match i {},
                        Poll::Pending => {}
                    }
                }
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

impl<T: 'static> ops::Deref for EventLoop<T> {
    type Target = EventLoopWindowTarget<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.window_target
    }
}

impl<T: 'static> ops::DerefMut for EventLoop<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.window_target
    }
}

struct ReactorWaker<T: 'static> {
    /// The proxy used to wake up the event loop.
    proxy: Mutex<EventLoopProxy<T>>,

    /// Whether or not we are already notified.
    notified: AtomicBool,

    /// Whether or not the reactor is awake.
    awake: AtomicBool,
}

impl<T: 'static> Proxy for ReactorWaker<T> {
    fn notify(&self) {
        // If we are already notified, don't notify again.
        if self.notified.swap(true, Ordering::SeqCst) {
            return;
        }

        // If we are currently polling the event loop, don't notify.
        if self.awake.load(Ordering::SeqCst) {
            return;
        }

        // Wake up the reactor.
        self.proxy
            .lock()
            .unwrap()
            .inner
            .send_event(Message::Wakeup)
            .ok();
    }
}

impl<T: 'static> Wake for ReactorWaker<T> {
    fn wake(self: Arc<Self>) {
        self.notify()
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.notify()
    }
}
