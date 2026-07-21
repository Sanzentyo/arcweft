//! Transactional overload probing, scoring, and selected-candidate replay.

use std::{cmp::Ordering, collections::HashSet};

use crate::{
    callable::{
        CallPoison, CallableAuthorityRank, CallableCandidateId, CallableDiagnosticCode,
        CallableDiagnosticSubject, CallableGroupIndex, CallableParameterPassing,
        CallableParameterPresence, CallableValidator, CheckedCallArgumentFact, CheckedCallTarget,
        NonEmptyResolvedCandidates, ResolvedCallable, SignatureQueryStep, SignatureWorkKind,
    },
    checker::{
        CallableDiagnosticDraft, ProjectCallableReference, TypeCheckError, TypeChecker,
        call_target_facts::CallableWorkOperation,
    },
    effect_model::EffectSite,
    types::TypeKind,
};

use super::{
    RegisteredArgumentCheck, RegisteredCallSite, RegisteredCandidateCheck,
    RegisteredSumConstructor, schema_result_type,
};

struct RegisteredCandidateProbe<'a> {
    candidate: &'a ResolvedCallable,
    specificity: RegisteredCandidateSpecificity,
    arguments: RegisteredArgumentCheck,
    focused_facts: crate::checker::call_target_facts::CallTargetFactRecorder,
}

struct RegisteredCandidateSelection<'a> {
    selected: &'a RegisteredCandidateProbe<'a>,
    tied: Vec<&'a RegisteredCandidateProbe<'a>>,
}

struct RejectedRegisteredCandidate {
    arguments: RegisteredArgumentCheck,
    result: TypeKind,
    retained_specific_error: bool,
}

#[derive(Clone, Copy)]
enum UnselectedRegisteredReason {
    Rejected,
    Ambiguous,
}

