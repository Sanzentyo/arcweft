//! Typed static Fx graphs, renderer interfaces, targets, and capability contracts.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use thiserror::Error;

use super::{
    canonical::{
        CanonicalEncodeError, CanonicalEncoder, CanonicalHashSink, CanonicalLengthSink,
        CanonicalReader, CanonicalSink, CanonicalVecSink,
    },
    capability::{
        FxPhase, FxRendererInterface, FxRendererInterfaceSet, FxSelectorNameError, FxShaderStage,
        FxTarget,
    },
    identity::{FxAbiHash, FxId, FxIdCanonicalDecodeError, FxSemanticHash},
    program::{FxSamplerProgram, FxSamplerProgramDecodeError},
    uniform::FxUniformRecordDecodeError,
    value::{FxRuntimeType, FxRuntimeValue, FxRuntimeValueDecodeError},
};

mod canonical;
pub use canonical::FxDefinitionDecodeError;

pub const FX_MAX_DEFINITIONS_PER_SECTION: usize = 4_096;
pub const FX_MAX_PARAMETERS_PER_DEFINITION: usize = 64;
pub const FX_MAX_GRAPH_NODES_PER_DEFINITION: usize = 4_096;
pub const FX_MAX_GRAPH_DEPTH: usize = 64;
pub const FX_MAX_GRAPH_CHILD_EDGES_PER_DEFINITION: usize = 4_096;
pub const FX_MAX_TOTAL_GRAPH_NODES_PER_SECTION: usize = 65_536;
pub const FX_MAX_RESOURCE_ID_BYTES: usize = 1_024;
pub const FX_MAX_DEFINITION_PARAMETER_NAME_BYTES: usize = 64;
pub const FX_MAX_FONT_FAMILY_NAME_BYTES: usize = 256;
pub const FX_MAX_DEFINITION_CANONICAL_BYTES: usize = 32 * 1024 * 1024;

/// Closed constructor inventory used by lowering and graph validation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FxNodeKind {
    Style = 0,
    Text = 1,
    Color = 2,
    Transform = 3,
    Mask = 4,
    Filter = 5,
    Shader = 6,
    OffscreenPass = 7,
    PostProcess = 8,
    Transition = 9,
    Conditional = 10,
    Stack = 11,
}

/// Closed source-callable constructor inventory. Runtime-only render passes are
/// intentionally absent from this authoring algebra.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FxSourceConstructor {
    Style,
    Text,
    Color,
    Transform,
    Mask,
    Filter,
    Transition,
    Conditional,
    Stack,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FxSourceParameterPassing {
    PositionalOnly,
    NamedOnly,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FxSourceParameterPresence {
    Required,
    Optional,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FxSourceParameterRole {
    Property(FxPropertyId),
    Condition,
    ThenGraph,
    ElseGraph,
    Graphs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FxSourceParameterType {
    Static(FxStaticType),
    Fx,
    FxList,
    TransitionKind,
    TransitionEasing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FxSourceParameter {
    source_name: &'static str,
    role: FxSourceParameterRole,
    value_type: FxSourceParameterType,
    passing: FxSourceParameterPassing,
    presence: FxSourceParameterPresence,
}

/// Closed identity of a property on an Fx graph node.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FxPropertyId {
    Target = 0,
    Phase = 1,
    Opacity = 2,
    Weight = 3,
    Slant = 4,
    FontFamily = 5,
    Size = 6,
    Spacing = 7,
    Color = 8,
    Tint = 9,
    Multiply = 10,
    Transform = 11,
    Sampler = 12,
    Resource = 13,
    Coverage = 14,
    Invert = 15,
    BlurRadius = 16,
    Brightness = 17,
    Contrast = 18,
    Saturation = 19,
    Stage = 20,
    Uniforms = 21,
    Kind = 22,
    Easing = 23,
    Duration = 24,
    Progress = 25,
}

/// Closed static property expectation returned to compiler lowering.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", content = "runtime_type", rename_all = "snake_case")]
pub enum FxStaticType {
    Runtime(FxRuntimeType),
    Resource,
    Selector(super::FxSelectorDomain),
    ShaderStage,
    FontFamily,
    Target,
    Phase,
    UniformRecord,
}

/// Validated resource identity retained as static graph data.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FxResourceId {
    value: String,
    byte_len: u16,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxResourceIdError {
    #[error("Fx resource ID cannot be empty")]
    Empty,
    #[error("Fx resource ID has {actual} UTF-8 bytes, exceeding the limit of {limit}")]
    TooLong { actual: usize, limit: usize },
    #[error("Fx resource ID contains a non-canonical character")]
    InvalidCharacter,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FxFontFamilyName {
    value: String,
    byte_len: u16,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxFontFamilyNameError {
    #[error("Fx font-family name cannot be empty")]
    Empty,
    #[error("Fx font-family name must already be trimmed")]
    NotTrimmed,
    #[error("Fx font-family name contains a control character")]
    ControlCharacter,
    #[error("Fx font-family name has {actual} UTF-8 bytes, exceeding the limit of {limit}")]
    TooLong { actual: usize, limit: usize },
}

/// ABI index of one definition parameter.
///
/// The value is bounded to the definition owner limit.  Parameter references
/// are issued by validated parameter rows; consumers can inspect this value but
/// cannot construct a reference by writing fields.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FxDefinitionParameterIndex(u16);

/// Dense runtime storage slot owned by one Fx definition layout.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FxRuntimeParameterSlot(u16);

/// Dense static storage slot owned by one Fx definition layout.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FxStaticParameterSlot(u16);

/// The closed ABI type algebra for one definition parameter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", content = "runtime_type", rename_all = "snake_case")]
pub enum FxDefinitionParameterType {
    Runtime(FxRuntimeType),
    Resource,
    UniformRecord,
}

/// Exact typed argument carried by an Fx application ABI row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FxDefinitionArgumentValue {
    Runtime(FxRuntimeValue),
    Resource(FxResourceId),
    UniformRecord(super::FxUniformRecord),
}

/// Owner-issued reference to one ABI parameter row.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FxDefinitionParameterRef {
    index: FxDefinitionParameterIndex,
    ty: FxDefinitionParameterType,
}

/// Owner-issued reference to one dense runtime parameter slot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FxRuntimeParameterRef {
    slot: FxRuntimeParameterSlot,
    ty: FxRuntimeType,
}

/// Dense storage selected for one ABI parameter row.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", content = "slot", rename_all = "snake_case")]
pub enum FxParameterStorageSlot {
    Runtime(FxRuntimeParameterSlot),
    Static(FxStaticParameterSlot),
}

/// Forward ABI-to-storage mapping row.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FxDefinitionParameterLayoutRow {
    parameter: FxDefinitionParameterRef,
    storage: FxParameterStorageSlot,
}

/// Reverse row for one dense runtime parameter slot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FxRuntimeParameterLayoutRow {
    slot: FxRuntimeParameterSlot,
    parameter: FxDefinitionParameterRef,
    ty: FxRuntimeType,
}

/// Reverse row for one dense static parameter slot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FxStaticParameterLayoutRow {
    slot: FxStaticParameterSlot,
    parameter: FxDefinitionParameterRef,
    ty: FxDefinitionParameterType,
}

/// Digest of the ABI-to-storage mapping and parameter rows.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FxDefinitionParameterLayoutDigest([u8; 32]);

/// Derived dense runtime/static storage for one definition ABI.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FxDefinitionParameterLayout {
    abi_to_storage: Box<[FxDefinitionParameterLayoutRow]>,
    runtime: Box<[FxRuntimeParameterLayoutRow]>,
    static_: Box<[FxStaticParameterLayoutRow]>,
    #[serde(skip)]
    abi_count: u16,
    #[serde(skip)]
    runtime_count: u16,
    #[serde(skip)]
    static_count: u16,
    digest: FxDefinitionParameterLayoutDigest,
}

/// Canonical ordered parameter authority shared by graph compilation and definition sealing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FxDefinitionParameterSchema {
    id: FxId,
    parameters: Vec<FxDefinitionParameter>,
    layout: FxDefinitionParameterLayout,
    digest: FxDefinitionParameterSchemaDigest,
    #[serde(skip)]
    canonical_len: u32,
}

