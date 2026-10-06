use super::*;

mod analysis_lease;
use arcweft_lang_hir::symbol::{
    CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectSymbolWorldId,
};
use arcweft_lang_hir::{
    expr::HirExprKind,
    item::{HirItemKind, HirProofBody},
    proof_return::HirProofReturnSemanticClass,
};
use arcweft_lang_sema::{
    env::identity::EnvironmentBindingId,
    registration::{ExternalRegistrationFact, RegisteredExternalOwner},
};
use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePathRoot, ModuleSegment},
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
};
use arcweft_lang_syntax::{
    incremental::{ParsedSource, SyntaxDatabase},
    lint::{SyntaxLintCode, SyntaxLintSeverity},
    parser::ParseOptions,
};
use arcweft_launch::{LaunchProfileSelection, ProfileId, accepted::SourceBackedManifest};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::graph::ModuleDependency;
use arcweft_project::sources::ProjectSourceFile;
use arcweft_source::{
    DiagnosticLabel, DiagnosticLabelStyle, SourceDocument, SourceRange, SourceSetRevision,
    identity::SourceSnapshotId,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

fn compilation_state(
    project: &ProjectSources,
) -> (
    ProjectCompilationSession,
    BTreeMap<CanonicalModulePath, ParsedSource>,
) {
    let mut syntax = SyntaxDatabase::try_new().expect("test syntax database");
    let parsed = project
        .modules()
        .map(|source| {
            let parsed = syntax
                .parse_initial(
                    SourceSnapshotId::initial(source.document().display_name().clone()),
                    Arc::clone(source.document()),
                    ParseOptions::default(),
                )
                .expect("attached test project source");
            (source.module().clone(), parsed)
        })
        .collect();
    (
        ProjectCompilationSession::try_new().expect("test HIR database"),
        parsed,
    )
}

fn removed_role_project(source_text: &str) -> (ProjectSources, ProjectCompilationContext) {
    removed_role_project_with_dialogue_profile(source_text, false)
}

fn removed_role_dialogue_project(source_text: &str) -> (ProjectSources, ProjectCompilationContext) {
    removed_role_project_with_dialogue_profile(source_text, true)
}

fn removed_role_project_with_dialogue_profile(
    source_text: &str,
    with_dialogue_profile: bool,
) -> (ProjectSources, ProjectCompilationContext) {
    let source_path = PathBuf::from("src/main.arcw");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://removed-role/src/main.arcw")
                .expect("document ID"),
            SourceName::path(source_path.display().to_string()),
            source_text,
        )
        .expect("source document"),
    );
    let manifest = if with_dialogue_profile {
        dialogue_manifest_document("removed-role")
    } else {
        manifest_document("removed-role")
    };
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        package("org.arcweft.removed-role"),
        BuildSpec::default(),
        Arc::clone(&manifest),
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            source_path,
            Arc::clone(&document),
            [],
        )],
    )
    .expect("project sources");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(project.package().id.as_str()).expect("package"),
        document.identity().id().clone(),
        "removed-role-test",
    )
    .expect("symbol world");
    let mut registration_documents = vec![Arc::clone(&document)];
    if with_dialogue_profile {
        registration_documents.push(Arc::clone(&manifest));
    }
    let facts = ProjectRegistrationFacts::try_new(
        world,
        registration_documents,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let resource_types = Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty());
    let mut context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::clone(&resource_types),
        None,
        None,
    );
    if with_dialogue_profile {
        let accepted = Arc::new(
            SourceBackedManifest::decode(Arc::clone(&manifest)).expect("accepted test manifest"),
        );
        let profile_id = ProfileId::new("dev").expect("profile ID");
        let resolved = accepted
            .resolve_profile(LaunchProfileSelection::Explicit(profile_id.as_str()))
            .expect("resolved dialogue test profile");
        let topology_revision =
            SourceSetRevision::try_for_identities([manifest.identity(), document.identity()])
                .expect("dialogue test topology revision");
        context = context.with_accepted_launch_profile(AcceptedLaunchProfileInput::new(
            accepted,
            profile_id,
            resolved,
            topology_revision,
            resource_types,
        ));
    }
    (project, context)
}

fn package(id: &str) -> PackageSpec {
    PackageSpec {
        id: PackageId::new(id).expect("package ID"),
        version: PackageVersion::new("0.1.0").expect("package version"),
    }
}

fn manifest_document(name: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-project://{name}/arcw.toml"))
                .expect("manifest document ID"),
            SourceName::path("arcw.toml"),
            format!("schema = 1\n[package]\nid = \"org.arcweft.{name}\"\nversion = \"0.1.0\"\n"),
        )
        .expect("manifest document"),
    )
}

