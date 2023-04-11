//! The [`EventLoop`] and associated structures.

use crate::reactor::Reactor;

use std::convert::Infallible;
use std::future::Future;

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
    inner: winit::event_loop::EventLoop<Message<T>>,

    /// The associated reactor, cached for convenience.
    reactor: &'static Reactor,
}

/// Object that allows for building the [`EventLoop`].
pub struct EventLoopBuilder<T: 'static> {
    /// The underlying builder.
    inner: winit::event_loop::EventLoopBuilder<Message<T>>,
}

/// Target that associates windows with an [`EventLoop`].
pub struct EventLoopWindowTarget<T: 'static> {
    /// The underlying ELWT.
    inner: winit::event_loop::EventLoopWindowTarget<Message<T>>,
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
            inner: self.inner.build(),
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
    #[inline]
    pub fn block_on(self, future: impl Future<Output = Infallible> + 'static) -> ! {
        todo!()
    }
}
