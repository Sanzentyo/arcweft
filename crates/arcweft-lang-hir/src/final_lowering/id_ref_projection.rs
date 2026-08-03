//! Shared typed entity-reference projection into final HIR.

use arcweft_lang_syntax::id_ref::{
    AuthoredIdRef, AuthoredIdRoot, AuthoredIdSegment, SyntaxIdRefIssue, SyntaxIdRefSyntax,
};

use crate::leaf::{
    HirEntityReference, HirFamilyRelativeId, HirIdFamily, HirIdRef, HirIdRefInvariantError,
    HirIdRefIssue, HirIdRefRecovery, HirIdRefShape, HirIdRefValue, HirIdSuffix, HirRelativeId,
};
use crate::lower::{HirInvariantFailure, HirLowerFailure};

pub(crate) fn id_ref(value: &SyntaxIdRefSyntax) -> Result<HirIdRefValue, HirLowerFailure> {
    match value.value() {
        Ok(value) => resolved_id_ref(value).map(HirIdRefValue::Resolved),
        Err(issue) => Ok(HirIdRefValue::Recovered(HirIdRefRecovery::new(
            id_ref_shape(value),
            match issue {
                SyntaxIdRefIssue::MissingSuffix => HirIdRefIssue::Missing,
                SyntaxIdRefIssue::InvalidFamily(_) => {
                    HirIdRefIssue::Invalid(HirIdRefInvariantError::InvalidFamily)
                }
                SyntaxIdRefIssue::InvalidSegment { .. } => {
                    HirIdRefIssue::Invalid(HirIdRefInvariantError::InvalidSuffix)
                }
            },
        ))),
    }
}

fn resolved_id_ref(value: &AuthoredIdRef) -> Result<HirIdRef, HirLowerFailure> {
    let suffix = value
        .segments()
        .iter()
        .map(AuthoredIdSegment::as_str)
        .collect::<Vec<_>>()
        .join(".");
    match value.root() {
        AuthoredIdRoot::Absolute { .. } => HirEntityReference::try_new(suffix.into())
            .map(HirIdRef::absolute)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into()),
        AuthoredIdRoot::Relative { parent_depth } => HirIdSuffix::try_new(suffix.into())
            .map(|suffix| HirIdRef::relative(HirRelativeId::new(suffix, *parent_depth)))
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into()),
        AuthoredIdRoot::FamilyRelative {
            family,
            parent_depth,
        } => {
            let family = HirIdFamily::try_new(family.as_str().into())
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let suffix = HirIdSuffix::try_new(suffix.into())
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            Ok(HirIdRef::family_relative(HirFamilyRelativeId::new(
                family,
                HirRelativeId::new(suffix, *parent_depth),
            )))
        }
    }
}

fn id_ref_shape(value: &SyntaxIdRefSyntax) -> HirIdRefShape {
    let shape = value.shape();
    if shape.has_absolute_marker() {
        HirIdRefShape::Absolute {
            segment_count: shape.segment_count(),
        }
    } else if shape.has_family() {
        HirIdRefShape::FamilyRelative {
            parent_depth: shape.parent_depth(),
            suffix_segment_count: shape.segment_count(),
        }
    } else if shape.segment_count() == 0 && shape.parent_depth() == 0 {
        HirIdRefShape::Missing
    } else {
        HirIdRefShape::Relative {
            parent_depth: shape.parent_depth(),
            suffix_segment_count: shape.segment_count(),
        }
    }
}
