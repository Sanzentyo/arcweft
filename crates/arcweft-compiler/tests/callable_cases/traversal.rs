use super::*;
use arcweft_core::awbc::{
    codec::AwbcDecodeBudget, product_step::AwbcProductStepExecutor, schema::AwbcProgram,
};
use arcweft_core::engine::{Engine, FlowFiberStatus};
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
    .unwrap_or_else(|error| panic!("manifest-backed traversal project: {error:?}"))
}