/// Borrowed graph-construction context for a definition's outer parameter
/// schema.
///
/// A graph fragment may reference only parameters issued by this context. The
/// context also supplies the complete dense runtime schema required by every
/// embedded value program and validates the finished graph against the outer
/// definition layout before publication.
#[derive(Clone, Copy, Debug)]
pub struct FxDefinitionGraphContext<'a> {
    schema: &'a FxDefinitionParameterSchema,
}

/// Digest of one definition identity and its ordered source-independent ABI rows.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FxDefinitionParameterSchemaDigest([u8; 32]);

/// Static graph value. Executable arithmetic is represented only by typed programs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FxStaticValue {
    Runtime(FxRuntimeValue),
    Resource(FxResourceId),
    Selector(super::FxSelectorId),
    ShaderStage(super::FxShaderStage),
    FontFamily(FxFontFamilyName),
    Target(FxTarget),
    Phase(FxPhase),
    Parameter(FxDefinitionParameterRef),
    Sampler(FxSamplerProgram),
    UniformRecord(super::FxUniformRecord),
}

/// Named, typed input owned by one graph node.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FxProperty {
    #[serde(rename = "name")]
    id: FxPropertyId,
    value: FxStaticValue,
}

/// One validated ABI row exported by an Fx definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FxDefinitionParameter {
    index: FxDefinitionParameterIndex,
    name: FxDefinitionParameterName,
    ty: FxDefinitionParameterType,
    default: Option<FxDefinitionArgumentValue>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FxDefinitionParameterName {
    value: String,
    byte_len: u8,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxDefinitionParameterNameError {
    #[error("invalid Fx definition parameter name `{name}`")]
    Invalid { name: String },
    #[error(
        "Fx definition parameter name has {actual} UTF-8 bytes, exceeding the limit of {limit}"
    )]
    TooLong { actual: usize, limit: usize },
}

/// One typed treatment node.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FxNode {
    Style {
        properties: Vec<FxProperty>,
    },
    Text {
        properties: Vec<FxProperty>,
    },
    Color {
        properties: Vec<FxProperty>,
    },
    Transform {
        fx: FxId,
        properties: Vec<FxProperty>,
    },
    Mask {
        fx: FxId,
        properties: Vec<FxProperty>,
    },
    Filter {
        fx: FxId,
        properties: Vec<FxProperty>,
    },
    Shader {
        fx: FxId,
        properties: Vec<FxProperty>,
    },
    OffscreenPass {
        fx: FxId,
        properties: Vec<FxProperty>,
    },
    PostProcess {
        fx: FxId,
        properties: Vec<FxProperty>,
    },
    Transition {
        fx: FxId,
        properties: Vec<FxProperty>,
    },
    Conditional {
        condition: FxStaticValue,
        then_graph: FxGraph,
        else_graph: FxGraph,
    },
    Stack {
        children: Vec<FxGraph>,
    },
}

/// Authored ordered graph.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct FxGraph {
    nodes: Vec<FxNode>,
    #[serde(skip)]
    node_count: u16,
}

/// One complete compiled `#[fx]` declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FxDefinition {
    id: FxId,
    parameters: Vec<FxDefinitionParameter>,
    layout: FxDefinitionParameterLayout,
    graph: FxGraph,
    abi_hash: FxAbiHash,
    semantic_hash: FxSemanticHash,
    #[serde(skip)]
    canonical_len: u32,
}

/// Invalid graph structure or closed constructor property.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxGraphError {
    #[error("graph has {actual} expanded nodes, exceeding the limit of {limit}")]
    TooManyNodes { actual: usize, limit: usize },
    #[error("graph depth {actual} exceeds the limit of {limit}")]
    TooDeep { actual: usize, limit: usize },
    #[error("graph has {actual} child-graph edges, exceeding the limit of {limit}")]
    TooManyChildEdges { actual: usize, limit: usize },
    #[error(
        "Fx `{node}` constructor has {actual} properties, exceeding its schema limit of {limit}"
    )]
    TooManyProperties {
        node: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("Fx `{node}` constructor has no property named `{property}`")]
    UnknownProperty {
        node: &'static str,
        property: String,
    },
    #[error("Fx `{node}.{property}` has value kind {actual}, expected {expected}")]
    PropertyTypeMismatch {
        node: &'static str,
        property: String,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("Fx conditional requires a Bool runtime value, parameter, or sampler")]
    InvalidCondition,
    #[error("Fx `{node}` constructor repeats property `{property}`")]
    DuplicateProperty {
        node: &'static str,
        property: String,
    },
}

/// Invalid definition schema or stored hash.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxDefinitionError {
    #[error("Fx definition has {actual} parameters, exceeding the limit of {limit}")]
    TooManyParameters { actual: usize, limit: usize },
    #[error(transparent)]
    ParameterName(#[from] FxDefinitionParameterNameError),
    #[error(
        "Fx definition parameter rows must use ABI indices 0..N; row {actual} has index {index}"
    )]
    ParameterIndex { actual: usize, index: u16 },
    #[error("duplicate Fx definition parameter `{name}`")]
    DuplicateParameter { name: String },
    #[error(
        "default for Fx definition parameter `{name}` has type {actual:?}, expected {expected:?}"
    )]
    ParameterDefaultType {
        name: String,
        expected: FxDefinitionParameterType,
        actual: FxDefinitionParameterType,
    },
    #[error(
        "Fx graph references ABI parameter index {index}, but only {available} parameters exist"
    )]
    ParameterReferenceOutOfBounds { index: u16, available: usize },
    #[error("Fx graph ABI parameter index {index} declares {actual:?}, expected {expected:?}")]
    ParameterReferenceType {
        index: u16,
        expected: FxDefinitionParameterType,
        actual: FxDefinitionParameterType,
    },
    #[error("Fx runtime parameter ref slot {slot} is not in the derived dense runtime layout")]
    RuntimeParameterReference { slot: u16, expected: usize },
    #[error("Fx sampler parameter schema does not match the derived dense runtime layout")]
    SamplerParameterSchema,
    #[error("stored Fx definition parameter layout is not the canonical derived layout")]
    ParameterLayoutMismatch,
    #[error(transparent)]
    Graph(#[from] FxGraphError),
    #[error("stored Fx ABI hash does not match the typed contract")]
    AbiHashMismatch,
    #[error("stored Fx semantic hash does not match the typed graph")]
    SemanticHashMismatch,
    #[error("Fx definition canonical transcript exceeds the byte limit of {limit}")]
    CanonicalTranscriptTooLarge { limit: usize },
    #[error("Fx definition canonical transcript length overflow")]
    CanonicalLengthOverflow,
    #[error("Fx definition canonical allocation failed")]
    CanonicalAllocationFailed,
    #[error("Fx definition canonical writer produced {actual} bytes, expected {expected}")]
    CanonicalLengthMismatch { actual: usize, expected: usize },
}

impl FxNodeKind {
    pub const ALL: [Self; 12] = [
        Self::Style,
        Self::Text,
        Self::Color,
        Self::Transform,
        Self::Mask,
        Self::Filter,
        Self::Shader,
        Self::OffscreenPass,
        Self::PostProcess,
        Self::Transition,
        Self::Conditional,
        Self::Stack,
    ];

    pub const fn source_constructor(self) -> Option<FxSourceConstructor> {
        Some(match self {
            Self::Style => FxSourceConstructor::Style,
            Self::Text => FxSourceConstructor::Text,
            Self::Color => FxSourceConstructor::Color,
            Self::Transform => FxSourceConstructor::Transform,
            Self::Mask => FxSourceConstructor::Mask,
            Self::Filter => FxSourceConstructor::Filter,
            Self::Transition => FxSourceConstructor::Transition,
            Self::Conditional => FxSourceConstructor::Conditional,
            Self::Stack => FxSourceConstructor::Stack,
            Self::Shader | Self::OffscreenPass | Self::PostProcess => return None,
        })
    }

