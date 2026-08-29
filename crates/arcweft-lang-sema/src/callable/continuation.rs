//! Prepared call-graph ownership and the sole lower-initialization issuer.
//!
//! A prepared call is deliberately split from the final checked call.  During
//! body analysis this module owns the generation-local graph, continuation
//! references, and the affine seed used by one lower constraint run.  No
//! caller can construct a lower initialization token from a scope and an
//! inherited solution pair.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_lang_hir::identity::{ExprId, LocalId};
use thiserror::Error;

use crate::effect_row::{EffectConstraintEligibility, EffectConstraintVariable};

use crate::types::{
    GenericConstParameterId, GenericTypeParameterId, TypeKind,
    constraints::{
        TypeConstraintConstEligibility, TypeConstraintEffectScope, TypeConstraintInvariant,
        TypeConstraintParameterEligibility, TypeConstraintParameterScope, TypeConstraintRejection,
        TypeConstraintSolution,
        context::{TypeConstraintConstParameterScopeRow, TypeConstraintTypeParameterScopeRow},
    },
};

use super::{CallableGenericFirstUse, CallableGroupIndex};

/// A checked call site is generation-local evidence, not a stable digest input.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedCallSite {
    HirCall(ExprId),
    DialogueApplication(ExprId),
}

impl CheckedCallSite {
    pub const fn expression(self) -> ExprId {
        match self {
            Self::HirCall(expression) | Self::DialogueApplication(expression) => expression,
        }
    }
}

/// Typed preparation/restore failures.  These are internal authority
/// violations; ordinary argument/type incompatibilities remain candidate
/// rejection in the lower algebra.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum CallConstraintInvariant {
    #[error("call argument mapping was not sealed by its producer")]
    MalformedMapperSeal,
    #[error("callable generic schema inventory is malformed")]
    MalformedSchemaInventory,
    #[error("prepared graph issuer is foreign")]
    ForeignPreparedIssuer,
    #[error("prepared graph node is missing or stale")]
    MissingOrStalePreparedNode,
    #[error("prepared graph node is in an invalid state")]
    InvalidPreparedNodeState,
    #[error("prepared graph dependency order is invalid")]
    InvalidPreparedDependencyOrder,
    #[error("prepared graph delta is stale or already closed")]
    PreparedGraphDeltaStale,
    #[error("prepared graph has already crossed its consuming seal boundary")]
    PreparedGraphConsumed,
    #[error("prepared graph delta is not the active LIFO transaction")]
    PreparedGraphDeltaOrder,
    #[error("prepared graph site already has a node")]
    PreparedGraphDuplicateSite,
    #[error("prepared graph replay does not match the sealed site payload")]
    PreparedGraphReplayMismatch,
    #[error("prepared callable base does not match continuation")]
    PreparedBaseMismatch,
    #[error("prepared callable schema does not match continuation")]
    PreparedSchemaMismatch,
    #[error("prepared callable record does not match its checked call site")]
    PreparedCallSiteMismatch,
    #[error("checked expression coordinate is absent for call source {owner:?}")]
    MissingCheckedExpressionCoordinate { owner: ExprId },
    #[error("checked binding coordinate is absent for call source {owner:?}")]
    MissingCheckedBindingCoordinate { owner: LocalId },
    #[error("prepared callable group does not match continuation")]
    PreparedGroupMismatch,
    #[error("prepared callable deferred rows do not match continuation")]
    PreparedDeferredMismatch,
    #[error("prepared callable effect instantiation does not match its checked issuer")]
    PreparedEffectInstantiationMismatch,
    #[error("raw effect source does not match the definition-owned typed position")]
    PreparedEffectSourceShapeMismatch,
    #[error("raw effect source has an unresolved tail at a closed or unowned position")]
    PreparedEffectSourceTailMismatch,
    #[error("raw effect source variable is foreign to its definition-owned position")]
    PreparedEffectSourceForeignVariable,
    #[error("prepared callable function type does not match continuation")]
    PreparedFunctionTypeMismatch,
    #[error("composite local function value cannot be prepared as a continuation")]
    CompositeFunctionValue,
    #[error("function-value origin evidence is missing")]
    MissingFunctionValueOrigin,
    #[error("function-value origin evidence is attached to a non-function callee")]
    UnexpectedFunctionValueOrigin,
    #[error("function-value origin evidence does not name its callee")]
    InvalidFunctionValueOrigin,
    #[error("checked Character project item is missing its canonical Character identity")]
    MissingCheckedCharacterIdentity,
    #[error("terminal candidate retains a future-eligible parameter")]
    TerminalFutureEligibleParameter,
    #[error("lower constraint invariant: {0}")]
    Lower(TypeConstraintInvariant),
    #[error("lower rejection escaped candidate selection: {0}")]
    UnexpectedLowerRejection(TypeConstraintRejection),
    #[error("a rejected call candidate reached selected publication")]
    UnexpectedRejectedSelection,
    #[error("selected call candidate was rejected during replay")]
    ReplayRejected,
    #[error("selected call replay returned an invalid transaction shape")]
    ReplayTransactionShapeMismatch,
    #[error("selected call replay rank does not match the sealed probe")]
    ReplayRankMismatch,
    #[error("selected call replay application does not match the sealed probe")]
    ReplayApplicationMismatch,
    #[error("selected call replay callee inputs do not match the sealed probe")]
    ReplayCalleeInputsMismatch,
    #[error("selected call replay argument mapping does not match the sealed probe")]
    ReplayArgumentMappingMismatch,
    #[error("selected call replay materialized branch shape does not match the sealed probe")]
    ReplaySealedBranchShapeMismatch,
    #[error("selected call replay branch projection authority does not match the sealed probe")]
    ReplayBranchProjectionAuthorityMismatch,
    #[error("selected call replay branch prepared graph does not match the sealed probe: {0}")]
    ReplayBranchPreparedGraphMismatch(PreparedCallGraphReplayMismatch),
    #[error("selected call replay branch local facts do not match the sealed probe")]
    ReplayBranchLocalFactsMismatch,
    #[error("selected call replay branch pattern facts do not match the sealed probe")]
    ReplayBranchPatternFactsMismatch,
    #[error("selected call replay branch expression facts do not match the sealed probe")]
    ReplayBranchExpressionFactsMismatch,
    #[error("selected call replay branch dialogue mark catalogs do not match the sealed probe")]
    ReplayBranchDialogueMarkCatalogMismatch,
    #[error("selected call replay branch iteration facts do not match the sealed probe")]
    ReplayBranchIterationFactsMismatch,
    #[error("selected call replay branch implicit captures do not match the sealed probe")]
    ReplayBranchImplicitCaptureMismatch,
    #[error("selected call replay branch physical transcript does not match the sealed probe")]
    ReplayBranchPhysicalTranscriptMismatch,
    #[error("selected call replay closed sources do not match the sealed probe")]
    ReplayClosedSourcesMismatch,
    #[error("selected call replay typed projections do not match the sealed probe")]
    ReplayProjectionMismatch,
    #[error("an analyzer callback fact scope remained active at candidate finish")]
    ActiveFactScope,
}

/// Opaque public carrier for a graph invariant.  The exact lower invariant
/// remains owned by the callable graph and cannot be pattern matched through
/// the final-analysis error surface; Debug retains typed provenance.
#[derive(Clone, Eq, PartialEq)]
pub struct PreparedCallGraphInvariant(CallConstraintInvariant);

impl From<CallConstraintInvariant> for PreparedCallGraphInvariant {
    fn from(invariant: CallConstraintInvariant) -> Self {
        Self(invariant)
    }
}

impl std::fmt::Debug for PreparedCallGraphInvariant {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("PreparedCallGraphInvariant")
            .field(&self.0)
            .finish()
    }
}

impl std::fmt::Display for PreparedCallGraphInvariant {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("prepared call graph invariant")
    }
}

impl std::error::Error for PreparedCallGraphInvariant {}

/// A checked, issuer-bound generic scope outside the callable's own schema.
/// The graph issuer consumes it and does not retain a parallel scope table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EnclosingGenericParameterScope {
    types: Box<[GenericTypeParameterId]>,
    consts: Box<[GenericConstParameterId]>,
}

impl EnclosingGenericParameterScope {
    pub(crate) fn sealed<T, C>(types: T, consts: C) -> Result<Self, CallConstraintInvariant>
    where
        T: IntoIterator<Item = GenericTypeParameterId>,
        C: IntoIterator<Item = GenericConstParameterId>,
    {
        let types = types.into_iter().collect::<Vec<_>>();
        let consts = consts.into_iter().collect::<Vec<_>>();
        if !strictly_ordered(&types) || !strictly_ordered(&consts) {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
        Ok(Self {
            types: types.into_boxed_slice(),
            consts: consts.into_boxed_slice(),
        })
    }

    pub(crate) fn types(&self) -> &[GenericTypeParameterId] {
        &self.types
    }

    pub(crate) fn consts(&self) -> &[GenericConstParameterId] {
        &self.consts
    }
}

/// The payload stored in one prepared graph node.  Unselected payloads are
/// analyzer-owned and are never interpreted by the callable graph.
pub(crate) trait PreparedCallPrefixPayload {
    type Unselected: PartialEq;

    fn application(&self) -> &super::PreparedCallableApplication;
    /// Every prepared continuation candidate retained by this prefix.  The
    /// graph owns the canonical dependency row; selected and non-selected
    /// inventory members may not hide additional continuation ancestry.
    fn dependencies(&self) -> Box<[PreparedCallContinuationRef]>;
    fn validate_site(&self, _site: CheckedCallSite) -> Result<(), CallConstraintInvariant> {
        Ok(())
    }
    fn replay_mismatch(&self, other: &Self) -> Option<PreparedCallPrefixReplayMismatch>;

