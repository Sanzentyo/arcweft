//! Selected expression-owner inventory for one executable final-HIR project.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

use thiserror::Error;

use crate::dialogue_application::{
    HirAttachedContentApplication, HirAttachedContentApplicationFamily,
    HirContentCallSemanticEvidence, HirPostfixBracket, HirPostfixBracketCandidates,
};
use crate::expr::{
    HirCallInvocation, HirExprKind, HirExpressionChildOwnership, HirExpressionChildRole,
};
use crate::identity::{ExprId, HirModuleId, SyntheticOwner, TypeId};
use crate::module::HirModule;

use super::{
    HirAnalysisProjectView, HirExpressionEvaluationEdge, HirProjectEvaluationTopology,
    HirRuntimeSemanticReachability,
};

pub(super) struct HirSelectedRuntimeExpressionOwners {
    pub(super) reached: BTreeSet<ExprId>,
    pub(super) typed: BTreeSet<ExprId>,
    pub(super) edges: BTreeMap<ExprId, Box<[HirExpressionEvaluationEdge]>>,
}

/// Topology-bound semantic expression graph after bounded alternatives have
/// been selected by the higher-layer checked facts.
///
/// HIR owns traversal, ordered edges, ownership filtering, and candidate
/// membership. Consumers may project the graph but cannot reconstruct its
/// edges from owner membership.
#[derive(Debug, Eq, PartialEq)]
pub struct HirSelectedExpressionGraph {
    topology: Arc<HirProjectEvaluationTopology>,
    owners: BTreeSet<SyntheticOwner>,
    edges: BTreeMap<ExprId, Box<[HirExpressionEvaluationEdge]>>,
    type_roots: BTreeSet<TypeId>,
}

/// Exact semantic child inventory selected for one accepted Call. Raw HIR
/// edges remain the sole evaluation-ownership authority; metadata rows also
/// name the enclosing semantic owner that must be selected in the same graph.
/// `None` is the closed representation for a static namespace/type spelling,
/// not permission to rediscover a callee from syntax.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSelectedCallExpressionInventory {
    arguments: Box<[HirSelectedCallArgument]>,
    callee: Option<ExprId>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSelectedCallArgument {
    expression: ExprId,
    semantic_owner: Option<ExprId>,
}

impl HirSelectedCallArgument {
    pub const fn new(expression: ExprId) -> Self {
        Self {
            expression,
            semantic_owner: None,
        }
    }

    pub const fn with_semantic_owner(expression: ExprId, semantic_owner: ExprId) -> Self {
        Self {
            expression,
            semantic_owner: Some(semantic_owner),
        }
    }

    pub const fn expression(self) -> ExprId {
        self.expression
    }

    pub const fn semantic_owner(self) -> Option<ExprId> {
        self.semantic_owner
    }
}

/// Higher-layer disposition of a raw final-HIR Call expression. A structural
/// Call-shaped expression (for example a scoped effect operand) follows its
/// ordinary HIR edges and is not required to own a callable graph node.
/// Callable applications must provide their closed mapper/callee inventory.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSelectedCallExpressionDisposition {
    Structural,
    Callable(HirSelectedCallExpressionInventory),
}

impl HirSelectedCallExpressionInventory {
    pub fn new(arguments: Box<[ExprId]>, callee: Option<ExprId>) -> Self {
        Self::with_argument_semantics(
            arguments
                .into_vec()
                .into_iter()
                .map(HirSelectedCallArgument::new)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            callee,
        )
    }

    pub fn with_argument_semantics(
        arguments: Box<[HirSelectedCallArgument]>,
        callee: Option<ExprId>,
    ) -> Self {
        Self { arguments, callee }
    }

    pub fn arguments(&self) -> &[HirSelectedCallArgument] {
        &self.arguments
    }

    pub const fn callee(&self) -> Option<ExprId> {
        self.callee
    }
}

impl HirSelectedExpressionGraph {
    /// Whether an arena owner belongs to the selected interpretations in this
    /// graph. Expression membership also observes the accepted call inventory.
    pub fn contains_owner(&self, owner: SyntheticOwner) -> bool {
        self.owners.contains(&owner)
    }

    /// Tests interpretation containment independently of expression disposition.
    /// A reference-only callee is still subject to its required syntax region.
    pub fn selects_owner_region(&self, owner: SyntheticOwner) -> bool {
        let Some(module) = self.topology.module(owner.module()) else {
            return false;
        };
        let provenance = module.candidate_provenance();
        let mut region = provenance.owner_region(owner);
        while let Some(current) = region {
            if !self
                .expression_edges(current.selector())
                .iter()
                .any(|edge| {
                    matches!(edge, HirExpressionEvaluationEdge::Expression { role, child, .. }
                    if role == &current.interpretation().edge_role() && *child == current.root())
                })
            {
                return false;
            }
            region = current
                .parent()
                .and_then(|parent| provenance.region(parent));
        }
        true
    }

    pub fn topology(&self) -> &Arc<HirProjectEvaluationTopology> {
        &self.topology
    }

    pub fn expression_owners(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.owners.iter().filter_map(|owner| match owner {
            SyntheticOwner::Expr(owner) => Some(*owner),
            _ => None,
        })
    }

