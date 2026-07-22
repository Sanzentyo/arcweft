//! Project-wide semantic snapshot used by Agent Script type/effect checking.
//!
//! Agent controllers compile against a stable view of project entities and
//! callable host surfaces. This module keeps that view typed and source-aware
//! without adding parser-specific command shapes.

use crate::checker::TypeCheckReport;
use crate::entry::{CheckedEntryCatalog, CheckedEntryId};
use crate::env::{
    AgentActionEnvSignature, DebugPathKind, EffectCapability, FunctionParam, FunctionSignature,
    TypeCheckEnv,
};
use crate::types::{EntityKind, EntityType, MapKind, TypeKind};
use arcweft_id::PublicId;
use arcweft_lang_hir::style::HirStyleDecl;
use arcweft_lang_hir::{
    entry::HirEntryItem,
    model::{HirFlowItem, HirModule, HirTopLevelDecl},
    project::HirProject,
    symbol::{
        CallableDeclarationId, CallableDeclarationOwner, CallablePackageId, ProjectSymbolTable,
        nominal::ProjectNominalDeclarationId,
    },
};
use arcweft_lang_syntax::{
    ast::{
        choice::ChoiceAction,
        flow::{Stmt, StmtMatchArm},
        ids::EntityRef,
        items::{EntityDeclItem, EntityDeclKind},
        module_path::CanonicalModulePath,
        pattern::Pattern,
    },
    expr::{CallArg, Expr, Literal, MatchExprArm},
    types::{FnParam as SyntaxFnParam, FnSignature as SyntaxFnSignature},
};
use arcweft_source::{SourceAnchor, SourceDocument, SourceDocumentIdentity, SourceSpan};
use std::collections::BTreeMap;
use thiserror::Error;

mod agent_prelude;
mod entities;
mod entry_roles;
mod flow_control;
mod nominal;
mod relations;

pub use entry_roles::{
    ProjectEntryRecord, ProjectEntryRoleEdge, ProjectEntryRoleKind, ProjectEntryRoleTarget,
};
pub use nominal::{ProjectNominalIndexRecord, ProjectNominalReferenceEdge};

type SourceName = SourceDocument;

#[cfg(test)]
mod tests;

/// Semantic index schema supported by this crate.
pub const PROJECT_SEMANTIC_INDEX_SCHEMA_VERSION: u32 = 2;

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

/// Directed semantic relation between two project entities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectGraphRelation {
    from: PublicId,
    to: PublicId,
    edge_kind: ProjectGraphRelationKind,
}

/// Directed semantic relation between project graph symbols.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectGraphDependencyRelation {
    from: ProjectGraphSymbolRef,
    to: ProjectGraphSymbolRef,
    edge_kind: ProjectGraphDependencyRelationKind,
}

/// Static and dynamic control-flow shape indexed for one source flow.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProjectFlowControlSummary {
    static_gotos: usize,
    dynamic_gotos: usize,
    branches: usize,
    loops: usize,
    awaits: usize,
    threads: usize,
    select_branches: usize,
}

/// Endpoint of a project graph dependency relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectGraphSymbolRef {
    Entity(PublicId),
    Callable(QualifiedName),
}

/// Project-owned callable symbol declared by source code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCallableSymbol {
    kind: ProjectCallableKind,
    declaration: CallableDeclarationId,
    signature: FunctionSignature,
    source: SourceAnchor,
    semantic_hash: SemanticHash,
}

/// Source callable family represented in the project graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectCallableKind {
    Function,
    View,
}

/// Kind of semantic relation represented in the project graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectGraphRelationKind {
    EntryGoto,
    EntryRoute,
    ContainsDialogue,
    ContainsChoice,
    ContainsChoiceOption,
    ChoiceOptionGoto,
    ContentRoot,
    FlowGoto,
    FlowInclude,
    ReferencesEntity,
}

/// Kind of cross-symbol dependency represented in the project graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectGraphDependencyRelationKind {
    CallsCallable,
    ReferencesEntity,
}

