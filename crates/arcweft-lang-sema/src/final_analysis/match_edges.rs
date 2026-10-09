//! Checked Match child-edge enrichment and semantic callable joins.
//!
//! HIR owns the structural child walk.  This module is deliberately the
//! semantic half of that boundary: it projects the HIR-only role vocabulary
//! into accepted identities only after the corresponding final-analysis fact
//! has been found.  In particular, no source spelling or arena identity is
//! used as a fallback identity.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_lang_hir::{
    expr::{
        HirExprKind, HirExpressionChildRole, HirNestedExpressionPath,
        HirNestedExpressionPathSegment,
    },
    identity::{ExprId, SyntheticOwner},
    item::{HirItemKind, HirTraitMember},
    module::HirModule,
    project::{
        HirExpressionEvaluationEdge, HirProjectEvaluationTopology,
        HirSelectedCallExpressionDisposition, HirSelectedCallExpressionInventory,
        HirSelectedExpressionGraph, HirSelectedExpressionInventoryError,
        HirSelectedSelectTargetDisposition,
    },
    symbol::CallableDeclarationKey,
};

use super::{
    CheckedExpressionResolution, CheckedMethodSelection, CheckedTryOperandAuthorityViolation,
    ExprId as SemaExprId, FinalCallSealFailure, FinalCallSealLocation, FinalSemanticAnalysis,
    FinalSemanticAnalysisError, HirAnalysisProjectView, HirModuleId, SemanticFactFamily, TypeKind,
};
use crate::callable::{
    CheckedCallArgumentPassing, CheckedCallArgumentSlotSource, CheckedCallSemanticOperandSource,
    CheckedCallSite, CheckedCallableCatalog, CheckedCallableJoin, CheckedCallableJoinError,
    validate_selected_application,
};
use crate::record_field::CheckedRecordFieldSemanticId;
use crate::semantic_coordinate::{
    CheckedExpressionChildRole, CheckedExpressionEdgeAuthority, CheckedNestedPathSegmentV1,
    CheckedNestedPathV1,
};

mod model;

#[cfg(test)]
mod tests;

pub use model::{
    CheckedChildEdgeError, CheckedExpressionChildEdge, CheckedExpressionEdgeError,
    CheckedExpressionEdgeFact, CheckedNestedEvidenceRole, NestedPathEvidence,
};

