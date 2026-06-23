use arcweft_bundle::container::{BundleDigest, BundleView, ReadBudget, SectionId};
use arcweft_bundle::patch::{
    BundlePatchArtifact, BundlePatchPlan, PatchCompatibility, SectionOperation, encode_patch_bundle,
};
use arcweft_bundle::{
    ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary, BundleSource,
};
use arcweft_core::bytecode::{BYTECODE_ABI_VERSION, BytecodeProgram, BytecodeVerificationError};
use arcweft_core::line_task::LineTaskGroup;
use arcweft_core::plan::{
    ChoiceRuntimeOption, FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeLineId, RuntimePlan,
};
use arcweft_core::task::{
    AwaitTarget, HostTaskArgTemplate, HostTaskRequestTemplate, NeedId, TaskId,
};
use arcweft_core::value::{RuntimeExpr, RuntimePayload, RuntimeValue};
use arcweft_id::PublicId;
use arcweft_presentation::input::{Action, ActionTarget};
use arcweft_render_text::{LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode};
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{
    BundleHotSwapError, BundlePatchReadiness, BundleSession, BundleSessionError,
    BundleSessionOptions, BundleStepInput,
};
use arcweft_runtime_driver::swap::SwapCompatibility;

fn fixture_bundle() -> ArcweftBundle {
    fixture_bundle_with("WebGPU dialogue", false, false)
}

fn awfb_bytes(bundle: &ArcweftBundle) -> Vec<u8> {
    bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("fixture encodes as AWFB")
}

fn awfb_root(bytes: &[u8]) -> BundleDigest {
    BundleView::parse(bytes, ReadBudget::default())
        .expect("fixture AWFB parses")
        .content_root()
}

fn fixture_bundle_with(
    display_text: &str,
    extra_flow: bool,
    changed_main_code: bool,
) -> ArcweftBundle {
    let line = RuntimeLineId("line.opening".to_owned());
    let main_ops = if changed_main_code {
        vec![FlowOp::Return("changed".to_owned())]
    } else {
        vec![
            FlowOp::Dialogue {
                line: line.clone(),
                task_group: 0,
            },
            FlowOp::Choice {
                id: Some("choice.opening".to_owned()),
                options: vec![ChoiceRuntimeOption {
                    id: Some("choice.opening.next".to_owned()),
                    label: "Next".to_owned(),
                    target: Some(FlowRuntimeId("flow.done".to_owned())),
                    out: None,
                    effects: Vec::new(),
                }],
            },
        ]
    };
    let mut flows = vec![
        RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: main_ops,
        },
        RuntimeFlow {
            id: FlowRuntimeId("flow.done".to_owned()),
            ops: vec![FlowOp::Return("done".to_owned())],
        },
    ];
    if extra_flow {
        flows.push(RuntimeFlow {
            id: FlowRuntimeId("flow.extra".to_owned()),
            ops: vec![FlowOp::Return("extra".to_owned())],
        });
    }
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        flows,
        vec![LineTaskGroup::default()],
    )
    .expect("runtime plan is valid");
    let stats = BytecodeProgram::from_runtime_plan(plan.clone()).stats();
    ArcweftBundle::new(
        BundleManifest {
            source_label: "web-demo.arcw".to_owned(),
            profile_id: None,
            profile_kind: None,
            entry: None,
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                entry_flow: Some("flow.main".to_owned()),
                flows: stats.flows,
                bytecode_instructions: stats.instructions,
                line_task_groups: stats.line_task_groups,
                stream_plans: 0,
                source_plans: 0,
            },
        },
        BundleSource {
            label: "web-demo.arcw".to_owned(),
            text: String::new(),
        },
        BytecodeProgram::from_runtime_plan(plan),
        LineDisplayCatalog::new(vec![LineDisplaySpec {
            line,
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![RichTextNode::Text {
                text: display_text.to_owned(),
            }]),
        }]),
    )
}

