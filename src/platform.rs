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
