use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    effect_diagnostics::{
        EffectDiagnostic, EffectDiagnosticCode, EffectDiagnosticKind, EffectSeverity, EffectTrace,
        EffectTraceStep,
    },
    effect_model::{CallTarget, CallableId, EffectProgram},
    effect_row::{
        ClosedEffectRowReport, EffectRowCloseError, EffectRowReport, EffectRowSummary,
        EffectSubstitution,
    },
    effects::{EffectId, EffectSet},
};

/// Final effect summary for one callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectSummary {
    callable: CallableId,
    declared: Option<EffectSet>,
    forbidden: EffectSet,
    inferred: EffectSet,
}

/// Stable trace for one inferred effect on one callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectTraceSummary {
    callable: CallableId,
    trace: EffectTrace,
}

/// Deterministic origin traces for effects inferred by analysis.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectTraceReport {
    traces: BTreeMap<CallableId, Vec<EffectTraceSummary>>,
}

/// Result of first-order effect closure and contract validation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectAnalysisReport {
    summaries: BTreeMap<CallableId, EffectSummary>,
    rows: EffectRowReport,
    traces: EffectTraceReport,
    row_substitutions: EffectSubstitution,
    diagnostics: Vec<EffectDiagnostic>,
    fixed_point_iterations: usize,
}

/// Computes the least fixed-point effect closure and validates all contracts.
pub fn analyze_effects(program: &EffectProgram) -> EffectAnalysisReport {
    let mut diagnostics = collect_graph_diagnostics(program);
    let mut summaries = initial_summaries(program);
    let fixed_point_iterations = propagate_local_effects(program, &mut summaries);
    let rows = collect_effect_rows(&summaries);
    let traces = collect_effect_traces(program, &summaries);

    diagnostics.extend(validate_contracts(program, &summaries));
    diagnostics.sort_by(|left, right| diagnostic_sort_key(left).cmp(&diagnostic_sort_key(right)));

    EffectAnalysisReport {
        summaries,
        rows,
        traces,
        row_substitutions: EffectSubstitution::new(),
        diagnostics,
        fixed_point_iterations,
    }
}

impl EffectSummary {
    pub const fn callable(&self) -> &CallableId {
        &self.callable
    }

    pub const fn declared(&self) -> Option<&EffectSet> {
        self.declared.as_ref()
    }

    pub const fn forbidden(&self) -> &EffectSet {
        &self.forbidden
    }

    pub const fn inferred(&self) -> &EffectSet {
        &self.inferred
    }
}

impl EffectTraceSummary {
    pub const fn callable(&self) -> &CallableId {
        &self.callable
    }

    pub const fn effect(&self) -> &EffectId {
        self.trace.effect()
    }

    pub const fn trace(&self) -> &EffectTrace {
        &self.trace
    }
}

impl EffectTraceReport {
    pub fn trace(&self, callable: &CallableId, effect: &EffectId) -> Option<&EffectTrace> {
        self.summary(callable, effect)
            .map(EffectTraceSummary::trace)
    }

    pub fn summary(&self, callable: &CallableId, effect: &EffectId) -> Option<&EffectTraceSummary> {
        self.traces
            .get(callable)?
            .iter()
            .find(|summary| summary.effect() == effect)
    }

    pub fn traces_for(&self, callable: &CallableId) -> &[EffectTraceSummary] {
        self.traces.get(callable).map_or(&[], Vec::as_slice)
    }

    pub fn callables(&self) -> impl ExactSizeIterator<Item = (&CallableId, &[EffectTraceSummary])> {
        self.traces
            .iter()
            .map(|(callable, traces)| (callable, traces.as_slice()))
    }

    pub fn summaries(&self) -> impl Iterator<Item = &EffectTraceSummary> {
        self.traces.values().flat_map(|traces| traces.iter())
    }
}

impl EffectAnalysisReport {
    pub fn summary(&self, callable: &CallableId) -> Option<&EffectSummary> {
        self.summaries.get(callable)
    }

    pub fn summaries(&self) -> impl ExactSizeIterator<Item = (&CallableId, &EffectSummary)> {
        self.summaries.iter()
    }

    pub const fn effect_traces(&self) -> &EffectTraceReport {
        &self.traces
    }

