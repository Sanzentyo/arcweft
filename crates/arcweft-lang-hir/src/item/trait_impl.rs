//! Final inline Trait and Impl item payloads.

use crate::identity::{HirModuleId, LocalId, PatternId, ScopeId, TypeId};

use super::{
    HirFunctionBody, HirGenericParameter, HirItemInvariantError, HirItemPrefix, HirParameter,
    HirRequiredName, HirWherePredicate, validate_function_body, validate_generic_parameters,
    validate_locals, validate_optional_type, validate_parameters, validate_pattern, validate_scope,
    validate_type, validate_types, validate_where_predicates,
};

/// Ownership mode of one typed method receiver.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirMethodReceiverKind {
    Owned,
    SharedReference,
    MutableReference,
}

/// One method receiver. Its schema deliberately has no Type ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMethodReceiver {
    kind: HirMethodReceiverKind,
    pattern: PatternId,
    locals: Box<[LocalId]>,
}

impl HirMethodReceiver {
    pub(crate) fn try_new(
        kind: HirMethodReceiverKind,
        pattern: PatternId,
        locals: Box<[LocalId]>,
    ) -> Result<Self, HirItemInvariantError> {
        validate_locals(pattern.module(), &locals)?;
        if locals.len() != 1 {
            return Err(HirItemInvariantError::MethodReceiverBindingCount {
                actual: locals.len(),
            });
        }
        Ok(Self {
            kind,
            pattern,
            locals,
        })
    }

    pub const fn kind(&self) -> HirMethodReceiverKind {
        self.kind
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    pub const fn locals(&self) -> &[LocalId] {
        &self.locals
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        validate_pattern(expected, self.pattern)?;
        validate_locals(expected, &self.locals)
    }
}

/// One source-ordered method parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirMethodParameter {
    Receiver(HirMethodReceiver),
    Typed(HirParameter),
}

impl HirMethodParameter {
    pub const fn receiver(&self) -> Option<&HirMethodReceiver> {
        match self {
            Self::Receiver(receiver) => Some(receiver),
            Self::Typed(_) => None,
        }
    }

    pub const fn typed(&self) -> Option<&HirParameter> {
        match self {
            Self::Receiver(_) => None,
            Self::Typed(parameter) => Some(parameter),
        }
    }

    pub const fn locals(&self) -> &[LocalId] {
        match self {
            Self::Receiver(receiver) => receiver.locals(),
            Self::Typed(parameter) => parameter.locals(),
        }
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        match self {
            Self::Receiver(receiver) => receiver.validate_module(expected),
            Self::Typed(parameter) => {
                validate_parameters(expected, core::slice::from_ref(parameter))
            }
        }
    }
}

/// One source-ordered curried method parameter group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMethodParameterGroup {
    parameters: Box<[HirMethodParameter]>,
}

impl HirMethodParameterGroup {
    pub(crate) fn try_new(
        expected: HirModuleId,
        parameters: Box<[HirMethodParameter]>,
    ) -> Result<Self, HirItemInvariantError> {
        for parameter in &parameters {
            parameter.validate_module(expected)?;
        }
        Ok(Self { parameters })
    }

    pub const fn parameters(&self) -> &[HirMethodParameter] {
        &self.parameters
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        for parameter in &self.parameters {
            parameter.validate_module(expected)?;
        }
        Ok(())
    }
}

/// One associated-type declaration owned inline by a Trait.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirTraitAssociatedType {
    prefix: HirItemPrefix,
    name: HirRequiredName,
    generic_parameters: Box<[HirGenericParameter]>,
    default: Option<TypeId>,
}

impl HirTraitAssociatedType {
    pub(crate) fn try_new(
        expected: HirModuleId,
        prefix: HirItemPrefix,
        name: HirRequiredName,
        generic_parameters: Box<[HirGenericParameter]>,
        default: Option<TypeId>,
    ) -> Result<Self, HirItemInvariantError> {
        prefix.validate_module(expected)?;
        validate_generic_parameters(expected, &generic_parameters)?;
        validate_optional_type(expected, default)?;
        Ok(Self {
            prefix,
            name,
            generic_parameters,
            default,
        })
    }

