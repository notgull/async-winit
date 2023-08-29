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

//! Filters, or the mechanism used internally by the event loop.
//!
//! This module is exposed such that it is possible to integrate `async-winit` easily with existing
//! `winit` applications. The `Filter` type can be provided events, and will send those events to this
//! library's event handlers.

use std::cmp;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Instant;

use futures_lite::prelude::*;
use parking::Parker;

use crate::event_loop::Wakeup;
use crate::reactor::Reactor;
use crate::sync::ThreadSafety;

use winit::event::Event;
use winit::event_loop::{ControlFlow, EventLoop, EventLoopProxy, EventLoopWindowTarget};

/// Either a function returned, or an associated future returned first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReturnOrFinish<O, T> {
    /// The function returned.
    Output(O),

    /// The associated future returned first.
    FutureReturned(T),
}

/// The filter for passing events to `async` contexts.
///
/// This type takes events and passes them to the event handlers. It also handles the `async` contexts
/// that are waiting for events.
pub struct Filter<TS: ThreadSafety> {
    /// The deadline to wait until.
    deadline: Option<Instant>,

    /// The wakers to wake up later.
    wakers: Vec<Waker>,

    /// The parker to use for posting.
    parker: Parker,

    /// The notifier.
    notifier: Arc<ReactorWaker>,

    /// The waker version of `notifier`.
    ///
    /// Keeping it cached like this is more efficient than creating a new waker every time.
    notifier_waker: Waker,

    /// A waker connected to `parker`.
    ///
    /// Again, it is more efficient to keep it cached like this.
    parker_waker: Waker,

    /// The future has indicated that it wants to yield.
    yielding: bool,

    /// The reactor.
    reactor: TS::Rc<Reactor<TS>>,
}

impl<TS: ThreadSafety> Filter<TS> {
    /// Create a new filter from an event loop.
    ///
    /// The future is polled once before returning to set up event handlers.
    pub fn new(inner: &EventLoop<Wakeup>) -> Filter<TS> {
        let reactor = Reactor::<TS>::get();

        // Create a waker to wake us up.
        let notifier = Arc::new(ReactorWaker {
            proxy: Mutex::new(inner.create_proxy()),
            notified: AtomicBool::new(true),
            awake: AtomicBool::new(false),
        });
        let notifier_waker = Waker::from(notifier.clone());
        reactor.set_proxy(notifier.clone());

        // Parker/unparker pair.
        let (parker, unparker) = parking::pair();
        let parker_waker = Waker::from(Arc::new(EventPostWaker {
            reactor_waker: notifier.clone(),
            unparker,
        }));

        Filter {
            deadline: None,
            wakers: vec![],
            parker,
            notifier,
            notifier_waker,
            parker_waker,
            yielding: false,
            reactor,
        }
    }

