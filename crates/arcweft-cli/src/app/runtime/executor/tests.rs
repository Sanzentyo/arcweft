use super::*;
use crate::app::runtime::invocation::seal_named_flow_invocation;
use arcweft_core::awbc::codec::AwbcDecodeBudget;
use arcweft_core::awbc::schema::AwbcProgram;
use arcweft_core::engine::{FlowExit, FlowFiberStatus};
use arcweft_core::step::{RuntimeStepBudget, RuntimeStepMode};
use arcweft_core::task::RuntimeProgramOwner;
use arcweft_core::value::{RuntimeBinding, RuntimeValue};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use std::sync::Arc;

const SOURCE: &str = r#"
entry cli @entry.main { goto @flow.launch }
flow launch() -> String { return "entry_result" }
flow selected(message: String, enabled: bool) -> String {
    return if enabled { message } else { "wrong_branch" }
}
"#;

fn compile(source: &str) -> (RuntimePlan, AwbcProductProgram) {
    let compiled = arcweft_compiler::source::compile_source(source).unwrap();
    let program = AwbcLowerer::new(&compiled.plan, &compiled.dialogue_content, "executor.arcw")
        .lower()
        .unwrap()
        .program;
    let decoded = AwbcProgram::decode_canonical(
        &program.encode_canonical().unwrap(),
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    let program = AwbcProductProgram::admit(
        Arc::new(decoded),
        arcweft_core::awbc::verify::AwbcVerifyBudget::default(),
        arcweft_core::awbc::verify::AwbcVerifyContext::default(),
        arcweft_core::plan::RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
    .unwrap();
    (compiled.plan, program)
}

fn options() -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: RuntimeStepMode::Drain,
        budget: RuntimeStepBudget { max_ops: 64 },
        ..RuntimeStepOptions::default()
    }
}

fn finish(executor: &mut RuntimeExecutorInstance) -> &FlowFiberStatus {
    for _ in 0..4 {
        let result = executor.step(RuntimeStepInput::default(), options());
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        if matches!(
            executor.fiber().frame.status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        ) {
            return &executor.fiber().frame.status;
        }
    }
    panic!("executor did not finish within the fixture's operation allowance");
}

fn invocation(plan: RuntimePlan) -> RuntimeFlowInvocation {
    seal_named_flow_invocation(
        plan,
        "flow.selected",
        &[
            RuntimeBinding {
                name: "enabled".to_owned(),
                value: RuntimeValue::Bool(true),
            },
            RuntimeBinding {
                name: "message".to_owned(),
                value: RuntimeValue::String("flow_result".to_owned()),
            },
        ],
    )
    .unwrap()
}

#[test]
fn runtime_executor_selection_starts_the_actual_tier_and_original_product_entry() {
    let (plan, product) = compile(SOURCE);
    let entry = plan.entries()[0].id.clone();
    for (tier, expected) in [
        (
            CliRuntimeExecutorTier::BytecodeVm,
            ArcweftExecutionTier::RuntimePlanVm,
        ),
        (
            CliRuntimeExecutorTier::Aot,
            ArcweftExecutionTier::StructuredAot,
        ),
        (
            CliRuntimeExecutorTier::AwbcProduct,
            ArcweftExecutionTier::AwbcProduct,
        ),
    ] {
        let mut executor = RuntimeExecutorInstance::new(
            plan.clone(),
            &entry,
            tier,
            RuntimePureAcceleratorConfig::default(),
            Some(product.clone()),
        )
        .unwrap();
        assert_eq!(executor.executor.tier(), expected);
        if tier == CliRuntimeExecutorTier::AwbcProduct {
            assert!(
                executor
                    .program_owner()
                    .same_program(&RuntimeProgramOwner::Awbc(product.program_arc()))
            );
        }
        assert_eq!(
            finish(&mut executor),
            &FlowFiberStatus::Done(FlowExit::Return("entry_result".to_owned()))
        );
    }
}

#[test]
fn runtime_executor_product_flow_and_repeated_bench_preserve_original_program_and_inputs() {
    let (plan, product) = compile(SOURCE);
    let mut executor = RuntimeExecutorInstance::from_flow_invocation(
        invocation(plan.clone()),
        CliRuntimeExecutorTier::AwbcProduct,
        RuntimePureAcceleratorConfig::default(),
        Some(product.clone()),
    )
    .unwrap();
    let owner = RuntimeProgramOwner::Awbc(product.program_arc());
    assert_eq!(executor.executor.tier(), ArcweftExecutionTier::AwbcProduct);
    assert!(executor.program_owner().same_program(&owner));
    assert_eq!(
        finish(&mut executor),
        &FlowFiberStatus::Done(FlowExit::Return("flow_result".to_owned()))
    );

    let template = RuntimeExecutorTemplate::new(
        invocation(plan.clone()),
        CliRuntimeExecutorTier::AwbcProduct,
        Some(product),
    );
    let mut pure = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig::default(),
        template.invocation.plan(),
    );
    for _ in 0..2 {
        let mut executor = template.instantiate().unwrap();
        assert_eq!(executor.executor.tier(), ArcweftExecutionTier::AwbcProduct);
        assert!(executor.program_owner().same_program(&owner));
        for _ in 0..4 {
            let result = executor.step(RuntimeStepInput::default(), options(), &mut pure);
            assert!(
                result.output.diagnostics.is_empty(),
                "{:?}",
                result.output.diagnostics
            );
            if matches!(
                executor.executor.fiber().frame.status,
                FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
            ) {
                break;
            }
        }
        assert_eq!(
            executor.executor.fiber().frame.status,
            FlowFiberStatus::Done(FlowExit::Return("flow_result".to_owned()))
        );
    }
}

#[test]
fn runtime_executor_product_never_falls_back_for_missing_executable_or_foreign_flow_schema() {
    let (plan, _) = compile(SOURCE);
    let entry = plan.entries()[0].id.clone();
    assert!(
        RuntimeExecutorInstance::new(
            plan.clone(),
            &entry,
            CliRuntimeExecutorTier::AwbcProduct,
            RuntimePureAcceleratorConfig::default(),
            None,
        )
        .is_err()
    );
    assert!(
        RuntimeExecutorInstance::from_flow_invocation(
            invocation(plan.clone()),
            CliRuntimeExecutorTier::AwbcProduct,
            RuntimePureAcceleratorConfig::default(),
            None,
        )
        .is_err()
    );
    assert!(
        RuntimeExecutorTemplate::new(
            invocation(plan.clone()),
            CliRuntimeExecutorTier::AwbcProduct,
            None,
        )
        .instantiate()
        .is_err()
    );

    let (_, foreign) = compile(
        &SOURCE
            .replace("enabled: bool", "enabled: String")
            .replace("if enabled", "if enabled == \"yes\""),
    );
    assert!(
        RuntimeExecutorInstance::from_flow_invocation(
            invocation(plan),
            CliRuntimeExecutorTier::AwbcProduct,
            RuntimePureAcceleratorConfig::default(),
            Some(foreign),
        )
        .is_err()
    );
}
