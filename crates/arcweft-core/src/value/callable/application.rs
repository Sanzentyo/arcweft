//! Checked group activation from the immutable program-owned state grammar.

use std::collections::BTreeMap;

use crate::awbc::schema::AwbcFunctionId;
use crate::entry::RuntimeSchemaLimits;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableInputSource, RuntimeCallableParameterKind,
    RuntimeCallableStateDefinition, RuntimeCallableTransition,
};
use crate::runtime_id::{RuntimeCallableStateId, RuntimeFunctionSiteId};
use crate::task::RuntimeProgramOwner;

use super::{
    RuntimeCallableOwnedInputError, RuntimeCallableValue, RuntimeCallableValueError, RuntimeValue,
};

/// A code reference already selected by an admitted state.
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
    AttachedDefault {
        invocation: RuntimeCallableInvocation,
        pending: RuntimeCallablePendingGroup,
    },
}

/// The owner packet suspended while an attached default body executes. An
/// input used only by that body has already moved out of its indexed slot;
/// every input needed on resume remains here under its original coordinate.
#[derive(Clone, Debug)]
pub(crate) struct RuntimeCallablePendingGroup {
    owner: RuntimeProgramOwner,
    state: RuntimeCallableStateId,
    retained: Vec<Option<RuntimeValue>>,
    arguments: Vec<Option<RuntimeValue>>,
    attached: Option<RuntimeValue>,
}

