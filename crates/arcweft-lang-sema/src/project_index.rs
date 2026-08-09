//! Project-wide semantic snapshot used by Agent Script type/effect checking.
//!
//! Agent controllers compile against a stable view of project entities and
//! callable host surfaces. This module keeps that view typed and source-aware
//! without adding parser-specific command shapes.

use crate::callable::{
    AgentIntrinsicSignatureId, CallableCandidateId, CallableInterfaceDigest, CallableValidator,
    CheckedCallableCatalog, CheckedCallableDeclaration, CheckedCallableFacts, CheckedCallableId,
    CheckedCallableLookupError, EnvironmentCallableId,
};
use crate::entry::{CheckedEntryCatalog, CheckedEntryId};
use crate::env::{EffectCapability, FunctionSignature, TypeCheckEnv};
use crate::types::{EntityType, TypeKind};
use arcweft_id::{PublicId, dialogue::DialogueLineId};
use arcweft_lang_hir::{
    identity::ExprId,
    project::HirPackageModuleKey,
    symbol::{
        CallableDeclarationKey, CallableDeclarationOwner, FlowDeclarationId,
        nominal::ProjectNominalDeclarationId,
    },
};
use arcweft_source::{SourceAnchor, SourceSpan};
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

mod entry_roles;
mod final_projection;
mod nominal;

pub use entry_roles::{
    ProjectEntryRecord, ProjectEntryRoleEdge, ProjectEntryRoleKind, ProjectEntryRoleTarget,
};
pub use nominal::{ProjectNominalIndexRecord, ProjectNominalReferenceEdge};

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
    identity: ProjectEntityId,
    ty: EntityType,
    source: SourceAnchor,
    semantic_hash: SemanticHash,
    agent_actions: Vec<AgentActionSignature>,
}

/// Sole project-index identity for an entity.
///
/// Public declarations use their project-global public identity. Structural
/// Flow declarations retain the exact module-preserving identity selected by
/// the accepted project symbol transaction; their public ID is presentation
/// metadata and is not a project-global lookup key.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectEntityId {
    Public(PublicId),
    StructuralFlow(FlowDeclarationId),
}

/// Directed semantic relation between two project entities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectGraphRelation {
    from: ProjectEntityId,
    to: ProjectEntityId,
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
    Entity(ProjectEntityId),
    Callable(CheckedCallableId),
}

/// Project-owned callable symbol declared by source code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCallableSymbol {
    declaration: CallableDeclarationKey,
    checked: CheckedCallableId,
    kind: ProjectCallableKind,
    interface_digest: CallableInterfaceDigest,
}

/// Source callable family represented in the project graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectCallableKind {
    Function,
    View,
    TraitRequirement,
    TraitImplementation,
    InherentMethod,
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

/// Non-authoritative execution projection for an accepted environment callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallableLowering {
    checked: CheckedCallableId,
    lowering: CallableLowering,
}

/// How an accepted environment callable is lowered after semantic checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableLowering {
    HostCapability(EnvironmentCallableId),
    AgentIntrinsic(AgentIntrinsicSignatureId),
}

/// Debug query symbol exposed to Agent RAG/debug commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugQuerySymbol {
    signature: FunctionSignature,
}

/// One source-backed reference to a dialogue line accepted by the same final
/// HIR project and semantic generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedDialogueLineReference {
    target: DialogueLineId,
    source: SourceSpan,
    module: HirPackageModuleKey,
    expression: ExprId,
}

impl AcceptedDialogueLineReference {
    pub const fn new(
        target: DialogueLineId,
        source: SourceSpan,
        module: HirPackageModuleKey,
        expression: ExprId,
    ) -> Self {
        Self {
            target,
            source,
            module,
            expression,
        }
    }

