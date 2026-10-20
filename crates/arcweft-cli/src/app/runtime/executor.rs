use super::options::CliRuntimeExecutorTier;
use arcweft_core::awbc::product_step::AwbcProductProgram;
use arcweft_core::awbc::schema::AwbcEntryId;
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
    invocation: RuntimeFlowInvocation,
    tier: ArcweftExecutionTier,
    product: Option<AwbcProductProgram>,
}

impl RuntimeExecutorTemplate {
    pub(in crate::app) fn new(
        invocation: RuntimeFlowInvocation,
        tier: CliRuntimeExecutorTier,
        product: Option<AwbcProductProgram>,
    ) -> Self {
        Self {
            invocation,
            tier: arcweft_execution_tier(tier),
            product,
        }
    }

    pub(in crate::app) fn instantiate(&self) -> Result<RuntimeExecutorCore, String> {
        let invocation = self
            .invocation
            .try_repeat_unrestricted()
            .map_err(|error| error.to_string())?;
        let executor = match self.tier {
            ArcweftExecutionTier::AwbcProduct => {
                ArcweftRuntimeExecutor::from_admitted_awbc_product_flow_invocation(
                    self.product.clone().ok_or_else(|| {
                        "Product AWBC execution requires the compiled executable".to_owned()
                    })?,
                    invocation,
                )
                .map_err(|error| error.to_string())?
            }
            tier => ArcweftRuntimeExecutor::from_runtime_flow_invocation(invocation, tier)
                .map_err(|error| error.to_string())?,
        };
        Ok(RuntimeExecutorCore { executor })
    }

    pub(in crate::app) fn pure_backend(
        &self,
        config: RuntimePureAcceleratorConfig,
    ) -> Result<RuntimePureAccelerator, String> {
        match self.tier {
            ArcweftExecutionTier::AwbcProduct => Ok(RuntimePureAccelerator::with_awbc_program(
                config,
                self.product.clone().ok_or_else(|| {
                    "Product AWBC execution requires the compiled executable".to_owned()
                })?,
            )),
            _ => Ok(RuntimePureAccelerator::with_config(
                config,
                self.invocation.plan(),
            )),
        }
    }
}

impl RuntimeExecutorCore {
    pub(in crate::app) fn program_owner(&self) -> arcweft_core::task::RuntimeProgramOwner {
        self.executor.program_owner()
    }

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
    pub(in crate::app) fn program_owner(&self) -> arcweft_core::task::RuntimeProgramOwner {
        self.executor.program_owner()
    }

    pub(in crate::app) fn new(
        plan: RuntimePlan,
        entry: &EntryRuntimeId,
        tier: CliRuntimeExecutorTier,
        pure_config: RuntimePureAcceleratorConfig,
        product: Option<AwbcProductProgram>,
    ) -> Result<Self, String> {
        let plan = Arc::new(plan);
        let (executor, pure) = match arcweft_execution_tier(tier) {
            ArcweftExecutionTier::AwbcProduct => {
                let product = product.ok_or_else(|| {
                    "Product AWBC execution requires the compiled executable".to_owned()
                })?;
                let index = product
                    .entries
                    .iter()
                    .position(|candidate| &candidate.runtime_id == entry)
                    .ok_or_else(|| {
                        format!("Product AWBC has no entry `{}`", entry.canonical_label())
                    })?;
                let index = u32::try_from(index)
                    .map_err(|_| "Product AWBC entry index exceeds u32".to_owned())?;
                let pure = RuntimePureAccelerator::with_awbc_program(pure_config, product.clone());
                let executor = ArcweftRuntimeExecutor::from_admitted_awbc_product(
                    product,
                    AwbcEntryId(index),
                    arcweft_core::task::GenerationId::new(0),
                )
                .map_err(|error| error.to_string())?;
                (executor, pure)
            }
            tier => {
                let pure = RuntimePureAccelerator::with_config(pure_config, &plan);
                let mut executor = ArcweftRuntimeExecutor::from_runtime_plan(plan, tier)
                    .map_err(|error| error.to_string())?;
                executor
                    .start_structured_entry(entry)
                    .map_err(|error| error.to_string())?;
                (executor, pure)
            }
        };
        Ok(Self { executor, pure })
    }

    pub(in crate::app) fn from_flow_invocation(
        invocation: RuntimeFlowInvocation,
        tier: CliRuntimeExecutorTier,
        pure_config: RuntimePureAcceleratorConfig,
        product: Option<AwbcProductProgram>,
    ) -> Result<Self, String> {
        let (executor, pure) = match arcweft_execution_tier(tier) {
            ArcweftExecutionTier::AwbcProduct => {
                let product = product.ok_or_else(|| {
                    "Product AWBC execution requires the compiled executable".to_owned()
                })?;
                let pure = RuntimePureAccelerator::with_awbc_program(pure_config, product.clone());
                let executor = ArcweftRuntimeExecutor::from_admitted_awbc_product_flow_invocation(
                    product, invocation,
                )
                .map_err(|error| error.to_string())?;
                (executor, pure)
            }
            tier => {
                let pure = RuntimePureAccelerator::with_config(pure_config, invocation.plan());
                let executor =
                    ArcweftRuntimeExecutor::from_runtime_flow_invocation(invocation, tier)
                        .map_err(|error| error.to_string())?;
                (executor, pure)
            }
        };
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
        CliRuntimeExecutorTier::AwbcProduct => ArcweftExecutionTier::AwbcProduct,
        CliRuntimeExecutorTier::BytecodeVm => ArcweftExecutionTier::RuntimePlanVm,
        CliRuntimeExecutorTier::Aot => ArcweftExecutionTier::StructuredAot,
    }
}

#[cfg(test)]
mod tests;
