//! Shared projection of parser-classified path segments into final HIR.

use arcweft_lang_syntax::ast::module_path::ModulePathRoot;
use arcweft_lang_syntax::ast::symbol_path::ProjectSymbolSegment;
use arcweft_lang_syntax::attachment::AttachedCandidatePathProjection;
use arcweft_lang_syntax::attachment::source_file::{
    AttachedPath, AttachedPathRoot, AttachedPathSegment, AttachedPathSegmentKind,
};
use arcweft_lang_syntax::types::TypePath;

use crate::identity::HirLimit;
use crate::leaf::{
    HirName, HirPath, HirPathIssue, HirPathRecovery, HirPathRoot, HirPathSegment,
    HirProjectSymbolSegment,
};
use crate::lowering::{HirInvariantFailure, HirLimitError, HirLowerFailure};

use super::require_limit;

/// Parser-selected semantic family and spelling of one ID-less path segment.
#[derive(Debug)]
pub(super) enum TypedPathSegment<'source> {
    Identifier(&'source str),
    ProjectSymbol(&'source str),
    Invalid(&'source str),
}

impl<'source> TypedPathSegment<'source> {
    pub(super) fn from_attached_kind(
        kind: AttachedPathSegmentKind,
        spelling: &'source str,
    ) -> Self {
        match kind {
            AttachedPathSegmentKind::Identifier => Self::Identifier(spelling),
            AttachedPathSegmentKind::Keyword | AttachedPathSegmentKind::ProjectSymbol => {
                Self::ProjectSymbol(spelling)
            }
            AttachedPathSegmentKind::Lifetime => Self::Invalid(spelling),
        }
    }

    const fn spelling(&self) -> &str {
        match self {
            Self::Identifier(spelling)
            | Self::ProjectSymbol(spelling)
            | Self::Invalid(spelling) => spelling,
        }
    }
}

impl<'source> From<&'source ProjectSymbolSegment> for TypedPathSegment<'source> {
    fn from(segment: &'source ProjectSymbolSegment) -> Self {
        let spelling = segment.as_str();
        if segment.try_as_module_segment().is_ok() {
            Self::Identifier(spelling)
        } else {
            Self::ProjectSymbol(spelling)
        }
    }
}

/// Final typed path or a source-known recoverable path issue.
#[derive(Debug)]
pub(super) enum TypedPathProjection {
    Resolved(HirPath),
    Recovered(HirPathRecovery),
}

/// Projects one already classified path without reclassifying its spelling.
pub(super) fn project_typed_path(
    root: HirPathRoot,
    segments: &[TypedPathSegment<'_>],
) -> Result<TypedPathProjection, HirLowerFailure> {
    require_limit(HirLimit::PathSegments, segments.len())?;
    let segment_count =
        u32::try_from(segments.len()).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
    let semantic_bytes = segments.iter().try_fold(0usize, |total, segment| {
        require_limit(HirLimit::NameBytes, segment.spelling().len())?;
        total
            .checked_add(segment.spelling().len())
            .ok_or_else(|| limit_overflow(HirLimit::PathSemanticBytes))
    })?;
    require_limit(HirLimit::PathSemanticBytes, semantic_bytes)?;

    if segments.is_empty() {
        return Ok(TypedPathProjection::Recovered(HirPathRecovery::new(
            root,
            segment_count,
            HirPathIssue::Empty,
        )));
    }

    let mut projected = Vec::with_capacity(segments.len());
    for (position, segment) in segments.iter().enumerate() {
        match segment {
            TypedPathSegment::Identifier(spelling) => projected.push(
                HirName::try_new(Box::from(*spelling))
                    .map(HirPathSegment::Identifier)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            ),
            TypedPathSegment::ProjectSymbol(spelling) => projected.push(
                HirProjectSymbolSegment::try_new(Box::from(*spelling))
                    .map(HirPathSegment::ProjectSymbol)
                    .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
            ),
            TypedPathSegment::Invalid(_) => {
                let ordinal =
                    u32::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                return Ok(TypedPathProjection::Recovered(HirPathRecovery::new(
                    root,
                    segment_count,
                    HirPathIssue::InvalidSegment { ordinal },
                )));
            }
        }
    }

    HirPath::try_new(root, projected.into_boxed_slice())
        .map(TypedPathProjection::Resolved)
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
}

/// Projects one grammar-validated type path through the shared path owner.
pub(super) fn project_type_path(path: &TypePath) -> Result<HirPath, HirLowerFailure> {
    let root = match path.root() {
        ModulePathRoot::ImplicitCrate => HirPathRoot::ImplicitCrate,
        ModulePathRoot::Crate => HirPathRoot::Crate,
        ModulePathRoot::SelfModule => HirPathRoot::SelfModule,
        ModulePathRoot::Super(depth) => HirPathRoot::Super { depth },
    };
    let segments = path
        .segments()
        .iter()
        .map(TypedPathSegment::from)
        .collect::<Vec<_>>();
    match project_typed_path(root, &segments)? {
        TypedPathProjection::Resolved(path) => Ok(path),
        TypedPathProjection::Recovered(_) => Err(HirInvariantFailure::InvalidArenaCommit.into()),
    }
}

/// Projects one parser-owned attached path without reopening its source text.
pub(super) fn project_attached_path(
    path: &AttachedPath,
) -> Result<TypedPathProjection, HirLowerFailure> {
    project_attached_path_with_tail(path, None)
}

/// Projects a value-expression path. In expression position, the parser-owned
/// `self` keyword denotes the extension receiver local; other keyword segments
/// retain their project-symbol family.
pub(super) fn project_expression_path(
    path: &AttachedPath,
) -> Result<TypedPathProjection, HirLowerFailure> {
    let root = attached_path_root(path);
    let mut segments = path
        .segments()
        .iter()
        .map(|segment| {
            if segment.kind() == AttachedPathSegmentKind::Keyword && segment.source_text() == "self"
            {
                TypedPathSegment::Identifier("self")
            } else {
                typed_attached_path_segment(segment)
            }
        })
        .collect::<Vec<_>>();
    if path.missing_name().is_some() {
        segments.push(TypedPathSegment::Invalid(""));
    }
    project_typed_path(root, &segments)
}

/// Projects one parser-validated ambiguous-candidate path from its bound
/// revision view. The candidate graph contributes typed segment kinds and
/// token spellings; this function never reopens or reparses source text.
pub(super) fn project_candidate_path(
    path: AttachedCandidatePathProjection<'_>,
) -> Result<TypedPathProjection, HirLowerFailure> {
    let root = match path.root() {
        AttachedPathRoot::ImplicitCrate => HirPathRoot::ImplicitCrate,
        AttachedPathRoot::Crate { .. } => HirPathRoot::Crate,
        AttachedPathRoot::SelfModule { .. } => HirPathRoot::SelfModule,
        AttachedPathRoot::Super { levels } => HirPathRoot::Super {
            depth: levels.len(),
        },
    };
    let mut segments = path
        .segments()
        .map(|segment| TypedPathSegment::from_attached_kind(segment.kind(), segment.source_text()))
        .collect::<Vec<_>>();
    if path.missing_name().is_some() {
        segments.push(TypedPathSegment::Invalid(""));
    }
    project_typed_path(root, &segments)
}

/// Projects an attached path with one grammar-classified terminal segment.
pub(super) fn project_attached_path_with_segment(
    path: &AttachedPath,
    segment: TypedPathSegment<'_>,
) -> Result<TypedPathProjection, HirLowerFailure> {
    project_attached_path_with_tail(path, Some(segment))
}

fn project_attached_path_with_tail(
    path: &AttachedPath,
    tail: Option<TypedPathSegment<'_>>,
) -> Result<TypedPathProjection, HirLowerFailure> {
    let root = attached_path_root(path);
    let mut segments = path
        .segments()
        .iter()
        .map(typed_attached_path_segment)
        .collect::<Vec<_>>();
    if path.missing_name().is_some() {
        segments.push(TypedPathSegment::Invalid(""));
    }
    segments.extend(tail);
    project_typed_path(root, &segments)
}

fn attached_path_root(path: &AttachedPath) -> HirPathRoot {
    match path.root() {
        AttachedPathRoot::ImplicitCrate => HirPathRoot::ImplicitCrate,
        AttachedPathRoot::Crate { .. } => HirPathRoot::Crate,
        AttachedPathRoot::SelfModule { .. } => HirPathRoot::SelfModule,
        AttachedPathRoot::Super { levels } => HirPathRoot::Super {
            depth: levels.len(),
        },
    }
}

pub(super) fn typed_attached_path_segment(segment: &AttachedPathSegment) -> TypedPathSegment<'_> {
    TypedPathSegment::from_attached_kind(segment.kind(), segment.source_text())
}

fn limit_overflow(limit: HirLimit) -> HirLowerFailure {
    HirLimitError::with_maximum(limit, usize::MAX, limit.maximum()).into()
}

#[cfg(test)]
mod tests {
    use crate::identity::HirLimit;
    use crate::leaf::{HirPathIssue, HirPathRoot, HirPathSegment};
    use crate::lowering::HirLowerFailure;

    use super::{TypedPathProjection, TypedPathSegment, project_typed_path};

    fn limit(error: &HirLowerFailure) -> HirLimit {
        let HirLowerFailure::Limit(error) = error else {
            panic!("expected HIR limit failure")
        };
        error.limit()
    }

    #[test]
    fn typed_segment_family_is_authoritative_and_invalid_segments_recover() {
        let projected = project_typed_path(
            HirPathRoot::Crate,
            &[
                TypedPathSegment::Identifier("ordinary"),
                TypedPathSegment::ProjectSymbol("self"),
            ],
        )
        .unwrap();
        let TypedPathProjection::Resolved(path) = projected else {
            panic!("typed path")
        };
        assert!(matches!(
            &path.segments()[0],
            HirPathSegment::Identifier(name) if name.as_str() == "ordinary"
        ));
        assert!(matches!(
            &path.segments()[1],
            HirPathSegment::ProjectSymbol(symbol) if symbol.as_str() == "self"
        ));

        let recovered = project_typed_path(
            HirPathRoot::ImplicitCrate,
            &[
                TypedPathSegment::Identifier("valid"),
                TypedPathSegment::Invalid("looks_like_an_identifier"),
            ],
        )
        .unwrap();
        let TypedPathProjection::Recovered(recovery) = recovered else {
            panic!("typed recovery")
        };
        assert_eq!(recovery.root(), HirPathRoot::ImplicitCrate);
        assert_eq!(recovery.segment_count(), 2);
        assert_eq!(
            recovery.issue(),
            &HirPathIssue::InvalidSegment { ordinal: 1 }
        );
    }

    #[test]
    fn path_segment_and_name_byte_limits_accept_exact_and_reject_one_over() {
        let exact_segments = (0..HirLimit::PathSegments.maximum())
            .map(|_| TypedPathSegment::Identifier("a"))
            .collect::<Vec<_>>();
        assert!(matches!(
            project_typed_path(HirPathRoot::ImplicitCrate, &exact_segments).unwrap(),
            TypedPathProjection::Resolved(_)
        ));
        let one_over_segments = (0..=HirLimit::PathSegments.maximum())
            .map(|_| TypedPathSegment::Identifier("a"))
            .collect::<Vec<_>>();
        assert_eq!(
            limit(&project_typed_path(HirPathRoot::ImplicitCrate, &one_over_segments).unwrap_err(),),
            HirLimit::PathSegments
        );

        let exact_name = "a".repeat(HirLimit::NameBytes.maximum());
        assert!(matches!(
            project_typed_path(
                HirPathRoot::ImplicitCrate,
                &[TypedPathSegment::Identifier(&exact_name)]
            )
            .unwrap(),
            TypedPathProjection::Resolved(_)
        ));
        let one_over_name = "a".repeat(HirLimit::NameBytes.maximum() + 1);
        assert_eq!(
            limit(
                &project_typed_path(
                    HirPathRoot::ImplicitCrate,
                    &[TypedPathSegment::Identifier(&one_over_name)]
                )
                .unwrap_err()
            ),
            HirLimit::NameBytes
        );
    }

    #[test]
    fn path_semantic_byte_limit_accepts_exact_and_rejects_one_over() {
        let component = "a".repeat(HirLimit::NameBytes.maximum());
        let exact = (0..HirLimit::PathSemanticBytes.maximum() / component.len())
            .map(|_| TypedPathSegment::Identifier(component.as_str()))
            .collect::<Vec<_>>();
        assert!(matches!(
            project_typed_path(HirPathRoot::ImplicitCrate, &exact).unwrap(),
            TypedPathProjection::Resolved(_)
        ));

        let mut one_over = exact;
        one_over.push(TypedPathSegment::Identifier("a"));
        assert_eq!(
            limit(&project_typed_path(HirPathRoot::ImplicitCrate, &one_over).unwrap_err()),
            HirLimit::PathSemanticBytes
        );
    }
}
