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

//! The [`EventLoop`] and associated structures.
//!
//! There are three main differences between [`EventLoop`]s here and in [`winit`]:
//!
//! - Instead of `run` or `run_return`, there are `block_on` and `block_on_return`, which take a future
//!   and run it to completion. Eent handling is done through the [`Handler`] structures instead.
//! - Methods on [`EventLoop`] and [`EventLoopWindowTarget`] are `async`.
//! - There is no `EventLoopProxy` type, since it is now obsolete with `async` blocks. Instead,
//!   consider using an async channel to communicate with the event loop.
//!
//! ```no_run
//! use async_winit::event_loop::EventLoop;
//! use async_winit::ThreadUnsafe;
//!
//! struct MyCustomType;
//!
//! let (sender, receiver) = async_channel::unbounded();
//!
//! EventLoop::<ThreadUnsafe>::new().block_on(async move {
//!     // Wait for a message from the channel.
//!     let message = receiver.recv().await.unwrap();
//! #   futures_lite::future::pending().await
//! });
//!
//! // In another thread, send a message to the event loop.
//! # futures_lite::future::block_on(async move {
//! sender.send(MyCustomType).await.unwrap();
//! # });
//! ```
//!
//! [`Handler`]: crate::Handler

use crate::filter::ReturnOrFinish;
use crate::handler::Handler;
use crate::reactor::{EventLoopOp, Reactor};
use crate::sync::ThreadSafety;

use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::ops;

use raw_window_handle::{HasRawDisplayHandle, RawDisplayHandle};
use winit::event_loop::EventLoopProxy;

#[doc(inline)]
pub use winit::event_loop::{ControlFlow, DeviceEventFilter, EventLoopClosed};

/// Used to indicate that we need to wake up the event loop.
///
/// This is a ZST used by the underlying event loop to wake up the event loop. It is not used
/// directly by the user.
///
/// It is public because it is used by the [`Filter`] type. Generally, you don't need to use it.
///
/// [`Filter`]: crate::filter::Filter
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Wakeup {
    pub(crate) _private: (),
}

/// Provides a way to retrieve events from the system and from the windows that were registered to
/// the events loop.
///
/// The [`EventLoop`] is a "context" for the GUI system. More specifically, it represents a connection
/// to the underlying GUI system. The [`EventLoop`] is the main object that you will use to drive
/// the program. Most `async` functions in `async-winit` rely on the [`EventLoop`] to be currently
/// running.
///
/// The [`EventLoop`] itself is `!Send` and `!Sync` due to underlying platform restrictions. However,
/// [`EventLoopWindowTarget`]` and [`Window`] are both not only `Send` and `Sync`, but also cheaply
/// clonable. This means that you can create a window on one thread, and then send it to another
/// thread to be used.
///
/// [`Window`]: crate::window::Window
pub struct EventLoop<TS: ThreadSafety> {
    /// The underlying event loop.
    pub(crate) inner: winit::event_loop::EventLoop<Wakeup>,

    /// The window target.
    window_target: EventLoopWindowTarget<TS>,
}

impl<TS: ThreadSafety> fmt::Debug for EventLoop<TS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EventLoop { .. }")
    }
}

/// A reference to the `EventLoop` that allows the user access to the underlying display connections.
///
/// Unlike in `winit`, this type is cheaply clonable. It is not actually used that often, since most of
/// its previous use cases don't directly require the window target to be passed in. However, it is
/// still useful for some things, like indicating the need to exit the application or getting
/// available monitors.
pub struct EventLoopWindowTarget<TS: ThreadSafety> {
    /// The associated reactor, cached for convenience.
    reactor: TS::Rc<Reactor<TS>>,

    /// The event loop proxy.
    proxy: EventLoopProxy<Wakeup>,

    /// The raw display handle.
    raw_display_handle: RawDisplayHandle,

    /// Is this using wayland?
    #[cfg(any(x11_platform, wayland_platform))]
    pub(crate) is_wayland: bool,
}

impl<TS: ThreadSafety> fmt::Debug for EventLoopWindowTarget<TS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EventLoopWindowTarget { .. }")
    }
}

impl<TS: ThreadSafety> Clone for EventLoopWindowTarget<TS> {
    fn clone(&self) -> Self {
        Self {
            reactor: self.reactor.clone(),
            proxy: self.proxy.clone(),
            raw_display_handle: self.raw_display_handle,
            #[cfg(any(x11_platform, wayland_platform))]
            is_wayland: self.is_wayland,
        }
    }
}

/// Object that allows for building the [`EventLoop`].
///
/// This specifies options that affect the whole application, like the current Android app or whether
/// to use the Wayland backend. You cannot create more than one [`EventLoop`] per application.
pub struct EventLoopBuilder {
    /// The underlying builder.
    pub(crate) inner: winit::event_loop::EventLoopBuilder<Wakeup>,
}

impl fmt::Debug for EventLoopBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EventLoopBuilder { .. }")
    }
}

impl EventLoopBuilder {
    /// Create a new [`EventLoopBuilder`].
    pub fn new() -> Self {
        Self {
            inner: winit::event_loop::EventLoopBuilder::with_user_event(),
        }
    }

