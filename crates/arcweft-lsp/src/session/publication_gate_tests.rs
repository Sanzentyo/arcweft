use std::sync::Arc;

use lsp_server::Notification;
use lsp_types::{
    DidChangeConfigurationParams, DidChangeWatchedFilesParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, TextDocumentIdentifier, TextDocumentItem,
    notification::{
        DidChangeConfiguration, DidChangeWatchedFiles, DidOpenTextDocument, DidSaveTextDocument,
        Notification as LspNotification,
    },
};

use super::{ArcweftLspSession, tests::TestProject, tests::file_uri};
use crate::{
    config::LspConfig,
    profiles::{
        AcceptedBuildWorkSnapshot, accepted_build_work_snapshot_for_test,
        state::AcceptedProfileEnvironment,
    },
    requests::{RequestRegistry, with_test_request_registry},
    uri_key::LspUriKey,
};
use arcweft_runtime_host::RuntimeHostRunnerKind;

const MANIFEST: &str = r#"schema = 1

[package]
id = "org.arcweft.tests.publication-gate"
version = "0.1.0"

[profiles.agent]
kind = "agent"
entry = "@entry.agent.main"
source = "src/main.arcw"
"#;

const MAIN: &str = r"
use crate.helpers.smoke

entry agent @entry.agent.main {
controller = smoke
}
";

const HELPERS: &str = r"
mod crate.helpers

pub fn smoke() -> Result<Unit, AgentError>
effects {}
{
Ok(())
}
";

#[test]
fn ah32_023a_and_028_each_event_publishes_one_complete_generation_per_shared_state() {
    let project = publication_project("lsp-single-publication-generation");
    let main_uri = file_uri(&project.path("src/main.arcw"));
    let helpers_uri = file_uri(&project.path("src/helpers.arcw"));
    let mut session = ArcweftLspSession::new(
        &LspConfig::new(RuntimeHostRunnerKind::Native).with_profile_id("agent"),
    );

    with_test_request_registry(|requests| {
        let before_initial_open = accepted_build_work_snapshot_for_test();
        notify(&mut session, requests, open(main_uri.clone(), 7, MAIN));
        assert_one_full_build(before_initial_open);
        let initial = accepted(&session, &main_uri);
        assert_eq!(initial.generation().get(), 1);
        assert_overlay(&initial, &main_uri, 7);
        assert_eq!(initial.overlays().iter().count(), 1);

        let before_second_open = accepted_build_work_snapshot_for_test();
        notify(
            &mut session,
            requests,
            open(helpers_uri.clone(), 9, HELPERS),
        );
        assert_no_full_build(before_second_open);
        let after_second_open = accepted(&session, &main_uri);
        assert_eq!(after_second_open.generation().get(), 2);
        assert!(Arc::ptr_eq(initial.world(), after_second_open.world()));
        assert!(Arc::ptr_eq(initial.project(), after_second_open.project()));
        assert_overlay(&after_second_open, &main_uri, 7);
        assert_overlay(&after_second_open, &helpers_uri, 9);
        assert_eq!(after_second_open.overlays().iter().count(), 2);
        let helpers_profile = session.profile_for_uri(&helpers_uri);
        assert!(Arc::ptr_eq(
            session.profile_for_uri(&main_uri).state(),
            helpers_profile.state()
        ));

        let mut generation = after_second_open.generation().get();
        let before_save = accepted_build_work_snapshot_for_test();
        notify(
            &mut session,
            requests,
            Notification::new(
                DidSaveTextDocument::METHOD.to_owned(),
                DidSaveTextDocumentParams {
                    text_document: TextDocumentIdentifier {
                        uri: main_uri.clone(),
                    },
                    text: None,
                },
            ),
        );
        assert_one_full_build(before_save);
        generation = assert_next_complete_generation(&session, &main_uri, &helpers_uri, generation);

        let before_watch = accepted_build_work_snapshot_for_test();
        notify(
            &mut session,
            requests,
            Notification::new(
                DidChangeWatchedFiles::METHOD.to_owned(),
                DidChangeWatchedFilesParams {
                    changes: Vec::new(),
                },
            ),
        );
        assert_one_full_build(before_watch);
        generation = assert_next_complete_generation(&session, &main_uri, &helpers_uri, generation);

        let before_configuration = accepted_build_work_snapshot_for_test();
        notify(
            &mut session,
            requests,
            Notification::new(
                DidChangeConfiguration::METHOD.to_owned(),
                DidChangeConfigurationParams {
                    settings: serde_json::Value::Null,
                },
            ),
        );
        assert_one_full_build(before_configuration);
        assert_eq!(
            assert_next_complete_generation(&session, &main_uri, &helpers_uri, generation,),
            5
        );
    });
}

