//! Checked field and variant shapes keyed by project nominal identity.

use std::collections::BTreeMap;

use arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationId;

use crate::{
    env::EnumVariantPayload,
    types::{GenericTypeOwnerId, GenericTypeParameterId, ProjectNominalType, TypeKind},
};

/// Checked body shape of one project struct or enum declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProjectNominalShape {
    Struct(BTreeMap<String, TypeKind>),
    Enum(BTreeMap<String, EnumVariantPayload>),
}

/// Shared identity-keyed inventory used by normal checking and tooling facts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProjectNominalShapeCatalog {
    declarations: BTreeMap<ProjectNominalDeclarationId, ProjectNominalShape>,
}

impl ProjectNominalShapeCatalog {
    pub(crate) fn insert_struct(
        &mut self,
        declaration: ProjectNominalDeclarationId,
        fields: impl IntoIterator<Item = (String, TypeKind)>,
    ) {
        let replaced = self.declarations.insert(
            declaration,
            ProjectNominalShape::Struct(fields.into_iter().collect()),
        );
        debug_assert!(replaced.is_none(), "project nominal IDs are unique");
    }

    pub(crate) fn insert_enum(
        &mut self,
        declaration: ProjectNominalDeclarationId,
        variants: impl IntoIterator<Item = (String, EnumVariantPayload)>,
    ) {
        let replaced = self.declarations.insert(
            declaration,
            ProjectNominalShape::Enum(variants.into_iter().collect()),
        );
        debug_assert!(replaced.is_none(), "project nominal IDs are unique");
    }

    pub(crate) fn field_type(&self, ty: &TypeKind, field: &str) -> Option<TypeKind> {
        let nominal = project_nominal(ty)?;
        let ProjectNominalShape::Struct(fields) = self.declarations.get(nominal.declaration())?
        else {
            return None;
        };
        let substitutions = substitutions(nominal);
        fields
            .get(field)
            .map(|ty| ty.substitute_type_parameters(&substitutions))
    }

    pub(crate) fn fields_for_type(&self, ty: &TypeKind) -> Option<BTreeMap<String, TypeKind>> {
        let nominal = project_nominal(ty)?;
        let ProjectNominalShape::Struct(fields) = self.declarations.get(nominal.declaration())?
        else {
            return None;
        };
        let substitutions = substitutions(nominal);
        Some(
            fields
                .iter()
                .map(|(name, ty)| (name.clone(), ty.substitute_type_parameters(&substitutions)))
                .collect(),
        )
    }

    pub(crate) fn variant_payload(
        &self,
        ty: &TypeKind,
        variant: &str,
    ) -> Option<EnumVariantPayload> {
        let nominal = project_nominal(ty)?;
        let ProjectNominalShape::Enum(variants) = self.declarations.get(nominal.declaration())?
        else {
            return None;
        };
        let substitutions = substitutions(nominal);
        variants
            .get(variant)
            .map(|payload| substitute_payload(payload, &substitutions))
    }

    pub(crate) fn struct_fields(
        &self,
        declaration: &ProjectNominalDeclarationId,
    ) -> Option<&BTreeMap<String, TypeKind>> {
        match self.declarations.get(declaration)? {
            ProjectNominalShape::Struct(fields) => Some(fields),
            ProjectNominalShape::Enum(_) => None,
        }
    }
}

fn project_nominal(ty: &TypeKind) -> Option<&ProjectNominalType> {
    match ty {
        TypeKind::ProjectNominal(nominal) => Some(nominal),
        TypeKind::BorrowRef { inner, .. } | TypeKind::Shared(inner) => project_nominal(inner),
        _ => None,
    }
}

fn substitutions(nominal: &ProjectNominalType) -> BTreeMap<GenericTypeParameterId, TypeKind> {
    nominal
        .arguments()
        .iter()
        .enumerate()
        .map(|(ordinal, argument)| {
            (
                GenericTypeParameterId::new(
                    GenericTypeOwnerId::Nominal(nominal.declaration().clone()),
                    u16::try_from(ordinal)
                        .expect("accepted project nominal arity is bounded by u16"),
                ),
                argument.clone(),
            )
        })
        .collect()
}

fn substitute_payload(
    payload: &EnumVariantPayload,
    substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
) -> EnumVariantPayload {
    match payload {
        EnumVariantPayload::Unit => EnumVariantPayload::Unit,
        EnumVariantPayload::Tuple(items) => EnumVariantPayload::Tuple(
            items
                .iter()
                .map(|item| item.substitute_type_parameters(substitutions))
                .collect(),
        ),
        EnumVariantPayload::Record(fields) => EnumVariantPayload::Record(
            fields
                .iter()
                .map(|(name, ty)| (name.clone(), ty.substitute_type_parameters(substitutions)))
                .collect(),
        ),
    }
}
