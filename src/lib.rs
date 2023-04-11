//! Use [`winit`] like the `async` runtime you've always wanted.

// Private modules.
mod reactor;

// Modules we need to change for `async-winit`.
pub mod event_loop;
pub mod platform;
pub mod window;

// Modules that can just be re-exported in `async-winit`.
#[doc(inline)]
pub use winit::{dpi, error, event, monitor};