/// Agent-visible semantic action attached to an entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentActionSignature {
    action: QualifiedName,
    params: Vec<AgentActionParam>,
    return_type: TypeKind,
}

/// Named payload parameter accepted by an Agent semantic action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentActionParam {
    name: String,
    ty: TypeKind,
    has_default: bool,
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
    AdvanceText,
    Choose,
    ChoiceAction,
    Invoke,
    ViewportPoint,
    PointerClick,
    SignalProbe,
    MetricProbe,
    DebugStatePath,
    ObservationFieldPath,
    StateProbe,
    ObservationProbe,
    EntityMetadata,
    ProjectGraphNeighborhood,
    Diagnostics,
    PredicateExists,
    PredicateActionEnabled,
    PredicateAll,
    PredicateAny,
    PredicateNot,
    Wait,
    Capture,
    Expect,
    Deny,
    Checkpoint,
    Attach,
    Note,
    ReadResource,
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
    project_callables: BTreeMap<CallableDeclarationId, ProjectCallableSymbol>,
    entry_records: BTreeMap<CheckedEntryId, ProjectEntryRecord>,
    entry_role_edges: Vec<ProjectEntryRoleEdge>,
    project_nominals: BTreeMap<ProjectNominalDeclarationId, ProjectNominalIndexRecord>,
    project_nominal_references: Box<[ProjectNominalReferenceEdge]>,
    types: BTreeMap<TypeName, TypeKind>,
    debug_queries: BTreeMap<QualifiedName, DebugQuerySymbol>,
    relations: Vec<ProjectGraphRelation>,
    dependency_relations: Vec<ProjectGraphDependencyRelation>,
    flow_control_summaries: BTreeMap<PublicId, ProjectFlowControlSummary>,
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

/// Failure while projecting a checked HIR module into an Agent project index.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectSemanticIndexError {
    #[error("invalid public id `{id}` while indexing {kind}: {message}")]
    InvalidPublicId {
        id: String,
        kind: &'static str,
        message: String,
    },
    #[error("invalid callable signature for `{name}`: {message}")]
    InvalidCallableSignature { name: String, message: String },
    #[error("invalid callable identity for `{name}`: {message}")]
    InvalidCallableIdentity { name: String, message: String },
    #[error("HIR project module `{module}` is not bound to its source document")]
    MissingProjectSource { module: String },
    #[error(
        "accepted type-check report has no semantic type for {document:?} bytes {range:?}: {reason}"
    )]
    MissingCheckedType {
        document: SourceDocumentIdentity,
        range: (usize, usize),
        reason: String,
    },
    #[error("accepted nominal reference {root:?} node {node} lacks {reason}")]
    MissingNominalReferenceEvidence {
        root: SourceSpan,
        node: String,
        reason: &'static str,
    },
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

impl ProjectGraphRelation {
    pub const fn new(from: PublicId, to: PublicId, edge_kind: ProjectGraphRelationKind) -> Self {
        Self {
            from,
            to,
            edge_kind,
        }
    }

    pub const fn from(&self) -> &PublicId {
        &self.from
    }

    pub const fn to(&self) -> &PublicId {
        &self.to
    }

    pub const fn edge_kind(&self) -> ProjectGraphRelationKind {
        self.edge_kind
    }
}

impl ProjectGraphDependencyRelation {
    pub const fn new(
        from: ProjectGraphSymbolRef,
        to: ProjectGraphSymbolRef,
        edge_kind: ProjectGraphDependencyRelationKind,
    ) -> Self {
        Self {
            from,
            to,
            edge_kind,
        }
    }

    pub const fn from(&self) -> &ProjectGraphSymbolRef {
        &self.from
    }

    pub const fn to(&self) -> &ProjectGraphSymbolRef {
        &self.to
    }

    pub const fn edge_kind(&self) -> ProjectGraphDependencyRelationKind {
        self.edge_kind
    }
}

