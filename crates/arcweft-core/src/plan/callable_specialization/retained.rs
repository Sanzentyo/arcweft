//! Specializing a physical retaining group gives the next function its own
//! binder. Both the type transport and the origin-specific state must agree.

use super::{
    RuntimeCallableSpecializationContext, RuntimeCallableSpecializationError as Error,
    RuntimeCallableStateDefinition, RuntimeCallableTransition, RuntimeFunctionTypeContract,
    RuntimePlanTypeProjection as Type, SpecializationWork, TypeChecker, TypeSide,
};
use crate::plan::RuntimeTypeBinder;

impl RuntimeFunctionTypeContract {
    /// A physical retaining group can move its residual binder to the result
    /// arrow. Substitution arguments then use that result's frame in both
    /// executable validation and original-source codecs.
    pub(in crate::plan) fn retained_result_argument_contract<'a>(
        &self,
        source_result: Option<&Self>,
        target: &Self,
        target_result: Option<&'a Self>,
    ) -> Option<&'a Self> {
        let source_result = source_result?;
        let target_result = target_result?;
        (!self.binder().is_empty()
            && target.binder().is_empty()
            && source_result.binder().is_empty()
            && !target_result.binder().is_empty())
        .then_some(target_result)
    }
}

pub(super) struct RetainedResultBinder<T> {
    binder: RuntimeTypeBinder,
    source_result: T,
    target_result: T,
}

impl<T: Copy + Eq + Ord> RetainedResultBinder<T> {
    pub(super) fn between<C: RuntimeCallableSpecializationContext<Type = T>>(
        context: &C,
        source: &RuntimeFunctionTypeContract,
        source_result: T,
        target: &RuntimeFunctionTypeContract,
        target_result: T,
        work: &mut SpecializationWork,
    ) -> Result<Option<Self>, Error> {
        if source.binder().is_empty() || !target.binder().is_empty() {
            return Ok(None);
        }
        work.visit()?;
        let Some(Type::Function { contract, .. }) = context.type_projection(target_result) else {
            return Ok(None);
        };
        // Ordinary substitution preserves the binder of a literal result
        // arrow. A source result parameter, however, can be replaced with a
        // polymorphic function through ordinary substitution.
        work.visit()?;
        let Some(Type::Function {
            contract: source_contract,
            ..
        }) = context.type_projection(source_result)
        else {
            return Ok(None);
        };
        let Some(argument_contract) = source.retained_result_argument_contract(
            Some(&source_contract),
            target,
            Some(&contract),
        ) else {
            return Ok(None);
        };
        Ok(Some(Self {
            binder: argument_contract.binder(),
            source_result,
            target_result,
        }))
    }

    pub(super) const fn binder(&self) -> RuntimeTypeBinder {
        self.binder
    }

    pub(super) fn compare<C: RuntimeCallableSpecializationContext<Type = T>>(
        &self,
        checker: &mut TypeChecker<'_, C>,
        source: T,
        target: T,
    ) -> Result<(), Error> {
        let Type::Function {
            contract: source,
            parameters: source_parameters,
            ..
        } = checker.projection(source)?
        else {
            return Err(Error::TypeMismatch);
        };
        let Type::Function {
            contract: target,
            parameters: target_parameters,
            ..
        } = checker.projection(target)?
        else {
            return Err(Error::TypeMismatch);
        };
        let side = TypeSide::Source { nested: 0 };
        if source_parameters.len() != target_parameters.len()
            || !target.predicate().is_unconstrained()
            || checker.effect_formula(source.invocation(), side)? != *target.invocation()
        {
            return Err(Error::TypeMismatch);
        }
        for (&source, &target) in source_parameters.iter().zip(&target_parameters) {
            checker.compare(source, target, side, false)?;
        }

        let Type::Function {
            contract: source_result,
            parameters: source_parameters,
            result: source_return,
        } = checker.projection(self.source_result)?
        else {
            return Err(Error::TypeMismatch);
        };
        let Type::Function {
            contract: target_result,
            parameters: target_parameters,
            result: target_return,
        } = checker.projection(self.target_result)?
        else {
            return Err(Error::TypeMismatch);
        };
        // The source result had no binder of its own. Its depth-zero
        // references use the same substitution as the source group's ABI.
        let predicate = checker.effect_predicate(source.predicate(), side)?.and(
            &checker.effect_predicate(source_result.predicate(), side)?,
            &mut checker.work,
        )?;
        if source_parameters.len() != target_parameters.len()
            || !predicate.equal_with(target_result.predicate(), &mut checker.work)?
            || checker.effect_formula(source_result.invocation(), side)?
                != *target_result.invocation()
        {
            return Err(Error::TypeMismatch);
        }
        for (&source, &target) in source_parameters.iter().zip(&target_parameters) {
            checker.compare(source, target, side, false)?;
        }
        checker.compare(source_return, target_return, side, false)
    }

    pub(super) fn validate_states<F>(
        &self,
        source_type: T,
        target_type: T,
        source: &RuntimeCallableStateDefinition<T, F>,
        target: &RuntimeCallableStateDefinition<T, F>,
    ) -> Result<(), Error> {
        if source.function_type != source_type
            || target.function_type != target_type
            || source.result != self.source_result
            || target.result != self.target_result
            || !matches!(
                source.transition,
                RuntimeCallableTransition::RequiresSpecialization
            )
            || !matches!(target.transition, RuntimeCallableTransition::Retain { .. })
        {
            return Err(Error::StateMismatch);
        }
        Ok(())
    }
}
