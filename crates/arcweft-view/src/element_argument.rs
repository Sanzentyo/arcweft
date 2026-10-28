//! Semantic grammar of builtin element arguments.

use crate::{
    ViewElementKind,
    style::{ViewPhysicalFlow, ViewPropertyKind},
};

/// One declaration-independent role in a builtin element's input schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewElementArgumentRole {
    Label,
    Enabled,
    X,
    Y,
    Width,
    Height,
    Spacing,
}

impl ViewElementArgumentRole {
    pub const DEFAULT_LABEL: &'static str = "";
    pub const DEFAULT_ENABLED: bool = true;
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Label => 0,
            Self::Enabled => 1,
            Self::X => 2,
            Self::Y => 3,
            Self::Width => 4,
            Self::Height => 5,
            Self::Spacing => 6,
        }
    }

    pub const fn positional(element: ViewElementKind, ordinal: usize) -> Option<Self> {
        if element.is_action_control() || element.is_text_input() {
            match ordinal {
                0 => Some(Self::Label),
                1 => Some(Self::Enabled),
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn from_source_name(name: &str) -> Option<Self> {
        match name {
            "label" => Some(Self::Label),
            "enabled" => Some(Self::Enabled),
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            "width" => Some(Self::Width),
            "height" => Some(Self::Height),
            "spacing" => Some(Self::Spacing),
            _ => None,
        }
    }

    pub const fn accepts_element(self, element: ViewElementKind) -> bool {
        match self {
            Self::Label | Self::Enabled => {
                element.is_action_control() || element.text_input_kind().is_some()
            }
            Self::Spacing => self.style_property(element).is_some(),
            Self::X | Self::Y | Self::Width | Self::Height => true,
        }
    }

    /// Canonical physical Style property for an admitted geometry argument.
    /// Spacing belongs to the element's physical flow, never an inherited box axis.
    pub const fn style_property(self, element: ViewElementKind) -> Option<ViewPropertyKind> {
        match self {
            Self::X => Some(ViewPropertyKind::Left),
            Self::Y => Some(ViewPropertyKind::Top),
            Self::Width => Some(ViewPropertyKind::Width),
            Self::Height => Some(ViewPropertyKind::Height),
            Self::Spacing => match element.default_physical_flow() {
                Some(ViewPhysicalFlow::Row | ViewPhysicalFlow::RowReverse) => {
                    Some(ViewPropertyKind::ColumnGap)
                }
                Some(ViewPhysicalFlow::Column | ViewPhysicalFlow::ColumnReverse) => {
                    Some(ViewPropertyKind::RowGap)
                }
                Some(ViewPhysicalFlow::Overlay) | None => None,
            },
            Self::Label | Self::Enabled => None,
        }
    }
}
