use std::sync::Arc;

use arcweft_compiler::source::compile_source;
use arcweft_core::{
    awbc::{codec::AwbcDecodeBudget, product_step::AwbcProductStepExecutor, schema::AwbcProgram},
    engine::{Engine, FlowExit, FlowFiberStatus},
    plan::RuntimeEntryTarget,
    pure::VmRuntimePureCallBackend,
    step::{RuntimeStepInput, RuntimeStepOptions},
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
        let mut engine = Engine::for_flow(compiled.plan.clone(), flow).unwrap();
        for _ in 0..256 {
            let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
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
        let program = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
        let mut executor = AwbcProductStepExecutor::for_entry_arc(
            Arc::new(program),
            arcweft_core::awbc::schema::AwbcEntryId(0),
            1,
        )
        .unwrap();
        let mut backend = VmRuntimePureCallBackend::default();
        for _ in 0..256 {
            let result = executor.step_with_pure_backend(
                RuntimeStepInput::default(),
                RuntimeStepOptions::default(),
                &mut backend,
            );
            assert!(result.output.diagnostics.is_empty(), "{:?}", result.output);
            match &executor.fiber().frame.status {
                FlowFiberStatus::Running => {}
                FlowFiberStatus::Done(exit) => {
                    assert_eq!(exit, &FlowExit::Return("42".to_owned()));
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
