//! Handle incoming events.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};

use async_broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender};
use async_channel::{Receiver as ChannelReceiver, Sender as ChannelSender};
use futures_lite::stream::Stream;

/// A handler for events.
pub struct Handler<T: Event> {
    /// The inner handler.
    inner: Arc<Inner<T>>,

    /// The broadcast receiver.
    receiver: BroadcastReceiver<T::Clonable>,
}

impl<T: Event> Clone for Handler<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            receiver: self.receiver.clone(),
        }
    }
}

/// The inner handler.
struct Inner<T: Event> {
    /// The event sender.
    sender: BroadcastSender<T::Clonable>,

    /// The hold sender.
    hold: RwLock<Option<ChannelSender<T>>>,
}

impl<T: Event> Handler<T> {
    /// Create a new handler.
    pub(crate) fn new(cap: usize) -> Self {
        let (mut sender, receiver) = async_broadcast::broadcast(cap);
        sender.set_overflow(true);
        sender.set_await_active(false);

        Self {
            inner: Arc::new(Inner {
                sender,
                hold: RwLock::new(None),
            }),
            receiver,
        }
    }

    /// Send an event to this handler.
    pub(crate) fn send(&self, event: T) {
        self.inner.sender.try_broadcast(event.downgrade()).ok();
    }
}

impl<T: Event> Future for Handler<T> {
    type Output = T::Clonable;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_next(cx).map(|x| x.unwrap())
    }
}

impl<T: Event> Stream for Handler<T> {
    type Item = T::Clonable;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().receiver).poll_next(cx)
    }
}

/// The type of event that occurred.
pub trait Event {
    /// The clonable version of this event.
    type Clonable: Clone + 'static;

    /// Downgrade this event to a clonable version.
    fn downgrade(&self) -> Self::Clonable;
}

impl<T: Clone + 'static> Event for T {
    type Clonable = T;

    fn downgrade(&self) -> Self::Clonable {
        self.clone()
    }
}
