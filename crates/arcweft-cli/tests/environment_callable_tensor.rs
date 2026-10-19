//! Real adapter tensors through a checked environment function value.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use arcweft_adapter_context::standard::inference_tensor_manifest;
use arcweft_adapter_sema::registration::AdapterSemanticRegistration;
use arcweft_compiler::{
    lower::{CompiledDeterministicProgram, ProjectInstantiationControl},
    project::{ProjectCompilationContext, ProjectCompilationSession, compile_project},
};
use arcweft_core::{
    awbc::{codec::AwbcDecodeBudget, product_step::AwbcProductStepExecutor, schema::AwbcProgram},
    engine::{Engine, FlowFiberStatus},
    math::DenseTensorF32,
    pure::RuntimePureCallBackend,
    step::{
        RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepResult,
    },
    task::GenerationId,
    value::RuntimeValue,
};
use arcweft_lang_hir::{
    project::HirDeclarationBodyRootRole,
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    final_analysis::{CheckedExecutionBodyOwner, CheckedExecutionSource},
    registration::ProjectRegistrationFacts,
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath, incremental::SyntaxDatabase, parser::ParseOptions,
};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_runtime_accelerator::{
    RuntimePureAccelerator, RuntimePureAcceleratorConfig,
    math::{RuntimeMathAcceleratorConfig, RuntimeMathBackend},
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId};

#[test]
fn environment_tensor_value_preserves_elements_and_one_op_restoration() {
    let selected = tensor_program();
    let program = AwbcLowerer::new(
        selected.plan(),
        &selected.lowering_report().dialogue_content_catalog,
        "environment-tensor.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let decoded = AwbcProgram::decode_canonical(
        &program.encode_canonical().unwrap(),
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    assert_eq!(decoded, program);
    let program = Arc::new(decoded);
    for math in [RuntimeMathBackend::Scalar, RuntimeMathBackend::Ndarray] {
        let mut native = Engine::for_program_invocation(
            Arc::clone(selected.plan()),
            selected.program(),
            tensor_inputs(),
        )
        .unwrap();
        let mut native_backend = accelerator(&selected, math);
        let mut awbc = Some(
            AwbcProductStepExecutor::for_program_invocation(
                Arc::clone(&program),
                selected.program(),
                tensor_inputs(),
                GenerationId::new(0),
                1,
            )
            .unwrap(),
        );
        let mut awbc_backend = accelerator(&selected, math);
        run_one_op(&mut || {
            native.step_with_pure_backend(
                RuntimeStepInput::default(),
                options(),
                &mut native_backend,
            )
        });
        let native_value = native.take_program_result().unwrap().unwrap().1;
        assert!(native.take_program_result().unwrap().is_none());
        run_one_op(&mut || {
            let mut executor = awbc.take().expect("one owned Product executor");
            let step = executor.step_with_pure_backend(
                RuntimeStepInput::default(),
                options(),
                &mut awbc_backend,
            );
            let snapshot = executor.inert_rollback_image().unwrap().product;
            let wire =
                arcweft_core::awbc::product_step::AwbcProductExecutorSaveSnapshot::from_live(
                    &snapshot,
                    &arcweft_core::task::RuntimeProgramOwner::Awbc(executor.program_arc()),
                )
                .unwrap();
            let bytes = serde_json::to_vec(&wire).unwrap();
            let decoded = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(wire, decoded);
            let restored = executor
                .restore_inert_snapshot_owned(decoded)
                .unwrap_or_else(|(_, error)| {
                    panic!("restore the original tensor program: {error}")
                });
            assert_eq!(restored.inert_rollback_image().unwrap().product, snapshot);
            awbc = Some(restored);
            step
        });
        let mut awbc = awbc.unwrap();
        let awbc_value = awbc.take_program_result().unwrap().unwrap().1;
        assert!(awbc.take_program_result().unwrap().is_none());
        assert_eq!(native_value, awbc_value);
        let RuntimeValue::TensorF32(value) = native_value else {
            panic!("the declared whole-formal adapter result is a tensor")
        };
        assert_eq!(value.shape().dims(), &[2, 2]);
        assert_eq!(value.values(), &[19.5, 23.5, 43.5, 51.5]);
        assert_counters(&native_backend, math);
        assert_counters(&awbc_backend, math);
    }
}

fn options() -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..RuntimeStepOptions::default()
    }
}

