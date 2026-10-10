//! One accepted entity expression, its selected checked payload and exact type.
//! Runtime consumers cannot assemble this witness from an ID or type digest.

use super::{
    CheckedExpressionResolution, CheckedValueResolution, ExprId, FinalSemanticAnalysis,
    SemanticTypeDigest, TypeKind,
};
use crate::semantic_coordinate::CheckedExpressionOrigin;
use arcweft_core::{plan::FlowRuntimeId, value::RuntimeEntityReference};
use arcweft_id::DeclarationIdentityFamily;

/// The existing checked resolution is retained as the sole payload authority.
/// The private issuer binds it to the accepted source owner and canonical type;
/// an expression-origin coordinate alone cannot authorize a different value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedEntityValueProjection {
    origin: CheckedExpressionOrigin,
    resolution: CheckedValueResolution,
    ty: TypeKind,
    type_identity: SemanticTypeDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CheckedEntityValueProjectionError {
    #[error("expression {owner:?} has no accepted semantic value")]
    MissingExpression { owner: ExprId },
    #[error("expression {owner:?} is not an admitted entity value")]
    NotEntity { owner: ExprId },
    #[error("expression {owner:?} has a type incompatible with its admitted entity payload")]
    TypeMismatch { owner: ExprId },
    #[error("expression {owner:?} has no accepted source-coordinate authority")]
    MissingOrigin { owner: ExprId },
}

impl FinalSemanticAnalysis {
    pub fn entity_value_projection(
        &self,
        owner: ExprId,
    ) -> Result<CheckedEntityValueProjection, CheckedEntityValueProjectionError> {
        let expression = self
            .expression(owner)
            .ok_or(CheckedEntityValueProjectionError::MissingExpression { owner })?;
        let CheckedExpressionResolution::Value(resolution) = expression.resolution() else {
            return Err(CheckedEntityValueProjectionError::NotEntity { owner });
        };
        let ty = expression
            .source_value_type()
            .ok_or(CheckedEntityValueProjectionError::NotEntity { owner })?;
        let expected = match resolution {
            CheckedValueResolution::CatalogAsset(_) => {
                TypeKind::entity_ref(crate::types::EntityKind::Asset)
            }
            CheckedValueResolution::ProjectItem(item) => item.ty(),
            CheckedValueResolution::ImportedProjectEntity(entity) => entity.ty(),
            _ => return Err(CheckedEntityValueProjectionError::NotEntity { owner }),
        };
        if ty != &expected || !matches!(ty, TypeKind::Ref(_)) {
            return Err(CheckedEntityValueProjectionError::TypeMismatch { owner });
        }
        let type_identity = ty
            .semantic_identity_digest()
            .map_err(|_| CheckedEntityValueProjectionError::TypeMismatch { owner })?;
        let origin = self
            .expression_origin(owner)
            .map_err(|_| CheckedEntityValueProjectionError::MissingOrigin { owner })?;
        Ok(CheckedEntityValueProjection {
            origin,
            resolution: resolution.clone(),
            ty: ty.clone(),
            type_identity,
        })
    }
}

impl CheckedEntityValueProjection {
    pub const fn origin(&self) -> &CheckedExpressionOrigin {
        &self.origin
    }
    pub const fn resolution(&self) -> &CheckedValueResolution {
        &self.resolution
    }
    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
    pub const fn type_identity(&self) -> SemanticTypeDigest {
        self.type_identity
    }

    /// Projects the same admitted payload into the existing Core value domain.
    /// It does not select declarations or resolve source labels at runtime.
    ///
    /// # Panics
    /// Panics only if a private witness is corrupted to hold a non-entity resolution.
    pub fn runtime_reference(&self) -> RuntimeEntityReference {
        match &self.resolution {
            CheckedValueResolution::CatalogAsset(asset) => RuntimeEntityReference::Project {
                family: DeclarationIdentityFamily::Asset,
                public_id: asset.as_public_id().clone(),
            },
            CheckedValueResolution::ProjectItem(item) => project_item_runtime_reference(item),
            CheckedValueResolution::ImportedProjectEntity(entity) => {
                entity.target().runtime_reference().into()
            }
            _ => unreachable!("the private issuer admits only checked entity resolutions"),
        }
    }

    /// Keeps static source Goto on its exact checked Flow declaration.
    pub fn flow_runtime_identity(
        &self,
    ) -> Result<Option<FlowRuntimeId>, arcweft_core::runtime_id::RuntimeIdError> {
        if let CheckedValueResolution::ImportedProjectEntity(entity) = &self.resolution {
            return Ok(entity.target().runtime_reference().flow().cloned());
        }
        let CheckedValueResolution::ProjectItem(item) = &self.resolution else {
            return Ok(None);
        };
        let Some((arcweft_lang_hir::symbol::CallableDeclarationKey::Flow(flow), _)) =
            item.flow_owner()
        else {
            return Ok(None);
        };
        FlowRuntimeId::from_checked_declaration_digest(
            flow.semantic_digest().into_bytes(),
            flow.public_id().as_str(),
        )
        .map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::final_analysis::tests::{analyze, fixture};

    #[test]
    fn entity_projection_issuer_retains_accepted_payload_type_and_generation_and_refuses_scalar_values()
     {
        let fixture = fixture(
            "fn selected() -> Ref<Asset> { @asset:.bg.pulse }\nfn scalar() -> i64 { 1i64 }\n",
            None,
        );
        let analysis = analyze(&fixture).expect("source analysis");
        let (owner, expression) = analysis
            .expressions()
            .find(|(_, row)| {
                matches!(
                    row.resolution(),
                    CheckedExpressionResolution::Value(CheckedValueResolution::CatalogAsset(_))
                )
            })
            .expect("source Asset");
        let projection = analysis
            .entity_value_projection(owner)
            .expect("sealed projection");
        assert_eq!(projection.origin().expression(), owner);
        assert!(
            projection
                .origin()
                .validate_owner(fixture.project.analysis_view().expect("HIR"), owner)
        );
        assert!(std::sync::Arc::ptr_eq(
            projection.origin().generation(),
            analysis.hir_generation()
        ));
        assert_eq!(
            projection.ty(),
            expression.source_value_type().expect("checked Ref type")
        );
        assert_eq!(
            projection.type_identity(),
            projection
                .ty()
                .semantic_identity_digest()
                .expect("canonical type")
        );
        assert_eq!(
            projection.runtime_reference().runtime_label(),
            "asset.bg.pulse"
        );
        let scalar = analysis
            .expressions()
            .find(|(_, row)| matches!(row.value_type(), Some(TypeKind::I64)))
            .expect("scalar value")
            .0;
        assert!(
            matches!(analysis.entity_value_projection(scalar), Err(CheckedEntityValueProjectionError::NotEntity { owner }) if owner == scalar)
        );
    }
}

/// The checked declaration is the sole source of a structural Flow identity.
///
/// # Panics
/// Panics only if a private checked Flow declaration no longer projects its
/// admitted runtime identity.
pub(super) fn project_item_runtime_reference(
    item: &super::CheckedProjectItem,
) -> RuntimeEntityReference {
    match item.flow_owner() {
        Some((arcweft_lang_hir::symbol::CallableDeclarationKey::Flow(flow), _)) => {
            RuntimeEntityReference::StructuralFlow(
                FlowRuntimeId::from_checked_declaration_digest(
                    flow.semantic_digest().into_bytes(),
                    flow.public_id().as_str(),
                )
                .expect("accepted structural Flow retains its checked identity"),
            )
        }
        _ => RuntimeEntityReference::Project {
            family: item.family(),
            public_id: item.public_id().clone(),
        },
    }
}
