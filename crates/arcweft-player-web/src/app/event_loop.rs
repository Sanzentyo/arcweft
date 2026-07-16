use super::registry::WebPlayerControl;
use super::{BrowserApp, WebPlayerError};
use std::cell::RefCell;
use std::rc::Rc;
use winit::event_loop::{EventLoop, EventLoopProxy};

thread_local! {
    static SHARED_EVENT_LOOP: RefCell<SharedEventLoopState> =
        const { RefCell::new(SharedEventLoopState::Uninitialized) };
}

enum SharedEventLoopState {
    Uninitialized,
    Running(EventLoopProxy),
    Failed(String),
}

enum Attachment {
    Existing(EventLoopProxy),
    Start {
        event_loop: EventLoop,
        proxy: EventLoopProxy,
    },
}

/// Attaches one player control to the page-owned event loop.
///
/// Winit permits exactly one event loop per page. Later players share its proxy;
/// the global application handler discovers their controls from the registry.
pub(super) fn attach(control: &Rc<WebPlayerControl>) -> Result<(), WebPlayerError> {
    let attachment = SHARED_EVENT_LOOP.with(|shared| {
        let mut shared = shared.borrow_mut();
        match &*shared {
            SharedEventLoopState::Running(proxy) => Ok(Attachment::Existing(proxy.clone())),
            SharedEventLoopState::Failed(message) => {
                Err(WebPlayerError::EventLoop(message.clone()))
            }
            SharedEventLoopState::Uninitialized => {
                let event_loop = EventLoop::new()
                    .map_err(|error| WebPlayerError::EventLoop(error.to_string()))?;
                let proxy = event_loop.create_proxy();
                *shared = SharedEventLoopState::Running(proxy.clone());
                Ok(Attachment::Start { event_loop, proxy })
            }
        }
    })?;

    let proxy = match attachment {
        Attachment::Existing(proxy) => proxy,
        Attachment::Start { event_loop, proxy } => {
            control.install_event_loop_proxy(proxy.clone());
            if let Err(error) = event_loop.run_app(BrowserApp) {
                let message = error.to_string();
                SHARED_EVENT_LOOP.with(|shared| {
                    *shared.borrow_mut() = SharedEventLoopState::Failed(message.clone());
                });
                return Err(WebPlayerError::EventLoop(message));
            }
            control.mark_running();
            proxy.wake_up();
            return Ok(());
        }
    };

    control.install_event_loop_proxy(proxy.clone());
    control.mark_running();
    proxy.wake_up();
    Ok(())
}
