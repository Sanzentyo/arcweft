//! Owned program continuation inputs. The executor remains the resource owner.

use crate::task::RuntimeProgramOwner;
use crate::value::{AwbcRuntimeValueSnapshot, RuntimeValue};
use thiserror::Error;

/// Positional input to another program on the same retained executor.
/// The previous result is consumed once together with its existing context.
#[derive(Debug, PartialEq)]
pub enum RuntimeProgramInput {
    Detached(RuntimeValue),
    PreviousResult,
}

#[derive(Debug, Error)]
pub enum RuntimeProgramContinuationFailure {
    #[error("program continuation requires a completed result and no active work")]
    NotCompleted,
    #[error("the previous result must occur exactly once, found {count} uses")]
    ResultUseCount { count: usize },
    #[error("detached input {position} cannot transfer custody: {source}")]
    DetachedInput {
        position: usize,
        #[source]
        source: crate::value::ownership::RuntimeDetachedValueError,
    },
    #[error(transparent)]
    Native(#[from] crate::value::RuntimeEvalError),
    #[error(transparent)]
    Awbc(#[from] crate::awbc::product_step::AwbcProductStepBuildError),
    #[error(transparent)]
    Fiber(#[from] crate::awbc::fiber::FiberStateError),
    #[error(transparent)]
    Custody(#[from] crate::line_task::LineRuntimeError),
    #[error("cannot prepare the inert continuation rollback: {message}")]
    Snapshot { message: String },
}

/// Rejection returns the executor and every detached input, including after
/// failed custody reconciliation. No host execution occurs at this boundary.
#[derive(Debug)]
pub struct RuntimeProgramContinuationError<E> {
    pub(crate) reason: RuntimeProgramContinuationFailure,
    pub(crate) executor: Box<E>,
    pub(crate) inputs: Vec<RuntimeProgramInput>,
}

impl<E> RuntimeProgramContinuationError<E> {
    pub fn into_parts(
        self,
    ) -> (
        RuntimeProgramContinuationFailure,
        E,
        Vec<RuntimeProgramInput>,
    ) {
        (self.reason, *self.executor, self.inputs)
    }
}

pub(crate) fn input_refs<'a>(
    inputs: &'a [RuntimeProgramInput],
    result: &'a RuntimeValue,
    producers: &crate::task::NeedProducerRegistry,
) -> Result<Vec<&'a RuntimeValue>, RuntimeProgramContinuationFailure> {
    let count = inputs
        .iter()
        .filter(|input| matches!(input, RuntimeProgramInput::PreviousResult))
        .count();
    if count != 1 {
        return Err(RuntimeProgramContinuationFailure::ResultUseCount { count });
    }
    inputs
        .iter()
        .enumerate()
        .map(|(position, input)| match input {
            RuntimeProgramInput::PreviousResult => Ok(result),
            RuntimeProgramInput::Detached(value) => {
                value
                    .validate_detached_custody_for(Some(&|need| {
                        producers.launch_for_need(need).is_some()
                    }))
                    .map_err(|source| RuntimeProgramContinuationFailure::DetachedInput {
                        position,
                        source,
                    })?;
                Ok(value)
            }
        })
        .collect()
}

pub(crate) fn into_values(
    inputs: Vec<RuntimeProgramInput>,
    result: RuntimeValue,
) -> Vec<RuntimeValue> {
    let mut result = Some(result);
    inputs
        .into_iter()
        .map(|input| match input {
            RuntimeProgramInput::Detached(value) => value,
            RuntimeProgramInput::PreviousResult => result
                .take()
                .expect("the complete borrowed input proof admits one result use"),
        })
        .collect()
}

pub(crate) enum RuntimeProgramInputRollback {
    Detached(AwbcRuntimeValueSnapshot),
    PreviousResult,
}

impl RuntimeProgramInputRollback {
    pub(crate) fn capture(
        inputs: &[RuntimeProgramInput],
        owner: &RuntimeProgramOwner,
    ) -> Result<Vec<Self>, RuntimeProgramContinuationFailure> {
        inputs
            .iter()
            .map(|input| match input {
                RuntimeProgramInput::PreviousResult => Ok(Self::PreviousResult),
                RuntimeProgramInput::Detached(value) => {
                    AwbcRuntimeValueSnapshot::from_runtime_value_for_program(value, owner)
                        .map(Self::Detached)
                        .map_err(|error| RuntimeProgramContinuationFailure::Snapshot {
                            message: error.to_string(),
                        })
                }
            })
            .collect()
    }

    pub(crate) fn restore(
        inputs: Vec<Self>,
        owner: &RuntimeProgramOwner,
    ) -> Vec<RuntimeProgramInput> {
        inputs
            .into_iter()
            .map(|input| match input {
                Self::PreviousResult => RuntimeProgramInput::PreviousResult,
                Self::Detached(value) => RuntimeProgramInput::Detached(
                    value.into_runtime_value_for_program(owner).expect(
                        "a captured input image restores under its unchanged program owner",
                    ),
                ),
            })
            .collect()
    }
}
