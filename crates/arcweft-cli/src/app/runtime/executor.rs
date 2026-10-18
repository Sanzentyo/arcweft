use super::options::CliRuntimeExecutorTier;
use arcweft_core::awbc::schema::{AwbcEntryId, AwbcProgram};
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::plan::{EntryRuntimeId, RuntimeFlowInvocation, RuntimePlan};
use arcweft_core::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepResult};
use arcweft_core::value::RuntimeDialoguePlainTextContextTemplateProof;
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
    product: Option<Arc<AwbcProgram>>,
    plain_text_context: Option<RuntimeDialoguePlainTextContextTemplateProof>,
}

impl RuntimeExecutorTemplate {
    pub(in crate::app) fn new(
        invocation: RuntimeFlowInvocation,
        tier: CliRuntimeExecutorTier,
        product: Option<Arc<AwbcProgram>>,
        plain_text_context: Option<RuntimeDialoguePlainTextContextTemplateProof>,
    ) -> Self {
        Self {
            invocation,
            tier: arcweft_execution_tier(tier),
            product,
            plain_text_context,
        }
    }

    pub(in crate::app) fn instantiate(&self) -> Result<RuntimeExecutorCore, String> {
        let invocation = self
            .invocation
            .try_repeat_unrestricted()
            .map_err(|error| error.to_string())?;
        let executor = match self.tier {
            ArcweftExecutionTier::AwbcProduct => {
                ArcweftRuntimeExecutor::from_awbc_product_flow_invocation(
                    self.product.as_ref().map(Arc::clone).ok_or_else(|| {
                        "Product AWBC execution requires the compiled executable".to_owned()
                    })?,
                    invocation,
                    self.plain_text_context,
                )
                .map_err(|error| error.to_string())?
            }
            tier => ArcweftRuntimeExecutor::from_runtime_flow_invocation(invocation, tier)
                .map_err(|error| error.to_string())?,
        };
        Ok(RuntimeExecutorCore { executor })
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
        product: Option<Arc<AwbcProgram>>,
        plain_text_context: Option<RuntimeDialoguePlainTextContextTemplateProof>,
    ) -> Result<Self, String> {
        let plan = Arc::new(plan);
        let pure = RuntimePureAccelerator::with_config(pure_config, &plan);
        let executor = match arcweft_execution_tier(tier) {
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
                match plain_text_context {
                    Some(proof) => {
                        ArcweftRuntimeExecutor::from_awbc_product_arc_with_plain_text_context_proof(
                            product,
                            AwbcEntryId(index),
                            proof,
                        )
                    }
                    None => {
                        ArcweftRuntimeExecutor::from_awbc_product_arc(product, AwbcEntryId(index))
                    }
                }
                .map_err(|error| error.to_string())?
            }
            tier => {
                let mut executor = ArcweftRuntimeExecutor::from_runtime_plan(plan, tier)
                    .map_err(|error| error.to_string())?;
                executor
                    .start_structured_entry(entry)
                    .map_err(|error| error.to_string())?;
                executor
            }
        };
        Ok(Self { executor, pure })
    }

    pub(in crate::app) fn from_flow_invocation(
        invocation: RuntimeFlowInvocation,
        tier: CliRuntimeExecutorTier,
        pure_config: RuntimePureAcceleratorConfig,
        product: Option<Arc<AwbcProgram>>,
        plain_text_context: Option<RuntimeDialoguePlainTextContextTemplateProof>,
    ) -> Result<Self, String> {
        let pure = RuntimePureAccelerator::with_config(pure_config, invocation.plan());
        let executor = match arcweft_execution_tier(tier) {
            ArcweftExecutionTier::AwbcProduct => {
                ArcweftRuntimeExecutor::from_awbc_product_flow_invocation(
                    product.ok_or_else(|| {
                        "Product AWBC execution requires the compiled executable".to_owned()
                    })?,
                    invocation,
                    plain_text_context,
                )
                .map_err(|error| error.to_string())?
            }
            tier => ArcweftRuntimeExecutor::from_runtime_flow_invocation(invocation, tier)
                .map_err(|error| error.to_string())?,
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
