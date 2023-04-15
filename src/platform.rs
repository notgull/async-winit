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

//! Platform specific code.

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_os = "ios")]
pub mod ios;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "redox")]
pub mod orbital;

#[cfg(all(
    unix,
    not(any(target_os = "android", target_os = "macos", target_os = "ios",)),
    feature = "x11"
))]
pub mod x11;

#[cfg(all(
    unix,
    not(any(target_os = "android", target_os = "macos", target_os = "ios",)),
    feature = "wayland"
))]
pub mod wayland;

#[cfg(windows)]
pub mod windows;

#[cfg(all(any(unix, windows, target_os = "redox"), not(target_os = "ios")))]
pub mod run_return;

cfg_if::cfg_if! {
    if #[cfg(target_os = "android")] {
        pub(crate) use android::PlatformSpecific;
    } else if #[cfg(target_os = "ios")] {
        pub(crate) use ios::PlatformSpecific;
    } else if #[cfg(target_os = "macos")] {
        pub(crate) use macos::PlatformSpecific;
    } else if #[cfg(target_os = "redox")] {
        pub(crate) use orbital::PlatformSpecific;
    } else if #[cfg(all(
        unix,
        not(any(target_os = "android", target_os = "macos", target_os = "ios",)),
    ))] {
        #[cfg(all(feature = "x11", not(feature = "wayland")))]
        pub(crate) use x11::PlatformSpecific;

        #[cfg(all(not(feature = "x11"), feature = "wayland"))]
        pub(crate) use wayland::PlatformSpecific;

        #[cfg(all(feature = "x11", feature = "wayland"))]
        mod free_unix;
        #[cfg(all(feature = "x11", feature = "wayland"))]
        pub(crate) use free_unix::PlatformSpecific;
    } else if #[cfg(windows)] {
        pub(crate) use windows::PlatformSpecific;
    }
}
