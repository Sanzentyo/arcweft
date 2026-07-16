//! Typed values accepted by native Style before computed-style resolution.

use super::{
    ViewAxisSign, ViewBoxAxisMode,
    property::{ViewPropertyKind, ViewStyleValueKind},
    sheet::ViewStyleTokenId,
};
use arcweft_id::PublicId;
use arcweft_presentation::appearance::{PresentationColor, SystemColor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod codec;

/// Normalized ratio/progress value in thousandths.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ViewRatioMilli(u16);

impl ViewRatioMilli {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1_000);

    pub const fn new(value: u16) -> Option<Self> {
        if value <= Self::ONE.0 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn value(self) -> u16 {
        self.0
    }

    /// Linearly interpolates two normalized ratios.
    #[must_use]
    pub fn lerp(self, target: Self, progress: Self) -> Self {
        Self(
            u16::try_from(lerp_unsigned(
                u64::from(self.0),
                u64::from(target.0),
                progress,
            ))
            .unwrap_or(Self::ONE.0),
        )
    }
}

/// Non-negative, unbounded dimensionless scalar in thousandths.
///
/// Unlike [`ViewRatioMilli`], this type intentionally permits values above one
/// for scale, flex factors, brightness, and contrast.
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct ViewScalarMilli(u32);

impl ViewScalarMilli {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1_000);

    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }

    /// Linearly interpolates two non-negative scalars.
    #[must_use]
    pub fn lerp(self, target: Self, progress: ViewRatioMilli) -> Self {
        Self(
            u32::try_from(lerp_unsigned(
                u64::from(self.0),
                u64::from(target.0),
                progress,
            ))
            .unwrap_or(u32::MAX),
        )
    }
}

/// Signed logical-pixel length represented in thousandths.
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct ViewLengthMilli(i32);

/// Failure while adapting a logical value to a physical axis.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ViewAxisValueError {
    #[error("logical translation cannot be negated within fixed-point range")]
    NonReversibleLength,
}

impl ViewLengthMilli {
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> i32 {
        self.0
    }

    /// Applies a resolved logical displacement sign without saturating.
    pub fn checked_apply_axis_sign(self, sign: ViewAxisSign) -> Result<Self, ViewAxisValueError> {
        if !self.is_axis_sign_reversible() {
            return Err(ViewAxisValueError::NonReversibleLength);
        }
        Ok(match sign {
            ViewAxisSign::Positive => self,
            ViewAxisSign::Negative => Self(-self.0),
        })
    }

    /// Whether the value can be mapped reversibly under every supported mode.
    pub const fn is_axis_sign_reversible(self) -> bool {
        self.0 != i32::MIN
    }

    /// Linearly interpolates two signed logical lengths.
    #[must_use]
    pub fn lerp(self, target: Self, progress: ViewRatioMilli) -> Self {
        Self(lerp_signed(self.0, target.0, progress))
    }
}

/// Angle represented in thousandths of a degree.
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct ViewAngleMilliDegrees(i32);

impl ViewAngleMilliDegrees {
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> i32 {
        self.0
    }

    /// Linearly interpolates two signed angles.
    #[must_use]
    pub fn lerp(self, target: Self, progress: ViewRatioMilli) -> Self {
        Self(lerp_signed(self.0, target.0, progress))
    }
}

/// Checked OpenType-compatible font weight.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ViewFontWeight(u16);

