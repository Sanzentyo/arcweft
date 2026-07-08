//! View layout input and result data for retained fragments.

use crate::{FragmentKind, NodeId, ViewError, ViewFragment};

/// Fixed-point View length in milli-pixels.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LayoutLength(pub i32);

/// Two-dimensional fixed-point View size.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LayoutSize {
    pub width: LayoutLength,
    pub height: LayoutLength,
}

/// Two-dimensional fixed-point View point.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LayoutPoint {
    pub x: LayoutLength,
    pub y: LayoutLength,
}

/// Axis-aligned layout box in local View coordinates.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LayoutBox {
    pub origin: LayoutPoint,
    pub size: LayoutSize,
}

/// Layout behavior requested for a retained fragment node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutKind {
    Container,
    Text,
    RichText,
    Image,
    View,
    Custom,
}

/// One node of layout input derived from a retained fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutNode {
    node: NodeId,
    kind: LayoutKind,
    child_count: u32,
}

/// Flat layout input derived from a retained fragment.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LayoutTree {
    nodes: Vec<LayoutNode>,
}

/// Frame-local layout results corresponding to a `LayoutTree`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LayoutResults {
    boxes: Vec<Option<LayoutBox>>,
}

impl LayoutLength {
    pub const fn px(value: i32) -> Self {
        Self(value.saturating_mul(1_000))
    }

    pub const fn milli(value: i32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> i32 {
        self.0
    }
}

impl LayoutSize {
    pub const fn new(width: LayoutLength, height: LayoutLength) -> Self {
        Self { width, height }
    }
}

impl LayoutPoint {
    pub const fn new(x: LayoutLength, y: LayoutLength) -> Self {
        Self { x, y }
    }
}

impl LayoutBox {
    pub const fn new(origin: LayoutPoint, size: LayoutSize) -> Self {
        Self { origin, size }
    }

    pub const fn milli_rect(self) -> [i32; 4] {
        [
            self.origin.x.value(),
            self.origin.y.value(),
            self.size.width.value(),
            self.size.height.value(),
        ]
    }
}

impl LayoutKind {
    pub const fn from_fragment_kind(kind: FragmentKind) -> Self {
        match kind {
            FragmentKind::Container(_) => Self::Container,
            FragmentKind::Text(_) => Self::Text,
            FragmentKind::RichText(_) => Self::RichText,
            FragmentKind::Image(_) => Self::Image,
            FragmentKind::View(_) => Self::View,
            FragmentKind::Custom(_) => Self::Custom,
        }
    }
}

impl LayoutNode {
    pub const fn node(self) -> NodeId {
        self.node
    }

    pub const fn kind(self) -> LayoutKind {
        self.kind
    }

    pub const fn child_count(self) -> u32 {
        self.child_count
    }
}

impl LayoutTree {
    pub fn from_fragment(fragment: &ViewFragment) -> Result<Self, ViewError> {
        let nodes = fragment
            .nodes()
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let id = NodeId(u32::try_from(index).map_err(|_| ViewError::CapacityExceeded)?);
                let children = fragment
                    .node_children(id)
                    .ok_or(ViewError::InvalidFragmentNode(id))?;
                let child_count =
                    u32::try_from(children.len()).map_err(|_| ViewError::CapacityExceeded)?;
                Ok(LayoutNode {
                    node: id,
                    kind: LayoutKind::from_fragment_kind(node.kind()),
                    child_count,
                })
            })
            .collect::<Result<Vec<_>, ViewError>>()?;
        Ok(Self { nodes })
    }

    pub fn nodes(&self) -> &[LayoutNode] {
        &self.nodes
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl LayoutResults {
    pub fn new(tree: &LayoutTree) -> Self {
        Self {
            boxes: vec![None; tree.len()],
        }
    }

    pub fn set(&mut self, node: NodeId, layout: LayoutBox) -> Result<(), ViewError> {
        let slot = self
            .boxes
            .get_mut(node.0 as usize)
            .ok_or(ViewError::InvalidFragmentNode(node))?;
        *slot = Some(layout);
        Ok(())
    }

    pub fn get(&self, node: NodeId) -> Option<LayoutBox> {
        self.boxes.get(node.0 as usize).copied().flatten()
    }

    pub fn require(&self, node: NodeId) -> Result<LayoutBox, ViewError> {
        self.get(node).ok_or(ViewError::MissingLayout(node))
    }

    pub fn as_slice(&self) -> &[Option<LayoutBox>] {
        &self.boxes
    }
}
