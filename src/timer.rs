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

// This file is partially derived from `async-io`, which was originally created by Stjepan Glavina
// and contributers. It was originally released under the MIT license and Apache 2.0 license.

//! Asynchronous timers.

use crate::reactor::Reactor;

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use futures_lite::stream::Stream;

/// A future or stream that emits timer events.
///
/// This timer waits for a specific duration or interval to elapse before returning `Poll::Ready`.
/// It uses the [`ControlFlow::WaitUntil`] mechanism to wait for the timer to fire.
///
/// This type is similar to the [`Timer`] type in the `async-io` crate. The main practical difference
/// is that, on certain platforms, this `Timer` type may have marginally higher precision.
///
/// [`ControlFlow::WaitUntil`]: crate::event_loop::ControlFlow::WaitUntil
/// [`Timer`]: https://docs.rs/async-io/latest/async_io/timer/struct.Timer.html
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

impl fmt::Debug for Timer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Timer")
            .field("deadline", &self.deadline)
            .field("period", &self.period)
            .field("registered", &self.id_and_waker.is_some())
            .finish()
    }
}

impl Unpin for Timer {}

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

    /// Returns `true` if this timer will eventually return `Poll::Ready`.
    pub fn will_fire(&self) -> bool {
        self.deadline.is_some()
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

    /// Set this timer to never fire.
    pub fn set_never(&mut self) {
        self.clear();
        self.deadline = None;
    }

    /// Set this timer to fire after the given duration.
    pub fn set_after(&mut self, duration: Duration) {
        match Instant::now().checked_add(duration) {
            Some(deadline) => self.set_at(deadline),
            None => self.set_never(),
        }
    }

    /// Set this timer to fire at the given deadline.
    pub fn set_at(&mut self, deadline: Instant) {
        self.set_interval_at(deadline, Duration::MAX)
    }

    /// Set this timer to run at an interval.
    pub fn set_interval(&mut self, period: Duration) {
        match Instant::now().checked_add(period) {
            Some(deadline) => self.set_interval_at(deadline, period),
            None => self.set_never(),
        }
    }

    /// Set this timer to run on an interval starting at the given time.
    pub fn set_interval_at(&mut self, start: Instant, period: Duration) {
        self.clear();

        self.deadline = Some(start);
        self.period = period;

        if let Some((id, waker)) = self.id_and_waker.as_mut() {
            // Re-register the timer into the reactor.
            *id = self.reactor.insert_timer(start, waker);
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

impl Future for Timer {
    type Output = Instant;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_next(cx).map(Option::unwrap)
    }
}

impl Stream for Timer {
    type Item = Instant;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(ref mut deadline) = this.deadline {
            // Check if the timer is ready.
            if *deadline < Instant::now() {
                if let Some((id, _)) = this.id_and_waker.take() {
                    this.reactor.remove_timer(*deadline, id);
                }

                let result_time = *deadline;

                if let Some(next) = deadline.checked_add(this.period) {
                    *deadline = next;

                    // Register the timer into the reactor.
                    let id = this.reactor.insert_timer(next, cx.waker());
                    this.id_and_waker = Some((id, cx.waker().clone()));
                } else {
                    this.deadline = None;
                }

                // Return the time that we fired at.
                return Poll::Ready(Some(result_time));
            } else {
                match &this.id_and_waker {
                    None => {
                        // This timer needs to be registered.
                        let id = this.reactor.insert_timer(*deadline, cx.waker());
                        this.id_and_waker = Some((id, cx.waker().clone()));
                    }

                    Some((id, w)) if !w.will_wake(cx.waker()) => {
                        // Deregister timer and remove the old waker.
                        this.reactor.remove_timer(*deadline, *id);

                        // Register the timer into the reactor.
                        let id = this.reactor.insert_timer(*deadline, cx.waker());
                        this.id_and_waker = Some((id, cx.waker().clone()));
                    }

                    _ => {}
                }
            }
        }

        Poll::Pending
    }
}
