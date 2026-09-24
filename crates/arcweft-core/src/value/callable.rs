//! Immutable callable values leased to one program-owned state definition.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::entry::RuntimeSchemaLimits;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableInputSource, RuntimeCallableParameterKind,
    RuntimeCallableRetainedRole, RuntimeCallableTransition,
};
use crate::runtime_id::RuntimeCallableStateId;
use crate::task::RuntimeProgramOwner;

use super::RuntimeValue;

mod application;
mod specialization;
#[cfg(test)]
mod tests;
pub(crate) use application::{
    RuntimeCallableApplication, RuntimeCallableBodyReference, RuntimeCallableInvocation,
};

/// A callable retains values, never a second copy of its ABI or code contract.
#[derive(Clone)]
pub struct RuntimeCallableValue {
    owner: RuntimeProgramOwner,
    state: RuntimeCallableStateId,
    retained: Box<[RuntimeValue]>,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimeCallableValueError {
    #[error("callable state {state} requires a checked type specialization before application")]
    RequiresSpecialization { state: RuntimeCallableStateId },
    #[error("callable state {state} is absent from its program")]
    MissingState { state: RuntimeCallableStateId },
    #[error("callable state {state} refers to an absent type")]
    MissingType { state: RuntimeCallableStateId },
    #[error("callable value belongs to another executable program")]
    ForeignProgram,
    #[error("callable specialization {specialization} is absent from its program")]
    MissingSpecialization {
        specialization: crate::runtime_id::RuntimeCallableSpecializationId,
    },
    #[error("callable state {state} is not a source of specialization {specialization}")]
    SpecializationSource {
        specialization: crate::runtime_id::RuntimeCallableSpecializationId,
        state: RuntimeCallableStateId,
    },
    #[error("callable state {actual} does not match checked input state {expected}")]
    UnexpectedState {
        expected: RuntimeCallableStateId,
        actual: RuntimeCallableStateId,
    },
    #[error("callable state {state} retains {actual} values, expected {expected}")]
    RetainedCount {
        state: RuntimeCallableStateId,
        expected: usize,
        actual: usize,
    },
    #[error("callable state {state} retained value {position} is invalid: {error}")]
    RetainedType {
        state: RuntimeCallableStateId,
        position: usize,
        error: Box<crate::program_types::RuntimeProgramTypeError>,
    },
    #[error("callable state {state} argument count is {actual}, expected {expected}")]
    ArgumentCount {
        state: RuntimeCallableStateId,
        expected: usize,
        actual: usize,
    },
    #[error("callable state {state} argument {position} is invalid: {error}")]
    ArgumentType {
        state: RuntimeCallableStateId,
        position: usize,
        error: Box<crate::program_types::RuntimeProgramTypeError>,
    },
    #[error("callable state {state} requires an attached argument")]
    RequiredAttached { state: RuntimeCallableStateId },
    #[error("callable state {state} has an invalid input projection")]
    InputProjection { state: RuntimeCallableStateId },
    #[error("callable state {state} cannot be evaluated by a pure-expression consumer")]
    RequiresControlTransfer { state: RuntimeCallableStateId },
    #[error("callable state {state} has no admitted partial application for {arguments} arguments")]
    MissingPartial {
        state: RuntimeCallableStateId,
        arguments: usize,
    },
}

impl RuntimeCallableValue {
    /// Validates the complete retained layout before publishing a value.
    pub fn try_new(
        owner: RuntimeProgramOwner,
        state: RuntimeCallableStateId,
        retained: impl IntoIterator<Item = RuntimeValue>,
    ) -> Result<Self, RuntimeCallableValueError> {
        let value = Self {
            owner,
            state,
            retained: retained.into_iter().collect(),
        };
        value.validate_retained()?;
        Ok(value)
    }

    #[must_use]
    pub const fn owner(&self) -> &RuntimeProgramOwner {
        &self.owner
    }

