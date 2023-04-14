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
