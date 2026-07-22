use super::*;
use arcweft_lang_hir::symbol::{
    CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectSymbolWorldId,
};
use arcweft_lang_sema::{
    env::identity::EnvironmentBindingId,
    registration::{ExternalRegistrationFact, RegisteredExternalOwner},
};
use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePathRoot},
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
};
use arcweft_lang_syntax::{lint::SyntaxLintCode, parser::recovery::ParseErrorKind};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::ProjectSourceFile;
use arcweft_source::{DiagnosticLabel, SourceDocument, SourceRange};
use std::path::PathBuf;

fn removed_role_project(source_text: &str) -> (ProjectSources, ProjectCompilationContext) {
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
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        package("org.arcweft.removed-role"),
        BuildSpec::default(),
        manifest_document("removed-role"),
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
    let facts = ProjectRegistrationFacts::try_new(world, vec![document], Vec::new(), Vec::new())
        .expect("registration facts");
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty()),
        None,
        None,
        Vec::new(),
    );
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
fn project_compiler_entrypoints_reject_removed_role_declarations_at_parse() {
    for source in [
        "state GameState {\n    value: i32\n}\n",
        "reducer update(state: GameState, event: GameEvent) -> GameState {\n    state\n}\n",
        "agent @agent.smoke smoke() {\n    Ok(())\n}\n",
    ] {
        let (project, context) = removed_role_project(source);
        let options = RuntimePlanLowerOptions::default();
        let error = compile_project(&project, &context, &options)
            .expect_err("removed declaration must fail project compilation");
        assert_eq!(error.stage(), ProjectCompileStage::Parse.as_str());

        let mut cache = InMemoryProjectCompileCache::default();
        let error = compile_project_with_cache(&project, &context, &options, &mut cache)
            .expect_err("cached project compilation must reject removed declaration");
        assert_eq!(error.stage(), ProjectCompileStage::Parse.as_str());
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
    assert!(diagnostic.parse_error().is_none());
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
    let (project, context) = removed_role_project("flow @flow.opening {\n}\n");
    let compiled = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .expect("valid project with a non-blocking syntax hint compiles");
    let lint = compiled.modules()[0]
        .syntax_lints()
        .iter()
        .find(|lint| lint.code() == SyntaxLintCode::ExplicitDeclId)
        .expect("compiled module retains the explicit declaration ID hint");

    assert_eq!(lint.code().stable_code(), "AWF0103");
    assert_eq!(lint.code().domain_name(), "style::explicit_decl_id");
}

#[test]
fn project_parse_diagnostics_retain_the_original_typed_parser_payload() {
    let source = r"pub view Card() {
    export part as card.heading
    Panel().part(header)
}
";
    let (project, context) = removed_role_project(source);
    let error = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .expect_err("malformed View export must fail project parsing");
    assert_eq!(error.stage(), ProjectCompileStage::Parse.as_str());

    let diagnostic = error
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic
                .parse_error()
                .is_some_and(|error| error.kind() == ParseErrorKind::ViewExportPartMissingLocal)
        })
        .expect("typed missing-local parser diagnostic");
    let parse_error = diagnostic.parse_error().expect("parser payload");
    assert_eq!(parse_error.code(), "view::export_part_missing_local");
    assert_eq!(&source[parse_error.range().as_range()], "as");
    assert_eq!(
        diagnostic
            .diagnostic()
            .code()
            .expect("diagnostic code")
            .as_str(),
        parse_error.code()
    );
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
            RegisteredExternalOwner::Environment(owner),
            declaration,
        )],
        Vec::new(),
    )
    .expect("facts");
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty()),
        None,
        None,
        Vec::new(),
    );
    let mut cache = RecordingCache::default();

    let error = compile_project_with_cache(
        &project,
        &context,
        &RuntimePlanLowerOptions::default(),
        &mut cache,
    )
    .expect_err("unknown character owner rejects project");
    assert_eq!(error.stage(), ProjectCompileStage::Registration.as_str());
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
