//! Registration of the window into the reactor.

use crate::handler::Handler;

use winit::event::WindowEvent;

pub(crate) struct Registration {
    /// `Event::CloseRequested`.
    pub(crate) close_requested: Handler<()>,
}

impl Registration {
    pub(crate) fn new() -> Self {
        Self {
            close_requested: Handler::new(),
        }
    }

    pub(crate) fn signal(&self, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.close_requested.run_with(&mut ()),
            _ => {}
        }
    }
}
