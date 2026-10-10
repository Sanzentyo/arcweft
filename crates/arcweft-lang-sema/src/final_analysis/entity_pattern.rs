//! Entity pattern payloads are issued only from the accepted final pattern row.

use super::{CheckedLocalUseAuthority, CheckedPatternResolution, FinalSemanticAnalysis};
use crate::types::{SemanticTypeDigest, TypeKind};
use arcweft_lang_hir::{
    identity::PatternId,
    project::{AcceptedHirProjectGeneration, HirAnalysisProjectView},
};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedEntityPatternProjection {
    owner: PatternId,
    generation: Arc<AcceptedHirProjectGeneration>,
    resolution: CheckedPatternResolution,
    ty: TypeKind,
    type_identity: SemanticTypeDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CheckedEntityPatternProjectionError {
    #[error("pattern {owner:?} has no accepted entity payload")]
    MissingEntity { owner: PatternId },
    #[error("pattern {owner:?} disagrees with its accepted entity type")]
    TypeMismatch { owner: PatternId },
}

impl FinalSemanticAnalysis {
    pub fn entity_pattern_projection(
        &self,
        owner: PatternId,
    ) -> Result<CheckedEntityPatternProjection, CheckedEntityPatternProjectionError> {
        let row = self
            .pattern(owner)
            .ok_or(CheckedEntityPatternProjectionError::MissingEntity { owner })?;
        let expected = match row.resolution() {
            CheckedPatternResolution::Entity(item) => item.ty(),
            CheckedPatternResolution::ImportedProjectEntity(entity) => entity.ty(),
            _ => return Err(CheckedEntityPatternProjectionError::MissingEntity { owner }),
        };
        if row.ty() != &expected || !matches!(row.ty(), TypeKind::Ref(_)) {
            return Err(CheckedEntityPatternProjectionError::TypeMismatch { owner });
        }
        let type_identity = row
            .ty()
            .semantic_identity_digest()
            .map_err(|_| CheckedEntityPatternProjectionError::TypeMismatch { owner })?;
        Ok(CheckedEntityPatternProjection {
            owner,
            generation: Arc::clone(self.hir_generation()),
            resolution: row.resolution().clone(),
            ty: row.ty().clone(),
            type_identity,
        })
    }
}

impl CheckedEntityPatternProjection {
    pub const fn owner(&self) -> PatternId {
        self.owner
    }
    pub const fn resolution(&self) -> &CheckedPatternResolution {
        &self.resolution
    }
    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
    pub const fn type_identity(&self) -> SemanticTypeDigest {
        self.type_identity
    }
    pub fn validate_owner(&self, project: HirAnalysisProjectView<'_>, owner: PatternId) -> bool {
        self.owner == owner && self.generation.validate_analysis_lease(project).is_ok()
    }
    pub fn validate_authority(
        &self,
        authority: &CheckedLocalUseAuthority,
        owner: PatternId,
    ) -> bool {
        self.owner == owner && Arc::ptr_eq(&self.generation, authority.generation())
    }
    /// Projects the actual source/target entity accepted for this pattern.
    ///
    /// # Panics
    /// Panics only if the private issuer invariant is corrupted to contain a
    /// non-entity pattern resolution.
    pub fn runtime_reference(&self) -> arcweft_core::value::RuntimeEntityReference {
        match &self.resolution {
            CheckedPatternResolution::Entity(item) => {
                super::entity_value::project_item_runtime_reference(item)
            }
            CheckedPatternResolution::ImportedProjectEntity(entity) => {
                entity.target().runtime_reference().into()
            }
            _ => unreachable!("private issuer retains only final entity-pattern payloads"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::final_analysis::tests::{analyze, fixture};

    #[test]
    fn final_entity_pattern_witness_keeps_exact_local_use_generation_and_rejects_another_source_owner()
     {
        let source = "pub character alice {}\nfn root() { match @character.alice { @character.alice => (), _ => () } }\n";
        let accepted = fixture(source, None);
        let analysis = analyze(&accepted).expect("source entity pattern");
        let (owner, witness) = analysis
            .patterns()
            .find_map(|(owner, _)| {
                analysis
                    .entity_pattern_projection(owner)
                    .ok()
                    .map(|witness| (owner, witness))
            })
            .expect("actual pattern issuer");
        assert_eq!(witness.owner(), owner);
        assert!(witness.validate_owner(accepted.project.analysis_view().expect("HIR"), owner));
        assert!(witness.validate_authority(
            &CheckedLocalUseAuthority::Global(Arc::clone(analysis.checked_local_uses())),
            owner
        ));
        let other_owner = analysis
            .patterns()
            .find(|(other, _)| *other != owner)
            .expect("wildcard owner")
            .0;
        assert!(
            !witness.validate_owner(accepted.project.analysis_view().expect("HIR"), other_owner)
        );
        let foreign = fixture(source, None);
        let foreign_analysis = analyze(&foreign).expect("other accepted generation");
        assert!(!witness.validate_authority(
            &CheckedLocalUseAuthority::Global(Arc::clone(foreign_analysis.checked_local_uses())),
            owner
        ));
        assert_eq!(
            witness.type_identity(),
            witness
                .ty()
                .semantic_identity_digest()
                .expect("exact source type")
        );
    }
}
