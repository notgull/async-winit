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

//! Platform-specific iOS features.

use std::os::raw::c_void;

#[doc(inline)]
pub use winit::platform::ios::{Idiom, MonitorHandleExtIOS, ScreenEdge, ValidOrientations};

use winit::platform::ios::{WindowBuilderExtIOS as _, WindowExtIOS as _};

use crate::event_loop::EventLoop;
use crate::window::{Window, WindowBuilder};

/// Additional methods on [`EventLoop`] that are specific to iOS.
pub trait EventLoopExtIOS {
    /// Returns the [`Idiom`] (phone/tablet/tv/etc) for the current device.
    fn idiom(&self) -> Idiom;
}

impl EventLoopExtIOS for EventLoop {
    fn idiom(&self) -> Idiom {
        use winit::platform::ios::EventLoopExtIOS as _;
        self.inner.idiom()
    }
}

/// Additional methods on [`Window`] that are specific to iOS.
pub trait WindowExtIOS {
    /// Returns a pointer to the [`UIWindow`] that is used by this window.
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    ///
    /// [`UIWindow`]: https://developer.apple.com/documentation/uikit/uiwindow?language=objc
    fn ui_window(&self) -> *mut c_void;

    /// Returns a pointer to the [`UIViewController`] that is used by this window.
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    ///
    /// [`UIViewController`]: https://developer.apple.com/documentation/uikit/uiviewcontroller?language=objc
    fn ui_view_controller(&self) -> *mut c_void;

    /// Returns a pointer to the [`UIView`] that is used by this window.
    ///
    /// The pointer will become invalid when the [`Window`] is destroyed.
    ///
    /// [`UIView`]: https://developer.apple.com/documentation/uikit/uiview?language=objc
    fn ui_view(&self) -> *mut c_void;

    /// Sets the [`contentScaleFactor`] of the underlying [`UIWindow`] to `scale_factor`.
    ///
    /// The default value is device dependent, and it's recommended GLES or Metal applications set
    /// this to [`MonitorHandle::scale_factor()`].
    ///
    /// [`UIWindow`]: https://developer.apple.com/documentation/uikit/uiwindow?language=objc
    /// [`contentScaleFactor`]: https://developer.apple.com/documentation/uikit/uiview/1622657-contentscalefactor?language=objc
    fn set_scale_factor(&self, scale_factor: f64);

    /// Sets the valid orientations for the [`Window`].
    ///
    /// The default value is [`ValidOrientations::LandscapeAndPortrait`].
    ///
    /// This changes the value returned by
    /// [`-[UIViewController supportedInterfaceOrientations]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621435-supportedinterfaceorientations?language=objc),
    /// and then calls
    /// [`-[UIViewController attemptRotationToDeviceOrientation]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621400-attemptrotationtodeviceorientati?language=objc).
    fn set_valid_orientations(&self, valid_orientations: ValidOrientations);

    /// Sets whether the [`Window`] prefers the home indicator hidden.
    ///
    /// The default is to prefer showing the home indicator.
    ///
    /// This changes the value returned by
    /// [`-[UIViewController prefersHomeIndicatorAutoHidden]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/2887510-prefershomeindicatorautohidden?language=objc),
    /// and then calls
    /// [`-[UIViewController setNeedsUpdateOfHomeIndicatorAutoHidden]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/2887509-setneedsupdateofhomeindicatoraut?language=objc).
    ///
    /// This only has an effect on iOS 11.0+.
    fn set_prefers_home_indicator_hidden(&self, hidden: bool);

    /// Sets the screen edges for which the system gestures will take a lower priority than the
    /// application's touch handling.
    ///
    /// This changes the value returned by
    /// [`-[UIViewController preferredScreenEdgesDeferringSystemGestures]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/2887512-preferredscreenedgesdeferringsys?language=objc),
    /// and then calls
    /// [`-[UIViewController setNeedsUpdateOfScreenEdgesDeferringSystemGestures]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/2887507-setneedsupdateofscreenedgesdefer?language=objc).
    ///
    /// This only has an effect on iOS 11.0+.
    fn set_preferred_screen_edges_deferring_system_gestures(&self, edges: ScreenEdge);

    /// Sets whether the [`Window`] prefers the status bar hidden.
    ///
    /// The default is to prefer showing the status bar.
    ///
    /// This changes the value returned by
    /// [`-[UIViewController prefersStatusBarHidden]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621440-prefersstatusbarhidden?language=objc),
    /// and then calls
    /// [`-[UIViewController setNeedsStatusBarAppearanceUpdate]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621354-setneedsstatusbarappearanceupdat?language=objc).
    fn set_prefers_status_bar_hidden(&self, hidden: bool);
}

impl WindowExtIOS for Window {
    #[inline]
    fn ui_window(&self) -> *mut c_void {
        self.window().ui_window()
    }

    #[inline]
    fn ui_view_controller(&self) -> *mut c_void {
        self.window().ui_view_controller()
    }

    #[inline]
    fn ui_view(&self) -> *mut c_void {
        self.window().ui_view()
    }

    #[inline]
    fn set_scale_factor(&self, scale_factor: f64) {
        self.window().set_scale_factor(scale_factor)
    }

    #[inline]
    fn set_valid_orientations(&self, valid_orientations: ValidOrientations) {
        self.window().set_valid_orientations(valid_orientations)
    }

