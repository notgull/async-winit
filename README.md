# async-winit

Use `winit` like the `async` runtime you've always wanted.

`winit` is actually asynchronous, contrary to popular belief; it's just not `async`. It uses an event loop to handle events, which is an good fit for some cases but not others. The maintainers of `winit` have referred to this type of event loop as "poor man's `async`"; a system that is not `async` but is still asynchronous.

This crate builds an `async` interface on top of this event loop.

## Example

Consider the following `winit` program, which creates a window and prints the size of the window when it is resized:

```rust
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::Window;

fn main2(evl: EventLoop<()>) {
    let mut window = None;

    evl.run(move |event, elwt, flow| {
        match event {
            Event::Resumed => {
                // Application is active; create a window.
                window = Some(Window::new(elwt).unwrap());
            },

            Event::Suspended => {
                // Application is inactive; destroy the window.
                window = None;
            },

            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    // Window is closed; exit the application.
                    flow.set_exit();
                },

                WindowEvent::Resized(size) => {
                    println!("{:?}", size);
                }

                _ => {},
            },

            _ => {},
        }
    });
}

fn main() {
#   return;
    let evl = EventLoop::new();
    main2(evl);
}
```

This strategy is a bit long winded. Now, compare against the equivalent `async-winit` program:

```rust
use async_winit::event_loop::EventLoop;
use async_winit::window::Window;
use async_winit::ThreadUnsafe;
use futures_lite::prelude::*;

fn main2(evl: EventLoop<ThreadUnsafe>) {
    let window_target = evl.window_target().clone();

    evl.block_on(async move {
        loop {
            // Wait for the application to be active.
            window_target.resumed().await;

            // Create a window.
            let window = Window::<ThreadUnsafe>::new().await.unwrap();

            // Print the size of the window when it is resized.
            let print_size = async {
                window
                    .resized()
                    .wait_many()
                    .for_each(|size| {
                        println!("{:?}", size);
                    })
                    .await;

                true
            };

            // Wait until the window is closed.
            let close = async {
                window.close_requested().wait_once().await;
                println!("Close");
                true
            };

            // Wait until the application is suspended.
            let suspend = async {
                window_target.suspended().wait_once().await;
                false
            };

            // Run all of these at once.
            let needs_exit = print_size.or(close).or(suspend).await;

            // If we need to exit, exit. Otherwise, loop again, destroying the window.
            if needs_exit {
                window_target.exit().await;
            } else {
                drop(window);
            }
        }
    });
}

fn main() {
#   return;
    let evl = EventLoop::new();
    main2(evl);
}
```

In my opinion, the flatter `async` style is much easier to read and understand. Your mileage may vary.

## Pros

- In many cases it may make more sense to think of a program as an `async` task, rather than an event loop.
- You don't need to tie everything to the `EventLoopWindowTarget`; `Window::new()` and other functions take no parameters and can be called from anywhere as long as an `EventLoop` is running somewhere.
- You can use the `async` ecosystem to its full potential here.

## Cons

- There is a not insignificant amount of overhead involved in using `async-winit`. This is because `async-winit` is built on top of `winit`, which is built on top of `winit`'s event loop. This means that `async-winit` has to convert between `async` and `winit`'s event loop, which is not free.
- `async-winit` is not as low level as `winit`. This means that you can't do everything that you can do with `winit`.
  - For instance, data cannot be shared mutable between individual tasks. This can be easily worked around with `RefCell` in simple cases, but still requires additional thought for shared state.

## Credits

`async-winit` was created by John Nunley ([@notgull](https://github.com/notgull)).

This project is heavily based on [`async-io`] by Stjepan Glavina et al, as well as [`winit`] by Pierre Kreiger et al.

[`async-io`]: https://crates.io/crates/async-io
[`winit`]: https://crates.io/crates/winit

## License 

`async-winit` is free software: you can redistribute it and/or modify it under the terms of one of the following licenses:

* GNU Lesser General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
* Mozilla Public License as published by the Mozilla Foundation, version 2. 

`async-winit` is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Lesser General Public License or the Mozilla Public License for more details.

You should have received a copy of the GNU Lesser General Public License and the Mozilla Public License along with `async-winit`. If not, see <https://www.gnu.org/licenses/>. 
