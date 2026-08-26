//! Canonical native Style property inventory and metadata.

use super::{ViewAxisSign, ViewBoxAxisMode, ViewPhysicalAxis, ViewPhysicalSide};
use crate::ViewElementKind;
use serde::{Deserialize, Serialize};

mod geometry;
mod semantic;

/// Closed value family accepted by a source-authored View style property.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewStyleValueKind {
    BoxAxes,
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
        Self::BoxAxes,
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

    /// Stable semantic tag in declaration order.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::BoxAxes => 0,
            Self::Bool => 1,
            Self::Integer => 2,
            Self::Ratio => 3,
            Self::Scalar => 4,
            Self::Length => 5,
            Self::Angle => 6,
            Self::Color => 7,
            Self::FontFamilyList => 8,
            Self::FontWeight => 9,
            Self::FontStyle => 10,
            Self::Display => 11,
            Self::Position => 12,
            Self::Overflow => 13,
            Self::FlexDirection => 14,
            Self::FlexWrap => 15,
            Self::Alignment => 16,
            Self::BorderRadii => 17,
            Self::ShadowList => 18,
            Self::FilterList => 19,
            Self::Clip => 20,
            Self::Mask => 21,
            Self::BlendMode => 22,
            Self::Transition => 23,
            Self::Resource => 24,
        }
    }

    /// Canonical case-sensitive spelling in native Style type annotations.
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::BoxAxes => "BoxAxes",
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
    // Inherited logical-axis context. It never becomes a computed value slot.
    BoxAxes,
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

/// Canonical physical property key admitted to a computed Style map.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewComputedPropertyKind(ViewPropertyKind);

/// Axis-specific mapping of one authored or expanded property.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewPropertyResolution {
    authored: ViewPropertyKind,
    resolved: ViewComputedPropertyKind,
    value_transform: ViewPropertyValueTransform,
}

/// Value adaptation required while mapping a logical alias to a physical slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewPropertyValueTransform {
    Identity,
    SignedLength(ViewAxisSign),
}

/// Shape of shorthand expansion performed before alias resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewPropertyExpansion {
    One(ViewPropertyKind),
    FourPhysicalEdges,
    TwoPhysicalAxes,
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
    pub const AXIS_CONTEXT: Self = Self { bits: 1 << 7 };
    pub const HIT_TEST: Self = Self { bits: 1 << 8 };
    pub const SCROLL: Self = Self { bits: 1 << 9 };
    pub const FOCUS_GEOMETRY: Self = Self { bits: 1 << 10 };
    pub const AVOIDANCE: Self = Self { bits: 1 << 11 };
    pub const PHYSICAL_GEOMETRY: Self = Self { bits: 1 << 12 };

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

impl ViewComputedPropertyKind {
    /// Admits only canonical physical or axis-neutral computed slots.
    pub const fn try_from_property(property: ViewPropertyKind) -> Option<Self> {
        if property.is_computed_canonical() {
            Some(Self(property))
        } else {
            None
        }
    }

    pub const fn as_property(self) -> ViewPropertyKind {
        self.0
    }
}

impl ViewPropertyResolution {
    const fn new(
        authored: ViewPropertyKind,
        resolved: ViewPropertyKind,
        value_transform: ViewPropertyValueTransform,
    ) -> Self {
        Self {
            authored,
            resolved: ViewComputedPropertyKind(resolved),
            value_transform,
        }
    }

    pub const fn authored(self) -> ViewPropertyKind {
        self.authored
    }

    pub const fn resolved(self) -> ViewComputedPropertyKind {
        self.resolved
    }

    pub const fn value_transform(self) -> ViewPropertyValueTransform {
        self.value_transform
    }
}

impl ViewPropertyKind {
    /// Whether this property selects the inherited axis context itself.
    pub const fn is_axis_context(self) -> bool {
        matches!(self, Self::BoxAxes)
    }

    /// Whether this property's canonical slot or sign depends on box axes.
    pub const fn is_axis_dependent(self) -> bool {
        matches!(
            self,
            Self::InlineSize
                | Self::BlockSize
                | Self::MinInlineSize
                | Self::MinBlockSize
                | Self::MaxInlineSize
                | Self::MaxBlockSize
                | Self::PaddingInlineStart
                | Self::PaddingInlineEnd
                | Self::PaddingBlockStart
                | Self::PaddingBlockEnd
                | Self::MarginInlineStart
                | Self::MarginInlineEnd
                | Self::MarginBlockStart
                | Self::MarginBlockEnd
                | Self::InsetInlineStart
                | Self::InsetInlineEnd
                | Self::InsetBlockStart
                | Self::InsetBlockEnd
                | Self::TranslateInline
                | Self::TranslateBlock
                | Self::OverflowInline
                | Self::OverflowBlock
        )
    }