fn checked_nested_path_from_hir(
    path: &HirNestedExpressionPath,
) -> Result<CheckedNestedPathV1, CheckedChildEdgeError> {
    let segments = path
        .segments()
        .iter()
        .map(|segment| match segment {
            HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal } => {
                CheckedNestedPathSegmentV1::ChoiceBodyItem { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::ChoiceIfBranch { ordinal } => {
                CheckedNestedPathSegmentV1::ChoiceIfBranch { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::ChoiceIfElse => {
                CheckedNestedPathSegmentV1::ChoiceIfElse
            }
            HirNestedExpressionPathSegment::ChoiceForBody => {
                CheckedNestedPathSegmentV1::ChoiceForBody
            }
            HirNestedExpressionPathSegment::ChoiceMatchArm { ordinal } => {
                CheckedNestedPathSegmentV1::ChoiceMatchArm { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::ChoiceOptionBody => {
                CheckedNestedPathSegmentV1::ChoiceOptionBody
            }
            HirNestedExpressionPathSegment::ChoiceOptionField { ordinal } => {
                CheckedNestedPathSegmentV1::ChoiceOptionField { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::ChoiceViewEntry { ordinal } => {
                CheckedNestedPathSegmentV1::ChoiceViewEntry { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::ChoicePlanItem { ordinal } => {
                CheckedNestedPathSegmentV1::ChoicePlanItem { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::LinePlanItem { ordinal } => {
                CheckedNestedPathSegmentV1::LinePlanItem { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::LinePlanStartGroupItem { ordinal } => {
                CheckedNestedPathSegmentV1::LinePlanStartGroupItem { ordinal: *ordinal }
            }
            HirNestedExpressionPathSegment::LinePlanTogetherGroupItem { ordinal } => {
                CheckedNestedPathSegmentV1::LinePlanTogetherGroupItem { ordinal: *ordinal }
            }
        })
        .collect::<Vec<_>>();
    CheckedNestedPathV1::try_from_segments(segments.into_boxed_slice())
        .map_err(|_| CheckedChildEdgeError::MissingNestedPath)
}

pub(super) type PreparedCallableJoins =
    BTreeMap<ExprId, Result<CheckedCallableJoin, CheckedCallableJoinError>>;

pub(crate) type SelectedHirExpressionEdge = (ExprId, HirExpressionChildRole);

fn checked_call_site_for_expression(
    expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
    expression: ExprId,
) -> Option<CheckedCallSite> {
    expressions
        .get(&expression)
        .and_then(|checked| checked.checked_call_site(expression))
}

/// Resolves the exact expression inventory selected by checked postfix facts.
/// HIR owns traversal and candidate membership; the checked fact supplies only
/// the already-accepted candidate identity.
#[derive(Debug)]
pub(super) struct CheckedSelectedExpressionGraph {
    graph: HirSelectedExpressionGraph,
    selected_call_inventories: BTreeMap<ExprId, HirSelectedCallExpressionInventory>,
    declaration_only_trait_receiver_owners: BTreeSet<SyntheticOwner>,
    dialogue_lines: arcweft_lang_hir::project::AcceptedDialogueLineInventory,
    fx_definition_declarations: BTreeSet<CallableDeclarationKey>,
    fx_body_expressions: BTreeSet<ExprId>,
}

impl CheckedSelectedExpressionGraph {
    pub(super) fn seal(
        project: HirAnalysisProjectView<'_>,
        topology: Arc<HirProjectEvaluationTopology>,
        expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
        prepared_calls: &super::analyzer::AnalyzerPreparedCallGraph,
        fx_body_obligations: &super::analyzer::PreparedFxDefinitionBodyObligations,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        if fx_body_obligations
            .owners()
            .any(|owner| expressions.contains_key(&owner))
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let mut call_inventories = BTreeMap::<ExprId, HirSelectedCallExpressionDisposition>::new();
        for site in prepared_calls.sites() {
            // HIR's selected-expression graph inventories only ordinary
            // `HirExprKind::Call` children. Attached content applications
            // still participate in the prepared callable graph, joins, and
            // transcript, but their body/callee ownership is sealed by the
            // attached-content authority rather than this HIR-call walker.
            if !matches!(site, CheckedCallSite::HirCall(_)) {
                continue;
            }
            let owner = site.expression();
            if expressions
                .get(&owner)
                .and_then(|expression| expression.checked_call_site(owner))
                != Some(site)
            {
                return Err(FinalSemanticAnalysisError::CallSeal(
                    FinalCallSealFailure::new(
                        FinalCallSealLocation::Site(site),
                        crate::callable::CallConstraintInvariant::PreparedCallSiteMismatch,
                    ),
                ));
            }
            let inventory = prepared_calls
                .project_site_payload(
                    site,
                    |prefix| {
                        prefix
                            .selected_expression_inventory()
                            .map(HirSelectedCallExpressionDisposition::Callable)
                    },
                    |unselected| Ok(unselected.source_expression_disposition()),
                )
                .ok_or_else(|| {
                    FinalSemanticAnalysisError::CallSeal(FinalCallSealFailure::new(
                        FinalCallSealLocation::Site(site),
                        crate::callable::CallConstraintInvariant::MissingOrStalePreparedNode,
                    ))
                })?
                .map_err(|failure| {
                    FinalSemanticAnalysisError::CallSeal(FinalCallSealFailure::new(
                        FinalCallSealLocation::Site(site),
                        failure,
                    ))
                })?;
            if call_inventories.insert(owner, inventory).is_some() {
                return Err(FinalSemanticAnalysisError::CallSeal(
                    FinalCallSealFailure::new(
                        FinalCallSealLocation::Site(site),
                        crate::callable::CallConstraintInvariant::PreparedCallSiteMismatch,
                    ),
                ));
            }
        }
        let mut selected = Self::seal_with_call_inventory(
            project,
            topology,
            expressions,
            fx_body_obligations,
            arcweft_lang_hir::project::HirSelectedExpressionRootPartition::Language,
            |owner| {
                fx_body_obligations
                    .contains(owner)
                    .then_some(HirSelectedCallExpressionDisposition::Structural)
                    .or_else(|| call_inventories.get(&owner).cloned())
                    .or_else(|| {
                        expressions
                            .get(&owner)
                            .filter(|expression| expression.checked_call_site(owner).is_none())
                            .map(|_| HirSelectedCallExpressionDisposition::Structural)
                    })
            },
        )?;
        selected.selected_call_inventories = call_inventories
            .into_iter()
            .filter(|(owner, _)| {
                selected.graph.contains_expression(*owner) && !fx_body_obligations.contains(*owner)
            })
            .filter_map(|(owner, disposition)| match disposition {
                HirSelectedCallExpressionDisposition::Callable(inventory)
                | HirSelectedCallExpressionDisposition::NonCallable(inventory)
                | HirSelectedCallExpressionDisposition::Unselected(inventory) => {
                    Some((owner, inventory))
                }
                HirSelectedCallExpressionDisposition::Structural => None,
            })
            .collect();
        Ok(selected)
    }

    /// Manual fact fixtures may omit prepared-call state only when their HIR
    /// contains no selected Call. Encountering a Call fails closed through the
    /// same inventory error; this helper never reconstructs children from raw
    /// syntax or final expression membership.
    #[cfg(test)]
    pub(super) fn seal_call_free_fixture(
        project: HirAnalysisProjectView<'_>,
        topology: Arc<HirProjectEvaluationTopology>,
        expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let obligations = super::analyzer::PreparedFxDefinitionBodyObligations::default();
        Self::seal_with_call_inventory(
            project,
            topology,
            expressions,
            &obligations,
            arcweft_lang_hir::project::HirSelectedExpressionRootPartition::CompleteProject,
            |owner| {
                expressions
                    .get(&owner)
                    .filter(|expression| expression.checked_call_site(owner).is_none())
                    .map(|_| HirSelectedCallExpressionDisposition::Structural)
            },
        )
    }

    fn seal_with_call_inventory(
        project: HirAnalysisProjectView<'_>,
        topology: Arc<HirProjectEvaluationTopology>,
        expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
        fx_body_obligations: &super::analyzer::PreparedFxDefinitionBodyObligations,
        root_partition: arcweft_lang_hir::project::HirSelectedExpressionRootPartition,
        selected_call: impl FnMut(ExprId) -> Option<HirSelectedCallExpressionDisposition>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let graph = project
            .selected_expression_graph_in_partition_with_select_target_disposition(
                &topology,
                root_partition,
                |owner| expressions.get(&owner)?.selected_postfix_candidate(),
                selected_call,
                |owner| {
                    expressions
                        .get(&owner)
                        .is_some_and(super::PreparedExpressionFact::is_variant_expression)
                        .then_some(HirSelectedSelectTargetDisposition::StaticVariantQualifier)
                },
            )
            .map_err(|error| {
                match error {
                HirSelectedExpressionInventoryError::MissingPostfixSelection { expression }
                    if !expressions.contains_key(&expression) =>
                {
                    FinalSemanticAnalysisError::MissingFact {
                        family: SemanticFactFamily::Expression,
                    }
                }
                HirSelectedExpressionInventoryError::MissingPostfixSelection { .. }
                | HirSelectedExpressionInventoryError::InvalidPostfixSelection { .. }
                | HirSelectedExpressionInventoryError::InvalidRuntimeCallDisposition { .. }
                | HirSelectedExpressionInventoryError::InvalidRuntimeStructuralDisposition {
                    ..
                }
                | HirSelectedExpressionInventoryError::MissingRuntimeExpressionProjection {
                    ..
                }
                | HirSelectedExpressionInventoryError::InvalidRuntimeValueRetention { .. }
                | HirSelectedExpressionInventoryError::MissingRuntimeCallReceiver { .. } => {
                    FinalSemanticAnalysisError::WrongPayloadFamily
                }
                HirSelectedExpressionInventoryError::MissingSelectedCallEdges { expression }
                    if !expressions.contains_key(&expression) =>
                {
                    FinalSemanticAnalysisError::MissingFact {
                        family: SemanticFactFamily::Expression,
                    }
                }
                HirSelectedExpressionInventoryError::MissingSelectedCallEdges { expression } => {
                    checked_call_site_for_expression(expressions, expression).map_or(
                        FinalSemanticAnalysisError::WrongPayloadFamily,
                        |site| {
                            FinalSemanticAnalysisError::CallSeal(FinalCallSealFailure::new(
                                FinalCallSealLocation::Site(site),
                                crate::callable::CallConstraintInvariant::MissingOrStalePreparedNode,
                            ))
                        },
                    )
                }
                HirSelectedExpressionInventoryError::InvalidSelectedCallCallee {
                    expression,
                    ..
                } => checked_call_site_for_expression(expressions, expression).map_or(
                    FinalSemanticAnalysisError::WrongPayloadFamily,
                    |site| {
                        FinalSemanticAnalysisError::CallSeal(FinalCallSealFailure::new(
                            FinalCallSealLocation::Site(site),
                            crate::callable::CallConstraintInvariant::PreparedCallSiteMismatch,
                        ))
                    },
                ),
                HirSelectedExpressionInventoryError::InvalidSelectedCallArguments {
                    expression,
                } => checked_call_site_for_expression(expressions, expression).map_or(
                    FinalSemanticAnalysisError::WrongPayloadFamily,
                    |site| {
                        FinalSemanticAnalysisError::CallSeal(FinalCallSealFailure::new(
                            FinalCallSealLocation::Site(site),
                            crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                        ))
                    },
                ),
                HirSelectedExpressionInventoryError::UnresolvedExpression { expression } => {
                    FinalSemanticAnalysisError::CallSeal(FinalCallSealFailure::new(
                        FinalCallSealLocation::Graph,
                        crate::callable::CallConstraintInvariant::MissingCheckedExpressionCoordinate {
                            owner: expression,
                        },
                    ))
                }
                HirSelectedExpressionInventoryError::UnknownModule { .. }
                | HirSelectedExpressionInventoryError::TopologyMismatch => {
                    FinalSemanticAnalysisError::InvalidOwner
                }
                HirSelectedExpressionInventoryError::InvalidSelectedGraph => {
                    FinalSemanticAnalysisError::WrongPayloadFamily
                }
                HirSelectedExpressionInventoryError::RecoveredOwner { .. } => {
                    FinalSemanticAnalysisError::RecoveredOwner
                }
            }
            })?;
        let fx_body_expressions = fx_body_obligations.owners().collect::<BTreeSet<_>>();
        let fx_definition_declarations = fx_body_obligations
            .declarations()
            .cloned()
            .collect::<BTreeSet<_>>();
        let graph_owners = graph.expression_owners().collect::<BTreeSet<_>>();
        if !fx_body_expressions.is_subset(&graph_owners)
            || graph_owners.iter().any(|owner| {
                graph.expression_edges(*owner).iter().any(|edge| {
                    fx_body_expressions.contains(owner)
                        != fx_body_expressions.contains(&edge.child())
                })
            })
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let dialogue_lines = project
            .seal_selected_dialogue_lines(&graph)
            .map_err(FinalSemanticAnalysisError::DialogueLineSeal)?;
        let declaration_only_trait_receiver_owners = project
            .modules()
            .flat_map(|(_, module)| module.items())
            .flat_map(|(_, item)| {
                let HirItemKind::Trait(trait_item) = item.kind() else {
                    return Vec::new();
                };
                trait_item
                    .members()
                    .iter()
                    .filter_map(|member| {
                        let HirTraitMember::Function(function) = member else {
                            return None;
                        };
                        function.body().is_none().then_some(function)
                    })
                    .flat_map(|function| {
                        function
                            .parameter_groups()
                            .iter()
                            .flat_map(|group| group.parameters())
                            .filter_map(|parameter| parameter.receiver())
                            .flat_map(|receiver| {
                                std::iter::once(SyntheticOwner::Pattern(receiver.pattern())).chain(
                                    receiver.locals().iter().copied().map(SyntheticOwner::Local),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        Ok(Self {
            graph,
            selected_call_inventories: BTreeMap::new(),
            declaration_only_trait_receiver_owners,
            dialogue_lines,
            fx_definition_declarations,
            fx_body_expressions,
        })
    }

    pub(super) fn topology(&self) -> &Arc<HirProjectEvaluationTopology> {
        self.graph.topology()
    }

    pub(super) fn into_selected_call_inventories(
        self,
    ) -> BTreeMap<ExprId, HirSelectedCallExpressionInventory> {
        self.selected_call_inventories
    }

    pub(super) fn contains_owner(&self, owner: arcweft_lang_hir::identity::SyntheticOwner) -> bool {
        !self.declaration_only_trait_receiver_owners.contains(&owner)
            && self.graph.contains_owner(owner)
    }

    pub(super) const fn dialogue_lines(
        &self,
    ) -> &arcweft_lang_hir::project::AcceptedDialogueLineInventory {
        &self.dialogue_lines
    }

    pub(super) fn owners(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.graph
            .expression_owners()
            .filter(|owner| !self.fx_body_expressions.contains(owner))
    }

    pub(super) fn expression_edges(&self, owner: ExprId) -> &[HirExpressionEvaluationEdge] {
        if self.fx_body_expressions.contains(&owner) {
            &[]
        } else {
            self.graph.expression_edges(owner)
        }
    }

    pub(super) fn owns_fx_definition(&self, declaration: &CallableDeclarationKey) -> bool {
        self.fx_definition_declarations.contains(declaration)
    }
}

/// The checked structural children of one expression owner. Ordinary
/// owning expression edges carry their sema-accepted roles; expression roots
/// reached through an executable body remain the original selected HIR
/// evaluation edges, including their typed body/statement roles.
#[derive(Debug)]
struct CheckedStructuralOwnerEdges {
    ordered: Box<[CheckedStructuralChildEdge]>,
    record: Option<CheckedRecordFieldPlan>,
}

#[derive(Debug)]
enum CheckedRecordFieldPlan {
    Semantic(Box<[super::CheckedRecordFieldSlot]>),
    Runtime(Box<[super::CheckedExpressionRecordField]>),
}

impl CheckedRecordFieldPlan {
    fn slot(&self, ordinal: u32) -> Option<&super::CheckedRecordFieldSlot> {
        let ordinal = usize::try_from(ordinal).ok()?;
        match self {
            Self::Semantic(slots) => slots.get(ordinal),
            Self::Runtime(fields) => fields
                .get(ordinal)
                .map(super::CheckedExpressionRecordField::slot),
        }
    }
}

#[derive(Debug)]
enum CheckedStructuralChildEdge {
    Expression {
        child: ExprId,
        role: CheckedExpressionChildRole,
    },
    Evaluation(HirExpressionEvaluationEdge),
}

impl CheckedStructuralChildEdge {
    const fn child(&self) -> ExprId {
        match self {
            Self::Expression { child, .. } => *child,
            Self::Evaluation(edge) => edge.child(),
        }
    }
}

/// Move-only structural edge authority sealed before call application
/// finalization.  C1 semantic coordinates borrow this exact draft; final edge
/// publication later consumes it together with callable joins.  No second HIR
/// walk or parallel checked-role table is permitted across those phases.
#[derive(Debug)]
pub(super) struct CheckedStructuralEdgeDraft {
    facts: BTreeMap<ExprId, Result<CheckedStructuralOwnerEdges, CheckedChildEdgeError>>,
    call_owners: BTreeSet<ExprId>,
}

impl CheckedExpressionEdgeAuthority for CheckedStructuralEdgeDraft {
    fn checked_record_input_field(
        &self,
        owner: ExprId,
        source_ordinal: u32,
        local: arcweft_lang_hir::identity::LocalId,
    ) -> Option<crate::record_field::CheckedRecordFieldSemanticId> {
        let slot = self.record_slot(owner, source_ordinal).ok()?;
        (slot.source() == super::prepared::PreparedRecordValueSource::Local(local))
            .then(|| slot.semantic_id())
    }
    fn checked_expression_child_role(
        &self,
        parent: ExprId,
        child: ExprId,
    ) -> Option<CheckedExpressionChildRole> {
        self.facts
            .get(&parent)?
            .as_ref()
            .ok()?
            .ordered
            .iter()
            .find_map(|edge| match edge {
                CheckedStructuralChildEdge::Expression {
                    child: candidate,
                    role,
                } if *candidate == child => Some(role.clone()),
                CheckedStructuralChildEdge::Expression { .. }
                | CheckedStructuralChildEdge::Evaluation(_) => None,
            })
    }
}

impl CheckedStructuralEdgeDraft {
    /// Membership is inherited from the canonical selected HIR graph used to
    /// seal this draft. A prepared query fact alone cannot grant membership.
    pub(super) fn contains_expression(&self, owner: ExprId) -> bool {
        self.facts.contains_key(&owner)
    }

    fn checked_expression_children(
        &self,
        owner: ExprId,
    ) -> Result<
        impl Iterator<Item = (ExprId, &CheckedExpressionChildRole)> + '_,
        CheckedChildEdgeError,
    > {
        let edges = self
            .facts
            .get(&owner)
            .ok_or(CheckedChildEdgeError::MissingExpression)?
            .as_ref()
            .map_err(Clone::clone)?;
        Ok(edges.ordered.iter().filter_map(|edge| match edge {
            CheckedStructuralChildEdge::Expression { child, role } => Some((*child, role)),
            CheckedStructuralChildEdge::Evaluation(_) => None,
        }))
    }

    /// Borrows every selected expression child reached by the expression's
    /// ordinary owning edges and executable body/statement edges, in the
    /// selected HIR evaluation order. This is the complete checked boundary
    /// used when finding free locals in executable bodies.
    pub(super) fn free_capture_children(
        &self,
        owner: ExprId,
    ) -> Result<impl DoubleEndedIterator<Item = ExprId> + '_, CheckedChildEdgeError> {
        let edges = self
            .facts
            .get(&owner)
            .ok_or(CheckedChildEdgeError::MissingExpression)?
            .as_ref()
            .map_err(Clone::clone)?;
        Ok(edges.ordered.iter().map(CheckedStructuralChildEdge::child))
    }

    /// Returns the unique checked `Operand` child edge for one Try owner.
    ///
    /// This is intentionally issued from the already-enriched structural edge
    /// draft. A missing or duplicate edge is an authority failure; no raw HIR
    /// walk may choose a first matching child.
    pub(super) fn exact_operand_child(
        &self,
        owner: ExprId,
    ) -> Result<ExprId, CheckedTryOperandAuthorityViolation> {
        let edges = self
            .checked_expression_children(owner)
            .map_err(|_| CheckedTryOperandAuthorityViolation::MissingOperand { owner })?;
        let mut operands = edges.filter_map(|(child, role)| {
            matches!(role, CheckedExpressionChildRole::Operand).then_some(child)
        });
        let Some(child) = operands.next() else {
            return Err(CheckedTryOperandAuthorityViolation::MissingOperand { owner });
        };
        if operands.next().is_some() {
            return Err(CheckedTryOperandAuthorityViolation::DuplicateOperand { owner });
        }
        Ok(child)
    }

    pub(super) fn attach_record_fields(
        &mut self,
        mut fields: BTreeMap<ExprId, Box<[super::CheckedExpressionRecordField]>>,
    ) -> Result<(), CheckedChildEdgeError> {
        let owners = self
            .facts
            .iter()
            .filter_map(|(owner, row)| {
                row.as_ref()
                    .ok()
                    .and_then(|row| row.record.as_ref())
                    .map(|_| *owner)
            })
            .collect::<Vec<_>>();
        if !fields.keys().copied().eq(owners.iter().copied()) {
            return Err(CheckedChildEdgeError::UnexpectedCheckedRecordField);
        }
        for owner in &owners {
            let row = self
                .facts
                .get(owner)
                .and_then(|row| row.as_ref().ok())
                .ok_or(CheckedChildEdgeError::MissingExpression)?;
            let Some(CheckedRecordFieldPlan::Semantic(slots)) = &row.record else {
                return Err(CheckedChildEdgeError::UnexpectedCheckedRecordField);
            };
            let actual = fields
                .get(owner)
                .ok_or(CheckedChildEdgeError::MissingCheckedRecordField)?;
            if slots.len() != actual.len()
                || slots
                    .iter()
                    .zip(actual)
                    .any(|(slot, field)| slot != field.slot())
            {
                return Err(CheckedChildEdgeError::CheckedRecordFieldOrderMismatch);
            }
        }
        for owner in owners {
            let actual = fields
                .remove(&owner)
                .expect("validated complete record batch");
            let row = self
                .facts
                .get_mut(&owner)
                .expect("validated record owner")
                .as_mut()
                .expect("validated record fact");
            row.record = Some(CheckedRecordFieldPlan::Runtime(actual));
        }
        Ok(())
    }

    pub(super) fn record_fields(
        &self,
        owner: ExprId,
    ) -> Result<&[super::CheckedExpressionRecordField], CheckedChildEdgeError> {
        let row = self
            .facts
            .get(&owner)
            .ok_or(CheckedChildEdgeError::MissingExpression)?
            .as_ref()
            .map_err(Clone::clone)?;
        match &row.record {
            None => Ok(&[]),
            Some(CheckedRecordFieldPlan::Runtime(fields)) => Ok(fields),
            Some(CheckedRecordFieldPlan::Semantic(_)) => {
                Err(CheckedChildEdgeError::MissingCheckedRecordField)
            }
        }
    }

    pub(super) fn record_slot(
        &self,
        owner: ExprId,
        ordinal: u32,
    ) -> Result<&super::CheckedRecordFieldSlot, CheckedChildEdgeError> {
        self.facts
            .get(&owner)
            .ok_or(CheckedChildEdgeError::MissingExpression)?
            .as_ref()
            .map_err(Clone::clone)?
            .record
            .as_ref()
            .and_then(|record| record.slot(ordinal))
            .ok_or(CheckedChildEdgeError::MissingCheckedRecordField)
    }

    fn call_callee(&self, owner: ExprId) -> Result<Option<ExprId>, CheckedCallableJoinError> {
        let edges = self
            .checked_expression_children(owner)
            .map_err(|_| CheckedCallableJoinError::NotSelected)?;
        let mut callees = edges.filter_map(|(child, role)| {
            matches!(
                role,
                CheckedExpressionChildRole::Callee | CheckedExpressionChildRole::ContentCallee
            )
            .then_some(child)
        });
        let callee = callees.next();
        if callees.next().is_some() {
            return Err(CheckedCallableJoinError::UnexpectedReceiverKey);
        }
        Ok(callee)
    }

    pub(super) fn seal(
        selected: &CheckedSelectedExpressionGraph,
        modules: &BTreeMap<HirModuleId, &HirModule>,
        expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
    ) -> Self {
        let mut facts = BTreeMap::new();
        let mut call_owners = BTreeSet::new();
        for owner in selected.owners() {
            let selected_edges = selected.expression_edges(owner);
            let edges = selected_edges
                .iter()
                .filter_map(|edge| match edge {
                    HirExpressionEvaluationEdge::Expression {
                        role,
                        ownership: arcweft_lang_hir::expr::HirExpressionChildOwnership::Owning,
                        child,
                    } => Some((*child, role.clone())),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let Some(owner_expression) = modules
                .get(&owner.module())
                .and_then(|module| module.resolve_expr(owner).ok())
            else {
                facts.insert(owner, Err(CheckedChildEdgeError::MissingExpression));
                continue;
            };
            let Some(checked_owner) = expressions.get(&owner) else {
                facts.insert(owner, Err(CheckedChildEdgeError::MissingExpression));
                continue;
            };
            if checked_owner.checked_call_site(owner).is_some() {
                call_owners.insert(owner);
            }
            let mut record = None;
            match (
                matches!(
                    owner_expression.kind(),
                    HirExprKind::Record(_) | HirExprKind::RecordLiteral(_)
                ),
                matches!(
                    checked_owner,
                    super::PreparedExpressionFact::ProjectRecord(_)
                ),
            ) {
                (true, true) => {
                    let super::PreparedExpressionFact::ProjectRecord(prepared) = checked_owner
                    else {
                        unreachable!()
                    };
                    let slots = prepared
                        .fields()
                        .iter()
                        .map(|field| {
                            super::CheckedRecordFieldSlot::issue(
                                prepared.nominal().identity(),
                                field,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>();
                    match slots {
                        Ok(slots)
                            if slots.iter().enumerate().all(|(ordinal, slot)| {
                                u32::try_from(ordinal).ok() == Some(slot.source_ordinal())
                            }) && slots
                                .iter()
                                .map(|slot| slot.declaration_ordinal())
                                .collect::<BTreeSet<_>>()
                                .len()
                                == slots.len() =>
                        {
                            record =
                                Some(CheckedRecordFieldPlan::Semantic(slots.into_boxed_slice()))
                        }
                        Err(error) => {
                            facts.insert(owner, Err(CheckedChildEdgeError::GenericScope(error)));
                            continue;
                        }
                        Ok(_) => {
                            facts.insert(
                                owner,
                                Err(CheckedChildEdgeError::CheckedRecordFieldOrderMismatch),
                            );
                            continue;
                        }
                    }
                }
                (false, false) => {}
                (true, false) => {
                    facts.insert(owner, Err(CheckedChildEdgeError::MissingCheckedRecordField));
                    continue;
                }
                (false, true) => {
                    facts.insert(
                        owner,
                        Err(CheckedChildEdgeError::UnexpectedCheckedRecordField),
                    );
                    continue;
                }
            }
            if let Some(complete) = checked_owner.complete() {
                if let Err(error) =
                    validate_match_owner(owner_expression.kind(), complete, expressions)
                {
                    facts.insert(owner, Err(error));
                    continue;
                }
                if let Err(error) = validate_nested_path_evidence(
                    owner_expression.kind(),
                    complete,
                    &edges,
                    expressions,
                ) {
                    facts.insert(owner, Err(error));
                    continue;
                }
            } else if let super::PreparedExpressionFact::DialogueApplication(prepared) =
                checked_owner
            {
                if let Err(error) =
                    validate_prepared_nested_path_evidence(prepared, &edges, expressions)
                {
                    facts.insert(owner, Err(error));
                    continue;
                }
            }
            let mut enriched = Vec::with_capacity(edges.len());
            let mut first_error = None;
            for (child, role) in &edges {
                let child = *child;
                let Some(checked_child) = expressions.get(&child) else {
                    first_error = Some(CheckedChildEdgeError::MissingExpression);
                    break;
                };
                if let Some(complete) = checked_owner.complete() {
                    if let Err(error) = validate_match_edge(
                        owner_expression.kind(),
                        complete,
                        child,
                        role,
                        expressions,
                    ) {
                        first_error = Some(error);
                        break;
                    }
                }
                let accepted_field = match role {
                    HirExpressionChildRole::RecordField { source_ordinal } => {
                        match record
                            .as_ref()
                            .and_then(|record| record.slot(*source_ordinal))
                        {
                            Some(slot)
                                if slot.source()
                                    == super::PreparedRecordValueSource::Expression(child) =>
                            {
                                Some(slot.semantic_id())
                            }
                            _ => {
                                first_error =
                                    Some(CheckedChildEdgeError::MissingCheckedRecordField);
                                break;
                            }
                        }
                    }
                    _ => None,
                };
                if matches!(role, HirExpressionChildRole::Guard { .. })
                    && !checked_child
                        .value_type()
                        .is_some_and(|ty| TypeKind::Bool.accepts(ty))
                {
                    first_error = Some(CheckedChildEdgeError::MatchGuardTypeMismatch);
                    break;
                }
                if matches!(role, HirExpressionChildRole::ChoiceMatchGuard { .. })
                    && !checked_child
                        .value_type()
                        .is_some_and(|ty| TypeKind::Bool.accepts(ty))
                {
                    first_error = Some(CheckedChildEdgeError::MatchGuardTypeMismatch);
                    break;
                }
                match checked_role_from_hir(role, accepted_field) {
                    Ok(role) => enriched.push((child, role)),
                    Err(error) => {
                        first_error = Some(error);
                        break;
                    }
                }
            }
            let mut ordered = Vec::with_capacity(selected_edges.len());
            let mut enriched = enriched.into_iter();
            if first_error.is_none() {
                for edge in selected_edges {
                    match edge {
                        HirExpressionEvaluationEdge::Expression {
                            ownership, child, ..
                        } if ownership
                            == &arcweft_lang_hir::expr::HirExpressionChildOwnership::Owning =>
                        {
                            let Some((checked_child, role)) = enriched.next() else {
                                first_error = Some(CheckedChildEdgeError::ChildCountMismatch);
                                break;
                            };
                            if checked_child != *child {
                                first_error = Some(CheckedChildEdgeError::ChildIdentityMismatch);
                                break;
                            }
                            ordered.push(CheckedStructuralChildEdge::Expression {
                                child: *child,
                                role,
                            });
                        }
                        HirExpressionEvaluationEdge::Expression { .. } => {}
                        evaluation => {
                            if !expressions.contains_key(&evaluation.child()) {
                                first_error = Some(CheckedChildEdgeError::MissingExpression);
                                break;
                            }
                            ordered
                                .push(CheckedStructuralChildEdge::Evaluation(evaluation.clone()));
                        }
                    }
                }
            }
            if first_error.is_none() && enriched.next().is_some() {
                first_error = Some(CheckedChildEdgeError::ChildCountMismatch);
            }
            facts.insert(
                owner,
                first_error.map_or_else(
                    || {
                        Ok(CheckedStructuralOwnerEdges {
                            ordered: ordered.into_boxed_slice(),
                            record,
                        })
                    },
                    Err,
                ),
            );
        }
        Self { facts, call_owners }
    }

    pub(super) fn into_final_facts(
        self,
        modules: &BTreeMap<HirModuleId, &HirModule>,
        calls: &BTreeMap<ExprId, super::CallTargetFacts>,
        mut callable_joins: PreparedCallableJoins,
    ) -> (
        BTreeMap<ExprId, Result<CheckedExpressionEdgeFact, CheckedExpressionEdgeError>>,
        PreparedCallableJoins,
    ) {
        let mut final_facts = BTreeMap::new();
        for (owner, structural) in self.facts {
            let callable = if self.call_owners.contains(&owner) {
                match callable_joins
                    .remove(&owner)
                    .unwrap_or(Err(CheckedCallableJoinError::NotSelected))
                {
                    Ok(join) => Some(join),
                    Err(error) => {
                        final_facts.insert(owner, Err(CheckedExpressionEdgeError::Callable(error)));
                        continue;
                    }
                }
            } else {
                None
            };
            let structural = match structural {
                Ok(edges) => edges,
                Err(error) => {
                    final_facts.insert(owner, Err(CheckedExpressionEdgeError::Child(error)));
                    continue;
                }
            };
            let record_fields = match structural.record {
                None => Vec::new().into_boxed_slice(),
                Some(CheckedRecordFieldPlan::Runtime(fields)) => fields,
                Some(CheckedRecordFieldPlan::Semantic(_)) => {
                    final_facts.insert(
                        owner,
                        Err(CheckedExpressionEdgeError::Child(
                            CheckedChildEdgeError::MissingCheckedRecordField,
                        )),
                    );
                    continue;
                }
            };
            let mut edges = structural
                .ordered
                .into_vec()
                .into_iter()
                .filter_map(|edge| match edge {
                    CheckedStructuralChildEdge::Expression { child, role } => Some((child, role)),
                    CheckedStructuralChildEdge::Evaluation(_) => None,
                })
                .collect::<Vec<_>>();
            if self.call_owners.contains(&owner) {
                let Some(call) = calls.get(&owner) else {
                    final_facts.insert(
                        owner,
                        Err(CheckedExpressionEdgeError::Child(
                            CheckedChildEdgeError::MissingCallFacts,
                        )),
                    );
                    continue;
                };
                if let Err(error) = refine_content_nominal_discriminator_edge(call, &mut edges) {
                    final_facts.insert(owner, Err(CheckedExpressionEdgeError::Child(error)));
                    continue;
                }
                if let Some(error) = edges.iter().find_map(|(child, role)| {
                    validate_checked_call_edge(
                        modules.get(&child.module()).copied(),
                        call,
                        *child,
                        role,
                    )
                    .err()
                }) {
                    final_facts.insert(owner, Err(CheckedExpressionEdgeError::Child(error)));
                    continue;
                }
            }
            match CheckedExpressionEdgeFact::seal(edges.into_boxed_slice(), record_fields, callable)
            {
                Ok(fact) => {
                    final_facts.insert(owner, Ok(fact));
                }
                Err(error) => {
                    final_facts.insert(owner, Err(CheckedExpressionEdgeError::Child(error)));
                }
            }
        }
        (final_facts, callable_joins)
    }
}

/// Composes the accepted callable-owner join for every call exactly once.
///
/// The returned transaction is staging input: Method rows borrow its accepted
/// joins during enrichment, then edge publication consumes the same values.
/// Neither phase resolves another method key or rejoins the callable catalog.
pub(super) fn prepare_checked_callable_joins(
    calls: &BTreeMap<ExprId, super::CallTargetFacts>,
    checked_callables: &CheckedCallableCatalog,
) -> PreparedCallableJoins {
    calls
        .iter()
        .filter_map(|(owner, facts)| {
            let joined = facts
                .selected_application()
                .ok_or(CheckedCallableJoinError::NotSelected)
                .and_then(|application| {
                    validate_selected_application(application, checked_callables)
                });
            Some((*owner, joined))
        })
        .collect()
}

pub(super) fn validate_callable_join_inventory(
    calls: &BTreeMap<ExprId, super::CallTargetFacts>,
    joins: &PreparedCallableJoins,
) -> Result<(), CheckedCallableJoinError> {
    let call_owners = calls.iter().collect::<Vec<_>>();
    if call_owners.len() != joins.len()
        || !call_owners
            .iter()
            .map(|(owner, _)| **owner)
            .eq(joins.keys().copied())
    {
        return Err(CheckedCallableJoinError::NotSelected);
    }
    for (owner, facts) in call_owners {
        let joined = joins
            .get(owner)
            .ok_or(CheckedCallableJoinError::NotSelected)?;
        if facts.selected_application().is_some() {
            if let Err(error) = joined {
                return Err(error.clone());
            }
        } else if joined.is_ok() {
            return Err(CheckedCallableJoinError::UnexpectedReceiverKey);
        }
    }
    Ok(())
}

pub(super) fn prepare_checked_method_selections(
    structural_edges: &CheckedStructuralEdgeDraft,
    expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
    joins: &PreparedCallableJoins,
    coordinates: &crate::semantic_coordinate::SemanticCoordinateIndex<'_, '_>,
) -> Result<BTreeMap<ExprId, CheckedMethodSelection>, FinalSemanticAnalysisError> {
    let mut methods = BTreeMap::new();
    for (call_owner, joined) in joins {
        let Some(value) = structural_edges.call_callee(*call_owner)? else {
            continue;
        };
        let Some(checked) = expressions.get(&value) else {
            return Err(CheckedCallableJoinError::MissingCheckedRecord.into());
        };
        if !matches!(checked, super::PreparedExpressionFact::Method(_)) {
            continue;
        }
        let join = match joined {
            Ok(join) => join,
            Err(CheckedCallableJoinError::NotSelected) => {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed {
                    owner: *call_owner,
                });
            }
            Err(error) => return Err(error.clone().into()),
        };
        let selection = CheckedMethodSelection::try_from_join(join, coordinates)?;
        if methods.insert(value, selection).is_some() {
            return Err(CheckedCallableJoinError::MethodLookupAmbiguous.into());
        }
    }

    let prepared = expressions
        .iter()
        .filter_map(|(owner, checked)| {
            (structural_edges.contains_expression(*owner)
                && matches!(checked, super::PreparedExpressionFact::Method(_)))
            .then_some(*owner)
        })
        .collect::<Vec<_>>();
    if prepared.len() != methods.len() || prepared.iter().any(|owner| !methods.contains_key(owner))
    {
        return Err(CheckedCallableJoinError::NotSelected.into());
    }
    Ok(methods)
}

fn validate_match_owner(
    kind: &HirExprKind,
    checked: &super::CheckedExpression,
    expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
) -> Result<(), CheckedChildEdgeError> {
    let HirExprKind::Match(authored) = kind else {
        return Ok(());
    };
    let fact = checked
        .match_fact()
        .ok_or(CheckedChildEdgeError::MatchFactMissing)?;
    if fact.scrutinee() != authored.scrutinee() {
        return Err(CheckedChildEdgeError::MatchScrutineeMismatch);
    }
    if fact.arms().len() != authored.arms().len() {
        return Err(CheckedChildEdgeError::MatchGuardArmMismatch);
    }
    if expressions.get(&fact.scrutinee()).is_none() {
        return Err(CheckedChildEdgeError::MatchScrutineeMismatch);
    }
    for (authored, accepted) in authored.arms().iter().zip(fact.arms()) {
        match (authored.guard(), accepted.guard()) {
            (None, None) => {}
            (Some(_), None) => return Err(CheckedChildEdgeError::MatchGuardMissing),
            (None, Some(_)) => return Err(CheckedChildEdgeError::MatchGuardArmMismatch),
            (Some(authored), Some(accepted)) if authored != accepted => {
                return Err(CheckedChildEdgeError::MatchGuardChildMismatch);
            }
            (Some(guard), Some(_)) => {
                let Some(checked_guard) = expressions.get(&guard) else {
                    return Err(CheckedChildEdgeError::MatchGuardChildMismatch);
                };
                if !checked_guard
                    .value_type()
                    .is_some_and(|ty| TypeKind::Bool.accepts(ty))
                {
                    return Err(CheckedChildEdgeError::MatchGuardTypeMismatch);
                }
            }
        }
        if authored.value() != accepted.value() || expressions.get(&accepted.value()).is_none() {
            return Err(CheckedChildEdgeError::MatchValueChildMismatch);
        }
    }
    Ok(())
}

fn validate_match_edge(
    kind: &HirExprKind,
    checked: &super::CheckedExpression,
    child: ExprId,
    role: &HirExpressionChildRole,
    expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
) -> Result<(), CheckedChildEdgeError> {
    let HirExprKind::Match(authored) = kind else {
        return Ok(());
    };
    let fact = checked
        .match_fact()
        .ok_or(CheckedChildEdgeError::MatchFactMissing)?;
    match role {
        HirExpressionChildRole::Scrutinee => {
            if fact.scrutinee() != authored.scrutinee() || child != fact.scrutinee() {
                return Err(CheckedChildEdgeError::MatchScrutineeMismatch);
            }
        }
        HirExpressionChildRole::Guard { arm } => {
            let index =
                usize::try_from(*arm).map_err(|_| CheckedChildEdgeError::MatchGuardArmMismatch)?;
            let authored_arm = authored
                .arms()
                .get(index)
                .ok_or(CheckedChildEdgeError::MatchGuardArmMismatch)?;
            let accepted_arm = fact
                .arms()
                .get(index)
                .ok_or(CheckedChildEdgeError::MatchGuardArmMismatch)?;
            let Some(authored_guard) = authored_arm.guard() else {
                return Err(CheckedChildEdgeError::MatchGuardMissing);
            };
            if accepted_arm.guard() != Some(authored_guard) {
                return Err(CheckedChildEdgeError::MatchGuardChildMismatch);
            }
            if child != authored_guard {
                return Err(CheckedChildEdgeError::MatchGuardChildMismatch);
            }
            let Some(checked_guard) = expressions.get(&child) else {
                return Err(CheckedChildEdgeError::MatchGuardChildMismatch);
            };
            if !checked_guard
                .value_type()
                .is_some_and(|ty| TypeKind::Bool.accepts(ty))
            {
                return Err(CheckedChildEdgeError::MatchGuardTypeMismatch);
            }
        }
        HirExpressionChildRole::ArmValue { arm } => {
            let index =
                usize::try_from(*arm).map_err(|_| CheckedChildEdgeError::MatchValueArmMismatch)?;
            let authored_arm = authored
                .arms()
                .get(index)
                .ok_or(CheckedChildEdgeError::MatchValueArmMismatch)?;
            let accepted_arm = fact
                .arms()
                .get(index)
                .ok_or(CheckedChildEdgeError::MatchValueArmMismatch)?;
            if accepted_arm.value() != authored_arm.value() || child != authored_arm.value() {
                return Err(CheckedChildEdgeError::MatchValueChildMismatch);
            }
        }
        _ => {}
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NestedPathFamily {
    Choice,
}

impl CheckedNestedPathSegmentV1 {
    fn family(&self) -> Option<NestedPathFamily> {
        match self {
            Self::ChoiceBodyItem { .. }
            | Self::ChoiceIfBranch { .. }
            | Self::ChoiceIfElse
            | Self::ChoiceForBody
            | Self::ChoiceMatchArm { .. }
            | Self::ChoiceOptionBody
            | Self::ChoiceOptionField { .. }
            | Self::ChoiceViewEntry { .. }
            | Self::ChoicePlanItem { .. } => Some(NestedPathFamily::Choice),
            Self::LinePlanItem { .. }
            | Self::LinePlanStartGroupItem { .. }
            | Self::LinePlanTogetherGroupItem { .. } => None,
        }
    }
}

fn nested_path_role(
    role: &HirExpressionChildRole,
) -> Option<(&HirNestedExpressionPath, NestedPathFamily)> {
    Some(match role {
        HirExpressionChildRole::ChoiceIfCondition { path, .. }
        | HirExpressionChildRole::ChoiceForSource { path }
        | HirExpressionChildRole::ChoiceMatchScrutinee { path }
        | HirExpressionChildRole::ChoiceMatchGuard { path, .. }
        | HirExpressionChildRole::ChoiceOptionId { path }
        | HirExpressionChildRole::ChoiceOptionForSource { path }
        | HirExpressionChildRole::ChoiceCompactLabel { path }
        | HirExpressionChildRole::ChoiceCompactCondition { path }
        | HirExpressionChildRole::ChoiceCompactOut { path }
        | HirExpressionChildRole::ChoiceOptionLabel { path, .. }
        | HirExpressionChildRole::ChoiceOptionFieldId { path, .. }
        | HirExpressionChildRole::ChoiceOptionValue { path, .. }
        | HirExpressionChildRole::ChoiceOptionVisible { path, .. }
        | HirExpressionChildRole::ChoiceOptionEnabled { path, .. }
        | HirExpressionChildRole::ChoiceOptionOrder { path, .. }
        | HirExpressionChildRole::ChoiceOptionHotkey { path, .. }
        | HirExpressionChildRole::ChoiceOptionViewKey { path, .. }
        | HirExpressionChildRole::ChoiceOptionViewValue { path, .. } => {
            (path, NestedPathFamily::Choice)
        }
        _ => return None,
    })
}

pub(crate) fn build_nested_path_evidence(
    kind: &HirExprKind,
    checked: &super::CheckedExpression,
    edges: &[SelectedHirExpressionEdge],
    expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
) -> Option<Result<NestedPathEvidence, CheckedChildEdgeError>> {
    let owner_family = match (kind, checked.resolution()) {
        (HirExprKind::Choice(_), CheckedExpressionResolution::Choice(_)) => {
            NestedPathFamily::Choice
        }
        (
            HirExprKind::AttachedContentApplication(application),
            CheckedExpressionResolution::DialogueApplication { .. },
        ) => {
            let arcweft_lang_hir::dialogue_application::HirAttachedContentApplicationFamily::DialogueLine {
                target: _,
                plan: _,
                coordinates: _,
            } = application.family()
            else {
                return None;
            };
            return None;
        }
        _ => return None,
    };
    build_nested_path_evidence_for_family(owner_family, edges, expressions)
}

fn build_nested_path_evidence_for_family(
    owner_family: NestedPathFamily,
    edges: &[SelectedHirExpressionEdge],
    expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
) -> Option<Result<NestedPathEvidence, CheckedChildEdgeError>> {
    let mut evidence =
        BTreeMap::<CheckedNestedPathV1, Vec<(CheckedNestedEvidenceRole, ExprId)>>::new();
    for (child, role) in edges {
        let Some((hir_path, family)) = nested_path_role(role) else {
            continue;
        };
        if owner_family != family {
            return Some(Err(CheckedChildEdgeError::StaleNestedPath));
        }
        let path = match checked_nested_path_from_hir(hir_path) {
            Ok(path) => path,
            Err(error) => return Some(Err(error)),
        };
        let path_family = path
            .segments()
            .first()
            .and_then(CheckedNestedPathSegmentV1::family)
            .ok_or(CheckedChildEdgeError::MissingNestedPath);
        let Ok(path_family) = path_family else {
            return Some(Err(CheckedChildEdgeError::MissingNestedPath));
        };
        if path_family != family
            || path
                .segments()
                .iter()
                .any(|segment| segment.family() != Some(path_family))
        {
            return Some(Err(CheckedChildEdgeError::StaleNestedPath));
        }
        if !expressions.contains_key(child) {
            return Some(Err(CheckedChildEdgeError::MissingExpression));
        }
        let checked_role = match checked_role_from_hir(role, None) {
            Ok(role) => role,
            Err(error) => return Some(Err(error)),
        };
        let Some(role) = CheckedNestedEvidenceRole::from_checked_role(&checked_role) else {
            return Some(Err(CheckedChildEdgeError::StaleNestedPath));
        };
        evidence.entry(path).or_default().push((role, *child));
    }
    Some(Ok(evidence
        .into_iter()
        .map(|(path, entries)| (path, entries.into_boxed_slice()))
        .collect()))
}

fn validate_nested_path_evidence(
    kind: &HirExprKind,
    checked: &super::CheckedExpression,
    edges: &[SelectedHirExpressionEdge],
    expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
) -> Result<(), CheckedChildEdgeError> {
    let expected = build_nested_path_evidence(kind, checked, edges, expressions);
    let Some(stored) = checked.nested_path_evidence() else {
        return if expected.is_some() {
            Err(CheckedChildEdgeError::MissingNestedPath)
        } else {
            Ok(())
        };
    };
    let stored = stored.as_ref().map_err(Clone::clone)?;
    let Some(expected) = expected else {
        return Err(CheckedChildEdgeError::StaleNestedPath);
    };
    let expected = expected?;
    if stored == &expected {
        Ok(())
    } else if stored.is_empty() && !expected.is_empty() {
        Err(CheckedChildEdgeError::MissingNestedPath)
    } else {
        Err(CheckedChildEdgeError::StaleNestedPath)
    }
}

fn validate_prepared_nested_path_evidence(
    prepared: &super::PreparedDialogueApplication,
    _edges: &[SelectedHirExpressionEdge],
    _expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
) -> Result<(), CheckedChildEdgeError> {
    if prepared.nested_path_evidence().is_some() {
        Err(CheckedChildEdgeError::StaleNestedPath)
    } else {
        Ok(())
    }
}

impl FinalSemanticAnalysis {
    /// Returns the one atomic checked edge/callable fact for an owner.
    pub fn checked_expression_edge_fact(
        &self,
        owner: ExprId,
    ) -> Result<&CheckedExpressionEdgeFact, CheckedExpressionEdgeError> {
        self.edge_facts
            .get(&owner)
            .ok_or(CheckedExpressionEdgeError::Child(
                CheckedChildEdgeError::MissingExpression,
            ))?
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Returns one immutable, publication-time checked child-edge vector.
    pub(crate) fn checked_child_edges(
        &self,
        owner: SemaExprId,
    ) -> Result<&[CheckedExpressionChildEdge], CheckedExpressionEdgeError> {
        self.checked_expression_edge_fact(owner)
            .map(CheckedExpressionEdgeFact::edges)
    }

    /// Returns the accepted callable join published with this report.
    pub fn checked_callable_join(
        &self,
        owner: ExprId,
    ) -> Result<&CheckedCallableJoin, CheckedExpressionEdgeError> {
        self.checked_expression_edge_fact(owner)?.callable().ok_or(
            CheckedExpressionEdgeError::Callable(CheckedCallableJoinError::NotSelected),
        )
    }
}

impl CheckedExpressionEdgeAuthority for FinalSemanticAnalysis {
    fn checked_record_input_field(
        &self,
        owner: ExprId,
        source_ordinal: u32,
        local: arcweft_lang_hir::identity::LocalId,
    ) -> Option<crate::record_field::CheckedRecordFieldSemanticId> {
        let field = self
            .checked_expression_edge_fact(owner)
            .ok()?
            .record_fields()
            .get(usize::try_from(source_ordinal).ok()?)?;
        (field.slot().source() == super::prepared::PreparedRecordValueSource::Local(local))
            .then(|| field.slot().semantic_id())
    }
    fn checked_expression_child_role(
        &self,
        parent: ExprId,
        child: ExprId,
    ) -> Option<CheckedExpressionChildRole> {
        self.checked_child_edges(parent)
            .ok()?
            .iter()
            .find_map(|edge| (edge.child() == child).then(|| edge.role().clone()))
    }
}

fn validate_checked_call_edge(
    module: Option<&HirModule>,
    facts: &super::CallTargetFacts,
    child: ExprId,
    role: &CheckedExpressionChildRole,
) -> Result<(), CheckedChildEdgeError> {
    let application = facts
        .selected_application()
        .ok_or(CheckedChildEdgeError::MissingCallFacts)?;
    match role {
        CheckedExpressionChildRole::Callee => Ok(()),
        CheckedExpressionChildRole::ContentCallee => Ok(()),
        CheckedExpressionChildRole::Argument { ordinal } => {
            let index =
                usize::try_from(*ordinal).map_err(|_| CheckedChildEdgeError::CallSlotMismatch)?;
            let execution = application.core().execution();
            let argument = execution
                .arguments()
                .get(index)
                .ok_or(CheckedChildEdgeError::CallSlotMismatch)?;
            if argument.passing() == CheckedCallArgumentPassing::Spread {
                let module = module.ok_or(CheckedChildEdgeError::CallSlotMismatch)?;
                let expression = module
                    .resolve_expr(child)
                    .map_err(|_| CheckedChildEdgeError::CallSlotMismatch)?;
                let whole_container = argument.slots().len() == 1
                    && argument.slots()[0].source().raw()
                        == CheckedCallArgumentSlotSource::Expression(child);
                let expanded_elements = match expression.kind() {
                    HirExprKind::BracketSequence(sequence) => argument
                        .slots()
                        .iter()
                        .map(|slot| slot.source().raw())
                        .eq(sequence
                            .elements()
                            .iter()
                            .copied()
                            .map(CheckedCallArgumentSlotSource::Expression)),
                    HirExprKind::NumericBracketSequence(sequence) => {
                        argument.slots().iter().enumerate().all(|(ordinal, slot)| {
                            u32::try_from(ordinal).ok().is_some_and(|ordinal| {
                                slot.source().raw()
                                    == CheckedCallArgumentSlotSource::CompactNumericElement {
                                        sequence: child,
                                        ordinal,
                                    }
                            })
                        }) && argument.slots().len() == sequence.elements().len()
                    }
                    _ => false,
                };
                return (whole_container || expanded_elements)
                    .then_some(())
                    .ok_or(CheckedChildEdgeError::CallSlotMismatch);
            }
            let runtime_sources = argument
                .slots()
                .iter()
                .filter(|slot| slot.source().owner() == child)
                .count();
            let semantic_sources = execution
                .semantic_operands()
                .iter()
                .filter(|operand| match operand.source() {
                    CheckedCallSemanticOperandSource::DialogueApplicationId {
                        argument,
                        source,
                        ..
                    }
                    | CheckedCallSemanticOperandSource::DialogueApplicationTextKey {
                        argument,
                        source,
                        ..
                    } => usize::from(argument.get()) == index && *source == child,
                    CheckedCallSemanticOperandSource::TextProxyObject { argument, source } => {
                        usize::from(argument.get()) == index && source.owner() == child
                    }
                    CheckedCallSemanticOperandSource::DialogueTarget(_)
                    | CheckedCallSemanticOperandSource::DialogueContent { .. }
                    | CheckedCallSemanticOperandSource::DialogueLinePlan { .. } => false,
                })
                .count();
            if (runtime_sources, semantic_sources) == (1, 0)
                || (runtime_sources, semantic_sources) == (0, 1)
            {
                Ok(())
            } else {
                Err(CheckedChildEdgeError::CallSlotMismatch)
            }
        }
        _ => Ok(()),
    }
}

fn refine_content_nominal_discriminator_edge(
    call: &super::CallTargetFacts,
    edges: &mut [(ExprId, CheckedExpressionChildRole)],
) -> Result<(), CheckedChildEdgeError> {
    let Some(application) = call.selected_application() else {
        return Ok(());
    };
    if !matches!(
        application.core().candidates().selected().id(),
        crate::callable::CallableCandidateId::Content(
            crate::callable::ContentCallableIdentity::TextProxyObject { .. },
        )
    ) {
        return Ok(());
    }
    let mut discriminator = None;
    for operand in application.core().execution().semantic_operands() {
        let crate::callable::CheckedCallSemanticOperandSource::TextProxyObject { argument, source } =
            operand.source()
        else {
            continue;
        };
        let row = (source.owner(), u32::from(argument.get()));
        if discriminator.replace(row).is_some() {
            return Err(CheckedChildEdgeError::CallSlotMismatch);
        }
    }
    let (source, ordinal) = discriminator.ok_or(CheckedChildEdgeError::CallSlotMismatch)?;
    let mut matching_index = None;
    let mut matching_count = 0usize;
    let mut ordinal_count = 0usize;
    for (index, (child, role)) in edges.iter().enumerate() {
        if matches!(
            role,
            CheckedExpressionChildRole::Argument { ordinal: actual } if *actual == ordinal
        ) {
            ordinal_count += 1;
            if *child == source {
                matching_count += 1;
                matching_index = Some(index);
            }
        }
    }
    if matching_count != 1
        || ordinal_count != 1
        || edges.iter().any(|(_, role)| {
            matches!(
                role,
                CheckedExpressionChildRole::ContentNominalDiscriminator { .. }
            )
        })
    {
        return Err(CheckedChildEdgeError::CallSlotMismatch);
    }
    let Some(index) = matching_index else {
        return Err(CheckedChildEdgeError::CallSlotMismatch);
    };
    edges[index].1 = CheckedExpressionChildRole::ContentNominalDiscriminator { ordinal };
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "the HIR-to-sema role projection is one exhaustive first-error mapping"
)]
fn checked_role_from_hir(
    role: &HirExpressionChildRole,
    accepted_field: Option<CheckedRecordFieldSemanticId>,
) -> Result<CheckedExpressionChildRole, CheckedChildEdgeError> {
    let path = checked_nested_path_from_hir;
    Ok(match role {
        HirExpressionChildRole::Element { ordinal } => {
            CheckedExpressionChildRole::Element { ordinal: *ordinal }
        }
        HirExpressionChildRole::RepeatedValue => CheckedExpressionChildRole::RepeatedValue,
        HirExpressionChildRole::RepeatLength => CheckedExpressionChildRole::RepeatLength,
        HirExpressionChildRole::Callee => CheckedExpressionChildRole::Callee,
        HirExpressionChildRole::ContentCallee => CheckedExpressionChildRole::ContentCallee,
        HirExpressionChildRole::Argument { ordinal } => {
            CheckedExpressionChildRole::Argument { ordinal: *ordinal }
        }
        HirExpressionChildRole::Target => CheckedExpressionChildRole::Target,
        HirExpressionChildRole::Index => CheckedExpressionChildRole::Index,
        HirExpressionChildRole::PipeLeft => CheckedExpressionChildRole::PipeLeft,
        HirExpressionChildRole::PipeRight => CheckedExpressionChildRole::PipeRight,
        HirExpressionChildRole::Operand => CheckedExpressionChildRole::Operand,
        HirExpressionChildRole::RangeStart => CheckedExpressionChildRole::RangeStart,
        HirExpressionChildRole::RangeEnd => CheckedExpressionChildRole::RangeEnd,
        HirExpressionChildRole::RecordField { source_ordinal } => {
            CheckedExpressionChildRole::RecordField {
                source_ordinal: *source_ordinal,
                accepted_field: accepted_field
                    .ok_or(CheckedChildEdgeError::MissingCheckedRecordField)?,
            }
        }
        HirExpressionChildRole::BinaryLeft => CheckedExpressionChildRole::BinaryLeft,
        HirExpressionChildRole::BinaryRight => CheckedExpressionChildRole::BinaryRight,
        HirExpressionChildRole::ClosureBody => CheckedExpressionChildRole::ClosureBody,
        HirExpressionChildRole::BlockTail => CheckedExpressionChildRole::BlockTail,
        HirExpressionChildRole::LoopTail => CheckedExpressionChildRole::LoopTail,
        HirExpressionChildRole::Condition => CheckedExpressionChildRole::Condition,
        HirExpressionChildRole::ThenBranch => CheckedExpressionChildRole::ThenBranch,
        HirExpressionChildRole::ElseBranch => CheckedExpressionChildRole::ElseBranch,
        HirExpressionChildRole::Scrutinee => CheckedExpressionChildRole::Scrutinee,
        HirExpressionChildRole::Guard { arm } => CheckedExpressionChildRole::Guard { arm: *arm },
        HirExpressionChildRole::ArmValue { arm } => {
            CheckedExpressionChildRole::ArmValue { arm: *arm }
        }
        HirExpressionChildRole::IfLetGuard => CheckedExpressionChildRole::IfLetGuard,
        HirExpressionChildRole::DialogueTarget => CheckedExpressionChildRole::DialogueTarget,
        HirExpressionChildRole::DialogueCoordinate { ordinal } => {
            CheckedExpressionChildRole::DialogueCoordinate { ordinal: *ordinal }
        }
        HirExpressionChildRole::DialogueInterpolation { ordinal } => {
            CheckedExpressionChildRole::DialogueInterpolation { ordinal: *ordinal }
        }
        HirExpressionChildRole::AttachedContentApplication { ordinal } => {
            CheckedExpressionChildRole::AttachedContentApplication { ordinal: *ordinal }
        }
        HirExpressionChildRole::DialoguePointActionPayload { ordinal } => {
            CheckedExpressionChildRole::DialoguePointActionPayload { ordinal: *ordinal }
        }
        HirExpressionChildRole::PostfixIndexCandidate => {
            CheckedExpressionChildRole::PostfixIndexCandidate
        }
        HirExpressionChildRole::PostfixDialogueCandidate => {
            CheckedExpressionChildRole::PostfixDialogueCandidate
        }
        HirExpressionChildRole::ForInput => CheckedExpressionChildRole::ForInput,
        HirExpressionChildRole::ChoiceIfCondition {
            path: value,
            branch,
        } => CheckedExpressionChildRole::ChoiceIfCondition {
            path: path(value)?,
            branch: *branch,
        },
        HirExpressionChildRole::ChoiceForSource { path: value } => {
            CheckedExpressionChildRole::ChoiceForSource { path: path(value)? }
        }
        HirExpressionChildRole::ChoiceMatchScrutinee { path: value } => {
            CheckedExpressionChildRole::ChoiceMatchScrutinee { path: path(value)? }
        }
        HirExpressionChildRole::ChoiceMatchGuard { path: value, arm } => {
            CheckedExpressionChildRole::ChoiceMatchGuard {
                path: path(value)?,
                arm: *arm,
            }
        }
        HirExpressionChildRole::ChoiceOptionId { path: value } => {
            CheckedExpressionChildRole::ChoiceOptionId { path: path(value)? }
        }
        HirExpressionChildRole::ChoiceOptionForSource { path: value } => {
            CheckedExpressionChildRole::ChoiceOptionForSource { path: path(value)? }
        }
        HirExpressionChildRole::ChoiceCompactLabel { path: value } => {
            CheckedExpressionChildRole::ChoiceCompactLabel { path: path(value)? }
        }
        HirExpressionChildRole::ChoiceCompactCondition { path: value } => {
            CheckedExpressionChildRole::ChoiceCompactCondition { path: path(value)? }
        }
        HirExpressionChildRole::ChoiceCompactOut { path: value } => {
            CheckedExpressionChildRole::ChoiceCompactOut { path: path(value)? }
        }
        HirExpressionChildRole::ChoiceOptionLabel { path: value, field } => {
            CheckedExpressionChildRole::ChoiceOptionLabel {
                path: path(value)?,
                field: *field,
            }
        }
        HirExpressionChildRole::ChoiceOptionFieldId { path: value, field } => {
            CheckedExpressionChildRole::ChoiceOptionFieldId {
                path: path(value)?,
                field: *field,
            }
        }
        HirExpressionChildRole::ChoiceOptionValue { path: value, field } => {
            CheckedExpressionChildRole::ChoiceOptionValue {
                path: path(value)?,
                field: *field,
            }
        }
        HirExpressionChildRole::ChoiceOptionVisible { path: value, field } => {
            CheckedExpressionChildRole::ChoiceOptionVisible {
                path: path(value)?,
                field: *field,
            }
        }
        HirExpressionChildRole::ChoiceOptionEnabled { path: value, field } => {
            CheckedExpressionChildRole::ChoiceOptionEnabled {
                path: path(value)?,
                field: *field,
            }
        }
        HirExpressionChildRole::ChoiceOptionOrder { path: value, field } => {
            CheckedExpressionChildRole::ChoiceOptionOrder {
                path: path(value)?,
                field: *field,
            }
        }
        HirExpressionChildRole::ChoiceOptionHotkey { path: value, field } => {
            CheckedExpressionChildRole::ChoiceOptionHotkey {
                path: path(value)?,
                field: *field,
            }
        }
        HirExpressionChildRole::ChoiceOptionViewKey {
            path: value,
            field,
            entry,
        } => CheckedExpressionChildRole::ChoiceOptionViewKey {
            path: path(value)?,
            field: *field,
            entry: *entry,
        },
        HirExpressionChildRole::ChoiceOptionViewValue {
            path: value,
            field,
            entry,
        } => CheckedExpressionChildRole::ChoiceOptionViewValue {
            path: path(value)?,
            field: *field,
            entry: *entry,
        },
        HirExpressionChildRole::ChoicePlanAssignment { item } => {
            CheckedExpressionChildRole::ChoicePlanAssignment { item: *item }
        }
        HirExpressionChildRole::ChoicePlanTimeout { item } => {
            CheckedExpressionChildRole::ChoicePlanTimeout { item: *item }
        }
        HirExpressionChildRole::ChoicePlanCancelSignal { item } => {
            CheckedExpressionChildRole::ChoicePlanCancelSignal { item: *item }
        }
        HirExpressionChildRole::ChoicePlanCancelTimeout { item } => {
            CheckedExpressionChildRole::ChoicePlanCancelTimeout { item: *item }
        }
        HirExpressionChildRole::ChoicePlanCancelExpr { item } => {
            CheckedExpressionChildRole::ChoicePlanCancelExpr { item: *item }
        }
    })
}