#[test]
fn runtime_call_facts_retain_named_argument_source_order_and_abi_destinations() {
    let (project, context) = removed_role_project(
        r#"
fn reorder(first: String, second: String) -> String { first }

flow main() -> String {
    return reorder(second = "second", first = "first")
}
"#,
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("named project call compiles through runtime fact publication");
    let call = compiled
        .runtime_facts()
        .calls()
        .map(|(_, call)| call)
        .find(|call| call.operands().len() == 2)
        .expect("project call has two physical operands");
    assert_eq!(
        call.operands()
            .iter()
            .map(|operand| (operand.origin().clone(), operand.abi_position()))
            .collect::<Vec<_>>(),
        vec![
            (
                arcweft_runtime_plan::semantic_facts::RuntimeResolvedCallOperandOrigin::Argument {
                    argument: 0,
                    slot: 0,
                },
                1,
            ),
            (
                arcweft_runtime_plan::semantic_facts::RuntimeResolvedCallOperandOrigin::Argument {
                    argument: 1,
                    slot: 0,
                },
                0,
            ),
        ]
    );
    assert_eq!(
        call.abi_operands()
            .map(|operand| operand.abi_position())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[test]
fn selected_need_producer_projects_its_instantiated_result_and_admission() {
    let (project, context) = removed_role_project(
        r#"
flow main(background: Ref<Asset>) -> i64 {
    let pending = asset.image(background)
    return 1i64
}
"#,
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("a selected standard Need producer has a closed runtime fact");
    let producers = compiled
        .runtime_facts()
        .calls()
        .filter_map(|(owner, call)| call.need_producer().map(|producer| (owner, call, producer)))
        .collect::<Vec<_>>();
    let [(owner, call, producer)] = producers.as_slice() else {
        panic!("one selected asset producer call is projected")
    };

    assert!(matches!(
        producer.plan().request(),
        arcweft_core::task::NeedProducerRequestProjection::AssetLoad {
            kind: arcweft_core::task::AssetLoadKind::Image,
            ..
        }
    ));
    assert_eq!(
        producer.plan().policy(),
        arcweft_core::task::TaskPolicy::JoinSameKey
    );
    assert_eq!(
        compiled.runtime_facts().expression_type(*owner),
        Some(producer.need_type()),
        "producer item T must be the exact selected call result Need<T>"
    );
    let arcweft_runtime_plan::semantic_facts::RuntimeTypeShape::Need(item) =
        producer.need_type().shape()
    else {
        panic!("selected producer result is Need<T>")
    };
    assert!(matches!(
        item.shape(),
        arcweft_runtime_plan::semantic_facts::RuntimeTypeShape::Result { .. }
    ));
    assert_eq!(producer.admission().arguments().len(), 1);
    assert_eq!(
        producer.admission().arguments()[0].ty().as_bytes(),
        call.operands()[0].ty().identity().as_bytes(),
        "the admission and runtime call retain the same checked argument type"
    );
    assert!(matches!(
        call.operands()[0].source(),
        arcweft_runtime_plan::semantic_facts::RuntimeResolvedCallOperandSource::Expression(_)
    ));
    assert!(matches!(
        call.dispatch(),
        arcweft_runtime_plan::semantic_facts::RuntimeResolvedCallDispatch::Static(
            arcweft_runtime_plan::semantic_facts::RuntimeResolvedStaticCallTarget::Registered(_)
        )
    ));
}

#[test]
fn runtime_project_materialization_indexes_the_source_row_for_rest_spread() {
    let (project, context) = removed_role_project(
        r#"
fn collect(head: i64, tail: ...i64) -> i64 { head }

flow main() -> i64 {
    return collect(1i64, [2i64, 3i64]...)
}
"#,
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("rest-spread project call compiles through runtime fact publication");
    let call = compiled
        .runtime_facts()
        .calls()
        .map(|(_, call)| call)
        .find(|call| call.project_function().is_some())
        .expect("project call fact");
    assert_eq!(
        call.operands()
            .iter()
            .map(|operand| (operand.origin().clone(), operand.abi_position()))
            .collect::<Vec<_>>(),
        vec![
            (
                arcweft_runtime_plan::semantic_facts::RuntimeResolvedCallOperandOrigin::Argument {
                    argument: 0,
                    slot: 0,
                },
                0,
            ),
            (
                arcweft_runtime_plan::semantic_facts::RuntimeResolvedCallOperandOrigin::Argument {
                    argument: 1,
                    slot: 0,
                },
                1,
            ),
            (
                arcweft_runtime_plan::semantic_facts::RuntimeResolvedCallOperandOrigin::Argument {
                    argument: 1,
                    slot: 1,
                },
                2,
            ),
        ]
    );
    let materialization = call
        .project_function()
        .expect("project materialization")
        .current_group_materialization();
    assert_eq!(materialization.len(), 2);
    assert_eq!(materialization[0].operand_indices(), &[0]);
    assert_eq!(materialization[1].operand_indices(), &[1, 2]);
    assert!(matches!(
        materialization[1].kind(),
        arcweft_lang_hir::item::HirParameterKind::RestPositional
    ));
    assert!(matches!(
        call.operands()[1].projection(),
        arcweft_runtime_plan::semantic_facts::RuntimeResolvedCallOperandProjection::Scalar
    ));
}

#[test]
fn generic_project_function_instances_close_one_body_under_distinct_runtime_types() {
    let (project, context) = removed_role_project(
        r#"
fn identity<T>(value: T) -> T { value }

flow main() -> i64 {
    identity(1i64)
    identity("text")
    return 0i64
}
"#,
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("one generic body publishes two closed runtime instances");
    let executable = compiled
        .analysis_lease()
        .hir_project()
        .analysis_view()
        .expect("accepted executable project");
    let (_, module) = executable.modules().next().expect("root module");
    let (function_owner, parameter_local) = module
        .items()
        .find_map(|(owner, item)| {
            let HirItemKind::Function(function) = item.kind() else {
                return None;
            };
            (function.name().resolved().map(|name| name.as_str()) == Some("identity")).then(|| {
                (
                    owner,
                    function.parameter_groups()[0].parameters()[0].locals()[0],
                )
            })
        })
        .expect("identity Function owner");
    let instances = compiled
        .runtime_facts()
        .project_function_instances()
        .filter(|instance| instance.callable().owner() == function_owner)
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    assert_ne!(instances[0].key(), instances[1].key());
    let mut closed = instances
        .iter()
        .map(|instance| {
            let arcweft_runtime_plan::semantic_facts::RuntimeTypeShape::Function {
                parameters,
                result,
                ..
            } = instance.function_type().shape()
            else {
                panic!("closed instance function type")
            };
            assert_eq!(parameters.len(), 1);
            assert_eq!(parameters[0], **result);
            assert_eq!(
                instance.semantics().local_type(parameter_local),
                Some(parameters.first().expect("closed parameter type")),
            );
            parameters[0].identity()
        })
        .collect::<Vec<_>>();
    closed.sort();
    closed.dedup();
    assert_eq!(
        closed.len(),
        2,
        "i64 and String remain distinct closed types"
    );
}

#[test]
fn generic_project_closure_instances_are_closed_per_parent_without_global_fallback() {
    let (project, context) = removed_role_project(
        r#"
fn make_reader<T>(value: T) -> ((Unit) -> T effects {}) {
    |_unit: Unit| -> T { value }
}

flow main() -> i64 {
    let number_reader = make_reader(1i64)
    let _ = number_reader(())
    let text_reader = make_reader("text")
    let _ = text_reader(())
    let affine_reader = make_reader(Vec<Need<i64>>::with_capacity(0usize))
    let _ = affine_reader(())
    return 0i64
}
"#,
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("generic captured closure publishes one closed closure per parent instance");
    let executable = compiled
        .analysis_lease()
        .hir_project()
        .analysis_view()
        .expect("accepted executable project");
    let (_, module) = executable.modules().next().expect("root module");
    let function_owner = module
        .items()
        .find_map(|(owner, item)| match item.kind() {
            HirItemKind::Function(function)
                if function.name().resolved().map(|name| name.as_str()) == Some("make_reader") =>
            {
                Some(owner)
            }
            _ => None,
        })
        .expect("make_reader Function owner");
    let instances = compiled
        .runtime_facts()
        .project_function_instances()
        .filter(|instance| instance.callable().owner() == function_owner)
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 3);
    let mut closure_owner = None;
    let mut definition_origins = BTreeSet::new();
    let mut capture_types = Vec::new();
    let mut capture_modes = BTreeSet::new();
    let mut closed_closures = Vec::new();
    let analysis = compiled.analysis_lease().final_analysis();
    let reachability = crate::lower::project_runtime_reachability(
        executable,
        compiled.analysis_lease().project_symbols(),
        analysis,
        analysis.checked_entries(),
        crate::lower::RuntimeEmissionMode::CheckAll,
    )
    .unwrap();
    for instance in instances {
        let closure = instance
            .semantics()
            .expressions()
            .iter()
            .find_map(|expression| match expression.payload() {
                arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionExpressionPayload::Closure(
                    closure,
                ) => Some(closure.as_ref()),
                _ => None,
            })
            .expect("instance semantic catalog owns its explicit closure");
        assert_eq!(
            closure.key().enclosing_owner(),
            Some(
                &arcweft_runtime_plan::semantic_facts::RuntimeClosureLexicalOwner::ProjectFunction(
                    instance.key().clone(),
                )
            ),
        );
        match closure_owner {
            Some(owner) => assert_eq!(owner, closure.owner()),
            None => closure_owner = Some(closure.owner()),
        }
        let [capture] = closure.captures() else {
            panic!("closure captures the generic parameter once")
        };
        capture_modes.insert(capture.transfer().mode());
        definition_origins.insert((closure.definition_identity(), capture.origin().clone()));
        let definition = analysis
            .execution_projection()
            .closure_execution(&reachability, closure.owner())
            .unwrap();
        assert_eq!(
            closure.definition_identity(),
            definition.definition_identity()
        );
        assert_eq!(capture.origin(), definition.captures()[0].origin());
        let [formal] = closure.parameters() else {
            panic!("one complete closure formal")
        };
        assert_eq!(
            formal.definition().definition_identity(),
            closure.definition_identity()
        );
        assert_eq!(
            formal.definition().authority(),
            closure.semantics().local_uses()
        );
        assert_eq!(
            formal.passing(),
            arcweft_core::plan::RuntimeFunctionParameterPassing::Value
        );
        assert!(matches!(
            arcweft_runtime_plan::semantic_facts::RuntimeClosureInstanceFact::try_new(
                closure.key().clone(), definition, closure.owner(), closure.function_type().clone(),
                closure.suspension(), closure.control(), closure.execution(), closure.effects().into(),
                closure.scope(), closure.body(), closure.parameters().into(), Box::new([]),
                closure.semantics().clone(),
            ),
            Err(arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionFactError::InvalidClosureInstance)
        ), "an otherwise valid instance cannot omit an accepted capture row");
        assert_eq!(
            closure.semantics().ty(
                arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionTypeOwner::Local(
                    capture.source()
                )
            ),
            None,
            "the captured source is not a local declaration of the closure body",
        );
        assert_eq!(
            closure.semantics().local_type(capture.source()),
            Some(capture.ty()),
            "the closure resolves the source through its own closed capture inventory",
        );
        assert_eq!(
            instance.semantics().local_type(capture.source()),
            Some(capture.ty()),
        );
        assert_eq!(
            closure.semantics().expression_type(closure.body()),
            Some(capture.ty()),
        );
        assert!(
            compiled
                .runtime_facts()
                .expression_type(closure.body())
                .is_none(),
            "instance closure body must not fall back to the global semantic catalog",
        );
        capture_types.push(capture.ty().identity());
        closed_closures.push(closure.clone());
    }
    capture_types.sort();
    assert_eq!(
        definition_origins.len(),
        1,
        "generic instances share one accepted lexical definition and capture origin"
    );
    capture_types.dedup();
    assert_eq!(
        capture_types.len(),
        3,
        "i64, String and affine Vec<Need<i64>> captures stay closed"
    );
    assert_eq!(
        capture_modes,
        BTreeSet::from([
            arcweft_lang_sema::final_analysis::CheckedLocalReadMode::Copy,
            arcweft_lang_sema::final_analysis::CheckedLocalReadMode::Move,
        ])
    );
    let copy = closed_closures
        .iter()
        .find(|closure| {
            closure.captures()[0].transfer().mode()
                == arcweft_lang_sema::final_analysis::CheckedLocalReadMode::Copy
        })
        .unwrap();
    let moved = closed_closures
        .iter()
        .find(|closure| {
            closure.captures()[0].transfer().mode()
                == arcweft_lang_sema::final_analysis::CheckedLocalReadMode::Move
        })
        .unwrap();
    assert_eq!(copy.captures()[0].source(), moved.captures()[0].source());
    let foreign_formal =
        arcweft_runtime_plan::semantic_facts::RuntimeClosureParameterFact::try_new(
            moved.parameters()[0].definition().clone(),
            copy.parameters()[0].ty().clone(),
        )
        .unwrap();
    assert_eq!(
        foreign_formal.definition().identity(),
        copy.parameters()[0].definition().identity()
    );
    assert_ne!(
        foreign_formal.definition().authority(),
        copy.parameters()[0].definition().authority()
    );
    assert!(matches!(arcweft_runtime_plan::semantic_facts::RuntimeClosureInstanceFact::try_new(
        copy.key().clone(), copy.definition().clone(), copy.owner(), copy.function_type().clone(),
        copy.suspension(), copy.control(), copy.execution(), copy.effects().into(),
        copy.scope(), copy.body(), Box::new([foreign_formal]), copy.captures().into(),
        copy.semantics().clone(),
    ), Err(arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionFactError::InvalidClosureInstance)),
        "an equal source and Unit type cannot substitute another closed formal authority");
    let foreign_transfer =
        arcweft_runtime_plan::semantic_facts::RuntimeClosureCaptureFact::try_new(
            copy.definition().captures()[0].clone(),
            moved.captures()[0].transfer().clone(),
            copy.captures()[0].ty().clone(),
        )
        .unwrap();
    assert!(matches!(arcweft_runtime_plan::semantic_facts::RuntimeClosureInstanceFact::try_new(
        copy.key().clone(), copy.definition().clone(), copy.owner(), copy.function_type().clone(),
        copy.suspension(), copy.control(), copy.execution(), copy.effects().into(),
        copy.scope(), copy.body(), copy.parameters().into(), Box::new([foreign_transfer]),
        copy.semantics().clone(),
    ), Err(arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionFactError::InvalidClosureInstance)),
        "an equal lexical local cannot substitute another closed instance's transfer mode");
}

#[test]
fn closure_definition_and_capture_origins_survive_source_revisions() {
    fn observe(source: &str) -> arcweft_runtime_plan::semantic_facts::RuntimeClosureInstanceFact {
        let (project, context) = removed_role_project(source);
        let (mut session, parsed_sources) = compilation_state(&project);
        let compiled = compile_project(&mut session, &project, &parsed_sources, &context).unwrap();
        let closures = compiled.runtime_facts().root_closures().collect::<Vec<_>>();
        assert_eq!(closures.len(), 1);
        closures[0].clone()
    }
    let source = "flow main() -> i64 { let first = 20i64; let second = 22i64; let callback = || first + second; return callback() }";
    let original = observe(source);
    let revised = observe(&format!(
        "fn unrelated() -> i64 {{ 99i64 }}\n{}",
        source.replace("first + second", "first + second + 1i64")
    ));
    assert_ne!(original.key(), revised.key());
    assert_eq!(
        original.definition_identity(),
        revised.definition_identity()
    );
    let origins = |closure: &arcweft_runtime_plan::semantic_facts::RuntimeClosureInstanceFact| {
        closure
            .captures()
            .iter()
            .map(|capture| (capture.position(), capture.origin().clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(origins(&original), origins(&revised));
    assert_eq!(original.captures().len(), 2);
    assert_ne!(
        original.captures()[0].origin(),
        original.captures()[1].origin()
    );
    assert_eq!(
        original.captures()[0].origin().semantic_digest().unwrap(),
        revised.captures()[0].origin().semantic_digest().unwrap()
    );
    assert_ne!(
        original.captures()[0].origin().semantic_digest().unwrap(),
        original.captures()[1].origin().semantic_digest().unwrap()
    );
    assert!(matches!(
        arcweft_runtime_plan::semantic_facts::RuntimeClosureCaptureFact::try_new(
            original.definition().captures()[0].clone(),
            original.captures()[1].transfer().clone(),
            original.captures()[0].ty().clone(),
        ),
        Err(arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionFactError::InvalidClosureInstance)
    ));
    assert_ne!(
        original.definition_identity(),
        observe(&source.replace("flow main", "flow other")).definition_identity()
    );
}

#[test]
fn root_closure_capture_transfer_proof_drives_creation_and_ingress() {
    use arcweft_core::{
        plan::{FlowOp, RuntimeFunctionInputOwnershipRequirement, RuntimeFunctionSemanticRole},
        value::{RuntimeExprKind, RuntimeLocalReadMode},
    };
    use arcweft_lang_sema::final_analysis::CheckedLocalReadMode;
    for (source, mode, read_mode, ownership) in [
        (
            "flow main() -> i64 { let value = 42i64; let callback = || value; return callback() }",
            CheckedLocalReadMode::Copy,
            RuntimeLocalReadMode::Copy,
            RuntimeFunctionInputOwnershipRequirement::Unrestricted,
        ),
        (
            "flow main() -> i64 { let value = Vec<Need<i64>>::with_capacity(0usize); let callback = || value; let _ = callback(); return 42i64 }",
            CheckedLocalReadMode::Move,
            RuntimeLocalReadMode::Move,
            RuntimeFunctionInputOwnershipRequirement::Owned,
        ),
    ] {
        let (project, context) = removed_role_project(source);
        let (mut session, parsed_sources) = compilation_state(&project);
        let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
            .expect("closure retains its selected capture transfer");
        let closure = compiled
            .runtime_facts()
            .root_closures()
            .find(|closure| !closure.captures().is_empty())
            .expect("capturing root closure");
        let [capture] = closure.captures() else {
            panic!("one capture")
        };
        assert_eq!(capture.transfer().mode(), mode);
        let plan = &compiled.runtime_plan().plan;
        let sites = plan
            .function_sites()
            .iter()
            .filter(|site| {
                site.role() == RuntimeFunctionSemanticRole::Closure
                    && site.capture_inputs().count() == 1
            })
            .collect::<Vec<_>>();
        let [site] = sites.as_slice() else {
            panic!("one capturing function site")
        };
        assert_eq!(site.capture_inputs().next().unwrap().ownership(), ownership);
        let mut creation_reads = Vec::new();
        plan.try_visit_flow_ops(&mut |operation| {
            if let FlowOp::Let { expr, .. } = operation
                && let RuntimeExprKind::MakeCallable { captures, .. } = expr.kind()
            {
                for capture in captures {
                    let RuntimeExprKind::Local(read) = capture.kind() else {
                        panic!("capture creation uses the admitted local read")
                    };
                    creation_reads.push(read.mode());
                }
            }
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
        assert_eq!(creation_reads, vec![read_mode]);
    }
}

#[test]
fn generic_project_dialogue_instances_own_closed_templates_without_global_fallback() {
    let (project, context) = removed_role_dialogue_project(
        r#"
pub character alice { display = "Alice" }

fn speak<T>(value: T) {
    alice[#[value]];
}

flow main() {
    speak(1i64);
    speak("text");
}
"#,
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("generic dialogue publishes one closed content occurrence per instance");
    let executable = compiled
        .analysis_lease()
        .hir_project()
        .analysis_view()
        .expect("accepted executable project");
    let (_, module) = executable.modules().next().expect("root module");
    let function_owner = module
        .items()
        .find_map(|(owner, item)| match item.kind() {
            HirItemKind::Function(function)
                if function.name().resolved().map(|name| name.as_str()) == Some("speak") =>
            {
                Some(owner)
            }
            _ => None,
        })
        .expect("speak Function owner");
    let instances = compiled
        .runtime_facts()
        .project_function_instances()
        .filter(|instance| instance.callable().owner() == function_owner)
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    assert!(
        compiled
            .runtime_facts()
            .dialogue_content_fragments()
            .is_empty(),
        "closed generic content must not leak into the global semantic catalog",
    );
    let mut templates = Vec::new();
    let mut slot_types = Vec::new();
    for instance in instances {
        let mut applications = 0_usize;
        instance.visit_dialogue_applications(&mut |_, owner, application| {
            applications += 1;
            let fragment = instance
                .semantics()
                .dialogue_content_fragment_for_source(owner)
                .expect("instance dialogue source owns its closed fragment");
            assert_eq!(
                fragment.template().id(),
                application.content().template_id()
            );
            let [slot] = fragment.values() else {
                panic!("generic interpolation owns one closed value slot")
            };
            templates.push(fragment.template().id());
            slot_types.push(slot.ty().identity());
        });
        assert_eq!(applications, 1);
    }
    templates.sort();
    templates.dedup();
    slot_types.sort();
    slot_types.dedup();
    assert_eq!(templates.len(), 2, "template identities are plan-unique");
    assert_eq!(
        slot_types.len(),
        2,
        "interpolation types close per instance"
    );
}

#[test]
fn generic_dialogue_interpolation_rejects_an_unsupported_closed_instance() {
    let (project, context) = removed_role_dialogue_project(
        r#"
pub character alice { display = "Alice" }

fn speak<T>(value: T) {
    alice[#[value]];
}

flow main() {
    speak((1i64, 2i64));
}
"#,
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let error = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect_err("a tuple instance has no selected DisplayText implementation");
    assert!(
        error.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .diagnostic()
                .message()
                .contains("closed generic interpolation has no supported DisplayText witness")
        }),
        "diagnostics={:?}",
        error.diagnostics()
    );
}

#[test]
fn selected_project_display_methods_are_reachable_from_fmt_and_interpolation() {
    use arcweft_lang_hir::project::{
        HirRuntimeExecutableOwner, HirRuntimeReachabilityEdgeKind, HirRuntimeReachabilitySite,
    };
    use arcweft_lang_sema::checked_rich_text::{CheckedDialogueToken, CheckedDisplayWitness};

    let (project, context) = removed_role_dialogue_project(
        r#"
pub character alice { display = "Alice" }
struct RouteInfo { label: String }
impl DisplayText for RouteInfo {
    fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> {
        Ok(fmt(self.label))
    }
}

flow main(value: RouteInfo) {
    let formatted = fmt(value, style="currency", locale="ja-JP", currency="JPY");
    alice[#[value]];
}
"#,
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("selected project display methods compile");
    let analysis = compiled.analysis_lease();
    let executable = analysis.hir_project().analysis_view().unwrap();
    let reachability = lower::project_runtime_reachability(
        executable,
        analysis.project_symbols(),
        analysis.final_analysis(),
        analysis.checked_entries(),
        lower::RuntimeEmissionMode::CheckAll,
    )
    .expect("selected display reachability");
    let (fmt_call, conformance) = analysis
        .final_analysis()
        .calls()
        .find_map(|(owner, facts)| {
            facts
                .selected_application()?
                .format_call()?
                .witness()
                .project_conformance()
                .map(|conformance| (owner, conformance))
        })
        .expect("fmt selects a project DisplayText method");
    let interpolation = analysis
        .final_analysis()
        .expressions()
        .find_map(|(_, checked)| {
            let arcweft_lang_sema::final_analysis::CheckedExpressionResolution::DialogueApplication {
                rich_text,
                ..
            } = checked.resolution() else {
                return None;
            };
            rich_text.content().tokens().iter().find_map(|token| {
                let CheckedDialogueToken::Interpolation {
                    expression,
                    witness: Some(CheckedDisplayWitness::Project(selected)),
                    ..
                } = token else {
                    return None;
                };
                (selected.method_declaration() == conformance.method_declaration())
                    .then_some(*expression)
            })
        })
        .expect("plain interpolation selects the same project method");
    let target = HirRuntimeExecutableOwner::ImplMethod(conformance.method_declaration().clone());
    assert!(reachability.contains_runtime_owner(&target));
    let expected_sources = [fmt_call, interpolation]
        .into_iter()
        .map(HirRuntimeReachabilitySite::Expression)
        .collect::<std::collections::BTreeSet<_>>();
    let actual_sources = reachability
        .edges()
        .filter_map(|edge| match edge.kind() {
            HirRuntimeReachabilityEdgeKind::CheckedSelectedTraitMethod {
                implementation,
                member,
                method,
                ..
            } if *implementation == conformance.implementation()
                && *member == conformance.method_ordinal()
                && method == conformance.method_declaration()
                && edge.target() == &target =>
            {
                Some(edge.source())
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual_sources, expected_sources);
    let method_owners = reachability.executable_owners(&target).unwrap();
    assert!(method_owners.locals().count() >= 2);
    assert!(method_owners.expressions().count() >= 2);
    assert!(method_owners.types().count() >= 1);

    let methods = compiled
        .runtime_facts()
        .trait_methods()
        .filter(|method| {
            method.trait_identity()
                == &arcweft_runtime_plan::semantic_facts::RuntimeTraitIdentity::StandardDisplayText
        })
        .collect::<Vec<_>>();
    assert_eq!(methods.len(), 1);
    let method = methods[0];
    assert_eq!(method.declaration(), conformance.method_declaration());
    let semantics = method
        .closed_semantics()
        .expect("DisplayText method is closed");
    assert_eq!(semantics.partition().executable(), &target);
    assert!(semantics.type_projection().len() >= 3);
    let mut project_slots = 0;
    compiled
        .runtime_facts()
        .visit_dialogue_content_fragments(&mut |_, fragment| {
            for value in fragment.values() {
                let Some(project) = value.project_display() else {
                    continue;
                };
                project_slots += 1;
                assert_eq!(value.expression(), interpolation);
                assert_eq!(
                    value.role(),
                    arcweft_core::plan::RuntimeDialogueValueRole::Content
                );
                assert_eq!(project.method(), method.key());
                assert!(
                    compiled
                        .runtime_facts()
                        .format_template(project.template())
                        .is_some()
                );
            }
        });
    assert_eq!(project_slots, 1);
}

#[test]
fn generic_display_text_publishes_distinct_closed_method_instances() {
    let (project, context) = removed_role_project(
        r#"
struct Route<T> { label: T }
impl<T> DisplayText for Route<T> {
    fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> {
        let label = self.label;
        let render = || fmt(label);
        Ok(render())
    }
}

flow main(number: Route<i32>, word: Route<String>) {
    let first = fmt(number);
    let second = fmt(word);
}
"#,
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("two closed Route display instances compile");
    let methods = compiled
        .runtime_facts()
        .trait_methods()
        .filter(|method| {
            method.trait_identity()
                == &arcweft_runtime_plan::semantic_facts::RuntimeTraitIdentity::StandardDisplayText
        })
        .collect::<Vec<_>>();
    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0].declaration(), methods[1].declaration());
    assert_ne!(methods[0].key(), methods[1].key());
    for method in methods {
        let semantics = method
            .closed_semantics()
            .expect("generic method body is closed");
        assert!(semantics.partition().locals().len() >= 2);
        assert!(semantics.partition().expressions().len() >= 2);
        let closure = semantics
            .expressions()
            .iter()
            .find_map(|expression| match expression.payload() {
                arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionExpressionPayload::Closure(closure) => Some(closure.as_ref()),
                _ => None,
            })
            .expect("generic method owns its nested formatter closure");
        assert_eq!(
            closure.key().enclosing_owner(),
            Some(
                &arcweft_runtime_plan::semantic_facts::RuntimeClosureLexicalOwner::TraitMethod(
                    method.key().clone()
                )
            )
        );
        assert!(
            semantics
                .type_projection()
                .iter()
                .all(|row| row.ty().is_some()
                    || matches!(
            row.owner(),
            arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionTypeOwner::Expression(_)
        ))
        );
    }
}

#[test]
fn project_display_context_runs_from_source_flow() {
    use arcweft_core::{
        engine::{Engine, FlowExit, FlowFiberStatus},
        step::{RuntimeStepInput, RuntimeStepOptions},
        value::{
            RuntimeDialogueContentBinding, RuntimeDialogueContentValue,
            RuntimeDialogueFormattedOutcome, RuntimeDialogueFormattedSuccess,
        },
    };

    fn content_text(content: &RuntimeDialogueContentValue) -> String {
        let [RuntimeDialogueContentBinding::Formatted { value, .. }] = content.bindings() else {
            panic!("one fmt call produces one formatted Content binding");
        };
        let RuntimeDialogueFormattedOutcome::Success { value, .. } = value.outcome() else {
            panic!("project DisplayText formatting succeeds");
        };
        match value {
            RuntimeDialogueFormattedSuccess::Text(text) => text.clone(),
            RuntimeDialogueFormattedSuccess::Content(nested) => content_text(nested),
        }
    }

    let (project, context) = removed_role_project(
        r#"
struct LocaleProbe { marker: i32 }
struct StyleProbe { marker: i32 }
struct CurrencyProbe { marker: i32 }
impl DisplayText for LocaleProbe {
    fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> {
        Ok(fmt(ctx.locale))
    }
}
impl DisplayText for StyleProbe {
    fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> {
        Ok(fmt(ctx.style, none="missing"))
    }
}
impl DisplayText for CurrencyProbe {
    fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> {
        Ok(fmt(ctx.currency, none="missing"))
    }
}
flow main() -> String {
    let rendered_locale = fmt(LocaleProbe { marker = 0i32 }, style="currency", locale="ja-JP", currency="JPY");
    let rendered_style = fmt(StyleProbe { marker = 0i32 }, style="currency", locale="ja-JP", currency="JPY");
    let rendered_currency = fmt(CurrencyProbe { marker = 0i32 }, style="currency", locale="ja-JP", currency="JPY");
    return "done"
}
"#,
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("source project DisplayText context compiles");
    let mut plan = compiled.runtime_plan().plan.clone();
    plan.bind_artifact(
        arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x6c; 32])
            .expect("test artifact fingerprint"),
    )
    .expect("source plan binds its test artifact");
    let flow = plan
        .flows()
        .first()
        .expect("authored flow is present")
        .id
        .clone();
    let mut engine = Engine::for_flow(plan, &flow).expect("authored flow starts");
    let mut actual = std::collections::BTreeSet::new();
    let mut completed = false;
    for _ in 0..128 {
        let output = engine
            .step(RuntimeStepInput::default(), RuntimeStepOptions::default())
            .output;
        for value in engine
            .fiber()
            .env
            .bindings_snapshot()
            .into_iter()
            .flat_map(|binding| binding.into_values())
        {
            if let Ok(content) = RuntimeDialogueContentValue::try_from_runtime_value(&value) {
                actual.insert(content_text(&content));
            }
        }
        match &engine.fiber().status {
            FlowFiberStatus::Done(FlowExit::Return(value)) => {
                assert_eq!(value, "done");
                completed = true;
                break;
            }
            FlowFiberStatus::Failed(reason) => {
                panic!(
                    "source DisplayContext flow failed: {reason}; diagnostics={:?}",
                    output.diagnostics
                )
            }
            _ => {}
        }
    }
    assert!(completed, "source DisplayContext flow completes");
    assert_eq!(
        actual,
        ["ja-JP", "currency", "JPY"].map(str::to_owned).into()
    );
}
#[test]
fn source_project_call_failure_enters_fmt_inline_fallback() {
    use arcweft_core::{
        engine::{Engine, FlowExit, FlowFiberStatus},
        step::{RuntimeStepInput, RuntimeStepOptions},
        value::{
            RuntimeDialogueContentBinding, RuntimeDialogueContentValue,
            RuntimeDialogueFormattedFailureSelection, RuntimeDialogueFormattedOutcome,
            RuntimeValue,
        },
    };

    fn is_recovered_fallback(failure: &RuntimeDialogueFormattedFailureSelection) -> bool {
        let RuntimeDialogueFormattedFailureSelection::OnError(RuntimeValue::Variant {
            name: policy,
            payload: Some(fallback),
            ..
        }) = failure
        else {
            return false;
        };
        let RuntimeValue::Variant {
            name: source,
            payload: Some(value),
            ..
        } = fallback.as_ref()
        else {
            return false;
        };
        let RuntimeValue::Tuple(fields) = value.as_ref() else {
            return false;
        };
        policy == "Fallback"
            && source == "Text"
            && matches!(fields.first(), Some(RuntimeValue::String(text)) if text == "recovered")
    }

    let (project, context) = removed_role_project(
        r#"
struct Route { label: i32 }
impl DisplayText for Route {
    fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> {
        Ok(fmt(self.label))
    }
}
fn source(divisor: i32) -> Route { Route { label = 42i32 / divisor } }
fn choose_style() -> String { "number" }
fn fallback_text() -> String { "recovered" }
flow main() -> String {
    let rendered = fmt(source(0i32), style=choose_style(), on_error=InlineFailure.fallback(fallback_text()));
    return "done"
}
"#,
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("Flow-lowered fmt operands compile as one selected attempt");
    let mut plan = compiled.runtime_plan().plan.clone();
    plan.bind_artifact(
        arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x6d; 32])
            .expect("test artifact fingerprint"),
    )
    .expect("source plan binds its test artifact");
    assert!(plan.format_attempts().iter().any(|attempt| {
        attempt
            .operands()
            .iter()
            .map(|operand| operand.parameter())
            .eq([
                arcweft_core::value::RuntimeFmtParameterId::Value,
                arcweft_core::value::RuntimeFmtParameterId::Style,
                arcweft_core::value::RuntimeFmtParameterId::OnError,
            ])
    }));
    let flow = plan
        .flows()
        .first()
        .expect("authored flow is present")
        .id
        .clone();
    let mut engine = Engine::for_flow(plan, &flow).expect("authored flow starts");
    let mut observed_fallback = false;
    let mut observed_style = false;
    let mut completed = false;
    for _ in 0..128 {
        let output = engine
            .step(RuntimeStepInput::default(), RuntimeStepOptions::default())
            .output;
        for value in engine
            .fiber()
            .env
            .bindings_snapshot()
            .into_iter()
            .flat_map(|binding| binding.into_values())
        {
            observed_style |= matches!(&value, RuntimeValue::String(text) if text == "number");
            let Ok(content) = RuntimeDialogueContentValue::try_from_runtime_value(&value) else {
                continue;
            };
            let [RuntimeDialogueContentBinding::Formatted { value, .. }] = content.bindings()
            else {
                continue;
            };
            observed_fallback |= matches!(value.outcome(), RuntimeDialogueFormattedOutcome::Failure { reason, .. } if reason.contains("division by zero"))
                && is_recovered_fallback(value.failure_selection());
        }
        match &engine.fiber().status {
            FlowFiberStatus::Done(FlowExit::Return(value)) => {
                assert_eq!(value, "done");
                completed = true;
                break;
            }
            FlowFiberStatus::Failed(reason) => {
                panic!(
                    "source fmt attempt failed: {reason}; diagnostics={:?}",
                    output.diagnostics
                )
            }
            _ => {}
        }
    }
    assert!(completed, "recoverable source fmt flow completes");
    assert!(observed_style, "later style operand is evaluated");
    assert!(
        observed_fallback,
        "source failure retains the evaluated fallback"
    );
}

fn dialogue_manifest_document(name: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-project://{name}/arcw.toml"))
                .expect("manifest document ID"),
            SourceName::path("arcw.toml"),
            format!(
                "schema = 1\n[package]\nid = \"org.arcweft.{name}\"\nversion = \"0.1.0\"\n\n[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n"
            ),
        )
        .expect("dialogue manifest document"),
    )
}

#[test]
fn recovered_source_commits_poisoned_hir_for_tooling() {
    for source in [
        "state GameState {\n    value: i32\n}\n",
        "reducer update(state: GameState, event: GameEvent) -> GameState {\n    state\n}\n",
        "agent @agent.smoke smoke() {\n    Ok(())\n}\n",
    ] {
        let (project, context) = removed_role_project(source);
        let (mut compiler, parsed_sources) = compilation_state(&project);
        let error = compile_project(&mut compiler, &project, &parsed_sources, &context)
            .expect_err("recovered declaration remains non-executable");
        assert_eq!(error.stage(), ProjectCompileStage::Readiness.as_str());
        let tooling = error
            .compilation_lease()
            .map(crate::project::ProjectCompilationLease::tooling_lease)
            .expect("recovered final HIR publishes one tooling lease");
        assert_eq!(tooling.modules().len(), 1);
        let module = &tooling.modules()[0];
        assert!(!module.hir().is_analysis_ready());
        assert!(Arc::ptr_eq(
            module.hir(),
            tooling
                .hir_project()
                .view()
                .module(module.module())
                .expect("tooling project retains the exact recovered module")
        ));
        assert!(
            tooling
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.syntax_diagnostic().is_some()),
            "syntax recovery diagnostics remain attached to the tooling lease"
        );

        let (mut compiler, parsed_sources) = compilation_state(&project);
        let mut cache = InMemoryProjectCompileCache::default();
        let error = compile_project_with_cache(
            &mut compiler,
            &project,
            &parsed_sources,
            &context,
            &mut cache,
        )
        .expect_err("cached compilation must not execute recovered HIR");
        assert_eq!(error.stage(), ProjectCompileStage::Readiness.as_str());
        assert!(
            error
                .compilation_lease()
                .map(crate::project::ProjectCompilationLease::tooling_lease)
                .is_some()
        );
    }
}

#[test]
fn project_compile_diagnostics_own_typed_diagnostic_and_source_snapshot() {
    let source_text = "flow @flow.opening start {\n}\n";
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("src/main.arcw").expect("document id"),
            SourceName::path("src/main.arcw"),
            source_text,
        )
        .expect("source document"),
    );
    let source = ProjectSourceFile::new(
        CanonicalModulePath::crate_root(),
        PathBuf::from("src/main.arcw"),
        Arc::clone(&document),
        [],
    );
    let span = document
        .span(SourceRange::new(5, 18))
        .expect("diagnostic span");
    let error = module_error(
        &source,
        &document,
        ProjectCompileStage::Parse,
        [Diagnostic::new(DiagnosticSeverity::Error, "parse failed")
            .with_code("syntax.parse")
            .with_label(DiagnosticLabel::primary(
                span,
                Some("found token here".to_owned()),
            ))],
    );

    let diagnostic = error.diagnostics().first().expect("diagnostic");
    assert!(diagnostic.syntax_diagnostic().is_none());
    assert_eq!(
        diagnostic.module(),
        Some(&CanonicalModulePath::crate_root())
    );
    assert_eq!(diagnostic.stage(), ProjectCompileStage::Parse);
    assert_eq!(
        diagnostic.diagnostic().code().expect("code").as_str(),
        "syntax.parse"
    );
    assert_eq!(
        diagnostic.source().expect("source").text(),
        Some(source_text)
    );
    assert_eq!(
        diagnostic.source().expect("source").name().display_name(),
        "src/main.arcw"
    );
}

#[test]
fn compiled_project_modules_retain_typed_non_blocking_lints() {
    let (project, context) = removed_role_project("flow @flow.opening opening {\n}\n");
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("valid project with a non-blocking syntax warning compiles");
    let lint = compiled.analysis_lease().modules()[0]
        .syntax_lints()
        .iter()
        .find(|lint| lint.code() == SyntaxLintCode::RedundantDeclIdentity)
        .expect("compiled module retains the redundant declaration identity warning");

    assert_eq!(lint.code().stable_code(), "AWF0101");
    assert_eq!(lint.code().domain_name(), "style::redundant_decl_identity");
    assert!(compiled.analysis_lease().syntax_warnings() > 0);
    assert_eq!(
        compiled.analysis_lease().syntax_warnings(),
        compiled
            .analysis_lease()
            .modules()
            .iter()
            .flat_map(CompiledProjectModule::syntax_lints)
            .filter(|lint| lint.severity() == SyntaxLintSeverity::Warning)
            .count()
    );
}

#[test]
fn noop_project_rebuild_reuses_the_exact_accepted_hir_project_arc() {
    let (project, context) = removed_role_project("flow opening {\n}\n");
    let (mut session, parsed_sources) = compilation_state(&project);
    let first = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("first project compilation");
    let retained = Arc::clone(first.analysis_lease().hir_project());

    let second = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("identical project recompilation");

    assert!(Arc::ptr_eq(
        &retained,
        second.analysis_lease().hir_project()
    ));
}

#[test]
fn runtime_dialogue_projection_keeps_the_admitted_profile_product_lease() {
    let (project, context) =
        removed_role_dialogue_project("pub character alice {}\nflow opening { alice[hello]; }\n");
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("dialogue project compiles with its accepted profile");
    let analysis = Arc::clone(compiled.analysis_lease());
    let profile = compiled.dialogue_profile().clone();
    let product = Arc::downgrade(compiled.view_product().product());
    drop(compiled);

    assert!(Arc::ptr_eq(
        profile.product(),
        &product
            .upgrade()
            .expect("the retained profile leases the accepted product"),
    ));
    let executable = analysis
        .hir_project()
        .analysis_view()
        .expect("retained executable HIR");
    let runtime_owners = lower::project_runtime_reachability(
        executable,
        analysis.project_symbols(),
        analysis.final_analysis(),
        analysis.checked_entries(),
        lower::RuntimeEmissionMode::CheckAll,
    )
    .expect("retained runtime reachability");
    let (facts, _) = lower::project_runtime_semantic_facts(
        executable,
        analysis.project_symbols(),
        analysis.registered_world(),
        analysis.final_analysis(),
        &runtime_owners,
        Some(&profile),
        &lower::ProjectInstantiationControl::default(),
    )
    .expect("the retained checked profile is accepted by runtime projection");
    let applications = facts.dialogue_applications().collect::<Vec<_>>();
    let [(_, application)] = applications.as_slice() else {
        panic!("one projected dialogue application")
    };
    assert_eq!(application.content().presentation(), profile.presentation());
    assert_eq!(
        application.content().presentation_revision(),
        profile.revision(),
    );
    assert_eq!(
        profile.revision().view_program_revision(),
        profile
            .product()
            .program()
            .expect("accepted View program")
            .accepted_revision(),
    );
}

#[test]
fn dialogue_line_reference_reaches_runtime_lowering_from_one_accepted_generation() {
    let (project, context) = removed_role_dialogue_project(
        r"
pub character alice {}

fn opening() {
    alice[前#strong()[強調]後];
}

flow reference {
    let selected: Ref<DialogueLine> = @say.fn.org.arcweft.removed-role.function.opening.001
}
",
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("typed dialogue-line reference compiles through runtime lowering");

    let [line] = compiled
        .analysis_lease()
        .final_analysis()
        .dialogue_lines()
        .records()
    else {
        panic!("one accepted dialogue line")
    };
    assert_eq!(
        line.id().as_str(),
        "say.fn.org.arcweft.removed-role.function.opening.001"
    );
    let [reference] = compiled
        .analysis_lease()
        .semantic_index()
        .dialogue_line_references()
    else {
        panic!("one accepted dialogue-line reference")
    };
    assert_eq!(reference.target(), line.id());
}

#[test]
fn multi_module_authored_proof_alias_to_unit_uses_one_semantic_project_transaction() {
    let aliases = CanonicalModulePath::crate_root()
        .join(ModuleSegment::new("aliases").expect("module segment"));
    let root_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://proof-return/src/main.arcw")
                .expect("root document ID"),
            SourceName::path("src/main.arcw"),
            "use crate.aliases.ProofUnit\nproof root_checked() -> ProofUnit {}\n",
        )
        .expect("root source document"),
    );
    let alias_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://proof-return/src/aliases.arcw")
                .expect("alias document ID"),
            SourceName::path("src/aliases.arcw"),
            "pub type ProofUnit = Unit\nproof alias_checked() -> Unit {}\n",
        )
        .expect("alias source document"),
    );
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        package("org.arcweft.proof-return"),
        BuildSpec::default(),
        manifest_document("proof-return"),
        [
            ProjectSourceFile::new(
                CanonicalModulePath::crate_root(),
                PathBuf::from("src/main.arcw"),
                Arc::clone(&root_document),
                [ModuleDependency::new(aliases.clone())],
            ),
            ProjectSourceFile::new(
                aliases,
                PathBuf::from("src/aliases.arcw"),
                Arc::clone(&alias_document),
                [],
            ),
        ],
    )
    .expect("multi-module project sources");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(project.package().id.as_str()).expect("package"),
        root_document.identity().id().clone(),
        "proof-return-test",
    )
    .expect("symbol world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&root_document), Arc::clone(&alias_document)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty()),
        None,
        None,
    );
    let (mut session, parsed_sources) = compilation_state(&project);

    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("semantic Unit aliases admit omitted Proof tails");

    let tooling = compiled.analysis_lease().tooling_lease();
    assert!(Arc::ptr_eq(
        tooling.hir_project(),
        compiled.analysis_lease().hir_project()
    ));
    assert!(std::ptr::eq(
        tooling.project_symbols(),
        compiled.analysis_lease().registered_world().symbols()
    ));
    assert_eq!(compiled.analysis_lease().modules().len(), 2);
    assert_eq!(
        compiled.analysis_lease().hir_project().database_id(),
        session.hir_database_id()
    );
    let mut proofs = 0_usize;
    for module in compiled.analysis_lease().modules() {
        for &item_id in module.hir().source_ordered_items() {
            let item = module.hir().resolve_item(item_id).expect("published item");
            let HirItemKind::Proof(proof) = item.kind() else {
                continue;
            };
            proofs += 1;
            assert_eq!(
                proof.return_semantic_class(),
                HirProofReturnSemanticClass::Unit
            );
            let HirProofBody::Block { tail, .. } = proof.body() else {
                panic!("fixture Proof must retain its authored block")
            };
            assert!(matches!(
                module.hir().resolve_expr(*tail).expect("Proof tail").kind(),
                HirExprKind::Unit
            ));
        }
    }
    assert_eq!(proofs, 2);
}

fn dialogue_collision_project() -> (
    ProjectSources,
    ProjectCompilationContext,
    Arc<SourceDocument>,
    Arc<SourceDocument>,
) {
    let child = CanonicalModulePath::crate_root()
        .join(ModuleSegment::new("child").expect("module segment"));
    let root_text = "pub character alice { display = \"Alice\" }\nfn root_line() {\n    alice(id = @say.shared)[before#strong()[root]after];\n}\n";
    let child_text = "pub character bob { display = \"Bob\" }\nfn child_line() {\n    bob(id = @say.shared)[before#strong()[child]after];\n}\n";
    let root_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://dialogue-collision/src/main.arcw")
                .expect("root document ID"),
            SourceName::path("src/main.arcw"),
            root_text,
        )
        .expect("root source document"),
    );
    let child_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://dialogue-collision/src/child.arcw")
                .expect("child document ID"),
            SourceName::path("src/child.arcw"),
            child_text,
        )
        .expect("child source document"),
    );
    let manifest = dialogue_manifest_document("dialogue-collision");
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        package("org.arcweft.dialogue-collision"),
        BuildSpec::default(),
        Arc::clone(&manifest),
        [
            ProjectSourceFile::new(
                CanonicalModulePath::crate_root(),
                PathBuf::from("src/main.arcw"),
                Arc::clone(&root_document),
                [ModuleDependency::new(child.clone())],
            ),
            ProjectSourceFile::new(
                child,
                PathBuf::from("src/child.arcw"),
                Arc::clone(&child_document),
                [],
            ),
        ],
    )
    .expect("multi-module project sources");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(project.package().id.as_str()).expect("package"),
        root_document.identity().id().clone(),
        "dialogue-collision-test",
    )
    .expect("symbol world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![
            Arc::clone(&root_document),
            Arc::clone(&child_document),
            Arc::clone(&manifest),
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let resource_types = Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty());
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::clone(&resource_types),
        None,
        None,
    );
    let accepted = Arc::new(SourceBackedManifest::decode(Arc::clone(&manifest)).unwrap());
    let profile_id = ProfileId::new("dev").unwrap();
    let resolved = accepted
        .resolve_profile(LaunchProfileSelection::Explicit(profile_id.as_str()))
        .unwrap();
    let revision = SourceSetRevision::try_for_identities([
        manifest.identity(),
        root_document.identity(),
        child_document.identity(),
    ])
    .unwrap();
    let context = context.with_accepted_launch_profile(AcceptedLaunchProfileInput::new(
        accepted,
        profile_id,
        resolved,
        revision,
        resource_types,
    ));
    (project, context, root_document, child_document)
}

