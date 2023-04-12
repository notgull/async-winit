//! The [`EventLoop`] and associated structures.

use crate::reactor::Reactor;

use std::cell::RefCell;
use std::convert::Infallible;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use winit::event::Event;
#[doc(inline)]
pub use winit::event_loop::{ControlFlow, DeviceEventFilter, EventLoopClosed};

pub(crate) enum Message<T> {
    User(T),
    Wakeup,
}

/// Provides a way to retrieve events from the system and from the windows that were registered to
/// the events loop.
pub struct EventLoop<T: 'static> {
    /// The underlying event loop.
    inner: RefCell<Option<winit::event_loop::EventLoop<Message<T>>>>,

    /// The associated reactor, cached for convenience.
    reactor: &'static Reactor,
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

impl EventLoopBuilder<()> {
    /// Create a new [`EventLoopBuilder`] with no user event.
    pub fn new() -> Self {
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
        EventLoop {
            inner: RefCell::new(Some(self.inner.build())),
            reactor: Reactor::get(),
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

impl<T: 'static> EventLoop<T> {
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
        let reactor = self.reactor;

        reactor.set_proxy(inner_loop.create_proxy());

        let mut timeout = None;
        let mut wakers = vec![];

        // Create a waker to wake us up.
        let notifier = Arc::new(ReactorWaker {
            reactor,
            notified: AtomicBool::new(true),
            awake: AtomicBool::new(false),
        });
        let notifier_waker = Waker::from(notifier.clone());

        // We have to allocate the future on the heap to make it movable.
        let mut future = Box::pin(future);

        inner_loop.run(move |event, _, flow| {
            match event {
                Event::NewEvents(_) => {
                    // We are now awake.
                    notifier.awake.store(true, Ordering::SeqCst);

                    // Figure out how long we should wait for.
                    timeout = reactor.process_timers(&mut wakers);
                }

                Event::MainEventsCleared => {
                    // Enter the sleeping state.
                    notifier.awake.store(false, Ordering::SeqCst);

                    for waker in wakers.drain(..) {
                        // Don't let a panicking waker blow everything up.
                        std::panic::catch_unwind(|| waker.wake()).ok();
                    }

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

                _ => {}
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

struct ReactorWaker {
    /// The reactor to notify.
    reactor: &'static Reactor,

    /// Whether or not we are already notified.
    notified: AtomicBool,

    /// Whether or not the reactor is awake.
    awake: AtomicBool,
}

impl Wake for ReactorWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref()
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if self.awake.load(Ordering::SeqCst) {
            return;
        }

        if self.notified.swap(true, Ordering::SeqCst) {
            return;
        }

        self.reactor.notify();
    }
}
