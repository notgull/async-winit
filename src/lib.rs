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

#![doc = include_str!("../README.md")]

// Private modules.
mod handler;
mod oneoff;
mod reactor;
mod timer;

// Modules we need to change for `async-winit`.
pub mod event_loop;
pub mod filter;
pub mod platform;
pub mod window;

pub mod event {
    #[doc(inline)]
    pub use winit::event::*;

    pub use super::window::registration::{
        AxisMotion, CursorMoved, KeyboardInput, MouseInput, MouseWheel, ScaleFactor,
        ScaleFactorChanged, ScaleFactorChanging, TouchpadMagnify, TouchpadPressure, TouchpadRotate,
    };
}

// Modules that can just be re-exported in `async-winit`.
#[doc(inline)]
pub use winit::{dpi, error, monitor};

pub use handler::{Event, Handler, WaitGuard, WaitMany, WaitOnce};
pub use timer::Timer;