#[test]
fn project_dialogue_collision_projects_exact_cross_module_source_labels() {
    let (project, context, root_document, child_document) = dialogue_collision_project();
    let (mut session, parsed_sources) = compilation_state(&project);

    let error = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect_err("duplicate dialogue line IDs reject the project transaction");
    assert_eq!(
        error.stage(),
        ProjectCompileStage::TypeCheck.as_str(),
        "diagnostics={:?}",
        error.diagnostics(),
    );
    let [diagnostic] = error.diagnostics() else {
        panic!("one collision diagnostic")
    };
    assert_eq!(
        diagnostic
            .diagnostic()
            .code()
            .expect("diagnostic code")
            .as_str(),
        "AW-CD-020"
    );
    let labels = diagnostic.diagnostic().labels();
    assert_eq!(labels.len(), 2);
    let root_start = root_document
        .text()
        .find("@say.shared")
        .expect("root ID span");
    let child_start = child_document
        .text()
        .find("@say.shared")
        .expect("child ID span");
    assert_eq!(labels[0].style(), DiagnosticLabelStyle::Primary);
    assert_eq!(
        labels[0].span(),
        &child_document
            .span(SourceRange::new(
                child_start,
                child_start + "@say.shared".len(),
            ))
            .expect("child exact span")
    );
    assert_eq!(labels[1].style(), DiagnosticLabelStyle::Secondary);
    assert_eq!(
        labels[1].span(),
        &root_document
            .span(SourceRange::new(
                root_start,
                root_start + "@say.shared".len(),
            ))
            .expect("root exact span")
    );
}

