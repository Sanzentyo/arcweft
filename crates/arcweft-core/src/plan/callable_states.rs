//! Program-owned callable states and reusable group transitions.
//!
//! State definitions contain typed formal coordinates and code references.
//! They never contain caller expressions, registers, result destinations, or
//! retained runtime values. The same grammar is used by native plans and AWBC.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

use crate::runtime_id::{RuntimeCallableStateId, RuntimeFunctionSiteId, RuntimePlanTypeId};

/// A declared formal parameter, independent of a physical call operand.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCallableParameterCoordinate {
    pub group: u32,
    pub parameter: u32,
}

/// Exact application position. A count alone cannot identify a named prefix.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeCallablePosition {
    Unapplied,
    WithinGroup {
        group: u32,
        bound: Box<[RuntimeCallableParameterCoordinate]>,
    },
    AfterGroup {
        completed: u32,
    },
}

/// Semantic role of one immutable retained value.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeCallableRetainedRole {
    Capture { position: u32 },
    Parameter(RuntimeCallableParameterCoordinate),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCallableRetainedInput<T> {
    pub role: RuntimeCallableRetainedRole,
    pub ty: T,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeCallableParameterKind {
    Fixed,
    Rest,
}

/// The callable arrow exposes ABI types; a rest parameter binds a typed pack.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCallableParameterInput<T> {
    pub coordinate: RuntimeCallableParameterCoordinate,
    pub kind: RuntimeCallableParameterKind,
    pub abi_ty: T,
    pub binding_ty: T,
}

/// An input of a reusable state transition, after caller-side materialization.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeCallableInputSource {
    Retained { position: u32 },
    Argument { position: u32 },
    Attached,
}

/// Default execution is attached to its terminal callable, never its producer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeCallableDefault<F> {
    RequiresSpecialization,
    Body {
        function: F,
        captures: Box<[RuntimeCallableInputSource]>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeCallableAttachedContract<T, F> {
    None,
    Required {
        ty: T,
    },
    Optional {
        value: T,
        binding: T,
    },
    Defaulted {
        ty: T,
        default: RuntimeCallableDefault<F>,
    },
}

/// Applying a group either retains a new state or invokes an actual body.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeCallableTransition<F, S = RuntimeCallableStateId> {
    /// A residual scheme owns no executable body until a checked type use
    /// chooses an admitted specialization of this same state.
    RequiresSpecialization,
    Retain {
        state: S,
        values: Box<[RuntimeCallableInputSource]>,
    },
    Invoke {
        function: F,
        captures: Box<[RuntimeCallableInputSource]>,
        arguments: Box<[RuntimeCallableInputSource]>,
    },
}

/// An admitted application of a proper subset of the current group's formals.
/// Formal coordinates, rather than an argument count, identify the prefix.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCallablePartialTransition<S = RuntimeCallableStateId> {
    pub parameters: Box<[RuntimeCallableParameterCoordinate]>,
    pub state: S,
    pub values: Box<[RuntimeCallableInputSource]>,
}

/// The only reusable execution contract for one callable value state.
/// Type and body references are program-local in both backends.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCallableStateDefinition<T, F, S = RuntimeCallableStateId> {
    pub function_type: T,
    pub origin: S,
    pub position: RuntimeCallablePosition,
    pub retained: Box<[RuntimeCallableRetainedInput<T>]>,
    pub parameters: Box<[RuntimeCallableParameterInput<T>]>,
    pub result: T,
    pub attached: RuntimeCallableAttachedContract<T, F>,
    pub transition: RuntimeCallableTransition<F, S>,
    pub partials: Box<[RuntimeCallablePartialTransition<S>]>,
}

