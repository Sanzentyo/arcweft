//! Routed presentation input to retained view handler dispatch.

use crate::{EventKind, NodeId, ViewError, ViewFragment, ViewSemanticFragment};
use arcweft_presentation::input::{InputEpoch, InputEvent, InteractionTarget};
use serde::{Deserialize, Serialize};

/// Opaque frame-scoped identity of one published mount/event token route.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ViewHandlerRouteId([u8; 32]);

/// One stable event route emitted from a retained fragment node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewHandlerRoute {
    node: NodeId,
    target: InteractionTarget,
    event: EventKind,
    route: ViewHandlerRouteId,
}

/// Ordered handler routes for one View layer output.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewHandlerRouteTable {
    routes: Vec<ViewHandlerRoute>,
}

/// One handler invocation accepted from an already routed presentation input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewHandlerInvocation {
    raw_epoch: InputEpoch,
    target: InteractionTarget,
    event: EventKind,
    route: ViewHandlerRouteId,
}

impl ViewHandlerRouteId {
    #[must_use]
    pub const fn from_digest(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl ViewHandlerRoute {
    pub const fn node(&self) -> NodeId {
        self.node
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub const fn event(&self) -> EventKind {
        self.event
    }

    pub const fn route(&self) -> ViewHandlerRouteId {
        self.route
    }

    pub fn accepts(&self, input: &InputEvent) -> bool {
        input.target() == self.target() && self.event.accepts(input.kind())
    }
}

impl ViewHandlerRouteTable {
    pub fn from_fragment(
        fragment: &ViewFragment,
        semantics: &ViewSemanticFragment,
    ) -> Result<Self, ViewError> {
        let mut routes = Vec::new();
        for (index, node) in fragment.nodes().iter().enumerate() {
            let node_id = NodeId(u32::try_from(index).map_err(|_| ViewError::CapacityExceeded)?);
            let events = fragment
                .node_events(node_id)
                .ok_or(ViewError::InvalidFragmentNode(node_id))?;
            if events.is_empty() {
                continue;
            }
            let semantic = node
                .semantics()
                .ok_or(ViewError::HandlerNodeMissingSemantics(node_id))?;
            let target = semantics
                .get(semantic)
                .ok_or(ViewError::UnknownHandlerSemantic {
                    node: node_id,
                    semantic,
                })?
                .target()
                .clone();
            routes.extend(events.iter().copied().map(|binding| ViewHandlerRoute {
                node: node_id,
                target: target.clone(),
                event: binding.kind(),
                route: binding.route(),
            }));
        }
        Ok(Self { routes })
    }

    pub fn as_slice(&self) -> &[ViewHandlerRoute] {
        &self.routes
    }

    pub fn dispatch_input(&self, input: &InputEvent) -> Vec<ViewHandlerInvocation> {
        self.routes
            .iter()
            .filter(|route| route.accepts(input))
            .filter_map(|route| {
                ViewHandlerInvocation::from_input(input, route.event(), route.route())
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

impl ViewHandlerInvocation {
    /// Seals an invocation only from an already routed presentation event.
    #[must_use]
    pub fn from_input(
        input: &InputEvent,
        event: EventKind,
        route: ViewHandlerRouteId,
    ) -> Option<Self> {
        event.accepts(input.kind()).then(|| Self {
            raw_epoch: input.raw_epoch(),
            target: input.target().clone(),
            event,
            route,
        })
    }

    pub const fn raw_epoch(&self) -> InputEpoch {
        self.raw_epoch
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub const fn event(&self) -> EventKind {
        self.event
    }

    pub const fn route(&self) -> ViewHandlerRouteId {
        self.route
    }
}
