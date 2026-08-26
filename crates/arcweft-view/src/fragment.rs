//! Flat retained View fragments emitted by Rust and Arcweft views.

use crate::{
    NodeKey, RawEntity, ViewError, ViewHandlerRouteId, ViewStyleApplicationTarget,
    program::EventKind,
};
use arcweft_presentation::input::InputEventKind;
use std::collections::BTreeSet;

/// Frame-local node identifier inside one retained fragment.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId(pub u32);

/// Compact index span into a sidecar vector.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Span32 {
    pub start: u32,
    pub len: u32,
}

/// Stable identifier for plain text content owned outside the fragment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextSourceId(pub u32);

/// Stable identifier for rich text content owned outside the fragment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RichTextSourceId(pub u32);

/// Stable identifier for image resources owned outside the fragment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImageId(pub u32);

/// Stable identifier for renderer/host-defined custom elements.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CustomElementId(pub u32);

/// Stable identifier for semantic metadata owned outside the fragment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticSpecId(pub u32);

/// Stable identifier for an event handler lowered by the view runtime.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HandlerId(pub u32);

/// Container layout intent before style/layout resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerKind {
    Block,
    Inline,
    Stack,
}

/// A retained View node payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FragmentKind {
    Container(ContainerKind),
    Text(TextSourceId),
    RichText(RichTextSourceId),
    Image(ImageId),
    View(RawEntity),
    Custom(CustomElementId),
}

/// View event binding stored as pure data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventBinding {
    kind: EventKind,
    route: ViewHandlerRouteId,
}

/// One flat retained fragment node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FragmentNode {
    key: NodeKey,
    kind: FragmentKind,
    style_applications: Span32,
    children: Span32,
    events: Span32,
    semantics: Option<SemanticSpecId>,
}

/// Flat retained fragment plus ordered sidecar vectors.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewFragment {
    nodes: Vec<FragmentNode>,
    style_applications: Vec<ViewStyleApplicationTarget>,
    child_indices: Vec<NodeId>,
    events: Vec<EventBinding>,
}

/// Builder for a flat retained fragment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewFragmentBuilder {
    nodes: Vec<FragmentNode>,
    style_applications: Vec<ViewStyleApplicationTarget>,
    child_indices: Vec<NodeId>,
    events: Vec<EventBinding>,
    keys: BTreeSet<NodeKey>,
}

impl Span32 {
    pub const fn new(start: u32, len: u32) -> Self {
        Self { start, len }
    }

    pub const fn end(self) -> Option<u32> {
        self.start.checked_add(self.len)
    }

    fn from_len(start: usize, len: usize) -> Result<Self, ViewError> {
        Ok(Self {
            start: u32::try_from(start).map_err(|_| ViewError::CapacityExceeded)?,
            len: u32::try_from(len).map_err(|_| ViewError::CapacityExceeded)?,
        })
    }
}

impl EventBinding {
    pub const fn new(kind: EventKind, route: ViewHandlerRouteId) -> Self {
        Self { kind, route }
    }

    pub const fn kind(self) -> EventKind {
        self.kind
    }

    pub const fn route(self) -> ViewHandlerRouteId {
        self.route
    }

    pub const fn accepts(self, input: &InputEventKind) -> bool {
        self.kind.accepts(input)
    }
}

impl FragmentNode {
    pub const fn key(&self) -> NodeKey {
        self.key
    }

    pub const fn kind(&self) -> FragmentKind {
        self.kind
    }

    pub const fn style_applications(&self) -> Span32 {
        self.style_applications
    }

    pub const fn children(&self) -> Span32 {
        self.children
    }

    pub const fn events(&self) -> Span32 {
        self.events
    }

    pub const fn semantics(&self) -> Option<SemanticSpecId> {
        self.semantics
    }
}

impl ViewFragment {
    pub fn nodes(&self) -> &[FragmentNode] {
        &self.nodes
    }

    pub fn child_indices(&self) -> &[NodeId] {
        &self.child_indices
    }

    pub fn style_application_targets(&self) -> &[ViewStyleApplicationTarget] {
        &self.style_applications
    }

    pub fn node_style_applications(&self, node: NodeId) -> Option<&[ViewStyleApplicationTarget]> {
        let span = self.nodes.get(node.0 as usize)?.style_applications;
        self.style_applications
            .get(span.start as usize..span.end()? as usize)
    }

    pub fn events(&self) -> &[EventBinding] {
        &self.events
    }

    pub fn node_children(&self, node: NodeId) -> Option<&[NodeId]> {
        let span = self.nodes.get(node.0 as usize)?.children;
        self.child_indices
            .get(span.start as usize..span.end()? as usize)
    }

    pub fn node_events(&self, node: NodeId) -> Option<&[EventBinding]> {
        let span = self.nodes.get(node.0 as usize)?.events;
        self.events.get(span.start as usize..span.end()? as usize)
    }
}

impl ViewFragmentBuilder {
    pub fn push_node(
        &mut self,
        key: NodeKey,
        kind: FragmentKind,
        style_applications: &[ViewStyleApplicationTarget],
        children: &[NodeId],
        events: &[EventBinding],
        semantics: Option<SemanticSpecId>,
    ) -> Result<NodeId, ViewError> {
        if !self.keys.insert(key) {
            return Err(ViewError::DuplicateNodeKey(key));
        }
        if let Some(invalid) = children
            .iter()
            .copied()
            .find(|child| self.get(*child).is_none())
        {
            return Err(ViewError::InvalidFragmentNode(invalid));
        }

        let style_span = Span32::from_len(self.style_applications.len(), style_applications.len())?;
        let child_span = Span32::from_len(self.child_indices.len(), children.len())?;
        let event_span = Span32::from_len(self.events.len(), events.len())?;
        self.style_applications
            .extend_from_slice(style_applications);
        self.child_indices.extend_from_slice(children);
        self.events.extend_from_slice(events);

        let node_id =
            NodeId(u32::try_from(self.nodes.len()).map_err(|_| ViewError::CapacityExceeded)?);
        self.nodes.push(FragmentNode {
            key,
            kind,
            style_applications: style_span,
            children: child_span,
            events: event_span,
            semantics,
        });
        Ok(node_id)
    }

    pub fn get(&self, node: NodeId) -> Option<&FragmentNode> {
        self.nodes.get(node.0 as usize)
    }

    pub fn finish(self) -> ViewFragment {
        ViewFragment {
            nodes: self.nodes,
            style_applications: self.style_applications,
            child_indices: self.child_indices,
            events: self.events,
        }
    }
}
