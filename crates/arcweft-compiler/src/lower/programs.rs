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
}
