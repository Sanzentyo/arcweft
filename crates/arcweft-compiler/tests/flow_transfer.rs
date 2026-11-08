use std::sync::Arc;

use arcweft_compiler::source::compile_source;
use arcweft_core::{
    awbc::{
        codec::AwbcDecodeBudget,
        product_step::{AwbcProductExecutorSaveSnapshot, AwbcProductProgram},
        schema::AwbcProgram,
        verify::{AwbcVerifyBudget, AwbcVerifyContext},
    },
    engine::{Engine, FlowExit, FlowFiberStatus},
    executor::{ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot, RuntimeExecutor},
    plan::{RuntimeEntryTarget, RuntimeTaskSourceArtifactLimits},
    step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

fn source(arguments: &str) -> String {
    format!(
        "entry cli @entry.main {{ goto @flow.main }}\n\
         flow main() -> i64 {{ goto @flow.next({arguments}) }}\n\
         flow next(left: i64, right: i64) -> i64 {{ return left * 10i64 + right }}\n\
         fn add(left: i64, right: i64) -> i64 {{ left + right }}\n"
    )
}

fn sources() -> Vec<String> {
    let mut cases = [
        "4i64, 2i64",
        "right = 2i64, left = 4i64",
        "right = add(1i64, 1i64), left = add(2i64, 2i64)",
    ]
    .map(source)
    .to_vec();
    cases.push(
        "entry cli @entry.main { goto @flow.main }\n\
         flow main() -> i64 { goto @flow.next(3i64) }\n\
         flow next(remaining: i64) -> i64 { if remaining > 0i64 { goto @flow.next(remaining - 1i64) }; return 42i64 }\n".to_owned()
    );
    for selected in [true, false] {
        let next = if selected { 42 } else { 99 };
        let other = if selected { 99 } else { 42 };
        cases.push(format!(
            "entry cli @entry.main {{ goto @flow.main }}\n\
             fn choose(flag: bool) -> Ref<Flow> {{ if flag {{ @flow.next }} else {{ @flow.other }} }}\n\
             flow main() -> i64 {{ goto choose({selected}) }}\n\
             flow next() -> i64 {{ return {next}i64 }}\n\
             flow other() -> i64 {{ return {other}i64 }}\n"
        ));
    }
    cases.push(
        "entry cli @entry.main { goto @flow.main }\n\
         fn leave() -> Unit { let handle = image(id = \"transfer.call\", width = 20px, height = 10px); goto @flow.next(handle) }\n\
         flow main() -> i64 { leave(); return 99i64 }\n\
         flow next(handle: Handle<ImageSurface>) -> i64 { handle.hide(); handle.show(); handle.release(); return 42i64 }\n".to_owned()
    );
    cases
}
#[test]
fn explicit_flow_inputs_reach_the_native_receiving_root() {
    for source in sources() {
        let compiled = compile_source(&source).expect("transfer source compiles");
        let [entry] = compiled.plan.entries() else {
            panic!("one authored entry");
        };
        let RuntimeEntryTarget::Flow(flow) = &entry.target else {
            panic!("the authored entry selects its Flow");
        };
        let bytes = AwbcLowerer::for_entry(
            &compiled.plan,
            &compiled.dialogue_content,
            "flow-transfer.arcw",
            &entry.id,
        )
        .lower()
        .unwrap()
        .program
        .encode_canonical()
        .unwrap();
        let mut plan = compiled.plan.clone();
        plan.bind_artifact(
            arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes(
                *blake3::hash(&bytes).as_bytes(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut engine = Engine::for_flow(plan, flow).unwrap();
        for _ in 0..256 {
            let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
            assert!(result.stats.executed_ops <= 1);
            assert!(result.output.diagnostics.is_empty(), "{:?}", result.output);
            match &engine.fiber().frame.status {
                FlowFiberStatus::Running => {}
                FlowFiberStatus::Done(exit) => {
                    assert_eq!(exit, &FlowExit::Return("42".to_owned()));
                    break;
                }
                status => panic!("transfer stopped: {status:?}"),
            }
        }
        assert!(matches!(
            engine.fiber().frame.status,
            FlowFiberStatus::Done(_)
        ));
    }
}

#[test]
fn explicit_flow_inputs_reach_the_decoded_awbc_receiving_root() {
    for source in sources() {
        let compiled = compile_source(&source).expect("transfer source compiles");
        let [entry] = compiled.plan.entries() else {
            panic!("one authored entry");
        };
        let RuntimeEntryTarget::Flow(flow) = &entry.target else {
            panic!("the authored entry selects its Flow");
        };
        let bytes = AwbcLowerer::for_entry(
            &compiled.plan,
            &compiled.dialogue_content,
            "flow-transfer.arcw",
            &entry.id,
        )
        .lower()
        .expect("transfer lowers to verified AWBC")
        .program
        .encode_canonical()
        .unwrap();
        let fresh = || {
            let program =
                AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
            let admitted = AwbcProductProgram::admit(
                Arc::new(program),
                AwbcVerifyBudget::default(),
                AwbcVerifyContext::default(),
                RuntimeTaskSourceArtifactLimits::default(),
                None,
            )
            .unwrap();
            ArcweftRuntimeExecutor::from_admitted_awbc_product_flow_invocation(
                admitted,
                compiled
                    .plan
                    .clone()
                    .seal_flow_invocation(flow.clone(), [])
                    .unwrap(),
            )
            .unwrap()
        };
        let mut executor = fresh();
        let mut saves = 0;
        for _ in 0..256 {
            let result = executor.step(
                RuntimeStepInput::default(),
                RuntimeStepOptions {
                    mode: RuntimeStepMode::OneOp,
                    budget: RuntimeStepBudget { max_ops: 1 },
                    ..Default::default()
                },
            );
            assert!(result.output.diagnostics.is_empty(), "{:?}", result.output);
            assert!(result.stats.executed_ops <= 1);
            let ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot) =
                executor.snapshot().unwrap();
            let saved = AwbcProductExecutorSaveSnapshot::from_live(
                &snapshot,
                executor.admitted_product_awbc_program().unwrap(),
            )
            .unwrap();
            let decoded = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
            assert_eq!(saved, decoded);
            drop(executor);
            let cold = fresh();
            let bound = cold.bind_product_save_snapshot(decoded).unwrap();
            executor = cold
                .restore_snapshot(ArcweftRuntimeExecutorSnapshot::AwbcProduct(bound))
                .unwrap();
            saves += 1;
            match &executor.fiber().frame.status {
                FlowFiberStatus::Running => {}
                FlowFiberStatus::Done(exit) => {
                    assert_eq!(exit, &FlowExit::Return("42".to_owned()));
                    assert!(saves > 1);
                    break;
                }
                status => panic!("transfer stopped: {status:?}"),
            }
        }
        assert!(matches!(
            executor.fiber().frame.status,
            FlowFiberStatus::Done(_)
        ));
    }
}
