//! Projection of checked lexical binders into executable type declarations.

use arcweft_core::effect_row::{DecisionControl, DecisionWork};
use arcweft_core::plan::{
    RuntimeArrayLength, RuntimeFunctionTypeContract, RuntimeTypeBinder, RuntimeTypeScope,
};
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

impl DecisionControl for ProjectionWork {
    type Error = RuntimeSemanticProjectionError;

    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        self.0 = self.0.checked_sub(1).ok_or_else(|| {
            invalid("function effect projection exceeded its shared validation budget")
        })?;
        Ok(())
    }
}

pub(super) fn function(
    binder: arcweft_lang_sema::types::GenericBinder,
    predicate: &arcweft_lang_sema::effect_row::EffectPredicate,
    effects: &arcweft_lang_sema::effect_row::EffectRow,
    scope: &RuntimeTypeScope,
) -> Result<RuntimeFunctionTypeContract, RuntimeSemanticProjectionError> {
    let binder = RuntimeTypeBinder::new(binder.types(), binder.const_lengths(), binder.effects());
    let child = scope.enter(binder).map_err(invalid)?;
    let mut work = ProjectionWork(
        arcweft_core::entry::RuntimeSchemaLimits::engine_default().max_validation_work,
    );
    let mut reference = |reference: &GenericEffectReference, _: &mut ProjectionWork| match reference
    {
        GenericEffectReference::Bound(reference) => child
            .bound_effect(reference.depth(), reference.slot())
            .map_err(invalid),
        _ => Err(invalid(
            "unsealed effect reference reached executable function projection",
        )),
    };
    let invocation = effects
        .formula()
        .ok_or_else(|| invalid("unknown effect row reached executable function projection"))?
        .map_references(&mut work, &mut reference)?;
    let predicate = predicate.map_references(&mut work, &mut reference)?;
    let contract = RuntimeFunctionTypeContract::new(binder, predicate, invocation);
    contract.child_scope(scope).map_err(invalid)?;
    Ok(contract)
}
