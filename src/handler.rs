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

//! Handle incoming events.

// TODO: Write more tests of holding.

mod waiters;

use std::future::Future;
use std::mem::ManuallyDrop;
use std::ops;
use std::pin::Pin;
use std::sync::atomic::{AtomicPtr, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use async_broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender};
use async_lock::Mutex as AsyncMutex;
use futures_lite::{future, Stream};
use slab::Slab;

pub(crate) use __private::{EventSealed, Internal};
use waiters::{Listener, RegisterResult, Waiters};

/// An event handler.
///
/// This type is used to receive events from the GUI system. Whenever an event occurs, it is sent to
/// all of the listeners of the corresponding event type. The listeners can then process the event
/// asynchronously.
///
/// There are four ways to listen to events:
///
/// - Using the `wait_once()` function, which waits for a single instance of the event. However, there
///   is a race condition where it can miss events in multithreaded environments where the event
///   occurs between the time the event is received and the time the listener is registered. To avoid
///   this, use one of the other methods. However, this method is the most efficient.
/// - Using the `wait_many()` stream, which asynchronously iterates over events.
/// - Using the `wait_direct[_async]()` function, which runs a closure in the event handler. This is
///   good for use cases like drawing.
/// - Using the `wait_guard()` function, which forces the event handler to stop until the event
///   has been completely processed. This is good for use cases like handling suspends.
///
/// This type does not allocate unless you use any waiting functions; therefore, you only pay overhead
/// for events that you use.
pub struct Handler<T: Event> {
    inner: AtomicPtr<Inner<T>>,
}

struct Inner<T: Event> {
    /// Queue of waiters for waiting once for a new event.
    once: Mutex<Waiters<T::Clonable>>,

    /// List of direct listeners.
    direct: AsyncMutex<Slab<DirectListener<T>>>,

    /// Number of holding listeners.
    holding: AtomicUsize,

    /// Generation of holding listeners.
    holding_gen: AtomicU64,

    /// Holding state.
    holding_state: Mutex<Option<HoldState<T::Clonable>>>,

    /// Listeners waiting on a holding state.
    holding_waiters: event_listener::Event,

    /// Channel for broadcasting events.
    broadcast: BroadcastSender<T::Clonable>,

    /// The corresponding receiver, to keep it alive.
    _recv: BroadcastReceiver<T::Clonable>,
}

