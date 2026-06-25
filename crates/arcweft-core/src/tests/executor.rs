use crate::aot::{AotDispatchShape, AotProgram};
use crate::bytecode::BytecodeProgram;
use crate::effect::{LineEffectRequest, RuntimeLog};
use crate::executor::{
    AotExecutor, ArcweftExecutionTier, ArcweftRuntimeExecutor, BytecodeVmExecutor, RuntimeExecutor,
    VmExecutor,
};
use crate::plan::{
    FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan, RuntimePureHelper, RuntimePureHelperId,
    RuntimePureHelperOrigin, RuntimePureOutputType,
};
use crate::step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions};
use crate::task::{AwaitTarget, HostCapabilityId, HostTaskRequestTemplate, NeedId, TaskId};
use crate::value::{RuntimeExpr, RuntimeValue};

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
    assert_eq!(aot.program().stats().flows, 1);
    assert_eq!(aot.program().stats().linear_dispatch_flows, 1);

    let vm_result = vm.step(RuntimeStepInput::default(), options);
    let aot_result = aot.step(RuntimeStepInput::default(), options);

    assert_eq!(aot_result, vm_result);
    assert_eq!(aot.fiber().status, vm.fiber().status);
}

#[test]
fn aot_program_records_nested_dispatch_shape() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![
                FlowOp::Noop,
                FlowOp::If {
                    condition: crate::value::RuntimeExpr::Value(crate::value::RuntimeValue::Bool(
                        true,
                    )),
                    then_ops: vec![FlowOp::Return("then".to_owned())],
                    else_ops: vec![FlowOp::Await {
                        binding: None,
                        target: AwaitTarget {
                            need: NeedId("need.ready".to_owned()),
                            task: TaskId("task.ready".to_owned()),
                            request: HostTaskRequestTemplate {
                                capability: HostCapabilityId("clock".to_owned()),
                                operation: "ready".to_owned(),
                                args: Vec::new(),
                            },
                        },
                        pending: Vec::new(),
                    }],
                },
            ],
        }],
        Vec::new(),
    )
    .expect("plan is valid")
    .with_pure_helpers(vec![RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "one".to_owned(),
        input_names: Vec::new(),
        input_types: Vec::new(),
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Value(RuntimeValue::i64(1)),
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);

    let program = AotProgram::from_runtime_plan(&plan);
    assert_eq!(program.flows().len(), 1);
    assert_eq!(program.flows()[0].dispatch, AotDispatchShape::Mixed);
    assert_eq!(program.flows()[0].linear_prefix_ops, 1);
    assert_eq!(program.flows()[0].lowered_linear_ops(), 1);
    assert_eq!(program.stats().flows, 1);
    assert_eq!(program.stats().ops, 4);
    assert_eq!(program.stats().linear_ops, 2);
    assert_eq!(program.stats().branch_ops, 1);
    assert_eq!(program.stats().await_ops, 1);
    assert_eq!(program.stats().mixed_dispatch_flows, 1);
}

#[test]
fn aot_executor_uses_fast_path_for_supported_linear_flow() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![
                FlowOp::Noop,
                FlowOp::Effect(LineEffectRequest::Log(RuntimeLog {
                    level: "info".to_owned(),
                    message: "fast".to_owned(),
                    fields: Vec::new(),
                })),
                FlowOp::Return("done".to_owned()),
            ],
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
    assert_eq!(aot.program().flows()[0].lowered_linear_ops(), 3);

    let vm_result = vm.step(RuntimeStepInput::default(), options);
    let aot_result = aot.step(RuntimeStepInput::default(), options);

    assert_eq!(aot_result, vm_result);
    assert_eq!(aot.fast_path_ops(), 3);
}

