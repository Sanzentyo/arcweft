//! Static Style value and operand types. Values remain owned by arcweft-view.

use arcweft_view::style::ViewStyleValueKind;

/// Type of one specified Style value or a typed operand used to construct it.
///
/// These are semantic-only values, including unresolved palette/token references.
/// Ordinary runtime quantities retain their registered scalar types.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompileTimeStyleType {
    Value(ViewStyleValueKind),
    FontFamily,
    SystemFontFamily,
    SystemColor,
    Shadow,
    Filter,
    Transition,
    Token(ViewStyleValueKind),
    Property,
}

impl CompileTimeStyleType {
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Value(_) => 0,
            Self::FontFamily => 1,
            Self::SystemFontFamily => 2,
            Self::SystemColor => 3,
            Self::Shadow => 4,
            Self::Filter => 5,
            Self::Transition => 6,
            Self::Token(_) => 7,
            Self::Property => 8,
        }
    }

    pub const fn value_kind(self) -> Option<ViewStyleValueKind> {
        match self {
            Self::Value(kind) | Self::Token(kind) => Some(kind),
            Self::FontFamily
            | Self::SystemFontFamily
            | Self::SystemColor
            | Self::Shadow
            | Self::Filter
            | Self::Transition => None,
            Self::Property => None,
        }
    }

    pub fn source_label(self) -> String {
        match self {
            Self::Value(kind) => format!("StyleValue<{}>", kind.source_name()),
            Self::Token(kind) => format!("StyleToken<{}>", kind.source_name()),
            Self::FontFamily => "StyleOperand<FontFamily>".to_owned(),
            Self::SystemFontFamily => "StyleOperand<SystemFontFamily>".to_owned(),
            Self::SystemColor => "StyleOperand<SystemColor>".to_owned(),
            Self::Shadow => "StyleOperand<Shadow>".to_owned(),
            Self::Filter => "StyleOperand<Filter>".to_owned(),
            Self::Transition => "StyleOperand<Transition>".to_owned(),
            Self::Property => "StyleOperand<Property>".to_owned(),
        }
    }
}
