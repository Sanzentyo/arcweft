//! Complete typed stamp-mutation matrix for accepted signature requests.

use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use arcweft_lang_sema::registration::RegisteredSemanticWorld;
use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModuleSegment};
use arcweft_launch::ProfileId;
use lsp_server::{Connection, ErrorCode, Message, Notification, RequestId, Response};
use lsp_types::{
    DidChangeWatchedFilesParams, DidOpenTextDocumentParams, FileChangeType, FileEvent,
    SignatureHelpParams, TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams,
    WorkDoneProgressParams,
    notification::{DidChangeWatchedFiles, Notification as LspNotification},
};

use super::{
    ArcweftLspSession,
    tests::{TestProject, file_uri, open_text, position_after},
};
use crate::{
    config::LspConfig,
    profiles::{
        LspProfile,
        accepted_project::{
            AcceptedProjectSnapshot, AcceptedSourceDocumentSeed, AcceptedSourceLocator,
            stamp_test_support::{AcceptedProjectStampMutation, mutated_project},
        },
        state::{
            AcceptedEnvironmentGeneration, AcceptedProfileEnvironment, AcceptedProfileKey,
            stamp_test_support::{AcceptedEnvironmentStampMutation, mutated_environment},
        },
    },
    requests::{
        SignatureRequestRuntime,
        registry::SIGNATURE_REQUEST_DEADLINE,
        signature::{
            PreparedSignatureRequest, SignatureRequestError, SignatureRequestResult,
            SignatureRequestStale, SignatureRequestWork,
        },
    },
};

const MANIFEST: &str = r#"
schema = 1

[package]
id = "org.arcweft.tests.lsp.signature-stamp"
version = "0.1.0"

[content-units.characters]
roots = ["@character.zundamon"]
visibility = "package"
demand = "required"

[profiles.dev]
kind = "server"
entry = "@entry.server.main"
source = "src/main.arcw"
adapter = "sans-io"

[profiles.dev.content.characters]
residency = "startup"
placement = "embedded"
compression = "none"

[profiles.alt]
kind = "server"
entry = "@entry.server.main"
source = "src/main.arcw"
adapter = "sans-io"

[profiles.alt.content.characters]
residency = "startup"
placement = "embedded"
compression = "none"
"#;

const SOURCE: &str = "fn sum(lhs: i64, rhs: i64) -> i64 { lhs + rhs }\n\
fn evaluate(value: i64) -> i64 {\n    sum(value, value)\n}\n\
entry server @entry.server.main { goto @flow.main }\n\
flow @flow.main main {}\n";

const CHANGED_SOURCE: &str = "fn sum(lhs: i64, rhs: i64) -> i64 { lhs + rhs }\n\
fn evaluate(value: i64) -> i64 {\n    sum(value,  value)\n}\n\
entry server @entry.server.main { goto @flow.main }\n\
flow @flow.main main {}\n";

const CHARACTER_A: &str =
    include_str!("../../tests/fixtures/zundamon.awchar/character.awchar.json");

const CHARACTER_MANIFEST_PATH: &str = "assets/zundamon.awchar/character.awchar.json";

fn character_b() -> String {
    CHARACTER_A.replacen("\"x\": 48", "\"x\": 47", 1)
}

struct StampMatrixFixture {
    project: TestProject,
    uri: lsp_types::Uri,
    session: Arc<RwLock<ArcweftLspSession>>,
    runtime: Option<SignatureRequestRuntime>,
}

