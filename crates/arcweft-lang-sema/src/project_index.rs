//! Project-wide semantic snapshot used by Agent Script type/effect checking.
//!
//! Agent controllers compile against a stable view of project entities and
//! callable host surfaces. This module keeps that view typed and source-aware
//! without adding parser-specific command shapes.

use crate::env::{EffectCapability, FunctionParam, FunctionSignature, TypeCheckEnv};
use crate::types::{EntityKind, EntityType, TypeKind};
use arcweft_id::PublicId;
use arcweft_source::SourceAnchor;
use std::collections::BTreeMap;

/// Semantic index schema supported by this crate.
pub const PROJECT_SEMANTIC_INDEX_SCHEMA_VERSION: u32 = 1;

/// Stable hash of the project program that produced a semantic index.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProgramHash(String);

/// Stable hash of a compiled bundle that produced a semantic index.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BundleHash(String);

/// Stable hash of one semantic symbol shape.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticHash(String);

/// Dotted callable or debug query name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct QualifiedName(String);

/// Named type key exported by a source project, bundle, or remote session.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeName(String);

/// Project entity symbol available to Agent Script.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntitySymbol {
    id: PublicId,
    ty: EntityType,
    source: SourceAnchor,
    semantic_hash: SemanticHash,
    agent_actions: Vec<AgentActionSignature>,
}

/// Agent-visible semantic action attached to an entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentActionSignature {
    action: QualifiedName,
    args: Vec<TypeKind>,
    return_type: TypeKind,
}

/// Callable symbol available in the Agent compile environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSymbol {
    signature: FunctionSignature,
    effects: Vec<EffectCapability>,
    lowering: CallableLowering,
}

/// How a callable is lowered after type/effect checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableLowering {
    Bytecode,
    HostCapability(QualifiedName),
    AgentIntrinsic(AgentIntrinsic),
}

/// Agent intrinsic call families with structured checker rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentIntrinsic {
    Observe,
    Choose,
    Invoke,
    SignalProbe,
    MetricProbe,
    StateProbe,
    Wait,
    Capture,
    Expect,
    Deny,
    Checkpoint,
    Attach,
    Note,
    RagQuery,
}

/// Debug query symbol exposed to Agent RAG/debug commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugQuerySymbol {
    signature: FunctionSignature,
}

/// Source, bundle, or remote semantic snapshot for Agent Script compilation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSemanticIndex {
    schema_version: u32,
    program_hash: ProgramHash,
    bundle_hash: Option<BundleHash>,
    entities: BTreeMap<PublicId, EntitySymbol>,
    callables: BTreeMap<QualifiedName, CallableSymbol>,
    types: BTreeMap<TypeName, TypeKind>,
    debug_queries: BTreeMap<QualifiedName, DebugQuerySymbol>,
}

/// Policy applied while compiling Agent Script.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentCompilePolicy {
    allowed_effects: Vec<EffectCapability>,
    max_timeout_millis: u64,
    max_steps: u64,
}

/// Agent compile context assembled from project, prelude, launch, and policy.
pub struct AgentCompileContext<'a> {
    pub project: &'a ProjectSemanticIndex,
    pub prelude: &'a TypeCheckEnv,
    pub launch_exports: &'a TypeCheckEnv,
    pub policy: AgentCompilePolicy,
}

