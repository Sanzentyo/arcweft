//! Project-wide semantic snapshot used by Agent Script type/effect checking.
//!
//! Agent controllers compile against a stable view of project entities and
//! callable host surfaces. This module keeps that view typed and source-aware
//! without adding parser-specific command shapes.

use crate::checker::helpers::type_ref_kind;
use crate::env::{
    AgentActionEnvSignature, DebugPathKind, EffectCapability, FunctionParam, FunctionSignature,
    TypeCheckEnv,
};
use crate::types::{EntityKind, EntityType, MapKind, TypeKind};
use arcweft_id::PublicId;
use arcweft_lang_hir::model::{HirFlowItem, HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::{
    ast::{flow::FlowKind, flow::Stmt, ids::EntityRef, items::EntityDeclKind},
    expr::{CallArg, Expr, Literal, MatchExprArm},
    types::{TypeRef, parse_type_ref},
};
use arcweft_source::{SourceAnchor, SourceName};
use std::collections::BTreeMap;
use thiserror::Error;

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

/// Failure while projecting a checked HIR module into an Agent project index.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectSemanticIndexError {
    #[error("invalid public id `{id}` while indexing {kind}: {message}")]
    InvalidPublicId {
        id: String,
        kind: &'static str,
        message: String,
    },
    #[error("invalid signal type for `{id}`: {message}")]
    InvalidSignalType { id: String, message: String },
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
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    let mut index = ProjectSemanticIndex::new(program_hash);
    for flow in module.flows() {
        if let Some(id) = flow.id() {
            let kind = match flow.kind() {
                FlowKind::Flow => EntityKind::Flow,
                FlowKind::Fragment => EntityKind::Fragment,
            };
            index = index.with_entity(entity_symbol(id, kind, None, source_name.clone(), "flow")?);
        }
        index = index_flow_items(flow.body(), index, source_name)?;
    }
    for agent in module.agents() {
        if let Some(id) = agent.item().id() {
            index = index.with_entity(entity_symbol(
                id,
                EntityKind::Agent,
                None,
                source_name.clone(),
                "agent",
            )?);
        }
    }
    for declaration in module.declarations() {
        match declaration {
            HirTopLevelDecl::Source(source) => {
                if let Some(id) = source.id() {
                    index = index.with_entity(entity_symbol(
                        id,
                        EntityKind::Source,
                        None,
                        source_name.clone(),
                        "source",
                    )?);
                }
            }
            HirTopLevelDecl::EntityDecl(item) => {
                let value = if item.kind() == EntityDeclKind::Signal {
                    signal_value_type(item.id().body(), item.signature_tail())?
                } else {
                    None
                };
                index = index.with_entity(entity_symbol(
                    item.id(),
                    entity_decl_kind(item.kind()),
                    value,
                    source_name.clone(),
                    entity_decl_kind_label(item.kind()),
                )?);
            }
            HirTopLevelDecl::Entry(item) => {
                index = index.with_entity(entity_symbol(
                    item.id(),
                    EntityKind::Entry,
                    None,
                    source_name.clone(),
                    "entry",
                )?);
            }
            HirTopLevelDecl::Test(item) => {
                if let Some(id) = item.id().as_absolute() {
                    index = index.with_entity(entity_symbol(
                        id,
                        EntityKind::Test,
                        None,
                        source_name.clone(),
                        "test",
                    )?);
                }
            }
            HirTopLevelDecl::Bench(item) => {
                if let Some(id) = item.id().as_absolute() {
                    index = index.with_entity(entity_symbol(
                        id,
                        EntityKind::Bench,
                        None,
                        source_name.clone(),
                        "bench",
                    )?);
                }
            }
            HirTopLevelDecl::Callable(_)
            | HirTopLevelDecl::State(_)
            | HirTopLevelDecl::Trait(_)
            | HirTopLevelDecl::Impl(_)
            | HirTopLevelDecl::Enum(_)
            | HirTopLevelDecl::ExternCapability(_)
            | HirTopLevelDecl::ExternMod(_)
            | HirTopLevelDecl::DialogueDefaults(_)
            | HirTopLevelDecl::Struct(_)
            | HirTopLevelDecl::TypeAlias(_)
            | HirTopLevelDecl::Hook(_)
            | HirTopLevelDecl::MemoFn(_)
            | HirTopLevelDecl::Proof(_)
            | HirTopLevelDecl::TrustedAxiom(_)
            | HirTopLevelDecl::Parser(_) => {}
        }
    }
    Ok(index)
}

fn index_flow_items(
    items: &[HirFlowItem],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for item in items {
        index = index_flow_item_entities(item, index, source_name)?;
        index = index_flow_item_agent_actions(item, index, source_name)?;
    }
    Ok(index)
}