    /// Whether this property is a legal key in canonical computed Style.
    pub const fn is_computed_canonical(self) -> bool {
        !self.is_axis_context()
            && !self.is_axis_dependent()
            && !matches!(
                self,
                Self::Padding | Self::Margin | Self::Gap | Self::Overflow
            )
    }

    /// Declares shorthand expansion shape before logical alias resolution.
    pub const fn shorthand_expansion(self) -> ViewPropertyExpansion {
        match self {
            Self::Padding | Self::Margin => ViewPropertyExpansion::FourPhysicalEdges,
            Self::Gap | Self::Overflow => ViewPropertyExpansion::TwoPhysicalAxes,
            _ => ViewPropertyExpansion::One(self),
        }
    }

    /// Exact ordered longhands produced by native shorthand expansion.
    pub const fn expanded_properties(self) -> &'static [Self] {
        const PADDING: &[ViewPropertyKind] = &[
            ViewPropertyKind::PaddingTop,
            ViewPropertyKind::PaddingRight,
            ViewPropertyKind::PaddingBottom,
            ViewPropertyKind::PaddingLeft,
        ];
        const MARGIN: &[ViewPropertyKind] = &[
            ViewPropertyKind::MarginTop,
            ViewPropertyKind::MarginRight,
            ViewPropertyKind::MarginBottom,
            ViewPropertyKind::MarginLeft,
        ];
        const OVERFLOW: &[ViewPropertyKind] =
            &[ViewPropertyKind::OverflowX, ViewPropertyKind::OverflowY];
        const GAP: &[ViewPropertyKind] = &[ViewPropertyKind::RowGap, ViewPropertyKind::ColumnGap];
        match self {
            Self::Padding => PADDING,
            Self::Margin => MARGIN,
            Self::Gap => GAP,
            Self::Overflow => OVERFLOW,
            _ => &[],
        }
    }

    /// Resolves a longhand property against one immutable axis snapshot.
    ///
    /// `box-axes` and shorthands must be handled before calling this method.
    ///
    /// # Panics
    ///
    /// Panics when called with `box-axes`, `padding`, `margin`, or `overflow`.
    pub const fn resolve_for_axes(self, mode: ViewBoxAxisMode) -> ViewPropertyResolution {
        let axes = mode.resolved();
        let inline = axes.inline();
        let block = axes.block();
        let (resolved, transform) = match self {
            Self::InlineSize => (
                size_property(inline.axis(), Self::Width, Self::Height),
                ViewPropertyValueTransform::Identity,
            ),
            Self::BlockSize => (
                size_property(block.axis(), Self::Width, Self::Height),
                ViewPropertyValueTransform::Identity,
            ),
            Self::MinInlineSize => (
                size_property(inline.axis(), Self::MinWidth, Self::MinHeight),
                ViewPropertyValueTransform::Identity,
            ),
            Self::MinBlockSize => (
                size_property(block.axis(), Self::MinWidth, Self::MinHeight),
                ViewPropertyValueTransform::Identity,
            ),
            Self::MaxInlineSize => (
                size_property(inline.axis(), Self::MaxWidth, Self::MaxHeight),
                ViewPropertyValueTransform::Identity,
            ),
            Self::MaxBlockSize => (
                size_property(block.axis(), Self::MaxWidth, Self::MaxHeight),
                ViewPropertyValueTransform::Identity,
            ),
            Self::PaddingInlineStart => (
                padding_property(inline.start()),
                ViewPropertyValueTransform::Identity,
            ),
            Self::PaddingInlineEnd => (
                padding_property(inline.end()),
                ViewPropertyValueTransform::Identity,
            ),
            Self::PaddingBlockStart => (
                padding_property(block.start()),
                ViewPropertyValueTransform::Identity,
            ),
            Self::PaddingBlockEnd => (
                padding_property(block.end()),
                ViewPropertyValueTransform::Identity,
            ),
            Self::MarginInlineStart => (
                margin_property(inline.start()),
                ViewPropertyValueTransform::Identity,
            ),
            Self::MarginInlineEnd => (
                margin_property(inline.end()),
                ViewPropertyValueTransform::Identity,
            ),
            Self::MarginBlockStart => (
                margin_property(block.start()),
                ViewPropertyValueTransform::Identity,
            ),
            Self::MarginBlockEnd => (
                margin_property(block.end()),
                ViewPropertyValueTransform::Identity,
            ),
            Self::InsetInlineStart => (
                inset_property(inline.start()),
                ViewPropertyValueTransform::Identity,
            ),
            Self::InsetInlineEnd => (
                inset_property(inline.end()),
                ViewPropertyValueTransform::Identity,
            ),
            Self::InsetBlockStart => (
                inset_property(block.start()),
                ViewPropertyValueTransform::Identity,
            ),
            Self::InsetBlockEnd => (
                inset_property(block.end()),
                ViewPropertyValueTransform::Identity,
            ),
            Self::TranslateInline => (
                size_property(inline.axis(), Self::TranslateX, Self::TranslateY),
                ViewPropertyValueTransform::SignedLength(inline.positive_displacement()),
            ),
            Self::TranslateBlock => (
                size_property(block.axis(), Self::TranslateX, Self::TranslateY),
                ViewPropertyValueTransform::SignedLength(block.positive_displacement()),
            ),
            Self::OverflowInline => (
                size_property(inline.axis(), Self::OverflowX, Self::OverflowY),
                ViewPropertyValueTransform::Identity,
            ),
            Self::OverflowBlock => (
                size_property(block.axis(), Self::OverflowX, Self::OverflowY),
                ViewPropertyValueTransform::Identity,
            ),
            Self::BoxAxes | Self::Padding | Self::Margin | Self::Overflow => {
                panic!("axis context and shorthands must be handled before alias resolution")
            }
            _ => (self, ViewPropertyValueTransform::Identity),
        };
        ViewPropertyResolution::new(self, resolved, transform)
    }

    /// Axis usage family recorded when this logical property reaches mapping.
    pub const fn axis_usage(self) -> super::ViewAxisUsageSet {
        use super::ViewAxisUsageSet;
        match self {
            Self::InlineSize | Self::BlockSize => ViewAxisUsageSet::SIZE,
            Self::MinInlineSize | Self::MinBlockSize | Self::MaxInlineSize | Self::MaxBlockSize => {
                ViewAxisUsageSet::MIN_MAX_SIZE
            }
            Self::PaddingInlineStart
            | Self::PaddingInlineEnd
            | Self::PaddingBlockStart
            | Self::PaddingBlockEnd
            | Self::MarginInlineStart
            | Self::MarginInlineEnd
            | Self::MarginBlockStart
            | Self::MarginBlockEnd => ViewAxisUsageSet::SPACING,
            Self::InsetInlineStart
            | Self::InsetInlineEnd
            | Self::InsetBlockStart
            | Self::InsetBlockEnd => ViewAxisUsageSet::INSET,
            Self::TranslateInline | Self::TranslateBlock => ViewAxisUsageSet::TRANSLATION,
            Self::OverflowInline | Self::OverflowBlock => ViewAxisUsageSet::OVERFLOW,
            _ => ViewAxisUsageSet::NONE,
        }
    }
}

