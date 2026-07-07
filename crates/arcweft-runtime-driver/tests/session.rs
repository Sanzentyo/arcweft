use arcweft_bundle::container::{
    ArtifactIdentity, BundleDigest, BundleKind, BundleView, ReadBudget, SectionId,
};
use arcweft_bundle::patch::{
    BundlePatchArtifact, BundlePatchManifest, BundlePatchPlan, PATCH_PLAN_SCHEMA_VERSION,
    PatchCompatibility, PatchMaterializationContract, RuntimeAbiRange, SectionChangeDerivation,
    SectionChangeOperation, SectionCompatibilityFingerprint, SectionOperation, encode_patch_bundle,
};
use arcweft_bundle::resource_codec::ui::{
    CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, UiInputKind,
    UiInputOptions, UiInputPurpose, UiInputResource, UiSecureInputPolicy, UiTextSelectionPolicy,
    UiTextShortcutPolicy, UiTextTabPolicy, UiTextVerticalNavigationPolicy,
};
use arcweft_bundle::{
    ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary, BundleSource,
};
use arcweft_core::awbc::schema::{
    AwbcEntry, AwbcEntryId, AwbcEntryKind, AwbcEntryTarget, AwbcProgram, AwbcRoute, AwbcStringId,
};
use arcweft_core::bytecode::{BYTECODE_ABI_VERSION, BytecodeProgram, BytecodeVerificationError};
use arcweft_core::line_task::LineTaskGroup;
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::plan::{
    ChoiceRuntimeOption, FlowEvent, FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeHostCallTarget,
    RuntimeLineId, RuntimePlan,
};
use arcweft_core::step::RuntimeHostCallMode;
use arcweft_core::task::{
    AwaitTarget, HostTaskArgTemplate, HostTaskRequestTemplate, NeedId, TaskId,
};
use arcweft_core::value::{RuntimeExpr, RuntimePayload, RuntimeValue};
use arcweft_id::PublicId;
use arcweft_presentation::input::{
    Action, ActionTarget, InteractionTarget as PresentationInteractionTarget,
};
use arcweft_presentation::text_input::{
    TextByteOffset, TextControlValue, TextControlWriteBack, TextInputSessionId, TextRange,
    TextRevision,
};
use arcweft_render_text::{LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode};
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{
    BundleEntryStart, BundleEntryStartError, BundleHotSwapError, BundlePatchReadiness,
    BundleSession, BundleSessionError, BundleSessionOptions, BundleStepInput,
};
use arcweft_runtime_driver::swap::SwapCompatibility;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::from_runtime_target_value(value).expect("test flow ID is valid")
}

fn line_id(value: &str) -> RuntimeLineId {
    RuntimeLineId::from_runtime_line_value(value).expect("test line ID is valid")
}

fn fixture_bundle() -> ArcweftBundle {
    fixture_bundle_with("WebGPU dialogue", false, false)
}

