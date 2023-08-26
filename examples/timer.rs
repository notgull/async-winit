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

//! An example using timers.

use std::time::Duration;

use async_winit::event_loop::{EventLoop, EventLoopBuilder};
use async_winit::{ThreadUnsafe, Timer};

fn main() {
    main2(EventLoopBuilder::new().build())
}

fn main2(evl: EventLoop<ThreadUnsafe>) {
    let target = evl.window_target().clone();
    evl.block_on(async move {
        // Wait one second.
        Timer::<ThreadUnsafe>::after(Duration::from_secs(1)).await;

        // Exit.
        target.exit().await
    });
}
