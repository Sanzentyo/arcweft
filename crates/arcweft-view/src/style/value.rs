//! Typed values accepted by native Style before computed-style resolution.

use super::{
    property::{ViewPropertyKind, ViewStyleValueKind},
    sheet::ViewStyleTokenId,
};
use arcweft_id::PublicId;
use arcweft_presentation::appearance::{PresentationColor, SystemColor};
use serde::{Deserialize, Serialize};

/// Normalized ratio/progress value in thousandths.
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
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
}

/// Signed logical-pixel length represented in thousandths.
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct ViewLengthMilli(i32);

impl ViewLengthMilli {
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> i32 {
        self.0
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
}

/// Checked OpenType-compatible font weight.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewFontFamily {
    Named(String),
    System(ViewSystemFontFamily),
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewColorValue {
    Literal { color: PresentationColor },
    System { role: SystemColor },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewBorderRadii {
    pub top_left: ViewLengthMilli,
    pub top_right: ViewLengthMilli,
    pub bottom_right: ViewLengthMilli,
    pub bottom_left: ViewLengthMilli,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewShadow {
    pub x: ViewLengthMilli,
    pub y: ViewLengthMilli,
    pub blur: ViewLengthMilli,
    pub spread: ViewLengthMilli,
    pub color: ViewColorValue,
    pub inset: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewFilter {
    Blur { radius: ViewLengthMilli },
    Brightness { amount: ViewScalarMilli },
    Contrast { amount: ViewScalarMilli },
    Opacity { amount: ViewRatioMilli },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewClip {
    None,
    RoundedRect(ViewBorderRadii),
}

impl ViewClip {
    pub fn from_source_name(value: &str) -> Option<Self> {
        (value == "None").then_some(Self::None)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewMask {
    None,
    Resource(PublicId),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewSpecifiedValue {
    Token {
        token: ViewStyleTokenId,
        value_kind: ViewStyleValueKind,
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
        value: PublicId,
    },
}

impl ViewSpecifiedValue {
    pub const fn kind(&self) -> ViewStyleValueKind {
        match self {
            Self::Token { value_kind, .. } => *value_kind,
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
}