    pub const fn semantic_tag(self) -> u8 {
        self as u8
    }

    /// Returns the single presentation-owned property expectation for lowering.
    ///
    /// `target` and `phase` are shared by every property-bearing treatment
    /// constructor. Conditional and stack use their fixed typed fields instead.
    pub const fn accepts_property(self, property: FxPropertyId) -> bool {
        use FxPropertyId as Property;
        if matches!(property, Property::Target | Property::Phase) {
            return !matches!(self, Self::Conditional | Self::Stack);
        }
        match self {
            Self::Style | Self::Text => matches!(
                property,
                Property::Opacity
                    | Property::Weight
                    | Property::Slant
                    | Property::FontFamily
                    | Property::Size
                    | Property::Spacing
                    | Property::Color
            ),
            Self::Color => matches!(
                property,
                Property::Tint | Property::Multiply | Property::Opacity
            ),
            Self::Transform => matches!(property, Property::Transform | Property::Sampler),
            Self::Mask => matches!(
                property,
                Property::Resource | Property::Coverage | Property::Invert
            ),
            Self::Filter => matches!(
                property,
                Property::BlurRadius
                    | Property::Brightness
                    | Property::Contrast
                    | Property::Saturation
            ),
            Self::Shader => matches!(
                property,
                Property::Resource | Property::Stage | Property::Uniforms
            ),
            Self::OffscreenPass | Self::PostProcess => matches!(property, Property::Resource),
            Self::Transition => matches!(
                property,
                Property::Kind | Property::Easing | Property::Duration | Property::Progress
            ),
            Self::Conditional | Self::Stack => false,
        }
    }

    pub const fn renderer_interface(self) -> Option<FxRendererInterface> {
        match self {
            Self::Style | Self::Text => Some(FxRendererInterface::TextStyle),
            Self::Color => Some(FxRendererInterface::Color),
            Self::Transform => Some(FxRendererInterface::Transform),
            Self::Mask => Some(FxRendererInterface::Mask),
            Self::Filter => Some(FxRendererInterface::Filter),
            Self::Shader => Some(FxRendererInterface::ShaderUniform),
            Self::OffscreenPass => Some(FxRendererInterface::OffscreenPass),
            Self::PostProcess => Some(FxRendererInterface::PostProcess),
            Self::Transition => Some(FxRendererInterface::Transition),
            Self::Conditional | Self::Stack => None,
        }
    }

    pub const fn property_capacity(self) -> usize {
        match self {
            Self::Style | Self::Text => 9,
            Self::Color | Self::Mask | Self::Shader => 5,
            Self::Transform => 4,
            Self::Filter | Self::Transition => 6,
            Self::OffscreenPass | Self::PostProcess => 3,
            Self::Conditional | Self::Stack => 0,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Style => "style",
            Self::Text => "text",
            Self::Color => "color",
            Self::Transform => "transform",
            Self::Mask => "mask",
            Self::Filter => "filter",
            Self::Shader => "shader",
            Self::OffscreenPass => "offscreen_pass",
            Self::PostProcess => "post_process",
            Self::Transition => "transition",
            Self::Conditional => "conditional",
            Self::Stack => "stack",
        }
    }
}

impl FxSourceConstructor {
    pub const ALL: [Self; 9] = [
        Self::Style,
        Self::Text,
        Self::Color,
        Self::Transform,
        Self::Mask,
        Self::Filter,
        Self::Transition,
        Self::Conditional,
        Self::Stack,
    ];

    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Style => 0,
            Self::Text => 1,
            Self::Color => 2,
            Self::Transform => 3,
            Self::Mask => 4,
            Self::Filter => 5,
            Self::Transition => 6,
            Self::Conditional => 7,
            Self::Stack => 8,
        }
    }

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Style => "style",
            Self::Text => "text",
            Self::Color => "color",
            Self::Transform => "transform",
            Self::Mask => "mask",
            Self::Filter => "filter",
            Self::Transition => "transition",
            Self::Conditional => "conditional",
            Self::Stack => "stack",
        }
    }

    pub fn from_source_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|constructor| constructor.source_name() == name)
    }

    pub const fn node_kind(self) -> FxNodeKind {
        match self {
            Self::Style => FxNodeKind::Style,
            Self::Text => FxNodeKind::Text,
            Self::Color => FxNodeKind::Color,
            Self::Transform => FxNodeKind::Transform,
            Self::Mask => FxNodeKind::Mask,
            Self::Filter => FxNodeKind::Filter,
            Self::Transition => FxNodeKind::Transition,
            Self::Conditional => FxNodeKind::Conditional,
            Self::Stack => FxNodeKind::Stack,
        }
    }

    pub fn property_from_source_name(self, name: &str) -> Option<FxPropertyId> {
        if self == Self::Transform && name == "sample" {
            return Some(FxPropertyId::Sampler);
        }
        let property = FxPropertyId::from_source_name(name)?;
        self.node_kind()
            .accepts_property(property)
            .then_some(property)
    }

    pub const fn parameter_schema(self) -> &'static [FxSourceParameter] {
        match self {
            Self::Style => STYLE_SOURCE_PARAMETERS,
            Self::Text => TEXT_SOURCE_PARAMETERS,
            Self::Color => COLOR_SOURCE_PARAMETERS,
            Self::Transform => TRANSFORM_SOURCE_PARAMETERS,
            Self::Mask => MASK_SOURCE_PARAMETERS,
            Self::Filter => FILTER_SOURCE_PARAMETERS,
            Self::Transition => TRANSITION_SOURCE_PARAMETERS,
            Self::Conditional => CONDITIONAL_SOURCE_PARAMETERS,
            Self::Stack => STACK_SOURCE_PARAMETERS,
        }
    }
}

impl FxSourceParameter {
    const fn new(
        source_name: &'static str,
        role: FxSourceParameterRole,
        value_type: FxSourceParameterType,
        passing: FxSourceParameterPassing,
        presence: FxSourceParameterPresence,
    ) -> Self {
        Self {
            source_name,
            role,
            value_type,
            passing,
            presence,
        }
    }
    pub const fn source_name(self) -> &'static str {
        self.source_name
    }
    pub const fn role(self) -> FxSourceParameterRole {
        self.role
    }
    pub const fn value_type(self) -> FxSourceParameterType {
        self.value_type
    }
    pub const fn passing(self) -> FxSourceParameterPassing {
        self.passing
    }
    pub const fn presence(self) -> FxSourceParameterPresence {
        self.presence
    }
}

macro_rules! source_property {
    ($id:ident) => {
        FxSourceParameter::new(
            FxPropertyId::$id.source_name(),
            FxSourceParameterRole::Property(FxPropertyId::$id),
            FxSourceParameterType::Static(FxPropertyId::$id.value_type()),
            FxSourceParameterPassing::NamedOnly,
            FxSourceParameterPresence::Optional,
        )
    };
    ($name:literal, $id:ident, $passing:ident) => {
        FxSourceParameter::new(
            $name,
            FxSourceParameterRole::Property(FxPropertyId::$id),
            FxSourceParameterType::Static(FxPropertyId::$id.value_type()),
            FxSourceParameterPassing::$passing,
            FxSourceParameterPresence::Optional,
        )
    };
}

