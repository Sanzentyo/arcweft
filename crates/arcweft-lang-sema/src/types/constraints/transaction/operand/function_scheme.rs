//! One checked scheme opening consumes its exact normalized result source.

use super::*;

pub(crate) struct ConstraintResultUseSource<D: ConstraintDomain> {
    operand: Arc<ConstraintOperand<D>>,
}

pub(crate) struct PreparedConstraintFunctionScheme<D: ConstraintDomain> {
    source: Arc<ConstraintOperand<D>>,
    application: D::Application,
    parameters: super::super::super::TypeConstraintParameterScope,
    template: TypeKind,
}

impl<D: ConstraintDomain> ConstraintResultUseSource<D> {
    pub(in crate::types::constraints::transaction) fn for_result(
        operand: Arc<ConstraintOperand<D>>,
    ) -> Result<Self, TypeConstraintError> {
        if operand.associated_result().is_none() {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        Ok(Self { operand })
    }

    pub(crate) fn source_type(&self) -> &TypeKind {
        self.operand.value()
    }

    pub(crate) fn source_scope(&self) -> &crate::types::GenericScope {
        self.operand.scope().shape()
    }

    /// The supplied prepared schema must open precisely this scheme's outer
    /// binder and D. The template changes no other header or value port.
    pub(crate) fn prepare_function_scheme<A: TypeConstraintAccounting>(
        &self,
        application: D::Application,
        parameters: super::super::super::TypeConstraintParameterScope,
        template: TypeKind,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PreparedConstraintFunctionScheme<D>, TypeConstraintError> {
        let TypeKind::Function {
            binder,
            predicate,
            params,
            return_type,
            effects,
        } = self.source_type()
        else {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        };
        let TypeKind::Function {
            binder: opened_binder,
            predicate: opened_predicate,
            params: opened_params,
            return_type: opened_return,
            effects: opened_effects,
        } = &template
        else {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        };
        for _ in parameters.iter() {
            context.enter_node()?;
        }
        for _ in parameters.const_iter() {
            context.enter_node()?;
        }
        for _ in parameters
            .completed_contract()
            .effect_contract()
            .variables()
        {
            context.enter_node()?;
        }
        if !parameters.matches_function_scheme(*binder, predicate)
            || !opened_binder.is_empty()
            || !opened_predicate.is_unconstrained()
            || opened_params != params
            || opened_return != return_type
            || opened_effects != effects
        {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        Ok(PreparedConstraintFunctionScheme {
            source: Arc::clone(&self.operand),
            application,
            parameters,
            template,
        })
    }
}

impl<D: ConstraintDomain> PreparedConstraintFunctionScheme<D> {
    pub(in crate::types::constraints::transaction) fn instantiate<A: TypeConstraintAccounting>(
        self,
        expected_source: &Arc<ConstraintOperand<D>>,
        mut path: ConstraintPath<D>,
        key: Arc<D::Projection>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(ConstraintPath<D>, ConstraintOperand<D>), TypeConstraintError> {
        if !Arc::ptr_eq(&self.source, expected_source) {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        let source_projection = self
            .source
            .associated_result()
            .ok_or(TypeConstraintInvariant::OriginalProducer)?;
        let scope = context.application_scope(
            self.application,
            self.parameters,
            path.lineage.original.clone(),
        )?;
        let application = scope.id();
        path = context.admit_application(path, scope)?;
        path = TypeConstraintTransaction::prepare_application(context, application, path, None)?;
        path.lineage.projections.push(Arc::new(ProjectionRequest {
            application,
            key: Arc::clone(&key),
            value: self.template,
            closure: TypeConstraintProjectionClosure::Closed,
            source: Some(source_projection),
            input: Some(Arc::clone(&self.source)),
        }));
        let (value, instance) = ResolvedConstraintResult::resolve(
            ConstraintResultProjection { application, key },
            &path,
            context,
        )?;
        let operand = ConstraintOperand {
            value: Arc::new(value),
            scope: self.source.shared_scope(),
            origin: Arc::new(ConstraintOperandOrigin::FunctionSchemeInstantiation {
                instance,
                source: self.source,
            }),
        };
        Ok((path, operand))
    }
}
