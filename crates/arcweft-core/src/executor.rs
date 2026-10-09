use crate::aot::AotProgram;
use crate::awbc::product_step::{
    AwbcProductExecutorRollbackImage, AwbcProductExecutorSaveSnapshot, AwbcProductSaveError,
    AwbcProductStepBuildError, AwbcProductStepExecutor,
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
use crate::task::{GenerationId, NeedId, RuntimeNeedProducerDispatch, TaskId};
use std::sync::Arc;
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
#[derive(Debug, PartialEq)]
pub(crate) struct VmExecutor {
    engine: Engine,
}

/// AOT executor boundary backed by a typed AOT program artifact.
///
/// The current backend runs through the VM-compatible state machine after AOT
/// shape analysis. Generated dispatch can replace that backend without changing
/// host-facing executor selection.
#[derive(Debug, PartialEq)]
pub(crate) struct AotExecutor {
    program: AotProgram,
    vm: VmExecutor,
    fast_path_ops: usize,
}

/// Runtime executor backed by canonical product AWBC.
#[derive(Debug, PartialEq)]
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
    AwbcProduct(AwbcProductExecutorSaveSnapshot),
}

#[derive(Debug, Error)]
#[error("{reason}")]
pub struct ArcweftRuntimeExecutorOwnedRestoreError {
    owner: ArcweftRuntimeExecutor,
    reason: ArcweftRuntimeExecutorSnapshotError,
}

