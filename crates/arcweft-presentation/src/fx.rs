//! Typed, renderer-independent presentation treatment graphs.
//!
//! `Fx` definitions are ordinary Arcweft functions at the language surface.
//! This module owns the stable presentation identity and compiled graph shared
//! by View, `RichText`, bundle lowering, and renderer adapters.

use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

/// Stable identity of one public `#[fx]` function.
///
/// Re-exports do not create a new identity: `function` is always the qualified
/// name of the original declaration inside `package`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxId {
    package: FxPackageId,
    function: FxQualifiedName,
}

/// Validated package component of an [`FxId`].
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FxPackageId(String);

/// Validated original qualified function component of an [`FxId`].
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FxQualifiedName(String);

/// Stable identity of one applied Fx graph.
#[derive(Clone, Copy, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxInstanceId([u8; 32]);

/// Hash of an Fx function's public parameter and renderer-interface contract.
#[derive(Clone, Copy, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxAbiHash([u8; 32]);

/// Hash of an Fx function's compiled graph and resource bindings.
#[derive(Clone, Copy, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FxSemanticHash([u8; 32]);

/// Invalid stable Fx identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxIdError {
    /// Package identity was omitted.
    #[error("Fx package identity cannot be empty")]
    EmptyPackage,
    /// Package identity contains whitespace, separators, or unsupported punctuation.
    #[error("Fx package identity must contain only letters, digits, `_`, `-`, or `.`")]
    InvalidPackage,
    /// Qualified function identity was omitted or contains an empty segment.
    #[error("Fx function identity must be a non-empty qualified name")]
    InvalidFunction,
}

/// Closed parameter schema exported by a compiled Fx function.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FxParameter {
    name: String,
    type_name: String,
    default: Option<FxValue>,
}

/// One compiled `#[fx]` function.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FxDefinition {
    id: FxId,
    parameters: Vec<FxParameter>,
    graph: FxGraph,
    abi_hash: FxAbiHash,
    semantic_hash: FxSemanticHash,
}

/// Ordered presentation-treatment graph returned by an Fx function.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct FxGraph {
    nodes: Vec<FxNode>,
}

/// One typed treatment node in an [`FxGraph`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FxNode {
    /// General View style properties.
    Style(Vec<FxProperty>),
    /// RichText-specific style properties such as weight, color, and font.
    Text(Vec<FxProperty>),
    /// A color treatment that is not limited to text foreground color.
    Color(Vec<FxProperty>),
    /// Geometry transformation or animated transform sampler.
    Transform {
        fx: FxId,
        properties: Vec<FxProperty>,
    },
    /// Ordered masking operation.
    Mask {
        fx: FxId,
        properties: Vec<FxProperty>,
    },
    /// Ordered image/text filtering operation.
    Filter {
        fx: FxId,
        properties: Vec<FxProperty>,
    },
    /// Renderer shader resource and its typed uniform bindings.
    Shader {
        fx: FxId,
        properties: Vec<FxProperty>,
    },
    /// Presentation transition policy.
    Transition {
        fx: FxId,
        properties: Vec<FxProperty>,
    },
    /// Compile-time graph branch with retained dynamic condition binding.
    Conditional {
        condition: FxValue,
        then_graph: FxGraph,
        else_graph: FxGraph,
    },
    /// Explicit ordered composition of child graphs.
    Stack(Vec<FxGraph>),
}

/// Named input owned by one typed Fx node.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FxProperty {
    name: String,
    value: FxValue,
}

/// Typed value retained in an Fx graph.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FxValue {
    Bool(bool),
    Integer(String),
    Decimal(String),
    String(String),
    /// Unit-bearing scalar such as `2px` or `40%`.
    Scalar {
        value: String,
        unit: String,
    },
    /// Duration scalar such as `120ms`.
    Duration {
        value: String,
        unit: String,
    },
    /// Canonical dot-prefixed enum shorthand.
    Selector(String),
    /// Bound Fx function parameter.
    Parameter(String),
    /// Reactive View expression compiled by the View binding layer.
    Binding(String),
    List(Vec<FxValue>),
    Record(Vec<FxProperty>),
}

impl FxId {
    /// Creates an identity from a package and original qualified function name.
    pub fn try_new(
        package: impl Into<String>,
        function: impl Into<String>,
    ) -> Result<Self, FxIdError> {
        let package = package.into();
        let function = function.into();
        Ok(Self {
            package: FxPackageId::try_new(package)?,
            function: FxQualifiedName::try_new(function)?,
        })
    }