    fn replay_eq(&self, other: &Self) -> bool {
        self.replay_mismatch(other).is_none()
    }
}

/// Analyzer-independent component of a prepared prefix replay mismatch.
/// Concrete payload owners decide what their metadata contains, while the
/// graph remains the single classifier for site/application/payload parity.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum PreparedCallPrefixReplayMismatch {
    #[error("checked call site differs")]
    Site,
    #[error("prepared callable application differs: {0}")]
    Application(super::PreparedCallableApplicationReplayMismatch),
    #[error("owner payload differs")]
    Payload,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct PreparedCallNodeId(u64);

#[derive(Clone)]
struct PreparedCallGraphIssuer;

#[derive(Clone)]
struct PreparedCallContinuationCoordinate {
    issuer: Arc<PreparedCallGraphIssuer>,
    node: PreparedCallNodeId,
}

/// Opaque graph evidence that one call produced a partial function value.
/// The reference may be copied as evidence, but the carrier itself is
/// move-only and can be consumed only by graph-owned preparation.
#[derive(Clone)]
pub(crate) struct PreparedCallContinuationRef(PreparedCallContinuationCoordinate);

impl std::fmt::Debug for PreparedCallContinuationRef {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PreparedCallContinuationRef(..)")
    }
}

impl PartialEq for PreparedCallContinuationRef {
    fn eq(&self, other: &Self) -> bool {
        self.0.node == other.0.node && Arc::ptr_eq(&self.0.issuer, &other.0.issuer)
    }
}

impl Eq for PreparedCallContinuationRef {}

/// The only two valid origins for a function-valued call callee.  A prepared
/// continuation is graph-owned evidence; a terminal function result is an
/// independent callable value and must not acquire a synthetic dependency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCallSiteContinuation {
    Prepared(PreparedCallContinuationRef),
    Independent,
}

pub(crate) struct PreparedCallContinuationSeed {
    coordinate: PreparedCallContinuationCoordinate,
    solution: Arc<TypeConstraintSolution>,
}

impl PreparedCallContinuationSeed {
    fn into_solution(self) -> Arc<TypeConstraintSolution> {
        let Self {
            coordinate: _coordinate,
            solution,
        } = self;
        solution
    }
}

/// Resolver-only evidence for the next prepared candidate.  Lower receives
/// only [`PreparedCallContinuationSeed`], so resolver execution cannot clone
/// or discard the inherited constraint solution.
pub(crate) struct PreparedContinuationCandidateSeed {
    base: Arc<super::PreparedResolvedCallableDefinition>,
    reference: PreparedCallContinuationRef,
    current_group: CallableGroupIndex,
    function_type: TypeKind,
}