    /// Handle an event.
    ///
    /// This function will block on the future if it is in the holding pattern.
    pub fn handle_event<F>(
        &mut self,
        future: Pin<&mut F>,
        event: Event<'_, Wakeup>,
        elwt: &EventLoopWindowTarget<Wakeup>,
        flow: &mut ControlFlow,
    ) -> ReturnOrFinish<(), F::Output>
    where
        F: Future,
    {
        // Create a future that can be polled freely.
        let mut output = ReturnOrFinish::Output(());
        let future = {
            let output = &mut output;
            async move {
                *output = ReturnOrFinish::FutureReturned(future.await);
            }
        };
        futures_lite::pin!(future);

        // Some events have special meanings.
        let about_to_sleep = match &event {
            Event::NewEvents(_) => {
                // Stop yielding now.
                self.yielding = false;

                // We were previously asleep and are now awake.
                self.notifier.awake.store(true, Ordering::SeqCst);

                // Figure out how long we should wait for.
                self.deadline = self.reactor.process_timers(&mut self.wakers);

                // We are not about to fall asleep.
                false
            }

            Event::RedrawEventsCleared => {
                // We are about to fall asleep, so make sure that the future knows it.
                self.notifier.awake.store(false, Ordering::SeqCst);

                // We are about to fall asleep.
                true
            }

            _ => {
                // We are not about to fall asleep.
                false
            }
        };

        // Notify the reactor with our event.
        let notifier = self.reactor.post_event(event);
        futures_lite::pin!(notifier);

        // Try to poll it once.
        let mut cx = Context::from_waker(&self.parker_waker);
        if notifier.as_mut().poll(&mut cx).is_pending() {
            // We've hit a point where the future is interested, stop yielding.
            self.yielding = false;

            // Poll the future in parallel with the user's future.
            let driver = future.as_mut().or(notifier);
            futures_lite::pin!(driver);

            // Drain the request queue before anything else.
            self.reactor.drain_loop_queue(elwt);

            // Block on the parker/unparker pair.
            loop {
                if let Poll::Ready(()) = driver.as_mut().poll(&mut cx) {
                    break;
                }

                // Drain the incoming queue of requests.
                self.reactor.drain_loop_queue(elwt);

                // Handle timers.
                let deadline = {
                    let current_deadline = self.reactor.process_timers(&mut self.wakers);

                    match (current_deadline, self.deadline) {
                        (None, None) => None,
                        (Some(x), None) | (None, Some(x)) => Some(x),
                        (Some(a), Some(b)) => Some(cmp::min(a, b)),
                    }
                };

                // Wake any wakers that need to be woken.
                for waker in self.wakers.drain(..) {
                    waker.wake();
                }

                // Park the thread until it is notified, or until the timeout.
                match deadline {
                    None => self.parker.park(),
                    Some(deadline) => {
                        self.parker.park_deadline(deadline);
                    }
                }
            }
        }

        // If the future is still notified, we should poll it.
        while !self.yielding && self.notifier.notified.swap(false, Ordering::SeqCst) {
            let mut cx = Context::from_waker(&self.notifier_waker);
            if future.as_mut().poll(&mut cx).is_pending() {
                // If the future is *still* notified, it's probably calling future::yield_now(), which
                // indicates that it wants to stop hogging the event loop. Indicate that we should stop
                // polling it until we get NewEvents.
                if self.notifier.notified.load(Ordering::SeqCst) {
                    self.yielding = true;
                }

                // Drain the incoming queue of requests.
                self.reactor.drain_loop_queue(elwt);
            }
        }

        // Wake everything up if we're about to sleep.
        if about_to_sleep {
            self.reactor.drain_loop_queue(elwt);
            for waker in self.wakers.drain(..) {
                waker.wake();
            }
        }

        // Set the control flow.
        if let Some(code) = self.reactor.exit_requested() {
            // The user wants to exit.
            flow.set_exit_with_code(code);
        } else if self.yielding {
            // The future wants to be polled again as soon as possible.
            flow.set_poll();
        } else if let Some(deadline) = self.deadline {
            // The future wants to be polled again when the deadline is reached.
            flow.set_wait_until(deadline);
        } else {
            // The future wants to poll.
            flow.set_wait();
        }

        // Return the output if any.
        output
    }
}

pub(crate) struct ReactorWaker {
    /// The proxy used to wake up the event loop.
    proxy: Mutex<EventLoopProxy<Wakeup>>,

    /// Whether or not we are already notified.
    notified: AtomicBool,

    /// Whether or not the reactor is awake.
    ///
    /// The reactor is awake when we don't
    awake: AtomicBool,
}

impl ReactorWaker {
    pub(crate) fn notify(&self) {
        // If we are already notified, don't notify again.
        if self.notified.swap(true, Ordering::SeqCst) {
            return;
        }

        // If we are currently polling the event loop, don't notify.
        if self.awake.load(Ordering::SeqCst) {
            return;
        }

        // Wake up the reactor.
        self.proxy
            .lock()
            .unwrap()
            .send_event(Wakeup { _private: () })
            .ok();
    }
}

impl Wake for ReactorWaker {
    fn wake(self: Arc<Self>) {
        self.notify()
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.notify()
    }
}

struct EventPostWaker {
    /// The underlying reactor waker.
    reactor_waker: Arc<ReactorWaker>,

    /// The unparker for the notifier.
    unparker: parking::Unparker,
}

impl Wake for EventPostWaker {
    fn wake(self: Arc<Self>) {
        self.reactor_waker.notify();
        self.unparker.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.reactor_waker.notify();
        self.unparker.unpark();
    }
}
