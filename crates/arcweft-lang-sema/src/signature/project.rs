//! Projection from checker-owned focused facts into the public read model.

use std::{cmp::Ordering, sync::Arc};

use arcweft_source::SourceDocument;

use crate::callable::{
    CallPoison, CallTargetFact, CallTargetFacts, CallableDiagnostic, CallableDiagnosticCode,
    CallableDiagnosticSeverity, CallableDiagnosticSubject, CallableInstantiation, CallableLimits,
    CallableLookupKey, CallableParameter, CallableParameterCoordinate, CallableParameterPassing,
    CallableParameterPresence, CallableParameterType, CallableRecord, CheckedCallableCatalog,
    ResolvedCallable, SemanticParameter, SemanticParameterGroup, SemanticSignature,
    SemanticSignatureHelp, SemanticSignatureIndex, SemanticSignatureRecovery, SignatureQueryLimits,
    SignatureQueryWorkMeter, SignatureWorkKind, SignatureWorkReport,
};

use super::{
    FocusedCallSite, SignatureNotApplicable, SignatureQueryControl, SignatureQueryError,
    SignatureQueryOutcome, map_signature_accounting_error,
};

pub(super) struct SignatureProjection<'a> {
    pub(super) document: &'a SourceDocument,
    pub(super) control: SignatureQueryControl<'a>,
    pub(super) site: &'a FocusedCallSite,
    pub(super) facts: &'a CallTargetFacts,
    pub(super) checked: &'a CheckedCallableCatalog,
    pub(super) callable_limits: &'a CallableLimits,
    pub(super) signature_limits: &'a SignatureQueryLimits,
    pub(super) signature_work: &'a mut SignatureQueryWorkMeter,
}

