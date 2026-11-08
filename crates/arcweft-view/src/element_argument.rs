//! Semantic grammar of builtin element arguments.

use crate::{
    ViewElementKind,
    style::{ViewPhysicalFlow, ViewPropertyKind},
};

/// One declaration-independent role in a builtin element's input schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewElementArgumentRole {
    Value,
    Label,
    Enabled,
    X,
    Y,
    Width,
    Height,
    Spacing,
    Part,
    Target,
    Purpose,
    EnterKey,
}

impl ViewElementArgumentRole {
    pub const DEFAULT_LABEL: &'static str = "";
    pub const DEFAULT_ENABLED: bool = true;
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Value => 8,
            Self::Target => 9,
            Self::Purpose => 10,
            Self::EnterKey => 11,
            Self::Label => 0,
            Self::Enabled => 1,
            Self::X => 2,
            Self::Y => 3,
            Self::Width => 4,
            Self::Height => 5,
            Self::Spacing => 6,
            Self::Part => 7,
        }
    }

    pub const fn positional(element: ViewElementKind, ordinal: usize) -> Option<Self> {
        if element.is_text_input() {
            match ordinal {
                0 => Some(Self::Value),
                _ => None,
            }
        } else if element.is_action_control() {
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
            "value" => Some(Self::Value),
            "label" => Some(Self::Label),
            "enabled" => Some(Self::Enabled),
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            "width" => Some(Self::Width),
            "height" => Some(Self::Height),
            "spacing" => Some(Self::Spacing),
            "part" => Some(Self::Part),
            "target" => Some(Self::Target),
            "purpose" => Some(Self::Purpose),
            "enter_key" => Some(Self::EnterKey),
            _ => None,
        }
    }

    pub const fn accepts_element(self, element: ViewElementKind) -> bool {
        match self {
            Self::Value => element.is_text_input(),
            Self::Purpose | Self::EnterKey => element.is_text_input(),
            Self::Label => element.is_action_control() || element.is_text_input(),
            Self::Enabled => element.is_action_control(),
            Self::Spacing => self.style_property(element).is_some(),
            Self::X | Self::Y | Self::Width | Self::Height | Self::Part | Self::Target => true,
        }
    }

    /// Canonical physical Style property for an admitted geometry argument.
    /// Spacing belongs to the element's physical flow, never an inherited box axis.
    pub const fn style_property(self, element: ViewElementKind) -> Option<ViewPropertyKind> {
        match self {
            Self::X | Self::Y | Self::Width | Self::Height => self.geometry_property(),
            Self::Spacing => match element.default_physical_flow() {
                Some(ViewPhysicalFlow::Row | ViewPhysicalFlow::RowReverse) => {
                    Some(ViewPropertyKind::ColumnGap)
                }
                Some(ViewPhysicalFlow::Column | ViewPhysicalFlow::ColumnReverse) => {
                    Some(ViewPropertyKind::RowGap)
                }
                Some(ViewPhysicalFlow::Overlay) | None => None,
            },
            Self::Value
            | Self::Label
            | Self::Enabled
            | Self::Part
            | Self::Target
            | Self::Purpose
            | Self::EnterKey => None,
        }
    }

    /// Physical property shared by constructor inputs and node modifiers.
    pub const fn geometry_property(self) -> Option<ViewPropertyKind> {
        match self {
            Self::X => Some(ViewPropertyKind::Left),
            Self::Y => Some(ViewPropertyKind::Top),
            Self::Width => Some(ViewPropertyKind::Width),
            Self::Height => Some(ViewPropertyKind::Height),
            Self::Value
            | Self::Label
            | Self::Enabled
            | Self::Spacing
            | Self::Part
            | Self::Target => None,
            Self::Purpose | Self::EnterKey => None,
        }
    }

    pub const fn compile_time_enum_domain(
        self,
    ) -> Option<arcweft_id::closed_enum::ClosedEnumDomainId> {
        use arcweft_presentation::text_input::TextInputPolicyDomain;
        match self {
            Self::Purpose => Some(TextInputPolicyDomain::Purpose.domain_id()),
            Self::EnterKey => Some(TextInputPolicyDomain::EnterKey.domain_id()),
            _ => None,
        }
    }
}