impl StampMatrixFixture {
    fn new(name: &str) -> Self {
        let project = TestProject::new(name);
        project.write("arcw.toml", MANIFEST);
        project.write("src/main.arcw", SOURCE);
        project.write(CHARACTER_MANIFEST_PATH, CHARACTER_A);
        std::fs::create_dir_all(project.path("assets/zundamon.awchar/layers"))
            .expect("character image directory");
        for (name, image) in [
            (
                "body--default.png",
                include_bytes!("../../tests/fixtures/zundamon.awchar/layers/body--default.png")
                    .as_slice(),
            ),
            (
                "eyes--normal.png",
                include_bytes!("../../tests/fixtures/zundamon.awchar/layers/eyes--normal.png")
                    .as_slice(),
            ),
            (
                "eyes--smile.png",
                include_bytes!("../../tests/fixtures/zundamon.awchar/layers/eyes--smile.png")
                    .as_slice(),
            ),
            (
                "mouth--neutral.png",
                include_bytes!("../../tests/fixtures/zundamon.awchar/layers/mouth--neutral.png")
                    .as_slice(),
            ),
            (
                "mouth--smile.png",
                include_bytes!("../../tests/fixtures/zundamon.awchar/layers/mouth--smile.png")
                    .as_slice(),
            ),
        ] {
            std::fs::write(
                project.path(&format!("assets/zundamon.awchar/layers/{name}")),
                image,
            )
            .expect("character image");
        }
        let uri = file_uri(&project.path("src/main.arcw"));
        let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
        open_text(&mut session, uri.clone(), SOURCE);
        assert!(
            session
                .profile_for_uri(&uri)
                .accepted_environment()
                .is_some(),
            "stamp fixture diagnostics: {:?}",
            session.profile_for_uri(&uri).diagnostics()
        );
        let session = Arc::new(RwLock::new(session));
        let (server, _client) = Connection::memory();
        let runtime = SignatureRequestRuntime::new_with_deadline_for_test(
            &server,
            Arc::clone(&session),
            SIGNATURE_REQUEST_DEADLINE.max(Duration::from_secs(10)),
        )
        .expect("signature stamp runtime");
        Self {
            project,
            uri,
            session,
            runtime: Some(runtime),
        }
    }

    fn prepare(&self, request_id: i32) -> PreparedSignatureRequest {
        self.session
            .read()
            .expect("session read")
            .prepare_signature_request(
                RequestId::from(request_id),
                SignatureHelpParams {
                    text_document_position_params: TextDocumentPositionParams {
                        text_document: TextDocumentIdentifier {
                            uri: self.uri.clone(),
                        },
                        position: position_after(SOURCE, "sum("),
                    },
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    context: None,
                },
                self.runtime
                    .as_ref()
                    .expect("active stamp runtime")
                    .registry(),
            )
            .expect("prepared stamp request")
    }

    fn accepted(&self) -> Arc<AcceptedProfileEnvironment> {
        self.session
            .read()
            .expect("session read")
            .profile_for_uri(&self.uri)
            .accepted_environment()
            .expect("accepted stamp environment")
    }

