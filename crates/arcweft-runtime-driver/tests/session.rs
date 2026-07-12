use arcweft_bundle::container::{
    ArtifactIdentity, BundleDigest, BundleKind, BundleView, ReadBudget, SectionId,
};
use arcweft_bundle::patch::{
    BundlePatchArtifact, BundlePatchManifest, BundlePatchPlan, PATCH_PLAN_SCHEMA_VERSION,
    PatchBundleError, PatchCompatibility, PatchMaterializationContract, RuntimeAbiRange,
    SectionChangeDerivation, SectionChangeOperation, SectionCompatibilityFingerprint,
    SectionOperation, encode_patch_bundle,
};
use arcweft_bundle::resource_codec::view::{
    CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization,
    ViewDefinitionResource, ViewElementKind, ViewInputKind, ViewInputOptions, ViewInputPurpose,
    ViewInputResource, ViewInstructionSpan, ViewLogicalRect, ViewParameterResource,
    ViewProgramInstruction, ViewProgramResource, ViewScrollAxis, ViewScrollRegionResource,
    ViewSecureInputPolicy, ViewTextSelectionPolicy, ViewTextShortcutPolicy, ViewTextSourceKind,
    ViewTextSourceRecord, ViewTextTabPolicy, ViewTextVerticalNavigationPolicy,
};
use arcweft_bundle::resource_codec::{
    ViewTextBlockBounds, ViewTextBlockResource, ViewTextResource, ViewValueInputNamespace,
    ViewValueInputResource, ViewValueInputSource,
};
use arcweft_bundle::{
    ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary, BundleSource,
};
use arcweft_core::awbc::schema::{
    AwbcEntry, AwbcEntryId, AwbcEntryKind, AwbcEntryTarget, AwbcProgram, AwbcRoute, AwbcStringId,
};
use arcweft_core::bytecode::{BYTECODE_ABI_VERSION, BytecodeProgram, BytecodeVerificationError};
use arcweft_core::effect::{LineEffectRequest, RuntimeCall};
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
use arcweft_core::value::{RuntimeBinding, RuntimeExpr, RuntimePayload, RuntimeValue};
use arcweft_id::PublicId;
use arcweft_interaction_model::{
    id::Identifier,
    input::{InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent},
    payload::InteractionPayload,
};
use arcweft_presentation::fx::{FxRuntimeType, ValueInstruction, ValueProgramSchema};
use arcweft_presentation::input::{
    Action, ActionTarget, InteractionTarget as PresentationInteractionTarget,
};
use arcweft_presentation::text_input::{
    TextByteOffset, TextControlValue, TextControlWriteBack, TextInputSessionId, TextRange,
    TextRevision,
};
use arcweft_render_text::{LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode};
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::dialogue::{
    BundleDialoguePresentation, BundlePresentationTransition, DialogueAdvanceRejection,
    DialogueStageAdvanceKind,
};
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use arcweft_runtime_driver::session::{
    BundleEntryStart, BundleEntryStartError, BundleHotSwapError, BundlePatchReadiness,
    BundleSession, BundleSessionError, BundleSessionOptions, BundleStepInput,
    BundleVirtualListMountError,
};
use arcweft_runtime_driver::session_save::BundleSessionSaveError;
use arcweft_runtime_driver::swap::SwapCompatibility;
use arcweft_runtime_driver::view_runtime::BundleViewTextValue;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_view::program::{ViewStableKey, ViewVirtualAxis};
use arcweft_view::virtualization::{ViewVirtualItem, ViewVirtualScrollTarget};
use arcweft_view::{ViewValueProgram, ViewValueProgramId};

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::from_runtime_target_value(value).expect("test flow ID is valid")
}

fn line_id(value: &str) -> RuntimeLineId {
    RuntimeLineId::from_runtime_line_value(value).expect("test line ID is valid")
}

fn dialogue_text(presentation: &BundlePresentationSnapshot) -> Option<&str> {
    presentation
        .dialogue
        .as_ref()
        .and_then(BundleDialoguePresentation::current_stage)
        .map(arcweft_render_text::LineDisplayStage::text)
}

fn queue_current_dialogue_advance(session: &mut BundleSession) {
    let target = session
        .presentation()
        .dialogue
        .as_ref()
        .and_then(BundleDialoguePresentation::advance_target)
        .expect("dialogue is waiting for advance");
    session.queue_dialogue_advance(target);
}

fn fixture_bundle() -> ArcweftBundle {
    fixture_bundle_with("WebGPU dialogue", false, false)
}

fn paged_fixture_bundle() -> ArcweftBundle {
    let mut bundle = fixture_bundle_with("unused", false, false);
    let line = line_id("line.opening");
    bundle.display = LineDisplayCatalog::new(vec![LineDisplaySpec {
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
        content: RichTextDocument::new(vec![
            RichTextNode::Text {
                text: "A".to_owned(),
            },
            RichTextNode::Control {
                control: arcweft_render_text::RichTextControl::Page,
            },
            RichTextNode::Text {
                text: "B".to_owned(),
            },
            RichTextNode::Control {
                control: arcweft_render_text::RichTextControl::LineWait,
            },
            RichTextNode::Text {
                text: "C".to_owned(),
            },
            RichTextNode::Control {
                control: arcweft_render_text::RichTextControl::Page,
            },
        ]),
    }]);
    bundle
}

fn structured_vm_fixture_bundle() -> ArcweftBundle {
    fixture_bundle_from_parts("WebGPU dialogue", false, false, false)
}