    /// Builds a new event loop.
    ///
    /// In general, this function must be called on the same thread that `main()` is being run inside of.
    /// This can be circumvented in some cases using platform specific options. See the [`platform`]
    /// module for more information. Attempting to violate this property or create more than one event
    /// loop per application will result in a panic.
    ///
    /// This function results in platform-specific backend initialization.
    ///
    /// [`platform`]: crate::platform
    pub fn build<TS: ThreadSafety>(&mut self) -> EventLoop<TS> {
        let inner = self.inner.build();
        EventLoop {
            window_target: EventLoopWindowTarget {
                reactor: Reactor::<TS>::get(),
                proxy: inner.create_proxy(),
                raw_display_handle: inner.raw_display_handle(),
                #[cfg(any(x11_platform, wayland_platform))]
                is_wayland: {
                    cfg_if::cfg_if! {
                        if #[cfg(feature = "x11")] {
                            use winit::platform::x11::EventLoopWindowTargetExtX11;
                            !inner.is_x11()
                        } else if #[cfg(feature = "wayland")] {
                            use winit::platform::wayland::EventLoopWindowTargetExtWayland;
                            inner.is_wayland()
                        } else {
                            false
                        }
                    }
                },
            },
            inner,
        }
    }
}

impl Default for EventLoopBuilder {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<TS: ThreadSafety> HasRawDisplayHandle for EventLoop<TS> {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        self.window_target.raw_display_handle
    }
}

impl<TS: ThreadSafety> EventLoop<TS> {
    /// Alias for [`EventLoopBuilder::new().build()`].
    ///
    /// [`EventLoopBuilder::new().build()`]: EventLoopBuilder::build
    #[inline]
    pub fn new() -> EventLoop<TS> {
        EventLoopBuilder::new().build()
    }
}

impl<TS: ThreadSafety> Default for EventLoop<TS> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<TS: ThreadSafety> EventLoopWindowTarget<TS> {
    /// Request that the event loop exit as soon as possible.
    #[inline]
    pub fn set_exit(&self) {
        self.reactor.request_exit(0);
    }

    /// Request that we exit as soon as possible with the given exit code.
    #[inline]
    pub fn set_exit_with_code(&self, code: i32) {
        self.reactor.request_exit(code);
    }

    /// Exit the program.
    #[inline]
    pub async fn exit(&self) -> ! {
        self.set_exit();
        futures_lite::future::pending().await
    }

    /// Exit the program with the given exit code.
    #[inline]
    pub async fn exit_with_code(&self, code: i32) -> ! {
        self.set_exit_with_code(code);
        futures_lite::future::pending().await
    }

    /// Get the handler for the `Resumed` event.
    #[inline]
    pub fn resumed(&self) -> &Handler<()> {
        &self.reactor.evl_registration.resumed
    }

    /// Get the handler for the `Suspended` event.
    #[inline]
    pub fn suspended(&self) -> &Handler<()> {
        &self.reactor.evl_registration.suspended
    }

    /// Get the primary monitor.
    #[inline]
    pub async fn primary_monitor(&self) -> Option<winit::monitor::MonitorHandle> {
        let (tx, rx) = crate::oneoff::oneoff();
        self.reactor
            .push_event_loop_op(EventLoopOp::PrimaryMonitor(tx))
            .await;
        rx.recv().await
    }

    /// Get the available monitors.
    #[inline]
    pub async fn available_monitors(&self) -> impl Iterator<Item = winit::monitor::MonitorHandle> {
        let (tx, rx) = crate::oneoff::oneoff();
        self.reactor
            .push_event_loop_op(EventLoopOp::AvailableMonitors(tx))
            .await;
        rx.recv().await.into_iter()
    }

    /// Set the device event filter.
    #[inline]
    pub async fn set_device_event_filter(&self, filter: DeviceEventFilter) {
        let (tx, rx) = crate::oneoff::oneoff();
        self.reactor
            .push_event_loop_op(EventLoopOp::SetDeviceFilter { filter, waker: tx })
            .await;

        // Wait for the filter to be set.
        rx.recv().await;
    }
}

unsafe impl<TS: ThreadSafety> HasRawDisplayHandle for EventLoopWindowTarget<TS> {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        self.raw_display_handle
    }
}

impl<TS: ThreadSafety + 'static> EventLoop<TS> {
    /// Manually get a reference to the event loop's window target.
    #[inline]
    pub fn window_target(&self) -> &EventLoopWindowTarget<TS> {
        &self.window_target
    }

    /// Block on a future forever.
    #[inline]
    pub fn block_on(self, future: impl Future<Output = Infallible> + 'static) -> ! {
        let inner = self.inner;

        let mut future = Box::pin(future);
        let mut filter = match crate::filter::Filter::<TS>::new(&inner, future.as_mut()) {
            ReturnOrFinish::FutureReturned(i) => match i {},
            ReturnOrFinish::Output(f) => f,
        };

        inner.run(move |event, elwt, flow| {
            filter.handle_event(future.as_mut(), event, elwt, flow);
        })
    }
}

impl<TS: ThreadSafety> ops::Deref for EventLoop<TS> {
    type Target = EventLoopWindowTarget<TS>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.window_target
    }
}

impl<TS: ThreadSafety> ops::DerefMut for EventLoop<TS> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.window_target
    }
}
