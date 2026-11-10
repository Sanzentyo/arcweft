//! Producer ownership of a retained group result, over the sealed state table.

use super::{
    CopyProof, Verifier, base_copy_proof, callable_type_admitted, runtime_shape,
    runtime_type_permits_copy,
};
use crate::{
    awbc::{
        schema::{AwbcRuntimeTypeShape, AwbcTypeId},
        verify::AwbcVerifyError,
    },
    plan::{
        RuntimeCallableAttachedContract, RuntimeCallableInputSource, RuntimeCallableParameterKind,
        RuntimeCallableTransition,
    },
};

impl CopyProof {
    fn optional_argument(value: Option<&Self>) -> Self {
        let Some(value) = value else {
            return Self::Copyable;
        };
        let identity = crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionSome;
        let (case, _) = identity
            .owner()
            .resolve_case(identity)
            .expect("builtin Option owns Some");
        Self::Variant {
            case,
            payload: Some(Box::new(Self::Tuple(vec![value.clone()]))),
        }
    }
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

/// Join every live state admitted by this exact declaration and source arrow.
/// Retention uses its actual remaining coordinates, including absent Option
/// bindings, so an unrestricted capture keeps its producer Copy proof.
pub(super) fn declared_group(
    verifier: &super::Verifier<'_, '_>,
    call: &crate::awbc::schema::AwbcProjectCall,
    base: &crate::plan::RuntimeCallableStateDefinition<
        AwbcTypeId,
        crate::awbc::schema::AwbcFunctionId,
    >,
    state: &super::FlowState,
    source: AwbcTypeId,
    work: &mut usize,
) -> Result<CopyProof, AwbcVerifyError> {
    use crate::awbc::schema::AwbcProjectCallOrdinaryMaterialization as Row;
    let program = verifier.program;
    let proofs = super::project_call_argument_copy_proofs(call, state);
    let supplied = call
        .ordinary
        .iter()
        .zip(&proofs)
        .map(|(row, proof)| {
            let (Row::Fixed { parameter, .. } | Row::Rest { parameter, .. }) = row;
            (*parameter, proof)
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let parameters = supplied.keys().copied().collect::<Vec<_>>();
    let callee = &state.copy_proofs[call.callee.index()];
    let mut joined: Option<CopyProof> = None;
    for candidate in &program.callable_states {
        charge_work(work, 1, verifier.budget.dataflow_steps)?;
        if candidate.origin != base.origin
            || candidate.active_group() != base.active_group()
            || candidate.function_type != source
            || !candidate.admits_declared_parameters(&parameters)
        {
            continue;
        }
        charge_work(
            work,
            candidate.parameters.len(),
            verifier.budget.dataflow_steps,
        )?;
        let logical = candidate
            .parameters
            .iter()
            .map(|input| {
                let value = supplied.get(&input.coordinate.parameter).copied();
                match input.kind {
                    RuntimeCallableParameterKind::Optional => CopyProof::optional_argument(value),
                    RuntimeCallableParameterKind::Fixed => {
                        value.expect("admitted required coordinate").clone()
                    }
                    RuntimeCallableParameterKind::Rest => {
                        value.cloned().unwrap_or(CopyProof::Copyable)
                    }
                }
            })
            .collect::<Vec<_>>();
        let proof = if let RuntimeCallableTransition::Retain { values, .. } = &candidate.transition
        {
            charge_work(work, values.len(), verifier.budget.dataflow_steps)?;
            callee.retaining(values, &logical, &CopyProof::Affine)
        } else {
            base_copy_proof(program, candidate.result)
        };
        joined = Some(match joined {
            Some(previous) => previous.merge(&proof),
            None => proof,
        });
    }
    joined.ok_or_else(|| AwbcVerifyError::InvalidInvariant {
        at: "declared project-call".to_owned(),
        message: "callee has no admitted remaining declaration state".to_owned(),
    })
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
/// produced by every state that can inhabit the source arrow. Actual
/// body invocation has no interprocedural ownership summary and stays opaque.
pub(super) fn applied_group(
    verifier: &Verifier<'_, '_>,
    owner: Option<AwbcTypeId>,
    (source, result): (AwbcTypeId, AwbcTypeId),
    callee: &CopyProof,
    arguments: &[CopyProof],
    work: &mut usize,
) -> Result<CopyProof, AwbcVerifyError> {
    let program = verifier.program;
    let budget = verifier.budget.dataflow_steps;
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
        if !callable_type_admitted(
            program,
            owner,
            source,
            definition.function_type,
            work,
            budget,
        )? {
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
                        RuntimeCallableParameterKind::Optional => {
                            CopyProof::optional_argument(Some(proof))
                        }
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
