//! Closed Fx targets, phases, renderer interfaces, and capability inventories.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use arcweft_id::closed_enum::{ClosedEnumDomainDescriptor, ClosedEnumMemberDescriptor};
use arcweft_id::closed_enum::{ClosedEnumDomainId, ClosedEnumOwnerId, ClosedEnumValueId};

use super::canonical::{CanonicalEncoder, CanonicalSink};

pub const FX_MAX_SELECTOR_NAME_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum FxEnumDomain {
    Target = 0,
    Phase = 1,
    ShaderStage = 2,
    MotionFunction = 3,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FxSelectorDomain {
    TransitionKind = 0,
    TransitionEasing = 1,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FxSelectorName {
    value: String,
    byte_len: u8,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxSelectorNameError {
    #[error("Fx selector name must be one canonical identifier")]
    Invalid,
    #[error("Fx selector name has {actual} UTF-8 bytes, exceeding the limit of {limit}")]
    TooLong { actual: usize, limit: usize },
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxSelectorId {
    domain: FxSelectorDomain,
    name: FxSelectorName,
}

impl FxSelectorName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, FxSelectorNameError> {
        let value = value.into();
        let mut chars = value.chars();
        if chars.next().is_none_or(|c| c != '_' && !c.is_alphabetic())
            || !chars.all(|c| c == '_' || c.is_alphanumeric())
        {
            return Err(FxSelectorNameError::Invalid);
        }
        if value.len() > FX_MAX_SELECTOR_NAME_BYTES {
            return Err(FxSelectorNameError::TooLong {
                actual: value.len(),
                limit: FX_MAX_SELECTOR_NAME_BYTES,
            });
        }
        let byte_len = u8::try_from(value.len()).map_err(|_| FxSelectorNameError::TooLong {
            actual: value.len(),
            limit: FX_MAX_SELECTOR_NAME_BYTES,
        })?;
        Ok(Self { value, byte_len })
    }
    pub fn as_str(&self) -> &str {
        &self.value
    }
    pub const fn byte_len(&self) -> u8 {
        self.byte_len
    }
    fn encode_canonical_v1<S: CanonicalSink>(
        &self,
        encoder: &mut CanonicalEncoder<S>,
    ) -> Result<(), S::Error> {
        encoder.unsigned(u64::from(self.byte_len))?;
        encoder.raw_bytes(self.value.as_bytes())
    }
}

impl Serialize for FxSelectorName {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.value)
    }
}

impl<'de> Deserialize<'de> for FxSelectorName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl FxSelectorId {
    pub fn try_new(
        domain: FxSelectorDomain,
        name: impl Into<String>,
    ) -> Result<Self, FxSelectorNameError> {
        Ok(Self {
            domain,
            name: FxSelectorName::try_new(name)?,
        })
    }
    pub const fn domain(&self) -> FxSelectorDomain {
        self.domain
    }
    pub const fn name(&self) -> &FxSelectorName {
        &self.name
    }
    pub(super) fn encode_canonical_v1<S: CanonicalSink>(
        &self,
        encoder: &mut CanonicalEncoder<S>,
    ) -> Result<(), S::Error> {
        encoder.tag(self.domain as u8)?;
        self.name.encode_canonical_v1(encoder)
    }
}

impl FxEnumDomain {
    pub const ALL: [Self; 4] = [
        Self::Target,
        Self::Phase,
        Self::ShaderStage,
        Self::MotionFunction,
    ];
    pub const fn domain_id(self) -> ClosedEnumDomainId {
        ClosedEnumDomainId::new(ClosedEnumOwnerId::Fx, self as u8)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FxShaderStage {
    GlyphColor,
    OffscreenPass,
    PostProcess,
}

/// Closed deterministic motion functions accepted by the builtin `motion`
/// callable.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionFunction {
    BreathOrbit,
    ElasticBloom,
}

macro_rules! fx_enum_inventory {
    ($ty:ident, $domain:ident, [$($variant:ident => $tag:expr => $name:literal),+ $(,)?]) => {
        impl $ty {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn tag(self) -> u16 { match self { $(Self::$variant => $tag),+ } }
            pub const fn source_name(self) -> &'static str { match self { $(Self::$variant => $name),+ } }
            pub fn from_source_name(name: &str) -> Option<Self> { match name { $($name => Some(Self::$variant),)+ _ => None } }
            pub const fn value_id(self) -> ClosedEnumValueId { ClosedEnumValueId::new(FxEnumDomain::$domain.domain_id(), self.tag()) }
            pub fn from_value_id(value: ClosedEnumValueId) -> Option<Self> {
                if value.domain().owner() != ClosedEnumOwnerId::Fx || value.domain().local_tag() != FxEnumDomain::$domain as u8 { return None; }
                match value.variant() { $($tag => Some(Self::$variant),)+ _ => None }
            }
        }
    }
}

fx_enum_inventory!(FxShaderStage, ShaderStage, [GlyphColor => 0 => "glyph_color", OffscreenPass => 1 => "offscreen_pass", PostProcess => 2 => "post_process"]);
fx_enum_inventory!(MotionFunction, MotionFunction, [BreathOrbit => 0 => "breath_orbit", ElasticBloom => 1 => "elastic_bloom"]);

const FX_TARGET_MEMBERS: &[ClosedEnumMemberDescriptor] = &[
    ClosedEnumMemberDescriptor::new(0, "node"),
    ClosedEnumMemberDescriptor::new(1, "content"),
    ClosedEnumMemberDescriptor::new(2, "background"),
    ClosedEnumMemberDescriptor::new(3, "line"),
    ClosedEnumMemberDescriptor::new(4, "glyph"),
    ClosedEnumMemberDescriptor::new(5, "viewport"),
];
const FX_PHASE_MEMBERS: &[ClosedEnumMemberDescriptor] = &[
    ClosedEnumMemberDescriptor::new(0, "before_layout"),
    ClosedEnumMemberDescriptor::new(1, "layout_transform"),
    ClosedEnumMemberDescriptor::new(2, "glyph_transform"),
    ClosedEnumMemberDescriptor::new(3, "glyph_color"),
    ClosedEnumMemberDescriptor::new(4, "glyph_mask"),
    ClosedEnumMemberDescriptor::new(5, "offscreen_pass"),
    ClosedEnumMemberDescriptor::new(6, "post_process"),
    ClosedEnumMemberDescriptor::new(7, "transition"),
];
const FX_SHADER_STAGE_MEMBERS: &[ClosedEnumMemberDescriptor] = &[
    ClosedEnumMemberDescriptor::new(0, "glyph_color"),
    ClosedEnumMemberDescriptor::new(1, "offscreen_pass"),
    ClosedEnumMemberDescriptor::new(2, "post_process"),
];
const FX_MOTION_FUNCTION_MEMBERS: &[ClosedEnumMemberDescriptor] = &[
    ClosedEnumMemberDescriptor::new(0, "breath_orbit"),
    ClosedEnumMemberDescriptor::new(1, "elastic_bloom"),
];
pub const FX_CLOSED_ENUM_DOMAINS: &[ClosedEnumDomainDescriptor] = &[
    ClosedEnumDomainDescriptor::new(
        FxEnumDomain::Target.domain_id(),
        "Fx target",
        FX_TARGET_MEMBERS,
    ),
    ClosedEnumDomainDescriptor::new(
        FxEnumDomain::Phase.domain_id(),
        "Fx phase",
        FX_PHASE_MEMBERS,
    ),
    ClosedEnumDomainDescriptor::new(
        FxEnumDomain::ShaderStage.domain_id(),
        "Fx shader stage",
        FX_SHADER_STAGE_MEMBERS,
    ),
    ClosedEnumDomainDescriptor::new(
        FxEnumDomain::MotionFunction.domain_id(),
        "Fx motion function",
        FX_MOTION_FUNCTION_MEMBERS,
    ),
];

/// Closed application target vocabulary.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FxTarget {
    Node = 0,
    #[default]
    Content = 1,
    Background = 2,
    Line = 3,
    Glyph = 4,
    Viewport = 5,
}

/// Closed renderer contract used in ABI hashes and capability resolution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FxRendererInterface {
    TextStyle = 0,
    Color = 1,
    Transform = 2,
    Mask = 3,
    Filter = 4,
    ShaderUniform = 5,
    OffscreenPass = 6,
    PostProcess = 7,
    Transition = 8,
    GeometryTransform = 9,
}