fn index_flow_item_entities(
    item: &HirFlowItem,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match item {
        HirFlowItem::Dialogue(dialogue) => {
            if let Some(id) = dialogue.id() {
                index = index.with_entity(entity_symbol(
                    id,
                    EntityKind::DialogueLine,
                    None,
                    source_name.clone(),
                    "dialogue line",
                )?);
            }
            if let Some(text_key) = dialogue.text_key() {
                index = index.with_entity(entity_symbol(
                    text_key,
                    EntityKind::Text,
                    None,
                    source_name.clone(),
                    "text",
                )?);
            }
        }
        HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
            if let Some(id) = choice.id() {
                index = index.with_entity(entity_symbol(
                    id,
                    EntityKind::Choice,
                    None,
                    source_name.clone(),
                    "choice",
                )?);
            }
            for option in choice.options() {
                if let Some(id) = option.id() {
                    index = index.with_entity(entity_symbol(
                        id,
                        EntityKind::ChoiceOption,
                        None,
                        source_name.clone(),
                        "choice option",
                    )?);
                }
            }
        }
        HirFlowItem::If(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
            index = index_flow_items(block.else_body(), index, source_name)?;
        }
        HirFlowItem::IfLet(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
            index = index_flow_items(block.else_body(), index, source_name)?;
        }
        HirFlowItem::Match(block) => {
            for arm in block.arms() {
                index = index_flow_items(arm.body(), index, source_name)?;
            }
        }
        HirFlowItem::Loop(block) | HirFlowItem::LetLoop { block, .. } => {
            index = index_flow_items(block.body(), index, source_name)?;
        }
        HirFlowItem::While(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
        }
        HirFlowItem::WhileLet(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
        }
        HirFlowItem::For(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
        }
        HirFlowItem::Borrow(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
        }
        HirFlowItem::SourceLocale(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
        }
        HirFlowItem::Scope(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
        }
        HirFlowItem::Select(block) => {
            for branch in block.branches() {
                index = index_flow_items(branch.body(), index, source_name)?;
            }
        }
        HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
            for branch in await_with.branches() {
                index = index_flow_items(branch.body(), index, source_name)?;
            }
        }
        HirFlowItem::Thread(thread) => {
            index = index_flow_items(thread.body(), index, source_name)?;
        }
        HirFlowItem::Stmt(_) | HirFlowItem::LetScope { .. } | HirFlowItem::Include(_) => {}
    }
    Ok(index)
}

fn index_flow_item_agent_actions(
    item: &HirFlowItem,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match item {
        HirFlowItem::Stmt(stmt) => index = index_stmt_agent_actions(stmt, index, source_name)?,
        HirFlowItem::LetScope { scope, .. } => {
            for stmt in scope.statements() {
                index = index_stmt_agent_actions(stmt, index, source_name)?;
            }
            if let Some(value) = scope.value() {
                index = index_expr_agent_actions(value, index, source_name)?;
            }
        }
        _ => {}
    }
    Ok(index)
}

