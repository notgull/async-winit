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

use crate::reactor::Reactor;
pub(crate) use __private::__ThreadSafety;

use core::cell::{Cell, RefCell, RefMut};
use core::convert::Infallible;
use core::ops::Add;

use std::collections::VecDeque;
use std::sync::atomic;
use std::thread;

use unsend::channel as us_channel;

pub(crate) mod prelude {
    pub use super::__private::{Atomic, Mutex, OnceLock};
}

/// A token that can be used to indicate whether the current implementation should be thread-safe or
/// not.
pub trait ThreadSafety: __ThreadSafety {}

/// Use thread-unsafe primitives.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThreadUnsafe {
    _private: (),
}

impl ThreadSafety for ThreadUnsafe {}

impl __ThreadSafety for ThreadUnsafe {
    type Error = Infallible;

    type AtomicUsize = Cell<usize>;
    type AtomicU64 = Cell<u64>;
    type AtomicI64 = Cell<i64>;

    type Receiver<T> = us_channel::Receiver<T>;
    type Sender<T> = us_channel::Sender<T>;
    type Rc<T> = std::rc::Rc<T>;

    type ConcurrentQueue<T> = RefCell<VecDeque<T>>;
    type Mutex<T> = RefCell<T>;
    type OnceLock<T> = once_cell::unsync::OnceCell<T>;

    fn channel_bounded<T>(_capacity: usize) -> (Self::Sender<T>, Self::Receiver<T>) {
        us_channel::channel()
    }

    fn get_reactor() -> Self::Rc<Reactor<Self>> {
        use once_cell::sync::OnceCell;

        /// The thread ID of the thread that created the reactor.
        static REACTOR_THREAD_ID: OnceCell<thread::ThreadId> = OnceCell::new();

        std::thread_local! {
            static REACTOR: RefCell<Option<std::rc::Rc<Reactor<ThreadUnsafe>>>> = RefCell::new(None);
        }

        // Try to set the thread ID.
        let thread_id = thread_id();
        let reactor_thread_id = REACTOR_THREAD_ID.get_or_init(|| thread_id);

        if thread_id != *reactor_thread_id {
            panic!("The reactor must be created on the main thread");
        }

        REACTOR
            .try_with(|reactor| {
                reactor
                    .borrow_mut()
                    .get_or_insert_with(|| std::rc::Rc::new(Reactor::new()))
                    .clone()
            })
            .unwrap_or_else(|_| {
                // We're in a destructor
                panic!("The reactor must be created on the main thread");
            })
    }
}

fn thread_id() -> thread::ThreadId {
    // Get the address of a thread-local variable.
    std::thread_local! {
        static THREAD_ID: Cell<Option<thread::ThreadId>> = Cell::new(None);
    }

    THREAD_ID
        .try_with(|thread_id| {
            thread_id.get().unwrap_or_else(|| {
                let id = thread::current().id();
                thread_id.set(Some(id));
                id
            })
        })
        .unwrap_or_else(|_| {
            // We're in a destructor
            thread::current().id()
        })
}

impl<T: Copy> __private::Atomic<T> for Cell<T> {
    fn new(value: T) -> Self {
        Self::new(value)
    }

    fn load(&self, _order: atomic::Ordering) -> T {
        self.get()
    }

    fn store(&self, value: T, _order: atomic::Ordering) {
        self.set(value);
    }

    fn fetch_add(&self, value: T, _order: atomic::Ordering) -> T
    where
        T: Add<Output = T>,
    {
        let old = self.get();
        self.set(old + value);
        old
    }
}

impl<T> __private::Sender<T> for us_channel::Sender<T> {
    type Send<'a> = core::future::Ready<()> where Self: 'a;

    fn send(&self, value: T) -> Self::Send<'_> {
        self.send(value).ok();
        core::future::ready(())
    }
}

impl<T> __private::Receiver<T> for us_channel::Receiver<T> {
    fn capacity(&self) -> usize {
        usize::MAX
    }

    fn try_recv(&self) -> Option<T> {
        self.try_recv().ok()
    }
}

impl<T> __private::ConcurrentQueue<T> for RefCell<VecDeque<T>> {
    type TryIter<'a> = TryIter<'a, T> where Self: 'a;

    fn bounded(capacity: usize) -> Self {
        Self::new(VecDeque::with_capacity(capacity))
    }

