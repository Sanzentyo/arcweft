use arcweft_bundle::{ArcweftBundle, BundleManifest, BundleRuntimeSummary, BundleSource};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_core::line_task::LineTaskGroup;
use arcweft_core::plan::{
    ChoiceRuntimeOption, FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeLineId, RuntimePlan,
};
use arcweft_id::PublicId;
use arcweft_presentation::input::{Action, ActionTarget};
use arcweft_render_text::{LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode};
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{BundleSession, BundleSessionOptions, BundleStepInput};

fn fixture_bundle() -> ArcweftBundle {
    let line = RuntimeLineId("line.opening".to_owned());
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![
            RuntimeFlow {
                id: FlowRuntimeId("flow.main".to_owned()),
                ops: vec![
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
                ],
            },
            RuntimeFlow {
                id: FlowRuntimeId("flow.done".to_owned()),
                ops: vec![FlowOp::Return("done".to_owned())],
            },
        ],
        vec![LineTaskGroup::default()],
    )
    .expect("runtime plan is valid");
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
                flows: 2,
                bytecode_instructions: 3,
                line_task_groups: 1,
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
                text: "WebGPU dialogue".to_owned(),
            }]),
        }]),
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
