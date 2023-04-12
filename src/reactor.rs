//! The shared reactor used by the runtime.

use crate::event_loop::Message;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::task::Waker;
use std::time::{Duration, Instant};

use concurrent_queue::ConcurrentQueue;
use once_cell::sync::OnceCell;
use winit::event_loop::EventLoopProxy;

pub(crate) struct Reactor {
    /// Whether the event loop is currently processing user events.
    ///
    /// If this is `true`, waking the event loop is unnecessary.
    processing_user_events: AtomicBool,

    /// Begin exiting the event loop.
    exit_requested: AtomicBool,

    /// The event loop proxy.
    ///
    /// Used to wake up the event loop.
    proxy: Mutex<Option<Box<dyn Proxy + Send + 'static>>>,

    /// The timer wheel.
    timers: Mutex<BTreeMap<(Instant, usize), Waker>>,

    /// Queue of timer operations.
    timer_op_queue: ConcurrentQueue<TimerOp>,

    /// The last timer ID we used.
    timer_id: AtomicUsize,
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
            processing_user_events: AtomicBool::new(false),
            exit_requested: AtomicBool::new(false),
            proxy: Mutex::new(None),
            timers: BTreeMap::new().into(),
            timer_op_queue: ConcurrentQueue::bounded(1024),
            timer_id: AtomicUsize::new(1),
        })
    }

    /// Set the event loop proxy.
    pub(crate) fn set_proxy(&self, proxy: impl Proxy + Send + 'static) {
        *self.proxy.lock().unwrap() = Some(Box::new(proxy));
    }

    /// Get whether or not we need to exit.
    pub(crate) fn exit_requested(&self) -> bool {
        self.exit_requested.load(Ordering::SeqCst)
    }

    /// Request that the event loop exit.
    pub(crate) fn request_exit(&self) {
        self.exit_requested.store(true, Ordering::SeqCst);
    }

    /// Insert a new timer into the timer wheel.
    pub(crate) fn insert_timer(&self, deadline: Instant, waker: &Waker) -> usize {
        // Generate a new ID.
        let id = self.timer_id.fetch_add(1, Ordering::Relaxed);

        // Insert the timer into the timer wheel.
        let mut op = TimerOp::InsertTimer(deadline, id, waker.clone());
        while let Err(e) = self.timer_op_queue.push(op) {
            // Process incoming timer operations.
            let mut timers = self.timers.lock().unwrap();
            self.process_timer_ops(&mut timers);
            op = e.into_inner();
        }

        // Notify that we have new timers.
        self.notify();

        // Return the ID.
        id
    }

    /// Remove a timer from the timer wheel.
    pub(crate) fn remove_timer(&self, deadline: Instant, id: usize) {
        let mut op = TimerOp::RemoveTimer(deadline, id);
        while let Err(e) = self.timer_op_queue.push(op) {
            // Process incoming timer operations.
            let mut timers = self.timers.lock().unwrap();
            self.process_timer_ops(&mut timers);
            op = e.into_inner();
        }
    }

    /// Process pending timer operations.
    fn process_timer_ops(&self, timers: &mut BTreeMap<(Instant, usize), Waker>) {
        // Limit the number of operations we process at once to avoid starving other tasks.
        let limit = self.timer_op_queue.capacity().unwrap();

        self.timer_op_queue
            .try_iter()
            .take(limit)
            .for_each(|op| match op {
                TimerOp::InsertTimer(deadline, id, waker) => {
                    timers.insert((deadline, id), waker);
                }
                TimerOp::RemoveTimer(deadline, id) => {
                    if let Some(waker) = timers.remove(&(deadline, id)) {
                        // Don't let a waker that panics on drop blow everything up.
                        std::panic::catch_unwind(|| drop(waker)).ok();
                    }
                }
            });
    }

    /// Process timers and return the amount of time to wait.
    pub(crate) fn process_timers(&self, wakers: &mut Vec<Waker>) -> Option<Duration> {
        // Process incoming timer operations.
        let mut timers = self.timers.lock().unwrap();
        self.process_timer_ops(&mut timers);

        let now = Instant::now();

        // Split timers into pending and ready timers.
        let pending = timers.split_off(&(now + Duration::from_nanos(1), 0));
        let ready = std::mem::replace(&mut *timers, pending);

        // Figure out how long it will be until the next timer is ready.
        let timeout = if ready.is_empty() {
            timers
                .keys()
                .next()
                .map(|(deadline, _)| deadline.saturating_duration_since(now))
        } else {
            // There are timers ready to fire now.
            Some(Duration::ZERO)
        };

        drop(timers);

        // Push wakers for ready timers.
        wakers.extend(ready.into_values());

        timeout
    }

    /// Wake up the event loop.
    pub(crate) fn notify(&self) {
        if let Some(proxy) = self.proxy.lock().unwrap().as_ref() {
            proxy.notify();
        }
    }
}

/// Trait used to abstract over the different event loop types.
pub(crate) trait Proxy {
    /// Notify the proxy with a wake-up.
    fn notify(&self);
}

impl<T: 'static> Proxy for EventLoopProxy<Message<T>> {
    fn notify(&self) {
        self.send_event(Message::Wakeup)
            .ok()
            .expect("failed to wake up event loop");
    }
}
