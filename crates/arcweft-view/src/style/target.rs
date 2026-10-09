//! Canonical selector targets and retained-node kinds for native Style.

use crate::ViewElementKind;
use serde::{Deserialize, Serialize};

/// Closed Style target inventory shared by source admission, codecs and matching.
/// Text selects the common text family; RichText additionally requires a rich
/// document or resolved display frame. Controls retain their actual element kind.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "element",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ViewStyleTargetKind {
    Element(ViewElementKind),
    Text,
    RichText,
    Image,
    View,
    Custom,
}

impl ViewStyleTargetKind {
    /// Stable identity for every closed target, including its element payload.
    pub const fn semantic_tag(self) -> u16 {
        match self {
            Self::Element(element) => element.semantic_tag() as u16,
            Self::Text => 256,
            Self::RichText => 257,
            Self::Image => 258,
            Self::View => 259,
            Self::Custom => 260,
        }
    }

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Element(element) => element.source_name(),
            Self::Text => "Text",
            Self::RichText => "RichText",
            Self::Image => "Image",
            Self::View => "View",
            Self::Custom => "Custom",
        }
    }

    /// Admits only canonical typed targets; container names use the element owner.
    pub fn from_source_name(name: &str) -> Option<Self> {
        match name {
            "Text" => Some(Self::Text),
            "RichText" => Some(Self::RichText),
            "Image" => Some(Self::Image),
            "View" => Some(Self::View),
            "Custom" => Some(Self::Custom),
            _ => ViewElementKind::from_source_name(name).map(Self::Element),
        }
    }

    /// A text family rule applies to both text representations. RichText refines it.
    pub fn matches(self, node: Self) -> bool {
        self == node || matches!((self, node), (Self::Text, Self::RichText))
    }

    pub const fn element(self) -> Option<ViewElementKind> {
        match self {
            Self::Element(element) => Some(element),
            _ => None,
        }
    }
}