impl<T, F, S> RuntimeCallableStateDefinition<T, F, S> {
    /// Rewrites all references together at the program construction boundary.
    pub fn try_map<U, G, R, E>(
        self,
        mut ty: impl FnMut(T) -> Result<U, E>,
        mut function: impl FnMut(F) -> Result<G, E>,
        mut state: impl FnMut(S) -> Result<R, E>,
    ) -> Result<RuntimeCallableStateDefinition<U, G, R>, E> {
        Ok(RuntimeCallableStateDefinition {
            function_type: ty(self.function_type)?,
            origin: state(self.origin)?,
            position: self.position,
            retained: self
                .retained
                .into_vec()
                .into_iter()
                .map(|input| {
                    Ok(RuntimeCallableRetainedInput {
                        role: input.role,
                        ty: ty(input.ty)?,
                    })
                })
                .collect::<Result<Box<[_]>, E>>()?,
            parameters: self
                .parameters
                .into_vec()
                .into_iter()
                .map(|input| {
                    Ok(RuntimeCallableParameterInput {
                        coordinate: input.coordinate,
                        kind: input.kind,
                        abi_ty: ty(input.abi_ty)?,
                        binding_ty: ty(input.binding_ty)?,
                    })
                })
                .collect::<Result<Box<[_]>, E>>()?,
            result: ty(self.result)?,
            attached: match self.attached {
                RuntimeCallableAttachedContract::None => RuntimeCallableAttachedContract::None,
                RuntimeCallableAttachedContract::Required { ty: value } => {
                    RuntimeCallableAttachedContract::Required { ty: ty(value)? }
                }
                RuntimeCallableAttachedContract::Optional { value, binding } => {
                    RuntimeCallableAttachedContract::Optional {
                        value: ty(value)?,
                        binding: ty(binding)?,
                    }
                }
                RuntimeCallableAttachedContract::Defaulted { ty: value, default } => {
                    RuntimeCallableAttachedContract::Defaulted {
                        ty: ty(value)?,
                        default: match default {
                            RuntimeCallableDefault::RequiresSpecialization => {
                                RuntimeCallableDefault::RequiresSpecialization
                            }
                            RuntimeCallableDefault::Body {
                                function: target,
                                captures,
                            } => RuntimeCallableDefault::Body {
                                function: function(target)?,
                                captures,
                            },
                        },
                    }
                }
            },
            transition: match self.transition {
                RuntimeCallableTransition::RequiresSpecialization => {
                    RuntimeCallableTransition::RequiresSpecialization
                }
                RuntimeCallableTransition::Retain {
                    state: next,
                    values,
                } => RuntimeCallableTransition::Retain {
                    state: state(next)?,
                    values,
                },
                RuntimeCallableTransition::Invoke {
                    function: target,
                    captures,
                    arguments,
                } => RuntimeCallableTransition::Invoke {
                    function: function(target)?,
                    captures,
                    arguments,
                },
            },
            partials: self
                .partials
                .into_vec()
                .into_iter()
                .map(|partial| {
                    Ok(RuntimeCallablePartialTransition {
                        parameters: partial.parameters,
                        state: state(partial.state)?,
                        values: partial.values,
                    })
                })
                .collect::<Result<Box<[_]>, E>>()?,
        })
    }
}

