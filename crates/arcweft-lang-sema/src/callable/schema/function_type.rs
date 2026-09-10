//! Callable-schema and function-type projection share one group topology.
//!
//! The schema owns group order. Projection contexts supply parameter/result
//! types and the terminal invocation row; parameter-contained function rows
//! remain owned by their original types.

use super::{
    CallableArgumentPolicy, CallableEffectSchema, CallableGenericParameterIssuer,
    CallableGroupKind, CallableParameter, CallableParameterAdmission, CallableParameterGroup,
    CallableParameterPassing, CallableParameterPresence, CallableSignatureSchema,
    CallableValidator, SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
};
use crate::{
    callable::{
        CallableFamily, CallableFamilyInvariantCode, CallableGroupIndex, CallableLimits,
        CallableName, CallableParameterCoordinate, CallableParameterIndex, CallableSchemaError,
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

impl CallableSignatureSchema {
    /// Projects a complete remaining group chain. Earlier groups only retain
    /// arguments; the terminal group carries the callable's invocation row.
    /// Projection failures retain the caller context's typed cause.
    pub(crate) fn project_function_type_from_group<E>(
        &self,
        first_group: CallableGroupIndex,
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
            result = TypeKind::function_with_effects(parameters, result, effects);
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
                        CallableName::try_new(format!("arg{}", index + 1)).map_err(|_| {
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
            CallableGenericParameterIssuer::function_scheme(*binder),
            limits,
        )
    }
}