fn structured_vm_fixture_bundle() -> ArcweftBundle {
    fixture_bundle_from_parts("WebGPU dialogue", false, false, false)
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

fn test_patch_artifact(plan: BundlePatchPlan) -> BundlePatchArtifact {
    let compatibility_fingerprints = plan
        .operations
        .iter()
        .map(|operation| {
            let (id, operation_kind, compatibility, derivation) = match operation {
                SectionOperation::Add(descriptor) => (
                    descriptor.id(),
                    SectionChangeOperation::Add,
                    PatchCompatibility::ContentOnly,
                    SectionChangeDerivation::SectionKindDefault,
                ),
                SectionOperation::Replace { next, .. } => (
                    next.id(),
                    SectionChangeOperation::Replace,
                    PatchCompatibility::ContentOnly,
                    SectionChangeDerivation::SectionKindDefault,
                ),
                SectionOperation::Remove { id, .. } => (
                    *id,
                    SectionChangeOperation::Remove,
                    PatchCompatibility::RestartRequired,
                    SectionChangeDerivation::RemovalRequiresRestart,
                ),
            };
            SectionCompatibilityFingerprint {
                id,
                operation: operation_kind,
                raw_kind_code: 0,
                known_kind: None,
                required: true,
                compatibility,
                derivation,
                base_descriptor_fingerprint: None,
                target_descriptor_fingerprint: None,
                base_content_fingerprint: None,
                target_content_fingerprint: None,
            }
        })
        .collect::<Vec<_>>();
    let compatibility = compatibility_fingerprints
        .iter()
        .map(|fingerprint| fingerprint.compatibility)
        .fold(PatchCompatibility::ContentOnly, PatchCompatibility::max);
    BundlePatchArtifact {
        manifest: BundlePatchManifest {
            schema_version: PATCH_PLAN_SCHEMA_VERSION,
            min_reader_schema_version: PATCH_PLAN_SCHEMA_VERSION,
            runtime_abi: RuntimeAbiRange::CURRENT,
            base_artifact: ArtifactIdentity::for_current_container(
                BundleKind::Program,
                plan.base_content_root,
                BundleDigest::of(b"test-base-manifest"),
            ),
            target_artifact: ArtifactIdentity::for_current_container(
                BundleKind::Program,
                plan.target_content_root,
                BundleDigest::of(b"test-target-manifest"),
            ),
            base_content_root: plan.base_content_root,
            target_content_root: plan.target_content_root,
            compatibility,
            materialization: PatchMaterializationContract::default(),
            compatibility_fingerprints,
        },
        plan,
        target_manifest_bytes: None,
        changed_sections: Vec::new(),
    }
}

fn fixture_bundle_with(
    display_text: &str,
    extra_flow: bool,
    changed_main_code: bool,
) -> ArcweftBundle {
    fixture_bundle_from_parts(display_text, extra_flow, changed_main_code, true)
}

fn fixture_bundle_from_parts(
    display_text: &str,
    extra_flow: bool,
    changed_main_code: bool,
    include_product_awbc: bool,
) -> ArcweftBundle {
    let line = line_id("line.opening");
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
                    target: Some(flow_id("flow.done")),
                    out: None,
                    effects: Vec::new(),
                }],
            },
        ]
    };
    let mut flows = vec![
        RuntimeFlow {
            id: flow_id("flow.main"),
            ops: main_ops,
        },
        RuntimeFlow {
            id: flow_id("flow.done"),
            ops: vec![FlowOp::Return("done".to_owned())],
        },
    ];
    if extra_flow {
        flows.push(RuntimeFlow {
            id: flow_id("flow.extra"),
            ops: vec![FlowOp::Return("extra".to_owned())],
        });
    }
    let plan = RuntimePlan::new(
        Some(flow_id("flow.main")),
        flows,
        vec![LineTaskGroup::default()],
    )
    .expect("runtime plan is valid");
    let stats = BytecodeProgram::from_runtime_plan(plan.clone()).stats();
    let display = LineDisplayCatalog::new(vec![LineDisplaySpec {
        line,
        callee: "alice".to_owned(),
        speaker_label: None,
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
    }]);
    let bundle = ArcweftBundle::new(
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
        BytecodeProgram::from_runtime_plan(plan.clone()),
        display.clone(),
    );
    if include_product_awbc {
        let product_awbc = AwbcLowerer::new(&plan, &display, "web-demo.arcw")
            .lower()
            .expect("product AWBC lowers")
            .program;
        bundle.with_product_awbc(product_awbc)
    } else {
        bundle
    }
}