    pub fn diagnostics(&self) -> &[EffectDiagnostic] {
        &self.diagnostics
    }

    pub fn errors(&self) -> impl Iterator<Item = &EffectDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity() == EffectSeverity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &EffectDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity() == EffectSeverity::Warning)
    }

    pub fn has_errors(&self) -> bool {
        self.errors().next().is_some()
    }

    pub const fn fixed_point_iterations(&self) -> usize {
        self.fixed_point_iterations
    }

    pub const fn effect_rows(&self) -> &EffectRowReport {
        &self.rows
    }

    pub fn closed_effect_rows(&self) -> Result<ClosedEffectRowReport, EffectRowCloseError> {
        self.rows.resolve_closed(&self.row_substitutions)
    }
}

fn collect_effect_rows(summaries: &BTreeMap<CallableId, EffectSummary>) -> EffectRowReport {
    EffectRowReport::new(summaries.values().map(|summary| {
        EffectRowSummary::closed(
            summary.callable().clone(),
            summary.inferred().clone(),
            summary.declared().cloned(),
            summary.forbidden().clone(),
        )
    }))
}

fn collect_effect_traces(
    program: &EffectProgram,
    summaries: &BTreeMap<CallableId, EffectSummary>,
) -> EffectTraceReport {
    let traces = summaries
        .iter()
        .filter_map(|(callable, summary)| {
            let callable_traces = summary
                .inferred()
                .iter()
                .filter_map(|effect| {
                    trace_for(program, summaries, callable, effect).map(|trace| {
                        EffectTraceSummary {
                            callable: callable.clone(),
                            trace,
                        }
                    })
                })
                .collect::<Vec<_>>();
            (!callable_traces.is_empty()).then(|| (callable.clone(), callable_traces))
        })
        .collect();
    EffectTraceReport { traces }
}

fn initial_summaries(program: &EffectProgram) -> BTreeMap<CallableId, EffectSummary> {
    program
        .callables()
        .map(|(id, facts)| {
            let mut inferred = facts
                .direct_effects()
                .iter()
                .map(|effect_use| effect_use.effect().clone())
                .collect::<EffectSet>();
            for edge in facts.calls() {
                match edge.target() {
                    CallTarget::External(callee) => {
                        inferred.union_with(callee.effects());
                    }
                    CallTarget::Dynamic {
                        effects: Some(effects),
                        ..
                    } => {
                        inferred.union_with(effects);
                    }
                    CallTarget::Local(_) | CallTarget::Dynamic { effects: None, .. } => {}
                }
            }
            (
                id.clone(),
                EffectSummary {
                    callable: id.clone(),
                    declared: facts.contract().upper_bound().cloned(),
                    forbidden: facts.contract().forbidden().clone(),
                    inferred,
                },
            )
        })
        .collect()
}

fn propagate_local_effects(
    program: &EffectProgram,
    summaries: &mut BTreeMap<CallableId, EffectSummary>,
) -> usize {
    let mut iterations = 0;
    loop {
        iterations += 1;
        let mut changed = false;
        for (caller, facts) in program.callables() {
            let propagated = facts
                .calls()
                .iter()
                .filter_map(|edge| match edge.target() {
                    CallTarget::Local(callee) => summaries.get(callee).map(EffectSummary::inferred),
                    CallTarget::External(_) | CallTarget::Dynamic { .. } => None,
                })
                .fold(EffectSet::new(), |mut effects, callee_effects| {
                    effects.union_with(callee_effects);
                    effects
                });
            if let Some(summary) = summaries.get_mut(caller) {
                changed |= summary.inferred.union_with(&propagated);
            }
        }
        if !changed {
            return iterations;
        }
    }
}