fn run_one_op(step: &mut dyn FnMut() -> RuntimeStepResult) {
    // Keep the original R626 fixture's total 64-step allowance and OneOp/1.
    for _ in 0..64 {
        let step = step();
        assert!(step.stats.executed_ops <= 1);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        match step.fiber_status {
            FlowFiberStatus::Done(_) => return,
            FlowFiberStatus::Failed(error) => panic!("tensor value failed: {error:?}"),
            _ => {}
        }
    }
    panic!("tensor function value did not complete the original OneOp budget");
}

fn accelerator(
    selected: &CompiledDeterministicProgram,
    backend: RuntimeMathBackend,
) -> RuntimePureAccelerator {
    RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: RuntimeMathAcceleratorConfig {
                backend,
                ..RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        selected.plan(),
    )
}

fn assert_counters(backend: &RuntimePureAccelerator, selected: RuntimeMathBackend) {
    let math = backend.math_stats();
    assert_eq!(math.fused_matmul_bias_add_calls, 1);
    assert_eq!(math.fallback_calls, 0);
    assert_eq!(
        math.scalar_calls,
        usize::from(selected == RuntimeMathBackend::Scalar)
    );
    assert_eq!(
        math.ndarray_calls,
        usize::from(selected == RuntimeMathBackend::Ndarray)
    );
    assert_eq!(math.bytes_borrowed, 40);
    assert_eq!(math.bytes_copied, 0);
    let pure = backend.stats();
    assert_eq!(pure.arg_bytes_borrowed, 40);
    assert_eq!(pure.arg_bytes_copied, 0);
    assert_eq!(pure.result_bytes_copied, 16);
    assert_eq!(pure.fallbacks, 0);
}

fn tensor_inputs() -> Vec<RuntimeValue> {
    [
        (vec![2, 2], vec![1.0, 2.0, 3.0, 4.0]),
        (vec![2, 2], vec![5.0, 6.0, 7.0, 8.0]),
        (vec![2], vec![0.5, 1.5]),
    ]
    .into_iter()
    .map(|(shape, values)| RuntimeValue::tensor_f32(DenseTensorF32::new(shape, values).unwrap()))
    .collect()
}

fn tensor_program() -> CompiledDeterministicProgram {
    let source = document(
        "src/main.arcw",
        r"
pub fn root(lhs: TensorF32, rhs: TensorF32, bias: TensorF32) -> TensorF32 {
    let combine = infer.matmul_bias_add_f32
    combine(lhs, rhs, bias)
}
flow main() -> Unit { return () }
",
    );
    let manifest_document = document("arcw.toml", "");
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new("org.arcweft.test.environment-tensor").unwrap(),
            version: PackageVersion::new("0.0.0").unwrap(),
        },
        BuildSpec::default(),
        Arc::clone(&manifest_document),
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            PathBuf::from("src/main.arcw"),
            Arc::clone(&source),
            [],
        )],
    )
    .unwrap();
    let manifest = inference_tensor_manifest();
    let parts = AdapterSemanticRegistration::new(&manifest)
        .source_backed_facts(0)
        .unwrap()
        .into_parts();
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new("org.arcweft.test.environment-tensor").unwrap(),
        source.identity().id().clone(),
        "environment-tensor",
    )
    .unwrap();
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![manifest_document, Arc::clone(&source), parts.document],
        parts.externals.into_vec(),
        Vec::new(),
        vec![parts.environment],
    )
    .unwrap();
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        None,
        None,
    );
    let parsed = SyntaxDatabase::try_new()
        .unwrap()
        .parse_initial(
            SourceSnapshotId::initial(source.display_name().clone()),
            source,
            ParseOptions::default(),
        )
        .unwrap();
    let compiled = compile_project(
        &mut ProjectCompilationSession::try_new().unwrap(),
        &project,
        &BTreeMap::from([(CanonicalModulePath::crate_root(), parsed)]),
        &context,
    )
    .unwrap();
    let lease = compiled.analysis_lease();
    let declaration = lease
        .final_analysis()
        .hir_topology()
        .modules()
        .iter()
        .flat_map(arcweft_lang_hir::project::HirModuleEvaluationTopology::entries)
        .filter_map(|entry| entry.body())
        .find(|body| body.declaration().name() == "root")
        .unwrap()
        .declaration()
        .clone();
    lease
        .compile_deterministic_program(
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                declaration,
                role: HirDeclarationBodyRootRole::FunctionBody,
            }),
            None,
            &ProjectInstantiationControl::default(),
        )
        .unwrap()
}

fn document(path: &str, source: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-source://{path}")).unwrap(),
            SourceName::path(path),
            source,
        )
        .unwrap(),
    )
}