struct UnselectedRegisteredCandidates<'a> {
    candidates: &'a [ResolvedCallable],
    arguments: Vec<CheckedCallArgumentFact>,
    argument_diagnostics: Vec<CallableDiagnosticDraft>,
    fallback_result: TypeKind,
    reason: UnselectedRegisteredReason,
    message: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RegisteredCandidateSpecificity {
    shape_viable: bool,
    poison: CallPoison,
    hard_errors: usize,
    exact_matches: usize,
    compatible_matches: usize,
    unchecked_or_open: usize,
    omissions: usize,
    rest_bindings: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegisteredSlotSpecificity {
    Exact,
    Compatible,
    UncheckedOrOpen,
}

impl TypeChecker<'_> {
    pub(super) fn check_registered_candidates(
        &mut self,
        site: &RegisteredCallSite<'_>,
        candidates: &NonEmptyResolvedCandidates,
    ) -> TypeKind {
        let focused_work = self.uses_focused_callable_work(site.call_span.as_ref());
        if !self.charge_registered_candidate_materialization(site, candidates, focused_work) {
            return TypeKind::Named("_".to_owned());
        }
        let Some(probes) = self.probe_registered_candidates(site, candidates, focused_work) else {
            return TypeKind::Named("_".to_owned());
        };
        let Ok(selection) = self.select_registered_candidate_probe(site, &probes) else {
            return TypeKind::Named("_".to_owned());
        };
        let Some(selection) = selection else {
            return self.finish_unselected_registered_candidates(
                site,
                UnselectedRegisteredCandidates {
                    candidates: candidates.as_slice(),
                    arguments: Vec::new(),
                    argument_diagnostics: Vec::new(),
                    fallback_result: TypeKind::Named("_".to_owned()),
                    reason: UnselectedRegisteredReason::Rejected,
                    message: Some(format!("call `{}` has no viable signature", site.label)),
                },
            );
        };
        if !selection.selected.specificity.is_viable() {
            return self
                .finish_rejected_registered_candidates(site, candidates, &probes, &selection);
        }
        if selection.tied.len() == 1 {
            return self.replay_registered_candidate(
                site,
                selection.selected.candidate,
                candidates.as_slice(),
                false,
                focused_work,
            );
        }
        let ambiguous = selection
            .tied
            .iter()
            .map(|probe| (*probe.candidate).clone())
            .collect::<Vec<_>>();
        self.call_target_fact_recorder
            .restore_selected_nested_facts_from(&selection.selected.focused_facts);
        self.finish_unselected_registered_candidates(
            site,
            UnselectedRegisteredCandidates {
                candidates: &ambiguous,
                arguments: selection.selected.arguments.facts.clone(),
                argument_diagnostics: selection.selected.arguments.diagnostics.clone(),
                fallback_result: TypeKind::Named("_".to_owned()),
                reason: UnselectedRegisteredReason::Ambiguous,
                message: Some(format!(
                    "call `{}` is ambiguous between equally specific signatures",
                    site.label
                )),
            },
        )
    }

    fn charge_registered_candidate_materialization(
        &mut self,
        site: &RegisteredCallSite<'_>,
        candidates: &NonEmptyResolvedCandidates,
        focused_work: bool,
    ) -> bool {
        for _ in candidates.as_slice() {
            if focused_work
                && let Err(error) = self
                    .call_resolver_control
                    .check_signature_query_step(SignatureQueryStep::CandidateMaterialization)
            {
                self.errors.push(TypeCheckError::new(error.to_string()));
                self.call_target_fact_recorder
                    .record_resolve_error(site.call_span.as_ref(), error);
                return false;
            }
            if focused_work && !self.charge_signature_work(SignatureWorkKind::Overloads, 1) {
                return false;
            }
        }
        true
    }

    fn finish_rejected_registered_candidates(
        &mut self,
        site: &RegisteredCallSite<'_>,
        candidates: &NonEmptyResolvedCandidates,
        probes: &[RegisteredCandidateProbe<'_>],
        selection: &RegisteredCandidateSelection<'_>,
    ) -> TypeKind {
        debug_assert!(!selection.selected.specificity.is_viable());
        let (arguments, argument_diagnostics, fallback_result, retained_specific_error) =
            if probes.len() == 1 {
                let Some(rejected) =
                    self.replay_rejected_registered_candidate(site, selection.selected.candidate)
                else {
                    return TypeKind::Named("_".to_owned());
                };
                (
                    rejected.arguments.facts,
                    rejected.arguments.diagnostics,
                    rejected.result,
                    rejected.retained_specific_error,
                )
            } else {
                (
                    probes[0].arguments.facts.clone(),
                    probes[0].arguments.diagnostics.clone(),
                    TypeKind::Named("_".to_owned()),
                    false,
                )
            };
        self.call_target_fact_recorder
            .restore_selected_nested_facts_from(&probes[0].focused_facts);
        self.finish_unselected_registered_candidates(
            site,
            UnselectedRegisteredCandidates {
                candidates: candidates.as_slice(),
                arguments,
                argument_diagnostics,
                fallback_result,
                reason: UnselectedRegisteredReason::Rejected,
                message: (!retained_specific_error)
                    .then(|| format!("call `{}` has no viable signature", site.label)),
            },
        )
    }

    fn probe_registered_candidates<'a>(
        &mut self,
        site: &RegisteredCallSite<'_>,
        candidates: &'a NonEmptyResolvedCandidates,
        charges_candidate_work: bool,
    ) -> Option<Vec<RegisteredCandidateProbe<'a>>> {
        let mut probes = Vec::with_capacity(candidates.as_slice().len());
        for candidate in candidates.as_slice() {
            if charges_candidate_work
                && let Err(error) = self
                    .call_resolver_control
                    .check_signature_query_step(SignatureQueryStep::CandidateProbe)
            {
                self.errors.push(TypeCheckError::new(error.to_string()));
                self.call_target_fact_recorder
                    .record_resolve_error(site.call_span.as_ref(), error);
                return None;
            }
            let checkpoint = self.checkpoint_registered_candidate();
            self.signature_work_charge.candidate_work = charges_candidate_work;
            let error_checkpoint = self.errors.len();
            let checked = self.evaluate_registered_candidate(site, candidate, true);
            let probe = RegisteredCandidateProbe {
                candidate,
                specificity: registered_candidate_specificity(
                    candidate,
                    site.group,
                    &checked.arguments,
                    self.errors.len().saturating_sub(error_checkpoint),
                    candidate.call_shape_is_viable(site.group, site.call.args()),
                ),
                arguments: checked.arguments,
                focused_facts: self.call_target_fact_recorder.clone(),
            };
            self.rollback_registered_candidate(checkpoint);
            if self
                .call_target_fact_recorder
                .terminal_query_error()
                .is_some()
            {
                return None;
            }
            probes.push(probe);
        }
        Some(probes)
    }

    fn replay_registered_candidate(
        &mut self,
        site: &RegisteredCallSite<'_>,
        candidate: &ResolvedCallable,
        considered: &[ResolvedCallable],
        charges_candidate_work: bool,
        focused_work: bool,
    ) -> TypeKind {
        if focused_work
            && let Err(error) = self
                .call_resolver_control
                .check_signature_query_step(SignatureQueryStep::SelectedReplay)
        {
            self.errors.push(TypeCheckError::new(error.to_string()));
            self.call_target_fact_recorder
                .record_resolve_error(site.call_span.as_ref(), error);
            return TypeKind::Named("_".to_owned());
        }
        let checkpoint = self.checkpoint_registered_candidate();
        let result = self.check_registered_candidate_with_work(
            site,
            candidate,
            considered,
            charges_candidate_work,
        );
        if self
            .call_target_fact_recorder
            .terminal_query_error()
            .is_some()
        {
            self.rollback_registered_candidate(checkpoint);
            return TypeKind::Named("_".to_owned());
        }
        self.commit_registered_candidate(&checkpoint);
        result
    }

    fn select_registered_candidate_probe<'a>(
        &mut self,
        site: &RegisteredCallSite<'_>,
        probes: &'a [RegisteredCandidateProbe<'a>],
    ) -> Result<Option<RegisteredCandidateSelection<'a>>, ()> {
        let Some(mut selected) = probes.first() else {
            return Ok(None);
        };
        let mut tied = vec![selected];
        let focused = self.uses_focused_callable_work(site.call_span.as_ref());
        for probe in probes.iter().skip(1) {
            if focused
                && let Err(error) = self
                    .call_resolver_control
                    .check_signature_query_step(SignatureQueryStep::CandidateComparison)
            {
                self.errors.push(TypeCheckError::new(error.to_string()));
                self.call_target_fact_recorder
                    .record_resolve_error(site.call_span.as_ref(), error);
                return Err(());
            }
            if !self.charge_callable_work(site.call, focused, CallableWorkOperation::Resolver) {
                return Err(());
            }
            match compare_registered_candidate_probes(probe, selected) {
                Ordering::Greater => {
                    selected = probe;
                    tied.clear();
                    tied.push(probe);
                }
                Ordering::Equal => tied.push(probe),
                Ordering::Less => {}
            }
        }
        Ok(Some(RegisteredCandidateSelection { selected, tied }))
    }

    fn finish_unselected_registered_candidates(
        &mut self,
        site: &RegisteredCallSite<'_>,
        unselected: UnselectedRegisteredCandidates<'_>,
    ) -> TypeKind {
        let UnselectedRegisteredCandidates {
            candidates,
            arguments,
            mut argument_diagnostics,
            fallback_result,
            reason,
            message,
        } = unselected;
        if let Some(message) = message {
            self.errors.push(TypeCheckError::new(message));
        }
        let records_facts = self.records_call_target_facts(site.call_span.as_ref());
        if records_facts && let Some(call_span) = &site.call_span {
            if matches!(reason, UnselectedRegisteredReason::Ambiguous)
                || argument_diagnostics.is_empty()
            {
                let diagnostic = match reason {
                    UnselectedRegisteredReason::Rejected => {
                        CallableDiagnosticCode::NoViableSignature
                    }
                    UnselectedRegisteredReason::Ambiguous => {
                        CallableDiagnosticCode::AmbiguousOverload
                    }
                };
                argument_diagnostics.push(CallableDiagnosticDraft::error(
                    diagnostic,
                    Some(call_span.clone()),
                    CallableDiagnosticSubject::Candidate(candidates[0].id().clone()),
                ));
            }
            let checked = match reason {
                UnselectedRegisteredReason::Rejected => {
                    CheckedCallTarget::rejected(candidates, arguments, site.group)
                }
                UnselectedRegisteredReason::Ambiguous => {
                    CheckedCallTarget::ambiguous(candidates, arguments, site.group)
                }
            };
            self.record_call_target_facts(
                site.expression,
                site.document,
                call_span,
                checked,
                argument_diagnostics,
            );
        }
        fallback_result
    }

    fn replay_rejected_registered_candidate(
        &mut self,
        site: &RegisteredCallSite<'_>,
        candidate: &ResolvedCallable,
    ) -> Option<RejectedRegisteredCandidate> {
        let checkpoint = self.checkpoint_registered_candidate();
        let error_start = self.errors.len();
        let warning_start = self.warnings.len();
        let judgment_start = self.judgments.len();
        let previous_candidate_work = self.signature_work_charge.candidate_work;
        self.signature_work_charge.candidate_work = false;
        let checked = self.evaluate_registered_candidate(site, candidate, true);
        self.signature_work_charge.candidate_work = previous_candidate_work;
        let errors = self.errors[error_start..].to_vec();
        let retained_specific_error = !errors.is_empty();
        let warnings = self.warnings[warning_start..].to_vec();
        let judgments = self.judgments[judgment_start..].to_vec();
        self.rollback_registered_candidate(checkpoint);
        if self
            .call_target_fact_recorder
            .terminal_query_error()
            .is_some()
        {
            return None;
        }
        self.errors.extend(errors);
        self.warnings.extend(warnings);
        self.judgments.extend(judgments);
        Some(RejectedRegisteredCandidate {
            arguments: checked.arguments,
            result: checked.result,
            retained_specific_error,
        })
    }

    pub(super) fn check_registered_candidate(
        &mut self,
        site: &RegisteredCallSite<'_>,
        candidate: &ResolvedCallable,
        considered: &[ResolvedCallable],
    ) -> TypeKind {
        let focused_work = self.uses_focused_callable_work(site.call_span.as_ref());
        self.check_registered_candidate_with_work(site, candidate, considered, focused_work)
    }

    fn check_registered_candidate_with_work(
        &mut self,
        site: &RegisteredCallSite<'_>,
        candidate: &ResolvedCallable,
        considered: &[ResolvedCallable],
        charges_candidate_work: bool,
    ) -> TypeKind {
        let records_facts = self.records_call_target_facts(site.call_span.as_ref());
        let previous_candidate_work = self.signature_work_charge.candidate_work;
        self.signature_work_charge.candidate_work = records_facts && charges_candidate_work;
        let checked = self.evaluate_registered_candidate(site, candidate, records_facts);
        self.signature_work_charge.candidate_work = previous_candidate_work;
        let label = site.label;
        let schema = candidate.schema();
        let effect_site = EffectSite::new(format!("call `{label}`"));
        if let CallableCandidateId::Presentation(id) = candidate.id() {
            match id {
                crate::callable::PresentationCallableId::ClearBackground => {
                    self.clear_active_presentation_default("background");
                }
                crate::callable::PresentationCallableId::Hide => {
                    self.clear_active_presentation_default("character");
                }
                _ => {}
            }
        }
        if let Some(declaration) = schema.effects().project_declaration() {
            self.effect_collector.record_local_call(
                crate::effect_model::CallableId::project_function(declaration),
                effect_site,
            );
        } else {
            self.effect_collector.record_named_call(
                label,
                Some(schema.effects().declared().concrete().clone()),
                effect_site,
            );
        }
        if let CallableCandidateId::Project(declaration) = candidate.id()
            && let (Some(module), Some(range)) = (&self.current_module, site.callee_range)
        {
            self.project_callable_references
                .push(ProjectCallableReference {
                    module: module.clone(),
                    declaration: declaration.clone(),
                    range,
                });
        }
        let result = checked.result;
        if records_facts && let Some(call_span) = &site.call_span {
            let RegisteredArgumentCheck {
                facts,
                poison,
                diagnostics,
            } = checked.arguments;
            let checked_target = CheckedCallTarget::selected(
                candidate,
                considered,
                facts,
                result.clone(),
                site.group,
                poison,
            );
            let checked_target = match &site.function_value_type {
                Some(function_value_type) => {
                    checked_target.with_function_value_type(function_value_type.clone())
                }
                None => checked_target,
            };
            self.record_call_target_facts(
                site.expression,
                site.document,
                call_span,
                checked_target,
                diagnostics,
            );
        }
        let completed_group = match candidate.instantiation() {
            crate::callable::CallableInstantiation::DataLast { group, .. } => *group,
            _ => site.group,
        };
        self.retain_registered_curried_result(label, candidate, completed_group, &result);
        result
    }

    fn evaluate_registered_candidate(
        &mut self,
        site: &RegisteredCallSite<'_>,
        candidate: &ResolvedCallable,
        records_facts: bool,
    ) -> RegisteredCandidateCheck {
        let focused = self.uses_focused_callable_work(site.call_span.as_ref());
        if focused {
            self.focused_candidate_depth += 1;
        }
        let checked = self.evaluate_registered_candidate_body(site, candidate, records_facts);
        if focused {
            self.focused_candidate_depth = self
                .focused_candidate_depth
                .checked_sub(1)
                .expect("focused candidate depth exits exactly once");
        }
        checked
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one validator dispatcher keeps candidate probing and committed replay behavior identical"
    )]
    fn evaluate_registered_candidate_body(
        &mut self,
        site: &RegisteredCallSite<'_>,
        candidate: &ResolvedCallable,
        records_facts: bool,
    ) -> RegisteredCandidateCheck {
        let schema = candidate.schema();
        let focused = self.uses_focused_callable_work(site.call_span.as_ref());
        if !self.charge_callable_work(site.call, focused, CallableWorkOperation::Resolver) {
            return RegisteredCandidateCheck {
                arguments: RegisteredArgumentCheck::new(Vec::new(), CallPoison::Rejected),
                result: schema_result_type(schema, site.group),
            };
        }
        self.check_virtual_path_call(site.label, site.call.args());
        if let CallableCandidateId::DataLast(id) = candidate.id() {
            let Some((receiver, receiver_type)) = site.receiver else {
                self.errors.push(TypeCheckError::new(format!(
                    "data-last callable `{}` has no selected receiver",
                    site.label
                )));
                return RegisteredCandidateCheck {
                    arguments: RegisteredArgumentCheck::new(
                        self.check_unmapped_registered_arguments(
                            site.call,
                            CallPoison::Rejected,
                            records_facts,
                        ),
                        CallPoison::Rejected,
                    ),
                    result: schema_result_type(schema, site.group),
                };
            };
            return self.check_registered_data_last_candidate(
                site.label,
                id,
                schema,
                site.call,
                receiver,
                receiver_type,
                site.group,
                site.expression,
                records_facts,
            );
        }
        match schema.validator() {
            CallableValidator::Ordinary
            | CallableValidator::Untyped
            | CallableValidator::Builtin(_)
            | CallableValidator::Agent(_)
            | CallableValidator::Fx(_)
            | CallableValidator::EnumConstructor(_)
            | CallableValidator::Presentation(_)
            | CallableValidator::Collection(_)
            | CallableValidator::PresentationHandle(_)
            | CallableValidator::Integer(_)
            | CallableValidator::Domain(_)
            | CallableValidator::Capacity(_)
            | CallableValidator::Stage(_)
            | CallableValidator::Trait(_)
            | CallableValidator::Drop
            | CallableValidator::Promotion(_)
            | CallableValidator::Speaker => RegisteredCandidateCheck {
                arguments: self.check_registered_schema_args(
                    site.label,
                    schema,
                    site.group,
                    site.call,
                    records_facts,
                ),
                result: schema_result_type(schema, site.group),
            },
            CallableValidator::ReductionConstructor(kind) => self
                .check_registered_reduction_constructor(
                    *kind,
                    site.label,
                    schema,
                    site.call,
                    records_facts,
                ),
            CallableValidator::ResultConstructor(kind) => self.check_registered_sum_constructor(
                RegisteredSumConstructor::Result(*kind),
                site.label,
                schema,
                site.call,
                records_facts,
            ),
            CallableValidator::OptionConstructor(kind) => self.check_registered_sum_constructor(
                RegisteredSumConstructor::Option(*kind),
                site.label,
                schema,
                site.call,
                records_facts,
            ),
            validator => {
                self.errors.push(TypeCheckError::new(format!(
                    "registered callable `{}` has unsupported validator {validator:?}",
                    site.label
                )));
                RegisteredCandidateCheck {
                    arguments: RegisteredArgumentCheck::new(
                        self.check_unmapped_registered_arguments(
                            site.call,
                            CallPoison::Rejected,
                            records_facts,
                        ),
                        CallPoison::Rejected,
                    ),
                    result: schema_result_type(schema, site.group),
                }
            }
        }
    }
}

