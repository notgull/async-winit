//! An example using timers.

use std::time::Duration;

use async_winit::event_loop::EventLoop;
use async_winit::Timer;

#[cfg(not(target_os = "android"))]
fn main() {
    main2(EventLoop::new())
}

fn main2(evl: EventLoop<()>) {
    evl.block_on(async move {
        Timer::after(Duration::from_secs(1)).await;
        std::process::exit(0)
    });
}
