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

//! X11-specific code.

use super::__private as sealed;
use crate::event_loop::{EventLoopBuilder, EventLoopWindowTarget};
use crate::sync::ThreadSafety;
use crate::window::{Window, WindowBuilder};

use std::os::raw;

use winit::dpi::Size;
use winit::platform::x11::{EventLoopBuilderExtX11 as _, WindowExtX11 as _};

#[doc(inline)]
pub use winit::platform::x11::{register_xlib_error_hook, XWindowType, XlibErrorHook};

/// Additional methods on [`EventLoopWindowTarget`] that are specific to X11.
///
/// [`EventLoopWindowTarget`]: crate::event_loop::EventLoopWindowTarget
pub trait EventLoopWindowTargetExtX11: sealed::EventLoopWindowTargetPrivate {
    /// True if the [`EventLoopWindowTarget`] uses X11.
    fn is_x11(&self) -> bool;
}

impl<TS: ThreadSafety> EventLoopWindowTargetExtX11 for EventLoopWindowTarget<TS> {
    #[inline]
    fn is_x11(&self) -> bool {
        !self.is_wayland
    }
}

/// Additional methods on [`EventLoopBuilder`] that are specific to X11.
///
/// [`EventLoopBuilder`]: crate::event_loop::EventLoopBuilder
pub trait EventLoopBuilderExtX11: sealed::EventLoopBuilderPrivate {
    /// Force using X11.
    fn with_x11(&mut self) -> &mut Self;

    /// Whether to allow the event loop to be created off of the main thread.
    ///
    /// By default, the window is only allowed to be created on the main
    /// thread, to make platform compatibility easier.
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self;
}

impl EventLoopBuilderExtX11 for EventLoopBuilder {
    #[inline]
    fn with_x11(&mut self) -> &mut Self {
        self.inner.with_x11();
        self
    }

    #[inline]
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self {
        self.inner.with_any_thread(any_thread);
        self
    }
}

/// Additional methods on [`Window`] that are specific to X11.
///
/// [`Window`]: crate::window::Window
pub trait WindowExtX11: sealed::WindowPrivate {
    /// Returns the ID of the [`Window`] xlib object that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    fn xlib_window(&self) -> Option<raw::c_ulong>;

    /// Returns a pointer to the `Display` object of xlib that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    fn xlib_display(&self) -> Option<*mut raw::c_void>;

    fn xlib_screen_id(&self) -> Option<raw::c_int>;

    /// This function returns the underlying `xcb_connection_t` of an xlib `Display`.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    fn xcb_connection(&self) -> Option<*mut raw::c_void>;
}

impl<TS: ThreadSafety> WindowExtX11 for Window<TS> {
    fn xcb_connection(&self) -> Option<*mut raw::c_void> {
        self.window().xcb_connection()
    }

    fn xlib_display(&self) -> Option<*mut raw::c_void> {
        self.window().xlib_display()
    }

    fn xlib_screen_id(&self) -> Option<raw::c_int> {
        self.window().xlib_screen_id()
    }

    fn xlib_window(&self) -> Option<raw::c_ulong> {
        self.window().xlib_window()
    }
}

/// Additional methods on [`WindowBuilder`] that are specific to X11.
///
/// [`WindowBuilder`]: crate::window::WindowBuilder
pub trait WindowBuilderExtX11: sealed::WindowBuilderPrivate {
    fn with_x11_screen(self, screen_id: i32) -> Self;

    /// Build window with the given `general` and `instance` names.
    ///
    /// The `general` sets general class of `WM_CLASS(STRING)`, while `instance` set the
    /// instance part of it. The resulted property looks like `WM_CLASS(STRING) = "general", "instance"`.
    ///
    /// For details about application ID conventions, see the
    /// [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#desktop-file-id)
    fn with_name(self, general: impl Into<String>, instance: impl Into<String>) -> Self;

    /// Build window with override-redirect flag; defaults to false. Only relevant on X11.
    fn with_override_redirect(self, override_redirect: bool) -> Self;

    /// Build window with `_NET_WM_WINDOW_TYPE` hints; defaults to `Normal`. Only relevant on X11.
    fn with_x11_window_type(self, x11_window_type: Vec<XWindowType>) -> Self;

    /// Build window with base size hint. Only implemented on X11.
    ///
    /// ```no_run
    /// # use winit::dpi::{LogicalSize, PhysicalSize};
    /// # use winit::window::WindowBuilder;
    /// # use winit::platform::x11::WindowBuilderExtX11;
    /// // Specify the size in logical dimensions like this:
    /// WindowBuilder::new().with_base_size(LogicalSize::new(400.0, 200.0));
    ///
    /// // Or specify the size in physical dimensions like this:
    /// WindowBuilder::new().with_base_size(PhysicalSize::new(400, 200));
    /// ```
    fn with_base_size<S: Into<Size>>(self, base_size: S) -> Self;
}

impl WindowBuilderExtX11 for WindowBuilder {
    fn with_x11_screen(mut self, screen_id: i32) -> Self {
        self.platform.set_x11_screen_id(screen_id);
        self
    }

    fn with_name(mut self, general: impl Into<String>, instance: impl Into<String>) -> Self {
        self.platform
            .set_x11_name((general.into(), instance.into()));
        self
    }

    fn with_override_redirect(mut self, override_redirect: bool) -> Self {
        self.platform.set_x11_override_redirect(override_redirect);
        self
    }

    fn with_x11_window_type(mut self, x11_window_type: Vec<XWindowType>) -> Self {
        self.platform.set_x11_window_type(x11_window_type);
        self
    }

    fn with_base_size<S: Into<Size>>(mut self, base_size: S) -> Self {
        self.platform.set_x11_base_size(base_size.into());
        self
    }
}

#[derive(Default)]
pub(crate) struct PlatformSpecific {
    pub x11_window_type: Vec<XWindowType>,
    pub x11_name: Option<(String, String)>,
    pub x11_screen_id: Option<i32>,
    pub x11_override_redirect: bool,
    pub x11_base_size: Option<Size>,
}

impl PlatformSpecific {
    pub(crate) fn set_x11_window_type(&mut self, x11_window_type: Vec<XWindowType>) {
        self.x11_window_type = x11_window_type;
    }

    pub(crate) fn set_x11_name(&mut self, x11_name: (String, String)) {
        self.x11_name = Some(x11_name);
    }

    pub(crate) fn set_x11_screen_id(&mut self, x11_screen_id: i32) {
        self.x11_screen_id = Some(x11_screen_id);
    }

    pub(crate) fn set_x11_override_redirect(&mut self, x11_override_redirect: bool) {
        self.x11_override_redirect = x11_override_redirect;
    }

    pub(crate) fn set_x11_base_size(&mut self, x11_base_size: Size) {
        self.x11_base_size = Some(x11_base_size);
    }

    pub(crate) fn apply_to(
        self,
        window_builder: winit::window::WindowBuilder,
    ) -> winit::window::WindowBuilder {
        use winit::platform::x11::WindowBuilderExtX11 as _;

        let mut window_builder = window_builder;
        if let Some(screen_id) = self.x11_screen_id {
            window_builder = window_builder.with_x11_screen(screen_id);
        }
        if self.x11_override_redirect {
            window_builder = window_builder.with_override_redirect(true);
        }
        if !self.x11_window_type.is_empty() {
            window_builder = window_builder.with_x11_window_type(self.x11_window_type);
        }
        if let Some(base_size) = self.x11_base_size {
            window_builder = window_builder.with_base_size(base_size);
        }
        window_builder
    }
}
