use super::*;
use arcweft_core::awbc::{
    codec::AwbcDecodeBudget, product_step::AwbcProductStepExecutor, schema::AwbcProgram,
};
use arcweft_core::engine::{Engine, FlowFiberStatus};
use arcweft_core::executor::{ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot};
use arcweft_core::step::{RuntimeStepBudget, RuntimeStepMode, RuntimeStepOptions};
use arcweft_lang_sema::final_analysis::{CheckedExecutionBodyOwner, CheckedExecutionSource};

#[test]
fn source_traversal_keeps_owned_collections_and_callbacks_without_starting_children() {
    for family in [
        "Vec<String>",
        "Array<String, 2>",
        "Seq<String>",
        "Slice<String>",
    ] {
        assert_source_traversal(family, "values.traverse(|path| request(prefix, path))");
    }
}

#[test]
fn source_traversal_retains_computed_source_and_callback_operands() {
    assert_source_traversal(
        "Vec<String>",
        "paths(values).traverse({ let retained = prefix; |path| request(retained, path) })",
    );
}

#[test]
fn source_traversal_does_not_enter_the_callback_body() {
    // Every supplied path violates this runtime guard. Successful construction
    // must retain the real callback without entering its body in either backend.
    assert_source_traversal(
        "Vec<String>",
        r#"values.traverse(|path| { assert.check(path == "callback was invoked"); request(prefix, path) })"#,
    );
}