#[allow(
    clippy::result_large_err,
    clippy::too_many_lines,
    reason = "one projection validates and publishes the complete public signature-help result"
)]
pub(super) fn project_signature_help(
    projection: SignatureProjection<'_>,
) -> Result<SignatureQueryOutcome, SignatureQueryError> {
    let SignatureProjection {
        document,
        control,
        site,
        facts,
        checked,
        callable_limits,
        signature_limits,
        signature_work,
    } = projection;
    control.check()?;
    site.callee()
        .validate_for(document)
        .map_err(|_| crate::callable::SemanticSignatureError::SourceIdentityMismatch)?;
    let authored_callee = document
        .text()
        .get(site.callee().range().as_range())
        .ok_or(crate::callable::SemanticSignatureError::InvalidSpan)?;
    let (candidates, selected_candidate) = match facts.target() {
        CallTargetFact::Selected {
            selected,
            considered,
        } => (considered.as_ref(), Some(selected.as_ref())),
        CallTargetFact::Ambiguous { candidates, .. } | CallTargetFact::Rejected { candidates } => {
            (candidates.as_ref(), None)
        }
        CallTargetFact::NonCallable { .. } => {
            return Ok(SignatureQueryOutcome::NotApplicable(
                SignatureNotApplicable::NonCallableCallee,
            ));
        }
        CallTargetFact::Missing { .. } => {
            return Ok(SignatureQueryOutcome::NotApplicable(
                SignatureNotApplicable::UnknownCallee,
            ));
        }
    };

    for _ in facts.arguments() {
        control.check()?;
        signature_work
            .charge(SignatureWorkKind::ArgumentProjections, 1)
            .map_err(map_signature_accounting_error)?;
    }

    let mut signatures = Vec::with_capacity(candidates.len());
    let mut active_signature = SemanticSignatureIndex::try_from_usize(0)?;
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        control.check()?;
        let record = candidate.record().map(AsRef::as_ref);
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
            checked,
            is_selected,
            control,
            callable_limits,
            signature_work,
        )?);
    }

    let active_parameter = active_parameter(site, facts);
    let (diagnostics, omitted_diagnostics) = bounded_diagnostics(
        facts.diagnostics(),
        document.identity(),
        site.call(),
        callable_limits,
        signature_limits,
        signature_work,
    )?;
    let work_report = SignatureWorkReport::from_final_call_facts(
        facts.accounting(),
        site.recovery_nodes(),
        facts.diagnostics().len(),
        callable_limits,
    )?;
    let query_work_report = signature_work.report();
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
        document.identity().clone(),
        site.call().clone(),
        site.arguments().clone(),
        facts.expression(),
        signatures,
        active_signature,
        active_parameter,
        facts.current_group(),
        facts.next_group(),
        recovery,
        diagnostics,
        omitted_diagnostics,
        work_report,
        query_work_report,
        callable_limits,
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
    checked: &CheckedCallableCatalog,
    active: bool,
    control: SignatureQueryControl<'_>,
    callable_limits: &CallableLimits,
    signature_work: &mut SignatureQueryWorkMeter,
) -> Result<SemanticSignature, SignatureQueryError> {
    let schema = candidate.schema();
    let mut groups = Vec::with_capacity(schema.groups().len());
    let mut parameters_in_signature = 0u64;
    for group in schema.groups() {
        control.check()?;
        let mut parameters = Vec::with_capacity(group.parameters().len());
        for parameter in group.parameters() {
            control.check()?;
            signature_work
                .charge_parameter(&mut parameters_in_signature)
                .map_err(map_signature_accounting_error)?;
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
            callable_limits,
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
        candidate_effects(candidate, checked)?
    };
    let canonical_callee = record.map_or_else(|| authored_callee.to_owned(), canonical_callee);
    let documentation = match record {
        Some(record) => {
            if authored_callee == canonical_callee {
                record.documentation().clone()
            } else {
                record
                    .documentation()
                    .with_canonical_owner_note(canonical_callee.as_str())
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
        Arc::from(authored_callee),
        Arc::from(canonical_callee),
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
        callable_limits,
    )?)
}

fn candidate_effects(
    candidate: &ResolvedCallable,
    checked: &CheckedCallableCatalog,
) -> Result<crate::effect_row::EffectRow, SignatureQueryError> {
    if let Some(id) = candidate.checked() {
        return checked
            .callable(id)
            .map(|facts| facts.exposed_row().clone())
            .map_err(|_| {
                crate::signature::SignatureSemanticUnavailable::MissingCallableAuthority {
                    candidate: Box::new(candidate.id().clone()),
                }
                .into()
            });
    }
    candidate
        .schema()
        .effects()
        .fixed_row()
        .cloned()
        .ok_or_else(|| {
            crate::signature::SignatureSemanticUnavailable::MissingCallableAuthority {
                candidate: Box::new(candidate.id().clone()),
            }
            .into()
        })
}

fn active_parameter(
    site: &FocusedCallSite,
    facts: &CallTargetFacts,
) -> Option<CallableParameterCoordinate> {
    let active_argument = site.active_argument()?;
    if let Some(argument) = facts.arguments().get(active_argument) {
        let mut mapped = argument
            .slots()
            .iter()
            .filter_map(crate::callable::CheckedCallArgumentSlotFact::mapped);
        let first = mapped.next();
        return (first.is_some() && mapped.all(|candidate| Some(candidate) == first))
            .then_some(first)
            .flatten();
    }

    let candidate = match facts.target() {
        CallTargetFact::Selected { selected, .. } => selected.as_ref(),
        CallTargetFact::Ambiguous { candidates, .. } | CallTargetFact::Rejected { candidates } => {
            candidates.first()?
        }
        CallTargetFact::NonCallable { .. } | CallTargetFact::Missing { .. } => return None,
    };
    let group = candidate.schema().group(facts.current_group())?;
    let mut provided = vec![false; group.parameters().len()];
    if let CallableInstantiation::DataLast {
        group: implicit_group,
        parameter,
        ..
    } = candidate.instantiation()
        && *implicit_group == facts.current_group()
        && let Some(provided) = provided.get_mut(parameter.get())
    {
        *provided = true;
    }
    for coordinate in facts
        .arguments()
        .iter()
        .flat_map(crate::callable::CheckedCallArgumentFact::slots)
        .filter_map(crate::callable::CheckedCallArgumentSlotFact::mapped)
        .filter(|coordinate| coordinate.group() == facts.current_group())
    {
        let Some(parameter) = group.parameters().get(coordinate.parameter().get()) else {
            continue;
        };
        if !matches!(
            parameter.passing(),
            CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed
        ) {
            provided[parameter.index().get()] = true;
        }
    }
    group
        .parameters()
        .iter()
        .find(|parameter| {
            !provided[parameter.index().get()]
                && matches!(
                    parameter.passing(),
                    CallableParameterPassing::PositionalOrNamed
                        | CallableParameterPassing::PositionalOnly
                        | CallableParameterPassing::RestPositional
                )
        })
        .map(|parameter| CallableParameterCoordinate::new(facts.current_group(), parameter.index()))
}

fn canonical_callee(record: &CallableRecord) -> String {
    match record.key() {
        CallableLookupKey::Free(path) => path.dotted_name(),
        CallableLookupKey::Method(key) => {
            format!(
                "{}.{}",
                key.receiver().source_label(),
                key.method().as_str()
            )
        }
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

fn bounded_diagnostics(
    diagnostics: &[CallableDiagnostic],
    document: &arcweft_source::SourceDocumentIdentity,
    call: &arcweft_source::SourceSpan,
    callable_limits: &CallableLimits,
    signature_limits: &SignatureQueryLimits,
    signature_work: &mut SignatureQueryWorkMeter,
) -> Result<(Vec<CallableDiagnostic>, u64), SignatureQueryError> {
    for _ in diagnostics {
        signature_work
            .charge(SignatureWorkKind::DiagnosticConsiderations, 1)
            .map_err(map_signature_accounting_error)?;
    }
    let observed =
        u64::try_from(diagnostics.len()).map_err(|_| SignatureQueryError::ArithmeticOverflow {
            counter: SignatureWorkKind::DiagnosticConsiderations,
        })?;
    let mut diagnostics = diagnostics.to_vec();
    diagnostics.sort_by(compare_diagnostics);
    if observed <= signature_limits.diagnostics() {
        return Ok((diagnostics, 0));
    }
    let retained = signature_limits.diagnostics().checked_sub(1).ok_or(
        SignatureQueryError::ArithmeticOverflow {
            counter: SignatureWorkKind::DiagnosticConsiderations,
        },
    )?;
    let omitted =
        observed
            .checked_sub(retained)
            .ok_or(SignatureQueryError::ArithmeticOverflow {
                counter: SignatureWorkKind::DiagnosticConsiderations,
            })?;
    let retained =
        usize::try_from(retained).map_err(|_| SignatureQueryError::ArithmeticOverflow {
            counter: SignatureWorkKind::DiagnosticConsiderations,
        })?;
    diagnostics.truncate(retained);
    diagnostics.push(CallableDiagnostic::try_new(
        CallableDiagnosticCode::DiagnosticsTruncated,
        CallableDiagnosticSeverity::Information,
        Some(call.clone()),
        CallableDiagnosticSubject::None,
        Vec::new(),
        Some(document),
        callable_limits,
    )?);
    Ok((diagnostics, omitted))
}

fn compare_diagnostics(left: &CallableDiagnostic, right: &CallableDiagnostic) -> Ordering {
    left.code()
        .cmp(&right.code())
        .then_with(|| severity_rank(left.severity()).cmp(&severity_rank(right.severity())))
        .then_with(|| left.span().cmp(&right.span()))
}

const fn severity_rank(severity: CallableDiagnosticSeverity) -> u8 {
    match severity {
        CallableDiagnosticSeverity::Error => 0,
        CallableDiagnosticSeverity::Warning => 1,
        CallableDiagnosticSeverity::Information => 2,
    }
}

#[cfg(test)]
mod tests;