    fn push(&self, value: T) -> Result<(), T> {
        self.borrow_mut().push_back(value);
        Ok(())
    }

    fn pop(&self) -> Option<T> {
        self.borrow_mut().pop_front()
    }

    fn capacity(&self) -> usize {
        usize::MAX
    }

    fn try_iter(&self) -> Self::TryIter<'_> {
        TryIter { queue: self }
    }
}

#[doc(hidden)]
pub struct TryIter<'a, T> {
    queue: &'a RefCell<VecDeque<T>>,
}

impl<'a, T> Iterator for TryIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.queue.borrow_mut().pop_front()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.queue.borrow().len();
        (len, Some(len))
    }
}

impl<T> __private::Mutex<T> for RefCell<T> {
    type Error = Infallible;
    type Lock<'a> = RefMut<'a, T> where Self: 'a;

    fn new(value: T) -> Self {
        Self::new(value)
    }

    fn lock(&self) -> Result<Self::Lock<'_>, Self::Error> {
        Ok(self.borrow_mut())
    }
}

impl<T> __private::OnceLock<T> for once_cell::unsync::OnceCell<T> {
    fn new() -> Self {
        Self::new()
    }

    fn get(&self) -> Option<&T> {
        self.get()
    }

    fn set(&self, value: T) -> Result<(), T> {
        self.set(value)
    }
}

impl<T> __private::Rc<T> for std::rc::Rc<T> {
    fn new(value: T) -> Self {
        Self::new(value)
    }
}

pub(crate) mod __private {
    use core::fmt::{Debug, Display};
    use core::future::Future;
    use core::ops::{Add, Deref, DerefMut};
    use core::sync::atomic;

    #[doc(hidden)]
    pub trait __ThreadSafety: Sized {
        type Error: Display + Debug;

        type AtomicUsize: Atomic<usize>;
        type AtomicU64: Atomic<u64>;
        type AtomicI64: Atomic<i64>;

        type Sender<T>: Sender<T>;
        type Receiver<T>: Receiver<T>;

        type ConcurrentQueue<T>: ConcurrentQueue<T>;
        type Mutex<T>: Mutex<T>;
        type OnceLock<T>: OnceLock<T>;
        type Rc<T>: Rc<T>;

        fn channel_bounded<T>(capacity: usize) -> (Self::Sender<T>, Self::Receiver<T>);
        fn get_reactor() -> Self::Rc<crate::reactor::Reactor<Self>>
        where
            Self: super::ThreadSafety;
    }

    #[doc(hidden)]
    pub trait Atomic<T> {
        fn new(value: T) -> Self;
        fn load(&self, order: atomic::Ordering) -> T;
        fn store(&self, value: T, order: atomic::Ordering);
        fn fetch_add(&self, value: T, order: atomic::Ordering) -> T
        where
            T: Add<Output = T>;
    }

    #[doc(hidden)]
    pub trait Sender<T> {
        type Send<'a>: Future<Output = ()> + 'a
        where
            Self: 'a;
        fn send(&self, value: T) -> Self::Send<'_>;
    }

    #[doc(hidden)]
    pub trait Receiver<T> {
        fn capacity(&self) -> usize;
        fn try_recv(&self) -> Option<T>;
    }

    #[doc(hidden)]
    pub trait OnceLock<T> {
        fn new() -> Self;
        fn get(&self) -> Option<&T>;
        fn set(&self, value: T) -> Result<(), T>;
    }

    #[doc(hidden)]
    pub trait Mutex<T> {
        type Error: Debug + Display;
        type Lock<'a>: DerefMut<Target = T> + 'a
        where
            Self: 'a;

        fn new(value: T) -> Self;
        fn lock(&self) -> Result<Self::Lock<'_>, Self::Error>;
    }

    #[doc(hidden)]
    pub trait ConcurrentQueue<T> {
        type TryIter<'a>: Iterator<Item = T> + 'a
        where
            Self: 'a;

        fn bounded(capacity: usize) -> Self;
        fn push(&self, value: T) -> Result<(), T>;
        fn pop(&self) -> Option<T>;
        fn capacity(&self) -> usize;
        fn try_iter(&self) -> Self::TryIter<'_>;
    }

    #[doc(hidden)]
    pub trait Rc<T>: Clone + Deref<Target = T> {
        fn new(value: T) -> Self;
    }
}