impl ProgramHash {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl BundleHash {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl SemanticHash {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl QualifiedName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TypeName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl EntitySymbol {
    pub fn new(
        id: PublicId,
        ty: EntityType,
        source: SourceAnchor,
        semantic_hash: SemanticHash,
    ) -> Self {
        Self {
            id,
            ty,
            source,
            semantic_hash,
            agent_actions: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_agent_action(mut self, action: AgentActionSignature) -> Self {
        self.agent_actions.push(action);
        self
    }

    pub const fn id(&self) -> &PublicId {
        &self.id
    }

    pub const fn ty(&self) -> &EntityType {
        &self.ty
    }

    pub const fn source(&self) -> &SourceAnchor {
        &self.source
    }

    pub const fn semantic_hash(&self) -> &SemanticHash {
        &self.semantic_hash
    }

    pub fn agent_actions(&self) -> &[AgentActionSignature] {
        &self.agent_actions
    }
}

impl AgentActionSignature {
    pub fn new(
        action: QualifiedName,
        args: impl IntoIterator<Item = TypeKind>,
        return_type: TypeKind,
    ) -> Self {
        Self {
            action,
            args: args.into_iter().collect(),
            return_type,
        }
    }

    pub const fn action(&self) -> &QualifiedName {
        &self.action
    }

    pub fn args(&self) -> &[TypeKind] {
        &self.args
    }

    pub const fn return_type(&self) -> &TypeKind {
        &self.return_type
    }
}

impl CallableSymbol {
    pub fn new(
        signature: FunctionSignature,
        effects: impl IntoIterator<Item = EffectCapability>,
        lowering: CallableLowering,
    ) -> Self {
        Self {
            signature,
            effects: effects.into_iter().collect(),
            lowering,
        }
    }

    pub const fn signature(&self) -> &FunctionSignature {
        &self.signature
    }

    pub fn effects(&self) -> &[EffectCapability] {
        &self.effects
    }

    pub const fn lowering(&self) -> &CallableLowering {
        &self.lowering
    }
}

impl DebugQuerySymbol {
    pub fn new(signature: FunctionSignature) -> Self {
        Self { signature }
    }

    pub const fn signature(&self) -> &FunctionSignature {
        &self.signature
    }
}

impl ProjectSemanticIndex {
    pub fn new(program_hash: ProgramHash) -> Self {
        Self {
            schema_version: PROJECT_SEMANTIC_INDEX_SCHEMA_VERSION,
            program_hash,
            bundle_hash: None,
            entities: BTreeMap::new(),
            callables: agent_prelude_callables(),
            types: BTreeMap::new(),
            debug_queries: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_bundle_hash(mut self, bundle_hash: BundleHash) -> Self {
        self.bundle_hash = Some(bundle_hash);
        self
    }

    #[must_use]
    pub fn with_entity(mut self, symbol: EntitySymbol) -> Self {
        self.entities.insert(symbol.id.clone(), symbol);
        self
    }

    #[must_use]
    pub fn with_callable(mut self, name: QualifiedName, symbol: CallableSymbol) -> Self {
        self.callables.insert(name, symbol);
        self
    }

    #[must_use]
    pub fn with_type(mut self, name: TypeName, ty: TypeKind) -> Self {
        self.types.insert(name, ty);
        self
    }

    #[must_use]
    pub fn with_debug_query(mut self, name: QualifiedName, symbol: DebugQuerySymbol) -> Self {
        self.debug_queries.insert(name, symbol);
        self
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub const fn program_hash(&self) -> &ProgramHash {
        &self.program_hash
    }

    pub const fn bundle_hash(&self) -> Option<&BundleHash> {
        self.bundle_hash.as_ref()
    }

    pub fn entities(&self) -> &BTreeMap<PublicId, EntitySymbol> {
        &self.entities
    }

    pub fn callables(&self) -> &BTreeMap<QualifiedName, CallableSymbol> {
        &self.callables
    }

    pub fn types(&self) -> &BTreeMap<TypeName, TypeKind> {
        &self.types
    }

    pub fn debug_queries(&self) -> &BTreeMap<QualifiedName, DebugQuerySymbol> {
        &self.debug_queries
    }

    pub fn entity(&self, id: &PublicId) -> Option<&EntitySymbol> {
        self.entities.get(id)
    }

    pub fn callable(&self, name: &QualifiedName) -> Option<&CallableSymbol> {
        self.callables.get(name)
    }

    pub fn typecheck_env(&self) -> TypeCheckEnv {
        let mut env = agent_prelude_env();
        for entity in self.entities.values() {
            env = env.with_symbol(entity.id.as_str(), TypeKind::Ref(entity.ty.clone()));
        }
        for (name, callable) in &self.callables {
            env = env
                .with_function_signature(name.as_str(), callable.signature.clone())
                .with_function_effects(name.as_str(), callable.effects.clone());
        }
        for (name, ty) in &self.types {
            env = env.with_symbol(name.as_str(), ty.clone());
        }
        env
    }
}

impl Default for AgentCompilePolicy {
    fn default() -> Self {
        Self {
            allowed_effects: vec![
                EffectCapability::new("agent.observe"),
                EffectCapability::new("agent.wait"),
            ],
            max_timeout_millis: 30_000,
            max_steps: 1_024,
        }
    }
}

impl AgentCompilePolicy {
    pub fn new(
        allowed_effects: impl IntoIterator<Item = EffectCapability>,
        max_timeout_millis: u64,
        max_steps: u64,
    ) -> Self {
        Self {
            allowed_effects: allowed_effects.into_iter().collect(),
            max_timeout_millis,
            max_steps,
        }
    }

    pub fn allowed_effects(&self) -> &[EffectCapability] {
        &self.allowed_effects
    }

    pub const fn max_timeout_millis(&self) -> u64 {
        self.max_timeout_millis
    }

    pub const fn max_steps(&self) -> u64 {
        self.max_steps
    }
}

/// Agent Prelude projected into the current lightweight type-checking env.
pub fn agent_prelude_env() -> TypeCheckEnv {
    let mut env = TypeCheckEnv::standard();
    for (name, callable) in agent_prelude_callables() {
        env = env
            .with_function_signature(name.as_str(), callable.signature.clone())
            .with_function_effects(name.as_str(), callable.effects.clone());
    }
    env.with_method_signature(
        TypeKind::Probe(Box::new(TypeKind::Bool)),
        "eq",
        FunctionSignature::new(
            TypeKind::Predicate,
            [FunctionParam::required("expected", TypeKind::Bool)],
        ),
    )
}

fn agent_prelude_callables() -> BTreeMap<QualifiedName, CallableSymbol> {
    agent_observation_callables()
        .into_iter()
        .chain(agent_action_callables())
        .chain(agent_capture_callables())
        .chain(agent_record_callables())
        .chain(agent_rag_callables())
        .map(|(name, callable)| (QualifiedName::new(name), callable))
        .collect()
}

fn agent_observation_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
        (
            "observe",
            CallableSymbol::new(
                FunctionSignature::new(TypeKind::Observation, []),
                [EffectCapability::new("agent.observe")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Observe),
            ),
        ),
        (
            "expect",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Unit,
                    [
                        FunctionParam::required("condition", TypeKind::Bool),
                        FunctionParam::required("message", TypeKind::String),
                    ],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Expect),
            ),
        ),
        (
            "deny",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Unit,
                    [
                        FunctionParam::required("condition", TypeKind::Bool),
                        FunctionParam::required("message", TypeKind::String),
                    ],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Deny),
            ),
        ),
        (
            "wait",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Observation,
                    [
                        FunctionParam::required("predicate", TypeKind::Predicate),
                        FunctionParam::required("timeout", TypeKind::Duration),
                    ],
                ),
                [
                    EffectCapability::new("agent.wait"),
                    EffectCapability::new("agent.observe"),
                ],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Wait),
            ),
        ),
        (
            "signal",
            CallableSymbol::new(
                FunctionSignature::return_only(TypeKind::Probe(Box::new(TypeKind::Named(
                    "_".to_owned(),
                )))),
                [EffectCapability::new("agent.observe")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::SignalProbe),
            ),
        ),
        (
            "metric",
            CallableSymbol::new(
                FunctionSignature::return_only(TypeKind::Probe(Box::new(TypeKind::Named(
                    "_".to_owned(),
                )))),
                [EffectCapability::new("agent.observe")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::MetricProbe),
            ),
        ),
    ]
}

