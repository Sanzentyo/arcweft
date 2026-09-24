//! Projection of Sema-owned callable sources and checked specialization proofs.

use arcweft_core::effect_row::EffectFormula;
use arcweft_core::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableDefault, RuntimeCallableParameterCoordinate,
    RuntimeCallableParameterInput, RuntimeCallableParameterKind, RuntimeCallableRetainedInput,
    RuntimeCallableRetainedRole, RuntimeFunctionSpecializationArguments, RuntimeTypeScope,
};
use arcweft_lang_sema::callable::{
    CallableParameterCoordinate, CallableParameterPassing, CallableParameterPresence,
    CheckedProjectFunctionCallableOrigin, CheckedProjectFunctionCallableSource,
    CheckedProjectFunctionSpecialization,
};
use arcweft_lang_sema::types::ScopedTypeView;
use arcweft_runtime_plan::semantic_facts::{
    RuntimeCallableSpecializationFact, RuntimeProjectCallableSourceFact,
    RuntimeProjectCallableSourceOrigin,
};

use super::*;

fn coordinate(
    source: CallableParameterCoordinate,
) -> Result<RuntimeCallableParameterCoordinate, RuntimeSemanticProjectionError> {
    Ok(RuntimeCallableParameterCoordinate {
        group: u32::try_from(source.group().get()).map_err(|error| {
            RuntimeSemanticProjectionError::Type {
                reason: error.to_string(),
            }
        })?,
        parameter: u32::try_from(source.parameter().get()).map_err(|error| {
            RuntimeSemanticProjectionError::Type {
                reason: error.to_string(),
            }
        })?,
    })
}

pub(super) fn source(
    checked: &CheckedProjectFunctionCallableSource,
    root: Option<arcweft_runtime_plan::semantic_facts::RuntimeProjectCallableSourceKey>,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeProjectCallableSourceFact, RuntimeSemanticProjectionError> {
    let ty = |view: ScopedTypeView<'_>| {
        runtime_type_scoped_at(
            view.value(),
            symbols,
            world,
            analysis,
            &RuntimeTypeProjectionPath::root(),
            view.scope(),
        )
    };
    let callable = runtime_project_callable(checked.declaration(), symbols, world, analysis)
        .map_err(|reason| RuntimeSemanticProjectionError::Type { reason })?;
    let origin = match checked.origin() {
        CheckedProjectFunctionCallableOrigin::Root => RuntimeProjectCallableSourceOrigin::Root,
        CheckedProjectFunctionCallableOrigin::Continuation { lineage } => {
            RuntimeProjectCallableSourceOrigin::Continuation(lineage.runtime_lineage_id())
        }
    };
    let retained = checked
        .retained_parameters()
        .iter()
        .map(|row| {
            Ok(RuntimeCallableRetainedInput {
                role: RuntimeCallableRetainedRole::Parameter(coordinate(row.coordinate())?),
                ty: ty(row.binding_type())?,
            })
        })
        .collect::<Result<_, RuntimeSemanticProjectionError>>()?;
    let parameters = checked
        .parameters()
        .iter()
        .map(|row| {
            if row.presence() != CallableParameterPresence::Required {
                return Err(RuntimeSemanticProjectionError::Type {
                    reason:
                        "ordinary callable source contains an unadmitted optional/default formal"
                            .to_owned(),
                });
            }
            let kind = match row.passing() {
                CallableParameterPassing::PositionalOnly
                | CallableParameterPassing::PositionalOrNamed
                | CallableParameterPassing::NamedOnly => RuntimeCallableParameterKind::Fixed,
                CallableParameterPassing::RestPositional => RuntimeCallableParameterKind::Rest,
                CallableParameterPassing::RestNamed => {
                    return Err(RuntimeSemanticProjectionError::Type {
                        reason: "ordinary callable source contains an unadmitted named rest formal"
                            .to_owned(),
                    });
                }
            };
            Ok(RuntimeCallableParameterInput {
                coordinate: coordinate(row.coordinate())?,
                kind,
                abi_ty: ty(row.abi_type())?,
                binding_ty: ty(row.binding_type())?,
            })
        })
        .collect::<Result<_, RuntimeSemanticProjectionError>>()?;
    let attached = checked
        .attached_source_schema()
        .map(|row| {
            let binding = ty(row.binding_type())?;
            Ok(match row.parameter().presence() {
                CallableParameterPresence::Required => {
                    RuntimeCallableAttachedContract::Required { ty: binding }
                }
                CallableParameterPresence::Optional => {
                    let RuntimeTypeShape::Option { item, .. } = binding.shape() else {
                        return Err(RuntimeSemanticProjectionError::Type {
                            reason: "checked optional attached binding is not Option".to_owned(),
                        });
                    };
                    RuntimeCallableAttachedContract::Optional {
                        value: item.as_ref().clone(),
                        binding,
                    }
                }
                CallableParameterPresence::Defaulted => {
                    RuntimeCallableAttachedContract::Defaulted {
                        ty: binding,
                        default: RuntimeCallableDefault::RequiresSpecialization,
                    }
                }
            })
        })
        .transpose()?
        .unwrap_or(RuntimeCallableAttachedContract::None);
    RuntimeProjectCallableSourceFact::try_new(
        callable,
        checked.source_digest(),
        origin,
        root,
        checked.group(),
        runtime_type(checked.function_type(), symbols, world, analysis)?,
        retained,
        parameters,
        attached,
    )
    .map_err(|error| RuntimeSemanticProjectionError::Type {
        reason: error.to_string(),
    })
}

