//! Typed static Fx graphs, renderer interfaces, targets, and capability contracts.

use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use super::{
    capability::{FxPhase, FxRendererInterface, FxRendererInterfaceSet, FxTarget},
    identity::{FxAbiHash, FxId, FxSemanticHash, hash_str, hash_usize},
    program::FxSamplerProgram,
    value::{FxRuntimeType, FxRuntimeValue, hash_runtime_value},
};

pub const FX_MAX_DEFINITIONS_PER_SECTION: usize = 4_096;
pub const FX_MAX_PARAMETERS_PER_DEFINITION: usize = 64;
pub const FX_MAX_GRAPH_NODES_PER_DEFINITION: usize = 4_096;
pub const FX_MAX_GRAPH_DEPTH: usize = 64;
pub const FX_MAX_TOTAL_GRAPH_NODES_PER_SECTION: usize = 65_536;

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

/// Closed static property expectation returned to compiler lowering.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", content = "runtime_type", rename_all = "snake_case")]
pub enum FxStaticType {
    Runtime(FxRuntimeType),
    Resource,
    Selector,
    String,
    Target,
    Phase,
    List,
    Record,
}

/// Validated resource identity retained as static graph data.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FxResourceId(String);

/// Typed reference to a definition parameter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FxParameterSlot {
    pub index: u16,
    pub ty: FxRuntimeType,
}

/// Static graph value. Executable arithmetic is represented only by typed programs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FxStaticValue {
    Runtime(FxRuntimeValue),
    Resource(FxResourceId),
    Selector(String),
    String(String),
    Target(FxTarget),
    Phase(FxPhase),
    Parameter(FxParameterSlot),
    Sampler(FxSamplerProgram),
    List(Vec<FxStaticValue>),
    Record(Vec<FxProperty>),
}

/// Named, typed input owned by one graph node.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FxProperty {
    name: String,
    value: FxStaticValue,
}

/// Closed parameter schema exported by one Fx definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FxParameter {
    name: String,
    ty: FxRuntimeType,
    default: Option<FxRuntimeValue>,
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
}

/// One complete compiled `#[fx]` declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FxDefinition {
    id: FxId,
    parameters: Vec<FxParameter>,
    graph: FxGraph,
    abi_hash: FxAbiHash,
    semantic_hash: FxSemanticHash,
}

/// Invalid graph structure or closed constructor property.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxGraphError {
    #[error("graph has {actual} expanded nodes, exceeding the limit of {limit}")]
    TooManyNodes { actual: usize, limit: usize },
    #[error("graph depth {actual} exceeds the limit of {limit}")]
    TooDeep { actual: usize, limit: usize },
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
    #[error("invalid Fx parameter name `{name}`")]
    InvalidParameterName { name: String },
    #[error("duplicate Fx parameter `{name}`")]
    DuplicateParameter { name: String },
    #[error("default for Fx parameter `{name}` has type {actual:?}, expected {expected:?}")]
    ParameterDefaultType {
        name: String,
        expected: FxRuntimeType,
        actual: FxRuntimeType,
    },
    #[error("Fx graph references parameter slot {slot}, but only {available} parameters exist")]
    ParameterSlotOutOfBounds { slot: u16, available: usize },
    #[error("Fx graph parameter slot {slot} declares {actual:?}, expected {expected:?}")]
    ParameterSlotType {
        slot: u16,
        expected: FxRuntimeType,
        actual: FxRuntimeType,
    },
    #[error("Fx sampler parameter schema does not match its definition parameters")]
    SamplerParameterSchema,
    #[error(transparent)]
    Graph(#[from] FxGraphError),
    #[error("stored Fx ABI hash does not match the typed contract")]
    AbiHashMismatch,
    #[error("stored Fx semantic hash does not match the typed graph")]
    SemanticHashMismatch,
}