impl FxRendererInterface {
    pub const ALL: [Self; 10] = [
        Self::TextStyle,
        Self::Color,
        Self::Transform,
        Self::Mask,
        Self::Filter,
        Self::ShaderUniform,
        Self::OffscreenPass,
        Self::PostProcess,
        Self::Transition,
        Self::GeometryTransform,
    ];

    pub const fn semantic_tag(self) -> u8 {
        self as u8
    }
}

/// Fixed evaluation and renderer submission phases.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FxPhase {
    BeforeLayout = 0,
    LayoutTransform = 1,
    GlyphTransform = 2,
    GlyphColor = 3,
    GlyphMask = 4,
    OffscreenPass = 5,
    PostProcess = 6,
    Transition = 7,
}

impl FxTarget {
    pub const ALL: [Self; 6] = [
        Self::Node,
        Self::Content,
        Self::Background,
        Self::Line,
        Self::Glyph,
        Self::Viewport,
    ];
    pub const fn tag(self) -> u16 {
        self as u16
    }
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Content => "content",
            Self::Background => "background",
            Self::Line => "line",
            Self::Glyph => "glyph",
            Self::Viewport => "viewport",
        }
    }
    pub fn from_source_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|value| value.source_name() == name)
    }
    pub const fn value_id(self) -> ClosedEnumValueId {
        ClosedEnumValueId::new(FxEnumDomain::Target.domain_id(), self.tag())
    }
    pub fn from_value_id(value: ClosedEnumValueId) -> Option<Self> {
        if value.domain().owner() != ClosedEnumOwnerId::Fx
            || value.domain().local_tag() != FxEnumDomain::Target as u8
        {
            return None;
        }
        match value.variant() {
            0 => Some(Self::Node),
            1 => Some(Self::Content),
            2 => Some(Self::Background),
            3 => Some(Self::Line),
            4 => Some(Self::Glyph),
            5 => Some(Self::Viewport),
            _ => None,
        }
    }
}

