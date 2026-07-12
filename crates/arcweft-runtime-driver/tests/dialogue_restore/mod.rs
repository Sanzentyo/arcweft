use super::paged_fixture_bundle;
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::presentation_handles::PresentationHandleId;
use arcweft_runtime_driver::session::{BundleSession, BundleSessionOptions, BundleStepInput};
use arcweft_runtime_driver::session_save::{BundleSessionSaveError, BundleSessionSnapshot};
use arcweft_runtime_driver::view_runtime::BundleViewMountOutput;
use arcweft_view::ViewMountId;

fn append_orphan_dialogue_mount(
    snapshot: &mut BundleSessionSnapshot,
    handle: &str,
) -> BundleViewMountOutput {
    let handle = PresentationHandleId::try_new(handle).expect("test handle is valid");
    let mount = ViewMountId::from_raw(snapshot.view_runtime.next_mount_id);
    snapshot.view_runtime.next_mount_id = snapshot
        .view_runtime
        .next_mount_id
        .checked_add(1)
        .expect("test mount cursor remains in range");

    let mut output = snapshot
        .presentation
        .view
        .mounts
        .iter()
        .find(|output| output.dialogue.is_some())
        .expect("dialogue root output is serialized")
        .clone();
    let mut retained = snapshot
        .view_runtime
        .mounts
        .iter()
        .find(|retained| retained.handle == output.handle && retained.path == output.path)
        .expect("dialogue root mount is retained")
        .clone();
    retained.handle = handle.clone();
    retained.state.mount = mount;
    snapshot.view_runtime.mounts.push(retained);

    output.handle = handle;
    output.mount = mount;
    output
}

fn assert_tamper_is_rejected_atomically(
    session: &mut BundleSession,
    before: &BundleSessionSnapshot,
    tampered: BundleSessionSnapshot,
    expected_message: &str,
) {
    let error = session
        .restore_session_snapshot(tampered)
        .expect_err("non-bijective dialogue View save state is rejected");
    assert!(
        matches!(
            error,
            BundleSessionSaveError::ViewRuntime { ref message }
                if message.contains(expected_message)
        ),
        "unexpected restore error: {error}"
    );
    assert_eq!(
        session
            .snapshot_session()
            .expect("failed restore leaves the live session valid"),
        *before
    );
}

#[test]
fn restore_requires_exact_store_mount_and_output_correspondence() {
    let bundle = paged_fixture_bundle();
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
    session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    let before = session.snapshot_session().expect("live snapshot exports");
    let dialogue_handle = before
        .presentation
        .view
        .mounts
        .iter()
        .find(|output| output.dialogue.is_some())
        .expect("dialogue View output exists")
        .handle
        .clone();

    let mut orphan_output_and_mount = before.clone();
    let orphan = append_orphan_dialogue_mount(
        &mut orphan_output_and_mount,
        "tampered.dialogue.output_and_mount",
    );
    orphan_output_and_mount
        .presentation
        .view
        .mounts
        .push(orphan);
    assert_tamper_is_rejected_atomically(
        &mut session,
        &before,
        orphan_output_and_mount,
        "has dialogue state but no presentation-store occurrence",
    );

    let mut orphan_retained_mount = before.clone();
    let _ = append_orphan_dialogue_mount(
        &mut orphan_retained_mount,
        "tampered.dialogue.retained_only",
    );
    assert_tamper_is_rejected_atomically(
        &mut session,
        &before,
        orphan_retained_mount,
        "has no live presentation owner",
    );

    let mut missing_output = before.clone();
    missing_output
        .presentation
        .view
        .mounts
        .retain(|output| output.handle != dialogue_handle);
    assert_tamper_is_rejected_atomically(
        &mut session,
        &before,
        missing_output,
        "has no serialized View output",
    );

    let mut missing_retained_mount = before.clone();
    missing_retained_mount
        .view_runtime
        .mounts
        .retain(|mount| mount.handle != dialogue_handle);
    assert_tamper_is_rejected_atomically(
        &mut session,
        &before,
        missing_retained_mount,
        "has no retained mount",
    );
}