fn fixture_await_bundle(extra_flow: bool) -> ArcweftBundle {
    let mut flows = vec![RuntimeFlow {
        id: FlowRuntimeId("flow.main".to_owned()),
        ops: vec![
            FlowOp::Await {
                binding: None,
                target: AwaitTarget {
                    need: NeedId("need.bg".to_owned()),
                    task: TaskId("task.bg".to_owned()),
                    request: HostTaskRequestTemplate::new(
                        "asset",
                        "image",
                        [HostTaskArgTemplate::positional(RuntimeExpr::Value(
                            RuntimeValue::String("asset.bg.room".to_owned()),
                        ))],
                    ),
                },
                pending: Vec::new(),
            },
            FlowOp::Return("ready".to_owned()),
        ],
    }];
    if extra_flow {
        flows.push(RuntimeFlow {
            id: FlowRuntimeId("flow.extra".to_owned()),
            ops: vec![FlowOp::Return("extra".to_owned())],
        });
    }
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        flows,
        Vec::new(),
    )
    .expect("runtime plan is valid");
    let stats = BytecodeProgram::from_runtime_plan(plan.clone()).stats();
    ArcweftBundle::new(
        BundleManifest {
            source_label: "await-demo.arcw".to_owned(),
            profile_id: None,
            profile_kind: None,
            entry: None,
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                entry_flow: Some("flow.main".to_owned()),
                flows: stats.flows,
                bytecode_instructions: stats.instructions,
                line_task_groups: stats.line_task_groups,
                stream_plans: 0,
                source_plans: 0,
            },
        },
        BundleSource {
            label: "await-demo.arcw".to_owned(),
            text: String::new(),
        },
        BytecodeProgram::from_runtime_plan(plan),
        LineDisplayCatalog::default(),
    )
}