#[test]
fn source_parallel_executes_callbacks_with_public_save_quiescence_and_private_rollback() {
    for (family, count, errors) in [
        ("Vec<String>", 3, false),
        ("Array<String, 3>", 3, false),
        ("Seq<String>", 3, false),
        ("Slice<String>", 3, false),
        ("Vec<String>", 3, true),
        ("Vec<String>", 0, false),
    ] {
        // Seq and Slice are admitted through their exact Flow input ABI;
        // a bracket literal does not implicitly construct either family.
        let bindings = || {
            [arcweft_core::value::RuntimeFlowParameterBinding {
                parameter: arcweft_core::entry::FlowParameterCoordinate::from_position(0),
                value: arcweft_core::value::runtime_sequence_values(
                    ["a.txt", "b.txt", "c.txt"]
                        .into_iter()
                        .take(count)
                        .map(|path| RuntimeValue::String(path.into()))
                        .collect(),
                ),
            }]
        };
        let expected_source = if errors {
            r#".Err("a.txt")"#
        } else if count == 0 {
            ".Ok([])"
        } else {
            r#".Ok(["root/a.txt", "root/b.txt", "root/c.txt"])"#
        };
        let source = format!(
            r#"
extern capability fs {{
    fn read_text(prefix: String, path: String) -> Need<Result<String, String>> effects {{ fs.read }}
}}
fn request(prefix: String, path: String) -> Need<Result<String, String>> effects {{ fs.read }} {{
    fs.read_text(prefix, path)
}}
pub fn root(values: {family}, prefix: String, limit: u32) -> Result<Vec<String>, String> effects {{ fs.read, control.suspend }} {{
    let traversal = values.traverse(|path| request(prefix, path));
    let work = traversal.parallel(limit = limit);
    await work
}}
flow main(values: {family}) -> String {{
    let result = root(values, "root/", 2u32);
    let expected: Result<Vec<String>, String> = {expected_source};
    assert.check(result == expected);
    return "parallel passed"
}}
"#
        );
        let compiled = compile_traversal_source(&source);
        let report = compiled.runtime_plan();
        let lowered = AwbcLowerer::new(
            &report.plan,
            &report.dialogue_content_catalog,
            "parallel.arcw",
        )
        .lower()
        .unwrap()
        .program;
        let encoded = lowered.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
        assert_eq!(decoded, lowered);
        let mut native_plan = report.plan.clone();
        native_plan
            .bind_artifact(
                arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes(
                    *blake3::hash(&encoded).as_bytes(),
                )
                .unwrap(),
            )
            .unwrap();
        let flow = native_plan.flows()[0].id.clone();
        let mut native = Engine::for_flow_invocation(
            native_plan
                .seal_flow_invocation(flow.clone(), bindings())
                .unwrap(),
        )
        .unwrap();
        drive_source_parallel(
            &format!("Native {family}"),
            RuntimeProgramOwner::Plan(native.program_plan()),
            count,
            errors,
            |input| native.step(input, parallel_options()),
        );
        let code = Arc::new(decoded);
        let executor = ArcweftRuntimeExecutor::from_awbc_product_flow_invocation(
            Arc::clone(&code),
            report
                .plan
                .clone()
                .seal_flow_invocation(flow, bindings())
                .unwrap(),
            None,
        )
        .unwrap();
        let mut awbc = Some(executor);
        let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
        let mut public_saves = 0;
        let mut quiescence_refusals = 0;
        drive_source_parallel(
            &format!("AWBC {family}"),
            RuntimeProgramOwner::Awbc(code),
            count,
            errors,
            |input| {
                let mut current = awbc.take().unwrap();
                let step = current.step_with_pure_backend(input, parallel_options(), &mut backend);
                let snapshot = current.product_inert_rollback_image().unwrap();
                awbc = Some(
                match arcweft_core::awbc::product_step::AwbcProductExecutorSaveSnapshot::from_live(
                    &snapshot,
                    current.admitted_product_awbc_program().unwrap(),
                ) {
                    Ok(saved) => {
                        let saved =
                            serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
                        public_saves += 1;
                        let saved = current.bind_product_save_snapshot(saved).unwrap();
                        current.restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(saved))
                            .unwrap_or_else(|error| panic!("parallel public restore: {error}"))
                    }
                    Err(
                        arcweft_core::awbc::product_step::AwbcProductSaveError::NeedsQuiescence {
                            needs,
                        },
                    ) => {
                        assert!(!needs.is_empty());
                        quiescence_refusals += 1;
                        let (original, error) = current
                            .restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot.clone()))
                            .expect_err("public resume must preserve its quiescence requirement").into_parts();
                        assert!(matches!(error, arcweft_core::executor::ArcweftRuntimeExecutorSnapshotError::ProductAwbc { message } if message.contains("public producer resume requires quiescence")));
                        current = original;
                        assert_eq!(current.product_inert_rollback_image().unwrap(), snapshot);
                        current
                            .restore_current_product_rollback_image(snapshot)
                            .unwrap_or_else(|error| {
                                panic!("parallel private rollback: {error}")
                            })
                    }
                    Err(error) => panic!("parallel save admission: {error}"),
                },
            );
                step
            },
        );
        assert!(public_saves > 0);
        assert_eq!(quiescence_refusals > 0, count > 0);
    }
}

fn parallel_options() -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    }
}

