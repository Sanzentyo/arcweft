//! Checked Match child-edge enrichment and semantic callable joins.
//!
//! HIR owns the structural child walk.  This module is deliberately the
//! semantic half of that boundary: it projects the HIR-only role vocabulary
//! into accepted identities only after the corresponding final-analysis fact
//! has been found.  In particular, no source spelling or arena identity is
//! used as a fallback identity.

use std::{collections::BTreeMap, sync::Arc};

use arcweft_core::value::RuntimeRecordFieldId;
use arcweft_lang_hir::{
    expr::{
        HirCallCallee, HirExprKind, HirExpressionChildEdge, HirExpressionChildRole,
        HirNestedExpressionPath, HirSelectedMember,
    },
    identity::ExprId,
    module::HirModule,
    symbol::{ProjectSymbolTable, nominal::ProjectNominalBody},
};

use super::{
    CallTargetFact, CheckedExpressionResolution, ExprId as SemaExprId, FinalSemanticAnalysis,
    HirModuleId, TypeId, TypeKind,
};
use crate::callable::{
    CallableName, CheckedCallableCatalog, CheckedCallableJoin, CheckedCallableJoinError,
    ReceiverMethodKey, validate_selected_call,
};

mod model;

pub use model::{
    CheckedChildEdgeError, CheckedExpressionChildRole, CheckedExpressionEdgeError,
    CheckedExpressionEdgeFact, CheckedNestedEvidenceRole, CheckedNestedPathError,
    CheckedNestedPathSegmentV1, CheckedNestedPathV1, NestedPathEvidence,
};

/// Copies the one structural edge inventory for all final-HIR expressions at
/// report publication.  The raw rows are publication-only input; they are
/// never retained by `FinalSemanticAnalysis`.
pub(super) fn collect_child_edges(
    modules: &BTreeMap<HirModuleId, &HirModule>,
) -> BTreeMap<ExprId, Box<[HirExpressionChildEdge]>> {
    modules
        .values()
        .flat_map(|module| {
            module.expressions().map(|(owner, expression)| {
                (owner, expression.kind().child_edges().into_boxed_slice())
            })
        })
        .collect()
}