impl FxPhase {
    pub const ALL: [Self; 8] = [
        Self::BeforeLayout,
        Self::LayoutTransform,
        Self::GlyphTransform,
        Self::GlyphColor,
        Self::GlyphMask,
        Self::OffscreenPass,
        Self::PostProcess,
        Self::Transition,
    ];
    pub const fn tag(self) -> u16 {
        self as u16
    }
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::BeforeLayout => "before_layout",
            Self::LayoutTransform => "layout_transform",
            Self::GlyphTransform => "glyph_transform",
            Self::GlyphColor => "glyph_color",
            Self::GlyphMask => "glyph_mask",
            Self::OffscreenPass => "offscreen_pass",
            Self::PostProcess => "post_process",
            Self::Transition => "transition",
        }
    }
    pub fn from_source_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|value| value.source_name() == name)
    }
    pub const fn value_id(self) -> ClosedEnumValueId {
        ClosedEnumValueId::new(FxEnumDomain::Phase.domain_id(), self.tag())
    }
    pub fn from_value_id(value: ClosedEnumValueId) -> Option<Self> {
        if value.domain().owner() != ClosedEnumOwnerId::Fx
            || value.domain().local_tag() != FxEnumDomain::Phase as u8
        {
            return None;
        }
        match value.variant() {
            0 => Some(Self::BeforeLayout),
            1 => Some(Self::LayoutTransform),
            2 => Some(Self::GlyphTransform),
            3 => Some(Self::GlyphColor),
            4 => Some(Self::GlyphMask),
            5 => Some(Self::OffscreenPass),
            6 => Some(Self::PostProcess),
            7 => Some(Self::Transition),
            _ => None,
        }
    }
}

