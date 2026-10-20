//! Complete executable authority and the formatter selected for an invocation.

use super::{RuntimeCallBackend, RuntimeEvalError};
use crate::awbc::product_step::AwbcProductProgram;
use crate::plan::RuntimePlan;
use crate::program_types::RuntimeProgramTypes;
use crate::task::RuntimeProgramOwner;
use crate::value::{RuntimeFormatContext, RuntimeValue};
use arcweft_id::runtime_program::RuntimePureProgramId;
use std::sync::Arc;

/// Execution keeps the admitted code/Source/catalog owner and current locale.
/// A raw type lease cannot construct the AWBC variant.
#[derive(Clone, Debug)]
pub struct RuntimeProgramExecutionContext {
    program: RuntimeProgramExecutionOwner,
    format_context: RuntimeFormatContext,
}

#[derive(Clone, Debug)]
enum RuntimeProgramExecutionOwner {
    Native(Arc<RuntimePlan>),
    Awbc(AwbcProductProgram),
}

/// Keeps the selected executor's typed failure and its source chain intact.
#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum RuntimeProgramExecutionError {
    #[error(transparent)]
    Native(#[from] Box<RuntimeEvalError>),
    #[error(transparent)]
    Awbc(#[from] Box<crate::awbc::vm::VmError>),
}

impl RuntimeProgramExecutionContext {
    #[must_use]
    pub fn native(program: Arc<RuntimePlan>, format_context: RuntimeFormatContext) -> Self {
        Self {
            program: RuntimeProgramExecutionOwner::Native(program),
            format_context,
        }
    }

    #[must_use]
    pub fn awbc(program: AwbcProductProgram, format_context: RuntimeFormatContext) -> Self {
        Self {
            program: RuntimeProgramExecutionOwner::Awbc(program),
            format_context,
        }
    }

    /// Borrows semantic types from this invocation's actual executable.
    #[must_use]
    pub fn types(&self) -> RuntimeProgramTypes<'_> {
        match &self.program {
            RuntimeProgramExecutionOwner::Native(program) => RuntimeProgramTypes::Plan(program),
            RuntimeProgramExecutionOwner::Awbc(program) => RuntimeProgramTypes::Awbc(program),
        }
    }

    /// Derives the nominal type/value-codec lease; it is not execution input.
    #[must_use]
    pub fn program_owner(&self) -> RuntimeProgramOwner {
        match &self.program {
            RuntimeProgramExecutionOwner::Native(program) => {
                RuntimeProgramOwner::Plan(Arc::clone(program))
            }
            RuntimeProgramExecutionOwner::Awbc(program) => {
                RuntimeProgramOwner::Awbc(program.program_arc())
            }
        }
    }

    #[must_use]
    pub const fn format_context(&self) -> &RuntimeFormatContext {
        &self.format_context
    }

    /// Executes under the retained authority, without readmission, owner
    /// reconstruction or a formatter default at a nested call boundary.
    pub fn evaluate_pure_program(
        &self,
        program: RuntimePureProgramId,
        args: &[RuntimeValue],
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeProgramExecutionError> {
        match &self.program {
            RuntimeProgramExecutionOwner::Native(plan) => {
                super::program::evaluate_pure_program_with_format_context(
                    plan,
                    program,
                    args,
                    &self.format_context,
                    backend,
                )
                .map_err(RuntimeProgramExecutionError::Native)
            }
            RuntimeProgramExecutionOwner::Awbc(admitted) => {
                crate::awbc::product_step::evaluate_admitted_pure_program_with_format_context(
                    admitted,
                    program,
                    args,
                    &self.format_context,
                    backend,
                )
                .map_err(RuntimeProgramExecutionError::Awbc)
            }
        }
    }
}
