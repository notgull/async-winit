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

#[cfg(android_platform)]
pub mod android;

#[cfg(ios_platform)]
pub mod ios;

#[cfg(macos_platform)]
pub mod macos;

#[cfg(orbital_platform)]
pub mod orbital;

#[cfg(x11_platform)]
pub mod x11;

#[cfg(wayland_platform)]
pub mod wayland;

#[cfg(windows)]
pub mod windows;

#[cfg(any(windows, x11_platform, wayland_platform))]
pub mod run_return;

cfg_if::cfg_if! {
    if #[cfg(android_platform)] {
        pub(crate) use android::PlatformSpecific;
    } else if #[cfg(ios_platform)] {
        pub(crate) use ios::PlatformSpecific;
    } else if #[cfg(macos_platform)] {
        pub(crate) use macos::PlatformSpecific;
    } else if #[cfg(orbital_platform)] {
        pub(crate) use orbital::PlatformSpecific;
    } else if #[cfg(any(x11_platform, wayland_platform))] {
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

mod __private {
    use crate::event_loop::{EventLoop, EventLoopBuilder, EventLoopWindowTarget};
    use crate::window::{Window, WindowBuilder};

    #[doc(hidden)]
    pub struct Internal(());

    macro_rules! sealed_trait {
        ($($name: ident $tname: ident)*) => {$(
            #[doc(hidden)]
            pub trait $tname {
                fn __sealed_marker(i: Internal);
            }

            impl $tname for $name {
                fn __sealed_marker(_: Internal) {}
            }
        )*}
    }

    sealed_trait! {
        EventLoopWindowTarget EventLoopWindowTargetPrivate
        EventLoop EventLoopPrivate
        EventLoopBuilder EventLoopBuilderPrivate
        Window WindowPrivate
        WindowBuilder WindowBuilderPrivate
    }
}
