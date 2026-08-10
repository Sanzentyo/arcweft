use super::*;
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
use std::{collections::BTreeMap, path::PathBuf};

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

fn dialogue_manifest_document(name: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-project://{name}/arcw.toml"))
                .expect("manifest document ID"),
            SourceName::path("arcw.toml"),
            format!(
                "schema = 1\n[package]\nid = \"org.arcweft.{name}\"\nversion = \"0.1.0\"\n\n[profiles.dev]\nkind = \"game\"\nsource = \"src/main.arcw\"\n\n[profiles.dev.localization.character_names]\nactive = \"ja-JP\"\nfallbacks = []\n"
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
            .tooling_lease()
            .expect("recovered final HIR publishes one tooling lease");
        assert_eq!(tooling.modules().len(), 1);
        let module = &tooling.modules()[0];
        assert!(!module.hir().is_executable());
        assert!(!module.hir().is_cache_eligible());
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
        assert!(error.tooling_lease().is_some());
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
    let lint = compiled.modules()[0]
        .syntax_lints()
        .iter()
        .find(|lint| lint.code() == SyntaxLintCode::RedundantDeclIdentity)
        .expect("compiled module retains the redundant declaration identity warning");

    assert_eq!(lint.code().stable_code(), "AWF0101");
    assert_eq!(lint.code().domain_name(), "style::redundant_decl_identity");
    assert!(compiled.syntax_warnings() > 0);
    assert_eq!(
        compiled.syntax_warnings(),
        compiled
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
    let retained = Arc::clone(first.hir_project());

    let second = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("identical project recompilation");

    assert!(Arc::ptr_eq(&retained, second.hir_project()));
}

#[test]
fn dialogue_line_reference_reaches_runtime_lowering_from_one_accepted_generation() {
    let (project, context) = removed_role_dialogue_project(
        r"
pub character @character.alice Alice as alice {}

fn opening() {
    alice[前[strong]強調[/strong]後];
}

flow reference {
    let selected: Ref<DialogueLine> = @say.fn.org.arcweft.removed-role.function.opening.001
}
",
    );
    let (mut session, parsed_sources) = compilation_state(&project);
    let compiled = compile_project(&mut session, &project, &parsed_sources, &context)
        .expect("typed dialogue-line reference compiles through runtime lowering");

    let [line] = compiled.hir_project().dialogue_lines().records() else {
        panic!("one accepted dialogue line")
    };
    assert_eq!(
        line.id().as_str(),
        "say.fn.org.arcweft.removed-role.function.opening.001"
    );
    let [reference] = compiled.semantic_index().dialogue_line_references() else {
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

    let tooling = compiled.tooling_lease();
    assert!(Arc::ptr_eq(tooling.hir_project(), compiled.hir_project()));
    assert!(std::ptr::eq(
        tooling.project_symbols(),
        compiled.registered_world().symbols()
    ));
    assert_eq!(compiled.modules().len(), 2);
    assert_eq!(
        compiled.hir_project().database_id(),
        session.hir_database_id()
    );
    let mut proofs = 0_usize;
    for module in compiled.modules() {
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
    let root_text =
        "fn root_line() {\n    alice(id = @say.shared)[before[strong]root[/strong]after];\n}\n";
    let child_text =
        "fn child_line() {\n    bob(id = @say.shared)[before[strong]child[/strong]after];\n}\n";
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
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        package("org.arcweft.dialogue-collision"),
        BuildSpec::default(),
        manifest_document("dialogue-collision"),
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
        ProjectCompileStage::HirProject.as_str(),
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
    let retained = Arc::clone(accepted.hir_project());

    let (collision_project, collision_context, _, _) = dialogue_collision_project();
    let (_, collision_sources) = compilation_state(&collision_project);
    let error = compile_project(
        &mut session,
        &collision_project,
        &collision_sources,
        &collision_context,
    )
    .expect_err("collision candidate rejects without replacing accepted cache");
    assert_eq!(error.stage(), ProjectCompileStage::HirProject.as_str());

    let rebuilt = compile_project(
        &mut session,
        &accepted_project,
        &accepted_sources,
        &accepted_context,
    )
    .expect("accepted input remains reusable after rejection");
    assert!(Arc::ptr_eq(&retained, rebuilt.hir_project()));
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
        .tooling_lease()
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
    assert!(error.tooling_lease().is_none());
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
        .tooling_lease()
        .expect("recovered module retains tooling evidence");
    assert!(
        tooling
            .modules()
            .iter()
            .all(|module| { !module.hir().is_executable() && !module.hir().is_cache_eligible() })
    );
    assert!(tooling.hir_project().executable_view().is_err());
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
        error.tooling_lease().is_none(),
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
fn agent_project_graph_preserves_same_public_flow_label_across_modules() {
    let child = CanonicalModulePath::crate_root()
        .join(ModuleSegment::new("child").expect("module segment"));
    let root_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://agent-flow-identity/src/main.arcw")
                .expect("root document ID"),
            SourceName::path("src/main.arcw"),
            "flow opening {\n}\n",
        )
        .expect("root source document"),
    );
    let child_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://agent-flow-identity/src/child.arcw")
                .expect("child document ID"),
            SourceName::path("src/child.arcw"),
            "flow opening {\n}\n",
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

    let graph = crate::agent_project::agent_project_graph_from_project(compiled.semantic_index())
        .expect("typed Agent project graph");
    let flow_symbols = graph
        .symbols
        .iter()
        .filter(|symbol| {
            symbol
                .public_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "flow.opening")
        })
        .collect::<Vec<_>>();
    assert_eq!(flow_symbols.len(), 2);
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

    let compatibility_entities =
        crate::agent_project::agent_required_entities_from_project(compiled.semantic_index())
            .expect("public compatibility entity projection");
    assert!(
        compatibility_entities
            .iter()
            .all(|entity| { entity.public_id.as_str() != "flow.opening" })
    );
}