fn index_stmt_agent_actions(
    stmt: &Stmt,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match stmt {
        Stmt::Let { expr, .. }
        | Stmt::Return(expr)
        | Stmt::Out { expr, .. }
        | Stmt::Goto(expr)
        | Stmt::Defer { expr, .. }
        | Stmt::Yield(expr)
        | Stmt::LifetimeSet { expr, .. }
        | Stmt::Close(expr)
        | Stmt::Select(expr)
        | Stmt::Expr(expr) => index = index_expr_agent_actions(expr, index, source_name)?,
        Stmt::Signal { target, value } => {
            index = index_expr_agent_actions(target, index, source_name)?;
            index = index_expr_agent_actions(value, index, source_name)?;
        }
        Stmt::LetElse {
            expr, else_body, ..
        } => {
            index = index_expr_agent_actions(expr, index, source_name)?;
            for stmt in else_body {
                index = index_stmt_agent_actions(stmt, index, source_name)?;
            }
        }
        Stmt::DeferBlock { statements, .. } => {
            for stmt in statements {
                index = index_stmt_agent_actions(stmt, index, source_name)?;
            }
        }
        Stmt::On { body, .. } | Stmt::UnsafeLifetime { body, .. } | Stmt::Loop { body } => {
            for stmt in body {
                index = index_stmt_agent_actions(stmt, index, source_name)?;
            }
        }
        Stmt::If { condition, body } | Stmt::While { condition, body } => {
            index = index_expr_agent_actions(condition, index, source_name)?;
            for stmt in body {
                index = index_stmt_agent_actions(stmt, index, source_name)?;
            }
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            index = index_expr_agent_actions(expr, index, source_name)?;
            if let Some(guard) = guard {
                index = index_expr_agent_actions(guard, index, source_name)?;
            }
            for stmt in body {
                index = index_stmt_agent_actions(stmt, index, source_name)?;
            }
        }
        Stmt::For { source, body, .. } => {
            index = index_expr_agent_actions(source, index, source_name)?;
            for stmt in body {
                index = index_stmt_agent_actions(stmt, index, source_name)?;
            }
        }
        Stmt::Match { expr, arms } => {
            index = index_expr_agent_actions(expr, index, source_name)?;
            for arm in arms {
                if let Some(guard) = arm.guard() {
                    index = index_expr_agent_actions(guard, index, source_name)?;
                }
                for stmt in arm.body() {
                    index = index_stmt_agent_actions(stmt, index, source_name)?;
                }
            }
        }
        Stmt::LetScope { scope, .. } => {
            for stmt in scope.statements() {
                index = index_stmt_agent_actions(stmt, index, source_name)?;
            }
            if let Some(value) = scope.value() {
                index = index_expr_agent_actions(value, index, source_name)?;
            }
        }
        Stmt::Wait(_)
        | Stmt::LetChoice { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::Thread(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => {}
    }
    Ok(index)
}

fn index_expr_agent_actions(
    expr: &Expr,
    index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match expr {
        Expr::Call { callee, args } => {
            index_call_expr_agent_actions(callee, args, index, source_name)
        }
        Expr::MethodCall { receiver, args, .. } => {
            index_method_call_expr_agent_actions(receiver, args, index, source_name)
        }
        Expr::Field { target, .. }
        | Expr::Try { expr: target }
        | Expr::Await { expr: target, .. }
        | Expr::Unary { expr: target, .. } => index_expr_agent_actions(target, index, source_name),
        Expr::DialogueCall { callee, .. } | Expr::Closure { body: callee, .. } => {
            index_expr_agent_actions(callee, index, source_name)
        }
        Expr::Index {
            target,
            index: item,
        } => index_two_expr_agent_actions(target, item, index, source_name),
        Expr::Pipe { lhs, rhs } | Expr::Binary { lhs, rhs, .. } => {
            index_two_expr_agent_actions(lhs, rhs, index, source_name)
        }
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            index_expr_list_agent_actions(items, index, source_name)
        }
        Expr::ArrayRepeat { value, len } => {
            index_two_expr_agent_actions(value, len, index, source_name)
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            index_record_expr_agent_actions(fields, index, source_name)
        }
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => index_expr_block_agent_actions(statements, value.as_deref(), index, source_name),
        Expr::MemoBlock {
            options,
            statements,
            value,
        } => {
            index_memo_expr_agent_actions(options, statements, value.as_deref(), index, source_name)
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => index_if_expr_agent_actions(
            condition,
            then_branch,
            else_branch.as_deref(),
            index,
            source_name,
        ),
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => index_if_let_expr_agent_actions(
            expr,
            guard.as_deref(),
            then_branch,
            else_branch.as_deref(),
            index,
            source_name,
        ),
        Expr::Match { scrutinee, arms } => {
            index_match_expr_agent_actions(scrutinee, arms, index, source_name)
        }
        Expr::Range {
            start,
            end,
            inclusive: _,
        } => index_range_expr_agent_actions(start.as_deref(), end.as_deref(), index, source_name),
        Expr::Thread { .. }
        | Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::Placeholder(_)
        | Expr::NumericBracketSeq(_)
        | Expr::Raw(_) => Ok(index),
    }
}

fn index_two_expr_agent_actions(
    first: &Expr,
    second: &Expr,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_agent_actions(first, index, source_name)?;
    index_expr_agent_actions(second, index, source_name)
}

fn index_expr_list_agent_actions(
    items: &[Expr],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for item in items {
        index = index_expr_agent_actions(item, index, source_name)?;
    }
    Ok(index)
}

fn index_record_expr_agent_actions(
    fields: &[(String, Expr)],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for (_, value) in fields {
        index = index_expr_agent_actions(value, index, source_name)?;
    }
    Ok(index)
}

fn index_range_expr_agent_actions(
    start: Option<&Expr>,
    end: Option<&Expr>,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    if let Some(start) = start {
        index = index_expr_agent_actions(start, index, source_name)?;
    }
    if let Some(end) = end {
        index = index_expr_agent_actions(end, index, source_name)?;
    }
    Ok(index)
}

fn index_call_expr_agent_actions(
    callee: &Expr,
    args: &[CallArg],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    if matches!(callee, Expr::Path(path) if path == "image" || path == "image.show") {
        index = index_image_call_agent_actions(args, index, source_name)?;
    }
    index = index_expr_agent_actions(callee, index, source_name)?;
    index_call_arg_agent_actions(args, index, source_name)
}

fn index_method_call_expr_agent_actions(
    receiver: &Expr,
    args: &[CallArg],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_agent_actions(receiver, index, source_name)?;
    index_call_arg_agent_actions(args, index, source_name)
}

fn index_expr_block_agent_actions(
    statements: &[Stmt],
    value: Option<&Expr>,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for stmt in statements {
        index = index_stmt_agent_actions(stmt, index, source_name)?;
    }
    if let Some(value) = value {
        index = index_expr_agent_actions(value, index, source_name)?;
    }
    Ok(index)
}

fn index_memo_expr_agent_actions(
    options: &[(String, Expr)],
    statements: &[Stmt],
    value: Option<&Expr>,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for (_, value) in options {
        index = index_expr_agent_actions(value, index, source_name)?;
    }
    index_expr_block_agent_actions(statements, value, index, source_name)
}

fn index_if_expr_agent_actions(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_agent_actions(condition, index, source_name)?;
    index = index_expr_agent_actions(then_branch, index, source_name)?;
    if let Some(else_branch) = else_branch {
        index = index_expr_agent_actions(else_branch, index, source_name)?;
    }
    Ok(index)
}

fn index_if_let_expr_agent_actions(
    expr: &Expr,
    guard: Option<&Expr>,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_agent_actions(expr, index, source_name)?;
    if let Some(guard) = guard {
        index = index_expr_agent_actions(guard, index, source_name)?;
    }
    index = index_expr_agent_actions(then_branch, index, source_name)?;
    if let Some(else_branch) = else_branch {
        index = index_expr_agent_actions(else_branch, index, source_name)?;
    }
    Ok(index)
}

fn index_match_expr_agent_actions(
    scrutinee: &Expr,
    arms: &[MatchExprArm],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_agent_actions(scrutinee, index, source_name)?;
    for arm in arms {
        if let Some(guard) = arm.guard() {
            index = index_expr_agent_actions(guard, index, source_name)?;
        }
        index = index_expr_agent_actions(arm.value(), index, source_name)?;
    }
    Ok(index)
}

fn index_call_arg_agent_actions(
    args: &[CallArg],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for arg in args {
        match arg {
            CallArg::Positional(value) => {
                index = index_expr_agent_actions(value, index, source_name)?;
            }
            CallArg::Named { value, .. } | CallArg::Spread { value } => {
                index = index_expr_agent_actions(value.as_ref(), index, source_name)?;
            }
        }
    }
    Ok(index)
}

fn index_image_call_agent_actions(
    args: &[CallArg],
    mut index: ProjectSemanticIndex,
    _source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    let Some(target) = image_call_target(args) else {
        return Ok(index);
    };
    let actions = image_call_actions(args)?;
    if actions.is_empty() {
        return Ok(index);
    }

    let target_id = PublicId::try_new(target.clone()).map_err(|error| {
        ProjectSemanticIndexError::InvalidPublicId {
            id: target.clone(),
            kind: "image action target",
            message: error.to_string(),
        }
    })?;
    let symbol = index.entities.entry(target_id.clone()).or_insert_with(|| {
        EntitySymbol::new(
            target_id,
            EntityType::new(EntityKind::Target, None),
            SourceAnchor::generated(),
            SemanticHash::new(format!("hir:image-target:{target}")),
        )
    });
    for action in actions {
        if symbol
            .agent_actions
            .iter()
            .any(|candidate| candidate.action().as_str() == action)
        {
            continue;
        }
        symbol.agent_actions.push(AgentActionSignature::new(
            QualifiedName::new(action),
            [],
            TypeKind::ActionResult,
        ));
    }
    Ok(index)
}

fn image_call_target(args: &[CallArg]) -> Option<String> {
    for arg in args {
        if let CallArg::Named { name, value } = arg
            && name == "target"
            && let Some(value) = image_call_id_value(value)
        {
            return Some(value);
        }
    }
    None
}

fn image_call_actions(args: &[CallArg]) -> Result<Vec<String>, ProjectSemanticIndexError> {
    args.iter()
        .filter_map(|arg| match arg {
            CallArg::Named { name, value } if name == "action" || name == "actions" => {
                Some(image_call_action_values(value))
            }
            _ => None,
        })
        .flatten()
        .collect()
}

fn image_call_action_values(value: &Expr) -> Vec<Result<String, ProjectSemanticIndexError>> {
    match value {
        Expr::Literal(Literal::String(value)) => value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| Ok(value.to_owned()))
            .collect(),
        Expr::Path(path) => vec![Ok(path.trim_start_matches('.').to_owned())],
        Expr::EntityRef(entity) => vec![Ok(entity.body().to_owned())],
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            items.iter().flat_map(image_call_action_values).collect()
        }
        _ => Vec::new(),
    }
}

