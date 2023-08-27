/*

`async-winit` is free software: you can redistribute it and/or modify it under the terms of one of
the following licenses:

* GNU Lesser General Public License as published by the Free Software Foundation, either
  version 3 of the License, or (at your option) any later version.
* Mozilla Public License as published by the Mozilla Foundation, version 2.

`async-winit` is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even
the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General
Public License and the Patron License for more details.

You should have received a copy of the GNU Lesser General Public License and the Mozilla
Public License along with `async-winit`. If not, see <https://www.gnu.org/licenses/>.

*/

//! Handle incoming events.

use std::cell::Cell;
use std::future::{Future, IntoFuture};
use std::mem;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use futures_lite::{future, Stream};
use slab::Slab;

use crate::sync::{MutexGuard, ThreadSafety, __private::*};

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
pub struct Handler<T: Event, TS: ThreadSafety> {
    /// State of the handler.
    ///
    /// `State` is around sixteen words plus the size of `T::Clonable`, and we store around 25 of
    /// them per instance of `window::Registration`. In the interest of not blowing up the size of
    /// `Registration`, we allocate this on the heap. Also, since sometimes the event will not ever
    /// be used, we use a `OnceLock` to avoid allocating the state until it is needed.
    state: TS::OnceLock<Box<TS::Mutex<State<T>>>>,
}

struct State<T: Event> {
    /// Listeners for the event.
    ///
    /// These form a linked list.
    listeners: Slab<Listener>,

    /// List of direct listeners.
    directs: Vec<DirectListener<T>>,

    /// The head and tail of the linked list.
    head_and_tail: Option<(usize, usize)>,

    /// The top-level task waiting for this task to finish.
    waker: Option<Waker>,

    /// The currently active event.
    instance: Option<T::Clonable>,
}

type DirectListener<T> =
    Box<dyn FnMut(&mut <T as Event>::Unique<'_>) -> DirectFuture + Send + 'static>;
type DirectFuture = Pin<Box<dyn Future<Output = bool> + Send + 'static>>;

impl<T: Event, TS: ThreadSafety> Handler<T, TS> {
    pub(crate) fn new() -> Self {
        Self {
            state: TS::OnceLock::new(),
        }
    }

    pub(crate) async fn run_with(&self, event: &mut T::Unique<'_>) {
        // If the state hasn't been created yet, return.
        let state = match self.state.get() {
            Some(state) => state,
            None => return,
        };

        // Run the direct listeners.
        let mut state_lock = Some(state.lock().unwrap());
        if self.run_direct_listeners(&mut state_lock, event).await {
            return;
        }

        // Set up the listeners to run.
        {
            let state = state_lock.get_or_insert_with(|| state.lock().unwrap());

            // If there are no listeners, return.
            let head = match state.head_and_tail {
                Some((head, _)) => head,
                None => return,
            };

            // Set up the state.
            state.instance = Some(T::downgrade(event));

            // Notify the first entry in the list.
            if let Some(waker) = state.notify(head) {
                waker.wake();
            }
        }

        // Wait for the listeners to finish running.
        future::poll_fn(|cx| {
            let mut state = state_lock.take().unwrap_or_else(|| state.lock().unwrap());

            // If there are no listeners, return.
            if state.head_and_tail.is_none() {
                return Poll::Ready(());
            }

            // If the waking is over, return.
            if state.instance.is_none() {
                return Poll::Ready(());
            }

            // If we don't need to set the waker, stop right now.
            if let Some(waker) = &state.waker {
                if waker.will_wake(cx.waker()) {
                    return Poll::Pending;
                }
            }

            // Set the waker and return.
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        })
        .await
    }

    async fn run_direct_listeners(
        &self,
        state: &mut Option<MutexGuard<'_, State<T>, TS>>,
        event: &mut T::Unique<'_>,
    ) -> bool {
        /// Guard to restore direct listeners event a
        struct RestoreDirects<'a, T: Event, TS: ThreadSafety> {
            state: &'a Handler<T, TS>,
            directs: Vec<DirectListener<T>>,
        }

        impl<T: Event, TS: ThreadSafety> Drop for RestoreDirects<'_, T, TS> {
            fn drop(&mut self) {
                let mut directs = mem::take(&mut self.directs);
                self.state
                    .state()
                    .lock()
                    .unwrap()
                    .directs
                    .append(&mut directs);
            }
        }

        // If there are not indirect listeners, skip this part entirely.
        let state_ref = state.as_mut().unwrap();
        if state_ref.directs.is_empty() {
            return false;
        }

        // Take out the direct listeners.
        let mut directs = RestoreDirects {
            directs: mem::take(&mut state_ref.directs),
            state: self,
        };

        // Make sure the mutex isn't locked while we call user code.
        *state = None;

        // Iterate over the direct listeners.
        for direct in &mut directs.directs {
            if direct(event).await {
                return true;
            }
        }

        false
    }

    /// Wait for the next event.
    pub fn wait(&self) -> Waiter<'_, T, TS> {
        Waiter::new(self)
    }

    /// Register an async closure be called when the event is received.
    pub fn wait_direct_async<
        Fut: Future<Output = bool> + Send + 'static,
        F: FnMut(&mut T::Unique<'_>) -> Fut + Send + 'static,
    >(
        &self,
        mut f: F,
    ) {
        let mut state = self.state().lock().unwrap();
        state.directs.push(Box::new(move |u| Box::pin(f(u))))
    }

    /// Register a closure be called when the event is received.
    pub fn wait_direct(&self, mut f: impl FnMut(&mut T::Unique<'_>) -> bool + Send + 'static) {
        self.wait_direct_async(move |u| std::future::ready(f(u)))
    }

    /// Get the inner state.
    fn state(&self) -> &TS::Mutex<State<T>> {
        self.state
            .get_or_init(|| Box::new(TS::Mutex::new(State::new())))
    }
}

impl<T: Event, TS: ThreadSafety> Unpin for Handler<T, TS> {}

impl<'a, T: Event, TS: ThreadSafety> IntoFuture for &'a Handler<T, TS> {
    type IntoFuture = Waiter<'a, T, TS>;
    type Output = T::Clonable;

    fn into_future(self) -> Self::IntoFuture {
        self.wait()
    }
}

/// Waits for an event to be received.
pub struct Waiter<'a, T: Event, TS: ThreadSafety> {
    /// The event handler.
    handler: &'a Handler<T, TS>,

    /// The index of our listener.
    index: usize,
}

