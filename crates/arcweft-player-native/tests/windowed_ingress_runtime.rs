mod character_support;

use arcweft_bundle::container::{BundleDigest, BundleView, ReadBudget};
use arcweft_bundle::patch::{BundlePatchArtifact, decode_patch_bundle, encode_patch_bundle};
use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_bundle::{ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_core::line_task::LineTaskGroup;
use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeLineId, RuntimePlan};
use arcweft_dialogue::{DialoguePresentationProfile, DialogueProfileRevision};
use arcweft_id::TextKey;
use arcweft_player_native::windowed_patch::{
    FrameBoundary, PatchEventSource, WindowedPatchEvent, WindowedPatchState,
};
use arcweft_player_native::{WindowedRuntimeOutcome, WindowedRuntimeOwner};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{BundleSessionOptions, BundleStepInput};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
use arcweft_text_model::{
    DialogueContentCatalog, DialogueContentSpec, RichTextDocument, RichTextNode,
};
use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn test_dialogue_profile_revision() -> DialogueProfileRevision {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("player-native-ingress-dialogue-profile").unwrap(),
        SourceName::Memory,
        "schema = 1\n",
    )
    .unwrap();
    let sources = SourceSetRevision::try_for_identities([source.identity()]).unwrap();
    DialogueProfileRevision::from_admitted_parts(
        source.identity().clone(),
        sources,
        sources,
        ViewProgramId::try_new("view_program.player_native_ingress").unwrap(),
        AcceptedViewProgramRevision::try_from_bytes([0x46; 32]).unwrap(),
        ResourceTypeRegistry::empty().digest(),
    )
}

fn fixture_runtime_artifact_fingerprint() -> arcweft_core::effect::RuntimeArtifactFingerprint {
    arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x6a; 32])
        .expect("fixture runtime artifact fingerprint is non-zero")
}

#[test]
fn malformed_sidecar_reaches_owner_retained_report_without_session_mutation() {
    let active = fixture_bundle_with("Active text");
    let active_bytes = awfb_bytes(&active);
    let active_root = awfb_root(&active_bytes);
    let mut owner = WindowedRuntimeOwner::from_bundle(&active, BundleSessionOptions::default())
        .expect("owner starts");

    owner.push_patch_event(WindowedPatchEvent::ApplyTransportSidecar {
        bytes: b"not json".to_vec(),
        base_dir: PathBuf::from("."),
        source: PatchEventSource::OneShotSidecar,
    });
    let outcomes = owner
        .drain_patch_boundary(FrameBoundary::AfterRenderSubmitted)
        .expect("malformed sidecar is retained as rejected outcome");

    assert!(matches!(
        outcomes.as_slice(),
        [WindowedRuntimeOutcome::Rejected { .. }]
    ));
    assert_eq!(
        owner.last_patch_report().state,
        WindowedPatchState::Rejected
    );
    assert_eq!(
        owner.session().active_container_content_root(),
        Some(active_root)
    );
    assert_eq!(
        step_dialogue_text(&mut owner),
        Some("Active text".to_owned())
    );
}

#[test]
fn wrong_base_sidecar_does_not_mutate_active_session_catalog() {
    let active = fixture_bundle_with("Active text");
    let other_base = fixture_bundle_with("Other base text");
    let target = fixture_bundle_with("Target text");
    let active_bytes = awfb_bytes(&active);
    let other_base_bytes = awfb_bytes(&other_base);
    let target_bytes = awfb_bytes(&target);
    let active_root = awfb_root(&active_bytes);
    let other_base_root = awfb_root(&other_base_bytes);
    let target_root = awfb_root(&target_bytes);
    let patch_bytes = patch_bytes(&other_base_bytes, &target_bytes);
    let operation_count = patch_operation_count(&patch_bytes);
    let temp_dir = temp_dir("wrong-base-sidecar");
    fs::create_dir_all(&temp_dir).expect("temp dir");
    fs::write(temp_dir.join("update.awfb"), patch_bytes).expect("patch writes");
    let sidecar = serde_json::to_vec(&serde_json::json!({
        "schema_version": 1,
        "runner": "native",
        "source": "src/main.arcw",
        "target_bundle": "target.awfb",
        "patch_bundle": "update.awfb",
        "base_content_root": other_base_root.to_string(),
        "target_content_root": target_root.to_string(),
        "compatibility": "content-only",
        "operation_count": operation_count,
        "action": "apply_patch"
    }))
    .expect("sidecar encodes");
    let mut owner = WindowedRuntimeOwner::from_bundle(&active, BundleSessionOptions::default())
        .expect("owner starts");

    owner.push_patch_event(WindowedPatchEvent::ApplyTransportSidecar {
        bytes: sidecar,
        base_dir: temp_dir.clone(),
        source: PatchEventSource::OneShotSidecar,
    });
    let outcomes = owner
        .drain_patch_boundary(FrameBoundary::AfterRenderSubmitted)
        .expect("wrong-base sidecar is retained as rejected outcome");
    let _ = fs::remove_dir_all(&temp_dir);

    assert!(matches!(
        outcomes.as_slice(),
        [WindowedRuntimeOutcome::Rejected { .. }]
    ));
    assert_eq!(
        owner.last_patch_report().state,
        WindowedPatchState::Rejected
    );
    assert_eq!(
        owner.session().active_container_content_root(),
        Some(active_root)
    );
    assert_eq!(
        step_dialogue_text(&mut owner),
        Some("Active text".to_owned())
    );
}

