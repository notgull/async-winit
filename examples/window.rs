/*

`async-winit` is free software: you can redistribute it and/or modify it under the terms of one of
the following licenses:

* GNU Lesser General Public License as published by the Free Software Foundation, either
  version 3 of the License, or (at your option) any later version.
* Mozilla Public License as published by the Mozilla Foundation, version 2.

`async-winit` is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even
the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General
Public License and the Patron License for more details.

You should have received a copy of the GNU Lesser General Public License and the Mozilla
Public License along with `async-winit`. If not, see <https://www.gnu.org/licenses/>.

*/

//! An example demonstrating windows.

use std::time::Duration;

use async_winit::event_loop::{EventLoop, EventLoopBuilder};
use async_winit::window::Window;
use async_winit::{ThreadUnsafe, Timer};

use futures_lite::prelude::*;
use softbuffer::GraphicsContext;

fn main() {
    main2(EventLoopBuilder::new().build())
}

fn main2(evl: EventLoop<ThreadUnsafe>) {
    let target = evl.window_target().clone();
    evl.block_on(async move {
        // Wait for a resume event to start.
        target.resumed().await;

        // Create a window.
        let window = Window::<ThreadUnsafe>::new().await.unwrap();

        // Print resize events.
        let print_resize = {
            window.resized().wait().for_each(|new_size| {
                println!("Window resized to {:?}", new_size);
            })
        };

        // Print the position every second.
        let print_position = {
            Timer::<ThreadUnsafe>::interval(Duration::from_secs(1))
                .then(|_| window.inner_position())
                .for_each(|posn| {
                    println!("Window position: {:?}", posn);
                })
        };

        // Drawing.
        let draw = {
            let window = window.clone();
            let mut sb = None;
            let mut buf = vec![];

            async move {
                let mut waiter = window.redraw_requested().wait();

                loop {
                    let _guard = waiter.hold().await;
                    let inner_size = window.inner_size().await;

                    // Get the softbuffer.
                    let graphics = match &mut sb {
                        Some(graphics) => graphics,
                        sb @ None => {
                            let graphics =
                                unsafe { GraphicsContext::new(&window, &window) }.unwrap();

                            sb.insert(graphics)
                        }
                    };

                    // Draw.
                    let pixel = 0xAA11AA11;
                    buf.resize(
                        inner_size.width as usize * inner_size.height as usize,
                        pixel,
                    );
                    graphics.set_buffer(&buf, inner_size.width as u16, inner_size.height as u16);
                }
            }
        };

        // Wait for the window to close.
        async { window.close_requested().wait().await }
            .or(print_resize)
            .or(print_position)
            .or(draw)
            .await;

        // Exit.
        target.exit().await
    });
}