impl<T: Eq + Clone, F: Eq + Clone, S: Eq + Clone> RuntimeCallableStateDefinition<T, F, S> {
    /// Verifies that a partial row preserves the source's exact code/default
    /// target and all input roles. Both native admission and AWBC verification
    /// use this projection law; matching function types alone is insufficient.
    pub fn partial_projects_to(
        &self,
        partial: &RuntimeCallablePartialTransition<S>,
        target: &Self,
    ) -> bool {
        let retained: BTreeMap<_, _> = target
            .retained
            .iter()
            .enumerate()
            .map(|(index, input)| (input.role, index as u32))
            .collect();
        let arguments: BTreeMap<_, _> = target
            .parameters
            .iter()
            .enumerate()
            .map(|(index, input)| (input.coordinate, index as u32))
            .collect();
        let project = |source| -> Option<RuntimeCallableInputSource> {
            match source {
                RuntimeCallableInputSource::Retained { position } => retained
                    .get(&self.retained.get(position as usize)?.role)
                    .map(|position| RuntimeCallableInputSource::Retained {
                        position: *position,
                    }),
                RuntimeCallableInputSource::Argument { position } => {
                    let coordinate = self.parameters.get(position as usize)?.coordinate;
                    if partial.parameters.contains(&coordinate) {
                        retained
                            .get(&RuntimeCallableRetainedRole::Parameter(coordinate))
                            .map(|position| RuntimeCallableInputSource::Retained {
                                position: *position,
                            })
                    } else {
                        arguments.get(&coordinate).map(|position| {
                            RuntimeCallableInputSource::Argument {
                                position: *position,
                            }
                        })
                    }
                }
                RuntimeCallableInputSource::Attached => Some(RuntimeCallableInputSource::Attached),
            }
        };
        let project_row = |row: &[RuntimeCallableInputSource]| {
            row.iter()
                .copied()
                .map(project)
                .collect::<Option<Box<[_]>>>()
        };
        let attached = match &self.attached {
            RuntimeCallableAttachedContract::Defaulted { ty, default } => {
                let default = match default {
                    RuntimeCallableDefault::RequiresSpecialization => {
                        RuntimeCallableDefault::RequiresSpecialization
                    }
                    RuntimeCallableDefault::Body { function, captures } => {
                        let Some(captures) = project_row(captures) else {
                            return false;
                        };
                        RuntimeCallableDefault::Body {
                            function: function.clone(),
                            captures,
                        }
                    }
                };
                RuntimeCallableAttachedContract::Defaulted {
                    ty: ty.clone(),
                    default,
                }
            }
            attached => attached.clone(),
        };
        let transition = match &self.transition {
            RuntimeCallableTransition::RequiresSpecialization => {
                RuntimeCallableTransition::RequiresSpecialization
            }
            RuntimeCallableTransition::Retain { state, values } => {
                let Some(values) = project_row(values) else {
                    return false;
                };
                RuntimeCallableTransition::Retain {
                    state: state.clone(),
                    values,
                }
            }
            RuntimeCallableTransition::Invoke {
                function,
                captures,
                arguments,
            } => {
                let (Some(captures), Some(arguments)) =
                    (project_row(captures), project_row(arguments))
                else {
                    return false;
                };
                RuntimeCallableTransition::Invoke {
                    function: function.clone(),
                    captures,
                    arguments,
                }
            }
        };
        target.attached == attached && target.transition == transition
    }
}

pub type RuntimeCallableState =
    RuntimeCallableStateDefinition<RuntimePlanTypeId, RuntimeFunctionSiteId>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeCallableStateTable {
    states: Box<[RuntimeCallableState]>,
}

impl RuntimeCallableStateTable {
    pub(crate) fn from_admitted(states: Vec<RuntimeCallableState>) -> Self {
        Self {
            states: states.into_boxed_slice(),
        }
    }

