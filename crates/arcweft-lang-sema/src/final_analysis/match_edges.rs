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
    identity::ExprId,
    module::HirModule,
    project::{
        HirExpressionEvaluationEdge, HirProjectEvaluationTopology,
        HirSelectedCallExpressionDisposition, HirSelectedCallExpressionInventory,
        HirSelectedExpressionGraph, HirSelectedExpressionInventoryError,
    },
};

use super::{
    CheckedExpressionResolution, CheckedMethodSelection, ExprId as SemaExprId,
    FinalCallSealFailure, FinalCallSealLocation, FinalSemanticAnalysis, FinalSemanticAnalysisError,
    HirExecutableProjectView, HirModuleId, SemanticFactFamily, TypeKind,
};
use crate::callable::{
    CheckedCallableCatalog, CheckedCallableJoin, CheckedCallableJoinError,
    validate_selected_application,
};
use crate::record_field::{AcceptedRecordFieldSemanticId, CheckedRecordFieldSemanticId};
use crate::semantic_coordinate::{
    CheckedExpressionChildRole, CheckedExpressionEdgeAuthority, CheckedNestedPathSegmentV1,
    CheckedNestedPathV1,
};

mod model;

pub use model::{
    CheckedChildEdgeError, CheckedExpressionEdgeError, CheckedExpressionEdgeFact,
    CheckedNestedEvidenceRole, NestedPathEvidence,
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

/// Resolves the exact expression inventory selected by checked postfix facts.
/// HIR owns traversal and candidate membership; the checked fact supplies only
/// the already-accepted candidate identity.
#[derive(Debug)]
pub(super) struct CheckedSelectedExpressionGraph {
    graph: HirSelectedExpressionGraph,
}

impl CheckedSelectedExpressionGraph {
    pub(super) fn seal(
        project: HirExecutableProjectView<'_>,
        topology: Arc<HirProjectEvaluationTopology>,
        expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
        prepared_calls: &super::analyzer::AnalyzerPreparedCallGraph,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let mut call_inventories = BTreeMap::<ExprId, HirSelectedCallExpressionInventory>::new();
        for site in prepared_calls.sites() {
            let crate::callable::CheckedCallSite::HirCall(owner) = site else {
                continue;
            };
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
                    |prefix| prefix.selected_expression_inventory(),
                    |unselected| Ok(unselected.selected_expression_inventory()),
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
        Self::seal_with_call_inventory(project, topology, expressions, |owner| {
            call_inventories
                .get(&owner)
                .cloned()
                .map(HirSelectedCallExpressionDisposition::Callable)
                .or_else(|| {
                    expressions
                        .get(&owner)
                        .filter(|expression| expression.checked_call_site(owner).is_none())
                        .map(|_| HirSelectedCallExpressionDisposition::Structural)
                })
        })
    }

    /// Manual fact fixtures may omit prepared-call state only when their HIR
    /// contains no selected Call. Encountering a Call fails closed through the
    /// same inventory error; this helper never reconstructs children from raw
    /// syntax or final expression membership.
    #[cfg(test)]
    pub(super) fn seal_call_free_fixture(
        project: HirExecutableProjectView<'_>,
        topology: Arc<HirProjectEvaluationTopology>,
        expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        Self::seal_with_call_inventory(project, topology, expressions, |owner| {
            expressions
                .get(&owner)
                .filter(|expression| expression.checked_call_site(owner).is_none())
                .map(|_| HirSelectedCallExpressionDisposition::Structural)
        })
    }

    fn seal_with_call_inventory(
        project: HirExecutableProjectView<'_>,
        topology: Arc<HirProjectEvaluationTopology>,
        expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
        selected_call: impl FnMut(ExprId) -> Option<HirSelectedCallExpressionDisposition>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let graph = project
            .selected_expression_graph(
                &topology,
                |owner| expressions.get(&owner)?.selected_postfix_candidate(),
                selected_call,
            )
            .map_err(|error| match error {
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
                    FinalSemanticAnalysisError::CallSeal(FinalCallSealFailure::new(
                        FinalCallSealLocation::Site(crate::callable::CheckedCallSite::HirCall(
                            expression,
                        )),
                        crate::callable::CallConstraintInvariant::MissingOrStalePreparedNode,
                    ))
                }
                HirSelectedExpressionInventoryError::InvalidSelectedCallCallee {
                    expression,
                    ..
                } => FinalSemanticAnalysisError::CallSeal(FinalCallSealFailure::new(
                    FinalCallSealLocation::Site(crate::callable::CheckedCallSite::HirCall(
                        expression,
                    )),
                    crate::callable::CallConstraintInvariant::PreparedCallSiteMismatch,
                )),
                HirSelectedExpressionInventoryError::InvalidSelectedCallArguments {
                    expression,
                } => FinalSemanticAnalysisError::CallSeal(FinalCallSealFailure::new(
                    FinalCallSealLocation::Site(crate::callable::CheckedCallSite::HirCall(
                        expression,
                    )),
                    crate::callable::CallConstraintInvariant::MalformedMapperSeal,
                )),
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
            })?;
        Ok(Self { graph })
    }

    pub(super) fn topology(&self) -> &Arc<HirProjectEvaluationTopology> {
        self.graph.topology()
    }

    pub(super) fn owners(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.graph.expression_owners()
    }
}

fn selected_expression_edges(
    graph: &CheckedSelectedExpressionGraph,
) -> BTreeMap<ExprId, Box<[SelectedHirExpressionEdge]>> {
    graph
        .graph
        .expression_owners()
        .map(|owner| {
            let edges = graph
                .graph
                .expression_edges(owner)
                .iter()
                .filter_map(|edge| match edge {
                    HirExpressionEvaluationEdge::Expression {
                        role,
                        ownership: arcweft_lang_hir::expr::HirExpressionChildOwnership::Owning,
                        child,
                    } => Some((*child, role.clone())),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .into_boxed_slice();
            (owner, edges)
        })
        .collect()
}

/// Move-only structural edge authority sealed before call application
/// finalization.  C1 semantic coordinates borrow this exact draft; final edge
/// publication later consumes it together with callable joins.  No second HIR
/// walk or parallel checked-role table is permitted across those phases.
#[derive(Debug)]
pub(super) struct CheckedStructuralEdgeDraft {
    facts: BTreeMap<
        ExprId,
        Result<Box<[(ExprId, CheckedExpressionChildRole)]>, CheckedChildEdgeError>,
    >,
    call_owners: BTreeSet<ExprId>,
    record_owners: BTreeSet<ExprId>,
    record_fields: Option<BTreeMap<ExprId, Box<[super::CheckedExpressionRecordField]>>>,
}

impl CheckedExpressionEdgeAuthority for CheckedStructuralEdgeDraft {
    fn checked_expression_child_role(
        &self,
        parent: ExprId,
        child: ExprId,
    ) -> Option<CheckedExpressionChildRole> {
        self.facts
            .get(&parent)?
            .as_ref()
            .ok()?
            .iter()
            .find_map(|(candidate, role)| (*candidate == child).then(|| role.clone()))
    }
}

impl CheckedStructuralEdgeDraft {
    pub(super) fn attach_record_fields(
        &mut self,
        fields: BTreeMap<ExprId, Box<[super::CheckedExpressionRecordField]>>,
    ) -> Result<(), CheckedChildEdgeError> {
        if self.record_fields.is_some()
            || !fields
                .keys()
                .copied()
                .eq(self.record_owners.iter().copied())
        {
            return Err(CheckedChildEdgeError::UnexpectedCheckedRecordField);
        }
        self.record_fields = Some(fields);
        Ok(())
    }

    fn call_callee(&self, owner: ExprId) -> Result<Option<ExprId>, CheckedCallableJoinError> {
        let edges = self
            .facts
            .get(&owner)
            .ok_or(CheckedCallableJoinError::NotSelected)?
            .as_ref()
            .map_err(|_| CheckedCallableJoinError::NotSelected)?;
        let mut callees = edges.iter().filter_map(|(child, role)| {
            matches!(role, CheckedExpressionChildRole::Callee).then_some(*child)
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
        let raw_edges = selected_expression_edges(selected);
        let mut facts = BTreeMap::new();
        let mut call_owners = BTreeSet::new();
        let mut record_owners = BTreeSet::new();
        for (owner, edges) in raw_edges {
            let Some(owner_expression) = modules
                .get(&owner.module())
                .and_then(|module| module.resolve_expr(owner).ok())
            else {
                facts.insert(owner, Err(CheckedChildEdgeError::MissingExpression));
                continue;
            };
            if matches!(owner_expression.kind(), HirExprKind::Call(_)) {
                call_owners.insert(owner);
            }
            let Some(checked_owner) = expressions.get(&owner) else {
                facts.insert(owner, Err(CheckedChildEdgeError::MissingExpression));
                continue;
            };
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
                    record_owners.insert(owner);
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
                        match prepared_record_field(checked_owner, child, *source_ordinal) {
                            Ok(field) => Some(field),
                            Err(error) => {
                                first_error = Some(error);
                                break;
                            }
                        }
                    }
                    _ => None,
                };
                if matches!(role, HirExpressionChildRole::Guard { .. })
                    && checked_child.ty() != &TypeKind::Bool
                {
                    first_error = Some(CheckedChildEdgeError::MatchGuardTypeMismatch);
                    break;
                }
                if matches!(role, HirExpressionChildRole::ChoiceMatchGuard { .. })
                    && checked_child.ty() != &TypeKind::Bool
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
            facts.insert(
                owner,
                first_error.map_or_else(|| Ok(enriched.into_boxed_slice()), Err),
            );
        }
        Self {
            facts,
            call_owners,
            record_owners,
            record_fields: None,
        }
    }

    pub(super) fn into_final_facts(
        mut self,
        calls: &BTreeMap<ExprId, super::CallTargetFacts>,
        mut callable_joins: PreparedCallableJoins,
    ) -> (
        BTreeMap<ExprId, Result<CheckedExpressionEdgeFact, CheckedExpressionEdgeError>>,
        PreparedCallableJoins,
    ) {
        let mut final_facts = BTreeMap::new();
        let record_fields_attached = self.record_fields.is_some();
        let mut record_fields = self.record_fields.take().unwrap_or_default();
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
            let edges = match structural {
                Ok(edges) => edges,
                Err(error) => {
                    final_facts.insert(owner, Err(CheckedExpressionEdgeError::Child(error)));
                    continue;
                }
            };
            let record_field_rows = record_fields.remove(&owner);
            if self.record_owners.contains(&owner) != record_field_rows.is_some()
                || (self.record_owners.contains(&owner) && !record_fields_attached)
            {
                final_facts.insert(
                    owner,
                    Err(CheckedExpressionEdgeError::Child(
                        CheckedChildEdgeError::MissingCheckedRecordField,
                    )),
                );
                continue;
            }
            let record_fields = record_field_rows.unwrap_or_default();
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
                if let Some(error) = edges
                    .iter()
                    .find_map(|(child, role)| validate_checked_call_edge(call, *child, role).err())
                {
                    final_facts.insert(owner, Err(CheckedExpressionEdgeError::Child(error)));
                    continue;
                }
            }
            match CheckedExpressionEdgeFact::try_new(edges, record_fields, callable) {
                Ok(fact) => {
                    final_facts.insert(owner, Ok(fact));
                }
                Err(error) => {
                    final_facts.insert(owner, Err(CheckedExpressionEdgeError::Child(error)));
                }
            }
        }
        for owner in record_fields.into_keys() {
            final_facts.insert(
                owner,
                Err(CheckedExpressionEdgeError::Child(
                    CheckedChildEdgeError::UnexpectedCheckedRecordField,
                )),
            );
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
            if !matches!(
                facts.outcome().site(),
                crate::callable::CheckedCallSite::HirCall(_)
            ) {
                return None;
            }
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
    let call_owners = calls
        .iter()
        .filter(|(_, facts)| {
            matches!(
                facts.outcome().site(),
                crate::callable::CheckedCallSite::HirCall(_)
            )
        })
        .collect::<Vec<_>>();
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
) -> Result<BTreeMap<ExprId, CheckedMethodSelection>, CheckedCallableJoinError> {
    let mut methods = BTreeMap::new();
    for (call_owner, joined) in joins {
        let Ok(join) = joined else {
            continue;
        };
        let Some(value) = structural_edges.call_callee(*call_owner)? else {
            continue;
        };
        let Some(checked) = expressions.get(&value) else {
            return Err(CheckedCallableJoinError::MissingCheckedRecord);
        };
        if !matches!(checked, super::PreparedExpressionFact::Method(_)) {
            continue;
        }
        let selection = CheckedMethodSelection::try_from_join(join)
            .ok_or(CheckedCallableJoinError::ReceiverModeMismatch)?;
        if methods.insert(value, selection).is_some() {
            return Err(CheckedCallableJoinError::MethodLookupAmbiguous);
        }
    }

    let prepared = expressions
        .iter()
        .filter_map(|(owner, checked)| {
            matches!(checked, super::PreparedExpressionFact::Method(_)).then_some(*owner)
        })
        .collect::<Vec<_>>();
    if prepared.len() != methods.len() || prepared.iter().any(|owner| !methods.contains_key(owner))
    {
        return Err(CheckedCallableJoinError::NotSelected);
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
                if !matches!(checked_guard.ty(), TypeKind::Bool) {
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
            if !matches!(checked_guard.ty(), TypeKind::Bool) {
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
    LinePlan,
}

impl CheckedNestedPathSegmentV1 {
    fn family(&self) -> NestedPathFamily {
        match self {
            Self::ChoiceBodyItem { .. }
            | Self::ChoiceIfBranch { .. }
            | Self::ChoiceIfElse
            | Self::ChoiceForBody
            | Self::ChoiceMatchArm { .. }
            | Self::ChoiceOptionBody
            | Self::ChoiceOptionField { .. }
            | Self::ChoiceViewEntry { .. }
            | Self::ChoicePlanItem { .. } => NestedPathFamily::Choice,
            Self::LinePlanItem { .. }
            | Self::LinePlanStartGroupItem { .. }
            | Self::LinePlanTogetherGroupItem { .. } => NestedPathFamily::LinePlan,
        }
    }
}

fn nested_path_role(
    role: &HirExpressionChildRole,
) -> Option<(&HirNestedExpressionPath, NestedPathFamily)> {
    Some(match role {
        HirExpressionChildRole::LinePlanOptionValue { path }
        | HirExpressionChildRole::LinePlanLetValue { path }
        | HirExpressionChildRole::LinePlanOut { path }
        | HirExpressionChildRole::LinePlanTimelineAssert { path }
        | HirExpressionChildRole::LinePlanExpression { path } => (path, NestedPathFamily::LinePlan),
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
            HirExprKind::DialogueContentApplication(_),
            CheckedExpressionResolution::DialogueApplication { .. },
        ) => NestedPathFamily::LinePlan,
        _ => return None,
    };
    build_nested_path_evidence_for_family(owner_family, edges, expressions)
}

/// Builds line-plan nested-path evidence while the dialogue expression is
/// still private and awaiting its callable seal.  The optional outer result
/// is intentional: the prepared owner may be observed before the structural
/// edge issuer has selected a path-bearing family.
pub(crate) fn build_line_plan_nested_path_evidence(
    edges: &[SelectedHirExpressionEdge],
    expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
) -> Option<Result<NestedPathEvidence, CheckedChildEdgeError>> {
    build_nested_path_evidence_for_family(NestedPathFamily::LinePlan, edges, expressions)
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
            .map(CheckedNestedPathSegmentV1::family)
            .ok_or(CheckedChildEdgeError::MissingNestedPath);
        let Ok(path_family) = path_family else {
            return Some(Err(CheckedChildEdgeError::MissingNestedPath));
        };
        if path_family != family
            || path
                .segments()
                .iter()
                .any(|segment| segment.family() != path_family)
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
    edges: &[SelectedHirExpressionEdge],
    expressions: &BTreeMap<ExprId, super::PreparedExpressionFact>,
) -> Result<(), CheckedChildEdgeError> {
    // A prepared owner may legitimately reach this draft before the issuer
    // has attached path evidence.  When evidence is present, however, it is
    // checked against the same HIR-owned edge projection as completed rows.
    let Some(stored) = prepared.nested_path_evidence() else {
        return Ok(());
    };
    let stored = stored.as_ref().map_err(Clone::clone)?;
    let Some(expected) = build_line_plan_nested_path_evidence(edges, expressions) else {
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
    ) -> Result<&[(SemaExprId, CheckedExpressionChildRole)], CheckedExpressionEdgeError> {
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
    fn checked_expression_child_role(
        &self,
        parent: ExprId,
        child: ExprId,
    ) -> Option<CheckedExpressionChildRole> {
        self.checked_child_edges(parent)
            .ok()?
            .iter()
            .find_map(|(candidate, role)| (*candidate == child).then(|| role.clone()))
    }
}

fn validate_checked_call_edge(
    facts: &super::CallTargetFacts,
    child: ExprId,
    role: &CheckedExpressionChildRole,
) -> Result<(), CheckedChildEdgeError> {
    let application = facts
        .selected_application()
        .ok_or(CheckedChildEdgeError::MissingCallFacts)?;
    match role {
        CheckedExpressionChildRole::Callee => Ok(()),
        CheckedExpressionChildRole::Argument { ordinal } => application
            .core()
            .execution()
            .arguments()
            .get(usize::try_from(*ordinal).map_err(|_| CheckedChildEdgeError::CallSlotMismatch)?)
            .filter(|argument| {
                argument
                    .slots()
                    .iter()
                    .any(|slot| slot.source().owner() == child)
            })
            .map(|_| ())
            .ok_or(CheckedChildEdgeError::CallSlotMismatch),
        _ => Ok(()),
    }
}

fn prepared_record_field(
    checked: &super::PreparedExpressionFact,
    child: ExprId,
    source_ordinal: u32,
) -> Result<CheckedRecordFieldSemanticId, CheckedChildEdgeError> {
    let super::PreparedExpressionFact::ProjectRecord(record) = checked else {
        return Err(CheckedChildEdgeError::MissingCheckedRecordField);
    };
    let field = record
        .fields()
        .iter()
        .find(|field| field.source_ordinal() == source_ordinal)
        .ok_or(CheckedChildEdgeError::MissingCheckedRecordField)?;
    if field.source() != super::PreparedRecordValueSource::Expression(child) {
        return Err(CheckedChildEdgeError::MissingCheckedRecordField);
    }
    Ok(CheckedRecordFieldSemanticId::Project(
        AcceptedRecordFieldSemanticId::issue(
            record.nominal().identity(),
            field.declaration_ordinal(),
            field.field_type().semantic_identity_digest(),
        ),
    ))
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
        HirExpressionChildRole::DialogueTagPayload { ordinal } => {
            CheckedExpressionChildRole::DialogueTagPayload { ordinal: *ordinal }
        }
        HirExpressionChildRole::LinePlanOptionValue { path: value } => {
            CheckedExpressionChildRole::LinePlanOptionValue { path: path(value)? }
        }
        HirExpressionChildRole::LinePlanLetValue { path: value } => {
            CheckedExpressionChildRole::LinePlanLetValue { path: path(value)? }
        }
        HirExpressionChildRole::LinePlanOut { path: value } => {
            CheckedExpressionChildRole::LinePlanOut { path: path(value)? }
        }
        HirExpressionChildRole::LinePlanTimelineAssert { path: value } => {
            CheckedExpressionChildRole::LinePlanTimelineAssert { path: path(value)? }
        }
        HirExpressionChildRole::LinePlanExpression { path: value } => {
            CheckedExpressionChildRole::LinePlanExpression { path: path(value)? }
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
