use crate::bytecode::BytecodeProgram;
use crate::executor::{AotExecutor, BytecodeVmExecutor, RuntimeExecutor, VmExecutor};
use crate::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan};
use crate::step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions};

#[test]
fn aot_executor_matches_vm_executor_at_runtime_boundary() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![FlowOp::Return("done".to_owned())],
        }],
        Vec::new(),
    )
    .expect("plan is valid");
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::Drain,
        budget: RuntimeStepBudget { max_ops: 8 },
    };
    let mut vm = VmExecutor::new(plan.clone());
    let mut aot = AotExecutor::new(plan);

    let vm_result = vm.step(RuntimeStepInput::default(), options);
    let aot_result = aot.step(RuntimeStepInput::default(), options);

    assert_eq!(aot_result, vm_result);
    assert_eq!(aot.fiber().status, vm.fiber().status);
}

#[test]
fn bytecode_program_roundtrips_runtime_plan_and_matches_vm_executor() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![
                FlowOp::Noop,
                FlowOp::Bind(Vec::new()),
                FlowOp::Return("done".to_owned()),
            ],
        }],
        Vec::new(),
    )
    .expect("plan is valid");
    let bytecode = BytecodeProgram::from_runtime_plan(plan.clone());
    assert_eq!(bytecode.stats().flows, 1);
    assert_eq!(bytecode.stats().instructions, 3);
    assert_eq!(
        bytecode.clone().into_runtime_plan().expect("roundtrip"),
        plan
    );

    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::Drain,
        budget: RuntimeStepBudget { max_ops: 8 },
    };
    let mut vm = VmExecutor::new(plan);
    let mut bytecode_vm =
        BytecodeVmExecutor::new(bytecode).expect("bytecode program converts to runtime plan");

    let vm_result = vm.step(RuntimeStepInput::default(), options);
    let bytecode_result = bytecode_vm.step(RuntimeStepInput::default(), options);

    assert_eq!(bytecode_result, vm_result);
    assert_eq!(bytecode_vm.fiber().status, vm.fiber().status);
}
