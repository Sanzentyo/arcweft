use arcweft_compiler::source::compile_source;
use arcweft_core::{
    awbc::{
        codec::AwbcDecodeBudget,
        product_step::AwbcProductStepExecutor,
        schema::{AwbcEntryId, AwbcProgram},
    },
    engine::{Engine, FlowExit, FlowFiberStatus},
    entry::RuntimeSchemaLimits,
    presentation::RuntimePresentationFrame,
    step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
    time::LogicalDuration,
    value::{RuntimeQuantity, RuntimeValue},
};
use arcweft_interaction_model::presentation::PresentationArgumentRole;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use std::sync::Arc;

const SOURCE: &str = r#"
flow main() -> i64 {
    let first = image(id = "image.first", x = 96px, width = 360px, height = 180px);
    let second = image(id = "image.second", x = 12px, width = 20px, height = 30px);
    wait(10ms)
    first.hide();
    wait(10ms)
    first.show();
    wait(10ms)
    first.release();
    wait(10ms)
    second.release();
    return 42i64
}
entry cli @entry.main { goto @flow.main }
"#;

#[derive(Default)]
struct Seen {
    two: bool,
    hidden: bool,
    shown: bool,
    second_only: bool,
    empty_after_both: bool,
}

impl Seen {
    fn observe(&mut self, frame: RuntimePresentationFrame<'_>) {
        self.two |= frame.len() == 2;
        self.empty_after_both |= self.two && frame.is_empty();
        let mut first = None;
        let mut second = None;
        for resource in frame.resources() {
            let call = resource.value().layout().factory().expect("source factory");
            let RuntimeValue::Tuple(values) = resource.value().body() else {
                panic!("original input tuple")
            };
            assert_eq!(call.body_len(), values.len());
            let mut id = None;
            let mut x = None;
            for ((_, key), value) in call.body_inputs().zip(values) {
                match key.role() {
                    PresentationArgumentRole::Id => {
                        let RuntimeValue::String(value) = value else {
                            panic!("authored metadata")
                        };
                        id = Some(value.as_ref());
                    }
                    PresentationArgumentRole::X => x = RuntimeQuantity::from_value(value),
                    _ => {}
                }
            }
            match id.expect("the actual authored id is retained") {
                "image.first" => {
                    assert_eq!(
                        x,
                        Some(RuntimeQuantity::Length(arcweft_primitives::CheckedLength {
                            milli: 96_000,
                            unit: arcweft_primitives::LengthUnit::Px,
                        }))
                    );
                    first = Some(resource.token());
                    self.hidden |= resource.visibility_override() == Some(false);
                    self.shown |= resource.visibility_override() == Some(true);
                }
                "image.second" => second = Some(resource.token()),
                other => panic!("unexpected original body: {other}"),
            }
        }
        if let (Some(first), Some(second)) = (first, second) {
            assert_ne!(
                first, second,
                "distinct source calls retain distinct issuances"
            );
        }
        self.second_only |= self.two && first.is_none() && second.is_some();
    }

    fn finish(self) {
        assert!(
            self.two && self.hidden && self.shown && self.second_only && self.empty_after_both,
            "the original frame observes create/hide/show/release, not an always-empty projection"
        );
    }
}

fn run(native: bool) {
    let compiled = compile_source(SOURCE).expect("source image lifecycle compiles");
    let program = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "presentation-frame.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let bytes = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded, program);
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    };
    let input = || RuntimeStepInput {
        dt: LogicalDuration::from_nanos(1_000_000),
        ..Default::default()
    };
    let mut seen = Seen::default();
    if native {
        let mut plan = compiled.plan;
        plan.bind_artifact(
            arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes(
                *blake3::hash(&bytes).as_bytes(),
            )
            .unwrap(),
        )
        .unwrap();
        let flow = plan
            .flows()
            .iter()
            .find(|flow| flow.id.public_label().as_str() == "flow.main")
            .unwrap()
            .id
            .clone();
        let mut engine = Engine::for_flow(plan, &flow).unwrap();
        for _ in 0..256 {
            let result = engine.step(input(), options);
            assert!(result.stats.executed_ops <= 1);
            assert!(
                result.output.diagnostics.is_empty(),
                "{:?}",
                result.output.diagnostics
            );
            assert!(result.output.requests.tasks.is_empty());
            seen.observe(
                engine
                    .presentation_frame(RuntimeSchemaLimits::engine_default())
                    .unwrap(),
            );
            if let FlowFiberStatus::Done(exit) = result.fiber_status {
                assert_eq!(exit, FlowExit::Return("42".into()));
                seen.finish();
                return;
            }
        }
    } else {
        let mut executor =
            AwbcProductStepExecutor::for_entry_arc(Arc::new(decoded), AwbcEntryId(0), 1).unwrap();
        let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
        for _ in 0..256 {
            let result = executor.step_with_pure_backend(input(), options, &mut backend);
            assert!(result.stats.executed_ops <= 1);
            assert!(
                result.output.diagnostics.is_empty(),
                "{:?}",
                result.output.diagnostics
            );
            assert!(result.output.requests.tasks.is_empty());
            seen.observe(
                executor
                    .presentation_frame(RuntimeSchemaLimits::engine_default())
                    .unwrap(),
            );
            let snapshot = executor
                .snapshot()
                .expect("public save retains the exact active frame");
            executor = executor
                .restore_snapshot_owned(snapshot)
                .unwrap_or_else(|(_, error)| panic!("owned restore: {error}"));
            seen.observe(
                executor
                    .presentation_frame(RuntimeSchemaLimits::engine_default())
                    .unwrap(),
            );
            if let FlowFiberStatus::Done(exit) = result.fiber_status {
                assert_eq!(exit, FlowExit::Return("42".into()));
                seen.finish();
                return;
            }
        }
    }
    panic!("source lifecycle exceeded its deterministic step bound");
}

#[test]
fn native_frame_borrows_live_source_resources_through_their_lifecycle() {
    run(true);
}

#[test]
fn decoded_awbc_frame_borrows_the_same_resources_after_each_owned_restore() {
    run(false);
}
