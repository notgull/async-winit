//! Platform-specific features for Wayland.

use crate::event_loop::{EventLoopBuilder, EventLoopWindowTarget};
use crate::window::{Window, WindowBuilder};

use std::os::raw;

use winit::platform::wayland::{
    EventLoopBuilderExtWayland as _, WindowBuilderExtWayland as _, WindowExtWayland as _,
};

#[doc(inline)]
pub use winit::platform::wayland::MonitorHandleExtWayland;

/// Additional methods on [`EventLoopWindowTarget`] that are specific to Wayland.
pub trait EventLoopWindowTargetExtWayland {
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

impl EventLoopWindowTargetExtWayland for EventLoopWindowTarget {
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
pub trait EventLoopBuilderExtWayland {
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
pub trait WindowExtWayland {
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

impl WindowExtWayland for Window {
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
pub trait WindowBuilderExtWayland {
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