impl<T: Event, TS: ThreadSafety> Unpin for Waiter<'_, T, TS> {}

impl<'a, T: Event, TS: ThreadSafety> Waiter<'a, T, TS> {
    /// Create a new waiter.
    pub(crate) fn new(handler: &'a Handler<T, TS>) -> Self {
        // Get the inner state.
        let state = handler.state();

        // Insert the listener.
        let index = state.lock().unwrap().insert();
        Self { handler, index }
    }

    fn notify_next(&mut self, mut state: MutexGuard<'_, State<T>, TS>) {
        if let Some(next) = state.listeners[self.index].next.get() {
            // Notify the next listener.
            if let Some(waker) = state.notify(next) {
                waker.wake();
            }
        } else {
            // We're done with the chain, notify the top-level task.
            state.instance = None;
            if let Some(waker) = state.waker.take() {
                waker.wake();
            }
        }
    }

    /// Wait for a guard that prevents the event from moving on.
    pub async fn hold(&mut self) -> HoldGuard<'_, 'a, T, TS> {
        // Wait for the event.
        let event = future::poll_fn(|cx| {
            let mut state = self.handler.state().lock().unwrap();

            // See if we are notified.
            if state.take_notification(self.index) {
                let event = match state.instance.clone() {
                    Some(event) => event,
                    None => return Poll::Pending,
                };

                // Return the event.
                return Poll::Ready(event);
            }

            // Register the waker and sleep.
            state.register_waker(self.index, cx.waker());
            Poll::Pending
        })
        .await;

        HoldGuard {
            waiter: self,
            event: Some(event),
        }
    }
}

impl<T: Event, TS: ThreadSafety> Future for Waiter<'_, T, TS> {
    type Output = T::Clonable;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.poll_next(cx) {
            Poll::Ready(Some(event)) => Poll::Ready(event),
            Poll::Ready(None) => panic!("event handler was dropped"),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T: Event, TS: ThreadSafety> Stream for Waiter<'_, T, TS> {
    type Item = T::Clonable;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut state = self.handler.state.get().unwrap().lock().unwrap();

        // See if we are notified.
        if state.take_notification(self.index) {
            let event = match state.instance.clone() {
                Some(event) => event,
                None => return Poll::Pending,
            };

            // Notify the next listener in the chain.
            self.notify_next(state);

            // Return the event.
            return Poll::Ready(Some(event));
        }

        // Register the waker.
        state.register_waker(self.index, cx.waker());

        Poll::Pending
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

impl<'a, T: Event, TS: ThreadSafety> Drop for Waiter<'a, T, TS> {
    fn drop(&mut self) {
        let mut state = self.handler.state().lock().unwrap();

        // Remove the listener.
        let listener = state.remove(self.index);

        // Notify the next listener if we are notified.
        if listener.notified.get() {
            self.notify_next(state);
        }
    }
}

/// A guard that notifies the next listener when dropped.
pub struct HoldGuard<'waiter, 'handler, T: Event, TS: ThreadSafety> {
    /// The waiter.
    waiter: &'waiter mut Waiter<'handler, T, TS>,

    /// The event we just received.
    event: Option<T::Clonable>,
}

impl<T: Event, TS: ThreadSafety> Deref for HoldGuard<'_, '_, T, TS> {
    type Target = T::Clonable;

    fn deref(&self) -> &Self::Target {
        self.event.as_ref().unwrap()
    }
}

impl<T: Event, TS: ThreadSafety> DerefMut for HoldGuard<'_, '_, T, TS> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.event.as_mut().unwrap()
    }
}

