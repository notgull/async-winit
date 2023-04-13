//! An example demonstrating windows.

use async_winit::event_loop::{EventLoop, EventLoopBuilder};
use async_winit::window::Window;

#[cfg(target_os = "android")]
use async_winit::platform::android::activity::AndroidApp;

#[cfg(not(target_os = "android"))]
fn main() {
    main2(EventLoopBuilder::new().build())
}

#[cfg(target_os = "android")]
fn android_main(app: AndroidApp) {
    todo!()
}

fn main2(evl: EventLoop<()>) {
    let target = evl.window_target().clone();
    evl.block_on(async move {
        // Create a window.
        let window = Window::new().await.unwrap();

        // Wait for the window to close.
        window.close_requested().clone().await;

        // Exit.
        target.exit();
        std::future::pending().await
    });
}