#[test]
fn failed_project_build_preserves_the_previous_accepted_hir_project_arc() {
    let (accepted_project, accepted_context) = removed_role_project("flow opening {\n}\n");
    let (mut session, accepted_sources) = compilation_state(&accepted_project);
    let accepted = compile_project(
        &mut session,
        &accepted_project,
        &accepted_sources,
        &accepted_context,
    )
    .expect("initial accepted project");
    let retained = Arc::clone(accepted.analysis_lease().hir_project());

    let (collision_project, collision_context, _, _) = dialogue_collision_project();
    let (_, collision_sources) = compilation_state(&collision_project);
    let error = compile_project(
        &mut session,
        &collision_project,
        &collision_sources,
        &collision_context,
    )
    .expect_err("collision candidate rejects without replacing accepted cache");
    assert_eq!(error.stage(), ProjectCompileStage::TypeCheck.as_str());

    let rebuilt = compile_project(
        &mut session,
        &accepted_project,
        &accepted_sources,
        &accepted_context,
    )
    .expect("accepted input remains reusable after rejection");
    assert!(Arc::ptr_eq(
        &retained,
        rebuilt.analysis_lease().hir_project()
    ));
}

#[test]
fn project_parse_diagnostics_retain_the_attached_source_payload() {
    let source = r"pub view Card() {
    export part as card.heading
    Panel().part(header)
}
";
    let (project, context) = removed_role_project(source);
    let (mut compiler, parsed_sources) = compilation_state(&project);
    let error = compile_project(&mut compiler, &project, &parsed_sources, &context)
        .expect_err("malformed View export must remain non-executable");
    assert_eq!(error.stage(), ProjectCompileStage::Readiness.as_str());
    let tooling = error
        .compilation_lease()
        .map(crate::project::ProjectCompilationLease::tooling_lease)
        .expect("recovered View retains a tooling project");
    assert_eq!(tooling.modules().len(), 1);
    assert!(Arc::ptr_eq(
        tooling.modules()[0].parsed().document_lease(),
        tooling.modules()[0].hir().provenance().document()
    ));

    let diagnostic = error
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic
                .syntax_diagnostic()
                .is_some_and(|error| error.code() == "syntax.view.export_missing_local")
        })
        .expect("attached missing-local parser diagnostic");
    let syntax_diagnostic = diagnostic
        .syntax_diagnostic()
        .expect("attached parser payload");
    assert_eq!(
        diagnostic.source().expect("attached source").text(),
        Some(source)
    );
    let alias_start = source.find("as card.heading").expect("alias keyword");
    assert_eq!(
        syntax_diagnostic.primary().range(),
        SourceRange::new(alias_start, alias_start)
    );
    assert_eq!(
        diagnostic
            .diagnostic()
            .code()
            .expect("diagnostic code")
            .as_str(),
        syntax_diagnostic.code()
    );
}

