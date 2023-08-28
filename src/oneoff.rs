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

//! One-off channel, which handles completions of ongoing events.

// TODO: This implementation uses a full channel, which allocates and is overall very inefficient.
//       We should use a leaner implementation later.

use crate::sync::{ThreadSafety, __private::*};

/// A oneoff channel that can be used to receive a single event.
pub(crate) struct Oneoff<T, TS: ThreadSafety> {
    /// The channel used to receive the event.
    rx: TS::Receiver<T>,
}

impl<T, TS: ThreadSafety> Oneoff<T, TS> {
    /// Wait for the event to be sent.
    pub(crate) async fn recv(self) -> T {
        self.rx.recv().await.unwrap()
    }
}

/// The sender end of the oneoff channel.
pub(crate) struct Complete<T, TS: ThreadSafety> {
    /// The channel used to send the event.
    tx: TS::Sender<T>,
}

impl<T, TS: ThreadSafety> Complete<T, TS> {
    /// Send the event.
    pub(crate) fn send(self, event: T) {
        self.tx.try_send(event).ok();
    }
}

/// Create a pair of oneoff channels.
pub(crate) fn oneoff<T, TS: ThreadSafety>() -> (Complete<T, TS>, Oneoff<T, TS>) {
    let (tx, rx) = TS::channel_bounded(1);

    (Complete { tx }, Oneoff { rx })
}