    pub const fn prefix(&self) -> &HirItemPrefix {
        &self.prefix
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn default(&self) -> Option<TypeId> {
        self.default
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        self.prefix.validate_module(expected)?;
        validate_generic_parameters(expected, &self.generic_parameters)?;
        validate_optional_type(expected, self.default)
    }

    const fn has_structural_recovery(&self) -> bool {
        self.name.is_recovered()
    }
}

/// One associated-type assignment owned inline by an Impl.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirImplAssociatedType {
    prefix: HirItemPrefix,
    name: HirRequiredName,
    generic_parameters: Box<[HirGenericParameter]>,
    target: TypeId,
}

impl HirImplAssociatedType {
    pub(crate) fn try_new(
        expected: HirModuleId,
        prefix: HirItemPrefix,
        name: HirRequiredName,
        generic_parameters: Box<[HirGenericParameter]>,
        target: TypeId,
    ) -> Result<Self, HirItemInvariantError> {
        prefix.validate_module(expected)?;
        validate_generic_parameters(expected, &generic_parameters)?;
        validate_type(expected, target)?;
        Ok(Self {
            prefix,
            name,
            generic_parameters,
            target,
        })
    }

    pub const fn prefix(&self) -> &HirItemPrefix {
        &self.prefix
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn target(&self) -> TypeId {
        self.target
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        self.prefix.validate_module(expected)?;
        validate_generic_parameters(expected, &self.generic_parameters)?;
        validate_type(expected, self.target)
    }

    const fn has_structural_recovery(&self) -> bool {
        self.name.is_recovered()
    }
}

/// One Trait method signature or default method.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirTraitFunction {
    prefix: HirItemPrefix,
    name: HirRequiredName,
    generic_parameters: Box<[HirGenericParameter]>,
    parameter_groups: Box<[HirMethodParameterGroup]>,
    where_predicates: Box<[HirWherePredicate]>,
    return_type: Option<TypeId>,
    callable_scope: ScopeId,
    body: Option<HirFunctionBody>,
}

impl HirTraitFunction {
    #[allow(
        clippy::too_many_arguments,
        reason = "the constructor validates the complete typed trait-method declaration schema"
    )]
    pub(crate) fn try_new(
        expected: HirModuleId,
        prefix: HirItemPrefix,
        name: HirRequiredName,
        generic_parameters: Box<[HirGenericParameter]>,
        parameter_groups: Box<[HirMethodParameterGroup]>,
        where_predicates: Box<[HirWherePredicate]>,
        return_type: Option<TypeId>,
        callable_scope: ScopeId,
        body: Option<HirFunctionBody>,
    ) -> Result<Self, HirItemInvariantError> {
        validate_method(
            expected,
            &prefix,
            &generic_parameters,
            &parameter_groups,
            &where_predicates,
            return_type,
            callable_scope,
            body.as_ref(),
        )?;
        Ok(Self {
            prefix,
            name,
            generic_parameters,
            parameter_groups,
            where_predicates,
            return_type,
            callable_scope,
            body,
        })
    }

    pub const fn prefix(&self) -> &HirItemPrefix {
        &self.prefix
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn parameter_groups(&self) -> &[HirMethodParameterGroup] {
        &self.parameter_groups
    }

    pub const fn where_predicates(&self) -> &[HirWherePredicate] {
        &self.where_predicates
    }

    pub const fn return_type(&self) -> Option<TypeId> {
        self.return_type
    }

    pub const fn callable_scope(&self) -> ScopeId {
        self.callable_scope
    }

    pub const fn body(&self) -> Option<&HirFunctionBody> {
        self.body.as_ref()
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        validate_method(
            expected,
            &self.prefix,
            &self.generic_parameters,
            &self.parameter_groups,
            &self.where_predicates,
            self.return_type,
            self.callable_scope,
            self.body.as_ref(),
        )
    }

    const fn has_structural_recovery(&self) -> bool {
        self.name.is_recovered()
    }
}

