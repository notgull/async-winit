/*

`async-winit` is free software: you can redistribute it and/or modify it under the terms of one of
the following licenses:

- The GNU Affero General Public License as published by the Free Software Foundation, either version
  3 of the License, or (at your option) any later version.
- The Patron License at https://github.com/notgull/async-winit/blob/main/LICENSE-PATRON.md, for
  sponsors and contributors, who can ignore the copyleft provisions of the GNU AGPL for this project.

`async-winit` is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even
the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General
Public License and the Patron License for more details.

You should have received a copy of the GNU Affero General Public License and the corresponding Patron
License along with `async-winit`. If not, see <https://www.gnu.org/licenses/>.

*/

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
        target.exit().await
    });
}
