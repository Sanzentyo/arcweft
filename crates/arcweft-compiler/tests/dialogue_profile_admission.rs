use arcweft_compiler::project::{
    AcceptedLaunchProfileInput, ProjectCompilationContext, ProjectCompilationSession,
    ProjectCompileStage, compile_project,
};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{env::TypeCheckEnv, registration::ProjectRegistrationFacts};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    incremental::{ParsedSource, SyntaxDatabase},
    parser::ParseOptions,
};
use arcweft_launch::{LaunchProfileSelection, ProfileId, accepted::SourceBackedManifest};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceName, SourceSetRevision, identity::SourceSnapshotId,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

const DIALOGUE_SOURCE: &str = r#"
pub view Mobile(dialogue: DialogueView) {
  RichText(dialogue.content)
}

pub view Plain() {
  Text("plain")
}

pub style Mobile {
  Button { color = rgba(10, 20, 30, 255) }
}
"#;

#[test]
fn compiler_admits_profile_against_the_same_view_product_and_revision() {
    let fixture = Fixture::new(
        DIALOGUE_SOURCE,
        r#"
[profiles.dev.dialogue]
view = "view.Mobile"
style = "style.Mobile"
"#,
        true,
    );
    let compiled = fixture.compile().expect("checked dialogue profile");
    let checked = compiled.dialogue_profile();
    let product = compiled.view_product();
    let program = product.product().program().expect("View program");

    assert_eq!(checked.profile_id().map(ProfileId::as_str), Some("dev"));
    assert_eq!(checked.presentation().view().as_str(), "view.Mobile");
    assert_eq!(
        checked
            .presentation()
            .style()
            .map(arcweft_view::style::ViewStyleSheetId::as_str),
        Some("style.Mobile")
    );
    assert!(Arc::ptr_eq(checked.product(), product.product()));
    assert_eq!(
        checked.revision().manifest_document(),
        fixture.manifest_document.identity()
    );
    assert_eq!(
        checked.revision().topology_sources(),
        fixture.topology_revision
    );
    assert_eq!(
        checked.revision().compiled_sources(),
        product.product_source_revision()
    );
    assert_eq!(checked.revision().view_program_id(), program.program_id());
    assert_eq!(
        checked.revision().view_program_revision(),
        program.accepted_revision()
    );
    assert_eq!(
        checked.selected_view_source().source(),
        fixture.source_document.identity()
    );
    assert_eq!(
        checked
            .selected_style_source()
            .expect("Style provenance")
            .source(),
        fixture.source_document.identity()
    );
}

#[test]
fn omitted_dialogue_fields_admit_the_engine_standard_view() {
    let fixture = Fixture::new(DIALOGUE_SOURCE, "", true);
    let compiled = fixture.compile().expect("standard dialogue profile");
    let checked = compiled.dialogue_profile();

    assert_eq!(checked.presentation().view().as_str(), "std.view.dialogue");
    assert!(checked.presentation().style().is_none());
    assert_eq!(
        checked.selected_view_source().source().id().as_str(),
        arcweft_bundle::standard_view::DIALOGUE_VIEW_SOURCE_ID
    );
}

#[test]
fn direct_project_compilation_admits_a_revision_bound_project_default() {
    let fixture = Fixture::new_without_launch_profile(DIALOGUE_SOURCE);
    let compiled = fixture.compile().expect("project-default dialogue profile");
    let checked = compiled.dialogue_profile();

    assert_eq!(
        checked.owner(),
        &arcweft_compiler::project::DialogueProfileOwner::ProjectDefault
    );
    assert_eq!(checked.profile_id(), None);
    assert_eq!(checked.presentation().view().as_str(), "std.view.dialogue");
    assert_eq!(
        checked.revision().manifest_document(),
        fixture.manifest_document.identity()
    );
    assert_eq!(
        checked.revision().topology_sources(),
        fixture.topology_revision
    );
}

#[test]
fn profile_admission_rejects_missing_or_non_dialogue_catalog_entries() {
    for (dialogue, code) in [
        (
            "[profiles.dev.dialogue]\nview = \"view.Missing\"\n",
            "profile.dialogue.view.missing",
        ),
        (
            "[profiles.dev.dialogue]\nview = \"view.Plain\"\n",
            "profile.dialogue.view.not-dialogue",
        ),
        (
            "[profiles.dev.dialogue]\nstyle = \"style.Missing\"\n",
            "profile.dialogue.style.missing",
        ),
    ] {
        let fixture = Fixture::new(DIALOGUE_SOURCE, dialogue, true);
        let error = fixture.compile().expect_err("profile must be rejected");
        assert_eq!(error.diagnostics().len(), 1, "{error:?}");
        let diagnostic = &error.diagnostics()[0];
        assert_eq!(
            diagnostic.stage(),
            ProjectCompileStage::DialogueProfileAdmission
        );
        assert_eq!(
            diagnostic
                .diagnostic()
                .code()
                .map(arcweft_source::DiagnosticCode::as_str),
            Some(code)
        );
        let source = diagnostic.source().expect("manifest source");
        assert_eq!(
            source.document().identity(),
            fixture.manifest_document.identity()
        );
    }
}

#[test]
fn profile_admission_requires_the_exact_resource_registry_arc() {
    let fixture = Fixture::new(DIALOGUE_SOURCE, "", false);
    let error = fixture
        .compile()
        .expect_err("a separately constructed registry is not the accepted object");
    let diagnostic = &error.diagnostics()[0];

    assert_eq!(
        diagnostic.stage(),
        ProjectCompileStage::DialogueProfileAdmission
    );
    assert_eq!(
        diagnostic
            .diagnostic()
            .code()
            .map(arcweft_source::DiagnosticCode::as_str),
        Some("profile.dialogue.revision.mismatch")
    );
}

