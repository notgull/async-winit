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
use core::future::Future;
use core::ops::Add;

use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic;
use std::thread;

use unsend::channel as us_channel;

#[cfg(feature = "thread_safe")]
pub use thread_safe::ThreadSafe;

#[cfg(feature = "thread_safe")]
type _DefaultTS = ThreadSafe;
#[cfg(not(feature = "thread_safe"))]
type _DefaultTS = ThreadUnsafe;

/// The default thread safe type to use.
pub type DefaultThreadSafety = _DefaultTS;

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
    type Rc<T> = Rc<T>;

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

        REACTOR.with(|reactor| {
            reactor
                .borrow_mut()
                .get_or_insert_with(|| std::rc::Rc::new(Reactor::new()))
                .clone()
        })
    }
}

pub(crate) type MutexGuard<'a, T, TS> =
    <<TS as __ThreadSafety>::Mutex<T> as __private::Mutex<T>>::Lock<'a>;

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
    type Error = Infallible;
    type Send<'a> = core::future::Ready<Result<(), Self::Error>> where Self: 'a;

    fn send(&self, value: T) -> Self::Send<'_> {
        self.send(value).ok();
        core::future::ready(Ok(()))
    }

    fn try_send(&self, value: T) -> Result<(), Self::Error> {
        self.send(value).ok();
        Ok(())
    }
}

impl<T> __private::Receiver<T> for us_channel::Receiver<T> {
    type Error = ();
    type Recv<'a> = std::pin::Pin<Box<dyn Future<Output = Result<T, Self::Error>> + 'a>> where Self: 'a;

    fn recv(&self) -> Self::Recv<'_> {
        Box::pin(async move { self.recv().await.map_err(|_| ()) })
    }

    fn capacity(&self) -> usize {
        usize::MAX
    }

    fn try_recv(&self) -> Option<T> {
        self.try_recv().ok()
    }

    fn len(&self) -> usize {
        todo!()
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

    fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        self.get_or_init(f)
    }
}

impl<T> __private::Rc<T> for std::rc::Rc<T> {
    fn new(value: T) -> Self {
        Self::new(value)
    }
}

#[cfg(feature = "thread_safe")]
pub(crate) mod thread_safe {
    use super::*;

    use concurrent_queue::ConcurrentQueue;
    use std::sync::atomic;
    use std::sync::{Arc, Mutex};

    /// Use thread-safe primitives.
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ThreadSafe {
        _private: (),
    }

    impl ThreadSafety for ThreadSafe {}

    impl __ThreadSafety for ThreadSafe {
        type Error = Box<dyn std::error::Error + Send + Sync>;

        type AtomicI64 = atomic::AtomicI64;
        type AtomicUsize = atomic::AtomicUsize;
        type AtomicU64 = atomic::AtomicU64;

        type Sender<T> = async_channel::Sender<T>;
        type Receiver<T> = async_channel::Receiver<T>;

        type ConcurrentQueue<T> = ConcurrentQueue<T>;
        type Mutex<T> = Mutex<T>;
        type OnceLock<T> = once_cell::sync::OnceCell<T>;
        type Rc<T> = Arc<T>;

        fn channel_bounded<T>(capacity: usize) -> (Self::Sender<T>, Self::Receiver<T>) {
            async_channel::bounded(capacity)
        }
        fn get_reactor() -> Self::Rc<crate::reactor::Reactor<Self>>
        where
            Self: super::ThreadSafety,
        {
            use once_cell::sync::OnceCell;

            static REACTOR: OnceCell<Arc<Reactor<ThreadSafe>>> = OnceCell::new();

            REACTOR.get_or_init(|| Arc::new(Reactor::new())).clone()
        }
    }

    impl __private::Atomic<i64> for atomic::AtomicI64 {
        fn new(value: i64) -> Self {
            Self::new(value)
        }

        fn fetch_add(&self, value: i64, order: atomic::Ordering) -> i64 {
            self.fetch_add(value, order)
        }

        fn load(&self, order: atomic::Ordering) -> i64 {
            self.load(order)
        }

        fn store(&self, value: i64, order: atomic::Ordering) {
            self.store(value, order)
        }
    }

    impl __private::Atomic<usize> for atomic::AtomicUsize {
        fn new(value: usize) -> Self {
            Self::new(value)
        }

        fn fetch_add(&self, value: usize, order: atomic::Ordering) -> usize {
            self.fetch_add(value, order)
        }

