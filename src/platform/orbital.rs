/*

`async-winit` is free software: you can redistribute it and/or modify it under the terms of one of
the following licenses:

* GNU Lesser General Public License as published by the Free Software Foundation, either
  version 3 of the License, or (at your option) any later version.
* Mozilla Public License as published by the Mozilla Foundation, version 2.

`async-winit` is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even
the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General
Public License and the Patron License for more details.

You should have received a copy of the GNU Lesser General Public License and the Mozilla
Public License along with `async-winit`. If not, see <https://www.gnu.org/licenses/>.

*/

// This file is partially derived from `winit`, which was originally created by Pierre Krieger and
// contributers. It was originally released under the MIT license.

// No platform-specific orbital code yet.

use winit::window::WindowBuilder;

#[derive(Default)]
pub(crate) struct PlatformSpecific;

impl PlatformSpecific {
    pub(crate) fn apply_to(self, builder: WindowBuilder) -> WindowBuilder {
        builder
    }
}