const STYLE_SOURCE_PARAMETERS: &[FxSourceParameter] = &[
    source_property!(Target),
    source_property!(Phase),
    source_property!(Opacity),
    source_property!(Weight),
    source_property!(Slant),
    source_property!(FontFamily),
    source_property!(Size),
    source_property!(Spacing),
    source_property!(Color),
];
const TEXT_SOURCE_PARAMETERS: &[FxSourceParameter] = STYLE_SOURCE_PARAMETERS;
const COLOR_SOURCE_PARAMETERS: &[FxSourceParameter] = &[
    source_property!(Target),
    source_property!(Phase),
    source_property!(Opacity),
    source_property!(Tint),
    source_property!(Multiply),
];
const TRANSFORM_SOURCE_PARAMETERS: &[FxSourceParameter] = &[
    source_property!(Target),
    source_property!(Phase),
    source_property!(Transform),
    source_property!("sample", Sampler, NamedOnly),
];
const MASK_SOURCE_PARAMETERS: &[FxSourceParameter] = &[
    source_property!(Target),
    source_property!(Phase),
    source_property!(Resource),
    source_property!(Coverage),
    source_property!(Invert),
];
const FILTER_SOURCE_PARAMETERS: &[FxSourceParameter] = &[
    source_property!(Target),
    source_property!(Phase),
    source_property!(BlurRadius),
    source_property!(Brightness),
    source_property!(Contrast),
    source_property!(Saturation),
];
const TRANSITION_SOURCE_PARAMETERS: &[FxSourceParameter] = &[
    source_property!(Target),
    source_property!(Phase),
    FxSourceParameter::new(
        "kind",
        FxSourceParameterRole::Property(FxPropertyId::Kind),
        FxSourceParameterType::TransitionKind,
        FxSourceParameterPassing::NamedOnly,
        FxSourceParameterPresence::Optional,
    ),
    FxSourceParameter::new(
        "easing",
        FxSourceParameterRole::Property(FxPropertyId::Easing),
        FxSourceParameterType::TransitionEasing,
        FxSourceParameterPassing::NamedOnly,
        FxSourceParameterPresence::Optional,
    ),
    source_property!(Duration),
    source_property!(Progress),
];
const CONDITIONAL_SOURCE_PARAMETERS: &[FxSourceParameter] = &[
    FxSourceParameter::new(
        "condition",
        FxSourceParameterRole::Condition,
        FxSourceParameterType::Static(FxStaticType::Runtime(FxRuntimeType::Bool)),
        FxSourceParameterPassing::NamedOnly,
        FxSourceParameterPresence::Required,
    ),
    FxSourceParameter::new(
        "then",
        FxSourceParameterRole::ThenGraph,
        FxSourceParameterType::Fx,
        FxSourceParameterPassing::NamedOnly,
        FxSourceParameterPresence::Required,
    ),
    FxSourceParameter::new(
        "else",
        FxSourceParameterRole::ElseGraph,
        FxSourceParameterType::Fx,
        FxSourceParameterPassing::NamedOnly,
        FxSourceParameterPresence::Required,
    ),
];
const STACK_SOURCE_PARAMETERS: &[FxSourceParameter] = &[FxSourceParameter::new(
    "graphs",
    FxSourceParameterRole::Graphs,
    FxSourceParameterType::FxList,
    FxSourceParameterPassing::PositionalOnly,
    FxSourceParameterPresence::Required,
)];

impl FxPropertyId {
    pub const ALL: [Self; 26] = [
        Self::Target,
        Self::Phase,
        Self::Opacity,
        Self::Weight,
        Self::Slant,
        Self::FontFamily,
        Self::Size,
        Self::Spacing,
        Self::Color,
        Self::Tint,
        Self::Multiply,
        Self::Transform,
        Self::Sampler,
        Self::Resource,
        Self::Coverage,
        Self::Invert,
        Self::BlurRadius,
        Self::Brightness,
        Self::Contrast,
        Self::Saturation,
        Self::Stage,
        Self::Uniforms,
        Self::Kind,
        Self::Easing,
        Self::Duration,
        Self::Progress,
    ];

    pub const fn semantic_tag(self) -> u8 {
        self as u8
    }

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Target => "target",
            Self::Phase => "phase",
            Self::Opacity => "opacity",
            Self::Weight => "weight",
            Self::Slant => "slant",
            Self::FontFamily => "font_family",
            Self::Size => "size",
            Self::Spacing => "spacing",
            Self::Color => "color",
            Self::Tint => "tint",
            Self::Multiply => "multiply",
            Self::Transform => "transform",
            Self::Sampler => "sampler",
            Self::Resource => "resource",
            Self::Coverage => "coverage",
            Self::Invert => "invert",
            Self::BlurRadius => "blur_radius",
            Self::Brightness => "brightness",
            Self::Contrast => "contrast",
            Self::Saturation => "saturation",
            Self::Stage => "stage",
            Self::Uniforms => "uniforms",
            Self::Kind => "kind",
            Self::Easing => "easing",
            Self::Duration => "duration",
            Self::Progress => "progress",
        }
    }

    pub fn from_source_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|property| property.source_name() == name)
    }

    pub const fn value_type(self) -> FxStaticType {
        use FxRuntimeType as Runtime;
        match self {
            Self::Target => FxStaticType::Target,
            Self::Phase => FxStaticType::Phase,
            Self::Opacity
            | Self::Coverage
            | Self::Brightness
            | Self::Contrast
            | Self::Saturation
            | Self::Progress => FxStaticType::Runtime(Runtime::F32),
            Self::Weight => FxStaticType::Runtime(Runtime::I32),
            Self::Slant => FxStaticType::Runtime(Runtime::Angle),
            Self::FontFamily => FxStaticType::FontFamily,
            Self::Size | Self::Spacing | Self::BlurRadius => FxStaticType::Runtime(Runtime::Length),
            Self::Color | Self::Tint | Self::Multiply => FxStaticType::Runtime(Runtime::Color),
            Self::Transform | Self::Sampler => FxStaticType::Runtime(Runtime::Transform2D),
            Self::Resource => FxStaticType::Resource,
            Self::Invert => FxStaticType::Runtime(Runtime::Bool),
            Self::Stage => FxStaticType::ShaderStage,
            Self::Kind => FxStaticType::Selector(super::FxSelectorDomain::TransitionKind),
            Self::Easing => FxStaticType::Selector(super::FxSelectorDomain::TransitionEasing),
            Self::Uniforms => FxStaticType::UniformRecord,
            Self::Duration => FxStaticType::Runtime(Runtime::Seconds),
        }
    }
}

impl FxResourceId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, FxResourceIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(FxResourceIdError::Empty);
        }
        if value.len() > FX_MAX_RESOURCE_ID_BYTES {
            return Err(FxResourceIdError::TooLong {
                actual: value.len(),
                limit: FX_MAX_RESOURCE_ID_BYTES,
            });
        }
        if !value.chars().all(|character| {
            character.is_alphanumeric() || matches!(character, '_' | '-' | '.' | '/')
        }) {
            return Err(FxResourceIdError::InvalidCharacter);
        }
        let byte_len = u16::try_from(value.len()).map_err(|_| FxResourceIdError::TooLong {
            actual: value.len(),
            limit: FX_MAX_RESOURCE_ID_BYTES,
        })?;
        Ok(Self { value, byte_len })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub const fn byte_len(&self) -> u16 {
        self.byte_len
    }
}

impl Serialize for FxResourceId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.value)
    }
}

impl<'de> Deserialize<'de> for FxResourceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl FxFontFamilyName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, FxFontFamilyNameError> {
        let value = value.into();
        if value.is_empty() {
            return Err(FxFontFamilyNameError::Empty);
        }
        if value.trim() != value {
            return Err(FxFontFamilyNameError::NotTrimmed);
        }
        if value.chars().any(char::is_control) {
            return Err(FxFontFamilyNameError::ControlCharacter);
        }
        if value.len() > FX_MAX_FONT_FAMILY_NAME_BYTES {
            return Err(FxFontFamilyNameError::TooLong {
                actual: value.len(),
                limit: FX_MAX_FONT_FAMILY_NAME_BYTES,
            });
        }
        let byte_len = u16::try_from(value.len()).map_err(|_| FxFontFamilyNameError::TooLong {
            actual: value.len(),
            limit: FX_MAX_FONT_FAMILY_NAME_BYTES,
        })?;
        Ok(Self { value, byte_len })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub const fn byte_len(&self) -> u16 {
        self.byte_len
    }
}

impl Serialize for FxFontFamilyName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.value)
    }
}