    #[must_use]
    pub const fn state(&self) -> RuntimeCallableStateId {
        self.state
    }

    #[must_use]
    pub const fn retained(&self) -> &[RuntimeValue] {
        &self.retained
    }

    /// Lexical captures are distinguished from parameters already retained by
    /// earlier applications. Content effect admission uses this exact layout.
    pub(crate) fn capture_count(&self) -> usize {
        match &self.owner {
            RuntimeProgramOwner::Plan(plan) => {
                plan.callable_states().get(self.state).map(|state| {
                    state
                        .retained
                        .iter()
                        .filter(|row| {
                            matches!(row.role, RuntimeCallableRetainedRole::Capture { .. })
                        })
                        .count()
                })
            }
            RuntimeProgramOwner::Awbc(program) => program
                .callable_states
                .get(self.state.index())
                .map(|state| {
                    state
                        .retained
                        .iter()
                        .filter(|row| {
                            matches!(row.role, RuntimeCallableRetainedRole::Capture { .. })
                        })
                        .count()
                }),
        }
        .expect("a live callable leases its admitted immutable program state")
    }

    /// Selects a sealed positional prefix row. No type or executable state is
    /// synthesized at runtime; every retained coordinate was admitted with the
    /// program. Named applications use their checked formal projection before
    /// reaching this positional callable-arrow operation.
    pub(crate) fn try_bind_prefix(
        &self,
        arguments: &[RuntimeValue],
    ) -> Result<Self, RuntimeCallableValueError> {
        if arguments.is_empty() {
            return Ok(self.clone());
        }
        let absent = || RuntimeCallableValueError::MissingState { state: self.state };
        let missing = || RuntimeCallableValueError::MissingPartial {
            state: self.state,
            arguments: arguments.len(),
        };
        let (target, sources) = match &self.owner {
            RuntimeProgramOwner::Plan(plan) => {
                let state = plan.callable_states().get(self.state).ok_or_else(absent)?;
                let partial = state
                    .partials
                    .iter()
                    .find(|row| {
                        row.parameters.iter().copied().eq(state
                            .parameters
                            .iter()
                            .take(arguments.len())
                            .map(|input| input.coordinate))
                            && row.parameters.len() == arguments.len()
                    })
                    .ok_or_else(missing)?;
                (partial.state, partial.values.as_ref())
            }
            RuntimeProgramOwner::Awbc(program) => {
                let state = program
                    .callable_states
                    .get(self.state.index())
                    .ok_or_else(absent)?;
                let partial = state
                    .partials
                    .iter()
                    .find(|row| {
                        row.parameters.iter().copied().eq(state
                            .parameters
                            .iter()
                            .take(arguments.len())
                            .map(|input| input.coordinate))
                            && row.parameters.len() == arguments.len()
                    })
                    .ok_or_else(missing)?;
                (partial.state, partial.values.as_ref())
            }
        };
        let inputs = self.ordinary_abi_inputs()?;
        let arguments = self.materialize_ordinary_inputs(&inputs, arguments)?;
        Self::try_new(
            self.owner.clone(),
            target,
            self.project_inputs(sources, &arguments, None)?,
        )
    }

    pub fn function_type(&self) -> Result<RuntimeSemanticTypeId, RuntimeCallableValueError> {
        let absent = || RuntimeCallableValueError::MissingState { state: self.state };
        let missing_type = || RuntimeCallableValueError::MissingType { state: self.state };
        match &self.owner {
            RuntimeProgramOwner::Plan(plan) => {
                let state = plan.callable_states().get(self.state).ok_or_else(absent)?;
                plan.type_table()
                    .get(state.function_type)
                    .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity)
                    .ok_or_else(missing_type)
            }
            RuntimeProgramOwner::Awbc(program) => {
                let state = program
                    .callable_states
                    .get(self.state.index())
                    .ok_or_else(absent)?;
                program
                    .runtime_types
                    .get(state.function_type.index())
                    .map(crate::awbc::schema::AwbcRuntimeType::semantic_identity)
                    .ok_or_else(missing_type)
            }
        }
    }

