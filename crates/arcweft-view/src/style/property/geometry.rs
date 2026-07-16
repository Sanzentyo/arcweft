use super::ViewPropertyKind;
use crate::geometry::{
    ViewGeometryPropertySupport as Support, ViewRepresentedGeometryFeature as Feature,
};

impl ViewPropertyKind {
    /// Executable physical-geometry support owned by the canonical property enum.
    #[expect(
        clippy::too_many_lines,
        reason = "the owner enum keeps geometry support exhaustive and auditable"
    )]
    pub const fn geometry_support(self) -> Support {
        match self {
            Self::Display
            | Self::Width
            | Self::Height
            | Self::InlineSize
            | Self::BlockSize
            | Self::MinWidth
            | Self::MinHeight
            | Self::MinInlineSize
            | Self::MinBlockSize
            | Self::MaxWidth
            | Self::MaxHeight
            | Self::MaxInlineSize
            | Self::MaxBlockSize
            | Self::Padding
            | Self::PaddingTop
            | Self::PaddingRight
            | Self::PaddingBottom
            | Self::PaddingLeft
            | Self::PaddingInlineStart
            | Self::PaddingInlineEnd
            | Self::PaddingBlockStart
            | Self::PaddingBlockEnd
            | Self::Margin
            | Self::MarginTop
            | Self::MarginRight
            | Self::MarginBottom
            | Self::MarginLeft
            | Self::MarginInlineStart
            | Self::MarginInlineEnd
            | Self::MarginBlockStart
            | Self::MarginBlockEnd
            | Self::Gap
            | Self::RowGap
            | Self::ColumnGap
            | Self::Position
            | Self::Top
            | Self::Right
            | Self::Bottom
            | Self::Left
            | Self::InsetInlineStart
            | Self::InsetInlineEnd
            | Self::InsetBlockStart
            | Self::InsetBlockEnd
            | Self::Overflow
            | Self::OverflowX
            | Self::OverflowY
            | Self::OverflowInline
            | Self::OverflowBlock
            | Self::FlexDirection
            | Self::BorderWidth
            | Self::OutlineWidth
            | Self::OutlineOffset
            | Self::FocusRingWidth
            | Self::TranslateX
            | Self::TranslateY
            | Self::TranslateInline
            | Self::TranslateBlock
            | Self::Scale => Support::Supported,
            Self::FlexWrap => Support::RepresentedOnly(Feature::FlexWrap),
            Self::FlexGrow | Self::FlexShrink => {
                Support::RepresentedOnly(Feature::FlexDistribution)
            }
            Self::FlexBasis => Support::RepresentedOnly(Feature::FlexBasis),
            Self::Order => Support::RepresentedOnly(Feature::Order),
            Self::AlignItems
            | Self::AlignSelf
            | Self::AlignContent
            | Self::JustifyContent
            | Self::JustifySelf => Support::RepresentedOnly(Feature::Alignment),
            Self::Rotate => Support::RepresentedOnly(Feature::Rotate),
            Self::Clip => Support::RepresentedOnly(Feature::NonRectClip),
            Self::Mask => Support::RepresentedOnly(Feature::Mask),
            Self::BoxShadow | Self::Filter | Self::BackdropFilter => {
                Support::RepresentedOnly(Feature::PaintEffectBounds)
            }
            Self::BoxAxes
            | Self::Visibility
            | Self::ZIndex
            | Self::Color
            | Self::FontFamily
            | Self::FontSize
            | Self::FontWeight
            | Self::FontStyle
            | Self::LineHeight
            | Self::LetterSpacing
            | Self::TextAlign
            | Self::BackgroundColor
            | Self::BorderColor
            | Self::BorderRadius
            | Self::OutlineColor
            | Self::FocusRingColor
            | Self::CornerFrameColor
            | Self::CornerFrameWidth
            | Self::CornerFrameLength
            | Self::CornerFrameOffset
            | Self::PlaceholderColor
            | Self::SelectionColor
            | Self::CaretColor
            | Self::CompositionUnderlineColor
            | Self::Opacity
            | Self::BlendMode
            | Self::Transition => Support::NotGeometry,
        }
    }
}