impl<'de> Deserialize<'de> for FxFontFamilyName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl FxStaticValue {
    pub fn runtime_type(&self) -> Option<FxRuntimeType> {
        match self {
            Self::Runtime(value) => Some(value.value_type()),
            Self::Parameter(parameter) => match parameter.ty {
                FxDefinitionParameterType::Runtime(ty) => Some(ty),
                FxDefinitionParameterType::Resource | FxDefinitionParameterType::UniformRecord => {
                    None
                }
            },
            Self::Sampler(program) => Some(program.return_type()),
            Self::Resource(_)
            | Self::Selector(_)
            | Self::ShaderStage(_)
            | Self::FontFamily(_)
            | Self::Target(_)
            | Self::Phase(_)
            | Self::UniformRecord(_) => None,
        }
    }

    pub fn static_type(&self) -> FxStaticType {
        match self {
            Self::Runtime(value) => FxStaticType::Runtime(value.value_type()),
            Self::Resource(_) => FxStaticType::Resource,
            Self::Selector(value) => FxStaticType::Selector(value.domain()),
            Self::ShaderStage(_) => FxStaticType::ShaderStage,
            Self::FontFamily(_) => FxStaticType::FontFamily,
            Self::Target(_) => FxStaticType::Target,
            Self::Phase(_) => FxStaticType::Phase,
            Self::Parameter(parameter) => parameter.ty.static_type(),
            Self::Sampler(program) => FxStaticType::Runtime(program.return_type()),
            Self::UniformRecord(_) => FxStaticType::UniformRecord,
        }
    }

    fn kind_name(&self) -> &'static str {
        self.static_type().as_str()
    }
}

impl FxStaticType {
    pub fn accepts(self, value: &FxStaticValue) -> bool {
        self == value.static_type()
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Runtime(ty) => runtime_type_name(ty),
            Self::Resource => "resource",
            Self::Selector(_) => "selector",
            Self::ShaderStage => "shader_stage",
            Self::FontFamily => "font_family",
            Self::Target => "target",
            Self::Phase => "phase",
            Self::UniformRecord => "uniform_record",
        }
    }
}

impl From<FxRuntimeValue> for FxStaticValue {
    fn from(value: FxRuntimeValue) -> Self {
        Self::Runtime(value)
    }
}

impl FxProperty {
    pub const fn new(id: FxPropertyId, value: FxStaticValue) -> Self {
        Self { id, value }
    }

    pub const fn id(&self) -> FxPropertyId {
        self.id
    }

    pub const fn name(&self) -> &'static str {
        self.id.source_name()
    }

    pub const fn value(&self) -> &FxStaticValue {
        &self.value
    }
}

impl FxDefinitionParameterType {
    pub const fn static_type(self) -> FxStaticType {
        match self {
            Self::Runtime(ty) => FxStaticType::Runtime(ty),
            Self::Resource => FxStaticType::Resource,
            Self::UniformRecord => FxStaticType::UniformRecord,
        }
    }
}

impl FxDefinitionArgumentValue {
    pub const fn parameter_type(&self) -> FxDefinitionParameterType {
        match self {
            Self::Runtime(value) => FxDefinitionParameterType::Runtime(value.value_type()),
            Self::Resource(_) => FxDefinitionParameterType::Resource,
            Self::UniformRecord(_) => FxDefinitionParameterType::UniformRecord,
        }
    }

    /// Canonical v1 bytes for one closed ABI value.
    pub fn canonical_v1_bytes(&self) -> Result<Vec<u8>, FxDefinitionError> {
        canonical::definition_argument_v1_bytes(self)
    }
}

impl FxDefinitionParameterIndex {
    pub fn from_index(index: usize) -> Option<Self> {
        u16::try_from(index).ok().map(Self)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

impl FxRuntimeParameterSlot {
    pub(crate) fn from_index(index: usize) -> Option<Self> {
        u16::try_from(index).ok().map(Self)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

impl FxStaticParameterSlot {
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl FxDefinitionParameterLayoutDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl FxDefinitionParameterRef {
    pub const fn index(self) -> FxDefinitionParameterIndex {
        self.index
    }

    pub const fn parameter_type(self) -> FxDefinitionParameterType {
        self.ty
    }
}

impl FxRuntimeParameterRef {
    pub(crate) const fn from_parts(slot: FxRuntimeParameterSlot, ty: FxRuntimeType) -> Self {
        Self { slot, ty }
    }

    pub const fn slot(self) -> FxRuntimeParameterSlot {
        self.slot
    }

    pub const fn runtime_type(self) -> FxRuntimeType {
        self.ty
    }
}

impl FxDefinitionParameterName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, FxDefinitionParameterNameError> {
        let value = value.into();
        if !valid_identifier(&value) {
            return Err(FxDefinitionParameterNameError::Invalid { name: value });
        }
        if value.len() > FX_MAX_DEFINITION_PARAMETER_NAME_BYTES {
            return Err(FxDefinitionParameterNameError::TooLong {
                actual: value.len(),
                limit: FX_MAX_DEFINITION_PARAMETER_NAME_BYTES,
            });
        }
        let byte_len =
            u8::try_from(value.len()).map_err(|_| FxDefinitionParameterNameError::TooLong {
                actual: value.len(),
                limit: FX_MAX_DEFINITION_PARAMETER_NAME_BYTES,
            })?;
        Ok(Self { value, byte_len })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub const fn byte_len(&self) -> u8 {
        self.byte_len
    }
}

impl Serialize for FxDefinitionParameterName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.value)
    }
}

impl<'de> Deserialize<'de> for FxDefinitionParameterName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl FxDefinitionParameter {
    pub fn try_new(
        index: usize,
        name: impl Into<String>,
        ty: FxDefinitionParameterType,
        default: Option<FxDefinitionArgumentValue>,
    ) -> Result<Self, FxDefinitionError> {
        let index = u16::try_from(index).map_err(|_| FxDefinitionError::TooManyParameters {
            actual: FX_MAX_PARAMETERS_PER_DEFINITION + 1,
            limit: FX_MAX_PARAMETERS_PER_DEFINITION,
        })?;
        let name = FxDefinitionParameterName::try_new(name)?;
        if let Some(value) = &default
            && value.parameter_type() != ty
        {
            return Err(FxDefinitionError::ParameterDefaultType {
                name: name.as_str().to_owned(),
                expected: ty,
                actual: value.parameter_type(),
            });
        }
        Ok(Self {
            index: FxDefinitionParameterIndex(index),
            name,
            ty,
            default,
        })
    }

    pub const fn index(&self) -> FxDefinitionParameterIndex {
        self.index
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub const fn parameter_type(&self) -> FxDefinitionParameterType {
        self.ty
    }

    pub const fn default(&self) -> Option<&FxDefinitionArgumentValue> {
        self.default.as_ref()
    }

    pub const fn parameter_ref(&self) -> FxDefinitionParameterRef {
        FxDefinitionParameterRef {
            index: self.index,
            ty: self.ty,
        }
    }
}

#[derive(Deserialize)]
struct FxDefinitionParameterWire {
    index: u16,
    name: String,
    ty: FxDefinitionParameterType,
    default: Option<FxDefinitionArgumentValue>,
}

impl<'de> Deserialize<'de> for FxDefinitionParameter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxDefinitionParameterWire::deserialize(deserializer)?;
        Self::try_new(usize::from(wire.index), wire.name, wire.ty, wire.default)
            .map_err(D::Error::custom)
    }
}

impl FxDefinitionParameterLayout {
    fn derive(parameters: &[FxDefinitionParameter]) -> Result<Self, FxDefinitionError> {
        let mut abi_to_storage = Vec::with_capacity(parameters.len());
        let mut runtime = Vec::new();
        let mut static_ = Vec::new();
        for (expected, parameter) in parameters.iter().enumerate() {
            if usize::from(parameter.index.get()) != expected {
                return Err(FxDefinitionError::ParameterIndex {
                    actual: expected,
                    index: parameter.index.get(),
                });
            }
            let reference = parameter.parameter_ref();
            let storage = match parameter.ty {
                FxDefinitionParameterType::Runtime(ty) => {
                    let slot =
                        FxRuntimeParameterSlot(u16::try_from(runtime.len()).map_err(|_| {
                            FxDefinitionError::TooManyParameters {
                                actual: parameters.len(),
                                limit: FX_MAX_PARAMETERS_PER_DEFINITION,
                            }
                        })?);
                    runtime.push(FxRuntimeParameterLayoutRow {
                        slot,
                        parameter: reference,
                        ty,
                    });
                    FxParameterStorageSlot::Runtime(slot)
                }
                FxDefinitionParameterType::Resource | FxDefinitionParameterType::UniformRecord => {
                    let slot =
                        FxStaticParameterSlot(u16::try_from(static_.len()).map_err(|_| {
                            FxDefinitionError::TooManyParameters {
                                actual: parameters.len(),
                                limit: FX_MAX_PARAMETERS_PER_DEFINITION,
                            }
                        })?);
                    static_.push(FxStaticParameterLayoutRow {
                        slot,
                        parameter: reference,
                        ty: parameter.ty,
                    });
                    FxParameterStorageSlot::Static(slot)
                }
            };
            abi_to_storage.push(FxDefinitionParameterLayoutRow {
                parameter: reference,
                storage,
            });
        }
        let abi_count = u16::try_from(abi_to_storage.len()).map_err(|_| {
            FxDefinitionError::TooManyParameters {
                actual: abi_to_storage.len(),
                limit: FX_MAX_PARAMETERS_PER_DEFINITION,
            }
        })?;
        let runtime_count =
            u16::try_from(runtime.len()).map_err(|_| FxDefinitionError::TooManyParameters {
                actual: runtime.len(),
                limit: FX_MAX_PARAMETERS_PER_DEFINITION,
            })?;
        let static_count =
            u16::try_from(static_.len()).map_err(|_| FxDefinitionError::TooManyParameters {
                actual: static_.len(),
                limit: FX_MAX_PARAMETERS_PER_DEFINITION,
            })?;
        let digest = canonical::derive_parameter_layout_digest(
            &abi_to_storage,
            abi_count,
            runtime_count,
            static_count,
        );
        Ok(Self {
            abi_to_storage: abi_to_storage.into_boxed_slice(),
            runtime: runtime.into_boxed_slice(),
            static_: static_.into_boxed_slice(),
            abi_count,
            runtime_count,
            static_count,
            digest,
        })
    }