fn drive_source_parallel(
    label: &str,
    owner: RuntimeProgramOwner,
    count: usize,
    errors: bool,
    mut step: impl FnMut(arcweft_core::step::RuntimeStepInput) -> arcweft_core::step::RuntimeStepResult,
) {
    use arcweft_core::task::{
        BoundTaskSpec, HostTaskRequest, LogicalEpoch, TaskDispatchIdentity, TaskEvent,
        TaskEventKind, TaskPublicationRevision, TaskRequest, TaskSequence,
    };
    let mut pending = Vec::<(usize, String, TaskEvent)>::new();
    let mut submitted = 0;
    let mut completed = Vec::new();
    let mut max_pending = 0;
    let mut last_step = String::new();
    for tick in 0..4096 {
        let mut input = arcweft_core::step::RuntimeStepInput::default();
        let mut index = pending.len();
        while index > 0 {
            index -= 1;
            if pending[index].0 <= tick {
                let (_, path, event) = pending.remove(index);
                completed.push(path);
                input.task_events.push(event);
            }
        }
        let mut delivered = input
            .task_events
            .iter()
            .map(|event| {
                (
                    event.publication_order(),
                    arcweft_core::step::RuntimeDiagnostic::new(format!(
                        "task {} sequence {} delivered",
                        event.correlation.task_id, event.cursor.sequence.0,
                    )),
                )
            })
            .collect::<Vec<_>>();
        delivered.sort_by_key(|(order, _)| *order);
        let expected_diagnostics = delivered
            .into_iter()
            .map(|(_, message)| message)
            .collect::<Vec<_>>();
        let result = step(input);
        if tick == 4095 {
            last_step = format!("{result:?}");
        }
        assert!(result.stats.executed_ops <= 1);
        assert_eq!(
            result.output.diagnostics, expected_diagnostics,
            "parallel diagnostics"
        );
        assert!(result.output.requests.host_calls.is_empty());
        assert!(result.output.requests.program_task_completions.is_empty());
        for submission in result.output.requests.tasks {
            let bound = BoundTaskSpec::bind(
                submission,
                Some(owner.clone()),
                arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
            )
            .unwrap();
            let TaskRequest::Host(HostTaskRequest::Custom {
                capability,
                operation,
                args,
                named_args,
                ..
            }) = &bound.spec().request
            else {
                panic!("callback must issue its actual Host request")
            };
            assert_eq!(capability.0, "fs");
            assert_eq!(operation, "read_text");
            assert!(named_args.is_empty());
            let [prefix, path] = args.as_slice() else {
                panic!("callback keeps both actual String arguments")
            };
            let (RuntimeValue::String(prefix), RuntimeValue::String(path)) =
                (prefix.value(), path.value())
            else {
                panic!("exact callback argument types")
            };
            assert_eq!(prefix, "root/");
            let value = if errors && path != "b.txt" {
                bound
                    .outcome()
                    .try_result_err(RuntimeValue::String(path.clone()))
                    .unwrap()
            } else {
                bound
                    .outcome()
                    .try_result_ok(RuntimeValue::String(format!("{prefix}{path}")))
                    .unwrap()
            };
            let event = TaskEvent::from_dispatch(
                TaskDispatchIdentity::new(
                    bound.handle().correlation,
                    LogicalEpoch(1),
                    TaskSequence(1),
                ),
                TaskPublicationRevision::FIRST,
                TaskEventKind::Ready(value),
            );
            // Keep the first source item pending while later source items finish.
            pending.push((
                tick + if path == "a.txt" { 64 } else { 4 },
                path.clone(),
                event,
            ));
            submitted += 1;
        }
        max_pending = max_pending.max(pending.len());
        assert!(pending.len() <= 2);
        if matches!(result.fiber_status, FlowFiberStatus::Done(_)) {
            assert_eq!(
                result.fiber_status,
                FlowFiberStatus::Done(arcweft_core::engine::FlowExit::Return(
                    "parallel passed".into()
                ))
            );
            assert_eq!(submitted, count);
            assert_eq!(completed.len(), count);
            assert_eq!(max_pending, count.min(2));
            if count > 1 {
                assert_ne!(completed.first().unwrap(), "a.txt");
            }
            return;
        }
        assert!(
            !matches!(result.fiber_status, FlowFiberStatus::Failed(_)),
            "{:?}",
            result.fiber_status
        );
    }
    panic!(
        "parallel did not finish its bounded fixture: {label}, submitted={submitted}, completed={completed:?}, pending={pending:?}, last={last_step}"
    );
}