fn image_call_id_value(value: &Expr) -> Option<String> {
    match value {
        Expr::Literal(Literal::String(value)) => Some(value.clone()),
        Expr::EntityRef(entity) => Some(entity.body().to_owned()),
        Expr::Path(path) => Some(path.trim_start_matches('.').to_owned()),
        _ => None,
    }
}

fn entity_symbol(
    id: &EntityRef,
    kind: EntityKind,
    value: Option<TypeKind>,
    source_name: SourceName,
    kind_label: &'static str,
) -> Result<EntitySymbol, ProjectSemanticIndexError> {
    let public_id = PublicId::try_new(id.body()).map_err(|error| {
        ProjectSemanticIndexError::InvalidPublicId {
            id: id.body().to_owned(),
            kind: kind_label,
            message: error.to_string(),
        }
    })?;
    let source = SourceAnchor::new(source_name, id.range().as_range());
    let semantic_hash = SemanticHash::new(format!(
        "hir:{kind_label}:{}:{}",
        id.body(),
        value
            .as_ref()
            .map_or_else(|| "_".to_owned(), type_kind_stable_label)
    ));
    Ok(EntitySymbol::new(
        public_id,
        EntityType::new(kind, value),
        source,
        semantic_hash,
    ))
}

fn signal_value_type(
    id: &str,
    signature_tail: &str,
) -> Result<Option<TypeKind>, ProjectSemanticIndexError> {
    let Some(type_source) = signature_tail.trim().strip_prefix(':') else {
        return Ok(None);
    };
    let type_ref = parse_type_ref(type_source.trim()).map_err(|error| {
        ProjectSemanticIndexError::InvalidSignalType {
            id: id.to_owned(),
            message: error.to_string(),
        }
    })?;
    Ok(Some(signal_declared_value_type(&type_ref)))
}