impl ProjectFlowControlSummary {
    /// Static `goto @flow...` statements in this flow.
    pub const fn static_goto_count(&self) -> usize {
        self.static_gotos
    }

    /// Dynamic `goto expr` statements whose target is not a static entity ref.
    pub const fn dynamic_goto_count(&self) -> usize {
        self.dynamic_gotos
    }

    /// Conditional or match control points.
    pub const fn branch_count(&self) -> usize {
        self.branches
    }

    /// Loop control points.
    pub const fn loop_count(&self) -> usize {
        self.loops
    }

    /// Await control points.
    pub const fn await_count(&self) -> usize {
        self.awaits
    }

    /// Thread control points.
    pub const fn thread_count(&self) -> usize {
        self.threads
    }

    /// Select branch arms.
    pub const fn select_branch_count(&self) -> usize {
        self.select_branches
    }

    /// Whether this flow contains any non-static control surface.
    pub const fn has_dynamic_control(&self) -> bool {
        self.dynamic_gotos > 0
            || self.branches > 0
            || self.loops > 0
            || self.awaits > 0
            || self.threads > 0
            || self.select_branches > 0
    }

    fn record_static_goto(&mut self) {
        self.static_gotos += 1;
    }

    fn record_dynamic_goto(&mut self) {
        self.dynamic_gotos += 1;
    }

    fn record_branch(&mut self) {
        self.branches += 1;
    }

    fn record_loop(&mut self) {
        self.loops += 1;
    }

    fn record_await(&mut self) {
        self.awaits += 1;
    }

    fn record_thread(&mut self) {
        self.threads += 1;
    }

    fn add_select_branches(&mut self, count: usize) {
        self.select_branches += count;
    }

    fn merge(&mut self, other: Self) {
        self.static_gotos += other.static_gotos;
        self.dynamic_gotos += other.dynamic_gotos;
        self.branches += other.branches;
        self.loops += other.loops;
        self.awaits += other.awaits;
        self.threads += other.threads;
        self.select_branches += other.select_branches;
    }
}

impl ProjectGraphSymbolRef {
    pub fn entity(id: impl Into<PublicId>) -> Self {
        Self::Entity(id.into())
    }

    pub fn callable(name: impl Into<QualifiedName>) -> Self {
        Self::Callable(name.into())
    }
}

impl ProjectCallableSymbol {
    /// Creates an ordinary function record with its canonical project identity.
    pub const fn function(
        declaration: CallableDeclarationId,
        signature: FunctionSignature,
        source: SourceAnchor,
        semantic_hash: SemanticHash,
    ) -> Self {
        Self {
            kind: ProjectCallableKind::Function,
            declaration,
            signature,
            source,
            semantic_hash,
        }
    }

    pub const fn view(
        declaration: CallableDeclarationId,
        signature: FunctionSignature,
        source: SourceAnchor,
        semantic_hash: SemanticHash,
    ) -> Self {
        Self {
            kind: ProjectCallableKind::View,
            declaration,
            signature,
            source,
            semantic_hash,
        }
    }

    /// Callable family from source syntax.
    pub const fn kind(&self) -> ProjectCallableKind {
        self.kind
    }

    /// Canonical source declaration for an ordinary function.
    pub const fn declaration(&self) -> &CallableDeclarationId {
        &self.declaration
    }

    /// Typed callable signature projected from source syntax.
    pub const fn signature(&self) -> &FunctionSignature {
        &self.signature
    }

    /// Source range that declared this callable.
    pub const fn source(&self) -> &SourceAnchor {
        &self.source
    }

    /// Stable semantic shape hash for graph/RAG indexing.
    pub const fn semantic_hash(&self) -> &SemanticHash {
        &self.semantic_hash
    }
}

impl ProjectCallableKind {
    /// Stable lowercase graph/RAG label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::View => "view",
        }
    }
}

