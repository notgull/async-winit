//! Asynchronous timers.

use crate::reactor::Reactor;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use futures_lite::stream::Stream;

/// A future or stream that emits timer events.
pub struct Timer {
    /// Static reference to the reactor.
    reactor: &'static Reactor,

    /// This timer's ID and the last waker that polled it.
    id_and_waker: Option<(usize, Waker)>,

    /// The time at which this timer will fire.
    deadline: Option<Instant>,

    /// The period.
    period: Duration,
}

impl Timer {
    /// Create a new timer that will never fire.
    pub fn never() -> Self {
        Self {
            reactor: Reactor::get(),
            id_and_waker: None,
            deadline: None,
            period: Duration::MAX,
        }
    }

    /// Create a timer that fires after the given duration.
    pub fn after(duration: Duration) -> Self {
        Instant::now()
            .checked_add(duration)
            .map_or_else(Self::never, Self::at)
    }

    /// Create a timer that fires at the given time.
    pub fn at(deadline: Instant) -> Self {
        Self::interval_at(deadline, Duration::MAX)
    }

    /// Create a timer that fires on an interval.
    pub fn interval(period: Duration) -> Self {
        Instant::now()
            .checked_add(period)
            .map_or_else(Self::never, |deadline| Self::interval_at(deadline, period))
    }

    /// Create a timer that fires on an interval starting at the given time.
    pub fn interval_at(start: Instant, period: Duration) -> Self {
        Self {
            reactor: Reactor::get(),
            id_and_waker: None,
            deadline: Some(start),
            period,
        }
    }

    fn clear(&mut self) {
        if let (Some(deadline), Some((id, _))) = (self.deadline.take(), self.id_and_waker.take()) {
            self.reactor.remove_timer(deadline, id);
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.clear();
    }
}