impl RegisteredCandidateSpecificity {
    const fn is_viable(self) -> bool {
        self.shape_viable && !matches!(self.poison, CallPoison::Rejected)
    }
}

fn registered_candidate_specificity(
    candidate: &ResolvedCallable,
    group: CallableGroupIndex,
    arguments: &RegisteredArgumentCheck,
    hard_errors: usize,
    shape_viable: bool,
) -> RegisteredCandidateSpecificity {
    let mut mapped = HashSet::new();
    let mut exact_matches = 0;
    let mut compatible_matches = 0;
    let mut unchecked_or_open = 0;
    let mut rest_bindings = 0;
    for slot in arguments
        .facts
        .iter()
        .flat_map(CheckedCallArgumentFact::slots)
    {
        if let Some(coordinate) = slot.mapped() {
            mapped.insert(coordinate);
            if candidate
                .schema()
                .group(coordinate.group())
                .and_then(|parameter_group| {
                    parameter_group
                        .parameters()
                        .get(coordinate.parameter().get())
                })
                .is_some_and(|parameter| {
                    matches!(
                        parameter.passing(),
                        CallableParameterPassing::RestPositional
                            | CallableParameterPassing::RestNamed
                    )
                })
            {
                rest_bindings += 1;
            }
        }
        match registered_slot_specificity(slot.expected(), slot.inferred(), slot.poison()) {
            RegisteredSlotSpecificity::Exact => exact_matches += 1,
            RegisteredSlotSpecificity::Compatible => compatible_matches += 1,
            RegisteredSlotSpecificity::UncheckedOrOpen => unchecked_or_open += 1,
        }
    }
    let omissions = candidate
        .schema()
        .group(group)
        .map_or(0, |parameter_group| {
            parameter_group
                .parameters()
                .iter()
                .filter(|parameter| {
                    parameter.presence() != CallableParameterPresence::Required
                        && !mapped.contains(&crate::callable::CallableParameterCoordinate::new(
                            group,
                            parameter.index(),
                        ))
                })
                .count()
        });
    RegisteredCandidateSpecificity {
        shape_viable,
        poison: arguments.poison,
        hard_errors,
        exact_matches,
        compatible_matches,
        unchecked_or_open,
        omissions,
        rest_bindings,
    }
}