impl ProjectGraphRelationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EntryGoto => "entry_goto",
            Self::EntryRoute => "entry_route",
            Self::ContainsDialogue => "contains_dialogue",
            Self::ContainsChoice => "contains_choice",
            Self::ContainsChoiceOption => "contains_choice_option",
            Self::ChoiceOptionGoto => "choice_option_goto",
            Self::ContentRoot => "content_root",
            Self::FlowGoto => "flow_goto",
            Self::FlowInclude => "flow_include",
            Self::ReferencesEntity => "references_entity",
        }
    }
}

impl ProjectGraphDependencyRelationKind {
    /// Stable lowercase graph/RAG edge label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CallsCallable => "calls_callable",
            Self::ReferencesEntity => "references_entity",
        }
    }
}

impl AgentActionSignature {
    pub fn new(
        action: QualifiedName,
        params: impl IntoIterator<Item = AgentActionParam>,
        return_type: TypeKind,
    ) -> Self {
        Self {
            action,
            params: params.into_iter().collect(),
            return_type,
        }
    }

    pub const fn action(&self) -> &QualifiedName {
        &self.action
    }

    pub fn params(&self) -> &[AgentActionParam] {
        &self.params
    }

    pub const fn return_type(&self) -> &TypeKind {
        &self.return_type
    }
}

impl AgentActionParam {
    pub fn required(name: impl Into<String>, ty: TypeKind) -> Self {
        Self {
            name: name.into(),
            ty,
            has_default: false,
        }
    }

