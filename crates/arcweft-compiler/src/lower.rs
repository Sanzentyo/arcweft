use arcweft_core::plan::{RuntimeIteratorEvidence, RuntimePlan, RuntimePureHelperOrigin};
use arcweft_lang_hir::model::{HirFunction, HirModule};
use arcweft_lang_sema::check::{
    ForIterationEvidence, ForIterationEvidenceFamily, StandardIteratorFamily, TypeCheckReport,
    TypedLoweringEvidence, TypedLoweringEvidenceKind,
};
use arcweft_runtime_plan::flow::{
    RuntimeClosureCapture, RuntimeClosureCaptureInventory, RuntimePlanLowerOptions,
    RuntimePlanLowerReport, lower_runtime_plan_with_options,
    lower_runtime_plan_with_stats_and_options,
};
use arcweft_runtime_plan::line_task::{LoweredLineTaskGroup, lower_line_task_groups};
use arcweft_runtime_plan::pure::{
    PureHelperCandidate, PureHelperCandidateReport, PureHelperLowerError,
    lower_pure_helper_candidate, lower_pure_helper_candidates,
};
use arcweft_runtime_plan::typed_evidence::{
    RuntimeDataLastMethodFallbackArg, RuntimeNumericType, RuntimeTypedExpressionId,
    RuntimeTypedLoweringEvidence, RuntimeTypedLoweringEvidenceKind,
    RuntimeTypedLoweringEvidenceOwner,
};

use crate::trait_methods::{
    lower_runtime_trait_methods_from_typecheck, runtime_iterator_identity_witness_evidence,
    runtime_witness_evidence,
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
    let closure_captures = typecheck
        .closure_captures
        .iter()
        .map(runtime_closure_capture_inventory)
        .collect::<Vec<_>>();
    let required_typed_lowering_evidence_len = typed_lowering_evidence.len();
    Ok(options
        .clone()
        .with_for_iteration_evidence(evidence)
        .with_trait_methods(trait_methods.methods)
        .with_typed_lowering_evidence(typed_lowering_evidence)
        .with_closure_capture_metadata(closure_captures)
        .with_required_typed_lowering_evidence_len(required_typed_lowering_evidence_len))
}

fn runtime_closure_capture_inventory(
    inventory: &arcweft_lang_sema::check::ClosureCaptureInventory,
) -> RuntimeClosureCaptureInventory {
    RuntimeClosureCaptureInventory {
        expression_id: RuntimeTypedExpressionId::from_index(inventory.expression_id.index()),
        captures: inventory
            .captures
            .iter()
            .map(|capture| RuntimeClosureCapture {
                name: capture.name.clone(),
                type_label: capture.ty.source_label(),
            })
            .collect(),
    }
}

fn runtime_typed_lowering_evidence(
    evidence: &TypedLoweringEvidence,
) -> RuntimeTypedLoweringEvidence {
    RuntimeTypedLoweringEvidence {
        expression_id: RuntimeTypedExpressionId::from_index(evidence.expression_id.index()),
        owner: evidence
            .owner
            .as_ref()
            .map(|owner| RuntimeTypedLoweringEvidenceOwner {
                declaration: owner.declaration.clone(),
                expression_id: RuntimeTypedExpressionId::from_index(owner.expression_id.index()),
            }),
        kind: match &evidence.kind {
            TypedLoweringEvidenceKind::ResolvedNumericType { target } => {
                RuntimeTypedLoweringEvidenceKind::ResolvedNumericType {
                    target: checked_numeric_primitive(target),
                }
            }
            TypedLoweringEvidenceKind::FunctionValueCall {
                callee,
                arg_count,
                partial,
                ..
            } => RuntimeTypedLoweringEvidenceKind::FunctionValueCall {
                callee: callee.clone(),
                arg_count: *arg_count,
                partial: *partial,
            },
            TypedLoweringEvidenceKind::ExpectedFunctionValue { arity, .. } => {
                RuntimeTypedLoweringEvidenceKind::ExpectedFunctionValue { arity: *arity }
            }
            TypedLoweringEvidenceKind::FunctionValueReference { callee, .. } => {
                RuntimeTypedLoweringEvidenceKind::FunctionValueReference {
                    callee: callee.clone(),
                }
            }
            TypedLoweringEvidenceKind::SignaturePartialCall {
                callee, arg_count, ..
            } => RuntimeTypedLoweringEvidenceKind::SignaturePartialCall {
                callee: callee.clone(),
                arg_count: *arg_count,
            },
            TypedLoweringEvidenceKind::FunctionEffectCallable { callable } => {
                RuntimeTypedLoweringEvidenceKind::FunctionEffectCallable {
                    callable: callable.as_str().to_owned(),
                }
            }
            TypedLoweringEvidenceKind::DataLastMethodFallback {
                method,
                arg_count,
                arg_order,
            } => RuntimeTypedLoweringEvidenceKind::DataLastMethodFallback {
                method: method.clone(),
                arg_count: *arg_count,
                arg_order: arg_order
                    .iter()
                    .map(|arg| match arg {
                        arcweft_lang_sema::check::DataLastMethodFallbackArg::CallArg { index } => {
                            RuntimeDataLastMethodFallbackArg::CallArg { index: *index }
                        }
                        arcweft_lang_sema::check::DataLastMethodFallbackArg::Receiver => {
                            RuntimeDataLastMethodFallbackArg::Receiver
                        }
                    })
                    .collect(),
            },
        },
    }
}

fn checked_numeric_primitive(target: &arcweft_lang_sema::types::TypeKind) -> RuntimeNumericType {
    use arcweft_lang_sema::types::TypeKind;
    match target {
        TypeKind::I8 => RuntimeNumericType::I8,
        TypeKind::I16 => RuntimeNumericType::I16,
        TypeKind::I32 => RuntimeNumericType::I32,
        TypeKind::I64 => RuntimeNumericType::I64,
        TypeKind::I128 => RuntimeNumericType::I128,
        TypeKind::ISize => RuntimeNumericType::ISize,
        TypeKind::U8 => RuntimeNumericType::U8,
        TypeKind::U16 => RuntimeNumericType::U16,
        TypeKind::U32 => RuntimeNumericType::U32,
        TypeKind::U64 => RuntimeNumericType::U64,
        TypeKind::U128 => RuntimeNumericType::U128,
        TypeKind::USize => RuntimeNumericType::USize,
        TypeKind::F32 => RuntimeNumericType::F32,
        TypeKind::F64 => RuntimeNumericType::F64,
        _ => unreachable!("semantic numeric evidence must name a numeric primitive"),
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
