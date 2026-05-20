use crate::executor::{AotExecutor, RuntimeExecutor, VmExecutor};
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
