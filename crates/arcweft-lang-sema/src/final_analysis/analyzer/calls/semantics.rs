//! Candidate selection and final callable semantic projections.

use super::super::{
    AcceptedCandidateRank, Analyzer, CallableAuthorityRank, CallableGroupIndex, CandidateSelection,
    CheckedCallArgumentSlotSource, CheckedCallableCatalog, CheckedProjectNominal, EffectRow,
    EffectSet, FinalSemanticAnalysisError, GenericParameterOwnerId, GenericTypeParameterId,
    HirCallArgument, HirCallValue, MappedCallArgumentSlot, Ordering,
    PhysicalArgumentEvaluationKind, PreparedResolvedCallable, ProjectNominalDeclaration,
    ProjectNominalType, SpreadArgumentPolicy, TypeKind, TypeParameterSubstitutions,
};
use super::PreparedCandidateOutcome;
use crate::callable::ResolvedCallable;
use std::borrow::Cow;

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

impl Analyzer<'_, '_, '_> {
    pub(super) fn checked_callable_effect_authority(
        &self,
    ) -> Result<crate::callable::CheckedCallResolverAuthority<'_>, FinalSemanticAnalysisError> {
        let staged = self
            .staged_callables
            .as_ref()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let graph = self
            .facts
            .prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?;
        Ok(crate::callable::CheckedCallResolverAuthority::preparing(
            &staged.builder,
            graph.effect_rows().view(),
        ))
    }

    pub(super) fn checked_callable_effect_authority_with_projection<'a>(
        &'a self,
        projection: &'a super::super::CandidateSemanticProjection,
    ) -> Result<crate::callable::CheckedCallResolverAuthority<'a>, FinalSemanticAnalysisError> {
        let staged = self
            .staged_callables
            .as_ref()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let graph = self
            .facts
            .prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?;
        let effects = projection
            .effect_view(graph)
            .map_err(FinalSemanticAnalysisError::from)?;
        Ok(crate::callable::CheckedCallResolverAuthority::preparing(
            &staged.builder,
            effects,
        ))
    }

    pub(in crate::final_analysis::analyzer) fn source_callable_terminal_effects<'a>(
        &'a self,
        candidate: &'a PreparedResolvedCallable,
    ) -> Result<crate::callable::CallableTerminalEffectProjection<'a>, FinalSemanticAnalysisError>
    {
        self.checked_callable_effect_authority()?
            .terminal_effects_for(candidate)
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)
    }

    /// The contribution available before body inference. An inferred project
    /// callee is already retained as a typed edge in the prepared call graph;
    /// its body row is added by callable closure and final call sealing.
    /// An unknown fixed/detached row has no such deferred authority.
    pub(super) fn source_call_intrinsic_effects(
        &self,
        candidate: &PreparedResolvedCallable,
        current_group: CallableGroupIndex,
    ) -> Result<EffectSet, FinalSemanticAnalysisError> {
        if candidate.next_group_for(current_group).is_some() {
            return Ok(EffectSet::new());
        }
        let terminal_effects = self.source_callable_terminal_effects(candidate)?;
        if let crate::callable::CallableTerminalEffectProjection::Known { effects: row, .. } =
            terminal_effects
            && row.is_known()
        {
            return row
                .constant_effects()
                .map_err(|_| FinalSemanticAnalysisError::OpenEffectRow);
        }
        let crate::callable::CallableEffectSchema::Project { declaration } =
            candidate.schema().effects()
        else {
            return Err(FinalSemanticAnalysisError::OpenEffectRow);
        };
        let owner = candidate
            .checked()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        if let crate::callable::CallableTerminalEffectProjection::Pending(checked) =
            terminal_effects
            && checked != owner
        {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
        let staged = self
            .staged_callables
            .as_ref()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let callable = staged
            .builder
            .pending_by_id(owner)
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        if !matches!(owner.declaration(), crate::callable::CheckedCallableDeclaration::Project(actual) if actual == declaration)
            || !staged.bodies.iter().any(|body| &body.id == owner)
            || !matches!(
                callable
                    .body_contract()
                    .map(crate::callable::CallableEffectContract::permission),
                Some(crate::callable::EffectPermission::UnboundedInference)
            )
        {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
        Ok(EffectSet::new())
    }

    pub(super) fn source_callable_effects(
        &self,
        candidate: &PreparedResolvedCallable,
        projection: Option<&super::super::CandidateSemanticProjection>,
    ) -> Result<Option<EffectRow>, FinalSemanticAnalysisError> {
        let authority = match projection {
            Some(projection) => {
                self.checked_callable_effect_authority_with_projection(projection)?
            }
            None => self.checked_callable_effect_authority()?,
        };
        match authority
            .terminal_effects_for(candidate)
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?
        {
            crate::callable::CallableTerminalEffectProjection::Known { effects: row, .. } => {
                Ok(Some(row.clone()))
            }
            crate::callable::CallableTerminalEffectProjection::Pending(checked)
                if candidate.checked() == Some(checked) =>
            {
                Ok(None)
            }
            crate::callable::CallableTerminalEffectProjection::Pending(_) => {
                Err(FinalSemanticAnalysisError::CheckedCallableCatalog)
            }
        }
    }

    pub(in crate::final_analysis::analyzer) fn source_result_schema_for_group(
        &self,
        owner: arcweft_lang_hir::identity::ExprId,
        candidate: &PreparedResolvedCallable,
        group: CallableGroupIndex,
    ) -> Result<crate::callable::CallableResultSchema, FinalSemanticAnalysisError> {
        let terminal_effects = self
            .checked_callable_effect_authority()?
            .terminal_effects_for(candidate)
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        match candidate
            .result_schema_for_group(group, terminal_effects)
            .map_err(|_| FinalSemanticAnalysisError::CallResolutionFailed { owner })?
        {
            crate::callable::CallableProjection::Ready(schema) => Ok(schema),
            crate::callable::CallableProjection::Pending(pending) => Err(
                FinalSemanticAnalysisError::CallableEffectProjectionPending {
                    checked: Box::new(pending.checked().clone()),
                    group: pending.group(),
                },
            ),
        }
    }
}

pub(in super::super) fn final_callable_effects(
    candidate: &ResolvedCallable,
    checked: &CheckedCallableCatalog,
) -> Result<EffectRow, FinalSemanticAnalysisError> {
    final_callable_effect_row(candidate, checked).map(Cow::into_owned)
}

pub(in super::super) fn final_callable_effect_row<'a>(
    candidate: &ResolvedCallable,
    checked: &'a CheckedCallableCatalog,
) -> Result<Cow<'a, EffectRow>, FinalSemanticAnalysisError> {
    if let Some(id) = candidate.checked() {
        return checked
            .callable(id)
            .map(|facts| Cow::Borrowed(facts.exposed_row()))
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog);
    }
    candidate
        .schema()
        .effects()
        .fixed_row()
        .cloned()
        .map(Cow::Owned)
        .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)
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
        .then_with(|| {
            left.declared_exact_matches
                .cmp(&right.declared_exact_matches)
        })
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
        ty.semantic_identity_digest()?,
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
        let parameter = TypeKind::generic_parameter(GenericTypeParameterId::new(
            GenericParameterOwnerId::Nominal(declaration.id().clone()),
            parameter.ordinal(),
        ));
        if !substitutions.observe(&parameter, argument) {
            return None;
        }
    }
    Some(substitutions)
}