impl PreparedContinuationCandidateSeed {
    pub(crate) fn into_candidate_parts(
        self,
    ) -> (
        Arc<super::PreparedResolvedCallableDefinition>,
        PreparedCallContinuationRef,
        CallableGroupIndex,
        TypeKind,
    ) {
        let Self {
            base,
            reference,
            current_group,
            function_type,
        } = self;
        (base, reference, current_group, function_type)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DeferredContinuationParameter {
    parameter: GenericTypeParameterId,
    first_remaining_group: CallableGroupIndex,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DeferredContinuationConstParameter {
    parameter: GenericConstParameterId,
    first_remaining_group: CallableGroupIndex,
}

struct DeferredContinuationInventory {
    types: Box<[DeferredContinuationParameter]>,
    consts: Box<[DeferredContinuationConstParameter]>,
}

impl DeferredContinuationInventory {
    fn is_canonical(&self) -> bool {
        strictly_ordered(&self.types) && strictly_ordered(&self.consts)
    }
}

struct PreparedCallContinuation<P> {
    coordinate: PreparedCallContinuationCoordinate,
    prefix: P,
}

struct PreparedCallNode<P, U> {
    site: CheckedCallSite,
    dependencies: Box<[PreparedCallContinuationRef]>,
    payload: PreparedCallNodePayload<P, U>,
}

enum PreparedCallNodePayload<P, U> {
    SelectedValue { prefix: P, result: TypeKind },
    SelectedContinuation(PreparedCallContinuation<P>),
    Unselected(U),
}

/// Move-only graph delta.  Node IDs allocated during a rolled-back delta are
/// never reused; this prevents stale references from becoming valid again.
pub(crate) struct PreparedCallGraphCheckpoint {
    issuer: Arc<PreparedCallGraphIssuer>,
    id: u64,
}

pub(crate) struct PreparedCallGraphDelta<P, U = ()> {
    issuer: Arc<PreparedCallGraphIssuer>,
    touched_nodes: BTreeSet<PreparedCallNodeId>,
    touched_sites: BTreeSet<CheckedCallSite>,
    baseline_nodes: BTreeSet<PreparedCallNodeId>,
    nodes: BTreeMap<PreparedCallNodeId, PreparedCallNode<P, U>>,
}

impl<P, U> PreparedCallGraphDelta<P, U> {
    /// Returns the exact site/payload-state inventory owned by this affine
    /// delta.  This is the graph owner's preflight projection; analyzer fact
    /// code never reconstructs node state from candidate records.
    pub(crate) fn site_states(
        &self,
    ) -> Result<Box<[(CheckedCallSite, PreparedCallGraphSiteState)]>, CallConstraintInvariant> {
        if self.touched_nodes.len() != self.nodes.len()
            || self.touched_sites.len() != self.nodes.len()
            || self
                .touched_nodes
                .iter()
                .any(|node| !self.nodes.contains_key(node))
        {
            return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
        }
        let mut rows = BTreeMap::new();
        for node in self.nodes.values() {
            if !self.touched_sites.contains(&node.site)
                || rows
                    .insert(
                        node.site,
                        match &node.payload {
                            PreparedCallNodePayload::SelectedValue { .. }
                            | PreparedCallNodePayload::SelectedContinuation(_) => {
                                PreparedCallGraphSiteState::Selected
                            }
                            PreparedCallNodePayload::Unselected(_) => {
                                PreparedCallGraphSiteState::Unselected
                            }
                        },
                    )
                    .is_some()
            {
                return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
            }
        }
        Ok(rows.into_iter().collect::<Vec<_>>().into_boxed_slice())
    }
}

/// Exact component of a semantic prepared-graph replay mismatch.  The graph
/// owner retains this provenance so analyzer replay cannot collapse an
/// issuer, topology, or payload failure into one undifferentiated invariant.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum PreparedCallGraphReplayMismatch {
    #[error("graph issuer differs")]
    Issuer,
    #[error("left graph delta is malformed")]
    LeftMalformed,
    #[error("right graph delta is malformed")]
    RightMalformed,
    #[error("touched call-site inventory differs")]
    Sites,
    #[error("external dependency inventory differs")]
    BaselineDependencies,
    #[error("node inventory differs")]
    Nodes,
    #[error("canonical node call site differs")]
    NodeSite,
    #[error("canonical node dependency topology differs")]
    NodeDependencies,
    #[error("canonical node payload state differs")]
    NodePayloadState,
    #[error("canonical selected node result differs")]
    NodeResult,
    #[error("canonical selected node prefix differs: {0}")]
    NodePrefix(PreparedCallPrefixReplayMismatch),
    #[error("canonical unselected node payload differs")]
    UnselectedPayload,
}

impl<P: PreparedCallPrefixPayload> PreparedCallGraphDelta<P, P::Unselected> {
    /// Compare graph evidence without treating generation-local node IDs as
    /// semantic values.  Nodes owned by the delta are mapped by a canonical
    /// dependency-topological order with call-site tie breaks; dependencies
    /// outside the delta remain exact issuer/coordinate references.
    pub(crate) fn replay_mismatch(&self, other: &Self) -> Option<PreparedCallGraphReplayMismatch> {
        if !Arc::ptr_eq(&self.issuer, &other.issuer) {
            return Some(PreparedCallGraphReplayMismatch::Issuer);
        }
        let Some(left_nodes) = canonical_delta_nodes(self) else {
            return Some(PreparedCallGraphReplayMismatch::LeftMalformed);
        };
        let Some(right_nodes) = canonical_delta_nodes(other) else {
            return Some(PreparedCallGraphReplayMismatch::RightMalformed);
        };
        if self.touched_sites != other.touched_sites {
            return Some(PreparedCallGraphReplayMismatch::Sites);
        }
        if self.baseline_nodes != other.baseline_nodes {
            return Some(PreparedCallGraphReplayMismatch::BaselineDependencies);
        }
        if left_nodes.len() != right_nodes.len() {
            return Some(PreparedCallGraphReplayMismatch::Nodes);
        }
        let left_ordinals = left_nodes
            .iter()
            .enumerate()
            .map(|(ordinal, node)| (*node, ordinal))
            .collect::<BTreeMap<_, _>>();
        let right_ordinals = right_nodes
            .iter()
            .enumerate()
            .map(|(ordinal, node)| (*node, ordinal))
            .collect::<BTreeMap<_, _>>();
        for (left_id, right_id) in left_nodes.iter().zip(right_nodes.iter()) {
            let left = &self.nodes[left_id];
            let right = &other.nodes[right_id];
            if left.site != right.site {
                return Some(PreparedCallGraphReplayMismatch::NodeSite);
            }
            if dependency_replay_keys(left, &left_ordinals)
                != dependency_replay_keys(right, &right_ordinals)
            {
                return Some(PreparedCallGraphReplayMismatch::NodeDependencies);
            }
            if let Some(mismatch) = node_payload_replay_mismatch(&left.payload, &right.payload) {
                return Some(mismatch);
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DependencyReplayKey {
    Local(usize),
    Baseline(u64),
}

fn canonical_delta_nodes<P, U>(
    delta: &PreparedCallGraphDelta<P, U>,
) -> Option<Vec<PreparedCallNodeId>> {
    if delta.nodes.len() != delta.touched_nodes.len()
        || delta.nodes.len() != delta.touched_sites.len()
    {
        return None;
    }
    let mut sites = BTreeMap::new();
    for node_id in &delta.touched_nodes {
        let node = delta.nodes.get(node_id)?;
        if !delta.touched_sites.contains(&node.site) || sites.insert(node.site, *node_id).is_some()
        {
            return None;
        }
        if delta
            .nodes
            .get(node_id)
            .and_then(|node| match &node.payload {
                PreparedCallNodePayload::SelectedContinuation(continuation)
                    if Arc::ptr_eq(&continuation.coordinate.issuer, &delta.issuer)
                        && continuation.coordinate.node == *node_id =>
                {
                    Some(())
                }
                PreparedCallNodePayload::SelectedContinuation(_) => None,
                PreparedCallNodePayload::SelectedValue { .. }
                | PreparedCallNodePayload::Unselected(_) => Some(()),
            })
            .is_none()
        {
            return None;
        }
    }
    if delta
        .touched_sites
        .iter()
        .any(|site| sites.get(site).is_none())
    {
        return None;
    }

    let mut indegree = delta
        .nodes
        .keys()
        .map(|node| (*node, 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<PreparedCallNodeId, Vec<PreparedCallNodeId>>::new();
    let mut referenced_baseline_nodes = BTreeSet::new();
    for (node_id, node) in &delta.nodes {
        let mut seen_dependencies = BTreeSet::new();
        for dependency in &node.dependencies {
            if !Arc::ptr_eq(&dependency.0.issuer, &delta.issuer) {
                return None;
            }
            if dependency.0.node == *node_id || !seen_dependencies.insert(dependency.0.node) {
                return None;
            }
            if delta.nodes.contains_key(&dependency.0.node) {
                let Some(degree) = indegree.get_mut(node_id) else {
                    return None;
                };
                *degree = degree.checked_add(1)?;
                outgoing
                    .entry(dependency.0.node)
                    .or_default()
                    .push(*node_id);
            } else if !delta.baseline_nodes.contains(&dependency.0.node) {
                return None;
            } else {
                referenced_baseline_nodes.insert(dependency.0.node);
            }
        }
    }
    if referenced_baseline_nodes != delta.baseline_nodes {
        return None;
    }

    let mut ready = BTreeSet::<(CheckedCallSite, PreparedCallNodeId)>::new();
    for (node_id, degree) in &indegree {
        if *degree == 0 {
            ready.insert((delta.nodes[node_id].site, *node_id));
        }
    }
    let mut ordered = Vec::with_capacity(delta.nodes.len());
    while let Some((site, node_id)) = ready.iter().next().copied() {
        ready.remove(&(site, node_id));
        ordered.push(node_id);
        if let Some(children) = outgoing.get(&node_id) {
            for child in children {
                let degree = indegree.get_mut(child)?;
                *degree = degree.checked_sub(1)?;
                if *degree == 0 {
                    ready.insert((delta.nodes[child].site, *child));
                }
            }
        }
    }
    (ordered.len() == delta.nodes.len()).then_some(ordered)
}

fn dependency_replay_keys<P, U>(
    node: &PreparedCallNode<P, U>,
    ordinals: &BTreeMap<PreparedCallNodeId, usize>,
) -> Vec<DependencyReplayKey> {
    let mut keys = node
        .dependencies
        .iter()
        .map(|dependency| {
            ordinals
                .get(&dependency.0.node)
                .copied()
                .map(DependencyReplayKey::Local)
                .unwrap_or(DependencyReplayKey::Baseline(dependency.0.node.0))
        })
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

fn node_payload_replay_mismatch<P: PreparedCallPrefixPayload>(
    left: &PreparedCallNodePayload<P, P::Unselected>,
    right: &PreparedCallNodePayload<P, P::Unselected>,
) -> Option<PreparedCallGraphReplayMismatch> {
    match (left, right) {
        (
            PreparedCallNodePayload::SelectedValue {
                prefix: left,
                result: left_result,
            },
            PreparedCallNodePayload::SelectedValue {
                prefix: right,
                result: right_result,
            },
        ) => {
            if let Some(mismatch) = left.replay_mismatch(right) {
                Some(PreparedCallGraphReplayMismatch::NodePrefix(mismatch))
            } else if left_result != right_result {
                Some(PreparedCallGraphReplayMismatch::NodeResult)
            } else {
                None
            }
        }
        (
            PreparedCallNodePayload::SelectedContinuation(left),
            PreparedCallNodePayload::SelectedContinuation(right),
        ) => left
            .prefix
            .replay_mismatch(&right.prefix)
            .map(PreparedCallGraphReplayMismatch::NodePrefix),
        (PreparedCallNodePayload::Unselected(left), PreparedCallNodePayload::Unselected(right)) => {
            (left != right).then_some(PreparedCallGraphReplayMismatch::UnselectedPayload)
        }
        _ => Some(PreparedCallGraphReplayMismatch::NodePayloadState),
    }
}

pub(crate) struct PreparedCallGraphCloseFailure {
    violation: CallConstraintInvariant,
    checkpoint: PreparedCallGraphCheckpoint,
}

impl std::fmt::Debug for PreparedCallGraphCloseFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedCallGraphCloseFailure")
            .field("violation", &self.violation)
            .finish()
    }
}

impl PreparedCallGraphCloseFailure {
    pub(crate) fn into_parts(self) -> (CallConstraintInvariant, PreparedCallGraphCheckpoint) {
        (self.violation, self.checkpoint)
    }
}

/// A restore failure retains the move-only delta so the enclosing fact
/// transaction can poison/rollback it without dropping semantic evidence.
pub(crate) struct PreparedCallGraphRestoreFailure<P, U = ()> {
    violation: CallConstraintInvariant,
    delta: Box<PreparedCallGraphDelta<P, U>>,
}

impl<P, U> std::fmt::Debug for PreparedCallGraphRestoreFailure<P, U> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedCallGraphRestoreFailure")
            .field("violation", &self.violation)
            .finish()
    }
}

impl<P, U> PreparedCallGraphRestoreFailure<P, U> {
    pub(crate) fn into_parts(self) -> (CallConstraintInvariant, PreparedCallGraphDelta<P, U>) {
        (self.violation, *self.delta)
    }
}

struct PreparedCallGraphActiveDelta {
    id: u64,
    touched_nodes: BTreeSet<PreparedCallNodeId>,
    touched_sites: BTreeSet<CheckedCallSite>,
}

impl Clone for PreparedCallGraphActiveDelta {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            touched_nodes: self.touched_nodes.clone(),
            touched_sites: self.touched_sites.clone(),
        }
    }
}

pub(crate) struct PreparedCallGraph<P, U = ()> {
    issuer: Arc<PreparedCallGraphIssuer>,
    next_node: u64,
    next_delta: u64,
    active_deltas: Vec<PreparedCallGraphActiveDelta>,
    sites: BTreeMap<CheckedCallSite, PreparedCallNodeId>,
    nodes: BTreeMap<PreparedCallNodeId, PreparedCallNode<P, U>>,
}

pub(crate) struct PreparedCallGraphSelectedNode<'a, P> {
    site: CheckedCallSite,
    prefix: &'a P,
}

/// The only two payload states that may back a checked call expression while
/// the prepared graph is live.  The graph owner derives this state from the
/// unique site row; callers cannot infer it from expression facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCallGraphSiteState {
    Selected,
    Unselected,
}

/// Opaque generation-local key used only while the final C sealer consumes a
/// prepared graph.  It is neither a semantic identity nor an encodable fact.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PreparedCallGraphSealNodeKey(PreparedCallNodeId);

/// Affine authority retained only for the duration of one consuming graph
/// seal.  It is the sole bridge from an opaque prepared continuation reference
/// to an earlier dependency node in that same graph.
pub(crate) struct PreparedCallGraphSealAuthority {
    issuer: Arc<PreparedCallGraphIssuer>,
}

impl PreparedCallGraphSealAuthority {
    pub(crate) fn resolve_reference(
        &self,
        reference: &PreparedCallContinuationRef,
    ) -> Result<PreparedCallGraphSealNodeKey, CallConstraintInvariant> {
        Arc::ptr_eq(&self.issuer, &reference.0.issuer)
            .then_some(PreparedCallGraphSealNodeKey(reference.0.node))
            .ok_or(CallConstraintInvariant::ForeignPreparedIssuer)
    }
}

/// One owned node yielded by consuming the complete prepared graph.  Node
/// keys and dependency keys remain opaque and disappear with the C-sealer
/// transaction.
pub(crate) struct PreparedCallGraphSealNode<P, U> {
    key: PreparedCallGraphSealNodeKey,
    site: CheckedCallSite,
    dependencies: Box<[PreparedCallGraphSealNodeKey]>,
    payload: PreparedCallGraphSealPayload<P, U>,
}

pub(crate) enum PreparedCallGraphSealPayload<P, U> {
    SelectedValue { prefix: P, result: TypeKind },
    SelectedContinuation { prefix: P },
    Unselected(U),
}

impl<P, U> PreparedCallGraphSealNode<P, U> {
    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedCallGraphSealNodeKey,
        CheckedCallSite,
        Box<[PreparedCallGraphSealNodeKey]>,
        PreparedCallGraphSealPayload<P, U>,
    ) {
        (self.key, self.site, self.dependencies, self.payload)
    }
}

impl<'a, P> PreparedCallGraphSelectedNode<'a, P> {
    pub(crate) const fn site(&self) -> CheckedCallSite {
        self.site
    }

    pub(crate) const fn prefix(&self) -> &'a P {
        self.prefix
    }
}

/// Read-only ingress for resolver preparation.  The resolver may ask the
/// graph whether a checked call site issued a continuation reference, but it
/// cannot inspect the graph's node maps or provisional call facts.  The view
/// deliberately exposes no mutation or selected-application payload.
#[derive(Clone, Copy)]
pub(crate) struct PreparedCallGraphIngress<'a, P, U = ()> {
    graph: &'a PreparedCallGraph<P, U>,
}

/// The resolver's only continuation lookup capability.  It is implemented
/// by the graph owner, so a function-value seed cannot carry or reconstruct a
/// base callable or group on its own.
pub(crate) trait PreparedCallContinuationAuthority {
    fn resolve_prepared_continuation(
        &self,
        reference: &PreparedCallContinuationRef,
        actual: &TypeKind,
    ) -> Result<super::PreparedResolvedCallable, CallConstraintInvariant>;
}

impl<'a, P, U> PreparedCallGraphIngress<'a, P, U> {
    pub(crate) const fn new(graph: &'a PreparedCallGraph<P, U>) -> Self {
        Self { graph }
    }

