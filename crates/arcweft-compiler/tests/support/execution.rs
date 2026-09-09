use arcweft_compiler::source::compile_source;
use arcweft_core::awbc::fiber::FiberState;
use arcweft_core::awbc::schema::AwbcEntryId;
use arcweft_core::awbc::vm::{self, VmExit, VmStepOptions};
use arcweft_core::engine::{Engine, FlowExit, FlowFiberStatus};
use arcweft_core::step::{RuntimeStepInput, RuntimeStepOptions};
use arcweft_core::value::RuntimeValue;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

pub(super) fn assert_native_return(source: &str, expected: &str) {
    let compiled = compile_source(source).expect("callable program compiles");
    let [flow] = compiled.plan.flows() else {
        panic!("fixture must contain exactly its main flow");
    };
    let flow = flow.id.clone();
    let mut engine = Engine::for_flow(compiled.plan, &flow).expect("main flow starts");
    for _ in 0..256 {
        let output = engine
            .step(RuntimeStepInput::default(), RuntimeStepOptions::default())
            .output;
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        match &engine.fiber().status {
            FlowFiberStatus::Running => {}
            FlowFiberStatus::Done(exit) => {
                assert_eq!(exit, &FlowExit::Return(expected.to_owned()));
                return;
            }
            status => panic!("callable execution stopped unexpectedly: {status:?}"),
        }
    }
    panic!("callable execution exceeded its deterministic step limit");
}

pub(super) fn assert_awbc_return(source: &str, expected: RuntimeValue) {
    let compiled = compile_source(source).expect("callable program compiles");
    let report = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "callable_execution.arcw",
    )
    .lower()
    .expect("callable program lowers to verified AWBC");
    let encoded = report
        .program
        .encode_canonical()
        .expect("callable AWBC encodes");
    let program = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
        &encoded,
        arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
    )
    .expect("callable AWBC decodes");
    assert_eq!(program, report.program);
    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 1, 65_536).expect("main entry starts");
    for _ in 0..256 {
        let output = vm::step(&program, &mut fiber, VmStepOptions::default())
            .expect("AWBC callable step succeeds");
        match output.exit {
            VmExit::Running => {}
            VmExit::Returned(value) => {
                assert_eq!(value, Some(expected));
                return;
            }
            exit => panic!("AWBC callable execution stopped unexpectedly: {exit:?}"),
        }
    }
    panic!("AWBC callable execution exceeded its deterministic step limit");
}
