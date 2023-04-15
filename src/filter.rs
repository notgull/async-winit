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

//! Filters, or the mechanism used internally by the event loop.

use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

use futures_lite::prelude::*;
use parking::Parker;

use crate::event_loop::Wakeup;
use crate::reactor::Reactor;

use winit::event::Event;
use winit::event_loop::{ControlFlow, EventLoop, EventLoopProxy, EventLoopWindowTarget};

pub enum ReturnOrFinish<O, T> {
    Output(O),
    FutureReturned(T),
}

pub struct Filter {
    /// The timeout to wait for.
    timeout: Option<Duration>,

    /// The wakers to wake up later.
    wakers: Vec<Waker>,

    /// The parker to use.
    parker: Parker,

    /// The notifier.
    notifier: Arc<ReactorWaker>,

    /// The waker version of `notifier`.
    notifier_waker: Waker,

    /// A holding pattern waker.
    holding_waker: Waker,

    /// The reactor.
    reactor: &'static Reactor,
}

impl Filter {
    pub fn new<F>(
        inner: &EventLoop<Wakeup>,
        future: Pin<&mut F>,
    ) -> ReturnOrFinish<Filter, F::Output>
    where
        F: Future,
    {
        let reactor = Reactor::get();

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

        // Create another waker to hold us in the holding pattern.
        let holding_waker = Waker::from(Arc::new(HoldingPattern {
            reactor_waker: notifier.clone(),
            unparker,
        }));

        // We have to allocate the future on the heap to make it movable.
        let mut future = Box::pin(future);

        let mut cx = Context::from_waker(&notifier_waker);
        if let Poll::Ready(i) = future.as_mut().poll(&mut cx) {
            return ReturnOrFinish::FutureReturned(i);
        }

        ReturnOrFinish::Output(Filter {
            timeout: None,
            wakers: vec![],
            parker,
            notifier,
            notifier_waker,
            holding_waker,
            reactor,
        })
    }

    /// Handle an event.
    pub fn handle_event<F>(
        &mut self,
        mut future: Pin<&mut F>,
        event: Event<'_, Wakeup>,
        elwt: &EventLoopWindowTarget<Wakeup>,
        flow: &mut ControlFlow,
    ) -> ReturnOrFinish<(), F::Output>
    where
        F: Future,
    {
        let output = Cell::new(None);

        // Function for blocking on holding.
        macro_rules! block_on {
            ($fut:expr) => {{
                let fut = $fut;
                futures_lite::pin!(fut);
                let mut cx = Context::from_waker(&self.holding_waker);

                loop {
                    self.notifier.awake.store(true, Ordering::SeqCst);

                    // Drain the incoming queue of requests.
                    // TODO: Poll timers as well?
                    self.reactor.drain_loop_queue(elwt);

                    if let Poll::Ready(i) = fut.as_mut().poll(&mut cx) {
                        if let Some(result) = output.take() {
                            return ReturnOrFinish::FutureReturned(result);
                        }

                        break i;
                    }

                    // Drain the incoming queue of requests.
                    // TODO: Poll timers as well?
                    self.reactor.drain_loop_queue(elwt);

                    self.notifier.awake.store(false, Ordering::SeqCst);
                    self.parker.park();
                }
            }};
        }

        let mut falling_asleep = false;

        match &event {
            Event::NewEvents(_) => {
                // We are now awake.
                self.notifier.awake.store(true, Ordering::SeqCst);

                // Figure out how long we should wait for.
                self.timeout = self.reactor.process_timers(&mut self.wakers);
            }

            Event::MainEventsCleared => {
                falling_asleep = true;
            }

            _ => {}
        }

        if falling_asleep {
            for waker in self.wakers.drain(..) {
                // Don't let a panicking waker blow everything up.
                std::panic::catch_unwind(|| waker.wake()).ok();
            }
        }

        // Post the event, block on it and poll the future at the same time.
        let posting = self.reactor.post_event(event).or({
            let future = future.as_mut();
            let output = &output;

            async move {
                output.set(Some(future.await));
            }
        });

        block_on!(posting);

        if falling_asleep {
            // Enter the sleeping state.
            self.notifier.awake.store(false, Ordering::SeqCst);
        }

        // Check the notification.
        if self.notifier.notified.swap(false, Ordering::SeqCst) {
            // We were notified, so we should poll the future.
            let mut cx = Context::from_waker(&self.notifier_waker);
            if let Poll::Ready(i) = future.poll(&mut cx) {
                return ReturnOrFinish::FutureReturned(i);
            }
        }

        // Set the control flow.
        if self.reactor.exit_requested() {
            flow.set_exit();
        } else {
            match self.timeout {
                Some(timeout) => flow.set_wait_timeout(timeout),
                None => flow.set_wait(),
            }
        }

        ReturnOrFinish::Output(())
    }
}

pub(crate) struct ReactorWaker {
    /// The proxy used to wake up the event loop.
    proxy: Mutex<EventLoopProxy<Wakeup>>,

    /// Whether or not we are already notified.
    notified: AtomicBool,

    /// Whether or not the reactor is awake.
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
        self.proxy.lock().unwrap().send_event(Wakeup).ok();
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

struct HoldingPattern {
    reactor_waker: Arc<ReactorWaker>,
    unparker: parking::Unparker,
}

impl Wake for HoldingPattern {
    fn wake(self: Arc<Self>) {
        self.reactor_waker.notify();
        self.unparker.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.reactor_waker.notify();
        self.unparker.unpark();
    }
}