    #[must_use]
    pub fn get(&self, id: RuntimeCallableStateId) -> Option<&RuntimeCallableState> {
        self.states.get(id.index())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.states.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &RuntimeCallableState> {
        self.states.iter()
    }

    pub fn iter_with_ids(
        &self,
    ) -> impl ExactSizeIterator<Item = (RuntimeCallableStateId, &RuntimeCallableState)> {
        self.states.iter().enumerate().map(|(index, state)| {
            (
                RuntimeCallableStateId::for_index(index).expect("admitted callable-state index"),
                state,
            )
        })
    }

    pub(crate) fn validate_for_plan(
        &self,
        plan: &super::RuntimePlan,
    ) -> Result<(), RuntimeCallableStateError> {
        for (index, definition) in self.states.iter().enumerate() {
            let state = RuntimeCallableStateId::for_index(index)
                .ok_or(RuntimeCallableStateError::IdentityExhausted)?;
            definition.validate_for_plan(state, plan)?;
        }
        Ok(())
    }
}

impl RuntimeCallableState {
    fn validate_for_plan(
        &self,
        state: RuntimeCallableStateId,
        plan: &super::RuntimePlan,
    ) -> Result<(), RuntimeCallableStateError> {
        use super::RuntimePlanTypeProjection as Type;
        let ty = |id, role| {
            plan.type_table()
                .get(id)
                .map(super::RuntimePlanTypeDeclaration::projection)
                .ok_or(RuntimeCallableStateError::InvalidType { state, role })
        };
        let invalid_layout = || RuntimeCallableStateError::InvalidLayout { state };
        let Type::Function {
            contract,
            parameters,
            result,
        } = ty(self.function_type, "function")?
        else {
            return Err(RuntimeCallableStateError::InvalidType {
                state,
                role: "function",
            });
        };
        if parameters.len()
            != self.parameters.len()
                + usize::from(!matches!(
                    self.attached,
                    RuntimeCallableAttachedContract::None
                ))
            || result != &self.result
            || parameters
                .iter()
                .zip(&self.parameters)
                .any(|(ty, input)| *ty != input.abi_ty)
        {
            return Err(invalid_layout());
        }
        ty(self.result, "result")?;
        let origin = plan.callable_states().get(self.origin).ok_or(
            RuntimeCallableStateError::MissingState {
                state,
                role: "origin",
            },
        )?;
        if origin.origin != self.origin
            || !matches!(origin.position, RuntimeCallablePosition::Unapplied)
        {
            return Err(invalid_layout());
        }
        let group = match &self.position {
            RuntimeCallablePosition::Unapplied => 0,
            RuntimeCallablePosition::WithinGroup { group, bound } => {
                if bound.is_empty()
                    || bound.windows(2).any(|pair| pair[0] >= pair[1])
                    || bound.iter().any(|coordinate| {
                        coordinate.group != *group
                            || !self.retained.iter().any(|input| {
                                input.role == RuntimeCallableRetainedRole::Parameter(*coordinate)
                            })
                    })
                {
                    return Err(invalid_layout());
                }
                *group
            }
            RuntimeCallablePosition::AfterGroup { completed } => {
                completed.checked_add(1).ok_or_else(invalid_layout)?
            }
        };
        let mut roles = BTreeSet::new();
        for input in &self.retained {
            ty(input.ty, "retained")?;
            if !roles.insert(input.role) {
                return Err(invalid_layout());
            }
        }
        if self
            .parameters
            .windows(2)
            .any(|pair| pair[0].coordinate >= pair[1].coordinate)
        {
            return Err(invalid_layout());
        }
        for input in &self.parameters {
            if input.coordinate.group != group
                || roles.contains(&RuntimeCallableRetainedRole::Parameter(input.coordinate))
            {
                return Err(invalid_layout());
            }
            ty(input.abi_ty, "parameter ABI")?;
            let binding = ty(input.binding_ty, "parameter binding")?;
            match input.kind {
                RuntimeCallableParameterKind::Fixed if input.abi_ty == input.binding_ty => {}
                RuntimeCallableParameterKind::Rest if matches!(binding, Type::Sequence { kind: super::RuntimePlanSequenceKind::Vec, item } if *item == input.abi_ty) =>
                    {}
                _ => return Err(invalid_layout()),
            }
        }
        let attached = match &self.attached {
            RuntimeCallableAttachedContract::None => None,
            RuntimeCallableAttachedContract::Required { ty: value } => {
                ty(*value, "attached")?;
                Some(*value)
            }
            RuntimeCallableAttachedContract::Optional { value, binding } => {
                ty(*value, "attached value")?;
                if !matches!(ty(*binding, "attached binding")?, Type::Option { item, .. } if item == value)
                {
                    return Err(invalid_layout());
                }
                Some(*binding)
            }
            RuntimeCallableAttachedContract::Defaulted { ty: value, default } => {
                ty(*value, "attached default result")?;
                match default {
                    RuntimeCallableDefault::RequiresSpecialization
                        if matches!(
                            self.transition,
                            RuntimeCallableTransition::RequiresSpecialization
                        ) && !contract.binder().is_empty() => {}
                    RuntimeCallableDefault::RequiresSpecialization => return Err(invalid_layout()),
                    RuntimeCallableDefault::Body { function, captures } => {
                        self.validate_body_inputs(
                            state,
                            plan,
                            *function,
                            captures,
                            &[],
                            None,
                            *value,
                        )?;
                    }
                }
                Some(*value)
            }
        };
        let attached_abi_matches = match &self.attached {
            RuntimeCallableAttachedContract::None => true,
            RuntimeCallableAttachedContract::Required { ty } => parameters.last() == Some(ty),
            RuntimeCallableAttachedContract::Optional { binding, .. } => {
                parameters.last() == Some(binding)
            }
            RuntimeCallableAttachedContract::Defaulted { ty: expected, .. } => {
                matches!(ty(*parameters.last().ok_or_else(invalid_layout)?, "attached ABI")?, Type::Option { item, .. } if item == expected)
            }
        };
        if !attached_abi_matches {
            return Err(invalid_layout());
        }
        match &self.transition {
            RuntimeCallableTransition::RequiresSpecialization => {
                if contract.binder().is_empty() || !self.partials.is_empty() {
                    return Err(invalid_layout());
                }
            }
            RuntimeCallableTransition::Retain {
                state: target,
                values,
            } => {
                let next = plan.callable_states().get(*target).ok_or(
                    RuntimeCallableStateError::MissingState {
                        state,
                        role: "result",
                    },
                )?;
                if attached.is_some()
                    || next.origin != self.origin
                    || next.function_type != self.result
                    || next.position != (RuntimeCallablePosition::AfterGroup { completed: group })
                    || values.len() != next.retained.len()
                {
                    return Err(invalid_layout());
                }
                for (source, retained) in values.iter().zip(&next.retained) {
                    if self.input_type(state, *source, None)? != retained.ty {
                        return Err(invalid_layout());
                    }
                    let role = match *source {
                        RuntimeCallableInputSource::Retained { position } => {
                            self.retained[position as usize].role
                        }
                        RuntimeCallableInputSource::Argument { position } => {
                            RuntimeCallableRetainedRole::Parameter(
                                self.parameters[position as usize].coordinate,
                            )
                        }
                        RuntimeCallableInputSource::Attached => return Err(invalid_layout()),
                    };
                    if role != retained.role {
                        return Err(invalid_layout());
                    }
                }
            }
            RuntimeCallableTransition::Invoke {
                function,
                captures,
                arguments,
            } => {
                self.validate_body_inputs(
                    state,
                    plan,
                    *function,
                    captures,
                    arguments,
                    attached,
                    self.result,
                )?;
            }
        }
        let mut partial_coordinates = BTreeSet::new();
        for partial in &self.partials {
            let next = plan.callable_states().get(partial.state).ok_or(
                RuntimeCallableStateError::MissingState {
                    state,
                    role: "partial",
                },
            )?;
            let mut bound = match &self.position {
                RuntimeCallablePosition::WithinGroup { bound, .. } => bound.to_vec(),
                _ => Vec::new(),
            };
            if partial.parameters.is_empty()
                || partial.parameters.len() >= parameters.len()
                || partial.parameters.windows(2).any(|pair| pair[0] >= pair[1])
                || !partial_coordinates.insert(&partial.parameters)
                || partial.parameters.iter().any(|coordinate| {
                    !self
                        .parameters
                        .iter()
                        .any(|input| input.coordinate == *coordinate)
                })
            {
                return Err(invalid_layout());
            }
            bound.extend_from_slice(&partial.parameters);
            bound.sort_unstable();
            if next.origin != self.origin
                || next.result != self.result
                || next.position
                    != (RuntimeCallablePosition::WithinGroup {
                        group,
                        bound: bound.into_boxed_slice(),
                    })
                || next.parameters.iter().ne(self
                    .parameters
                    .iter()
                    .filter(|input| !partial.parameters.contains(&input.coordinate)))
                || !self.partial_projects_to(partial, next)
                || partial.values.len() != next.retained.len()
            {
                return Err(invalid_layout());
            }
            for (source, retained) in partial.values.iter().zip(&next.retained) {
                let (role, source_type) = match *source {
                    RuntimeCallableInputSource::Retained { position } => self
                        .retained
                        .get(position as usize)
                        .map(|input| (input.role, input.ty)),
                    RuntimeCallableInputSource::Argument { position } => partial
                        .parameters
                        .get(position as usize)
                        .and_then(|coordinate| {
                            self.parameters
                                .iter()
                                .find(|input| input.coordinate == *coordinate)
                        })
                        .map(|input| {
                            (
                                RuntimeCallableRetainedRole::Parameter(input.coordinate),
                                input.binding_ty,
                            )
                        }),
                    RuntimeCallableInputSource::Attached => None,
                }
                .ok_or_else(invalid_layout)?;
                if role != retained.role || source_type != retained.ty {
                    return Err(invalid_layout());
                }
            }
        }
        Ok(())
    }