fn fixture_await_bundle(extra_flow: bool) -> ArcweftBundle {
    let mut flows = vec![RuntimeFlow {
        id: flow_id("flow.main"),
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
            id: flow_id("flow.extra"),
            ops: vec![FlowOp::Return("extra".to_owned())],
        });
    }
    let plan = RuntimePlan::new(Some(flow_id("flow.main")), flows, Vec::new())
        .expect("runtime plan is valid");
    let stats = BytecodeProgram::from_runtime_plan(plan.clone()).stats();
    let display = LineDisplayCatalog::new(Vec::new());
    let product_awbc = AwbcLowerer::new(&plan, &display, "await-demo.arcw")
        .lower()
        .expect("product AWBC lowers")
        .program;
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
        display,
    )
    .with_product_awbc(product_awbc)
}

fn fixture_action_receive_bundle() -> ArcweftBundle {
    let plan = RuntimePlan::new(
        Some(flow_id("flow.main")),
        vec![RuntimeFlow {
            id: flow_id("flow.main"),
            ops: vec![
                FlowOp::HostCall {
                    binding: Some(RuntimePattern::Ident("event".to_owned())),
                    target: RuntimeHostCallTarget::new(
                        "ui.action.await",
                        "ui.action",
                        "await",
                        [RuntimeExpr::EntityRef("action.feedback.submit".to_owned())],
                        RuntimeHostCallMode::Suspend,
                        true,
                    ),
                },
                FlowOp::ReturnExpr(RuntimeExpr::Field {
                    target: Box::new(RuntimeExpr::Local("event".to_owned())),
                    field: "value".to_owned(),
                }),
            ],
        }],
        vec![LineTaskGroup::default()],
    )
    .expect("runtime plan is valid");
    let stats = BytecodeProgram::from_runtime_plan(plan.clone()).stats();
    let display = LineDisplayCatalog::default();
    let bundle = ArcweftBundle::new(
        BundleManifest {
            source_label: "action-receive.arcw".to_owned(),
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
            label: "action-receive.arcw".to_owned(),
            text: String::new(),
        },
        BytecodeProgram::from_runtime_plan(plan.clone()),
        display.clone(),
    );
    let product_awbc = AwbcLowerer::new(&plan, &display, "action-receive.arcw")
        .lower()
        .expect("product AWBC lowers")
        .program;
    bundle.with_product_awbc(product_awbc)
}

fn fixture_action_receive_bundle_with_submit_input() -> ArcweftBundle {
    fixture_action_receive_bundle().with_ui_input(UiInputResource {
        options: vec![UiInputOptions {
            public_id: "input.feedback".to_owned(),
            view: Some("view.FeedbackForm".to_owned()),
            containing_scroll_region: None,
            kind: UiInputKind::TextField,
            value_text_source: "text.value.input.feedback".to_owned(),
            placeholder_text_source: None,
            purpose: UiInputPurpose::Text,
            autocorrect: TextAssistPolicy::PlatformDefault,
            spellcheck: TextAssistPolicy::PlatformDefault,
            capitalization: TextCapitalization::None,
            enter_key: EnterKeyHint::Send,
            multiline: false,
            selection_policy: UiTextSelectionPolicy::Enabled,
            shortcut_policy: UiTextShortcutPolicy::Enabled,
            tab_policy: UiTextTabPolicy::FocusNavigation,
            vertical_navigation_policy: UiTextVerticalNavigationPolicy::LogicalLine,
            secure_policy: UiSecureInputPolicy::Plain,
            composition_on_blur: CompositionOnBlurPolicy::Commit,
            submit_handler: Some("action.feedback.submit".to_owned()),
            change_handler: Some("input.feedback".to_owned()),
            adapter_requirements: Vec::new(),
        }],
        adapter_requirements: Vec::new(),
    })
}

