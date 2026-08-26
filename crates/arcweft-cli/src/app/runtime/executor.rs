use super::options::CliRuntimeExecutorTier;
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::plan::{EntryRuntimeId, RuntimeFlowInvocation, RuntimePlan};
use arcweft_core::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepResult};
use arcweft_runtime_accelerator::{RuntimePureAccelerator, RuntimePureAcceleratorConfig};
use arcweft_runtime_host::{RuntimeExecutorStats, runtime_executor_stats};
use std::sync::Arc;

pub(in crate::app) struct RuntimeExecutorInstance {
    executor: ArcweftRuntimeExecutor,
    pure: RuntimePureAccelerator,
}

pub(in crate::app) struct RuntimeExecutorCore {
    executor: ArcweftRuntimeExecutor,
}

pub(in crate::app) struct RuntimeExecutorTemplate {
    plan: RuntimePlan,
    entry: EntryRuntimeId,
    tier: ArcweftExecutionTier,
}

impl RuntimeExecutorTemplate {
    pub(in crate::app) fn new(
        plan: &RuntimePlan,
        entry: EntryRuntimeId,
        tier: CliRuntimeExecutorTier,
    ) -> Self {
        Self {
            plan: plan.clone(),
            entry,
            tier: arcweft_execution_tier(tier),
        }
    }

    pub(in crate::app) fn instantiate(&self) -> Result<RuntimeExecutorCore, String> {
        let mut executor = ArcweftRuntimeExecutor::from_runtime_plan(self.plan.clone(), self.tier)
            .map_err(|error| error.to_string())?;
        executor
            .start_structured_entry(&self.entry)
            .map_err(|error| error.to_string())?;
        Ok(RuntimeExecutorCore { executor })
    }
}

impl RuntimeExecutorCore {
    pub(in crate::app) fn step(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
        pure: &mut RuntimePureAccelerator,
    ) -> RuntimeStepResult {
        self.executor.step_with_pure_backend(input, options, pure)
    }

    pub(in crate::app) fn fast_path_ops(&self) -> usize {
        self.executor.fast_path_ops()
    }
}

impl RuntimeExecutorInstance {
    pub(in crate::app) fn new(
        plan: RuntimePlan,
        entry: &EntryRuntimeId,
        tier: CliRuntimeExecutorTier,
        pure_config: RuntimePureAcceleratorConfig,
    ) -> Result<Self, String> {
        let plan = Arc::new(plan);
        let pure = RuntimePureAccelerator::with_config(pure_config, &plan);
        let mut executor = ArcweftRuntimeExecutor::from_runtime_plan(
            Arc::unwrap_or_clone(plan),
            arcweft_execution_tier(tier),
        )
        .map_err(|error| error.to_string())?;
        executor
            .start_structured_entry(entry)
            .map_err(|error| error.to_string())?;
        Ok(Self { executor, pure })
    }

    pub(in crate::app) fn from_flow_invocation(
        invocation: RuntimeFlowInvocation,
        tier: CliRuntimeExecutorTier,
        pure_config: RuntimePureAcceleratorConfig,
    ) -> Result<Self, String> {
        let pure = RuntimePureAccelerator::with_config(pure_config, invocation.plan());
        let executor = ArcweftRuntimeExecutor::from_runtime_flow_invocation(
            invocation,
            arcweft_execution_tier(tier),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self { executor, pure })
    }

    pub(in crate::app) fn step(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
    ) -> RuntimeStepResult {
        self.executor
            .step_with_pure_backend(input, options, &mut self.pure)
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
            ArcweftExecutionTier::RuntimePlanVm
        }
        CliRuntimeExecutorTier::Aot => ArcweftExecutionTier::StructuredAot,
    }
}