impl ViewFontWeight {
    pub const fn new(value: u16) -> Option<Self> {
        if value >= 1 && value <= 1_000 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewSystemFontFamily {
    Ui,
    Serif,
    Sans,
    Monospace,
    Emoji,
}

impl ViewSystemFontFamily {
    pub const ALL: &'static [Self] = &[
        Self::Ui,
        Self::Serif,
        Self::Sans,
        Self::Monospace,
        Self::Emoji,
    ];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Ui => "Ui",
            Self::Serif => "Serif",
            Self::Sans => "Sans",
            Self::Monospace => "Monospace",
            Self::Emoji => "Emoji",
        }
    }

    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|family| family.source_name() == value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewFontFamily {
    Named(String),
    System(ViewSystemFontFamily),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ViewFontFamilyList {
    families: Vec<ViewFontFamily>,
}

impl ViewFontFamily {
    pub fn named(name: impl Into<String>) -> Option<Self> {
        let name = name.into();
        (!name.trim().is_empty()).then_some(Self::Named(name))
    }

    pub const fn system(family: ViewSystemFontFamily) -> Self {
        Self::System(family)
    }
}

impl ViewFontFamilyList {
    pub fn new(families: Vec<ViewFontFamily>) -> Option<Self> {
        (!families.is_empty()).then_some(Self { families })
    }

    pub fn as_slice(&self) -> &[ViewFontFamily] {
        &self.families
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewDisplay {
    None,
    Stack,
    Block,
    Inline,
    Flex,
}

impl ViewDisplay {
    pub const ALL: &'static [Self] = &[
        Self::None,
        Self::Stack,
        Self::Block,
        Self::Inline,
        Self::Flex,
    ];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Stack => "Stack",
            Self::Block => "Block",
            Self::Inline => "Inline",
            Self::Flex => "Flex",
        }
    }

    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|display| display.source_name() == value)
    }

    /// Stable ordinal used by deterministic hashes and codecs.
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Stack => 1,
            Self::Block => 2,
            Self::Inline => 3,
            Self::Flex => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewPosition {
    Static,
    Relative,
    Absolute,
    Fixed,
}

impl ViewPosition {
    pub const ALL: &'static [Self] = &[Self::Static, Self::Relative, Self::Absolute, Self::Fixed];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Static => "Static",
            Self::Relative => "Relative",
            Self::Absolute => "Absolute",
            Self::Fixed => "Fixed",
        }
    }

    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|position| position.source_name() == value)
    }

    /// Stable ordinal used by deterministic hashes and codecs.
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::Static => 0,
            Self::Relative => 1,
            Self::Absolute => 2,
            Self::Fixed => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewOverflow {
    Visible,
    Hidden,
    Clip,
    Auto,
    Scroll,
}

impl ViewOverflow {
    pub const ALL: &'static [Self] = &[
        Self::Visible,
        Self::Hidden,
        Self::Clip,
        Self::Auto,
        Self::Scroll,
    ];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Visible => "Visible",
            Self::Hidden => "Hidden",
            Self::Clip => "Clip",
            Self::Auto => "Auto",
            Self::Scroll => "Scroll",
        }
    }

    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|overflow| overflow.source_name() == value)
    }

    /// Stable ordinal used by deterministic hashes and codecs.
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::Visible => 0,
            Self::Hidden => 1,
            Self::Clip => 2,
            Self::Auto => 3,
            Self::Scroll => 4,
        }
    }

    /// Whether this overflow mode clips descendant geometry to the padding box.
    pub const fn clips_descendants(self) -> bool {
        !matches!(self, Self::Visible)
    }

    /// Physical scrolling capability after the signed range is known.
    pub const fn scroll_capability(self, has_range: bool) -> crate::geometry::ViewScrollCapability {
        use crate::geometry::ViewScrollCapability;
        match self {
            Self::Hidden => ViewScrollCapability::Programmatic,
            Self::Auto if has_range => ViewScrollCapability::UserAndProgrammatic,
            Self::Scroll => ViewScrollCapability::UserAndProgrammatic,
            _ => ViewScrollCapability::None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewFlexDirection {
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

impl ViewFlexDirection {
    pub const ALL: &'static [Self] = &[
        Self::Row,
        Self::RowReverse,
        Self::Column,
        Self::ColumnReverse,
    ];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Row => "Row",
            Self::RowReverse => "RowReverse",
            Self::Column => "Column",
            Self::ColumnReverse => "ColumnReverse",
        }
    }

    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|direction| direction.source_name() == value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewFlexWrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

impl ViewFlexWrap {
    pub const ALL: &'static [Self] = &[Self::NoWrap, Self::Wrap, Self::WrapReverse];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::NoWrap => "NoWrap",
            Self::Wrap => "Wrap",
            Self::WrapReverse => "WrapReverse",
        }
    }

    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|wrap| wrap.source_name() == value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewFontStyle {
    Normal,
    Italic,
    Oblique,
}

