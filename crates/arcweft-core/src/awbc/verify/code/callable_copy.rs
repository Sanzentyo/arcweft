//! Producer ownership of a retained group result, over the sealed state table.

use super::{CopyProof, base_copy_proof, runtime_shape, runtime_type_permits_copy};
use crate::{
    awbc::{
        schema::{AwbcProgram, AwbcRuntimeTypeShape, AwbcTypeId},
        verify::AwbcVerifyError,
    },
    plan::{
        RuntimeCallableAttachedContract, RuntimeCallableInputSource, RuntimeCallableParameterKind,
        RuntimeCallableTransition,
    },
};

impl CopyProof {
    pub(super) fn retaining(
        &self,
        values: &[RuntimeCallableInputSource],
        arguments: &[CopyProof],
        attached: &CopyProof,
    ) -> Self {
        Self::Function(
            values
                .iter()
                .map(|source| match source {
                    RuntimeCallableInputSource::Retained { position } => match self {
                        Self::Copyable => Self::Copyable,
                        Self::Function(retained) => retained
                            .get(*position as usize)
                            .cloned()
                            .unwrap_or(Self::Affine),
                        _ => Self::Affine,
                    },
                    RuntimeCallableInputSource::Argument { position } => arguments
                        .get(*position as usize)
                        .cloned()
                        .unwrap_or(Self::Affine),
                    RuntimeCallableInputSource::Attached => attached.clone(),
                })
                .collect(),
        )
        .compact()
    }
}

pub(super) fn charge_work(
    work: &mut usize,
    amount: usize,
    budget: usize,
) -> Result<(), AwbcVerifyError> {
    *work = work
        .checked_add(amount)
        .filter(|total| *total <= budget)
        .ok_or(AwbcVerifyError::BudgetExceeded {
            budget: "dataflow_steps",
        })?;
    Ok(())
}

/// An arrow alone does not prove a function result Copy. Meet the ownership
/// produced by every state that can inhabit this exact source arrow. Actual
/// body invocation has no interprocedural ownership summary and stays opaque.
pub(super) fn applied_group(
    program: &AwbcProgram,
    source: AwbcTypeId,
    result: AwbcTypeId,
    callee: &CopyProof,
    arguments: &[CopyProof],
    work: &mut usize,
    budget: usize,
) -> Result<CopyProof, AwbcVerifyError> {
    if runtime_type_permits_copy(program, result, 0) {
        return Ok(CopyProof::Copyable);
    }
    let Some(AwbcRuntimeTypeShape::Function { parameters, .. }) = runtime_shape(program, source)
    else {
        return Ok(CopyProof::Affine);
    };
    let mut joined: Option<CopyProof> = None;
    for definition in &program.callable_states {
        charge_work(work, 1, budget)?;
        if definition.function_type != source {
            continue;
        }
        let count = definition
            .parameters
            .len()
            .checked_add(arguments.len())
            .ok_or(AwbcVerifyError::BudgetExceeded {
                budget: "dataflow_steps",
            })?;
        charge_work(work, count, budget)?;
        let proof = match &definition.attached {
            RuntimeCallableAttachedContract::None => {
                let logical = definition
                    .parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, proof)| match parameter.kind {
                        RuntimeCallableParameterKind::Fixed => proof.clone(),
                        RuntimeCallableParameterKind::Rest => {
                            CopyProof::Sequence(vec![proof.clone()])
                        }
                    })
                    .collect::<Vec<_>>();
                let values = if arguments.len() < parameters.len() {
                    if arguments.is_empty() {
                        joined = Some(match joined {
                            Some(previous) => previous.merge(callee),
                            None => callee.clone(),
                        });
                        continue;
                    }
                    definition
                        .partials
                        .iter()
                        .find(|partial| {
                            definition
                                .parameters
                                .iter()
                                .take(arguments.len())
                                .map(|parameter| parameter.coordinate)
                                .eq(partial.parameters.iter().copied())
                        })
                        .map(|partial| partial.values.as_ref())
                } else {
                    match &definition.transition {
                        RuntimeCallableTransition::Retain { values, .. } => Some(values.as_ref()),
                        RuntimeCallableTransition::Invoke { .. }
                        | RuntimeCallableTransition::RequiresSpecialization => None,
                    }
                };
                if let Some(values) = values {
                    charge_work(work, values.len(), budget)?;
                    callee.retaining(values, &logical, &CopyProof::Affine)
                } else {
                    base_copy_proof(program, result)
                }
            }
            _ => {
                // Optional/default wrapping belongs to the explicit materialization
                // contract, which supplies its own attached producer projection.
                base_copy_proof(program, result)
            }
        };
        joined = Some(match joined {
            Some(previous) => previous.merge(&proof),
            None => proof,
        });
    }
    Ok(joined.unwrap_or_else(|| base_copy_proof(program, result)))
}