    pub fn abi_rows(&self) -> &[FxDefinitionParameterLayoutRow] {
        &self.abi_to_storage
    }

    pub fn runtime_rows(&self) -> &[FxRuntimeParameterLayoutRow] {
        &self.runtime
    }

    pub fn static_rows(&self) -> &[FxStaticParameterLayoutRow] {
        &self.static_
    }

    pub const fn digest(&self) -> FxDefinitionParameterLayoutDigest {
        self.digest
    }

    pub const fn abi_count(&self) -> u16 {
        self.abi_count
    }

    pub const fn runtime_count(&self) -> u16 {
        self.runtime_count
    }

    pub const fn static_count(&self) -> u16 {
        self.static_count
    }

    pub fn parameter_ref(
        &self,
        index: FxDefinitionParameterIndex,
    ) -> Option<FxDefinitionParameterRef> {
        self.abi_to_storage
            .get(usize::from(index.get()))
            .map(|row| row.parameter)
    }

    pub fn runtime_ref(&self, index: FxDefinitionParameterIndex) -> Option<FxRuntimeParameterRef> {
        let row = self.abi_to_storage.get(usize::from(index.get()))?;
        let FxParameterStorageSlot::Runtime(slot) = row.storage else {
            return None;
        };
        self.runtime
            .get(usize::from(slot.get()))
            .map(|row| FxRuntimeParameterRef {
                slot: row.slot,
                ty: row.ty,
            })
    }

    pub fn matches_runtime_schema(&self, types: &[FxRuntimeType]) -> bool {
        self.runtime.len() == types.len()
            && self
                .runtime
                .iter()
                .zip(types)
                .all(|(row, ty)| row.ty == *ty)
    }

    fn from_rows(
        abi_to_storage: Box<[FxDefinitionParameterLayoutRow]>,
        runtime: Box<[FxRuntimeParameterLayoutRow]>,
        static_: Box<[FxStaticParameterLayoutRow]>,
        digest: FxDefinitionParameterLayoutDigest,
    ) -> Result<Self, FxDefinitionError> {
        if abi_to_storage.len() > FX_MAX_PARAMETERS_PER_DEFINITION
            || runtime.len() > FX_MAX_PARAMETERS_PER_DEFINITION
            || static_.len() > FX_MAX_PARAMETERS_PER_DEFINITION
        {
            return Err(FxDefinitionError::ParameterLayoutMismatch);
        }
        for (index, row) in abi_to_storage.iter().enumerate() {
            if usize::from(row.parameter.index.get()) != index {
                return Err(FxDefinitionError::ParameterLayoutMismatch);
            }
            match row.storage {
                FxParameterStorageSlot::Runtime(slot) => {
                    let Some(reverse) = runtime.get(usize::from(slot.get())) else {
                        return Err(FxDefinitionError::ParameterLayoutMismatch);
                    };
                    if reverse.slot != slot
                        || reverse.parameter != row.parameter
                        || row.parameter.ty != FxDefinitionParameterType::Runtime(reverse.ty)
                    {
                        return Err(FxDefinitionError::ParameterLayoutMismatch);
                    }
                }
                FxParameterStorageSlot::Static(slot) => {
                    let Some(reverse) = static_.get(usize::from(slot.get())) else {
                        return Err(FxDefinitionError::ParameterLayoutMismatch);
                    };
                    if reverse.slot != slot
                        || reverse.parameter != row.parameter
                        || reverse.ty != row.parameter.ty
                        || matches!(reverse.ty, FxDefinitionParameterType::Runtime(_))
                    {
                        return Err(FxDefinitionError::ParameterLayoutMismatch);
                    }
                }
            }
        }
        for (slot, row) in runtime.iter().enumerate() {
            if usize::from(row.slot.get()) != slot {
                return Err(FxDefinitionError::ParameterLayoutMismatch);
            }
        }
        for (slot, row) in static_.iter().enumerate() {
            if usize::from(row.slot.get()) != slot {
                return Err(FxDefinitionError::ParameterLayoutMismatch);
            }
        }
        let abi_count = u16::try_from(abi_to_storage.len())
            .map_err(|_| FxDefinitionError::ParameterLayoutMismatch)?;
        let runtime_count =
            u16::try_from(runtime.len()).map_err(|_| FxDefinitionError::ParameterLayoutMismatch)?;
        let static_count =
            u16::try_from(static_.len()).map_err(|_| FxDefinitionError::ParameterLayoutMismatch)?;
        if runtime.len().checked_add(static_.len()) != Some(abi_to_storage.len())
            || digest
                != canonical::derive_parameter_layout_digest(
                    &abi_to_storage,
                    abi_count,
                    runtime_count,
                    static_count,
                )
        {
            return Err(FxDefinitionError::ParameterLayoutMismatch);
        }
        Ok(Self {
            abi_to_storage,
            runtime,
            static_,
            abi_count,
            runtime_count,
            static_count,
            digest,
        })
    }
}

#[derive(Deserialize)]
struct FxDefinitionParameterLayoutWire {
    abi_to_storage: Box<[FxDefinitionParameterLayoutRow]>,
    runtime: Box<[FxRuntimeParameterLayoutRow]>,
    static_: Box<[FxStaticParameterLayoutRow]>,
    digest: FxDefinitionParameterLayoutDigest,
}

impl<'de> Deserialize<'de> for FxDefinitionParameterLayout {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxDefinitionParameterLayoutWire::deserialize(deserializer)?;
        Self::from_rows(wire.abi_to_storage, wire.runtime, wire.static_, wire.digest)
            .map_err(D::Error::custom)
    }
}

impl FxDefinitionParameterLayoutRow {
    pub const fn parameter(self) -> FxDefinitionParameterRef {
        self.parameter
    }

    pub const fn storage(self) -> FxParameterStorageSlot {
        self.storage
    }
}

impl FxRuntimeParameterLayoutRow {
    pub const fn reference(self) -> FxRuntimeParameterRef {
        FxRuntimeParameterRef {
            slot: self.slot,
            ty: self.ty,
        }
    }

