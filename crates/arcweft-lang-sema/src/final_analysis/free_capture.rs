//! One checked free-local traversal for executable callback and defer bodies.

use std::collections::BTreeSet;

use arcweft_lang_hir::identity::{ExprId, LocalId};

use crate::{semantic_coordinate::SemanticCoordinateIndex, types::TypeKind};

use super::{
    CheckedExecutableCapture, CheckedExpression, FinalSemanticAnalysisError,
    match_edges::CheckedStructuralEdgeDraft,
};

/// Collects the free locals of a selected expression body in source order.
/// The supplied lookups are generation-bound checked facts; this traversal
/// never reopens HIR children or infers captures from source spelling.
pub(super) fn collect_checked_free_locals(
    root: ExprId,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    structural_edges: &CheckedStructuralEdgeDraft,
    expression: impl Fn(ExprId) -> Option<CheckedCaptureExpression>,
    local_type: impl Fn(LocalId) -> Option<TypeKind>,
) -> Result<Box<[CheckedExecutableCapture]>, FinalSemanticAnalysisError> {
    let root_path = coordinates
        .expression_evidence(root)
        .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?
        .into_coordinate();
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    let mut captured = BTreeSet::new();
    let mut captures = Vec::new();
    while let Some(owner) = pending.pop() {
        if !visited.insert(owner) {
            continue;
        }
        let checked = expression(owner).ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if let CheckedCaptureExpression::Local { local, ty } = checked {
            let accepted = local_type(local)
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?;
            if ty.as_ref() != Some(&accepted) {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            let origin = coordinates
                .binding(local)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            if !origin.path().is_at_or_below(&root_path) && captured.insert(local) {
                captures.push(CheckedExecutableCapture::new(local, origin, accepted));
            }
        }
        let children = structural_edges
            .expression_children(owner)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        pending.extend(children.iter().rev().map(|(child, _)| *child));
    }
    Ok(captures.into_boxed_slice())
}

pub(super) enum CheckedCaptureExpression {
    NoLocal,
    Local {
        local: LocalId,
        ty: Option<TypeKind>,
    },
}

impl CheckedCaptureExpression {
    pub(super) fn from_checked(checked: &CheckedExpression) -> Self {
        match checked.execution_local_use() {
            Some(local) => Self::Local {
                local,
                ty: checked.source_value_type().cloned(),
            },
            None => Self::NoLocal,
        }
    }
}