    #[inline]
    fn set_prefers_home_indicator_hidden(&self, hidden: bool) {
        self.window().set_prefers_home_indicator_hidden(hidden)
    }

    #[inline]
    fn set_preferred_screen_edges_deferring_system_gestures(&self, edges: ScreenEdge) {
        self.window()
            .set_preferred_screen_edges_deferring_system_gestures(edges)
    }

    #[inline]
    fn set_prefers_status_bar_hidden(&self, hidden: bool) {
        self.window().set_prefers_status_bar_hidden(hidden)
    }
}

/// Additional methods on [`WindowBuilder`] that are specific to iOS.
pub trait WindowBuilderExtIOS {
    /// Sets the [`contentScaleFactor`] of the underlying [`UIWindow`] to `scale_factor`.
    ///
    /// The default value is device dependent, and it's recommended GLES or Metal applications set
    /// this to [`MonitorHandle::scale_factor()`].
    ///
    /// [`UIWindow`]: https://developer.apple.com/documentation/uikit/uiwindow?language=objc
    /// [`contentScaleFactor`]: https://developer.apple.com/documentation/uikit/uiview/1622657-contentscalefactor?language=objc
    fn with_scale_factor(self, scale_factor: f64) -> WindowBuilder;

    /// Sets the valid orientations for the [`Window`].
    ///
    /// The default value is [`ValidOrientations::LandscapeAndPortrait`].
    ///
    /// This sets the initial value returned by
    /// [`-[UIViewController supportedInterfaceOrientations]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621435-supportedinterfaceorientations?language=objc).
    fn with_valid_orientations(self, valid_orientations: ValidOrientations) -> WindowBuilder;

    /// Sets whether the [`Window`] prefers the home indicator hidden.
    ///
    /// The default is to prefer showing the home indicator.
    ///
    /// This sets the initial value returned by
    /// [`-[UIViewController prefersHomeIndicatorAutoHidden]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/2887510-prefershomeindicatorautohidden?language=objc).
    ///
    /// This only has an effect on iOS 11.0+.
    fn with_prefers_home_indicator_hidden(self, hidden: bool) -> WindowBuilder;

    /// Sets the screen edges for which the system gestures will take a lower priority than the
    /// application's touch handling.
    ///
    /// This sets the initial value returned by
    /// [`-[UIViewController preferredScreenEdgesDeferringSystemGestures]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/2887512-preferredscreenedgesdeferringsys?language=objc).
    ///
    /// This only has an effect on iOS 11.0+.
    fn with_preferred_screen_edges_deferring_system_gestures(
        self,
        edges: ScreenEdge,
    ) -> WindowBuilder;

    /// Sets whether the [`Window`] prefers the status bar hidden.
    ///
    /// The default is to prefer showing the status bar.
    ///
    /// This sets the initial value returned by
    /// [`-[UIViewController prefersStatusBarHidden]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621440-prefersstatusbarhidden?language=objc).
    fn with_prefers_status_bar_hidden(self, hidden: bool) -> WindowBuilder;
}

impl WindowBuilderExtIOS for WindowBuilder {
    fn with_scale_factor(mut self, scale_factor: f64) -> WindowBuilder {
        self.platform.scale_factor = Some(scale_factor);
        self
    }

    fn with_valid_orientations(mut self, valid_orientations: ValidOrientations) -> WindowBuilder {
        self.platform.valid_orientations = Some(valid_orientations);
        self
    }

    fn with_prefers_home_indicator_hidden(mut self, hidden: bool) -> WindowBuilder {
        self.platform.prefers_home_indicator_hidden = Some(hidden);
        self
    }

    fn with_preferred_screen_edges_deferring_system_gestures(
        mut self,
        edges: ScreenEdge,
    ) -> WindowBuilder {
        self.platform
            .preferred_screen_edges_deferring_system_gestures = Some(edges);
        self
    }

    fn with_prefers_status_bar_hidden(mut self, hidden: bool) -> WindowBuilder {
        self.platform.prefers_status_bar_hidden = Some(hidden);
        self
    }
}

#[derive(Default)]
pub(crate) struct PlatformSpecific {
    scale_factor: Option<f64>,
    valid_orientations: Option<ValidOrientations>,
    prefers_home_indicator_hidden: Option<bool>,
    preferred_screen_edges_deferring_system_gestures: Option<ScreenEdge>,
    prefers_status_bar_hidden: Option<bool>,
}

impl PlatformSpecific {
    pub(crate) fn apply_to(
        self,
        mut wb: winit::window::WindowBuilder,
    ) -> winit::window::WindowBuilder {
        if let Some(scale_factor) = self.scale_factor {
            wb = wb.with_scale_factor(scale_factor);
        }

        if let Some(valid_orientations) = self.valid_orientations {
            wb = wb.with_valid_orientations(valid_orientations);
        }

        if let Some(prefers_home_indicator_hidden) = self.prefers_home_indicator_hidden {
            wb = wb.with_prefers_home_indicator_hidden(prefers_home_indicator_hidden);
        }

        if let Some(preferred_screen_edges_deferring_system_gestures) =
            self.preferred_screen_edges_deferring_system_gestures
        {
            wb = wb.with_preferred_screen_edges_deferring_system_gestures(
                preferred_screen_edges_deferring_system_gestures,
            );
        }

        if let Some(prefers_status_bar_hidden) = self.prefers_status_bar_hidden {
            wb = wb.with_prefers_status_bar_hidden(prefers_status_bar_hidden);
        }

        wb
    }
}