    /// Return the graph-issued continuation at `site`.  A missing site is a
    /// stale producer fact, never an independent function value.
    pub(crate) fn continuation_at(
        &self,
        site: CheckedCallSite,
        actual: &TypeKind,
    ) -> Result<PreparedCallSiteContinuation, CallConstraintInvariant>
    where
        P: PreparedCallPrefixPayload<Unselected = U>,
    {
        let Some(node_id) = self.graph.sites.get(&site) else {
            return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
        };
        let node = self
            .graph
            .nodes
            .get(node_id)
            .ok_or(CallConstraintInvariant::MissingOrStalePreparedNode)?;
        match &node.payload {
            PreparedCallNodePayload::SelectedContinuation(continuation) => {
                let reference = PreparedCallContinuationRef(continuation.coordinate.clone());
                self.graph.validate_continuation_chain(&reference)?;
                continuation.prefix.validate_site(site)?;
                let function_type = continuation.prefix.application().function_type()?;
                if actual != &function_type {
                    return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch);
                }
                Ok(PreparedCallSiteContinuation::Prepared(reference))
            }
            PreparedCallNodePayload::SelectedValue { prefix, result } => {
                prefix.validate_site(site)?;
                self.graph.validate_prefix(prefix)?;
                let application = prefix.application();
                let sealed_result = application.result_type()?;
                if *result != sealed_result
                    || application
                        .selected()
                        .next_group_for(application.completed_group())
                        .is_some()
                {
                    return Err(CallConstraintInvariant::InvalidPreparedNodeState);
                }
                if actual != &sealed_result {
                    return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch);
                }
                if matches!(sealed_result, TypeKind::Function { .. }) {
                    Ok(PreparedCallSiteContinuation::Independent)
                } else {
                    Err(CallConstraintInvariant::InvalidPreparedNodeState)
                }
            }
            PreparedCallNodePayload::Unselected(_) => {
                Err(CallConstraintInvariant::InvalidPreparedNodeState)
            }
        }
    }
}

impl<P, U> Default for PreparedCallGraph<P, U> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P, U> PreparedCallGraph<P, U> {
    pub(crate) fn new() -> Self {
        Self {
            issuer: Arc::new(PreparedCallGraphIssuer),
            next_node: 0,
            next_delta: 0,
            active_deltas: Vec::new(),
            sites: BTreeMap::new(),
            nodes: BTreeMap::new(),
        }
    }

    pub(crate) fn validate_seal_ready(&self) -> Result<(), CallConstraintInvariant> {
        if self.active_deltas.is_empty() {
            Ok(())
        } else {
            Err(CallConstraintInvariant::ActiveFactScope)
        }
    }

