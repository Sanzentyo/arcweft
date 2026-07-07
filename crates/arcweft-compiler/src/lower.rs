use std::fmt;

use arcweft_core::plan::{RuntimeIteratorEvidence, RuntimePlan, RuntimePureHelperOrigin};
use arcweft_lang_hir::model::{HirFunction, HirModule};
use arcweft_lang_sema::check::{
    ForIterationEvidence, ForIterationEvidenceFamily, StandardIteratorFamily, TypeCheckReport,
    TypedLoweringEvidence, TypedLoweringEvidenceKind,
};
use arcweft_runtime_plan::flow::{
    RuntimePlanLowerOptions, RuntimePlanLowerReport, lower_runtime_plan_with_options,
    lower_runtime_plan_with_stats_and_options,
};
use arcweft_runtime_plan::line_task::{LoweredLineTaskGroup, lower_line_task_groups};
use arcweft_runtime_plan::pure::{
    PureHelperCandidate, PureHelperCandidateReport, PureHelperLowerError,
    lower_pure_helper_candidate, lower_pure_helper_candidates,
};
use arcweft_runtime_plan::typed_evidence::{
    RuntimeTypedExpressionId, RuntimeTypedLoweringEvidence, RuntimeTypedLoweringEvidenceKind,
};

use crate::trait_methods::{
    lower_runtime_trait_methods_from_typecheck, runtime_iterator_identity_witness_evidence,
    runtime_witness_evidence,
};
use crate::types::{
    TextPureHelperCandidateError, TextPureHelperCandidateReport, TextPureHelperKind,
};

/// Lowers dialogue line plans from HIR into runtime task groups.
pub fn lower_source_line_tasks(
    hir: &HirModule,
) -> Result<Vec<LoweredLineTaskGroup>, Vec<arcweft_runtime_plan::errors::LinePlanLowerError>> {
    lower_line_task_groups(hir)
}

/// Lowers checked HIR into a runtime plan with explicit profile/build-context options.
pub fn lower_source_runtime_plan_with_options(
    hir: &HirModule,
    options: &RuntimePlanLowerOptions,
) -> Result<RuntimePlan, Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>> {
    lower_runtime_plan_with_options(hir, options)
}

/// Lowers checked HIR using the `for` iteration evidence recorded by type checking.
pub fn lower_source_runtime_plan_with_typecheck_and_options(
    hir: &HirModule,
    typecheck: &TypeCheckReport,
    options: &RuntimePlanLowerOptions,
) -> Result<RuntimePlan, Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>> {
    let options = runtime_plan_options_with_typecheck_evidence(options, typecheck)?;
    lower_runtime_plan_with_options(hir, &options)
}

/// Lowers checked HIR into a runtime plan and display catalog with compiler counters.
pub fn lower_source_runtime_plan_with_stats_and_options(
    hir: &HirModule,
    options: &RuntimePlanLowerOptions,
) -> Result<RuntimePlanLowerReport, Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>> {
    lower_runtime_plan_with_stats_and_options(hir, options)
}

/// Lowers checked HIR into a runtime plan and display catalog with type-checker
/// iteration evidence.
pub fn lower_source_runtime_plan_with_typecheck_stats_and_options(
    hir: &HirModule,
    typecheck: &TypeCheckReport,
    options: &RuntimePlanLowerOptions,
) -> Result<RuntimePlanLowerReport, Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>> {
    let options = runtime_plan_options_with_typecheck_evidence(options, typecheck)?;
    lower_runtime_plan_with_stats_and_options(hir, &options)
}

pub fn runtime_plan_options_with_typecheck_evidence(
    options: &RuntimePlanLowerOptions,
    typecheck: &TypeCheckReport,
) -> Result<RuntimePlanLowerOptions, Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>> {
    let trait_methods = lower_runtime_trait_methods_from_typecheck(
        &typecheck.trait_catalog,
        &typecheck.for_iteration_evidence,
    )?;
    let evidence = typecheck
        .for_iteration_evidence
        .iter()
        .map(|evidence| runtime_iterator_evidence(evidence, &trait_methods))
        .collect::<Result<Vec<_>, _>>()?;
    let typed_lowering_evidence = typecheck
        .typed_lowering_evidence
        .iter()
        .map(runtime_typed_lowering_evidence)
        .collect::<Vec<_>>();
    Ok(options
        .clone()
        .with_for_iteration_evidence(evidence)
        .with_trait_methods(trait_methods.methods)
        .with_typed_lowering_evidence(typed_lowering_evidence))
}

fn runtime_typed_lowering_evidence(
    evidence: &TypedLoweringEvidence,
) -> RuntimeTypedLoweringEvidence {
    RuntimeTypedLoweringEvidence {
        expression_id: RuntimeTypedExpressionId::from_index(evidence.expression_id.index()),
        kind: match &evidence.kind {
            TypedLoweringEvidenceKind::FunctionValueCall {
                callee, arg_count, ..
            } => RuntimeTypedLoweringEvidenceKind::FunctionValueCall {
                callee: callee.clone(),
                arg_count: *arg_count,
            },
            TypedLoweringEvidenceKind::ExpectedFunctionValue { arity, .. } => {
                RuntimeTypedLoweringEvidenceKind::ExpectedFunctionValue { arity: *arity }
            }
        },
    }
}

