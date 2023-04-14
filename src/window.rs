//! Window code adapted for `async` usage.

use crate::dpi::{Position, Size};
use crate::error::OsError;
use crate::handler::Handler;
use crate::oneoff::oneoff;
use crate::reactor::{EventLoopOp, Reactor};

pub(crate) mod registration;

use registration::Registration;
use std::sync::Arc;
use winit::error::NotSupportedError;

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::DeviceId;

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
    pub(crate) platform: crate::platform::PlatformSpecific,
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
                builder: Box::new(self),
                waker: tx,
            })
            .await;

        let inner = rx.recv().await?;

        // Insert the window into the global window map.
        let registration = Reactor::get().insert_window(inner.id());

        Ok(Window {
            inner: Arc::new(inner),
            registration,
        })
    }

    pub(crate) fn into_winit_builder(self) -> winit::window::WindowBuilder {
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

        builder = self.platform.apply_to(builder);

        builder
    }
}

/// A window.
pub struct Window {
    /// Underlying window.
    inner: Arc<winit::window::Window>,

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

    /// Get a reference to the underlying window.
    pub fn window(&self) -> &winit::window::Window {
        &self.inner
    }

    /// Get the ID of the window.
    pub fn id(&self) -> winit::window::WindowId {
        self.inner.id()
    }

    /// Get the scale factor of the window.
    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor()
    }

    /// Request a redraw.
    pub fn request_redraw(&self) {
        self.inner.request_redraw();
    }

    /// Get whether the window is visible.
    pub fn is_visible(&self) -> Option<bool> {
        self.inner.is_visible()
    }
}

impl Window {
    /// Get the inner position of the window.
    pub async fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::InnerPosition {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Get the outer position of the window.
    pub async fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::OuterPosition {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the outer position of the window.
    pub async fn set_outer_position(&self, position: impl Into<Position>) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetOuterPosition {
                window: self.inner.clone(),
                position: position.into(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Get the inner size of the window.
    pub async fn inner_size(&self) -> PhysicalSize<u32> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::InnerSize {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Get the outer size of the window.
    pub async fn outer_size(&self) -> PhysicalSize<u32> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::OuterSize {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the inner size of the window.
    pub async fn set_inner_size(&self, size: impl Into<Size>) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetInnerSize {
                window: self.inner.clone(),
                size: size.into(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the minimum inner size of the window.
    pub async fn set_min_inner_size(&self, size: impl Into<Option<Size>>) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetMinInnerSize {
                window: self.inner.clone(),
                size: size.into(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the maximum inner size of the window.
    pub async fn set_max_inner_size(&self, size: impl Into<Option<Size>>) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetMaxInnerSize {
                window: self.inner.clone(),
                size: size.into(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Get the resize increments of the window.
    pub async fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::ResizeIncrements {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the resize increments of the window.
    pub async fn set_resize_increments(&self, size: impl Into<Option<Size>>) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetResizeIncrements {
                window: self.inner.clone(),
                size: size.into(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the title of the window.
    pub async fn set_title(&self, title: impl Into<String>) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetTitle {
                window: self.inner.clone(),
                title: title.into(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }
}

/// Waiting for events.
impl Window {
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

    /// Get handler for the `Destroyed` event.
    pub fn destroyed(&self) -> &Handler<()> {
        &self.registration.destroyed
    }

    /// Get the handler for the `Focused` event.
    pub fn focused(&self) -> &Handler<bool> {
        &self.registration.focused
    }

    /// Get the handler for the `KeyboardInput` event.
    pub fn keyboard_input(&self) -> &Handler<crate::event::KeyboardInput> {
        &self.registration.keyboard_input
    }

    /// Get the handler for the `ModifiersChanged` event.
    pub fn modifiers_changed(&self) -> &Handler<crate::event::ModifiersState> {
        &self.registration.modifiers_changed
    }

    /// Get the handler for the `ReceivedCharacter` event.
    pub fn received_character(&self) -> &Handler<char> {
        &self.registration.received_character
    }

    /// Get the handler for the `Ime` event.
    pub fn ime(&self) -> &Handler<crate::event::Ime> {
        &self.registration.ime
    }

    /// Get the handler for the `CursorMoved` event.
    pub fn cursor_moved(&self) -> &Handler<crate::event::CursorMoved> {
        &self.registration.cursor_moved
    }

    /// Get the handler for the `CursorEntered` event.
    pub fn cursor_entered(&self) -> &Handler<DeviceId> {
        &self.registration.cursor_entered
    }

    /// Get the handler for the `CursorLeft` event.
    pub fn cursor_left(&self) -> &Handler<DeviceId> {
        &self.registration.cursor_left
    }

    /// Get the handle for the `MouseWheel` event.
    pub fn mouse_wheel(&self) -> &Handler<crate::event::MouseWheel> {
        &self.registration.mouse_wheel
    }

    /// Get the handle for the `MouseInput` event.
    pub fn mouse_input(&self) -> &Handler<crate::event::MouseInput> {
        &self.registration.mouse_input
    }

    /// Get the handle for the `TouchpadMagnify` event.
    pub fn touchpad_magnify(&self) -> &Handler<crate::event::TouchpadMagnify> {
        &self.registration.touchpad_magnify
    }

    /// Get the handle for the `TouchpadPressure` event.
    pub fn touchpad_pressure(&self) -> &Handler<crate::event::TouchpadPressure> {
        &self.registration.touchpad_pressure
    }

    /// Get the handle for the `Touch` event.
    pub fn touch(&self) -> &Handler<crate::event::Touch> {
        &self.registration.touch
    }

    /// Get the handle for the `ScaleFactorChanged` event.
    pub fn scale_factor_changed(&self) -> &Handler<crate::event::ScaleFactor> {
        &self.registration.scale_factor_changed
    }

    /// Get the handle for the `TouchpadRotate` event.
    pub fn touchpad_rotate(&self) -> &Handler<crate::event::TouchpadRotate> {
        &self.registration.touchpad_rotate
    }

    /// Get the handle for the `SmartMagnify` event.
    pub fn smart_magnify(&self) -> &Handler<DeviceId> {
        &self.registration.smart_magnify
    }

    /// Get the handle for the `AxisMotion` event.
    pub fn axis_motion(&self) -> &Handler<crate::event::AxisMotion> {
        &self.registration.axis_motion
    }

    /// Get the handle for the `ThemeChanged` event.
    pub fn theme_changed(&self) -> &Handler<Theme> {
        &self.registration.theme_changed
    }

    /// Get the handle for the `Occulded` event.
    pub fn occluded(&self) -> &Handler<bool> {
        &self.registration.occluded
    }
}