    #[allow(
        clippy::result_large_err,
        reason = "the matrix asserts the exact production request error"
    )]
    fn execute(
        &self,
        prepared: &PreparedSignatureRequest,
    ) -> Result<SignatureRequestResult, SignatureRequestError> {
        let work = self
            .session
            .read()
            .expect("session read")
            .signature_work(prepared)?;
        match work {
            SignatureRequestWork::Hit(result) => Ok(result),
            SignatureRequestWork::Miss(key) => ArcweftLspSession::compute_signature(prepared, key),
        }
    }

    fn publish(
        &self,
        prepared: &PreparedSignatureRequest,
        result: Result<SignatureRequestResult, SignatureRequestError>,
    ) -> Response {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        self.session
            .read()
            .expect("session read")
            .publish_signature_result(prepared, result, &sender);
        match receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("stamp response")
        {
            Message::Response(response) => response,
            other => panic!("unexpected stamp message: {other:?}"),
        }
    }

    fn accepted_from_disk(
        &self,
        profile: &str,
        source: &str,
        character: &str,
    ) -> Arc<AcceptedProfileEnvironment> {
        self.project.write("src/main.arcw", source);
        self.project.write(CHARACTER_MANIFEST_PATH, character);
        let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id(profile));
        open_text(&mut session, self.uri.clone(), source);
        let accepted = session
            .profile_for_uri(&self.uri)
            .accepted_environment()
            .expect("alternate accepted environment");
        self.project.write("src/main.arcw", SOURCE);
        self.project.write(CHARACTER_MANIFEST_PATH, CHARACTER_A);
        accepted
    }

    fn accepted_with_adapter_from_disk(&self, adapter: &str) -> Arc<AcceptedProfileEnvironment> {
        let manifest = MANIFEST.replacen(
            "adapter = \"sans-io\"",
            &format!("adapter = \"{adapter}\""),
            1,
        );
        self.project.write("arcw.toml", &manifest);
        let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
        open_text(&mut session, self.uri.clone(), SOURCE);
        let accepted = session
            .profile_for_uri(&self.uri)
            .accepted_environment()
            .expect("alternate adapter environment");
        self.project.write("arcw.toml", MANIFEST);
        accepted
    }

    fn advanced_character_revision(&self) -> Arc<AcceptedProfileEnvironment> {
        self.project.write("src/main.arcw", SOURCE);
        self.project.write(CHARACTER_MANIFEST_PATH, &character_b());
        let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
        open_text(&mut session, self.uri.clone(), SOURCE);
        self.project.write(CHARACTER_MANIFEST_PATH, CHARACTER_A);
        session
            .handle_notification(Notification::new(
                DidChangeWatchedFiles::METHOD.to_owned(),
                DidChangeWatchedFilesParams {
                    changes: vec![FileEvent {
                        uri: file_uri(&self.project.path(CHARACTER_MANIFEST_PATH)),
                        typ: FileChangeType::CHANGED,
                    }],
                },
            ))
            .expect("character manifest reload");
        let accepted = session
            .profile_for_uri(&self.uri)
            .accepted_environment()
            .expect("revision-advanced environment");
        self.project.write("src/main.arcw", SOURCE);
        self.project.write(CHARACTER_MANIFEST_PATH, CHARACTER_A);
        accepted
    }
}

