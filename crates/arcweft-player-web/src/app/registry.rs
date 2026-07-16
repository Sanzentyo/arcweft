use super::PlayerState;
use super::environment::WebEnvironmentError;
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};
use winit::event_loop::EventLoopProxy;

thread_local! {
    static REGISTRY: RefCell<WebPlayerRegistry> = RefCell::new(WebPlayerRegistry::default());
    static NEXT_PLAYER_ID: Cell<u32> = const { Cell::new(1) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WebPlayerLifecycle {
    Starting,
    Running,
    Closed,
}

pub(super) struct WebPlayerControl {
    pub(super) player: RefCell<Option<PlayerState>>,
    pub(super) lifecycle: Cell<WebPlayerLifecycle>,
    pub(super) registry_retained: Cell<bool>,
    pub(super) canvas_id: String,
    id: u32,
    event_loop_proxy: OnceCell<EventLoopProxy>,
}

#[derive(Default)]
struct WebPlayerRegistry {
    retained: BTreeMap<u32, Rc<WebPlayerControl>>,
    canvas_index: BTreeMap<String, Weak<WebPlayerControl>>,
}

impl WebPlayerControl {
    pub(super) fn id(&self) -> u32 {
        self.id
    }

    pub(super) fn is_closed(&self) -> bool {
        self.lifecycle.get() == WebPlayerLifecycle::Closed
    }

    pub(super) fn mark_running(&self) {
        if !self.is_closed() {
            self.lifecycle.set(WebPlayerLifecycle::Running);
        }
    }

    pub(super) fn install_event_loop_proxy(&self, proxy: EventLoopProxy) {
        self.event_loop_proxy
            .set(proxy)
            .expect("one Web player control owns exactly one winit event loop");
    }

    pub(super) fn request_event_loop_wake(&self) {
        if let Some(proxy) = self.event_loop_proxy.get() {
            proxy.wake_up();
        }
    }

    fn close(&self) {
        if self.lifecycle.replace(WebPlayerLifecycle::Closed) == WebPlayerLifecycle::Closed {
            return;
        }
        if let Ok(mut player) = self.player.try_borrow_mut() {
            player.take();
        }
        self.request_event_loop_wake();
    }
}

pub(super) fn create_control(
    canvas_id: String,
    player: PlayerState,
) -> Result<Rc<WebPlayerControl>, WebEnvironmentError> {
    REGISTRY.with(|registry| {
        let mut registry = registry
            .try_borrow_mut()
            .map_err(|_| WebEnvironmentError::reentrant_update(None))?;
        if registry
            .canvas_index
            .get(&canvas_id)
            .and_then(Weak::upgrade)
            .is_some_and(|control| !control.is_closed())
        {
            return Err(WebEnvironmentError::canvas_in_use(&canvas_id));
        }
        registry.canvas_index.remove(&canvas_id);
        let id = allocate_player_id()?;
        let control = Rc::new(WebPlayerControl {
            player: RefCell::new(Some(player)),
            lifecycle: Cell::new(WebPlayerLifecycle::Starting),
            registry_retained: Cell::new(false),
            canvas_id: canvas_id.clone(),
            id,
            event_loop_proxy: OnceCell::new(),
        });
        registry
            .canvas_index
            .insert(canvas_id, Rc::downgrade(&control));
        Ok(control)
    })
}

pub(super) fn retain(control: &Rc<WebPlayerControl>) -> Result<(), WebEnvironmentError> {
    REGISTRY.with(|registry| {
        let mut registry = registry
            .try_borrow_mut()
            .map_err(|_| WebEnvironmentError::reentrant_update(Some(control.id())))?;
        if control.is_closed() {
            return Err(WebEnvironmentError::player_closed(control.id()));
        }
        registry.retained.insert(control.id(), Rc::clone(control));
        control.registry_retained.set(true);
        Ok(())
    })
}

pub(super) fn lookup(player_id: u32) -> Result<Rc<WebPlayerControl>, WebEnvironmentError> {
    REGISTRY.with(|registry| {
        let registry = registry
            .try_borrow()
            .map_err(|_| WebEnvironmentError::reentrant_update(Some(player_id)))?;
        registry
            .retained
            .get(&player_id)
            .cloned()
            .ok_or_else(|| WebEnvironmentError::unknown_player(player_id))
    })
}

pub(super) fn active_controls() -> Vec<Rc<WebPlayerControl>> {
    REGISTRY.with(|registry| {
        registry
            .try_borrow()
            .map(|registry| {
                registry
                    .canvas_index
                    .values()
                    .filter_map(Weak::upgrade)
                    .filter(|control| !control.is_closed())
                    .collect()
            })
            .unwrap_or_default()
    })
}

pub(super) fn stop(player_id: u32) -> Result<(), WebEnvironmentError> {
    let control = lookup(player_id)?;
    shutdown(&control)
}

pub(super) fn shutdown(control: &Rc<WebPlayerControl>) -> Result<(), WebEnvironmentError> {
    release(control)?;
    control.close();
    Ok(())
}

pub(super) fn shutdown_after_event_loop_failure(control: &Rc<WebPlayerControl>) {
    let _ = release(control);
    control.close();
}

pub(super) fn shutdown_on_drop(control: &Rc<WebPlayerControl>) {
    let _ = release(control);
    control.close();
}

fn release(control: &Rc<WebPlayerControl>) -> Result<(), WebEnvironmentError> {
    REGISTRY.with(|registry| {
        let mut registry = registry
            .try_borrow_mut()
            .map_err(|_| WebEnvironmentError::reentrant_update(Some(control.id())))?;
        if registry
            .retained
            .get(&control.id())
            .is_some_and(|retained| Rc::ptr_eq(retained, control))
        {
            registry.retained.remove(&control.id());
        }
        control.registry_retained.set(false);
        let owned_canvas = registry
            .canvas_index
            .get(&control.canvas_id)
            .is_some_and(|indexed| Weak::ptr_eq(indexed, &Rc::downgrade(control)));
        if owned_canvas {
            registry.canvas_index.remove(&control.canvas_id);
        }
        Ok(())
    })
}

fn allocate_player_id() -> Result<u32, WebEnvironmentError> {
    NEXT_PLAYER_ID.with(|next| {
        let id = next.get();
        if id == 0 {
            return Err(WebEnvironmentError::player_id_overflow());
        }
        next.set(id.checked_add(1).unwrap_or(0));
        Ok(id)
    })
}