fn signal_declared_value_type(ty: &TypeRef) -> TypeKind {
    match ty {
        TypeRef::Generic { base, args } if base == "Watch" && args.len() == 1 => {
            project_type_ref_kind(&args[0])
        }
        _ => project_type_ref_kind(ty),
    }
}

fn project_type_ref_kind(ty: &TypeRef) -> TypeKind {
    match ty {
        TypeRef::Generic { base, args } if base == "Ref" && args.len() == 1 => {
            if let TypeRef::Path(name) = &args[0] {
                entity_kind_from_type_name(name)
                    .map_or_else(|| type_ref_kind(ty), TypeKind::entity_ref)
            } else {
                type_ref_kind(ty)
            }
        }
        TypeRef::Generic { base, args } if base == "Option" && args.len() == 1 => {
            TypeKind::Option(Box::new(project_type_ref_kind(&args[0])))
        }
        TypeRef::Generic { base, args } if base == "Vec" && args.len() == 1 => {
            TypeKind::Vec(Box::new(project_type_ref_kind(&args[0])))
        }
        _ => type_ref_kind(ty),
    }
}

fn entity_kind_from_type_name(name: &str) -> Option<EntityKind> {
    Some(match name {
        "Agent" => EntityKind::Agent,
        "Entry" => EntityKind::Entry,
        "Flow" => EntityKind::Flow,
        "Fragment" => EntityKind::Fragment,
        "Choice" => EntityKind::Choice,
        "ChoiceOption" => EntityKind::ChoiceOption,
        "Character" => EntityKind::Character,
        "Component" => EntityKind::Component,
        "Activity" => EntityKind::Activity,
        "Textbox" => EntityKind::Textbox,
        "DialogueLine" => EntityKind::DialogueLine,
        "Text" => EntityKind::Text,
        "Asset" => EntityKind::Asset,
        "Image" => EntityKind::Image,
        "Animation" => EntityKind::Animation,
        "Capture" => EntityKind::Capture,
        "Hook" => EntityKind::Hook,
        "Signal" => EntityKind::Signal,
        "Metric" => EntityKind::Metric,
        "Scene" => EntityKind::Scene,
        "Source" => EntityKind::Source,
        "Test" => EntityKind::Test,
        "Bench" => EntityKind::Bench,
        "Layer" => EntityKind::Layer,
        "Voice" => EntityKind::Voice,
        "Se" => EntityKind::Se,
        "Bgm" => EntityKind::Bgm,
        "AudioBus" => EntityKind::AudioBus,
        "MixerSnapshot" => EntityKind::MixerSnapshot,
        "Ducking" => EntityKind::Ducking,
        "Motion" => EntityKind::Motion,
        "Rig" => EntityKind::Rig,
        "Slot" => EntityKind::Slot,
        "Target" => EntityKind::Target,
        _ => return None,
    })
}

fn entity_decl_kind(kind: EntityDeclKind) -> EntityKind {
    match kind {
        EntityDeclKind::Asset => EntityKind::Asset,
        EntityDeclKind::Image => EntityKind::Image,
        EntityDeclKind::Character => EntityKind::Character,
        EntityDeclKind::Component => EntityKind::Component,
        EntityDeclKind::Activity => EntityKind::Activity,
        EntityDeclKind::Signal => EntityKind::Signal,
        EntityDeclKind::Metric => EntityKind::Metric,
        EntityDeclKind::Layer => EntityKind::Layer,
        EntityDeclKind::Textbox => EntityKind::Textbox,
        EntityDeclKind::Voice => EntityKind::Voice,
        EntityDeclKind::Se => EntityKind::Se,
        EntityDeclKind::Bgm => EntityKind::Bgm,
        EntityDeclKind::AudioBus => EntityKind::AudioBus,
        EntityDeclKind::MixerSnapshot => EntityKind::MixerSnapshot,
        EntityDeclKind::Ducking => EntityKind::Ducking,
        EntityDeclKind::Motion => EntityKind::Motion,
        EntityDeclKind::Rig => EntityKind::Rig,
    }
}