fn registered_slot_specificity(
    expected: Option<&TypeKind>,
    inferred: Option<&TypeKind>,
    poison: CallPoison,
) -> RegisteredSlotSpecificity {
    match (expected, inferred) {
        (Some(expected), Some(inferred))
            if expected.has_open_components() || inferred.has_open_components() =>
        {
            RegisteredSlotSpecificity::UncheckedOrOpen
        }
        (Some(expected), Some(inferred)) if expected == inferred => {
            RegisteredSlotSpecificity::Exact
        }
        (Some(_), Some(_)) if !matches!(poison, CallPoison::Rejected) => {
            RegisteredSlotSpecificity::Compatible
        }
        _ => RegisteredSlotSpecificity::UncheckedOrOpen,
    }
}

fn compare_registered_candidate_probes(
    left: &RegisteredCandidateProbe<'_>,
    right: &RegisteredCandidateProbe<'_>,
) -> Ordering {
    let left_specificity = left.specificity;
    let right_specificity = right.specificity;
    compare_registered_candidate_specificity(left_specificity, right_specificity)
        .then_with(|| compare_standard_adapter(left.candidate, right.candidate))
}

fn compare_registered_candidate_specificity(
    left_specificity: RegisteredCandidateSpecificity,
    right_specificity: RegisteredCandidateSpecificity,
) -> Ordering {
    poison_rank(left_specificity.poison)
        .cmp(&poison_rank(right_specificity.poison))
        .then_with(|| {
            right_specificity
                .hard_errors
                .cmp(&left_specificity.hard_errors)
        })
        .then_with(|| {
            left_specificity
                .exact_matches
                .cmp(&right_specificity.exact_matches)
        })
        .then_with(|| {
            right_specificity
                .unchecked_or_open
                .cmp(&left_specificity.unchecked_or_open)
        })
        .then_with(|| right_specificity.omissions.cmp(&left_specificity.omissions))
}