        fn load(&self, order: atomic::Ordering) -> usize {
            self.load(order)
        }

        fn store(&self, value: usize, order: atomic::Ordering) {
            self.store(value, order)
        }
    }

    impl __private::Atomic<u64> for atomic::AtomicU64 {
        fn new(value: u64) -> Self {
            Self::new(value)
        }

        fn fetch_add(&self, value: u64, order: atomic::Ordering) -> u64 {
            self.fetch_add(value, order)
        }

        fn load(&self, order: atomic::Ordering) -> u64 {
            self.load(order)
        }

        fn store(&self, value: u64, order: atomic::Ordering) {
            self.store(value, order)
        }
    }

    impl<T> __private::Sender<T> for async_channel::Sender<T> {
        type Error = async_channel::SendError<T>;
        type Send<'a> = async_channel::Send<'a, T> where Self: 'a;

        fn send(&self, value: T) -> Self::Send<'_> {
            self.send(value)
        }

        fn try_send(&self, value: T) -> Result<(), Self::Error> {
            self.try_send(value).map_err(|_e| todo!())
        }
    }

    impl<T> __private::Receiver<T> for async_channel::Receiver<T> {
        type Error = async_channel::RecvError;
        type Recv<'a> = async_channel::Recv<'a, T> where Self: 'a;

        fn recv(&self) -> Self::Recv<'_> {
            self.recv()
        }

        fn capacity(&self) -> usize {
            self.capacity().unwrap()
        }

        fn try_recv(&self) -> Option<T> {
            self.try_recv().ok()
        }

        fn len(&self) -> usize {
            self.len()
        }
    }

    impl<T> __private::ConcurrentQueue<T> for ConcurrentQueue<T> {
        type TryIter<'a> = concurrent_queue::TryIter<'a, T> where Self: 'a;

        fn bounded(capacity: usize) -> Self {
            Self::bounded(capacity)
        }

        fn push(&self, value: T) -> Result<(), T> {
            self.push(value).map_err(|e| e.into_inner())
        }

        fn pop(&self) -> Option<T> {
            self.pop().ok()
        }

        fn capacity(&self) -> usize {
            self.capacity().unwrap()
        }

        fn try_iter(&self) -> Self::TryIter<'_> {
            self.try_iter()
        }
    }

    impl<T> __private::Mutex<T> for Mutex<T> {
        type Error = Infallible;
        type Lock<'a> = std::sync::MutexGuard<'a, T> where Self: 'a;

        fn new(value: T) -> Self {
            Self::new(value)
        }

        fn lock(&self) -> Result<Self::Lock<'_>, Self::Error> {
            Ok(self.lock().unwrap_or_else(|e| e.into_inner()))
        }
    }

    impl<T> __private::OnceLock<T> for once_cell::sync::OnceCell<T> {
        fn new() -> Self {
            Self::new()
        }

        fn get(&self) -> Option<&T> {
            self.get()
        }

        fn set(&self, value: T) -> Result<(), T> {
            self.set(value)
        }

        fn get_or_init<F>(&self, f: F) -> &T
        where
            F: FnOnce() -> T,
        {
            self.get_or_init(f)
        }
    }

    impl<T> __private::Rc<T> for Arc<T> {
        fn new(value: T) -> Self {
            Self::new(value)
        }
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

        type AtomicI64: Atomic<i64>;
        type AtomicUsize: Atomic<usize>;
        type AtomicU64: Atomic<u64>;

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
        type Error;
        type Send<'a>: Future<Output = Result<(), Self::Error>> + 'a
        where
            Self: 'a;
        fn send(&self, value: T) -> Self::Send<'_>;
        fn try_send(&self, value: T) -> Result<(), Self::Error>;
    }

    #[doc(hidden)]
    pub trait Receiver<T> {
        type Error: std::fmt::Debug;
        type Recv<'a>: Future<Output = Result<T, Self::Error>> + 'a
        where
            Self: 'a;

        fn recv(&self) -> Self::Recv<'_>;
        fn capacity(&self) -> usize;
        fn try_recv(&self) -> Option<T>;
        fn len(&self) -> usize;
    }

    #[doc(hidden)]
    pub trait OnceLock<T> {
        fn new() -> Self;
        fn get(&self) -> Option<&T>;
        fn get_or_init<F>(&self, f: F) -> &T
        where
            F: FnOnce() -> T;
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