fn fixture_await_replacement_bundle() -> ArcweftBundle {
    let plan = RuntimePlan::new(
        Some(flow_id("flow.main")),
        vec![RuntimeFlow {
            id: flow_id("flow.main"),
            ops: vec![FlowOp::Return("changed".to_owned())],
        }],
        Vec::new(),
    )
    .expect("runtime plan is valid");
    let stats = BytecodeProgram::from_runtime_plan(plan.clone()).stats();
    let display = LineDisplayCatalog::default();
    let product_awbc = AwbcLowerer::new(&plan, &display, "await-replacement.arcw")
        .lower()
        .expect("product AWBC lowers")
        .program;
    ArcweftBundle::new(
        BundleManifest {
            source_label: "await-replacement.arcw".to_owned(),
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
            label: "await-replacement.arcw".to_owned(),
            text: String::new(),
        },
        BytecodeProgram::from_runtime_plan(plan),
        display,
    )
    .with_product_awbc(product_awbc)
}

fn bundle_with_route_entry() -> ArcweftBundle {
    let mut bundle = fixture_bundle();
    let program = &mut bundle.product_awbc.as_mut().expect("product AWBC").program;
    let [method, path, public_id] = add_awbc_strings(program, ["GET", "/route", "entry.route"]);
    let first = program.entries.first().expect("default entry exists");
    let function = first
        .target
        .function()
        .expect("default entry targets a function");
    let signature = first.signature;
    program.entries.push(AwbcEntry {
        public_id,
        kind: AwbcEntryKind::Server,
        signature,
        target: AwbcEntryTarget::Routes(vec![AwbcRoute {
            method,
            path,
            target: function,
            bindings: Vec::new(),
        }]),
    });
    bundle
}

fn add_awbc_strings<const N: usize>(
    program: &mut AwbcProgram,
    values: [&str; N],
) -> [AwbcStringId; N] {
    program
        .strings
        .extend(values.iter().map(|value| (*value).to_owned()));
    program.canonicalize_string_table();
    std::array::from_fn(|index| {
        let table_index = program
            .strings
            .binary_search(&values[index].to_owned())
            .expect("inserted string is present");
        AwbcStringId(u32::try_from(table_index).expect("string index fits in u32"))
    })
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

    session.queue_dialogue_advance();
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
fn session_accepts_generic_semantic_action_invoke() {
    let bundle = fixture_bundle();
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
    let action = Action::new(
        ActionTarget::Runtime,
        PublicId::try_new("action.feedback.submit_name").expect("action id"),
    )
    .with_payload("literal payload");

    session
        .queue_semantic_action(&action)
        .expect("generic semantic action is accepted");
}

#[test]
fn product_awbc_session_flow_option_selects_flow_function() {
    let bundle = fixture_bundle_with("WebGPU dialogue", true, false);

    for selector in ["extra", "flow.extra"] {
        let mut session = BundleSession::new(
            &bundle,
            BundleSessionOptions {
                flow: Some(selector.to_owned()),
                ..BundleSessionOptions::default()
            },
        )
        .expect("flow selector starts Product AWBC session");

        let mut step = session.step_with_clock(
            RuntimeClockStep::from_millis(1, 16).expect("clock"),
            BundleStepInput::default(),
        );
        for tick in 2..=4 {
            if step.finished {
                break;
            }
            step = session.step_with_clock(
                RuntimeClockStep::from_millis(tick, 16).expect("clock"),
                BundleStepInput::default(),
            );
        }

        assert!(
            step.finished,
            "{selector} should return, stopped at {:?} with {}; diagnostics: {}",
            step.stop_reason,
            step.status_label,
            step.diagnostics.join("; ")
        );
        assert!(
            step.status_label.contains("extra"),
            "{selector} should run flow.extra, got {}",
            step.status_label
        );
    }
}

#[test]
fn product_awbc_session_flow_option_reports_unknown_flow() {
    let bundle = fixture_bundle_with("WebGPU dialogue", true, false);

    let error = BundleSession::new(
        &bundle,
        BundleSessionOptions {
            flow: Some("missing".to_owned()),
            ..BundleSessionOptions::default()
        },
    )
    .expect_err("unknown flow rejects");

    assert_eq!(
        error,
        BundleSessionError::UnknownFlow {
            flow: "flow.missing".to_owned()
        }
    );
}

#[test]
fn session_receive_action_host_call_resumes_with_event_value() {
    let bundle = fixture_action_receive_bundle();
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
    let waiting = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert!(!waiting.finished);

    let action = Action::new(
        ActionTarget::Runtime,
        PublicId::try_new("action.feedback.submit").expect("action id"),
    )
    .with_payload("Ada");
    session
        .queue_semantic_action(&action)
        .expect("generic semantic action is accepted");
    let resumed = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput::default(),
    );

    assert!(resumed.finished);
    assert!(resumed.flow_events.iter().any(|event| {
        matches!(
            event,
            FlowEvent::Return { value } if value == "Ada"
        )
    }));
}