    /// Returns whether this exact checked graph selected one expression owner.
    ///
    /// Selection is intentionally queried through the sealed graph rather
    /// than reconstructed from HIR child edges. This lets downstream HIR
    /// acceptance distinguish an outer postfix source site from its selected
    /// synthetic dialogue candidate.
    pub fn contains_expression(&self, owner: ExprId) -> bool {
        self.owners.contains(&SyntheticOwner::Expr(owner))
    }

    pub fn expression_edges(&self, owner: ExprId) -> &[HirExpressionEvaluationEdge] {
        self.edges.get(&owner).map_or(&[], Box::as_ref)
    }

    /// Returns every type root reached by the complete semantic expression
    /// graph, including roots that are intentionally semantic-only at the
    /// runtime boundary.
    pub fn type_roots(&self) -> impl Iterator<Item = TypeId> + '_ {
        self.type_roots.iter().copied()
    }
}

struct HirSelectedExpressionTraversal {
    reached: BTreeSet<ExprId>,
    typed: BTreeSet<ExprId>,
    edges: BTreeMap<ExprId, Box<[HirExpressionEvaluationEdge]>>,
    type_roots: BTreeSet<TypeId>,
}

struct SelectedExpressionTraversalInput<'a, Postfix, Calls, Disposition> {
    domain: SelectedExpressionDomain,
    topology: &'a HirProjectEvaluationTopology,
    outer_owners: Option<&'a BTreeSet<ExprId>>,
    execution_roots: &'a [ExprId],
    selected_postfix: Postfix,
    selected_call_edges: Calls,
    expression_disposition: Disposition,
}

/// Failure to resolve the expression owners selected by a higher-layer
/// postfix decision.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirSelectedExpressionInventoryError {
    #[error("selected expression topology does not belong to this exact executable project")]
    TopologyMismatch,
    #[error("selected expression owner references unknown HIR module {module:?}")]
    UnknownModule { module: HirModuleId },
    #[error("selected expression owner references unresolved expression {expression:?}")]
    UnresolvedExpression { expression: ExprId },
    #[error("postfix expression {expression:?} has no selected candidate")]
    MissingPostfixSelection { expression: ExprId },
    #[error(
        "expression {candidate:?} is not one of postfix expression {expression:?}'s candidates"
    )]
    InvalidPostfixSelection {
        expression: ExprId,
        candidate: ExprId,
    },
    #[error("expression {expression:?} has a runtime call disposition but is not a Call")]
    InvalidRuntimeCallDisposition { expression: ExprId },
    #[error("Call expression {expression:?} has a structural runtime projection")]
    InvalidRuntimeStructuralDisposition { expression: ExprId },
    #[error("expression {expression:?} has no runtime projection")]
    MissingRuntimeExpressionProjection { expression: ExprId },
    #[error("non-value runtime expression {expression:?} has a retained value projection")]
    InvalidRuntimeValueRetention { expression: ExprId },
    #[error("selected runtime call {expression:?} requires a runtime receiver but has none")]
    MissingRuntimeCallReceiver { expression: ExprId },
    #[error("selected semantic call {expression:?} has no accepted call-edge inventory")]
    MissingSelectedCallEdges { expression: ExprId },
    #[error("selected semantic call {expression:?} names illegal callee expression {callee:?}")]
    InvalidSelectedCallCallee { expression: ExprId, callee: ExprId },
    #[error("selected semantic call {expression:?} has an invalid authored argument inventory")]
    InvalidSelectedCallArguments { expression: ExprId },
    #[error("selected expression traversal did not close over its ordered owning edges")]
    InvalidSelectedGraph,
    #[error("selected syntax region contains recovered HIR owner {owner:?}")]
    RecoveredOwner { owner: SyntheticOwner },
}

/// Accepted use of a final-HIR call callee in runtime lowering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeCallCalleeDisposition {
    /// The accepted target is selected statically, so the callee subtree is
    /// not a runtime value operand.
    Static,
    /// The accepted call arguments include the call's value receiver.
    RuntimeReceiver,
}

/// Whether one reached HIR expression publishes a runtime value/type fact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeValueRetention {
    Retain,
    Omit,
}

/// Exact checked projection of one HIR expression at the runtime boundary.
/// The shape axis prevents structural traversal from standing in for an
/// accepted Call application, while retention controls only owner publication.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeExpressionProjection {
    Structural {
        value: HirRuntimeValueRetention,
    },
    Call {
        result: HirRuntimeValueRetention,
        callee: HirRuntimeCallCalleeDisposition,
    },
}

