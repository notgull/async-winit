//! Features for both X11 and Wayland.

use super::x11::XWindowType;
use crate::dpi::Size;

#[derive(Default)]
pub(crate) struct PlatformSpecific {
    x11: super::x11::PlatformSpecific,
    wayland: super::wayland::PlatformSpecific,
}

impl PlatformSpecific {
    pub(crate) fn set_x11_window_type(&mut self, x11_window_type: Vec<XWindowType>) {
        self.x11.set_x11_window_type(x11_window_type);
    }

    pub(crate) fn set_x11_name(&mut self, x11_name: (String, String)) {
        self.x11.set_x11_name(x11_name.clone());
        self.wayland.set_x11_name(x11_name);
    }

    pub(crate) fn set_x11_screen_id(&mut self, x11_screen_id: i32) {
        self.x11.set_x11_screen_id(x11_screen_id);
    }

    pub(crate) fn set_x11_override_redirect(&mut self, x11_override_redirect: bool) {
        self.x11.set_x11_override_redirect(x11_override_redirect);
    }

    pub(crate) fn set_x11_base_size(&mut self, x11_base_size: Size) {
        self.x11.set_x11_base_size(x11_base_size);
    }

    pub(crate) fn apply_to(self, wb: winit::window::WindowBuilder) -> winit::window::WindowBuilder {
        let Self { x11, wayland } = self;

        x11.apply_to(wayland.apply_to(wb))
    }
}