    pub const fn target(&self) -> &DialogueLineId {
        &self.target
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn module(&self) -> &HirPackageModuleKey {
        &self.module
    }

    pub const fn expression(&self) -> ExprId {
        self.expression
    }
}

/// Source, bundle, or remote semantic snapshot for Agent Script compilation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSemanticIndex {
    schema_version: u32,
    program_hash: ProgramHash,
    bundle_hash: Option<BundleHash>,
    entities: BTreeMap<ProjectEntityId, EntitySymbol>,
    checked_callables: Arc<CheckedCallableCatalog>,
    project_callables: BTreeMap<CallableDeclarationKey, ProjectCallableSymbol>,
    environment_lowerings: BTreeMap<EnvironmentCallableId, EnvironmentCallableLowering>,
    entry_records: BTreeMap<CheckedEntryId, ProjectEntryRecord>,
    entry_role_edges: Vec<ProjectEntryRoleEdge>,
    project_nominals: BTreeMap<ProjectNominalDeclarationId, ProjectNominalIndexRecord>,
    project_nominal_references: Box<[ProjectNominalReferenceEdge]>,
    dialogue_line_references: Box<[AcceptedDialogueLineReference]>,
    types: BTreeMap<TypeName, TypeKind>,
    debug_queries: BTreeMap<QualifiedName, DebugQuerySymbol>,
    relations: Vec<ProjectGraphRelation>,
    dependency_relations: Vec<ProjectGraphDependencyRelation>,
    flow_control_summaries: BTreeMap<ProjectEntityId, ProjectFlowControlSummary>,
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
    #[error("final semantic generation is not accepted by this project index: {0}")]
    FinalAnalysis(Box<crate::final_analysis::FinalSemanticAnalysisError>),
    #[error("typed final-HIR source lookup failed: {0}")]
    SourceQuery(Box<arcweft_lang_hir::source_index::HirSourceQueryError>),
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
    #[error("accepted dialogue line `{target}` is missing from the project generation")]
    MissingAcceptedDialogueLine { target: DialogueLineId },
    #[error("dialogue-line reference expression {owner:?} has no accepted project module")]
    MissingDialogueLineReferenceModule { owner: ExprId },
    #[error("dialogue-line reference expression {owner:?} has no exact source span")]
    MissingDialogueLineReferenceSource { owner: ExprId },
    #[error(
        "accepted type-check report has no semantic type for final-HIR root {root:?}: {reason}"
    )]
    MissingCheckedType {
        root: arcweft_lang_hir::identity::TypeId,
        reason: String,
    },
    #[error("final semantic analysis has no checked item fact for {owner:?}")]
    MissingCheckedItem {
        owner: arcweft_lang_hir::identity::ItemId,
    },
    #[error("project semantic indexing cannot resolve final-HIR item {owner:?}")]
    MissingProjectItem {
        owner: arcweft_lang_hir::identity::ItemId,
    },
    #[error("accepted Flow item {owner:?} has no structural project symbol")]
    MissingFlowSymbol {
        owner: arcweft_lang_hir::identity::ItemId,
    },
    #[error("project semantic indexing cannot resolve final-HIR statement {owner:?}")]
    MissingFlowStatement {
        owner: arcweft_lang_hir::identity::StmtId,
    },
    #[error("project semantic indexing cannot resolve final-HIR expression {owner:?}")]
    MissingFlowExpression {
        owner: arcweft_lang_hir::identity::ExprId,
    },
    #[error("accepted Flow statement {owner:?} has no authored source span")]
    MissingFlowStatementSource {
        owner: arcweft_lang_hir::identity::StmtId,
    },
    #[error("accepted Flow target lookup failed: {0}")]
    FlowTargetLookup(Box<arcweft_lang_hir::symbol::ProjectEntityReferenceLookupError>),
    #[error("final-HIR item {owner:?} does not match the accepted semantic entity family")]
    WrongEntityOwner {
        owner: arcweft_lang_hir::identity::ItemId,
    },
    #[error("accepted entity {id} has no authored whole-declaration source span")]
    MissingEntitySource { id: PublicId },
    #[error("project semantic index contains duplicate entity {id:?}")]
    DuplicateEntity { id: ProjectEntityId },
    #[error("project semantic index contains duplicate type name `{name}`")]
    DuplicateType { name: String },
    #[error("typed entity identity `{id}` is invalid for {family}: {message}")]
    InvalidEntityIdentity {
        id: String,
        family: &'static str,
        message: String,
    },
    #[error("project relation references missing entity endpoint {id:?}")]
    MissingRelationEndpoint { id: ProjectEntityId },
    #[error("checked call parent {declaration:?} is absent from the project callable index")]
    MissingProjectCallableParent {
        declaration: Box<CallableDeclarationKey>,
    },
    #[error("accepted nominal reference {root:?} node {node:?} lacks {reason}")]
    MissingNominalReferenceEvidence {
        root: arcweft_lang_hir::identity::TypeId,
        node: arcweft_lang_hir::identity::TypeId,
        reason: &'static str,
    },
    #[error("checked callable catalog lookup failed while constructing the project index: {0:?}")]
    CheckedCallableLookup(Box<CheckedCallableLookupError>),
    #[error("checked callable catalog contains inconsistent project callable identity")]
    InvalidProjectCallableIdentity,
    #[error("checked callable catalog contains inconsistent environment callable identity")]
    InvalidEnvironmentCallableIdentity,
    #[error("checked callable catalog contains a duplicate structural project declaration")]
    DuplicateProjectCallable,
    #[error("checked callable catalog contains a duplicate environment declaration")]
    DuplicateEnvironmentCallable,
}

