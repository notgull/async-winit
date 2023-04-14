//! Handle incoming events.

mod waiters;

use std::future::Future;
use std::mem::ManuallyDrop;
use std::pin::Pin;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use async_broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender};
use futures_lite::Stream;

use waiters::{Listener, RegisterResult, Waiters};

pub struct Handler<T: Event> {
    inner: AtomicPtr<Inner<T>>,
}

struct Inner<T: Event> {
    /// Queue of waiters for waiting once for a new event.
    once: Mutex<Waiters<T::Clonable>>,

    /// Channel for broadcasting events.
    broadcast: BroadcastSender<T::Clonable>,

    /// The corresponding receiver, to keep it alive.
    _recv: BroadcastReceiver<T::Clonable>,
}

impl<T: Event> Drop for Handler<T> {
    fn drop(&mut self) {
        let inner = *self.inner.get_mut();

        if !inner.is_null() {
            unsafe {
                let inner = Arc::from_raw(inner);
                drop(inner);
            }
        }
    }
}

impl<T: Event> Handler<T> {
    pub(crate) const fn new() -> Self {
        Self {
            inner: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    pub(crate) fn run_with(&self, event: &mut T::Unique<'_>) {
        let inner = match self.try_inner() {
            Some(inner) => inner,
            None => return,
        };

        let clonable = T::downgrade(event);
        inner
            .once
            .lock()
            .unwrap()
            .notify(usize::MAX, || clonable.clone());

        // Don't broadcast unless someone is listening.
        if inner.broadcast.receiver_count() > 1 {
            inner.broadcast.try_broadcast(clonable).ok();
        }
    }

    pub fn wait_once(&self) -> WaitOnce<T> {
        WaitOnce {
            inner: unsafe {
                Pin::new_unchecked(Arc::clone(&ManuallyDrop::new(Arc::from_raw(self.inner()))))
            },
            listener: Listener::new(),
        }
    }

    pub fn wait_many(&self) -> WaitMany<T> {
        let inner = unsafe { &*self.inner() };

        WaitMany {
            recv: inner.broadcast.new_receiver(),
        }
    }

    /// Try to get a reference to the inner event.
    ///
    /// Returns `None` if we haven't been initialized yet.
    fn try_inner(&self) -> Option<&Inner<T>> {
        let ptr = self.inner.load(Ordering::Acquire);
        unsafe { ptr.as_ref() }
    }

    /// Get a reference to the inner event, initializing it if necessary.
    fn inner(&self) -> *const Inner<T> {
        let mut ptr = self.inner.load(Ordering::Acquire);

        if ptr.is_null() {
            // Create a new inner event.
            let (mut sender, _recv) = async_broadcast::broadcast(16);
            sender.set_await_active(false);
            sender.set_overflow(true);
            let new = Arc::new(Inner::<T> {
                broadcast: sender,
                _recv,
                once: Mutex::new(Waiters::new()),
            });

            // Convert to a raw pointer.
            let new_ptr = Arc::into_raw(new) as *mut Inner<T>;

            // Try to swap it in.
            ptr = self
                .inner
                .compare_exchange(ptr, new_ptr, Ordering::AcqRel, Ordering::Acquire)
                .unwrap_or_else(|x| x);

            if ptr.is_null() {
                ptr = new_ptr;
            } else {
                unsafe {
                    drop(Arc::from_raw(new_ptr));
                }
            }
        }

        ptr as _
    }
}

impl<T: Event> Unpin for Handler<T> {}

impl<T: Event> Future for Handler<T> {
    type Output = T::Clonable;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut &*self).poll(cx)
    }
}

impl<T: Event> Future for &Handler<T> {
    type Output = T::Clonable;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = *self.get_mut();
        let inner = unsafe { &*this.inner() };

        let mut table = inner.once.lock().unwrap();
        unsafe { table.poll_internal(cx) }
    }
}

pin_project_lite::pin_project! {
    pub struct WaitOnce<T: Event> {
        // Back-reference to the table.
        inner: Pin<Arc<Inner<T>>>,

        // Listener for the next event.
        #[pin]
        listener: Listener<T::Clonable>
    }

    impl<T: Event> PinnedDrop for WaitOnce<T> {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();
            let mut table = this.inner.once.lock().unwrap();
            table.remove(this.listener);
        }
    }
}

impl<T: Event> Future for WaitOnce<T> {
    type Output = T::Clonable;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let inner = this.inner.as_ref();
        let mut table = inner.once.lock().unwrap();

        // Insert into the table if we haven't already.
        if this.listener.as_ref().is_empty() {
            table.insert(this.listener.as_mut());
        }

        // Check for an event.
        match table.register(this.listener.as_mut(), cx.waker()) {
            RegisterResult::NoTask => panic!("polled future after completion"),
            RegisterResult::Task => Poll::Pending,
            RegisterResult::Notified(event) => Poll::Ready(event),
        }
    }
}

pub struct WaitMany<T: Event> {
    recv: BroadcastReceiver<T::Clonable>,
}

impl<T: Event> Stream for WaitMany<T> {
    type Item = T::Clonable;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.recv).poll_next(cx)
    }
}

pub trait Event {
    type Clonable: Clone + 'static;
    type Unique<'a>: 'a;

    fn downgrade(unique: &mut Self::Unique<'_>) -> Self::Clonable;
}

impl<T: Clone + 'static> Event for T {
    type Clonable = T;
    type Unique<'a> = T;

    fn downgrade(unique: &mut Self::Unique<'_>) -> Self::Clonable {
        unique.clone()
    }
}
