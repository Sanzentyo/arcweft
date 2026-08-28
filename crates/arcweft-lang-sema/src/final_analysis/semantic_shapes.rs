//! Accepted non-project structural semantics retained by final analysis.
//!
//! This catalog reuses the exact checked variant owner and accepted
//! environment-record authorities. It contains no Match-specific case/field
//! DTO and no source-name lookup path.

use std::collections::BTreeMap;

use crate::{
    env::{RegisteredTypeCheckEnv, nominal::AcceptedEnvironmentRecordSemantics},
    types::{SemanticTypeDigest, TypeKind},
};

use super::{CheckedVariantOwner, FinalSemanticAnalysisError};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct AcceptedSemanticShapeCatalog {
    closed_variants: BTreeMap<SemanticTypeDigest, CheckedVariantOwner>,
    environment_records: BTreeMap<SemanticTypeDigest, AcceptedEnvironmentRecordSemantics>,
}

impl AcceptedSemanticShapeCatalog {
    pub(crate) fn build(
        environment: &RegisteredTypeCheckEnv,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let mut catalog = Self::default();
        for (ty, schema) in environment.typecheck_env().closed_enums() {
            let canonical_ty = environment
                .typecheck_env()
                .canonical_accepted_type(ty.clone());
            let owner = CheckedVariantOwner::try_environment(schema, &canonical_ty)
                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
            catalog.insert_variant(&canonical_ty, owner)?;
        }
        for (nominal, variants) in environment.character_enum_variant_sets() {
            let ty = TypeKind::CharacterNominal(nominal.clone());
            let owner = CheckedVariantOwner::try_character_nominal(
                nominal.clone(),
                variants.iter().cloned(),
            )
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            catalog.insert_variant(&ty, owner)?;
        }
        for accepted in environment.nominal_catalog().exact_records() {
            let Some(record) = accepted.environment_record() else {
                continue;
            };
            let semantic_type = record.semantic_type();
            if record.ty().semantic_identity_digest() != semantic_type
                || catalog.closed_variants.contains_key(&semantic_type)
            {
                return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
            }
            match catalog.environment_records.entry(semantic_type) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(record.clone());
                }
                std::collections::btree_map::Entry::Occupied(entry) if entry.get() == record => {}
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
                }
            }
        }
        Ok(catalog)
    }

    fn insert_variant(
        &mut self,
        ty: &TypeKind,
        owner: CheckedVariantOwner,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let semantic_type = ty.semantic_identity_digest();
        if owner.semantic_type() != semantic_type
            || self.environment_records.contains_key(&semantic_type)
        {
            return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
        }
        match self.closed_variants.entry(semantic_type) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(owner);
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(entry)
                if entry.get().has_same_diagnostic_schema(&owner) =>
            {
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(FinalSemanticAnalysisError::InvalidNominalOwner)
            }
        }
    }

    pub(crate) fn closed_variant(
        &self,
        semantic_type: SemanticTypeDigest,
    ) -> Option<&CheckedVariantOwner> {
        self.closed_variants
            .get(&semantic_type)
            .filter(|owner| owner.semantic_type() == semantic_type && owner.has_valid_case_rows())
    }

    pub(crate) fn environment_record(
        &self,
        semantic_type: SemanticTypeDigest,
    ) -> Option<&AcceptedEnvironmentRecordSemantics> {
        self.environment_records
            .get(&semantic_type)
            .filter(|record| {
                record.semantic_type() == semantic_type
                    && record.ty().semantic_identity_digest() == semantic_type
            })
    }
}