    pub(crate) fn selected_nodes(
        &self,
    ) -> impl Iterator<Item = PreparedCallGraphSelectedNode<'_, P>> {
        self.nodes.values().filter_map(|node| match &node.payload {
            PreparedCallNodePayload::SelectedValue { prefix, .. }
            | PreparedCallNodePayload::SelectedContinuation(PreparedCallContinuation {
                prefix,
                ..
            }) => Some(PreparedCallGraphSelectedNode {
                site: node.site,
                prefix,
            }),
            PreparedCallNodePayload::Unselected(_) => None,
        })
    }

    pub(crate) fn site_state(&self, site: CheckedCallSite) -> Option<PreparedCallGraphSiteState> {
        let node = self
            .sites
            .get(&site)
            .and_then(|node| self.nodes.get(node))?;
        Some(match &node.payload {
            PreparedCallNodePayload::SelectedValue { .. }
            | PreparedCallNodePayload::SelectedContinuation(_) => {
                PreparedCallGraphSiteState::Selected
            }
            PreparedCallNodePayload::Unselected(_) => PreparedCallGraphSiteState::Unselected,
        })
    }

    /// Projects one exact live site payload without exposing graph node IDs or
    /// requiring a caller to repeat the selected/value/continuation state
    /// match.  This is the sole read-only bridge used while HIR seals the
    /// selected semantic child graph; the consuming C seal still owns the
    /// eventual graph teardown.
    pub(crate) fn project_site_payload<R>(
        &self,
        site: CheckedCallSite,
        selected: impl FnOnce(&P) -> R,
        unselected: impl FnOnce(&U) -> R,
    ) -> Option<R> {
        let node = self
            .sites
            .get(&site)
            .and_then(|node| self.nodes.get(node))?;
        Some(match &node.payload {
            PreparedCallNodePayload::SelectedValue { prefix, .. }
            | PreparedCallNodePayload::SelectedContinuation(PreparedCallContinuation {
                prefix,
                ..
            }) => selected(prefix),
            PreparedCallNodePayload::Unselected(value) => unselected(value),
        })
    }

    pub(crate) fn sites(&self) -> impl ExactSizeIterator<Item = CheckedCallSite> + '_ {
        self.sites.keys().copied()
    }

    /// Consume the complete generation-local graph into dependency-ordered
    /// C-sealer inputs.  No active candidate delta may survive this boundary,
    /// and every issuer-bound reference is reconciled to an opaque key before
    /// the graph tables disappear.
    pub(crate) fn into_seal_nodes(
        self,
    ) -> Result<
        (
            PreparedCallGraphSealAuthority,
            Box<[PreparedCallGraphSealNode<P, U>]>,
        ),
        CallConstraintInvariant,
    > {
        if !self.active_deltas.is_empty() || self.sites.len() != self.nodes.len() {
            return Err(CallConstraintInvariant::ActiveFactScope);
        }
        let authority = PreparedCallGraphSealAuthority {
            issuer: Arc::clone(&self.issuer),
        };
        let all_nodes = self.nodes.keys().copied().collect::<BTreeSet<_>>();
        if self
            .sites
            .iter()
            .any(|(site, node)| self.nodes.get(node).is_none_or(|value| value.site != *site))
        {
            return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
        }
        let mut sealed = Vec::with_capacity(self.nodes.len());
        for (node_id, node) in self.nodes {
            let mut previous = None;
            let dependencies = node
                .dependencies
                .iter()
                .map(|reference| {
                    if !Arc::ptr_eq(&self.issuer, &reference.0.issuer)
                        || reference.0.node >= node_id
                        || previous.is_some_and(|prior| prior >= reference.0.node)
                        || !all_nodes.contains(&reference.0.node)
                    {
                        return Err(CallConstraintInvariant::InvalidPreparedDependencyOrder);
                    }
                    previous = Some(reference.0.node);
                    Ok(PreparedCallGraphSealNodeKey(reference.0.node))
                })
                .collect::<Result<Box<[_]>, _>>()?;
            let payload = match node.payload {
                PreparedCallNodePayload::SelectedValue { prefix, result } => {
                    PreparedCallGraphSealPayload::SelectedValue { prefix, result }
                }
                PreparedCallNodePayload::SelectedContinuation(continuation) => {
                    if !Arc::ptr_eq(&self.issuer, &continuation.coordinate.issuer)
                        || continuation.coordinate.node != node_id
                    {
                        return Err(CallConstraintInvariant::InvalidPreparedNodeState);
                    }
                    PreparedCallGraphSealPayload::SelectedContinuation {
                        prefix: continuation.prefix,
                    }
                }
                PreparedCallNodePayload::Unselected(value) => {
                    PreparedCallGraphSealPayload::Unselected(value)
                }
            };
            sealed.push(PreparedCallGraphSealNode {
                key: PreparedCallGraphSealNodeKey(node_id),
                site: node.site,
                dependencies,
                payload,
            });
        }
        Ok((authority, sealed.into_boxed_slice()))
    }

    pub(crate) fn begin_delta(
        &mut self,
    ) -> Result<PreparedCallGraphCheckpoint, CallConstraintInvariant> {
        let id = self.next_delta;
        self.next_delta = self
            .next_delta
            .checked_add(1)
            .ok_or(CallConstraintInvariant::InvalidPreparedDependencyOrder)?;
        self.active_deltas.push(PreparedCallGraphActiveDelta {
            id,
            touched_nodes: BTreeSet::new(),
            touched_sites: BTreeSet::new(),
        });
        Ok(PreparedCallGraphCheckpoint {
            issuer: Arc::clone(&self.issuer),
            id,
        })
    }

    fn close_position(
        &self,
        checkpoint: &PreparedCallGraphCheckpoint,
    ) -> Result<(), CallConstraintInvariant> {
        if !Arc::ptr_eq(&self.issuer, &checkpoint.issuer) {
            return Err(CallConstraintInvariant::ForeignPreparedIssuer);
        }
        match self.active_deltas.last() {
            Some(active) if active.id == checkpoint.id => Ok(()),
            Some(_)
                if self
                    .active_deltas
                    .iter()
                    .any(|active| active.id == checkpoint.id) =>
            {
                Err(CallConstraintInvariant::PreparedGraphDeltaOrder)
            }
            Some(_) | None => Err(CallConstraintInvariant::PreparedGraphDeltaStale),
        }
    }

    pub(crate) fn validate_checkpoint(
        &self,
        checkpoint: &PreparedCallGraphCheckpoint,
    ) -> Result<(), CallConstraintInvariant> {
        self.close_position(checkpoint)
    }

    pub(crate) fn validate_ancestor_checkpoint(
        &self,
        checkpoint: &PreparedCallGraphCheckpoint,
    ) -> Result<(), CallConstraintInvariant> {
        if !Arc::ptr_eq(&self.issuer, &checkpoint.issuer) {
            return Err(CallConstraintInvariant::ForeignPreparedIssuer);
        }
        self.active_deltas
            .iter()
            .any(|active| active.id == checkpoint.id)
            .then_some(())
            .ok_or(CallConstraintInvariant::PreparedGraphDeltaStale)
    }

    pub(crate) fn validate_delta(
        &self,
        delta: &PreparedCallGraphDelta<P, U>,
    ) -> Result<(), CallConstraintInvariant> {
        if !Arc::ptr_eq(&self.issuer, &delta.issuer) {
            return Err(CallConstraintInvariant::ForeignPreparedIssuer);
        }
        canonical_delta_nodes(delta)
            .map(|_| ())
            .ok_or(CallConstraintInvariant::MissingOrStalePreparedNode)
    }

    /// Trusted ledger recovery used after a fact close failure.  The caller
    /// has already decided to poison the enclosing candidate transaction, so
    /// every active graph frame and its touched nodes are consumed together.
    pub(crate) fn abort_after_close_failure(
        &mut self,
        checkpoint: PreparedCallGraphCheckpoint,
    ) -> Result<(), CallConstraintInvariant> {
        if !Arc::ptr_eq(&self.issuer, &checkpoint.issuer) {
            return Err(CallConstraintInvariant::ForeignPreparedIssuer);
        }
        self.active_deltas.clear();
        self.sites.clear();
        self.nodes.clear();
        Ok(())
    }

    fn active_for_checkpoint(
        &self,
        checkpoint: &PreparedCallGraphCheckpoint,
    ) -> Result<PreparedCallGraphActiveDelta, CallConstraintInvariant> {
        self.close_position(checkpoint)?;
        self.active_deltas
            .last()
            .cloned()
            .ok_or(CallConstraintInvariant::PreparedGraphDeltaStale)
    }

    fn pop_active(
        &mut self,
        checkpoint: &PreparedCallGraphCheckpoint,
    ) -> Result<PreparedCallGraphActiveDelta, CallConstraintInvariant> {
        self.close_position(checkpoint)?;
        self.active_deltas
            .pop()
            .ok_or(CallConstraintInvariant::PreparedGraphDeltaStale)
    }

    /// Validate the complete active mutation before any close mutates the
    /// graph.  The touched node/site sets are a bijection and every payload's
    /// embedded site must agree with both indexes.
    fn preflight_active_delta(
        &self,
        active: &PreparedCallGraphActiveDelta,
    ) -> Result<(), CallConstraintInvariant> {
        if active.touched_nodes.len() != active.touched_sites.len() {
            return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
        }
        for node_id in &active.touched_nodes {
            let Some(node) = self.nodes.get(node_id) else {
                return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
            };
            if !active.touched_sites.contains(&node.site)
                || self.sites.get(&node.site) != Some(node_id)
            {
                return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
            }
            let mut seen_dependencies = BTreeSet::new();
            let mut previous_dependency = None;
            for dependency in &node.dependencies {
                if !Arc::ptr_eq(&dependency.0.issuer, &self.issuer)
                    || dependency.0.node == *node_id
                    || !seen_dependencies.insert(dependency.0.node)
                    || !self.nodes.contains_key(&dependency.0.node)
                {
                    return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
                }
                if previous_dependency.is_some_and(|previous| previous >= dependency.0.node)
                    || dependency.0.node >= *node_id
                {
                    return Err(CallConstraintInvariant::InvalidPreparedDependencyOrder);
                }
                previous_dependency = Some(dependency.0.node);
            }
        }
        for site in &active.touched_sites {
            let Some(node_id) = self.sites.get(site) else {
                return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
            };
            if !active.touched_nodes.contains(node_id)
                || self
                    .nodes
                    .get(node_id)
                    .is_none_or(|node| node.site != *site)
            {
                return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
            }
        }
        Ok(())
    }

    pub(crate) fn rollback_delta(
        &mut self,
        checkpoint: PreparedCallGraphCheckpoint,
    ) -> Result<(), PreparedCallGraphCloseFailure> {
        let active = match self.active_for_checkpoint(&checkpoint) {
            Ok(active) => active,
            Err(violation) => {
                return Err(PreparedCallGraphCloseFailure {
                    violation,
                    checkpoint,
                });
            }
        };
        if let Err(violation) = self.preflight_active_delta(&active) {
            return Err(PreparedCallGraphCloseFailure {
                violation,
                checkpoint,
            });
        }
        if let Err(violation) = self.pop_active(&checkpoint) {
            return Err(PreparedCallGraphCloseFailure {
                violation,
                checkpoint,
            });
        }
        for site in active.touched_sites {
            self.sites.remove(&site);
        }
        for node in active.touched_nodes {
            self.nodes.remove(&node);
        }
        Ok(())
    }

    pub(crate) fn commit_delta(
        &mut self,
        checkpoint: PreparedCallGraphCheckpoint,
    ) -> Result<(), PreparedCallGraphCloseFailure> {
        let active = match self.active_for_checkpoint(&checkpoint) {
            Ok(active) => active,
            Err(violation) => {
                return Err(PreparedCallGraphCloseFailure {
                    violation,
                    checkpoint,
                });
            }
        };
        if let Err(violation) = self.preflight_active_delta(&active) {
            return Err(PreparedCallGraphCloseFailure {
                violation,
                checkpoint,
            });
        }
        if let Err(violation) = self.pop_active(&checkpoint) {
            return Err(PreparedCallGraphCloseFailure {
                violation,
                checkpoint,
            });
        }
        if let Some(parent) = self.active_deltas.last_mut() {
            parent.touched_nodes.extend(active.touched_nodes);
            parent.touched_sites.extend(active.touched_sites);
        }
        Ok(())
    }

    pub(crate) fn extract_delta(
        &mut self,
        checkpoint: PreparedCallGraphCheckpoint,
    ) -> Result<PreparedCallGraphDelta<P, U>, PreparedCallGraphCloseFailure> {
        let active = match self.active_for_checkpoint(&checkpoint) {
            Ok(active) => active,
            Err(violation) => {
                return Err(PreparedCallGraphCloseFailure {
                    violation,
                    checkpoint,
                });
            }
        };
        if let Err(violation) = self.preflight_active_delta(&active) {
            return Err(PreparedCallGraphCloseFailure {
                violation,
                checkpoint,
            });
        }
        let mut baseline_nodes = BTreeSet::new();
        for node_id in &active.touched_nodes {
            let Some(node) = self.nodes.get(node_id) else {
                return Err(PreparedCallGraphCloseFailure {
                    violation: CallConstraintInvariant::MissingOrStalePreparedNode,
                    checkpoint,
                });
            };
            for dependency in &node.dependencies {
                if !active.touched_nodes.contains(&dependency.0.node) {
                    baseline_nodes.insert(dependency.0.node);
                }
            }
        }
        if let Err(violation) = self.pop_active(&checkpoint) {
            return Err(PreparedCallGraphCloseFailure {
                violation,
                checkpoint,
            });
        }
        let mut delta = PreparedCallGraphDelta {
            issuer: Arc::clone(&checkpoint.issuer),
            touched_nodes: active.touched_nodes.clone(),
            touched_sites: active.touched_sites.clone(),
            baseline_nodes,
            nodes: BTreeMap::new(),
        };
        for site in &active.touched_sites {
            self.sites.remove(site);
        }
        for node in &active.touched_nodes {
            if let Some(value) = self.nodes.remove(node) {
                delta.nodes.insert(*node, value);
            }
        }
        delta.touched_nodes = active.touched_nodes;
        delta.touched_sites = active.touched_sites;
        Ok(delta)
    }

    pub(crate) fn restore_delta(
        &mut self,
        target: &PreparedCallGraphCheckpoint,
        mut delta: PreparedCallGraphDelta<P, U>,
    ) -> Result<(), PreparedCallGraphRestoreFailure<P, U>> {
        macro_rules! restore_error {
            ($violation:expr) => {
                return Err(PreparedCallGraphRestoreFailure {
                    violation: $violation,
                    delta: Box::new(delta),
                });
            };
        }
        if let Err(violation) = self.close_position(target) {
            restore_error!(violation);
        }
        let Some(target_active) = self.active_deltas.last() else {
            restore_error!(CallConstraintInvariant::PreparedGraphDeltaStale);
        };
        if let Err(violation) = self.preflight_active_delta(target_active) {
            restore_error!(violation);
        }
        if !Arc::ptr_eq(&self.issuer, &delta.issuer) {
            restore_error!(CallConstraintInvariant::ForeignPreparedIssuer);
        }
        let touched_nodes = delta.touched_nodes.clone();
        let touched_sites = delta.touched_sites.clone();
        if touched_nodes.len() != delta.nodes.len()
            || touched_sites.len() != delta.nodes.len()
            || touched_nodes
                .iter()
                .any(|node| !delta.nodes.contains_key(node))
            || touched_sites
                .iter()
                .any(|site| !delta.nodes.values().any(|node| node.site == *site))
        {
            restore_error!(CallConstraintInvariant::MissingOrStalePreparedNode);
        }
        if delta
            .baseline_nodes
            .iter()
            .any(|node| delta.nodes.contains_key(node))
        {
            restore_error!(CallConstraintInvariant::MissingOrStalePreparedNode);
        }
        let mut referenced_baseline_nodes = BTreeSet::new();
        for (node, value) in &delta.nodes {
            if self.nodes.contains_key(node) || self.sites.contains_key(&value.site) {
                restore_error!(CallConstraintInvariant::PreparedGraphDuplicateSite);
            }
            let mut previous = None;
            for dependency in value.dependencies.iter() {
                if !Arc::ptr_eq(&self.issuer, &dependency.0.issuer)
                    || previous.is_some_and(|previous| previous >= dependency.0.node)
                    || dependency.0.node >= *node
                    || (!delta.nodes.contains_key(&dependency.0.node)
                        && (!delta.baseline_nodes.contains(&dependency.0.node)
                            || !self.nodes.contains_key(&dependency.0.node)))
                {
                    restore_error!(
                        if previous.is_some_and(|previous| previous >= dependency.0.node)
                            || dependency.0.node >= *node
                        {
                            CallConstraintInvariant::InvalidPreparedDependencyOrder
                        } else {
                            CallConstraintInvariant::MissingOrStalePreparedNode
                        }
                    );
                }
                if !delta.nodes.contains_key(&dependency.0.node) {
                    referenced_baseline_nodes.insert(dependency.0.node);
                }
                previous = Some(dependency.0.node);
            }
        }
        if referenced_baseline_nodes != delta.baseline_nodes {
            restore_error!(CallConstraintInvariant::MissingOrStalePreparedNode);
        }
        let mut next_node = self.next_node;
        for node in delta.nodes.keys() {
            let Some(after) = node.0.checked_add(1) else {
                restore_error!(CallConstraintInvariant::InvalidPreparedDependencyOrder);
            };
            next_node = next_node.max(after);
        }
        self.next_node = next_node;
        let nodes = std::mem::take(&mut delta.nodes);
        for (node, value) in nodes {
            self.sites.insert(value.site, node);
            self.nodes.insert(node, value);
        }
        if let Some(parent) = self.active_deltas.last_mut() {
            parent.touched_nodes.extend(touched_nodes);
            parent.touched_sites.extend(touched_sites);
        }
        Ok(())
    }

    fn validate_dependencies(
        &self,
        dependencies: &[PreparedCallContinuationRef],
    ) -> Result<(), CallConstraintInvariant> {
        let mut previous = None;
        for dependency in dependencies {
            if !Arc::ptr_eq(&self.issuer, &dependency.0.issuer) {
                return Err(CallConstraintInvariant::ForeignPreparedIssuer);
            }
            if previous.is_some_and(|previous| previous >= dependency.0.node) {
                return Err(CallConstraintInvariant::InvalidPreparedDependencyOrder);
            }
            if dependency.0.node.0 >= self.next_node || !self.nodes.contains_key(&dependency.0.node)
            {
                return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
            }
            previous = Some(dependency.0.node);
        }
        Ok(())
    }

    fn allocate_node(&mut self) -> Result<PreparedCallNodeId, CallConstraintInvariant> {
        let node = PreparedCallNodeId(self.next_node);
        self.next_node = self
            .next_node
            .checked_add(1)
            .ok_or(CallConstraintInvariant::InvalidPreparedDependencyOrder)?;
        Ok(node)
    }

    fn record_insert(
        &mut self,
        node: PreparedCallNodeId,
        site: CheckedCallSite,
    ) -> Result<(), CallConstraintInvariant> {
        let Some(active) = self.active_deltas.last_mut() else {
            return Err(CallConstraintInvariant::PreparedGraphDeltaStale);
        };
        active.touched_nodes.insert(node);
        active.touched_sites.insert(site);
        Ok(())
    }

    /// Insert one selected application.  The application is the sole source
    /// for result projection, terminality, continuation group, deferred
    /// parameters, and function type; callers cannot supply a parallel set
    /// of values that could disagree with the sealed callable authority.
    pub(crate) fn seal_selected_application(
        &mut self,
        site: CheckedCallSite,
        prefix: P,
    ) -> Result<(TypeKind, Option<PreparedCallContinuationRef>), CallConstraintInvariant>
    where
        P: PreparedCallPrefixPayload<Unselected = U>,
    {
        self.ensure_active_delta()?;
        let mut dependencies = prefix.dependencies().into_vec();
        dependencies.sort_by_key(|reference| reference.0.node);
        dependencies.dedup_by(|left, right| left == right);
        let dependencies = dependencies.into_boxed_slice();
        self.validate_dependencies(&dependencies)?;
        prefix.validate_site(site)?;
        self.validate_prefix(&prefix)?;

        let application = prefix.application();
        let completed_group = application.completed_group();
        let selected = application.selected();
        self.validate_prepared_candidate_ancestry(selected)?;
        let result = application.result_type()?;
        let next_group = selected.next_group_for(completed_group);
        let function_type = if next_group.is_some() {
            Some(application.function_type()?)
        } else {
            None
        };
        if let Some(function_type) = function_type.as_ref() {
            let deferred = deferred_for_candidate(selected, completed_group);
            if *function_type != result
                || !deferred.is_canonical()
                || !matches!(function_type, TypeKind::Function { .. })
            {
                return Err(CallConstraintInvariant::PreparedDeferredMismatch);
            }
        }
        if let Some(sealed) = self.reconcile_selected_site(
            site,
            &dependencies,
            &prefix,
            &result,
            function_type.as_ref(),
        )? {
            return Ok(sealed);
        }
        let node = self.allocate_node()?;
        let continuation = if function_type.is_some() {
            let coordinate = PreparedCallContinuationCoordinate {
                issuer: Arc::clone(&self.issuer),
                node,
            };
            self.nodes.insert(
                node,
                PreparedCallNode {
                    site,
                    dependencies,
                    payload: PreparedCallNodePayload::SelectedContinuation(
                        PreparedCallContinuation {
                            coordinate: coordinate.clone(),
                            prefix,
                        },
                    ),
                },
            );
            Some(PreparedCallContinuationRef(coordinate))
        } else {
            self.nodes.insert(
                node,
                PreparedCallNode {
                    site,
                    dependencies,
                    payload: PreparedCallNodePayload::SelectedValue {
                        prefix,
                        result: result.clone(),
                    },
                },
            );
            None
        };
        self.sites.insert(site, node);
        if let Err(violation) = self.record_insert(node, site) {
            self.sites.remove(&site);
            self.nodes.remove(&node);
            return Err(violation);
        }
        Ok((result, continuation))
    }

    fn reconcile_selected_site(
        &self,
        site: CheckedCallSite,
        dependencies: &[PreparedCallContinuationRef],
        prefix: &P,
        result: &TypeKind,
        function_type: Option<&TypeKind>,
    ) -> Result<Option<(TypeKind, Option<PreparedCallContinuationRef>)>, CallConstraintInvariant>
    where
        P: PreparedCallPrefixPayload<Unselected = U>,
    {
        let Some(node_id) = self.sites.get(&site).copied() else {
            return Ok(None);
        };
        let node = self
            .nodes
            .get(&node_id)
            .ok_or(CallConstraintInvariant::MissingOrStalePreparedNode)?;
        if node.site != site || node.dependencies.as_ref() != dependencies {
            return Err(CallConstraintInvariant::PreparedGraphReplayMismatch);
        }
        match (&node.payload, function_type) {
            (
                PreparedCallNodePayload::SelectedValue {
                    prefix: sealed,
                    result: sealed_result,
                },
                None,
            ) if sealed_result == result && sealed.replay_eq(prefix) => {
                Ok(Some((result.clone(), None)))
            }
            (PreparedCallNodePayload::SelectedContinuation(sealed), Some(function_type))
                if sealed.prefix.replay_eq(prefix)
                    && sealed.prefix.application().result_type()? == *result
                    && sealed.prefix.application().function_type()? == *function_type
                    && Arc::ptr_eq(&sealed.coordinate.issuer, &self.issuer)
                    && sealed.coordinate.node == node_id =>
            {
                let reference = PreparedCallContinuationRef(sealed.coordinate.clone());
                self.validate_continuation_chain(&reference)?;
                Ok(Some((result.clone(), Some(reference))))
            }
            _ => Err(CallConstraintInvariant::PreparedGraphReplayMismatch),
        }
    }

    pub(crate) fn seal_unselected(
        &mut self,
        site: CheckedCallSite,
        dependencies: impl Into<Box<[PreparedCallContinuationRef]>>,
        value: U,
    ) -> Result<(), CallConstraintInvariant>
    where
        U: PartialEq,
    {
        self.ensure_active_delta()?;
        let dependencies = dependencies.into();
        self.validate_dependencies(&dependencies)?;
        if let Some(node_id) = self.sites.get(&site) {
            let node = self
                .nodes
                .get(node_id)
                .ok_or(CallConstraintInvariant::MissingOrStalePreparedNode)?;
            return if node.site == site
                && node.dependencies == dependencies
                && matches!(&node.payload, PreparedCallNodePayload::Unselected(sealed) if sealed == &value)
            {
                Ok(())
            } else {
                Err(CallConstraintInvariant::PreparedGraphReplayMismatch)
            };
        }
        let node = self.allocate_node()?;
        self.nodes.insert(
            node,
            PreparedCallNode {
                site,
                dependencies,
                payload: PreparedCallNodePayload::Unselected(value),
            },
        );
        self.sites.insert(site, node);
        if self.record_insert(node, site).is_err() {
            self.sites.remove(&site);
            self.nodes.remove(&node);
            return Err(CallConstraintInvariant::PreparedGraphDeltaStale);
        }
        Ok(())
    }

    fn validate_prefix(&self, prefix: &P) -> Result<(), CallConstraintInvariant>
    where
        P: PreparedCallPrefixPayload<Unselected = U>,
    {
        let application = prefix.application();
        if application.schema() != application.selected().schema().semantic_digest() {
            return Err(CallConstraintInvariant::PreparedSchemaMismatch);
        }
        if application.completed_group() != application.selected().call_group() {
            return Err(CallConstraintInvariant::PreparedGroupMismatch);
        }
        Ok(())
    }

    fn ensure_active_delta(&self) -> Result<(), CallConstraintInvariant> {
        (!self.active_deltas.is_empty())
            .then_some(())
            .ok_or(CallConstraintInvariant::PreparedGraphDeltaStale)
    }

    fn validate_prepared_candidate_ancestry(
        &self,
        selected: &super::PreparedResolvedCallable,
    ) -> Result<(), CallConstraintInvariant>
    where
        P: PreparedCallPrefixPayload<Unselected = U>,
    {
        let Some(parent_reference) = selected.prepared_continuation() else {
            return if selected.call_group() == CallableGroupIndex::ZERO {
                Ok(())
            } else {
                Err(CallConstraintInvariant::PreparedGroupMismatch)
            };
        };
        self.validate_continuation_chain(parent_reference)?;
        let parent_node = self
            .nodes
            .get(&parent_reference.0.node)
            .ok_or(CallConstraintInvariant::MissingOrStalePreparedNode)?;
        let PreparedCallNodePayload::SelectedContinuation(parent_continuation) =
            &parent_node.payload
        else {
            return Err(CallConstraintInvariant::InvalidPreparedNodeState);
        };
        let parent_application = parent_continuation.prefix.application();
        if !parent_application.base_matches(selected) {
            return Err(CallConstraintInvariant::PreparedBaseMismatch);
        }
        let parent_next = parent_application
            .selected()
            .next_group_for(parent_application.completed_group())
            .ok_or(CallConstraintInvariant::PreparedGroupMismatch)?;
        if selected.call_group() != parent_next {
            return Err(CallConstraintInvariant::PreparedGroupMismatch);
        }
        let parent_function_type = parent_application.function_type()?;
        if selected.prepared_function_type() != Some(&parent_function_type) {
            return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch);
        }
        Ok(())
    }

    fn validate_continuation_chain(
        &self,
        reference: &PreparedCallContinuationRef,
    ) -> Result<(), CallConstraintInvariant>
    where
        P: PreparedCallPrefixPayload<Unselected = U>,
    {
        if !Arc::ptr_eq(&self.issuer, &reference.0.issuer) {
            return Err(CallConstraintInvariant::ForeignPreparedIssuer);
        }
        let node = self
            .nodes
            .get(&reference.0.node)
            .ok_or(CallConstraintInvariant::MissingOrStalePreparedNode)?;
        let PreparedCallNodePayload::SelectedContinuation(continuation) = &node.payload else {
            return Err(CallConstraintInvariant::InvalidPreparedNodeState);
        };
        if !Arc::ptr_eq(&continuation.coordinate.issuer, &self.issuer)
            || continuation.coordinate.node != reference.0.node
        {
            return Err(CallConstraintInvariant::ForeignPreparedIssuer);
        }
        continuation.prefix.validate_site(node.site)?;
        self.validate_dependencies(&node.dependencies)?;
        self.validate_prefix(&continuation.prefix)?;
        let application = continuation.prefix.application();
        let selected = application.selected();
        self.validate_prepared_candidate_ancestry(selected)?;
        if let Some(parent_reference) = selected.prepared_continuation() {
            if node.dependencies.len() != 1 || node.dependencies[0] != *parent_reference {
                return Err(CallConstraintInvariant::PreparedBaseMismatch);
            }
        } else if !node.dependencies.is_empty()
            || application.completed_group() != CallableGroupIndex::ZERO
        {
            return Err(CallConstraintInvariant::PreparedGroupMismatch);
        }
        let result = application.result_type()?;
        let next_group = selected.next_group_for(application.completed_group());
        match &node.payload {
            PreparedCallNodePayload::SelectedContinuation(_) => {
                let function_type = application.function_type()?;
                if next_group.is_none()
                    || function_type != result
                    || !matches!(function_type, TypeKind::Function { .. })
                {
                    return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch);
                }
            }
            PreparedCallNodePayload::SelectedValue { result: stored, .. } => {
                if next_group.is_some()
                    || *stored != result
                    || matches!(result, TypeKind::Function { .. })
                {
                    return Err(CallConstraintInvariant::InvalidPreparedNodeState);
                }
            }
            PreparedCallNodePayload::Unselected(_) => {
                return Err(CallConstraintInvariant::InvalidPreparedNodeState);
            }
        }
        Ok(())
    }

    pub(crate) fn resolve_continuation(
        &self,
        reference: &PreparedCallContinuationRef,
        candidate: &super::PreparedResolvedCallable,
    ) -> Result<PreparedCallContinuationSeed, CallConstraintInvariant>
    where
        P: PreparedCallPrefixPayload<Unselected = U>,
    {
        self.validate_continuation_chain(reference)?;
        let Some(node) = self.nodes.get(&reference.0.node) else {
            return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
        };
        let PreparedCallNodePayload::SelectedContinuation(continuation) = &node.payload else {
            return Err(CallConstraintInvariant::InvalidPreparedNodeState);
        };
        debug_assert!(Arc::ptr_eq(&continuation.coordinate.issuer, &self.issuer));
        let application = continuation.prefix.application();
        let selected = application.selected();
        self.validate_prefix(&continuation.prefix)?;
        if !candidate
            .prepared_continuation()
            .is_some_and(|candidate_reference| candidate_reference == reference)
        {
            return Err(CallConstraintInvariant::PreparedBaseMismatch);
        }
        if !application.base_matches(candidate) {
            return Err(CallConstraintInvariant::PreparedBaseMismatch);
        }
        let expected_next = selected
            .next_group_for(application.completed_group())
            .ok_or(CallConstraintInvariant::PreparedGroupMismatch)?;
        if candidate.call_group() != expected_next {
            return Err(CallConstraintInvariant::PreparedGroupMismatch);
        }
        let function_type = application.function_type()?;
        let deferred = deferred_for_candidate(selected, application.completed_group());
        if !deferred.is_canonical() || !matches!(function_type, TypeKind::Function { .. }) {
            return Err(CallConstraintInvariant::PreparedDeferredMismatch);
        }
        if candidate.prepared_function_type() != Some(&function_type) {
            return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch);
        }
        Ok(PreparedCallContinuationSeed {
            coordinate: continuation.coordinate.clone(),
            solution: Arc::clone(application.solution()),
        })
    }

    /// Issue a prepared continuation candidate directly from the graph node.
    /// This is the only path used by a function-value resolver.  It validates
    /// issuer, node state, site payload, dependency order, and the exact next
    /// group before transferring the selected callable base into the prepared
    /// carrier.
    pub(crate) fn continuation_candidate_seed(
        &self,
        reference: &PreparedCallContinuationRef,
    ) -> Result<PreparedContinuationCandidateSeed, CallConstraintInvariant>
    where
        P: PreparedCallPrefixPayload<Unselected = U>,
    {
        self.validate_continuation_chain(reference)?;
        let Some(node) = self.nodes.get(&reference.0.node) else {
            return Err(CallConstraintInvariant::MissingOrStalePreparedNode);
        };
        let PreparedCallNodePayload::SelectedContinuation(continuation) = &node.payload else {
            return Err(CallConstraintInvariant::InvalidPreparedNodeState);
        };
        debug_assert!(Arc::ptr_eq(&continuation.coordinate.issuer, &self.issuer));
        debug_assert_eq!(continuation.coordinate.node, reference.0.node);
        let application = continuation.prefix.application();
        let selected = application.selected();
        let current_group = selected
            .next_group_for(application.completed_group())
            .ok_or(CallConstraintInvariant::PreparedGroupMismatch)?;
        let function_type = application.function_type()?;
        let deferred = deferred_for_candidate(selected, application.completed_group());
        if !deferred.is_canonical() || !matches!(function_type, TypeKind::Function { .. }) {
            return Err(CallConstraintInvariant::PreparedDeferredMismatch);
        }
        Ok(PreparedContinuationCandidateSeed {
            base: selected.definition(),
            reference: PreparedCallContinuationRef(continuation.coordinate.clone()),
            current_group,
            function_type,
        })
    }

    /// Derive the exact lower parameter scope and pair it with the one affine
    /// seed.  `candidate.current_group()` is the only group authority; no
    /// terminal flag or caller override is accepted.
    pub(crate) fn validate_and_issue_constraint_initialization(
        &self,
        candidate: &super::PreparedResolvedCallable,
        enclosing: &EnclosingGenericParameterScope,
    ) -> Result<PreparedConstraintInitialization, CallConstraintInvariant>
    where
        P: PreparedCallPrefixPayload<Unselected = U>,
    {
        issue_constraint_initialization(
            Arc::clone(&self.issuer),
            candidate,
            enclosing,
            |reference| self.resolve_continuation(reference, candidate),
        )
    }

    #[cfg(test)]
    pub(crate) fn validate_and_issue_base_constraint_initialization(
        &self,
        candidate: &super::PreparedResolvedCallable,
        enclosing: &EnclosingGenericParameterScope,
    ) -> Result<PreparedConstraintInitialization, CallConstraintInvariant> {
        issue_constraint_initialization(Arc::clone(&self.issuer), candidate, enclosing, |_| {
            Err(CallConstraintInvariant::MissingOrStalePreparedNode)
        })
    }
}

