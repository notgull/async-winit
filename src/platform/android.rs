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

//! Android-specific platform code.

use super::__private as sealed;

#[doc(inline)]
pub use winit::platform::android::activity;

use crate::event_loop::EventLoopBuilder;
use activity::AndroidApp;

use winit::platform::android::EventLoopBuilderExtAndroid as _;
use winit::window::WindowBuilder;

/// Additional methods on [`EventLoopBuilder`] specific to Android.
///
/// [`EventLoopBuilder`]: crate::event_loop::EventLoopBuilder
pub trait EventLoopBuilderExtAndroid: sealed::EventLoopBuilderPrivate {
    /// Associates the `AndroidApp` that was passed to `android_main()` with the event loop
    ///
    /// This must be called on Android since the `AndroidApp` is not global state.
    fn with_android_app(&mut self, app: AndroidApp) -> &mut Self;
}

impl EventLoopBuilderExtAndroid for EventLoopBuilder {
    fn with_android_app(&mut self, app: AndroidApp) -> &mut Self {
        self.inner.with_android_app(app);
        self
    }
}

#[derive(Default)]
pub(crate) struct PlatformSpecific;

impl PlatformSpecific {
    pub(crate) fn apply_to(self, builder: WindowBuilder) -> WindowBuilder {
        builder
    }
}
