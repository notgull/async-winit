//! Combine smol's networking primitives with the async-winit reactor.
//!
//! smol's I/O reactor runs on another thread, giving the main thread a chance to
//! run the winit reactor.

use async_winit::dpi::PhysicalSize;
use async_winit::event_loop::EventLoop;
use async_winit::window::Window;

use smol::channel::bounded;
use smol::prelude::*;

use std::cell::RefCell;
use std::rc::Rc;

use softbuffer::GraphicsContext;
use tiny_skia::{Color, PixmapMut};

fn main() {
    // TODO: Convert to a library and use main2() as the entry point on Android.
    main2(EventLoop::new())
}

fn main2(event_loop: EventLoop) {
    let target = event_loop.window_target().clone();

    event_loop.block_on(async move {
        // Overall program state.
        let state = RefCell::new(State {
            running: true,
            requests: vec![],
        });

        // Create an executor to handle all of our tasks.
        let executor = Rc::new(smol::LocalExecutor::new());

        // Create a channel that signifies that the HTTP query should run again.
        let (run_again, try_again) = bounded(1);

        // Spawn the future that does the HTTP work on this executor.
        executor
            .spawn({
                let executor = executor.clone();
                let state = &state;

                async move {
                    loop {
                        // Run the URL queries.
                        if let Err(e) = make_url_queries(state, &executor).await {
                            let mut stderr_writer = smol::Unblock::new(std::io::stderr());
                            stderr_writer
                                .write_all(format!("Error: {}", e).as_bytes())
                                .await
                                .ok();
                        }

                        // Indicate that we are no longer running.
                        state.borrow_mut().running = false;

                        // Wait for the next run.
                        try_again.recv().await.ok();

                        // Indicate that we are running again.
                        state.borrow_mut().running = true;
                    }
                }
            })
            .detach();

        loop {
            // Wait for the application to become resumed, poll the executor while we do.
            executor.run(target.resumed()).await;

            // Create a window.
            let window = Window::new().await.unwrap();

            // Wait for the application to be suspended.
            let mut suspend_guard = target.suspended().wait_guard();

            // Wait for the window to close.
            let mut wait_for_close = executor.spawn({
                let window = window.clone();
                async move {
                    window.close_requested().wait_once().await;
                    None
                }
            });

            // Draw to the window.
            let draw = executor.spawn({
                let window = window.clone();
                let state = &state;
                let mut buf = vec![];

                async move {
                    let mut graphics_context = None;
                    let mut draw_guard = window.redraw_requested().wait_guard();

                    loop {
                        // Wait until we need to draw.
                        let _guard = draw_guard.wait().await;

                        // Get the window's size.
                        let size = window.inner_size().await;

                        // Get the graphics context.
                        let graphics = match &mut graphics_context {
                            Some(graphics) => graphics,
                            graphics @ None => graphics
                                .insert(unsafe { GraphicsContext::new(&window, &window) }.unwrap()),
                        };

                        // Draw with the state.
                        state.borrow().draw(graphics, &mut buf, size);
                    }
                }
            });

            // Try to re-run the HTTP queries when the "R" key is pressed.
            let rerun_http = executor.spawn({
                let state = &state;
                let window = window.clone();
                let run_again = run_again.clone();

                async move {
                    window
                        .received_character()
                        .wait_many()
                        .for_each(|ch| {
                            if ch == 'R' && !state.borrow().running {
                                run_again.try_send(()).ok();
                            }
                        })
                        .await;
                }
            });

            // Run the executor until either the window closes or the application suspends.
            let hold_guard = async {
                let hold_guard = suspend_guard.wait().await;
                Some(hold_guard)
            }
            .or(executor.run(&mut wait_for_close))
            .await;

            if let Some(guard) = hold_guard {
                // Wait for the tasks to die before suspending.
                rerun_http.cancel().await;
                wait_for_close.cancel().await;
                draw.cancel().await;
                drop((window, guard));
            } else {
                target.exit().await;
            }
        }
    });
}

async fn make_url_queries(
    state: &RefCell<State>,
    ex: &smol::LocalExecutor<'_>,
) -> std::io::Result<()> {
    // Read the urls.txt file.
    let urls_path = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "urls.txt".into());
    let urls = smol::fs::read_to_string(urls_path).await?;

    Ok(())
}

struct State {
    running: bool,
    requests: Vec<HttpRequest>,
}

impl State {
    fn draw(&self, gc: &mut GraphicsContext, buf: &mut Vec<u32>, size: PhysicalSize<u32>) {
        // Resize the buffer to the window's size.
        buf.resize((size.width * size.height) as usize, 0);

        // Create a pixmap from the buffer.
        let mut pixmap =
            PixmapMut::from_bytes(bytemuck::cast_slice_mut(buf), size.width, size.height).unwrap();

        // Fill with a solid color.
        pixmap.fill(Color::from_rgba(0.9, 0.9, 0.9, 1.0).unwrap());

        // Draw to the surface.
        gc.set_buffer(buf, size.width as u16, size.height as u16);
    }
}

struct HttpRequest {
    url: String,
    progress: u8,
}