/// Move-only image of a suspended default group. Snapshot code may transform
/// the values into inert images and reconstruct them only through validation.
#[derive(Debug)]
pub(crate) struct RuntimeCallablePendingGroupParts {
    pub owner: RuntimeProgramOwner,
    pub state: RuntimeCallableStateId,
    pub retained: Vec<Option<RuntimeValue>>,
    pub arguments: Vec<Option<RuntimeValue>>,
    pub attached: Option<RuntimeValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RuntimeCallablePendingRollbackImage {
    state: RuntimeCallableStateId,
    retained: Vec<Option<crate::value::AwbcRuntimeValueSnapshot>>,
    arguments: Vec<Option<crate::value::AwbcRuntimeValueSnapshot>>,
    attached: Option<crate::value::AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeCallableGroupInspection {
    Retain,
    Invoke(RuntimeCallableBodyReference),
    AttachedDefault(RuntimeCallableBodyReference),
}

/// Sealed proof that one zero-argument callable layout can invoke now.
#[derive(Debug)]
pub(crate) struct RuntimeCallableZeroArgInvocationProof {
    owner: RuntimeProgramOwner,
    state: RuntimeCallableStateId,
    body: RuntimeCallableBodyReference,
    retained_ownership: Box<[crate::value::ownership::RuntimeValueOwnership]>,
}

impl RuntimeCallableZeroArgInvocationProof {
    pub(crate) const fn body(&self) -> RuntimeCallableBodyReference {
        self.body
    }
}

/// A logical group input before a VM has moved physical operands into packs.
/// Rest elements remain borrowed so an affine element is never cloned merely
/// to inspect the eventual pack.
#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeCallableMaterializedArgument<'a> {
    Fixed(&'a RuntimeValue),
    Rest(&'a [&'a RuntimeValue]),
    Bound(&'a RuntimeValue),
}

impl RuntimeCallableMaterializedArgument<'_> {
    fn permits_copy(self) -> bool {
        match self {
            Self::Fixed(value) | Self::Bound(value) => value.ownership().permits_copy(),
            Self::Rest(values) => values.iter().all(|value| value.ownership().permits_copy()),
        }
    }
}

impl PartialEq for RuntimeCallablePendingGroup {
    fn eq(&self, other: &Self) -> bool {
        self.owner.same_program(&other.owner)
            && self.state == other.state
            && self.retained == other.retained
            && self.arguments == other.arguments
            && self.attached == other.attached
    }
}

impl RuntimeCallablePendingGroup {
    pub(crate) fn inert_rollback_image(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> Result<RuntimeCallablePendingRollbackImage, String> {
        if !self.owner.same_program(owner) {
            return Err("pending callable belongs to a different rollback program".to_owned());
        }
        let image = |value: &RuntimeValue| {
            crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(value, owner)
                .map_err(|error| error.to_string())
        };
        Ok(RuntimeCallablePendingRollbackImage {
            state: self.state,
            retained: self
                .retained
                .iter()
                .map(|value| value.as_ref().map(image).transpose())
                .collect::<Result<_, String>>()?,
            arguments: self
                .arguments
                .iter()
                .map(|value| value.as_ref().map(image).transpose())
                .collect::<Result<_, String>>()?,
            attached: self.attached.as_ref().map(image).transpose()?,
        })
    }

    pub(crate) fn from_rollback_image(
        image: RuntimeCallablePendingRollbackImage,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, String> {
        let value = |saved: crate::value::AwbcRuntimeValueSnapshot| {
            saved
                .into_runtime_value_for_program(owner)
                .map_err(|error| error.to_string())
        };
        let parts = RuntimeCallablePendingGroupParts {
            owner: owner.clone(),
            state: image.state,
            retained: image
                .retained
                .into_iter()
                .map(|saved| saved.map(value).transpose())
                .collect::<Result<_, String>>()?,
            arguments: image
                .arguments
                .into_iter()
                .map(|saved| saved.map(value).transpose())
                .collect::<Result<_, String>>()?,
            attached: image.attached.map(value).transpose()?,
        };
        Self::try_from_parts(parts).map_err(|error| error.to_string())
    }

    fn new(
        callable: RuntimeCallableValue,
        arguments: Vec<RuntimeValue>,
        attached: Option<RuntimeValue>,
    ) -> Self {
        Self {
            owner: callable.owner,
            state: callable.state,
            retained: callable.retained.into_vec().into_iter().map(Some).collect(),
            arguments: arguments.into_iter().map(Some).collect(),
            attached,
        }
    }

    pub(crate) const fn state(&self) -> RuntimeCallableStateId {
        self.state
    }

    pub(crate) fn retained(&self) -> &[Option<RuntimeValue>] {
        &self.retained
    }

    pub(crate) fn arguments(&self) -> &[Option<RuntimeValue>] {
        &self.arguments
    }

    pub(crate) const fn attached(&self) -> Option<&RuntimeValue> {
        self.attached.as_ref()
    }

    pub(crate) fn into_parts(self) -> RuntimeCallablePendingGroupParts {
        RuntimeCallablePendingGroupParts {
            owner: self.owner,
            state: self.state,
            retained: self.retained,
            arguments: self.arguments,
            attached: self.attached,
        }
    }

    /// Reclaims the original callable and group inputs only if no projection
    /// has moved an indexed owner out of this packet. A partially projected
    /// failure remains an owned pending packet for the caller to preserve.
    pub(crate) fn try_reclaim_inputs(
        self,
    ) -> Result<
        (
            RuntimeCallableValue,
            Vec<RuntimeValue>,
            Option<RuntimeValue>,
        ),
        Self,
    > {
        if self.retained.iter().any(Option::is_none) || self.arguments.iter().any(Option::is_none) {
            return Err(self);
        }
        Ok((
            RuntimeCallableValue {
                owner: self.owner,
                state: self.state,
                retained: self.retained.into_iter().map(Option::unwrap).collect(),
            },
            self.arguments.into_iter().map(Option::unwrap).collect(),
            self.attached,
        ))
    }

    pub(crate) fn try_from_parts(
        parts: RuntimeCallablePendingGroupParts,
    ) -> Result<Self, RuntimeCallableOwnedInputError<Self>> {
        let pending = Self {
            owner: parts.owner,
            state: parts.state,
            retained: parts.retained,
            arguments: parts.arguments,
            attached: parts.attached,
        };
        let owner = pending.owner.clone();
        let state_id = pending.state;
        let validation = match owner {
            RuntimeProgramOwner::Plan(plan) => plan
                .callable_states()
                .get(state_id)
                .ok_or(RuntimeCallableValueError::MissingState { state: state_id })
                .and_then(|state| {
                    pending.validate_default_packet(state, |ty| {
                        plan.type_table()
                            .get(ty)
                            .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity)
                            .ok_or(RuntimeCallableValueError::MissingType { state: state_id })
                    })
                }),
            RuntimeProgramOwner::Awbc(program) => program
                .callable_states
                .get(state_id.index())
                .ok_or(RuntimeCallableValueError::MissingState { state: state_id })
                .and_then(|state| {
                    pending.validate_default_packet(state, |ty| {
                        program
                            .runtime_types
                            .get(ty.index())
                            .map(crate::awbc::schema::AwbcRuntimeType::semantic_identity)
                            .ok_or(RuntimeCallableValueError::MissingType { state: state_id })
                    })
                }),
        };
        match validation {
            Ok(()) => Ok(pending),
            Err(reason) => Err(RuntimeCallableOwnedInputError::new(reason, pending)),
        }
    }

    fn validate_default_packet<T: Copy, F>(
        &self,
        state: &RuntimeCallableStateDefinition<T, F>,
        semantic: impl Fn(T) -> Result<RuntimeSemanticTypeId, RuntimeCallableValueError>,
    ) -> Result<(), RuntimeCallableValueError> {
        let invalid = || RuntimeCallableValueError::InputProjection { state: self.state };
        let RuntimeCallableAttachedContract::Defaulted {
            default: crate::plan::RuntimeCallableDefault::Body { captures, .. },
            ..
        } = &state.attached
        else {
            return Err(invalid());
        };
        if self.attached.is_some()
            || self.retained.len() != state.retained.len()
            || self.arguments.len() != state.parameters.len()
        {
            return Err(invalid());
        }
        let future = transition_sources(&state.transition);
        for (position, (source, input)) in self.retained.iter().zip(&state.retained).enumerate() {
            let coordinate = RuntimeCallableInputSource::Retained {
                position: position as u32,
            };
            let should_be_present = !captures.contains(&coordinate) || future.contains(&coordinate);
            if source.is_some() != should_be_present {
                return Err(invalid());
            }
            if let Some(value) = source {
                self.owner
                    .types()
                    .validate_live_value(
                        semantic(input.ty)?,
                        value,
                        RuntimeSchemaLimits::engine_default(),
                    )
                    .map_err(|error| RuntimeCallableValueError::RetainedType {
                        state: self.state,
                        position,
                        error: Box::new(error),
                    })?;
            }
        }
        for (position, (source, input)) in self.arguments.iter().zip(&state.parameters).enumerate()
        {
            let coordinate = RuntimeCallableInputSource::Argument {
                position: position as u32,
            };
            let should_be_present = !captures.contains(&coordinate) || future.contains(&coordinate);
            if source.is_some() != should_be_present {
                return Err(invalid());
            }
            if let Some(value) = source {
                self.owner
                    .types()
                    .validate_live_value(
                        semantic(input.binding_ty)?,
                        value,
                        RuntimeSchemaLimits::engine_default(),
                    )
                    .map_err(|error| RuntimeCallableValueError::ArgumentType {
                        state: self.state,
                        position,
                        error: Box::new(error),
                    })?;
            }
        }
        Ok(())
    }

    pub(crate) fn complete_default(
        mut self,
        attached: RuntimeValue,
    ) -> Result<RuntimeCallableApplication, RuntimeCallableOwnedInputError<Self>> {
        if self.attached.is_some() {
            return Err(RuntimeCallableOwnedInputError::new(
                RuntimeCallableValueError::InputProjection { state: self.state },
                self,
            ));
        }
        self.attached = Some(attached);
        self.prepare(true)
    }

    fn value(&self, source: RuntimeCallableInputSource) -> Option<&RuntimeValue> {
        match source {
            RuntimeCallableInputSource::Retained { position } => self
                .retained
                .get(position as usize)
                .and_then(Option::as_ref),
            RuntimeCallableInputSource::Argument { position } => self
                .arguments
                .get(position as usize)
                .and_then(Option::as_ref),
            RuntimeCallableInputSource::Attached => self.attached.as_ref(),
        }
    }

    fn take(&mut self, source: RuntimeCallableInputSource) -> RuntimeValue {
        match source {
            RuntimeCallableInputSource::Retained { position } => self.retained[position as usize]
                .take()
                .expect("preflight checked retained callable input"),
            RuntimeCallableInputSource::Argument { position } => self.arguments[position as usize]
                .take()
                .expect("preflight checked callable argument"),
            RuntimeCallableInputSource::Attached => self
                .attached
                .take()
                .expect("preflight checked attached callable input"),
        }
    }

    /// Every extra use duplicates an unrestricted source before the final use
    /// moves its owner. Future uses include the terminal group after a default.
    fn project(
        &mut self,
        sources: &[RuntimeCallableInputSource],
        future: &[RuntimeCallableInputSource],
    ) -> Result<Vec<RuntimeValue>, RuntimeCallableValueError> {
        let mut remaining = BTreeMap::<RuntimeCallableInputSource, usize>::new();
        for source in sources.iter().chain(future) {
            *remaining.entry(*source).or_default() += 1;
        }
        for source in sources {
            let value = self
                .value(*source)
                .ok_or(RuntimeCallableValueError::InputProjection { state: self.state })?;
            if remaining[source] > 1 && !value.ownership().permits_copy() {
                return Err(RuntimeCallableValueError::AffineInputProjection {
                    state: self.state,
                    input: *source,
                });
            }
        }
        Ok(sources
            .iter()
            .map(|source| {
                let count = remaining
                    .get_mut(source)
                    .expect("projection source was preflighted");
                let value = if *count > 1 {
                    self.value(*source)
                        .expect("preflight checked callable duplication")
                        .clone()
                } else {
                    self.take(*source)
                };
                *count -= 1;
                value
            })
            .collect())
    }

    fn prepare(
        self,
        default_completed: bool,
    ) -> Result<RuntimeCallableApplication, RuntimeCallableOwnedInputError<Self>> {
        let owner = self.owner.clone();
        let state_id = self.state;
        let result = match owner {
            RuntimeProgramOwner::Plan(plan) => {
                let Some(state) = plan.callable_states().get(state_id) else {
                    return Err(RuntimeCallableOwnedInputError::new(
                        RuntimeCallableValueError::MissingState { state: state_id },
                        self,
                    ));
                };
                self.prepare_definition(
                    state,
                    default_completed,
                    |ty| {
                        plan.type_table()
                            .get(ty)
                            .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity)
                            .ok_or(RuntimeCallableValueError::MissingType { state: state_id })
                    },
                    RuntimeCallableBodyReference::Plan,
                )
            }
            RuntimeProgramOwner::Awbc(program) => {
                let Some(state) = program.callable_states.get(state_id.index()) else {
                    return Err(RuntimeCallableOwnedInputError::new(
                        RuntimeCallableValueError::MissingState { state: state_id },
                        self,
                    ));
                };
                self.prepare_definition(
                    state,
                    default_completed,
                    |ty| {
                        program
                            .runtime_types
                            .get(ty.index())
                            .map(crate::awbc::schema::AwbcRuntimeType::semantic_identity)
                            .ok_or(RuntimeCallableValueError::MissingType { state: state_id })
                    },
                    RuntimeCallableBodyReference::Awbc,
                )
            }
        };
        result
    }