impl HirAnalysisProjectView<'_> {
    /// Returns the exact expression graph reachable after bounded postfix
    /// ambiguity has been resolved by the supplied accepted decisions.
    ///
    /// HIR owns graph traversal and candidate membership. The callback remains
    /// the sole higher-layer authority for which candidate semantic analysis
    /// accepted; this method neither infers nor stores that decision.
    pub fn selected_expression_graph(
        self,
        topology: &Arc<HirProjectEvaluationTopology>,
        selected_postfix: impl FnMut(ExprId) -> Option<ExprId>,
        selected_call_edges: impl FnMut(ExprId) -> Option<HirSelectedCallExpressionDisposition>,
    ) -> Result<HirSelectedExpressionGraph, HirSelectedExpressionInventoryError> {
        let traversal =
            self.selected_expression_owners_in_domain(SelectedExpressionTraversalInput {
                domain: SelectedExpressionDomain::SemanticAnalysis,
                topology,
                outer_owners: None,
                execution_roots: &[],
                selected_postfix,
                selected_call_edges,
                expression_disposition: |_| {
                    Some(HirRuntimeExpressionProjection::Structural {
                        value: HirRuntimeValueRetention::Retain,
                    })
                },
            })?;
        if traversal.reached != traversal.typed
            || traversal.edges.len() != traversal.typed.len()
            || traversal.edges.iter().any(|(owner, edges)| {
                !traversal.typed.contains(owner)
                    || edges
                        .iter()
                        .any(|edge| !traversal.typed.contains(&edge.child()))
            })
        {
            return Err(HirSelectedExpressionInventoryError::InvalidSelectedGraph);
        }
        let mut graph = HirSelectedExpressionGraph {
            topology: Arc::clone(topology),
            owners: traversal
                .typed
                .into_iter()
                .map(SyntheticOwner::Expr)
                .collect(),
            edges: traversal.edges,
            type_roots: traversal.type_roots,
        };
        for (_, module) in self.modules() {
            for owner in module.slots().poisoned_live_owners() {
                if graph.selects_owner_region(owner) {
                    return Err(HirSelectedExpressionInventoryError::RecoveredOwner { owner });
                }
            }
            for (owner, statement) in module.statements() {
                if super::HirControlTransferKind::from_statement(statement.kind()).is_some()
                    && graph.selects_owner_region(SyntheticOwner::Stmt(owner))
                    && !topology
                        .control_transfer_row(owner)
                        .is_ok_and(|row| row.target().is_ok())
                {
                    return Err(HirSelectedExpressionInventoryError::InvalidSelectedGraph);
                }
            }
            let selected = module
                .items()
                .map(|(owner, _)| SyntheticOwner::Item(owner))
                .chain(
                    module
                        .statements()
                        .map(|(owner, _)| SyntheticOwner::Stmt(owner)),
                )
                .chain(
                    module
                        .patterns()
                        .map(|(owner, _)| SyntheticOwner::Pattern(owner)),
                )
                .chain(module.types().map(|(owner, _)| SyntheticOwner::Type(owner)))
                .chain(
                    module
                        .locals()
                        .map(|(owner, _)| SyntheticOwner::Local(owner)),
                )
                .chain(
                    module
                        .scopes()
                        .map(|(owner, _)| SyntheticOwner::Scope(owner)),
                )
                .filter(|owner| graph.selects_owner_region(*owner))
                .collect::<Vec<_>>();
            graph.owners.extend(selected);
            let captures = module
                .captures()
                .filter(|(_, capture)| {
                    graph.contains_expression(capture.closure())
                        && capture.uses().iter().any(|use_site| {
                            graph
                                .selects_owner_region(SyntheticOwner::Expr(use_site.site().owner()))
                        })
                })
                .map(|(owner, _)| SyntheticOwner::Capture(owner))
                .collect::<Vec<_>>();
            graph.owners.extend(captures);
        }
        Ok(graph)
    }

    pub(super) fn selected_runtime_expression_owners(
        self,
        topology: &HirProjectEvaluationTopology,
        outer_owners: &BTreeSet<ExprId>,
        execution_roots: &[ExprId],
        selected_postfix: impl FnMut(ExprId) -> Option<ExprId>,
        expression_disposition: impl FnMut(ExprId) -> Option<HirRuntimeExpressionProjection>,
    ) -> Result<HirSelectedRuntimeExpressionOwners, HirSelectedExpressionInventoryError> {
        let traversal =
            self.selected_expression_owners_in_domain(SelectedExpressionTraversalInput {
                domain: SelectedExpressionDomain::RuntimeType,
                topology,
                outer_owners: Some(outer_owners),
                execution_roots,
                selected_postfix,
                selected_call_edges: |_| None,
                expression_disposition,
            })?;
        Ok(HirSelectedRuntimeExpressionOwners {
            reached: traversal.reached,
            typed: traversal.typed,
            edges: traversal.edges,
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one traversal keeps selected-call projection, ownership, and fail-closed completeness atomic"
    )]
    fn selected_expression_owners_in_domain<Postfix, Calls, Disposition>(
        self,
        input: SelectedExpressionTraversalInput<'_, Postfix, Calls, Disposition>,
    ) -> Result<HirSelectedExpressionTraversal, HirSelectedExpressionInventoryError>
    where
        Postfix: FnMut(ExprId) -> Option<ExprId>,
        Calls: FnMut(ExprId) -> Option<HirSelectedCallExpressionDisposition>,
        Disposition: FnMut(ExprId) -> Option<HirRuntimeExpressionProjection>,
    {
        let SelectedExpressionTraversalInput {
            domain,
            topology,
            outer_owners,
            execution_roots,
            mut selected_postfix,
            mut selected_call_edges,
            mut expression_disposition,
        } = input;
        validate_selection_topology(self, topology)?;
        let modules = selected_expression_modules(self);
        let mut pending = selected_expression_pending(topology, outer_owners, execution_roots);
        let excluded_roots = selected_expression_excluded_roots(self, domain);
        let mut visited = BTreeSet::new();
        let mut selected = BTreeSet::new();
        let mut selected_edges = BTreeMap::new();
        let mut type_roots = BTreeSet::new();
        let mut required_semantic_owners = BTreeSet::new();

        while let Some(owner) = pending.pop_front() {
            if excluded_roots.contains(&owner)
                || outer_owners.is_some_and(|outer| !outer.contains(&owner))
                || !visited.insert(owner)
            {
                continue;
            }
            let kind = resolve_expression(&modules, owner)?;
            type_roots.extend(
                kind.direct_type_roots()
                    .into_iter()
                    .map(crate::expr::HirExpressionTypeRoot::type_id),
            );
            if domain == SelectedExpressionDomain::SemanticAnalysis
                && matches!(kind, HirExprKind::Call(_))
            {
                let selected_call = selected_call_edges(owner).ok_or(
                    HirSelectedExpressionInventoryError::MissingSelectedCallEdges {
                        expression: owner,
                    },
                )?;
                if apply_selected_semantic_call(
                    topology,
                    owner,
                    kind,
                    selected_call,
                    &mut pending,
                    &mut selected,
                    &mut selected_edges,
                    &mut required_semantic_owners,
                )? {
                    continue;
                }
            }
            if domain == SelectedExpressionDomain::SemanticAnalysis
                && let HirExprKind::AttachedContentApplication(application) = kind
                && let HirAttachedContentApplicationFamily::ContentCall { invocation, .. } =
                    application.family()
                && invocation.form() == crate::expr::HirCallInvocationForm::Parenthesized
            {
                // Language-owned content callees are static namespace
                // identities. Their path is checked through the attached
                // callable fact and is not a semantic expression operand;
                // ordinary authored arguments and the attached body remain
                // part of the selected graph.
                selected.insert(owner);
                let mut followed_edges = Vec::new();
                append_selected_language_content_operands(
                    topology,
                    owner,
                    invocation,
                    &mut pending,
                    &mut followed_edges,
                )?;
                selected_edges.insert(owner, followed_edges.into_boxed_slice());
                continue;
            }
            let projection = if domain == SelectedExpressionDomain::RuntimeType {
                expression_disposition(owner).ok_or(
                    HirSelectedExpressionInventoryError::MissingRuntimeExpressionProjection {
                        expression: owner,
                    },
                )?
            } else {
                HirRuntimeExpressionProjection::Structural {
                    value: HirRuntimeValueRetention::Retain,
                }
            };
            let mut followed_edges = Vec::new();
            let value = match (kind, projection) {
                (
                    HirExprKind::Call(call),
                    HirRuntimeExpressionProjection::Call { result, callee },
                ) => {
                    if result == HirRuntimeValueRetention::Retain {
                        selected.insert(owner);
                    }
                    append_selected_invocation_operands(
                        topology,
                        &modules,
                        owner,
                        call,
                        callee,
                        &mut pending,
                        &mut followed_edges,
                    )?;
                    selected_edges.insert(owner, followed_edges.into_boxed_slice());
                    continue;
                }
                (HirExprKind::Call(_), HirRuntimeExpressionProjection::Structural { value })
                    if domain == SelectedExpressionDomain::SemanticAnalysis =>
                {
                    value
                }
                (HirExprKind::Call(_), HirRuntimeExpressionProjection::Structural { .. }) => {
                    return Err(
                        HirSelectedExpressionInventoryError::InvalidRuntimeStructuralDisposition {
                            expression: owner,
                        },
                    );
                }
                (HirExprKind::AttachedContentApplication(application), projection)
                    if domain == SelectedExpressionDomain::RuntimeType =>
                {
                    let value = append_selected_attached_content_operands(
                        topology,
                        &modules,
                        owner,
                        application,
                        projection,
                        &mut pending,
                        &mut followed_edges,
                    )?;
                    selected_edges.insert(owner, followed_edges.into_boxed_slice());
                    if value == HirRuntimeValueRetention::Retain {
                        selected.insert(owner);
                    }
                    continue;
                }
                (_, HirRuntimeExpressionProjection::Call { .. }) => {
                    return Err(
                        HirSelectedExpressionInventoryError::InvalidRuntimeCallDisposition {
                            expression: owner,
                        },
                    );
                }
                (_, HirRuntimeExpressionProjection::Structural { value }) => value,
            };
            match kind {
                HirExprKind::PostfixBracket(postfix) => {
                    let candidate = selected_postfix(owner).ok_or(
                        HirSelectedExpressionInventoryError::MissingPostfixSelection {
                            expression: owner,
                        },
                    )?;
                    SelectedPostfixContext {
                        topology,
                        owner,
                        postfix,
                        domain,
                        value,
                        pending: &mut pending,
                        selected: &mut selected,
                        followed: &mut followed_edges,
                    }
                    .apply(candidate)?;
                }
                _ => {
                    if value == HirRuntimeValueRetention::Retain {
                        selected.insert(owner);
                    }
                    enqueue_expression_edges(topology, owner, &mut pending, &mut followed_edges);
                }
            }
            selected_edges.insert(owner, followed_edges.into_boxed_slice());
        }
        if !required_semantic_owners.is_subset(&selected) {
            return Err(HirSelectedExpressionInventoryError::InvalidSelectedGraph);
        }
        Ok(HirSelectedExpressionTraversal {
            reached: visited,
            typed: selected,
            edges: selected_edges,
            type_roots,
        })
    }
}

