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

// This file is partially derived from `winit`, which was originally created by Pierre Krieger and
// contributers. It was originally released under the MIT license.

//! Platform-specific features for Wayland.

use super::__private as sealed;
use crate::event_loop::{EventLoopBuilder, EventLoopWindowTarget};
use crate::sync::ThreadSafety;
use crate::window::{Window, WindowBuilder};

use std::os::raw;

use winit::platform::wayland::{
    EventLoopBuilderExtWayland as _, WindowBuilderExtWayland as _, WindowExtWayland as _,
};

#[doc(inline)]
pub use winit::platform::wayland::MonitorHandleExtWayland;

/// Additional methods on [`EventLoopWindowTarget`] that are specific to Wayland.
///
/// [`EventLoopWindowTarget`]: crate::event_loop::EventLoopWindowTarget
pub trait EventLoopWindowTargetExtWayland: sealed::EventLoopWindowTargetPrivate {
    /// True if the [`EventLoopWindowTarget`] uses Wayland.
    fn is_wayland(&self) -> bool;

    /// Returns a pointer to the `wl_display` object of wayland that is used by this
    /// [`EventLoopWindowTarget`].
    ///
    /// Returns `None` if the [`EventLoop`] doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the winit [`EventLoop`] is destroyed.
    ///
    /// [`EventLoop`]: crate::event_loop::EventLoop
    fn wayland_display(&self) -> Option<*mut raw::c_void>;
}

impl<TS: ThreadSafety> EventLoopWindowTargetExtWayland for EventLoopWindowTarget<TS> {
    #[inline]
    fn is_wayland(&self) -> bool {
        self.is_wayland
    }

    #[inline]
    fn wayland_display(&self) -> Option<*mut raw::c_void> {
        todo!()
    }
}

/// Additional methods on [`EventLoopBuilder`] that are specific to Wayland.
///
/// [`EventLoopBuilder`]: crate::event_loop::EventLoopBuilder
pub trait EventLoopBuilderExtWayland: sealed::EventLoopBuilderPrivate {
    /// Force using Wayland.
    fn with_wayland(&mut self) -> &mut Self;

    /// Whether to allow the event loop to be created off of the main thread.
    ///
    /// By default, the window is only allowed to be created on the main
    /// thread, to make platform compatibility easier.
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self;
}

impl EventLoopBuilderExtWayland for EventLoopBuilder {
    fn with_wayland(&mut self) -> &mut Self {
        self.inner.with_wayland();
        self
    }

    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self {
        self.inner.with_any_thread(any_thread);
        self
    }
}

/// Additional methods on [`Window`] that are specific to Wayland.
///
/// [`Window`]: crate::window::Window
pub trait WindowExtWayland: sealed::WindowPrivate {
    /// Returns a pointer to the `wl_surface` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    fn wayland_surface(&self) -> Option<*mut raw::c_void>;

    /// Returns a pointer to the `wl_display` object of wayland that is used by this window.
    ///
    /// Returns `None` if the window doesn't use wayland (if it uses xlib for example).
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    fn wayland_display(&self) -> Option<*mut raw::c_void>;
}

impl<TS: ThreadSafety> WindowExtWayland for Window<TS> {
    #[inline]
    fn wayland_surface(&self) -> Option<*mut raw::c_void> {
        self.window().wayland_surface()
    }

    #[inline]
    fn wayland_display(&self) -> Option<*mut raw::c_void> {
        self.window().wayland_display()
    }
}

/// Additional methods on [`WindowBuilder`] that are specific to Wayland.
///
/// [`WindowBuilder`]: crate::window::WindowBuilder
pub trait WindowBuilderExtWayland: sealed::WindowBuilderPrivate {
    /// Build window with the given name.
    ///
    /// The `general` name sets an application ID, which should match the `.desktop`
    /// file destributed with your program. The `instance` is a `no-op`.
    ///
    /// For details about application ID conventions, see the
    /// [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#desktop-file-id)
    fn with_name(self, general: impl Into<String>, instance: impl Into<String>) -> Self;
}

impl WindowBuilderExtWayland for WindowBuilder {
    #[inline]
    fn with_name(mut self, general: impl Into<String>, instance: impl Into<String>) -> Self {
        self.platform
            .set_x11_name((general.into(), instance.into()));
        self
    }
}

#[derive(Default)]
pub(crate) struct PlatformSpecific {
    pub name: Option<(String, String)>,
}

impl PlatformSpecific {
    pub fn set_x11_name(&mut self, x11_name: (String, String)) {
        self.name = Some(x11_name);
    }

    pub fn apply_to(
        self,
        window_builder: winit::window::WindowBuilder,
    ) -> winit::window::WindowBuilder {
        let mut window_builder = window_builder;

        if let Some((general, instance)) = self.name {
            window_builder = window_builder.with_name(general, instance);
        }

        window_builder
    }
}
