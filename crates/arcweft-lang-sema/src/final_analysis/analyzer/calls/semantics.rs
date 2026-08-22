//! Candidate selection and final callable semantic projections.

use super::super::{
    CallableAuthorityRank, CallableGroupIndex, CallableInstantiation, CallableParameterType,
    CandidateExpectedType, CandidateProbe, CandidateScore, CandidateSelection,
    CheckedCallArgumentSlotSource, CheckedCallableCatalog, CheckedProjectNominal, EffectRow,
    EffectSet, FinalSemanticAnalysisError, GenericTypeOwnerId, GenericTypeParameterId,
    HirCallArgument, HirCallValue, MappedCallArgumentSlot, Ordering,
    PhysicalArgumentEvaluationKind, ProjectNominalDeclaration, ProjectNominalType,
    ResolvedCallable, SpreadArgumentPolicy, TypeKind, TypeParameterSubstitutions,
};

pub(super) fn physical_expected_type(
    expected: Option<&TypeKind>,
    has_coordinate: bool,
    shape_rejected: bool,
) -> CandidateExpectedType {
    if shape_rejected || !has_coordinate {
        CandidateExpectedType::Unmapped
    } else if let Some(expected) = expected {
        CandidateExpectedType::Exact(expected.clone())
    } else {
        CandidateExpectedType::Unchecked
    }
}

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

pub(in super::super) fn callable_schema_type(
    schema: &crate::callable::CallableSignatureSchema,
) -> Option<TypeKind> {
    let mut result = schema.result().clone();
    for group in schema.groups().iter().rev() {
        let parameters = group
            .parameters()
            .iter()
            .map(|parameter| match parameter.ty() {
                CallableParameterType::Exact(ty) => Some(ty.clone()),
                CallableParameterType::Unchecked => None,
            })
            .collect::<Option<Vec<_>>>()?;
        result = TypeKind::function_with_effects(
            parameters,
            result,
            schema.effects().fixed_row()?.clone(),
        );
    }
    Some(result)
}

pub(in super::super) fn callable_schema_type_with_effects(
    schema: &crate::callable::CallableSignatureSchema,
    effects: &EffectRow,
) -> Option<TypeKind> {
    let mut result = schema.result().clone();
    for group in schema.groups().iter().rev() {
        let parameters = group
            .parameters()
            .iter()
            .map(|parameter| match parameter.ty() {
                CallableParameterType::Exact(ty) => Some(ty.clone()),
                CallableParameterType::Unchecked => None,
            })
            .collect::<Option<Vec<_>>>()?;
        result = TypeKind::function_with_effects(parameters, result, effects.clone());
    }
    Some(result)
}

pub(super) fn provisional_call_effects(
    candidate: &ResolvedCallable,
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

pub(super) fn provisional_callable_effects(candidate: &ResolvedCallable) -> EffectRow {
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
    let provisional = provisional_call_effects(candidate, current_group)?;
    let next = CallableGroupIndex::try_from_usize(
        current_group
            .get()
            .checked_add(1)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?,
    )
    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
    if candidate.schema().group(next).is_some()
        || candidate.schema().effects().fixed_row().is_some()
    {
        return Ok(provisional);
    }
    let id = candidate
        .checked()
        .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
    checked
        .callable(id)
        .map(|facts| facts.exposed_row().clone())
        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)
}

pub(super) fn select_candidate_probes(probes: &[CandidateProbe]) -> CandidateSelection {
    let mut best = None::<usize>;
    let mut tied = Vec::new();
    for (index, probe) in probes.iter().enumerate() {
        if probe.score.hard_errors != 0 {
            continue;
        }
        match best {
            None => {
                best = Some(index);
                tied.clear();
                tied.push(index);
            }
            Some(current) => match compare_candidate_score(&probe.score, &probes[current].score) {
                Ordering::Greater => {
                    best = Some(index);
                    tied.clear();
                    tied.push(index);
                }
                Ordering::Equal => tied.push(index),
                Ordering::Less => {}
            },
        }
    }
    match (best, tied.as_slice()) {
        (Some(selected), [_]) => CandidateSelection::Selected(selected),
        (Some(primary), [_, _, ..]) => CandidateSelection::Ambiguous { primary, tied },
        (None, _) => CandidateSelection::Rejected { primary: 0 },
        (Some(_), []) => unreachable!("a selected candidate is inserted into the tie set"),
    }
}

fn compare_candidate_score(left: &CandidateScore, right: &CandidateScore) -> Ordering {
    right
        .hard_errors
        .cmp(&left.hard_errors)
        .then_with(|| left.exact_matches.cmp(&right.exact_matches))
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
            GenericTypeOwnerId::Nominal(declaration.id().clone()),
            parameter.ordinal(),
        ));
        if !substitutions.observe(&parameter, argument) {
            return None;
        }
    }
    Some(substitutions)
}
