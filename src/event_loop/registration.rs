//! Registration for lifecycle events.

use crate::handler::Handler;

pub(crate) struct Registration {
    pub(crate) resumed: Handler<()>,
    pub(crate) suspended: Handler<()>,
}

impl Registration {
    pub(crate) fn new() -> Self {
        Self {
            resumed: Handler::new(),
            suspended: Handler::new(),
        }
    }
}
