use crate::aot::AotProgram;
use crate::bytecode::BytecodeProgram;
use crate::engine::{Engine, FlowFiber};
use crate::plan::{RuntimePlan, RuntimePlanError};
use crate::pure::RuntimePureCallBackend;
use crate::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepResult};

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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VmExecutor {
    engine: Engine,
}

/// AOT executor boundary backed by a typed AOT program artifact.
///
/// The current backend runs through the VM-compatible state machine after AOT
/// shape analysis. Generated dispatch can replace that backend without changing
/// host-facing executor selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AotExecutor {
    program: AotProgram,
    vm: VmExecutor,
    fast_path_ops: usize,
}

/// Runtime executor backed by a bytecode bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BytecodeVmExecutor {
    program: BytecodeProgram,
    vm: VmExecutor,
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
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> RuntimeStepResult {
        self.engine
            .step_with_pure_backend(input, options, pure_backend)
    }
}

impl AotExecutor {
    pub fn new(plan: RuntimePlan) -> Self {
        let program = AotProgram::from_runtime_plan(plan);
        let vm = VmExecutor::new(program.plan().clone());
        Self {
            program,
            vm,
            fast_path_ops: 0,
        }
    }

    pub fn from_program(program: AotProgram) -> Self {
        let vm = VmExecutor::new(program.plan().clone());
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
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> RuntimeStepResult {
        if self
            .vm
            .engine()
            .can_start_aot_linear_step(&self.program, &input)
        {
            let result = self
                .vm
                .engine_mut()
                .step_prechecked_aot_linear_with_pure_backend(input, options, pure_backend);
            self.fast_path_ops += result.stats.executed_ops;
            return result;
        }
        self.vm.step_with_pure_backend(input, options, pure_backend)
    }
}

impl BytecodeVmExecutor {
    pub fn new(program: BytecodeProgram) -> Result<Self, RuntimePlanError> {
        let vm = VmExecutor::new(program.clone().into_runtime_plan()?);
        Ok(Self { program, vm })
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
        pure_backend: &mut impl RuntimePureCallBackend,
    ) -> RuntimeStepResult {
        self.vm.step_with_pure_backend(input, options, pure_backend)
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
            let result = self
                .vm
                .engine_mut()
                .step_prechecked_aot_linear_with_pure_backend(input, options, &mut pure_backend);
            self.fast_path_ops += result.stats.executed_ops;
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

impl RuntimeExecutor for Engine {
    fn step(&mut self, input: RuntimeStepInput, options: RuntimeStepOptions) -> RuntimeStepResult {
        Engine::step(self, input, options)
    }

    fn fiber(&self) -> &FlowFiber {
        self.fiber()
    }
}
