//! Candidate selection and final callable semantic projections.

use super::super::{
    AcceptedCandidateRank, CallableAuthorityRank, CallableGroupIndex, CallableInstantiation,
    CandidateSelection, CheckedCallArgumentSlotSource, CheckedCallableCatalog,
    CheckedProjectNominal, EffectRow, EffectSet, FinalSemanticAnalysisError,
    GenericParameterOwnerId, GenericTypeParameterId, HirCallArgument, HirCallValue,
    MappedCallArgumentSlot, Ordering, PhysicalArgumentEvaluationKind, PreparedResolvedCallable,
    ProjectNominalDeclaration, ProjectNominalType, SpreadArgumentPolicy, TypeKind,
    TypeParameterSubstitutions,
};
use super::PreparedCandidateOutcome;
use crate::callable::ResolvedCallable;

pub(super) fn physical_evaluation_kind(
    argument: &HirCallArgument,
    slot: &MappedCallArgumentSlot,
    shape_rejected: bool,
    spread: SpreadArgumentPolicy,
) -> PhysicalArgumentEvaluationKind {
    if matches!(argument.value_state(), HirCallValue::Missing { .. }) {
        return PhysicalArgumentEvaluationKind::Recovered;
    }
    if shape_rejected || slot.coordinate().is_none() {
        return PhysicalArgumentEvaluationKind::Unmapped;
    }
    if matches!(argument, HirCallArgument::Spread { .. })
        && slot.source() != CheckedCallArgumentSlotSource::Expression(argument.value())
    {
        return PhysicalArgumentEvaluationKind::FixedLiteralSpread;
    }
    if matches!(argument, HirCallArgument::Spread { .. })
        && spread == SpreadArgumentPolicy::TypedRest
    {
        return PhysicalArgumentEvaluationKind::TypedRestSpread;
    }
    PhysicalArgumentEvaluationKind::Authored
}

pub(in super::super) fn source_callable_schema_type(
    schema: &crate::callable::CallableSignatureSchema,
) -> Option<TypeKind> {
    let mut result = schema.result().clone();
    for group in schema.groups().iter().rev() {
        let parameters = group
            .parameters()
            .iter()
            .map(|parameter| parameter.declared_type().cloned())
            .collect::<Option<Vec<_>>>()?;
        result = TypeKind::function_with_effects(
            parameters,
            result,
            schema.effects().fixed_row()?.clone(),
        );
    }
    Some(result)
}

pub(super) fn provisional_call_effects(
    candidate: &PreparedResolvedCallable,
    current_group: CallableGroupIndex,
) -> Result<EffectRow, FinalSemanticAnalysisError> {
    let next = CallableGroupIndex::try_from_usize(
        current_group
            .get()
            .checked_add(1)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?,
    )
    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
    if matches!(
        candidate.instantiation(),
        CallableInstantiation::Extension { group, .. } if *group == next
    ) {
        return Ok(candidate
            .schema()
            .effects()
            .fixed_row()
            .cloned()
            .unwrap_or_else(|| EffectRow::closed(EffectSet::new())));
    }
    if candidate.schema().group(next).is_some() {
        return Ok(EffectRow::closed(EffectSet::new()));
    }
    Ok(candidate
        .schema()
        .effects()
        .fixed_row()
        .cloned()
        .unwrap_or_else(|| EffectRow::closed(EffectSet::new())))
}

pub(super) fn provisional_callable_effects(candidate: &PreparedResolvedCallable) -> EffectRow {
    candidate
        .schema()
        .effects()
        .fixed_row()
        .cloned()
        .unwrap_or_else(|| EffectRow::closed(EffectSet::new()))
}

pub(in super::super) fn final_callable_effects(
    candidate: &ResolvedCallable,
    checked: &CheckedCallableCatalog,
) -> Result<EffectRow, FinalSemanticAnalysisError> {
    if let Some(fixed) = candidate.schema().effects().fixed_row() {
        return Ok(fixed.clone());
    }
    let id = candidate
        .checked()
        .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
    checked
        .callable(id)
        .map(|facts| facts.exposed_row().clone())
        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)
}

