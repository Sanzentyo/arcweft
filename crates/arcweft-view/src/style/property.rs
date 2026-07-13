//! Canonical native Style property inventory and metadata.

use crate::ViewElementKind;
use serde::{Deserialize, Serialize};

/// Closed value family accepted by a source-authored View style property.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewStyleValueKind {
    Bool,
    Integer,
    Ratio,
    Scalar,
    Length,
    Angle,
    Color,
    FontFamilyList,
    FontWeight,
    FontStyle,
    Display,
    Position,
    Overflow,
    FlexDirection,
    FlexWrap,
    Alignment,
    BorderRadii,
    ShadowList,
    FilterList,
    Clip,
    Mask,
    BlendMode,
    Transition,
    Resource,
}

impl ViewStyleValueKind {
    /// Complete inventory of native Style type annotation names.
    pub const ALL: &'static [Self] = &[
        Self::Bool,
        Self::Integer,
        Self::Ratio,
        Self::Scalar,
        Self::Length,
        Self::Angle,
        Self::Color,
        Self::FontFamilyList,
        Self::FontWeight,
        Self::FontStyle,
        Self::Display,
        Self::Position,
        Self::Overflow,
        Self::FlexDirection,
        Self::FlexWrap,
        Self::Alignment,
        Self::BorderRadii,
        Self::ShadowList,
        Self::FilterList,
        Self::Clip,
        Self::Mask,
        Self::BlendMode,
        Self::Transition,
        Self::Resource,
    ];

    /// Canonical case-sensitive spelling in native Style type annotations.
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Bool => "Bool",
            Self::Integer => "Integer",
            Self::Ratio => "Ratio",
            Self::Scalar => "Scalar",
            Self::Length => "Length",
            Self::Angle => "Angle",
            Self::Color => "Color",
            Self::FontFamilyList => "FontFamilyList",
            Self::FontWeight => "FontWeight",
            Self::FontStyle => "FontStyle",
            Self::Display => "Display",
            Self::Position => "Position",
            Self::Overflow => "Overflow",
            Self::FlexDirection => "FlexDirection",
            Self::FlexWrap => "FlexWrap",
            Self::Alignment => "Alignment",
            Self::BorderRadii => "BorderRadii",
            Self::ShadowList => "ShadowList",
            Self::FilterList => "FilterList",
            Self::Clip => "Clip",
            Self::Mask => "Mask",
            Self::BlendMode => "BlendMode",
            Self::Transition => "Transition",
            Self::Resource => "Resource",
        }
    }

    /// Looks up an exact native Style type annotation without aliases.
    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.source_name() == value)
    }
}

/// Property family used by style/property binding and invalidation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewPropertyKind {
    // Visibility and sizing.
    Visibility,
    Display,
    Width,
    Height,
    InlineSize,
    BlockSize,
    MinWidth,
    MinHeight,
    MinInlineSize,
    MinBlockSize,
    MaxWidth,
    MaxHeight,
    MaxInlineSize,
    MaxBlockSize,
    // Box layout.
    Padding,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    PaddingInlineStart,
    PaddingInlineEnd,
    PaddingBlockStart,
    PaddingBlockEnd,
    Margin,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,
    MarginInlineStart,
    MarginInlineEnd,
    MarginBlockStart,
    MarginBlockEnd,
    Gap,
    RowGap,
    ColumnGap,
    Position,
    Top,
    Right,
    Bottom,
    Left,
    InsetInlineStart,
    InsetInlineEnd,
    InsetBlockStart,
    InsetBlockEnd,
    ZIndex,
    Overflow,
    OverflowX,
    OverflowY,
    OverflowInline,
    OverflowBlock,
    FlexDirection,
    FlexWrap,
    FlexGrow,
    FlexShrink,
    FlexBasis,
    Order,
    AlignItems,
    AlignSelf,
    AlignContent,
    JustifyContent,
    JustifySelf,
    // Text.
    Color,
    FontFamily,
    FontSize,
    FontWeight,
    FontStyle,
    LineHeight,
    LetterSpacing,
    TextAlign,
    // Paint and decoration.
    BackgroundColor,
    BorderColor,
    BorderWidth,
    BorderRadius,
    OutlineColor,
    OutlineWidth,
    OutlineOffset,
    FocusRingColor,
    FocusRingWidth,
    CornerFrameColor,
    CornerFrameWidth,
    CornerFrameLength,
    CornerFrameOffset,
    PlaceholderColor,
    SelectionColor,
    CaretColor,
    CompositionUnderlineColor,
    Opacity,
    TranslateX,
    TranslateY,
    TranslateInline,
    TranslateBlock,
    Scale,
    Rotate,
    BoxShadow,
    Filter,
    BackdropFilter,
    Clip,
    Mask,
    BlendMode,
    Transition,
}

