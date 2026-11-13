//! Introduced leaves retain the exact application lease that owns their key.

use std::sync::Arc;

use super::{ConstraintOperand, ConstraintOperandOrigin};
use crate::types::{
    ArrayLength, GenericConstReference, GenericTypeReference, TypeKind,
    constraints::{
        ConstraintDomain, TypeConstraintError,
        application::ApplicationParameterScopeLease,
        context::{TypeConstraintAccounting, TypeConstraintContext},
        lexical::ConstraintLexicalScope,
        references,
    },
};

impl<D: ConstraintDomain> ConstraintOperand<D> {
    pub(in crate::types::constraints) fn introduced_type<A: TypeConstraintAccounting>(
        parameter: &GenericTypeReference,
        scope: Arc<ConstraintLexicalScope>,
        owner: ApplicationParameterScopeLease,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        context.enter_node()?;
        if !owner.contains(&parameter.clone().into()) {
            return Err(references::type_out_of_scope(parameter));
        }
        Ok(Self {
            value: Arc::new(TypeKind::GenericParam(parameter.clone())),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Parameter(owner)),
        })
    }
}

impl<D: ConstraintDomain> ConstraintOperand<D, ArrayLength> {
    pub(in crate::types::constraints) fn introduced_const<A: TypeConstraintAccounting>(
        parameter: &GenericConstReference,
        scope: Arc<ConstraintLexicalScope>,
        owner: ApplicationParameterScopeLease,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        context.enter_node()?;
        if !owner.contains(&parameter.clone().into()) {
            return Err(references::const_out_of_scope(parameter));
        }
        Ok(Self {
            value: Arc::new(ArrayLength::Generic(parameter.clone())),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Parameter(owner)),
        })
    }
}