fn entity_decl_kind_label(kind: EntityDeclKind) -> &'static str {
    match kind {
        EntityDeclKind::Asset => "asset",
        EntityDeclKind::Image => "image",
        EntityDeclKind::Character => "character",
        EntityDeclKind::Component => "component",
        EntityDeclKind::Activity => "activity",
        EntityDeclKind::Signal => "signal",
        EntityDeclKind::Metric => "metric",
        EntityDeclKind::Layer => "layer",
        EntityDeclKind::Textbox => "textbox",
        EntityDeclKind::Voice => "voice",
        EntityDeclKind::Se => "se",
        EntityDeclKind::Bgm => "bgm",
        EntityDeclKind::AudioBus => "audio bus",
        EntityDeclKind::MixerSnapshot => "mixer snapshot",
        EntityDeclKind::Ducking => "ducking",
        EntityDeclKind::Motion => "motion",
        EntityDeclKind::Rig => "rig",
    }
}

fn type_kind_stable_label(ty: &TypeKind) -> String {
    match ty {
        TypeKind::Ref(entity) => entity.value().map_or_else(
            || format!("Ref<{:?}>", entity.kind()),
            |value| format!("Ref<{:?},{}>", entity.kind(), type_kind_stable_label(value)),
        ),
        TypeKind::Probe(inner) => format!("Probe<{}>", type_kind_stable_label(inner)),
        TypeKind::Vec(inner) => format!("Vec<{}>", type_kind_stable_label(inner)),
        TypeKind::Array { item, len } => {
            format!("Array<{},{}>", type_kind_stable_label(item), len)
        }
        TypeKind::Slice(inner) => format!("Slice<{}>", type_kind_stable_label(inner)),
        TypeKind::Seq(inner) => format!("Seq<{}>", type_kind_stable_label(inner)),
        TypeKind::Map { kind, key, value } => format!(
            "Map<{:?},{},{}>",
            kind,
            type_kind_stable_label(key),
            type_kind_stable_label(value)
        ),
        TypeKind::BorrowRef { inner, .. } => {
            format!("BorrowRef<{}>", type_kind_stable_label(inner))
        }
        TypeKind::Need { ready, error } => format!(
            "Need<{},{}>",
            type_kind_stable_label(ready),
            type_kind_stable_label(error)
        ),
        TypeKind::Stream { item, error } => format!(
            "Stream<{},{}>",
            type_kind_stable_label(item),
            type_kind_stable_label(error)
        ),
        TypeKind::Source { item, error } => format!(
            "Source<{},{}>",
            type_kind_stable_label(item),
            type_kind_stable_label(error)
        ),
        TypeKind::Result { ok, error } => format!(
            "Result<{},{}>",
            type_kind_stable_label(ok),
            type_kind_stable_label(error)
        ),
        TypeKind::Option(inner) => format!("Option<{}>", type_kind_stable_label(inner)),
        TypeKind::Handle { name, .. } => format!("Handle<{name}>"),
        TypeKind::ThreadHandle(inner) => format!("ThreadHandle<{}>", type_kind_stable_label(inner)),
        TypeKind::Shared(inner) => format!("Shared<{}>", type_kind_stable_label(inner)),
        TypeKind::Function { return_type } => {
            format!("Function<{}>", type_kind_stable_label(return_type))
        }
        TypeKind::Speaker(kind) => format!("Speaker<{kind:?}>"),
        TypeKind::SpeakerPreset(kind) => format!("SpeakerPreset<{kind:?}>"),
        TypeKind::CharacterPatch(kind) => format!("CharacterPatch<{kind:?}>"),
        TypeKind::Named(name) => name.clone(),
        TypeKind::Tuple(items) => format!(
            "Tuple<{}>",
            items
                .iter()
                .map(type_kind_stable_label)
                .collect::<Vec<_>>()
                .join(",")
        ),
        TypeKind::Choice(items) => format!(
            "Choice<{}>",
            items
                .iter()
                .map(type_kind_stable_label)
                .collect::<Vec<_>>()
                .join("|")
        ),
        other => format!("{other:?}"),
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
    .with_method_signature(
        TypeKind::Named("Diagnostics".to_owned()),
        "has_error",
        FunctionSignature::return_only(TypeKind::Predicate),
    )
    .with_method_signature(
        TypeKind::RagContextPack,
        "summary",
        FunctionSignature::return_only(TypeKind::DisplayText),
    )
}

fn agent_prelude_callables() -> BTreeMap<QualifiedName, CallableSymbol> {
    agent_observation_callables()
        .into_iter()
        .chain(agent_probe_callables())
        .chain(agent_predicate_callables())
        .chain(agent_action_callables())
        .chain(agent_capture_callables())
        .chain(agent_resource_callables())
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
                FunctionSignature::new(agent_result(TypeKind::Observation), []),
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
                    TypeKind::Result {
                        ok: Box::new(TypeKind::Observation),
                        error: Box::new(TypeKind::Named("WaitError".to_owned())),
                    },
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
    ]
}

