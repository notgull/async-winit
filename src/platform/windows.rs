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

//! Windows-specific functionality.

#[doc(inline)]
pub use winit::platform::windows::{
    IconExtWindows, MonitorHandleExtWindows, HINSTANCE, HMENU, HMONITOR, HWND,
};

use super::__private as sealed;
use crate::event_loop::EventLoopBuilder;
use crate::window::{Icon, Window, WindowBuilder};

use std::os::raw::c_void;

use winit::platform::windows::{
    EventLoopBuilderExtWindows as _, WindowBuilderExtWindows as _, WindowExtWindows as _,
};

/// Additional methods on `EventLoop` that are specific to Windows.
pub trait EventLoopBuilderExtWindows: sealed::EventLoopBuilderPrivate {
    /// Whether to allow the event loop to be created off of the main thread.
    ///
    /// By default, the window is only allowed to be created on the main
    /// thread, to make platform compatibility easier.
    ///
    /// # `Window` caveats
    ///
    /// Note that any `Window` created on the new thread will be destroyed when the thread
    /// terminates. Attempting to use a `Window` after its parent thread terminates has
    /// unspecified, although explicitly not undefined, behavior.
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self;

    /// Whether to enable process-wide DPI awareness.
    ///
    /// By default, `winit` will attempt to enable process-wide DPI awareness. If
    /// that's undesirable, you can disable it with this function.
    ///
    /// # Example
    ///
    /// Disable process-wide DPI awareness.
    ///
    /// ```
    /// use winit::event_loop::EventLoopBuilder;
    /// #[cfg(target_os = "windows")]
    /// use winit::platform::windows::EventLoopBuilderExtWindows;
    ///
    /// let mut builder = EventLoopBuilder::new();
    /// #[cfg(target_os = "windows")]
    /// builder.with_dpi_aware(false);
    /// # if false { // We can't test this part
    /// let event_loop = builder.build();
    /// # }
    /// ```
    fn with_dpi_aware(&mut self, dpi_aware: bool) -> &mut Self;

    /// A callback to be executed before dispatching a win32 message to the window procedure.
    /// Return true to disable winit's internal message dispatching.
    ///
    /// # Example
    ///
    /// ```
    /// # use windows_sys::Win32::UI::WindowsAndMessaging::{ACCEL, CreateAcceleratorTableW, TranslateAcceleratorW, DispatchMessageW, TranslateMessage, MSG};
    /// use winit::event_loop::EventLoopBuilder;
    /// #[cfg(target_os = "windows")]
    /// use winit::platform::windows::EventLoopBuilderExtWindows;
    ///
    /// let mut builder = EventLoopBuilder::new();
    /// #[cfg(target_os = "windows")]
    /// builder.with_msg_hook(|msg|{
    ///     let msg = msg as *const MSG;
    /// #   let accels: Vec<ACCEL> = Vec::new();
    ///     let translated = unsafe {
    ///         TranslateAcceleratorW(
    ///             (*msg).hwnd,
    ///             CreateAcceleratorTableW(accels.as_ptr() as _, 1),
    ///             msg,
    ///         ) == 1
    ///     };
    ///     translated
    /// });
    /// ```
    fn with_msg_hook<F>(&mut self, callback: F) -> &mut Self
    where
        F: FnMut(*const c_void) -> bool + 'static;
}

impl EventLoopBuilderExtWindows for EventLoopBuilder {
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self {
        self.inner.with_any_thread(any_thread);
        self
    }

    fn with_dpi_aware(&mut self, dpi_aware: bool) -> &mut Self {
        self.inner.with_dpi_aware(dpi_aware);
        self
    }

    fn with_msg_hook<F>(&mut self, callback: F) -> &mut Self
    where
        F: FnMut(*const c_void) -> bool + 'static,
    {
        self.inner.with_msg_hook(callback);
        self
    }
}

/// Additional methods on `Window` that are specific to Windows.
pub trait WindowExtWindows: sealed::WindowPrivate {
    /// Returns the HINSTANCE of the window
    fn hinstance(&self) -> HINSTANCE;
    /// Returns the native handle that is used by this window.
    ///
    /// The pointer will become invalid when the native window was destroyed.
    fn hwnd(&self) -> HWND;

    /// Enables or disables mouse and keyboard input to the specified window.
    ///
    /// A window must be enabled before it can be activated.
    /// If an application has create a modal dialog box by disabling its owner window
    /// (as described in [`WindowBuilderExtWindows::with_owner_window`]), the application must enable
    /// the owner window before destroying the dialog box.
    /// Otherwise, another window will receive the keyboard focus and be activated.
    ///
    /// If a child window is disabled, it is ignored when the system tries to determine which
    /// window should receive mouse messages.
    ///
    /// For more information, see <https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enablewindow#remarks>
    /// and <https://docs.microsoft.com/en-us/windows/win32/winmsg/window-features#disabled-windows>
    fn set_enable(&self, enabled: bool);

    /// This sets `ICON_BIG`. A good ceiling here is 256x256.
    fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>);

    /// Whether to show or hide the window icon in the taskbar.
    fn set_skip_taskbar(&self, skip: bool);

    /// Shows or hides the background drop shadow for undecorated windows.
    ///
    /// Enabling the shadow causes a thin 1px line to appear on the top of the window.
    fn set_undecorated_shadow(&self, shadow: bool);
}