#[expect(
    clippy::too_many_lines,
    reason = "the executable View fixture keeps its typed program, text, input, and AWBC mount contract together"
)]
fn executable_view_fixture_bundle() -> ArcweftBundle {
    let plan = RuntimePlan::new(
        Some(flow_id("flow.main")),
        vec![RuntimeFlow {
            id: flow_id("flow.main"),
            ops: vec![
                FlowOp::If {
                    condition: RuntimeExpr::Local("active".to_owned()),
                    then_ops: Vec::new(),
                    else_ops: Vec::new(),
                },
                FlowOp::Effect(LineEffectRequest::Call(RuntimeCall {
                    callee: "presentation.handle.create".to_owned(),
                    args: vec![
                        "handle = @handle.main.root".to_owned(),
                        "kind = \"view\"".to_owned(),
                        "resource = @view.Root".to_owned(),
                    ],
                })),
                FlowOp::Effect(LineEffectRequest::Call(RuntimeCall {
                    callee: "presentation.handle.create".to_owned(),
                    args: vec![
                        "handle = @handle.side.root".to_owned(),
                        "kind = \"view\"".to_owned(),
                        "resource = @view.Root".to_owned(),
                    ],
                })),
                FlowOp::Loop {
                    body: vec![FlowOp::Noop],
                },
            ],
        }],
        vec![LineTaskGroup::default()],
    )
    .unwrap();
    let display = LineDisplayCatalog::new(Vec::new());
    let stats = BytecodeProgram::from_runtime_plan(plan.clone()).stats();
    let product_awbc = AwbcLowerer::new(&plan, &display, "view-runtime.arcw")
        .lower()
        .unwrap()
        .program;
    let mut bundle = ArcweftBundle::new(
        BundleManifest {
            source_label: "view-runtime.arcw".to_owned(),
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
            label: "view-runtime.arcw".to_owned(),
            text: String::new(),
        },
        BytecodeProgram::from_runtime_plan(plan),
        display,
    )
    .with_product_awbc(product_awbc);
    bundle.view_program = Some(ViewProgramResource {
        program_id: "view.program.session".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.Root".to_owned(),
            body: ViewInstructionSpan::new(0, 5),
            parameters: vec![ViewParameterResource {
                ordinal: 0,
                name: "active".to_owned(),
                value_type: Some(FxRuntimeType::Bool),
                value_slot: Some(0),
                default_program: None,
            }],
            state_schema_hash: 0xA11CE,
        }],
        value_programs: vec![
            ViewValueProgram::validate(
                ViewValueProgramId(0),
                ValueProgramSchema::new(vec![FxRuntimeType::Bool], Vec::new(), FxRuntimeType::Bool),
                vec![
                    ValueInstruction::LoadParameter {
                        slot: 0,
                        ty: FxRuntimeType::Bool,
                    },
                    ValueInstruction::Return,
                ],
            )
            .unwrap(),
        ],
        value_inputs: vec![ViewValueInputResource {
            namespace: ViewValueInputNamespace::Parameter,
            slot: 0,
            value_type: FxRuntimeType::Bool,
            source: ViewValueInputSource::DefinitionParameter {
                view: "view.Root".to_owned(),
                name: "active".to_owned(),
            },
        }],
        instructions: vec![
            ViewProgramInstruction::Branch {
                condition_program: ViewValueProgramId(0),
                then_span: 1,
                else_span: Some(1),
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.session.yes".to_owned(),
                style: None,
                part: None,
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.session.no".to_owned(),
                style: None,
                part: None,
                source: None,
            },
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::TextField,
                target: Some("input.session.name".to_owned()),
                style: None,
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::CloseElement,
        ],
        text_blocks: vec![
            ViewTextBlockResource::new(
                "text.block.yes",
                Some("view.Root".to_owned()),
                None,
                "text.session.yes",
                ViewTextBlockBounds::new(0, 0, 100_000, 24_000),
            ),
            ViewTextBlockResource::new(
                "text.block.no",
                Some("view.Root".to_owned()),
                None,
                "text.session.no",
                ViewTextBlockBounds::new(0, 0, 100_000, 24_000),
            ),
        ],
        ..ViewProgramResource::default()
    });
    bundle.view_text = Some(ViewTextResource {
        sources: vec![
            ViewTextSourceRecord {
                public_id: "text.session.yes".to_owned(),
                kind: ViewTextSourceKind::Literal {
                    value: "yes".to_owned(),
                },
                source: None,
            },
            ViewTextSourceRecord {
                public_id: "text.session.no".to_owned(),
                kind: ViewTextSourceKind::Literal {
                    value: "no".to_owned(),
                },
                source: None,
            },
            ViewTextSourceRecord {
                public_id: "text.session.input.initial".to_owned(),
                kind: ViewTextSourceKind::Literal {
                    value: "initial".to_owned(),
                },
                source: None,
            },
        ],
        ..ViewTextResource::default()
    });
    bundle.with_view_input(ViewInputResource {
        options: vec![ViewInputOptions {
            public_id: "input.session.name".to_owned(),
            view: Some("view.Root".to_owned()),
            containing_scroll_region: None,
            kind: ViewInputKind::TextField,
            value_text_source: "text.session.input.initial".to_owned(),
            placeholder_text_source: None,
            purpose: ViewInputPurpose::Name,
            autocorrect: TextAssistPolicy::PlatformDefault,
            spellcheck: TextAssistPolicy::PlatformDefault,
            capitalization: TextCapitalization::Words,
            enter_key: EnterKeyHint::Done,
            multiline: false,
            selection_policy: ViewTextSelectionPolicy::Enabled,
            shortcut_policy: ViewTextShortcutPolicy::Enabled,
            tab_policy: ViewTextTabPolicy::FocusNavigation,
            vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
            secure_policy: ViewSecureInputPolicy::Plain,
            composition_on_blur: CompositionOnBlurPolicy::Commit,
            submit_handler: None,
            change_handler: None,
            adapter_requirements: Vec::new(),
        }],
        adapter_requirements: Vec::new(),
    })
}

