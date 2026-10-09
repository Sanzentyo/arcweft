//! One compiler projection from an admitted root's complete runtime signature.

use std::sync::Arc;

use arcweft_lang_hir::{
    project::{HirAnalysisProjectView, HirRuntimeSemanticReachability},
    symbol::ProjectSymbolTable,
};
use arcweft_lang_sema::{
    final_analysis::FinalSemanticAnalysis, registration::RegisteredSemanticWorld,
};
use arcweft_runtime_plan::semantic_facts::{
    RuntimeProjectFunctionInstanceSemanticFacts, RuntimePureProgramFact,
};

use super::{
    DiscoveredProjectInstances, ProjectInstanceProjection, ProjectInstantiationOrigin,
    RuntimeDialogueProjectionCatalog, RuntimeExecutableInstantiation,
    RuntimeSemanticProjectionError, runtime_executable_semantic_facts,
};

#[allow(
    clippy::too_many_arguments,
    reason = "program catalogs use the same authenticated project, semantic world, frozen environment and discovery ledger as ordinary executable catalogs"
)]
pub(super) fn materialize(
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
    owners: &HirRuntimeSemanticReachability<'_>,
    programs: &[RuntimePureProgramFact],
    dialogue: &RuntimeDialogueProjectionCatalog,
    instances: &DiscoveredProjectInstances,
) -> Result<
    Vec<(
        arcweft_id::runtime_program::RuntimePureProgramId,
        RuntimeProjectFunctionInstanceSemanticFacts,
    )>,
    RuntimeSemanticProjectionError,
