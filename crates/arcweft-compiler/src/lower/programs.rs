//! One compiler projection from an admitted root's complete runtime signature.

use std::sync::Arc;

use arcweft_lang_hir::symbol::ProjectSymbolTable;
use arcweft_lang_sema::{
    final_analysis::FinalSemanticAnalysis, registration::RegisteredSemanticWorld,
};
use arcweft_runtime_plan::semantic_facts::RuntimePureProgramFact;

use super::{RuntimeSemanticProjectionError, runtime_type};

pub(crate) fn project(
    program: arcweft_id::runtime_program::RuntimePureProgramId,
    admission: Arc<arcweft_lang_sema::final_analysis::CheckedDeterministicProgram>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimePureProgramFact, RuntimeSemanticProjectionError> {
    let abi = admission.input_abi();
    abi.validate_analysis(analysis).map_err(Box::new)?;
    let inputs = abi
        .inputs()
        .iter()
        .filter(|input| {
            matches!(
                input.role(),
                arcweft_lang_sema::final_analysis::CheckedExecutionInputRole::Free
            )
        })
        .map(|input| input.binding().ty())
        .chain(abi.parameters().iter().map(|parameter| parameter.ty()))
        .map(|ty| runtime_type(ty, symbols, world, analysis))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let result = runtime_type(
        abi.result()
            .value_type()
            .ok_or_else(|| RuntimeSemanticProjectionError::Type {
                reason: "runtime program requires a value result".to_owned(),
            })?,
        symbols,
        world,
        analysis,
    )?;
    Ok(RuntimePureProgramFact::try_new(
        program, admission, inputs, result,
    )?)
}

#[cfg(test)]
mod tests {
    use super::{RuntimePureProgramFact, RuntimeSemanticProjectionError, project, runtime_type};
    use crate::lower::{
        ProjectInstantiationControl, RuntimeEmissionMode, project_program_reachability,
        project_runtime_reachability, project_runtime_semantic_facts_with_programs_and_fx,
    };
    use arcweft_core::value::{RuntimeSignedIntWidth, RuntimeValue};
    use arcweft_lang_sema::final_analysis::{CheckedExecutionBodyOwner, CheckedExecutionSource};
    use arcweft_lang_sema::{final_analysis::CheckedExpressionResolution, types::TypeKind};
    use arcweft_runtime_plan::semantic_facts::RuntimeTypeShape;
    use std::sync::Arc;

    fn id() -> arcweft_id::runtime_program::RuntimePureProgramId {
        arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([71; 32])
    }

    fn lower_program(
        compiled: &crate::types::CompiledSource,
        fact: RuntimePureProgramFact,
    ) -> arcweft_runtime_plan::flow::RuntimePlanLowerReport {
        let lease = &compiled.analysis;
        let analysis = lease.final_analysis();
        let hir = lease.hir_project().analysis_view().unwrap();
        let ordinary = project_runtime_reachability(
            hir,
            lease.project_symbols(),
            analysis,
            analysis.checked_entries(),
            RuntimeEmissionMode::CheckAll,
        )
        .unwrap();
        let roots = project_program_reachability(
            hir,
            lease.project_symbols(),
            analysis,
            [fact.admission()],
        )
        .unwrap();
        let fx = crate::fx_catalog::CompiledFxCatalog::lower(analysis).unwrap();
        let facts = project_runtime_semantic_facts_with_programs_and_fx(
            hir,
            lease.project_symbols(),
            lease.registered_world(),
            analysis,
            &ordinary,
            &roots,
            &[fact],
            None,
            &fx,
            &ProjectInstantiationControl::default(),
        )
        .unwrap();
        arcweft_runtime_plan::flow::lower_runtime_plan_with_stats(
            hir,
            &facts,
            &arcweft_runtime_plan::flow::RuntimeEntryLoweringInput::new(
                hir,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
        )
        .unwrap()
    }

    #[test]
    fn declared_program_executes_full_formal_inputs_through_native_and_awbc() {
        let compiled = crate::source::compile_source(
            "pub fn root(value: i64, unused: i64) -> i64 { value + 1i64 }\nflow main() -> String { return \"ok\" }\n",
        ).unwrap();
        let lease = &compiled.analysis;
        let analysis = lease.final_analysis();
        let declaration = analysis
            .hir_topology()
            .modules()
            .iter()
            .flat_map(|module| module.entries())
            .filter_map(|entry| entry.body())
            .find(|body| body.declaration().name() == "root")
            .unwrap()
            .declaration()
            .clone();
        let source = CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
            declaration,
            role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::FunctionBody,
        });
        let context = analysis
            .checked_execution_context(
                lease.hir_project().analysis_view().unwrap(),
                lease.project_symbols(),
                source.clone(),
                None,
            )
            .unwrap();
        let fact = project(
            id(),
            Arc::new(context.checked_deterministic_program(source).unwrap()),
            lease.project_symbols(),
            lease.registered_world(),
            analysis,
        )
        .unwrap();
        let args = [
            RuntimeValue::Int(arcweft_core::value::RuntimeInt::I64(41)),
            RuntimeValue::Int(arcweft_core::value::RuntimeInt::I64(999)),
        ];
        let expected = RuntimeValue::Int(arcweft_core::value::RuntimeInt::I64(42));
        assert_program_execution(&compiled, fact, &args, expected);
    }

    fn assert_program_execution(
        compiled: &crate::types::CompiledSource,
        fact: RuntimePureProgramFact,
        args: &[RuntimeValue],
        expected: RuntimeValue,
    ) {
        let report = lower_program(compiled, fact);
        let plan = Arc::new(report.plan);
        let mut native = arcweft_core::pure::VmRuntimePureCallBackend::default();
        assert_eq!(
            arcweft_core::pure::evaluate_pure_program_with_backend(&plan, id(), args, &mut native)
                .unwrap(),
            expected
        );
        let awbc = Arc::new(
            arcweft_runtime_plan::awbc_lower::AwbcLowerer::new(
                &plan,
                &report.dialogue_content_catalog,
                "program-formals.arcw",
            )
            .lower()
            .unwrap()
            .program,
        );
        let mut product = arcweft_core::pure::VmRuntimePureCallBackend::default();
        assert_eq!(
            arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
                &awbc,
                id(),
                args,
                &mut product
            )
            .unwrap(),
            expected
        );
    }

    #[test]
    fn closed_program_selects_exact_generic_instance_catalog_and_frame() {
        use arcweft_lang_sema::final_analysis::CheckedLocalUseInstantiation;
        let compiled = crate::source::compile_source(
            "fn identity<T>(value: T) -> T { value }\nflow main() -> String { let signed = identity(1i64); let unsigned = identity(2u64); return \"ok\" }\n",
        ).unwrap();
        let lease = &compiled.analysis;
        let analysis = lease.final_analysis();
        let declaration = analysis
            .hir_topology()
            .modules()
            .iter()
            .flat_map(|module| module.entries())
            .filter_map(|entry| entry.body())
            .find(|body| body.declaration().name() == "identity")
            .unwrap()
            .declaration()
            .clone();
        let source = CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
            declaration: declaration.clone(),
            role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::FunctionBody,
        });
        let instances = analysis
            .calls()
            .filter_map(|(owner, call)| {
                let application = call.selected_application()?;
                let arcweft_lang_sema::callable::CallableCandidateId::Project(selected) =
                    application.core().candidates().selected().id()
                else {
                    return None;
                };
                (selected == &declaration).then(|| {
                    analysis
                        .project_function_runtime(owner)
                        .unwrap()
                        .unwrap()
                        .close_instance(None)
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(instances.len(), 2);
        for instance in &instances {
            let context = analysis
                .checked_execution_context(
                    lease.hir_project().analysis_view().unwrap(),
                    lease.project_symbols(),
                    source.clone(),
                    Some(CheckedLocalUseInstantiation::ProjectFunction(instance)),
                )
                .unwrap();
            let fact = project(
                id(),
                Arc::new(
                    context
                        .checked_deterministic_program(source.clone())
                        .unwrap(),
                ),
                lease.project_symbols(),
                lease.registered_world(),
                analysis,
            )
            .unwrap();
            let value = match fact.parameters().next().unwrap().0.ty() {
                TypeKind::I64 => RuntimeValue::Int(arcweft_core::value::RuntimeInt::I64(42)),
                TypeKind::U64 => RuntimeValue::u64(42),
                other => panic!("unexpected closed type {other:?}"),
            };
            assert_program_execution(&compiled, fact, &[value.clone()], value);
        }
    }

    #[test]
    fn declared_program_retains_destructuring_unused_formals_and_exact_signature() {
        let compiled = crate::source::compile_source(
            "pub fn root((left, right): (i64, i64), unused: i64) -> i64 { left + right }\nflow main() -> String { return \"ok\" }\n",
        ).unwrap();
        let lease = &compiled.analysis;
        let analysis = lease.final_analysis();
        let declaration = analysis
            .hir_topology()
            .modules()
            .iter()
            .flat_map(|module| module.entries())
            .filter_map(|entry| entry.body())
            .find(|body| body.declaration().name() == "root")
            .unwrap()
            .declaration()
            .clone();
        let source = CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
            declaration,
            role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::FunctionBody,
        });
        let context = analysis
            .checked_execution_context(
                lease.hir_project().analysis_view().unwrap(),
                lease.project_symbols(),
                source.clone(),
                None,
            )
            .unwrap();
        let admission = Arc::new(context.checked_deterministic_program(source).unwrap());
        let fact = project(
            id(),
            Arc::clone(&admission),
            lease.project_symbols(),
            lease.registered_world(),
            analysis,
        )
        .unwrap();
        assert_eq!(fact.free_inputs().count(), 0);
        assert_eq!(fact.parameters().count(), 2);
        assert_eq!(fact.parameters().next().unwrap().0.bindings().len(), 2);
        assert_eq!(fact.parameters().nth(1).unwrap().0.bindings().len(), 1);
        assert!(
            matches!(fact.input_types()[0].shape(), RuntimeTypeShape::Tuple(items) if items.len() == 2)
        );
        assert!(
            RuntimePureProgramFact::try_new(
                id(),
                Arc::clone(&admission),
                Box::new([fact.input_types()[0].clone()]),
                fact.result().clone()
            )
            .is_err()
        );
        let boolean = runtime_type(
            &TypeKind::Bool,
            lease.project_symbols(),
            lease.registered_world(),
            analysis,
        )
        .unwrap();
        assert!(
            RuntimePureProgramFact::try_new(
                id(),
                admission,
                fact.input_types().to_vec().into_boxed_slice(),
                boolean
            )
            .is_err()
        );
        assert_program_execution(
            &compiled,
            fact,
            &[
                RuntimeValue::Tuple(vec![RuntimeValue::i64(20), RuntimeValue::i64(22)]),
                RuntimeValue::i64(999),
            ],
            RuntimeValue::i64(42),
        );
    }

    #[test]
    fn callable_creation_and_invocation_keep_different_program_abis() {
        let compiled = crate::source::compile_source(
            "flow main() -> String { let produce = |value: i64| value + 1i64; return \"ok\" }\n",
        )
        .unwrap();
        let lease = &compiled.analysis;
        let analysis = lease.final_analysis();
        let closure = analysis
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(
                    expression.resolution(),
                    CheckedExpressionResolution::Closure(_)
                )
                .then_some(owner)
            })
            .unwrap();
        let mut facts = Vec::new();
        for source in [
            CheckedExecutionSource::EvaluateValue(closure),
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::CallableValue(closure)),
        ] {
            let context = analysis
                .checked_execution_context(
                    lease.hir_project().analysis_view().unwrap(),
                    lease.project_symbols(),
                    source.clone(),
                    None,
                )
                .unwrap();
            let admission = Arc::new(context.checked_deterministic_program(source).unwrap());
            facts.push(
                project(
                    id(),
                    admission,
                    lease.project_symbols(),
                    lease.registered_world(),
                    analysis,
                )
                .unwrap(),
            );
        }
        assert!(facts[0].input_types().is_empty());
        assert!(matches!(
            facts[0].result().shape(),
            RuntimeTypeShape::Function { .. }
        ));
        assert_eq!(facts[1].parameters().count(), 1);
        assert!(matches!(
            facts[1].result().shape(),
            RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I64)
        ));
        assert_ne!(facts[0].reachability_owner(), facts[1].reachability_owner());
    }

    #[test]
    fn implicit_body_uses_its_checked_synthetic_formal_through_both_engines() {
        let compiled = crate::source::compile_source(
            "flow main() -> String { let callback: (i64) -> i64 effects {} = _ + 1i64; return \"ok\" }\n",
        )
        .unwrap();
        let lease = &compiled.analysis;
        let analysis = lease.final_analysis();
        let root = analysis
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(
                    expression.resolution(),
                    CheckedExpressionResolution::ImplicitCallable(_)
                )
                .then_some(owner)
            })
            .unwrap();
        let source =
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::CallableValue(root));
        let context = analysis
            .checked_execution_context(
                lease.hir_project().analysis_view().unwrap(),
                lease.project_symbols(),
                source.clone(),
                None,
            )
            .unwrap();
        let fact = project(
            id(),
            Arc::new(context.checked_deterministic_program(source).unwrap()),
            lease.project_symbols(),
            lease.registered_world(),
            analysis,
        )
        .unwrap();
        assert_eq!(fact.parameters().count(), 1);
        assert!(fact.parameters().next().unwrap().0.pattern().is_none());
        assert_program_execution(
            &compiled,
            fact,
            &[RuntimeValue::i64(41)],
            RuntimeValue::i64(42),
        );
    }

    #[test]
    fn program_projection_rejects_a_foreign_report_even_with_identical_types() {
        let source = "flow main() -> String { return \"ok\" }\n";
        let first = crate::source::compile_source(source).unwrap();
        let second = crate::source::compile_source(source).unwrap();
        let lease = &first.analysis;
        let analysis = lease.final_analysis();
        let root = analysis
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(expression.value_type(), Some(TypeKind::String)).then_some(owner)
            })
            .unwrap();
        let context = analysis
            .checked_execution_context(
                lease.hir_project().analysis_view().unwrap(),
                lease.project_symbols(),
                root,
                None,
            )
            .unwrap();
        let admission = Arc::new(context.checked_deterministic_program(root).unwrap());
        let foreign = &second.analysis;
        assert!(matches!(
            project(id(), admission, foreign.project_symbols(), foreign.registered_world(), foreign.final_analysis()),
            Err(RuntimeSemanticProjectionError::ExecutionContext(error))
                if matches!(*error, arcweft_lang_sema::final_analysis::CheckedExecutionContextError::ForeignAuthority)
        ));
    }
}
