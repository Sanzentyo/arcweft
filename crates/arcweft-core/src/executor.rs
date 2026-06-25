use crate::aot::AotProgram;
use crate::bytecode::BytecodeProgram;
use crate::engine::{Engine, FlowFiber};
use crate::plan::{RuntimePlan, RuntimePlanError};
use crate::pure::RuntimeCallBackend;
use crate::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepResult};
use crate::value::RuntimeBinding;

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
pub struct VmExecutor {
    engine: Engine,
}

/// AOT executor boundary backed by a typed AOT program artifact.
///
/// The current backend runs through the VM-compatible state machine after AOT
/// shape analysis. Generated dispatch can replace that backend without changing
/// host-facing executor selection.
#[derive(Clone, Debug, PartialEq)]
pub struct AotExecutor {
    program: AotProgram,
    vm: VmExecutor,
    fast_path_ops: usize,
}

/// Runtime executor backed by a bytecode bundle.
#[derive(Clone, Debug, PartialEq)]
pub struct BytecodeVmExecutor {
    program: BytecodeProgram,
    vm: VmExecutor,
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
}

impl VmExecutor {
    pub fn new(plan: RuntimePlan) -> Self {
        Self {
            engine: Engine::new(plan),
        }
    }

    pub const fn engine(&self) -> &Engine {
        &self.engine
    }

    pub const fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    pub fn into_engine(self) -> Engine {
        self.engine
    }

    pub fn step_with_pure_backend(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        self.engine
            .step_with_pure_backend(input, options, pure_backend)
    }

    pub fn step_with_root_bindings_and_pure_backend(
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
    pub fn new(plan: RuntimePlan) -> Self {
        let program = AotProgram::from_runtime_plan(&plan);
        let vm = VmExecutor::new(plan);
        Self {
            program,
            vm,
            fast_path_ops: 0,
        }
    }

    pub fn from_parts(program: AotProgram, plan: RuntimePlan) -> Self {
        let vm = VmExecutor::new(plan);
        Self {
            program,
            vm,
            fast_path_ops: 0,
        }
    }

    pub const fn program(&self) -> &AotProgram {
        &self.program
    }

    pub const fn vm(&self) -> &VmExecutor {
        &self.vm
    }

    pub const fn fast_path_ops(&self) -> usize {
        self.fast_path_ops
    }

    pub fn into_program(self) -> AotProgram {
        self.program
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
    pub fn new(program: BytecodeProgram) -> Result<Self, RuntimePlanError> {
        let vm = VmExecutor::new(program.clone().into_runtime_plan()?);
        Ok(Self { program, vm })
    }

    pub fn from_parts(program: BytecodeProgram, plan: RuntimePlan) -> Self {
        Self {
            program,
            vm: VmExecutor::new(plan),
        }
    }

    pub fn from_runtime_plan(plan: RuntimePlan) -> Self {
        Self {
            program: BytecodeProgram::from_runtime_plan(plan.clone()),
            vm: VmExecutor::new(plan),
        }
    }

    pub const fn program(&self) -> &BytecodeProgram {
        &self.program
    }

    pub const fn vm(&self) -> &VmExecutor {
        &self.vm
    }

    pub fn into_program(self) -> BytecodeProgram {
        self.program
    }

    pub fn step_with_pure_backend(
        &mut self,
        input: RuntimeStepInput,
        options: RuntimeStepOptions,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> RuntimeStepResult {
        self.vm.step_with_pure_backend(input, options, pure_backend)
    }

    pub fn step_with_root_bindings_and_pure_backend(
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

impl ArcweftRuntimeExecutor {
    pub fn from_runtime_plan(plan: RuntimePlan, tier: ArcweftExecutionTier) -> Self {
        match tier {
            ArcweftExecutionTier::StructuredVm => {
                Self::from_inner(ArcweftRuntimeExecutorInner::StructuredVm(
                    BytecodeVmExecutor::from_runtime_plan(plan),
                ))
            }
            ArcweftExecutionTier::StructuredAot => Self::from_inner(
                ArcweftRuntimeExecutorInner::StructuredAot(AotExecutor::new(plan)),
            ),
        }
    }

    pub fn from_bytecode(
        program: BytecodeProgram,
        tier: ArcweftExecutionTier,
    ) -> Result<Self, RuntimePlanError> {
        Ok(match tier {
            ArcweftExecutionTier::StructuredVm => Self::from_inner(
                ArcweftRuntimeExecutorInner::StructuredVm(BytecodeVmExecutor::new(program)?),
            ),
            ArcweftExecutionTier::StructuredAot => {
                Self::from_inner(ArcweftRuntimeExecutorInner::StructuredAot(
                    AotExecutor::new(program.into_runtime_plan()?),
                ))
            }
        })
    }

    pub fn from_bytecode_parts(
        program: BytecodeProgram,
        plan: RuntimePlan,
        tier: ArcweftExecutionTier,
    ) -> Self {
        match tier {
            ArcweftExecutionTier::StructuredVm => {
                Self::from_inner(ArcweftRuntimeExecutorInner::StructuredVm(
                    BytecodeVmExecutor::from_parts(program, plan),
                ))
            }
            ArcweftExecutionTier::StructuredAot => {
                let _ = program;
                Self::from_inner(ArcweftRuntimeExecutorInner::StructuredAot(
                    AotExecutor::new(plan),
                ))
            }
        }
    }

    pub fn from_aot_parts(program: AotProgram, plan: RuntimePlan) -> Self {
        Self::from_inner(ArcweftRuntimeExecutorInner::StructuredAot(
            AotExecutor::from_parts(program, plan),
        ))
    }

    pub const fn tier(&self) -> ArcweftExecutionTier {
        match &self.inner {
            ArcweftRuntimeExecutorInner::StructuredVm(_) => ArcweftExecutionTier::StructuredVm,
            ArcweftRuntimeExecutorInner::StructuredAot(_) => ArcweftExecutionTier::StructuredAot,
        }
    }

    pub const fn fast_path_ops(&self) -> usize {
        match &self.inner {
            ArcweftRuntimeExecutorInner::StructuredVm(_) => 0,
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => executor.fast_path_ops(),
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
        }
    }

    fn fiber(&self) -> &FlowFiber {
        match &self.inner {
            ArcweftRuntimeExecutorInner::StructuredVm(executor) => executor.fiber(),
            ArcweftRuntimeExecutorInner::StructuredAot(executor) => executor.fiber(),
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
