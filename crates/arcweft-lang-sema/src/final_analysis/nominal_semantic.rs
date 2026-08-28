//! Layout-free accepted semantics for project nominal declarations.
//!
//! This catalog is the permanent semantic owner for declaration order,
//! instantiated field/case types, and their accepted semantic identities.
//! Runtime layout projection is a later reachability concern and is
//! deliberately absent from every value in this module.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    identity::TypeId,
    symbol::{
        ProjectSymbolTable,
        nominal::{ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalVariant},
    },
};

use super::{
    CheckedProjectNominal, FinalSemanticAnalysisControl, FinalSemanticAnalysisError,
    nominal_schema::validate_checked_nominal,
};
use crate::{
    record_field::AcceptedRecordFieldSemanticId,
    types::{
        AcceptedVariantCaseSemanticId, SemanticTypeDigest, TypeKind, VariantPayloadOwnerFamily,
        VariantPayloadShape,
    },
};

const PROJECT_NOMINAL_SEMANTIC_DOMAIN: &[u8] = b"arcweft.lang.project-nominal-semantic.v1\0";

/// Typed digest of one complete layout-free project nominal definition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ProjectNominalSemanticDigest([u8; 32]);

impl ProjectNominalSemanticDigest {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Declaration-ordered semantic row for one project record field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectNominalSemanticField {
    declaration_ordinal: u32,
    semantic_id: AcceptedRecordFieldSemanticId,
    ty: TypeKind,
}

impl ProjectNominalSemanticField {
    pub(crate) const fn declaration_ordinal(&self) -> u32 {
        self.declaration_ordinal
    }

    pub(crate) const fn semantic_id(&self) -> AcceptedRecordFieldSemanticId {
        self.semantic_id
    }

    pub(crate) const fn ty(&self) -> &TypeKind {
        &self.ty
    }
}

/// Declaration-ordered semantic row for one project enum case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectNominalSemanticCase {
    ordinal: u32,
    semantic_id: AcceptedVariantCaseSemanticId,
    payload: VariantPayloadShape,
    diagnostic_name: arcweft_lang_syntax::ast::module_path::ModuleSegment,
}

impl ProjectNominalSemanticCase {
    pub(crate) const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub(crate) const fn semantic_id(&self) -> AcceptedVariantCaseSemanticId {
        self.semantic_id
    }

    pub(crate) const fn payload(&self) -> &VariantPayloadShape {
        &self.payload
    }

    pub(crate) const fn project_payload_field(&self) -> Option<&TypeKind> {
        self.payload.single_tuple_field()
    }

    pub(crate) fn diagnostic_name(&self) -> &str {
        self.diagnostic_name.as_str()
    }
}

/// Complete layout-free semantics of one instantiated project nominal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProjectNominalSemanticDefinition {
    Record {
        nominal: CheckedProjectNominal,
        fields: Box<[ProjectNominalSemanticField]>,
        digest: ProjectNominalSemanticDigest,
    },
    Variant {
        nominal: CheckedProjectNominal,
        cases: Box<[ProjectNominalSemanticCase]>,
        digest: ProjectNominalSemanticDigest,
    },
}

impl ProjectNominalSemanticDefinition {
    pub(crate) const fn nominal(&self) -> &CheckedProjectNominal {
        match self {
            Self::Record { nominal, .. } | Self::Variant { nominal, .. } => nominal,
        }
    }

    pub(crate) const fn semantic_type(&self) -> SemanticTypeDigest {
        self.nominal().identity()
    }

    pub(crate) const fn fields(&self) -> Option<&[ProjectNominalSemanticField]> {
        match self {
            Self::Record { fields, .. } => Some(fields),
            Self::Variant { .. } => None,
        }
    }

    pub(crate) const fn cases(&self) -> Option<&[ProjectNominalSemanticCase]> {
        match self {
            Self::Variant { cases, .. } => Some(cases),
            Self::Record { .. } => None,
        }
    }

    pub(crate) const fn digest(&self) -> ProjectNominalSemanticDigest {
        match self {
            Self::Record { digest, .. } | Self::Variant { digest, .. } => *digest,
        }
    }
}

/// Immutable generation-bound catalog keyed only by accepted semantic type.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProjectNominalSemanticCatalog {
    by_semantic_type: BTreeMap<SemanticTypeDigest, ProjectNominalSemanticDefinition>,
}

impl ProjectNominalSemanticCatalog {
    pub(super) fn build<'a>(
        nominals: impl ExactSizeIterator<Item = &'a CheckedProjectNominal>,
        symbols: &ProjectSymbolTable,
        types: &BTreeMap<TypeId, TypeKind>,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let mut by_semantic_type = BTreeMap::new();
        for nominal in nominals {
            control.check()?;
            validate_checked_nominal(symbols, nominal)?;
            let declaration = symbols
                .nominal(nominal.declaration())
                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
            let definition = build_semantic_definition(nominal, declaration, types, &control)?;
            match by_semantic_type.entry(nominal.identity()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(definition);
                }
                std::collections::btree_map::Entry::Occupied(entry)
                    if entry.get() == &definition => {}
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
                }
            }
        }
        Ok(Self { by_semantic_type })
    }

    pub(crate) fn get(
        &self,
        semantic_type: SemanticTypeDigest,
    ) -> Option<&ProjectNominalSemanticDefinition> {
        self.by_semantic_type
            .get(&semantic_type)
            .filter(|definition| definition.semantic_type() == semantic_type)
    }

    pub(super) fn validate_inventory<'a>(
        &self,
        nominals: impl ExactSizeIterator<Item = &'a CheckedProjectNominal>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        if nominals.len() != self.by_semantic_type.len() {
            return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
        }
        for nominal in nominals {
            let Some(definition) = self.get(nominal.identity()) else {
                return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
            };
            if definition.nominal() != nominal {
                return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
            }
        }
        Ok(())
    }
}

