//! Handle incoming events.

use std::cell::{Cell, UnsafeCell};
use std::future::Future;
use std::marker::PhantomPinned;
use std::mem;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex, RwLock};
use std::task::{Context, Poll, Waker};

use async_broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender};
use async_channel::{Receiver as ChannelReceiver, Sender as ChannelSender};
use futures_lite::stream::Stream;

pub struct Handler<T: Event> {
    once: Pin<Box<Mutex<Waiters<T::Clonable>>>>,
}

pin_project_lite::pin_project! {
    pub struct WaitOnce<'a, T: Event> {
        // Back-reference to the table.
        table: Pin<&'a Mutex<Waiters<T::Clonable>>>,

        // Listener for the next event.
        #[pin]
        listener: Listener<T::Clonable>
    }
}

impl<T: Event> Future for WaitOnce<'_, T> {
    type Output = T::Clonable;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let mut table = this.table.get_ref().lock().unwrap();

        // Check for an event.
        match table.table.register(this.listener.as_mut(), cx.waker()) {
            RegisterResult::NoTask => panic!("polled future after completion"),
            RegisterResult::Task => Poll::Pending,
            RegisterResult::Notified(event) => Poll::Ready(event),
        }
    }
}

pub trait Event {
    type Clonable;
}

struct Waiters<T> {
    /// Table of listeners to wake up.
    table: Table<T>,

    /// A listener for pollers.
    poller: Listener<T>,
}

struct Listener<T> {
    /// The inner entry in the table, if any.
    entry: Option<UnsafeCell<Entry<T>>>,

    /// This is never moved.
    _pin: PhantomPinned,
}

struct Table<T> {
    /// The head of the linked list.
    head: Option<NonNull<Entry<T>>>,

    /// The tail of the linked list.
    tail: Option<NonNull<Entry<T>>>,

    /// The first entry that hasn't been polled yet.
    start: Option<NonNull<Entry<T>>>,

    /// The number of entries in the table.
    len: usize,
}

impl<T> Table<T> {
    /// Create a new, empty table.
    fn new() -> Self {
        Self {
            head: None,
            tail: None,
            start: None,
            len: 0,
        }
    }

    /// Insert a new entry into the table.
    fn insert(&mut self, listener: Pin<&mut Listener<T>>) {
        // Get a pointer to the underlying entry.
        // SAFETY: The lock is held, so we can access the entry.
        let entry = unsafe {
            let listener = listener.get_unchecked_mut();
            debug_assert!(listener.entry.is_none());

            let cell = listener.entry.insert(UnsafeCell::new(Entry {
                state: Cell::new(State::Created),
                next: Cell::new(self.tail),
                prev: Cell::new(None),
            }));

            &*cell.get()
        };

        // Replace the tail with the new entry.
        match mem::replace(&mut self.tail, Some(entry.into())) {
            None => self.head = Some(entry.into()),
            Some(t) => unsafe {
                t.as_ref().next.set(Some(entry.into()));
            },
        }

        // If there are no unnotified entries, this is the first.
        if self.start.is_none() {
            self.start = Some(entry.into());
        }

        // Bump the length.
        self.len += 1;
    }

    /// Remove an entry from the table.
    fn remove(&mut self, listener: Pin<&mut Listener<T>>, propagate: bool) -> Option<State<T>> {
        let entry = unsafe {
            // SAFETY: We never move out the entry.
            let listener = listener.as_mut().get_unchecked_mut();

            &*listener.entry.as_ref()?.get()
        };

        let prev = entry.prev.get();
        let next = entry.next.get();

        // Unlink from the previous entry.
        match prev {
            None => self.head = next,
            Some(p) => unsafe {
                p.as_ref().next.set(next);
            },
        }

        // Unlink from the next entry.
        match next {
            None => self.tail = prev,
            Some(n) => unsafe {
                n.as_ref().prev.set(prev);
            },
        }

        // If this was the first unnotified entry, update the start.
        if Some(entry.into()) == self.start {
            self.start = next;
        }

        // We can now take out the entry safely.
        let entry = unsafe {
            listener
                .get_unchecked_mut()
                .entry
                .take()
                .unwrap()
                .into_inner()
        };

        let mut state = entry.state.into_inner();

        // Propagate the state if necessary.
        if propagate {
            if let State::Ready(tag) = mem::replace(&mut state, State::Done) {
                let mut tag = Some(tag);
                self.notify(1, || tag.take().unwrap());
            }
        }

        Some(state)
    }

    // Register a task to be notified when the event is triggered.
    fn register(&mut self, mut listener: Pin<&mut Listener<T>>, task: &Waker) -> RegisterResult<T> {
        // SAFETY: We never move out the entry.
        let entry = unsafe {
            let listener = listener.as_mut().get_unchecked_mut();

            match listener.entry {
                None => return RegisterResult::NoTask,
                Some(entry) => &*entry.get(),
            }
        };

        // Take out the state and check it.
        match entry.state.replace(State::Done) {
            State::Ready(tag) => {
                // We have been notified, remove the listener and return the tag.
                self.remove(listener, false);
                RegisterResult::Notified(tag)
            }

            State::Listening(other_task) => {
                // Try replacing the task.
                entry.state.set(State::Listening({
                    if !task.will_wake(&other_task) {
                        task.clone()
                    } else {
                        other_task
                    }
                }));

                RegisterResult::Task
            }

            _ => {
                // We have not been notified, so register the task.
                entry.state.set(State::Listening(task.clone()));
                RegisterResult::Task
            }
        }
    }

    /// Notify the next entry in the table.
    fn notify(&mut self, mut n: usize, mut generator: impl FnMut() -> T) -> usize {
        let mut count = 0;

        while n > 0 {
            n -= 1;

            // Notify the next entry.
            match self.start {
                None => return count,

                Some(e) => {
                    // Get the entry and update the start.
                    let entry = unsafe { e.as_ref() };
                    self.start = entry.next.get();

                    // Notify the entry.
                    if let State::Listening(task) = entry.state.replace(State::Ready(generator())) {
                        task.wake();
                    }
                }
            }

            count += 1;
        }

        count
    }
}

enum RegisterResult<T> {
    /// No task was registered.
    NoTask,

    /// A task was registered.
    Task,

    /// We were notified.
    Notified(T),
}

struct Entry<T> {
    /// State of the entry.
    state: Cell<State<T>>,

    /// The next entry in the linked list.
    next: Cell<Option<NonNull<Entry<T>>>>,

    /// The previous entry in the linked list.
    prev: Cell<Option<NonNull<Entry<T>>>>,
}

enum State<T> {
    /// Listener was just created.
    Created,

    /// Listener is waiting for an event.
    Listening(Waker),

    /// Listener is ready to be polled.
    Ready(T),

    /// Listener is done.
    Done,
}

impl<T> State<T> {
    fn is_ready(&self) -> bool {
        matches!(self, State::Ready(_) | State::Done)
    }
}