impl<P, U> PreparedCallContinuationAuthority for PreparedCallGraph<P, U>
where
    P: PreparedCallPrefixPayload<Unselected = U>,
{
    fn resolve_prepared_continuation(
        &self,
        reference: &PreparedCallContinuationRef,
        actual: &TypeKind,
    ) -> Result<super::PreparedResolvedCallable, CallConstraintInvariant> {
        super::PreparedResolvedCallable::try_from_prepared_continuation(self, reference, actual)
    }
}

fn issue_constraint_initialization<F>(
    issuer: Arc<PreparedCallGraphIssuer>,
    candidate: &super::PreparedResolvedCallable,
    enclosing: &EnclosingGenericParameterScope,
    mut resolve_continuation: F,
) -> Result<PreparedConstraintInitialization, CallConstraintInvariant>
where
    F: FnMut(
        &PreparedCallContinuationRef,
    ) -> Result<PreparedCallContinuationSeed, CallConstraintInvariant>,
{
    let current_group = candidate.call_group();
    let inventory = candidate.schema().generic_inventory();
    if candidate.schema().group(current_group).is_none() {
        return Err(CallConstraintInvariant::MalformedSchemaInventory);
    }
    let continuation = candidate.prepared_continuation();
    let continuation_seed = if let Some(reference) = continuation {
        Some(resolve_continuation(reference)?)
    } else {
        None
    };
    let terminal = is_terminal_group(candidate);
    let implicit_extension_group = match candidate.instantiation() {
        super::CallableInstantiation::Extension { group, .. } if *group > current_group => {
            Some(*group)
        }
        _ => None,
    };
    let mut types = BTreeMap::<GenericTypeParameterId, TypeConstraintParameterEligibility>::new();
    let mut consts = BTreeMap::<GenericConstParameterId, TypeConstraintConstEligibility>::new();
    for parameter in enclosing.types() {
        if types
            .insert(parameter.clone(), TypeConstraintParameterEligibility::Rigid)
            .is_some()
        {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
    }
    for parameter in enclosing.consts() {
        if consts
            .insert(parameter.clone(), TypeConstraintConstEligibility::Rigid)
            .is_some()
        {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
    }

    let mut required = Vec::new();
    for entry in inventory.types() {
        let eligibility = match (entry.role(), entry.first_use()) {
            (super::CallableSchemaGenericRole::RigidReference, _) => {
                TypeConstraintParameterEligibility::Rigid
            }
            (
                super::CallableSchemaGenericRole::Candidate,
                CallableGenericFirstUse::Group(group),
            ) if group < current_group => {
                required.push(entry.parameter().clone());
                TypeConstraintParameterEligibility::Bindable
            }
            (
                super::CallableSchemaGenericRole::Candidate,
                CallableGenericFirstUse::Group(group),
            ) if group == current_group || Some(group) == implicit_extension_group => {
                TypeConstraintParameterEligibility::Bindable
            }
            (super::CallableSchemaGenericRole::Candidate, CallableGenericFirstUse::Group(_))
                if !terminal =>
            {
                TypeConstraintParameterEligibility::FutureEligible
            }
            (super::CallableSchemaGenericRole::Candidate, CallableGenericFirstUse::Result)
                if !terminal =>
            {
                TypeConstraintParameterEligibility::FutureEligible
            }
            (super::CallableSchemaGenericRole::Candidate, CallableGenericFirstUse::Result)
                if terminal =>
            {
                TypeConstraintParameterEligibility::Bindable
            }
            (super::CallableSchemaGenericRole::Candidate, _) => {
                return Err(CallConstraintInvariant::TerminalFutureEligibleParameter);
            }
        };
        if types
            .insert(entry.parameter().clone(), eligibility)
            .is_some()
        {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
    }
    let mut required_consts = Vec::new();
    for entry in inventory.consts() {
        let eligibility = match (entry.role(), entry.first_use()) {
            (super::CallableSchemaGenericRole::RigidReference, _) => {
                TypeConstraintConstEligibility::Rigid
            }
            (
                super::CallableSchemaGenericRole::Candidate,
                CallableGenericFirstUse::Group(group),
            ) if group < current_group => {
                required_consts.push(entry.parameter().clone());
                TypeConstraintConstEligibility::Bindable
            }
            (
                super::CallableSchemaGenericRole::Candidate,
                CallableGenericFirstUse::Group(group),
            ) if group == current_group || Some(group) == implicit_extension_group => {
                TypeConstraintConstEligibility::Bindable
            }
            (super::CallableSchemaGenericRole::Candidate, CallableGenericFirstUse::Group(_))
                if !terminal =>
            {
                TypeConstraintConstEligibility::FutureEligible
            }
            (super::CallableSchemaGenericRole::Candidate, CallableGenericFirstUse::Result)
                if !terminal =>
            {
                TypeConstraintConstEligibility::FutureEligible
            }
            (super::CallableSchemaGenericRole::Candidate, CallableGenericFirstUse::Result)
                if terminal =>
            {
                TypeConstraintConstEligibility::Bindable
            }
            (super::CallableSchemaGenericRole::Candidate, _) => {
                return Err(CallConstraintInvariant::TerminalFutureEligibleParameter);
            }
        };
        if consts
            .insert(entry.parameter().clone(), eligibility)
            .is_some()
        {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
    }
    let future_parameters = types
        .iter()
        .filter_map(|(parameter, eligibility)| {
            matches!(
                eligibility,
                TypeConstraintParameterEligibility::FutureEligible
            )
            .then_some(parameter.clone())
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let type_rows = types.into_iter().map(|(parameter, eligibility)| {
        TypeConstraintTypeParameterScopeRow::new(parameter, eligibility)
    });
    let const_rows = consts.into_iter().map(|(parameter, eligibility)| {
        TypeConstraintConstParameterScopeRow::new(parameter, eligibility)
    });
    let scope = TypeConstraintParameterScope::seal_call_scope(
        type_rows,
        const_rows,
        required,
        required_consts,
    )
    .map_err(CallConstraintInvariant::Lower)?;

    let inherited_effects = continuation_seed
        .as_ref()
        .map(|seed| {
            seed.solution
                .effect_bindings()
                .map(|(variable, _)| *variable)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let inherited_effects = inherited_effects.into_iter().collect::<BTreeSet<_>>();
    let mut required_effects = Vec::new();
    let effect_rows = candidate
        .prepared_effect_instantiation()
        .variables()
        .iter()
        .map(|row| {
            let variable = row.variable();
            let inherited = inherited_effects.contains(&variable);
            let eligibility = if inherited {
                required_effects.push(variable);
                EffectConstraintEligibility::Bindable
            } else {
                match row.first_use() {
                    CallableGenericFirstUse::Group(group) if group < current_group => {
                        required_effects.push(variable);
                        EffectConstraintEligibility::Bindable
                    }
                    CallableGenericFirstUse::Group(group)
                        if group == current_group || Some(group) == implicit_extension_group =>
                    {
                        EffectConstraintEligibility::Bindable
                    }
                    CallableGenericFirstUse::Group(_) if !terminal => {
                        EffectConstraintEligibility::FutureEligible
                    }
                    CallableGenericFirstUse::Result if !terminal => {
                        EffectConstraintEligibility::FutureEligible
                    }
                    CallableGenericFirstUse::Result if terminal => {
                        EffectConstraintEligibility::Bindable
                    }
                    CallableGenericFirstUse::Result => {
                        unreachable!("terminal state exhaustively selects result eligibility")
                    }
                    CallableGenericFirstUse::Group(_) => {
                        return Err(CallConstraintInvariant::TerminalFutureEligibleParameter);
                    }
                }
            };
            Ok(EffectConstraintVariable::new(variable, eligibility))
        })
        .collect::<Result<Vec<_>, _>>()?;
    required_effects.sort_unstable();
    required_effects.dedup();
    let effect_scope = TypeConstraintEffectScope::seal_call_scope(effect_rows, required_effects)
        .map_err(CallConstraintInvariant::Lower)?;

    let seed = match continuation_seed {
        Some(seed) => PreparedCallConstraintSeed::Prepared(seed),
        None => PreparedCallConstraintSeed::None {
            issuer: Arc::clone(&issuer),
        },
    };
    Ok(PreparedConstraintInitialization {
        issuer,
        parameter_scope: scope,
        effect_scope,
        future_parameters,
        continuation_seed: seed,
    })
}

/// The only callable-to-lower initialization carrier.  Fields and the
/// constructor are private to this module; the driver can only consume it.
pub(crate) struct PreparedConstraintInitialization {
    issuer: Arc<PreparedCallGraphIssuer>,
    parameter_scope: TypeConstraintParameterScope,
    effect_scope: TypeConstraintEffectScope,
    future_parameters: Box<[GenericTypeParameterId]>,
    continuation_seed: PreparedCallConstraintSeed,
}

enum PreparedCallConstraintSeed {
    None {
        issuer: Arc<PreparedCallGraphIssuer>,
    },
    Prepared(PreparedCallContinuationSeed),
}

impl PreparedConstraintInitialization {
    pub(crate) fn future_parameters(&self) -> &[GenericTypeParameterId] {
        &self.future_parameters
    }

    pub(super) fn into_lower_parts(
        self,
    ) -> Result<
        (
            TypeConstraintParameterScope,
            TypeConstraintEffectScope,
            Option<Arc<TypeConstraintSolution>>,
        ),
        CallConstraintInvariant,
    > {
        let Self {
            issuer,
            parameter_scope,
            effect_scope,
            future_parameters: _,
            continuation_seed,
        } = self;
        let solution = match continuation_seed {
            PreparedCallConstraintSeed::None {
                issuer: seed_issuer,
            } => {
                if !Arc::ptr_eq(&issuer, &seed_issuer) {
                    return Err(CallConstraintInvariant::ForeignPreparedIssuer);
                }
                None
            }
            PreparedCallConstraintSeed::Prepared(seed) => {
                if !Arc::ptr_eq(&issuer, &seed.coordinate.issuer) {
                    return Err(CallConstraintInvariant::ForeignPreparedIssuer);
                }
                Some(seed.into_solution())
            }
        };
        Ok((parameter_scope, effect_scope, solution))
    }
}

#[cfg(test)]
mod initialization_tests {
    use super::*;

    #[test]
    fn foreign_initialization_issuer_is_rejected_before_lower_open() {
        let initialization = crate::callable::constraints::tests::no_constraint_initialization();
        let PreparedConstraintInitialization {
            issuer: _,
            parameter_scope,
            effect_scope,
            future_parameters,
            continuation_seed,
        } = initialization;
        let foreign = PreparedConstraintInitialization {
            issuer: Arc::new(PreparedCallGraphIssuer),
            parameter_scope,
            effect_scope,
            future_parameters,
            continuation_seed,
        };
        assert!(matches!(
            foreign.into_lower_parts(),
            Err(CallConstraintInvariant::ForeignPreparedIssuer)
        ));
    }
}

fn strictly_ordered<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn deferred_for_candidate(
    candidate: &super::PreparedResolvedCallable,
    completed_group: CallableGroupIndex,
) -> DeferredContinuationInventory {
    let implicit_extension_group = match candidate.instantiation() {
        super::CallableInstantiation::Extension { group, .. } => Some(*group),
        _ => None,
    };
    let types = candidate
        .schema()
        .generic_inventory()
        .types()
        .iter()
        .filter_map(|entry| match (entry.role(), entry.first_use()) {
            (
                super::CallableSchemaGenericRole::Candidate,
                CallableGenericFirstUse::Group(group),
            ) if group > completed_group && Some(group) != implicit_extension_group => {
                Some(DeferredContinuationParameter {
                    parameter: entry.parameter().clone(),
                    first_remaining_group: group,
                })
            }
            _ => None,
        })
        .collect();
    let consts = candidate
        .schema()
        .generic_inventory()
        .consts()
        .iter()
        .filter_map(|entry| match (entry.role(), entry.first_use()) {
            (
                super::CallableSchemaGenericRole::Candidate,
                CallableGenericFirstUse::Group(group),
            ) if group > completed_group && Some(group) != implicit_extension_group => {
                Some(DeferredContinuationConstParameter {
                    parameter: entry.parameter().clone(),
                    first_remaining_group: group,
                })
            }
            _ => None,
        })
        .collect();
    DeferredContinuationInventory { types, consts }
}

fn is_terminal_group(candidate: &super::PreparedResolvedCallable) -> bool {
    candidate.next_group_for(candidate.call_group()).is_none()
}