    pub fn package(&self) -> &str {
        self.package.as_str()
    }

    pub fn function(&self) -> &str {
        self.function.as_str()
    }
}

impl FxPackageId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, FxIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(FxIdError::EmptyPackage);
        }
        if !value
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-' | '.'))
        {
            return Err(FxIdError::InvalidPackage);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FxQualifiedName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, FxIdError> {
        let value = value.into();
        if value.is_empty() || !value.split('.').all(valid_identifier) {
            return Err(FxIdError::InvalidFunction);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize)]
struct FxIdWire {
    package: String,
    function: String,
}

impl<'de> Deserialize<'de> for FxId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FxIdWire::deserialize(deserializer)?;
        Self::try_new(wire.package, wire.function).map_err(D::Error::custom)
    }
}

impl fmt::Display for FxId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}::{}", self.package(), self.function())
    }
}

impl FxInstanceId {
    /// Derives an application identity from its definition and stable owner path.
    pub fn derive<'a>(fx: &FxId, components: impl IntoIterator<Item = &'a str>) -> Self {
        let mut hasher = blake3::Hasher::new();
        hash_part(&mut hasher, "arcweft.fx-instance.v1");
        hash_part(&mut hasher, fx.package());
        hash_part(&mut hasher, fx.function());
        for component in components {
            hash_part(&mut hasher, component);
        }
        Self(*hasher.finalize().as_bytes())
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl FxAbiHash {
    /// Derives a deterministic ABI hash from canonical schema parts.
    pub fn derive<'a>(parts: impl IntoIterator<Item = &'a str>) -> Self {
        Self(derive_hash("arcweft.fx-abi.v1", parts))
    }

    /// Derives the public contract from named parameters and required renderer interfaces.
    pub fn for_definition(parameters: &[FxParameter], graph: &FxGraph) -> Self {
        let mut hasher = blake3::Hasher::new();
        hash_part(&mut hasher, "arcweft.fx-abi.v2");
        let mut parameters = parameters.iter().collect::<Vec<_>>();
        parameters.sort_by(|left, right| left.name.cmp(&right.name));
        hash_usize(&mut hasher, parameters.len());
        for parameter in parameters {
            hash_part(&mut hasher, &parameter.name);
            hash_part(&mut hasher, &parameter.type_name);
            match &parameter.default {
                Some(default) => {
                    hash_part(&mut hasher, "default");
                    hash_value(&mut hasher, default);
                }
                None => hash_part(&mut hasher, "required"),
            }
        }
        let interfaces = graph.renderer_interfaces();
        hash_usize(&mut hasher, interfaces.len());
        for interface in interfaces {
            hash_part(&mut hasher, interface);
        }
        Self(*hasher.finalize().as_bytes())
    }

    pub const fn from_bytes(value: [u8; 32]) -> Self {
        Self(value)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl FxSemanticHash {
    /// Derives a deterministic semantic hash from canonical graph parts.
    pub fn derive<'a>(parts: impl IntoIterator<Item = &'a str>) -> Self {
        Self(derive_hash("arcweft.fx-semantic.v1", parts))
    }

    /// Hashes canonical typed graph structure without depending on Rust `Debug` output.
    pub fn for_graph(graph: &FxGraph) -> Self {
        let mut hasher = blake3::Hasher::new();
        hash_part(&mut hasher, "arcweft.fx-semantic.v2");
        hash_graph(&mut hasher, graph);
        Self(*hasher.finalize().as_bytes())
    }

    pub const fn from_bytes(value: [u8; 32]) -> Self {
        Self(value)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl FxParameter {
    pub fn new(
        name: impl Into<String>,
        type_name: impl Into<String>,
        default: Option<FxValue>,
    ) -> Self {
        Self {
            name: name.into(),
            type_name: type_name.into(),
            default,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub const fn default(&self) -> Option<&FxValue> {
        self.default.as_ref()
    }
}

impl FxDefinition {
    pub fn new(
        id: FxId,
        parameters: Vec<FxParameter>,
        graph: FxGraph,
        abi_hash: FxAbiHash,
        semantic_hash: FxSemanticHash,
    ) -> Self {
        Self {
            id,
            parameters,
            graph,
            abi_hash,
            semantic_hash,
        }
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

impl FxGraph {
    pub const fn new(nodes: Vec<FxNode>) -> Self {
        Self { nodes }
    }

    pub fn nodes(&self) -> &[FxNode] {
        &self.nodes
    }

    fn renderer_interfaces(&self) -> BTreeSet<&'static str> {
        let mut interfaces = BTreeSet::new();
        collect_renderer_interfaces(self, &mut interfaces);
        interfaces
    }
}

impl FxProperty {
    pub fn new(name: impl Into<String>, value: FxValue) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn value(&self) -> &FxValue {
        &self.value
    }
}

impl fmt::Debug for FxInstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hash(formatter, "FxInstanceId", &self.0)
    }
}

impl fmt::Debug for FxAbiHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hash(formatter, "FxAbiHash", &self.0)
    }
}

impl fmt::Debug for FxSemanticHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hash(formatter, "FxSemanticHash", &self.0)
    }
}