fn assert_source_traversal(family: &str, expression: &str) {
    let source = format!(
        r#"
extern capability fs {{
    fn read_text(prefix: String, path: String) -> Need<Result<String, String>> effects {{ fs.read }}
}}
fn request(prefix: String, path: String) -> Need<Result<String, String>> effects {{ fs.read }} {{
    fs.read_text(prefix, path)
}}
fn paths(values: {family}) -> {family} {{ values }}
pub fn root(values: {family}, prefix: String) -> Traversal<{family}, String -> Need<Result<String, String>> effects {{ fs.read }}> effects {{}} {{
    {expression}
}}
flow main() -> String {{ return "unused" }}
"#
    );
    let compiled = compile_traversal_source(&source);
    let declaration = compiled
        .analysis_lease()
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
    let selected = compiled
        .analysis_lease()
        .compile_deterministic_program(
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                declaration,
                role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::FunctionBody,
            }),
            None,
            &Default::default(),
        )
        .unwrap();
    let collection = || {
        arcweft_core::value::runtime_sequence_values(vec![
            RuntimeValue::String("a.txt".to_owned()),
            RuntimeValue::String("b.txt".to_owned()),
        ])
    };
    let inputs = || vec![collection(), RuntimeValue::String("root/".to_owned())];
    let plan = Arc::clone(selected.plan());
    let native_owner = RuntimeProgramOwner::Plan(Arc::clone(&plan));
    let mut native = Engine::for_program_invocation(plan, selected.program(), inputs()).unwrap();
    let lowered = AwbcLowerer::new(
        selected.plan(),
        &selected.lowering_report().dialogue_content_catalog,
        "traversal.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let bytes = lowered.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded, lowered);
    let code = Arc::new(decoded);
    let awbc_owner = RuntimeProgramOwner::Awbc(Arc::clone(&code));
    let mut awbc = AwbcProductStepExecutor::for_program_invocation(
        code,
        selected.program(),
        inputs(),
        arcweft_core::task::GenerationId::new(0),
        1,
    )
    .unwrap();
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    };
    let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
    let mut native_done = false;
    let mut awbc_done = false;
    for _ in 0..4096 {
        if !native_done {
            let step = native.step(Default::default(), options);
            assert!(step.stats.executed_ops <= 1);
            assert!(
                step.output.diagnostics.is_empty(),
                "{family}: {:?}",
                step.output.diagnostics
            );
            assert!(
                step.output.requests.tasks.is_empty(),
                "capturing traversal must not start children"
            );
            assert!(
                !matches!(step.fiber_status, FlowFiberStatus::Failed(_)),
                "{family}: {:?}",
                step.fiber_status
            );
            native_done = matches!(step.fiber_status, FlowFiberStatus::Done(_));
        }
        if !awbc_done {
            let step = awbc.step_with_pure_backend(Default::default(), options, &mut backend);
            assert!(step.stats.executed_ops <= 1);
            assert!(
                step.output.diagnostics.is_empty(),
                "{family}: {:?}",
                step.output.diagnostics
            );
            assert!(
                step.output.requests.tasks.is_empty(),
                "capturing traversal must not start children"
            );
            assert!(
                !matches!(step.fiber_status, FlowFiberStatus::Failed(_)),
                "{family}: {:?}",
                step.fiber_status
            );
            awbc_done = matches!(step.fiber_status, FlowFiberStatus::Done(_));
        }
        if native_done && awbc_done {
            break;
        }
    }
    assert!(
        native_done && awbc_done,
        "{family} must complete bounded construction"
    );
    for (value, owner) in [
        (
            native.take_program_result().unwrap().unwrap().1,
            native_owner,
        ),
        (awbc.take_program_result().unwrap().unwrap().1, awbc_owner),
    ] {
        let RuntimeValue::Tuple(parts) = value else {
            panic!("expected the owning product")
        };
        let [source, RuntimeValue::Callable(mapping)] = parts.as_slice() else {
            panic!("expected the collection and actual callable")
        };
        assert_eq!(source, &collection());
        assert!(
            mapping.owner().same_program(&owner),
            "callback retains its original executable lease"
        );
        assert_eq!(
            mapping.retained(),
            &[RuntimeValue::String("root/".to_owned())]
        );
    }
}