#[test]
fn session_requires_explicit_clock_and_exposes_presentation() {
    let bundle = fixture_bundle();
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
    let first = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(
        first
            .presentation
            .dialogue
            .as_ref()
            .map(|frame| frame.text.as_str()),
        Some("WebGPU dialogue")
    );

    let choice_step = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(choice_step.presentation.choices.len(), 1);

    let action = Action::new(
        ActionTarget::Runtime,
        PublicId::try_new("action.choice.select").expect("action id"),
    )
    .with_payload("choice.opening.next");
    session
        .queue_semantic_action(&action)
        .expect("choice action is accepted");
    let selected = session.step_with_clock(
        RuntimeClockStep::from_millis(3, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert!(!selected.finished);
    assert!(selected.presentation.choices.is_empty());

    let second = session.step_with_clock(
        RuntimeClockStep::from_millis(4, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert!(second.finished);
    assert!(second.presentation.choices.is_empty());
}

#[test]
fn session_rejects_unverified_bytecode_before_construction() {
    let mut bundle = fixture_bundle();
    bundle.bytecode.program.abi_version = BYTECODE_ABI_VERSION + 1;

    let error = BundleSession::new(&bundle, BundleSessionOptions::default())
        .expect_err("invalid bytecode is rejected before session construction");

    assert!(matches!(
        error,
        BundleSessionError::VerifyBytecode(BytecodeVerificationError::UnsupportedAbi {
            actual,
            expected,
        }) if actual == BYTECODE_ABI_VERSION + 1 && expected == BYTECODE_ABI_VERSION
    ));
}

#[test]
fn session_rejects_missing_bytecode_entrypoint_before_construction() {
    let mut bundle = fixture_bundle();
    bundle.bytecode.program.entry_flow = None;

    let error = BundleSession::new(&bundle, BundleSessionOptions::default())
        .expect_err("bytecode without an entrypoint is rejected before session construction");

    assert!(matches!(
        error,
        BundleSessionError::VerifyBytecode(BytecodeVerificationError::MissingEntrypoint)
    ));
}

#[test]
fn hot_swap_content_only_updates_future_presentation_without_rebuilding_code() {
    let old_bundle = fixture_bundle_with("Old text", false, false);
    let new_bundle = fixture_bundle_with("New text", false, false);
    let mut session =
        BundleSession::new(&old_bundle, BundleSessionOptions::default()).expect("session starts");
    let old_generation = session.active_generation().id;

    let report = session
        .hot_swap_bundle(&new_bundle)
        .expect("content-only swap applies");

    assert_eq!(report.compatibility, SwapCompatibility::ContentOnly);
    assert_ne!(session.active_generation().id, old_generation);
    let step = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(
        step.presentation
            .dialogue
            .as_ref()
            .map(|frame| frame.text.as_str()),
        Some("New text")
    );
}

#[test]
fn generation_pin_retains_old_bundle_generation_until_handle_drops() {
    let old_bundle = fixture_bundle_with("Old text", false, false);
    let new_bundle = fixture_bundle_with("New text", false, false);
    let mut session =
        BundleSession::new(&old_bundle, BundleSessionOptions::default()).expect("session starts");
    let old_generation = session.active_generation().id;
    let pin = session.pin_active_generation();

    let report = session
        .hot_swap_bundle(&new_bundle)
        .expect("content-only swap applies");

    assert_eq!(report.compatibility, SwapCompatibility::ContentOnly);
    assert_ne!(session.active_generation().id, old_generation);
    assert_eq!(pin.id, old_generation);
    assert_eq!(session.retired_generation_count(), 1);

    drop(pin);
    session.retire_unused_generations();

    assert_eq!(session.retired_generation_count(), 1);

    let _dialogue = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    let _choice = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput::default(),
    );
    session.queue_choice_selection("choice.opening.next");
    let _selected = session.step_with_clock(
        RuntimeClockStep::from_millis(3, 16).expect("clock"),
        BundleStepInput::default(),
    );
    let finished = session.step_with_clock(
        RuntimeClockStep::from_millis(4, 16).expect("clock"),
        BundleStepInput::default(),
    );

    assert!(finished.finished);
    assert_eq!(session.retired_generation_count(), 0);
}

#[test]
fn active_fiber_pin_retains_old_generation_until_fiber_finishes() {
    let old_bundle = fixture_bundle_with("Old text", false, false);
    let new_bundle = fixture_bundle_with("New text", false, false);
    let mut session =
        BundleSession::new(&old_bundle, BundleSessionOptions::default()).expect("session starts");
    let old_generation = session.active_generation().id;

    let report = session
        .hot_swap_bundle(&new_bundle)
        .expect("content-only swap applies");

    assert_eq!(report.compatibility, SwapCompatibility::ContentOnly);
    assert_ne!(session.active_generation().id, old_generation);
    assert_eq!(session.retired_generation_count(), 1);

    let blocked = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert!(!blocked.finished);
    assert_eq!(session.retired_generation_count(), 1);

    let choice = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert!(!choice.finished);
    assert_eq!(session.retired_generation_count(), 1);

    session.queue_choice_selection("choice.opening.next");
    let mut finished = session.step_with_clock(
        RuntimeClockStep::from_millis(3, 16).expect("clock"),
        BundleStepInput::default(),
    );
    for tick in 3..=5 {
        if finished.finished {
            break;
        }
        finished = session.step_with_clock(
            RuntimeClockStep::from_millis(tick + 1, 16).expect("clock"),
            BundleStepInput::default(),
        );
    }

    assert!(finished.finished);
    assert_eq!(session.retired_generation_count(), 0);
}

#[test]
fn pending_task_pin_survives_code_compatible_runtime_rebuild() {
    let old_bundle = fixture_await_bundle(false);
    let new_bundle = fixture_await_bundle(true);
    let mut session =
        BundleSession::new(&old_bundle, BundleSessionOptions::default()).expect("session starts");

    let waiting = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(waiting.requested_tasks.len(), 1);
    let task = waiting.requested_tasks[0].clone();

    let report = session
        .hot_swap_bundle(&new_bundle)
        .expect("code-compatible swap applies");

    assert_eq!(report.compatibility, SwapCompatibility::CodeCompatible);
    assert_eq!(session.retired_generation_count(), 1);

    let _ignored_completion = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput {
            task_events: vec![task.ready(RuntimePayload::new(RuntimeValue::String(
                "bg_handle".to_owned(),
            )))],
            ..BundleStepInput::default()
        },
    );
    session.retire_unused_generations();

    assert_eq!(session.retired_generation_count(), 0);
}

#[test]
fn hot_swap_code_compatible_bundle_replaces_runtime_at_quiescent_boundary() {
    let old_bundle = fixture_bundle();
    let new_bundle = fixture_bundle_with("WebGPU dialogue", true, false);
    let mut session =
        BundleSession::new(&old_bundle, BundleSessionOptions::default()).expect("session starts");

    let report = session
        .hot_swap_bundle(&new_bundle)
        .expect("code-compatible swap applies");

    assert_eq!(report.compatibility, SwapCompatibility::CodeCompatible);
    let step = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(
        step.presentation
            .dialogue
            .as_ref()
            .map(|frame| frame.text.as_str()),
        Some("WebGPU dialogue")
    );
}

#[test]
fn hot_swap_changed_structured_flow_requires_host_restart() {
    let old_bundle = fixture_bundle();
    let new_bundle = fixture_bundle_with("WebGPU dialogue", false, true);
    let mut session =
        BundleSession::new(&old_bundle, BundleSessionOptions::default()).expect("session starts");

    let error = session
        .hot_swap_bundle(&new_bundle)
        .expect_err("changed structured flow cannot be applied live yet");

    assert!(matches!(
        error,
        BundleHotSwapError::RestartRequired {
            compatibility: SwapCompatibility::CodeGenerational,
        }
    ));
}

#[test]
fn patch_readiness_accepts_noop_patch_for_active_generation() {
    let bundle = fixture_bundle();
    let bytes = awfb_bytes(&bundle);
    let session = BundleSession::from_awfb_bytes(&bytes, BundleSessionOptions::default())
        .expect("session starts");
    let base = awfb_root(&bytes);
    let artifact = BundlePatchArtifact::new(BundlePatchPlan {
        base_content_root: base,
        target_content_root: base,
        operations: Vec::new(),
    });

    let report = session
        .inspect_hot_swap_patch_artifact(&artifact)
        .expect("noop patch is ready");

    assert_eq!(report.base_generation, session.active_generation().id);
    assert_eq!(report.compatibility, PatchCompatibility::ContentOnly);
    assert_eq!(report.readiness, BundlePatchReadiness::Noop);
}

#[test]
fn patch_readiness_reports_target_bundle_required_for_section_operations() {
    let bundle = fixture_bundle();
    let bytes = awfb_bytes(&bundle);
    let session = BundleSession::from_awfb_bytes(&bytes, BundleSessionOptions::default())
        .expect("session starts");
    let base = awfb_root(&bytes);
    let artifact = BundlePatchArtifact::new(BundlePatchPlan {
        base_content_root: base,
        target_content_root: BundleDigest::of(b"target"),
        operations: vec![SectionOperation::Remove {
            id: SectionId::from_bytes([7; 16]),
            old: BundleDigest::of(b"old"),
        }],
    });

    let report = session
        .inspect_hot_swap_patch_artifact(&artifact)
        .expect("patch validates against active base");

    assert_eq!(
        report.readiness,
        BundlePatchReadiness::TargetBundleRequired { operations: 1 }
    );
    assert_eq!(report.compatibility, PatchCompatibility::RestartRequired);
}

#[test]
fn patch_readiness_decodes_awfb_patch_bytes() {
    let bundle = fixture_bundle();
    let bundle_bytes = awfb_bytes(&bundle);
    let session = BundleSession::from_awfb_bytes(&bundle_bytes, BundleSessionOptions::default())
        .expect("session starts");
    let base = awfb_root(&bundle_bytes);
    let artifact = BundlePatchArtifact::new(BundlePatchPlan {
        base_content_root: base,
        target_content_root: base,
        operations: Vec::new(),
    });
    let bytes = encode_patch_bundle(&artifact).expect("patch bundle encodes");

    let report = session
        .inspect_hot_swap_patch_bytes(&bytes)
        .expect("patch bundle decodes");

    assert_eq!(report.readiness, BundlePatchReadiness::Noop);
    assert_eq!(report.compatibility, PatchCompatibility::ContentOnly);
}

#[test]
fn patch_readiness_rejects_wrong_active_base() {
    let bundle = fixture_bundle();
    let bytes = awfb_bytes(&bundle);
    let session = BundleSession::from_awfb_bytes(&bytes, BundleSessionOptions::default())
        .expect("session starts");
    let artifact = BundlePatchArtifact::new(BundlePatchPlan {
        base_content_root: BundleDigest::of(b"other-base"),
        target_content_root: BundleDigest::of(b"target"),
        operations: Vec::new(),
    });

    let error = session
        .inspect_hot_swap_patch_artifact(&artifact)
        .expect_err("wrong base rejects");

    assert!(matches!(error, BundleHotSwapError::WrongPatchBase(_)));
}

#[test]
fn patch_readiness_requires_awfb_backed_session() {
    let bundle = fixture_bundle();
    let session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
    let artifact = BundlePatchArtifact::new(BundlePatchPlan {
        base_content_root: BundleDigest::of(b"base"),
        target_content_root: BundleDigest::of(b"target"),
        operations: Vec::new(),
    });

    let error = session
        .inspect_hot_swap_patch_artifact(&artifact)
        .expect_err("decoded-only session has no AWFB base root");

    assert!(matches!(
        error,
        BundleHotSwapError::MissingActiveContainerRoot
    ));
}

#[test]
fn hot_swap_patch_bytes_reports_restart_required_before_materializing_target() {
    let bundle = fixture_bundle();
    let bundle_bytes = awfb_bytes(&bundle);
    let base = awfb_root(&bundle_bytes);
    let artifact = BundlePatchArtifact::new(BundlePatchPlan {
        base_content_root: base,
        target_content_root: BundleDigest::of(b"target"),
        operations: vec![SectionOperation::Remove {
            id: SectionId::from_bytes([9; 16]),
            old: BundleDigest::of(b"old"),
        }],
    });
    let patch_bytes = encode_patch_bundle(&artifact).expect("patch bundle encodes");
    let mut session =
        BundleSession::from_awfb_bytes(&bundle_bytes, BundleSessionOptions::default())
            .expect("session starts");

    let error = session
        .hot_swap_patch_bytes(b"not an AWFB base", &patch_bytes)
        .expect_err("restart-required patch rejects before materialization");

    assert!(matches!(
        error,
        BundleHotSwapError::RestartRequired {
            compatibility: SwapCompatibility::RestartRequired,
        }
    ));
    assert_eq!(session.active_container_content_root(), Some(base));
}

#[test]
fn hot_swap_patch_bytes_materializes_target_and_applies_content_only_swap() {
    let old_bundle = fixture_bundle_with("Old text", false, false);
    let new_bundle = fixture_bundle_with("New text", false, false);
    let old_bytes = awfb_bytes(&old_bundle);
    let new_bytes = awfb_bytes(&new_bundle);
    let old_view = BundleView::parse(&old_bytes, ReadBudget::default()).expect("old AWFB parses");
    let new_view = BundleView::parse(&new_bytes, ReadBudget::default()).expect("new AWFB parses");
    let patch = BundlePatchArtifact::from_views(&old_view, &new_view).expect("patch artifact");
    let patch_bytes = encode_patch_bundle(&patch).expect("patch encodes");
    let mut session = BundleSession::from_awfb_bytes(&old_bytes, BundleSessionOptions::default())
        .expect("session starts");

    let report = session
        .hot_swap_patch_bytes(&old_bytes, &patch_bytes)
        .expect("patch applies");

    assert_eq!(report.compatibility, SwapCompatibility::ContentOnly);
    assert_eq!(
        session.active_container_content_root(),
        Some(new_view.content_root())
    );
    let step = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(
        step.presentation
            .dialogue
            .as_ref()
            .map(|frame| frame.text.as_str()),
        Some("New text")
    );
}