#[test]
fn dialogue_view_outside_a_view_remains_rejected_by_runtime_projection() {
    let fixture = Fixture::new("fn bad(value: DialogueView) { () }\n", "", true);
    let error = fixture
        .compile()
        .expect_err("DialogueView is a presentation role, not an ordinary runtime value");

    assert_eq!(error.diagnostics().len(), 1, "{error:?}");
    let diagnostic = &error.diagnostics()[0];
    assert_eq!(diagnostic.stage(), ProjectCompileStage::RuntimePlanLower);
    assert!(
        diagnostic
            .diagnostic()
            .message()
            .contains("runtime type `DialogueView` has no opaque producer evidence"),
        "{diagnostic:?}"
    );
    assert_eq!(
        diagnostic
            .diagnostic()
            .code()
            .map(arcweft_source::DiagnosticCode::as_str),
        Some("compiler.runtime_semantic_projection")
    );
}

struct Fixture {
    project: ProjectSources,
    context: ProjectCompilationContext,
    source_document: Arc<SourceDocument>,
    manifest_document: Arc<SourceDocument>,
    topology_revision: SourceSetRevision,
}

impl Fixture {
    fn new(source: &str, dialogue: &str, share_registry: bool) -> Self {
        Self::build(source, dialogue, share_registry, true)
    }

    fn new_without_launch_profile(source: &str) -> Self {
        Self::build(source, "", true, false)
    }

    fn build(source: &str, dialogue: &str, share_registry: bool, launch_profile: bool) -> Self {
        let manifest_text = format!(
            r#"schema = 1
[package]
id = "local.arcweft.dialogue-profile"
version = "0.0.0"

[profiles.dev]
kind = "game"
source = "main.arcw"

{dialogue}
"#
        );
        let source_document = fixture_document(
            "arcweft-test://dialogue-profile/source",
            "main.arcw",
            source,
        );
        let manifest_document = fixture_document(
            "arcweft-test://dialogue-profile/manifest",
            "arcw.toml",
            manifest_text,
        );
        let accepted = Arc::new(
            SourceBackedManifest::decode(Arc::clone(&manifest_document))
                .expect("accepted manifest"),
        );
        let resolved = accepted
            .resolve_profile(LaunchProfileSelection::Explicit("dev"))
            .expect("resolved profile");
        let topology_revision = SourceSetRevision::try_for_identities([
            manifest_document.identity(),
            source_document.identity(),
        ])
        .expect("topology source revision");
        let project = ProjectSources::new(
            PathBuf::from("arcw.toml"),
            PathBuf::new(),
            PackageSpec {
                id: PackageId::new("local.arcweft.dialogue-profile").expect("package ID"),
                version: PackageVersion::new("0.0.0").expect("package version"),
            },
            BuildSpec::default(),
            Arc::clone(&manifest_document),
            [ProjectSourceFile::new(
                CanonicalModulePath::crate_root(),
                PathBuf::from("main.arcw"),
                Arc::clone(&source_document),
                [],
            )],
        )
        .expect("project sources");
        let package =
            CallablePackageId::try_new(project.package().id.as_str()).expect("callable package ID");
        let world = ProjectSymbolWorldId::try_new(
            package,
            source_document.identity().id().clone(),
            "dialogue-profile-admission-test",
        )
        .expect("symbol world");
        let facts = ProjectRegistrationFacts::try_new(
            world,
            vec![Arc::clone(&source_document), Arc::clone(&manifest_document)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("registration facts");
        let compiler_registry = Arc::new(ResourceTypeRegistry::empty());
        let accepted_registry = if share_registry {
            Arc::clone(&compiler_registry)
        } else {
            Arc::new(ResourceTypeRegistry::empty())
        };
        let input = AcceptedLaunchProfileInput::new(
            Arc::clone(&accepted),
            resolved.id().clone(),
            resolved,
            topology_revision,
            accepted_registry,
        );
        let context = ProjectCompilationContext::new(
            Arc::new(TypeCheckEnv::standard()),
            Arc::new(facts),
            compiler_registry,
            None,
            None,
        );
        let context = if launch_profile {
            context.with_accepted_launch_profile(input)
        } else {
            context
        };
        Self {
            project,
            context,
            source_document,
            manifest_document,
            topology_revision,
        }
    }

    fn compile(
        &self,
    ) -> Result<
        arcweft_compiler::project::CompiledProject,
        arcweft_compiler::project::ProjectCompileError,
    > {
        let mut syntax = SyntaxDatabase::try_new().expect("dialogue test syntax database");
        let parsed_sources: BTreeMap<CanonicalModulePath, ParsedSource> = self
            .project
            .modules()
            .map(|source| {
                let parsed = syntax
                    .parse_initial(
                        SourceSnapshotId::initial(source.document().display_name().clone()),
                        Arc::clone(source.document()),
                        ParseOptions::default(),
                    )
                    .expect("dialogue test attached source");
                (source.module().clone(), parsed)
            })
            .collect();
        let mut compiler =
            ProjectCompilationSession::try_new().expect("dialogue test HIR database");
        compile_project(&mut compiler, &self.project, &parsed_sources, &self.context)
    }
}

fn fixture_document(id: &str, name: &str, text: impl Into<Arc<str>>) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("source document ID"),
            SourceName::path(name),
            text,
        )
        .expect("source document"),
    )
}
