//! An example demonstrating windows.

use std::time::Duration;

use async_winit::event_loop::{EventLoop, EventLoopBuilder};
use async_winit::window::Window;
use async_winit::Timer;

use futures_lite::prelude::*;

#[cfg(target_os = "android")]
use async_winit::platform::android::activity::AndroidApp;

#[cfg(not(target_os = "android"))]
fn main() {
    main2(EventLoopBuilder::new().build())
}

#[cfg(target_os = "android")]
fn android_main(app: AndroidApp) {
    use async_winit::platform::android::EventLoopBuilderExtAndroid as _;
    main2(EventLoopBuilder::new().with_android_app(app).build())
}

fn main2(evl: EventLoop) {
    let target = evl.window_target().clone();
    evl.block_on(async move {
        // Wait for a resume event to start.
        target.resumed().await;

        // Create a window.
        let window = Window::new().await.unwrap();

        // Print resize events.
        let print_resize = {
            window.resized().wait_many().for_each(|new_size| {
                println!("Window resized to {:?}", new_size);
            })
        };

        // Print the position every second.
        let print_position = {
            Timer::interval(Duration::from_secs(1))
                .then(|_| window.inner_position())
                .for_each(|posn| {
                    println!("Window position: {:?}", posn);
                })
        };

        // Wait for the window to close.
        window
            .close_requested()
            .wait_once()
            .or(print_resize)
            .or(print_position)
            .await;

        // Exit.
        target.exit();
        std::future::pending().await
    });
}
