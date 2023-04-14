//! Android-specific platform code.

#[doc(inline)]
pub use winit::platform::android::activity;

use crate::event_loop::EventLoopBuilder;
use activity::AndroidApp;

use winit::platform::android::EventLoopBuilderExtAndroid as _;
use winit::window::WindowBuilder;

pub trait EventLoopBuilderExtAndroid {
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