fn build_semantic_definition(
    nominal: &CheckedProjectNominal,
    declaration: &ProjectNominalDeclaration,
    types: &BTreeMap<TypeId, TypeKind>,
    control: &FinalSemanticAnalysisControl<'_>,
) -> Result<ProjectNominalSemanticDefinition, FinalSemanticAnalysisError> {
    match declaration.body() {
        ProjectNominalBody::Struct { fields } => {
            let mut accepted = Vec::new();
            accepted
                .try_reserve_exact(fields.len())
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
            for (ordinal, field) in fields.iter().enumerate() {
                control.check()?;
                let ordinal = u32::try_from(ordinal)
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                let declared = types
                    .get(&field.ty())
                    .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
                let ty = nominal
                    .instantiate_declaration_type(declaration, declared)
                    .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
                accepted.push(ProjectNominalSemanticField {
                    declaration_ordinal: ordinal,
                    semantic_id: AcceptedRecordFieldSemanticId::issue(
                        nominal.identity(),
                        ordinal,
                        ty.semantic_identity_digest(),
                    ),
                    ty,
                });
            }
            let digest = record_digest(nominal, &accepted)?;
            Ok(ProjectNominalSemanticDefinition::Record {
                nominal: nominal.clone(),
                fields: accepted.into_boxed_slice(),
                digest,
            })
        }
        ProjectNominalBody::Enum { variants } => {
            build_semantic_variant_definition(nominal, declaration, variants, types, control)
        }
        ProjectNominalBody::TypeAlias { .. } => {
            Err(FinalSemanticAnalysisError::InvalidNominalOwner)
        }
    }
}

fn build_semantic_variant_definition(
    nominal: &CheckedProjectNominal,
    declaration: &ProjectNominalDeclaration,
    variants: &[ProjectNominalVariant],
    types: &BTreeMap<TypeId, TypeKind>,
    control: &FinalSemanticAnalysisControl<'_>,
) -> Result<ProjectNominalSemanticDefinition, FinalSemanticAnalysisError> {
    let mut accepted = Vec::new();
    accepted
        .try_reserve_exact(variants.len())
        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
    for (ordinal, variant) in variants.iter().enumerate() {
        control.check()?;
        let ordinal =
            u32::try_from(ordinal).map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let payload = match variant.payload() {
            Some(owner) => {
                let declared = types
                    .get(&owner)
                    .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
                let payload = nominal
                    .instantiate_declaration_type(declaration, declared)
                    .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
                VariantPayloadShape::try_tuple(
                    VariantPayloadOwnerFamily::Project,
                    nominal.identity(),
                    ordinal,
                    [payload],
                )
                .map_err(|_| FinalSemanticAnalysisError::InvalidNominalOwner)?
            }
            None => VariantPayloadShape::Unit,
        };
        accepted.push(ProjectNominalSemanticCase {
            ordinal,
            semantic_id: AcceptedVariantCaseSemanticId::issue(
                VariantPayloadOwnerFamily::Project,
                nominal.identity(),
                ordinal,
                &payload,
            ),
            payload,
            diagnostic_name: variant.name().clone(),
        });
    }
    let digest = variant_digest(nominal, &accepted)?;
    Ok(ProjectNominalSemanticDefinition::Variant {
        nominal: nominal.clone(),
        cases: accepted.into_boxed_slice(),
        digest,
    })
}

fn record_digest(
    nominal: &CheckedProjectNominal,
    fields: &[ProjectNominalSemanticField],
) -> Result<ProjectNominalSemanticDigest, FinalSemanticAnalysisError> {
    let mut hasher = begin_digest(nominal, 0, fields.len())?;
    for field in fields {
        hasher.update(&field.declaration_ordinal.to_le_bytes());
        hasher.update(field.semantic_id.as_bytes());
        hasher.update(field.ty.semantic_identity_digest().as_bytes());
    }
    Ok(ProjectNominalSemanticDigest(hasher.finalize().into()))
}

fn variant_digest(
    nominal: &CheckedProjectNominal,
    cases: &[ProjectNominalSemanticCase],
) -> Result<ProjectNominalSemanticDigest, FinalSemanticAnalysisError> {
    let mut hasher = begin_digest(nominal, 1, cases.len())?;
    for case in cases {
        hasher.update(&case.ordinal.to_le_bytes());
        hasher.update(case.semantic_id.as_bytes());
        hasher.update(&[case.payload.semantic_shape_tag()]);
        let name = case.diagnostic_name.as_str().as_bytes();
        let name_len = u64::try_from(name.len())
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        hasher.update(&name_len.to_le_bytes());
        hasher.update(name);
    }
    Ok(ProjectNominalSemanticDigest(hasher.finalize().into()))
}

fn begin_digest(
    nominal: &CheckedProjectNominal,
    kind: u8,
    count: usize,
) -> Result<blake3::Hasher, FinalSemanticAnalysisError> {
    let count = u64::try_from(count).map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(PROJECT_NOMINAL_SEMANTIC_DOMAIN);
    hasher.update(nominal.identity().as_bytes());
    hasher.update(&[kind]);
    hasher.update(&count.to_le_bytes());
    Ok(hasher)
}