impl ArcweftRuntimeExecutorOwnedRestoreError {
    pub fn into_parts(self) -> (ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshotError) {
        (self.owner, self.reason)
    }
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
    #[error("Product save requires quiescence for Need identities {needs:?}")]
    NeedsQuiescence { needs: Vec<NeedId> },
    #[error("product AWBC snapshot error: {message}")]
    ProductAwbc { message: String },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ArcweftRuntimeExecutorBuildError {
    #[error("execution tier `{tier}` requires an AWBC product")]
    TierRequiresAwbc { tier: &'static str },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ArcweftRuntimeExecutorGenerationError {
    #[error("runtime generation cannot move backward (current {current}, requested {requested})")]
    Backward { current: u64, requested: u64 },
    #[error("Product AWBC generation rebind failed: {message}")]
    ProductAwbc { message: String },
}

/// Shared runtime executor facade used by application-facing crates.
///
/// The facade owns concrete executor construction so runtime hosts, CLI paths,
/// native players, and development runners do not wire concrete engines
/// directly.
#[derive(Debug, PartialEq)]
pub struct ArcweftRuntimeExecutor {
    inner: ArcweftRuntimeExecutorInner,
}

#[derive(Debug, PartialEq)]
enum ArcweftRuntimeExecutorInner {
    RuntimePlanVm(VmExecutor),
    StructuredAot(AotExecutor),
    AwbcProduct(Box<AwbcProductExecutor>),
    /// Exists only while an owning restore transaction has moved its sole
    /// executor out of the facade. It is never exposed as a runnable tier.
    Vacant,
}

impl VmExecutor {
    pub(crate) fn new(plan: RuntimePlan) -> Self {
        Self::new_with_generation(plan, GenerationId::new(0))
    }

    pub(crate) fn new_with_generation(
        plan: impl Into<Arc<RuntimePlan>>,
        generation: GenerationId,
    ) -> Self {
        Self {
            engine: Engine::new_with_generation(plan, generation),
        }
    }

    pub(crate) fn from_flow_invocation(
        invocation: RuntimeFlowInvocation,
    ) -> Result<Self, EngineStartError> {
        Self::from_flow_invocation_with_generation(invocation, GenerationId::new(0))
    }

    pub(crate) fn from_flow_invocation_with_generation(
        invocation: RuntimeFlowInvocation,
        generation: GenerationId,
    ) -> Result<Self, EngineStartError> {
        Engine::for_flow_invocation_with_generation(invocation, generation)
            .map(|engine| Self { engine })
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
        Self::new_with_generation(plan, GenerationId::new(0))
    }

    pub(crate) fn new_with_generation(
        plan: impl Into<Arc<RuntimePlan>>,
        generation: GenerationId,
    ) -> Self {
        let plan = plan.into();
        let program = AotProgram::from_runtime_plan(&plan);
        let vm = VmExecutor::new_with_generation(plan, generation);
        Self {
            program,
            vm,
            fast_path_ops: 0,
        }
    }

    pub(crate) fn from_flow_invocation(
        invocation: RuntimeFlowInvocation,
    ) -> Result<Self, EngineStartError> {
        Self::from_flow_invocation_with_generation(invocation, GenerationId::new(0))
    }

    pub(crate) fn from_flow_invocation_with_generation(
        invocation: RuntimeFlowInvocation,
        generation: GenerationId,
    ) -> Result<Self, EngineStartError> {
        let program = AotProgram::from_runtime_plan(invocation.plan());
        let vm = VmExecutor::from_flow_invocation_with_generation(invocation, generation)?;
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
    pub(crate) fn snapshot_for_save(
        &self,
    ) -> Result<AwbcProductExecutorSaveSnapshot, AwbcProductSaveError> {
        let needs = self.vm.quiescence_blocking_needs();
        if !needs.is_empty() {
            return Err(AwbcProductSaveError::NeedsQuiescence { needs });
        }
        self.vm.inert_rollback_image().map(|image| image.product)
    }
}

impl ArcweftRuntimeExecutor {
    /// Moves the sole Product executor into a restore transaction, leaving a
    /// non-runnable slot that must be filled before the session is observed.
    pub fn take_product_for_restore(&mut self) -> Option<Self> {
        if !matches!(self.inner, ArcweftRuntimeExecutorInner::AwbcProduct(_)) {
            return None;
        }
        Some(Self {
            inner: std::mem::replace(&mut self.inner, ArcweftRuntimeExecutorInner::Vacant),
        })
    }

    pub fn install_after_restore(&mut self, restored: Self) {
        assert!(matches!(self.inner, ArcweftRuntimeExecutorInner::Vacant));
        self.inner = restored.inner;
    }
    /// Selects the ambient locale for subsequent native or Product AWBC
    /// formatter attempts. In-flight AWBC attempts retain their start locale.
    pub fn set_format_context(&mut self, context: crate::value::RuntimeFormatContext) {
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => {
                executor.engine.set_format_context(context);
            }
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => {
                executor.vm.engine.set_format_context(context);
            }
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => {
                executor.vm.set_format_context(context);
            }
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
        }
    }

    #[must_use]
    pub fn format_context(&self) -> &crate::value::RuntimeFormatContext {
        match &self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => {
                executor.engine.format_context()
            }
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => {
                executor.vm.engine.format_context()
            }
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor.vm.format_context(),
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
        }
    }

    pub fn from_runtime_plan(
        plan: impl Into<Arc<RuntimePlan>>,
        tier: ArcweftExecutionTier,
    ) -> Result<Self, ArcweftRuntimeExecutorBuildError> {
        Self::from_runtime_plan_with_generation(plan, tier, GenerationId::new(0))
    }

    pub fn from_runtime_plan_with_generation(
        plan: impl Into<Arc<RuntimePlan>>,
        tier: ArcweftExecutionTier,
        generation: GenerationId,
    ) -> Result<Self, ArcweftRuntimeExecutorBuildError> {
        let plan = plan.into();
        Ok(match tier {
            ArcweftExecutionTier::RuntimePlanVm => {
                Self::from_inner(ArcweftRuntimeExecutorInner::RuntimePlanVm(
                    VmExecutor::new_with_generation(plan, generation),
                ))
            }
            ArcweftExecutionTier::StructuredAot => {
                Self::from_inner(ArcweftRuntimeExecutorInner::StructuredAot(
                    AotExecutor::new_with_generation(plan, generation),
                ))
            }
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
        Self::from_runtime_flow_invocation_with_generation(invocation, tier, GenerationId::new(0))
    }

    pub fn from_runtime_flow_invocation_with_generation(
        invocation: RuntimeFlowInvocation,
        tier: ArcweftExecutionTier,
        generation: GenerationId,
    ) -> Result<Self, EngineStartError> {
        match tier {
            ArcweftExecutionTier::RuntimePlanVm => {
                VmExecutor::from_flow_invocation_with_generation(invocation, generation)
                    .map(ArcweftRuntimeExecutorInner::RuntimePlanVm)
                    .map(Self::from_inner)
            }
            ArcweftExecutionTier::StructuredAot => {
                AotExecutor::from_flow_invocation_with_generation(invocation, generation)
                    .map(ArcweftRuntimeExecutorInner::StructuredAot)
                    .map(Self::from_inner)
            }
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
        Self::from_awbc_product_arc(Arc::new(program), entry)
    }

    /// Starts Product AWBC with an exact executable lease shared by its
    /// generation-local consumers.
    pub fn from_awbc_product_arc(
        program: Arc<AwbcProgram>,
        entry: AwbcEntryId,
    ) -> Result<Self, AwbcProductStepBuildError> {
        Self::from_awbc_product_arc_with_generation(program, entry, GenerationId::new(0))
    }

    pub fn from_awbc_product_arc_with_generation(
        program: Arc<AwbcProgram>,
        entry: AwbcEntryId,
        generation: GenerationId,
    ) -> Result<Self, AwbcProductStepBuildError> {
        let vm =
            AwbcProductStepExecutor::for_entry_arc_with_generation(program, entry, 64, generation)?;
        Ok(Self::from_inner(ArcweftRuntimeExecutorInner::AwbcProduct(
            Box::new(AwbcProductExecutor { vm }),
        )))
    }

    /// Starts Product AWBC with the non-serialized plain-text Content proof
    /// supplied by a validated bundle/catalog join.
    pub fn from_awbc_product_arc_with_plain_text_context_proof(
        program: Arc<AwbcProgram>,
        entry: AwbcEntryId,
        proof: crate::value::RuntimeDialoguePlainTextContextTemplateProof,
    ) -> Result<Self, AwbcProductStepBuildError> {
        Self::from_awbc_product_arc_with_plain_text_context_proof_and_generation(
            program,
            entry,
            proof,
            GenerationId::new(0),
        )
    }

    pub fn from_awbc_product_arc_with_plain_text_context_proof_and_generation(
        program: Arc<AwbcProgram>,
        entry: AwbcEntryId,
        proof: crate::value::RuntimeDialoguePlainTextContextTemplateProof,
        generation: GenerationId,
    ) -> Result<Self, AwbcProductStepBuildError> {
        let vm =
            AwbcProductStepExecutor::for_entry_arc_with_plain_text_context_proof_and_generation(
                program, entry, 64, generation, proof,
            )?;
        Ok(Self::from_inner(ArcweftRuntimeExecutorInner::AwbcProduct(
            Box::new(AwbcProductExecutor { vm }),
        )))
    }

    pub fn from_awbc_product_function(
        program: AwbcProgram,
        entry: AwbcEntryId,
        function: AwbcFunctionId,
    ) -> Result<Self, AwbcProductStepBuildError> {
        Self::from_awbc_product_function_with_generation(
            program,
            entry,
            function,
            GenerationId::new(0),
        )
    }

    pub fn from_awbc_product_function_with_generation(
        program: AwbcProgram,
        entry: AwbcEntryId,
        function: AwbcFunctionId,
        generation: GenerationId,
    ) -> Result<Self, AwbcProductStepBuildError> {
        let vm = AwbcProductStepExecutor::for_function_invocation_with_generation(
            program,
            entry,
            function,
            [],
            64,
            generation,
        )?;
        Ok(Self::from_inner(ArcweftRuntimeExecutorInner::AwbcProduct(
            Box::new(AwbcProductExecutor { vm }),
        )))
    }

    pub fn tier(&self) -> ArcweftExecutionTier {
        match &self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_) => ArcweftExecutionTier::RuntimePlanVm,
            ArcweftRuntimeExecutorInner::StructuredAot(_) => ArcweftExecutionTier::StructuredAot,
            ArcweftRuntimeExecutorInner::AwbcProduct(_) => ArcweftExecutionTier::AwbcProduct,
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
        }
    }

    /// Admits a host-owned catalog call through this execution's sole journal.
    pub fn admit_host_call(
        &mut self,
        start: crate::step::RuntimeHostCallStart,
    ) -> Result<crate::step::RuntimeHostCallRequest, crate::task::NeedProducerAdmissionError> {
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => {
                executor.engine.admit_host_call(start)
            }
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => {
                executor.vm.engine.admit_host_call(start)
            }
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => {
                executor.vm.admit_host_call(start)
            }
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
        }
    }

    #[must_use]
    pub fn generation(&self) -> GenerationId {
        match &self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => executor.engine.generation(),
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => executor.vm.engine.generation(),
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor.vm.runtime_generation(),
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
        }
    }