#[test]
fn session_text_control_submit_handler_resumes_receive_action() {
    let bundle = fixture_action_receive_bundle_with_submit_input();
    let submit_session = bundle
        .ui_input
        .as_ref()
        .expect("fixture has input resource")
        .options[0]
        .runtime_text_session();
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
    let waiting = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert!(!waiting.finished);

    let submit = TextControlWriteBack::submit(
        PresentationInteractionTarget::new(
            PublicId::try_new("input.feedback").expect("valid target"),
        ),
        TextInputSessionId(submit_session),
        TextControlValue::plain("Ada"),
        TextRange::new(TextByteOffset(3), TextByteOffset(3)),
        TextRevision(1),
    );
    session
        .queue_text_control_write_back(&submit)
        .expect("submit writeback is accepted");
    let resumed = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput::default(),
    );

    assert!(resumed.finished);
    assert!(resumed.flow_events.iter().any(|event| {
        matches!(
            event,
            FlowEvent::Return { value } if value == "Ada"
        )
    }));
}

#[test]
fn session_rejects_unverified_bytecode_before_construction() {
    let mut bundle = structured_vm_fixture_bundle();
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
    let mut bundle = structured_vm_fixture_bundle();
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
    session.queue_dialogue_advance();
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

    session.queue_dialogue_advance();
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
    assert_eq!(
        task.generation,
        session
            .current_fiber_generation()
            .expect("fiber generation")
    );

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
fn hot_swap_changed_structured_flow_keeps_current_fiber_on_old_generation() {
    let old_bundle = fixture_bundle();
    let new_bundle = fixture_bundle_with("WebGPU dialogue", false, true);
    let mut session =
        BundleSession::new(&old_bundle, BundleSessionOptions::default()).expect("session starts");
    let old_generation = session.active_generation().id;

    let report = session
        .hot_swap_bundle(&new_bundle)
        .expect("code-generational swap applies with mixed generation support");

    assert_eq!(report.compatibility, SwapCompatibility::CodeGenerational);
    assert_ne!(session.active_generation().id, old_generation);
    assert_eq!(session.current_fiber_generation(), Some(old_generation));
    assert_eq!(session.retired_generation_count(), 1);
}

#[test]
fn entry_generation_start_binds_to_new_generation_after_code_generational_commit() {
    let old_bundle = fixture_bundle();
    let new_bundle = fixture_bundle_with("WebGPU dialogue", false, true);
    let mut session =
        BundleSession::new(&old_bundle, BundleSessionOptions::default()).expect("session starts");

    let report = session
        .hot_swap_bundle(&new_bundle)
        .expect("code-generational swap applies");
    let new_generation = report.generation;

    let started = session
        .start_foreground_entry_on_current_generation(BundleEntryStart::session_default())
        .expect("new entry binds to committed active generation");

    assert_eq!(started.generation, new_generation);
    assert_eq!(started.entry, AwbcEntryId(0));
    assert_eq!(session.current_fiber_generation(), Some(new_generation));

    let step = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );

    assert!(step.finished);
    assert!(step.status_label.contains("changed"));
}