    pub fn defaulted(name: impl Into<String>, ty: TypeKind) -> Self {
        Self {
            name: name.into(),
            ty,
            has_default: true,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    pub const fn has_default(&self) -> bool {
        self.has_default
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
            callables: agent_prelude::agent_prelude_callables(),
            project_callables: BTreeMap::new(),
            entry_records: BTreeMap::new(),
            entry_role_edges: Vec::new(),
            project_nominals: BTreeMap::new(),
            project_nominal_references: Box::new([]),
            types: BTreeMap::new(),
            debug_queries: BTreeMap::new(),
            relations: Vec::new(),
            dependency_relations: Vec::new(),
            flow_control_summaries: BTreeMap::new(),
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
    pub fn with_project_callable(mut self, symbol: ProjectCallableSymbol) -> Self {
        self.project_callables
            .insert(symbol.declaration().clone(), symbol);
        self
    }

    /// Replaces the schema-v1 entry records and role edges from one exact checked catalog.
    #[must_use]
    pub fn with_checked_entry_catalog(mut self, catalog: &CheckedEntryCatalog) -> Self {
        (self.entry_records, self.entry_role_edges) =
            entry_roles::checked_entry_records_and_edges(catalog);
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

    #[must_use]
    pub fn with_relation(mut self, relation: ProjectGraphRelation) -> Self {
        if !self.relations.contains(&relation) {
            self.relations.push(relation);
        }
        self
    }

    #[must_use]
    pub fn with_dependency_relation(mut self, relation: ProjectGraphDependencyRelation) -> Self {
        if !self.dependency_relations.contains(&relation) {
            self.dependency_relations.push(relation);
        }
        self
    }

    #[must_use]
    pub fn with_flow_control_summary(
        mut self,
        flow_id: PublicId,
        summary: ProjectFlowControlSummary,
    ) -> Self {
        self.flow_control_summaries.insert(flow_id, summary);
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

    pub fn project_callables(&self) -> &BTreeMap<CallableDeclarationId, ProjectCallableSymbol> {
        &self.project_callables
    }

    pub fn entry_records(&self) -> &BTreeMap<CheckedEntryId, ProjectEntryRecord> {
        &self.entry_records
    }

    pub fn entry_role_edges(&self) -> &[ProjectEntryRoleEdge] {
        &self.entry_role_edges
    }

    pub fn project_nominals(
        &self,
    ) -> &BTreeMap<ProjectNominalDeclarationId, ProjectNominalIndexRecord> {
        &self.project_nominals
    }

    pub fn project_nominal_references(&self) -> &[ProjectNominalReferenceEdge] {
        &self.project_nominal_references
    }

    pub fn project_nominal(
        &self,
        declaration: &ProjectNominalDeclarationId,
    ) -> Option<&ProjectNominalIndexRecord> {
        self.project_nominals.get(declaration)
    }

    pub fn types(&self) -> &BTreeMap<TypeName, TypeKind> {
        &self.types
    }

    pub fn debug_queries(&self) -> &BTreeMap<QualifiedName, DebugQuerySymbol> {
        &self.debug_queries
    }

    pub fn relations(&self) -> &[ProjectGraphRelation] {
        &self.relations
    }

    pub fn dependency_relations(&self) -> &[ProjectGraphDependencyRelation] {
        &self.dependency_relations
    }

    pub fn flow_control_summaries(&self) -> &BTreeMap<PublicId, ProjectFlowControlSummary> {
        &self.flow_control_summaries
    }

    pub fn flow_control_summary(&self, flow_id: &PublicId) -> Option<&ProjectFlowControlSummary> {
        self.flow_control_summaries.get(flow_id)
    }

    pub fn entity(&self, id: &PublicId) -> Option<&EntitySymbol> {
        self.entities.get(id)
    }

    pub fn callable(&self, name: &QualifiedName) -> Option<&CallableSymbol> {
        self.callables.get(name)
    }

    pub fn project_callable(&self, name: &QualifiedName) -> Option<&ProjectCallableSymbol> {
        let mut matches = self
            .project_callables
            .values()
            .filter(|symbol| symbol.declaration().qualified_name() == name.as_str());
        let callable = matches.next()?;
        matches.next().is_none().then_some(callable)
    }

    pub fn project_callable_by_declaration(
        &self,
        declaration: &CallableDeclarationId,
    ) -> Option<&ProjectCallableSymbol> {
        self.project_callables.get(declaration)
    }

    pub fn entry_record(&self, id: &CheckedEntryId) -> Option<&ProjectEntryRecord> {
        self.entry_records.get(id)
    }

    pub fn entry_role_edges_for(
        &self,
        id: &CheckedEntryId,
    ) -> impl Iterator<Item = &ProjectEntryRoleEdge> {
        self.entry_role_edges
            .iter()
            .filter(move |edge| edge.entry() == id)
    }

    pub fn typecheck_env(&self) -> TypeCheckEnv {
        let mut env = agent_prelude::agent_prelude_env();
        for entity in self.entities.values() {
            env = env.with_symbol(entity.id.as_str(), TypeKind::Ref(entity.ty.clone()));
            for action in entity.agent_actions() {
                env = env.with_agent_action(
                    entity.id.as_str(),
                    AgentActionEnvSignature::new(
                        action.action().as_str(),
                        action.params().iter().map(|param| {
                            crate::env::AgentActionEnvParam::new(
                                param.name(),
                                param.ty().clone(),
                                param.has_default(),
                            )
                        }),
                        action.return_type().clone(),
                    ),
                );
            }
        }
        for (name, callable) in &self.callables {
            env = env
                .with_function_signature(name.as_str(), callable.signature.clone())
                .with_function_effects(name.as_str(), callable.effects.clone());
        }
        for (name, ty) in &self.types {
            env = env.with_symbol(name.as_str(), ty.clone());
        }
        for (name, query) in &self.debug_queries {
            if let Some((kind, path)) = debug_path_from_query_name(name.as_str()) {
                env = env.with_debug_path(kind, path, query.signature().return_type().clone());
            }
        }
        env
    }
}

fn debug_path_from_query_name(name: &str) -> Option<(DebugPathKind, &str)> {
    name.strip_prefix("state.")
        .map(|path| (DebugPathKind::State, path))
        .or_else(|| {
            name.strip_prefix("observation.")
                .map(|path| (DebugPathKind::Observation, path))
        })
        .filter(|(_, path)| !path.is_empty())
}

/// Builds the Agent-facing project semantic index for one checked HIR module.
///
/// The index is the stable entity snapshot Agent Script compiles against. It
/// intentionally mirrors the source/HIR declarations instead of accepting
/// ad-hoc CLI-only entity shims.
pub fn project_semantic_index_from_hir(
    module: &HirModule,
    program_hash: ProgramHash,
    document: &SourceDocument,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    let index = index_hir_module_symbols(
        module,
        ProjectSemanticIndex::new(program_hash),
        document,
        None,
        None,
    )?;
    relations::index_project_symbol_dependency_relations(module, index)
}

/// Builds the final project-wide schema-v2 index from canonical checked project facts.
pub fn project_semantic_index_from_checked_project(
    project: &HirProject,
    symbols: &ProjectSymbolTable,
    typecheck: &TypeCheckReport,
    program_hash: ProgramHash,
    entries: &CheckedEntryCatalog,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    let mut index = ProjectSemanticIndex::new(program_hash);
    for (module_path, module) in project.modules() {
        let document = module.source_document().ok_or_else(|| {
            ProjectSemanticIndexError::MissingProjectSource {
                module: module_path.to_string(),
            }
        })?;
        let checked_types = nominal::CheckedTypeProjection::new(document, typecheck);
        index = index_hir_module_symbols(
            module,
            index,
            document,
            Some(project.package()),
            Some(&checked_types),
        )?;
    }
    for (_, module) in project.modules() {
        index = relations::index_project_symbol_dependency_relations(module, index)?;
    }
    let (project_nominals, project_nominal_references) =
        nominal::checked_project_nominals(symbols, typecheck)?;
    index.project_nominals = project_nominals;
    index.project_nominal_references = project_nominal_references;
    Ok(index.with_checked_entry_catalog(entries))
}

fn index_hir_module_symbols(
    module: &HirModule,
    mut index: ProjectSemanticIndex,
    document: &SourceDocument,
    package: Option<&CallablePackageId>,
    checked_types: Option<&nominal::CheckedTypeProjection<'_>>,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for flow in module.flows() {
        if let Some(id) = flow.id() {
            index = index.with_entity(entities::entity_symbol(
                id,
                EntityKind::Flow,
                None,
                document,
                "flow",
            )?);
        }
        index = entities::index_flow_items(flow.body(), index, document)?;
        index = relations::index_flow_item_relations(flow.id(), flow.body(), index)?;
        if let Some(id) = flow.id() {
            index = index.with_flow_control_summary(
                relations::public_id_for_relation(id, "flow control summary")?,
                flow_control::summarize_flow_control_items(flow.body()),
            );
        }
    }
    if let (Some(package), Some(checked_types)) = (package, checked_types) {
        for function in module.functions() {
            let declaration =
                CallableDeclarationId::for_function(package, function).map_err(|error| {
                    ProjectSemanticIndexError::InvalidCallableIdentity {
                        name: function.qualified_name(),
                        message: error.to_string(),
                    }
                })?;
            index = index.with_project_callable(entities::project_function_symbol(
                declaration,
                function,
                document,
                checked_types,
            )?);
        }
    }
    for declaration in module.declarations() {
        index = index_top_level_declaration(
            declaration,
            index,
            document,
            package,
            module.module_path(),
            checked_types,
        )?;
    }
    Ok(index)
}

fn index_top_level_declaration(
    declaration: &HirTopLevelDecl,
    mut index: ProjectSemanticIndex,
    document: &SourceDocument,
    package: Option<&CallablePackageId>,
    module: &CanonicalModulePath,
    checked_types: Option<&nominal::CheckedTypeProjection<'_>>,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match declaration {
        HirTopLevelDecl::Source(source) => {
            if let Some(id) = source.item().id() {
                index = index.with_entity(entities::entity_symbol(
                    id,
                    EntityKind::Source,
                    None,
                    document,
                    "source",
                )?);
            }
        }
        HirTopLevelDecl::EntityDecl(item) => {
            index = index_view_callable(index, item, document, package, module, checked_types)?;
            index = index.with_entity(entities::entity_symbol(
                item.id(),
                entities::entity_decl_kind(item.kind()),
                None,
                document,
                entities::entity_decl_kind_label(item.kind()),
            )?);
            index = index_view_text_control_inputs(index, item, document)?;
            if let Some(content) = item.content_body() {
                index = relations::index_content_root_relations(item.id(), content.roots(), index)?;
            }
        }
        HirTopLevelDecl::Entry(item) => {
            index = index.with_entity(entities::entity_symbol(
                item.id(),
                EntityKind::Entry,
                None,
                document,
                "entry",
            )?);
            index = relations::index_entry_relations(item.id(), item.items(), index)?;
        }
        HirTopLevelDecl::Test(item) => {
            if let Some(id) = item.id().as_absolute() {
                index = index.with_entity(entities::entity_symbol(
                    id,
                    EntityKind::Test,
                    None,
                    document,
                    "test",
                )?);
            }
        }
        HirTopLevelDecl::Bench(item) => {
            if let Some(id) = item.id().as_absolute() {
                index = index.with_entity(entities::entity_symbol(
                    id,
                    EntityKind::Bench,
                    None,
                    document,
                    "bench",
                )?);
            }
        }
        HirTopLevelDecl::Style(item) => {
            index = index_view_style_entity(index, item, document)?;
        }
        HirTopLevelDecl::Trait(_)
        | HirTopLevelDecl::Impl(_)
        | HirTopLevelDecl::Enum(_)
        | HirTopLevelDecl::ExternCapability(_)
        | HirTopLevelDecl::ExternMod(_)
        | HirTopLevelDecl::Struct(_)
        | HirTopLevelDecl::TypeAlias(_)
        | HirTopLevelDecl::Proof(_) => {}
    }
    Ok(index)
}

fn index_view_callable(
    index: ProjectSemanticIndex,
    item: &EntityDeclItem,
    document: &SourceDocument,
    package: Option<&CallablePackageId>,
    module: &CanonicalModulePath,
    checked_types: Option<&nominal::CheckedTypeProjection<'_>>,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    if item.kind() != EntityDeclKind::View {
        return Ok(index);
    }
    let (Some(package), Some(checked_types)) = (package, checked_types) else {
        return Ok(index);
    };
    let name = item.local_binding_name().ok_or_else(|| {
        ProjectSemanticIndexError::InvalidCallableIdentity {
            name: item.id().body().to_owned(),
            message: "View declaration has no local binding name".to_owned(),
        }
    })?;
    let callable = CallableDeclarationId::try_new(
        package.clone(),
        module.clone(),
        CallableDeclarationOwner::View,
        name,
    )
    .map_err(|error| ProjectSemanticIndexError::InvalidCallableIdentity {
        name: name.to_owned(),
        message: error.to_string(),
    })?;
    Ok(
        index.with_project_callable(entities::project_view_callable_symbol(
            callable,
            item,
            document,
            checked_types,
        )?),
    )
}

fn index_view_style_entity(
    index: ProjectSemanticIndex,
    item: &HirStyleDecl,
    document: &SourceDocument,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index_view_resource_entity(index, item.id(), EntityKind::Style, document, "style")
}

fn index_view_resource_entity(
    index: ProjectSemanticIndex,
    id: &EntityRef,
    kind: EntityKind,
    document: &SourceDocument,
    label: &'static str,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    Ok(index.with_entity(entities::entity_symbol(id, kind, None, document, label)?))
}

fn index_view_text_control_inputs(
    mut index: ProjectSemanticIndex,
    item: &EntityDeclItem,
    document: &SourceDocument,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    let Some(view) = item.view_body().and_then(|body| body.view()) else {
        return Ok(index);
    };
    for input in view.text_control_inputs() {
        let input = input.canonical_entity_ref();
        index = index_view_resource_entity(index, &input, EntityKind::Input, document, "input")?;
    }
    Ok(index)
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
