use crate::aot::AotProgram;
use crate::awbc::product_step::{
    AwbcProductExecutorSnapshot, AwbcProductStepBuildError, AwbcProductStepExecutor,
};
use crate::awbc::schema::{AwbcEntryId, AwbcFunctionId, AwbcProgram};
use crate::bytecode::BytecodeProgram;
use crate::engine::{Engine, FlowFiber};
use crate::plan::{RuntimePlan, RuntimePlanError};
use crate::pure::RuntimeCallBackend;
use crate::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepResult};
use crate::value::RuntimeBinding;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Sans I/O execution boundary used by CLI, LSP, tests, and future adapters.
///
/// The trait is intentionally small: the VM remains the semantic source of
/// truth, while hosts can depend on this boundary instead of the concrete
/// engine type.
pub trait RuntimeExecutor {
    fn step(&mut self, input: RuntimeStepInput, options: RuntimeStepOptions) -> RuntimeStepResult;

    fn fiber(&self) -> &FlowFiber;
}

/// Runtime executor backed by the built-in Arcweft VM.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VmExecutor {
    engine: Engine,
}

/// AOT executor boundary backed by a typed AOT program artifact.
///
/// The current backend runs through the VM-compatible state machine after AOT
/// shape analysis. Generated dispatch can replace that backend without changing
/// host-facing executor selection.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AotExecutor {
    program: AotProgram,
    vm: VmExecutor,
    fast_path_ops: usize,
}

/// Runtime executor backed by a bytecode bundle.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BytecodeVmExecutor {
    vm: VmExecutor,
}

/// Runtime executor backed by canonical product AWBC.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AwbcProductExecutor {
    vm: AwbcProductStepExecutor,
}

/// Product-facing execution tier selected through the shared executor facade.
///
/// These tiers currently preserve the structured runtime behavior while the
/// product AWBC migration remains a separate cut. Keeping the variants here
/// prevents hosts from constructing low-level executors directly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcweftExecutionTier {
    StructuredVm,
    StructuredAot,
    AwbcProduct,
}

impl ArcweftExecutionTier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StructuredVm => "structured_vm",
            Self::StructuredAot => "structured_aot",
            Self::AwbcProduct => "awbc_product",
        }
    }

    #[must_use]
    pub const fn is_vm_first(self) -> bool {
        matches!(self, Self::StructuredVm | Self::AwbcProduct)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "tier", rename_all = "snake_case")]