fn selected_expression_modules(
    view: HirAnalysisProjectView<'_>,
) -> BTreeMap<HirModuleId, &HirModule> {
    view.modules()
        .map(|(_, module)| (module.module_id(), module.as_ref()))
        .collect()
}

fn selected_expression_pending(
    topology: &HirProjectEvaluationTopology,
    outer_owners: Option<&BTreeSet<ExprId>>,
    execution_roots: &[ExprId],
) -> VecDeque<ExprId> {
    let mut pending = topology
        .selection_roots()
        .filter(|owner| outer_owners.is_none_or(|outer| outer.contains(owner)))
        .collect::<VecDeque<_>>();
    pending.extend(execution_roots.iter().copied());
    pending
}

fn selected_expression_excluded_roots(
    view: HirAnalysisProjectView<'_>,
    domain: SelectedExpressionDomain,
) -> BTreeSet<ExprId> {
    if domain == SelectedExpressionDomain::RuntimeType {
        view.items()
            .flat_map(|item| item.item().kind().effect_expression_roots())
            .collect()
    } else {
        BTreeSet::new()
    }
}

fn apply_selected_semantic_call(
    topology: &HirProjectEvaluationTopology,
    owner: ExprId,
    kind: &HirExprKind,
    disposition: HirSelectedCallExpressionDisposition,
    pending: &mut VecDeque<ExprId>,
    selected: &mut BTreeSet<ExprId>,
    selected_edges: &mut BTreeMap<ExprId, Box<[HirExpressionEvaluationEdge]>>,
    required_semantic_owners: &mut BTreeSet<ExprId>,
) -> Result<bool, HirSelectedExpressionInventoryError> {
    let HirSelectedCallExpressionDisposition::Callable(call) = disposition else {
        return Ok(false);
    };
    selected.insert(owner);
    let mut followed_edges = Vec::new();
    append_selected_call_expression_edges(
        topology,
        owner,
        kind,
        call.arguments(),
        call.callee(),
        pending,
        &mut followed_edges,
        required_semantic_owners,
    )?;
    selected_edges.insert(owner, followed_edges.into_boxed_slice());
    Ok(true)
}