    pub const fn parameter(self) -> FxDefinitionParameterRef {
        self.parameter
    }
}

impl FxStaticParameterLayoutRow {
    pub const fn slot(self) -> FxStaticParameterSlot {
        self.slot
    }

    pub const fn parameter(self) -> FxDefinitionParameterRef {
        self.parameter
    }
}

impl FxNode {
    pub const fn node_kind(&self) -> FxNodeKind {
        match self {
            Self::Style { .. } => FxNodeKind::Style,
            Self::Text { .. } => FxNodeKind::Text,
            Self::Color { .. } => FxNodeKind::Color,
            Self::Transform { .. } => FxNodeKind::Transform,
            Self::Mask { .. } => FxNodeKind::Mask,
            Self::Filter { .. } => FxNodeKind::Filter,
            Self::Shader { .. } => FxNodeKind::Shader,
            Self::OffscreenPass { .. } => FxNodeKind::OffscreenPass,
            Self::PostProcess { .. } => FxNodeKind::PostProcess,
            Self::Transition { .. } => FxNodeKind::Transition,
            Self::Conditional { .. } => FxNodeKind::Conditional,
            Self::Stack { .. } => FxNodeKind::Stack,
        }
    }

    pub const fn renderer_interface(&self) -> Option<FxRendererInterface> {
        self.node_kind().renderer_interface()
    }

    pub fn properties(&self) -> Option<&[FxProperty]> {
        match self {
            Self::Style { properties }
            | Self::Text { properties }
            | Self::Color { properties }
            | Self::Transform { properties, .. }
            | Self::Mask { properties, .. }
            | Self::Filter { properties, .. }
            | Self::Shader { properties, .. }
            | Self::OffscreenPass { properties, .. }
            | Self::PostProcess { properties, .. }
            | Self::Transition { properties, .. } => Some(properties),
            Self::Conditional { .. } | Self::Stack { .. } => None,
        }
    }

    fn properties_mut(&mut self) -> Option<&mut Vec<FxProperty>> {
        match self {
            Self::Style { properties }
            | Self::Text { properties }
            | Self::Color { properties }
            | Self::Transform { properties, .. }
            | Self::Mask { properties, .. }
            | Self::Filter { properties, .. }
            | Self::Shader { properties, .. }
            | Self::OffscreenPass { properties, .. }
            | Self::PostProcess { properties, .. }
            | Self::Transition { properties, .. } => Some(properties),
            Self::Conditional { .. } | Self::Stack { .. } => None,
        }
    }

    fn kind_name(&self) -> &'static str {
        self.node_kind().as_str()
    }
}

impl FxGraph {
    pub fn try_new(mut nodes: Vec<FxNode>) -> Result<Self, FxGraphError> {
        for node in &mut nodes {
            if let Some(properties) = node.properties_mut() {
                properties.sort_by_key(|property| property.id);
            }
        }
        let node_count = u16::try_from(nodes.len()).map_err(|_| FxGraphError::TooManyNodes {
            actual: nodes.len(),
            limit: FX_MAX_GRAPH_NODES_PER_DEFINITION,
        })?;
        let graph = Self { nodes, node_count };
        graph.validate()?;
        Ok(graph)
    }

    pub fn nodes(&self) -> &[FxNode] {
        &self.nodes
    }

    pub const fn node_count(&self) -> u16 {
        self.node_count
    }

    /// Returns the total number of nodes in this graph and all nested child graphs.
    ///
    /// Construction and deserialization enforce the per-definition node limit, so
    /// this count is exact and cannot overflow.
    pub fn total_node_count(&self) -> usize {
        self.nodes
            .iter()
            .map(|node| {
                let child_nodes = match node {
                    FxNode::Conditional {
                        then_graph,
                        else_graph,
                        ..
                    } => then_graph.total_node_count() + else_graph.total_node_count(),
                    FxNode::Stack { children } => children.iter().map(Self::total_node_count).sum(),
                    _ => 0,
                };
                1 + child_nodes
            })
            .sum()
    }

    pub fn validate(&self) -> Result<(), FxGraphError> {
        let (nodes, depth, child_edges) = validate_graph(self, 1)?;
        if nodes > FX_MAX_GRAPH_NODES_PER_DEFINITION {
            return Err(FxGraphError::TooManyNodes {
                actual: nodes,
                limit: FX_MAX_GRAPH_NODES_PER_DEFINITION,
            });
        }
        if depth > FX_MAX_GRAPH_DEPTH {
            return Err(FxGraphError::TooDeep {
                actual: depth,
                limit: FX_MAX_GRAPH_DEPTH,
            });
        }
        if child_edges > FX_MAX_GRAPH_CHILD_EDGES_PER_DEFINITION {
            return Err(FxGraphError::TooManyChildEdges {
                actual: child_edges,
                limit: FX_MAX_GRAPH_CHILD_EDGES_PER_DEFINITION,
            });
        }
        Ok(())
    }

    pub fn renderer_interfaces(&self) -> FxRendererInterfaceSet {
        let mut interfaces = FxRendererInterfaceSet::default();
        collect_interfaces(self, &mut interfaces);
        interfaces
    }
}

#[derive(Deserialize)]
struct FxGraphWire {
    nodes: Vec<FxNode>,
}

impl<'de> Deserialize<'de> for FxGraph {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxGraphWire::deserialize(deserializer)?;
        Self::try_new(wire.nodes).map_err(D::Error::custom)
    }
}

#[derive(Deserialize)]
struct FxDefinitionWire {
    id: FxId,
    parameters: Vec<FxDefinitionParameter>,
    layout: FxDefinitionParameterLayout,
    graph: FxGraph,
    abi_hash: FxAbiHash,
    semantic_hash: FxSemanticHash,
}

#[derive(Deserialize)]
struct FxDefinitionParameterSchemaWire {
    id: FxId,
    parameters: Vec<FxDefinitionParameter>,
    layout: FxDefinitionParameterLayout,
    digest: FxDefinitionParameterSchemaDigest,
}

impl<'de> Deserialize<'de> for FxDefinition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxDefinitionWire::deserialize(deserializer)?;
        Self::from_parts(
            wire.id,
            wire.parameters,
            wire.layout,
            wire.graph,
            wire.abi_hash,
            wire.semantic_hash,
        )
        .map_err(D::Error::custom)
    }
}

impl<'de> Deserialize<'de> for FxDefinitionParameterSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxDefinitionParameterSchemaWire::deserialize(deserializer)?;
        let schema = Self::new(wire.id, wire.parameters).map_err(D::Error::custom)?;
        if schema.layout != wire.layout {
            return Err(D::Error::custom(FxDefinitionError::ParameterLayoutMismatch));
        }
        if schema.digest != wire.digest {
            return Err(D::Error::custom(
                "Fx definition parameter-schema digest mismatch",
            ));
        }
        Ok(schema)
    }
}

fn validate_definition_parameters(
    parameters: &[FxDefinitionParameter],
) -> Result<FxDefinitionParameterLayout, FxDefinitionError> {
    if parameters.len() > FX_MAX_PARAMETERS_PER_DEFINITION {
        return Err(FxDefinitionError::TooManyParameters {
            actual: parameters.len(),
            limit: FX_MAX_PARAMETERS_PER_DEFINITION,
        });
    }
    for (expected_index, parameter) in parameters.iter().enumerate() {
        if usize::from(parameter.index.get()) != expected_index {
            return Err(FxDefinitionError::ParameterIndex {
                actual: expected_index,
                index: parameter.index.get(),
            });
        }
        if parameters[..expected_index]
            .iter()
            .any(|prior| prior.name == parameter.name)
        {
            return Err(FxDefinitionError::DuplicateParameter {
                name: parameter.name.as_str().to_owned(),
            });
        }
    }
    FxDefinitionParameterLayout::derive(parameters)
}

fn validate_definition_graph(
    graph: &FxGraph,
    layout: &FxDefinitionParameterLayout,
) -> Result<(), FxDefinitionError> {
    graph.validate()?;
    validate_graph_parameter_references(graph, layout)?;
    Ok(())
}

