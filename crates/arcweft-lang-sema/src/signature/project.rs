//! Projection from checker-owned focused facts into the public read model.

use std::{collections::HashSet, sync::Arc};

use arcweft_source::SourceDocument;

use crate::{
    callable::{
        CallPoison, CallTargetFact, CallTargetFacts, CallableCandidateId, CallableLimits,
        CallableLookupKey, CallableParameter, CallableParameterCoordinate,
        CallableParameterPassing, CallableParameterPresence, CallableParameterType, CallableRecord,
        CheckedCallArgumentFact, CheckedCallArgumentSlotFact, RegisteredCallableCatalog,
        ResolvedCallable, ResolverWork, SemanticParameter, SemanticParameterGroup,
        SemanticSignature, SemanticSignatureHelp, SemanticSignatureIndex,
        SemanticSignatureRecovery, SignatureWorkReport,
    },
    checker::FocusedCallSite,
    registration::RegisteredSemanticWorld,
};

use super::{
    SignatureNotApplicable, SignatureQueryControl, SignatureQueryError, SignatureQueryOutcome,
};

#[allow(
    clippy::result_large_err,
    clippy::too_many_lines,
    reason = "one projection validates and publishes the complete public signature-help result"
)]
pub(super) fn project_signature_help(
    world: &RegisteredSemanticWorld,
    document: &SourceDocument,
    control: SignatureQueryControl<'_>,
    site: &FocusedCallSite,
    facts: &CallTargetFacts,
    work: &mut ResolverWork,
    limits: &CallableLimits,
) -> Result<SignatureQueryOutcome, SignatureQueryError> {
    control.check()?;
    site.callee()
        .validate_for(document)
        .map_err(|_| crate::callable::SemanticSignatureError::SourceIdentityMismatch)?;
    let authored_callee = document
        .text()
        .get(site.callee().range().as_range())
        .ok_or(crate::callable::SemanticSignatureError::InvalidSpan)?;
    let initial_resolver_work = work.consumed();
    let arguments = facts
        .arguments()
        .iter()
        .flat_map(CheckedCallArgumentFact::slots)
        .count();
    let argument_mapping = u64::try_from(arguments)
        .map_err(|_| crate::callable::CallableQueryLimitError::ArithmeticOverflow)?;
    let type_checks = argument_mapping;
    work.charge(
        argument_mapping
            .checked_add(type_checks)
            .ok_or(crate::callable::CallableQueryLimitError::ArithmeticOverflow)?,
    )?;

    let (candidates, selected_candidate) = match facts.target() {
        CallTargetFact::Selected {
            selected,
            considered,
        } => (considered.as_ref(), Some(selected.as_ref())),
        CallTargetFact::Ambiguous { candidates } => (candidates.as_ref(), None),
        CallTargetFact::NonCallable { source, ty } => {
            return Ok(SignatureQueryOutcome::NotApplicable(
                SignatureNotApplicable::NonCallableCallee {
                    source: source.clone(),
                    ty: ty.clone(),
                },
            ));
        }
    };

    let mut signatures = Vec::with_capacity(candidates.len());
    let mut active_signature = SemanticSignatureIndex::try_from_usize(0)?;
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        control.check()?;
        let record = accepted_record(world.environment().callable_catalog(), candidate.id());
        let is_selected =
            selected_candidate.is_some_and(|selected| selected.id() == candidate.id());
        if is_selected {
            active_signature = SemanticSignatureIndex::try_from_usize(candidate_index)?;
        }
        signatures.push(project_signature(
            candidate,
            record,
            authored_callee,
            facts,
            is_selected,
            control,
            work,
            limits,
        )?);
    }

    let active_parameter = candidates
        .get(active_signature.get())
        .and_then(|candidate| active_parameter(facts, site.active_argument(), candidate));
    let resolver = initial_resolver_work
        .checked_add(
            work.consumed()
                .checked_sub(initial_resolver_work)
                .and_then(|projection_and_arguments| {
                    projection_and_arguments.checked_sub(argument_mapping.checked_add(type_checks)?)
                })
                .ok_or(crate::callable::CallableQueryLimitError::ArithmeticOverflow)?,
        )
        .ok_or(crate::callable::CallableQueryLimitError::ArithmeticOverflow)?;
    let work_report = SignatureWorkReport::try_new(
        resolver,
        argument_mapping,
        type_checks,
        site.recovery_nodes(),
        facts.diagnostics().len(),
        limits,
    )?;
    control.check()?;
    let recovery = if site.recovery_nodes() == 0 {
        SemanticSignatureRecovery::Complete
    } else {
        SemanticSignatureRecovery::Recovered {
            missing_close_delimiter: site.missing_close_delimiter(),
            nodes: site.recovery_nodes(),
        }
    };
    Ok(SignatureQueryOutcome::Help(SemanticSignatureHelp::try_new(
        facts.document().clone(),
        site.call().clone(),
        site.arguments().clone(),
        facts.expression(),
        signatures,
        active_signature,
        active_parameter,
        facts.current_group(),
        facts.next_group(),
        recovery,
        facts.diagnostics().to_vec(),
        work_report,
        limits,
    )?))
}