#[test]
fn aot_executor_falls_back_for_control_effect_flow() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![
            RuntimeFlow {
                id: FlowRuntimeId("flow.main".to_owned()),
                ops: vec![
                    FlowOp::Effect(LineEffectRequest::Goto("flow.next".to_owned())),
                    FlowOp::Return("unreachable".to_owned()),
                ],
            },
            RuntimeFlow {
                id: FlowRuntimeId("flow.next".to_owned()),
                ops: vec![FlowOp::Return("done".to_owned())],
            },
        ],
        Vec::new(),
    )
    .expect("plan is valid");
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::Drain,
        budget: RuntimeStepBudget { max_ops: 8 },
    };
    let mut vm = VmExecutor::new(plan.clone());
    let mut aot = AotExecutor::new(plan);

    assert_eq!(aot.program().flows()[0].dispatch, AotDispatchShape::Mixed);

    let vm_result = vm.step(RuntimeStepInput::default(), options);
    let aot_result = aot.step(RuntimeStepInput::default(), options);

    assert_eq!(aot_result, vm_result);
    assert_eq!(aot.fast_path_ops(), 0);
}

#[test]
fn aot_executor_falls_back_for_branching_flow() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![FlowOp::If {
                condition: crate::value::RuntimeExpr::Value(crate::value::RuntimeValue::Bool(true)),
                then_ops: vec![FlowOp::Return("then".to_owned())],
                else_ops: vec![FlowOp::Return("else".to_owned())],
            }],
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
    assert_eq!(aot.fast_path_ops(), 0);
}

#[test]
fn aot_executor_runs_mixed_flow_linear_prefix_before_vm_compatible_dispatch() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![
                FlowOp::Noop,
                FlowOp::If {
                    condition: crate::value::RuntimeExpr::Value(crate::value::RuntimeValue::Bool(
                        true,
                    )),
                    then_ops: vec![FlowOp::Return("then".to_owned())],
                    else_ops: vec![FlowOp::Return("else".to_owned())],
                },
            ],
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

    assert_eq!(aot.program().flows()[0].dispatch, AotDispatchShape::Mixed);
    assert_eq!(aot.program().flows()[0].lowered_linear_ops(), 1);

    let vm_result = vm.step(RuntimeStepInput::default(), options);
    let aot_result = aot.step(RuntimeStepInput::default(), options);
    assert_eq!(aot_result.stats.executed_ops, 4);
    assert_eq!(aot.fast_path_ops(), 1);

    assert_eq!(aot_result.output, vm_result.output);
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

#[test]
fn runtime_executor_facade_matches_structured_vm_and_aot_boundaries() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![
                FlowOp::Noop,
                FlowOp::Effect(LineEffectRequest::Log(RuntimeLog {
                    level: "info".to_owned(),
                    message: "facade".to_owned(),
                    fields: Vec::new(),
                })),
                FlowOp::Return("done".to_owned()),
            ],
        }],
        Vec::new(),
    )
    .expect("plan is valid");
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::Drain,
        budget: RuntimeStepBudget { max_ops: 8 },
    };
    let mut vm = VmExecutor::new(plan.clone());
    let mut facade_vm =
        ArcweftRuntimeExecutor::from_runtime_plan(plan.clone(), ArcweftExecutionTier::StructuredVm);
    let mut facade_aot =
        ArcweftRuntimeExecutor::from_runtime_plan(plan, ArcweftExecutionTier::StructuredAot);

    let vm_result = vm.step(RuntimeStepInput::default(), options);
    let facade_vm_result = facade_vm.step(RuntimeStepInput::default(), options);
    let facade_aot_result = facade_aot.step(RuntimeStepInput::default(), options);

    assert_eq!(facade_vm.tier(), ArcweftExecutionTier::StructuredVm);
    assert_eq!(facade_aot.tier(), ArcweftExecutionTier::StructuredAot);
    assert_eq!(facade_vm_result, vm_result);
    assert_eq!(facade_aot_result.output, vm_result.output);
    assert_eq!(facade_aot.fiber().status, vm.fiber().status);
    assert_eq!(facade_aot.fast_path_ops(), 3);
}