impl From<crate::final_analysis::FinalSemanticAnalysisError> for ProjectSemanticIndexError {
    fn from(error: crate::final_analysis::FinalSemanticAnalysisError) -> Self {
        Self::FinalAnalysis(Box::new(error))
    }
}

impl From<arcweft_lang_hir::source_index::HirSourceQueryError> for ProjectSemanticIndexError {
    fn from(error: arcweft_lang_hir::source_index::HirSourceQueryError) -> Self {
        Self::SourceQuery(Box::new(error))
    }
}

impl From<CheckedCallableLookupError> for ProjectSemanticIndexError {
    fn from(error: CheckedCallableLookupError) -> Self {
        Self::CheckedCallableLookup(Box::new(error))
    }
}

impl From<arcweft_lang_hir::symbol::ProjectEntityReferenceLookupError>
    for ProjectSemanticIndexError
{
    fn from(error: arcweft_lang_hir::symbol::ProjectEntityReferenceLookupError) -> Self {
        Self::FlowTargetLookup(Box::new(error))
    }
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
        identity: ProjectEntityId,
        ty: EntityType,
        source: SourceAnchor,
        semantic_hash: SemanticHash,
    ) -> Self {
        Self {
            identity,
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

    pub const fn identity(&self) -> &ProjectEntityId {
        &self.identity
    }

    pub const fn public_id(&self) -> &PublicId {
        self.identity.public_id()
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
    pub const fn new(
        from: ProjectEntityId,
        to: ProjectEntityId,
        edge_kind: ProjectGraphRelationKind,
    ) -> Self {
        Self {
            from,
            to,
            edge_kind,
        }
    }

    pub const fn from(&self) -> &ProjectEntityId {
        &self.from
    }

    pub const fn to(&self) -> &ProjectEntityId {
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
}

impl ProjectGraphSymbolRef {
    pub const fn entity(id: ProjectEntityId) -> Self {
        Self::Entity(id)
    }

    pub const fn callable(id: CheckedCallableId) -> Self {
        Self::Callable(id)
    }
}

impl ProjectEntityId {
    pub const fn public(id: PublicId) -> Self {
        Self::Public(id)
    }

    pub const fn structural_flow(declaration: FlowDeclarationId) -> Self {
        Self::StructuralFlow(declaration)
    }

    pub const fn public_id(&self) -> &PublicId {
        match self {
            Self::Public(id) => id,
            Self::StructuralFlow(declaration) => declaration.public_id(),
        }
    }

    pub const fn flow_declaration(&self) -> Option<&FlowDeclarationId> {
        match self {
            Self::StructuralFlow(declaration) => Some(declaration),
            Self::Public(_) => None,
        }
    }

    /// Canonical project-index key for durable diagnostics and graph protocol
    /// projection. Public labels remain display metadata for structural Flow.
    pub fn canonical_key(&self) -> String {
        match self {
            Self::Public(id) => format!("public:{}", id.as_str()),
            Self::StructuralFlow(declaration) => {
                format!("flow:{}", declaration.semantic_digest())
            }
        }
    }
}

impl ProjectCallableSymbol {
    /// Callable family from source syntax.
    pub const fn kind(&self) -> ProjectCallableKind {
        self.kind
    }

    /// Canonical structural source declaration.
    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    /// Revision-bound checked identity admitted for this declaration.
    pub const fn checked(&self) -> &CheckedCallableId {
        &self.checked
    }

    /// Derived checked interface digest used by durable projections.
    pub const fn interface_digest(&self) -> CallableInterfaceDigest {
        self.interface_digest
    }
}

impl ProjectCallableKind {
    /// Stable lowercase graph/RAG label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::View => "view",
            Self::TraitRequirement => "trait_requirement",
            Self::TraitImplementation => "trait_implementation",
            Self::InherentMethod => "inherent_method",
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

impl EnvironmentCallableLowering {
    pub const fn checked(&self) -> &CheckedCallableId {
        &self.checked
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
    pub fn try_new(
        program_hash: ProgramHash,
        checked_callables: Arc<CheckedCallableCatalog>,
    ) -> Result<Self, ProjectSemanticIndexError> {
        let (project_callables, environment_lowerings) =
            checked_callable_projections(&checked_callables)?;
        Ok(Self {
            schema_version: PROJECT_SEMANTIC_INDEX_SCHEMA_VERSION,
            program_hash,
            bundle_hash: None,
            entities: BTreeMap::new(),
            checked_callables,
            project_callables,
            environment_lowerings,
            entry_records: BTreeMap::new(),
            entry_role_edges: Vec::new(),
            project_nominals: BTreeMap::new(),
            project_nominal_references: Box::new([]),
            dialogue_line_references: Box::new([]),
            types: BTreeMap::new(),
            debug_queries: BTreeMap::new(),
            relations: Vec::new(),
            dependency_relations: Vec::new(),
            flow_control_summaries: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn with_bundle_hash(mut self, bundle_hash: BundleHash) -> Self {
        self.bundle_hash = Some(bundle_hash);
        self
    }

    #[must_use]
    pub fn with_entity(mut self, symbol: EntitySymbol) -> Self {
        self.entities.insert(symbol.identity.clone(), symbol);
        self
    }

    /// Replaces the schema-v1 entry records and role edges from one exact checked catalog.
    #[must_use]
    pub fn with_checked_entry_catalog(mut self, catalog: &CheckedEntryCatalog) -> Self {
        (self.entry_records, self.entry_role_edges) =
            entry_roles::checked_entry_records_and_edges(catalog);
        self
    }

    /// Replaces the entry inventory with records produced by the final typed
    /// entry transaction.
    #[must_use]
    pub fn with_entry_inventory(
        mut self,
        records: BTreeMap<CheckedEntryId, ProjectEntryRecord>,
        edges: impl Into<Vec<ProjectEntryRoleEdge>>,
    ) -> Self {
        self.entry_records = records;
        self.entry_role_edges = edges.into();
        self
    }

    /// Replaces nominal tooling projections produced from the exact accepted
    /// final semantic generation.
    #[must_use]
    pub fn with_project_nominal_inventory(
        mut self,
        records: BTreeMap<ProjectNominalDeclarationId, ProjectNominalIndexRecord>,
        references: impl Into<Box<[ProjectNominalReferenceEdge]>>,
    ) -> Self {
        self.project_nominals = records;
        self.project_nominal_references = references.into();
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
        flow_id: ProjectEntityId,
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

    pub fn entities(&self) -> &BTreeMap<ProjectEntityId, EntitySymbol> {
        &self.entities
    }

    pub const fn checked_callables(&self) -> &Arc<CheckedCallableCatalog> {
        &self.checked_callables
    }

    pub fn project_callables(&self) -> &BTreeMap<CallableDeclarationKey, ProjectCallableSymbol> {
        &self.project_callables
    }

    pub fn environment_lowerings(
        &self,
    ) -> &BTreeMap<EnvironmentCallableId, EnvironmentCallableLowering> {
        &self.environment_lowerings
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

    pub fn dialogue_line_references(&self) -> &[AcceptedDialogueLineReference] {
        &self.dialogue_line_references
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

    pub fn flow_control_summaries(&self) -> &BTreeMap<ProjectEntityId, ProjectFlowControlSummary> {
        &self.flow_control_summaries
    }

    pub fn flow_control_summary(
        &self,
        flow_id: &ProjectEntityId,
    ) -> Option<&ProjectFlowControlSummary> {
        self.flow_control_summaries.get(flow_id)
    }

    pub fn entity(&self, id: &ProjectEntityId) -> Option<&EntitySymbol> {
        self.entities.get(id)
    }

    pub fn project_callable_by_declaration(
        &self,
        declaration: &CallableDeclarationKey,
    ) -> Option<&ProjectCallableSymbol> {
        self.project_callables.get(declaration)
    }

    pub fn checked_callable(
        &self,
        checked: &CheckedCallableId,
    ) -> Result<&CheckedCallableFacts, CheckedCallableLookupError> {
        self.checked_callables.callable(checked)
    }

    pub fn environment_lowering(
        &self,
        declaration: &EnvironmentCallableId,
    ) -> Option<&EnvironmentCallableLowering> {
        self.environment_lowerings.get(declaration)
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
}

type ProjectCallableProjectionMap = BTreeMap<CallableDeclarationKey, ProjectCallableSymbol>;
type EnvironmentCallableProjectionMap =
    BTreeMap<EnvironmentCallableId, EnvironmentCallableLowering>;
type CheckedCallableProjections = (
    ProjectCallableProjectionMap,
    EnvironmentCallableProjectionMap,
);

fn checked_callable_projections(
    catalog: &CheckedCallableCatalog,
) -> Result<CheckedCallableProjections, ProjectSemanticIndexError> {
    let mut project_callables = BTreeMap::new();
    let mut environment_lowerings = BTreeMap::new();
    for facts in catalog.records() {
        match facts.id().declaration() {
            CheckedCallableDeclaration::Project(declaration) => {
                let Some(kind) = project_callable_kind(declaration.owner()) else {
                    continue;
                };
                let retained = catalog.project_callable(declaration)?;
                if !std::ptr::eq(retained, facts)
                    || retained.id() != facts.id()
                    || retained.interface_digest() != facts.interface_digest()
                    || !matches!(
                        retained.record().id(),
                        CallableCandidateId::Project(candidate) if candidate == declaration
                    )
                {
                    return Err(ProjectSemanticIndexError::InvalidProjectCallableIdentity);
                }
                let symbol = ProjectCallableSymbol {
                    declaration: declaration.clone(),
                    checked: facts.id().clone(),
                    kind,
                    interface_digest: facts.interface_digest(),
                };
                if project_callables
                    .insert(declaration.clone(), symbol)
                    .is_some()
                {
                    return Err(ProjectSemanticIndexError::DuplicateProjectCallable);
                }
            }
            CheckedCallableDeclaration::Environment(declaration) => {
                if !matches!(
                    facts.record().id(),
                    CallableCandidateId::Environment(candidate) if candidate == declaration
                ) {
                    return Err(ProjectSemanticIndexError::InvalidEnvironmentCallableIdentity);
                }
                let lowering = match facts.signature().validator() {
                    CallableValidator::Agent(intrinsic) => {
                        CallableLowering::AgentIntrinsic(*intrinsic)
                    }
                    _ => CallableLowering::HostCapability(declaration.clone()),
                };
                let projection = EnvironmentCallableLowering {
                    checked: facts.id().clone(),
                    lowering,
                };
                if environment_lowerings
                    .insert(declaration.clone(), projection)
                    .is_some()
                {
                    return Err(ProjectSemanticIndexError::DuplicateEnvironmentCallable);
                }
            }
            CheckedCallableDeclaration::Detached(_) | CheckedCallableDeclaration::Standard(_) => {}
        }
    }
    Ok((project_callables, environment_lowerings))
}

const fn project_callable_kind(owner: CallableDeclarationOwner) -> Option<ProjectCallableKind> {
    match owner {
        CallableDeclarationOwner::Function => Some(ProjectCallableKind::Function),
        CallableDeclarationOwner::View => Some(ProjectCallableKind::View),
        CallableDeclarationOwner::TraitRequirement => Some(ProjectCallableKind::TraitRequirement),
        CallableDeclarationOwner::TraitImplementation => {
            Some(ProjectCallableKind::TraitImplementation)
        }
        CallableDeclarationOwner::InherentMethod => Some(ProjectCallableKind::InherentMethod),
        CallableDeclarationOwner::ExternCapability
        | CallableDeclarationOwner::Flow
        | CallableDeclarationOwner::Predicate
        | CallableDeclarationOwner::Proof => None,
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
