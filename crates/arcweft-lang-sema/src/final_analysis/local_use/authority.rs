//! Shared, sealed ownership certificates for global and closed executable scopes.

use super::{
    CheckedLocalUseCatalog, CheckedLocalUseInstanceCatalog, CheckedLocalUseSite,
    CheckedLocalValueTransfer, CheckedSyntheticUse,
};
use arcweft_lang_hir::identity::{ExprId, LocalId};
use std::sync::Arc;
/// Selected local-use authority for one executable body. Closed instances
/// carry their own sealed rows; global bodies retain the accepted global seal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedLocalUseAuthority {
    Global(Arc<CheckedLocalUseCatalog>),
    Instance(Arc<CheckedLocalUseInstanceCatalog>),
}

impl CheckedLocalUseAuthority {
    pub fn place_access_at(
        &self,
        source: ExprId,
    ) -> Option<&crate::final_analysis::CheckedLocalPlaceAccess> {
        let site = CheckedLocalUseSite::Place(source);
        match self {
            Self::Global(catalog) => catalog.access_at(site),
            Self::Instance(catalog) => catalog.access_at(site),
        }?
        .place_access()
    }
    pub fn generation(&self) -> &Arc<arcweft_lang_hir::project::AcceptedHirProjectGeneration> {
        match self {
            Self::Global(catalog) => catalog.generation(),
            Self::Instance(catalog) => catalog.generation(),
        }
    }

    pub fn value_transfer_at(
        &self,
        site: CheckedLocalUseSite,
    ) -> Option<CheckedLocalValueTransfer> {
        match self {
            Self::Global(catalog) => catalog.value_transfer_at(site),
            Self::Instance(catalog) => catalog.value_transfer_at(site),
        }
    }

    pub fn is_pattern_guard(&self, guard: ExprId) -> bool {
        match self {
            Self::Global(catalog) => catalog.is_pattern_guard(guard),
            Self::Instance(catalog) => catalog.is_pattern_guard(guard),
        }
    }

    pub fn guard_copy_locals(&self, guard: ExprId) -> Vec<LocalId> {
        match self {
            Self::Global(catalog) => catalog.guard_copy_locals(guard).collect(),
            Self::Instance(catalog) => catalog.guard_copy_locals(guard).collect(),
        }
    }

    pub fn synthetic_at(&self, expression: ExprId) -> Option<CheckedSyntheticUse> {
        match self {
            Self::Global(catalog) => catalog.synthetic_at(expression),
            Self::Instance(catalog) => catalog.synthetic_at(expression),
        }
    }

    pub fn copy_requirement(
        &self,
        local: LocalId,
    ) -> Option<&crate::final_analysis::CheckedLocalCopyRequirement> {
        match self {
            Self::Global(catalog) => catalog.copy_requirement(local),
            Self::Instance(catalog) => catalog.copy_requirement(local),
        }
    }

    pub fn copy_requirements(&self) -> Vec<crate::final_analysis::CheckedLocalCopyRequirement> {
        match self {
            Self::Global(catalog) => catalog.copy_requirements().cloned().collect(),
            Self::Instance(catalog) => catalog.copy_requirements().cloned().collect(),
        }
    }

    pub fn synthetic_copy_requirement(
        &self,
        callable: crate::final_analysis::CheckedImplicitCallableIdentity,
    ) -> Option<crate::final_analysis::CheckedSyntheticCopyRequirement> {
        match self {
            Self::Global(catalog) => catalog.synthetic_copy_requirement(callable),
            Self::Instance(catalog) => catalog.synthetic_copy_requirement(callable),
        }
    }

    pub fn captures_at(&self, owner: ExprId) -> Vec<CheckedLocalValueTransfer> {
        match self {
            Self::Global(catalog) => catalog.captures_at(owner).collect(),
            Self::Instance(catalog) => catalog.captures_at(owner).collect(),
        }
    }

    pub fn instance_identity(
        &self,
    ) -> Option<&crate::final_analysis::CheckedLocalUseInstanceIdentity> {
        match self {
            Self::Global(_) => None,
            Self::Instance(catalog) => Some(catalog.identity()),
        }
    }
}