fn collect_graph_diagnostics(program: &EffectProgram) -> Vec<EffectDiagnostic> {
    program
        .callables()
        .flat_map(|(caller, facts)| {
            facts.calls().iter().filter_map(move |edge| match edge.target() {
                CallTarget::Local(callee) if program.callable(callee).is_none() => {
                    Some(EffectDiagnostic::new(
                        EffectDiagnosticCode::UnknownLocalCallable,
                        EffectSeverity::Error,
                        caller.clone(),
                        format!("callable `{caller}` references unknown callable `{callee}`"),
                        EffectDiagnosticKind::UnknownLocalCallable {
                            callee: callee.clone(),
                        },
                        None,
                    ))
                }
                CallTarget::Dynamic {
                    label,
                    effects: None,
                } => Some(EffectDiagnostic::new(
                    EffectDiagnosticCode::DynamicSignatureRequired,
                    EffectSeverity::Error,
                    caller.clone(),
                    format!(
                        "dynamic call `{label}` in `{caller}` requires a function effect signature"
                    ),
                    EffectDiagnosticKind::DynamicSignatureRequired {
                        target: label.clone(),
                    },
                    None,
                )),
                CallTarget::Local(_)
                | CallTarget::External(_)
                | CallTarget::Dynamic {
                    effects: Some(_), ..
                } => None,
            })
        })
        .collect()
}

fn validate_contracts(
    program: &EffectProgram,
    summaries: &BTreeMap<CallableId, EffectSummary>,
) -> Vec<EffectDiagnostic> {
    let mut diagnostics = Vec::new();
    for (id, facts) in program.callables() {
        let Some(summary) = summaries.get(id) else {
            continue;
        };
        let inferred = summary.inferred();

        if facts.contract().is_pure() && !inferred.is_empty() {
            let effect = first_effect(inferred);
            diagnostics.push(EffectDiagnostic::new(
                EffectDiagnosticCode::PureCallableEffect,
                EffectSeverity::Error,
                id.clone(),
                format!("pure callable `{id}` performs effects {inferred}"),
                EffectDiagnosticKind::PureCallableEffect {
                    inferred: inferred.clone(),
                },
                effect.and_then(|effect| trace_for(program, summaries, id, effect)),
            ));
        } else if let Some(declared) = summary.declared() {
            push_declared_effect_diagnostics(
                program,
                summaries,
                id,
                inferred,
                declared,
                &mut diagnostics,
            );
        }

        push_forbidden_effect_diagnostics(
            program,
            summaries,
            id,
            inferred,
            summary,
            &mut diagnostics,
        );

        if let Some(available) = program.available_capabilities() {
            push_capability_availability_diagnostic(
                program,
                summaries,
                id,
                inferred,
                available,
                &mut diagnostics,
            );
        }
    }
    diagnostics
}

fn push_declared_effect_diagnostics(
    program: &EffectProgram,
    summaries: &BTreeMap<CallableId, EffectSummary>,
    id: &CallableId,
    inferred: &EffectSet,
    declared: &EffectSet,
    diagnostics: &mut Vec<EffectDiagnostic>,
) {
    let missing = inferred.effects_not_covered_by(declared);
    for effect in &missing {
        let missing_one = std::iter::once(effect.clone()).collect();
        diagnostics.push(EffectDiagnostic::new(
            EffectDiagnosticCode::UpperBoundExceeded,
            EffectSeverity::Error,
            id.clone(),
            format!(
                "callable `{id}` infers effect `{effect}`, exceeding explicit upper bound {declared}"
            ),
            EffectDiagnosticKind::UpperBoundExceeded {
                excess: missing_one,
                upper_bound: declared.clone(),
            },
            trace_for(program, summaries, id, effect),
        ));
    }
}

fn push_forbidden_effect_diagnostics(
    program: &EffectProgram,
    summaries: &BTreeMap<CallableId, EffectSummary>,
    id: &CallableId,
    inferred: &EffectSet,
    summary: &EffectSummary,
    diagnostics: &mut Vec<EffectDiagnostic>,
) {
    let forbidden = inferred
        .iter()
        .filter(|effect| {
            summary
                .forbidden()
                .iter()
                .any(|forbidden| forbidden.covers(effect))
        })
        .cloned()
        .collect::<EffectSet>();
    for effect in &forbidden {
        let forbidden_one = std::iter::once(effect.clone()).collect();
        diagnostics.push(EffectDiagnostic::new(
            EffectDiagnosticCode::ForbiddenEffect,
            EffectSeverity::Error,
            id.clone(),
            format!("callable `{id}` forbids effect `{effect}`, but it is reachable"),
            EffectDiagnosticKind::ForbiddenEffect {
                forbidden: forbidden_one,
            },
            trace_for(program, summaries, id, effect),
        ));
    }
}

