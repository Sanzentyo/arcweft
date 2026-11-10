//! Callable-schema and function-type projection share one group topology.
//!
//! The schema owns group order. Projection contexts supply parameter/result
//! types and the terminal invocation row; parameter-contained function rows
//! remain owned by their original types.

use super::{
    CallableArgumentPolicy, CallableEffectSchema, CallableGenericFirstUse,
    CallableGenericParameterIssuer, CallableGroupKind, CallableParameter,
    CallableParameterAdmission, CallableParameterGroup, CallableParameterPassing,
    CallableParameterPresence, CallableSignatureSchema, CallableValidator, SpreadArgumentPolicy,
    UnknownNamedArgumentPolicy,
};
use crate::{
    callable::{
        CallableFamily, CallableFamilyInvariantCode, CallableGroupIndex, CallableLimits,
        CallableParameterCoordinate, CallableParameterIndex, CallableSchemaError,
    },
    effect_row::EffectRow,
    effects::EffectSet,
    types::TypeKind,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum CallableFunctionTypeProjectionError<E> {
    #[error("function type starts at missing callable group {group:?}")]
    MissingGroup { group: CallableGroupIndex },
    #[error("callable type projection failed: {0}")]
    Projection(E),
}

/// The admitted boundary of one application. An abstract function value can
/// retain a positional prefix within its arrow, without a declared next group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CallableGroupTransition {
    current: CallableGroupIndex,
    outcome: CallableGroupOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallableGroupOutcome {
    Invoke,
    NextGroup(CallableGroupIndex),
    ValuePartial(CallableParameterIndex),
}

impl CallableGroupTransition {
    pub(crate) const fn current(self) -> CallableGroupIndex {
        self.current
    }

    pub(crate) const fn next(self) -> Option<CallableGroupIndex> {
        match self.outcome {
            CallableGroupOutcome::NextGroup(group) => Some(group),
            _ => None,
        }
    }

    pub(crate) const fn first_unbound(self) -> Option<CallableParameterIndex> {
        match self.outcome {
            CallableGroupOutcome::ValuePartial(parameter) => Some(parameter),
            _ => None,
        }
    }

    pub(crate) const fn retains_inputs(self) -> bool {
        !matches!(self.outcome, CallableGroupOutcome::Invoke)
    }

    pub(crate) const fn remaining_group(self) -> Option<CallableGroupIndex> {
        match self.outcome {
            CallableGroupOutcome::Invoke => None,
            CallableGroupOutcome::NextGroup(group) => Some(group),
            CallableGroupOutcome::ValuePartial(_) => Some(self.current),
        }
    }

    pub(crate) fn remaining_parameter_use(
        self,
        first_use: CallableGenericFirstUse,
        implicit_receiver: Option<CallableGroupIndex>,
    ) -> Option<CallableParameterCoordinate> {
        let CallableGenericFirstUse::Parameter(coordinate) = first_use else {
            return None;
        };
        let first_remaining_group = self.remaining_group()?;
        (coordinate.group() >= first_remaining_group
            && (coordinate.group() > first_remaining_group
                || self
                    .first_unbound()
                    .is_none_or(|first| coordinate.parameter() >= first))
            && Some(coordinate.group()) != implicit_receiver)
            .then_some(coordinate)
    }
}

impl CallableParameterPassing {
    /// Binding storage for an ordinary runtime parameter. Named rest values
    /// have no ordinary project-function ABI and remain a typed absence.
    pub(crate) fn value_binding_type(self, abi_type: TypeKind) -> Option<TypeKind> {
        match self {
            Self::RestPositional => Some(TypeKind::Vec(Box::new(abi_type))),
            Self::RestNamed => None,
            Self::PositionalOnly | Self::PositionalOrNamed | Self::NamedOnly => Some(abi_type),
        }
    }
}

impl CallableSignatureSchema {
    pub(crate) fn complete_group(
        &self,
        current: CallableGroupIndex,
        implicit_receiver: Option<CallableGroupIndex>,
    ) -> Option<CallableGroupTransition> {
        self.group(current)?;
        let next = current
            .get()
            .checked_add(1)
            .and_then(|index| CallableGroupIndex::try_from_usize(index).ok())
            .filter(|next| Some(*next) != implicit_receiver && self.group(*next).is_some());
        Some(CallableGroupTransition {
            current,
            outcome: next.map_or(
                CallableGroupOutcome::Invoke,
                CallableGroupOutcome::NextGroup,
            ),
        })
    }

    pub(crate) fn partial_value_group(
        &self,
        candidate: &crate::callable::CallableCandidateId,
        group: CallableGroupIndex,
        first_unbound: CallableParameterIndex,
    ) -> Option<CallableGroupTransition> {
        let parameters = self.group(group)?.parameters();
        (matches!(
            candidate,
            crate::callable::CallableCandidateId::Local(_)
                | crate::callable::CallableCandidateId::FunctionValue(_)
        ) && self.groups().len() == 1
            && self.attached_content_for_group(group).is_none()
            && self.generic_inventory().template_binder().is_empty()
            && first_unbound.get() > 0
            && first_unbound.get() < parameters.len()
            && parameters.iter().all(|parameter| {
                parameter.passing() == CallableParameterPassing::PositionalOnly
                    && parameter.presence() == CallableParameterPresence::Required
            }))
        .then_some(CallableGroupTransition {
            current: group,
            outcome: CallableGroupOutcome::ValuePartial(first_unbound),
        })
    }

    /// A declaration's value owns its used generic slots as lexical binders.
    /// Rigid references from an enclosing declaration remain free.
    pub(crate) fn quantify_function_value(
        &self,
        value: &TypeKind,
    ) -> Result<TypeKind, crate::types::TypeInstantiationError> {
        self.function_value_binder()?
            .quantify_function_with_control(value, &mut crate::types::UnmeteredTypeProjection)
            .map_err(crate::types::TypeProjectionError::into_instantiation)
    }

    pub(crate) fn function_value_binder(
        &self,
    ) -> Result<crate::types::GenericDeclarationBinder, crate::types::GenericScopeError> {
        let inventory = self.generic_inventory();
        let types = inventory
            .types()
            .iter()
            .filter(|entry| entry.role() == super::CallableSchemaGenericRole::Candidate)
            .map(|entry| entry.parameter().clone())
            .collect::<Box<[_]>>();
        let consts = inventory
            .consts()
            .iter()
            .filter(|entry| entry.role() == super::CallableSchemaGenericRole::Candidate)
            .map(|entry| entry.parameter().clone())
            .collect::<Box<[_]>>();
        let effects = inventory
            .effects()
            .iter()
            .filter(|entry| entry.role() == super::CallableSchemaGenericRole::Candidate)
            .map(|entry| entry.parameter().clone())
            .collect::<Box<[_]>>();
        crate::types::GenericDeclarationBinder::new(
            inventory.template_scope().clone(),
            types,
            consts,
            effects,
        )
    }

    pub(crate) fn parameter_type(
        &self,
        coordinate: CallableParameterCoordinate,
    ) -> Option<&TypeKind> {
        self.group(coordinate.group())
            .and_then(|group| group.parameter(coordinate.parameter()))
            .and_then(CallableParameter::declared_type)
    }

    pub(crate) fn declared_function_type_from_group(
        &self,
        first_group: CallableGroupIndex,
        terminal_effects: &EffectRow,
    ) -> Result<TypeKind, crate::callable::CallConstraintInvariant> {
        self.declared_function_type_from_position(first_group, None, terminal_effects)
    }

    pub(crate) fn declared_function_type_from_position(
        &self,
        first_group: CallableGroupIndex,
        first_unbound: Option<CallableParameterIndex>,
        terminal_effects: &EffectRow,
    ) -> Result<TypeKind, crate::callable::CallConstraintInvariant> {
        self.project_function_type_from_position(
            first_group,
            first_unbound,
            terminal_effects,
            || {
                self.value_type()
                    .cloned()
                    .ok_or(crate::callable::CallConstraintInvariant::MalformedSchemaInventory)
            },
            |_, parameter| {
                parameter
                    .declared_type()
                    .cloned()
                    .ok_or(crate::callable::CallConstraintInvariant::MalformedSchemaInventory)
            },
        )
        .map_err(Into::into)
    }

    /// Projects a complete remaining group chain. Earlier groups only retain
    /// arguments; the terminal group carries the callable's invocation row.
    /// Projection failures retain the caller context's typed cause.
    pub(crate) fn project_function_type_from_group<E>(
        &self,
        first_group: CallableGroupIndex,
        terminal_effects: &EffectRow,
        result: impl FnOnce() -> Result<TypeKind, E>,
        parameter: impl FnMut(CallableParameterCoordinate, &CallableParameter) -> Result<TypeKind, E>,
    ) -> Result<TypeKind, CallableFunctionTypeProjectionError<E>> {
        self.project_function_type_from_position(
            first_group,
            None,
            terminal_effects,
            result,
            parameter,
        )
    }

    fn project_function_type_from_position<E>(
        &self,
        first_group: CallableGroupIndex,
        first_unbound: Option<CallableParameterIndex>,
        terminal_effects: &EffectRow,
        result: impl FnOnce() -> Result<TypeKind, E>,
        mut parameter: impl FnMut(
            CallableParameterCoordinate,
            &CallableParameter,
        ) -> Result<TypeKind, E>,
    ) -> Result<TypeKind, CallableFunctionTypeProjectionError<E>> {
        let groups = self
            .groups()
            .get(first_group.get()..)
            .filter(|groups| !groups.is_empty())
            .ok_or(CallableFunctionTypeProjectionError::MissingGroup { group: first_group })?;
        let mut result = result().map_err(CallableFunctionTypeProjectionError::Projection)?;
        for (distance_from_terminal, group) in groups.iter().rev().enumerate() {
            let parameters = group
                .parameters()
                .iter()
                .skip(if group.index() == first_group {
                    first_unbound.map_or(0, CallableParameterIndex::get)
                } else {
                    0
                })
                .map(|value| {
                    parameter(
                        CallableParameterCoordinate::new(group.index(), value.index()),
                        value,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(CallableFunctionTypeProjectionError::Projection)?;
            let effects = if distance_from_terminal == 0 {
                terminal_effects.clone()
            } else {
                EffectRow::closed(EffectSet::new())
            };
            result = TypeKind::function_with_contract(
                crate::types::GenericBinder::EMPTY,
                if group.index() == first_group {
                    self.effect_predicate().clone()
                } else {
                    crate::effect_row::EffectPredicate::unconstrained()
                },
                parameters,
                result,
                effects,
            );
        }
        Ok(result)
    }

    /// Builds the strict positional schema for an evaluated function value.
    pub(crate) fn for_function_value(
        ty: &TypeKind,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError> {
        let TypeKind::Function {
            binder,
            predicate,
            params,
            return_type,
            effects,
        } = ty
        else {
            return Err(CallableSchemaError::FamilyInvariant {
                family: CallableFamily::FunctionValue,
                code: CallableFamilyInvariantCode::InvalidParameterType,
            });
        };
        let parameters = params
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                CallableParameter::try_new(
                    CallableParameterIndex::try_from_usize(index).map_err(|_| {
                        CallableSchemaError::ParameterLimit {
                            actual: params.len(),
                            limit: limits.max_parameters_per_callable(),
                        }
                    })?,
                    Some(
                        arcweft_lang_syntax::name::CallArgumentName::try_new(&format!(
                            "arg{}",
                            index + 1
                        ))
                        .map_err(|_| {
                            CallableSchemaError::FamilyInvariant {
                                family: CallableFamily::FunctionValue,
                                code: CallableFamilyInvariantCode::InvalidParameterType,
                            }
                        })?,
                    ),
                    CallableParameterAdmission::checked(parameter.clone()),
                    CallableParameterPassing::PositionalOnly,
                    CallableParameterPresence::Required,
                    None,
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let group = CallableParameterGroup::try_new(
            CallableGroupIndex::ZERO,
            CallableGroupKind::Initial,
            parameters,
            limits,
        )?;
        Self::try_new(
            vec![group],
            return_type.as_ref().clone(),
            CallableEffectSchema::fixed(effects.clone()),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            CallableGenericParameterIssuer::function_scheme(*binder, predicate.clone()),
            limits,
        )
    }
}