#[test]
fn fatal_pre_hir_failure_exposes_no_tooling_lease() {
    let (project, context) = removed_role_project("fn main() -> Unit { () }\n");
    let (mut compiler, mut parsed_sources) = compilation_state(&project);
    parsed_sources.clear();

    let error = compile_project(&mut compiler, &project, &parsed_sources, &context)
        .expect_err("missing accepted ParsedSource is fatal before HIR publication");

    assert_eq!(error.stage(), ProjectCompileStage::Parse.as_str());
    assert!(
        error
            .compilation_lease()
            .map(crate::project::ProjectCompilationLease::tooling_lease)
            .is_none()
    );
}

#[test]
fn recovered_module_never_enters_runtime_plan_or_compile_cache() {
    #[derive(Default)]
    struct RecordingCache {
        stores: usize,
    }

    impl ProjectCompileCache for RecordingCache {
        fn load(
            &mut self,
            _fingerprint: ProjectCompileUnitFingerprint,
        ) -> Option<Vec<CompiledProjectModule>> {
            None
        }

        fn store(
            &mut self,
            _fingerprint: ProjectCompileUnitFingerprint,
            _modules: &[CompiledProjectModule],
        ) {
            self.stores += 1;
        }
    }

    let (project, context) = removed_role_project("fn {\n");
    let (mut compiler, parsed_sources) = compilation_state(&project);
    let mut cache = RecordingCache::default();
    let error = compile_project_with_cache(
        &mut compiler,
        &project,
        &parsed_sources,
        &context,
        &mut cache,
    )
    .expect_err("recovered module cannot reach executable products");

    assert_eq!(error.stage(), ProjectCompileStage::Readiness.as_str());
    let tooling = error
        .compilation_lease()
        .map(crate::project::ProjectCompilationLease::tooling_lease)
        .expect("recovered module retains tooling evidence");
    assert!(
        tooling
            .modules()
            .iter()
            .all(|module| !module.hir().is_analysis_ready())
    );
    assert!(tooling.hir_project().analysis_view().is_err());
    assert_eq!(cache.stores, 0);
}

#[test]
fn pending_store_state_is_one_way() {
    #[derive(Default)]
    struct RecordingCache {
        stores: Vec<(ProjectCompileUnitFingerprint, usize)>,
    }

    impl ProjectCompileCache for RecordingCache {
        fn load(
            &mut self,
            _fingerprint: ProjectCompileUnitFingerprint,
        ) -> Option<Vec<CompiledProjectModule>> {
            None
        }

        fn store(
            &mut self,
            fingerprint: ProjectCompileUnitFingerprint,
            modules: &[CompiledProjectModule],
        ) {
            self.stores.push((fingerprint, modules.len()));
        }
    }

    let fingerprint = ProjectCompileUnitFingerprint([7; 32]);
    let mut pending = PendingProjectCompileStores::new();
    pending
        .push(fingerprint, Vec::new())
        .expect("collecting accepts stores");
    let mut cache = RecordingCache::default();
    pending.flush(&mut cache).expect("first flush succeeds");
    assert_eq!(cache.stores, vec![(fingerprint, 0)]);
    assert_eq!(
        pending.push(fingerprint, Vec::new()),
        Err(PendingStoreTransitionError::AlreadyFinalized)
    );
    assert_eq!(
        pending.flush(&mut cache),
        Err(PendingStoreTransitionError::AlreadyFinalized)
    );
    assert_eq!(cache.stores, vec![(fingerprint, 0)]);

    let mut discarded = PendingProjectCompileStores::new();
    discarded.discard();
    discarded.discard();
    assert_eq!(
        discarded.push(fingerprint, Vec::new()),
        Err(PendingStoreTransitionError::AlreadyFinalized)
    );
    assert_eq!(
        discarded.flush(&mut cache),
        Err(PendingStoreTransitionError::AlreadyFinalized)
    );
}

#[test]
fn registration_diagnostic_retains_accepted_source_document() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://compiler-registration/src/main.arcw")
                .expect("document id"),
            SourceName::path("src/main.arcw"),
            "fn main() -> Unit { () }\n",
        )
        .expect("document"),
    );
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new("compiler-registration").expect("package"),
        document.identity().id().clone(),
        "test",
    )
    .expect("world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("facts");
    let span = document.span(SourceRange::new(0, 2)).expect("span");
    let error = linked_error_with_registration_sources(
        ProjectCompileStage::Registration,
        &facts,
        [
            Diagnostic::new(DiagnosticSeverity::Error, "registration failed")
                .with_code("aw.character.registration.unknown_owner")
                .with_span(span),
        ],
    );

    let diagnostic = error.diagnostics().first().expect("diagnostic");
    let source = diagnostic.source().expect("accepted source document");
    assert_eq!(source.document().identity(), document.identity());
    assert_eq!(source.document().text(), document.text());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the cache-rollback test retains every stage input and the zero-store assertion in one scenario"
)]
fn pending_stores_discard_on_registration_error() {
    #[derive(Default)]
    struct RecordingCache {
        stores: usize,
    }

    impl ProjectCompileCache for RecordingCache {
        fn load(
            &mut self,
            _fingerprint: ProjectCompileUnitFingerprint,
        ) -> Option<Vec<CompiledProjectModule>> {
            None
        }

        fn store(
            &mut self,
            _fingerprint: ProjectCompileUnitFingerprint,
            _modules: &[CompiledProjectModule],
        ) {
            self.stores += 1;
        }
    }

    let source_text = "fn main() -> Unit { () }\n";
    let source_path = PathBuf::from("src/main.arcw");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://compiler-registration/src/main.arcw")
                .expect("document id"),
            SourceName::path(source_path.display().to_string()),
            source_text,
        )
        .expect("document"),
    );
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        package("org.arcweft.compiler-registration"),
        BuildSpec::default(),
        manifest_document("compiler-registration"),
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            source_path.clone(),
            Arc::clone(&document),
            [],
        )],
    )
    .expect("project");
    let declaration = document.span(SourceRange::new(0, 2)).expect("span");
    let owner = EnvironmentBindingId::try_new("environment.missing").expect("environment id");
    let path = ProjectSymbolPath::new(
        ModulePathRoot::ImplicitCrate,
        ["environment", "missing"]
            .map(|segment| ProjectSymbolSegment::try_new(segment).expect("valid fixture segment")),
    )
    .expect("qualified fixture binding path");
    let direct_bindings = vec![
        ProjectDirectBinding::try_new(
            CanonicalModulePath::crate_root(),
            path,
            Some(Visibility::Public),
            declaration.clone(),
            false,
        )
        .expect("direct binding"),
    ];
    let seed = ExternalDeclarationSeed::try_new(
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), owner.as_str())
            .expect("symbol path"),
        Some(Visibility::Public),
        declaration.clone(),
        direct_bindings,
    )
    .expect("external seed");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(project.package().id.as_str()).expect("package"),
        document.identity().id().clone(),
        "test",
    )
    .expect("world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![document],
        vec![ExternalRegistrationFact::new(
            seed,
            RegisteredExternalOwner::environment(owner.clone(), owner),
            declaration,
        )],
        Vec::new(),
        Vec::new(),
    )
    .expect("facts");
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty()),
        None,
        None,
    );
    let mut cache = RecordingCache::default();
    let (mut compiler, parsed_sources) = compilation_state(&project);

    let error = compile_project_with_cache(
        &mut compiler,
        &project,
        &parsed_sources,
        &context,
        &mut cache,
    )
    .expect_err("unknown character owner rejects project");
    assert_eq!(error.stage(), ProjectCompileStage::Registration.as_str());
    assert!(
        error
            .compilation_lease()
            .map(crate::project::ProjectCompilationLease::tooling_lease)
            .is_none(),
        "registration prelude rejection occurs before a complete tooling lease exists"
    );
    assert_eq!(cache.stores, 0);
    assert!(error.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .diagnostic()
            .code()
            .is_some_and(|code| code.as_str() == "aw.character.registration.unknown_owner")
    }));
}

#[test]
fn registration_failure_discards_project() {
    pending_stores_discard_on_registration_error();
}