#[test]
fn ah32_063_failed_watch_build_preserves_profile_metadata_accepted_arc_and_cache() {
    let project = publication_project("lsp-failed-publication-preserves-state");
    let main_uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(
        &LspConfig::new(RuntimeHostRunnerKind::Native).with_profile_id("agent"),
    );

    with_test_request_registry(|requests| {
        notify(&mut session, requests, open(main_uri.clone(), 11, MAIN));
        let profile_before = session.profile_for_uri(&main_uri).clone();
        let accepted_before = profile_before
            .accepted_environment()
            .expect("initial accepted environment");
        accepted_before.seed_signature_cache_for_test(0);
        let cache_before = accepted_before.signature_cache_snapshot_for_test();
        let generation_before = accepted_before.generation();

        project.write("arcw.toml", "not valid manifest = [");
        notify(
            &mut session,
            requests,
            Notification::new(
                DidChangeWatchedFiles::METHOD.to_owned(),
                DidChangeWatchedFilesParams {
                    changes: Vec::new(),
                },
            ),
        );

        let profile_after = session.profile_for_uri(&main_uri);
        let accepted_after = profile_after
            .accepted_environment()
            .expect("failed build retains the accepted environment");
        assert!(Arc::ptr_eq(profile_before.state(), profile_after.state()));
        assert!(Arc::ptr_eq(&accepted_before, &accepted_after));
        assert!(Arc::ptr_eq(accepted_before.world(), accepted_after.world()));
        assert!(Arc::ptr_eq(
            accepted_before.project(),
            accepted_after.project()
        ));
        assert_eq!(accepted_after.generation(), generation_before);
        assert_eq!(
            accepted_after.signature_cache_snapshot_for_test(),
            cache_before
        );
        assert_eq!(profile_after.adapter(), profile_before.adapter());
        assert_eq!(
            profile_after.declared_manifests(),
            profile_before.declared_manifests()
        );
        assert_eq!(
            profile_after.resolved_profile(),
            profile_before.resolved_profile()
        );
        assert_eq!(profile_after.characters(), profile_before.characters());
        assert_eq!(
            profile_after.entry_selections(),
            profile_before.entry_selections()
        );
        assert!(!profile_after.diagnostics().is_empty());
    });
}

#[test]
fn ah32_064_shutdown_rejects_late_notifications_without_repopulating_session_maps() {
    let uri: lsp_types::Uri = "file:///late-after-shutdown.arcw"
        .parse()
        .expect("test URI");
    let mut session = ArcweftLspSession::new(&LspConfig::default());

    with_test_request_registry(|requests| {
        session.begin_shutdown(requests);
        let notifications = session
            .handle_notification_with_requests(open(uri.clone(), 1, MAIN), requests)
            .expect("late notification is ignored");

        assert!(notifications.is_empty());
        assert!(session.documents.get(&uri).is_none());
        assert!(session.profiles_by_uri.is_empty());
        assert!(session.profile_keys_by_uri.is_empty());
        assert!(session.analyses_by_uri.is_empty());
        assert!(!session.signature_admission_open);
    });
}

fn publication_project(name: &str) -> TestProject {
    let project = TestProject::new(name);
    project.write("arcw.toml", MANIFEST);
    project.write("src/main.arcw", MAIN);
    project.write("src/helpers.arcw", HELPERS);
    project
}

fn open(uri: lsp_types::Uri, version: i32, text: &str) -> Notification {
    Notification::new(
        DidOpenTextDocument::METHOD.to_owned(),
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: "arcweft".to_owned(),
                version,
                text: text.to_owned(),
            },
        },
    )
}

fn notify(session: &mut ArcweftLspSession, requests: &RequestRegistry, notification: Notification) {
    session
        .handle_notification_with_requests(notification, requests)
        .expect("notification succeeds");
}

fn accepted(session: &ArcweftLspSession, uri: &lsp_types::Uri) -> Arc<AcceptedProfileEnvironment> {
    session
        .profile_for_uri(uri)
        .accepted_environment()
        .expect("accepted profile environment")
}

fn assert_overlay(accepted: &AcceptedProfileEnvironment, uri: &lsp_types::Uri, version: i32) {
    let key = LspUriKey::from_uri(uri);
    let overlay = accepted.overlays().get(&key).expect("accepted overlay");
    assert_eq!(overlay.version(), version);
    assert_eq!(
        accepted
            .project()
            .source_identity_by_uri(&key)
            .expect("accepted source identity"),
        overlay.logical_identity()
    );
}

fn assert_next_complete_generation(
    session: &ArcweftLspSession,
    main_uri: &lsp_types::Uri,
    helpers_uri: &lsp_types::Uri,
    previous: u64,
) -> u64 {
    let accepted = accepted(session, main_uri);
    assert!(Arc::ptr_eq(
        session.profile_for_uri(main_uri).state(),
        session.profile_for_uri(helpers_uri).state()
    ));
    assert_eq!(accepted.generation().get(), previous + 1);
    assert_overlay(&accepted, main_uri, 7);
    assert_overlay(&accepted, helpers_uri, 9);
    assert_eq!(accepted.overlays().iter().count(), 2);
    accepted.generation().get()
}

fn assert_one_full_build(before: AcceptedBuildWorkSnapshot) {
    let after = accepted_build_work_snapshot_for_test();
    assert_eq!(after.topology_loads - before.topology_loads, 1);
    assert_eq!(after.compiler_builds - before.compiler_builds, 1);
}

fn assert_no_full_build(before: AcceptedBuildWorkSnapshot) {
    assert_eq!(accepted_build_work_snapshot_for_test(), before);
}