#[allow(
    clippy::result_large_err,
    clippy::too_many_arguments,
    reason = "the projection requires the selected candidate, accepted record, checked facts, and query authorities"
)]
fn project_signature(
    candidate: &ResolvedCallable,
    record: Option<&CallableRecord>,
    authored_callee: &str,
    facts: &CallTargetFacts,
    active: bool,
    control: SignatureQueryControl<'_>,
    work: &mut ResolverWork,
    limits: &CallableLimits,
) -> Result<SemanticSignature, SignatureQueryError> {
    work.charge(1)?;
    let schema = candidate.schema();
    let mut groups = Vec::with_capacity(schema.groups().len());
    for group in schema.groups() {
        control.check()?;
        work.charge(1)?;
        let mut parameters = Vec::with_capacity(group.parameters().len());
        for parameter in group.parameters() {
            control.check()?;
            work.charge(1)?;
            let coordinate = CallableParameterCoordinate::new(group.index(), parameter.index());
            parameters.push(SemanticParameter::try_new(
                coordinate,
                parameter_label(parameter),
                parameter.name().cloned(),
                parameter.ty().clone(),
                parameter.passing(),
                parameter.presence(),
                parameter.documentation().map(Arc::<str>::from).or_else(|| {
                    record.and_then(|record| {
                        record
                            .documentation()
                            .parameter(group.index(), parameter.index())
                            .map(Arc::<str>::from)
                    })
                }),
                parameter.source().cloned().or_else(|| {
                    record
                        .and_then(CallableRecord::source)
                        .and_then(|source| source.parameter(group.index(), parameter.index()))
                        .cloned()
                }),
            )?);
        }
        groups.push(SemanticParameterGroup::try_new(
            group.index(),
            group.kind(),
            parameters,
            limits,
        )?);
    }
    let result = if active {
        facts
            .result()
            .cloned()
            .unwrap_or_else(|| schema.result().clone())
    } else {
        schema.result().clone()
    };
    let effects = if active {
        facts.effects().clone()
    } else {
        schema.effects().declared().clone()
    };
    let documentation = match record {
        Some(record) => {
            let canonical_callee = canonical_callee(record);
            if authored_callee == canonical_callee {
                record.documentation().clone()
            } else {
                record
                    .documentation()
                    .with_canonical_owner_note(&canonical_callee)
            }
        }
        None => crate::callable::CallableDocumentation::missing(),
    };
    Ok(SemanticSignature::try_new(
        candidate.id().clone(),
        candidate
            .equivalent_sources()
            .iter()
            .map(|source| source.id().clone())
            .collect(),
        candidate.origin().clone(),
        Arc::from(signature_label(authored_callee, schema, &result)),
        groups,
        result,
        effects,
        documentation,
        record.and_then(CallableRecord::source).cloned(),
        facts.current_group(),
        if active {
            facts.poison()
        } else {
            CallPoison::Clean
        },
        limits,
    )?)
}

fn accepted_record<'a>(
    catalog: &'a RegisteredCallableCatalog,
    candidate: &CallableCandidateId,
) -> Option<&'a CallableRecord> {
    match candidate {
        CallableCandidateId::Project(id) => catalog.project_record(id).map(AsRef::as_ref),
        CallableCandidateId::Environment(id) => catalog.environment_record(id).map(AsRef::as_ref),
        CallableCandidateId::Curried(id) => accepted_record(catalog, id.base()),
        _ => None,
    }
}