impl HirRuntimeSemanticReachability<'_> {
    /// Returns the exact retained expression owners whose accepted types enter
    /// runtime lowering after bounded postfix ambiguity has been resolved.
    ///
    /// Effect metadata and non-value dialogue carrier nodes remain in the
    /// semantic inventory but do not publish runtime type facts. Their runtime
    /// operands are still traversed from the same HIR-owned graph authority.
    /// The second callback supplies the accepted use of selected call carriers;
    /// HIR validates that a call-only disposition cannot hide another family.
    pub fn selected_expression_type_owners(
        &self,
    ) -> Result<BTreeSet<ExprId>, HirSelectedExpressionInventoryError> {
        Ok(self.expression_type_owners().clone())
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SelectedExpressionDomain {
    SemanticAnalysis,
    RuntimeType,
}

struct SelectedPostfixContext<'a> {
    topology: &'a HirProjectEvaluationTopology,
    owner: ExprId,
    postfix: &'a HirPostfixBracket,
    domain: SelectedExpressionDomain,
    value: HirRuntimeValueRetention,
    pending: &'a mut VecDeque<ExprId>,
    selected: &'a mut BTreeSet<ExprId>,
    followed: &'a mut Vec<HirExpressionEvaluationEdge>,
}

impl SelectedPostfixContext<'_> {
    fn apply(&mut self, candidate: ExprId) -> Result<(), HirSelectedExpressionInventoryError> {
        let selected_index = match self.postfix.candidates() {
            HirPostfixBracketCandidates::Ambiguous { index, dialogue }
                if candidate == *index || candidate == *dialogue =>
            {
                candidate == *index
            }
            HirPostfixBracketCandidates::Ambiguous { .. }
            | HirPostfixBracketCandidates::Invalid { .. } => {
                return Err(
                    HirSelectedExpressionInventoryError::InvalidPostfixSelection {
                        expression: self.owner,
                        candidate,
                    },
                );
            }
        };
        let edges = self.topology.expression_edges(self.owner);
        let target = edges.iter().find(|edge| {
            matches!(
                edge,
                HirExpressionEvaluationEdge::Expression {
                    role: HirExpressionChildRole::Target,
                    ownership: HirExpressionChildOwnership::Owning,
                    ..
                }
            )
        });
        let candidate_edge = edges.iter().find(|edge| match edge {
            HirExpressionEvaluationEdge::Expression {
                role: HirExpressionChildRole::PostfixIndexCandidate,
                ownership: HirExpressionChildOwnership::Owning,
                child,
            } if selected_index && *child == candidate => true,
            HirExpressionEvaluationEdge::Expression {
                role: HirExpressionChildRole::PostfixDialogueCandidate,
                ownership: HirExpressionChildOwnership::Owning,
                child,
            } if !selected_index && *child == candidate => true,
            _ => false,
        });
        let target = target.ok_or(HirSelectedExpressionInventoryError::UnresolvedExpression {
            expression: self.owner,
        })?;
        let candidate_edge = candidate_edge.ok_or(
            HirSelectedExpressionInventoryError::InvalidPostfixSelection {
                expression: self.owner,
                candidate,
            },
        )?;
        if self.domain == SelectedExpressionDomain::SemanticAnalysis
            || (selected_index && self.value == HirRuntimeValueRetention::Retain)
        {
            self.selected.insert(self.owner);
        }
        self.pending
            .extend([target.child(), candidate_edge.child()]);
        self.followed
            .extend([target.clone(), candidate_edge.clone()]);
        Ok(())
    }
}