fn agent_probe_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
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
        (
            "state_path",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::DebugStatePath,
                    [FunctionParam::required("path", TypeKind::String)],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::DebugStatePath),
            ),
        ),
        (
            "observation_path",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::ObservationFieldPath,
                    [FunctionParam::required("path", TypeKind::String)],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::ObservationFieldPath),
            ),
        ),
        (
            "state",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Probe(Box::new(TypeKind::AgentValue)),
                    [FunctionParam::required("path", TypeKind::DebugStatePath)],
                ),
                [EffectCapability::new("debug.read")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::StateProbe),
            ),
        ),
        (
            "observation",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Probe(Box::new(TypeKind::AgentValue)),
                    [FunctionParam::required(
                        "path",
                        TypeKind::ObservationFieldPath,
                    )],
                ),
                [EffectCapability::new("agent.observe")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::ObservationProbe),
            ),
        ),
        (
            "diagnostics",
            CallableSymbol::new(
                FunctionSignature::return_only(TypeKind::Named("Diagnostics".to_owned())),
                [EffectCapability::new("agent.observe")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Diagnostics),
            ),
        ),
    ]
}

fn agent_predicate_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
        (
            "exists",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Predicate,
                    [FunctionParam::required(
                        "probe",
                        TypeKind::Probe(Box::new(TypeKind::Named("_".to_owned()))),
                    )],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::PredicateExists),
            ),
        ),
        (
            "action_enabled",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Predicate,
                    [FunctionParam::required("target", TypeKind::ActionTarget)],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::PredicateActionEnabled),
            ),
        ),
        (
            "all",
            CallableSymbol::new(
                FunctionSignature::return_only(TypeKind::Predicate),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::PredicateAll),
            ),
        ),
        (
            "any",
            CallableSymbol::new(
                FunctionSignature::return_only(TypeKind::Predicate),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::PredicateAny),
            ),
        ),
        (
            "not",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Predicate,
                    [FunctionParam::required("predicate", TypeKind::Predicate)],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::PredicateNot),
            ),
        ),
    ]
}

fn agent_action_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
        (
            "choice_action",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::ActionTarget,
                    [FunctionParam::required(
                        "choice",
                        TypeKind::entity_ref(EntityKind::ChoiceOption),
                    )],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::ChoiceAction),
            ),
        ),
        (
            "viewport_point",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Named("ViewportPoint".to_owned()),
                    [
                        FunctionParam::required("x", TypeKind::U32),
                        FunctionParam::required("y", TypeKind::U32),
                    ],
                ),
                [],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::ViewportPoint),
            ),
        ),
        (
            "pointer.click",
            CallableSymbol::new(
                FunctionSignature::new(
                    agent_result(TypeKind::ActionResult),
                    [
                        FunctionParam::required(
                            "point",
                            TypeKind::Named("ViewportPoint".to_owned()),
                        ),
                        FunctionParam::defaulted("button", TypeKind::ActionName),
                    ],
                ),
                [EffectCapability::new("agent.act.physical")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::PointerClick),
            ),
        ),
        (
            "advance_text",
            CallableSymbol::new(
                FunctionSignature::new(agent_result(TypeKind::ActionResult), []),
                [EffectCapability::new("agent.act.semantic")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::AdvanceText),
            ),
        ),
        (
            "choose",
            CallableSymbol::new(
                FunctionSignature::new(
                    agent_result(TypeKind::ActionResult),
                    [FunctionParam::required(
                        "choice",
                        TypeKind::entity_ref(EntityKind::ChoiceOption),
                    )],
                ),
                [EffectCapability::new("agent.act.semantic")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Choose),
            ),
        ),
        (
            "invoke",
            CallableSymbol::new(
                FunctionSignature::new(
                    agent_result(TypeKind::ActionResult),
                    [
                        FunctionParam::required(
                            "target",
                            TypeKind::entity_ref(EntityKind::Other("_".to_owned())),
                        ),
                        FunctionParam::required("action", TypeKind::ActionName),
                        FunctionParam::required(
                            "args",
                            TypeKind::Map {
                                kind: MapKind::Sorted,
                                key: Box::new(TypeKind::String),
                                value: Box::new(TypeKind::AgentValue),
                            },
                        ),
                    ],
                ),
                [EffectCapability::new("agent.act.semantic")],
                CallableLowering::AgentIntrinsic(AgentIntrinsic::Invoke),
            ),
        ),
    ]
}