/// Deterministic set of renderer interfaces.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FxRendererInterfaceSet(BTreeSet<FxRendererInterface>);

/// One exact target/interface capability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxCapability {
    pub target: FxTarget,
    pub interface: FxRendererInterface,
}

/// Renderer/provider capability inventory. Unsupported pairs are observable errors.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FxCapabilitySet(BTreeSet<FxCapability>);

impl FxRendererInterfaceSet {
    pub fn new(values: impl IntoIterator<Item = FxRendererInterface>) -> Self {
        Self(values.into_iter().collect())
    }

    pub fn contains(&self, interface: FxRendererInterface) -> bool {
        self.0.contains(&interface)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = FxRendererInterface> + '_ {
        self.0.iter().copied()
    }

    pub fn insert(&mut self, interface: FxRendererInterface) -> bool {
        self.0.insert(interface)
    }
}

impl FxCapabilitySet {
    pub fn new(values: impl IntoIterator<Item = FxCapability>) -> Self {
        Self(values.into_iter().collect())
    }

    /// Full capability contract described by the shared Arcweft renderer model.
    pub fn canonical() -> Self {
        let mut values = BTreeSet::new();
        for target in [
            FxTarget::Node,
            FxTarget::Content,
            FxTarget::Background,
            FxTarget::Line,
            FxTarget::Glyph,
            FxTarget::Viewport,
        ] {
            for interface in [
                FxRendererInterface::TextStyle,
                FxRendererInterface::Color,
                FxRendererInterface::Transform,
                FxRendererInterface::Mask,
                FxRendererInterface::Filter,
                FxRendererInterface::ShaderUniform,
                FxRendererInterface::OffscreenPass,
                FxRendererInterface::PostProcess,
                FxRendererInterface::Transition,
                FxRendererInterface::GeometryTransform,
            ] {
                if canonical_supports(target, interface) {
                    values.insert(FxCapability { target, interface });
                }
            }
        }
        Self(values)
    }

    pub fn supports(&self, target: FxTarget, interface: FxRendererInterface) -> bool {
        self.0.contains(&FxCapability { target, interface })
    }

    pub fn insert(&mut self, target: FxTarget, interface: FxRendererInterface) -> bool {
        self.0.insert(FxCapability { target, interface })
    }
}

fn canonical_supports(target: FxTarget, interface: FxRendererInterface) -> bool {
    use FxRendererInterface as Interface;
    match target {
        FxTarget::Node => matches!(
            interface,
            Interface::TextStyle
                | Interface::Color
                | Interface::Transform
                | Interface::GeometryTransform
                | Interface::Mask
                | Interface::Filter
                | Interface::ShaderUniform
                | Interface::OffscreenPass
                | Interface::Transition
        ),
        FxTarget::Content => matches!(
            interface,
            Interface::TextStyle
                | Interface::Color
                | Interface::Transform
                | Interface::GeometryTransform
                | Interface::Mask
                | Interface::Filter
                | Interface::ShaderUniform
                | Interface::OffscreenPass
        ),
        FxTarget::Background => matches!(
            interface,
            Interface::Color
                | Interface::Transform
                | Interface::Mask
                | Interface::Filter
                | Interface::ShaderUniform
                | Interface::OffscreenPass
        ),
        FxTarget::Line => matches!(
            interface,
            Interface::TextStyle
                | Interface::Color
                | Interface::Transform
                | Interface::Mask
                | Interface::ShaderUniform
                | Interface::OffscreenPass
        ),
        FxTarget::Glyph => matches!(
            interface,
            Interface::TextStyle
                | Interface::Color
                | Interface::Transform
                | Interface::Mask
                | Interface::ShaderUniform
        ),
        FxTarget::Viewport => matches!(
            interface,
            Interface::Transform
                | Interface::GeometryTransform
                | Interface::ShaderUniform
                | Interface::PostProcess
                | Interface::Transition
        ),
    }
}
