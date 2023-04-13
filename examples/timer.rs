//! An example using timers.

use std::time::Duration;

use async_winit::event_loop::{EventLoop, EventLoopBuilder};
use async_winit::window::Window;
use async_winit::Timer;

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

        // Wait one second.
        Timer::after(Duration::from_secs(1)).await;

        // Close the window.
        window.close_requested().clone().await;

        target.exit();
        std::future::pending().await
    });
}