#[test]
fn generic_suspending_function_uses_its_closed_await_frame() {
    use arcweft_adapter_context::manifest::{
        AdapterCallableGroupIndex, AdapterEffectCapability, AdapterFunctionSignature,
        AdapterHostCall, AdapterManifest, AdapterParameterGroup, AdapterTypeKind,
    };
    use arcweft_adapter_sema::registration::AdapterSemanticRegistration;

    let (project, context) = removed_role_project(
        r#"
extern capability fixture {
    fn load() -> Need<Result<Unit, String>> effects { control.suspend }
}
fn load_opening_assets<T>(value: T) -> Result<T, String> {
    let _bg = try await fixture.load()
    Ok(value)
}
fn wait_tail<T>(value: T) -> Result<Unit, String> {
    await fixture.load()
}
flow main() -> Result<i64, String> {
    wait_tail("ready")
    wait_tail(1i64)
    load_opening_assets("ready")
    return load_opening_assets(1i64)
}
"#,
    );
    let effect = AdapterEffectCapability::new("control.suspend");
    let manifest = AdapterManifest::new("test.suspending-function", "Suspending function test")
        .with_effect(effect.clone())
        .with_host_call(AdapterHostCall::with_signature(
            "fixture.load",
            AdapterFunctionSignature::try_new(
                vec![
                    AdapterParameterGroup::try_new(
                        AdapterCallableGroupIndex::try_from_usize(0).unwrap(),
                        Vec::new(),
                    )
                    .unwrap(),
                ],
                AdapterTypeKind::Need {
                    item: Box::new(AdapterTypeKind::Result {
                        ok: Box::new(AdapterTypeKind::Unit),
                        error: Box::new(AdapterTypeKind::String),
                    }),
                },
            )
            .unwrap(),
            [effect],
        ));
    let registration = AdapterSemanticRegistration::new(&manifest);
    let parts = registration.source_backed_facts(0).unwrap().into_parts();
    let mut documents = project
        .modules()
        .map(|module| Arc::clone(module.document()))
        .collect::<Vec<_>>();
    documents.push(parts.document);
    let facts = ProjectRegistrationFacts::try_new(
        context.facts().world().clone(),
        documents,
        parts.externals.into_vec(),
        Vec::new(),
        vec![parts.environment],
    )
    .unwrap();
    let context = ProjectCompilationContext::new(
        Arc::new(registration.declare_target(TypeCheckEnv::standard())),
        Arc::new(facts),
        Arc::clone(context.resource_types()),
        None,
        None,
    );
    let (mut compiler, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut compiler, &project, &parsed_sources, &context)
        .expect("a reached suspending project function owns an executable frame");
    let instances = compiled
        .runtime_facts()
        .project_function_instances()
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 4);
    for instance in &instances {
        assert_eq!(
            instance.suspension(),
            arcweft_lang_sema::final_analysis::CheckedSuspensionRole::MaySuspend
        );
        assert_eq!(instance.execution(), arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionExecution::ExecutableFunctionSite);
    }
    let mut bound_extern_need_calls = 0;
    for instance in &instances {
        let semantics = instance.semantics();
        semantics.visit_calls(&mut |owner, call| {
            let arcweft_runtime_plan::semantic_facts::RuntimeResolvedCallDispatch::Static(
                arcweft_runtime_plan::semantic_facts::RuntimeResolvedStaticCallTarget::Host(host),
            ) = call.dispatch()
            else {
                return;
            };
            if !matches!(
                host.owner(),
                arcweft_runtime_plan::semantic_facts::RuntimeResolvedHostCallOwner::ExternCapability(
                    _
                )
            ) {
                return;
            }
            bound_extern_need_calls += 1;
            assert!(host.contract().is_some());
            assert_eq!(host.mode(), arcweft_core::step::RuntimeHostCallMode::Suspend);
            let producer = call
                .need_producer()
                .expect("manifest-bound suspended Need call has one producer plan");
            assert!(matches!(
                producer.plan().request(),
                arcweft_core::task::NeedProducerRequestProjection::ExternCapability {
                    capability,
                    operation,
                    contract,
                    ..
                } if capability.0 == host.capability()
                    && operation == host.operation()
                    && Some(*contract) == host.contract()
            ));
            let result = semantics
                .expression_source_type(owner)
                .expect("suspending extern call retains its selected result type");
            assert_eq!(producer.need_type(), result);
            let arcweft_runtime_plan::semantic_facts::RuntimeTypeShape::Need(item) = result.shape()
            else {
                panic!("manifest-bound extern call result remains Need<T>")
            };
            assert!(matches!(
                item.shape(),
                arcweft_runtime_plan::semantic_facts::RuntimeTypeShape::Result {
                    value,
                    error,
                    ..
                } if matches!(value.shape(), arcweft_runtime_plan::semantic_facts::RuntimeTypeShape::Unit)
                    && matches!(error.shape(), arcweft_runtime_plan::semantic_facts::RuntimeTypeShape::String)
            ));
        });
    }
    assert!(bound_extern_need_calls > 0);
    assert_eq!(
        compiled
            .runtime_plan()
            .plan
            .function_sites()
            .iter()
            .filter(|site| {
                matches!(site.body(), arcweft_core::plan::RuntimeFunctionSiteBody::Executable(body)
            if body.ops().iter().any(|op| matches!(op, arcweft_core::plan::FlowOp::Await { .. })))
            })
            .count(),
        4
    );
}

