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

// This file is partially derived from `winit`, which was originally created by Pierre Krieger and
// contributers. It was originally released under the MIT license.

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
use winit::error::{ExternalError, NotSupportedError};
use winit::event::DeviceId;
use winit::monitor::MonitorHandle;

#[doc(inline)]
pub use winit::window::{
    CursorGrabMode, CursorIcon, Fullscreen, Icon, ImePurpose, ResizeDirection, Theme,
    UserAttentionType, WindowButtons, WindowLevel,
};

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
#[derive(Clone)]
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

unsafe impl raw_window_handle::HasRawDisplayHandle for Window {
    fn raw_display_handle(&self) -> raw_window_handle::RawDisplayHandle {
        self.inner.raw_display_handle()
    }
}

unsafe impl raw_window_handle::HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        self.inner.raw_window_handle()
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

    /// Set whether the window is visible.
    pub async fn set_visible(&self, visible: bool) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetVisible {
                window: self.inner.clone(),
                visible,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Get the window's visibility.
    pub async fn is_visible(&self) -> Option<bool> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::Visible {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the window's transparency.
    pub async fn set_transparent(&self, transparent: bool) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetTransparent {
                window: self.inner.clone(),
                transparent,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the window's resizable property.
    pub async fn set_resizable(&self, resizable: bool) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetResizable {
                window: self.inner.clone(),
                resizable,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Get the window's resizable property.
    pub async fn is_resizable(&self) -> bool {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::Resizable {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the window's minimization.
    pub async fn set_minimized(&self, minimized: bool) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetMinimized {
                window: self.inner.clone(),
                minimized,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Get the window's minimization.
    pub async fn is_minimized(&self) -> Option<bool> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::Minimized {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the window's maximization.
    pub async fn set_maximized(&self, maximized: bool) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetMaximized {
                window: self.inner.clone(),
                maximized,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Get the window's maximization.
    pub async fn is_maximized(&self) -> bool {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::Maximized {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the window's fullscreen state.
    pub async fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetFullscreen {
                window: self.inner.clone(),
                fullscreen,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Get the fullscreen state of the window.
    pub async fn fullscreen(&self) -> Option<Fullscreen> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::Fullscreen {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the window's decorations.
    pub async fn set_decorations(&self, decorations: bool) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetDecorated {
                window: self.inner.clone(),
                decorated: decorations,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Get the window's decorations.
    pub async fn is_decorated(&self) -> bool {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::Decorated {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the window level.
    pub async fn set_window_level(&self, level: WindowLevel) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetWindowLevel {
                window: self.inner.clone(),
                level,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the window icon.
    pub async fn set_window_icon(&self, icon: Option<Icon>) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetWindowIcon {
                window: self.inner.clone(),
                icon,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the IME position.
    pub async fn set_ime_position(&self, posn: impl Into<Position>) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetImePosition {
                window: self.inner.clone(),
                position: posn.into(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set whether IME is allowed.
    pub async fn set_ime_allowed(&self, allowed: bool) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetImeAllowed {
                window: self.inner.clone(),
                allowed,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the IME purpose.
    pub async fn set_ime_purpose(&self, purpose: ImePurpose) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetImePurpose {
                window: self.inner.clone(),
                purpose,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Focus the window.
    pub async fn focus_window(&self) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::FocusWindow {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Tell whether the window is focused.
    pub async fn is_focused(&self) -> bool {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::Focused {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Request the user's attention.
    pub async fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::RequestUserAttention {
                window: self.inner.clone(),
                request_type,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the window's theme.
    pub async fn set_theme(&self, theme: Option<Theme>) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetTheme {
                window: self.inner.clone(),
                theme,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Get the window's theme.
    pub async fn theme(&self) -> Option<Theme> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::Theme {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the window's protected content.
    pub async fn set_content_protected(&self, protected: bool) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetProtectedContent {
                window: self.inner.clone(),
                protected,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Get the title of the window.
    pub async fn title(&self) -> String {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::Title {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the cursor icon.
    pub async fn set_cursor_icon(&self, icon: CursorIcon) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetCursorIcon {
                window: self.inner.clone(),
                icon,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the cursor position.
    pub async fn set_cursor_position(
        &self,
        posn: impl Into<Position>,
    ) -> Result<(), ExternalError> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetCursorPosition {
                window: self.inner.clone(),
                position: posn.into(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the cursor's grab mode.
    pub async fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetCursorGrab {
                window: self.inner.clone(),
                mode,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the cursor's visibility.
    pub async fn set_cursor_visible(&self, visible: bool) {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetCursorVisible {
                window: self.inner.clone(),
                visible,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Drag the window.
    pub async fn drag_window(&self) -> Result<(), ExternalError> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::DragWindow {
                window: self.inner.clone(),
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Drag-resize the window.
    pub async fn drag_resize_window(
        &self,
        direction: ResizeDirection,
    ) -> Result<(), ExternalError> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::DragResizeWindow {
                window: self.inner.clone(),
                direction,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Set the cursor hit test.
    pub async fn set_cursor_hittest(&self, hit_test: bool) -> Result<(), ExternalError> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::SetCursorHitTest {
                window: self.inner.clone(),
                hit_test,
                waker: tx,
            })
            .await;

        rx.recv().await
    }

    /// Get the current monitor of this window.
    pub async fn current_monitor(&self) -> Option<MonitorHandle> {
        let (tx, rx) = oneoff();
        Reactor::get()
            .push_event_loop_op(EventLoopOp::CurrentMonitor {
                window: self.inner.clone(),
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
