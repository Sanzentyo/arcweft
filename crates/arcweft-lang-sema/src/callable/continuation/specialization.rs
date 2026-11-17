//! Graph-authorized opening of a known function scheme at one value use.

use super::*;
use crate::callable::FunctionSpecializationFailure;
use crate::effect_row::prepared::{PreparedEffectPredicate, PreparedEffectReadView};
use crate::types::constraints::{
    ConstraintDomain, ConstraintSourceReceipt, TypeConstraintError,
    context::{TypeConstraintAccounting, TypeConstraintContext},
};
use crate::types::{
    GenericScope, PreparedEffects, PreparedTypeRetentionFailure, TypeProjectionError,
};

/// Affine authorization for a source scheme, bound to its live probe receipt.
/// This is a value conversion, so it has no call site or argument mapping.
pub(in crate::callable) struct PreparedFunctionSpecialization<D: ConstraintDomain> {
    scope: PreparedFunctionSpecializationScope,
    receipt: ConstraintSourceReceipt<D>,
    application: D::Application,
}

pub(in crate::callable) struct PreparedFunctionSpecializationScope {
    initialization: PreparedConstraintInitialization,
    template: TypeKind<PreparedEffects>,
}

impl PreparedFunctionSpecializationScope {
    pub(in crate::callable) fn into_parts(
        self,
    ) -> (PreparedConstraintInitialization, TypeKind<PreparedEffects>) {
        (self.initialization, self.template)
    }
}

