//! Display-list generation boundary for laid-out retained View fragments.

use crate::{
    CustomElementId, FragmentKind, ImageId, LayoutBox, LayoutResults, NodeId, ResolvedViewStyle,
    RichTextSourceId, SemanticSpecId, StyleId, TextSourceId, ViewError, ViewFragment,
    ViewSemanticFragment, ViewStyleTable,
};
use arcweft_presentation::interaction::InteractionState;

/// Frame-local display item identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DisplayItemId(pub u32);

/// Renderer-facing display payload produced from a retained View fragment.
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
    style: StyleId,
    layout: LayoutBox,
    semantics: Option<SemanticSpecId>,
}

/// Ordered View display list for renderer submission.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DisplayList {
    items: Vec<DisplayItem>,
}

/// One display item with interaction selectors resolved for the current frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedDisplayItem {
    item: DisplayItem,
    style: ResolvedViewStyle,
}

/// Ordered display list after hover/focus/pressed/disabled style resolution.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedDisplayList {
    items: Vec<ResolvedDisplayItem>,
}

impl DisplayItem {
    pub const fn node(self) -> NodeId {
        self.node
    }

    pub const fn kind(self) -> DisplayItemKind {
        self.kind
    }

    pub const fn style(self) -> StyleId {
        self.style
    }

    pub const fn layout(self) -> LayoutBox {
        self.layout
    }

    pub const fn semantics(self) -> Option<SemanticSpecId> {
        self.semantics
    }
}

impl DisplayList {
    pub fn from_fragment(
        fragment: &ViewFragment,
        layouts: &LayoutResults,
    ) -> Result<Self, ViewError> {
        let items = fragment
            .nodes()
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                let id = match u32::try_from(index) {
                    Ok(index) => NodeId(index),
                    Err(_) => return Some(Err(ViewError::CapacityExceeded)),
                };
                let kind = match node.kind() {
                    FragmentKind::Text(source) => DisplayItemKind::Text(source),
                    FragmentKind::RichText(source) => DisplayItemKind::RichText(source),
                    FragmentKind::Image(image) => DisplayItemKind::Image(image),
                    FragmentKind::Custom(custom) => DisplayItemKind::Custom(custom),
                    FragmentKind::Container(_) | FragmentKind::View(_) => return None,
                };
                Some(layouts.require(id).map(|layout| DisplayItem {
                    node: id,
                    kind,
                    style: node.style(),
                    layout,
                    semantics: node.semantics(),
                }))
            })
            .collect::<Result<Vec<_>, ViewError>>()?;
        Ok(Self { items })
    }

    pub fn resolve_interaction_styles(
        &self,
        semantics: &ViewSemanticFragment,
        styles: &ViewStyleTable,
        interaction: &InteractionState,
    ) -> Result<ResolvedDisplayList, ViewError> {
        self.items
            .iter()
            .copied()
            .map(|item| {
                let semantic = match item.semantics() {
                    Some(id) => {
                        Some(semantics.get(id).ok_or(ViewError::UnknownDisplaySemantic {
                            node: item.node(),
                            semantic: id,
                        })?)
                    }
                    None => None,
                };
                let resolved = styles.resolve(
                    item.style(),
                    semantic.map(crate::ViewSemanticNode::target),
                    semantic.is_none_or(crate::ViewSemanticNode::enabled),
                    interaction,
                )?;
                Ok(ResolvedDisplayItem {
                    item,
                    style: resolved,
                })
            })
            .collect::<Result<Vec<_>, ViewError>>()
            .map(|items| ResolvedDisplayList { items })
    }

    pub fn as_slice(&self) -> &[DisplayItem] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl ResolvedDisplayItem {
    pub const fn item(&self) -> DisplayItem {
        self.item
    }

    pub const fn style(&self) -> &ResolvedViewStyle {
        &self.style
    }
}

impl ResolvedDisplayList {
    pub fn as_slice(&self) -> &[ResolvedDisplayItem] {
        &self.items
    }

    pub fn into_vec(self) -> Vec<ResolvedDisplayItem> {
        self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
