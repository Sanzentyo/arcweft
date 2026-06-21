use super::options::CliRuntimeExecutorTier;
use arcweft_core::aot::AotProgram;
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_core::executor::{AotExecutor, BytecodeVmExecutor, RuntimeExecutor};
use arcweft_core::plan::RuntimePlan;
use arcweft_core::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepResult};
use arcweft_core::value::RuntimeBinding;
use arcweft_runtime_accelerator::{RuntimePureAccelerator, RuntimePureAcceleratorConfig};
use arcweft_runtime_host::{RuntimeExecutorStats, runtime_executor_stats};

pub(in crate::app) enum RuntimeExecutorInstance {
    BytecodeVm {
        executor: BytecodeVmExecutor,
        pure: RuntimePureAccelerator,
    },
    Aot {
        executor: AotExecutor,
        pure: RuntimePureAccelerator,
    },
}

pub(in crate::app) enum RuntimeExecutorCore {
    BytecodeVm(BytecodeVmExecutor),
    Aot(AotExecutor),
}

pub(in crate::app) enum RuntimeExecutorTemplate {
    BytecodeVm {
        plan: RuntimePlan,
        program: BytecodeProgram,
    },
    Aot {
        plan: RuntimePlan,
        program: AotProgram,
    },
}

impl RuntimeExecutorTemplate {
    pub(in crate::app) fn new(plan: &RuntimePlan, tier: CliRuntimeExecutorTier) -> Self {
        match tier {
            CliRuntimeExecutorTier::BytecodeVm => Self::BytecodeVm {
                plan: plan.clone(),
                program: BytecodeProgram::from_runtime_plan(plan.clone()),
            },
            CliRuntimeExecutorTier::Aot => Self::Aot {
                plan: plan.clone(),
                program: AotProgram::from_runtime_plan(plan),
            },
        }
    }

    pub(in crate::app) fn instantiate(&self) -> RuntimeExecutorCore {
        match self {
            Self::BytecodeVm { plan, program } => RuntimeExecutorCore::BytecodeVm(
                BytecodeVmExecutor::from_parts(program.clone(), plan.clone()),
            ),
            Self::Aot { plan, program } => {
                RuntimeExecutorCore::Aot(AotExecutor::from_parts(program.clone(), plan.clone()))
            }
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
        match self {
            Self::BytecodeVm(executor) => executor.step_with_root_bindings_and_pure_backend(
                input,
                root_bindings,
                options,
                pure,
            ),
            Self::Aot(executor) => executor.step_with_root_bindings_and_pure_backend(
                input,
                root_bindings,
                options,
                pure,
            ),
        }
    }

    pub(in crate::app) fn fast_path_ops(&self) -> usize {
        match self {
            Self::BytecodeVm(_) => 0,
            Self::Aot(executor) => executor.fast_path_ops(),
        }
    }
}

impl RuntimeExecutorInstance {
    pub(in crate::app) fn new(
        plan: RuntimePlan,
        tier: CliRuntimeExecutorTier,
        pure_config: RuntimePureAcceleratorConfig,
    ) -> Self {
        let pure = RuntimePureAccelerator::with_config(pure_config, &plan.pure_helpers);
        match tier {
            CliRuntimeExecutorTier::BytecodeVm => Self::BytecodeVm {
                executor: BytecodeVmExecutor::from_runtime_plan(plan),
                pure,
            },
            CliRuntimeExecutorTier::Aot => Self::Aot {
                executor: AotExecutor::new(plan),
                pure,
            },
        }
    }

    pub(in crate::app) fn step_with_root_bindings(
        &mut self,
        input: RuntimeStepInput,
        root_bindings: &[RuntimeBinding],
        options: RuntimeStepOptions,
    ) -> RuntimeStepResult {
        match self {
            Self::BytecodeVm { executor, pure } => executor
                .step_with_root_bindings_and_pure_backend(input, root_bindings, options, pure),
            Self::Aot { executor, pure } => executor.step_with_root_bindings_and_pure_backend(
                input,
                root_bindings,
                options,
                pure,
            ),
        }
    }

    pub(in crate::app) fn fiber(&self) -> &arcweft_core::engine::FlowFiber {
        match self {
            Self::BytecodeVm { executor, .. } => executor.fiber(),
            Self::Aot { executor, .. } => executor.fiber(),
        }
    }

    pub(in crate::app) fn executor_stats(&self) -> RuntimeExecutorStats {
        match self {
            Self::BytecodeVm { pure, .. } => runtime_executor_stats(0, pure),
            Self::Aot { executor, pure } => runtime_executor_stats(executor.fast_path_ops(), pure),
        }
    }
}