> {
    let mut projection = ProjectInstanceProjection::Materialize {
        graph: instances,
        caller: None,
    };
    programs
        .iter()
        .map(|program| {
            let origin = ProjectInstantiationOrigin::Program(program.program());
            let environment = program.admission().input_abi().environment();
            environment.validate_analysis(analysis).map_err(Box::new)?;
            environment.validate_project(project).map_err(Box::new)?;
            let partition = analysis
                .execution_projection()
                .runtime_program_fact_partition(owners, program.admission())?;
            let lexical = RuntimeExecutableInstantiation::Program {
                program: program.program(),
                environment,
                types: instances.environment_types(origin, environment),
            };
            let semantics = runtime_executable_semantic_facts(
                origin,
                lexical,
                partition,
                [],
                project,
                symbols,
                world,
                analysis,
                owners,
                dialogue,
                &mut projection,
            )?;
            Ok((program.program(), semantics))
        })
        .collect()
}

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
        .map(|ty| {
            super::runtime_type_scoped_at(
                ty,
                symbols,
                world,
                analysis,
                &super::RuntimeTypeProjectionPath::root(),
                &abi.environment().type_scope(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let result = super::runtime_type_scoped_at(
        abi.result()
            .value_type()
            .ok_or_else(|| RuntimeSemanticProjectionError::Type {
                reason: "runtime program requires a value result".to_owned(),
            })?,
        symbols,
        world,
        analysis,
        &super::RuntimeTypeProjectionPath::root(),
        &abi.environment().type_scope(),
    )?;
    Ok(RuntimePureProgramFact::try_new(
        program,
        Arc::clone(&admission),
        inputs,
        result,
        super::runtime_type(
            &abi.function_type()
                .ok_or_else(|| RuntimeSemanticProjectionError::Type {
                    reason: "program ABI has no function contract".to_owned(),
                })?,
            symbols,
            world,
            analysis,
        )?,
    )?)
}

// Public owner seam for compiling a selected accepted deterministic body.
// The selected root uses the same checked context, root reachability and
// instance materializer as the other accepted deterministic programs.

use arcweft_core::plan::RuntimePlan;
use arcweft_id::runtime_program::RuntimePureProgramId;
use arcweft_lang_sema::final_analysis::{
    CheckedExecutionSource, CheckedLocalUseInstanceIdentity, CheckedLocalUseInstantiation,
};
use arcweft_runtime_plan::flow::RuntimePlanLowerStats;
use thiserror::Error;

/// One selected root and its immutable, complete plan. Persisted artifact
/// binding remains with the transport that emits the canonical artifact.
#[derive(Clone, Debug)]
pub struct CompiledDeterministicProgram {
    plan: Arc<RuntimePlan>,
    program: RuntimePureProgramId,
    report: Arc<arcweft_runtime_plan::flow::RuntimePlanLowerReport>,
}

impl CompiledDeterministicProgram {
    pub const fn plan(&self) -> &Arc<RuntimePlan> {
        &self.plan
    }

    pub const fn program(&self) -> RuntimePureProgramId {
        self.program
    }

    pub fn stats(&self) -> &RuntimePlanLowerStats {
        &self.report.stats
    }

    /// Retains the same lowering generation's Content, presentation and
    /// assertion products required by bytecode and persisted consumers.
    pub const fn lowering_report(
        &self,
    ) -> &Arc<arcweft_runtime_plan::flow::RuntimePlanLowerReport> {
        &self.report
    }

    pub fn function_site(
        &self,
    ) -> Result<
        arcweft_core::runtime_id::RuntimeFunctionSiteId,
        arcweft_core::plan::RuntimePureProgramLookupError,
    > {
        self.plan
            .pure_program_binding(self.program)
            .map(|binding| binding.site())
    }
}

#[derive(Debug, Error)]
pub enum DeterministicProgramCompileError {
    #[error(transparent)]
    Hir(#[from] arcweft_lang_hir::project::HirProjectAnalysisError),
    #[error(transparent)]
    Context(#[from] Box<arcweft_lang_sema::final_analysis::CheckedExecutionContextError>),
    #[error(transparent)]
    Admission(#[from] arcweft_lang_sema::final_analysis::CheckedProgramAdmissionError),
    #[error(transparent)]
    Reachability(#[from] Box<super::RuntimeReachabilityProjectionError>),
    #[error(transparent)]
    Projection(#[from] Box<super::RuntimeSemanticProjectionError>),
    #[error("selected deterministic program Fx catalog failed: {reason}")]
    Fx { reason: String },
    #[error("selected deterministic program lowering failed: {diagnostics:?}")]
    RuntimeLower {
        diagnostics: Box<[arcweft_runtime_plan::errors::RuntimePlanLowerError]>,
    },
}

pub fn compile_deterministic_program(
    lease: &crate::project::ProjectAnalysisLease,
    source: CheckedExecutionSource,
    instance: Option<CheckedLocalUseInstantiation<'_>>,
    control: &super::ProjectInstantiationControl,
) -> Result<CompiledDeterministicProgram, DeterministicProgramCompileError> {
    let analysis = lease.final_analysis();
    let hir = lease.hir_project().analysis_view()?;
    let context = analysis
        .checked_execution_context(hir, lease.project_symbols(), source.clone(), instance)
        .map_err(Box::new)?;
    let admission = Arc::new(context.checked_deterministic_program(source)?);
    let abi = admission.input_abi();
    let mut hash = blake3::Hasher::new();
    hash.update(b"arcweft.compiler.selected-deterministic-program.v1\0");
    hash.update(lease.program_hash().as_str().as_bytes());
    hash.update(abi.definition_identity().runtime_identity().as_bytes());
    match abi.instance_identity() {
        None => {
            hash.update(&[0]);
        }
        Some(CheckedLocalUseInstanceIdentity::ProjectFunction { instantiation, .. }) => {
            hash.update(&[1]);
            hash.update(instantiation.bytes());
        }
        Some(CheckedLocalUseInstanceIdentity::DisplayText { self_type, .. }) => {
            hash.update(&[2]);
            hash.update(self_type.as_bytes());
        }
    }
    let program = RuntimePureProgramId::from_checked_digest(*hash.finalize().as_bytes());
    let fact = project(
        program,
        admission,
        lease.project_symbols(),
        lease.registered_world(),
        analysis,
    )
    .map_err(Box::new)?;
    // No ordinary Flow/Entry root is selected. These empty roots and the
    // actual program roots are both authenticated by the existing owner API.
    let ordinary = super::project_program_reachability(
        hir,
        lease.project_symbols(),
        analysis,
        std::iter::empty::<&arcweft_lang_sema::final_analysis::CheckedDeterministicProgram>(),
    )
    .map_err(Box::new)?;
    let roots = super::project_program_reachability(
        hir,
        lease.project_symbols(),
        analysis,
        [fact.admission()],
    )
    .map_err(Box::new)?;
    let fx = crate::fx_catalog::CompiledFxCatalog::lower(analysis).map_err(|error| {
        DeterministicProgramCompileError::Fx {
            reason: error.to_string(),
        }
    })?;
    let facts = super::project_runtime_semantic_facts_with_programs_and_fx(
        hir,
        lease.project_symbols(),
        lease.registered_world(),
        analysis,
        &ordinary,
        &roots,
        &[fact],
        None,
        &fx,
        control,
    )
    .map_err(Box::new)?;
    let lowered = arcweft_runtime_plan::flow::lower_runtime_plan_with_stats(
        hir,
        &facts,
        &arcweft_runtime_plan::flow::RuntimeEntryLoweringInput::new(
            hir,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
    )
    .map_err(
        |diagnostics| DeterministicProgramCompileError::RuntimeLower {
            diagnostics: diagnostics.into_boxed_slice(),
        },
    )?;
    Ok(CompiledDeterministicProgram {
        plan: Arc::new(lowered.plan.clone()),
        program,
        report: Arc::new(lowered),
    })
}

#[cfg(test)]
mod tests {
    use super::{RuntimePureProgramFact, RuntimeSemanticProjectionError, project};
    use crate::lower::runtime_type;
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
    fn binding_program_exports_selected_pattern_locals_through_native_and_awbc() {
        for (source, args, expected, output_count) in [
            (
                "fn evidence(value: (String, i64)) { let (label, count) = value; }\nflow main() -> String { return \"ok\" }\n",
                vec![RuntimeValue::Tuple(vec![
                    RuntimeValue::String("seed".to_owned()),
                    RuntimeValue::i64(42),
                ])],
                RuntimeValue::Tuple(vec![
                    RuntimeValue::String("seed".to_owned()),
                    RuntimeValue::i64(42),
                ]),
                2,
            ),
            (
                "struct Row { label: String, count: i64 }\nfn evidence(value: i64) { let Row { label, count } = Row { label: \"seed\", count: value }; }\nflow main() -> String { return \"ok\" }\n",
                vec![RuntimeValue::i64(42)],
                RuntimeValue::Tuple(vec![
                    RuntimeValue::String("seed".to_owned()),
                    RuntimeValue::i64(42),
                ]),
                2,
            ),
            (
                "fn evidence(value: i64) { let _ = value; }\nflow main() -> String { return \"ok\" }\n",
                vec![RuntimeValue::i64(42)],
                RuntimeValue::Unit,
                0,
            ),
        ] {
            let compiled = crate::source::compile_source(source).unwrap();
            let lease = &compiled.analysis;
            let analysis = lease.final_analysis();
            let statement = lease
                .hir_project()
                .analysis_view()
                .unwrap()
                .modules()
                .flat_map(|(_, module)| module.statements())
                .find_map(|(owner, statement)| {
                    matches!(
                        statement.kind(),
                        arcweft_lang_hir::stmt::HirStmtKind::Let { .. }
                    )
                    .then_some(owner)
                })
                .unwrap();
            let source = CheckedExecutionSource::ExportBinding(statement);
            let context = analysis
                .checked_execution_context(
                    lease.hir_project().analysis_view().unwrap(),
                    lease.project_symbols(),
                    source.clone(),
                    None,
                )
                .unwrap();
            let admission = Arc::new(context.checked_deterministic_program(source).unwrap());
            assert_eq!(admission.input_abi().binding_outputs().len(), output_count);
            assert_eq!(admission.input_abi().inputs().len(), 1);
            let fact = project(
                id(),
                admission,
                lease.project_symbols(),
                lease.registered_world(),
                analysis,
            )
            .unwrap();
            assert_program_execution(&compiled, fact, &args, expected);
        }
    }

    #[test]
    fn declared_program_executes_full_formal_inputs_through_native_and_awbc() {
        assert_declared_program(
            "pub fn root(value: i64, unused: i64) -> i64 { value + 1i64 }\nflow main() -> String { return \"ok\" }\n",
        );
    }

    #[test]
    fn selected_unused_function_retains_full_formals_in_pure_aot_and_awbc() {
        use arcweft_core::pure::{
            AotPureFunctionBackend, PureFunctionBackend, PureFunctionRequest,
            RuntimePureFunctionRef, VmPureFunctionBackend,
        };

        let compiled = crate::source::compile_source(
            "pub fn root(value: i64, unused: i64) -> i64 { value + 1i64 }\nflow main() -> String { return \"ok\" }\n",
        )
        .unwrap();
        let lease = &compiled.analysis;
        let declaration = lease
            .final_analysis()
            .hir_topology()
            .modules()
            .iter()
            .flat_map(|module| module.entries())
            .filter_map(|entry| entry.body())
            .find(|body| body.declaration().name() == "root")
            .unwrap()
            .declaration()
            .clone();
        let selected = lease
            .compile_deterministic_program(
                CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                    declaration,
                    role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::FunctionBody,
                }),
                None,
                &ProjectInstantiationControl::default(),
            )
            .unwrap();
        let site = selected.function_site().unwrap();
        let function = RuntimePureFunctionRef::resolve(selected.plan(), site).unwrap();
        assert_eq!(function.inputs.len(), 2);
        assert_eq!(
            function.function_site().unwrap().parameter_inputs().count(),
            2
        );
        assert!(selected.plan().pure_helpers().is_empty());
        assert!(
            PureFunctionRequest::try_new(
                Arc::clone(selected.plan()),
                site,
                vec![RuntimeValue::i64(41)],
            )
            .is_err()
        );
        let arguments = [RuntimeValue::i64(41), RuntimeValue::i64(999)];
        let request =
            PureFunctionRequest::try_new(Arc::clone(selected.plan()), site, arguments.to_vec())
                .unwrap();
        assert_eq!(
            VmPureFunctionBackend.evaluate(&request).unwrap().value,
            RuntimeValue::i64(42)
        );
        let locals = function
            .inputs
            .iter()
            .map(|input| input.local())
            .collect::<Vec<_>>();
        let aot = AotPureFunctionBackend::new()
            .compile_i64_with_inputs(&request, locals.iter().copied())
            .unwrap();
        assert_eq!(aot.call_with_inputs(&[41, 999]).unwrap().0, 42);

        let mut plan = selected.plan().as_ref().clone();
        plan.bind_artifact(
            arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([71; 32]).unwrap(),
        )
        .unwrap();
        let plan = Arc::new(plan);
        let mut native = arcweft_core::pure::VmRuntimePureCallBackend::default();
        assert_eq!(
            arcweft_core::pure::evaluate_pure_program_with_backend(
                &plan,
                selected.program(),
                &arguments,
                &mut native,
            )
            .unwrap(),
            RuntimeValue::i64(42)
        );
        let awbc = Arc::new(
            arcweft_runtime_plan::awbc_lower::AwbcLowerer::new(
                &plan,
                &selected.lowering_report().dialogue_content_catalog,
                "selected-unused-formals.arcw",
            )
            .lower()
            .unwrap()
            .program,
        );
        let mut product = arcweft_core::pure::VmRuntimePureCallBackend::default();
        assert_eq!(
            arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
                &awbc,
                selected.program(),
                &arguments,
                &mut product,
            )
            .unwrap(),
            RuntimeValue::i64(42)
        );
    }

    #[test]
    fn declared_program_owns_its_catalog_when_same_body_has_an_ordinary_instance() {
        assert_declared_program(
            "pub fn root(value: i64, unused: i64) -> i64 { value + 1i64 }\nflow main() -> String { let ignored = root(1i64, 2i64); return \"ok\" }\n",
        );
    }

    #[test]
    fn program_nested_closure_uses_the_program_parent_frame() {
        assert_declared_program(
            "pub fn root(value: i64, unused: i64) -> i64 { let add = |item: i64| value + item; add(1i64) }\nflow main() -> String { return \"ok\" }\n",
        );
    }

    #[test]
    fn program_format_call_uses_its_own_template_scope() {
        assert_declared_program(
            "pub fn root(value: i64, unused: i64) -> i64 { let formatted = fmt(value); value + 1i64 }\nflow main() -> String { return \"ok\" }\n",
        );
    }

    #[test]
    fn program_body_retains_all_argument_group_coordinates() {
        assert_declared_program(
            "pub fn root(value: i64)(unused: i64) -> i64 { value + 1i64 }\nflow main() -> String { return \"ok\" }\n",
        );
    }

    #[test]
    fn program_only_format_call_selects_its_closed_display_method() {
        assert_declared_program(
            "struct RouteInfo { value: i64 }\nimpl DisplayText for RouteInfo { fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> { Ok(fmt(self.value)) } }\npub fn root(value: i64, unused: i64) -> i64 { let formatted = fmt(RouteInfo { value }); value + 1i64 }\nflow main() -> String { return \"ok\" }\n",
        );
    }

    fn assert_declared_program(source: &str) {
        let compiled = crate::source::compile_source(source).unwrap();
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
        let mut report = lower_program(compiled, fact);
        report
            .plan
            .bind_artifact(
                arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([71; 32]).unwrap(),
            )
            .unwrap();
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
        assert_closed_program_instances(
            "fn identity<T>(value: T) -> T { value }\nflow main() -> String { let signed = identity(1i64); let unsigned = identity(2u64); return \"ok\" }\n",
            true,
        );
    }

    #[test]
    fn closed_program_materializes_instances_outside_ordinary_call_reachability() {
        assert_closed_program_instances(
            "fn identity<T>(value: T) -> T { value }\nfn evidence() -> String { let signed = identity(1i64); let unsigned = identity(2u64); \"ok\" }\nflow main() -> String { return \"ok\" }\n",
            false,
        );
    }

    #[test]
    fn program_only_generic_calls_discover_and_execute_their_transitive_instances() {
        assert_closed_program_instances(
            "fn leaf<T>(value: T) -> T { value }\nfn identity<T>(value: T) -> T { leaf(value) }\nfn evidence() -> String { let signed = identity(1i64); let unsigned = identity(2u64); \"ok\" }\nflow main() -> String { return \"ok\" }\n",
            false,
        );
    }

    fn assert_closed_program_instances(source: &str, ordinarily_reached: bool) {
        use arcweft_lang_sema::final_analysis::CheckedLocalUseInstantiation;
        let compiled = crate::source::compile_source(source).unwrap();
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
        let hir = lease.hir_project().analysis_view().unwrap();
        let ordinary = project_runtime_reachability(
            hir,
            lease.project_symbols(),
            analysis,
            analysis.checked_entries(),
            RuntimeEmissionMode::CheckAll,
        )
        .unwrap();
        let owner = arcweft_lang_hir::project::HirRuntimeExecutableOwner::Item(
            analysis
                .hir_topology()
                .declaration(&declaration)
                .unwrap()
                .body()
                .source_item(),
        );
        assert_eq!(
            ordinary.executable_owners(&owner).is_some(),
            ordinarily_reached
        );
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
        assert_eq!(
            fact.definition_identity(),
            admission.input_abi().definition_identity()
        );
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
                fact.result().clone(),
                fact.function_type().clone()
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
                boolean,
                fact.function_type().clone()
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
    fn captured_body_uses_program_ingress_instead_of_the_ordinary_parent_frame() {
        let compiled = crate::source::compile_source(
            "flow main() -> String { let base = 1i64; let callback: (i64) -> i64 effects {} = |item: i64| { base + item }; return \"ok\" }\n",
        ).unwrap();
        let lease = &compiled.analysis;
        let analysis = lease.final_analysis();
        let owner = analysis
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(
                    expression.resolution(),
                    CheckedExpressionResolution::Closure(_)
                )
                .then_some(owner)
            })
            .unwrap();
        let source =
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::CallableValue(owner));
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
        assert_eq!(fact.free_inputs().count(), 1);
        assert_program_execution(
            &compiled,
            fact,
            &[RuntimeValue::i64(41), RuntimeValue::i64(1)],
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

    #[test]
    fn authored_scalar_executable_retains_its_function_site_in_vm_aot_and_awbc() {
        use arcweft_core::pure::{
            AotPureFunctionBackend, PureFunctionBackend, PureFunctionRequest,
            RuntimePureFunctionBodyRef, RuntimePureFunctionInputRef, VmPureFunctionBackend,
        };
        let selected = authored_map_program(
            "pub fn root(base: i64, bonus: i64, unused: i64) -> i64 { let weighted: i64 = base * (bonus + 2i64); return if base >= 3i64 { weighted } else { weighted + 1i64 } }\nflow main() -> String { return \"ok\" }\n",
        );
        let site = selected.function_site().unwrap();
        assert!(selected.plan().pure_helpers().is_empty());
        assert_eq!(selected.plan().pure_function_candidate_count(), 1);
        let omitted = PureFunctionRequest::try_new(
            Arc::clone(selected.plan()),
            site,
            [RuntimeValue::i64(3), RuntimeValue::i64(4)],
        );
        assert!(matches!(
            omitted,
            Err(arcweft_core::value::RuntimeEvalError::FunctionApply(_))
                | Err(arcweft_core::value::RuntimeEvalError::TooManyPureArgs { .. })
        ));
        let mut artifact = selected.plan().as_ref().clone();
        artifact
            .bind_artifact(
                arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x76; 32])
                    .unwrap(),
            )
            .unwrap();
        let product = Arc::new(
            arcweft_runtime_plan::awbc_lower::AwbcLowerer::new(
                &artifact,
                &selected.lowering_report().dialogue_content_catalog,
                "scalar-executable.arcw",
            )
            .lower()
            .unwrap()
            .program,
        );
        for (values, expected) in [([3, 4, 99], 18), ([1, 3, 77], 6)] {
            let values = values.map(RuntimeValue::i64);
            let request =
                PureFunctionRequest::try_new(Arc::clone(selected.plan()), site, values.clone())
                    .unwrap();
            let function = request.function_ref().unwrap();
            assert!(matches!(
                function.body,
                RuntimePureFunctionBodyRef::Executable(_)
            ));
            assert!(std::ptr::eq(
                function.function_site().unwrap(),
                selected.plan().function_sites().get(site).unwrap()
            ));
            assert!(Arc::ptr_eq(function.plan(), selected.plan()));
            assert_eq!(function.inputs.len(), 3);
            assert_eq!(
                function.inputs.get(2).unwrap().passing(),
                Some(arcweft_core::plan::RuntimeFunctionParameterPassing::Value)
            );
            let vm = VmPureFunctionBackend
                .evaluate_invocation(
                    &request,
                    arcweft_core::step::RuntimeStepBudget { max_ops: 64 },
                )
                .unwrap();
            assert_eq!(vm.value, RuntimeValue::i64(expected));
            assert!(
                vm.stats.evaluated_exprs > 0,
                "the existing Engine records actual interpreter expressions"
            );
            let aot = AotPureFunctionBackend
                .compile_i64_with_inputs(
                    &request,
                    function
                        .inputs
                        .iter()
                        .map(RuntimePureFunctionInputRef::local),
                )
                .unwrap();
            assert!(
                aot.call_with_inputs(&[3, 4]).is_err(),
                "full unused formal remains in the physical ABI"
            );
            assert_eq!(
                aot.call_with_inputs(
                    &values
                        .iter()
                        .map(|value| match value {
                            RuntimeValue::Int(value) => value.exact_i64().unwrap(),
                            _ => unreachable!(),
                        })
                        .collect::<Vec<_>>()
                )
                .unwrap()
                .0,
                expected
            );
            let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
            assert_eq!(
                arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
                    &product,
                    selected.program(),
                    &values,
                    &mut backend
                )
                .unwrap(),
                RuntimeValue::i64(expected)
            );
        }
    }

    fn authored_map_program(source: &str) -> super::CompiledDeterministicProgram {
        let compiled = crate::source::compile_source(source).unwrap();
        let lease = &compiled.analysis;
        let declaration = lease
            .final_analysis()
            .hir_topology()
            .modules()
            .iter()
            .flat_map(|module| module.entries())
            .filter_map(|entry| entry.body())
            .find(|body| body.declaration().name() == "root")
            .unwrap()
            .declaration()
            .clone();
        lease
            .compile_deterministic_program(
                CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                    declaration,
                    role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::FunctionBody,
                }),
                None,
                &ProjectInstantiationControl::default(),
            )
            .unwrap()
    }

    enum AuthoredMapResultFamily {
        Sequence(arcweft_core::plan::RuntimePlanSequenceKind),
        Array(u64),
        Option,
        Result,
    }

    fn assert_authored_map_family(
        selected: &super::CompiledDeterministicProgram,
        expected: &AuthoredMapResultFamily,
    ) {
        use arcweft_core::plan::RuntimePlanTypeProjection as Projection;
        let result = selected
            .plan()
            .function_sites()
            .get(selected.function_site().unwrap())
            .unwrap()
            .result();
        let actual = selected
            .plan()
            .type_table()
            .get(result)
            .unwrap()
            .projection();
        match (expected, actual) {
            (AuthoredMapResultFamily::Sequence(expected), Projection::Sequence { kind, .. }) => {
                assert_eq!(kind, expected)
            }
            (
                AuthoredMapResultFamily::Array(expected),
                Projection::Array {
                    length: arcweft_core::plan::RuntimeArrayLength::Constant(actual),
                    ..
                },
            ) => assert_eq!(actual, expected),
            (AuthoredMapResultFamily::Option, Projection::Option { .. })
            | (AuthoredMapResultFamily::Result, Projection::Result { .. }) => {}
            _ => panic!("the accepted Map result has a different family: {actual:?}"),
        }
    }

    fn assert_authored_map_value(actual: RuntimeValue, expected: &RuntimeValue) {
        match (actual, expected) {
            (RuntimeValue::Seq(actual), RuntimeValue::Seq(expected)) => {
                assert_eq!(actual.len(), expected.len());
                for ordinal in 0..expected.len() {
                    assert_eq!(actual.value_at(ordinal), expected.value_at(ordinal));
                }
            }
            (actual, expected) => assert_eq!(&actual, expected),
        }
    }

    #[test]
    fn authored_map_callbacks_share_native_and_awbc_function_return_authority() {
        use AuthoredMapResultFamily as Family;
        use arcweft_core::plan::RuntimePlanSequenceKind as Sequence;
        use arcweft_core::value::runtime_sequence_values;
        let sequence = || {
            runtime_sequence_values(vec![
                RuntimeValue::i64(1),
                RuntimeValue::i64(2),
                RuntimeValue::i64(3),
            ])
        };
        let mapped = || {
            runtime_sequence_values(vec![
                RuntimeValue::i64(2),
                RuntimeValue::i64(3),
                RuntimeValue::i64(4),
            ])
        };
        for (ty, result, family, argument, expected) in [
            (
                "Vec<i64>",
                "Vec<i64>",
                Family::Sequence(Sequence::Vec),
                sequence(),
                mapped(),
            ),
            (
                "Seq<i64>",
                "Seq<i64>",
                Family::Sequence(Sequence::Seq),
                sequence(),
                mapped(),
            ),
            (
                "Array<i64, 3>",
                "Array<i64, 3>",
                Family::Array(3),
                sequence(),
                mapped(),
            ),
            (
                "Slice<i64>",
                "Vec<i64>",
                Family::Sequence(Sequence::Vec),
                sequence(),
                mapped(),
            ),
            (
                "Option<i64>",
                "Option<i64>",
                Family::Option,
                RuntimeValue::option_some(RuntimeValue::i64(1)),
                RuntimeValue::option_some(RuntimeValue::i64(2)),
            ),
            (
                "Option<i64>",
                "Option<i64>",
                Family::Option,
                RuntimeValue::option_none(),
                RuntimeValue::option_none(),
            ),
            (
                "Result<i64, String>",
                "Result<i64, String>",
                Family::Result,
                RuntimeValue::result_ok(RuntimeValue::i64(1)),
                RuntimeValue::result_ok(RuntimeValue::i64(2)),
            ),
            (
                "Result<i64, String>",
                "Result<i64, String>",
                Family::Result,
                RuntimeValue::result_err(RuntimeValue::String("retained".to_owned())),
                RuntimeValue::result_err(RuntimeValue::String("retained".to_owned())),
            ),
        ] {
            let selected = authored_map_program(&format!(
                "fn score(value: i64, unused: i64) -> i64 {{ return value + 1i64 }}\npub fn root(values: {ty}, unused: i64) -> {result} {{ values.map(|item| score(item, 9i64)) }}\nflow main() -> String {{ return \"ok\" }}\n"
            ));
            assert_eq!(
                selected
                    .plan()
                    .function_sites()
                    .get(selected.function_site().unwrap())
                    .unwrap()
                    .parameter_inputs()
                    .count(),
                2
            );
            assert!(
                selected.plan().pure_helpers().is_empty(),
                "source functions retain their actual sites"
            );
            assert_authored_map_family(&selected, &family);
            let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
            let args = [argument.clone(), RuntimeValue::i64(70)];
            assert_authored_map_value(
                arcweft_core::pure::evaluate_pure_program_with_backend(
                    selected.plan(),
                    selected.program(),
                    &args,
                    &mut backend,
                )
                .unwrap(),
                &expected,
            );
            let rejected = arcweft_core::engine::Engine::for_program_invocation(
                Arc::clone(selected.plan()),
                selected.program(),
                vec![argument],
            )
            .err()
            .expect("omitted full formal must be refused");
            assert_eq!(
                rejected.into_parts().1.len(),
                1,
                "omitted unused formal is refused before transfer"
            );
            let mut plan = selected.plan().as_ref().clone();
            plan.bind_artifact(
                arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([72; 32]).unwrap(),
            )
            .unwrap();
            let product = Arc::new(
                arcweft_runtime_plan::awbc_lower::AwbcLowerer::new(
                    &plan,
                    &selected.lowering_report().dialogue_content_catalog,
                    "map-call-return.arcw",
                )
                .lower()
                .unwrap()
                .program,
            );
            assert_authored_map_value(
                arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
                    &product,
                    selected.program(),
                    &args,
                    &mut backend,
                )
                .unwrap(),
                &expected,
            );
        }
    }

    #[test]
    fn authored_map_callback_execution_survives_single_op_budget_and_native_rollback() {
        let selected = authored_map_program(
            "fn score(value: i64, unused: i64) -> i64 { return value + 1i64 }\npub fn root(values: Vec<i64>) -> Vec<i64> { values.map(|item| score(item, 9i64)) }\nflow main() -> String { return \"ok\" }\n",
        );
        let values = arcweft_core::value::runtime_sequence_values(vec![
            RuntimeValue::i64(1),
            RuntimeValue::i64(2),
            RuntimeValue::i64(3),
        ]);
        let mut engine = arcweft_core::engine::Engine::for_program_invocation(
            Arc::clone(selected.plan()),
            selected.program(),
            vec![values],
        )
        .unwrap();
        let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
        let mut steps = 0;
        loop {
            let step = engine.step_with_pure_backend(
                Default::default(),
                arcweft_core::step::RuntimeStepOptions {
                    budget: arcweft_core::step::RuntimeStepBudget { max_ops: 1 },
                    ..Default::default()
                },
                &mut backend,
            );
            assert!(step.output.diagnostics.is_empty(), "{step:?}");
            steps += 1;
            if let Some((program, value)) = engine.take_program_result().unwrap() {
                assert_eq!(program, selected.program());
                assert_eq!(
                    value,
                    arcweft_core::value::runtime_sequence_values(vec![
                        RuntimeValue::i64(2),
                        RuntimeValue::i64(3),
                        RuntimeValue::i64(4)
                    ])
                );
                break;
            }
            assert!(
                steps < 256,
                "Map must resume instead of restarting a source operand"
            );
            // The public step transaction owns its inert rollback image.
            // This integration keeps the budgeted continuation opaque; the
            // image roundtrip itself is covered by Core owning tests.
        }
        assert!(steps > 3, "callbacks remain budgeted ordinary calls");
        assert!(
            engine.fiber().env.bindings_snapshot().is_empty(),
            "completed callbacks and iterator scopes release all local bindings"
        );
    }

    #[test]
    fn authored_map_callback_failure_retains_its_owning_execution_and_no_result() {
        let selected = authored_map_program(
            "fn score(value: i64) -> i64 { return 12i64 / (value - 2i64) }\npub fn root(values: Vec<i64>) -> Vec<i64> { values.map(|item| score(item)) }\nflow main() -> String { return \"ok\" }\n",
        );
        let values = arcweft_core::value::runtime_sequence_values(vec![
            RuntimeValue::i64(1),
            RuntimeValue::i64(2),
            RuntimeValue::i64(3),
        ]);
        let mut engine = arcweft_core::engine::Engine::for_program_invocation(
            Arc::clone(selected.plan()),
            selected.program(),
            vec![values],
        )
        .unwrap();
        let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
        let mut diagnostics = Vec::new();
        let mut terminal = false;
        for _ in 0..256 {
            let step = engine.step_with_pure_backend(
                Default::default(),
                arcweft_core::step::RuntimeStepOptions {
                    budget: arcweft_core::step::RuntimeStepBudget { max_ops: 1 },
                    ..Default::default()
                },
                &mut backend,
            );
            diagnostics.extend(step.output.diagnostics);
            assert!(
                engine.take_program_result().unwrap().is_none(),
                "a failing callback cannot publish a Map result"
            );
            if !matches!(
                engine.fiber().status,
                arcweft_core::engine::FlowFiberStatus::Running
            ) {
                terminal = true;
                break;
            }
        }
        assert!(
            terminal,
            "Map callback did not reach terminal within256 one-op steps: status={:?}, diagnostics={diagnostics:?}",
            engine.fiber().status
        );
        assert!(matches!(
            engine.fiber().status,
            arcweft_core::engine::FlowFiberStatus::Failed(_)
        ));
        assert!(!diagnostics.is_empty());
        assert!(
            engine.take_program_result().unwrap().is_none(),
            "an incomplete Map result cannot escape its frame"
        );
        let arcweft_core::engine::FlowFiberStatus::Failed(failure) = &engine.fiber().status else {
            panic!("the callback must fail in its owning frame");
        };
        let failure = failure.clone();
        let bindings = engine.fiber().env.bindings_snapshot();
        let cursor = engine.fiber().cursor;
        let execution = engine.fiber().execution;
        engine.step_with_pure_backend(
            Default::default(),
            arcweft_core::step::RuntimeStepOptions {
                budget: arcweft_core::step::RuntimeStepBudget { max_ops: 1 },
                ..Default::default()
            },
            &mut backend,
        );
        assert!(matches!(&engine.fiber().status,
            arcweft_core::engine::FlowFiberStatus::Failed(message) if message == &failure));
        assert_eq!(
            engine.fiber().env.bindings_snapshot(),
            bindings,
            "failed Map ownership is retained by the same public execution"
        );
        assert_eq!(engine.fiber().cursor, cursor);
        assert_eq!(engine.fiber().execution, execution);
        assert!(engine.take_program_result().unwrap().is_none());
    }

    #[test]
    fn authored_sequence_equality_uses_checked_family_and_logical_elements_in_native_and_awbc() {
        use arcweft_core::plan::{RuntimePlanSequenceKind, RuntimePlanTypeProjection};
        use arcweft_core::value::{RuntimeSeq, runtime_sequence_values};
        fn arguments(input: &[i64], expected: &[i64]) -> [RuntimeValue; 2] {
            [
                runtime_sequence_values(input.iter().copied().map(RuntimeValue::i64).collect()),
                RuntimeValue::Seq(RuntimeSeq::values(
                    expected.iter().copied().map(RuntimeValue::i64).collect(),
                )),
            ]
        }
        fn native_result(
            selected: &super::CompiledDeterministicProgram,
            arguments: [RuntimeValue; 2],
        ) -> RuntimeValue {
            let mut engine = arcweft_core::engine::Engine::for_program_invocation(
                Arc::clone(selected.plan()),
                selected.program(),
                Vec::from(arguments),
            )
            .unwrap();
            let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
            for _ in 0..256 {
                let step = engine.step_with_pure_backend(
                    Default::default(),
                    arcweft_core::step::RuntimeStepOptions {
                        budget: arcweft_core::step::RuntimeStepBudget { max_ops: 1 },
                        ..Default::default()
                    },
                    &mut backend,
                );
                assert!(step.output.diagnostics.is_empty(), "{step:?}");
                if let Some((program, value)) = engine.take_program_result().unwrap() {
                    assert_eq!(program, selected.program());
                    assert!(
                        engine.fiber().env.bindings_snapshot().is_empty(),
                        "native equality releases callback, iterator and root scopes"
                    );
                    return value;
                }
                assert!(
                    matches!(
                        engine.fiber().status,
                        arcweft_core::engine::FlowFiberStatus::Running
                    ),
                    "native equality ended without a result: {:?}",
                    engine.fiber().status
                );
            }
            panic!(
                "native source equality did not complete within256 one-op steps: {:?}",
                engine.fiber().status
            );
        }
        for (family, kind) in [
            ("Vec<i64>", RuntimePlanSequenceKind::Vec),
            ("Seq<i64>", RuntimePlanSequenceKind::Seq),
        ] {
            for (operator, equal_result) in [("==", true), ("!=", false)] {
                let selected = authored_map_program(&format!(
                    "fn score(value: i64, unused: i64) -> i64 {{ return value + 1i64 }}\npub fn root(values: {family}, expected: {family}) -> bool {{ let mapped: {family} = values.map(|item| score(item, 9i64)); mapped {operator} expected }}\nflow main() -> String {{ return \"ok\" }}\n"
                ));
                let site = selected
                    .plan()
                    .function_sites()
                    .get(selected.function_site().unwrap())
                    .unwrap();
                assert!(matches!(
                    selected
                        .plan()
                        .type_table()
                        .get(site.result())
                        .unwrap()
                        .projection(),
                    RuntimePlanTypeProjection::Bool
                ));
                assert_eq!(site.parameter_inputs().count(), 2);
                for input in site.parameter_inputs() {
                    let RuntimePlanTypeProjection::Sequence { kind: actual, .. } = selected
                        .plan()
                        .type_table()
                        .get(input.pattern().ty())
                        .unwrap()
                        .projection()
                    else {
                        panic!("checked sequence input")
                    };
                    assert_eq!(actual, &kind);
                }
                let mut artifact_plan = selected.plan().as_ref().clone();
                artifact_plan
                    .bind_artifact(
                        arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([75; 32])
                            .unwrap(),
                    )
                    .unwrap();
                let product = Arc::new(
                    arcweft_runtime_plan::awbc_lower::AwbcLowerer::new(
                        &artifact_plan,
                        &selected.lowering_report().dialogue_content_catalog,
                        "sequence-equality.arcw",
                    )
                    .lower()
                    .unwrap()
                    .program,
                );
                for (input, expected, result) in [
                    (vec![1, 2, 3], vec![2, 3, 4], equal_result),
                    (vec![1, 2, 3], vec![2, 3, 9], !equal_result),
                    (Vec::new(), Vec::new(), equal_result),
                ] {
                    assert_eq!(
                        native_result(&selected, arguments(&input, &expected)),
                        RuntimeValue::Bool(result)
                    );
                    let args = arguments(&input, &expected);
                    let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
                    assert_eq!(
                        arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
                            &product,
                            selected.program(),
                            &args,
                            &mut backend
                        )
                        .unwrap(),
                        RuntimeValue::Bool(result)
                    );
                }
            }
        }
    }

    #[test]
    fn authored_recursive_executable_vm_invocation_charges_one_explicit_owner_budget() {
        use arcweft_core::pure::{PureFunctionBackend, PureFunctionRequest, VmPureFunctionBackend};
        let selected = authored_map_program(
            "pub fn root(value: i64) -> i64 effects {} { return root(value) }\nflow main() -> String { return \"ok\" }\n",
        );
        let site = selected.function_site().unwrap();
        let request =
            PureFunctionRequest::try_new(Arc::clone(selected.plan()), site, [RuntimeValue::i64(4)])
                .unwrap();
        assert!(matches!(VmPureFunctionBackend.evaluate(&request),
            Err(arcweft_core::value::RuntimeEvalError::UnsupportedPure { reason, .. })
                if reason.contains("requires function-call control transfer")));
        for max_ops in [0, 1, 7] {
            let failure = VmPureFunctionBackend
                .evaluate_invocation(&request, arcweft_core::step::RuntimeStepBudget { max_ops })
                .unwrap_err();
            assert!(matches!(failure,
                arcweft_core::value::RuntimeEvalError::UnsupportedPure { reason, .. }
                    if reason.contains(&format!("explicit {max_ops}-operation budget"))));
            assert_eq!(request.bindings()[0].value, RuntimeValue::i64(4));
        }
        let mut engine = arcweft_core::engine::Engine::for_program_invocation(
            Arc::clone(selected.plan()),
            selected.program(),
            vec![RuntimeValue::i64(4)],
        )
        .unwrap();
        let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
        let step = engine.step_with_pure_backend(
            Default::default(),
            arcweft_core::step::RuntimeStepOptions {
                mode: arcweft_core::step::RuntimeStepMode::Drain,
                budget: arcweft_core::step::RuntimeStepBudget { max_ops: 7 },
                ..Default::default()
            },
            &mut backend,
        );
        assert_eq!(step.stats.executed_ops, 7);
        assert_eq!(
            step.stop_reason,
            arcweft_core::step::RuntimeStepStopReason::BudgetExhausted
        );
        assert!(matches!(
            step.fiber_status,
            arcweft_core::engine::FlowFiberStatus::Running
        ));
        assert!(engine.take_program_result().unwrap().is_none());
    }

    #[test]
    fn source_entity_reference_fields_execute_through_the_accepted_native_and_awbc_root() {
        let compiled = crate::source::compile_source(concat!(
            "pub character alice { display = \"Alice\" }\n",
            "pub fn root() -> (String, String, String) effects {} {\n",
            "    let reference = @character.alice\n",
            "    (reference.id, reference.family, reference.name)\n",
            "}\n",
            "flow main() -> String { return \"ok\" }\n",
        ))
        .unwrap();
        let lease = &compiled.analysis;
        let declaration = lease
            .final_analysis()
            .hir_topology()
            .modules()
            .iter()
            .flat_map(|module| module.entries())
            .filter_map(|entry| entry.body())
            .find(|body| body.declaration().name() == "root")
            .unwrap()
            .declaration()
            .clone();
        let selected = lease
            .compile_deterministic_program(
                CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                    declaration,
                    role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::FunctionBody,
                }),
                None,
                &ProjectInstantiationControl::default(),
            )
            .unwrap();
        let expected = RuntimeValue::Tuple(vec![
            RuntimeValue::String("character.alice".into()),
            RuntimeValue::String("character".into()),
            RuntimeValue::String("alice".into()),
        ]);
        let mut native = arcweft_core::engine::Engine::for_program_invocation(
            Arc::clone(selected.plan()),
            selected.program(),
            Vec::new(),
        )
        .unwrap();
        let mut options = arcweft_core::step::RuntimeStepOptions::default();
        options.budget.max_ops = 1;
        for _ in 0..32 {
            let step = native.step(arcweft_core::step::RuntimeStepInput::default(), options);
            assert!(
                step.output.diagnostics.is_empty(),
                "{:?}",
                step.output.diagnostics
            );
            if matches!(
                native.fiber().status,
                arcweft_core::engine::FlowFiberStatus::Done(_)
            ) {
                break;
            }
        }
        assert_eq!(
            native.take_program_result().unwrap(),
            Some((selected.program(), expected.clone()))
        );
        let lowered = arcweft_runtime_plan::awbc_lower::AwbcLowerer::new(
            selected.plan(),
            &selected.lowering_report().dialogue_content_catalog,
            "reference-fields.arcw",
        )
        .lower()
        .unwrap();
        assert!(lowered.diagnostics.is_empty());
        let encoded = lowered.program.encode_canonical().unwrap();
        let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
            &encoded,
            arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
        )
        .unwrap();
        assert_eq!(decoded.encode_canonical().unwrap(), encoded);
        let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
        assert_eq!(
            arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
                &Arc::new(decoded),
                selected.program(),
                &[],
                &mut backend,
            )
            .unwrap(),
            expected
        );
    }
    mod collection_owner_tests {
        use super::*;
        use arcweft_core::engine::{Engine, FlowFiberStatus};
        use arcweft_core::step::{RuntimeStepBudget, RuntimeStepOptions};
        use arcweft_core::value::*;

        fn run_collection_native(
            selected: &super::super::CompiledDeterministicProgram,
            receiver: RuntimeValue,
        ) -> Result<RuntimeValue, String> {
            let mut engine = Engine::for_program_invocation(
                Arc::clone(selected.plan()),
                selected.program(),
                vec![receiver],
            )
            .unwrap();
            let options = RuntimeStepOptions {
                budget: RuntimeStepBudget { max_ops: 1 },
                ..Default::default()
            };
            let mut diagnostics = Vec::new();
            for _ in 0..64 {
                let step = engine.step(Default::default(), options);
                assert!(step.stats.executed_ops <= 1);
                diagnostics.extend(step.output.diagnostics);
                if !matches!(engine.fiber().status, FlowFiberStatus::Running) {
                    break;
                }
            }
            match &engine.fiber().status {
                FlowFiberStatus::Done(_) => {
                    assert!(diagnostics.is_empty(), "{diagnostics:?}");
                    let (program, result) = engine
                        .take_program_result()
                        .unwrap()
                        .expect("the selected collection result is published");
                    assert_eq!(program, selected.program());
                    Ok(result)
                }
                FlowFiberStatus::Failed(message) => {
                    let message = message.clone();
                    assert!(
                        diagnostics
                            .iter()
                            .any(|diagnostic| diagnostic.message == message)
                    );
                    assert!(engine.take_program_result().unwrap().is_none());
                    let execution = engine.fiber().execution;
                    let cursor = engine.fiber().cursor;
                    let second = engine.step(Default::default(), options);
                    assert_eq!(second.stats.executed_ops, 0);
                    assert!(
                        matches!(&engine.fiber().status,FlowFiberStatus::Failed(actual) if actual==&message)
                    );
                    assert_eq!(engine.fiber().execution, execution);
                    assert_eq!(engine.fiber().cursor, cursor);
                    Err(message)
                }
                status => panic!(
                    "the admitted collection did not finish within 64 one-op steps: {status:?}, {diagnostics:?}"
                ),
            }
        }

        fn run_collection_awbc(
            selected: &super::super::CompiledDeterministicProgram,
            receiver: RuntimeValue,
        ) -> Result<RuntimeValue, String> {
            let lowered = arcweft_runtime_plan::awbc_lower::AwbcLowerer::new(
                selected.plan(),
                &selected.lowering_report().dialogue_content_catalog,
                "collection-owner.arcw",
            )
            .lower()
            .unwrap();
            assert!(lowered.diagnostics.is_empty());
            let encoded = lowered.program.encode_canonical().unwrap();
            let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
                &encoded,
                arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
            )
            .unwrap();
            assert_eq!(decoded.encode_canonical().unwrap(), encoded);
            decoded
                .verify(
                    Default::default(),
                    arcweft_core::awbc::verify::AwbcVerifyContext {
                        require_entrypoint: false,
                        ..Default::default()
                    },
                )
                .unwrap();
            let mut executor =
                arcweft_core::awbc::product_step::AwbcProductStepExecutor::for_program_invocation(
                    Arc::new(decoded),
                    selected.program(),
                    vec![receiver],
                    arcweft_core::task::GenerationId::new(0),
                    1,
                )
                .unwrap();
            let options = RuntimeStepOptions {
                budget: RuntimeStepBudget { max_ops: 1 },
                ..Default::default()
            };
            let mut diagnostics = Vec::new();
            for _ in 0..64 {
                let step = executor.step(Default::default(), options);
                assert!(step.stats.executed_ops <= 1);
                diagnostics.extend(step.output.diagnostics);
                if !matches!(executor.fiber().status, FlowFiberStatus::Running) {
                    break;
                }
            }
            match &executor.fiber().status {
                FlowFiberStatus::Done(_) => {
                    assert!(diagnostics.is_empty(), "{diagnostics:?}");
                    let (program, result) = executor
                        .take_program_result()
                        .unwrap()
                        .expect("the canonical AWBC collection result is published");
                    assert_eq!(program, selected.program());
                    Ok(result)
                }
                FlowFiberStatus::Failed(message) => {
                    let message = message.clone();
                    assert!(
                        diagnostics
                            .iter()
                            .any(|diagnostic| diagnostic.message == message)
                    );
                    assert!(executor.take_program_result().unwrap().is_none());
                    let second = executor.step(Default::default(), options);
                    assert_eq!(second.stats.executed_ops, 0);
                    assert!(
                        matches!(&executor.fiber().status,FlowFiberStatus::Failed(actual) if actual==&message)
                    );
                    assert!(executor.take_program_result().unwrap().is_none());
                    Err(message)
                }
                status => panic!(
                    "the canonical collection did not finish within 64 one-op steps: {status:?}, {diagnostics:?}"
                ),
            }
        }

        fn assert_collection_parity(
            selected: &super::super::CompiledDeterministicProgram,
            receiver: RuntimeValue,
            expected: Result<RuntimeValue, String>,
        ) {
            assert_eq!(run_collection_native(selected, receiver.clone()), expected);
            assert_eq!(run_collection_awbc(selected, receiver), expected);
        }

        #[test]
        fn source_collection_sum_matches_native_and_canonical_awbc_for_all_integer_widths() {
            macro_rules! row {
                ($name:literal,$dense:ident,$value:ident,$maximum:expr,$expected:expr) => {
                    (
                        $name,
                        $dense(vec![$maximum, 1]),
                        runtime_sequence_values(vec![
                            RuntimeValue::$value($maximum),
                            RuntimeValue::$value(1),
                        ]),
                        $expected,
                    )
                };
            }
            for (item, dense, dynamic, expected) in [
                row!(
                    "i8",
                    runtime_sequence_dense_i8,
                    i8,
                    i8::MAX,
                    i64::from(i8::MAX) + 1
                ),
                row!(
                    "i16",
                    runtime_sequence_dense_i16,
                    i16,
                    i16::MAX,
                    i64::from(i16::MAX) + 1
                ),
                row!(
                    "i32",
                    runtime_sequence_dense_i32,
                    i32,
                    i32::MAX,
                    i64::from(i32::MAX) + 1
                ),
                row!("i64", runtime_sequence_dense_i64, i64, i64::MAX, i64::MIN),
                row!(
                    "i128",
                    runtime_sequence_dense_i128,
                    i128,
                    i128::from(i64::MAX),
                    i64::MIN
                ),
                row!(
                    "isize",
                    runtime_sequence_dense_isize,
                    isize,
                    i64::MAX,
                    i64::MIN
                ),
                row!(
                    "u8",
                    runtime_sequence_dense_u8,
                    u8,
                    u8::MAX,
                    i64::from(u8::MAX) + 1
                ),
                row!(
                    "u16",
                    runtime_sequence_dense_u16,
                    u16,
                    u16::MAX,
                    i64::from(u16::MAX) + 1
                ),
                row!(
                    "u32",
                    runtime_sequence_dense_u32,
                    u32,
                    u32::MAX,
                    i64::from(u32::MAX) + 1
                ),
                row!(
                    "u64",
                    runtime_sequence_dense_u64,
                    u64,
                    i64::MAX as u64,
                    i64::MIN
                ),
                row!(
                    "u128",
                    runtime_sequence_dense_u128,
                    u128,
                    i64::MAX as u128,
                    i64::MIN
                ),
                row!(
                    "usize",
                    runtime_sequence_dense_usize,
                    usize,
                    i64::MAX as u64,
                    i64::MIN
                ),
            ] {
                for ty in [format!("Vec<{item}>"), format!("Array<{item}, 2>")] {
                    let selected = authored_map_program(&format!(
                        "pub fn root(values: {ty}) -> i64 {{ values.sum() }}\nflow main() -> String {{ return \"ok\" }}\n"
                    ));
                    for receiver in [dense.clone(), dynamic.clone()] {
                        assert_collection_parity(
                            &selected,
                            receiver,
                            Ok(RuntimeValue::i64(expected)),
                        );
                    }
                }
            }
            for ty in ["Vec<u8>", "Array<u8, 2>"] {
                let selected = authored_map_program(&format!(
                    "pub fn root(values: {ty}) -> i64 {{ values.sum() }}\nflow main() -> String {{ return \"ok\" }}\n"
                ));
                assert_collection_parity(
                    &selected,
                    runtime_sequence_dense_bytes(vec![u8::MAX, 1]),
                    Ok(RuntimeValue::i64(256)),
                );
            }
        }

        #[test]
        fn source_collection_length_matches_native_and_canonical_awbc_storage() {
            for (ty, receiver, length) in [
                ("Vec<Unit>", runtime_sequence_dense_units(1024), 1024),
                ("Seq<i32>", runtime_sequence_dense_i32(vec![1, 2, 3]), 3),
                (
                    "Slice<i32>",
                    runtime_sequence_values(vec![RuntimeValue::i32(1), RuntimeValue::i32(2)]),
                    2,
                ),
                (
                    "Array<u8, 3>",
                    runtime_sequence_dense_bytes(vec![1, 2, 3]),
                    3,
                ),
            ] {
                let selected = authored_map_program(&format!(
                    "pub fn root(values: {ty}) -> usize {{ values.len() }}\nflow main() -> String {{ return \"ok\" }}\n"
                ));
                assert_collection_parity(&selected, receiver, Ok(RuntimeValue::usize(length)));
            }
        }

        #[test]
        fn source_collection_sum_retains_first_exact_rejection_in_native_and_canonical_awbc() {
            let first_signed = i128::from(i64::MIN) - 1;
            let first_unsigned = i64::MAX as u128 + 1;
            let first_u64 = i64::MAX as u64 + 1;
            for (item, dense, dynamic, first) in [
                (
                    "i128",
                    runtime_sequence_dense_i128(vec![7, first_signed, i128::MAX]),
                    runtime_sequence_values(vec![
                        RuntimeValue::i128(7),
                        RuntimeValue::i128(first_signed),
                        RuntimeValue::i128(i128::MAX),
                    ]),
                    first_signed.to_string(),
                ),
                (
                    "u128",
                    runtime_sequence_dense_u128(vec![7, first_unsigned, u128::MAX]),
                    runtime_sequence_values(vec![
                        RuntimeValue::u128(7),
                        RuntimeValue::u128(first_unsigned),
                        RuntimeValue::u128(u128::MAX),
                    ]),
                    first_unsigned.to_string(),
                ),
                (
                    "u64",
                    runtime_sequence_dense_u64(vec![7, first_u64, u64::MAX]),
                    runtime_sequence_values(vec![
                        RuntimeValue::u64(7),
                        RuntimeValue::u64(first_u64),
                        RuntimeValue::u64(u64::MAX),
                    ]),
                    first_u64.to_string(),
                ),
                (
                    "usize",
                    runtime_sequence_dense_usize(vec![7, first_u64, u64::MAX]),
                    runtime_sequence_values(vec![
                        RuntimeValue::usize(7),
                        RuntimeValue::usize(first_u64),
                        RuntimeValue::usize(u64::MAX),
                    ]),
                    first_u64.to_string(),
                ),
            ] {
                for ty in [format!("Vec<{item}>"), format!("Array<{item}, 3>")] {
                    let selected = authored_map_program(&format!(
                        "pub fn root(values: {ty}) -> i64 {{ values.sum() }}\nflow main() -> String {{ return \"ok\" }}\n"
                    ));
                    let expected = RuntimeEvalError::UnsupportedBinary {
                        op: "+",
                        lhs: "int".into(),
                        rhs: first.clone(),
                    }
                    .to_string();
                    for receiver in [dense.clone(), dynamic.clone()] {
                        assert_collection_parity(&selected, receiver, Err(expected.clone()));
                    }
                }
            }
        }
    }
}