    /// Number of ordinary inputs still required by this callable arrow.
    /// Attached content is supplied through the separate group-application ABI.
    pub fn remaining_arity(&self) -> Result<usize, RuntimeCallableValueError> {
        let absent = || RuntimeCallableValueError::MissingState { state: self.state };
        match &self.owner {
            RuntimeProgramOwner::Plan(plan) => {
                let state = plan.callable_states().get(self.state).ok_or_else(absent)?;
                Ok(state.parameters.len())
            }
            RuntimeProgramOwner::Awbc(program) => {
                let state = program
                    .callable_states
                    .get(self.state.index())
                    .ok_or_else(absent)?;
                Ok(state.parameters.len())
            }
        }
    }
    pub fn validate_for_owner(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> Result<(), RuntimeCallableValueError> {
        if !self.owner.same_program(owner) {
            return Err(RuntimeCallableValueError::ForeignProgram);
        }
        self.validate_retained()
    }

    /// Materializes one positional callable arrow into the declaration's
    /// logical bindings. A rest arrow input is an element, not an existing pack.
    /// Attached content is not an arrow input; ordinary application omits it.
    pub(crate) fn materialize_arrow_arguments(
        &self,
        values: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, RuntimeCallableValueError> {
        let inputs = self.ordinary_abi_inputs()?;
        let arity = inputs.len();
        if values.len() != arity {
            return Err(RuntimeCallableValueError::ArgumentCount {
                state: self.state,
                expected: arity,
                actual: values.len(),
            });
        }
        self.materialize_ordinary_inputs(&inputs, values)
    }

    fn ordinary_abi_inputs(
        &self,
    ) -> Result<Vec<(RuntimeCallableParameterKind, RuntimeSemanticTypeId)>, RuntimeCallableValueError>
    {
        let absent = || RuntimeCallableValueError::MissingState { state: self.state };
        let missing = || RuntimeCallableValueError::MissingType { state: self.state };
        match &self.owner {
            RuntimeProgramOwner::Plan(plan) => plan
                .callable_states()
                .get(self.state)
                .ok_or_else(absent)?
                .parameters
                .iter()
                .map(|input| {
                    Ok((
                        input.kind,
                        plan.type_table()
                            .get(input.abi_ty)
                            .ok_or_else(missing)?
                            .semantic_identity(),
                    ))
                })
                .collect(),
            RuntimeProgramOwner::Awbc(program) => program
                .callable_states
                .get(self.state.index())
                .ok_or_else(absent)?
                .parameters
                .iter()
                .map(|input| {
                    Ok((
                        input.kind,
                        program
                            .runtime_types
                            .get(input.abi_ty.index())
                            .ok_or_else(missing)?
                            .semantic_identity(),
                    ))
                })
                .collect(),
        }
    }

    /// Prefix and complete applications retain the same logical binding ABI.
    fn materialize_ordinary_inputs(
        &self,
        inputs: &[(RuntimeCallableParameterKind, RuntimeSemanticTypeId)],
        values: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, RuntimeCallableValueError> {
        if values.len() > inputs.len() {
            return Err(RuntimeCallableValueError::ArgumentCount {
                state: self.state,
                expected: inputs.len(),
                actual: values.len(),
            });
        }
        inputs
            .iter()
            .zip(values)
            .enumerate()
            .map(|(position, ((kind, ty), value))| {
                self.owner
                    .types()
                    .validate_live_value(*ty, value, RuntimeSchemaLimits::engine_default())
                    .map_err(|error| RuntimeCallableValueError::ArgumentType {
                        state: self.state,
                        position,
                        error: Box::new(error),
                    })?;
                Ok(match kind {
                    RuntimeCallableParameterKind::Fixed => value.clone(),
                    RuntimeCallableParameterKind::Rest => {
                        super::runtime_sequence_values(vec![value.clone()])
                    }
                })
            })
            .collect()
    }

    pub(crate) fn validate_retained(&self) -> Result<(), RuntimeCallableValueError> {
        let absent = || RuntimeCallableValueError::MissingState { state: self.state };
        let missing_type = || RuntimeCallableValueError::MissingType { state: self.state };
        let types = match &self.owner {
            RuntimeProgramOwner::Plan(plan) => plan
                .callable_states()
                .get(self.state)
                .ok_or_else(absent)?
                .retained
                .iter()
                .map(|input| {
                    plan.type_table()
                        .get(input.ty)
                        .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity)
                        .ok_or_else(missing_type)
                })
                .collect::<Result<Vec<_>, _>>()?,
            RuntimeProgramOwner::Awbc(program) => program
                .callable_states
                .get(self.state.index())
                .ok_or_else(absent)?
                .retained
                .iter()
                .map(|input| {
                    program
                        .runtime_types
                        .get(input.ty.index())
                        .map(crate::awbc::schema::AwbcRuntimeType::semantic_identity)
                        .ok_or_else(missing_type)
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        if types.len() != self.retained.len() {
            return Err(RuntimeCallableValueError::RetainedCount {
                state: self.state,
                expected: types.len(),
                actual: self.retained.len(),
            });
        }
        for (position, (ty, value)) in types.into_iter().zip(&self.retained).enumerate() {
            self.owner
                .types()
                .validate_live_value(ty, value, RuntimeSchemaLimits::engine_default())
                .map_err(|error| RuntimeCallableValueError::RetainedType {
                    state: self.state,
                    position,
                    error: Box::new(error),
                })?;
        }
        Ok(())
    }

    /// Applies an admitted formal projection to already evaluated input values.
    pub(crate) fn project_inputs(
        &self,
        sources: &[RuntimeCallableInputSource],
        arguments: &[RuntimeValue],
        attached: Option<&RuntimeValue>,
    ) -> Result<Vec<RuntimeValue>, RuntimeCallableValueError> {
        sources
            .iter()
            .map(|source| {
                match source {
                    RuntimeCallableInputSource::Retained { position } => {
                        self.retained.get(*position as usize)
                    }
                    RuntimeCallableInputSource::Argument { position } => {
                        arguments.get(*position as usize)
                    }
                    RuntimeCallableInputSource::Attached => attached,
                }
                .cloned()
                .ok_or(RuntimeCallableValueError::InputProjection { state: self.state })
            })
            .collect()
    }

    #[must_use]
    pub fn is_structured_executable_callback(&self) -> bool {
        let RuntimeProgramOwner::Plan(plan) = &self.owner else {
            return false;
        };
        plan.callable_states().get(self.state).is_some_and(|state| {
            state.parameters.is_empty()
                && matches!(state.attached, RuntimeCallableAttachedContract::None)
                && matches!(plan.checked_type(state.result), Ok(Some(crate::pattern::RuntimeCheckedType::Unit)))
                && matches!(&state.transition, RuntimeCallableTransition::Invoke { function, arguments, .. }
                    if arguments.is_empty() && plan.function_sites().get(*function).is_some_and(|site| site.body().executable().is_some()))
        })
    }
}

impl PartialEq for RuntimeCallableValue {
    fn eq(&self, other: &Self) -> bool {
        self.owner.same_program(&other.owner)
            && self.state == other.state
            && self.retained == other.retained
    }
}

impl fmt::Debug for RuntimeCallableValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeCallableValue")
            .field("state", &self.state)
            .field("retained", &self.retained)
            .finish_non_exhaustive()
    }
}

impl Serialize for RuntimeCallableValue {
    fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom(
            "callable values require a program-bound snapshot",
        ))
    }
}

impl<'de> Deserialize<'de> for RuntimeCallableValue {
    fn deserialize<D: Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
        Err(serde::de::Error::custom(
            "callable values require a program-bound snapshot",
        ))
    }
}