    /// Returns active Restartable Need producer requests with their exact task
    /// identity and generation pin. Hosts keep these rows alongside ordinary
    /// task dispatches until a terminal Need publication is accepted.
    #[must_use]
    pub fn restartable_dispatches(&self) -> Vec<RuntimeNeedProducerDispatch> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => {
                executor.engine.restartable_dispatches()
            }
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => {
                executor.vm.engine.restartable_dispatches()
            }
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => {
                executor.vm.restartable_dispatches()
            }
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
        }
    }

    /// Returns the owning generation for a producer task request. Ordinary
    /// AwaitMany requests have no registry launch and use the current pin.
    #[must_use]
    pub fn task_generation(&self, task: &TaskId) -> Option<GenerationId> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => {
                executor.engine.need_producer_generation_for_task(task)
            }
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => {
                executor.vm.engine.need_producer_generation_for_task(task)
            }
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor.vm.task_generation(task),
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
        }
    }

    /// Reports active producer Needs that must finish before a session save.
    /// Native VM tiers expose the same typed blocker even though their current
    /// executor snapshot tier is unsupported.
    #[must_use]
    pub fn quiescence_blocking_needs(&self) -> Vec<NeedId> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => {
                executor.engine.quiescence_blocking_needs()
            }
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => {
                executor.vm.engine.quiescence_blocking_needs()
            }
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => {
                executor.vm.quiescence_blocking_needs()
            }
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
        }
    }

    /// Rebinds the generation used by future Need producer admissions.
    /// Existing launches retain their original generation and identifiers.
    /// Rebinding to the current generation is an idempotent no-op.
    pub fn rebind_generation(
        &mut self,
        generation: GenerationId,
    ) -> Result<(), ArcweftRuntimeExecutorGenerationError> {
        let current = self.generation();
        if generation < current {
            return Err(ArcweftRuntimeExecutorGenerationError::Backward {
                current: current.get(),
                requested: generation.get(),
            });
        }
        if generation == current {
            return Ok(());
        }
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => {
                executor.engine_mut().rebind_generation(generation);
                Ok(())
            }
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => {
                executor.vm.engine_mut().rebind_generation(generation);
                Ok(())
            }
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor
                .vm
                .rebind_generation(generation)
                .map_err(|error| ArcweftRuntimeExecutorGenerationError::ProductAwbc {
                    message: error.to_string(),
                }),
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
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
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
        }
    }

    /// Returns the canonical program that owns Product AWBC fiber values.
    pub fn product_awbc_program(&self) -> Option<&AwbcProgram> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => Some(executor.vm.program()),
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
            | ArcweftRuntimeExecutorInner::StructuredAot(_)
            | ArcweftRuntimeExecutorInner::Vacant => None,
        }
    }

    /// Borrows every live Product value for external catalog validation.
    /// The caller must have selected the Product tier.
    pub fn visit_product_live_runtime_values<E>(
        &self,
        visitor: impl FnMut(&crate::value::RuntimeValue) -> Result<(), E>,
    ) -> Result<(), E> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => {
                executor.vm.visit_live_runtime_values(visitor)
            }
            _ => unreachable!("Product live-value visitor requires Product executor"),
        }
    }

    pub fn product_inert_rollback_image(
        &self,
    ) -> Result<AwbcProductExecutorRollbackImage, ArcweftRuntimeExecutorSnapshotError> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor
                .vm
                .inert_rollback_image()
                .map_err(|error| ArcweftRuntimeExecutorSnapshotError::ProductAwbc {
                    message: error.to_string(),
                }),
            _ => unreachable!("Product rollback image requires Product executor"),
        }
    }

    pub fn restore_product_rollback_image(
        &mut self,
        image: AwbcProductExecutorRollbackImage,
    ) -> Result<(), ArcweftRuntimeExecutorSnapshotError> {
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor
                .vm
                .restore_rollback_image(image)
                .map_err(|error| ArcweftRuntimeExecutorSnapshotError::ProductAwbc {
                    message: error.to_string(),
                }),
            _ => unreachable!("Product rollback restore requires Product executor"),
        }
    }

    /// Retains the exact selected program for an asynchronous host result.
    pub fn program_owner(&self) -> crate::task::RuntimeProgramOwner {
        match &self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => {
                crate::task::RuntimeProgramOwner::Plan(executor.engine.program_plan())
            }
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => {
                crate::task::RuntimeProgramOwner::Plan(executor.vm.engine.program_plan())
            }
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => {
                crate::task::RuntimeProgramOwner::Awbc(executor.vm.program_arc())
            }
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
        }
    }

    /// Installs a code-compatible Product AWBC program while preserving the
    /// current executor, fiber, and durable root transaction state.
    pub fn replace_product_awbc_program(
        &mut self,
        program: AwbcProgram,
    ) -> Result<(), AwbcProductStepBuildError> {
        self.replace_product_awbc_program_arc(Arc::new(program))
    }

    /// Rebinds a compatible exact lease while retaining the live fiber state.
    pub fn replace_product_awbc_program_arc(
        &mut self,
        program: Arc<AwbcProgram>,
    ) -> Result<(), AwbcProductStepBuildError> {
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => {
                executor.vm.replace_program_preserving_state_arc(program)
            }
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
            | ArcweftRuntimeExecutorInner::StructuredAot(_)
            | ArcweftRuntimeExecutorInner::Vacant => {
                Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "code-compatible Product AWBC replacement requires Product AWBC tier"
                        .to_owned(),
                })
            }
        }
    }

    /// Rebinds a compatible Product AWBC lease and its bundle-certified
    /// plain-text Content proof while retaining live fiber state.
    pub fn replace_product_awbc_program_arc_with_plain_text_context_proof(
        &mut self,
        program: Arc<AwbcProgram>,
        proof: crate::value::RuntimeDialoguePlainTextContextTemplateProof,
    ) -> Result<(), AwbcProductStepBuildError> {
        match &mut self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor
                .vm
                .replace_program_preserving_state_arc_with_plain_text_context_proof(program, proof),
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
            | ArcweftRuntimeExecutorInner::StructuredAot(_)
            | ArcweftRuntimeExecutorInner::Vacant => {
                Err(AwbcProductStepBuildError::RestoreSnapshot {
                    message: "code-compatible Product AWBC replacement requires Product AWBC tier"
                        .to_owned(),
                })
            }
        }
    }

    pub fn fast_path_ops(&self) -> usize {
        match &self.inner {
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => executor.fast_path_ops(),
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
            | ArcweftRuntimeExecutorInner::AwbcProduct(_) => 0,
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
        }
    }

    pub fn snapshot(
        &self,
    ) -> Result<ArcweftRuntimeExecutorSnapshot, ArcweftRuntimeExecutorSnapshotError> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => {
                match executor.snapshot_for_save() {
                    Ok(snapshot) => Ok(ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot)),
                    Err(AwbcProductSaveError::NeedsQuiescence { needs }) => {
                        Err(ArcweftRuntimeExecutorSnapshotError::NeedsQuiescence { needs })
                    }
                    Err(AwbcProductSaveError::InvalidSnapshot { message }) => {
                        Err(ArcweftRuntimeExecutorSnapshotError::ProductAwbc { message })
                    }
                }
            }
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
            | ArcweftRuntimeExecutorInner::StructuredAot(_)
            | ArcweftRuntimeExecutorInner::Vacant => {
                Err(ArcweftRuntimeExecutorSnapshotError::UnsupportedTier {
                    tier: self.tier().as_str(),
                })
            }
        }
    }

    pub fn restore_snapshot(
        self,
        snapshot: ArcweftRuntimeExecutorSnapshot,
    ) -> Result<Self, ArcweftRuntimeExecutorOwnedRestoreError> {
        match (self.inner, snapshot) {
            (
                ArcweftRuntimeExecutorInner::AwbcProduct(executor),
                ArcweftRuntimeExecutorSnapshot::AwbcProduct(snapshot),
            ) => match executor.vm.restore_inert_snapshot_owned(snapshot) {
                Ok(vm) => Ok(Self::from_inner(ArcweftRuntimeExecutorInner::AwbcProduct(
                    Box::new(AwbcProductExecutor { vm }),
                ))),
                Err((vm, error)) => Err(ArcweftRuntimeExecutorOwnedRestoreError {
                    owner: Self::from_inner(ArcweftRuntimeExecutorInner::AwbcProduct(Box::new(
                        AwbcProductExecutor { vm },
                    ))),
                    reason: ArcweftRuntimeExecutorSnapshotError::ProductAwbc {
                        message: error.to_string(),
                    },
                }),
            },
            (inner, ArcweftRuntimeExecutorSnapshot::AwbcProduct(_)) => {
                let owner = Self::from_inner(inner);
                Err(ArcweftRuntimeExecutorOwnedRestoreError {
                    reason: ArcweftRuntimeExecutorSnapshotError::TierMismatch {
                        snapshot: ArcweftExecutionTier::AwbcProduct.as_str(),
                        actual: owner.tier().as_str(),
                    },
                    owner,
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
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
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
            | ArcweftRuntimeExecutorInner::StructuredAot(_)
            | ArcweftRuntimeExecutorInner::Vacant => Ok(None),
        }
    }

    #[must_use]
    pub fn product_root_state_snapshot(&self) -> Option<RootStateSnapshotV1> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor.vm.root_state_snapshot(),
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
            | ArcweftRuntimeExecutorInner::StructuredAot(_)
            | ArcweftRuntimeExecutorInner::Vacant => None,
        }
    }

    #[must_use]
    pub fn product_root_save_blockers(&self) -> Option<RootSaveBlockers> {
        match &self.inner {
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor.vm.root_save_blockers(),
            ArcweftRuntimeExecutorInner::RuntimePlanVm(_)
            | ArcweftRuntimeExecutorInner::StructuredAot(_)
            | ArcweftRuntimeExecutorInner::Vacant => None,
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
            | ArcweftRuntimeExecutorInner::StructuredAot(_)
            | ArcweftRuntimeExecutorInner::Vacant => {
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
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
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
            pure_backend.set_format_context(self.vm.engine.format_context().clone());
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
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
        }
    }

    fn fiber(&self) -> &FlowFiber {
        match &self.inner {
            ArcweftRuntimeExecutorInner::RuntimePlanVm(executor) => executor.fiber(),
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => executor.fiber(),
            ArcweftRuntimeExecutorInner::AwbcProduct(executor) => executor.vm.fiber(),
            ArcweftRuntimeExecutorInner::Vacant => unreachable!("restore slot is not runnable"),
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
