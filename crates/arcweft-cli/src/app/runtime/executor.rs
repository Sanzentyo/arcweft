use super::options::CliRuntimeExecutorTier;
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::plan::RuntimePlan;
use arcweft_core::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepResult};
use arcweft_core::value::RuntimeBinding;
use arcweft_runtime_accelerator::{RuntimePureAccelerator, RuntimePureAcceleratorConfig};
use arcweft_runtime_host::{RuntimeExecutorStats, runtime_executor_stats};

pub(in crate::app) struct RuntimeExecutorInstance {
    executor: ArcweftRuntimeExecutor,
    pure: RuntimePureAccelerator,
}

pub(in crate::app) struct RuntimeExecutorCore {
    executor: ArcweftRuntimeExecutor,
}

pub(in crate::app) struct RuntimeExecutorTemplate {
    plan: RuntimePlan,
    tier: ArcweftExecutionTier,
}

impl RuntimeExecutorTemplate {
    pub(in crate::app) fn new(plan: &RuntimePlan, tier: CliRuntimeExecutorTier) -> Self {
        Self {
            plan: plan.clone(),
            tier: arcweft_execution_tier(tier),
        }
    }

    pub(in crate::app) fn instantiate(&self) -> RuntimeExecutorCore {
        RuntimeExecutorCore {
            executor: ArcweftRuntimeExecutor::from_runtime_plan(self.plan.clone(), self.tier),
        }
    }
}

impl RuntimeExecutorCore {
    pub(in crate::app) fn step_with_root_bindings(
        &mut self,
        input: RuntimeStepInput,
        root_bindings: &[RuntimeBinding],
        options: RuntimeStepOptions,
        pure: &mut RuntimePureAccelerator,
    ) -> RuntimeStepResult {
        self.executor
            .step_with_root_bindings_and_pure_backend(input, root_bindings, options, pure)
    }

    pub(in crate::app) fn fast_path_ops(&self) -> usize {
        self.executor.fast_path_ops()
    }
}

impl RuntimeExecutorInstance {
    pub(in crate::app) fn new(
        plan: RuntimePlan,
        tier: CliRuntimeExecutorTier,
        pure_config: RuntimePureAcceleratorConfig,
    ) -> Self {
        let pure = RuntimePureAccelerator::with_config(pure_config, &plan.pure_helpers);
        Self {
            executor: ArcweftRuntimeExecutor::from_runtime_plan(plan, arcweft_execution_tier(tier)),
            pure,
        }
    }

    pub(in crate::app) fn step_with_root_bindings(
        &mut self,
        input: RuntimeStepInput,
        root_bindings: &[RuntimeBinding],
        options: RuntimeStepOptions,
    ) -> RuntimeStepResult {
        self.executor.step_with_root_bindings_and_pure_backend(
            input,
            root_bindings,
            options,
            &mut self.pure,
        )
    }

    pub(in crate::app) fn fiber(&self) -> &arcweft_core::engine::FlowFiber {
        self.executor.fiber()
    }

    pub(in crate::app) fn executor_stats(&self) -> RuntimeExecutorStats {
        runtime_executor_stats(self.executor.fast_path_ops(), &self.pure)
    }
}

const fn arcweft_execution_tier(tier: CliRuntimeExecutorTier) -> ArcweftExecutionTier {
    match tier {
        CliRuntimeExecutorTier::AwbcProduct | CliRuntimeExecutorTier::BytecodeVm => {
            ArcweftExecutionTier::StructuredVm
        }
        CliRuntimeExecutorTier::Aot => ArcweftExecutionTier::StructuredAot,
    }
}