/// One Impl method signature or authored method.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirImplFunction {
    prefix: HirItemPrefix,
    name: HirRequiredName,
    generic_parameters: Box<[HirGenericParameter]>,
    parameter_groups: Box<[HirMethodParameterGroup]>,
    where_predicates: Box<[HirWherePredicate]>,
    return_type: Option<TypeId>,
    callable_scope: ScopeId,
    body: Option<HirFunctionBody>,
}

impl HirImplFunction {
    #[allow(
        clippy::too_many_arguments,
        reason = "the constructor validates the complete typed implementation-method declaration schema"
    )]
    pub(crate) fn try_new(
        expected: HirModuleId,
        prefix: HirItemPrefix,
        name: HirRequiredName,
        generic_parameters: Box<[HirGenericParameter]>,
        parameter_groups: Box<[HirMethodParameterGroup]>,
        where_predicates: Box<[HirWherePredicate]>,
        return_type: Option<TypeId>,
        callable_scope: ScopeId,
        body: Option<HirFunctionBody>,
    ) -> Result<Self, HirItemInvariantError> {
        validate_method(
            expected,
            &prefix,
            &generic_parameters,
            &parameter_groups,
            &where_predicates,
            return_type,
            callable_scope,
            body.as_ref(),
        )?;
        Ok(Self {
            prefix,
            name,
            generic_parameters,
            parameter_groups,
            where_predicates,
            return_type,
            callable_scope,
            body,
        })
    }

    pub const fn prefix(&self) -> &HirItemPrefix {
        &self.prefix
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn parameter_groups(&self) -> &[HirMethodParameterGroup] {
        &self.parameter_groups
    }

    pub const fn where_predicates(&self) -> &[HirWherePredicate] {
        &self.where_predicates
    }

    pub const fn return_type(&self) -> Option<TypeId> {
        self.return_type
    }

    pub const fn callable_scope(&self) -> ScopeId {
        self.callable_scope
    }

    pub const fn body(&self) -> Option<&HirFunctionBody> {
        self.body.as_ref()
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        validate_method(
            expected,
            &self.prefix,
            &self.generic_parameters,
            &self.parameter_groups,
            &self.where_predicates,
            self.return_type,
            self.callable_scope,
            self.body.as_ref(),
        )
    }

    const fn has_structural_recovery(&self) -> bool {
        self.name.is_recovered()
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the shared validator mirrors the complete method declaration schema without a compatibility carrier"
)]
fn validate_method(
    expected: HirModuleId,
    prefix: &HirItemPrefix,
    generic_parameters: &[HirGenericParameter],
    parameter_groups: &[HirMethodParameterGroup],
    where_predicates: &[HirWherePredicate],
    return_type: Option<TypeId>,
    callable_scope: ScopeId,
    body: Option<&HirFunctionBody>,
) -> Result<(), HirItemInvariantError> {
    if parameter_groups.is_empty() {
        return Err(HirItemInvariantError::EmptyMethodParameterGroups);
    }
    prefix.validate_module(expected)?;
    validate_generic_parameters(expected, generic_parameters)?;
    for group in parameter_groups {
        group.validate_module(expected)?;
    }
    validate_where_predicates(expected, where_predicates)?;
    validate_optional_type(expected, return_type)?;
    validate_scope(expected, callable_scope)?;
    if let Some(body) = body {
        validate_function_body(expected, body)?;
        if let HirFunctionBody::Block { scope, .. } = body
            && *scope != callable_scope
        {
            return Err(HirItemInvariantError::MethodBodyScopeMismatch {
                callable: callable_scope,
                body: *scope,
            });
        }
    }
    Ok(())
}

/// Closed Trait member family in source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirTraitMember {
    AssociatedType(HirTraitAssociatedType),
    Function(HirTraitFunction),
    Error,
}

impl HirTraitMember {
    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        match self {
            Self::AssociatedType(member) => member.validate_module(expected),
            Self::Function(member) => member.validate_module(expected),
            Self::Error => Ok(()),
        }
    }

    const fn has_structural_recovery(&self) -> bool {
        match self {
            Self::AssociatedType(member) => member.has_structural_recovery(),
            Self::Function(member) => member.has_structural_recovery(),
            Self::Error => true,
        }
    }
}