#[test]
fn agent_project_graph_preserves_canonical_public_flow_ids_across_modules() {
    let child = CanonicalModulePath::crate_root()
        .join(ModuleSegment::new("child").expect("module segment"));
    let root_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://agent-flow-identity/src/main.arcw")
                .expect("root document ID"),
            SourceName::path("src/main.arcw"),
            "entry cli @entry.main { goto @flow.opening }\npub character Hero {}\nstyle theme {}\nflow opening {\n}\n",
        )
        .expect("root source document"),
    );
    let child_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://agent-flow-identity/src/child.arcw")
                .expect("child document ID"),
            SourceName::path("src/child.arcw"),
            "pub character Hero {}\nstyle theme {}\nflow opening {\n}\n",
        )
        .expect("child source document"),
    );
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        package("org.arcweft.agent-flow-identity"),
        BuildSpec::default(),
        manifest_document("agent-flow-identity"),
        [
            ProjectSourceFile::new(
                CanonicalModulePath::crate_root(),
                PathBuf::from("src/main.arcw"),
                Arc::clone(&root_document),
                [ModuleDependency::new(child.clone())],
            ),
            ProjectSourceFile::new(
                child,
                PathBuf::from("src/child.arcw"),
                Arc::clone(&child_document),
                [],
            ),
        ],
    )
    .expect("multi-module project sources");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(project.package().id.as_str()).expect("package"),
        root_document.identity().id().clone(),
        "agent-flow-identity-test",
    )
    .expect("symbol world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&root_document), Arc::clone(&child_document)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty()),
        None,
        None,
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("same-labeled module Flow project compiles");

    let graph = crate::agent_project::agent_project_graph_from_project(
        compiled.analysis_lease().semantic_index(),
    )
    .expect("typed Agent project graph");
    let flow_symbols = graph
        .symbols
        .iter()
        .filter(|symbol| {
            symbol
                .public_id
                .as_ref()
                .is_some_and(|id| matches!(id.as_str(), "flow.opening" | "flow.child.opening"))
        })
        .collect::<Vec<_>>();
    assert_eq!(flow_symbols.len(), 2);
    assert_ne!(flow_symbols[0].public_id, flow_symbols[1].public_id);
    let public_ids = graph
        .symbols
        .iter()
        .filter_map(|symbol| symbol.public_id.as_ref().map(|id| id.as_str()))
        .collect::<BTreeSet<_>>();
    for id in [
        "character.Hero",
        "character.child.Hero",
        "style.theme",
        "style.child.theme",
    ] {
        assert!(public_ids.contains(id), "missing published ID {id}");
    }
    let runtime_labels = compiled
        .runtime_plan()
        .plan
        .flows()
        .iter()
        .map(|flow| flow.id.public_label().into_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        runtime_labels,
        BTreeSet::from(["flow.opening".to_owned(), "flow.child.opening".to_owned()])
    );
    let sheets = compiled
        .style()
        .resource()
        .program
        .sheets()
        .iter()
        .map(|sheet| sheet.id().as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(sheets, BTreeSet::from(["style.theme", "style.child.theme"]));
    let runtime = compiled.runtime_plan();
    let program = arcweft_runtime_plan::awbc_lower::AwbcLowerer::new(
        &runtime.plan,
        &runtime.dialogue_content_catalog,
        "canonical_ids.arcw",
    )
    .lower()
    .expect("canonical Flow IDs lower to AWBC")
    .program;
    let encoded = program.encode_canonical().expect("canonical AWBC");
    let decoded =
        arcweft_core::awbc::schema::AwbcProgram::decode_canonical(&encoded, Default::default())
            .expect("AWBC identity round trip");
    for flow in runtime.plan.flows() {
        assert!(decoded.flow_function(&flow.id).is_some());
    }
    assert!(flow_symbols.iter().all(|symbol| {
        symbol
            .symbol_id
            .as_str()
            .starts_with("project:entity:flow:v1:")
    }));
    assert_ne!(flow_symbols[0].symbol_id, flow_symbols[1].symbol_id);
    assert_ne!(
        flow_symbols[0].qualified_name,
        flow_symbols[1].qualified_name
    );

    let compatibility_entities = crate::agent_project::agent_required_entities_from_project(
        compiled.analysis_lease().semantic_index(),
    )
    .expect("public compatibility entity projection");
    assert!(
        compatibility_entities
            .iter()
            .all(|entity| { entity.public_id.as_str() != "flow.opening" })
    );
}
#[test]
fn implicit_callable_definition_and_capture_origins_survive_source_revisions() {
    fn observe(source: &str) -> arcweft_runtime_plan::semantic_facts::RuntimeImplicitCallableFact {
        let (project, context) = removed_role_project(source);
        let (mut session, parsed_sources) = compilation_state(&project);
        let compiled = compile_project(&mut session, &project, &parsed_sources, &context).unwrap();
        let facts = compiled
            .runtime_facts()
            .implicit_callables()
            .collect::<Vec<_>>();
        let [(_, fact)] = facts.as_slice() else {
            panic!("one implicit callable")
        };
        (*fact).clone()
    }
    let source = "fn apply(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }\nflow main() -> i64 { let offset = 1i64; return apply(_ + offset, 41i64) }";
    let original = observe(source);
    let revised = observe(&format!(
        "fn unrelated() -> i64 {{ 99i64 }}\n{}",
        source.replace("1i64;", "2i64;")
    ));
    assert_ne!(original.definition().owner(), revised.definition().owner());
    assert_eq!(
        original.definition_identity(),
        revised.definition_identity()
    );
    let [capture] = original.captures() else {
        panic!("one captured offset")
    };
    let [revised_capture] = revised.captures() else {
        panic!("one captured offset")
    };
    assert_eq!(capture.position(), 0);
    assert_eq!(capture.origin(), revised_capture.origin());
    assert_eq!(
        capture.origin().semantic_digest().unwrap(),
        revised_capture.origin().semantic_digest().unwrap()
    );
    assert_ne!(capture.local(), revised_capture.local());
    assert_ne!(
        original.definition_identity(),
        observe(&source.replace("flow main", "flow other")).definition_identity()
    );
}
#[test]
fn generic_implicit_callable_instances_share_the_accepted_definition_and_capture_origin() {
    let (project, context) = removed_role_project(
        "fn make<T>(tag: T, offset: i64) -> (i64 -> i64 effects {}) { _ + offset }\nflow main() -> i64 { let number = make(1i64, 2i64); let text = make(\"tag\", 3i64); return number(40i64) + text(0i64) }",
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context).unwrap();
    let callables = compiled.runtime_facts().project_function_instances().flat_map(|instance| {
        instance.semantics().expressions().iter().filter_map(|expression| match expression.payload() {
            arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionExpressionPayload::ImplicitCallable { callable, .. } => Some(callable),
            _ => None,
        })
    }).collect::<Vec<_>>();
    let [first, second] = callables.as_slice() else {
        panic!("two closed implicit callable instances")
    };
    assert_eq!(first.definition_identity(), second.definition_identity());
    assert_eq!(
        first.formal().definition_identity(),
        first.definition_identity()
    );
    assert_eq!(first.formal().identity(), second.formal().identity());
    assert!(
        arcweft_runtime_plan::semantic_facts::RuntimeImplicitCallableFact::try_new(
            first.definition().clone(),
            second.formal().clone(),
            first.parameter().clone(),
            first.result().clone(),
            first.placeholders().into(),
            first.formal().authority(),
        )
        .is_err(),
        "a valid foreign formal cannot reuse the target's closed authority"
    );
    assert_eq!(first.definition().owner(), second.definition().owner());
    let [first_capture] = first.captures() else {
        panic!("one captured offset")
    };
    let [second_capture] = second.captures() else {
        panic!("one captured offset")
    };
    assert_eq!(first_capture.position(), 0);
    assert_eq!(first_capture.origin(), second_capture.origin());
    assert_eq!(
        first_capture.origin().semantic_digest().unwrap(),
        second_capture.origin().semantic_digest().unwrap()
    );
}
#[test]
fn closed_implicit_callable_definition_cannot_be_rebound_to_another_expression() {
    use arcweft_runtime_plan::semantic_facts::{
        RuntimeImplicitCallableFact, RuntimeProjectFunctionExpressionPayload,
        RuntimeProjectFunctionExpressionSemanticFact, RuntimeProjectFunctionFactError,
        RuntimeProjectFunctionInstanceSemanticFacts,
    };
    let (project, context) = removed_role_project(
        "fn apply(handler: i64 -> i64 effects {}, value: i64) -> i64 { handler(value) }\nfn choose<T>(tag: T, offset: i64) -> i64 { let first = apply(_ + offset, 1i64); let second = apply(_ + offset, 2i64); first + second }\nflow main() -> i64 { return choose(0i64, 40i64) }",
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context).unwrap();
    let semantics = compiled
        .runtime_facts()
        .project_function_instances()
        .map(|instance| instance.semantics())
        .find(|semantics| {
            semantics
                .expressions()
                .iter()
                .filter(|row| {
                    matches!(
                        row.payload(),
                        RuntimeProjectFunctionExpressionPayload::ImplicitCallable { .. }
                    )
                })
                .count()
                == 2
        })
        .expect("one closed owner of two implicit callables");
    let callables = semantics
        .expressions()
        .iter()
        .filter_map(|row| match row.payload() {
            RuntimeProjectFunctionExpressionPayload::ImplicitCallable { callable, .. } => {
                Some(callable)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [first, second] = callables.as_slice() else {
        panic!("two callable definitions")
    };
    assert_ne!(first.definition_identity(), second.definition_identity());
    let mut rows = semantics.expressions().to_vec();
    let row = rows
        .iter_mut()
        .find(|row| row.owner() == first.definition().owner())
        .unwrap();
    let RuntimeProjectFunctionExpressionPayload::ImplicitCallable { tried, pipe, .. } =
        row.payload()
    else {
        panic!("implicit payload")
    };
    *row = RuntimeProjectFunctionExpressionSemanticFact::new(
        row.owner(),
        row.children().into(),
        RuntimeProjectFunctionExpressionPayload::ImplicitCallable {
            callable: RuntimeImplicitCallableFact::try_new(
                second.definition().clone(),
                second.formal().clone(),
                first.parameter().clone(),
                first.result().clone(),
                first.placeholders().into(),
                semantics.local_uses(),
            )
            .unwrap(),
            tried: tried.clone(),
            pipe: pipe.clone(),
        },
    );
    assert!(matches!(
        RuntimeProjectFunctionInstanceSemanticFacts::try_new(
            semantics.partition().clone(),
            semantics.local_uses().clone(),
            semantics.type_projection().into(),
            rows.into_boxed_slice(),
            semantics.patterns().into(),
            semantics.statements().into(),
            semantics.captures().into(),
        ),
        Err(RuntimeProjectFunctionFactError::NonCanonicalSemanticFacts)
    ));
}

#[test]
fn implicit_capture_transfer_is_selected_per_closed_instance() {
    use arcweft_lang_sema::final_analysis::CheckedLocalReadMode;
    use arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionExpressionPayload;
    let (project, context) = removed_role_project(
        "fn make<T>(value: T) -> (i64 -> (i64, T) effects {}) { (_, value) }\nflow main() -> i64 { let number = make(42i64); let affine = make(Vec<Need<i64>>::with_capacity(0usize)); let (result, _) = number(42i64); let _ = affine(0i64); return result }",
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context).unwrap();
    let callables = compiled
        .runtime_facts()
        .project_function_instances()
        .flat_map(|instance| {
            instance
                .semantics()
                .expressions()
                .iter()
                .filter_map(|row| match row.payload() {
                    RuntimeProjectFunctionExpressionPayload::ImplicitCallable {
                        callable, ..
                    } => Some((instance.semantics(), callable)),
                    _ => None,
                })
        })
        .collect::<Vec<_>>();
    let [(first_semantics, first), (second_semantics, second)] = callables.as_slice() else {
        panic!("two closed implicit callables")
    };
    assert_eq!(first.definition_identity(), second.definition_identity());
    let [first_capture] = first.captures() else {
        panic!("one capture")
    };
    let [second_capture] = second.captures() else {
        panic!("one capture")
    };
    assert_eq!(first_capture.origin(), second_capture.origin());
    assert!(matches!(
        (
            first_capture.transfer().mode(),
            second_capture.transfer().mode()
        ),
        (CheckedLocalReadMode::Copy, CheckedLocalReadMode::Move)
            | (CheckedLocalReadMode::Move, CheckedLocalReadMode::Copy)
    ));
    // A fact legitimately issued by the other closed instance must not be
    // accepted under this owner's closed transfer authority.
    let foreign = arcweft_runtime_plan::semantic_facts::RuntimeImplicitCallableFact::try_new(
        second.definition().clone(),
        second.formal().clone(),
        second.parameter().clone(),
        second.result().clone(),
        second.placeholders().into(),
        second_semantics.local_uses(),
    )
    .unwrap();
    assert_ne!(foreign.captures()[0].transfer(), first_capture.transfer());
    let mut rows = first_semantics.expressions().to_vec();
    let row = rows
        .iter_mut()
        .find(|row| row.owner() == first.definition().owner())
        .unwrap();
    let RuntimeProjectFunctionExpressionPayload::ImplicitCallable { tried, pipe, .. } =
        row.payload()
    else {
        panic!("implicit payload")
    };
    *row = arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionExpressionSemanticFact::new(
        row.owner(),
        row.children().into(),
        RuntimeProjectFunctionExpressionPayload::ImplicitCallable {
            callable: foreign,
            tried: tried.clone(),
            pipe: pipe.clone(),
        },
    );
    assert!(matches!(arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionInstanceSemanticFacts::try_new(first_semantics.partition().clone(), first_semantics.local_uses().clone(), first_semantics.type_projection().into(), rows.into_boxed_slice(), first_semantics.patterns().into(), first_semantics.statements().into(), first_semantics.captures().into()), Err(arcweft_runtime_plan::semantic_facts::RuntimeProjectFunctionFactError::NonCanonicalSemanticFacts)));
}

#[test]
fn root_implicit_capture_transfer_drives_creation_and_ingress() {
    use arcweft_core::{
        plan::{FlowOp, RuntimeFunctionInputOwnershipRequirement, RuntimeFunctionSemanticRole},
        value::{RuntimeExprKind, RuntimeLocalReadMode},
    };
    use arcweft_lang_sema::final_analysis::CheckedLocalReadMode;
    for (source, mode, read, ownership) in [
        (
            "flow main() -> i64 { let value = 42i64; let callback: i64 -> (i64, i64) effects {} = (_, value); let (result, _) = callback(42i64); return result }",
            CheckedLocalReadMode::Copy,
            RuntimeLocalReadMode::Copy,
            RuntimeFunctionInputOwnershipRequirement::Unrestricted,
        ),
        (
            "flow main() -> i64 { let value = Vec<Need<i64>>::with_capacity(0usize); let callback: i64 -> (i64, Vec<Need<i64>>) effects {} = (_, value); let _ = callback(0i64); return 42i64 }",
            CheckedLocalReadMode::Move,
            RuntimeLocalReadMode::Move,
            RuntimeFunctionInputOwnershipRequirement::Owned,
        ),
    ] {
        let (project, context) = removed_role_project(source);
        let (mut session, sources) = compilation_state(&project);
        let compiled = compile_project(&mut session, &project, &sources, &context).unwrap();
        let callables = compiled
            .runtime_facts()
            .implicit_callables()
            .collect::<Vec<_>>();
        let [(_, callable)] = callables.as_slice() else {
            panic!("one implicit callable")
        };
        assert_eq!(callable.captures()[0].transfer().mode(), mode);
        let plan = &compiled.runtime_plan().plan;
        let sites = plan
            .function_sites()
            .iter()
            .filter(|site| {
                site.role() == RuntimeFunctionSemanticRole::Closure
                    && site.capture_inputs().count() == 1
            })
            .collect::<Vec<_>>();
        let [site] = sites.as_slice() else {
            panic!("one capturing site")
        };
        assert_eq!(site.capture_inputs().next().unwrap().ownership(), ownership);
        let mut reads = Vec::new();
        plan.try_visit_flow_ops(&mut |op| {
            if let FlowOp::Let { expr, .. } = op
                && let RuntimeExprKind::MakeCallable { captures, .. } = expr.kind()
            {
                for capture in captures {
                    let RuntimeExprKind::Local(local) = capture.kind() else {
                        panic!("local capture")
                    };
                    reads.push(local.mode());
                }
            }
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
        assert_eq!(reads, vec![read]);
    }
}

#[test]
fn function_sites_preserve_accepted_parameter_passing_separately_from_ingress() {
    use arcweft_core::plan::{
        RuntimeFunctionInputOwnershipRequirement, RuntimeFunctionInputSource,
        RuntimeFunctionParameterPassing, RuntimeFunctionSemanticRole,
    };
    let (project, context) = removed_role_project(
        "fn identity<T>(value: T) -> T { value }\nflow main() -> i64 { let number = identity(42i64); let pending = identity(Vec<Need<i64>>::with_capacity(0usize)); let explicit = |value: i64| value; let implicit = (_ + 1i64); return implicit(explicit(number)) }",
    );
    let (mut session, parsed) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed, &context).unwrap();
    let plan = &compiled.runtime_plan().plan;
    let mut ordinary = Vec::new();
    let mut closures = 0;
    for site in plan.function_sites().iter() {
        for input in site.parameter_inputs() {
            let RuntimeFunctionInputSource::Parameter { position, passing } = input.source() else {
                unreachable!()
            };
            assert_eq!(position, 0);
            assert_eq!(
                input.ownership(),
                RuntimeFunctionInputOwnershipRequirement::Owned
            );
            match site.role() {
                RuntimeFunctionSemanticRole::Ordinary => ordinary.push(passing),
                RuntimeFunctionSemanticRole::Closure => {
                    assert_eq!(passing, RuntimeFunctionParameterPassing::Value);
                    closures += 1;
                }
                _ => panic!("fixture only defines ordinary and closure inputs"),
            }
        }
    }
    ordinary.sort();
    assert_eq!(
        ordinary,
        vec![
            RuntimeFunctionParameterPassing::Value,
            RuntimeFunctionParameterPassing::Affine
        ]
    );
    assert_eq!(
        closures, 2,
        "explicit and implicit formals retain their accepted mode"
    );
}

#[test]
fn flow_facts_retain_the_accepted_body_and_complete_formals() {
    use arcweft_core::plan::RuntimeFunctionParameterPassing;
    use arcweft_lang_sema::final_analysis::{CheckedExecutionBodyOwner, CheckedExecutionSource};
    let (project, context) = removed_role_project(
        "fn ordinary() -> Unit { () }\nflow root(value: i64, unused: String, pending: Vec<Need<i64>>) -> i64 effects { fs.write } { return value }\nflow other() -> i64 { return 0i64 }",
    );
    let (mut session, parsed) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed, &context).unwrap();
    let executable = compiled
        .analysis_lease()
        .hir_project()
        .analysis_view()
        .unwrap();
    let symbols = compiled.analysis_lease().project_symbols();
    let root = executable
        .items()
        .find(|item| {
            symbols
                .flow_symbol_for_item(item.id())
                .is_some_and(|symbol| symbol.declaration().name() == "root")
        })
        .unwrap();
    let fact = compiled.runtime_facts().flow(root.id()).unwrap();
    assert!(
        matches!(fact.definition().source(), CheckedExecutionSource::InvokeBody(
        CheckedExecutionBodyOwner::Declaration { declaration, .. }
    ) if declaration == symbols.flow_symbol_for_item(root.id()).unwrap().declaration())
    );
    assert_eq!(fact.definition().parameters().len(), 3);
    for (formal, passing) in fact.definition().parameters().iter().zip([
        RuntimeFunctionParameterPassing::Value,
        RuntimeFunctionParameterPassing::Value,
        RuntimeFunctionParameterPassing::Affine,
    ]) {
        assert_eq!(formal.passing(), passing);
        assert_eq!(formal.bindings().len(), 1);
        assert_eq!(
            formal.definition_identity(),
            fact.definition().definition_identity()
        );
    }
    assert_ne!(
        fact.definition().parameters()[0].identity(),
        fact.definition().parameters()[1].identity()
    );
    let CheckedExecutionSource::InvokeBody(owner) = fact.definition().source() else {
        unreachable!()
    };
    let source = CheckedExecutionSource::ExportMutation(owner.clone());
    let analysis = compiled.analysis_lease().final_analysis();
    let context = analysis
        .checked_execution_context(executable, symbols, source.clone(), None)
        .unwrap();
    let mutation = Arc::new(context.checked_execution_input_abi(source).unwrap());
    assert_ne!(
        mutation.definition_identity(),
        fact.definition().definition_identity()
    );
    assert!(fact.definition().effects().is_empty());
    assert_eq!(
        fact.effects()
            .iter()
            .map(arcweft_id::EffectId::as_str)
            .collect::<Vec<_>>(),
        vec!["fs.write"]
    );
    let ordinary = executable
        .items()
        .find(|item| matches!(item.item().kind(), HirItemKind::Function(_)))
        .unwrap();
    assert!(
        analysis
            .checked_flow_execution_definition(executable, symbols, ordinary.id())
            .is_err()
    );
}

#[test]
fn ordinary_function_formals_retain_accepted_whole_parameter_coordinates() {
    let source = "fn root((left, right): (i64, i64), unused: i64) -> i64 { left + right }\nflow main() -> i64 { return root((20i64, 22i64), 0i64) }";
    let observe = |source: &str| {
        let (project, context) = removed_role_project(source);
        let (mut session, parsed) = compilation_state(&project);
        let compiled = compile_project(&mut session, &project, &parsed, &context)
            .expect("whole formal parameter roles compile");
        let instance = compiled
            .runtime_facts()
            .project_function_instances()
            .next()
            .unwrap();
        let [pair, unused] = instance.parameters() else {
            panic!("complete ordinary arity")
        };
        assert_eq!(pair.bindings().len(), 2);
        assert_eq!(unused.bindings().len(), 1);
        assert_eq!(pair.definition(), &instance.definition().parameters()[0]);
        assert_eq!(unused.definition(), &instance.definition().parameters()[1]);
        assert_ne!(pair.identity(), unused.identity());
        (
            instance.definition_identity(),
            pair.identity(),
            unused.identity(),
            pair.pattern(),
        )
    };
    let original = observe(source);
    let revised = observe(&format!(
        "fn unrelated() -> i64 {{ 0i64 }}\n{}",
        source.replace("left + right", "left + right + 1i64")
    ));
    assert_eq!(
        (original.0, original.1, original.2),
        (revised.0, revised.1, revised.2)
    );
    assert_ne!(original.3, revised.3);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one authored callback fixture proves definition/capture revision invariance and exact source-definition rejection"
)]
fn callback_definitions_and_capture_origins_survive_body_and_source_revisions() {
    use arcweft_runtime_plan::semantic_facts::{
        RuntimeDeferFact, RuntimeDialogueEffectProgramFact, RuntimeExecutableCaptureFactError,
    };
    let source = "pub character alice { display = \"Alice\" }\nfn consume<T>(value: T) { let consumed = value; }\nflow main() {\n let first = 42i64\n let second = 7i64\n alice: hello [call consume((first, second))][p]\n with:\n  defer { let result = first + second; }\n}\nentry cli @entry.main { goto @flow.main }";
    let observe = |source: &str| {
        let (project, context) = removed_role_dialogue_project(source);
        let (mut session, parsed) = compilation_state(&project);
        let compiled = compile_project(&mut session, &project, &parsed, &context)
            .expect("accepted callback definitions compile");
        let (_, defer) = compiled.runtime_facts().defers().next().expect("one defer");
        let effect = &compiled.runtime_facts().dialogue_content_fragments()[0].effects()[0];
        assert_eq!(defer.captures().len(), 2);
        assert_eq!(effect.captures().len(), 2);
        (defer.clone(), effect.clone())
    };
    let (defer, effect) = observe(source);
    let revised = format!(
        "fn unrelated() -> i64 {{ 1i64 }}\n{}",
        source
            .replace("hello", "updated")
            .replace("first + second;", "first + second + 1i64;")
    );
    let (revised_defer, revised_effect) = observe(&revised);
    assert_eq!(
        defer.definition_identity(),
        revised_defer.definition_identity()
    );
    assert_eq!(
        effect.definition_identity(),
        revised_effect.definition_identity()
    );
    assert_ne!(defer.definition_identity(), effect.definition_identity());
    for (original, revised) in [
        (defer.captures(), revised_defer.captures()),
        (effect.captures(), revised_effect.captures()),
    ] {
        assert_ne!(original[0].local(), revised[0].local());
        for (original, revised) in original.iter().zip(revised) {
            assert_eq!(
                original.origin().semantic_digest().unwrap(),
                revised.origin().semantic_digest().unwrap()
            );
        }
    }
    let (other_defer, other_effect) = observe(
        &source
            .replace("flow main", "flow other")
            .replace("@flow.main", "@flow.other"),
    );
    assert_ne!(
        defer.definition_identity(),
        other_defer.definition_identity()
    );
    assert_ne!(
        effect.definition_identity(),
        other_effect.definition_identity()
    );
    let (_, delayed) = observe(&source.replace(
        "[call consume((first, second))]",
        "[at 1ms call=consume((first, second))]",
    ));
    assert_eq!(
        RuntimeDialogueEffectProgramFact::try_new(
            effect.site(),
            delayed.trigger().clone(),
            effect.definition().clone(),
            effect.callable_type().clone(),
            effect.operation().clone(),
            effect.captures().to_vec(),
        )
        .unwrap_err(),
        RuntimeExecutableCaptureFactError::DefinitionMismatch
    );
    // Accepted source rows, not caller-supplied capture order, own the ABI.
    let mut captures = defer.captures().to_vec();
    captures.reverse();
    assert_eq!(
        RuntimeDeferFact::try_new(
            defer.definition().clone(),
            defer.effects().clone(),
            captures
        )
        .unwrap_err(),
        RuntimeExecutableCaptureFactError::DefinitionMismatch
    );
    assert_eq!(
        RuntimeDeferFact::try_new(
            defer.definition().clone(),
            defer.effects().clone(),
            Vec::new()
        )
        .unwrap_err(),
        RuntimeExecutableCaptureFactError::DefinitionMismatch
    );
    let mut captures = effect.captures().to_vec();
    captures.reverse();
    assert_eq!(
        RuntimeDialogueEffectProgramFact::try_new(
            effect.site(),
            effect.trigger().clone(),
            effect.definition().clone(),
            effect.callable_type().clone(),
            effect.operation().clone(),
            captures
        )
        .unwrap_err(),
        RuntimeExecutableCaptureFactError::DefinitionMismatch
    );
    assert_eq!(
        RuntimeDialogueEffectProgramFact::try_new(
            effect.site(),
            effect.trigger().clone(),
            effect.definition().clone(),
            effect.callable_type().clone(),
            other_effect.operation().clone(),
            effect.captures().to_vec()
        )
        .unwrap_err(),
        RuntimeExecutableCaptureFactError::DefinitionMismatch
    );
}

#[test]
fn closed_dialogue_effect_captures_reject_foreign_transfer_authority() {
    use arcweft_lang_sema::final_analysis::CheckedLocalReadMode;
    use arcweft_runtime_plan::semantic_facts::{
        RuntimeProjectFunctionExpressionPayload, RuntimeProjectFunctionFactError,
        RuntimeProjectFunctionInstanceSemanticFacts,
    };
    let (project, context) = removed_role_dialogue_project(
        "pub character alice { display = \"Alice\" }\nfn consume<T>(value: T) { let consumed = value; }\nfn speak<T>(value: T) { alice[hello [call consume(value)]]; }\nflow main() { speak(42i64); speak(Vec<Need<i64>>::with_capacity(0usize)); }",
    );
    let (mut session, parsed) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed, &context)
        .expect("closed effect captures compile");
    let effects = compiled
        .runtime_facts()
        .project_function_instances()
        .flat_map(|instance| {
            instance
                .semantics()
                .expressions()
                .iter()
                .filter_map(|row| match row.payload() {
                    RuntimeProjectFunctionExpressionPayload::DialogueApplication {
                        fragments,
                        ..
                    } => Some((
                        instance.semantics(),
                        fragments[0].effects()[0].captures()[0].clone(),
                    )),
                    _ => None,
                })
        })
        .collect::<Vec<_>>();
    let [(first, first_capture), (second, second_capture)] = effects.as_slice() else {
        panic!("two closed effects")
    };
    assert_eq!(first_capture.origin(), second_capture.origin());
    let definitions = compiled
        .runtime_facts()
        .project_function_instances()
        .flat_map(|instance| {
            instance
                .semantics()
                .expressions()
                .iter()
                .filter_map(|row| match row.payload() {
                    RuntimeProjectFunctionExpressionPayload::DialogueApplication {
                        fragments,
                        ..
                    } => Some(fragments[0].effects()[0].definition_identity()),
                    _ => None,
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(definitions.len(), 2);
    assert_eq!(
        definitions[0], definitions[1],
        "closed types and transfer modes do not rename the accepted definition"
    );
    assert!(matches!(
        (
            first_capture.transfer().mode(),
            second_capture.transfer().mode()
        ),
        (CheckedLocalReadMode::Copy, CheckedLocalReadMode::Move)
            | (CheckedLocalReadMode::Move, CheckedLocalReadMode::Copy)
    ));
    // The lexical partition is shared, but its closed transfer proof cannot
    // be substituted by another instantiation's authority.
    assert!(matches!(
        RuntimeProjectFunctionInstanceSemanticFacts::try_new(
            first.partition().clone(),
            second.local_uses().clone(),
            first.type_projection().into(),
            first.expressions().into(),
            first.patterns().into(),
            first.statements().into(),
            first.captures().into(),
        ),
        Err(RuntimeProjectFunctionFactError::NonCanonicalSemanticFacts)
    ));
}

#[test]
fn defer_capture_transfer_drives_registration_and_ingress() {
    use arcweft_core::{
        plan::{FlowOp, RuntimeFunctionInputOwnershipRequirement},
        value::{RuntimeExprKind, RuntimeLocalReadMode},
    };
    use arcweft_lang_sema::final_analysis::CheckedLocalReadMode;
    for (initializer, mode, read_mode, ownership) in [
        (
            "42i64",
            CheckedLocalReadMode::Copy,
            RuntimeLocalReadMode::Copy,
            RuntimeFunctionInputOwnershipRequirement::Unrestricted,
        ),
        (
            "Vec<Need<i64>>::with_capacity(0usize)",
            CheckedLocalReadMode::Move,
            RuntimeLocalReadMode::Move,
            RuntimeFunctionInputOwnershipRequirement::Owned,
        ),
    ] {
        let source = format!(
            "pub character alice {{ display = \"Alice\" }}\nflow main() -> Unit {{\n let value = {initializer}\n alice: hello[p]\n with:\n  defer {{ let _ = value; }}\n}}"
        );
        let (project, context) = removed_role_dialogue_project(&source);
        let (mut session, parsed) = compilation_state(&project);
        let compiled = compile_project(&mut session, &project, &parsed, &context)
            .expect("checked deferred transfer compiles");
        let (_, defer) = compiled
            .runtime_facts()
            .defers()
            .next()
            .expect("defer fact");
        let [capture] = defer.captures() else {
            panic!("one capture")
        };
        assert_eq!(capture.transfer().mode(), mode);
        let plan = &compiled.runtime_plan().plan;
        let site = plan.function_sites().get(plan.defer_sites()[0]).unwrap();
        let input = site.capture_inputs().next().unwrap();
        assert_eq!(input.ownership(), ownership);
        assert_eq!(
            input.unrestricted_bindings().len(),
            usize::from(mode == CheckedLocalReadMode::Copy)
        );
        let mut registrations = Vec::new();
        plan.try_visit_flow_ops(&mut |op| {
            if let FlowOp::RegisterDefer { captures, .. } = op {
                for value in captures {
                    let RuntimeExprKind::Local(read) = value.kind() else {
                        panic!("local transfer")
                    };
                    registrations.push(read.mode());
                }
            }
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
        assert_eq!(registrations, vec![read_mode]);
    }
}

#[test]
fn dialogue_effect_capture_transfer_drives_creation_and_ingress() {
    use arcweft_core::plan::{
        RuntimeFunctionInputOwnershipRequirement, RuntimeFunctionSemanticRole,
    };
    use arcweft_core::value::{RuntimeExprKind, RuntimeLocalReadMode};
    use arcweft_lang_sema::final_analysis::CheckedLocalReadMode;
    for (initializer, mode, ownership) in [
        (
            "42i64",
            CheckedLocalReadMode::Copy,
            RuntimeFunctionInputOwnershipRequirement::Unrestricted,
        ),
        (
            "Vec<Need<i64>>::with_capacity(0usize)",
            CheckedLocalReadMode::Move,
            RuntimeFunctionInputOwnershipRequirement::Owned,
        ),
    ] {
        let source = format!(
            "pub character alice {{ display = \"Alice\" }}\nfn consume<T>(value: T) {{ let consumed = value; }}\nflow main() {{ let value = {initializer}; alice[hello [call consume(value)]]; }}"
        );
        let (project, context) = removed_role_dialogue_project(&source);
        let (mut session, parsed) = compilation_state(&project);
        let compiled = compile_project(&mut session, &project, &parsed, &context)
            .expect("checked effect transfer compiles");
        let [fragment] = compiled.runtime_facts().dialogue_content_fragments() else {
            panic!("one fragment")
        };
        let [effect] = fragment.effects() else {
            panic!("one effect")
        };
        let [capture] = effect.captures() else {
            panic!("one capture")
        };
        assert_eq!(capture.transfer().mode(), mode);
        let sites = compiled
            .runtime_plan()
            .plan
            .function_sites()
            .iter()
            .filter(|site| {
                site.role() == RuntimeFunctionSemanticRole::Effect
                    && site.capture_inputs().count() == 1
            })
            .collect::<Vec<_>>();
        let [site] = sites.as_slice() else {
            panic!("one effect function")
        };
        let input = site.capture_inputs().next().unwrap();
        assert_eq!(input.ownership(), ownership);
        assert_eq!(
            input.unrestricted_bindings().len(),
            usize::from(mode == CheckedLocalReadMode::Copy)
        );
        let creation = compiled
            .runtime_plan()
            .plan
            .dialogue_content()
            .rows()
            .iter()
            .flat_map(|content| content.effect_sites())
            .flat_map(|effect| effect.captures())
            .map(|capture| {
                let RuntimeExprKind::Local(read) = capture.kind() else {
                    panic!("activation capture transfers the accepted local")
                };
                read.mode()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            creation,
            vec![match mode {
                CheckedLocalReadMode::Copy => RuntimeLocalReadMode::Copy,
                CheckedLocalReadMode::Move => RuntimeLocalReadMode::Move,
                CheckedLocalReadMode::Borrow => unreachable!("fixture captures values"),
            }]
        );
    }
}