pub(super) fn specialization(
    source: &RuntimeProjectCallableSourceFact,
    checked: &CheckedProjectFunctionSpecialization,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<RuntimeCallableSpecializationFact, RuntimeSemanticProjectionError> {
    if checked.source_digest() != source.key().digest() {
        return Err(RuntimeSemanticProjectionError::Type {
            reason: "specialization proof names a different checked callable source".to_owned(),
        });
    }
    let (source_type, target_type, arguments) =
        specialization_types(checked, symbols, world, analysis)?;
    let closed = checked.closed_selection();
    let instance = RuntimeProjectFunctionInstanceKey::new(
        source.callable().runtime().clone(),
        closed.solution().instantiation(),
        closed.group(),
    );
    RuntimeCallableSpecializationFact::try_new(
        source_type,
        target_type,
        arguments,
        [(source.key().clone(), instance)],
    )
    .map_err(|error| RuntimeSemanticProjectionError::Type {
        reason: error.to_string(),
    })
}

pub(super) fn specialization_types(
    checked: &CheckedProjectFunctionSpecialization,
    symbols: &ProjectSymbolTable,
    world: &RegisteredSemanticWorld,
    analysis: &FinalSemanticAnalysis,
) -> Result<
    (
        RuntimeNormalizedType,
        RuntimeNormalizedType,
        RuntimeFunctionSpecializationArguments<RuntimeNormalizedType>,
    ),
    RuntimeSemanticProjectionError,
> {
    let arguments = RuntimeFunctionSpecializationArguments {
        types: checked
            .type_arguments()
            .iter()
            .map(|ty| runtime_type(ty, symbols, world, analysis))
            .collect::<Result<_, _>>()?,
        const_lengths: checked
            .const_arguments()
            .iter()
            .map(|length| type_scopes::length(length, &RuntimeTypeScope::root()))
            .collect::<Result<_, _>>()?,
        effects: checked
            .effect_arguments()
            .iter()
            .map(|effects| EffectFormula::literal(effects.clone(), None))
            .collect(),
    };
    Ok((
        runtime_type(checked.source_type(), symbols, world, analysis)?,
        runtime_type(checked.specialized_type(), symbols, world, analysis)?,
        arguments,
    ))
}
