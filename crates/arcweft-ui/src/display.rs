//! Display-list generation boundary for laid-out retained UI fragments.

use crate::{
    CustomElementId, FragmentKind, ImageId, LayoutBox, LayoutResults, NodeId, RichTextSourceId,
    TextSourceId, UiError, ViewFragment,
};

/// Frame-local display item identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DisplayItemId(pub u32);

/// Renderer-facing display payload produced from a retained UI fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayItemKind {
    Text(TextSourceId),
    RichText(RichTextSourceId),
    Image(ImageId),
    Custom(CustomElementId),
}

/// One ordered display item with its source node and resolved layout box.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayItem {
    node: NodeId,
    kind: DisplayItemKind,
    layout: LayoutBox,
}

/// Ordered UI display list for renderer submission.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DisplayList {
    items: Vec<DisplayItem>,
}

impl DisplayItem {
    pub const fn node(self) -> NodeId {
        self.node
    }

    pub const fn kind(self) -> DisplayItemKind {
        self.kind
    }

    pub const fn layout(self) -> LayoutBox {
        self.layout
    }
}

impl DisplayList {
    pub fn from_fragment(
        fragment: &ViewFragment,
        layouts: &LayoutResults,
    ) -> Result<Self, UiError> {
        let items = fragment
            .nodes()
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                let id = match u32::try_from(index) {
                    Ok(index) => NodeId(index),
                    Err(_) => return Some(Err(UiError::CapacityExceeded)),
                };
                let kind = match node.kind() {
                    FragmentKind::Text(source) => DisplayItemKind::Text(source),
                    FragmentKind::RichText(source) => DisplayItemKind::RichText(source),
                    FragmentKind::Image(image) => DisplayItemKind::Image(image),
                    FragmentKind::Custom(custom) => DisplayItemKind::Custom(custom),
                    FragmentKind::Container(_) | FragmentKind::Component(_) => return None,
                };
                Some(layouts.require(id).map(|layout| DisplayItem {
                    node: id,
                    kind,
                    layout,
                }))
            })
            .collect::<Result<Vec<_>, UiError>>()?;
        Ok(Self { items })
    }

    pub fn as_slice(&self) -> &[DisplayItem] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
