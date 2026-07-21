//! Projection from checker-owned focused facts into the public read model.

use std::{cmp::Ordering, sync::Arc};

use arcweft_source::SourceDocument;

use crate::{
    callable::{
        CallPoison, CallTargetFact, CallTargetFacts, CallableCandidateId, CallableDiagnostic,
        CallableDiagnosticCode, CallableDiagnosticSeverity, CallableDiagnosticSubject,
        CallableLimits, CallableLookupKey, CallableParameter, CallableParameterCoordinate,
        CallableParameterPassing, CallableParameterPresence, CallableParameterType, CallableRecord,
        RegisteredCallableCatalog, ResolvedCallable, ResolverWork, SemanticParameter,
        SemanticParameterGroup, SemanticSignature, SemanticSignatureHelp, SemanticSignatureIndex,
        SemanticSignatureRecovery, SignatureQueryLimits, SignatureQueryWorkMeter,
        SignatureWorkKind,
    },
    checker::FocusedCallSite,
    registration::RegisteredSemanticWorld,
};

use super::{
    SignatureNotApplicable, SignatureQueryControl, SignatureQueryError, SignatureQueryOutcome,
    map_signature_accounting_error,
};

pub(super) struct SignatureProjection<'a> {
    pub(super) world: &'a RegisteredSemanticWorld,
    pub(super) document: &'a SourceDocument,
    pub(super) control: SignatureQueryControl<'a>,
    pub(super) site: &'a FocusedCallSite,
    pub(super) facts: &'a CallTargetFacts,
    pub(super) callable_limits: &'a CallableLimits,
    pub(super) signature_limits: &'a SignatureQueryLimits,
    pub(super) signature_work: &'a mut SignatureQueryWorkMeter,
    pub(super) work: &'a mut ResolverWork,
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
        world,
        document,
        control,
        site,
        facts,
        callable_limits,
        signature_limits,
        signature_work,
        work,
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
        CallTargetFact::Ambiguous { candidates } | CallTargetFact::Rejected { candidates } => {
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
            callable_limits,
            signature_work,
            work,
        )?);
    }

    let active_parameter = facts.active_parameter();
    let (diagnostics, omitted_diagnostics) = bounded_diagnostics(
        facts.diagnostics(),
        facts.document(),
        site.call(),
        callable_limits,
        signature_limits,
        signature_work,
    )?;
    let work_report = work
        .signature_report(
            site.recovery_nodes(),
            facts.diagnostics().len(),
            callable_limits,
        )
        .map_err(SignatureQueryError::from)?;
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
    active: bool,
    control: SignatureQueryControl<'_>,
    callable_limits: &CallableLimits,
    signature_work: &mut SignatureQueryWorkMeter,
    work: &mut ResolverWork,
) -> Result<SemanticSignature, SignatureQueryError> {
    let schema = candidate.schema();
    let mut groups = Vec::with_capacity(schema.groups().len());
    let mut parameters_in_signature = 0u64;
    for group in schema.groups() {
        control.check()?;
        let mut parameters = Vec::with_capacity(group.parameters().len());
        for parameter in group.parameters() {
            control.check()?;
            work.charge(1).map_err(SignatureQueryError::from)?;
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
        schema.effects().declared().clone()
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
