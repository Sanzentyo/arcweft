use crate::bytecode::BytecodeProgram;
use crate::engine::{Engine, FlowFiber};
use crate::plan::{RuntimePlan, RuntimePlanError};
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

/// AOT executor boundary that currently preserves VM-equivalent semantics.
///
/// The Phase 2 implementation intentionally delegates to `VmExecutor` rather
/// than introducing a second semantic engine. Native or generated dispatch can
/// replace the backend behind this type once bytecode/state-machine generation
/// exists, while callers already depend on the stable `RuntimeExecutor` shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AotExecutor {
    vm: VmExecutor,
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
}

impl AotExecutor {
    pub fn new(plan: RuntimePlan) -> Self {
        Self {
            vm: VmExecutor::new(plan),
        }
    }

    pub const fn vm(&self) -> &VmExecutor {
        &self.vm
    }

    pub fn into_vm(self) -> VmExecutor {
        self.vm
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