pub enum ArcweftRuntimeExecutorSnapshot {
    AwbcProduct(AwbcProductExecutorSnapshot),
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ArcweftRuntimeExecutorSnapshotError {
    #[error("runtime executor tier `{tier}` does not support session save/load snapshots")]
    UnsupportedTier { tier: &'static str },
    #[error("executor snapshot tier `{snapshot}` cannot be restored into `{actual}`")]
    TierMismatch {
        snapshot: &'static str,
        actual: &'static str,
    },
    #[error("product AWBC snapshot error: {message}")]
    ProductAwbc { message: String },
}

/// Shared runtime executor facade used by application-facing crates.
///
/// The facade owns concrete executor construction so runtime hosts, CLI paths,
/// native players, and development runners do not wire `BytecodeVmExecutor`
/// directly. Product AWBC bytecode execution can replace or extend these
/// variants without changing those callers.
#[derive(Clone, Debug, PartialEq)]
pub struct ArcweftRuntimeExecutor {
    inner: ArcweftRuntimeExecutorInner,
}

#[derive(Clone, Debug, PartialEq)]
enum ArcweftRuntimeExecutorInner {
    StructuredVm(BytecodeVmExecutor),
    StructuredAot(AotExecutor),
    AwbcProduct(Box<AwbcProductExecutor>),
}

impl VmExecutor {
    pub(crate) fn new(plan: RuntimePlan) -> Self {
        Self {
            engine: Engine::new(plan),
        }
    }

    pub(crate) const fn engine(&self) -> &Engine {
        &self.engine
    }

    pub(crate) const fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    pub(crate) fn step_with_root_bindings_and_pure_backend(
        &mut self,
        input: RuntimeStepInput,
        root_bindings: &[RuntimeBinding],
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        self.engine.step_with_root_bindings_and_pure_backend(
            input,
            root_bindings,
            options,
            pure_backend,
        )
    }
}

impl AotExecutor {
    pub(crate) fn new(plan: RuntimePlan) -> Self {
        let program = AotProgram::from_runtime_plan(&plan);
        let vm = VmExecutor::new(plan);
        Self {
            program,
            vm,
            fast_path_ops: 0,
        }
    }

    #[cfg(test)]
    pub(crate) const fn program(&self) -> &AotProgram {
        &self.program
    }

    pub(crate) const fn fast_path_ops(&self) -> usize {
        self.fast_path_ops
    }

    pub(crate) fn step_with_root_bindings_and_pure_backend(
        &mut self,
        input: RuntimeStepInput,
        root_bindings: &[RuntimeBinding],
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        if self
            .vm
            .engine()
            .can_start_aot_linear_step(&self.program, &input)
        {
            let (result, fast_path_ops) = self
                .vm
                .engine_mut()
                .step_prechecked_aot_linear_with_pure_backend(
                    &self.program,
                    input,
                    root_bindings,
                    options,
                    pure_backend,
                );
            self.fast_path_ops += fast_path_ops;
            return result;
        }
        self.vm.step_with_root_bindings_and_pure_backend(
            input,
            root_bindings,
            options,
            pure_backend,
        )
    }
}

impl BytecodeVmExecutor {
    pub(crate) fn new(program: BytecodeProgram) -> Result<Self, RuntimePlanError> {
        Ok(Self {
            vm: VmExecutor::new(program.into_runtime_plan()?),
        })
    }

    pub(crate) fn from_runtime_plan(plan: RuntimePlan) -> Self {
        Self {
            vm: VmExecutor::new(plan),
        }
    }

    pub(crate) fn step_with_root_bindings_and_pure_backend(
        &mut self,
        input: RuntimeStepInput,
        root_bindings: &[RuntimeBinding],
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        self.vm.step_with_root_bindings_and_pure_backend(
            input,
            root_bindings,
            options,
            pure_backend,
        )
    }
}

impl AwbcProductExecutor {
    #[must_use]
    pub(crate) fn snapshot(&self) -> AwbcProductExecutorSnapshot {
        self.vm.snapshot()
    }

    pub(crate) fn restore_snapshot(
        &mut self,
        snapshot: AwbcProductExecutorSnapshot,
    ) -> Result<(), AwbcProductStepBuildError> {
        self.vm.restore_snapshot(snapshot)
    }
}

impl ArcweftRuntimeExecutor {
    pub fn from_runtime_plan(plan: RuntimePlan, tier: ArcweftExecutionTier) -> Self {
        match tier {
            ArcweftExecutionTier::StructuredVm | ArcweftExecutionTier::AwbcProduct => {
                Self::from_inner(ArcweftRuntimeExecutorInner::StructuredVm(
                    BytecodeVmExecutor::from_runtime_plan(plan),
                ))
            }
            ArcweftExecutionTier::StructuredAot => Self::from_inner(
                ArcweftRuntimeExecutorInner::StructuredAot(AotExecutor::new(plan)),
            ),
        }
    }

    pub fn from_awbc_product(
        program: AwbcProgram,
        entry: AwbcEntryId,
    ) -> Result<Self, AwbcProductStepBuildError> {
        let vm = AwbcProductStepExecutor::for_entry(program, entry, 64)?;
        Ok(Self::from_inner(ArcweftRuntimeExecutorInner::AwbcProduct(
            Box::new(AwbcProductExecutor { vm }),
        )))
    }

    pub fn from_awbc_product_function(
        program: AwbcProgram,
        entry: AwbcEntryId,
        function: AwbcFunctionId,
    ) -> Result<Self, AwbcProductStepBuildError> {
        let vm = AwbcProductStepExecutor::for_function(program, entry, function, 64)?;
        Ok(Self::from_inner(ArcweftRuntimeExecutorInner::AwbcProduct(
            Box::new(AwbcProductExecutor { vm }),
        )))
    }

    pub fn from_bytecode(
        program: BytecodeProgram,
        tier: ArcweftExecutionTier,
    ) -> Result<Self, RuntimePlanError> {
        Ok(match tier {
            ArcweftExecutionTier::StructuredVm | ArcweftExecutionTier::AwbcProduct => {
                Self::from_inner(ArcweftRuntimeExecutorInner::StructuredVm(
                    BytecodeVmExecutor::new(program)?,
                ))
            }
            ArcweftExecutionTier::StructuredAot => {
                Self::from_inner(ArcweftRuntimeExecutorInner::StructuredAot(
                    AotExecutor::new(program.into_runtime_plan()?),
                ))
            }
        })
    }

    pub const fn tier(&self) -> ArcweftExecutionTier {
        match &self.inner {
            ArcweftRuntimeExecutorInner::StructuredVm(_) => ArcweftExecutionTier::StructuredVm,
            ArcweftRuntimeExecutorInner::StructuredAot(_) => ArcweftExecutionTier::StructuredAot,
            ArcweftRuntimeExecutorInner::AwbcProduct(_) => ArcweftExecutionTier::AwbcProduct,
        }
    }

    pub const fn fast_path_ops(&self) -> usize {
        match &self.inner {
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => executor.fast_path_ops(),
            ArcweftRuntimeExecutorInner::StructuredVm(_)
            | ArcweftRuntimeExecutorInner::AwbcProduct(_) => 0,
        }
    }

    pub fn snapshot(
        &self,
    ) -> Result<ArcweftRuntimeExecutorSnapshot, ArcweftRuntimeExecutorSnapshotError> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => Ok(
                ArcweftRuntimeExecutorSnapshot::AwbcProduct(executor.snapshot()),
            ),
            ArcweftRuntimeExecutorInner::StructuredVm(_)
            | ArcweftRuntimeExecutorInner::StructuredAot(_) => {
                Err(ArcweftRuntimeExecutorSnapshotError::UnsupportedTier {
                    tier: self.tier().as_str(),
                })
            }
        }
    }