fn validate_match_owner(
    kind: &HirExprKind,
    checked: &super::CheckedExpression,
    expressions: &BTreeMap<ExprId, super::CheckedExpression>,
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
    expressions: &BTreeMap<ExprId, super::CheckedExpression>,
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
        | HirExpressionChildRole::LinePlanExpression { path }
        | HirExpressionChildRole::LinePlanTimedCueAnchor { path }
        | HirExpressionChildRole::LinePlanTimedCueBody { path } => {
            (path, NestedPathFamily::LinePlan)
        }
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
    edges: &[HirExpressionChildEdge],
    expressions: &BTreeMap<ExprId, super::CheckedExpression>,
) -> Option<Result<NestedPathEvidence, CheckedChildEdgeError>> {
    let owner_family = match (kind, checked.resolution()) {
        (HirExprKind::Choice(_), CheckedExpressionResolution::Choice(_)) => {
            Some(NestedPathFamily::Choice)
        }
        (
            HirExprKind::DialogueContentApplication(_),
            CheckedExpressionResolution::DialogueApplication { .. },
        ) => Some(NestedPathFamily::LinePlan),
        _ => return None,
    };
    let mut evidence =
        BTreeMap::<CheckedNestedPathV1, Vec<(CheckedNestedEvidenceRole, ExprId)>>::new();
    for edge in edges {
        let Some((hir_path, family)) = nested_path_role(edge.role()) else {
            continue;
        };
        if owner_family != Some(family) {
            return Some(Err(CheckedChildEdgeError::StaleNestedPath));
        }
        let path = match CheckedNestedPathV1::from_hir(hir_path) {
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
        if !expressions.contains_key(&edge.child()) {
            return Some(Err(CheckedChildEdgeError::MissingExpression));
        }
        let checked_role = match checked_role_from_hir(edge.role(), None) {
            Ok(role) => role,
            Err(error) => return Some(Err(error)),
        };
        let Some(role) = CheckedNestedEvidenceRole::from_checked_role(&checked_role) else {
            return Some(Err(CheckedChildEdgeError::StaleNestedPath));
        };
        evidence.entry(path).or_default().push((role, edge.child()));
    }
    Some(Ok(evidence
        .into_iter()
        .map(|(path, entries)| (path, entries.into_boxed_slice()))
        .collect()))
}

fn validate_nested_path_evidence(
    kind: &HirExprKind,
    checked: &super::CheckedExpression,
    edges: &[HirExpressionChildEdge],
    expressions: &BTreeMap<ExprId, super::CheckedExpression>,
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

/// Enriches all owners that have complete checked evidence and retains only
/// those final sema rows.  Recoverable/ambiguous call facts remain available
/// through their ordinary call inventory, but cannot accidentally expose a
/// partial child transcript.
#[allow(
    clippy::too_many_lines,
    reason = "one publication transaction owns the exhaustive edge/callable first-error order"
)]
pub(super) fn collect_checked_edges(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    symbols: &ProjectSymbolTable,
    types: &BTreeMap<TypeId, TypeKind>,
    expressions: &BTreeMap<ExprId, super::CheckedExpression>,
    calls: &BTreeMap<ExprId, super::CallTargetFacts>,
    checked_callables: &Arc<CheckedCallableCatalog>,
    raw_edges: BTreeMap<ExprId, Box<[HirExpressionChildEdge]>>,
) -> BTreeMap<ExprId, Result<CheckedExpressionEdgeFact, CheckedExpressionEdgeError>> {
    let mut facts = BTreeMap::new();
    for (owner, edges) in raw_edges {
        let owner_expression = modules
            .get(&owner.module())
            .and_then(|module| module.resolve_expr(owner).ok());
        let is_call_owner = owner_expression
            .as_ref()
            .is_some_and(|expression| matches!(expression.kind(), HirExprKind::Call(_)));
        let callable_result = if is_call_owner {
            if let Some(call_facts) = calls.get(&owner) {
                match method_key_for_call(owner, modules, types, expressions, call_facts) {
                    Ok(method_key) => Some(validate_selected_call(
                        call_facts,
                        checked_callables,
                        method_key.as_ref(),
                    )),
                    Err(error) => Some(Err(error)),
                }
            } else {
                Some(Err(CheckedCallableJoinError::NotSelected))
            }
        } else {
            None
        };
        let callable_join = callable_result
            .as_ref()
            .and_then(|result| result.as_ref().ok().cloned());

        // A call owner must publish a callable join together with its edge
        // vector.  Recoverable call facts remain accepted in the ordinary
        // report, but this owner-level product is rejected atomically.
        if is_call_owner && callable_join.is_none() {
            let error = callable_result
                .and_then(Result::err)
                .unwrap_or(CheckedCallableJoinError::NotSelected);
            facts.insert(owner, Err(CheckedExpressionEdgeError::Callable(error)));
            continue;
        }

        let Some(owner_expression) = owner_expression else {
            facts.insert(
                owner,
                Err(CheckedExpressionEdgeError::Child(
                    CheckedChildEdgeError::MissingExpression,
                )),
            );
            continue;
        };
        let Some(checked_owner) = expressions.get(&owner) else {
            facts.insert(
                owner,
                Err(CheckedExpressionEdgeError::Child(
                    CheckedChildEdgeError::MissingExpression,
                )),
            );
            continue;
        };
        if let Err(error) =
            validate_match_owner(owner_expression.kind(), checked_owner, expressions)
        {
            facts.insert(owner, Err(CheckedExpressionEdgeError::Child(error)));
            continue;
        }
        if let Err(error) = validate_nested_path_evidence(
            owner_expression.kind(),
            checked_owner,
            &edges,
            expressions,
        ) {
            facts.insert(owner, Err(CheckedExpressionEdgeError::Child(error)));
            continue;
        }
        let mut enriched = Vec::with_capacity(edges.len());
        let mut first_error = None;
        for edge in &edges {
            let child = edge.child();
            let Some(checked_child) = expressions.get(&child) else {
                first_error = Some(CheckedChildEdgeError::MissingExpression);
                break;
            };
            if let Some(facts) = calls.get(&owner)
                && let Err(error) = validate_call_edge(facts, child, edge.role())
            {
                first_error = Some(error);
                break;
            }
            if let Err(error) = validate_match_edge(
                owner_expression.kind(),
                checked_owner,
                child,
                edge.role(),
                expressions,
            ) {
                first_error = Some(error);
                break;
            }
            let accepted_field = match edge.role() {
                HirExpressionChildRole::RecordField { source_ordinal } => {
                    match accepted_record_field(
                        owner,
                        owner_expression.kind(),
                        *source_ordinal,
                        symbols,
                        expressions,
                    ) {
                        Ok(field) => Some(field),
                        Err(error) => {
                            first_error = Some(error);
                            break;
                        }
                    }
                }
                _ => None,
            };
            if matches!(edge.role(), HirExpressionChildRole::Guard { .. })
                && checked_child.ty() != &TypeKind::Bool
            {
                first_error = Some(CheckedChildEdgeError::MatchGuardTypeMismatch);
                break;
            }
            if matches!(edge.role(), HirExpressionChildRole::ChoiceMatchGuard { .. })
                && checked_child.ty() != &TypeKind::Bool
            {
                first_error = Some(CheckedChildEdgeError::MatchGuardTypeMismatch);
                break;
            }
            let role = match checked_role_from_hir(edge.role(), accepted_field) {
                Ok(role) => role,
                Err(error) => {
                    first_error = Some(error);
                    break;
                }
            };
            enriched.push((child, role));
        }
        if let Some(error) = first_error {
            facts.insert(owner, Err(CheckedExpressionEdgeError::Child(error)));
        } else {
            facts.insert(
                owner,
                Ok(CheckedExpressionEdgeFact::new(
                    enriched.into_boxed_slice(),
                    callable_join,
                )),
            );
        }
    }
    facts
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
    pub fn checked_child_edges(
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

fn validate_call_edge(
    facts: &super::CallTargetFacts,
    child: ExprId,
    role: &HirExpressionChildRole,
) -> Result<(), CheckedChildEdgeError> {
    if !matches!(facts.target(), CallTargetFact::Selected { .. }) {
        return Err(CheckedChildEdgeError::MissingCallFacts);
    }
    match role {
        HirExpressionChildRole::Callee => match facts.callee() {
            Some(crate::callable::CallCalleeClassificationFact::Value { expression })
                if expression == child =>
            {
                Ok(())
            }
            Some(crate::callable::CallCalleeClassificationFact::AssociatedType { .. }) => Ok(()),
            _ => Err(CheckedChildEdgeError::CallSlotMismatch),
        },
        HirExpressionChildRole::Argument { ordinal } => {
            let argument = facts
                .arguments()
                .get(
                    usize::try_from(*ordinal)
                        .map_err(|_| CheckedChildEdgeError::CallSlotMismatch)?,
                )
                .ok_or(CheckedChildEdgeError::CallSlotMismatch)?;
            argument
                .slots()
                .iter()
                .any(|slot| slot.source().owner() == child)
                .then_some(())
                .ok_or(CheckedChildEdgeError::CallSlotMismatch)
        }
        _ => Ok(()),
    }
}

fn method_key_for_call(
    owner: ExprId,
    modules: &BTreeMap<HirModuleId, &HirModule>,
    types: &BTreeMap<TypeId, TypeKind>,
    expressions: &BTreeMap<ExprId, super::CheckedExpression>,
    facts: &super::CallTargetFacts,
) -> Result<Option<ReceiverMethodKey>, CheckedCallableJoinError> {
    let CallTargetFact::Selected { selected, .. } = facts.target() else {
        return Ok(None);
    };
    let requires_key = matches!(
        selected.instantiation(),
        crate::callable::CallableInstantiation::Receiver { .. }
            | crate::callable::CallableInstantiation::TypeReceiver { .. }
            | crate::callable::CallableInstantiation::Extension { .. }
    );
    if !requires_key {
        return Ok(None);
    }
    let Some(module) = modules.get(&owner.module()) else {
        return Err(CheckedCallableJoinError::MissingReceiverKey);
    };
    let expression = module
        .resolve_expr(owner)
        .map_err(|_| CheckedCallableJoinError::MissingReceiverKey)?;
    let HirExprKind::Call(call) = expression.kind() else {
        return Err(CheckedCallableJoinError::MissingReceiverKey);
    };
    let (receiver, name) = match call.callee() {
        HirCallCallee::Value { value } => {
            let callee = module
                .resolve_expr(*value)
                .map_err(|_| CheckedCallableJoinError::MissingReceiverKey)?;
            let HirExprKind::Select(select) = callee.kind() else {
                return Err(CheckedCallableJoinError::MissingReceiverKey);
            };
            let HirSelectedMember::Name(name) = select.member() else {
                return Err(CheckedCallableJoinError::MissingReceiverKey);
            };
            (
                expressions
                    .get(&select.target())
                    .ok_or(CheckedCallableJoinError::MissingReceiverKey)?
                    .ty()
                    .clone(),
                name.clone(),
            )
        }
        HirCallCallee::UnresolvedDot {
            value_receiver,
            member,
            ..
        } => (
            expressions
                .get(value_receiver)
                .ok_or(CheckedCallableJoinError::MissingReceiverKey)?
                .ty()
                .clone(),
            member
                .resolved()
                .cloned()
                .ok_or(CheckedCallableJoinError::MissingReceiverKey)?,
        ),
        HirCallCallee::Associated {
            receiver, member, ..
        } => {
            let receiver = receiver
                .type_id()
                .and_then(|receiver| types.get(&receiver))
                .cloned()
                .ok_or(CheckedCallableJoinError::MissingReceiverKey)?;
            (
                receiver,
                member
                    .resolved()
                    .cloned()
                    .ok_or(CheckedCallableJoinError::MissingReceiverKey)?,
            )
        }
    };
    let method = CallableName::try_new(name.as_str())
        .map_err(|_| CheckedCallableJoinError::MissingReceiverKey)?;
    Ok(Some(ReceiverMethodKey::new(receiver, method)))
}

fn accepted_record_field(
    owner: ExprId,
    owner_kind: &HirExprKind,
    source_ordinal: u32,
    symbols: &ProjectSymbolTable,
    expressions: &BTreeMap<ExprId, super::CheckedExpression>,
) -> Result<RuntimeRecordFieldId, CheckedChildEdgeError> {
    let checked = expressions
        .get(&owner)
        .ok_or(CheckedChildEdgeError::MissingExpression)?;
    let CheckedExpressionResolution::Nominal(nominal) = checked.resolution() else {
        return Err(CheckedChildEdgeError::MissingCheckedRecordField);
    };
    let fields = match owner_kind {
        HirExprKind::Record(record) => record.fields(),
        HirExprKind::RecordLiteral(record) => record.fields(),
        _ => return Err(CheckedChildEdgeError::MissingCheckedRecordField),
    };
    let field = fields
        .get(
            usize::try_from(source_ordinal)
                .map_err(|_| CheckedChildEdgeError::MissingCheckedRecordField)?,
        )
        .ok_or(CheckedChildEdgeError::MissingCheckedRecordField)?;
    let name = field
        .name()
        .ok_or(CheckedChildEdgeError::MissingCheckedRecordField)?;
    let declaration = symbols
        .nominal(nominal.declaration())
        .ok_or(CheckedChildEdgeError::MissingCheckedRecordField)?;
    let ProjectNominalBody::Struct { fields } = declaration.body() else {
        return Err(CheckedChildEdgeError::MissingCheckedRecordField);
    };
    let declared_ordinal = fields
        .iter()
        .position(|declared| declared.name().as_str() == name.as_str())
        .ok_or(CheckedChildEdgeError::MissingCheckedRecordField)?;
    RuntimeRecordFieldId::try_from_zero_based_ordinal(declared_ordinal)
        .map_err(|_| CheckedChildEdgeError::MissingCheckedRecordField)
}

#[allow(
    clippy::too_many_lines,
    reason = "the HIR-to-sema role projection is one exhaustive first-error mapping"
)]
fn checked_role_from_hir(
    role: &HirExpressionChildRole,
    accepted_field: Option<RuntimeRecordFieldId>,
) -> Result<CheckedExpressionChildRole, CheckedChildEdgeError> {
    let path = |path: &HirNestedExpressionPath| CheckedNestedPathV1::from_hir(path);
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
        HirExpressionChildRole::LinePlanTimedCueAnchor { path: value } => {
            CheckedExpressionChildRole::LinePlanTimedCueAnchor { path: path(value)? }
        }
        HirExpressionChildRole::LinePlanTimedCueBody { path: value } => {
            CheckedExpressionChildRole::LinePlanTimedCueBody { path: path(value)? }
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
