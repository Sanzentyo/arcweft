//! Shared typed entity-reference projection into final HIR.

use arcweft_id::{UnsafeAuditId, UnsafeAuditIdError};
use arcweft_lang_syntax::id_ref::{
    AuthoredIdRef, AuthoredIdRoot, AuthoredIdSegment, SyntaxIdRefIssue, SyntaxIdRefSyntax,
};

use crate::leaf::{
    HirEntityReference, HirFamilyRelativeId, HirIdFamily, HirIdRef, HirIdRefInvariantError,
    HirIdRefIssue, HirIdRefRecovery, HirIdRefShape, HirIdRefValue, HirIdSuffix, HirRelativeId,
};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::stmt::{HirUnsafeAuditIdentity, HirUnsafeAuditIdentityIssue};

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

/// Projects the one accepted local dialogue-mark identity from the shared
/// entity-reference authority. Mark selectors deliberately retain the typed
/// one-segment suffix rather than reparsing it as a HIR name or string.
pub(crate) fn dialogue_mark_suffix(
    value: &SyntaxIdRefSyntax,
) -> Result<HirIdSuffix, HirLowerFailure> {
    match id_ref(value)? {
        HirIdRefValue::Resolved(HirIdRef::Relative(relative))
            if relative.parent_depth() == 0 && relative.suffix().segment_count() == 1 =>
        {
            Ok(relative.suffix().clone())
        }
        HirIdRefValue::Resolved(_) | HirIdRefValue::Recovered(_) => {
            Err(HirInvariantFailure::InvalidArenaCommit.into())
        }
    }
}

/// Projects the one accepted absolute `@unsafe.*` identity family.
///
/// The general ID-reference shape is intentionally consumed inside this HIR
/// boundary so no successful raw reference reaches sema or the verifier.
pub(crate) fn unsafe_audit_identity(
    value: &SyntaxIdRefSyntax,
) -> Result<HirUnsafeAuditIdentity, HirLowerFailure> {
    let identity = match id_ref(value)? {
        HirIdRefValue::Resolved(HirIdRef::Absolute(reference)) => {
            match UnsafeAuditId::try_new(reference.as_str().to_owned()) {
                Ok(id) => HirUnsafeAuditIdentity::Accepted(id),
                Err(UnsafeAuditIdError::InvalidPublicId(_)) => {
                    HirUnsafeAuditIdentity::Recovered(HirUnsafeAuditIdentityIssue::InvalidReference)
                }
                Err(UnsafeAuditIdError::WrongFamily) => {
                    HirUnsafeAuditIdentity::Recovered(HirUnsafeAuditIdentityIssue::WrongFamily)
                }
            }
        }
        HirIdRefValue::Resolved(HirIdRef::Relative(_) | HirIdRef::FamilyRelative(_)) => {
            HirUnsafeAuditIdentity::Recovered(HirUnsafeAuditIdentityIssue::NonAbsolute)
        }
        HirIdRefValue::Recovered(_) => {
            HirUnsafeAuditIdentity::Recovered(HirUnsafeAuditIdentityIssue::InvalidReference)
        }
    };
    Ok(identity)
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
