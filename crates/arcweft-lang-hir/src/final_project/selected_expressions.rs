//! Selected expression-owner inventory for one executable final-HIR project.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

use thiserror::Error;

use crate::dialogue_application::{HirPostfixBracket, HirPostfixBracketCandidates};
use crate::expr::{HirExprKind, HirExpressionChildOwnership, HirExpressionChildRole};
use crate::identity::{ExprId, HirModuleId};
use crate::module::HirModule;

use super::{
    HirExecutableProjectView, HirExpressionEvaluationEdge, HirProjectEvaluationTopology,
    HirRuntimeSemanticReachability,
};

pub(super) struct HirSelectedRuntimeExpressionOwners {
    pub(super) reached: BTreeSet<ExprId>,
    pub(super) typed: BTreeSet<ExprId>,
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
    owners: BTreeSet<ExprId>,
    edges: BTreeMap<ExprId, Box<[HirExpressionEvaluationEdge]>>,
}

/// Exact semantic child inventory selected for one accepted Call. Authored
/// arguments remain HIR-owned and are always followed; the higher layer owns
/// whether one already-checked callee expression is semantically retained.
/// `None` is the closed representation for a static namespace/type spelling,
/// not permission to rediscover a callee from syntax.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSelectedCallExpressionInventory {
    arguments: Box<[ExprId]>,
    callee: Option<ExprId>,
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
        Self { arguments, callee }
    }

    pub fn arguments(&self) -> &[ExprId] {
        &self.arguments
    }

    pub const fn callee(&self) -> Option<ExprId> {
        self.callee
    }
}

impl HirSelectedExpressionGraph {
    pub fn topology(&self) -> &Arc<HirProjectEvaluationTopology> {
        &self.topology
    }

    pub fn expression_owners(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.owners.iter().copied()
    }

    pub fn expression_edges(&self, owner: ExprId) -> &[HirExpressionEvaluationEdge] {
        self.edges.get(&owner).map_or(&[], Box::as_ref)
    }
}

struct HirSelectedExpressionTraversal {
    reached: BTreeSet<ExprId>,
    typed: BTreeSet<ExprId>,
    edges: BTreeMap<ExprId, Box<[HirExpressionEvaluationEdge]>>,
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

impl HirExecutableProjectView<'_> {
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
        Ok(HirSelectedExpressionGraph {
            topology: Arc::clone(topology),
            owners: traversal.typed,
            edges: traversal.edges,
        })
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

        while let Some(owner) = pending.pop_front() {
            if excluded_roots.contains(&owner)
                || outer_owners.is_some_and(|outer| !outer.contains(&owner))
                || !visited.insert(owner)
            {
                continue;
            }
            let kind = resolve_expression(&modules, owner)?;
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
                )? {
                    continue;
                }
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
                (HirExprKind::Call(_), HirRuntimeExpressionProjection::Call { result, callee }) => {
                    if result == HirRuntimeValueRetention::Retain {
                        selected.insert(owner);
                    }
                    append_selected_call_operands(
                        topology,
                        &modules,
                        owner,
                        kind,
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
                HirExprKind::DialogueContentApplication(_)
                    if domain == SelectedExpressionDomain::RuntimeType =>
                {
                    if value != HirRuntimeValueRetention::Omit {
                        return Err(
                            HirSelectedExpressionInventoryError::InvalidRuntimeValueRetention {
                                expression: owner,
                            },
                        );
                    }
                    enqueue_expression_edges(topology, owner, &mut pending, &mut followed_edges);
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
        Ok(HirSelectedExpressionTraversal {
            reached: visited,
            typed: selected,
            edges: selected_edges,
        })
    }
}

fn selected_expression_modules(
    view: HirExecutableProjectView<'_>,
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
    view: HirExecutableProjectView<'_>,
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
    arguments: &[ExprId],
    callee: Option<ExprId>,
    pending: &mut VecDeque<ExprId>,
    followed: &mut Vec<HirExpressionEvaluationEdge>,
) -> Result<(), HirSelectedExpressionInventoryError> {
    let HirExprKind::Call(_) = kind else {
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
                    ownership: HirExpressionChildOwnership::Owning,
                    ..
                }
            )
        })
        .collect::<Vec<_>>();
    if argument_edges.len() != arguments.len()
        || argument_edges
            .iter()
            .zip(arguments)
            .enumerate()
            .any(|(ordinal, (edge, expected))| {
                edge.child() != *expected
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

fn append_selected_call_operands(
    topology: &HirProjectEvaluationTopology,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owner: ExprId,
    kind: &HirExprKind,
    callee: HirRuntimeCallCalleeDisposition,
    pending: &mut VecDeque<ExprId>,
    followed: &mut Vec<HirExpressionEvaluationEdge>,
) -> Result<(), HirSelectedExpressionInventoryError> {
    let HirExprKind::Call(call) = kind else {
        return Err(
            HirSelectedExpressionInventoryError::InvalidRuntimeCallDisposition {
                expression: owner,
            },
        );
    };
    for edge in topology.expression_edges(owner).iter().filter(|edge| {
        matches!(
            edge,
            HirExpressionEvaluationEdge::Expression {
                role: HirExpressionChildRole::Argument { .. },
                ownership: HirExpressionChildOwnership::Owning,
                ..
            }
        )
    }) {
        pending.push_back(edge.child());
        followed.push(edge.clone());
    }
    if callee == HirRuntimeCallCalleeDisposition::Static {
        return Ok(());
    }
    let callee_owner = call.callee().value_expression().ok_or(
        HirSelectedExpressionInventoryError::MissingRuntimeCallReceiver { expression: owner },
    )?;
    let module = modules.get(&callee_owner.module()).copied().ok_or(
        HirSelectedExpressionInventoryError::UnknownModule {
            module: callee_owner.module(),
        },
    )?;
    let receiver = module
        .resolve_call_value_receiver(call)
        .map_err(
            |_| HirSelectedExpressionInventoryError::UnresolvedExpression {
                expression: callee_owner,
            },
        )?
        .ok_or(
            HirSelectedExpressionInventoryError::MissingRuntimeCallReceiver { expression: owner },
        )?;
    pending.push_back(receiver);
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
    view: HirExecutableProjectView<'_>,
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
