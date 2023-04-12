//! Window code adapted for `async` usage.

use crate::dpi::{Position, Size};
use crate::error::OsError;
use crate::oneoff::oneoff;
use crate::reactor::{EventLoopOp, Reactor};

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

/// A builder to use to create windows.
pub struct WindowBuilder {
    attributes: WindowAttributes,
    // TODO: Platform specific attributes.
}

impl WindowBuilder {
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
        Ok(Window { inner })
    }

    pub(crate) fn into_winit_builder(&self) -> winit::window::WindowBuilder {
        todo!()
    }
}

/// A window.
pub struct Window {
    inner: winit::window::Window,
}