impl ViewFontStyle {
    pub const ALL: &'static [Self] = &[Self::Normal, Self::Italic, Self::Oblique];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Italic => "Italic",
            Self::Oblique => "Oblique",
        }
    }

    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|style| style.source_name() == value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewAlignment {
    Start,
    End,
    Center,
    Stretch,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

impl ViewAlignment {
    pub const ALL: &'static [Self] = &[
        Self::Start,
        Self::End,
        Self::Center,
        Self::Stretch,
        Self::SpaceBetween,
        Self::SpaceAround,
        Self::SpaceEvenly,
    ];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::End => "End",
            Self::Center => "Center",
            Self::Stretch => "Stretch",
            Self::SpaceBetween => "SpaceBetween",
            Self::SpaceAround => "SpaceAround",
            Self::SpaceEvenly => "SpaceEvenly",
        }
    }

    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|alignment| alignment.source_name() == value)
    }

    /// Whether this alignment keyword is valid for the owning property.
    pub const fn applies_to(self, property: ViewPropertyKind) -> bool {
        match property {
            ViewPropertyKind::AlignContent | ViewPropertyKind::JustifyContent => true,
            ViewPropertyKind::AlignItems
            | ViewPropertyKind::AlignSelf
            | ViewPropertyKind::JustifySelf => {
                matches!(self, Self::Start | Self::End | Self::Center | Self::Stretch)
            }
            ViewPropertyKind::TextAlign => {
                matches!(self, Self::Start | Self::End | Self::Center)
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum ViewColorValue {
    Literal { color: PresentationColor },
    System { role: SystemColor },
}

impl ViewColorValue {
    /// Interpolates literal colors. System roles require environment resolution first.
    pub fn lerp(self, target: Self, progress: ViewRatioMilli) -> Option<Self> {
        match (self, target) {
            (Self::Literal { color }, Self::Literal { color: target }) => Some(Self::Literal {
                color: color.lerp(target, progress.value()),
            }),
            (Self::System { .. }, _) | (Self::Literal { .. }, Self::System { .. }) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewBorderRadii {
    pub top_left: ViewLengthMilli,
    pub top_right: ViewLengthMilli,
    pub bottom_right: ViewLengthMilli,
    pub bottom_left: ViewLengthMilli,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewShadow {
    pub x: ViewLengthMilli,
    pub y: ViewLengthMilli,
    pub blur: ViewLengthMilli,
    pub spread: ViewLengthMilli,
    pub color: ViewColorValue,
    pub inset: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum ViewFilter {
    Blur { radius: ViewLengthMilli },
    Brightness { amount: ViewScalarMilli },
    Contrast { amount: ViewScalarMilli },
    Opacity { amount: ViewRatioMilli },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewClip {
    None,
    RoundedRect(ViewBorderRadii),
}

impl ViewClip {
    pub fn from_source_name(value: &str) -> Option<Self> {
        (value == "None").then_some(Self::None)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMask {
    None,
    Resource(#[serde(with = "codec::public_id")] PublicId),
}

impl ViewMask {
    pub fn from_source_name(value: &str) -> Option<Self> {
        (value == "None").then_some(Self::None)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewBlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
}

impl ViewBlendMode {
    pub const ALL: &'static [Self] = &[
        Self::Normal,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::Darken,
        Self::Lighten,
    ];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
            Self::Darken => "Darken",
            Self::Lighten => "Lighten",
        }
    }

    pub fn from_source_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|mode| mode.source_name() == value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ViewStyleTransition {
    property: ViewPropertyKind,
    duration_millis: u32,
    delay_millis: u32,
}

impl ViewStyleTransition {
    pub const fn new(
        property: ViewPropertyKind,
        duration_millis: u32,
        delay_millis: u32,
    ) -> Option<Self> {
        if property.is_transitionable() {
            Some(Self {
                property,
                duration_millis,
                delay_millis,
            })
        } else {
            None
        }
    }

    pub const fn property(self) -> ViewPropertyKind {
        self.property
    }

    pub const fn duration_millis(self) -> u32 {
        self.duration_millis
    }

    pub const fn delay_millis(self) -> u32 {
        self.delay_millis
    }
}

/// Typed native Style value before token/environment resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum ViewSpecifiedValue {
    Token {
        token: ViewStyleTokenId,
        value_kind: ViewStyleValueKind,
    },
    BoxAxes {
        value: ViewBoxAxisMode,
    },
    Bool {
        value: bool,
    },
    Integer {
        value: i32,
    },
    Ratio {
        value: ViewRatioMilli,
    },
    Scalar {
        value: ViewScalarMilli,
    },
    Length {
        value: ViewLengthMilli,
    },
    Angle {
        value: ViewAngleMilliDegrees,
    },
    Color {
        value: ViewColorValue,
    },
    FontFamilyList {
        value: ViewFontFamilyList,
    },
    FontWeight {
        value: ViewFontWeight,
    },
    FontStyle {
        value: ViewFontStyle,
    },
    Display {
        value: ViewDisplay,
    },
    Position {
        value: ViewPosition,
    },
    Overflow {
        value: ViewOverflow,
    },
    FlexDirection {
        value: ViewFlexDirection,
    },
    FlexWrap {
        value: ViewFlexWrap,
    },
    Alignment {
        value: ViewAlignment,
    },
    BorderRadii {
        value: ViewBorderRadii,
    },
    ShadowList {
        value: Vec<ViewShadow>,
    },
    FilterList {
        value: Vec<ViewFilter>,
    },
    Clip {
        value: ViewClip,
    },
    Mask {
        value: ViewMask,
    },
    BlendMode {
        value: ViewBlendMode,
    },
    Transition {
        value: Vec<ViewStyleTransition>,
    },
    Resource {
        #[serde(with = "codec::public_id")]
        value: PublicId,
    },
}

impl ViewSpecifiedValue {
    pub const fn kind(&self) -> ViewStyleValueKind {
        match self {
            Self::Token { value_kind, .. } => *value_kind,
            Self::BoxAxes { .. } => ViewStyleValueKind::BoxAxes,
            Self::Bool { .. } => ViewStyleValueKind::Bool,
            Self::Integer { .. } => ViewStyleValueKind::Integer,
            Self::Ratio { .. } => ViewStyleValueKind::Ratio,
            Self::Scalar { .. } => ViewStyleValueKind::Scalar,
            Self::Length { .. } => ViewStyleValueKind::Length,
            Self::Angle { .. } => ViewStyleValueKind::Angle,
            Self::Color { .. } => ViewStyleValueKind::Color,
            Self::FontFamilyList { .. } => ViewStyleValueKind::FontFamilyList,
            Self::FontWeight { .. } => ViewStyleValueKind::FontWeight,
            Self::FontStyle { .. } => ViewStyleValueKind::FontStyle,
            Self::Display { .. } => ViewStyleValueKind::Display,
            Self::Position { .. } => ViewStyleValueKind::Position,
            Self::Overflow { .. } => ViewStyleValueKind::Overflow,
            Self::FlexDirection { .. } => ViewStyleValueKind::FlexDirection,
            Self::FlexWrap { .. } => ViewStyleValueKind::FlexWrap,
            Self::Alignment { .. } => ViewStyleValueKind::Alignment,
            Self::BorderRadii { .. } => ViewStyleValueKind::BorderRadii,
            Self::ShadowList { .. } => ViewStyleValueKind::ShadowList,
            Self::FilterList { .. } => ViewStyleValueKind::FilterList,
            Self::Clip { .. } => ViewStyleValueKind::Clip,
            Self::Mask { .. } => ViewStyleValueKind::Mask,
            Self::BlendMode { .. } => ViewStyleValueKind::BlendMode,
            Self::Transition { .. } => ViewStyleValueKind::Transition,
            Self::Resource { .. } => ViewStyleValueKind::Resource,
        }
    }

    /// Interpolates canonical computed values for one transitionable property.
    pub fn interpolate(
        &self,
        property: ViewPropertyKind,
        target: &Self,
        progress: ViewRatioMilli,
    ) -> Option<Self> {
        if !property.is_transitionable()
            || self.kind() != property.value_kind()
            || target.kind() != property.value_kind()
        {
            return None;
        }
        if progress == ViewRatioMilli::ZERO {
            return Some(self.clone());
        }
        if progress == ViewRatioMilli::ONE {
            return Some(target.clone());
        }
        match (self, target) {
            (Self::Ratio { value }, Self::Ratio { value: target }) => Some(Self::Ratio {
                value: value.lerp(*target, progress),
            }),
            (Self::Scalar { value }, Self::Scalar { value: target }) => Some(Self::Scalar {
                value: value.lerp(*target, progress),
            }),
            (Self::Length { value }, Self::Length { value: target }) => Some(Self::Length {
                value: value.lerp(*target, progress),
            }),
            (Self::Angle { value }, Self::Angle { value: target }) => Some(Self::Angle {
                value: value.lerp(*target, progress),
            }),
            (Self::Color { value }, Self::Color { value: target }) => Some(Self::Color {
                value: value.lerp(*target, progress)?,
            }),
            _ => None,
        }
    }

    /// Sheet-local token referenced by this value, together with its checked kind.
    pub const fn token_reference(&self) -> Option<(&ViewStyleTokenId, ViewStyleValueKind)> {
        match self {
            Self::Token { token, value_kind } => Some((token, *value_kind)),
            Self::BoxAxes { .. }
            | Self::Bool { .. }
            | Self::Integer { .. }
            | Self::Ratio { .. }
            | Self::Scalar { .. }
            | Self::Length { .. }
            | Self::Angle { .. }
            | Self::Color { .. }
            | Self::FontFamilyList { .. }
            | Self::FontWeight { .. }
            | Self::FontStyle { .. }
            | Self::Display { .. }
            | Self::Position { .. }
            | Self::Overflow { .. }
            | Self::FlexDirection { .. }
            | Self::FlexWrap { .. }
            | Self::Alignment { .. }
            | Self::BorderRadii { .. }
            | Self::ShadowList { .. }
            | Self::FilterList { .. }
            | Self::Clip { .. }
            | Self::Mask { .. }
            | Self::BlendMode { .. }
            | Self::Transition { .. }
            | Self::Resource { .. } => None,
        }
    }
}

fn lerp_signed(source: i32, target: i32, progress: ViewRatioMilli) -> i32 {
    let source = i64::from(source);
    let delta = i64::from(target).saturating_sub(source);
    let value =
        source.saturating_add((delta.saturating_mul(i64::from(progress.value())) + 500) / 1_000);
    i32::try_from(value.clamp(i64::from(i32::MIN), i64::from(i32::MAX))).unwrap_or_else(|_| {
        if value.is_negative() {
            i32::MIN
        } else {
            i32::MAX
        }
    })
}

fn lerp_unsigned(source: u64, target: u64, progress: ViewRatioMilli) -> u64 {
    if target >= source {
        source.saturating_add(
            target
                .saturating_sub(source)
                .saturating_mul(u64::from(progress.value()))
                .saturating_add(500)
                / 1_000,
        )
    } else {
        source.saturating_sub(
            source
                .saturating_sub(target)
                .saturating_mul(u64::from(progress.value()))
                .saturating_sub(500)
                / 1_000,
        )
    }
}