fn agent_capture_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
        (
            "capture",
            CallableSymbol::new(
                FunctionSignature::new(
                    agent_result(TypeKind::CaptureRef),
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

fn agent_resource_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![(
        "read_resource",
        CallableSymbol::new(
            FunctionSignature::new(
                agent_result(TypeKind::AgentResource),
                [FunctionParam::required("uri", TypeKind::String)],
            ),
            [EffectCapability::new("agent.resource.read")],
            CallableLowering::AgentIntrinsic(AgentIntrinsic::ReadResource),
        ),
    )]
}

fn agent_record_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![
        (
            "attach",
            CallableSymbol::new(
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required(
                        "resource",
                        agent_attach_resource_type(),
                    )],
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

fn agent_attach_resource_type() -> TypeKind {
    TypeKind::Choice(vec![TypeKind::CaptureRef, TypeKind::AgentResource])
}

fn agent_result(ok: TypeKind) -> TypeKind {
    TypeKind::Result {
        ok: Box::new(ok),
        error: Box::new(TypeKind::Named("AgentError".to_owned())),
    }
}

fn agent_rag_callables() -> Vec<(&'static str, CallableSymbol)> {
    vec![(
        "rag.query",
        CallableSymbol::new(
            FunctionSignature::new(
                TypeKind::Result {
                    ok: Box::new(TypeKind::RagContextPack),
                    error: Box::new(TypeKind::Named("RagError".to_owned())),
                },
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
    use arcweft_lang_hir::lower::lower_to_hir;
    use arcweft_lang_syntax::parser::parse_source;
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
            Some(&agent_result(TypeKind::ActionResult))
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

        let advance_text = prelude
            .get(&QualifiedName::new("advance_text"))
            .expect("advance_text intrinsic");
        assert_eq!(
            advance_text.lowering(),
            &CallableLowering::AgentIntrinsic(AgentIntrinsic::AdvanceText)
        );
        assert_eq!(
            advance_text
                .effects()
                .iter()
                .map(EffectCapability::as_str)
                .collect::<Vec<_>>(),
            vec!["agent.act.semantic"]
        );
    }

    #[test]
    fn project_index_from_hir_preserves_flow_and_signal_ref_value_types() {
        let tree = parse_source(
            r#"
signal @signal.current_flow: Watch<Ref<Flow>>
flow @flow.opening opening {
    return "ok"
}
"#,
        )
        .into_typed_tree();
        let hir = lower_to_hir(&tree).expect("source lowers to HIR");
        let index = project_semantic_index_from_hir(
            &hir,
            ProgramHash::new("program-a"),
            &SourceName::path("game.arcw"),
        )
        .expect("HIR indexes for Agent Script");

        assert_eq!(
            index.typecheck_env().symbol_type("flow.opening"),
            Some(&TypeKind::entity_ref(EntityKind::Flow))
        );
        assert_eq!(
            index.typecheck_env().symbol_type("signal.current_flow"),
            Some(&TypeKind::entity_ref_with_value(
                EntityKind::Signal,
                TypeKind::entity_ref(EntityKind::Flow)
            ))
        );
    }

    #[test]
    fn project_index_from_hir_projects_inline_image_agent_actions() {
        let tree = parse_source(
            r#"
asset @asset.bg.pulse {
    file = "bg/pulse.gif"
    kind = image
}

flow @flow.opening opening {
    let pulse = image(asset = @asset.bg.pulse, target = "target.sample.pulse", layer = "layer.foreground", x = 96px, y = 72px, width = 360px, height = 180px, action = "action.inspect.pulse")
}
"#,
        )
        .into_typed_tree();
        let hir = lower_to_hir(&tree).expect("source lowers to HIR");
        let index = project_semantic_index_from_hir(
            &hir,
            ProgramHash::new("program-a"),
            &SourceName::path("game.arcw"),
        )
        .expect("HIR indexes inline image actions");
        let env = index.typecheck_env();

        assert_eq!(
            env.symbol_type("target.sample.pulse"),
            Some(&TypeKind::entity_ref(EntityKind::Target))
        );
        let actions = env
            .agent_actions("target.sample.pulse")
            .expect("target exposes image action");
        assert_eq!(actions[0].action(), "action.inspect.pulse");
        assert_eq!(actions[0].return_type(), &TypeKind::ActionResult);
    }

    #[test]
    fn project_index_projects_agent_action_signatures() {
        let index = ProjectSemanticIndex::new(ProgramHash::new("program-a")).with_entity(
            EntitySymbol::new(
                public_id("activity.inventory"),
                EntityType::new(EntityKind::Activity, None),
                SourceAnchor::generated(),
                SemanticHash::new("shape.activity.inventory.v1"),
            )
            .with_agent_action(AgentActionSignature::new(
                QualifiedName::new("open"),
                [AgentActionParam::required("label", TypeKind::String)],
                TypeKind::ActionResult,
            )),
        );
        let env = index.typecheck_env();
        let actions = env
            .agent_actions("activity.inventory")
            .expect("agent action projected");

        assert_eq!(actions[0].action(), "open");
        assert_eq!(actions[0].params()[0].name(), "label");
        assert_eq!(actions[0].params()[0].ty(), &TypeKind::String);
        assert_eq!(actions[0].return_type(), &TypeKind::ActionResult);
        assert_eq!(
            env.function_signature("invoke")
                .map(FunctionSignature::return_type),
            Some(&agent_result(TypeKind::ActionResult))
        );
    }
}