impl<P, U> PreparedCallGraph<P, U> {
    pub(in crate::callable) fn issue_function_specialization<
        D: ConstraintDomain,
        A: TypeConstraintAccounting,
    >(
        &self,
        parent: &PreparedConstraintAuthority,
        receipt: ConstraintSourceReceipt<D>,
        application: D::Application,
        source: &TypeKind<PreparedEffects>,
        source_scope: &GenericScope,
        enclosing: &EnclosingGenericParameterScope,
        limits: &crate::callable::CallableLimits,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PreparedFunctionSpecialization<D>, FunctionSpecializationFailure<D>> {
        self.validate_constraint_authority(parent)
            .map_err(FunctionSpecializationFailure::Prepared)?;
        let graph = self
            .effect_declarations()
            .map_err(FunctionSpecializationFailure::Prepared)?
            .read();
        Ok(PreparedFunctionSpecialization {
            scope: parent.prepare_function_specialization(
                source,
                source_scope,
                enclosing,
                limits,
                graph,
                context,
            )?,
            receipt,
            application,
        })
    }

    pub(in crate::callable) fn prepare_root_function_specialization<
        D: ConstraintDomain,
        A: TypeConstraintAccounting,
    >(
        &self,
        expression: ExprId,
        source: &TypeKind<PreparedEffects>,
        enclosing: &EnclosingGenericParameterScope,
        limits: &crate::callable::CallableLimits,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PreparedFunctionSpecializationScope, FunctionSpecializationFailure<D>> {
        if !enclosing.header.source.admits(expression) {
            return Err(FunctionSpecializationFailure::Prepared(
                CallConstraintInvariant::PreparedCallSiteMismatch,
            ));
        }
        let graph = self
            .effect_declarations()
            .map_err(FunctionSpecializationFailure::Prepared)?
            .read();
        PreparedConstraintAuthority {
            issuer: Arc::clone(&self.issuer),
            enclosing: enclosing.clone(),
        }
        .prepare_function_specialization(
            source,
            &GenericScope::default(),
            enclosing,
            limits,
            graph,
            context,
        )
    }
}

impl PreparedConstraintAuthority {
    pub(in crate::callable) fn prepare_function_specialization<
        D: ConstraintDomain,
        A: TypeConstraintAccounting,
    >(
        &self,
        source: &TypeKind<PreparedEffects>,
        source_scope: &GenericScope,
        enclosing: &EnclosingGenericParameterScope,
        limits: &crate::callable::CallableLimits,
        graph: PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PreparedFunctionSpecializationScope, FunctionSpecializationFailure<D>> {
        if self.enclosing.header.source != enclosing.header.source {
            return Err(FunctionSpecializationFailure::Prepared(
                CallConstraintInvariant::PreparedCallSiteMismatch,
            ));
        }
        let TypeKind::Function { binder, params, .. } = source else {
            return Err(FunctionSpecializationFailure::Prepared(
                CallConstraintInvariant::PreparedFunctionTypeMismatch,
            ));
        };
        if binder.is_empty() {
            return Err(FunctionSpecializationFailure::Prepared(
                CallConstraintInvariant::PreparedFunctionTypeMismatch,
            ));
        }
        if params.len() > limits.max_parameters_per_callable() {
            return Err(FunctionSpecializationFailure::Prepared(
                CallConstraintInvariant::FunctionSchemeSchema(
                    super::super::CallableSchemaError::ParameterLimit {
                        actual: params.len(),
                        limit: limits.max_parameters_per_callable(),
                    },
                ),
            ));
        }
        let retained = source
            .retain_effects(source_scope, graph, context)
            .map_err(|error| match error {
                TypeProjectionError::Instantiation(error) => {
                    FunctionSpecializationFailure::Prepared(error.into())
                }
                TypeProjectionError::Control(PreparedTypeRetentionFailure::Control(error)) => {
                    FunctionSpecializationFailure::Constraint(error.into())
                }
                TypeProjectionError::Control(PreparedTypeRetentionFailure::Effect(error)) => {
                    FunctionSpecializationFailure::Constraint(
                        TypeConstraintError::from(error).into(),
                    )
                }
            })?;
        let TypeKind::Function {
            binder,
            predicate,
            params,
            return_type,
            effects,
        } = retained
        else {
            return Err(FunctionSpecializationFailure::Prepared(
                CallConstraintInvariant::PreparedFunctionTypeMismatch,
            ));
        };
        let parameter_scope = TypeConstraintParameterScope::for_function_scheme(
            binder,
            predicate,
            enclosing.types(),
            enclosing.consts(),
            enclosing.effects(),
            context,
        )
        .map_err(|error| FunctionSpecializationFailure::Constraint(error.into()))?;
        context
            .enter_node()
            .map_err(|error| FunctionSpecializationFailure::Constraint(error.into()))?;
        let initialization = PreparedConstraintInitialization {
            issuer: Arc::clone(&self.issuer),
            parameter_scope,
            future_parameters: Box::new([]),
            continuation_seed: PreparedCallConstraintSeed::None {
                issuer: Arc::clone(&self.issuer),
            },
            imported_scope: enclosing.imported_scope().cloned(),
            enclosing: enclosing.clone(),
        };
        // The outer binder becomes this application's lexical template scope.
        // Its predicate enters that same scope's constraint environment. Nested
        // function binders and their predicates stay attached to their values.
        let template = TypeKind::Function {
            binder: crate::types::GenericBinder::EMPTY,
            predicate: PreparedEffectPredicate::Canonical(
                crate::effect_row::EffectPredicate::unconstrained(),
            ),
            params,
            return_type,
            effects,
        };
        Ok(PreparedFunctionSpecializationScope {
            initialization,
            template,
        })
    }
}

impl<D: ConstraintDomain> PreparedFunctionSpecialization<D> {
    pub(in crate::callable) fn into_lower_parts(
        self,
        authority: &PreparedConstraintAuthority,
        receipt: &ConstraintSourceReceipt<D>,
    ) -> Result<
        (
            D::Application,
            TypeConstraintParameterScope,
            TypeKind<PreparedEffects>,
        ),
        CallConstraintInvariant,
    > {
        if !Arc::ptr_eq(&self.scope.initialization.issuer, &authority.issuer) {
            return Err(CallConstraintInvariant::ForeignPreparedIssuer);
        }
        if !self.receipt.matches(receipt)
            || self.scope.initialization.enclosing.header.source
                != authority.enclosing.header.source
        {
            return Err(CallConstraintInvariant::PreparedCallSiteMismatch);
        }
        let (_, parameters, inherited, imported) = self.scope.initialization.into_lower_parts()?;
        if inherited.is_some() || imported.is_some() {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
        Ok((self.application, parameters, self.scope.template))
    }
}