impl FxNodeKind {
    /// Returns the single presentation-owned property expectation for lowering.
    ///
    /// `target` and `phase` are shared by every property-bearing treatment
    /// constructor. Conditional and stack use their fixed typed fields instead.
    pub fn property_type(self, name: &str) -> Option<FxStaticType> {
        use FxRuntimeType as Runtime;
        use FxStaticType as Static;

        if matches!(self, Self::Conditional | Self::Stack) {
            return None;
        }
        if name == "target" {
            return Some(Static::Target);
        }
        if name == "phase" {
            return Some(Static::Phase);
        }
        match self {
            Self::Style | Self::Text => match name {
                "opacity" => Some(Static::Runtime(Runtime::F32)),
                "weight" => Some(Static::Runtime(Runtime::I32)),
                "slant" => Some(Static::Runtime(Runtime::Angle)),
                "font_family" => Some(Static::String),
                "size" | "spacing" => Some(Static::Runtime(Runtime::Length)),
                "color" => Some(Static::Runtime(Runtime::Color)),
                _ => None,
            },
            Self::Color => match name {
                "tint" | "multiply" => Some(Static::Runtime(Runtime::Color)),
                "opacity" => Some(Static::Runtime(Runtime::F32)),
                _ => None,
            },
            Self::Transform => match name {
                "transform" | "sampler" => Some(Static::Runtime(Runtime::Transform2D)),
                _ => None,
            },
            Self::Mask => match name {
                "resource" => Some(Static::Resource),
                "coverage" => Some(Static::Runtime(Runtime::F32)),
                "invert" => Some(Static::Runtime(Runtime::Bool)),
                _ => None,
            },
            Self::Filter => match name {
                "blur_radius" => Some(Static::Runtime(Runtime::Length)),
                "brightness" | "contrast" | "saturation" => Some(Static::Runtime(Runtime::F32)),
                _ => None,
            },
            Self::Shader => match name {
                "resource" => Some(Static::Resource),
                "stage" => Some(Static::Selector),
                "uniforms" => Some(Static::Record),
                _ => None,
            },
            Self::OffscreenPass | Self::PostProcess => match name {
                "resource" => Some(Static::Resource),
                _ => None,
            },
            Self::Transition => match name {
                "kind" | "easing" => Some(Static::Selector),
                "duration" => Some(Static::Runtime(Runtime::Seconds)),
                "progress" => Some(Static::Runtime(Runtime::F32)),
                _ => None,
            },
            Self::Conditional | Self::Stack => None,
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

impl FxResourceId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.is_empty()
            || !value.chars().all(|character| {
                character.is_alphanumeric() || matches!(character, '_' | '-' | '.' | '/')
            })
        {
            Err("Fx resource ID must be non-empty and canonical".to_owned())
        } else {
            Ok(Self(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
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

impl FxStaticValue {
    pub fn runtime_type(&self) -> Option<FxRuntimeType> {
        match self {
            Self::Runtime(value) => Some(value.value_type()),
            Self::Parameter(slot) => Some(slot.ty),
            Self::Sampler(program) => Some(program.return_type()),
            Self::Resource(_)
            | Self::Selector(_)
            | Self::String(_)
            | Self::Target(_)
            | Self::Phase(_)
            | Self::List(_)
            | Self::Record(_) => None,
        }
    }

    pub fn static_type(&self) -> FxStaticType {
        match self {
            Self::Runtime(value) => FxStaticType::Runtime(value.value_type()),
            Self::Resource(_) => FxStaticType::Resource,
            Self::Selector(_) => FxStaticType::Selector,
            Self::String(_) => FxStaticType::String,
            Self::Target(_) => FxStaticType::Target,
            Self::Phase(_) => FxStaticType::Phase,
            Self::Parameter(slot) => FxStaticType::Runtime(slot.ty),
            Self::Sampler(program) => FxStaticType::Runtime(program.return_type()),
            Self::List(_) => FxStaticType::List,
            Self::Record(_) => FxStaticType::Record,
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
            Self::Selector => "selector",
            Self::String => "string",
            Self::Target => "target",
            Self::Phase => "phase",
            Self::List => "list",
            Self::Record => "record",
        }
    }
}

impl From<FxRuntimeValue> for FxStaticValue {
    fn from(value: FxRuntimeValue) -> Self {
        Self::Runtime(value)
    }
}

impl FxProperty {
    pub fn new(name: impl Into<String>, value: FxStaticValue) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn value(&self) -> &FxStaticValue {
        &self.value
    }
}

impl FxParameter {
    pub fn try_new(
        name: impl Into<String>,
        ty: FxRuntimeType,
        default: Option<FxRuntimeValue>,
    ) -> Result<Self, FxDefinitionError> {
        let name = name.into();
        if !valid_identifier(&name) {
            return Err(FxDefinitionError::InvalidParameterName { name });
        }
        if let Some(value) = &default
            && value.value_type() != ty
        {
            return Err(FxDefinitionError::ParameterDefaultType {
                name,
                expected: ty,
                actual: value.value_type(),
            });
        }
        Ok(Self { name, ty, default })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn value_type(&self) -> FxRuntimeType {
        self.ty
    }

    pub const fn default(&self) -> Option<&FxRuntimeValue> {
        self.default.as_ref()
    }
}

#[derive(Deserialize)]
struct FxParameterWire {
    name: String,
    ty: FxRuntimeType,
    default: Option<FxRuntimeValue>,
}

impl<'de> Deserialize<'de> for FxParameter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxParameterWire::deserialize(deserializer)?;
        Self::try_new(wire.name, wire.ty, wire.default).map_err(D::Error::custom)
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

    fn kind_name(&self) -> &'static str {
        self.node_kind().as_str()
    }
}

impl FxGraph {
    pub fn new(nodes: Vec<FxNode>) -> Self {
        Self { nodes }
    }

    pub fn try_new(nodes: Vec<FxNode>) -> Result<Self, FxGraphError> {
        let graph = Self { nodes };
        graph.validate()?;
        Ok(graph)
    }

    pub fn nodes(&self) -> &[FxNode] {
        &self.nodes
    }

    pub fn validate(&self) -> Result<(), FxGraphError> {
        let (nodes, depth) = validate_graph(self, 1)?;
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

impl FxDefinition {
    /// Validates a definition and derives both hashes from its typed contract.
    pub fn new(
        id: FxId,
        parameters: Vec<FxParameter>,
        graph: FxGraph,
    ) -> Result<Self, FxDefinitionError> {
        validate_definition(&parameters, &graph)?;
        let abi_hash = FxAbiHash::for_definition(&parameters, &graph);
        let semantic_hash = FxSemanticHash::for_graph(&graph);
        Ok(Self {
            id,
            parameters,
            graph,
            abi_hash,
            semantic_hash,
        })
    }

    /// Validates a decoded definition and rejects tampered stored hashes.
    pub fn from_parts(
        id: FxId,
        parameters: Vec<FxParameter>,
        graph: FxGraph,
        abi_hash: FxAbiHash,
        semantic_hash: FxSemanticHash,
    ) -> Result<Self, FxDefinitionError> {
        validate_definition(&parameters, &graph)?;
        if abi_hash != FxAbiHash::for_definition(&parameters, &graph) {
            return Err(FxDefinitionError::AbiHashMismatch);
        }
        if semantic_hash != FxSemanticHash::for_graph(&graph) {
            return Err(FxDefinitionError::SemanticHashMismatch);
        }
        Ok(Self {
            id,
            parameters,
            graph,
            abi_hash,
            semantic_hash,
        })
    }

    pub const fn id(&self) -> &FxId {
        &self.id
    }

    pub fn parameters(&self) -> &[FxParameter] {
        &self.parameters
    }

    pub const fn graph(&self) -> &FxGraph {
        &self.graph
    }

    pub const fn abi_hash(&self) -> FxAbiHash {
        self.abi_hash
    }

    pub const fn semantic_hash(&self) -> FxSemanticHash {
        self.semantic_hash
    }
}

#[derive(Deserialize)]
struct FxDefinitionWire {
    id: FxId,
    parameters: Vec<FxParameter>,
    graph: FxGraph,
    abi_hash: FxAbiHash,
    semantic_hash: FxSemanticHash,
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
            wire.graph,
            wire.abi_hash,
            wire.semantic_hash,
        )
        .map_err(D::Error::custom)
    }
}

impl FxAbiHash {
    pub fn for_definition(parameters: &[FxParameter], graph: &FxGraph) -> Self {
        let mut hasher = blake3::Hasher::new();
        hash_str(&mut hasher, "arcweft.fx-abi.v1");
        let mut parameters = parameters.iter().collect::<Vec<_>>();
        parameters.sort_by(|left, right| left.name.cmp(&right.name));
        hash_usize(&mut hasher, parameters.len());
        for parameter in parameters {
            hash_str(&mut hasher, &parameter.name);
            hasher.update(&[parameter.ty as u8]);
            match &parameter.default {
                Some(value) => {
                    hasher.update(&[1]);
                    hash_runtime_value(&mut hasher, value);
                }
                None => {
                    hasher.update(&[0]);
                }
            }
        }
        let interfaces = graph.renderer_interfaces();
        hash_usize(&mut hasher, interfaces.iter().len());
        for interface in interfaces.iter() {
            hasher.update(&[interface as u8]);
        }
        let mut schemas = Vec::new();
        collect_property_schemas(graph, &mut schemas);
        schemas.sort_unstable();
        hash_usize(&mut hasher, schemas.len());
        for (interface, name, kind) in schemas {
            hasher.update(&[interface as u8]);
            hash_str(&mut hasher, name);
            hash_str(&mut hasher, kind);
        }
        Self::from_bytes(*hasher.finalize().as_bytes())
    }
}

impl FxSemanticHash {
    pub fn for_graph(graph: &FxGraph) -> Self {
        let mut hasher = blake3::Hasher::new();
        hash_str(&mut hasher, "arcweft.fx-semantic.v1");
        hash_graph(&mut hasher, graph);
        Self::from_bytes(*hasher.finalize().as_bytes())
    }
}

fn validate_definition(
    parameters: &[FxParameter],
    graph: &FxGraph,
) -> Result<(), FxDefinitionError> {
    if parameters.len() > FX_MAX_PARAMETERS_PER_DEFINITION {
        return Err(FxDefinitionError::TooManyParameters {
            actual: parameters.len(),
            limit: FX_MAX_PARAMETERS_PER_DEFINITION,
        });
    }
    let mut names = BTreeSet::new();
    for parameter in parameters {
        if !names.insert(parameter.name.as_str()) {
            return Err(FxDefinitionError::DuplicateParameter {
                name: parameter.name.clone(),
            });
        }
    }
    graph.validate()?;
    let parameter_types = parameters
        .iter()
        .map(|parameter| parameter.ty)
        .collect::<Vec<_>>();
    validate_graph_parameter_references(graph, &parameter_types)?;
    Ok(())
}

fn validate_graph_parameter_references(
    graph: &FxGraph,
    parameter_types: &[FxRuntimeType],
) -> Result<(), FxDefinitionError> {
    for node in &graph.nodes {
        if let Some(properties) = node.properties() {
            for property in properties {
                validate_value_parameter_references(&property.value, parameter_types)?;
            }
        }
        match node {
            FxNode::Conditional {
                condition,
                then_graph,
                else_graph,
            } => {
                validate_value_parameter_references(condition, parameter_types)?;
                validate_graph_parameter_references(then_graph, parameter_types)?;
                validate_graph_parameter_references(else_graph, parameter_types)?;
            }
            FxNode::Stack { children } => {
                for child in children {
                    validate_graph_parameter_references(child, parameter_types)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_value_parameter_references(
    value: &FxStaticValue,
    parameter_types: &[FxRuntimeType],
) -> Result<(), FxDefinitionError> {
    match value {
        FxStaticValue::Parameter(slot) => {
            let Some(expected) = parameter_types.get(usize::from(slot.index)).copied() else {
                return Err(FxDefinitionError::ParameterSlotOutOfBounds {
                    slot: slot.index,
                    available: parameter_types.len(),
                });
            };
            if expected != slot.ty {
                return Err(FxDefinitionError::ParameterSlotType {
                    slot: slot.index,
                    expected,
                    actual: slot.ty,
                });
            }
        }
        FxStaticValue::Sampler(program)
            if program.program().schema().parameter_types() != parameter_types =>
        {
            return Err(FxDefinitionError::SamplerParameterSchema);
        }
        FxStaticValue::List(values) => {
            for value in values {
                validate_value_parameter_references(value, parameter_types)?;
            }
        }
        FxStaticValue::Record(properties) => {
            for property in properties {
                validate_value_parameter_references(&property.value, parameter_types)?;
            }
        }
        FxStaticValue::Runtime(_)
        | FxStaticValue::Resource(_)
        | FxStaticValue::Selector(_)
        | FxStaticValue::String(_)
        | FxStaticValue::Target(_)
        | FxStaticValue::Phase(_)
        | FxStaticValue::Sampler(_) => {}
    }
    Ok(())
}

fn validate_graph(graph: &FxGraph, depth: usize) -> Result<(usize, usize), FxGraphError> {
    if depth > FX_MAX_GRAPH_DEPTH {
        return Err(FxGraphError::TooDeep {
            actual: depth,
            limit: FX_MAX_GRAPH_DEPTH,
        });
    }
    let mut nodes = graph.nodes.len();
    let mut maximum_depth = depth;
    for node in &graph.nodes {
        validate_node(node)?;
        match node {
            FxNode::Conditional {
                then_graph,
                else_graph,
                ..
            } => {
                for child in [then_graph, else_graph] {
                    let (child_nodes, child_depth) = validate_graph(child, depth + 1)?;
                    nodes = nodes.saturating_add(child_nodes);
                    maximum_depth = maximum_depth.max(child_depth);
                }
            }
            FxNode::Stack { children } => {
                for child in children {
                    let (child_nodes, child_depth) = validate_graph(child, depth + 1)?;
                    nodes = nodes.saturating_add(child_nodes);
                    maximum_depth = maximum_depth.max(child_depth);
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
    }
    Ok((nodes, maximum_depth))
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
    let mut names = BTreeSet::new();
    for property in properties {
        if !names.insert(property.name.as_str()) {
            return Err(FxGraphError::DuplicateProperty {
                node: node.kind_name(),
                property: property.name.clone(),
            });
        }
        let Some(expected) = node.node_kind().property_type(&property.name) else {
            return Err(FxGraphError::UnknownProperty {
                node: node.kind_name(),
                property: property.name.clone(),
            });
        };
        if !expected.accepts(&property.value) {
            return Err(FxGraphError::PropertyTypeMismatch {
                node: node.kind_name(),
                property: property.name.clone(),
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

fn collect_property_schemas<'a>(
    graph: &'a FxGraph,
    output: &mut Vec<(FxRendererInterface, &'a str, &'static str)>,
) {
    for node in &graph.nodes {
        if let (Some(interface), Some(properties)) = (node.renderer_interface(), node.properties())
        {
            output.extend(properties.iter().map(|property| {
                (
                    interface,
                    property.name.as_str(),
                    property.value.kind_name(),
                )
            }));
        }
        match node {
            FxNode::Conditional {
                then_graph,
                else_graph,
                ..
            } => {
                collect_property_schemas(then_graph, output);
                collect_property_schemas(else_graph, output);
            }
            FxNode::Stack { children } => {
                for child in children {
                    collect_property_schemas(child, output);
                }
            }
            _ => {}
        }
    }
}

fn hash_graph(hasher: &mut blake3::Hasher, graph: &FxGraph) {
    hash_usize(hasher, graph.nodes.len());
    for node in &graph.nodes {
        hash_str(hasher, node.kind_name());
        if let Some(interface) = node.renderer_interface() {
            hasher.update(&[interface as u8]);
        }
        if let Some(properties) = node.properties() {
            hash_properties(hasher, properties);
        }
        match node {
            FxNode::Transform { fx, .. }
            | FxNode::Mask { fx, .. }
            | FxNode::Filter { fx, .. }
            | FxNode::Shader { fx, .. }
            | FxNode::OffscreenPass { fx, .. }
            | FxNode::PostProcess { fx, .. }
            | FxNode::Transition { fx, .. } => {
                hash_str(hasher, fx.package());
                hash_str(hasher, fx.function());
            }
            FxNode::Conditional {
                condition,
                then_graph,
                else_graph,
            } => {
                hash_value(hasher, condition);
                hash_graph(hasher, then_graph);
                hash_graph(hasher, else_graph);
            }
            FxNode::Stack { children } => {
                hash_usize(hasher, children.len());
                for child in children {
                    hash_graph(hasher, child);
                }
            }
            FxNode::Style { .. } | FxNode::Text { .. } | FxNode::Color { .. } => {}
        }
    }
}

fn hash_properties(hasher: &mut blake3::Hasher, properties: &[FxProperty]) {
    let mut canonical = properties.iter().collect::<Vec<_>>();
    canonical.sort_by(|left, right| left.name.cmp(&right.name));
    hash_usize(hasher, canonical.len());
    for property in canonical {
        hash_str(hasher, &property.name);
        hash_value(hasher, &property.value);
    }
}

fn hash_value(hasher: &mut blake3::Hasher, value: &FxStaticValue) {
    match value {
        FxStaticValue::Runtime(value) => {
            hasher.update(&[0]);
            hash_runtime_value(hasher, value);
        }
        FxStaticValue::Resource(value) => {
            hasher.update(&[1]);
            hash_str(hasher, value.as_str());
        }
        FxStaticValue::Selector(value) => {
            hasher.update(&[2]);
            hash_str(hasher, value);
        }
        FxStaticValue::String(value) => {
            hasher.update(&[3]);
            hash_str(hasher, value);
        }
        FxStaticValue::Target(value) => {
            hasher.update(&[4, *value as u8]);
        }
        FxStaticValue::Phase(value) => {
            hasher.update(&[5, *value as u8]);
        }
        FxStaticValue::Parameter(value) => {
            hasher.update(&[6]);
            hasher.update(&value.index.to_le_bytes());
            hasher.update(&[value.ty as u8]);
        }
        FxStaticValue::Sampler(value) => {
            hasher.update(&[7]);
            value.hash_into(hasher);
        }
        FxStaticValue::List(values) => {
            hasher.update(&[8]);
            hash_usize(hasher, values.len());
            for value in values {
                hash_value(hasher, value);
            }
        }
        FxStaticValue::Record(properties) => {
            hasher.update(&[9]);
            hash_properties(hasher, properties);
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
    }
}