type DirectListener<T> =
    Box<dyn FnMut(&mut <T as EventSealed>::Unique<'_>) -> DirectFuture + Send + 'static>;
type DirectFuture = Pin<Box<dyn Future<Output = bool> + Send + 'static>>;

/// The state of the hold.
struct HoldState<T> {
    /// The actual data.
    data: T,

    /// The generation of the holding listeners.
    gen: u64,

    /// The number of holding listeners left to observe the event.
    waiters_left: usize,

    /// Waker for the top-level future.
    waker: Option<Waker>,
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

    pub(crate) async fn run_with(&self, event: &mut T::Unique<'_>) {
        let inner = match self.try_inner() {
            Some(inner) => inner,
            None => return,
        };

        let clonable = T::downgrade(event, Internal::new());
        inner
            .once
            .lock()
            .unwrap()
            .notify(usize::MAX, || clonable.clone());

        // Don't broadcast unless someone is listening.
        if inner.broadcast.receiver_count() > 1 {
            inner.broadcast.try_broadcast(clonable).ok();
        }

        // Handle direct listeners.
        let mut direct = inner.direct.lock().await;
        let mut remove = vec![];

        for (key, listener) in direct.iter_mut() {
            if listener(event).await {
                remove.push(key);
            }
        }

        for key in remove {
            let _ = direct.remove(key);
        }

        // Handle held listeners.
        let held = inner.holding.load(Ordering::Acquire);
        if held > 0 {
            let gen = inner.holding_gen.fetch_add(1, Ordering::Acquire);
            let waker = future::poll_fn(|cx| Poll::Ready(cx.waker().clone())).await;

            {
                let mut hold_state = inner.holding_state.lock().unwrap();

                // There should be no hold state; create one.
                debug_assert!(hold_state.is_none());
                *hold_state = Some(HoldState {
                    data: T::downgrade(event, Internal::new()),
                    gen,
                    waiters_left: held,
                    waker: Some(waker),
                });
            }

            // Drop the lock and wake up a single waiter.
            inner.holding_waiters.notify(1);

            // Wait for the hold state to be consumed by waiters.
            future::poll_fn(|cx| {
                let mut hold_state = inner.holding_state.lock().unwrap();

                if hold_state.is_none() {
                    Poll::Ready(())
                } else {
                    hold_state.as_mut().unwrap().waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            })
            .await;
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

    pub async fn wait_direct_async<
        Fut: Future<Output = bool> + Send + 'static,
        F: FnMut(&mut T::Unique<'_>) -> Fut + Send + 'static,
    >(
        &self,
        mut f: F,
    ) -> usize {
        let inner = unsafe { &*self.inner() };
        let mut direct = inner.direct.lock().await;

        direct.insert(Box::new(move |u| Box::pin(f(u))))
    }

    pub async fn wait_direct(
        &self,
        mut f: impl FnMut(&mut T::Unique<'_>) -> bool + Send + 'static,
    ) -> usize {
        self.wait_direct_async(move |u| std::future::ready(f(u)))
            .await
    }

    pub async fn remove_direct(&self, id: usize) {
        let inner = match self.try_inner() {
            Some(inner) => inner,
            None => return,
        };

        let mut direct = inner.direct.lock().await;
        let _ = direct.remove(id);
    }

    pub fn wait_guard(&self) -> WaitGuard<'_, T> {
        let inner = unsafe { &*self.inner() };

        let gen = inner.holding_gen.load(Ordering::Acquire);
        inner.holding.fetch_add(1, Ordering::AcqRel);

        WaitGuard {
            inner,
            gen,
            waiter: None,
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
                direct: AsyncMutex::new(Slab::new()),
                holding: AtomicUsize::new(0),
                holding_gen: AtomicU64::new(0),
                holding_state: Mutex::new(None),
                holding_waiters: event_listener::Event::new(),
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

pub struct WaitGuard<'a, T: Event> {
    /// Back reference to the inner state.
    inner: &'a Inner<T>,

    /// The generation of the event we're waiting for.
    gen: u64,

    /// Waiter for a new event.
    waiter: Option<event_listener::EventListener>,
}

impl<T: Event> Drop for WaitGuard<'_, T> {
    fn drop(&mut self) {
        // Decrement the number of holders.
        self.inner.holding.fetch_sub(1, Ordering::Release);

        // If we're not waiting, we're done.
        if self.waiter.is_none() {
            return;
        }

        // If we're mid-waiter, make sure we aren't the last one.
        let mut state_lock = self.inner.holding_state.lock().unwrap();
        if let Some(ref mut state) = &mut *state_lock {
            if state.gen != self.gen {
                return;
            }

            // Decrement the count.
            state.waiters_left -= 1;
            if state.waiters_left == 0 {
                // Wake up the top-level waiter.
                if let Some(waker) = state.waker.take() {
                    std::panic::catch_unwind(|| waker.wake()).ok();
                }

                *state_lock = None;
            }
        }
    }
}

impl<'a, T: Event> WaitGuard<'a, T> {
    pub async fn wait(&mut self) -> HeldGuard<'a, '_, T> {
        loop {
            {
                // Try to acquire the lock.
                let mut state_lock = self.inner.holding_state.lock().unwrap();

                // If we are waiting...
                if let Some(ref mut state) = &mut *state_lock {
                    // ...and if it's in our generation...
                    if state.gen == self.gen {
                        // Update our generation.
                        self.gen = self.inner.holding_gen.load(Ordering::Acquire);

                        // ...then we can hold the lock.
                        return HeldGuard {
                            inner: self.inner,
                            data: state.data.clone(),
                            _guard: self,
                        };
                    } else {
                        // We probably got an event intended for another listener.
                        self.inner.holding_waiters.notify(1);

                        // Update our generation.
                        self.gen = self.inner.holding_gen.load(Ordering::Acquire);
                    }
                }
            }

            // Begin waiting.
            match self.waiter.take() {
                Some(listener) => listener.await,

                None => {
                    // Register and try again.
                    self.waiter = Some(self.inner.holding_waiters.listen());
                }
            }
        }
    }
}

pub struct HeldGuard<'a, 'b, T: Event> {
    /// Inner state.
    inner: &'a Inner<T>,

    /// The data type.
    data: T::Clonable,

    /// Back reference to the guard.
    _guard: &'b mut WaitGuard<'a, T>,
}

impl<T: Event> ops::Deref for HeldGuard<'_, '_, T> {
    type Target = T::Clonable;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Event> ops::DerefMut for HeldGuard<'_, '_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<T: Event> Drop for HeldGuard<'_, '_, T> {
    fn drop(&mut self) {
        // Decrement the number of holders.
        let mut state_lock = self.inner.holding_state.lock().unwrap();
        let state = state_lock.as_mut().unwrap();

        state.waiters_left -= 1;

        // If we're out of waiters, we're done.
        if state.waiters_left == 0 {
            // Wake up the top-level waiter.
            if let Some(waker) = state.waker.take() {
                std::panic::catch_unwind(|| waker.wake()).ok();
            }

            *state_lock = None;
        } else {
            // Otherwise, wake up the next waiter.
            drop(state_lock);
            self.inner.holding_waiters.notify(1);
        }
    }
}

pub trait Event: EventSealed {}

impl<T: Clone + 'static> Event for T {}

mod __private {
    #[doc(hidden)]
    pub struct Internal(());

    impl Internal {
        pub(crate) fn new() -> Self {
            Internal(())
        }
    }

    #[doc(hidden)]
    pub trait EventSealed {
        type Clonable: Clone + 'static;
        type Unique<'a>: 'a;

        fn downgrade(unique: &mut Self::Unique<'_>, i: Internal) -> Self::Clonable;
    }

    impl<T: Clone + 'static> EventSealed for T {
        type Clonable = T;
        type Unique<'a> = T;

        fn downgrade(unique: &mut Self::Unique<'_>, _: Internal) -> Self::Clonable {
            unique.clone()
        }
    }
}
