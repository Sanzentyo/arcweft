//! Nominal-type and typed-resource item payloads.

use arcweft_id::PublicId;

use super::callable::{HirGenericParameter, HirWherePredicate};
use super::{
    HirDocumentation, HirItemInvariantError, HirRequiredName, validate_expr,
    validate_generic_parameters, validate_optional_type, validate_type, validate_where_predicates,
};
use crate::identity::{ExprId, HirModuleId, TypeId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirEnumItem {
    name: HirRequiredName,
    generic_parameters: Box<[HirGenericParameter]>,
    where_predicates: Box<[HirWherePredicate]>,
    variants: Box<[HirEnumVariant]>,
}

impl HirEnumItem {
    pub(crate) const fn new(
        name: HirRequiredName,
        generic_parameters: Box<[HirGenericParameter]>,
        where_predicates: Box<[HirWherePredicate]>,
        variants: Box<[HirEnumVariant]>,
    ) -> Self {
        Self {
            name,
            generic_parameters,
            where_predicates,
            variants,
        }
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn where_predicates(&self) -> &[HirWherePredicate] {
        &self.where_predicates
    }

    pub const fn variants(&self) -> &[HirEnumVariant] {
        &self.variants
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_generic_parameters(expected, &self.generic_parameters)?;
        validate_where_predicates(expected, &self.where_predicates)?;
        for variant in &self.variants {
            validate_optional_type(expected, variant.payload)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirEnumVariant {
    documentation: Option<HirDocumentation>,
    name: HirRequiredName,
    payload: Option<TypeId>,
}

impl HirEnumVariant {
    pub(crate) const fn new(
        documentation: Option<HirDocumentation>,
        name: HirRequiredName,
        payload: Option<TypeId>,
    ) -> Self {
        Self {
            documentation,
            name,
            payload,
        }
    }

    pub const fn documentation(&self) -> Option<&HirDocumentation> {
        self.documentation.as_ref()
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn payload(&self) -> Option<TypeId> {
        self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStructItem {
    name: HirRequiredName,
    generic_parameters: Box<[HirGenericParameter]>,
    where_predicates: Box<[HirWherePredicate]>,
    fields: Box<[HirStructField]>,
}

impl HirStructItem {
    pub(crate) const fn new(
        name: HirRequiredName,
        generic_parameters: Box<[HirGenericParameter]>,
        where_predicates: Box<[HirWherePredicate]>,
        fields: Box<[HirStructField]>,
    ) -> Self {
        Self {
            name,
            generic_parameters,
            where_predicates,
            fields,
        }
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn where_predicates(&self) -> &[HirWherePredicate] {
        &self.where_predicates
    }

    pub const fn fields(&self) -> &[HirStructField] {
        &self.fields
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_generic_parameters(expected, &self.generic_parameters)?;
        validate_where_predicates(expected, &self.where_predicates)?;
        for field in &self.fields {
            validate_type(expected, field.ty)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStructField {
    documentation: Option<HirDocumentation>,
    name: HirRequiredName,
    ty: TypeId,
}

impl HirStructField {
    pub(crate) const fn new(
        documentation: Option<HirDocumentation>,
        name: HirRequiredName,
        ty: TypeId,
    ) -> Self {
        Self {
            documentation,
            name,
            ty,
        }
    }

    pub const fn documentation(&self) -> Option<&HirDocumentation> {
        self.documentation.as_ref()
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn ty(&self) -> TypeId {
        self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirTypeAliasItem {
    name: HirRequiredName,
    generic_parameters: Box<[HirGenericParameter]>,
    where_predicates: Box<[HirWherePredicate]>,
    target: TypeId,
}

impl HirTypeAliasItem {
    pub(crate) const fn new(
        name: HirRequiredName,
        generic_parameters: Box<[HirGenericParameter]>,
        where_predicates: Box<[HirWherePredicate]>,
        target: TypeId,
    ) -> Self {
        Self {
            name,
            generic_parameters,
            where_predicates,
            target,
        }
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn where_predicates(&self) -> &[HirWherePredicate] {
        &self.where_predicates
    }

    pub const fn target(&self) -> TypeId {
        self.target
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_generic_parameters(expected, &self.generic_parameters)?;
        validate_where_predicates(expected, &self.where_predicates)?;
        validate_type(expected, self.target)
    }
}

/// One final typed resource declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirResourceDeclaration {
    public_id: Option<PublicId>,
    name: HirRequiredName,
    resource_type: TypeId,
    fields: Box<[HirResourceField]>,
}

impl HirResourceDeclaration {
    pub(crate) const fn new(
        public_id: Option<PublicId>,
        name: HirRequiredName,
        resource_type: TypeId,
        fields: Box<[HirResourceField]>,
    ) -> Self {
        Self {
            public_id,
            name,
            resource_type,
            fields,
        }
    }

    pub const fn public_id(&self) -> Option<&PublicId> {
        self.public_id.as_ref()
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn resource_type(&self) -> TypeId {
        self.resource_type
    }

    pub const fn fields(&self) -> &[HirResourceField] {
        &self.fields
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_type(expected, self.resource_type)?;
        for field in &self.fields {
            validate_expr(expected, field.value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirResourceField {
    name: HirRequiredName,
    value: ExprId,
}

impl HirResourceField {
    pub(crate) const fn new(name: HirRequiredName, value: ExprId) -> Self {
        Self { name, value }
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn value(&self) -> ExprId {
        self.value
    }
}