fn agent_action_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![(
        "choose",
        CallableSymbol::new(
            FunctionSignature::new(
                TypeKind::ActionResult,
                [FunctionParam::required(
                    "choice",
                    TypeKind::entity_ref(EntityKind::ChoiceOption),
                )],
            ),
            [EffectCapability::new("agent.act.semantic")],
            CallableLowering::AgentIntrinsic(AgentIntrinsic::Choose),
        ),
    )]
}

fn agent_capture_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
        (
            "capture",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::CaptureRef,
                    [FunctionParam::required("target", TypeKind::CaptureTarget)],
                ),
                [EffectCapability::new("agent.capture")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Capture),
            ),
        ),
        (
            "viewport",
            CallableSymbol::new(
                FunctionSignature::new(TypeKind::CaptureTarget, []),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Capture),
            ),
        ),
        (
            "layer",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::CaptureTarget,
                    [FunctionParam::required(
                        "target",
                        TypeKind::entity_ref(EntityKind::Layer),
                    )],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Capture),
            ),
        ),
        (
            "object",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::CaptureTarget,
                    [FunctionParam::required(
                        "id",
                        TypeKind::Named("ObservedObjectId".to_owned()),
                    )],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Capture),
            ),
        ),
    ]
}

fn agent_record_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
        (
            "attach",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required("resource", TypeKind::CaptureRef)],
                ),
                [EffectCapability::new("debug.record")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Attach),
            ),
        ),
        (
            "checkpoint",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required("name", TypeKind::String)],
                ),
                [EffectCapability::new("debug.record")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Checkpoint),
            ),
        ),
        (
            "note",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required("text", TypeKind::DisplayText)],
                ),
                [EffectCapability::new("debug.record")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Note),
            ),
        ),
    ]
}

