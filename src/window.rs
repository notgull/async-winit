//! Window code adapted for `async` usage.

use crate::dpi::{Position, Size};

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
}