impl WindowExtWindows for Window {
    fn hwnd(&self) -> HWND {
        self.window().hwnd()
    }

    fn hinstance(&self) -> HINSTANCE {
        self.window().hinstance()
    }

    fn set_enable(&self, enabled: bool) {
        self.window().set_enable(enabled);
    }

    fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>) {
        self.window().set_taskbar_icon(taskbar_icon);
    }

    fn set_skip_taskbar(&self, skip: bool) {
        self.window().set_skip_taskbar(skip);
    }

    fn set_undecorated_shadow(&self, shadow: bool) {
        self.window().set_undecorated_shadow(shadow);
    }
}

/// Additional methods on `WindowBuilder` that are specific to Windows.
pub trait WindowBuilderExtWindows: sealed::WindowBuilderPrivate {
    /// Set an owner to the window to be created. Can be used to create a dialog box, for example.
    /// This only works when [`WindowBuilder::with_parent_window`] isn't called or set to `None`.
    /// Can be used in combination with [`WindowExtWindows::set_enable(false)`](WindowExtWindows::set_enable)
    /// on the owner window to create a modal dialog box.
    ///
    /// From MSDN:
    /// - An owned window is always above its owner in the z-order.
    /// - The system automatically destroys an owned window when its owner is destroyed.
    /// - An owned window is hidden when its owner is minimized.
    ///
    /// For more information, see <https://docs.microsoft.com/en-us/windows/win32/winmsg/window-features#owned-windows>
    fn with_owner_window(self, parent: HWND) -> WindowBuilder;

    /// Sets a menu on the window to be created.
    ///
    /// Parent and menu are mutually exclusive; a child window cannot have a menu!
    ///
    /// The menu must have been manually created beforehand with [`CreateMenu`] or similar.
    ///
    /// Note: Dark mode cannot be supported for win32 menus, it's simply not possible to change how the menus look.
    /// If you use this, it is recommended that you combine it with `with_theme(Some(Theme::Light))` to avoid a jarring effect.
    ///
    /// [`CreateMenu`]: windows_sys::Win32::UI::WindowsAndMessaging::CreateMenu
    fn with_menu(self, menu: HMENU) -> WindowBuilder;

    /// This sets `ICON_BIG`. A good ceiling here is 256x256.
    fn with_taskbar_icon(self, taskbar_icon: Option<Icon>) -> WindowBuilder;

    /// This sets `WS_EX_NOREDIRECTIONBITMAP`.
    fn with_no_redirection_bitmap(self, flag: bool) -> WindowBuilder;

    /// Enables or disables drag and drop support (enabled by default). Will interfere with other crates
    /// that use multi-threaded COM API (`CoInitializeEx` with `COINIT_MULTITHREADED` instead of
    /// `COINIT_APARTMENTTHREADED`) on the same thread. Note that winit may still attempt to initialize
    /// COM API regardless of this option. Currently only fullscreen mode does that, but there may be more in the future.
    /// If you need COM API with `COINIT_MULTITHREADED` you must initialize it before calling any winit functions.
    /// See <https://docs.microsoft.com/en-us/windows/win32/api/objbase/nf-objbase-coinitialize#remarks> for more information.
    fn with_drag_and_drop(self, flag: bool) -> WindowBuilder;

    /// Whether show or hide the window icon in the taskbar.
    fn with_skip_taskbar(self, skip: bool) -> WindowBuilder;

    /// Shows or hides the background drop shadow for undecorated windows.
    ///
    /// The shadow is hidden by default.
    /// Enabling the shadow causes a thin 1px line to appear on the top of the window.
    fn with_undecorated_shadow(self, shadow: bool) -> WindowBuilder;
}

#[derive(Default)]
pub(crate) struct PlatformSpecific {
    owner_window: Option<HWND>,
    menu: Option<HMENU>,
    taskbar_icon: Option<Icon>,
    no_redirection_bitmap: Option<bool>,
    drag_and_drop: Option<bool>,
    skip_taskbar: Option<bool>,
    undecorated_shadow: Option<bool>,
}

impl PlatformSpecific {
    pub(crate) fn apply_to(
        self,
        mut wb: winit::window::WindowBuilder,
    ) -> winit::window::WindowBuilder {
        if let Some(owner_window) = self.owner_window {
            wb = wb.with_owner_window(owner_window);
        }

        if let Some(menu) = self.menu {
            wb = wb.with_menu(menu);
        }

        if let Some(taskbar_icon) = self.taskbar_icon {
            wb = wb.with_taskbar_icon(Some(taskbar_icon));
        }

        if let Some(no_redirection_bitmap) = self.no_redirection_bitmap {
            wb = wb.with_no_redirection_bitmap(no_redirection_bitmap);
        }

        if let Some(drag_and_drop) = self.drag_and_drop {
            wb = wb.with_drag_and_drop(drag_and_drop);
        }

        if let Some(skip_taskbar) = self.skip_taskbar {
            wb = wb.with_skip_taskbar(skip_taskbar);
        }

        if let Some(undecorated_shadow) = self.undecorated_shadow {
            wb = wb.with_undecorated_shadow(undecorated_shadow);
        }

        wb
    }
}