impl<T: Event, TS: ThreadSafety> HoldGuard<'_, '_, T, TS> {
    /// Get the event.
    pub fn into_inner(mut self) -> T::Clonable {
        self.event.take().unwrap()
    }
}

impl<T: Event, TS: ThreadSafety> Drop for HoldGuard<'_, '_, T, TS> {
    fn drop(&mut self) {
        // Tell the waiter to notify the next listener.
        self.waiter
            .notify_next(self.waiter.handler.state().lock().unwrap());
    }
}

impl<T: Event> State<T> {
    /// Get a fresh state instance.
    fn new() -> Self {
        Self {
            listeners: Slab::new(),
            directs: Vec::new(),
            head_and_tail: None,
            waker: None,
            instance: None,
        }
    }

    /// Insert a new listener into the list.
    fn insert(&mut self) -> usize {
        // Create the listener.
        let listener = Listener {
            next: Cell::new(None),
            prev: Cell::new(self.head_and_tail.map(|(_, tail)| tail)),
            waker: Cell::new(None),
            notified: Cell::new(false),
        };

        // Insert the listener into the list.
        let index = self.listeners.insert(listener);

        // Update the head and tail.
        match &mut self.head_and_tail {
            Some((_head, tail)) => {
                self.listeners[*tail].next.set(Some(index));
                *tail = index;
            }

            None => {
                self.head_and_tail = Some((index, index));
            }
        }

        index
    }

    /// Remove a listener from the list.
    fn remove(&mut self, index: usize) -> Listener {
        // Get the listener.
        let listener = self.listeners.remove(index);

        // Update the head and tail.
        match &mut self.head_and_tail {
            Some((head, tail)) => {
                if *head == index && *tail == index {
                    self.head_and_tail = None;
                } else if *head == index {
                    self.head_and_tail = Some((listener.next.get().unwrap(), *tail));
                } else if *tail == index {
                    self.head_and_tail = Some((*head, listener.prev.get().unwrap()));
                }
            }

            None => panic!("invalid listener list: head and tail are both None"),
        }

        // Update the next and previous listeners.
        if let Some(next) = listener.next.get() {
            self.listeners[next].prev.set(listener.prev.get());
        }

        if let Some(prev) = listener.prev.get() {
            self.listeners[prev].next.set(listener.next.get());
        }

        listener
    }

    /// Take out the notification.
    fn take_notification(&mut self, index: usize) -> bool {
        self.listeners[index].notified.replace(false)
    }

    /// Register a waker.
    fn register_waker(&mut self, index: usize, waker: &Waker) {
        let listener = &mut self.listeners[index];

        // If the listener's waker is the same as ours, no need to clone.
        let current_waker = listener.waker.take();
        match current_waker {
            Some(current_waker) if current_waker.will_wake(waker) => {
                listener.waker.replace(Some(current_waker));
            }
            _ => {
                listener.waker.replace(Some(waker.clone()));
            }
        }
    }

    /// Notify the listener.
    fn notify(&mut self, index: usize) -> Option<Waker> {
        // If the listener is already notified, return.
        if self.listeners[index].notified.replace(true) {
            return None;
        }

        // Return the waker.
        self.listeners[index].waker.replace(None)
    }
}

/// A registered listener in the event handler.
struct Listener {
    /// The next listener in the list.
    next: Cell<Option<usize>>,

    /// The previous listener in the list.
    prev: Cell<Option<usize>>,

    /// The waker for the listener.
    waker: Cell<Option<Waker>>,

    /// Whether or not this listener is notified.
    notified: Cell<bool>,
}

/// The type of event that can be sent over a [`Handler`].
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

struct CallOnDrop<F: FnMut()>(F);

impl<F: FnMut()> Drop for CallOnDrop<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}