pub(in super::super) fn final_call_effects(
    candidate: &ResolvedCallable,
    current_group: CallableGroupIndex,
    checked: &CheckedCallableCatalog,
) -> Result<EffectRow, FinalSemanticAnalysisError> {
    if candidate.base().next_group_for(current_group).is_some() {
        return Ok(EffectRow::closed(EffectSet::new()));
    }
    final_callable_effects(candidate, checked)
}

pub(super) fn select_prepared_candidates(
    probes: &[PreparedCandidateOutcome],
) -> CandidateSelection {
    let mut best = None::<usize>;
    let mut tied = Vec::new();
    for (index, probe) in probes.iter().enumerate() {
        let PreparedCandidateOutcome::Accepted { rank, .. } = probe else {
            continue;
        };
        match best {
            None => {
                best = Some(index);
                tied.clear();
                tied.push(index);
            }
            Some(current) => {
                let PreparedCandidateOutcome::Accepted {
                    rank: current_rank, ..
                } = &probes[current]
                else {
                    continue;
                };
                match compare_accepted_candidate_rank(rank, current_rank) {
                    Ordering::Greater => {
                        best = Some(index);
                        tied.clear();
                        tied.push(index);
                    }
                    Ordering::Equal => tied.push(index),
                    Ordering::Less => {}
                }
            }
        }
    }
    match (best, tied.as_slice()) {
        (Some(selected), [_]) => CandidateSelection::Selected(selected),
        (Some(primary), [_, _, ..]) => CandidateSelection::Ambiguous { primary, tied },
        (None, _) => CandidateSelection::Rejected { primary: 0 },
        (Some(selected), []) => CandidateSelection::Selected(selected),
    }
}

fn compare_accepted_candidate_rank(
    left: &AcceptedCandidateRank,
    right: &AcceptedCandidateRank,
) -> Ordering {
    left.exact_matches
        .cmp(&right.exact_matches)
        .then_with(|| right.unchecked_or_open.cmp(&left.unchecked_or_open))
        .then_with(|| right.omitted_parameters.cmp(&left.omitted_parameters))
        .then_with(|| compare_candidate_authority(left.authority, right.authority))
}

const fn compare_candidate_authority(
    left: Option<CallableAuthorityRank>,
    right: Option<CallableAuthorityRank>,
) -> Ordering {
    match (left, right) {
        (Some(CallableAuthorityRank::Standard), Some(CallableAuthorityRank::Adapter)) => {
            Ordering::Greater
        }
        (Some(CallableAuthorityRank::Adapter), Some(CallableAuthorityRank::Standard)) => {
            Ordering::Less
        }
        _ => Ordering::Equal,
    }
}

pub(in super::super) fn checked_project_nominal(
    declaration: &ProjectNominalDeclaration,
    ty: &TypeKind,
) -> Result<CheckedProjectNominal, FinalSemanticAnalysisError> {
    let TypeKind::ProjectNominal(nominal) = ty else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    if nominal.declaration() != declaration.id() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    Ok(CheckedProjectNominal::new(
        declaration.id().clone(),
        declaration.owner(),
        ty.semantic_identity_digest(),
        nominal.arguments().to_vec(),
    ))
}

pub(in super::super) fn nominal_substitutions(
    declaration: &ProjectNominalDeclaration,
    nominal: &ProjectNominalType,
) -> Option<TypeParameterSubstitutions> {
    if nominal.declaration() != declaration.id()
        || nominal.arguments().len() != declaration.type_parameters().len()
    {
        return None;
    }
    let mut substitutions = TypeParameterSubstitutions::default();
    for (parameter, argument) in declaration
        .type_parameters()
        .iter()
        .zip(nominal.arguments())
    {
        let parameter = TypeKind::GenericParam(GenericTypeParameterId::new(
            GenericParameterOwnerId::Nominal(declaration.id().clone()),
            parameter.ordinal(),
        ));
        if !substitutions.observe(&parameter, argument) {
            return None;
        }
    }
    Some(substitutions)
}