    pub fn restore_snapshot(
        &mut self,
        snapshot: ArcweftRuntimeExecutorSnapshot,
    ) -> Result<(), ArcweftRuntimeExecutorSnapshotError> {
        match (&mut self.inner, snapshot) {
            (
                ArcweftRuntimeExecutorInner::AwbcProduct(executor),
                ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot),
            ) => executor.restore_snapshot(snapshot).map_err(|error| {
                ArcweftRuntimeExecutorSnapshotError::ProductAwbc {
                    message: error.to_string(),
                }
            }),
            (_, ArcweftRuntimeExecutorSnapshot::AwbcProduct(_)) => {
                Err(ArcweftRuntimeExecutorSnapshotError::TierMismatch {
                    snapshot: ArcweftExecutionTier::AwbcProduct.as_str(),
                    actual: self.tier().as_str(),
                })
            }
        }
    }

    pub fn step_with_pure_backend(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        self.step_with_root_bindings_and_pure_backend(input, &[], options, pure_backend)
    }

    pub fn step_with_root_bindings_and_pure_backend(
        &mut self,
        input: RuntimeStepInput,
        root_bindings: &[RuntimeBinding],
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::StructuredVm(executor) => executor
                .step_with_root_bindings_and_pure_backend(
                    input,
                    root_bindings,
                    options,
                    pure_backend,
                ),
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => executor
                .step_with_root_bindings_and_pure_backend(
                    input,
                    root_bindings,
                    options,
                    pure_backend,
                ),
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => {
                executor.vm.step_with_root_bindings_and_pure_backend(
                    input,
                    root_bindings,
                    options,
                    pure_backend,
                )
            }
        }
    }

    const fn from_inner(inner: ArcweftRuntimeExecutorInner) -> Self {
        Self { inner }
    }
}

impl RuntimeExecutor for VmExecutor {
    fn step(&mut self, input: RuntimeStepInput, options: RuntimeStepOptions) -> RuntimeStepResult {
        self.engine.step(input, options)
    }

    fn fiber(&self) -> &FlowFiber {
        self.engine.fiber()
    }
}

impl RuntimeExecutor for AotExecutor {
    fn step(&mut self, input: RuntimeStepInput, options: RuntimeStepOptions) -> RuntimeStepResult {
        if self
            .vm
            .engine()
            .can_start_aot_linear_step(&self.program, &input)
        {
            let mut pure_backend = crate::pure::VmRuntimePureCallBackend::default();
            let (result, fast_path_ops) = self
                .vm
                .engine_mut()
                .step_prechecked_aot_linear_with_pure_backend(
                    &self.program,
                    input,
                    &[],
                    options,
                    &mut pure_backend,
                );
            self.fast_path_ops += fast_path_ops;
            return result;
        }
        self.vm.step(input, options)
    }

    fn fiber(&self) -> &FlowFiber {
        self.vm.fiber()
    }
}

impl RuntimeExecutor for BytecodeVmExecutor {
    fn step(&mut self, input: RuntimeStepInput, options: RuntimeStepOptions) -> RuntimeStepResult {
        self.vm.step(input, options)
    }

    fn fiber(&self) -> &FlowFiber {
        self.vm.fiber()
    }
}

impl RuntimeExecutor for ArcweftRuntimeExecutor {
    fn step(&mut self, input: RuntimeStepInput, options: RuntimeStepOptions) -> RuntimeStepResult {
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::StructuredVm(executor) => executor.step(input, options),
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => executor.step(input, options),
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor.vm.step(input, options),
        }
    }

    fn fiber(&self) -> &FlowFiber {
        match &self.inner {
            ArcweftRuntimeExecutorInner::StructuredVm(executor) => executor.fiber(),
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => executor.fiber(),
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor.vm.fiber(),
        }
    }
}

impl RuntimeExecutor for Engine {
    fn step(&mut self, input: RuntimeStepInput, options: RuntimeStepOptions) -> RuntimeStepResult {
        Engine::step(self, input, options)
    }

    fn fiber(&self) -> &FlowFiber {
        self.fiber()
    }
}