/// Closed Impl member family in source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirImplMember {
    AssociatedType(HirImplAssociatedType),
    Function(HirImplFunction),
    Error,
}

impl HirImplMember {
    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        match self {
            Self::AssociatedType(member) => member.validate_module(expected),
            Self::Function(member) => member.validate_module(expected),
            Self::Error => Ok(()),
        }
    }

    const fn has_structural_recovery(&self) -> bool {
        match self {
            Self::AssociatedType(member) => member.has_structural_recovery(),
            Self::Function(member) => member.has_structural_recovery(),
            Self::Error => true,
        }
    }
}

/// One final Trait declaration with inline members and no member arena.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirTraitItem {
    name: HirRequiredName,
    generic_parameters: Box<[HirGenericParameter]>,
    supertraits: Box<[TypeId]>,
    where_predicates: Box<[HirWherePredicate]>,
    members: Box<[HirTraitMember]>,
}

impl HirTraitItem {
    pub(crate) fn try_new(
        expected: HirModuleId,
        name: HirRequiredName,
        generic_parameters: Box<[HirGenericParameter]>,
        supertraits: Box<[TypeId]>,
        where_predicates: Box<[HirWherePredicate]>,
        members: Box<[HirTraitMember]>,
    ) -> Result<Self, HirItemInvariantError> {
        let value = Self {
            name,
            generic_parameters,
            supertraits,
            where_predicates,
            members,
        };
        value.validate_module(expected)?;
        Ok(value)
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn supertraits(&self) -> &[TypeId] {
        &self.supertraits
    }

    pub const fn where_predicates(&self) -> &[HirWherePredicate] {
        &self.where_predicates
    }

    pub const fn members(&self) -> &[HirTraitMember] {
        &self.members
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_generic_parameters(expected, &self.generic_parameters)?;
        validate_types(expected, &self.supertraits)?;
        validate_where_predicates(expected, &self.where_predicates)?;
        for member in &self.members {
            member.validate_module(expected)?;
        }
        Ok(())
    }

    pub(super) fn has_structural_recovery(&self) -> bool {
        self.name.is_recovered()
            || self
                .members
                .iter()
                .any(HirTraitMember::has_structural_recovery)
    }
}

/// One final Impl declaration with inline members and no member arena.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirImplItem {
    generic_parameters: Box<[HirGenericParameter]>,
    trait_ref: Option<TypeId>,
    target: TypeId,
    where_predicates: Box<[HirWherePredicate]>,
    members: Box<[HirImplMember]>,
}

impl HirImplItem {
    pub(crate) fn try_new(
        expected: HirModuleId,
        generic_parameters: Box<[HirGenericParameter]>,
        trait_ref: Option<TypeId>,
        target: TypeId,
        where_predicates: Box<[HirWherePredicate]>,
        members: Box<[HirImplMember]>,
    ) -> Result<Self, HirItemInvariantError> {
        let value = Self {
            generic_parameters,
            trait_ref,
            target,
            where_predicates,
            members,
        };
        value.validate_module(expected)?;
        Ok(value)
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn trait_ref(&self) -> Option<TypeId> {
        self.trait_ref
    }

    pub const fn target(&self) -> TypeId {
        self.target
    }

    pub const fn where_predicates(&self) -> &[HirWherePredicate] {
        &self.where_predicates
    }

    pub const fn members(&self) -> &[HirImplMember] {
        &self.members
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_generic_parameters(expected, &self.generic_parameters)?;
        validate_optional_type(expected, self.trait_ref)?;
        validate_type(expected, self.target)?;
        validate_where_predicates(expected, &self.where_predicates)?;
        for member in &self.members {
            member.validate_module(expected)?;
        }
        Ok(())
    }

    pub(super) fn has_structural_recovery(&self) -> bool {
        self.members
            .iter()
            .any(HirImplMember::has_structural_recovery)
    }
}