fn validate_graph_parameter_references(
    graph: &FxGraph,
    layout: &FxDefinitionParameterLayout,
) -> Result<(), FxDefinitionError> {
    for node in &graph.nodes {
        if let Some(properties) = node.properties() {
            for property in properties {
                validate_value_parameter_references(&property.value, layout)?;
            }
        }
        match node {
            FxNode::Conditional {
                condition,
                then_graph,
                else_graph,
            } => {
                validate_value_parameter_references(condition, layout)?;
                validate_graph_parameter_references(then_graph, layout)?;
                validate_graph_parameter_references(else_graph, layout)?;
            }
            FxNode::Stack { children } => {
                for child in children {
                    validate_graph_parameter_references(child, layout)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_value_parameter_references(
    value: &FxStaticValue,
    layout: &FxDefinitionParameterLayout,
) -> Result<(), FxDefinitionError> {
    match value {
        FxStaticValue::Parameter(slot) => {
            let Some(expected) = layout.parameter_ref(slot.index) else {
                return Err(FxDefinitionError::ParameterReferenceOutOfBounds {
                    index: slot.index.get(),
                    available: layout.abi_rows().len(),
                });
            };
            if expected.ty != slot.ty {
                return Err(FxDefinitionError::ParameterReferenceType {
                    index: slot.index.get(),
                    expected: expected.ty,
                    actual: slot.ty,
                });
            }
        }
        FxStaticValue::Sampler(program)
            if !layout.matches_runtime_schema(program.program().schema().parameter_types()) =>
        {
            return Err(FxDefinitionError::SamplerParameterSchema);
        }
        FxStaticValue::UniformRecord(record) => record
            .validate_definition_layout(layout)
            .map_err(|_| FxDefinitionError::SamplerParameterSchema)?,
        FxStaticValue::Runtime(_)
        | FxStaticValue::Resource(_)
        | FxStaticValue::Selector(_)
        | FxStaticValue::ShaderStage(_)
        | FxStaticValue::FontFamily(_)
        | FxStaticValue::Target(_)
        | FxStaticValue::Phase(_)
        | FxStaticValue::Sampler(_) => {}
    }
    Ok(())
}

fn validate_graph(graph: &FxGraph, depth: usize) -> Result<(usize, usize, usize), FxGraphError> {
    if depth > FX_MAX_GRAPH_DEPTH {
        return Err(FxGraphError::TooDeep {
            actual: depth,
            limit: FX_MAX_GRAPH_DEPTH,
        });
    }
    let mut nodes = graph.nodes.len();
    let mut maximum_depth = depth;
    let mut child_edges = 0_usize;
    for node in &graph.nodes {
        validate_node(node)?;
        match node {
            FxNode::Conditional {
                then_graph,
                else_graph,
                ..
            } => {
                child_edges =
                    child_edges
                        .checked_add(2)
                        .ok_or(FxGraphError::TooManyChildEdges {
                            actual: usize::MAX,
                            limit: FX_MAX_GRAPH_CHILD_EDGES_PER_DEFINITION,
                        })?;
                for child in [then_graph, else_graph] {
                    let (child_nodes, child_depth, child_child_edges) =
                        validate_graph(child, depth + 1)?;
                    nodes = nodes
                        .checked_add(child_nodes)
                        .ok_or(FxGraphError::TooManyNodes {
                            actual: usize::MAX,
                            limit: FX_MAX_GRAPH_NODES_PER_DEFINITION,
                        })?;
                    maximum_depth = maximum_depth.max(child_depth);
                    child_edges = child_edges.checked_add(child_child_edges).ok_or(
                        FxGraphError::TooManyChildEdges {
                            actual: usize::MAX,
                            limit: FX_MAX_GRAPH_CHILD_EDGES_PER_DEFINITION,
                        },
                    )?;
                }
            }
            FxNode::Stack { children } => {
                child_edges = child_edges.checked_add(children.len()).ok_or(
                    FxGraphError::TooManyChildEdges {
                        actual: usize::MAX,
                        limit: FX_MAX_GRAPH_CHILD_EDGES_PER_DEFINITION,
                    },
                )?;
                for child in children {
                    let (child_nodes, child_depth, child_child_edges) =
                        validate_graph(child, depth + 1)?;
                    nodes = nodes
                        .checked_add(child_nodes)
                        .ok_or(FxGraphError::TooManyNodes {
                            actual: usize::MAX,
                            limit: FX_MAX_GRAPH_NODES_PER_DEFINITION,
                        })?;
                    maximum_depth = maximum_depth.max(child_depth);
                    child_edges = child_edges.checked_add(child_child_edges).ok_or(
                        FxGraphError::TooManyChildEdges {
                            actual: usize::MAX,
                            limit: FX_MAX_GRAPH_CHILD_EDGES_PER_DEFINITION,
                        },
                    )?;
                }
            }
            _ => {}
        }
        if nodes > FX_MAX_GRAPH_NODES_PER_DEFINITION {
            return Err(FxGraphError::TooManyNodes {
                actual: nodes,
                limit: FX_MAX_GRAPH_NODES_PER_DEFINITION,
            });
        }
        if child_edges > FX_MAX_GRAPH_CHILD_EDGES_PER_DEFINITION {
            return Err(FxGraphError::TooManyChildEdges {
                actual: child_edges,
                limit: FX_MAX_GRAPH_CHILD_EDGES_PER_DEFINITION,
            });
        }
    }
    Ok((nodes, maximum_depth, child_edges))
}

fn validate_node(node: &FxNode) -> Result<(), FxGraphError> {
    if let FxNode::Conditional { condition, .. } = node
        && condition.runtime_type() != Some(FxRuntimeType::Bool)
    {
        return Err(FxGraphError::InvalidCondition);
    }
    let Some(properties) = node.properties() else {
        return Ok(());
    };
    let capacity = node.node_kind().property_capacity();
    if properties.len() > capacity {
        return Err(FxGraphError::TooManyProperties {
            node: node.kind_name(),
            actual: properties.len(),
            limit: capacity,
        });
    }
    if let Some(pair) = properties.windows(2).find(|pair| pair[0].id == pair[1].id) {
        return Err(FxGraphError::DuplicateProperty {
            node: node.kind_name(),
            property: pair[0].name().to_owned(),
        });
    }
    for property in properties {
        if !node.node_kind().accepts_property(property.id) {
            return Err(FxGraphError::UnknownProperty {
                node: node.kind_name(),
                property: property.name().to_owned(),
            });
        }
        let expected = property.id.value_type();
        if !expected.accepts(&property.value) {
            return Err(FxGraphError::PropertyTypeMismatch {
                node: node.kind_name(),
                property: property.name().to_owned(),
                expected: expected.as_str(),
                actual: property.value.kind_name(),
            });
        }
    }
    Ok(())
}

fn collect_interfaces(graph: &FxGraph, interfaces: &mut FxRendererInterfaceSet) {
    for node in &graph.nodes {
        if let Some(interface) = node.renderer_interface() {
            interfaces.insert(interface);
        }
        match node {
            FxNode::Conditional {
                then_graph,
                else_graph,
                ..
            } => {
                collect_interfaces(then_graph, interfaces);
                collect_interfaces(else_graph, interfaces);
            }
            FxNode::Stack { children } => {
                for child in children {
                    collect_interfaces(child, interfaces);
                }
            }
            _ => {}
        }
    }
}

fn valid_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

const fn runtime_type_name(value: FxRuntimeType) -> &'static str {
    match value {
        FxRuntimeType::Bool => "bool",
        FxRuntimeType::I32 => "i32",
        FxRuntimeType::F32 => "f32",
        FxRuntimeType::Length => "length",
        FxRuntimeType::Angle => "angle",
        FxRuntimeType::Seconds => "seconds",
        FxRuntimeType::Color => "color",
        FxRuntimeType::Vec2 => "vec2",
        FxRuntimeType::Transform2D => "transform_2d",
        FxRuntimeType::U32 => "u32",
    }
}
