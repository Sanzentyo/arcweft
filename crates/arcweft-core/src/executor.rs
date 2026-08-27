use crate::aot::AotProgram;
use crate::awbc::product_step::{
    AwbcProductExecutorSnapshot, AwbcProductStepBuildError, AwbcProductStepExecutor,
};
use crate::awbc::schema::{AwbcEntryId, AwbcFunctionId, AwbcProgram};
use crate::engine::{Engine, EngineStartError, FlowFiber};
use crate::entry::ActiveEntrySnapshotV1;
use crate::plan::{EntryRuntimeId, RuntimeFlowInvocation, RuntimePlan};
use crate::pure::RuntimeCallBackend;
use crate::root::{
    RootRuntimeError, RootSaveBlockers, RootStateSnapshotV1, RuntimeCommandEnvelope,
};
use crate::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepResult};
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
    RuntimePlanVm,
    StructuredAot,
    AwbcProduct,
}

impl ArcweftExecutionTier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimePlanVm => "runtime_plan_vm",
            Self::StructuredAot => "structured_aot",
            Self::AwbcProduct => "awbc_product",
        }
    }

    #[must_use]
    pub const fn is_vm_first(self) -> bool {
        matches!(self, Self::RuntimePlanVm | Self::AwbcProduct)
    }
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ArcweftRuntimeExecutorBuildError {
    #[error("execution tier `{tier}` requires an AWBC product")]
    TierRequiresAwbc { tier: &'static str },
}

/// Shared runtime executor facade used by application-facing crates.
///
/// The facade owns concrete executor construction so runtime hosts, CLI paths,
/// native players, and development runners do not wire concrete engines
/// directly.
#[derive(Clone, Debug, PartialEq)]
pub struct ArcweftRuntimeExecutor {
    inner: ArcweftRuntimeExecutorInner,
}

#[derive(Clone, Debug, PartialEq)]
enum ArcweftRuntimeExecutorInner {
    RuntimePlanVm(VmExecutor),
    StructuredAot(AotExecutor),
    AwbcProduct(Box<AwbcProductExecutor>),
}

impl VmExecutor {
    pub(crate) fn new(plan: RuntimePlan) -> Self {
        Self {
            engine: Engine::new(plan),
        }
    }

    pub(crate) fn from_flow_invocation(
        invocation: RuntimeFlowInvocation,
    ) -> Result<Self, EngineStartError> {
        Engine::for_flow_invocation(invocation).map(|engine| Self { engine })
    }

    pub(crate) const fn engine(&self) -> &Engine {
        &self.engine
    }

    pub(crate) const fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    pub(crate) fn start_entry(&mut self, entry: &EntryRuntimeId) -> Result<(), EngineStartError> {
        self.engine.start_entry(entry)
    }

    pub(crate) fn step_with_pure_backend(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        self.engine
            .step_with_pure_backend(input, options, pure_backend)
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

    pub(crate) fn from_flow_invocation(
        invocation: RuntimeFlowInvocation,
    ) -> Result<Self, EngineStartError> {
        let program = AotProgram::from_runtime_plan(invocation.plan());
        let vm = VmExecutor::from_flow_invocation(invocation)?;
        Ok(Self {
            program,
            vm,
            fast_path_ops: 0,
        })
    }

    pub(crate) const fn fast_path_ops(&self) -> usize {
        self.fast_path_ops
    }

    pub(crate) fn start_entry(&mut self, entry: &EntryRuntimeId) -> Result<(), EngineStartError> {
        self.vm.start_entry(entry)
    }

    pub(crate) fn step_with_pure_backend(
        &mut self,
        input: RuntimeStepInput,
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
                .step_prechecked_aot_linear_with_pure_backend(&self.program, options, pure_backend);
            self.fast_path_ops += fast_path_ops;
            return result;
        }
        self.vm.step_with_pure_backend(input, options, pure_backend)
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
    pub fn from_runtime_plan(
        plan: RuntimePlan,
        tier: ArcweftExecutionTier,
    ) -> Result<Self, ArcweftRuntimeExecutorBuildError> {
        Ok(match tier {
            ArcweftExecutionTier::RuntimePlanVm => Self::from_inner(
                ArcweftRuntimeExecutorInner::RuntimePlanVm(VmExecutor::new(plan)),
            ),
            ArcweftExecutionTier::StructuredAot => Self::from_inner(
                ArcweftRuntimeExecutorInner::StructuredAot(AotExecutor::new(plan)),
            ),
            ArcweftExecutionTier::AwbcProduct => {
                return Err(ArcweftRuntimeExecutorBuildError::TierRequiresAwbc {
                    tier: tier.as_str(),
                });
            }
        })
    }

    pub fn from_runtime_flow_invocation(
        invocation: RuntimeFlowInvocation,
        tier: ArcweftExecutionTier,
    ) -> Result<Self, EngineStartError> {
        match tier {
            ArcweftExecutionTier::RuntimePlanVm => VmExecutor::from_flow_invocation(invocation)
                .map(ArcweftRuntimeExecutorInner::RuntimePlanVm)
                .map(Self::from_inner),
            ArcweftExecutionTier::StructuredAot => AotExecutor::from_flow_invocation(invocation)
                .map(ArcweftRuntimeExecutorInner::StructuredAot)
                .map(Self::from_inner),
            ArcweftExecutionTier::AwbcProduct => Err(EngineStartError::InvalidFlowInvocation {
                message: "RuntimePlan Flow invocation cannot initialize a Product AWBC executor"
                    .to_owned(),
            }),
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

    pub const fn tier(&self) -> ArcweftExecutionTier {
        match &self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_) => ArcweftExecutionTier::RuntimePlanVm,
            ArcweftRuntimeExecutorInner::StructuredAot(_) => ArcweftExecutionTier::StructuredAot,
            ArcweftRuntimeExecutorInner::AwbcProduct(_) => ArcweftExecutionTier::AwbcProduct,
        }
    }

    pub fn start_structured_entry(
        &mut self,
        entry: &EntryRuntimeId,
    ) -> Result<(), EngineStartError> {
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => executor.start_entry(entry),
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => executor.start_entry(entry),
            ArcweftRuntimeExecutorInner::AwbcProduct(_) => {
                Err(EngineStartError::EntryDoesNotSelectFlow {
                    entry: entry.canonical_label(),
                })
            }
        }
    }

    /// Returns the canonical program that owns Product AWBC fiber values.
    pub const fn product_awbc_program(&self) -> Option<&AwbcProgram> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => Some(executor.vm.program()),
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
            | ArcweftRuntimeExecutorInner::StructuredAot(_) => None,
        }
    }

    /// Installs a code-compatible Product AWBC program while preserving the
    /// current executor, fiber, and durable root transaction state.
    pub fn replace_product_awbc_program(
        &mut self,
        program: AwbcProgram,
    ) -> Result<(), AwbcProductStepBuildError> {
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => {
                executor.vm.replace_program_preserving_state(program)
            }
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
            | ArcweftRuntimeExecutorInner::StructuredAot(_) => {
                Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "code-compatible Product AWBC replacement requires Product AWBC tier"
                        .to_owned(),
                })
            }
        }
    }

    pub const fn fast_path_ops(&self) -> usize {
        match &self.inner {
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => executor.fast_path_ops(),
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
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
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
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

    /// Confirms the exact committed root-command prefix after the driver has
    /// accepted it into its dispatch/result boundary.
    pub fn acknowledge_root_commands(
        &mut self,
        accepted: &[RuntimeCommandEnvelope],
    ) -> Result<(), RootRuntimeError> {
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => {
                executor.engine_mut().acknowledge_root_commands(accepted)
            }
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => {
                executor.vm.engine_mut().acknowledge_root_commands(accepted)
            }
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => {
                executor.vm.acknowledge_root_commands(accepted)
            }
        }
    }

    pub fn product_active_entry_snapshot_identity(
        &self,
    ) -> Result<Option<ActiveEntrySnapshotV1>, RootRuntimeError> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => {
                executor.vm.active_entry_snapshot_identity().map(Some)
            }
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
            | ArcweftRuntimeExecutorInner::StructuredAot(_) => Ok(None),
        }
    }

    #[must_use]
    pub fn product_root_state_snapshot(&self) -> Option<RootStateSnapshotV1> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor.vm.root_state_snapshot(),
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
            | ArcweftRuntimeExecutorInner::StructuredAot(_) => None,
        }
    }

    #[must_use]
    pub fn product_root_save_blockers(&self) -> Option<RootSaveBlockers> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor.vm.root_save_blockers(),
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
            | ArcweftRuntimeExecutorInner::StructuredAot(_) => None,
        }
    }

    pub fn restore_product_root_snapshot(
        &mut self,
        active: &ActiveEntrySnapshotV1,
        snapshot: Option<RootStateSnapshotV1>,
    ) -> Result<(), RootRuntimeError> {
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => {
                executor.vm.restore_root_snapshot(active, snapshot)
            }
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
            | ArcweftRuntimeExecutorInner::StructuredAot(_) => {
                Err(RootRuntimeError::SnapshotRoleMismatch("executor tier"))
            }
        }
    }

    pub fn step_with_pure_backend(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => {
                executor.step_with_pure_backend(input, options, pure_backend)
            }
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => {
                executor.step_with_pure_backend(input, options, pure_backend)
            }
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor
                .vm
                .step_with_pure_backend(input, options, pure_backend),
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

impl RuntimeExecutor for ArcweftRuntimeExecutor {
    fn step(&mut self, input: RuntimeStepInput, options: RuntimeStepOptions) -> RuntimeStepResult {
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => executor.step(input, options),
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => executor.step(input, options),
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor.vm.step(input, options),
        }
    }

    fn fiber(&self) -> &FlowFiber {
        match &self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => executor.fiber(),
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