    fn prepare_definition<T: Copy, F: Copy>(
        mut self,
        state: &RuntimeCallableStateDefinition<T, F>,
        default_completed: bool,
        semantic: impl Fn(T) -> Result<RuntimeSemanticTypeId, RuntimeCallableValueError>,
        body: impl Fn(F) -> RuntimeCallableBodyReference,
    ) -> Result<RuntimeCallableApplication, RuntimeCallableOwnedInputError<Self>> {
        let check = |ty: T, value: &RuntimeValue, position: usize| {
            self.owner
                .types()
                .validate_live_value(semantic(ty)?, value, RuntimeSchemaLimits::engine_default())
                .map_err(|error| RuntimeCallableValueError::ArgumentType {
                    state: self.state,
                    position,
                    error: Box::new(error),
                })
        };
        let validation = (|| {
            if matches!(
                state.transition,
                RuntimeCallableTransition::RequiresSpecialization
            ) {
                return Err(RuntimeCallableValueError::RequiresSpecialization {
                    state: self.state,
                });
            }
            if state.parameters.len() != self.arguments.len() {
                return Err(RuntimeCallableValueError::ArgumentCount {
                    state: self.state,
                    expected: state.parameters.len(),
                    actual: self.arguments.len(),
                });
            }
            for (position, (input, value)) in
                state.parameters.iter().zip(&self.arguments).enumerate()
            {
                match value {
                    Some(value) => check(input.binding_ty, value, position)?,
                    None if default_completed => {}
                    None => {
                        return Err(RuntimeCallableValueError::InputProjection {
                            state: self.state,
                        });
                    }
                }
            }
            if default_completed
                && !matches!(
                    state.attached,
                    RuntimeCallableAttachedContract::Defaulted { .. }
                )
            {
                return Err(RuntimeCallableValueError::InputProjection { state: self.state });
            }
            match (&state.attached, self.attached.as_ref()) {
                (RuntimeCallableAttachedContract::None, None)
                | (RuntimeCallableAttachedContract::Optional { .. }, None)
                | (RuntimeCallableAttachedContract::Defaulted { .. }, None) => {}
                (RuntimeCallableAttachedContract::Required { ty }, Some(value))
                | (RuntimeCallableAttachedContract::Defaulted { ty, .. }, Some(value)) => {
                    check(*ty, value, self.arguments.len())?
                }
                (RuntimeCallableAttachedContract::Optional { value: ty, .. }, Some(value)) => {
                    check(*ty, value, self.arguments.len())?
                }
                (RuntimeCallableAttachedContract::Required { .. }, None) => {
                    return Err(RuntimeCallableValueError::RequiredAttached { state: self.state });
                }
                (RuntimeCallableAttachedContract::None, Some(_)) => {
                    return Err(RuntimeCallableValueError::ArgumentCount {
                        state: self.state,
                        expected: self.arguments.len(),
                        actual: self.arguments.len() + 1,
                    });
                }
            }
            Ok(())
        })();
        if let Err(error) = validation {
            return Err(RuntimeCallableOwnedInputError::new(error, self));
        }
        if let RuntimeCallableAttachedContract::Optional { .. } = state.attached {
            self.attached = Some(
                self.attached
                    .take()
                    .map_or_else(RuntimeValue::option_none, RuntimeValue::option_some),
            );
        }
        if let RuntimeCallableAttachedContract::Defaulted { default, .. } = &state.attached
            && self.attached.is_none()
        {
            let crate::plan::RuntimeCallableDefault::Body { function, captures } = default else {
                return Err(RuntimeCallableOwnedInputError::new(
                    RuntimeCallableValueError::RequiresSpecialization { state: self.state },
                    self,
                ));
            };
            let future = transition_sources(&state.transition);
            let projected = match self.project(captures, &future) {
                Ok(values) => values,
                Err(error) => return Err(RuntimeCallableOwnedInputError::new(error, self)),
            };
            return Ok(RuntimeCallableApplication::AttachedDefault {
                invocation: RuntimeCallableInvocation {
                    body: body(*function),
                    captures: projected,
                    arguments: Vec::new(),
                },
                pending: self,
            });
        }
        match &state.transition {
            RuntimeCallableTransition::RequiresSpecialization => {
                unreachable!("preflight checked specialization")
            }
            RuntimeCallableTransition::Retain {
                state: target,
                values,
            } => {
                let projected = match self.project(values, &[]) {
                    Ok(values) => values,
                    Err(error) => return Err(RuntimeCallableOwnedInputError::new(error, self)),
                };
                Ok(RuntimeCallableApplication::Complete(
                    RuntimeValue::Callable(RuntimeCallableValue {
                        owner: self.owner,
                        state: *target,
                        retained: projected.into_boxed_slice(),
                    }),
                ))
            }
            RuntimeCallableTransition::Invoke {
                function,
                captures,
                arguments,
            } => {
                let sources = captures
                    .iter()
                    .chain(arguments)
                    .copied()
                    .collect::<Vec<_>>();
                let projected = match self.project(&sources, &[]) {
                    Ok(values) => values,
                    Err(error) => return Err(RuntimeCallableOwnedInputError::new(error, self)),
                };
                let mut projected = projected.into_iter();
                let captures = projected.by_ref().take(captures.len()).collect();
                let arguments = projected.collect();
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

fn transition_sources<F, S>(
    transition: &RuntimeCallableTransition<F, S>,
) -> Vec<RuntimeCallableInputSource> {
    match transition {
        RuntimeCallableTransition::RequiresSpecialization => Vec::new(),
        RuntimeCallableTransition::Retain { values, .. } => values.to_vec(),
        RuntimeCallableTransition::Invoke {
            captures,
            arguments,
            ..
        } => captures.iter().chain(arguments).copied().collect(),
    }
}

impl RuntimeCallableValue {
    pub(crate) fn inspect_zero_arg_invocation(
        &self,
    ) -> Result<RuntimeCallableZeroArgInvocationProof, RuntimeCallableValueError> {
        match self.inspect_group_refs(&[], None)? {
            RuntimeCallableGroupInspection::Invoke(body) => {
                Ok(RuntimeCallableZeroArgInvocationProof {
                    owner: self.owner.clone(),
                    state: self.state,
                    body,
                    retained_ownership: self.retained.iter().map(RuntimeValue::ownership).collect(),
                })
            }
            RuntimeCallableGroupInspection::Retain
            | RuntimeCallableGroupInspection::AttachedDefault(_) => {
                Err(RuntimeCallableValueError::RequiresControlTransfer { state: self.state })
            }
        }
    }

    /// Returns the exact borrowed native invocation projection sealed by a
    /// zero-argument proof. The caller may validate frame ABI and patterns
    /// before taking the retained callable from its dialogue owner.
    pub(crate) fn inspect_zero_arg_plan_inputs(
        &self,
    ) -> Result<
        (
            RuntimeCallableZeroArgInvocationProof,
            Vec<&RuntimeValue>,
            Vec<&RuntimeValue>,
        ),
        RuntimeCallableValueError,
    > {
        let proof = self.inspect_zero_arg_invocation()?;
        if !matches!(proof.body(), RuntimeCallableBodyReference::Plan(_)) {
            return Err(RuntimeCallableValueError::ForeignProgram);
        }
        let RuntimeProgramOwner::Plan(plan) = &self.owner else {
            return Err(RuntimeCallableValueError::ForeignProgram);
        };
        let state = plan
            .callable_states()
            .get(self.state)
            .ok_or(RuntimeCallableValueError::MissingState { state: self.state })?;
        let RuntimeCallableTransition::Invoke {
            captures,
            arguments,
            ..
        } = &state.transition
        else {
            unreachable!("zero-argument proof selected an invocation")
        };
        let project = |source: &RuntimeCallableInputSource| match source {
            RuntimeCallableInputSource::Retained { position } => self
                .retained
                .get(*position as usize)
                .ok_or(RuntimeCallableValueError::InputProjection { state: self.state }),
            RuntimeCallableInputSource::Argument { .. } | RuntimeCallableInputSource::Attached => {
                Err(RuntimeCallableValueError::InputProjection { state: self.state })
            }
        };
        let captures = captures
            .iter()
            .map(&project)
            .collect::<Result<Vec<_>, _>>()?;
        let arguments = arguments
            .iter()
            .map(&project)
            .collect::<Result<Vec<_>, _>>()?;
        Ok((proof, captures, arguments))
    }

    /// Consumes a layout that passed borrowed inspection. A mismatched proof
    /// is an internal programming error detected before any owner projection.
    pub(crate) fn commit_zero_arg_invocation(
        self,
        proof: RuntimeCallableZeroArgInvocationProof,
    ) -> RuntimeCallableInvocation {
        assert!(
            self.owner.same_program(&proof.owner)
                && self.state == proof.state
                && self
                    .retained
                    .iter()
                    .map(RuntimeValue::ownership)
                    .eq(proof.retained_ownership.iter().copied()),
            "zero-argument invocation proof belongs to another callable layout"
        );
        match RuntimeCallablePendingGroup::new(self, Vec::new(), None)
            .prepare(false)
            .expect("borrowed zero-argument proof sealed the complete transition")
        {
            RuntimeCallableApplication::Invoke(invocation) => invocation,
            RuntimeCallableApplication::Complete(_)
            | RuntimeCallableApplication::AttachedDefault { .. } => {
                unreachable!("borrowed proof selected an Invoke transition")
            }
        }
    }

    pub(super) fn inspect_arrow_projection_ownership(
        &self,
        values: &[&RuntimeValue],
    ) -> Result<(), RuntimeCallableValueError> {
        let absent = || RuntimeCallableValueError::MissingState { state: self.state };
        match &self.owner {
            RuntimeProgramOwner::Plan(plan) => self.inspect_arrow_state(
                plan.callable_states().get(self.state).ok_or_else(absent)?,
                values,
            ),
            RuntimeProgramOwner::Awbc(program) => self.inspect_arrow_state(
                program
                    .callable_states
                    .get(self.state.index())
                    .ok_or_else(absent)?,
                values,
            ),
        }
    }

    fn inspect_arrow_state<T, F, S>(
        &self,
        state: &RuntimeCallableStateDefinition<T, F, S>,
        values: &[&RuntimeValue],
    ) -> Result<(), RuntimeCallableValueError> {
        if values.len() < state.parameters.len() {
            let partial = state
                .partials
                .iter()
                .find(|row| {
                    row.parameters.iter().copied().eq(state
                        .parameters
                        .iter()
                        .take(values.len())
                        .map(|input| input.coordinate))
                        && row.parameters.len() == values.len()
                })
                .ok_or(RuntimeCallableValueError::MissingPartial {
                    state: self.state,
                    arguments: values.len(),
                })?;
            return self.check_arrow_sources(values, &partial.values, &[]);
        }
        let current = match &state.attached {
            RuntimeCallableAttachedContract::Defaulted {
                default: crate::plan::RuntimeCallableDefault::Body { captures, .. },
                ..
            } => captures.to_vec(),
            _ => transition_sources(&state.transition),
        };
        let future = if matches!(
            state.attached,
            RuntimeCallableAttachedContract::Defaulted { .. }
        ) {
            transition_sources(&state.transition)
        } else {
            Vec::new()
        };
        self.check_arrow_sources(values, &current, &future)
    }

    fn check_arrow_sources(
        &self,
        values: &[&RuntimeValue],
        current: &[RuntimeCallableInputSource],
        future: &[RuntimeCallableInputSource],
    ) -> Result<(), RuntimeCallableValueError> {
        let source_value = |source| match source {
            RuntimeCallableInputSource::Retained { position } => {
                self.retained.get(position as usize)
            }
            RuntimeCallableInputSource::Argument { position } => {
                values.get(position as usize).copied()
            }
            RuntimeCallableInputSource::Attached => None,
        };
        let mut counts = BTreeMap::<RuntimeCallableInputSource, usize>::new();
        for source in current.iter().chain(future.iter()) {
            *counts.entry(*source).or_default() += 1;
        }
        for source in current {
            let value = source_value(*source)
                .ok_or(RuntimeCallableValueError::InputProjection { state: self.state })?;
            if counts[source] > 1 && !value.ownership().permits_copy() {
                return Err(RuntimeCallableValueError::AffineInputProjection {
                    state: self.state,
                    input: *source,
                });
            }
        }
        Ok(())
    }

    /// Validates one group without materializing or duplicating any owner.
    /// Snapshot and continuation checks use this view of the same state row.
    pub(crate) fn inspect_group(
        &self,
        arguments: &[RuntimeValue],
        attached: Option<&RuntimeValue>,
    ) -> Result<RuntimeCallableGroupInspection, RuntimeCallableValueError> {
        let references = arguments.iter().collect::<Vec<_>>();
        self.inspect_group_refs(&references, attached)
    }

    /// Borrowed logical argument view used before a register-file transaction
    /// commits physical values into owned rest packs.
    pub(crate) fn inspect_group_refs(
        &self,
        arguments: &[&RuntimeValue],
        attached: Option<&RuntimeValue>,
    ) -> Result<RuntimeCallableGroupInspection, RuntimeCallableValueError> {
        let arguments = arguments
            .iter()
            .copied()
            .map(RuntimeCallableMaterializedArgument::Bound)
            .collect::<Vec<_>>();
        self.inspect_group_materialization(&arguments, attached)
    }

    /// Validates fixed values and rest elements before an owning VM commits
    /// register operands into the logical argument pack.
    pub(crate) fn inspect_group_materialization(
        &self,
        arguments: &[RuntimeCallableMaterializedArgument<'_>],
        attached: Option<&RuntimeValue>,
    ) -> Result<RuntimeCallableGroupInspection, RuntimeCallableValueError> {
        self.validate_retained()?;
        let absent = || RuntimeCallableValueError::MissingState { state: self.state };
        match &self.owner {
            RuntimeProgramOwner::Plan(plan) => {
                let state = plan.callable_states().get(self.state).ok_or_else(absent)?;
                self.inspect_definition(
                    state,
                    arguments,
                    attached,
                    |ty| {
                        plan.type_table()
                            .get(ty)
                            .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity)
                            .ok_or(RuntimeCallableValueError::MissingType { state: self.state })
                    },
                    RuntimeCallableBodyReference::Plan,
                )
            }
            RuntimeProgramOwner::Awbc(program) => {
                let state = program
                    .callable_states
                    .get(self.state.index())
                    .ok_or_else(absent)?;
                self.inspect_definition(
                    state,
                    arguments,
                    attached,
                    |ty| {
                        program
                            .runtime_types
                            .get(ty.index())
                            .map(crate::awbc::schema::AwbcRuntimeType::semantic_identity)
                            .ok_or(RuntimeCallableValueError::MissingType { state: self.state })
                    },
                    RuntimeCallableBodyReference::Awbc,
                )
            }
        }
    }

    fn inspect_definition<T: Copy, F: Copy>(
        &self,
        state: &RuntimeCallableStateDefinition<T, F>,
        arguments: &[RuntimeCallableMaterializedArgument<'_>],
        attached: Option<&RuntimeValue>,
        semantic: impl Fn(T) -> Result<RuntimeSemanticTypeId, RuntimeCallableValueError>,
        body: impl Fn(F) -> RuntimeCallableBodyReference,
    ) -> Result<RuntimeCallableGroupInspection, RuntimeCallableValueError> {
        let invalid = || RuntimeCallableValueError::InputProjection { state: self.state };
        if state.parameters.len() != arguments.len() {
            return Err(RuntimeCallableValueError::ArgumentCount {
                state: self.state,
                expected: state.parameters.len(),
                actual: arguments.len(),
            });
        }
        for (position, (input, value)) in state.parameters.iter().zip(arguments).enumerate() {
            let check = |ty, value: &RuntimeValue| {
                self.owner
                    .types()
                    .validate_live_value(
                        semantic(ty)?,
                        value,
                        RuntimeSchemaLimits::engine_default(),
                    )
                    .map_err(|error| RuntimeCallableValueError::ArgumentType {
                        state: self.state,
                        position,
                        error: Box::new(error),
                    })
            };
            match (input.kind, value) {
                (
                    RuntimeCallableParameterKind::Fixed,
                    RuntimeCallableMaterializedArgument::Fixed(value),
                ) => check(input.binding_ty, value)?,
                (
                    RuntimeCallableParameterKind::Rest,
                    RuntimeCallableMaterializedArgument::Rest(values),
                ) => {
                    // The admitted callable grammar seals binding_ty as Vec<abi_ty>.
                    // Validate the pack shape without materializing its owners.
                    check(
                        input.binding_ty,
                        &crate::value::runtime_sequence_values(Vec::new()),
                    )?;
                    for value in *values {
                        check(input.abi_ty, value)?;
                    }
                }
                (_, RuntimeCallableMaterializedArgument::Bound(value)) => {
                    check(input.binding_ty, value)?
                }
                _ => return Err(invalid()),
            }
        }
        match (&state.attached, attached) {
            (RuntimeCallableAttachedContract::None, None)
            | (RuntimeCallableAttachedContract::Optional { .. }, None)
            | (RuntimeCallableAttachedContract::Defaulted { .. }, None) => {}
            (RuntimeCallableAttachedContract::Required { ty }, Some(value))
            | (RuntimeCallableAttachedContract::Defaulted { ty, .. }, Some(value))
            | (RuntimeCallableAttachedContract::Optional { value: ty, .. }, Some(value)) => {
                self.owner
                    .types()
                    .validate_live_value(
                        semantic(*ty)?,
                        value,
                        RuntimeSchemaLimits::engine_default(),
                    )
                    .map_err(|error| RuntimeCallableValueError::ArgumentType {
                        state: self.state,
                        position: arguments.len(),
                        error: Box::new(error),
                    })?;
            }
            (RuntimeCallableAttachedContract::Required { .. }, None) => {
                return Err(RuntimeCallableValueError::RequiredAttached { state: self.state });
            }
            (RuntimeCallableAttachedContract::None, Some(_)) => return Err(invalid()),
        }
        let source_permits_copy = |source| match source {
            RuntimeCallableInputSource::Retained { position } => self
                .retained
                .get(position as usize)
                .map(|value| value.ownership().permits_copy()),
            RuntimeCallableInputSource::Argument { position } => arguments
                .get(position as usize)
                .map(|value| value.permits_copy()),
            RuntimeCallableInputSource::Attached => {
                attached.map(|value| value.ownership().permits_copy())
            }
        };
        let validate_projection =
            |sources: &[RuntimeCallableInputSource], future: &[RuntimeCallableInputSource]| {
                let mut counts = BTreeMap::<RuntimeCallableInputSource, usize>::new();
                for source in sources.iter().chain(future) {
                    *counts.entry(*source).or_default() += 1;
                }
                for source in sources {
                    let permits_copy = source_permits_copy(*source).ok_or_else(invalid)?;
                    if counts[source] > 1 && !permits_copy {
                        return Err(RuntimeCallableValueError::AffineInputProjection {
                            state: self.state,
                            input: *source,
                        });
                    }
                }
                Ok(())
            };
        if let RuntimeCallableAttachedContract::Defaulted {
            default: crate::plan::RuntimeCallableDefault::Body { function, captures },
            ..
        } = &state.attached
            && attached.is_none()
        {
            validate_projection(captures, &transition_sources(&state.transition))?;
            return Ok(RuntimeCallableGroupInspection::AttachedDefault(body(
                *function,
            )));
        }
        match &state.transition {
            RuntimeCallableTransition::RequiresSpecialization => {
                Err(RuntimeCallableValueError::RequiresSpecialization { state: self.state })
            }
            RuntimeCallableTransition::Retain { values, .. } => {
                validate_projection(values, &[])?;
                Ok(RuntimeCallableGroupInspection::Retain)
            }
            RuntimeCallableTransition::Invoke {
                function,
                captures,
                arguments: sources,
            } => {
                let combined = captures.iter().chain(sources).copied().collect::<Vec<_>>();
                validate_projection(&combined, &[])?;
                Ok(RuntimeCallableGroupInspection::Invoke(body(*function)))
            }
        }
    }

    /// All source values become owned by one result, invocation, or pending
    /// default packet. On rejection the error retains the untouched packet.
    pub(crate) fn prepare_group(
        self,
        arguments: Vec<RuntimeValue>,
        attached: Option<RuntimeValue>,
    ) -> Result<
        RuntimeCallableApplication,
        RuntimeCallableOwnedInputError<RuntimeCallablePendingGroup>,
    > {
        RuntimeCallablePendingGroup::new(self, arguments, attached).prepare(false)
    }

    pub(crate) fn try_bind_prefix(
        self,
        values: Vec<RuntimeValue>,
    ) -> Result<Self, RuntimeCallableOwnedInputError<RuntimeCallablePendingGroup>> {
        let mut pending = RuntimeCallablePendingGroup::new(self, values, None);
        if pending.arguments.is_empty() {
            return Ok(RuntimeCallableValue {
                owner: pending.owner,
                state: pending.state,
                retained: pending.retained.into_iter().map(Option::unwrap).collect(),
            });
        }
        let selection = match &pending.owner {
            RuntimeProgramOwner::Plan(plan) => {
                plan.callable_states().get(pending.state).map(|state| {
                    state
                        .partials
                        .iter()
                        .find(|row| {
                            row.parameters.iter().copied().eq(state
                                .parameters
                                .iter()
                                .take(pending.arguments.len())
                                .map(|input| input.coordinate))
                                && row.parameters.len() == pending.arguments.len()
                        })
                        .map(|row| (row.state, row.values.to_vec()))
                })
            }
            RuntimeProgramOwner::Awbc(program) => program
                .callable_states
                .get(pending.state.index())
                .map(|state| {
                    state
                        .partials
                        .iter()
                        .find(|row| {
                            row.parameters.iter().copied().eq(state
                                .parameters
                                .iter()
                                .take(pending.arguments.len())
                                .map(|input| input.coordinate))
                                && row.parameters.len() == pending.arguments.len()
                        })
                        .map(|row| (row.state, row.values.to_vec()))
                }),
        };
        let Some(Some((target, sources))) = selection else {
            let reason = if selection.is_none() {
                RuntimeCallableValueError::MissingState {
                    state: pending.state,
                }
            } else {
                RuntimeCallableValueError::MissingPartial {
                    state: pending.state,
                    arguments: pending.arguments.len(),
                }
            };
            return Err(RuntimeCallableOwnedInputError::new(reason, pending));
        };
        let inputs =
            match RuntimeCallableValue::ordinary_abi_inputs_for(&pending.owner, pending.state) {
                Ok(inputs) => inputs,
                Err(error) => return Err(RuntimeCallableOwnedInputError::new(error, pending)),
            };
        if pending.arguments.iter().any(Option::is_none) {
            return Err(RuntimeCallableOwnedInputError::new(
                RuntimeCallableValueError::InputProjection {
                    state: pending.state,
                },
                pending,
            ));
        }
        if pending.arguments.len() > inputs.len() {
            return Err(RuntimeCallableOwnedInputError::new(
                RuntimeCallableValueError::ArgumentCount {
                    state: pending.state,
                    expected: inputs.len(),
                    actual: pending.arguments.len(),
                },
                pending,
            ));
        }
        for (position, ((_, ty), value)) in inputs.iter().zip(&pending.arguments).enumerate() {
            let result = pending.owner.types().validate_live_value(
                *ty,
                value.as_ref().expect("prefix value was checked present"),
                RuntimeSchemaLimits::engine_default(),
            );
            if let Err(error) = result {
                return Err(RuntimeCallableOwnedInputError::new(
                    RuntimeCallableValueError::ArgumentType {
                        state: pending.state,
                        position,
                        error: Box::new(error),
                    },
                    pending,
                ));
            }
        }
        for ((kind, _), value) in inputs.iter().zip(&mut pending.arguments) {
            if matches!(kind, RuntimeCallableParameterKind::Rest) {
                let original = value.take().expect("preflight checked prefix argument");
                *value = Some(crate::value::runtime_sequence_values(vec![original]));
            }
        }
        let projected = match pending.project(&sources, &[]) {
            Ok(values) => values,
            Err(error) => return Err(RuntimeCallableOwnedInputError::new(error, pending)),
        };
        Ok(RuntimeCallableValue {
            owner: pending.owner,
            state: target,
            retained: projected.into_boxed_slice(),
        })
    }
}