fn compile_traversal_source(source: &str) -> arcweft_compiler::project::CompiledProject {
    use arcweft_adapter_context::manifest::{
        AdapterCallableGroupIndex, AdapterCallableName, AdapterCallableParameterIndex,
        AdapterEffectCapability, AdapterFunctionParam, AdapterFunctionSignature, AdapterHostCall,
        AdapterManifest, AdapterParameterGroup, AdapterParameterPassing, AdapterParameterPresence,
        AdapterTypeKind,
    };
    use arcweft_adapter_sema::registration::AdapterSemanticRegistration;
    use arcweft_compiler::project::{
        ProjectCompilationContext, ProjectCompilationSession, compile_project,
    };
    use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
    use arcweft_lang_sema::{env::TypeCheckEnv, registration::ProjectRegistrationFacts};
    use arcweft_lang_syntax::{
        ast::module_path::CanonicalModulePath, incremental::SyntaxDatabase, parser::ParseOptions,
    };
    use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
    use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
    use arcweft_resource_model::registry::ResourceTypeRegistry;
    use arcweft_source::{
        SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId,
    };
    use std::{collections::BTreeMap, path::PathBuf};

    let effect = AdapterEffectCapability::new("fs.read");
    let parameters = ["prefix", "path"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            AdapterFunctionParam::try_new(
                AdapterCallableParameterIndex::try_from_usize(index).unwrap(),
                Some(AdapterCallableName::try_new(name).unwrap()),
                AdapterTypeKind::String,
                AdapterParameterPassing::PositionalOrNamed,
                AdapterParameterPresence::Required,
            )
            .unwrap()
        })
        .collect();
    let manifest = AdapterManifest::new("test.traversal", "Source traversal fixture")
        .with_effect(effect.clone())
        .with_host_call(AdapterHostCall::with_signature(
            "fs.read_text",
            AdapterFunctionSignature::try_new(
                vec![
                    AdapterParameterGroup::try_new(
                        AdapterCallableGroupIndex::try_from_usize(0).unwrap(),
                        parameters,
                    )
                    .unwrap(),
                ],
                AdapterTypeKind::Need {
                    item: Box::new(AdapterTypeKind::Result {
                        ok: Box::new(AdapterTypeKind::String),
                        error: Box::new(AdapterTypeKind::String),
                    }),
                },
            )
            .unwrap(),
            [effect],
        ));
    let registration = AdapterSemanticRegistration::new(&manifest);
    let parts = registration.source_backed_facts(0).unwrap().into_parts();
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://traversal/src/main.arcw").unwrap(),
            SourceName::path("src/main.arcw"),
            source,
        )
        .unwrap(),
    );
    let manifest_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://traversal/arcw.toml").unwrap(),
            SourceName::path("arcw.toml"),
            "schema = 1\n[package]\nid = \"org.arcweft.traversal-fixture\"\nversion = \"0.1.0\"\n",
        )
        .unwrap(),
    );
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new("org.arcweft.traversal-fixture").unwrap(),
            version: PackageVersion::new("0.1.0").unwrap(),
        },
        BuildSpec::default(),
        Arc::clone(&manifest_document),
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            PathBuf::from("src/main.arcw"),
            Arc::clone(&document),
            [],
        )],
    )
    .unwrap();
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new("org.arcweft.traversal-fixture").unwrap(),
        document.identity().id().clone(),
        "traversal",
    )
    .unwrap();
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![manifest_document, Arc::clone(&document), parts.document],
        parts.externals.into_vec(),
        Vec::new(),
        vec![parts.environment],
    )
    .unwrap();
    let context = ProjectCompilationContext::new(
        Arc::new(registration.declare_target(TypeCheckEnv::standard())),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        None,
        None,
    );
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            document,
            ParseOptions::default(),
        )
        .unwrap();
    compile_project(
        &mut ProjectCompilationSession::try_new().unwrap(),
        &project,
        &BTreeMap::from([(CanonicalModulePath::crate_root(), parsed)]),
        &context,
    )
    .unwrap_or_else(|error| panic!("manifest-backed traversal project: {error:?}\n{source}"))
}