fn step_dialogue_text(owner: &mut WindowedRuntimeOwner) -> Option<String> {
    let step = owner.session_mut().step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    step.presentation
        .dialogue
        .latest_active()
        .and_then(|(_, entry)| entry.current_stage())
        .map(|stage| stage.text().to_owned())
}

fn awfb_bytes(bundle: &ArcweftBundle) -> Vec<u8> {
    bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("fixture encodes")
}

fn awfb_root(bytes: &[u8]) -> BundleDigest {
    BundleView::parse(bytes, ReadBudget::default())
        .expect("fixture parses")
        .content_root()
}

fn patch_bytes(old: &[u8], new: &[u8]) -> Vec<u8> {
    let old_view = BundleView::parse(old, ReadBudget::default()).expect("old parses");
    let new_view = BundleView::parse(new, ReadBudget::default()).expect("new parses");
    let artifact = BundlePatchArtifact::from_views(&old_view, &new_view).expect("patch builds");
    encode_patch_bundle(&artifact).expect("patch encodes")
}

fn patch_operation_count(patch_bytes: &[u8]) -> usize {
    decode_patch_bundle(patch_bytes)
        .expect("patch decodes")
        .plan
        .operations
        .len()
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after UNIX epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "arcweft-windowed-ingress-{label}-{}-{nanos}",
        std::process::id()
    ))
}

fn fixture_bundle_with(display_text: &str) -> ArcweftBundle {
    let line = RuntimeLineId::from_runtime_line_value("line.opening").expect("runtime line id");
    let plan = RuntimePlan::new(
        vec![RuntimeFlow {
            id: FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow runtime id"),
            ops: vec![
                FlowOp::Dialogue {
                    line: line.clone(),
                    task_group: 0,
                },
                FlowOp::Return("done".to_owned()),
            ],
        }],
        vec![LineTaskGroup::default()],
    )
    .expect("runtime plan is valid")
    .with_entries(vec![arcweft_core::plan::RuntimeEntrySpec {
        id: arcweft_core::plan::EntryRuntimeId::from_source_entity_body("entry.main")
            .expect("test entry ID is valid"),
        kind: arcweft_core::plan::RuntimeEntryKind::Cli,
        binding: arcweft_core::entry::EntryBindingIdentity::from_bytes([1; 32]),
        target: arcweft_core::plan::RuntimeEntryTarget::Flow(
            FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow runtime id"),
        ),
        roles: arcweft_core::entry::RuntimeEntryRoles::None,
    }]);
    let source_map = source_map("windowed-ingress.arcw", "flow main { ... }");
    let dialogue_content =
        DialogueContentCatalog::try_from_records(vec![DialogueContentSpec::new(
            line,
            TextKey::try_new("text.opening").expect("text key"),
            RichTextDocument::new(vec![RichTextNode::Text {
                text: display_text.to_owned(),
            }]),
            character_support::character_plan(),
            arcweft_text_model::DialoguePresentationSnapshot::new(
                DialoguePresentationProfile::engine_default(),
                test_dialogue_profile_revision(),
            ),
            Vec::new(),
            source_map
                .primary_document()
                .expect("fixture source map retains its source")
                .product_source_ref(),
        )])
        .expect("final dialogue content catalog");
    let product_awbc = AwbcLowerer::new(&plan, &dialogue_content, "windowed-ingress.arcw")
        .lower()
        .expect("product AWBC lowers")
        .program;
    let bytecode = BytecodeProgram::from_runtime_plan(plan);
    let stats = bytecode.stats();
    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some("entry.main".to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                artifact_fingerprint: fixture_runtime_artifact_fingerprint(),
                entry_flow: Some("flow.main".to_owned()),
                flows: stats.flows,
                bytecode_instructions: stats.instructions,
                line_task_groups: stats.line_task_groups,
                stream_plans: stats.stream_plans,
                source_plans: stats.source_plans,
            },
        },
        source_map,
        bytecode,
        dialogue_content,
    )
    .expect("standard dialogue source joins source map")
    .with_character_presentation_catalog(character_support::character_catalog())
    .with_product_awbc(product_awbc)
}

fn source_map(label: &str, text: &str) -> SourceMapSection {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new(label).expect("source ID"),
        SourceName::path(label),
        text,
    )
    .expect("source document");
    SourceMapSection::try_from_documents(&[&document]).expect("source map")
}