fn append_selected_call_expression_edges(
    topology: &HirProjectEvaluationTopology,
    owner: ExprId,
    kind: &HirExprKind,
    arguments: &[HirSelectedCallArgument],
    callee: Option<ExprId>,
    pending: &mut VecDeque<ExprId>,
    followed: &mut Vec<HirExpressionEvaluationEdge>,
    required_semantic_owners: &mut BTreeSet<ExprId>,
) -> Result<(), HirSelectedExpressionInventoryError> {
    let HirExprKind::Call(invocation) = kind else {
        return Err(
            HirSelectedExpressionInventoryError::InvalidRuntimeCallDisposition {
                expression: owner,
            },
        );
    };
    let argument_edges = topology
        .expression_edges(owner)
        .iter()
        .filter(|edge| {
            matches!(
                edge,
                HirExpressionEvaluationEdge::Expression {
                    role: HirExpressionChildRole::Argument { .. },
                    ..
                }
            )
        })
        .collect::<Vec<_>>();
    if invocation.arguments().len() != arguments.len() {
        return Err(
            HirSelectedExpressionInventoryError::InvalidSelectedCallArguments { expression: owner },
        );
    }
    let mut consumed_edges = BTreeSet::new();
    for (ordinal, (authored, selected)) in invocation.arguments().iter().zip(arguments).enumerate()
    {
        if authored.value() != selected.expression() {
            return Err(
                HirSelectedExpressionInventoryError::InvalidSelectedCallArguments {
                    expression: owner,
                },
            );
        }
        let matching = argument_edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                edge.child() == selected.expression()
                    && matches!(
                        edge,
                        HirExpressionEvaluationEdge::Expression {
                            role: HirExpressionChildRole::Argument { ordinal: actual },
                            ..
                        } if usize::try_from(*actual).ok() == Some(ordinal)
                    )
            })
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(
                HirSelectedExpressionInventoryError::InvalidSelectedCallArguments {
                    expression: owner,
                },
            );
        }
        let (edge_index, edge) = matching.first().copied().ok_or(
            HirSelectedExpressionInventoryError::InvalidSelectedCallArguments { expression: owner },
        )?;
        consumed_edges.insert(edge_index);
        if selected.semantic_owner().is_none()
            && !matches!(
                edge,
                HirExpressionEvaluationEdge::Expression {
                    ownership: HirExpressionChildOwnership::Owning,
                    ..
                }
            )
        {
            return Err(
                HirSelectedExpressionInventoryError::InvalidSelectedCallArguments {
                    expression: owner,
                },
            );
        }
        if let Some(semantic_owner) = selected.semantic_owner() {
            let semantic_edges = topology.expression_edges(semantic_owner);
            let target_count = semantic_edges
                .iter()
                .filter(|edge| {
                    matches!(
                        edge,
                        HirExpressionEvaluationEdge::Expression {
                            role: HirExpressionChildRole::DialogueTarget,
                            child,
                            ..
                        } if *child == owner
                    )
                })
                .count();
            let coordinate_count = semantic_edges
                .iter()
                .filter(|edge| {
                    matches!(
                        edge,
                        HirExpressionEvaluationEdge::Expression {
                            role: HirExpressionChildRole::DialogueCoordinate {
                                ordinal: coordinate_ordinal,
                            },
                            child,
                            ..
                        } if *child == selected.expression()
                            && usize::try_from(*coordinate_ordinal).ok() == Some(ordinal)
                    )
                })
                .count();
            if target_count != 1 || coordinate_count != 1 {
                return Err(
                    HirSelectedExpressionInventoryError::InvalidSelectedCallArguments {
                        expression: owner,
                    },
                );
            }
            required_semantic_owners.insert(semantic_owner);
        }
        match edge {
            HirExpressionEvaluationEdge::Expression {
                ownership: HirExpressionChildOwnership::Owning,
                ..
            } => {
                let edge = *edge;
                pending.push_back(edge.child());
                followed.push(edge.clone());
            }
            HirExpressionEvaluationEdge::Expression {
                ownership: HirExpressionChildOwnership::ReferenceOnly,
                ..
            } => followed.push((*edge).clone()),
            _ => {
                return Err(
                    HirSelectedExpressionInventoryError::InvalidSelectedCallArguments {
                        expression: owner,
                    },
                );
            }
        }
    }
    if consumed_edges.len() != argument_edges.len() {
        return Err(
            HirSelectedExpressionInventoryError::InvalidSelectedCallArguments { expression: owner },
        );
    }
    let Some(callee) = callee else {
        return Ok(());
    };
    let callee_edge = topology
        .expression_edges(owner)
        .iter()
        .find(|edge| {
            matches!(
                edge,
                HirExpressionEvaluationEdge::Expression {
                    role: HirExpressionChildRole::Callee,
                    ownership: HirExpressionChildOwnership::Owning,
                    child,
                } if *child == callee
            )
        })
        .cloned()
        .ok_or(
            HirSelectedExpressionInventoryError::InvalidSelectedCallCallee {
                expression: owner,
                callee,
            },
        )?;
    pending.push_back(callee);
    followed.push(callee_edge);
    Ok(())
}

