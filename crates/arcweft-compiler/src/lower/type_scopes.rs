//! Projection of checked lexical binders into executable type declarations.

use arcweft_core::effect_row::{DecisionControl, DecisionWork, EffectFormula};
use arcweft_core::plan::{
    RuntimeArrayLength, RuntimeBoundEffectReference, RuntimeFunctionTypeContract,
    RuntimeTypeBinder, RuntimeTypeScope,
};
use arcweft_lang_sema::effect_row::EffectRow;
use arcweft_lang_sema::types::{
    ArrayLength, GenericConstReference, GenericEffectReference, GenericScope,
};

use super::RuntimeSemanticProjectionError;

fn invalid(reason: impl ToString) -> RuntimeSemanticProjectionError {
    RuntimeSemanticProjectionError::Type {
        reason: reason.to_string(),
    }
}

pub(super) fn scope(
    source: &GenericScope,
) -> Result<RuntimeTypeScope, RuntimeSemanticProjectionError> {
    RuntimeTypeScope::try_from_binders(
        source
            .binders()
            .iter()
            .map(|binder| {
                RuntimeTypeBinder::new(binder.types(), binder.const_lengths(), binder.effects())
            })
            .collect::<Box<[_]>>(),
    )
    .map_err(invalid)
}

pub(super) fn length(
    source: &ArrayLength,
    scope: &RuntimeTypeScope,
) -> Result<RuntimeArrayLength, RuntimeSemanticProjectionError> {
    match source {
        ArrayLength::Const(value) => u64::try_from(*value)
            .map(RuntimeArrayLength::Constant)
            .map_err(invalid),
        ArrayLength::Generic(GenericConstReference::Bound(reference)) => scope
            .bound_const(reference.depth(), reference.slot())
            .map(RuntimeArrayLength::Bound)
            .map_err(invalid),
        _ => Err(invalid(
            "unsealed array length reached executable type projection",
        )),
    }
}

struct ProjectionWork(u64);

impl ProjectionWork {
    fn new() -> Self {
        Self(arcweft_core::entry::RuntimeSchemaLimits::engine_default().max_validation_work)
    }
}

impl DecisionControl for ProjectionWork {
    type Error = RuntimeSemanticProjectionError;

    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        self.0 = self.0.checked_sub(1).ok_or_else(|| {
            invalid("function effect projection exceeded its shared validation budget")
        })?;
        Ok(())
    }
}

fn effect_reference(
    reference: &GenericEffectReference,
    scope: &RuntimeTypeScope,
) -> Result<RuntimeBoundEffectReference, RuntimeSemanticProjectionError> {
    match reference {
        GenericEffectReference::Bound(reference) => scope
            .bound_effect(reference.depth(), reference.slot())
            .map_err(invalid),
        _ => Err(invalid(
            "unsealed effect reference reached executable function projection",
        )),
    }
}

fn project_effects(
    effects: &EffectRow,
    scope: &RuntimeTypeScope,
    work: &mut ProjectionWork,
) -> Result<EffectFormula<RuntimeBoundEffectReference>, RuntimeSemanticProjectionError> {
    effects
        .formula()
        .ok_or_else(|| invalid("unknown effect row reached executable function projection"))?
        .map_references(work, &mut |reference, _| effect_reference(reference, scope))
}

pub(super) fn effects(
    source: &EffectRow,
    scope: &RuntimeTypeScope,
) -> Result<EffectFormula<RuntimeBoundEffectReference>, RuntimeSemanticProjectionError> {
    project_effects(source, scope, &mut ProjectionWork::new())
}

pub(super) fn function(
    binder: arcweft_lang_sema::types::GenericBinder,
    predicate: &arcweft_lang_sema::effect_row::EffectPredicate,
    effects: &arcweft_lang_sema::effect_row::EffectRow,
    scope: &RuntimeTypeScope,
) -> Result<RuntimeFunctionTypeContract, RuntimeSemanticProjectionError> {
    let binder = RuntimeTypeBinder::new(binder.types(), binder.const_lengths(), binder.effects());
    let child = scope.enter(binder).map_err(invalid)?;
    let mut work = ProjectionWork::new();
    let invocation = project_effects(effects, &child, &mut work)?;
    let predicate = predicate.map_references(&mut work, &mut |reference, _| {
        effect_reference(reference, &child)
    })?;
    let contract = RuntimeFunctionTypeContract::new(binder, predicate, invocation);
    contract.child_scope(scope).map_err(invalid)?;
    Ok(contract)
}
