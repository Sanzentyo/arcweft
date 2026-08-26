//! Stable semantic tags for the closed Style property inventory.

use super::ViewPropertyKind;

impl ViewPropertyKind {
    /// Stable semantic tag in declaration order.
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive owner encoder keeps every property tag reviewable and compiler-checked"
    )]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::BoxAxes => 0,
            Self::Visibility => 1,
            Self::Display => 2,
            Self::Width => 3,
            Self::Height => 4,
            Self::InlineSize => 5,
            Self::BlockSize => 6,
            Self::MinWidth => 7,
            Self::MinHeight => 8,
            Self::MinInlineSize => 9,
            Self::MinBlockSize => 10,
            Self::MaxWidth => 11,
            Self::MaxHeight => 12,
            Self::MaxInlineSize => 13,
            Self::MaxBlockSize => 14,
            Self::Padding => 15,
            Self::PaddingTop => 16,
            Self::PaddingRight => 17,
            Self::PaddingBottom => 18,
            Self::PaddingLeft => 19,
            Self::PaddingInlineStart => 20,
            Self::PaddingInlineEnd => 21,
            Self::PaddingBlockStart => 22,
            Self::PaddingBlockEnd => 23,
            Self::Margin => 24,
            Self::MarginTop => 25,
            Self::MarginRight => 26,
            Self::MarginBottom => 27,
            Self::MarginLeft => 28,
            Self::MarginInlineStart => 29,
            Self::MarginInlineEnd => 30,
            Self::MarginBlockStart => 31,
            Self::MarginBlockEnd => 32,
            Self::Gap => 33,
            Self::RowGap => 34,
            Self::ColumnGap => 35,
            Self::Position => 36,
            Self::Top => 37,
            Self::Right => 38,
            Self::Bottom => 39,
            Self::Left => 40,
            Self::InsetInlineStart => 41,
            Self::InsetInlineEnd => 42,
            Self::InsetBlockStart => 43,
            Self::InsetBlockEnd => 44,
            Self::ZIndex => 45,
            Self::Overflow => 46,
            Self::OverflowX => 47,
            Self::OverflowY => 48,
            Self::OverflowInline => 49,
            Self::OverflowBlock => 50,
            Self::FlexDirection => 51,
            Self::FlexWrap => 52,
            Self::FlexGrow => 53,
            Self::FlexShrink => 54,
            Self::FlexBasis => 55,
            Self::Order => 56,
            Self::AlignItems => 57,
            Self::AlignSelf => 58,
            Self::AlignContent => 59,
            Self::JustifyContent => 60,
            Self::JustifySelf => 61,
            Self::Color => 62,
            Self::FontFamily => 63,
            Self::FontSize => 64,
            Self::FontWeight => 65,
            Self::FontStyle => 66,
            Self::LineHeight => 67,
            Self::LetterSpacing => 68,
            Self::TextAlign => 69,
            Self::BackgroundColor => 70,
            Self::BorderColor => 71,
            Self::BorderWidth => 72,
            Self::BorderRadius => 73,
            Self::OutlineColor => 74,
            Self::OutlineWidth => 75,
            Self::OutlineOffset => 76,
            Self::FocusRingColor => 77,
            Self::FocusRingWidth => 78,
            Self::CornerFrameColor => 79,
            Self::CornerFrameWidth => 80,
            Self::CornerFrameLength => 81,
            Self::CornerFrameOffset => 82,
            Self::PlaceholderColor => 83,
            Self::SelectionColor => 84,
            Self::CaretColor => 85,
            Self::CompositionUnderlineColor => 86,
            Self::Opacity => 87,
            Self::TranslateX => 88,
            Self::TranslateY => 89,
            Self::TranslateInline => 90,
            Self::TranslateBlock => 91,
            Self::Scale => 92,
            Self::Rotate => 93,
            Self::BoxShadow => 94,
            Self::Filter => 95,
            Self::BackdropFilter => 96,
            Self::Clip => 97,
            Self::Mask => 98,
            Self::BlendMode => 99,
            Self::Transition => 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ViewPropertyKind;
    use std::collections::BTreeSet;

    #[test]
    fn property_semantic_tags_are_unique() {
        let tags = ViewPropertyKind::ALL
            .iter()
            .copied()
            .map(ViewPropertyKind::semantic_tag)
            .collect::<BTreeSet<_>>();
        assert_eq!(tags.len(), ViewPropertyKind::ALL.len());
    }
}