fn push_capability_availability_diagnostic(
    program: &EffectProgram,
    summaries: &BTreeMap<CallableId, EffectSummary>,
    id: &CallableId,
    inferred: &EffectSet,
    available: &EffectSet,
    diagnostics: &mut Vec<EffectDiagnostic>,
) {
    let unavailable = inferred.effects_not_covered_by(available);
    if unavailable.is_empty() {
        return;
    }
    let effect = first_effect(&unavailable).cloned();
    diagnostics.push(EffectDiagnostic::new(
        EffectDiagnosticCode::CapabilityUnavailable,
        EffectSeverity::Error,
        id.clone(),
        format!("target environment cannot provide effects {unavailable} required by `{id}`"),
        EffectDiagnosticKind::CapabilityUnavailable { unavailable },
        effect
            .as_ref()
            .and_then(|effect| trace_for(program, summaries, id, effect)),
    ));
}

fn trace_for(
    program: &EffectProgram,
    summaries: &BTreeMap<CallableId, EffectSummary>,
    root: &CallableId,
    effect: &EffectId,
) -> Option<EffectTrace> {
    let mut queue = VecDeque::from([(root.clone(), Vec::new())]);
    let mut visited = BTreeSet::from([root.clone()]);

    while let Some((current, path)) = queue.pop_front() {
        let facts = program.callable(&current)?;
        if let Some(effect_use) = facts
            .direct_effects()
            .iter()
            .find(|effect_use| effect_use.effect() == effect)
        {
            let mut steps = path;
            steps.push(EffectTraceStep::Perform {
                callable: current,
                effect: effect.clone(),
                site: effect_use.site().clone(),
            });
            return Some(EffectTrace::new(effect.clone(), steps));
        }

        let mut calls = facts.calls().iter().collect::<Vec<_>>();
        calls.sort_by_key(|edge| call_target_label(edge.target()));
        for edge in calls {
            match edge.target() {
                CallTarget::External(callee) if callee.effects().contains(effect) => {
                    let mut steps = path.clone();
                    steps.push(EffectTraceStep::ExternalCall {
                        caller: current.clone(),
                        callee: callee.name().to_owned(),
                        site: edge.site().clone(),
                    });
                    return Some(EffectTrace::new(effect.clone(), steps));
                }
                CallTarget::Dynamic {
                    label,
                    effects: Some(effects),
                } if effects.contains(effect) => {
                    let mut steps = path.clone();
                    steps.push(EffectTraceStep::DynamicCall {
                        caller: current.clone(),
                        target: label.clone(),
                        site: edge.site().clone(),
                    });
                    return Some(EffectTrace::new(effect.clone(), steps));
                }
                CallTarget::Local(callee)
                    if summaries
                        .get(callee)
                        .is_some_and(|summary| summary.inferred().contains(effect))
                        && visited.insert(callee.clone()) =>
                {
                    let mut steps = path.clone();
                    steps.push(EffectTraceStep::Call {
                        caller: current.clone(),
                        callee: callee.clone(),
                        site: edge.site().clone(),
                    });
                    queue.push_back((callee.clone(), steps));
                }
                CallTarget::Local(_) | CallTarget::External(_) | CallTarget::Dynamic { .. } => {}
            }
        }
    }
    None
}

fn first_effect(effects: &EffectSet) -> Option<&EffectId> {
    effects.iter().next()
}

fn call_target_label(target: &CallTarget) -> String {
    match target {
        CallTarget::Local(callee) => format!("0:{callee}"),
        CallTarget::External(callee) => format!("1:{}", callee.name()),
        CallTarget::Dynamic { label, .. } => format!("2:{label}"),
    }
}

fn diagnostic_sort_key(diagnostic: &EffectDiagnostic) -> (u8, &'static str, String, String) {
    (
        match diagnostic.severity() {
            EffectSeverity::Error => 0,
            EffectSeverity::Warning => 1,
        },
        diagnostic.code().as_str(),
        diagnostic.callable().as_str().to_owned(),
        diagnostic.message().to_owned(),
    )
}