#[test]
fn entry_generation_start_prunes_replaced_old_fiber_runtime_image() {
    let old_bundle = fixture_bundle();
    let new_bundle = fixture_bundle_with("WebGPU dialogue", false, true);
    let mut session =
        BundleSession::new(&old_bundle, BundleSessionOptions::default()).expect("session starts");
    let old_generation = session.active_generation().id;

    let report = session
        .hot_swap_bundle(&new_bundle)
        .expect("code-generational swap applies");

    assert_eq!(report.compatibility, SwapCompatibility::CodeGenerational);
    assert_eq!(session.current_fiber_generation(), Some(old_generation));
    assert!(session.has_runtime_image(old_generation));
    assert!(session.has_runtime_image(report.generation));

    let started = session
        .start_foreground_entry_on_current_generation(BundleEntryStart::session_default())
        .expect("foreground entry starts on active generation");

    assert_eq!(started.generation, report.generation);
    assert_eq!(session.current_fiber_generation(), Some(report.generation));
    assert!(!session.has_runtime_image(old_generation));
    assert_eq!(session.retired_generation_count(), 0);
}

#[test]
fn entry_generation_start_keeps_old_runtime_image_until_task_pin_releases() {
    let old_bundle = fixture_await_bundle(false);
    let new_bundle = fixture_await_replacement_bundle();
    let mut session =
        BundleSession::new(&old_bundle, BundleSessionOptions::default()).expect("session starts");
    let old_generation = session.active_generation().id;
    let waiting = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    let task = waiting.requested_tasks[0].clone();
    let task_sequence = task.sequence;

    let report = session
        .hot_swap_bundle(&new_bundle)
        .expect("code-generational swap applies");
    assert_eq!(report.compatibility, SwapCompatibility::CodeGenerational);

    session
        .start_foreground_entry_on_current_generation(BundleEntryStart::session_default())
        .expect("new foreground entry starts");

    assert_eq!(session.task_generation(task_sequence), Some(old_generation));
    assert!(session.has_runtime_image(old_generation));
    assert!(session.has_runtime_image(report.generation));

    let _completion = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput {
            task_events: vec![task.ready(RuntimePayload::new(RuntimeValue::String(
                "bg_handle".to_owned(),
            )))],
            ..BundleStepInput::default()
        },
    );

    assert_eq!(session.task_generation(task_sequence), None);
    assert!(!session.has_runtime_image(old_generation));
    assert_eq!(session.retired_generation_count(), 0);
}

#[test]
fn entry_generation_start_reports_invalid_entry_selection_deterministically() {
    let bundle = fixture_bundle();
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");

    let error = session
        .start_foreground_entry_on_current_generation(BundleEntryStart::entry(AwbcEntryId(777)))
        .expect_err("missing typed entry rejects");

    assert_eq!(
        error,
        BundleEntryStartError::UnknownEntry {
            entry: AwbcEntryId(777)
        }
    );
}

#[test]
fn entry_generation_start_reports_non_flow_entry_selection_deterministically() {
    let bundle = bundle_with_route_entry();
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");

    let error = session
        .start_foreground_entry_on_current_generation(BundleEntryStart::entry(AwbcEntryId(1)))
        .expect_err("route entry is not a foreground flow");

    assert_eq!(
        error,
        BundleEntryStartError::NonFlowEntry {
            entry: AwbcEntryId(1)
        }
    );
}

#[test]
fn patch_readiness_accepts_noop_patch_for_active_generation() {
    let bundle = fixture_bundle();
    let bytes = awfb_bytes(&bundle);
    let session = BundleSession::from_awfb_bytes(&bytes, BundleSessionOptions::default())
        .expect("session starts");
    let base = awfb_root(&bytes);
    let artifact = test_patch_artifact(BundlePatchPlan {
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
    let artifact = test_patch_artifact(BundlePatchPlan {
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
    let artifact = test_patch_artifact(BundlePatchPlan {
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
    let artifact = test_patch_artifact(BundlePatchPlan {
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
    let artifact = test_patch_artifact(BundlePatchPlan {
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
    let artifact = test_patch_artifact(BundlePatchPlan {
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