fn agent_rag_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![(
        "rag.query",
        CallableSymbol::new(
            FunctionSignature::new(
                TypeKind::RagContextPack,
                [FunctionParam::required("query", TypeKind::String)],
            ),
            [EffectCapability::new("rag.query")],
            CallableLowering::AgentIntrinsic(AgentIntrinsic::RagQuery),
        ),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_source::SourceAnchor;

    fn public_id(value: &str) -> PublicId {
        PublicId::try_new(value).expect("valid public id")
    }

    #[test]
    fn project_index_preserves_entity_payload_type() {
        let signal = EntitySymbol::new(
            public_id("signal.ready"),
            EntityType::new(EntityKind::Signal, Some(TypeKind::Bool)),
            SourceAnchor::generated(),
            SemanticHash::new("shape.signal.ready.v1"),
        );
        let index =
            ProjectSemanticIndex::new(ProgramHash::new("program-a")).with_entity(signal.clone());

        let stored = index.entity(signal.id()).expect("signal stored");

        assert_eq!(stored.ty().kind(), &EntityKind::Signal);
        assert_eq!(stored.ty().value(), Some(&TypeKind::Bool));
        assert_eq!(stored.semantic_hash().as_str(), "shape.signal.ready.v1");
        assert_eq!(
            index.typecheck_env().symbol_type("signal.ready"),
            Some(&TypeKind::entity_ref_with_value(
                EntityKind::Signal,
                TypeKind::Bool
            ))
        );
    }

    #[test]
    fn project_index_projects_entities_and_agent_prelude_to_env() {
        let index = ProjectSemanticIndex::new(ProgramHash::new("program-a")).with_entity(
            EntitySymbol::new(
                public_id("choice.opening.listen"),
                EntityType::new(EntityKind::ChoiceOption, None),
                SourceAnchor::generated(),
                SemanticHash::new("shape.choice.opening.listen.v1"),
            ),
        );
        let env = index.typecheck_env();

        assert_eq!(
            env.symbol_type("choice.opening.listen"),
            Some(&TypeKind::entity_ref(EntityKind::ChoiceOption))
        );
        assert_eq!(
            env.function_signature("choose")
                .map(FunctionSignature::return_type),
            Some(&TypeKind::ActionResult)
        );
        assert_eq!(
            env.function_effects("choose").map(|effects| {
                effects
                    .iter()
                    .map(EffectCapability::as_str)
                    .collect::<Vec<_>>()
            }),
            Some(vec!["agent.act.semantic"])
        );
    }

    #[test]
    fn agent_prelude_marks_structured_intrinsic_lowering() {
        let prelude = agent_prelude_callables();
        let wait = prelude
            .get(&QualifiedName::new("wait"))
            .expect("wait intrinsic");

        assert_eq!(
            wait.lowering(),
            &CallableLowering::AgentIntrinsic(AgentIntrinsic::Wait)
        );
        assert_eq!(
            wait.effects()
                .iter()
                .map(EffectCapability::as_str)
                .collect::<Vec<_>>(),
            vec!["agent.wait", "agent.observe"]
        );
    }
}
