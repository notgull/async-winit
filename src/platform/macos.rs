//! Platform-specific macOS features.

#[doc(inline)]
pub use winit::platform::macos::{ActivationPolicy, OptionAsAlt};

use winit::platform::macos::{EventLoopBuilderExtMacOS as _, WindowExtMacOS as _};

use std::os::raw::c_void;

use crate::event_loop::EventLoopBuilder;
use crate::window::{Window, WindowBuilder};

/// Additional methods on [`Window`] that are specific to MacOS.
pub trait WindowExtMacOS {
    /// Returns a pointer to the cocoa `NSWindow` that is used by this window.
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    fn ns_window(&self) -> *mut c_void;

    /// Returns a pointer to the cocoa `NSView` that is used by this window.
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    fn ns_view(&self) -> *mut c_void;

    /// Returns whether or not the window is in simple fullscreen mode.
    fn simple_fullscreen(&self) -> bool;

    /// Toggles a fullscreen mode that doesn't require a new macOS space.
    /// Returns a boolean indicating whether the transition was successful (this
    /// won't work if the window was already in the native fullscreen).
    ///
    /// This is how fullscreen used to work on macOS in versions before Lion.
    /// And allows the user to have a fullscreen window without using another
    /// space or taking control over the entire monitor.
    fn set_simple_fullscreen(&self, fullscreen: bool) -> bool;

    /// Returns whether or not the window has shadow.
    fn has_shadow(&self) -> bool;

    /// Sets whether or not the window has shadow.
    fn set_has_shadow(&self, has_shadow: bool);

    /// Get the window's edit state.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// WindowEvent::CloseRequested => {
    ///     if window.is_document_edited() {
    ///         // Show the user a save pop-up or similar
    ///     } else {
    ///         // Close the window
    ///         drop(window);
    ///     }
    /// }
    /// ```
    fn is_document_edited(&self) -> bool;

    /// Put the window in a state which indicates a file save is required.
    fn set_document_edited(&self, edited: bool);

    /// Set option as alt behavior as described in [`OptionAsAlt`].
    ///
    /// This will ignore diacritical marks and accent characters from
    /// being processed as received characters. Instead, the input
    /// device's raw character will be placed in event queues with the
    /// Alt modifier set.
    fn set_option_as_alt(&self, option_as_alt: OptionAsAlt);

    /// Getter for the [`WindowExtMacOS::set_option_as_alt`].
    fn option_as_alt(&self) -> OptionAsAlt;
}

impl WindowExtMacOS for Window {
    fn ns_view(&self) -> *mut c_void {
        self.window().ns_view()
    }

    fn ns_window(&self) -> *mut c_void {
        self.window().ns_window()
    }

    fn simple_fullscreen(&self) -> bool {
        self.window().simple_fullscreen()
    }

    fn set_simple_fullscreen(&self, fullscreen: bool) -> bool {
        self.window().set_simple_fullscreen(fullscreen)
    }

    fn has_shadow(&self) -> bool {
        self.window().has_shadow()
    }

    fn set_has_shadow(&self, has_shadow: bool) {
        self.window().set_has_shadow(has_shadow)
    }

    fn is_document_edited(&self) -> bool {
        self.window().is_document_edited()
    }

    fn set_document_edited(&self, edited: bool) {
        self.window().set_document_edited(edited)
    }

    fn set_option_as_alt(&self, option_as_alt: OptionAsAlt) {
        self.window().set_option_as_alt(option_as_alt)
    }

    fn option_as_alt(&self) -> OptionAsAlt {
        self.window().option_as_alt()
    }
}

