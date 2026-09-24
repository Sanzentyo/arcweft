//! Graph-authorized opening of a known function scheme at one value use.

use super::*;
use crate::types::constraints::{ConstraintDomain, ConstraintSourceReceipt};

/// Affine authorization for a source scheme, bound to its live probe receipt.
/// This is a value conversion, so it has no call site or argument mapping.
pub(in crate::callable) struct PreparedFunctionSpecialization<D: ConstraintDomain> {
    initialization: PreparedConstraintInitialization,
    receipt: ConstraintSourceReceipt<D>,
    application: D::Application,
    template: TypeKind,
}

impl<P, U> PreparedCallGraph<P, U> {
    pub(in crate::callable) fn issue_function_specialization<D: ConstraintDomain>(
        &self,
        parent: &PreparedConstraintAuthority,
        receipt: ConstraintSourceReceipt<D>,
        application: D::Application,
        source: &TypeKind,
        enclosing: &EnclosingGenericParameterScope,
        limits: &crate::callable::CallableLimits,
    ) -> Result<PreparedFunctionSpecialization<D>, CallConstraintInvariant> {
        self.validate_constraint_authority(parent)?;
        let TypeKind::Function {
            binder,
            params,
            return_type,
            effects,
            ..
        } = source
        else {
            return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch);
        };
        if binder.is_empty() {
            return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch);
        }
        let schema = crate::callable::CallableSignatureSchema::for_function_value(source, limits)
            .map_err(CallConstraintInvariant::FunctionSchemeSchema)?;
        let initialization = issue_scope_initialization(
            Arc::clone(&self.issuer),
            ConstraintScopeUse::Specialization(&schema),
            enclosing,
            None,
        )?;
        // The outer binder becomes this application's lexical template scope.
        // Its predicate enters that same scope's constraint environment. Nested
        // function binders and their predicates stay attached to their values.
        let template = TypeKind::function_with_effects(
            params.clone(),
            return_type.as_ref().clone(),
            effects.clone(),
        );
        Ok(PreparedFunctionSpecialization {
            initialization,
            receipt,
            application,
            template,
        })
    }
}

impl<D: ConstraintDomain> PreparedFunctionSpecialization<D> {
    pub(in crate::callable) fn into_lower_parts(
        self,
        authority: &PreparedConstraintAuthority,
        receipt: &ConstraintSourceReceipt<D>,
    ) -> Result<(D::Application, TypeConstraintParameterScope, TypeKind), CallConstraintInvariant>
    {
        if !Arc::ptr_eq(&self.initialization.issuer, &authority.issuer) {
            return Err(CallConstraintInvariant::ForeignPreparedIssuer);
        }
        if !self.receipt.matches(receipt) {
            return Err(CallConstraintInvariant::PreparedCallSiteMismatch);
        }
        let (_, parameters, inherited, imported) = self.initialization.into_lower_parts()?;
        if inherited.is_some() || imported.is_some() {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
        Ok((self.application, parameters, self.template))
    }
}