fn append_selected_invocation_operands(
    topology: &HirProjectEvaluationTopology,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owner: ExprId,
    invocation: &HirCallInvocation,
    callee: HirRuntimeCallCalleeDisposition,
    pending: &mut VecDeque<ExprId>,
    followed: &mut Vec<HirExpressionEvaluationEdge>,
) -> Result<(), HirSelectedExpressionInventoryError> {
    let argument_edges = topology
        .expression_edges(owner)
        .iter()
        .filter(|edge| {
            matches!(
                edge,
                HirExpressionEvaluationEdge::Expression {
                    role: HirExpressionChildRole::Argument { .. },
                    ownership: HirExpressionChildOwnership::Owning,
                    ..
                }
            )
        })
        .collect::<Vec<_>>();
    if argument_edges.len() != invocation.arguments().len()
        || argument_edges
            .iter()
            .zip(invocation.arguments())
            .enumerate()
            .any(|(ordinal, (edge, expected))| {
                edge.child() != expected.value()
                    || !matches!(
                        edge,
                        HirExpressionEvaluationEdge::Expression {
                            role: HirExpressionChildRole::Argument { ordinal: actual },
                            ..
                        } if usize::try_from(*actual).ok() == Some(ordinal)
                    )
            })
    {
        return Err(
            HirSelectedExpressionInventoryError::InvalidSelectedCallArguments { expression: owner },
        );
    }
    for edge in argument_edges {
        pending.push_back(edge.child());
        followed.push(edge.clone());
    }
    if callee == HirRuntimeCallCalleeDisposition::Static {
        return Ok(());
    }
    let callee_owner = invocation.callee().value_expression().ok_or(
        HirSelectedExpressionInventoryError::MissingRuntimeCallReceiver { expression: owner },
    )?;
    let module = modules.get(&callee_owner.module()).copied().ok_or(
        HirSelectedExpressionInventoryError::UnknownModule {
            module: callee_owner.module(),
        },
    )?;
    let receiver = module
        .resolve_call_value_receiver(invocation)
        .map_err(
            |_| HirSelectedExpressionInventoryError::UnresolvedExpression {
                expression: callee_owner,
            },
        )?
        .ok_or(
            HirSelectedExpressionInventoryError::MissingRuntimeCallReceiver { expression: owner },
        )?;
    if let Some(edge) = topology.expression_edges(owner).iter().find(|edge| {
        matches!(
            edge,
            HirExpressionEvaluationEdge::Expression {
                role: HirExpressionChildRole::Callee,
                ownership: HirExpressionChildOwnership::Owning,
                child,
            } if *child == receiver
        )
    }) {
        pending.push_back(edge.child());
        followed.push(edge.clone());
    } else {
        pending.push_back(receiver);
    }
    Ok(())
}

fn append_selected_attached_content_operands(
    topology: &HirProjectEvaluationTopology,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owner: ExprId,
    application: &HirAttachedContentApplication,
    projection: HirRuntimeExpressionProjection,
    pending: &mut VecDeque<ExprId>,
    followed: &mut Vec<HirExpressionEvaluationEdge>,
) -> Result<HirRuntimeValueRetention, HirSelectedExpressionInventoryError> {
    let HirAttachedContentApplicationFamily::ContentCall {
        invocation,
        evidence,
    } = application.family()
    else {
        let HirRuntimeExpressionProjection::Structural { value } = projection else {
            return Err(
                HirSelectedExpressionInventoryError::InvalidRuntimeCallDisposition {
                    expression: owner,
                },
            );
        };
        if value != HirRuntimeValueRetention::Omit {
            return Err(
                HirSelectedExpressionInventoryError::InvalidRuntimeValueRetention {
                    expression: owner,
                },
            );
        }
        enqueue_expression_edges(topology, owner, pending, followed);
        return Ok(value);
    };

    match (evidence, invocation.form()) {
        (HirContentCallSemanticEvidence::TextProxyObject { .. }, _)
        | (
            HirContentCallSemanticEvidence::None,
            crate::expr::HirCallInvocationForm::Parenthesized,
        ) => {
            if invocation.form() != crate::expr::HirCallInvocationForm::Parenthesized
                || projection
                    != (HirRuntimeExpressionProjection::Structural {
                        value: HirRuntimeValueRetention::Omit,
                    })
            {
                return Err(
                    HirSelectedExpressionInventoryError::InvalidRuntimeValueRetention {
                        expression: owner,
                    },
                );
            }
            enqueue_attached_content_body_edges(topology, owner, pending, followed);
            Ok(HirRuntimeValueRetention::Omit)
        }
        (HirContentCallSemanticEvidence::None, _) => match (invocation.form(), projection) {
            (
                crate::expr::HirCallInvocationForm::Value,
                HirRuntimeExpressionProjection::Structural { value },
            ) if value == HirRuntimeValueRetention::Retain => {
                append_selected_invocation_operands(
                    topology,
                    modules,
                    owner,
                    invocation,
                    HirRuntimeCallCalleeDisposition::RuntimeReceiver,
                    pending,
                    followed,
                )?;
                enqueue_attached_content_body_edges(topology, owner, pending, followed);
                Ok(value)
            }
            (
                crate::expr::HirCallInvocationForm::Parenthesized,
                HirRuntimeExpressionProjection::Call { result, callee },
            ) if result == HirRuntimeValueRetention::Retain => {
                append_selected_invocation_operands(
                    topology, modules, owner, invocation, callee, pending, followed,
                )?;
                enqueue_attached_content_body_edges(topology, owner, pending, followed);
                Ok(result)
            }
            _ => Err(
                HirSelectedExpressionInventoryError::InvalidRuntimeCallDisposition {
                    expression: owner,
                },
            ),
        },
    }
}

