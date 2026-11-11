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
    current: CallableParameterCoordinate,
    outcome: CallableGroupOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallableGroupOutcome {
    Invoke,
    DeclaredContinuation(CallableParameterCoordinate),
    ValuePartial(CallableParameterIndex),
}

impl CallableGroupTransition {
    pub(crate) const fn current(self) -> CallableGroupIndex {
        self.current.group()
    }

    pub(crate) const fn position(self) -> CallableParameterCoordinate {
        self.current
    }

    pub(crate) const fn continuation_position(self) -> Option<CallableParameterCoordinate> {
        match self.outcome {
            CallableGroupOutcome::DeclaredContinuation(position) => Some(position),
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

    pub(crate) const fn remaining_position(self) -> Option<CallableParameterCoordinate> {
        match self.outcome {
            CallableGroupOutcome::Invoke => None,
            CallableGroupOutcome::DeclaredContinuation(position) => Some(position),
            CallableGroupOutcome::ValuePartial(first) => Some(CallableParameterCoordinate::new(
                self.current.group(),
                first,
            )),
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
        (coordinate >= self.remaining_position()? && Some(coordinate.group()) != implicit_receiver)
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
        current: CallableParameterCoordinate,
        implicit_receiver: Option<CallableGroupIndex>,
    ) -> Option<CallableGroupTransition> {
        let group = self.group(current.group())?;
        if current.parameter().get() > 0 && group.parameter(current.parameter()).is_none() {
            return None;
        }
        let next = current
            .group()
            .get()
            .checked_add(1)
            .and_then(|index| CallableGroupIndex::try_from_usize(index).ok())
            .filter(|next| Some(*next) != implicit_receiver && self.group(*next).is_some());
        Some(CallableGroupTransition {
            current,
            outcome: next.map_or(CallableGroupOutcome::Invoke, |group| {
                CallableGroupOutcome::DeclaredContinuation(
                    CallableParameterCoordinate::group_start(group),
                )
            }),
        })
    }

    pub(crate) fn partial_value_group(
        &self,
        candidate: &crate::callable::CallableCandidateId,
        current: CallableParameterCoordinate,
        first_unbound: CallableParameterIndex,
    ) -> Option<CallableGroupTransition> {
        let parameters = self.group(current.group())?.parameters();
        use crate::callable::CallableCandidateId;
        let declared_function = matches!(candidate, CallableCandidateId::Project(declaration)
            if declaration.owner() == arcweft_lang_hir::symbol::CallableDeclarationOwner::Function);
        (matches!(
            candidate,
            CallableCandidateId::Local(_) | CallableCandidateId::FunctionValue(_)
        ) || declared_function)
            .then_some(())?;
        let inventory = self.generic_inventory();
        let candidate_parameter = |role| role == super::CallableSchemaGenericRole::Candidate;
        let declaration_has_generics = declared_function
            && (inventory
                .types()
                .iter()
                .any(|row| candidate_parameter(row.role()))
                || inventory
                    .consts()
                    .iter()
                    .any(|row| candidate_parameter(row.role()))
                || inventory
                    .effects()
                    .iter()
                    .any(|row| candidate_parameter(row.role())));
        (self.groups().len() == 1
            && self.attached_content_for_group(current.group()).is_none()
            && inventory.template_binder().is_empty()
            && !declaration_has_generics
            && first_unbound > current.parameter()
            && first_unbound.get() < parameters.len()
            && parameters.iter().all(|parameter| {
                (parameter.passing() == CallableParameterPassing::PositionalOnly
                    || (declared_function
                        && parameter.passing() == CallableParameterPassing::PositionalOrNamed))
                    && parameter.presence() == CallableParameterPresence::Required
                    && parameter.consumer() == &super::CallableParameterConsumer::Value
            }))
        .then_some(CallableGroupTransition {
            current,
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
        self.declared_function_type_from_position(
            CallableParameterCoordinate::group_start(first_group),
            terminal_effects,
        )
    }

    pub(crate) fn declared_function_type_from_position(
        &self,
        position: CallableParameterCoordinate,
        terminal_effects: &EffectRow,
    ) -> Result<TypeKind, crate::callable::CallConstraintInvariant> {
        self.project_function_type_from_position(
            position,
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
            CallableParameterCoordinate::group_start(first_group),
            terminal_effects,
            result,
            parameter,
        )
    }

    fn project_function_type_from_position<E>(
        &self,
        position: CallableParameterCoordinate,
        terminal_effects: &EffectRow,
        result: impl FnOnce() -> Result<TypeKind, E>,
        mut parameter: impl FnMut(
            CallableParameterCoordinate,
            &CallableParameter,
        ) -> Result<TypeKind, E>,
    ) -> Result<TypeKind, CallableFunctionTypeProjectionError<E>> {
        let first_group = position.group();
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
                    position.parameter().get()
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