fn awfb_bytes(bundle: &ArcweftBundle) -> Vec<u8> {
    bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("fixture encodes as AWFB")
}

fn awfb_identity(bytes: &[u8]) -> ArtifactIdentity {
    BundleView::parse(bytes, ReadBudget::default())
        .expect("fixture AWFB parses")
        .artifact_identity()
}

fn test_patch_artifact(
    plan: BundlePatchPlan,
    base_artifact: ArtifactIdentity,
) -> BundlePatchArtifact {
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
    let target_artifact =
        if plan.is_empty() && plan.target_content_root == base_artifact.content_root {
            base_artifact
        } else {
            ArtifactIdentity::for_current_container(
                BundleKind::Program,
                plan.target_content_root,
                BundleDigest::of(b"test-target-manifest"),
            )
        };
    BundlePatchArtifact {
        manifest: BundlePatchManifest {
            schema_version: PATCH_PLAN_SCHEMA_VERSION,
            min_reader_schema_version: PATCH_PLAN_SCHEMA_VERSION,
            runtime_abi: RuntimeAbiRange::CURRENT,
            base_artifact,
            target_artifact,
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
                        "view.action.await",
                        "view.action",
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

fn fixture_action_receive_after_dialogue_bundle() -> ArcweftBundle {
    let line = line_id("line.action_intro");
    let plan = RuntimePlan::new(
        Some(flow_id("flow.main")),
        vec![RuntimeFlow {
            id: flow_id("flow.main"),
            ops: vec![
                FlowOp::Dialogue {
                    line: line.clone(),
                    task_group: 0,
                },
                FlowOp::HostCall {
                    binding: Some(RuntimePattern::Ident("event".to_owned())),
                    target: RuntimeHostCallTarget::new(
                        "view.action.await",
                        "view.action",
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
    let display = LineDisplayCatalog::new(vec![LineDisplaySpec {
        line,
        callee: "concierge".to_owned(),
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
            text: "Submit the form.".to_owned(),
        }]),
    }]);
    let bundle = ArcweftBundle::new(
        BundleManifest {
            source_label: "action-receive-after-dialogue.arcw".to_owned(),
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
            label: "action-receive-after-dialogue.arcw".to_owned(),
            text: String::new(),
        },
        BytecodeProgram::from_runtime_plan(plan.clone()),
        display.clone(),
    );
    let product_awbc = AwbcLowerer::new(&plan, &display, "action-receive-after-dialogue.arcw")
        .lower()
        .expect("product AWBC lowers")
        .program;
    bundle.with_product_awbc(product_awbc)
}

fn fixture_action_receive_bundle_with_submit_input() -> ArcweftBundle {
    fixture_action_receive_bundle().with_view_input(ViewInputResource {
        options: vec![ViewInputOptions {
            public_id: "input.feedback".to_owned(),
            view: Some("view.FeedbackForm".to_owned()),
            containing_scroll_region: None,
            kind: ViewInputKind::TextField,
            value_text_source: "text.value.input.feedback".to_owned(),
            placeholder_text_source: None,
            purpose: ViewInputPurpose::Text,
            autocorrect: TextAssistPolicy::PlatformDefault,
            spellcheck: TextAssistPolicy::PlatformDefault,
            capitalization: TextCapitalization::None,
            enter_key: EnterKeyHint::Send,
            multiline: false,
            selection_policy: ViewTextSelectionPolicy::Enabled,
            shortcut_policy: ViewTextShortcutPolicy::Enabled,
            tab_policy: ViewTextTabPolicy::FocusNavigation,
            vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
            secure_policy: ViewSecureInputPolicy::Plain,
            composition_on_blur: CompositionOnBlurPolicy::Commit,
            submit_handler: Some("action.feedback.submit".to_owned()),
            change_handler: Some("input.feedback".to_owned()),
            adapter_requirements: Vec::new(),
        }],
        adapter_requirements: Vec::new(),
    })
}

fn fixture_action_receive_after_dialogue_bundle_with_submit_input() -> ArcweftBundle {
    fixture_action_receive_after_dialogue_bundle().with_view_input(ViewInputResource {
        options: vec![ViewInputOptions {
            public_id: "input.feedback".to_owned(),
            view: Some("view.FeedbackForm".to_owned()),
            containing_scroll_region: None,
            kind: ViewInputKind::TextField,
            value_text_source: "text.value.input.feedback".to_owned(),
            placeholder_text_source: None,
            purpose: ViewInputPurpose::Text,
            autocorrect: TextAssistPolicy::PlatformDefault,
            spellcheck: TextAssistPolicy::PlatformDefault,
            capitalization: TextCapitalization::None,
            enter_key: EnterKeyHint::Send,
            multiline: false,
            selection_policy: ViewTextSelectionPolicy::Enabled,
            shortcut_policy: ViewTextShortcutPolicy::Enabled,
            tab_policy: ViewTextTabPolicy::FocusNavigation,
            vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
            secure_policy: ViewSecureInputPolicy::Plain,
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
    assert_eq!(dialogue_text(&first.presentation), Some("WebGPU dialogue"));

    queue_current_dialogue_advance(&mut session);
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
fn dialogue_advance_consumes_page_and_line_wait_stages_before_runtime_line() {
    let bundle = paged_fixture_bundle();
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
    let first = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    let first_dialogue = first.presentation.dialogue.as_ref().expect("dialogue");
    let first_target = first_dialogue.advance_target().expect("advance target");
    assert_eq!(dialogue_text(&first.presentation), Some("A"));

    session.queue_dialogue_advance(first_target);
    let second = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(dialogue_text(&second.presentation), Some("B"));
    assert!(second.presentation.choices.is_empty());
    assert!(matches!(
        second.presentation_transitions.as_slice(),
        [BundlePresentationTransition::StageAdvanced {
            advance: DialogueStageAdvanceKind::NextPage,
            ..
        }]
    ));

    let save = session
        .export_session_save_bytes()
        .expect("page-stage session save exports");
    let mut restored =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session restarts");
    restored
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect("page-stage session save restores");
    assert_eq!(dialogue_text(restored.presentation()), Some("B"));
    session = restored;

    session.queue_dialogue_advance(first_target);
    let stale = session.step_with_clock(
        RuntimeClockStep::from_millis(3, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(dialogue_text(&stale.presentation), Some("B"));
    assert!(matches!(
        stale.presentation_transitions.as_slice(),
        [BundlePresentationTransition::DialogueAdvanceRejected {
            reason: DialogueAdvanceRejection::StaleStage,
            ..
        }]
    ));

    queue_current_dialogue_advance(&mut session);
    let third = session.step_with_clock(
        RuntimeClockStep::from_millis(4, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(dialogue_text(&third.presentation), Some("BC"));
    assert!(third.presentation.choices.is_empty());
    assert!(matches!(
        third.presentation_transitions.as_slice(),
        [BundlePresentationTransition::StageAdvanced {
            advance: DialogueStageAdvanceKind::ContinuePage,
            ..
        }]
    ));

    let final_target = session
        .presentation()
        .dialogue
        .as_ref()
        .and_then(BundleDialoguePresentation::advance_target)
        .expect("final stage is actionable");
    session.queue_dialogue_advance(final_target);
    session.queue_dialogue_advance(final_target);
    let choice = session.step_with_clock(
        RuntimeClockStep::from_millis(5, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(dialogue_text(&choice.presentation), Some("BC"));
    assert!(
        !choice
            .presentation
            .dialogue
            .as_ref()
            .expect("retained dialogue")
            .is_waiting_for_advance()
    );
    assert_eq!(choice.presentation.choices.len(), 1);
    assert!(matches!(
        choice.presentation_transitions.as_slice(),
        [
            BundlePresentationTransition::RuntimeLineAdvanceQueued { .. },
            BundlePresentationTransition::DialogueAdvanceRejected {
                reason: DialogueAdvanceRejection::NotWaiting,
                ..
            }
        ]
    ));
}

#[test]
fn session_save_restores_complete_per_mount_virtual_range_state() {
    let mut bundle = paged_fixture_bundle();
    bundle.view_program = Some(ViewProgramResource {
        program_id: "view.program.inventory".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.inventory".to_owned(),
            body: ViewInstructionSpan::new(0, 0),
            parameters: Vec::new(),
            state_schema_hash: 0,
        }],
        scroll_regions: vec![ViewScrollRegionResource::new(
            "scroll.inventory",
            Some("view.inventory".to_owned()),
            ViewLogicalRect::from_px(0, 0, 100, 100),
            100_000,
            240,
            ViewScrollAxis::Vertical,
        )],
        ..ViewProgramResource::default()
    });
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
    session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert!(matches!(
        session.mount_virtual_list(
            ViewVirtualScrollTarget::from(PublicId::try_new("scroll.inventory").unwrap()),
            ViewVirtualAxis::Horizontal,
            100,
            vec![ViewVirtualItem::new(ViewStableKey(99), 60)],
        ),
        Err(BundleVirtualListMountError::AxisMismatch { .. })
    ));
    let mount = session
        .mount_virtual_list(
            ViewVirtualScrollTarget::from(PublicId::try_new("scroll.inventory").unwrap()),
            ViewVirtualAxis::Vertical,
            100,
            vec![
                ViewVirtualItem::new(ViewStableKey(1), 60),
                ViewVirtualItem::new(ViewStableKey(2), 60),
                ViewVirtualItem::new(ViewStableKey(3), 60),
                ViewVirtualItem::new(ViewStableKey(4), 60),
            ],
        )
        .expect("list mounts under a matching authored Scroll");
    assert_eq!(mount.get(), 0);
    session.virtual_list_mut(mount).unwrap().scroll_to_milli(70);

    let save = session.export_session_save_bytes().expect("session saves");
    let mut restored =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session restarts");
    restored
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .expect("virtual range state restores");

    let list = restored
        .view_virtualization()
        .get(mount)
        .expect("mount is reconstructed from the save");
    assert_eq!(list.offset_milli(), 70);
    assert_eq!(list.materialized_window().start, 1);
    assert_eq!(list.materialized_window().end, 3);
    assert_eq!(
        list.range_table()
            .items
            .iter()
            .map(|range| range.materialized)
            .collect::<Vec<_>>(),
        vec![false, true, true, false]
    );

    let before_hot_swap = restored
        .snapshot_session()
        .expect("snapshot before hot swap");
    assert!(matches!(
        restored.hot_swap_bundle(&paged_fixture_bundle()),
        Err(BundleHotSwapError::ViewVirtualization { .. })
    ));
    assert_eq!(
        restored
            .snapshot_session()
            .expect("rejected hot swap is atomic"),
        before_hot_swap
    );

    let before = restored
        .snapshot_session()
        .expect("restored snapshot exports");
    let mut tampered = before.clone();
    tampered.view_virtualization.mounts[0].absolute_offset_milli = u64::MAX;
    assert!(matches!(
        restored.restore_session_snapshot(tampered),
        Err(BundleSessionSaveError::ViewVirtualization { .. })
    ));
    assert_eq!(
        restored
            .snapshot_session()
            .expect("failed restore is atomic"),
        before
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the end-to-end assertion covers two mounts, reactive output, scoped input, save/load, and atomic tamper rejection"
)]
fn session_executes_mount_scoped_view_branch_and_restores_it() {
    let bundle = executable_view_fixture_bundle();
    let options = BundleSessionOptions {
        mode: arcweft_core::step::RuntimeStepMode::Drain,
        ..BundleSessionOptions::default()
    };
    let mut session = BundleSession::new(&bundle, options.clone()).expect("session starts");
    let active = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).unwrap(),
        BundleStepInput {
            bindings: vec![RuntimeBinding {
                name: "active".to_owned(),
                value: RuntimeValue::Bool(true),
            }],
            ..BundleStepInput::default()
        },
    );
    assert!(active.presentation.view.diagnostics.is_empty());
    assert_eq!(active.presentation.view.mounts.len(), 2, "{active:#?}");
    assert_eq!(
        active.presentation.view.mounts[0].view,
        active.presentation.view.mounts[1].view
    );
    assert_ne!(
        active.presentation.view.mounts[0].mount,
        active.presentation.view.mounts[1].mount
    );
    assert_eq!(
        active
            .presentation
            .view
            .mounts
            .iter()
            .map(|mount| mount.text.len())
            .sum::<usize>(),
        2,
        "{active:#?}"
    );
    assert!(
        active
            .presentation
            .view
            .mounts
            .iter()
            .flat_map(|mount| &mount.text)
            .all(
                |text| matches!(&text.value, BundleViewTextValue::Plain { value } if value == "yes")
            )
    );
    assert_eq!(
        active
            .presentation
            .view
            .mounts
            .iter()
            .flat_map(|mount| {
                mount.text.iter().flat_map(|text| {
                    text.targets
                        .iter()
                        .map(|target| mount.scoped_id(&target.public_id))
                })
            })
            .collect::<Vec<_>>(),
        vec!["view_mount_0.text.block.yes", "view_mount_1.text.block.yes"]
    );
    assert_eq!(active.presentation.text_inputs.len(), 2);
    assert!(
        active
            .presentation
            .text_inputs
            .iter()
            .all(|control| control.value == "initial")
    );

    let side_control = active
        .presentation
        .text_inputs
        .iter()
        .find(|control| control.target == "view_mount_1.input.session.name")
        .unwrap();
    session
        .queue_text_control_write_back(&TextControlWriteBack::change(
            PresentationInteractionTarget::new(
                PublicId::try_new(side_control.target.clone()).unwrap(),
            ),
            TextInputSessionId(side_control.session),
            TextControlValue::plain("Alice"),
            TextRange::new(TextByteOffset(5), TextByteOffset(5)),
            TextRevision(1),
        ))
        .unwrap();
    let edited = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).unwrap(),
        BundleStepInput {
            bindings: vec![RuntimeBinding {
                name: "active".to_owned(),
                value: RuntimeValue::Bool(true),
            }],
            ..BundleStepInput::default()
        },
    );
    assert_eq!(
        edited
            .presentation
            .text_inputs
            .iter()
            .map(|control| control.value.as_str())
            .collect::<Vec<_>>(),
        vec!["initial", "Alice"]
    );

    let save = session.export_session_save_bytes().unwrap();
    let mut restored = BundleSession::new(&bundle, options).unwrap();
    restored
        .import_session_save_bytes(&save, &arcweft_save::SaveDecodeOptions::default())
        .unwrap();
    assert_eq!(restored.presentation().view, edited.presentation.view);
    assert_eq!(
        restored
            .presentation()
            .text_inputs
            .iter()
            .map(|control| control.value.as_str())
            .collect::<Vec<_>>(),
        vec!["initial", "Alice"]
    );

    let inactive = restored.step_with_clock(
        RuntimeClockStep::from_millis(3, 16).unwrap(),
        BundleStepInput {
            bindings: vec![RuntimeBinding {
                name: "active".to_owned(),
                value: RuntimeValue::Bool(false),
            }],
            ..BundleStepInput::default()
        },
    );
    assert!(inactive.presentation.view.diagnostics.is_empty());
    assert_eq!(inactive.presentation.view.mounts[0].mount.get(), 0);
    assert_eq!(inactive.presentation.view.mounts[1].mount.get(), 1);
    assert_eq!(
        inactive
            .presentation
            .view
            .mounts
            .iter()
            .map(|mount| mount.text.len())
            .sum::<usize>(),
        2
    );
    assert!(
        inactive
            .presentation
            .view
            .mounts
            .iter()
            .flat_map(|mount| &mount.text)
            .all(
                |text| matches!(&text.value, BundleViewTextValue::Plain { value } if value == "no")
            )
    );
    assert_eq!(
        inactive
            .presentation
            .view
            .mounts
            .iter()
            .flat_map(|mount| {
                mount.text.iter().flat_map(|text| {
                    text.targets
                        .iter()
                        .map(|target| mount.scoped_id(&target.public_id))
                })
            })
            .collect::<Vec<_>>(),
        vec!["view_mount_0.text.block.no", "view_mount_1.text.block.no"]
    );

    let before_tampered_restore = restored.snapshot_session().unwrap();
    let mut tampered = before_tampered_restore.clone();
    tampered.presentation.view.mounts[0].view = "view.Other".to_owned();
    assert!(matches!(
        restored.restore_session_snapshot(tampered),
        Err(BundleSessionSaveError::ViewRuntime { .. })
    ));
    assert_eq!(
        restored.snapshot_session().unwrap(),
        before_tampered_restore
    );
}

#[test]
fn raw_dialogue_events_cannot_bypass_the_presentation_stage_target() {
    let bundle = paged_fixture_bundle();
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
    let first = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    let line = first
        .presentation
        .dialogue
        .as_ref()
        .expect("dialogue")
        .frame()
        .line
        .public_label()
        .as_str()
        .to_owned();
    let raw_event = |target: &str, name: &str| {
        RoutedInputEvent::new(
            InputEpoch::default(),
            InputSequence::default(),
            InteractionTarget::new(target).expect("test target"),
            InputEventKind::Custom {
                name: Identifier::new(name).expect("test input name"),
            },
        )
        .with_payload(InteractionPayload::Text(line.clone()))
    };

    let rejected = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput {
            input_events: vec![
                raw_event("dialogue-widget", "dialogue.advance"),
                raw_event("runtime", "advance"),
            ],
            ..BundleStepInput::default()
        },
    );

    assert_eq!(dialogue_text(&rejected.presentation), Some("A"));
    assert!(
        rejected
            .presentation
            .dialogue
            .as_ref()
            .expect("dialogue remains active")
            .is_waiting_for_advance()
    );
    assert_eq!(rejected.presentation_transitions.len(), 2);
    assert!(rejected.presentation_transitions.iter().all(|transition| {
        matches!(
            transition,
            BundlePresentationTransition::DialogueAdvanceRejected {
                target: None,
                reason: DialogueAdvanceRejection::UntargetedRuntimeInput,
            }
        )
    }));
}

#[test]
fn malformed_dialogue_frame_restore_is_rejected_without_mutating_the_session() {
    let bundle = paged_fixture_bundle();
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
    session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    let before = session.snapshot_session().expect("live snapshot exports");
    let mut value = serde_json::to_value(&before).expect("snapshot encodes as JSON value");
    let controls = value
        .pointer_mut("/presentation/dialogue/frame/display_map/controls")
        .and_then(serde_json::Value::as_array_mut)
        .expect("dialogue controls are present");
    controls[1]["text_offset"] = serde_json::json!(0);
    let invalid = serde_json::from_value(value).expect("structurally typed snapshot decodes");

    let error = session
        .restore_session_snapshot(invalid)
        .expect_err("regressing dialogue marker is rejected");
    assert!(matches!(error, BundleSessionSaveError::Presentation { .. }));
    assert_eq!(
        session
            .snapshot_session()
            .expect("live session remains valid"),
        before
    );
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
fn session_receive_action_reached_by_dialogue_advance_uses_same_step_action() {
    let bundle = fixture_action_receive_after_dialogue_bundle();
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
    let dialogue = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(
        dialogue_text(&dialogue.presentation),
        Some("Submit the form.")
    );

    queue_current_dialogue_advance(&mut session);
    let action = Action::new(
        ActionTarget::Runtime,
        PublicId::try_new("action.feedback.submit").expect("action id"),
    )
    .with_payload("Ada");
    session
        .queue_semantic_action(&action)
        .expect("generic semantic action is accepted");
    let reached_receive = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert!(!reached_receive.finished);

    let resumed = session.step_with_clock(
        RuntimeClockStep::from_millis(3, 16).expect("clock"),
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
        .view_input
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
fn session_receive_action_reached_by_dialogue_advance_uses_same_step_text_submit() {
    let bundle = fixture_action_receive_after_dialogue_bundle_with_submit_input();
    let submit_session = bundle
        .view_input
        .as_ref()
        .expect("fixture has input resource")
        .options[0]
        .runtime_text_session();
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
    let dialogue = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert_eq!(
        dialogue_text(&dialogue.presentation),
        Some("Submit the form.")
    );

    let submit = TextControlWriteBack::submit(
        PresentationInteractionTarget::new(
            PublicId::try_new("input.feedback").expect("valid target"),
        ),
        TextInputSessionId(submit_session),
        TextControlValue::plain("Ada"),
        TextRange::new(TextByteOffset(3), TextByteOffset(3)),
        TextRevision(1),
    );
    queue_current_dialogue_advance(&mut session);
    session
        .queue_text_control_write_back(&submit)
        .expect("submit writeback is accepted");
    let reached_receive = session.step_with_clock(
        RuntimeClockStep::from_millis(2, 16).expect("clock"),
        BundleStepInput::default(),
    );
    assert!(!reached_receive.finished);

    let resumed = session.step_with_clock(
        RuntimeClockStep::from_millis(3, 16).expect("clock"),
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
    assert_eq!(dialogue_text(&step.presentation), Some("New text"));
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
    queue_current_dialogue_advance(&mut session);
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

    queue_current_dialogue_advance(&mut session);
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
    assert_eq!(dialogue_text(&step.presentation), Some("WebGPU dialogue"));
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
    let base_artifact = awfb_identity(&bytes);
    let artifact = test_patch_artifact(
        BundlePatchPlan {
            base_content_root: base_artifact.content_root,
            target_content_root: base_artifact.content_root,
            operations: Vec::new(),
        },
        base_artifact,
    );

    let report = session
        .inspect_hot_swap_patch_artifact(&artifact)
        .expect("noop patch is ready");

    assert_eq!(report.base_generation, session.active_generation().id);
    assert_eq!(
        report.declared_compatibility,
        PatchCompatibility::ContentOnly
    );
    assert_eq!(report.readiness, BundlePatchReadiness::Noop);
}

#[test]
fn patch_readiness_rejects_same_root_from_a_different_base_artifact() {
    let expected_base = fixture_bundle();
    let mut active_base = expected_base.clone();
    active_base.manifest.source_label = "active-base.arcw".to_owned();
    let target = fixture_bundle_with("Updated text", false, false);
    let expected_base_bytes = awfb_bytes(&expected_base);
    let active_base_bytes = awfb_bytes(&active_base);
    let target_bytes = awfb_bytes(&target);
    let expected_base_view =
        BundleView::parse(&expected_base_bytes, ReadBudget::default()).expect("base parses");
    let active_base_view =
        BundleView::parse(&active_base_bytes, ReadBudget::default()).expect("active base parses");
    let target_view =
        BundleView::parse(&target_bytes, ReadBudget::default()).expect("target parses");
    assert_eq!(
        expected_base_view.content_root(),
        active_base_view.content_root()
    );
    assert_ne!(
        expected_base_view.artifact_identity(),
        active_base_view.artifact_identity()
    );
    let patch = BundlePatchArtifact::from_views(&expected_base_view, &target_view)
        .expect("patch artifact builds");
    let patch_bytes = encode_patch_bundle(&patch).expect("patch encodes");
    let mut session =
        BundleSession::from_awfb_bytes(&active_base_bytes, BundleSessionOptions::default())
            .expect("session starts from the other manifest");

    let error = session
        .inspect_hot_swap_patch_artifact(&patch)
        .expect_err("matching section roots do not authorize another base artifact");

    assert!(matches!(
        error,
        BundleHotSwapError::WrongPatchBaseArtifact { active, expected }
            if *active == active_base_view.artifact_identity()
                && *expected == expected_base_view.artifact_identity()
    ));

    let error = session
        .hot_swap_patch_bytes(&expected_base_bytes, &patch_bytes)
        .expect_err("caller-provided base bytes cannot bypass the active artifact identity");
    assert!(matches!(
        error,
        BundleHotSwapError::WrongPatchBaseArtifact { active, expected }
            if *active == active_base_view.artifact_identity()
                && *expected == expected_base_view.artifact_identity()
    ));
}

#[test]
fn patch_readiness_reports_target_bundle_required_for_section_operations() {
    let bundle = fixture_bundle();
    let bytes = awfb_bytes(&bundle);
    let session = BundleSession::from_awfb_bytes(&bytes, BundleSessionOptions::default())
        .expect("session starts");
    let base_artifact = awfb_identity(&bytes);
    let artifact = test_patch_artifact(
        BundlePatchPlan {
            base_content_root: base_artifact.content_root,
            target_content_root: BundleDigest::of(b"target"),
            operations: vec![SectionOperation::Remove {
                id: SectionId::from_bytes([7; 16]),
                old: BundleDigest::of(b"old"),
            }],
        },
        base_artifact,
    );

    let report = session
        .inspect_hot_swap_patch_artifact(&artifact)
        .expect("patch validates against active base");

    assert_eq!(
        report.readiness,
        BundlePatchReadiness::TargetBundleRequired { operations: 1 }
    );
    assert_eq!(
        report.declared_compatibility,
        PatchCompatibility::RestartRequired
    );
}

#[test]
fn patch_readiness_decodes_awfb_patch_bytes() {
    let bundle = fixture_bundle();
    let bundle_bytes = awfb_bytes(&bundle);
    let session = BundleSession::from_awfb_bytes(&bundle_bytes, BundleSessionOptions::default())
        .expect("session starts");
    let base_artifact = awfb_identity(&bundle_bytes);
    let artifact = test_patch_artifact(
        BundlePatchPlan {
            base_content_root: base_artifact.content_root,
            target_content_root: base_artifact.content_root,
            operations: Vec::new(),
        },
        base_artifact,
    );
    let bytes = encode_patch_bundle(&artifact).expect("patch bundle encodes");

    let report = session
        .inspect_hot_swap_patch_bytes(&bytes)
        .expect("patch bundle decodes");

    assert_eq!(report.readiness, BundlePatchReadiness::Noop);
    assert_eq!(
        report.declared_compatibility,
        PatchCompatibility::ContentOnly
    );
}

#[test]
fn patch_readiness_rejects_wrong_active_base() {
    let bundle = fixture_bundle();
    let bytes = awfb_bytes(&bundle);
    let session = BundleSession::from_awfb_bytes(&bytes, BundleSessionOptions::default())
        .expect("session starts");
    let other_base = BundleDigest::of(b"other-base");
    let artifact = test_patch_artifact(
        BundlePatchPlan {
            base_content_root: other_base,
            target_content_root: BundleDigest::of(b"target"),
            operations: Vec::new(),
        },
        ArtifactIdentity::for_current_container(
            BundleKind::Program,
            other_base,
            BundleDigest::of(b"other-base-manifest"),
        ),
    );

    let error = session
        .inspect_hot_swap_patch_artifact(&artifact)
        .expect_err("wrong base rejects");

    assert!(matches!(
        error,
        BundleHotSwapError::WrongPatchBaseArtifact { .. }
    ));
}

#[test]
fn patch_readiness_requires_awfb_backed_session() {
    let bundle = fixture_bundle();
    let session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
    let base = BundleDigest::of(b"base");
    let artifact = test_patch_artifact(
        BundlePatchPlan {
            base_content_root: base,
            target_content_root: BundleDigest::of(b"target"),
            operations: Vec::new(),
        },
        ArtifactIdentity::for_current_container(
            BundleKind::Program,
            base,
            BundleDigest::of(b"base-manifest"),
        ),
    );

    let error = session
        .inspect_hot_swap_patch_artifact(&artifact)
        .expect_err("decoded-only session has no AWFB artifact identity");

    assert!(matches!(
        error,
        BundleHotSwapError::MissingActiveContainerIdentity
    ));
}

#[test]
fn hot_swap_patch_bytes_does_not_trust_declared_restart_before_materialization() {
    let bundle = fixture_bundle();
    let bundle_bytes = awfb_bytes(&bundle);
    let base_artifact = awfb_identity(&bundle_bytes);
    let base = base_artifact.content_root;
    let artifact = test_patch_artifact(
        BundlePatchPlan {
            base_content_root: base,
            target_content_root: BundleDigest::of(b"target"),
            operations: vec![SectionOperation::Remove {
                id: SectionId::from_bytes([9; 16]),
                old: BundleDigest::of(b"old"),
            }],
        },
        base_artifact,
    );
    let patch_bytes = encode_patch_bundle(&artifact).expect("patch bundle encodes");
    let mut session =
        BundleSession::from_awfb_bytes(&bundle_bytes, BundleSessionOptions::default())
            .expect("session starts");

    let error = session
        .hot_swap_patch_bytes(b"not an AWFB base", &patch_bytes)
        .expect_err("unverified restart declaration must not bypass materialization");

    assert!(matches!(
        error,
        BundleHotSwapError::MaterializePatch(PatchBundleError::Container(_))
    ));
    assert_eq!(session.active_container_content_root(), Some(base));
}

#[test]
fn hot_swap_patch_bytes_materializes_noop_before_reporting_success() {
    let bundle = fixture_bundle();
    let bundle_bytes = awfb_bytes(&bundle);
    let view = BundleView::parse(&bundle_bytes, ReadBudget::default()).expect("AWFB parses");
    let patch = BundlePatchArtifact::from_views(&view, &view).expect("noop patch artifact");
    let patch_bytes = encode_patch_bundle(&patch).expect("noop patch encodes");
    let mut session =
        BundleSession::from_awfb_bytes(&bundle_bytes, BundleSessionOptions::default())
            .expect("session starts");
    let generation = session.active_generation().id;

    let error = session
        .hot_swap_patch_bytes(b"not an AWFB base", &patch_bytes)
        .expect_err("noop patch still validates its materialized base identity");
    assert!(matches!(
        error,
        BundleHotSwapError::MaterializePatch(PatchBundleError::Container(_))
    ));

    let report = session
        .hot_swap_patch_bytes(&bundle_bytes, &patch_bytes)
        .expect("verified noop succeeds");
    assert_eq!(report.generation, generation);
    assert_eq!(report.compatibility, SwapCompatibility::ContentOnly);
    assert_eq!(session.active_generation().id, generation);
    assert_eq!(
        session.active_container_content_root(),
        Some(view.content_root())
    );
    assert_eq!(
        session.active_container_artifact_identity(),
        Some(view.artifact_identity())
    );
}

#[test]
fn hot_swap_patch_bytes_applies_manifest_only_target() {
    let old_bundle = fixture_bundle();
    let mut new_bundle = old_bundle.clone();
    new_bundle.manifest.source_label = "manifest-only-target.arcw".to_owned();
    let old_bytes = awfb_bytes(&old_bundle);
    let new_bytes = awfb_bytes(&new_bundle);
    let old_view = BundleView::parse(&old_bytes, ReadBudget::default()).expect("old AWFB parses");
    let new_view = BundleView::parse(&new_bytes, ReadBudget::default()).expect("new AWFB parses");
    assert_eq!(old_view.content_root(), new_view.content_root());
    assert_ne!(old_view.artifact_identity(), new_view.artifact_identity());
    let patch = BundlePatchArtifact::from_views(&old_view, &new_view).expect("patch artifact");
    assert!(patch.plan.is_empty());
    let patch_bytes = encode_patch_bundle(&patch).expect("patch encodes");
    let mut session = BundleSession::from_awfb_bytes(&old_bytes, BundleSessionOptions::default())
        .expect("session starts");

    let readiness = session
        .inspect_hot_swap_patch_artifact(&patch)
        .expect("manifest-only patch is ready");
    assert_eq!(
        readiness.readiness,
        BundlePatchReadiness::TargetBundleRequired { operations: 0 }
    );
    let report = session
        .hot_swap_patch_bytes(&old_bytes, &patch_bytes)
        .expect("manifest-only patch applies");

    assert_eq!(
        report.generation,
        arcweft_runtime_driver::swap::GenerationId(1)
    );
    assert_eq!(report.compatibility, SwapCompatibility::ContentOnly);
    assert_eq!(session.source_label(), "manifest-only-target.arcw");
    assert_eq!(
        session.active_container_artifact_identity(),
        Some(new_view.artifact_identity())
    );
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
    assert_eq!(dialogue_text(&step.presentation), Some("New text"));
}

#[test]
fn hot_swap_patch_bytes_rejects_tampered_compatibility_without_mutating_session() {
    let old_bundle = fixture_bundle_with("Old text", false, false);
    let new_bundle = fixture_bundle_with("New text", false, false);
    let old_bytes = awfb_bytes(&old_bundle);
    let new_bytes = awfb_bytes(&new_bundle);
    let old_view = BundleView::parse(&old_bytes, ReadBudget::default()).expect("old AWFB parses");
    let new_view = BundleView::parse(&new_bytes, ReadBudget::default()).expect("new AWFB parses");
    let mut patch = BundlePatchArtifact::from_views(&old_view, &new_view).expect("patch artifact");
    patch.manifest.compatibility = PatchCompatibility::CodeCompatible;
    for fingerprint in &mut patch.manifest.compatibility_fingerprints {
        fingerprint.compatibility = PatchCompatibility::CodeCompatible;
    }
    let patch_bytes = encode_patch_bundle(&patch).expect("self-consistent tamper encodes");
    let mut session = BundleSession::from_awfb_bytes(&old_bytes, BundleSessionOptions::default())
        .expect("session starts");
    let generation_before = session.active_generation().id;
    let root_before = session.active_container_content_root();
    let source_before = session.source_label().to_owned();
    let presentation_before = session.presentation().clone();

    let error = session
        .hot_swap_patch_bytes(&old_bytes, &patch_bytes)
        .expect_err("materialized fingerprint verification rejects tampering");

    assert!(matches!(
        error,
        BundleHotSwapError::MaterializePatch(
            PatchBundleError::MaterializedFingerprintMismatch { .. }
        )
    ));
    assert_eq!(session.active_generation().id, generation_before);
    assert_eq!(session.active_container_content_root(), root_before);
    assert_eq!(session.source_label(), source_before);
    assert_eq!(session.presentation(), &presentation_before);
}