impl Drop for StampMatrixFixture {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown();
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ValidationPoint {
    CacheHit,
    PostCompute,
}

impl ValidationPoint {
    const fn label(self) -> &'static str {
        match self {
            Self::CacheHit => "hit",
            Self::PostCompute => "post-compute",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum StampMutation {
    ProfileRemappedOther,
    ProfileRemappedNone,
    ProfileState,
    AcceptedAllocation,
    Generation,
    WorldAllocation,
    WorldIdentity,
    SymbolRevision,
    CharacterDigest,
    CharacterRevision,
    EnvironmentDigest,
    ProjectAllocation,
    HirAllocation,
    DocumentIdentity,
    DocumentVersion,
    UriMapping,
    AcceptedDocumentAllocation,
    ModuleMapping,
    ProfileKey,
}

const STAMP_MUTATIONS: [StampMutation; 19] = [
    StampMutation::ProfileRemappedOther,
    StampMutation::ProfileRemappedNone,
    StampMutation::ProfileState,
    StampMutation::AcceptedAllocation,
    StampMutation::Generation,
    StampMutation::WorldAllocation,
    StampMutation::WorldIdentity,
    StampMutation::SymbolRevision,
    StampMutation::CharacterDigest,
    StampMutation::CharacterRevision,
    StampMutation::EnvironmentDigest,
    StampMutation::ProjectAllocation,
    StampMutation::HirAllocation,
    StampMutation::DocumentIdentity,
    StampMutation::DocumentVersion,
    StampMutation::UriMapping,
    StampMutation::AcceptedDocumentAllocation,
    StampMutation::ModuleMapping,
    StampMutation::ProfileKey,
];

impl StampMutation {
    const fn label(self) -> &'static str {
        match self {
            Self::ProfileRemappedOther => "profile-other",
            Self::ProfileRemappedNone => "profile-none",
            Self::ProfileState => "profile-state",
            Self::AcceptedAllocation => "accepted-arc",
            Self::Generation => "generation",
            Self::WorldAllocation => "world-arc",
            Self::WorldIdentity => "world-id",
            Self::SymbolRevision => "symbol-revision",
            Self::CharacterDigest => "character-digest",
            Self::CharacterRevision => "character-revision",
            Self::EnvironmentDigest => "environment-digest",
            Self::ProjectAllocation => "project-arc",
            Self::HirAllocation => "hir-arc",
            Self::DocumentIdentity => "document-identity",
            Self::DocumentVersion => "document-version",
            Self::UriMapping => "uri-mapping",
            Self::AcceptedDocumentAllocation => "accepted-document-arc",
            Self::ModuleMapping => "module",
            Self::ProfileKey => "profile-key",
        }
    }

    const fn stable_code(self) -> &'static str {
        match self {
            Self::ProfileRemappedOther | Self::ProfileRemappedNone => {
                "aw.signature.stale.profile_remapped"
            }
            Self::ProfileState => "aw.signature.stale.profile_state_replaced",
            Self::AcceptedAllocation => "aw.signature.stale.accepted_replaced",
            Self::Generation => "aw.signature.stale.generation_changed",
            Self::WorldAllocation => "aw.signature.stale.world_arc_changed",
            Self::WorldIdentity => "aw.signature.stale.world_identity_changed",
            Self::SymbolRevision => "aw.signature.stale.symbol_revision_changed",
            Self::CharacterDigest => "aw.signature.stale.character_digest_changed",
            Self::CharacterRevision => "aw.signature.stale.character_revision_changed",
            Self::EnvironmentDigest => "aw.signature.stale.environment_digest_changed",
            Self::ProjectAllocation => "aw.signature.stale.project_arc_changed",
            Self::HirAllocation => "aw.signature.stale.hir_changed",
            Self::DocumentIdentity => "aw.signature.stale.document_changed",
            Self::DocumentVersion => "aw.signature.stale.document_version_changed",
            Self::UriMapping => "aw.signature.stale.uri_remapped",
            Self::AcceptedDocumentAllocation => "aw.signature.stale.accepted_document_changed",
            Self::ModuleMapping => "aw.signature.stale.module_changed",
            Self::ProfileKey => "aw.signature.stale.profile_key_changed",
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the typed mutation table keeps every contract and later cache authority visibly exhaustive"
    )]
    fn apply(
        self,
        fixture: &StampMatrixFixture,
        prepared: &mut PreparedSignatureRequest,
        point: ValidationPoint,
    ) -> Option<Arc<AcceptedProfileEnvironment>> {
        match self {
            Self::ProfileRemappedOther => {
                fixture
                    .session
                    .write()
                    .expect("session write")
                    .profile_keys_by_uri
                    .insert(prepared.stamp().uri().clone(), alternate_profile(prepared));
                None
            }
            Self::ProfileRemappedNone => {
                fixture
                    .session
                    .write()
                    .expect("session write")
                    .profile_keys_by_uri
                    .remove(prepared.stamp().uri());
                None
            }
            Self::ProfileState => {
                let mut session = fixture.session.write().expect("session write");
                let runner = session
                    .profiles_by_uri
                    .get(prepared.stamp().uri())
                    .expect("mapped profile")
                    .runner();
                session.profiles_by_uri.insert(
                    prepared.stamp().uri().clone(),
                    LspProfile::default_for_runner(runner),
                );
                None
            }
            Self::AcceptedAllocation => Some(install_environment(
                prepared,
                AcceptedEnvironmentStampMutation::Allocation,
            )),
            Self::Generation => Some(install_environment(
                prepared,
                AcceptedEnvironmentStampMutation::Generation(
                    AcceptedEnvironmentGeneration::for_test(
                        prepared.stamp().generation().get() + 1,
                    ),
                ),
            )),
            Self::WorldAllocation => Some(install_environment(
                prepared,
                AcceptedEnvironmentStampMutation::World(Arc::new(
                    prepared.stamp().world().as_ref().clone(),
                )),
            )),
            Self::WorldIdentity => {
                let alternate = fixture.accepted_from_disk("alt", SOURCE, CHARACTER_A);
                assert_ne!(
                    prepared.stamp().world_id(),
                    alternate.world().symbols().world()
                );
                Some(install_environment(
                    prepared,
                    AcceptedEnvironmentStampMutation::World(Arc::clone(alternate.world())),
                ))
            }
            Self::SymbolRevision => {
                let alternate = fixture.accepted_from_disk("dev", CHANGED_SOURCE, CHARACTER_A);
                assert_eq!(
                    prepared.stamp().world_id(),
                    alternate.world().symbols().world()
                );
                assert_ne!(
                    prepared.stamp().symbol_revision(),
                    *alternate.world().symbols().revision()
                );
                Some(install_environment(
                    prepared,
                    AcceptedEnvironmentStampMutation::World(Arc::clone(alternate.world())),
                ))
            }
            Self::CharacterDigest => {
                let alternate = fixture.accepted_from_disk("dev", SOURCE, &character_b());
                assert_eq!(
                    prepared.stamp().world_id(),
                    alternate.world().symbols().world()
                );
                prepared
                    .align_symbol_revision_for_stamp_test(*alternate.world().symbols().revision());
                assert_ne!(
                    prepared.stamp().character_digest(),
                    alternate.world().environment().character_digest()
                );
                Some(install_environment(
                    prepared,
                    AcceptedEnvironmentStampMutation::World(Arc::clone(alternate.world())),
                ))
            }
            Self::CharacterRevision => {
                let alternate = fixture.advanced_character_revision();
                assert_world_and_revision(prepared, alternate.world());
                assert_eq!(
                    prepared.stamp().character_digest(),
                    alternate.world().environment().character_digest()
                );
                assert_ne!(
                    prepared.stamp().character_revision(),
                    alternate.world().environment().character_revision()
                );
                Some(install_environment(
                    prepared,
                    AcceptedEnvironmentStampMutation::World(Arc::clone(alternate.world())),
                ))
            }
            Self::EnvironmentDigest => {
                let alternate = fixture.accepted_with_adapter_from_disk("inference-tensor");
                assert_eq!(
                    prepared.stamp().world_id(),
                    alternate.world().symbols().world()
                );
                let alternate_symbol_revision = *alternate.world().symbols().revision();
                assert_eq!(
                    prepared.stamp().character_digest(),
                    alternate.world().environment().character_digest()
                );
                assert_eq!(
                    prepared.stamp().character_revision(),
                    alternate.world().environment().character_revision()
                );
                assert_ne!(
                    prepared.stamp().environment_digest(),
                    alternate.world().environment().environment_digest()
                );
                if matches!(point, ValidationPoint::CacheHit) {
                    let byte_offset = prepared
                        .snapshot()
                        .line_index()
                        .try_byte_offset_from_position(prepared.position())
                        .expect("stamp cursor byte offset");
                    let cached = fixture
                        .execute(prepared)
                        .expect("baseline cached result before environment mutation");
                    let outcome = Arc::clone(cached.outcome());
                    prepared.align_symbol_revision_for_stamp_test(alternate_symbol_revision);
                    prepared.stamp().accepted().clear_caches();
                    let _ = prepared.stamp().accepted().signature_cache().insert(
                        prepared.stamp().cache_key(byte_offset),
                        outcome,
                        prepared.stamp().project().footprint().source_bytes(),
                    );
                } else {
                    prepared.align_symbol_revision_for_stamp_test(alternate_symbol_revision);
                }
                Some(install_environment(
                    prepared,
                    AcceptedEnvironmentStampMutation::World(Arc::clone(alternate.world())),
                ))
            }
            Self::ProjectAllocation => Some(install_project(
                prepared,
                rebuilt_project(prepared, ProjectRebuild::Allocation),
            )),
            Self::HirAllocation => Some(install_project(
                prepared,
                rebuilt_project(prepared, ProjectRebuild::HirAllocation),
            )),
            Self::DocumentIdentity => {
                let mut session = fixture.session.write().expect("session write");
                let encoding = session.position_encoding;
                session.documents.open(
                    DidOpenTextDocumentParams {
                        text_document: TextDocumentItem {
                            uri: fixture.uri.clone(),
                            language_id: "arcweft".to_owned(),
                            version: prepared.stamp().lsp_version(),
                            text: CHANGED_SOURCE.to_owned(),
                        },
                    },
                    encoding,
                );
                None
            }
            Self::DocumentVersion => {
                let mut session = fixture.session.write().expect("session write");
                let encoding = session.position_encoding;
                session.documents.open(
                    DidOpenTextDocumentParams {
                        text_document: TextDocumentItem {
                            uri: fixture.uri.clone(),
                            language_id: "arcweft".to_owned(),
                            version: prepared.stamp().lsp_version() + 1,
                            text: SOURCE.to_owned(),
                        },
                    },
                    encoding,
                );
                None
            }
            Self::UriMapping => Some(install_project(
                prepared,
                rebuilt_project(prepared, ProjectRebuild::RemoveUri),
            )),
            Self::AcceptedDocumentAllocation => Some(install_project(
                prepared,
                rebuilt_project(prepared, ProjectRebuild::DocumentAllocation),
            )),
            Self::ModuleMapping => {
                let module =
                    CanonicalModulePath::from_segments([
                        ModuleSegment::new("stamp_other").expect("module segment")
                    ]);
                let project = mutated_project(
                    prepared.stamp().project(),
                    AcceptedProjectStampMutation::ModuleMapping {
                        source: prepared.stamp().accepted_document_identity().clone(),
                        module,
                    },
                );
                Some(install_project(prepared, project))
            }
            Self::ProfileKey => Some(install_environment(
                prepared,
                AcceptedEnvironmentStampMutation::Profile(alternate_profile(prepared)),
            )),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the exact stale-variant table mirrors every mutation without string decoding"
    )]
    fn assert_exact(
        self,
        stale: &SignatureRequestStale,
        prepared: &PreparedSignatureRequest,
        fixture: &StampMatrixFixture,
    ) {
        match (self, stale) {
            (
                Self::ProfileRemappedOther,
                SignatureRequestStale::ProfileRemapped { expected, actual },
            ) => {
                assert_eq!(expected, prepared.stamp().profile());
                assert_eq!(actual.as_ref(), Some(&alternate_profile(prepared)));
            }
            (
                Self::ProfileRemappedNone,
                SignatureRequestStale::ProfileRemapped { expected, actual },
            ) => {
                assert_eq!(expected, prepared.stamp().profile());
                assert_eq!(actual, &None);
            }
            (Self::ProfileState, SignatureRequestStale::ProfileStateReplaced)
            | (Self::AcceptedAllocation, SignatureRequestStale::AcceptedReplaced)
            | (Self::WorldAllocation, SignatureRequestStale::WorldArcChanged)
            | (Self::ProjectAllocation, SignatureRequestStale::ProjectArcChanged)
            | (Self::HirAllocation, SignatureRequestStale::HirChanged { .. }) => {}
            (Self::Generation, SignatureRequestStale::GenerationChanged { expected, actual }) => {
                assert_eq!(*expected, prepared.stamp().generation());
                assert_eq!(actual.get(), expected.get() + 1);
            }
            (
                Self::WorldIdentity,
                SignatureRequestStale::WorldIdentityChanged { expected, actual },
            ) => {
                assert_eq!(expected, prepared.stamp().world_id());
                assert_eq!(
                    actual,
                    current_environment(prepared).world().symbols().world()
                );
            }
            (
                Self::SymbolRevision,
                SignatureRequestStale::SymbolRevisionChanged { expected, actual },
            ) => {
                assert_eq!(*expected, prepared.stamp().symbol_revision());
                assert_eq!(
                    *actual,
                    *current_environment(prepared).world().symbols().revision()
                );
            }
            (
                Self::CharacterDigest,
                SignatureRequestStale::CharacterDigestChanged { expected, actual },
            ) => {
                assert_eq!(*expected, prepared.stamp().character_digest());
                assert_eq!(
                    *actual,
                    current_environment(prepared)
                        .world()
                        .environment()
                        .character_digest()
                );
            }
            (
                Self::CharacterRevision,
                SignatureRequestStale::CharacterRevisionChanged { expected, actual },
            ) => {
                assert_eq!(*expected, prepared.stamp().character_revision());
                assert_eq!(
                    *actual,
                    current_environment(prepared)
                        .world()
                        .environment()
                        .character_revision()
                );
            }
            (
                Self::EnvironmentDigest,
                SignatureRequestStale::EnvironmentDigestChanged { expected, actual },
            ) => {
                assert_eq!(*expected, prepared.stamp().environment_digest());
                assert_eq!(
                    *actual,
                    current_environment(prepared)
                        .world()
                        .environment()
                        .environment_digest()
                );
            }
            (
                Self::DocumentIdentity,
                SignatureRequestStale::DocumentChanged { expected, actual },
            ) => {
                assert_eq!(expected, prepared.stamp().protocol_document());
                assert_ne!(actual, prepared.stamp().protocol_document());
            }
            (
                Self::DocumentVersion,
                SignatureRequestStale::DocumentVersionChanged { expected, actual },
            ) => {
                assert_eq!(*expected, prepared.stamp().lsp_version());
                assert_eq!(*actual, prepared.stamp().lsp_version() + 1);
            }
            (Self::UriMapping, SignatureRequestStale::UriRemapped { expected, actual }) => {
                assert_eq!(expected, prepared.stamp().accepted_document_identity());
                assert_eq!(actual, &None);
            }
            (
                Self::AcceptedDocumentAllocation,
                SignatureRequestStale::AcceptedDocumentChanged { expected, actual },
            ) => {
                assert_eq!(expected, prepared.stamp().accepted_document_identity());
                assert_eq!(
                    actual.as_ref(),
                    Some(prepared.stamp().accepted_document_identity())
                );
            }
            (Self::ModuleMapping, SignatureRequestStale::ModuleChanged { expected, actual }) => {
                assert_eq!(expected, prepared.stamp().module());
                assert_eq!(
                    actual.as_ref(),
                    current_environment(prepared)
                        .project()
                        .module_key(prepared.stamp().accepted_document_identity())
                        .as_ref()
                );
            }
            (Self::ProfileKey, SignatureRequestStale::ProfileKeyChanged { expected, actual }) => {
                assert_eq!(expected, prepared.stamp().profile());
                assert_eq!(actual, current_environment(prepared).profile());
            }
            _ => panic!("{} produced another stale variant: {stale:?}", self.label()),
        }
        assert_eq!(stale.stable_code(), self.stable_code());
        let error = fixture
            .session
            .read()
            .expect("session read")
            .signature_work(prepared)
            .expect_err("authority remains stale");
        assert!(matches!(error, SignatureRequestError::Stale(_)));
    }
}

#[derive(Clone, Copy)]
enum ProjectRebuild {
    Allocation,
    HirAllocation,
    RemoveUri,
    DocumentAllocation,
}

fn rebuilt_project(
    prepared: &PreparedSignatureRequest,
    mutation: ProjectRebuild,
) -> Arc<AcceptedProjectSnapshot> {
    let current = prepared.stamp().project();
    let target = prepared.stamp().accepted_document_identity();
    let seeds = current
        .sources()
        .documents()
        .map(|source| {
            let is_target = source.document().identity() == target;
            let document = if is_target && matches!(mutation, ProjectRebuild::DocumentAllocation) {
                Arc::new(source.document().as_ref().clone())
            } else {
                Arc::clone(source.document())
            };
            let locator = if is_target && matches!(mutation, ProjectRebuild::RemoveUri) {
                AcceptedSourceLocator::Unavailable
            } else {
                source.locator().clone()
            };
            AcceptedSourceDocumentSeed::new(document, locator, source.ownership(), source.access())
        })
        .collect();
    let hir = if matches!(mutation, ProjectRebuild::HirAllocation) {
        Arc::new(current.hir_project().as_ref().clone())
    } else {
        Arc::clone(current.hir_project())
    };
    Arc::new(
        AcceptedProjectSnapshot::try_new(hir, prepared.stamp().world().as_ref(), seeds)
            .expect("reconstructed accepted stamp project"),
    )
}

fn install_environment(
    prepared: &PreparedSignatureRequest,
    mutation: AcceptedEnvironmentStampMutation,
) -> Arc<AcceptedProfileEnvironment> {
    let environment = mutated_environment(prepared.stamp().accepted(), mutation);
    prepared
        .stamp()
        .profile_state()
        .install_stamp_environment_for_test(Arc::clone(&environment));
    environment
}

fn install_project(
    prepared: &PreparedSignatureRequest,
    project: Arc<AcceptedProjectSnapshot>,
) -> Arc<AcceptedProfileEnvironment> {
    install_environment(prepared, AcceptedEnvironmentStampMutation::Project(project))
}

fn current_environment(prepared: &PreparedSignatureRequest) -> Arc<AcceptedProfileEnvironment> {
    prepared
        .stamp()
        .profile_state()
        .current()
        .expect("current stamp environment")
}

fn alternate_profile(prepared: &PreparedSignatureRequest) -> AcceptedProfileKey {
    let workspace = prepared
        .stamp()
        .profile()
        .workspace_key()
        .as_str()
        .parse()
        .expect("workspace URI");
    let manifest = prepared
        .stamp()
        .profile()
        .manifest_key()
        .as_str()
        .parse()
        .expect("manifest URI");
    AcceptedProfileKey::new(
        &workspace,
        &manifest,
        ProfileId::new("stamp-other").expect("profile id"),
    )
}

fn assert_world_and_revision(
    prepared: &PreparedSignatureRequest,
    world: &Arc<RegisteredSemanticWorld>,
) {
    assert_eq!(prepared.stamp().world_id(), world.symbols().world());
    assert_eq!(
        prepared.stamp().symbol_revision(),
        *world.symbols().revision()
    );
}

fn stale_from(error: SignatureRequestError) -> SignatureRequestStale {
    match error {
        SignatureRequestError::Stale(stale) => stale,
        other => panic!("expected typed stamp staleness, got {other:?}"),
    }
}

fn prime_signature_cache(fixture: &StampMatrixFixture) {
    let prepared = fixture.prepare(1);
    let response = fixture.publish(&prepared, fixture.execute(&prepared));
    assert!(response.error.is_none(), "{:?}", response.error);
    drop(prepared);
}

fn run_stamp_case(mutation: StampMutation, point: ValidationPoint) {
    let fixture = StampMatrixFixture::new(&format!(
        "lsp-signature-stamp-{}-{}",
        mutation.label(),
        point.label()
    ));
    if matches!(point, ValidationPoint::CacheHit) {
        prime_signature_cache(&fixture);
    }
    let accepted = fixture.accepted();
    let mut prepared = fixture.prepare(2);
    let computed = matches!(point, ValidationPoint::PostCompute)
        .then(|| fixture.execute(&prepared).expect("post-compute result"));
    let next = mutation.apply(&fixture, &mut prepared, point);
    let before = accepted.signature_cache_snapshot_for_test();
    assert_eq!(
        before.entries,
        usize::from(matches!(point, ValidationPoint::CacheHit))
    );
    let stale = stale_from(
        fixture
            .execute(&prepared)
            .expect_err("mutated authority must reject before cache access"),
    );
    mutation.assert_exact(&stale, &prepared, &fixture);

    let publication = match computed {
        Some(result) => Ok(result),
        None => Err(SignatureRequestError::from(stale.clone())),
    };
    let response = fixture.publish(&prepared, publication);
    let error = response.error.expect("typed stale response");
    assert_eq!(error.code, ErrorCode::ContentModified as i32);
    assert_eq!(
        error.data,
        Some(serde_json::json!({ "code": mutation.stable_code() }))
    );
    drop(prepared);

    let after = accepted.signature_cache_snapshot_for_test();
    assert_eq!(after, before);
    if let Some(next) = next {
        assert!(!Arc::ptr_eq(&accepted, &next));
        assert_eq!(next.signature_cache_snapshot_for_test().entries, 0);
    }
}

#[test]
fn every_stamp_authority_is_fail_closed_on_hit_and_after_compute() {
    for mutation in STAMP_MUTATIONS {
        for point in [ValidationPoint::CacheHit, ValidationPoint::PostCompute] {
            run_stamp_case(mutation, point);
        }
    }
}
