//! Static Style value and operand types. Values remain owned by arcweft-view.

use arcweft_view::style::ViewStyleValueKind;

/// Numeric literal grammars used to construct complete native Style values.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StyleNumericType {
    Integer,
    Ratio,
    Scalar,
    Length,
    Angle,
    FontWeight,
}

impl StyleNumericType {
    pub const ALL: [Self; 6] = [
        Self::Integer,
        Self::Ratio,
        Self::Scalar,
        Self::Length,
        Self::Angle,
        Self::FontWeight,
    ];

    pub const fn value_kind(self) -> ViewStyleValueKind {
        match self {
            Self::Integer => ViewStyleValueKind::Integer,
            Self::Ratio => ViewStyleValueKind::Ratio,
            Self::Scalar => ViewStyleValueKind::Scalar,
            Self::Length => ViewStyleValueKind::Length,
            Self::Angle => ViewStyleValueKind::Angle,
            Self::FontWeight => ViewStyleValueKind::FontWeight,
        }
    }

    pub(crate) const fn maximum_magnitude(self) -> u32 {
        match self {
            Self::Integer | Self::Length | Self::Angle => 1 << 31,
            Self::Scalar => u32::MAX,
            Self::Ratio | Self::FontWeight => 1000,
        }
    }
}

impl TryFrom<ViewStyleValueKind> for StyleNumericType {
    type Error = ViewStyleValueKind;

    fn try_from(kind: ViewStyleValueKind) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|numeric| numeric.value_kind() == kind)
            .ok_or(kind)
    }
}

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
    NumericLiteral(StyleNumericType),
    Duration,
    ResourceId,
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
            Self::NumericLiteral(_) => 9,
            Self::Duration => 10,
            Self::ResourceId => 11,
        }
    }

    pub const fn value_kind(self) -> Option<ViewStyleValueKind> {
        match self {
            Self::Value(kind) | Self::Token(kind) => Some(kind),
            Self::NumericLiteral(kind) => Some(kind.value_kind()),
            Self::FontFamily
            | Self::SystemFontFamily
            | Self::SystemColor
            | Self::Shadow
            | Self::Filter
            | Self::Transition => None,
            Self::Property | Self::Duration | Self::ResourceId => None,
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
            Self::Duration => "StyleOperand<Duration>".to_owned(),
            Self::ResourceId => "StyleOperand<ResourceId>".to_owned(),
            Self::NumericLiteral(kind) => {
                format!("StyleLiteral<{}>", kind.value_kind().source_name())
            }
        }
    }
}
