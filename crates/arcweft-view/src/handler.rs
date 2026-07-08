//! Routed presentation input to retained view handler dispatch.

use crate::{EventKind, HandlerId, NodeId, ViewError, ViewFragment, ViewSemanticFragment};
use arcweft_presentation::input::{InputEpoch, InputEvent, InteractionTarget};

/// One stable event route emitted from a retained fragment node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewHandlerRoute {
    node: NodeId,
    target: InteractionTarget,
    event: EventKind,
    handler: HandlerId,
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
    node: NodeId,
    target: InteractionTarget,
    event: EventKind,
    handler: HandlerId,
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

    pub const fn handler(&self) -> HandlerId {
        self.handler
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
                handler: binding.handler(),
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
            .map(|route| ViewHandlerInvocation {
                raw_epoch: input.raw_epoch(),
                node: route.node(),
                target: route.target().clone(),
                event: route.event(),
                handler: route.handler(),
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

impl ViewHandlerInvocation {
    pub const fn raw_epoch(&self) -> InputEpoch {
        self.raw_epoch
    }

    pub const fn node(&self) -> NodeId {
        self.node
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub const fn event(&self) -> EventKind {
        self.event
    }

    pub const fn handler(&self) -> HandlerId {
        self.handler
    }
}
