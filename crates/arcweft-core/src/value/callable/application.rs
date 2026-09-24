//! Checked group activation from the immutable program-owned state grammar.

use crate::awbc::schema::AwbcFunctionId;
use crate::entry::RuntimeSchemaLimits;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableStateDefinition, RuntimeCallableTransition,
};
use crate::runtime_id::RuntimeFunctionSiteId;
use crate::task::RuntimeProgramOwner;

use super::{RuntimeCallableValue, RuntimeCallableValueError, RuntimeValue};

/// A code reference already selected by an admitted state. It carries no
/// alternate capture or parameter model.
#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeCallableBodyReference {
    Plan(RuntimeFunctionSiteId),
    Awbc(AwbcFunctionId),
}

#[derive(Debug)]
pub(crate) struct RuntimeCallableInvocation {
    pub body: RuntimeCallableBodyReference,
    pub captures: Vec<RuntimeValue>,
    pub arguments: Vec<RuntimeValue>,
}

#[derive(Debug)]
pub(crate) enum RuntimeCallableApplication {
    Complete(RuntimeValue),
    Invoke(RuntimeCallableInvocation),
    AttachedDefault(RuntimeCallableInvocation),
}

impl RuntimeCallableValue {
    /// Checks the entire logical input row before selecting retention or code.
    /// Source evaluation and physical-to-logical materialization already ended.
    pub(crate) fn prepare_group(
        &self,
        arguments: &[RuntimeValue],
        attached: Option<RuntimeValue>,
    ) -> Result<RuntimeCallableApplication, RuntimeCallableValueError> {
        self.prepare_group_inner(arguments, attached, false)
    }

    /// Resumes the same checked application after its omitted default returns.
    /// The supplied value is a binding value, not another source expression.
    pub(crate) fn complete_group_default(
        &self,
        arguments: &[RuntimeValue],
        attached: RuntimeValue,
    ) -> Result<RuntimeCallableApplication, RuntimeCallableValueError> {
        self.prepare_group_inner(arguments, Some(attached), true)
    }

    fn prepare_group_inner(
        &self,
        arguments: &[RuntimeValue],
        attached: Option<RuntimeValue>,
        default_completed: bool,
    ) -> Result<RuntimeCallableApplication, RuntimeCallableValueError> {
        let absent = || RuntimeCallableValueError::MissingState { state: self.state };
        let missing_type = || RuntimeCallableValueError::MissingType { state: self.state };
        match &self.owner {
            RuntimeProgramOwner::Plan(plan) => {
                let state = plan.callable_states().get(self.state).ok_or_else(absent)?;
                self.prepare_definition(
                    state,
                    arguments,
                    attached,
                    default_completed,
                    |ty| {
                        plan.type_table()
                            .get(ty)
                            .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity)
                            .ok_or_else(missing_type)
                    },
                    RuntimeCallableBodyReference::Plan,
                )
            }
            RuntimeProgramOwner::Awbc(program) => {
                let state = program
                    .callable_states
                    .get(self.state.index())
                    .ok_or_else(absent)?;
                self.prepare_definition(
                    state,
                    arguments,
                    attached,
                    default_completed,
                    |ty| {
                        program
                            .runtime_types
                            .get(ty.index())
                            .map(crate::awbc::schema::AwbcRuntimeType::semantic_identity)
                            .ok_or_else(missing_type)
                    },
                    RuntimeCallableBodyReference::Awbc,
                )
            }
        }
    }

    fn prepare_definition<T: Copy, F: Copy>(
        &self,
        state: &RuntimeCallableStateDefinition<T, F>,
        arguments: &[RuntimeValue],
        attached: Option<RuntimeValue>,
        default_completed: bool,
        semantic: impl Fn(T) -> Result<RuntimeSemanticTypeId, RuntimeCallableValueError>,
        body: impl Fn(F) -> RuntimeCallableBodyReference,
    ) -> Result<RuntimeCallableApplication, RuntimeCallableValueError> {
        if state.parameters.len() != arguments.len() {
            return Err(RuntimeCallableValueError::ArgumentCount {
                state: self.state,
                expected: state.parameters.len(),
                actual: arguments.len(),
            });
        }
        let validate = |ty: T, value: &RuntimeValue, position: usize| {
            self.owner
                .types()
                .validate_live_value(semantic(ty)?, value, RuntimeSchemaLimits::engine_default())
                .map_err(|error| RuntimeCallableValueError::ArgumentType {
                    state: self.state,
                    position,
                    error: Box::new(error),
                })
        };
        for (position, (input, value)) in state.parameters.iter().zip(arguments).enumerate() {
            validate(input.binding_ty, value, position)?;
        }
        if default_completed
            && !matches!(
                state.attached,
                RuntimeCallableAttachedContract::Defaulted { .. }
            )
        {
            return Err(RuntimeCallableValueError::InputProjection { state: self.state });
        }
        let attached = match (&state.attached, attached) {
            (RuntimeCallableAttachedContract::None, None) => None,
            (RuntimeCallableAttachedContract::None, Some(_)) => {
                return Err(RuntimeCallableValueError::ArgumentCount {
                    state: self.state,
                    expected: arguments.len(),
                    actual: arguments.len() + 1,
                });
            }
            (RuntimeCallableAttachedContract::Required { ty }, Some(value))
            | (RuntimeCallableAttachedContract::Defaulted { ty, .. }, Some(value)) => {
                validate(*ty, &value, arguments.len())?;
                Some(value)
            }
            (RuntimeCallableAttachedContract::Required { .. }, None) => {
                return Err(RuntimeCallableValueError::RequiredAttached { state: self.state });
            }
            (RuntimeCallableAttachedContract::Optional { value: ty, binding }, value) => {
                if let Some(value) = &value {
                    validate(*ty, value, arguments.len())?;
                }
                let value = value.map_or_else(RuntimeValue::option_none, RuntimeValue::option_some);
                validate(*binding, &value, arguments.len())?;
                Some(value)
            }
            (RuntimeCallableAttachedContract::Defaulted { default, .. }, None) => {
                let captures = self.project_inputs(&default.captures, arguments, None)?;
                return Ok(RuntimeCallableApplication::AttachedDefault(
                    RuntimeCallableInvocation {
                        body: body(default.function),
                        captures,
                        arguments: Vec::new(),
                    },
                ));
            }
        };
        match &state.transition {
            RuntimeCallableTransition::Retain { state, values } => {
                let values = self.project_inputs(values, arguments, attached.as_ref())?;
                Ok(RuntimeCallableApplication::Complete(
                    RuntimeValue::Callable(Self::try_new(self.owner.clone(), *state, values)?),
                ))
            }
            RuntimeCallableTransition::Invoke {
                function,
                captures,
                arguments: sources,
            } => {
                let captures = self.project_inputs(captures, arguments, attached.as_ref())?;
                let arguments = self.project_inputs(sources, arguments, attached.as_ref())?;
                Ok(RuntimeCallableApplication::Invoke(
                    RuntimeCallableInvocation {
                        body: body(*function),
                        captures,
                        arguments,
                    },
                ))
            }
        }
    }
}
