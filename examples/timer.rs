//! An example using timers.

use std::time::Duration;

use async_winit::event_loop::{EventLoop, EventLoopBuilder};
use async_winit::Timer;

fn main() {
    main2(EventLoopBuilder::new().build())
}

fn main2(evl: EventLoop) {
    let target = evl.window_target().clone();
    evl.block_on(async move {
        // Wait one second.
        Timer::after(Duration::from_secs(1)).await;

        // Exit.
        target.exit();
        std::future::pending().await
    });
}