const fn poison_rank(poison: CallPoison) -> u8 {
    match poison {
        CallPoison::Clean => 2,
        CallPoison::Recovered => 1,
        CallPoison::Rejected => 0,
    }
}

fn compare_standard_adapter(left: &ResolvedCallable, right: &ResolvedCallable) -> Ordering {
    match (left.authority(), right.authority()) {
        (Some(CallableAuthorityRank::Standard), Some(CallableAuthorityRank::Adapter)) => {
            Ordering::Greater
        }
        (Some(CallableAuthorityRank::Adapter), Some(CallableAuthorityRank::Standard)) => {
            Ordering::Less
        }
        _ => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specificity() -> RegisteredCandidateSpecificity {
        RegisteredCandidateSpecificity {
            shape_viable: true,
            poison: CallPoison::Clean,
            hard_errors: 0,
            exact_matches: 0,
            compatible_matches: 0,
            unchecked_or_open: 0,
            omissions: 0,
            rest_bindings: 0,
        }
    }

    #[test]
    fn specificity_comparator_applies_every_declared_dimension_in_order() {
        let base = specificity();
        let preferred_pairs = [
            (
                base,
                RegisteredCandidateSpecificity {
                    poison: CallPoison::Recovered,
                    ..base
                },
                "clean poison",
            ),
            (
                base,
                RegisteredCandidateSpecificity {
                    hard_errors: 1,
                    ..base
                },
                "fewer hard errors",
            ),
            (
                RegisteredCandidateSpecificity {
                    exact_matches: 1,
                    ..base
                },
                base,
                "more exact matches",
            ),
            (
                base,
                RegisteredCandidateSpecificity {
                    unchecked_or_open: 1,
                    ..base
                },
                "fewer unchecked or open slots",
            ),
            (
                base,
                RegisteredCandidateSpecificity {
                    omissions: 1,
                    ..base
                },
                "fewer omissions",
            ),
        ];
        for (preferred, other, dimension) in preferred_pairs {
            assert_eq!(
                compare_registered_candidate_specificity(preferred, other),
                Ordering::Greater,
                "{dimension}"
            );
            assert_eq!(
                compare_registered_candidate_specificity(other, preferred),
                Ordering::Less,
                "{dimension}"
            );
        }
        assert_eq!(
            compare_registered_candidate_specificity(base, base),
            Ordering::Equal
        );
        assert_eq!(
            compare_registered_candidate_specificity(
                RegisteredCandidateSpecificity {
                    compatible_matches: 1,
                    rest_bindings: 1,
                    ..base
                },
                base,
            ),
            Ordering::Equal,
            "observational compatible/rest metrics are not tie-breakers"
        );
    }

    #[test]
    fn slot_specificity_separates_exact_compatible_and_recursively_open_types() {
        assert_eq!(
            registered_slot_specificity(
                Some(&TypeKind::I32),
                Some(&TypeKind::I32),
                CallPoison::Clean
            ),
            RegisteredSlotSpecificity::Exact
        );
        assert_eq!(
            registered_slot_specificity(
                Some(&TypeKind::Bytes),
                Some(&TypeKind::Vec(Box::new(TypeKind::U8))),
                CallPoison::Clean,
            ),
            RegisteredSlotSpecificity::Compatible
        );
        for open in [
            TypeKind::Named("_".to_owned()),
            TypeKind::Option(Box::new(TypeKind::Named("_".to_owned()))),
            TypeKind::function_with_effects(
                [TypeKind::I32],
                TypeKind::Unit,
                crate::effect_row::EffectRow::unknown(),
            ),
        ] {
            assert_eq!(
                registered_slot_specificity(Some(&open), Some(&TypeKind::I32), CallPoison::Clean),
                RegisteredSlotSpecificity::UncheckedOrOpen
            );
        }
    }
}
