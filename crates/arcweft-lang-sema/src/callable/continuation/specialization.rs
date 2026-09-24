//! Graph-authorized opening of a known function scheme at one value use.

use super::*;
use crate::types::constraints::{ConstraintDomain, ConstraintSourceReceipt};

/// Affine authorization for a source scheme, bound to its live probe receipt.
/// This is a value conversion, so it has no call site or argument mapping.
pub(in crate::callable) struct PreparedFunctionSpecialization<D: ConstraintDomain> {
    scope: PreparedFunctionSpecializationScope,
    receipt: ConstraintSourceReceipt<D>,
    application: D::Application,
}

pub(in crate::callable) struct PreparedFunctionSpecializationScope {
    initialization: PreparedConstraintInitialization,
    source: TypeKind,
    template: TypeKind,
}

impl PreparedFunctionSpecializationScope {
    pub(in crate::callable) fn into_parts(
        self,
    ) -> (PreparedConstraintInitialization, TypeKind, TypeKind) {
        (self.initialization, self.source, self.template)
    }
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
        Ok(PreparedFunctionSpecialization {
            scope: parent.prepare_function_specialization(source, enclosing, limits)?,
            receipt,
            application,
        })
    }

    pub(in crate::callable) fn prepare_root_function_specialization(
        &self,
        source: &TypeKind,
        enclosing: &EnclosingGenericParameterScope,
        limits: &crate::callable::CallableLimits,
    ) -> Result<PreparedFunctionSpecializationScope, CallConstraintInvariant> {
        PreparedConstraintAuthority {
            issuer: Arc::clone(&self.issuer),
        }
        .prepare_function_specialization(source, enclosing, limits)
    }
}

impl PreparedConstraintAuthority {
    pub(in crate::callable) fn prepare_function_specialization(
        &self,
        source: &TypeKind,
        enclosing: &EnclosingGenericParameterScope,
        limits: &crate::callable::CallableLimits,
    ) -> Result<PreparedFunctionSpecializationScope, CallConstraintInvariant> {
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
        Ok(PreparedFunctionSpecializationScope {
            initialization,
            source: source.clone(),
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
        if !Arc::ptr_eq(&self.scope.initialization.issuer, &authority.issuer) {
            return Err(CallConstraintInvariant::ForeignPreparedIssuer);
        }
        if !self.receipt.matches(receipt) {
            return Err(CallConstraintInvariant::PreparedCallSiteMismatch);
        }
        let (_, parameters, inherited, imported) = self.scope.initialization.into_lower_parts()?;
        if inherited.is_some() || imported.is_some() {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
        Ok((self.application, parameters, self.scope.template))
    }
}