fn signature_label(
    authored_callee: &str,
    schema: &crate::callable::CallableSignatureSchema,
    result: &crate::types::TypeKind,
) -> String {
    let mut groups = String::new();
    for group in schema.groups() {
        let parameters = group
            .parameters()
            .iter()
            .map(parameter_label)
            .collect::<Vec<_>>()
            .join(", ");
        groups.push('(');
        groups.push_str(&parameters);
        groups.push(')');
    }
    format!("{authored_callee}{groups} -> {}", result.source_label())
}

fn canonical_callee(record: &CallableRecord) -> String {
    match record.key() {
        CallableLookupKey::Free(path) => path.dotted_name(),
        CallableLookupKey::Method(key) => key.method().as_str().to_owned(),
    }
}

fn parameter_label(parameter: &CallableParameter) -> String {
    let ty = match parameter.ty() {
        CallableParameterType::Exact(ty) => ty.source_label(),
        CallableParameterType::Unchecked => "_".to_owned(),
    };
    let name = parameter.name().map(crate::callable::CallableName::as_str);
    let mut label = match (parameter.passing(), name) {
        (
            CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed,
            Some(name),
        ) => format!("...{name}: {ty}"),
        (CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed, None) => {
            format!("...{ty}")
        }
        (_, Some(name)) => format!("{name}: {ty}"),
        (_, None) => ty,
    };
    match parameter.presence() {
        CallableParameterPresence::Required => {}
        CallableParameterPresence::Optional => label.push('?'),
        CallableParameterPresence::Defaulted => label.push_str(" = _"),
    }
    label
}

fn active_parameter(
    facts: &CallTargetFacts,
    active_argument: Option<usize>,
    selected: &ResolvedCallable,
) -> Option<CallableParameterCoordinate> {
    let active_argument = active_argument?;
    let current_group = facts.current_group();
    let group = selected.schema().group(current_group)?;
    if let Some(argument) = facts.arguments().get(active_argument) {
        let mapped = argument
            .slots()
            .iter()
            .filter_map(CheckedCallArgumentSlotFact::mapped)
            .collect::<HashSet<_>>();
        if mapped.len() == 1 {
            return mapped.into_iter().next();
        }
        if let Some(name) = argument.authored_name() {
            return group.parameters().iter().find_map(|parameter| {
                (parameter.name() == Some(name))
                    .then(|| CallableParameterCoordinate::new(current_group, parameter.index()))
            });
        }
        if argument.spread() {
            return None;
        }
        let positional_ordinal = facts
            .arguments()
            .iter()
            .take(active_argument + 1)
            .filter(|argument| !argument.spread() && argument.authored_name().is_none())
            .count()
            .saturating_sub(1);
        return positional_parameter(group.parameters(), current_group, positional_ordinal);
    }

    let mapped = facts
        .arguments()
        .iter()
        .flat_map(CheckedCallArgumentFact::slots)
        .filter_map(CheckedCallArgumentSlotFact::mapped)
        .collect::<HashSet<_>>();
    group.parameters().iter().find_map(|parameter| {
        let coordinate = CallableParameterCoordinate::new(current_group, parameter.index());
        (!mapped.contains(&coordinate) && accepts_positional(parameter.passing()))
            .then_some(coordinate)
    })
}

fn positional_parameter(
    parameters: &[CallableParameter],
    group: crate::callable::CallableGroupIndex,
    mut ordinal: usize,
) -> Option<CallableParameterCoordinate> {
    for parameter in parameters {
        if !accepts_positional(parameter.passing()) {
            continue;
        }
        if parameter.passing() == CallableParameterPassing::RestPositional || ordinal == 0 {
            return Some(CallableParameterCoordinate::new(group, parameter.index()));
        }
        ordinal -= 1;
    }
    None
}

const fn accepts_positional(passing: CallableParameterPassing) -> bool {
    matches!(
        passing,
        CallableParameterPassing::PositionalOnly
            | CallableParameterPassing::PositionalOrNamed
            | CallableParameterPassing::RestPositional
    )
}