fn enqueue_attached_content_body_edges(
    topology: &HirProjectEvaluationTopology,
    owner: ExprId,
    pending: &mut VecDeque<ExprId>,
    followed: &mut Vec<HirExpressionEvaluationEdge>,
) {
    for edge in topology.expression_edges(owner).iter().filter(|edge| {
        matches!(
            edge,
            HirExpressionEvaluationEdge::Expression {
                role: HirExpressionChildRole::DialogueInterpolation { .. }
                    | HirExpressionChildRole::AttachedContentApplication { .. }
                    | HirExpressionChildRole::DialoguePointActionPayload { .. },
                ownership: HirExpressionChildOwnership::Owning,
                ..
            }
        )
    }) {
        pending.push_back(edge.child());
        followed.push(edge.clone());
    }
}

fn append_selected_language_content_operands(
    topology: &HirProjectEvaluationTopology,
    owner: ExprId,
    invocation: &HirCallInvocation,
    pending: &mut VecDeque<ExprId>,
    followed: &mut Vec<HirExpressionEvaluationEdge>,
) -> Result<(), HirSelectedExpressionInventoryError> {
    let argument_edges = topology
        .expression_edges(owner)
        .iter()
        .filter(|edge| {
            matches!(
                edge,
                HirExpressionEvaluationEdge::Expression {
                    role: HirExpressionChildRole::Argument { .. },
                    ownership: HirExpressionChildOwnership::Owning,
                    ..
                }
            )
        })
        .collect::<Vec<_>>();
    if argument_edges.len() != invocation.arguments().len()
        || argument_edges
            .iter()
            .zip(invocation.arguments())
            .enumerate()
            .any(|(ordinal, (edge, expected))| {
                edge.child() != expected.value()
                    || !matches!(
                        edge,
                        HirExpressionEvaluationEdge::Expression {
                            role: HirExpressionChildRole::Argument { ordinal: actual },
                            ..
                        } if usize::try_from(*actual).ok() == Some(ordinal)
                    )
            })
    {
        return Err(
            HirSelectedExpressionInventoryError::InvalidSelectedCallArguments { expression: owner },
        );
    }
    for edge in argument_edges {
        pending.push_back(edge.child());
        followed.push(edge.clone());
    }
    enqueue_attached_content_body_edges(topology, owner, pending, followed);
    Ok(())
}

fn enqueue_expression_edges(
    topology: &HirProjectEvaluationTopology,
    owner: ExprId,
    pending: &mut VecDeque<ExprId>,
    followed: &mut Vec<HirExpressionEvaluationEdge>,
) {
    for edge in topology.expression_edges(owner).iter().filter(|edge| {
        !matches!(
            edge,
            HirExpressionEvaluationEdge::Expression {
                ownership: HirExpressionChildOwnership::ReferenceOnly,
                ..
            }
        )
    }) {
        pending.push_back(edge.child());
        followed.push(edge.clone());
    }
}

fn validate_selection_topology(
    view: HirAnalysisProjectView<'_>,
    topology: &HirProjectEvaluationTopology,
) -> Result<(), HirSelectedExpressionInventoryError> {
    if topology.package() != view.package() {
        return Err(HirSelectedExpressionInventoryError::TopologyMismatch);
    }
    let modules = view.modules().collect::<Vec<_>>();
    if modules.len() != topology.modules().len()
        || modules.iter().any(|(_, module)| {
            topology
                .module(module.module_id())
                .is_none_or(|entry| entry.snapshot() != module.snapshot_id())
        })
        || topology.modules().iter().any(|entry| {
            !modules
                .iter()
                .any(|(_, module)| module.module_id() == entry.module())
        })
    {
        return Err(HirSelectedExpressionInventoryError::TopologyMismatch);
    }
    Ok(())
}

fn resolve_expression<'project>(
    modules: &BTreeMap<HirModuleId, &'project HirModule>,
    expression: ExprId,
) -> Result<&'project HirExprKind, HirSelectedExpressionInventoryError> {
    modules
        .get(&expression.module())
        .copied()
        .ok_or(HirSelectedExpressionInventoryError::UnknownModule {
            module: expression.module(),
        })?
        .resolve_expr(expression)
        .map(crate::expr::HirExpr::kind)
        .map_err(|_| HirSelectedExpressionInventoryError::UnresolvedExpression { expression })
}