const fn size_property(
    axis: ViewPhysicalAxis,
    x: ViewPropertyKind,
    y: ViewPropertyKind,
) -> ViewPropertyKind {
    match axis {
        ViewPhysicalAxis::X => x,
        ViewPhysicalAxis::Y => y,
    }
}

const fn padding_property(side: ViewPhysicalSide) -> ViewPropertyKind {
    match side {
        ViewPhysicalSide::Top => ViewPropertyKind::PaddingTop,
        ViewPhysicalSide::Right => ViewPropertyKind::PaddingRight,
        ViewPhysicalSide::Bottom => ViewPropertyKind::PaddingBottom,
        ViewPhysicalSide::Left => ViewPropertyKind::PaddingLeft,
    }
}

const fn margin_property(side: ViewPhysicalSide) -> ViewPropertyKind {
    match side {
        ViewPhysicalSide::Top => ViewPropertyKind::MarginTop,
        ViewPhysicalSide::Right => ViewPropertyKind::MarginRight,
        ViewPhysicalSide::Bottom => ViewPropertyKind::MarginBottom,
        ViewPhysicalSide::Left => ViewPropertyKind::MarginLeft,
    }
}

const fn inset_property(side: ViewPhysicalSide) -> ViewPropertyKind {
    match side {
        ViewPhysicalSide::Top => ViewPropertyKind::Top,
        ViewPhysicalSide::Right => ViewPropertyKind::Right,
        ViewPhysicalSide::Bottom => ViewPropertyKind::Bottom,
        ViewPhysicalSide::Left => ViewPropertyKind::Left,
    }
}

