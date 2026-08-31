//! Generation-bound runtime reachability for one executable final-HIR project.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use thiserror::Error;

#[path = "runtime_semantic_owners/digest.rs"]
mod digest;
#[cfg(test)]
#[path = "runtime_semantic_owners/tests.rs"]
mod tests;
#[path = "runtime_semantic_owners/validation.rs"]
mod validation;

use self::digest::reachability_digest;
use self::validation::validate_roots_and_edges;

use super::{HirExecutableProjectView, selected_expressions::HirSelectedRuntimeExpressionOwners};
use crate::expr::HirExprKind;
use crate::identity::{
    CaptureId, ExprId, HirModuleId, HirSnapshotId, ItemId, LocalId, PatternId, ScopeId, StmtId,
    TypeId,
};
use crate::item::{HirEntryMember, HirImplMember, HirItemKind};
use crate::module::HirModule;
use crate::pattern::HirPatternChild;
use crate::scope::HirScopeOwner;
use crate::stmt::HirStatementChild;
use crate::symbol::{
    CallableDeclarationKey, ImplMethodDeclarationId, ProjectSymbolRevision, ProjectSymbolWorldId,
};

use super::{HirRuntimeExpressionProjection, HirSelectedExpressionInventoryError};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeEmissionMode {
    CheckAll,
    SelectedEntry,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeExecutableOwner {
    Item(ItemId),
    ImplMethod(ImplMethodDeclarationId),
    Closure(ExprId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeReachabilitySite {
    Item(ItemId),
    Expression(ExprId),
    Statement(StmtId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeReachabilityRootKind {
    CheckedFlow,
    CheckedEntry,
    SelectedEntry,
    CheckedViewValueProgram,
}

/// Exact standard-trait operation selected by one checked `for` witness.
///
/// This role is owned by final HIR because it is part of runtime dependency
/// reachability, not an authored call expression. Consumers must not infer it
/// from a method name or from the position of a row in a side table.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeIteratorWitnessMethodRole {
    IntoIterator,
    IteratorNext,
}

impl HirRuntimeIteratorWitnessMethodRole {
    pub(crate) const fn digest_tag(self) -> u8 {
        match self {
            Self::IntoIterator => 0,
            Self::IteratorNext => 1,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRuntimeReachabilityRoot {
    kind: HirRuntimeReachabilityRootKind,
    owner: HirRuntimeExecutableOwner,
}

impl HirRuntimeReachabilityRoot {
    pub const fn new(
        kind: HirRuntimeReachabilityRootKind,
        owner: HirRuntimeExecutableOwner,
    ) -> Self {
        Self { kind, owner }
    }

    pub const fn kind(&self) -> HirRuntimeReachabilityRootKind {
        self.kind
    }

    pub const fn owner(&self) -> &HirRuntimeExecutableOwner {
        &self.owner
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeReachabilityEdgeKind {
    CheckedProjectCall {
        call: ExprId,
        declaration: CallableDeclarationKey,
    },
    CheckedTraitMethodCall {
        call: ExprId,
        implementation: ItemId,
        method: ImplMethodDeclarationId,
    },
    CheckedIteratorWitnessMethod {
        role: HirRuntimeIteratorWitnessMethodRole,
        implementation: ItemId,
        member: u16,
        method: ImplMethodDeclarationId,
    },
    /// Executes the body and capture scope owned by one checked closure value.
    ///
    /// Closure bodies are executable boundaries: their locals must not enter
    /// the enclosing frame's structural closure. The owning closure expression
    /// therefore reaches its body through this exact checked edge instead of
    /// being reopened by a parent-scope traversal exception.
    CheckedClosureExecution { closure: ExprId },
    CheckedFlowTransfer {
        source: HirRuntimeReachabilitySite,
        declaration: CallableDeclarationKey,
    },
    CheckedEntryBinding {
        entry: ItemId,
        declaration: CallableDeclarationKey,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum HirRuntimeReachabilityEdgeAuthority {
    ProjectCall(CallableDeclarationKey),
    TraitMethodCall(ImplMethodDeclarationId),
    IteratorWitnessMethod(HirRuntimeIteratorWitnessMethodRole),
    ClosureExecution,
    FlowTransfer(CallableDeclarationKey),
    EntryBinding(CallableDeclarationKey),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRuntimeReachabilityEdge {
    source: HirRuntimeReachabilitySite,
    target: HirRuntimeExecutableOwner,
    kind: HirRuntimeReachabilityEdgeKind,
}

impl HirRuntimeReachabilityEdge {
    pub const fn new(
        source: HirRuntimeReachabilitySite,
        target: HirRuntimeExecutableOwner,
        kind: HirRuntimeReachabilityEdgeKind,
    ) -> Self {
        Self {
            source,
            target,
            kind,
        }
    }

    pub const fn source(&self) -> HirRuntimeReachabilitySite {
        self.source
    }

    pub const fn target(&self) -> &HirRuntimeExecutableOwner {
        &self.target
    }

    pub const fn kind(&self) -> &HirRuntimeReachabilityEdgeKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRuntimeReachabilityPath {
    root: HirRuntimeReachabilityRoot,
    steps: Box<[HirRuntimeReachabilityEdge]>,
}

impl HirRuntimeReachabilityPath {
    pub const fn root(&self) -> &HirRuntimeReachabilityRoot {
        &self.root
    }

    pub const fn steps(&self) -> &[HirRuntimeReachabilityEdge] {
        &self.steps
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRuntimeReachabilityDigest([u8; 32]);

impl HirRuntimeReachabilityDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirRuntimeReachabilityIdentity {
    module_snapshots: Box<[(HirModuleId, HirSnapshotId)]>,
    symbol_world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    mode: HirRuntimeEmissionMode,
    digest: HirRuntimeReachabilityDigest,
}

impl HirRuntimeReachabilityIdentity {
    pub const fn module_snapshots(&self) -> &[(HirModuleId, HirSnapshotId)] {
        &self.module_snapshots
    }

    pub const fn symbol_world(&self) -> &ProjectSymbolWorldId {
        &self.symbol_world
    }

    pub const fn symbol_revision(&self) -> ProjectSymbolRevision {
        self.symbol_revision
    }

    pub const fn mode(&self) -> HirRuntimeEmissionMode {
        self.mode
    }

    pub const fn digest(&self) -> HirRuntimeReachabilityDigest {
        self.digest
    }
}

pub struct HirRuntimeSemanticReachabilityInput {
    mode: HirRuntimeEmissionMode,
    symbol_world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    roots: Vec<HirRuntimeReachabilityRoot>,
    edges: Vec<HirRuntimeReachabilityEdge>,
}

impl HirRuntimeSemanticReachabilityInput {
    pub fn try_new(
        mode: HirRuntimeEmissionMode,
        symbol_world: ProjectSymbolWorldId,
        symbol_revision: ProjectSymbolRevision,
        mut roots: Vec<HirRuntimeReachabilityRoot>,
        mut edges: Vec<HirRuntimeReachabilityEdge>,
    ) -> Result<Self, HirRuntimeReachabilityError> {
        roots.sort();
        if let Some(root) = roots
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then(|| pair[0].clone()))
        {
            return Err(HirRuntimeReachabilityError::DuplicateRoot { root });
        }
        edges.sort();
        if let Some(edge) = edges
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then(|| pair[0].clone()))
        {
            return Err(HirRuntimeReachabilityError::DuplicateEdge {
                edge: Box::new(edge),
            });
        }
        let mut iterator_witness_roles = BTreeSet::new();
        for edge in &edges {
            let HirRuntimeReachabilityEdgeKind::CheckedIteratorWitnessMethod { role, .. } =
                &edge.kind
            else {
                continue;
            };
            if !iterator_witness_roles.insert((edge.source, *role)) {
                return Err(
                    HirRuntimeReachabilityError::DuplicateIteratorWitnessMethodRole {
                        site: edge.source,
                        role: *role,
                    },
                );
            }
        }
        let mut authority_targets = BTreeMap::new();
        for edge in &edges {
            let key = (edge.source, edge_authority(&edge.kind));
            if let Some(first) = authority_targets.insert(key, edge.target.clone())
                && first != edge.target
            {
                return Err(HirRuntimeReachabilityError::ConflictingEdge {
                    site: edge.source,
                    first: Box::new(first),
                    second: Box::new(edge.target.clone()),
                });
            }
        }
        for edge in &edges {
            if !edge_kind_matches_source(edge) {
                return Err(HirRuntimeReachabilityError::InvalidEdgeKind {
                    site: edge.source,
                    kind: Box::new(edge.kind.clone()),
                });
            }
        }
        if roots.len() > u32::MAX as usize {
            return Err(HirRuntimeReachabilityError::LimitExceeded {
                family: HirRuntimeReachabilityLimitFamily::Roots,
                actual: roots.len(),
                limit: u32::MAX as usize,
            });
        }
        if edges.len() > u32::MAX as usize {
            return Err(HirRuntimeReachabilityError::LimitExceeded {
                family: HirRuntimeReachabilityLimitFamily::Edges,
                actual: edges.len(),
                limit: u32::MAX as usize,
            });
        }
        Ok(Self {
            mode,
            symbol_world,
            symbol_revision,
            roots,
            edges,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeReachabilityLimitFamily {
    Roots,
    Edges,
    Executables,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirRuntimeReachabilityError {
    #[error("runtime reachability symbol world does not match the executable project")]
    SymbolWorldMismatch,
    #[error("runtime reachability topology does not lease the exact input and HIR generation")]
    TopologyGenerationMismatch,
    #[error("runtime reachability root references an unknown executable owner")]
    UnknownRoot { owner: HirRuntimeExecutableOwner },
    #[error("runtime reachability root kind does not match its executable owner")]
    InvalidRootKind { root: HirRuntimeReachabilityRoot },
    #[error("runtime reachability edge source is unresolved")]
    UnknownEdgeSource { site: HirRuntimeReachabilitySite },
    #[error("runtime reachability edge target {target:?} is unresolved")]
    UnknownEdgeTarget { target: HirRuntimeExecutableOwner },
    #[error("runtime reachability edge targets a presentation product")]
    PresentationTarget { target: HirRuntimeExecutableOwner },
    #[error("runtime reachability contains a duplicate root")]
    DuplicateRoot { root: HirRuntimeReachabilityRoot },
    #[error("runtime reachability contains a duplicate edge")]
    DuplicateEdge {
        edge: Box<HirRuntimeReachabilityEdge>,
    },
    #[error("runtime reachability repeats one checked iterator-witness method role")]
    DuplicateIteratorWitnessMethodRole {
        site: HirRuntimeReachabilitySite,
        role: HirRuntimeIteratorWitnessMethodRole,
    },
    #[error("runtime reachability contains conflicting edges for one checked source")]
    ConflictingEdge {
        site: HirRuntimeReachabilitySite,
        first: Box<HirRuntimeExecutableOwner>,
        second: Box<HirRuntimeExecutableOwner>,
    },
    #[error("runtime reachability source does not match its edge kind")]
    InvalidEdgeKind {
        site: HirRuntimeReachabilitySite,
        kind: Box<HirRuntimeReachabilityEdgeKind>,
    },
    #[error("runtime reachability edge kind does not match its executable target")]
    InvalidEdgeTarget {
        edge: Box<HirRuntimeReachabilityEdge>,
    },
    #[error("runtime reachability references an unresolved scope")]
    UnresolvedScope { scope: ScopeId },
    #[error("runtime reachability references an unresolved local")]
    UnresolvedLocal { local: LocalId },
    #[error("runtime reachability references an unresolved expression")]
    UnresolvedExpression { expression: ExprId },
    #[error("runtime reachability references an unresolved statement")]
    UnresolvedStatement { statement: StmtId },
    #[error("runtime reachability references an unresolved type")]
    UnresolvedType { ty: TypeId },
    #[error("runtime reachability references an unresolved pattern")]
    UnresolvedPattern { pattern: PatternId },
    #[error("runtime reachability exceeds the accepted graph limit")]
    LimitExceeded {
        family: HirRuntimeReachabilityLimitFamily,
        actual: usize,
        limit: usize,
    },
    #[error(transparent)]
    SelectedExpressions(#[from] HirSelectedExpressionInventoryError),
}

pub struct HirRuntimeSemanticReachability<'project> {
    pub(super) project: HirExecutableProjectView<'project>,
    mode: HirRuntimeEmissionMode,
    roots: Box<[HirRuntimeReachabilityRoot]>,
    edges: Box<[HirRuntimeReachabilityEdge]>,
    reachable_executables: BTreeSet<HirRuntimeExecutableOwner>,
    first_paths: BTreeMap<HirRuntimeExecutableOwner, HirRuntimeReachabilityPath>,
    locals: Box<[LocalId]>,
    expressions: BTreeSet<ExprId>,
    expression_type_owners: BTreeSet<ExprId>,
    statements: BTreeSet<StmtId>,
    types: BTreeSet<TypeId>,
    patterns: BTreeSet<PatternId>,
    captures: BTreeSet<CaptureId>,
    identity: HirRuntimeReachabilityIdentity,
}

impl HirRuntimeSemanticReachability<'_> {
    pub const fn project(&self) -> HirExecutableProjectView<'_> {
        self.project
    }

    pub const fn mode(&self) -> HirRuntimeEmissionMode {
        self.mode
    }

    pub const fn identity(&self) -> &HirRuntimeReachabilityIdentity {
        &self.identity
    }

    pub fn roots(&self) -> impl ExactSizeIterator<Item = &HirRuntimeReachabilityRoot> {
        self.roots.iter()
    }

    pub fn edges(&self) -> impl ExactSizeIterator<Item = &HirRuntimeReachabilityEdge> {
        self.edges.iter()
    }

    pub fn reachable_executables(
        &self,
    ) -> impl ExactSizeIterator<Item = &HirRuntimeExecutableOwner> {
        self.reachable_executables.iter()
    }

    pub fn first_path(
        &self,
        owner: &HirRuntimeExecutableOwner,
    ) -> Option<&HirRuntimeReachabilityPath> {
        self.first_paths.get(owner)
    }

    pub fn edge_from(
        &self,
        source: HirRuntimeReachabilitySite,
    ) -> impl Iterator<Item = &HirRuntimeReachabilityEdge> {
        self.edges.iter().filter(move |edge| edge.source == source)
    }

    pub fn locals(&self) -> impl ExactSizeIterator<Item = LocalId> + '_ {
        self.locals.iter().copied()
    }

    pub fn patterns(&self) -> impl ExactSizeIterator<Item = PatternId> + '_ {
        self.patterns.iter().copied()
    }

    pub fn contains_runtime_owner(&self, owner: &HirRuntimeExecutableOwner) -> bool {
        self.reachable_executables.contains(owner)
    }

    pub fn contains_local(&self, owner: LocalId) -> bool {
        self.locals.binary_search(&owner).is_ok()
    }

    pub fn contains_expression(&self, owner: ExprId) -> bool {
        self.expressions.contains(&owner)
    }

    pub fn contains_statement(&self, owner: StmtId) -> bool {
        self.statements.contains(&owner)
    }

    pub fn contains_type(&self, owner: TypeId) -> bool {
        self.types.contains(&owner)
    }

    pub fn contains_pattern(&self, owner: PatternId) -> bool {
        self.patterns.contains(&owner)
    }

    pub fn contains_capture(&self, owner: CaptureId) -> bool {
        self.captures.contains(&owner)
    }

    pub(super) const fn expression_type_owners(&self) -> &BTreeSet<ExprId> {
        &self.expression_type_owners
    }
}

#[derive(Clone)]
struct ScopeEdges {
    owner: HirScopeOwner,
    children: Box<[ScopeId]>,
    locals: Box<[LocalId]>,
}

#[derive(Default)]
struct ScopedOwners {
    expressions: Vec<ExprId>,
    statements: Vec<StmtId>,
    types: Vec<TypeId>,
    patterns: Vec<PatternId>,
}

struct StructuralIndex {
    scopes: BTreeMap<ScopeId, ScopeEdges>,
    scope_members: BTreeMap<ScopeId, ScopedOwners>,
    local_types: BTreeMap<LocalId, Option<TypeId>>,
    expression_edges: BTreeMap<ExprId, (Vec<ExprId>, Vec<TypeId>, bool)>,
    statement_edges: BTreeMap<StmtId, Vec<HirStatementChild>>,
    type_edges: BTreeMap<TypeId, Vec<TypeId>>,
    pattern_edges: BTreeMap<PatternId, Vec<HirPatternChild>>,
    owned_scopes: BTreeMap<HirScopeOwner, Vec<ScopeId>>,
    capture_closures: BTreeMap<CaptureId, ExprId>,
    closure_captures: BTreeMap<ExprId, Vec<(CaptureId, LocalId)>>,
}

#[derive(Default)]
struct StructuralOwners {
    locals: BTreeSet<LocalId>,
    expressions: BTreeSet<ExprId>,
    statements: BTreeSet<StmtId>,
    types: BTreeSet<TypeId>,
    patterns: BTreeSet<PatternId>,
    captures: BTreeSet<CaptureId>,
}

#[derive(Clone, Copy)]
enum PendingOwner {
    Scope(ScopeId),
    Local(LocalId),
    Expression(ExprId),
    Statement(StmtId),
    Type(TypeId),
    Pattern(PatternId),
}

impl From<HirStatementChild> for PendingOwner {
    fn from(child: HirStatementChild) -> Self {
        match child {
            HirStatementChild::Expression(owner) => Self::Expression(owner),
            HirStatementChild::Statement(owner) => Self::Statement(owner),
            HirStatementChild::Pattern(owner) => Self::Pattern(owner),
            HirStatementChild::Type(owner) => Self::Type(owner),
            HirStatementChild::Local(owner) => Self::Local(owner),
        }
    }
}

impl From<HirPatternChild> for PendingOwner {
    fn from(child: HirPatternChild) -> Self {
        match child {
            HirPatternChild::Pattern(owner) => Self::Pattern(owner),
            HirPatternChild::Type(owner) => Self::Type(owner),
            HirPatternChild::Local(owner) => Self::Local(owner),
        }
    }
}

#[derive(Default)]
struct HirRuntimeExecutionRoots {
    scopes: Vec<ScopeId>,
    expressions: Vec<ExprId>,
    types: Vec<TypeId>,
}

impl HirItemKind {
    fn runtime_execution_roots(&self) -> Option<HirRuntimeExecutionRoots> {
        match self {
            Self::Flow(flow) => Some(HirRuntimeExecutionRoots {
                scopes: vec![flow.callable_scope()],
                types: flow.result().authored_type().into_iter().collect(),
                ..HirRuntimeExecutionRoots::default()
            }),
            Self::Function(function) => Some(HirRuntimeExecutionRoots {
                scopes: vec![function.callable_scope()],
                types: function.return_type().into_iter().collect(),
                ..HirRuntimeExecutionRoots::default()
            }),
            Self::Entry(entry) => {
                let mut roots = HirRuntimeExecutionRoots::default();
                for member in entry.members() {
                    match member {
                        HirEntryMember::StateType(binding) | HirEntryMember::EventType(binding) => {
                            roots.types.push(binding.ty());
                        }
                        HirEntryMember::Option(option) => {
                            roots.expressions.extend(option.value().expression());
                        }
                        HirEntryMember::Initializer(_)
                        | HirEntryMember::Reducer(_)
                        | HirEntryMember::Controller(_)
                        | HirEntryMember::Goto(_)
                        | HirEntryMember::Route(_)
                        | HirEntryMember::Error => {}
                    }
                }
                Some(roots)
            }
            Self::Module(_)
            | Self::Use(_)
            | Self::Predicate(_)
            | Self::Proof(_)
            | Self::Trait(_)
            | Self::Impl(_)
            | Self::Enum(_)
            | Self::Struct(_)
            | Self::TypeAlias(_)
            | Self::Resource(_)
            | Self::Character(_)
            | Self::View(_)
            | Self::Action(_)
            | Self::Activity(_)
            | Self::Signal(_)
            | Self::Metric(_)
            | Self::Layer(_)
            | Self::ExternCapability(_)
            | Self::Test(_)
            | Self::Bench(_)
            | Self::Style(_)
            | Self::Error(_) => None,
        }
    }
}

impl<'project> HirExecutableProjectView<'project> {
    #[expect(
        clippy::too_many_lines,
        reason = "one atomic transaction validates the generation, closes structural owners, and records deterministic paths"
    )]
    pub fn runtime_semantic_reachability(
        self,
        input: HirRuntimeSemanticReachabilityInput,
        topology: &super::HirProjectEvaluationTopology,
        mut selected_postfix: impl FnMut(ExprId) -> Option<ExprId>,
        mut expression_projection: impl FnMut(ExprId) -> Option<HirRuntimeExpressionProjection>,
    ) -> Result<HirRuntimeSemanticReachability<'project>, HirRuntimeReachabilityError> {
        self.validate_reachability_generation(&input, topology)?;
        validate_roots_and_edges(self, &input)?;
        let index = StructuralIndex::new(self);
        let mut reachable_executables = BTreeSet::new();
        let mut first_paths = BTreeMap::new();
        let mut accepted = StructuralOwners::default();
        let mut expression_type_owners = BTreeSet::new();
        let mut pending = input
            .roots
            .iter()
            .cloned()
            .map(|root| {
                let owner = root.owner.clone();
                let path = HirRuntimeReachabilityPath {
                    root,
                    steps: Box::new([]),
                };
                (owner, path)
            })
            .collect::<VecDeque<_>>();

        while let Some((owner, path)) = pending.pop_front() {
            if !reachable_executables.insert(owner.clone()) {
                continue;
            }
            first_paths.insert(owner.clone(), path.clone());
            let (structural, execution_expression_roots) = self.close_executable(&index, &owner)?;
            let HirSelectedRuntimeExpressionOwners { reached, typed } = self
                .selected_runtime_expression_owners(
                    topology,
                    &structural.expressions,
                    &execution_expression_roots,
                    &mut selected_postfix,
                    &mut expression_projection,
                )?;
            expression_type_owners.extend(typed);
            accepted.locals.extend(structural.locals);
            accepted.expressions.extend(reached.iter().copied());
            accepted
                .statements
                .extend(structural.statements.iter().copied());
            accepted.types.extend(structural.types);
            accepted.patterns.extend(structural.patterns);
            accepted.captures.extend(structural.captures);

            for edge in input.edges.iter().filter(|edge| {
                edge_reached_by(&owner, edge.source, &reached, &structural.statements)
            }) {
                let mut steps = path.steps.to_vec();
                steps.push(edge.clone());
                pending.push_back((
                    edge.target.clone(),
                    HirRuntimeReachabilityPath {
                        root: path.root.clone(),
                        steps: steps.into_boxed_slice(),
                    },
                ));
            }
        }

        if reachable_executables.len() > u32::MAX as usize {
            return Err(HirRuntimeReachabilityError::LimitExceeded {
                family: HirRuntimeReachabilityLimitFamily::Executables,
                actual: reachable_executables.len(),
                limit: u32::MAX as usize,
            });
        }

        let module_snapshots = self
            .modules()
            .map(|(_, module)| (module.module_id(), module.snapshot_id()))
            .collect::<Box<[_]>>();
        let locals = accepted.locals.iter().copied().collect::<Box<[_]>>();
        let digest = reachability_digest(
            input.mode,
            &module_snapshots,
            &input.symbol_world,
            input.symbol_revision,
            &input.roots,
            &input.edges,
            &reachable_executables,
            &locals,
            &accepted,
        );
        let identity = HirRuntimeReachabilityIdentity {
            module_snapshots,
            symbol_world: input.symbol_world,
            symbol_revision: input.symbol_revision,
            mode: input.mode,
            digest,
        };
        Ok(HirRuntimeSemanticReachability {
            project: self,
            mode: input.mode,
            roots: input.roots.into_boxed_slice(),
            edges: input.edges.into_boxed_slice(),
            reachable_executables,
            first_paths,
            locals,
            expressions: accepted.expressions,
            expression_type_owners,
            statements: accepted.statements,
            types: accepted.types,
            patterns: accepted.patterns,
            captures: accepted.captures,
            identity,
        })
    }

    fn validate_reachability_generation(
        self,
        input: &HirRuntimeSemanticReachabilityInput,
        topology: &super::HirProjectEvaluationTopology,
    ) -> Result<(), HirRuntimeReachabilityError> {
        if input.symbol_world.package() != self.package() {
            return Err(HirRuntimeReachabilityError::SymbolWorldMismatch);
        }
        let generation = topology.generation();
        if generation.symbol_world() != &input.symbol_world
            || generation.symbol_revision() != input.symbol_revision
            || generation.validate_executable_lease(self).is_err()
        {
            return Err(HirRuntimeReachabilityError::TopologyGenerationMismatch);
        }
        Ok(())
    }

    fn close_executable(
        self,
        index: &StructuralIndex,
        owner: &HirRuntimeExecutableOwner,
    ) -> Result<(StructuralOwners, Vec<ExprId>), HirRuntimeReachabilityError> {
        let roots = execution_roots(self, owner)?;
        let expression_roots = roots.expressions.clone();
        let active_closure = match owner {
            HirRuntimeExecutableOwner::Closure(expression) => Some(*expression),
            HirRuntimeExecutableOwner::Item(_) | HirRuntimeExecutableOwner::ImplMethod(_) => None,
        };
        index
            .close(roots, active_closure)
            .map(|owners| (owners, expression_roots))
    }
}

impl StructuralIndex {
    fn new(project: HirExecutableProjectView<'_>) -> Self {
        let mut index = Self {
            scopes: BTreeMap::new(),
            scope_members: BTreeMap::new(),
            local_types: BTreeMap::new(),
            expression_edges: BTreeMap::new(),
            statement_edges: BTreeMap::new(),
            type_edges: BTreeMap::new(),
            pattern_edges: BTreeMap::new(),
            owned_scopes: BTreeMap::new(),
            capture_closures: BTreeMap::new(),
            closure_captures: BTreeMap::new(),
        };
        for (_, module) in project.modules() {
            Self::index_module(&mut index, module);
        }
        index
    }

    fn index_module(index: &mut Self, module: &HirModule) {
        for (owner, scope) in module.scopes() {
            index.scopes.insert(
                owner,
                ScopeEdges {
                    owner: *scope.owner(),
                    children: scope.children().into(),
                    locals: scope.locals().into(),
                },
            );
            index
                .owned_scopes
                .entry(*scope.owner())
                .or_default()
                .push(owner);
        }
        for (owner, local) in module.locals() {
            index.local_types.insert(owner, local.annotation());
        }
        for (owner, expression) in module.expressions() {
            index
                .scope_members
                .entry(expression.scope())
                .or_default()
                .expressions
                .push(owner);
            index.expression_edges.insert(
                owner,
                (
                    expression.kind().direct_expression_children(),
                    expression.kind().direct_type_roots(),
                    matches!(expression.kind(), HirExprKind::Closure(_)),
                ),
            );
        }
        for (owner, statement) in module.statements() {
            index
                .scope_members
                .entry(statement.scope())
                .or_default()
                .statements
                .push(owner);
            index.statement_edges.insert(
                owner,
                statement
                    .kind()
                    .child_edges()
                    .into_iter()
                    .map(|edge| edge.child())
                    .collect(),
            );
        }
        for (owner, ty) in module.types() {
            index
                .scope_members
                .entry(ty.scope())
                .or_default()
                .types
                .push(owner);
            index
                .type_edges
                .insert(owner, ty.kind().direct_type_children());
        }
        for (owner, pattern) in module.patterns() {
            index
                .scope_members
                .entry(pattern.scope())
                .or_default()
                .patterns
                .push(owner);
            index.pattern_edges.insert(
                owner,
                pattern
                    .kind()
                    .child_edges()
                    .into_iter()
                    .map(|edge| edge.child())
                    .collect(),
            );
        }
        for (owner, capture) in module.captures() {
            index.capture_closures.insert(owner, capture.closure());
            index
                .closure_captures
                .entry(capture.closure())
                .or_default()
                .push((owner, capture.local()));
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the structural closure exhaustively walks every typed final-HIR owner family"
    )]
    fn close(
        &self,
        roots: HirRuntimeExecutionRoots,
        active_closure: Option<ExprId>,
    ) -> Result<StructuralOwners, HirRuntimeReachabilityError> {
        let mut pending = VecDeque::new();
        pending.extend(roots.scopes.into_iter().map(PendingOwner::Scope));
        pending.extend(roots.expressions.into_iter().map(PendingOwner::Expression));
        pending.extend(roots.types.into_iter().map(PendingOwner::Type));
        let mut owners = StructuralOwners::default();
        let mut scopes = BTreeSet::new();

        while let Some(owner) = pending.pop_front() {
            match owner {
                PendingOwner::Scope(owner) => {
                    if !scopes.insert(owner) {
                        continue;
                    }
                    let edges = self
                        .scopes
                        .get(&owner)
                        .ok_or(HirRuntimeReachabilityError::UnresolvedScope { scope: owner })?;
                    for child in edges.children.iter().copied() {
                        if self.scope_is_inactive_closure(child, active_closure) {
                            continue;
                        }
                        pending.push_back(PendingOwner::Scope(child));
                    }
                    pending.extend(edges.locals.iter().copied().map(PendingOwner::Local));
                    if let Some(members) = self.scope_members.get(&owner) {
                        pending.extend(
                            members
                                .expressions
                                .iter()
                                .copied()
                                .map(PendingOwner::Expression),
                        );
                        pending.extend(
                            members
                                .statements
                                .iter()
                                .copied()
                                .map(PendingOwner::Statement),
                        );
                        pending.extend(members.types.iter().copied().map(PendingOwner::Type));
                        pending.extend(members.patterns.iter().copied().map(PendingOwner::Pattern));
                    }
                }
                PendingOwner::Local(owner) => {
                    if !owners.locals.insert(owner) {
                        continue;
                    }
                    let ty = self
                        .local_types
                        .get(&owner)
                        .ok_or(HirRuntimeReachabilityError::UnresolvedLocal { local: owner })?;
                    pending.extend(ty.iter().copied().map(PendingOwner::Type));
                }
                PendingOwner::Expression(owner) => {
                    if !owners.expressions.insert(owner) {
                        continue;
                    }
                    let (expressions, types, is_closure) =
                        self.expression_edges.get(&owner).ok_or(
                            HirRuntimeReachabilityError::UnresolvedExpression { expression: owner },
                        )?;
                    if !*is_closure || active_closure == Some(owner) {
                        pending.extend(expressions.iter().copied().map(PendingOwner::Expression));
                        pending.extend(
                            self.owned_scopes
                                .get(&HirScopeOwner::Expr(owner))
                                .into_iter()
                                .flatten()
                                .copied()
                                .map(PendingOwner::Scope),
                        );
                    }
                    if active_closure == Some(owner) {
                        pending.extend(
                            self.closure_captures
                                .get(&owner)
                                .into_iter()
                                .flatten()
                                .map(|(_, local)| PendingOwner::Local(*local)),
                        );
                    }
                    pending.extend(types.iter().copied().map(PendingOwner::Type));
                }
                PendingOwner::Statement(owner) => {
                    if !owners.statements.insert(owner) {
                        continue;
                    }
                    let children = self.statement_edges.get(&owner).ok_or(
                        HirRuntimeReachabilityError::UnresolvedStatement { statement: owner },
                    )?;
                    pending.extend(children.iter().copied().map(PendingOwner::from));
                    pending.extend(
                        self.owned_scopes
                            .get(&HirScopeOwner::Stmt(owner))
                            .into_iter()
                            .flatten()
                            .copied()
                            .map(PendingOwner::Scope),
                    );
                }
                PendingOwner::Type(owner) => {
                    if !owners.types.insert(owner) {
                        continue;
                    }
                    let children = self
                        .type_edges
                        .get(&owner)
                        .ok_or(HirRuntimeReachabilityError::UnresolvedType { ty: owner })?;
                    pending.extend(children.iter().copied().map(PendingOwner::Type));
                }
                PendingOwner::Pattern(owner) => {
                    if !owners.patterns.insert(owner) {
                        continue;
                    }
                    let children = self
                        .pattern_edges
                        .get(&owner)
                        .ok_or(HirRuntimeReachabilityError::UnresolvedPattern { pattern: owner })?;
                    pending.extend(children.iter().copied().map(PendingOwner::from));
                }
            }
        }
        owners.captures.extend(
            self.capture_closures
                .iter()
                .filter_map(|(capture, closure)| {
                    owners.expressions.contains(closure).then_some(*capture)
                }),
        );
        Ok(owners)
    }

    fn scope_is_inactive_closure(&self, scope: ScopeId, active_closure: Option<ExprId>) -> bool {
        let Some(edges) = self.scopes.get(&scope) else {
            return false;
        };
        let HirScopeOwner::Expr(owner) = edges.owner else {
            return false;
        };
        self.expression_edges
            .get(&owner)
            .is_some_and(|(_, _, is_closure)| *is_closure && active_closure != Some(owner))
    }
}

fn execution_roots(
    project: HirExecutableProjectView<'_>,
    owner: &HirRuntimeExecutableOwner,
) -> Result<HirRuntimeExecutionRoots, HirRuntimeReachabilityError> {
    match owner {
        HirRuntimeExecutableOwner::Item(owner) => {
            let kind = resolve_item_kind(project, *owner).ok_or({
                HirRuntimeReachabilityError::UnknownRoot {
                    owner: HirRuntimeExecutableOwner::Item(*owner),
                }
            })?;
            if matches!(kind, HirItemKind::View(_) | HirItemKind::Style(_)) {
                return Err(HirRuntimeReachabilityError::PresentationTarget {
                    target: HirRuntimeExecutableOwner::Item(*owner),
                });
            }
            kind.runtime_execution_roots()
                .ok_or(HirRuntimeReachabilityError::UnknownRoot {
                    owner: HirRuntimeExecutableOwner::Item(*owner),
                })
        }
        HirRuntimeExecutableOwner::Closure(owner) => {
            let module = project
                .modules()
                .find_map(|(_, module)| {
                    (module.module_id() == owner.module()).then_some(module.as_ref())
                })
                .ok_or(HirRuntimeReachabilityError::UnresolvedExpression { expression: *owner })?;
            let expression = module.resolve_expr(*owner).map_err(|_| {
                HirRuntimeReachabilityError::UnresolvedExpression { expression: *owner }
            })?;
            if !matches!(expression.kind(), HirExprKind::Closure(_)) {
                return Err(HirRuntimeReachabilityError::UnknownRoot {
                    owner: HirRuntimeExecutableOwner::Closure(*owner),
                });
            }
            Ok(HirRuntimeExecutionRoots {
                expressions: vec![*owner],
                ..HirRuntimeExecutionRoots::default()
            })
        }
        HirRuntimeExecutableOwner::ImplMethod(method) => impl_method_roots(project, method),
    }
}

fn impl_method_roots(
    project: HirExecutableProjectView<'_>,
    method: &ImplMethodDeclarationId,
) -> Result<HirRuntimeExecutionRoots, HirRuntimeReachabilityError> {
    let implementation = method.implementation();
    let module = project
        .modules()
        .find_map(|(path, module)| (path == implementation.module()).then_some(module.as_ref()))
        .ok_or_else(|| HirRuntimeReachabilityError::UnknownRoot {
            owner: HirRuntimeExecutableOwner::ImplMethod(method.clone()),
        })?;
    let source_ordinal = usize::try_from(implementation.source_ordinal()).ok();
    let Some(implementation) = source_ordinal.and_then(|ordinal| {
        module
            .items()
            .filter_map(|(_, item)| match item.kind() {
                HirItemKind::Impl(implementation) => Some(implementation),
                _ => None,
            })
            .nth(ordinal)
    }) else {
        return Err(HirRuntimeReachabilityError::UnknownRoot {
            owner: HirRuntimeExecutableOwner::ImplMethod(method.clone()),
        });
    };
    let function = implementation
        .members()
        .iter()
        .find_map(|member| match member {
            HirImplMember::Function(function)
                if function
                    .name()
                    .resolved()
                    .is_some_and(|name| name.as_str() == method.method().as_str()) =>
            {
                Some(function)
            }
            HirImplMember::AssociatedType(_)
            | HirImplMember::Function(_)
            | HirImplMember::Error => None,
        });
    let Some(function) = function else {
        return Err(HirRuntimeReachabilityError::UnknownRoot {
            owner: HirRuntimeExecutableOwner::ImplMethod(method.clone()),
        });
    };
    Ok(HirRuntimeExecutionRoots {
        scopes: vec![function.callable_scope()],
        types: function.return_type().into_iter().collect(),
        ..HirRuntimeExecutionRoots::default()
    })
}

fn resolve_item_kind(project: HirExecutableProjectView<'_>, owner: ItemId) -> Option<&HirItemKind> {
    project
        .modules()
        .find_map(|(_, module)| (module.module_id() == owner.module()).then_some(module.as_ref()))?
        .resolve_item(owner)
        .ok()
        .map(crate::item::HirItem::kind)
}

fn edge_kind_matches_source(edge: &HirRuntimeReachabilityEdge) -> bool {
    match (&edge.source, &edge.kind) {
        (
            HirRuntimeReachabilitySite::Expression(source),
            HirRuntimeReachabilityEdgeKind::CheckedProjectCall { call, .. }
            | HirRuntimeReachabilityEdgeKind::CheckedTraitMethodCall { call, .. },
        ) => source == call,
        (
            HirRuntimeReachabilitySite::Statement(_),
            HirRuntimeReachabilityEdgeKind::CheckedIteratorWitnessMethod { .. },
        ) => true,
        (
            HirRuntimeReachabilitySite::Expression(source),
            HirRuntimeReachabilityEdgeKind::CheckedClosureExecution { closure },
        ) => source == closure,
        (
            source,
            HirRuntimeReachabilityEdgeKind::CheckedFlowTransfer {
                source: transfer, ..
            },
        ) => source == transfer,
        (
            HirRuntimeReachabilitySite::Item(owner),
            HirRuntimeReachabilityEdgeKind::CheckedEntryBinding { entry, .. },
        ) => owner == entry,
        _ => false,
    }
}

fn edge_authority(kind: &HirRuntimeReachabilityEdgeKind) -> HirRuntimeReachabilityEdgeAuthority {
    match kind {
        HirRuntimeReachabilityEdgeKind::CheckedProjectCall { declaration, .. } => {
            HirRuntimeReachabilityEdgeAuthority::ProjectCall(declaration.clone())
        }
        HirRuntimeReachabilityEdgeKind::CheckedTraitMethodCall { method, .. } => {
            HirRuntimeReachabilityEdgeAuthority::TraitMethodCall(method.clone())
        }
        HirRuntimeReachabilityEdgeKind::CheckedIteratorWitnessMethod { role, .. } => {
            HirRuntimeReachabilityEdgeAuthority::IteratorWitnessMethod(*role)
        }
        HirRuntimeReachabilityEdgeKind::CheckedClosureExecution { .. } => {
            HirRuntimeReachabilityEdgeAuthority::ClosureExecution
        }
        HirRuntimeReachabilityEdgeKind::CheckedFlowTransfer { declaration, .. } => {
            HirRuntimeReachabilityEdgeAuthority::FlowTransfer(declaration.clone())
        }
        HirRuntimeReachabilityEdgeKind::CheckedEntryBinding { declaration, .. } => {
            HirRuntimeReachabilityEdgeAuthority::EntryBinding(declaration.clone())
        }
    }
}

fn edge_reached_by(
    owner: &HirRuntimeExecutableOwner,
    source: HirRuntimeReachabilitySite,
    expressions: &BTreeSet<ExprId>,
    statements: &BTreeSet<StmtId>,
) -> bool {
    match source {
        HirRuntimeReachabilitySite::Item(item) => {
            matches!(owner, HirRuntimeExecutableOwner::Item(owner) if *owner == item)
        }
        HirRuntimeReachabilitySite::Expression(expression) => expressions.contains(&expression),
        HirRuntimeReachabilitySite::Statement(statement) => statements.contains(&statement),
    }
}
