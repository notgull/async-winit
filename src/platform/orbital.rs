// No platform-specific orbital code yet.

use winit::window::WindowBuilder;

#[derive(Default)]
pub(crate) struct PlatformSpecific;

impl PlatformSpecific {
    pub(crate) fn apply_to(self, builder: WindowBuilder) -> WindowBuilder {
        builder
    }
}