impl ViewPropertyKind {
    /// Complete canonical inventory accepted by native Style source.
    pub const ALL: &'static [Self] = &[
        Self::BoxAxes,
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
            Self::BoxAxes => "box-axes",
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
            Self::BoxAxes => ViewStyleValueKind::BoxAxes,
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
            Self::BoxAxes
                | Self::Color
                | Self::FontFamily
                | Self::FontSize
                | Self::FontWeight
                | Self::FontStyle
                | Self::LineHeight
                | Self::LetterSpacing
                | Self::TextAlign
                | Self::SelectionColor
                | Self::CaretColor
                | Self::CompositionUnderlineColor
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
            Self::BoxAxes => ViewStyleInvalidationSet::AXIS_CONTEXT,
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
            | Self::FlexDirection
            | Self::BorderWidth => physical_layout_invalidation(),
            Self::ZIndex
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
            Self::TranslateX
            | Self::TranslateY
            | Self::TranslateInline
            | Self::TranslateBlock
            | Self::Scale => physical_transform_invalidation(),
            Self::Overflow
            | Self::OverflowX
            | Self::OverflowY
            | Self::OverflowInline
            | Self::OverflowBlock => physical_clip_invalidation(),
            Self::OutlineWidth
            | Self::OutlineOffset
            | Self::FocusRingWidth
            | Self::BoxShadow
            | Self::Filter
            | Self::BackdropFilter => physical_paint_outset_invalidation(),
            Self::Opacity | Self::Rotate | Self::BlendMode => {
                ViewStyleInvalidationSet::COMPOSITE.union(ViewStyleInvalidationSet::PAINT)
            }
            Self::Color
            | Self::BackgroundColor
            | Self::BorderColor
            | Self::PlaceholderColor
            | Self::SelectionColor
            | Self::CaretColor
            | Self::CompositionUnderlineColor
            | Self::OutlineColor
            | Self::FocusRingColor
            | Self::CornerFrameColor
            | Self::CornerFrameWidth
            | Self::CornerFrameLength
            | Self::CornerFrameOffset
            | Self::BorderRadius
            | Self::Visibility
            | Self::Transition => ViewStyleInvalidationSet::PAINT,
            Self::Clip | Self::Mask => {
                physical_clip_invalidation().union(ViewStyleInvalidationSet::RESOURCE)
            }
        }
    }
}

const fn physical_layout_invalidation() -> ViewStyleInvalidationSet {
    ViewStyleInvalidationSet::PHYSICAL_GEOMETRY
        .union(ViewStyleInvalidationSet::LAYOUT)
        .union(ViewStyleInvalidationSet::PAINT)
        .union(ViewStyleInvalidationSet::HIT_TEST)
        .union(ViewStyleInvalidationSet::FOCUS_GEOMETRY)
        .union(ViewStyleInvalidationSet::AVOIDANCE)
        .union(ViewStyleInvalidationSet::SCROLL)
}

const fn physical_transform_invalidation() -> ViewStyleInvalidationSet {
    ViewStyleInvalidationSet::PHYSICAL_GEOMETRY
        .union(ViewStyleInvalidationSet::COMPOSITE)
        .union(ViewStyleInvalidationSet::PAINT)
        .union(ViewStyleInvalidationSet::HIT_TEST)
        .union(ViewStyleInvalidationSet::FOCUS_GEOMETRY)
        .union(ViewStyleInvalidationSet::AVOIDANCE)
        .union(ViewStyleInvalidationSet::SCROLL)
}

const fn physical_clip_invalidation() -> ViewStyleInvalidationSet {
    ViewStyleInvalidationSet::PHYSICAL_GEOMETRY
        .union(ViewStyleInvalidationSet::PAINT)
        .union(ViewStyleInvalidationSet::HIT_TEST)
        .union(ViewStyleInvalidationSet::FOCUS_GEOMETRY)
        .union(ViewStyleInvalidationSet::AVOIDANCE)
        .union(ViewStyleInvalidationSet::SCROLL)
}

const fn physical_paint_outset_invalidation() -> ViewStyleInvalidationSet {
    ViewStyleInvalidationSet::PHYSICAL_GEOMETRY
        .union(ViewStyleInvalidationSet::PAINT)
        .union(ViewStyleInvalidationSet::FOCUS_GEOMETRY)
        .union(ViewStyleInvalidationSet::AVOIDANCE)
        .union(ViewStyleInvalidationSet::SCROLL)
}
