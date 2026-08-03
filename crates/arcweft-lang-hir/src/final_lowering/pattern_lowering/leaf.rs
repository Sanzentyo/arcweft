//! Typed Pattern leaf projection without source-string readers.

use arcweft_lang_syntax::patterns::{
    PatternPath, PatternPathIssue, PatternPathRoot, PatternPathSegment, PatternPathSyntax,
};

use crate::identity::HirLimit;
use crate::leaf::{HirPath, HirPathIssue, HirPathRoot};
use crate::lower::{HirInvariantFailure, HirLimitError, HirLowerFailure};
use crate::pattern::{HirPatternRecordPath, HirPatternRecordPathIssue};

use super::super::path_projection::{TypedPathProjection, TypedPathSegment, project_typed_path};
use super::super::require_limit;

pub(crate) fn record_path(
    value: &PatternPathSyntax,
) -> Result<HirPatternRecordPath, HirLowerFailure> {
    match value {
        PatternPathSyntax::Absent => Ok(HirPatternRecordPath::Absent),
        PatternPathSyntax::Resolved(path) => path_value(path).map(HirPatternRecordPath::Resolved),
        PatternPathSyntax::Recovered(recovery) => {
            preflight_path_segments(recovery.segments().iter().map(AsRef::as_ref))?;
            let segment_count = u32::try_from(recovery.segments().len())
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let issue = match recovery.issue() {
                PatternPathIssue::MissingSegment => HirPathIssue::Empty,
                PatternPathIssue::InvalidSegment { ordinal, .. } => {
                    HirPathIssue::InvalidSegment { ordinal: *ordinal }
                }
                PatternPathIssue::InvalidRootDepth => {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
            };
            Ok(HirPatternRecordPath::Recovered(
                HirPatternRecordPathIssue::new(issue, segment_count),
            ))
        }
    }
}

pub(crate) fn path_value(value: &PatternPath) -> Result<HirPath, HirLowerFailure> {
    let root = match value.root() {
        PatternPathRoot::ImplicitCrate => HirPathRoot::ImplicitCrate,
        PatternPathRoot::Crate => HirPathRoot::Crate,
        PatternPathRoot::SelfModule => HirPathRoot::SelfModule,
        PatternPathRoot::Super(depth) => HirPathRoot::Super { depth },
    };
    let segments = value
        .segments()
        .iter()
        .map(|segment| match segment {
            PatternPathSegment::Identifier(value) => TypedPathSegment::Identifier(value.as_str()),
            PatternPathSegment::ProjectSymbol(value) => {
                TypedPathSegment::ProjectSymbol(value.as_str())
            }
        })
        .collect::<Vec<_>>();
    match project_typed_path(root, &segments)? {
        TypedPathProjection::Resolved(path) => Ok(path),
        TypedPathProjection::Recovered(_) => Err(HirInvariantFailure::InvalidArenaCommit.into()),
    }
}

pub(super) fn preflight_path_segments<'a>(
    mut segments: impl ExactSizeIterator<Item = &'a str>,
) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::PathSegments, segments.len())?;
    let semantic_bytes = segments.try_fold(0usize, |total, segment| {
        require_limit(HirLimit::NameBytes, segment.len())?;
        total.checked_add(segment.len()).ok_or_else(|| {
            HirLowerFailure::from(HirLimitError::with_maximum(
                HirLimit::PathSemanticBytes,
                usize::MAX,
                HirLimit::PathSemanticBytes.maximum(),
            ))
        })
    })?;
    require_limit(HirLimit::PathSemanticBytes, semantic_bytes)
}
