//! Declaration bindings cross the provider boundary under the original lease.

use super::{RuntimeExternalCallContext, RuntimeExternalCallContextState};
use crate::{
    pattern::{RuntimeBuiltinVariantCaseIdentity as Case, RuntimeSemanticTypeId},
    program_types::RuntimeProgramTypeError,
    value::{
        RuntimeCallTarget, RuntimeExternalCallOperandError,
        RuntimeExternalCallOperandProjection as Projection, RuntimeExternalCallable, RuntimeValue,
    },
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeExternalCallArgumentError {
    #[error("declaration binding projection requires the original program context")]
    Unbound,
    #[error("external bindings have {actual} values, expected {expected}")]
    Count { expected: usize, actual: usize },
    #[error(transparent)]
    Layout(#[from] RuntimeExternalCallOperandError),
    #[error("external argument {argument} is outside its binding inventory")]
    Missing { argument: u32 },
    #[error("external argument {argument} cannot bind type {ty:?}: {source}")]
    Type {
        argument: usize,
        ty: RuntimeSemanticTypeId,
        source: Box<RuntimeProgramTypeError>,
    },
    #[error("external argument {argument} must bind a canonical Option, found {ty:?}")]
    ExpectedOption {
        argument: usize,
        ty: RuntimeSemanticTypeId,
    },
    #[error("external argument {argument} has an invalid canonical Option value")]
    OptionValue { argument: usize },
}

#[derive(Debug)]
pub(crate) struct RuntimePreparedExternalCall {
    pub(crate) context: RuntimeExternalCallContext,
    pub(crate) callee: RuntimeCallTarget,
    pub(crate) arguments: Vec<RuntimeValue>,
}

#[derive(Clone, Copy)]
enum AdmittedArgument {
    Value(RuntimeSemanticTypeId),
    OptionalSome(RuntimeSemanticTypeId),
    Omitted,
}

impl AdmittedArgument {
    fn inspect_bindings(
        execution: &super::RuntimeProgramExecutionContext,
        callee: &RuntimeExternalCallable,
        argument_types: &[RuntimeSemanticTypeId],
        arguments: &[RuntimeValue],
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<Vec<Self>, Box<RuntimeExternalCallArgumentError>> {
        use RuntimeExternalCallArgumentError as Error;
        let types = execution.types();
        for (argument, (ty, value)) in argument_types.iter().copied().zip(arguments).enumerate() {
            types
                .validate_live_value(ty, value, limits)
                .map_err(|source| {
                    Box::new(Error::Type {
                        argument,
                        ty,
                        source: Box::new(source),
                    })
                })?;
        }
        let mut projections = argument_types
            .iter()
            .copied()
            .map(AdmittedArgument::Value)
            .collect::<Vec<_>>();
        for row in callee.operands() {
            if row.projection == Projection::Value {
                continue;
            }
            let index = usize::try_from(row.abi_position).map_err(|_| {
                Box::new(Error::Missing {
                    argument: row.abi_position,
                })
            })?;
            let ty = *argument_types.get(index).ok_or_else(|| {
                Box::new(Error::Missing {
                    argument: row.abi_position,
                })
            })?;
            projections[index] =
                AdmittedArgument::inspect_optional(types, index, ty, &arguments[index])?;
        }
        Ok(projections)
    }

    fn inspect_optional(
        types: crate::program_types::RuntimeProgramTypes<'_>,
        argument: usize,
        ty: RuntimeSemanticTypeId,
        value: &RuntimeValue,
    ) -> Result<Self, Box<RuntimeExternalCallArgumentError>> {
        use RuntimeExternalCallArgumentError as Error;
        let item = types
            .value_type(ty)
            .map_err(|source| {
                Box::new(Error::Type {
                    argument,
                    ty,
                    source: Box::new(source),
                })
            })?
            .option_item()
            .ok_or_else(|| Box::new(Error::ExpectedOption { argument, ty }))?;
        match value.view().builtin_variant_case() {
            Some((Case::OptionSome, Some(_))) => Ok(Self::OptionalSome(item)),
            Some((Case::OptionNone, None)) => Ok(Self::Omitted),
            _ => Err(Box::new(Error::OptionValue { argument })),
        }
    }

    const fn ty(self) -> Option<RuntimeSemanticTypeId> {
        match self {
            Self::Value(ty) | Self::OptionalSome(ty) => Some(ty),
            Self::Omitted => None,
        }
    }

    fn take(self, value: RuntimeValue) -> Option<RuntimeValue> {
        match self {
            Self::Value(_) => Some(value),
            Self::Omitted => None,
            Self::OptionalSome(_) => {
                let Ok((Case::OptionSome, Some(value))) = value.try_into_builtin_variant_case()
                else {
                    unreachable!("borrowed canonical binding proof")
                };
                Some(value)
            }
        }
    }
}

impl RuntimeExternalCallContext {
    /// All source bindings and types are inspected before any payload is moved.
    /// The projected context keeps the original execution, formatter and limits.
    pub(crate) fn prepare_external_bindings(
        &self,
        callee: &RuntimeExternalCallable,
        arguments: Vec<RuntimeValue>,
    ) -> Result<RuntimePreparedExternalCall, Box<RuntimeExternalCallArgumentError>> {
        use RuntimeExternalCallArgumentError as Error;
        let RuntimeExternalCallContextState::Program {
            execution,
            argument_types,
            result_type,
            limits,
        } = &self.state
        else {
            return Err(Box::new(Error::Unbound));
        };
        if argument_types.len() != arguments.len() {
            return Err(Box::new(Error::Count {
                expected: argument_types.len(),
                actual: arguments.len(),
            }));
        }
        let count = callee
            .operands()
            .iter()
            .try_fold(0_usize, |count, row| {
                count.checked_add(row.expansion_width as usize)
            })
            .ok_or_else(|| {
                Box::new(Error::Layout(RuntimeExternalCallOperandError::InvalidRange))
            })?;
        if count != arguments.len() {
            return Err(Box::new(Error::Count {
                expected: count,
                actual: arguments.len(),
            }));
        }
        let projections = AdmittedArgument::inspect_bindings(
            execution,
            callee,
            argument_types,
            &arguments,
            *limits,
        )?;
        // Canonical ranges bound the original argument count to u32.
        let positions = std::iter::once(0_u32)
            .chain(projections.iter().scan(0_u32, |position, row| {
                *position = position
                    .checked_add(u32::from(row.ty().is_some()))
                    .expect("admitted external argument range");
                Some(*position)
            }))
            .collect::<Vec<_>>();
        let operands = callee
            .operands()
            .iter()
            .filter_map(|row| {
                let index = row.abi_position as usize;
                if row.projection == Projection::OptionalBinding
                    && projections[index].ty().is_none()
                {
                    return None;
                }
                Some(crate::value::RuntimeExternalCallOperand {
                    projection: Projection::Value,
                    parameter: row.parameter,
                    abi_position: positions[index],
                    expansion_width: row.expansion_width,
                })
            })
            .collect();
        let callee = callee
            .clone()
            .with_operands(operands)
            .map_err(|source| Box::new(Error::Layout(source)))?;
        let values = arguments
            .into_iter()
            .zip(&projections)
            .filter_map(|(value, projection)| projection.take(value))
            .collect();
        Ok(RuntimePreparedExternalCall {
            context: Self {
                state: RuntimeExternalCallContextState::Program {
                    execution: execution.clone(),
                    argument_types: projections
                        .into_iter()
                        .filter_map(AdmittedArgument::ty)
                        .collect(),
                    result_type: *result_type,
                    limits: *limits,
                },
            },
            callee: RuntimeCallTarget::External(callee),
            arguments: values,
        })
    }
}