/// A compact union of retained work required after a property changes.
#[must_use]
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct ViewStyleInvalidationSet {
    bits: u16,
}

impl ViewStyleInvalidationSet {
    pub const NONE: Self = Self { bits: 0 };
    pub const TEXT_LAYOUT: Self = Self { bits: 1 << 0 };
    pub const LAYOUT: Self = Self { bits: 1 << 1 };
    pub const PAINT: Self = Self { bits: 1 << 2 };
    pub const COMPOSITE: Self = Self { bits: 1 << 3 };
    pub const RESOURCE: Self = Self { bits: 1 << 4 };
    pub const SEMANTICS: Self = Self { bits: 1 << 5 };
    pub const FRAGMENT: Self = Self { bits: 1 << 6 };

    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    pub const fn contains(self, other: Self) -> bool {
        self.bits & other.bits == other.bits
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }
}

impl ViewPropertyKind {
    /// Complete canonical inventory accepted by native Style source.
    pub const ALL: &'static [Self] = &[
        Self::Visibility,
        Self::Display,
        Self::Width,
        Self::Height,
        Self::InlineSize,
        Self::BlockSize,
        Self::MinWidth,
        Self::MinHeight,
        Self::MinInlineSize,
        Self::MinBlockSize,
        Self::MaxWidth,
        Self::MaxHeight,
        Self::MaxInlineSize,
        Self::MaxBlockSize,
        Self::Padding,
        Self::PaddingTop,
        Self::PaddingRight,
        Self::PaddingBottom,
        Self::PaddingLeft,
        Self::PaddingInlineStart,
        Self::PaddingInlineEnd,
        Self::PaddingBlockStart,
        Self::PaddingBlockEnd,
        Self::Margin,
        Self::MarginTop,
        Self::MarginRight,
        Self::MarginBottom,
        Self::MarginLeft,
        Self::MarginInlineStart,
        Self::MarginInlineEnd,
        Self::MarginBlockStart,
        Self::MarginBlockEnd,
        Self::Gap,
        Self::RowGap,
        Self::ColumnGap,
        Self::Position,
        Self::Top,
        Self::Right,
        Self::Bottom,
        Self::Left,
        Self::InsetInlineStart,
        Self::InsetInlineEnd,
        Self::InsetBlockStart,
        Self::InsetBlockEnd,
        Self::ZIndex,
        Self::Overflow,
        Self::OverflowX,
        Self::OverflowY,
        Self::OverflowInline,
        Self::OverflowBlock,
        Self::FlexDirection,
        Self::FlexWrap,
        Self::FlexGrow,
        Self::FlexShrink,
        Self::FlexBasis,
        Self::Order,
        Self::AlignItems,
        Self::AlignSelf,
        Self::AlignContent,
        Self::JustifyContent,
        Self::JustifySelf,
        Self::Color,
        Self::FontFamily,
        Self::FontSize,
        Self::FontWeight,
        Self::FontStyle,
        Self::LineHeight,
        Self::LetterSpacing,
        Self::TextAlign,
        Self::BackgroundColor,
        Self::BorderColor,
        Self::BorderWidth,
        Self::BorderRadius,
        Self::OutlineColor,
        Self::OutlineWidth,
        Self::OutlineOffset,
        Self::FocusRingColor,
        Self::FocusRingWidth,
        Self::CornerFrameColor,
        Self::CornerFrameWidth,
        Self::CornerFrameLength,
        Self::CornerFrameOffset,
        Self::PlaceholderColor,
        Self::SelectionColor,
        Self::CaretColor,
        Self::CompositionUnderlineColor,
        Self::Opacity,
        Self::TranslateX,
        Self::TranslateY,
        Self::TranslateInline,
        Self::TranslateBlock,
        Self::Scale,
        Self::Rotate,
        Self::BoxShadow,
        Self::Filter,
        Self::BackdropFilter,
        Self::Clip,
        Self::Mask,
        Self::BlendMode,
        Self::Transition,
    ];

    /// Canonical case-sensitive spelling in native Style source.
    #[expect(
        clippy::too_many_lines,
        reason = "the closed property inventory keeps one auditable canonical spelling per variant"
    )]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Visibility => "visibility",
            Self::Display => "display",
            Self::Width => "width",
            Self::Height => "height",
            Self::InlineSize => "inline-size",
            Self::BlockSize => "block-size",
            Self::MinWidth => "min-width",
            Self::MinHeight => "min-height",
            Self::MinInlineSize => "min-inline-size",
            Self::MinBlockSize => "min-block-size",
            Self::MaxWidth => "max-width",
            Self::MaxHeight => "max-height",
            Self::MaxInlineSize => "max-inline-size",
            Self::MaxBlockSize => "max-block-size",
            Self::Padding => "padding",
            Self::PaddingTop => "padding-top",
            Self::PaddingRight => "padding-right",
            Self::PaddingBottom => "padding-bottom",
            Self::PaddingLeft => "padding-left",
            Self::PaddingInlineStart => "padding-inline-start",
            Self::PaddingInlineEnd => "padding-inline-end",
            Self::PaddingBlockStart => "padding-block-start",
            Self::PaddingBlockEnd => "padding-block-end",
            Self::Margin => "margin",
            Self::MarginTop => "margin-top",
            Self::MarginRight => "margin-right",
            Self::MarginBottom => "margin-bottom",
            Self::MarginLeft => "margin-left",
            Self::MarginInlineStart => "margin-inline-start",
            Self::MarginInlineEnd => "margin-inline-end",
            Self::MarginBlockStart => "margin-block-start",
            Self::MarginBlockEnd => "margin-block-end",
            Self::Gap => "gap",
            Self::RowGap => "row-gap",
            Self::ColumnGap => "column-gap",
            Self::Position => "position",
            Self::Top => "top",
            Self::Right => "right",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::InsetInlineStart => "inset-inline-start",
            Self::InsetInlineEnd => "inset-inline-end",
            Self::InsetBlockStart => "inset-block-start",
            Self::InsetBlockEnd => "inset-block-end",
            Self::ZIndex => "z-index",
            Self::Overflow => "overflow",
            Self::OverflowX => "overflow-x",
            Self::OverflowY => "overflow-y",
            Self::OverflowInline => "overflow-inline",
            Self::OverflowBlock => "overflow-block",
            Self::FlexDirection => "flex-direction",
            Self::FlexWrap => "flex-wrap",
            Self::FlexGrow => "flex-grow",
            Self::FlexShrink => "flex-shrink",
            Self::FlexBasis => "flex-basis",
            Self::Order => "order",
            Self::AlignItems => "align-items",
            Self::AlignSelf => "align-self",
            Self::AlignContent => "align-content",
            Self::JustifyContent => "justify-content",
            Self::JustifySelf => "justify-self",
            Self::Color => "color",
            Self::FontFamily => "font-family",
            Self::FontSize => "font-size",
            Self::FontWeight => "font-weight",
            Self::FontStyle => "font-style",
            Self::LineHeight => "line-height",
            Self::LetterSpacing => "letter-spacing",
            Self::TextAlign => "text-align",
            Self::BackgroundColor => "background-color",
            Self::BorderColor => "border-color",
            Self::BorderWidth => "border-width",
            Self::BorderRadius => "border-radius",
            Self::OutlineColor => "outline-color",
            Self::OutlineWidth => "outline-width",
            Self::OutlineOffset => "outline-offset",
            Self::FocusRingColor => "focus-ring-color",
            Self::FocusRingWidth => "focus-ring-width",
            Self::CornerFrameColor => "corner-frame-color",
            Self::CornerFrameWidth => "corner-frame-width",
            Self::CornerFrameLength => "corner-frame-length",
            Self::CornerFrameOffset => "corner-frame-offset",
            Self::PlaceholderColor => "placeholder-color",
            Self::SelectionColor => "selection-color",
            Self::CaretColor => "caret-color",
            Self::CompositionUnderlineColor => "composition-underline-color",
            Self::Opacity => "opacity",
            Self::TranslateX => "translate-x",
            Self::TranslateY => "translate-y",
            Self::TranslateInline => "translate-inline",
            Self::TranslateBlock => "translate-block",
            Self::Scale => "scale",
            Self::Rotate => "rotate",
            Self::BoxShadow => "box-shadow",
            Self::Filter => "filter",
            Self::BackdropFilter => "backdrop-filter",
            Self::Clip => "clip",
            Self::Mask => "mask",
            Self::BlendMode => "blend-mode",
            Self::Transition => "transition",
        }
    }

    /// Looks up canonical spelling only; legacy underscores and runtime-only
    /// property categories are deliberately rejected.
    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|property| property.source_name() == value)
    }

    pub const fn value_kind(self) -> ViewStyleValueKind {
        match self {
            Self::Visibility => ViewStyleValueKind::Bool,
            Self::ZIndex | Self::Order => ViewStyleValueKind::Integer,
            Self::Opacity => ViewStyleValueKind::Ratio,
            Self::Scale | Self::FlexGrow | Self::FlexShrink => ViewStyleValueKind::Scalar,
            Self::Width
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
            | Self::InsetInlineStart
            | Self::InsetInlineEnd
            | Self::InsetBlockStart
            | Self::InsetBlockEnd
            | Self::Top
            | Self::Right
            | Self::Bottom
            | Self::Left
            | Self::FlexBasis
            | Self::FontSize
            | Self::LineHeight
            | Self::LetterSpacing
            | Self::BorderWidth
            | Self::OutlineWidth
            | Self::OutlineOffset
            | Self::FocusRingWidth
            | Self::CornerFrameWidth
            | Self::CornerFrameLength
            | Self::CornerFrameOffset
            | Self::TranslateX
            | Self::TranslateY
            | Self::TranslateInline
            | Self::TranslateBlock
            // A single logical length is the canonical authored shorthand;
            // computed style expands it to four typed corners.
            | Self::BorderRadius => ViewStyleValueKind::Length,
            Self::Rotate => ViewStyleValueKind::Angle,
            Self::Color
            | Self::BackgroundColor
            | Self::BorderColor
            | Self::OutlineColor
            | Self::FocusRingColor
            | Self::CornerFrameColor
            | Self::PlaceholderColor
            | Self::SelectionColor
            | Self::CaretColor
            | Self::CompositionUnderlineColor => ViewStyleValueKind::Color,
            Self::FontFamily => ViewStyleValueKind::FontFamilyList,
            Self::FontWeight => ViewStyleValueKind::FontWeight,
            Self::FontStyle => ViewStyleValueKind::FontStyle,
            Self::Display => ViewStyleValueKind::Display,
            Self::Position => ViewStyleValueKind::Position,
            Self::Overflow
            | Self::OverflowX
            | Self::OverflowY
            | Self::OverflowInline
            | Self::OverflowBlock => ViewStyleValueKind::Overflow,
            Self::FlexDirection => ViewStyleValueKind::FlexDirection,
            Self::FlexWrap => ViewStyleValueKind::FlexWrap,
            Self::AlignItems
            | Self::AlignSelf
            | Self::AlignContent
            | Self::JustifyContent
            | Self::JustifySelf
            | Self::TextAlign => ViewStyleValueKind::Alignment,
            Self::BoxShadow => ViewStyleValueKind::ShadowList,
            Self::Filter | Self::BackdropFilter => ViewStyleValueKind::FilterList,
            Self::Clip => ViewStyleValueKind::Clip,
            Self::Mask => ViewStyleValueKind::Mask,
            Self::BlendMode => ViewStyleValueKind::BlendMode,
            Self::Transition => ViewStyleValueKind::Transition,
        }
    }

    pub const fn is_transitionable(self) -> bool {
        matches!(
            self,
            Self::Opacity
                | Self::TranslateX
                | Self::TranslateY
                | Self::TranslateInline
                | Self::TranslateBlock
                | Self::Scale
                | Self::Rotate
                | Self::Color
                | Self::BackgroundColor
                | Self::BorderColor
                | Self::BorderWidth
                | Self::PlaceholderColor
                | Self::SelectionColor
                | Self::CaretColor
                | Self::CompositionUnderlineColor
                | Self::OutlineColor
                | Self::OutlineWidth
                | Self::OutlineOffset
                | Self::FocusRingColor
                | Self::FocusRingWidth
                | Self::BorderRadius
        )
    }

    pub const fn is_inherited(self) -> bool {
        matches!(
            self,
            Self::Color
                | Self::FontFamily
                | Self::FontSize
                | Self::FontWeight
                | Self::FontStyle
                | Self::LineHeight
                | Self::LetterSpacing
                | Self::TextAlign
        )
    }

    pub const fn is_appendable(self) -> bool {
        matches!(
            self,
            Self::FontFamily
                | Self::BoxShadow
                | Self::Filter
                | Self::BackdropFilter
                | Self::Transition
        )
    }

    pub const fn applies_to(self, element: ViewElementKind) -> bool {
        match self {
            Self::PlaceholderColor
            | Self::SelectionColor
            | Self::CaretColor
            | Self::CompositionUnderlineColor => element.is_text_input(),
            Self::FlexDirection
            | Self::FlexWrap
            | Self::AlignItems
            | Self::AlignContent
            | Self::JustifyContent => element.is_layout_container(),
            _ => true,
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the owner enum keeps the complete invalidation table in one exhaustive match"
    )]
    pub const fn default_invalidation(self) -> ViewStyleInvalidationSet {
        match self {
            Self::FontFamily
            | Self::FontSize
            | Self::FontWeight
            | Self::FontStyle
            | Self::LineHeight
            | Self::LetterSpacing
            | Self::TextAlign => ViewStyleInvalidationSet::TEXT_LAYOUT
                .union(ViewStyleInvalidationSet::LAYOUT)
                .union(ViewStyleInvalidationSet::PAINT),
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
            | Self::ZIndex
            | Self::Overflow
            | Self::OverflowX
            | Self::OverflowY
            | Self::OverflowInline
            | Self::OverflowBlock
            | Self::FlexDirection
            | Self::FlexWrap
            | Self::FlexGrow
            | Self::FlexShrink
            | Self::FlexBasis
            | Self::Order
            | Self::AlignItems
            | Self::AlignSelf
            | Self::AlignContent
            | Self::JustifyContent
            | Self::JustifySelf => ViewStyleInvalidationSet::LAYOUT,
            Self::Opacity
            | Self::TranslateX
            | Self::TranslateY
            | Self::TranslateInline
            | Self::TranslateBlock
            | Self::Scale
            | Self::Rotate
            | Self::Filter
            | Self::BackdropFilter
            | Self::BlendMode => {
                ViewStyleInvalidationSet::COMPOSITE.union(ViewStyleInvalidationSet::PAINT)
            }
            Self::Color
            | Self::BackgroundColor
            | Self::BorderColor
            | Self::BorderWidth
            | Self::PlaceholderColor
            | Self::SelectionColor
            | Self::CaretColor
            | Self::CompositionUnderlineColor
            | Self::OutlineColor
            | Self::OutlineWidth
            | Self::OutlineOffset
            | Self::FocusRingColor
            | Self::FocusRingWidth
            | Self::CornerFrameColor
            | Self::CornerFrameWidth
            | Self::CornerFrameLength
            | Self::CornerFrameOffset
            | Self::BorderRadius
            | Self::BoxShadow
            | Self::Visibility
            | Self::Transition => ViewStyleInvalidationSet::PAINT,
            Self::Clip | Self::Mask => {
                ViewStyleInvalidationSet::PAINT.union(ViewStyleInvalidationSet::RESOURCE)
            }
        }
    }
}