fn hash_part(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn hash_usize(hasher: &mut blake3::Hasher, value: usize) {
    hasher.update(&(value as u64).to_le_bytes());
}

fn hash_graph(hasher: &mut blake3::Hasher, graph: &FxGraph) {
    hash_usize(hasher, graph.nodes.len());
    for node in &graph.nodes {
        match node {
            FxNode::Style(properties) => hash_node_properties(hasher, "style", properties),
            FxNode::Text(properties) => hash_node_properties(hasher, "text", properties),
            FxNode::Color(properties) => hash_node_properties(hasher, "color", properties),
            FxNode::Transform { fx, properties } => {
                hash_owned_node(hasher, "transform", fx, properties);
            }
            FxNode::Mask { fx, properties } => hash_owned_node(hasher, "mask", fx, properties),
            FxNode::Filter { fx, properties } => {
                hash_owned_node(hasher, "filter", fx, properties);
            }
            FxNode::Shader { fx, properties } => {
                hash_owned_node(hasher, "shader", fx, properties);
            }
            FxNode::Transition { fx, properties } => {
                hash_owned_node(hasher, "transition", fx, properties);
            }
            FxNode::Conditional {
                condition,
                then_graph,
                else_graph,
            } => {
                hash_part(hasher, "conditional");
                hash_value(hasher, condition);
                hash_graph(hasher, then_graph);
                hash_graph(hasher, else_graph);
            }
            FxNode::Stack(children) => {
                hash_part(hasher, "stack");
                hash_usize(hasher, children.len());
                for child in children {
                    hash_graph(hasher, child);
                }
            }
        }
    }
}

fn hash_owned_node(hasher: &mut blake3::Hasher, kind: &str, fx: &FxId, properties: &[FxProperty]) {
    hash_part(hasher, kind);
    hash_part(hasher, fx.package());
    hash_part(hasher, fx.function());
    hash_properties(hasher, properties);
}

fn hash_node_properties(hasher: &mut blake3::Hasher, kind: &str, properties: &[FxProperty]) {
    hash_part(hasher, kind);
    hash_properties(hasher, properties);
}

fn hash_properties(hasher: &mut blake3::Hasher, properties: &[FxProperty]) {
    let mut canonical = properties
        .iter()
        .map(|property| {
            let mut property_hasher = blake3::Hasher::new();
            hash_part(&mut property_hasher, &property.name);
            hash_value(&mut property_hasher, &property.value);
            (
                property.name.as_str(),
                *property_hasher.finalize().as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    canonical.sort_unstable();
    hash_usize(hasher, canonical.len());
    for (_, digest) in canonical {
        hasher.update(&digest);
    }
}

fn hash_value(hasher: &mut blake3::Hasher, value: &FxValue) {
    match value {
        FxValue::Bool(value) => hash_part(hasher, if *value { "bool:true" } else { "bool:false" }),
        FxValue::Integer(value) => {
            hash_part(hasher, "integer");
            hash_part(hasher, value);
        }
        FxValue::Decimal(value) => {
            hash_part(hasher, "decimal");
            hash_part(hasher, value);
        }
        FxValue::String(value) => {
            hash_part(hasher, "string");
            hash_part(hasher, value);
        }
        FxValue::Scalar { value, unit } => {
            hash_part(hasher, "scalar");
            hash_part(hasher, value);
            hash_part(hasher, unit);
        }
        FxValue::Duration { value, unit } => {
            hash_part(hasher, "duration");
            hash_part(hasher, value);
            hash_part(hasher, unit);
        }
        FxValue::Selector(value) => {
            hash_part(hasher, "selector");
            hash_part(hasher, value);
        }
        FxValue::Parameter(value) => {
            hash_part(hasher, "parameter");
            hash_part(hasher, value);
        }
        FxValue::Binding(value) => {
            hash_part(hasher, "binding");
            hash_part(hasher, value);
        }
        FxValue::List(values) => {
            hash_part(hasher, "list");
            hash_usize(hasher, values.len());
            for value in values {
                hash_value(hasher, value);
            }
        }
        FxValue::Record(properties) => {
            hash_part(hasher, "record");
            hash_properties(hasher, properties);
        }
    }
}

fn collect_renderer_interfaces(graph: &FxGraph, interfaces: &mut BTreeSet<&'static str>) {
    for node in &graph.nodes {
        match node {
            FxNode::Style(_) => {
                interfaces.insert("style");
            }
            FxNode::Text(_) => {
                interfaces.insert("text");
            }
            FxNode::Color(_) => {
                interfaces.insert("color");
            }
            FxNode::Transform { .. } => {
                interfaces.insert("transform");
            }
            FxNode::Mask { .. } => {
                interfaces.insert("mask");
            }
            FxNode::Filter { .. } => {
                interfaces.insert("filter");
            }
            FxNode::Shader { .. } => {
                interfaces.insert("shader");
            }
            FxNode::Transition { .. } => {
                interfaces.insert("transition");
            }
            FxNode::Conditional {
                then_graph,
                else_graph,
                ..
            } => {
                interfaces.insert("conditional");
                collect_renderer_interfaces(then_graph, interfaces);
                collect_renderer_interfaces(else_graph, interfaces);
            }
            FxNode::Stack(children) => {
                interfaces.insert("stack");
                for child in children {
                    collect_renderer_interfaces(child, interfaces);
                }
            }
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

fn derive_hash<'a>(domain: &str, parts: impl IntoIterator<Item = &'a str>) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hash_part(&mut hasher, domain);
    for part in parts {
        hash_part(&mut hasher, part);
    }
    *hasher.finalize().as_bytes()
}

fn write_hash(formatter: &mut fmt::Formatter<'_>, label: &str, bytes: &[u8; 32]) -> fmt::Result {
    write!(formatter, "{label}(")?;
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    formatter.write_str(")")
}

#[cfg(test)]
mod tests {
    use super::{FxGraph, FxId, FxInstanceId, FxNode, FxProperty, FxSemanticHash, FxValue};

    #[test]
    fn definition_identity_uses_original_qualified_name() {
        let id = FxId::try_new("game", "game.ui.effects.notice").expect("valid Fx id");
        assert_eq!(id.package(), "game");
        assert_eq!(id.function(), "game.ui.effects.notice");
        assert_eq!(id.to_string(), "game::game.ui.effects.notice");
    }

    #[test]
    fn instance_identity_distinguishes_application_path() {
        let id = FxId::try_new("game", "game.ui.effects.wave").expect("valid Fx id");
        let first = FxInstanceId::derive(&id, ["view.battle", "enemy.1", "0"]);
        let second = FxInstanceId::derive(&id, ["view.battle", "enemy.2", "0"]);
        assert_ne!(first, second);
        assert_eq!(
            first,
            FxInstanceId::derive(&id, ["view.battle", "enemy.1", "0"])
        );
    }

    #[test]
    fn identity_deserialization_revalidates_opaque_components() {
        let error =
            serde_json::from_str::<FxId>(r#"{"package":"game::host","function":"effects.wave"}"#)
                .expect_err("ambiguous package separator is rejected");
        assert!(error.to_string().contains("Fx package identity"));

        let error =
            serde_json::from_str::<FxId>(r#"{"package":"game","function":"effects..wave"}"#)
                .expect_err("empty qualified-name segment is rejected");
        assert!(error.to_string().contains("Fx function identity"));
    }

    #[test]
    fn semantic_hash_is_canonical_for_named_property_order() {
        let first = FxGraph::new(vec![FxNode::Text(vec![
            FxProperty::new("weight", FxValue::Selector("strong".to_owned())),
            FxProperty::new("color", FxValue::String("red".to_owned())),
        ])]);
        let reordered = FxGraph::new(vec![FxNode::Text(vec![
            FxProperty::new("color", FxValue::String("red".to_owned())),
            FxProperty::new("weight", FxValue::Selector("strong".to_owned())),
        ])]);
        let changed = FxGraph::new(vec![FxNode::Text(vec![FxProperty::new(
            "color",
            FxValue::String("blue".to_owned()),
        )])]);

        assert_eq!(
            FxSemanticHash::for_graph(&first),
            FxSemanticHash::for_graph(&reordered)
        );
        assert_ne!(
            FxSemanticHash::for_graph(&first),
            FxSemanticHash::for_graph(&changed)
        );
    }
}