/// Additional methods on [`WindowBuilder`] that are specific to MacOS.
///
/// **Note:** Properties dealing with the titlebar will be overwritten by the [`WindowBuilder::with_decorations`] method:
/// - `with_titlebar_transparent`
/// - `with_title_hidden`
/// - `with_titlebar_hidden`
/// - `with_titlebar_buttons_hidden`
/// - `with_fullsize_content_view`
pub trait WindowBuilderExtMacOS {
    /// Enables click-and-drag behavior for the entire window, not just the titlebar.
    fn with_movable_by_window_background(self, movable_by_window_background: bool)
        -> WindowBuilder;
    /// Makes the titlebar transparent and allows the content to appear behind it.
    fn with_titlebar_transparent(self, titlebar_transparent: bool) -> WindowBuilder;
    /// Hides the window title.
    fn with_title_hidden(self, title_hidden: bool) -> WindowBuilder;
    /// Hides the window titlebar.
    fn with_titlebar_hidden(self, titlebar_hidden: bool) -> WindowBuilder;
    /// Hides the window titlebar buttons.
    fn with_titlebar_buttons_hidden(self, titlebar_buttons_hidden: bool) -> WindowBuilder;
    /// Makes the window content appear behind the titlebar.
    fn with_fullsize_content_view(self, fullsize_content_view: bool) -> WindowBuilder;
    fn with_disallow_hidpi(self, disallow_hidpi: bool) -> WindowBuilder;
    fn with_has_shadow(self, has_shadow: bool) -> WindowBuilder;
    /// Window accepts click-through mouse events.
    fn with_accepts_first_mouse(self, accepts_first_mouse: bool) -> WindowBuilder;

    /// Set whether the `OptionAsAlt` key is interpreted as the `Alt` modifier.
    ///
    /// See [`WindowExtMacOS::set_option_as_alt`] for details on what this means if set.
    fn with_option_as_alt(self, option_as_alt: OptionAsAlt) -> WindowBuilder;
}

impl WindowBuilderExtMacOS for WindowBuilder {
    fn with_accepts_first_mouse(mut self, accepts_first_mouse: bool) -> WindowBuilder {
        self.platform.accepts_first_mouse = Some(accepts_first_mouse);
        self
    }

    fn with_movable_by_window_background(
        mut self,
        movable_by_window_background: bool,
    ) -> WindowBuilder {
        self.platform.movable_by_window_background = Some(movable_by_window_background);
        self
    }

    fn with_disallow_hidpi(mut self, disallow_hidpi: bool) -> WindowBuilder {
        self.platform.disallow_hidpi = Some(disallow_hidpi);
        self
    }

    fn with_has_shadow(mut self, has_shadow: bool) -> WindowBuilder {
        self.platform.has_shadow = Some(has_shadow);
        self
    }

    fn with_fullsize_content_view(mut self, fullsize_content_view: bool) -> WindowBuilder {
        self.platform.fullsize_content_view = Some(fullsize_content_view);
        self
    }

    fn with_titlebar_buttons_hidden(mut self, titlebar_buttons_hidden: bool) -> WindowBuilder {
        self.platform.titlebar_buttons_hidden = Some(titlebar_buttons_hidden);
        self
    }

    fn with_titlebar_hidden(mut self, titlebar_hidden: bool) -> WindowBuilder {
        self.platform.titlebar_hidden = Some(titlebar_hidden);
        self
    }

    fn with_option_as_alt(mut self, option_as_alt: OptionAsAlt) -> WindowBuilder {
        self.platform.option_as_alt = Some(option_as_alt);
        self
    }

    fn with_title_hidden(mut self, title_hidden: bool) -> WindowBuilder {
        self.platform.title_hidden = Some(title_hidden);
        self
    }

    fn with_titlebar_transparent(mut self, titlebar_transparent: bool) -> WindowBuilder {
        self.platform.titlebar_transparent = Some(titlebar_transparent);
        self
    }
}

pub trait EventLoopBuilderExtMacOS {
    /// Sets the activation policy for the application.
    ///
    /// It is set to [`ActivationPolicy::Regular`] by default.
    ///
    /// # Example
    ///
    /// Set the activation policy to "accessory".
    ///
    /// ```
    /// use winit::event_loop::EventLoopBuilder;
    /// #[cfg(target_os = "macos")]
    /// use winit::platform::macos::{EventLoopBuilderExtMacOS, ActivationPolicy};
    ///
    /// let mut builder = EventLoopBuilder::new();
    /// #[cfg(target_os = "macos")]
    /// builder.with_activation_policy(ActivationPolicy::Accessory);
    /// # if false { // We can't test this part
    /// let event_loop = builder.build();
    /// # }
    /// ```
    fn with_activation_policy(&mut self, activation_policy: ActivationPolicy) -> &mut Self;