    fn input_type(
        &self,
        state: RuntimeCallableStateId,
        source: RuntimeCallableInputSource,
        attached: Option<RuntimePlanTypeId>,
    ) -> Result<RuntimePlanTypeId, RuntimeCallableStateError> {
        match source {
            RuntimeCallableInputSource::Retained { position } => {
                self.retained.get(position as usize).map(|row| row.ty)
            }
            RuntimeCallableInputSource::Argument { position } => self
                .parameters
                .get(position as usize)
                .map(|row| row.binding_ty),
            RuntimeCallableInputSource::Attached => attached,
        }
        .ok_or(RuntimeCallableStateError::InvalidProjection { state })
    }

    fn validate_body_inputs(
        &self,
        state: RuntimeCallableStateId,
        plan: &super::RuntimePlan,
        target: RuntimeFunctionSiteId,
        captures: &[RuntimeCallableInputSource],
        arguments: &[RuntimeCallableInputSource],
        attached: Option<RuntimePlanTypeId>,
        result: RuntimePlanTypeId,
    ) -> Result<(), RuntimeCallableStateError> {
        let invalid = || RuntimeCallableStateError::InvalidBody { state };
        let body = plan.function_sites().get(target).ok_or_else(invalid)?;
        if body.result() != result
            || body.capture_inputs().count() != captures.len()
            || body.parameter_inputs().count() != arguments.len()
        {
            return Err(invalid());
        }
        for input in body.inputs() {
            let (values, position) = match input.source() {
                super::RuntimeFunctionInputSource::Capture { position } => (captures, position),
                super::RuntimeFunctionInputSource::Parameter { position } => (arguments, position),
            };
            let source = *values.get(position as usize).ok_or_else(invalid)?;
            let expected = plan
                .local_declarations()
                .get(input.input_local())
                .ok_or_else(invalid)?
                .ty();
            if self.input_type(state, source, attached)? != expected {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeCallableStateError {
    #[error("callable state identity space is exhausted")]
    IdentityExhausted,
    #[error("callable state {state} references an absent {role} state")]
    MissingState {
        state: RuntimeCallableStateId,
        role: &'static str,
    },
    #[error("callable state {state} has an invalid {role} type")]
    InvalidType {
        state: RuntimeCallableStateId,
        role: &'static str,
    },
    #[error("callable state {state} has an invalid formal or retained-value layout")]
    InvalidLayout { state: RuntimeCallableStateId },
    #[error("callable state {state} references an invalid executable body")]
    InvalidBody { state: RuntimeCallableStateId },
    #[error("callable state {state} has an invalid transition input projection")]
    InvalidProjection { state: RuntimeCallableStateId },
}