fn runtime_iterator_evidence(
    evidence: &ForIterationEvidence,
    trait_methods: &arcweft_runtime_plan::trait_methods::RuntimeTraitMethodInventory,
) -> Result<RuntimeIteratorEvidence, Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>> {
    match evidence.family {
        ForIterationEvidenceFamily::Builtin(StandardIteratorFamily::Range) => {
            Ok(RuntimeIteratorEvidence::builtin_range())
        }
        ForIterationEvidenceFamily::Builtin(StandardIteratorFamily::Seq) => {
            Ok(RuntimeIteratorEvidence::builtin_seq())
        }
        ForIterationEvidenceFamily::Builtin(StandardIteratorFamily::Stream) => {
            Ok(RuntimeIteratorEvidence::builtin_stream())
        }
        ForIterationEvidenceFamily::Builtin(StandardIteratorFamily::Vec) => {
            Ok(RuntimeIteratorEvidence::builtin_vec())
        }
        ForIterationEvidenceFamily::Builtin(StandardIteratorFamily::Array) => {
            Ok(RuntimeIteratorEvidence::builtin_array())
        }
        ForIterationEvidenceFamily::Builtin(StandardIteratorFamily::Slice) => {
            Ok(RuntimeIteratorEvidence::builtin_slice())
        }
        ForIterationEvidenceFamily::Witness {
            into_iterator,
            iterator,
        } => runtime_witness_evidence(
            format!("{:?}", evidence.item_ty),
            format!("{:?}", evidence.into_iter_ty),
            trait_methods,
            into_iterator,
            iterator,
        )
        .map(RuntimeIteratorEvidence::Witness)
        .ok_or_else(|| {
            vec![arcweft_runtime_plan::errors::RuntimePlanLowerError::new(
                "missing executable trait method body for IntoIterator/Iterator witness",
            )]
        }),
        ForIterationEvidenceFamily::IteratorWitness { iterator } => {
            runtime_iterator_identity_witness_evidence(
                format!("{:?}", evidence.item_ty),
                format!("{:?}", evidence.into_iter_ty),
                trait_methods,
                iterator,
            )
            .map(RuntimeIteratorEvidence::Witness)
            .ok_or_else(|| {
                vec![arcweft_runtime_plan::errors::RuntimePlanLowerError::new(
                    "missing executable trait method body for Iterator identity witness",
                )]
            })
        }
        ForIterationEvidenceFamily::WitnessUnsupported { ref reason } => Err(vec![
            arcweft_runtime_plan::errors::RuntimePlanLowerError::new(format!(
                "unsupported IntoIterator witness dispatch: {reason}"
            )),
        ]),
    }
}

/// Lowers pure helper candidates from checked HIR.
pub fn lower_source_pure_helper_candidates(
    hir: &HirModule,
) -> Result<PureHelperCandidateReport, Vec<PureHelperLowerError>> {
    lower_pure_helper_candidates(hir)
}

/// Lowers one checked pure function into a runtime helper candidate.
pub fn lower_source_pure_helper_candidate(
    function: &HirFunction,
    origin: RuntimePureHelperOrigin,
) -> Result<PureHelperCandidate, PureHelperLowerError> {
    lower_pure_helper_candidate(function, origin)
}

/// Lowers checked HIR functions annotated for native text shader/effect/motion registries.
pub fn lower_source_text_pure_helper_candidates(
    hir: &HirModule,
) -> Result<TextPureHelperCandidateReport, Vec<TextPureHelperCandidateError>> {
    let mut report = TextPureHelperCandidateReport::default();
    let mut errors = Vec::new();
    for function in hir.functions() {
        for kind in TextPureHelperKind::from_function(function) {
            if !function.has_attribute("pure") {
                errors.push(TextPureHelperCandidateError::MissingPureAttribute {
                    kind,
                    name: function.name().to_owned(),
                });
                continue;
            }
            match lower_source_pure_helper_candidate(function, RuntimePureHelperOrigin::Annotated) {
                Ok(candidate) => report.push(kind, candidate),
                Err(source) => {
                    errors.push(TextPureHelperCandidateError::PureLower { kind, source });
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(report)
    } else {
        Err(errors)
    }
}
impl TextPureHelperKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Shader => "shader",
            Self::Effect => "effect",
            Self::Motion => "motion",
        }
    }

    fn from_function(function: &HirFunction) -> impl Iterator<Item = Self> + '_ {
        [
            (
                Self::Shader,
                function.has_attribute("text_shader") || function.has_attribute("rich_text_shader"),
            ),
            (
                Self::Effect,
                function.has_attribute("text_effect") || function.has_attribute("rich_text_effect"),
            ),
            (
                Self::Motion,
                function.has_attribute("text_motion") || function.has_attribute("rich_text_motion"),
            ),
        ]
        .into_iter()
        .filter_map(|(kind, selected)| selected.then_some(kind))
    }
}

impl fmt::Display for TextPureHelperKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TextPureHelperCandidateReport {
    fn push(&mut self, kind: TextPureHelperKind, candidate: PureHelperCandidate) {
        match kind {
            TextPureHelperKind::Shader => self.shaders.push(candidate),
            TextPureHelperKind::Effect => self.effects.push(candidate),
            TextPureHelperKind::Motion => self.motions.push(candidate),
        }
    }
}