    /// Used to control whether a default menubar menu is created.
    ///
    /// Menu creation is enabled by default.
    ///
    /// # Example
    ///
    /// Disable creating a default menubar.
    ///
    /// ```
    /// use winit::event_loop::EventLoopBuilder;
    /// #[cfg(target_os = "macos")]
    /// use winit::platform::macos::EventLoopBuilderExtMacOS;
    ///
    /// let mut builder = EventLoopBuilder::new();
    /// #[cfg(target_os = "macos")]
    /// builder.with_default_menu(false);
    /// # if false { // We can't test this part
    /// let event_loop = builder.build();
    /// # }
    /// ```
    fn with_default_menu(&mut self, enable: bool) -> &mut Self;

    /// Used to prevent the application from automatically activating when launched if
    /// another application is already active.
    ///
    /// The default behavior is to ignore other applications and activate when launched.
    fn with_activate_ignoring_other_apps(&mut self, ignore: bool) -> &mut Self;
}

impl EventLoopBuilderExtMacOS for EventLoopBuilder {
    fn with_activate_ignoring_other_apps(&mut self, ignore: bool) -> &mut Self {
        self.inner.with_activate_ignoring_other_apps(ignore);
        self
    }

    fn with_activation_policy(&mut self, activation_policy: ActivationPolicy) -> &mut Self {
        self.inner.with_activation_policy(activation_policy);
        self
    }

    fn with_default_menu(&mut self, enable: bool) -> &mut Self {
        self.inner.with_default_menu(enable);
        self
    }
}

#[derive(Default)]
pub(crate) struct PlatformSpecific {
    movable_by_window_background: Option<bool>,
    titlebar_transparent: Option<bool>,
    title_hidden: Option<bool>,
    titlebar_hidden: Option<bool>,
    titlebar_buttons_hidden: Option<bool>,
    fullsize_content_view: Option<bool>,
    disallow_hidpi: Option<bool>,
    has_shadow: Option<bool>,
    accepts_first_mouse: Option<bool>,
    option_as_alt: Option<OptionAsAlt>,
}

impl PlatformSpecific {
    pub(crate) fn apply_to(
        self,
        mut wb: winit::window::WindowBuilder,
    ) -> winit::window::WindowBuilder {
        use winit::platform::macos::WindowBuilderExtMacOS as _;

        if let Some(movable_by_window_background) = self.movable_by_window_background {
            wb = wb.with_movable_by_window_background(movable_by_window_background);
        }

        if let Some(titlebar_transparent) = self.titlebar_transparent {
            wb = wb.with_titlebar_transparent(titlebar_transparent);
        }

        if let Some(title_hidden) = self.title_hidden {
            wb = wb.with_title_hidden(title_hidden);
        }

        if let Some(titlebar_hidden) = self.titlebar_hidden {
            wb = wb.with_titlebar_hidden(titlebar_hidden);
        }

        if let Some(titlebar_buttons_hidden) = self.titlebar_buttons_hidden {
            wb = wb.with_titlebar_buttons_hidden(titlebar_buttons_hidden);
        }

        if let Some(fullsize_content_view) = self.fullsize_content_view {
            wb = wb.with_fullsize_content_view(fullsize_content_view);
        }

        if let Some(disallow_hidpi) = self.disallow_hidpi {
            wb = wb.with_disallow_hidpi(disallow_hidpi);
        }

        if let Some(has_shadow) = self.has_shadow {
            wb = wb.with_has_shadow(has_shadow);
        }

        if let Some(accepts_first_mouse) = self.accepts_first_mouse {
            wb = wb.with_accepts_first_mouse(accepts_first_mouse);
        }

        if let Some(option_as_alt) = self.option_as_alt {
            wb = wb.with_option_as_alt(option_as_alt);
        }

        wb
    }
}
