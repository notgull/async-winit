//! Window code adapted for `async` usage.

use crate::dpi::{Position, Size};
use crate::error::OsError;
use crate::handler::Handler;
use crate::oneoff::oneoff;
use crate::reactor::{EventLoopOp, Reactor};

pub(crate) mod registration;

use registration::Registration;
use std::sync::Arc;
use winit::dpi::{PhysicalPosition, PhysicalSize};

#[doc(inline)]
pub use winit::window::{Fullscreen, Icon, Theme, WindowButtons, WindowLevel};

/// Attributes to use when creating a window.
#[derive(Debug, Clone)]
pub struct WindowAttributes {
    pub inner_size: Option<Size>,
    pub min_inner_size: Option<Size>,
    pub max_inner_size: Option<Size>,
    pub position: Option<Position>,
    pub resizable: bool,
    pub enabled_buttons: WindowButtons,
    pub title: String,
    pub fullscreen: Option<Fullscreen>,
    pub maximized: bool,
    pub visible: bool,
    pub transparent: bool,
    pub decorations: bool,
    pub window_icon: Option<Icon>,
    pub preferred_theme: Option<Theme>,
    pub resize_increments: Option<Size>,
    pub content_protected: bool,
    pub window_level: WindowLevel,
    pub active: bool,
}

impl Default for WindowAttributes {
    #[inline]
    fn default() -> WindowAttributes {
        WindowAttributes {
            inner_size: None,
            min_inner_size: None,
            max_inner_size: None,
            position: None,
            resizable: true,
            enabled_buttons: WindowButtons::all(),
            title: "winit window".to_owned(),
            maximized: false,
            fullscreen: None,
            visible: true,
            transparent: false,
            decorations: true,
            window_level: Default::default(),
            window_icon: None,
            preferred_theme: None,
            resize_increments: None,
            content_protected: false,
            active: true,
        }
    }
}

/// A builder to use to create windows.
#[derive(Default)]
pub struct WindowBuilder {
    attributes: WindowAttributes,
    // TODO: Platform specific attributes.
}

impl WindowBuilder {
    /// Create a new window builder.
    pub fn new() -> WindowBuilder {
        WindowBuilder::default()
    }

    pub fn attributes(&self) -> &WindowAttributes {
        &self.attributes
    }

    /// Build a new window.
    pub async fn build(self) -> Result<Window, OsError> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::BuildWindow {
                builder: self,
                waker: tx,
            })
            .await;

        let inner = rx.recv().await?;

        // Insert the window into the global window map.
        let registration = Reactor::get().insert_window(inner.id());

        Ok(Window {
            inner,
            registration,
        })
    }

    pub(crate) fn as_winit_builder(&self) -> winit::window::WindowBuilder {
        let mut builder = winit::window::WindowBuilder::new();

        if let Some(size) = self.attributes.inner_size {
            builder = builder.with_inner_size(size);
        }

        if let Some(size) = self.attributes.min_inner_size {
            builder = builder.with_min_inner_size(size);
        }

        if let Some(size) = self.attributes.max_inner_size {
            builder = builder.with_max_inner_size(size);
        }

        if let Some(position) = self.attributes.position {
            builder = builder.with_position(position);
        }

        builder = builder
            .with_resizable(self.attributes.resizable)
            .with_enabled_buttons(self.attributes.enabled_buttons)
            .with_title(&self.attributes.title)
            .with_fullscreen(self.attributes.fullscreen.clone())
            .with_maximized(self.attributes.maximized)
            .with_visible(self.attributes.visible)
            .with_transparent(self.attributes.transparent)
            .with_decorations(self.attributes.decorations);

        if let Some(icon) = self.attributes.window_icon.clone() {
            builder = builder.with_window_icon(Some(icon));
        }

        builder = builder.with_theme(self.attributes.preferred_theme);

        if let Some(size) = self.attributes.resize_increments {
            builder = builder.with_resize_increments(size);
        }

        builder = builder
            .with_content_protected(self.attributes.content_protected)
            .with_window_level(self.attributes.window_level)
            .with_active(self.attributes.active);

        builder
    }
}

/// A window.
pub struct Window {
    /// Underlying window.
    inner: winit::window::Window,

    /// Registration for the window.
    registration: Arc<Registration>,
}

impl Drop for Window {
    fn drop(&mut self) {
        Reactor::get().remove_window(self.inner.id());
    }
}

impl Window {
    /// Create a new window.
    pub async fn new() -> Result<Window, OsError> {
        WindowBuilder::new().build().await
    }

    /// Get the handler for the `RedrawRequested` event.
    pub fn redraw_requested(&self) -> &Handler<()> {
        &self.registration.redraw_requested
    }

    /// Get the handler for the `CloseRequested` event.
    pub fn close_requested(&self) -> &Handler<()> {
        &self.registration.close_requested
    }

    /// Get the handler for the `Resized` event.
    pub fn resized(&self) -> &Handler<PhysicalSize<u32>> {
        &self.registration.resized
    }

    /// Get the handler for the `Moved` event.
    pub fn moved(&self) -> &Handler<PhysicalPosition<i32>> {
        &self.registration.moved
    }

    // TODO Docs
}
