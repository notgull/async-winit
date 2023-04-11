//! The shared reactor used by the runtime.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::task::Waker;
use std::time::Instant;

use concurrent_queue::ConcurrentQueue;
use once_cell::sync::OnceCell;

pub(crate) struct Reactor {
    /// The timer wheel.
    timers: Mutex<BTreeMap<(Instant, usize), Waker>>,

    /// Queue of timer operations.
    timer_op_queue: ConcurrentQueue<TimerOp>
}

enum TimerOp {
    /// Add a new timer.
    InsertTimer(Instant, usize, Waker),

    /// Delete an existing timer.
    RemoveTimer(Instant, usize),
}

impl Reactor {
    /// Get the global instance of the `Reactor`.
    ///
    /// Since there can only be one instance of `EventLoop`, we can also have only one instance of a `Reactor`.
    /// If `winit` is ever updated so that `EventLoopBuilder::build()` doesn't panic if it's called more than
    /// once, remove this!
    ///
    /// Relevant winit code:
    /// https://github.com/rust-windowing/winit/blob/2486f0f1a1d00ac9e5936a5222b2cfe90ceeca02/src/event_loop.rs#L114-L117
    pub(crate) fn get() -> &'static Self {
        static REACTOR: OnceCell<Reactor> = OnceCell::new();

        REACTOR.get_or_init(|| Reactor {
            timers: BTreeMap::new().into(),
            timer_op_queue: ConcurrentQueue::bounded(1024),
        })
    }
}
