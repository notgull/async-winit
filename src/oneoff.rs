//! One-off channel, which handles completions of ongoing events.

// TODO: This implementation uses a full channel, which allocates and is overall very inefficient.
//       We should use a leaner implementation later.

use async_channel::{Receiver, Sender};

/// A oneoff channel that can be used to receive a single event.
pub(crate) struct Oneoff<T> {
    /// The channel used to receive the event.
    rx: Receiver<T>,
}

impl<T> Oneoff<T> {
    /// Wait for the event to be sent.
    pub(crate) async fn recv(self) -> T {
        self.rx.recv().await.unwrap()
    }
}

/// The sender end of the oneoff channel.
pub(crate) struct Complete<T> {
    /// The channel used to send the event.
    tx: Sender<T>,
}

impl<T> Complete<T> {
    /// Send the event.
    pub(crate) async fn send(self, event: T) {
        self.tx.send(event).await.ok();
    }
}

/// Create a pair of oneoff channels.
pub(crate) fn oneoff<T>() -> (Oneoff<T>, Complete<T>) {
    let (tx, rx) = async_channel::bounded(1);

    (Oneoff { rx }, Complete { tx })
}
