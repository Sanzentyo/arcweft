//! Leaf-expression payload and path projection validation.

use arcweft_lang_syntax::attachment::source_file::{
    AttachedPath, AttachedPathRoot, AttachedPathSegmentKind,
};
use arcweft_lang_syntax::expressions::{
    SyntaxLifetimeRegistryPath, SyntaxLifetimeRegistryScope, SyntaxNumericSequence,
    SyntaxNumericSequenceRecovery,
};
use arcweft_lang_syntax::literal::{SyntaxLiteralIssue, SyntaxLiteralSyntax, SyntaxLiteralValue};
use arcweft_lang_syntax::name::SyntaxNameIssue;

use crate::final_lowering::literal_projection::{integer_issue, integer_literal, integer_suffix};
use crate::leaf::{
    HirCharacterLiteral, HirDurationLiteral, HirFloatLiteral, HirIdRef, HirIdRefShape,
    HirIdRefValue, HirIntegerLiteral, HirLifetimePathValue, HirLifetimeRegistryIssue,
    HirLifetimeRegistryScope, HirLiteral, HirNumericSequence, HirNumericSequenceRecovery, HirPath,
    HirPathIssue, HirPathRoot, HirPathSegment, HirPathValue, HirShortVariantName, HirStringLiteral,
    HirUnitNumberLiteral,
};

pub(super) fn numeric_sequence_matches(
    actual: &HirNumericSequence,
    expected: &SyntaxNumericSequence,
) -> bool {
    if actual.elements().len() != expected.elements().len()
        || actual.common_suffix() != expected.common_suffix().map(integer_suffix)
        || !numeric_recovery_matches(actual.recovery(), expected.recovery())
    {
        return false;
    }
    actual
        .elements()
        .iter()
        .zip(expected.elements())
        .all(|(actual, expected)| {
            let Ok(HirIntegerLiteral::Value {
                magnitude, radix, ..
            }) = integer_literal(expected.integer())
            else {
                return false;
            };
            actual.magnitude() == &magnitude && actual.radix() == radix
        })
}

fn numeric_recovery_matches(
    actual: &HirNumericSequenceRecovery,
    expected: &SyntaxNumericSequenceRecovery,
) -> bool {
    match (actual, expected) {
        (HirNumericSequenceRecovery::Complete, SyntaxNumericSequenceRecovery::Complete) => true,
        (
            HirNumericSequenceRecovery::MissingFinalElement { ordinal: actual },
            SyntaxNumericSequenceRecovery::MissingFinalElement { ordinal: expected },
        ) => actual == expected,
        (
            HirNumericSequenceRecovery::InvalidElement {
                ordinal: actual_ordinal,
                issue: actual_issue,
            },
            SyntaxNumericSequenceRecovery::InvalidElement {
                ordinal: expected_ordinal,
                issue: expected_issue,
                ..
            },
        ) => actual_ordinal == expected_ordinal && *actual_issue == integer_issue(expected_issue),
        (
            HirNumericSequenceRecovery::ConflictingSuffix {
                ordinal: actual_ordinal,
                first: actual_first,
                conflicting: actual_conflicting,
            },
            SyntaxNumericSequenceRecovery::ConflictingSuffix {
                ordinal: expected_ordinal,
                first: expected_first,
                conflicting: expected_conflicting,
            },
        ) => {
            actual_ordinal == expected_ordinal
                && *actual_first == integer_suffix(*expected_first)
                && *actual_conflicting == integer_suffix(*expected_conflicting)
        }
        _ => false,
    }
}

pub(super) fn literal_projection_matches(
    actual: &HirLiteral,
    expected: &SyntaxLiteralSyntax,
) -> bool {
    match (actual, expected.value()) {
        (HirLiteral::Boolean(actual), SyntaxLiteralValue::Bool(expected)) => actual == expected,
        (
            HirLiteral::String(HirStringLiteral::Value(actual)),
            SyntaxLiteralValue::String {
                value: expected, ..
            },
        ) => actual == expected,
        (
            HirLiteral::Character(HirCharacterLiteral::Value(actual)),
            SyntaxLiteralValue::Character(expected),
        ) => actual == expected,
        (HirLiteral::Integer(HirIntegerLiteral::Value { .. }), SyntaxLiteralValue::Integer(_))
        | (HirLiteral::Float(HirFloatLiteral::Value { .. }), SyntaxLiteralValue::Decimal(_))
        | (
            HirLiteral::UnitNumber(HirUnitNumberLiteral::Value { .. }),
            SyntaxLiteralValue::Unit { .. },
        )
        | (
            HirLiteral::Duration(HirDurationLiteral::Value(_)),
            SyntaxLiteralValue::Duration { .. },
        )
        | (
            HirLiteral::Duration(HirDurationLiteral::Invalid(
                crate::leaf::HirDurationIssue::FractionalNanosecond,
            )),
            SyntaxLiteralValue::Duration { .. },
        ) => true,
        (_, SyntaxLiteralValue::Invalid(issue)) => literal_issue_family_matches(actual, issue),
        _ => false,
    }
}

fn literal_issue_family_matches(actual: &HirLiteral, expected: &SyntaxLiteralIssue) -> bool {
    matches!(
        (actual, expected),
        (
            HirLiteral::String(HirStringLiteral::Invalid(_)),
            SyntaxLiteralIssue::String(_)
        ) | (
            HirLiteral::Character(HirCharacterLiteral::Invalid(_)),
            SyntaxLiteralIssue::Character(_)
        ) | (
            HirLiteral::Integer(HirIntegerLiteral::Invalid(_)),
            SyntaxLiteralIssue::Integer(_)
        ) | (
            HirLiteral::Float(HirFloatLiteral::Invalid(_)),
            SyntaxLiteralIssue::Decimal(_)
        ) | (
            HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(_)),
            SyntaxLiteralIssue::UnitNumber(_)
        ) | (
            HirLiteral::Duration(HirDurationLiteral::Invalid(_)),
            SyntaxLiteralIssue::Duration(_)
        )
    )
}

pub(super) fn id_ref_source_shape(reference: &HirIdRefValue) -> HirIdRefShape {
    match reference {
        HirIdRefValue::Resolved(HirIdRef::Absolute(reference)) => HirIdRefShape::Absolute {
            segment_count: u32::try_from(reference.segment_count())
                .expect("HIR entity segment count fits u32"),
        },
        HirIdRefValue::Resolved(HirIdRef::Relative(reference)) => HirIdRefShape::Relative {
            parent_depth: reference.parent_depth(),
            suffix_segment_count: u32::try_from(reference.suffix().segment_count())
                .expect("HIR entity segment count fits u32"),
        },
        HirIdRefValue::Resolved(HirIdRef::FamilyRelative(reference)) => {
            HirIdRefShape::FamilyRelative {
                parent_depth: reference.relative().parent_depth(),
                suffix_segment_count: u32::try_from(reference.relative().suffix().segment_count())
                    .expect("HIR entity segment count fits u32"),
            }
        }
        HirIdRefValue::Recovered(recovery) => recovery.shape(),
    }
}

pub(super) fn syntax_id_ref_source_shape(
    reference: &arcweft_lang_syntax::id_ref::SyntaxIdRefSyntax,
) -> HirIdRefShape {
    let shape = reference.shape();
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

pub(super) fn lifetime_projection_matches(
    actual: &HirLifetimePathValue,
    expected: &SyntaxLifetimeRegistryPath,
) -> bool {
    match actual {
        HirLifetimePathValue::Resolved(actual) => {
            !expected.has_recovery()
                && actual.optional() == expected.is_optional()
                && lifetime_scope_matches(actual.scope(), expected.scope())
                && actual.segments().len() == expected.segments().len()
                && actual
                    .segments()
                    .iter()
                    .zip(expected.segments())
                    .all(|(actual, expected)| {
                        expected
                            .as_ref()
                            .is_ok_and(|expected| actual.as_str() == expected.as_str())
                    })
        }
        HirLifetimePathValue::Recovered(actual) => {
            actual.scope_present() == lifetime_scope_present(expected.scope())
                && usize::try_from(actual.segment_count()).ok() == Some(expected.segments().len())
                && actual.optional_marker() == expected.is_optional()
                && expected_lifetime_issue(expected).is_some_and(|issue| issue == actual.issue())
        }
    }
}

fn lifetime_scope_matches(
    actual: &HirLifetimeRegistryScope,
    expected: &SyntaxLifetimeRegistryScope,
) -> bool {
    matches!(
        (actual, expected),
        (
            HirLifetimeRegistryScope::Frame,
            SyntaxLifetimeRegistryScope::Frame
        ) | (
            HirLifetimeRegistryScope::Tick,
            SyntaxLifetimeRegistryScope::Tick
        ) | (
            HirLifetimeRegistryScope::Cue,
            SyntaxLifetimeRegistryScope::Cue
        ) | (
            HirLifetimeRegistryScope::Line,
            SyntaxLifetimeRegistryScope::Line
        ) | (
            HirLifetimeRegistryScope::Scene,
            SyntaxLifetimeRegistryScope::Scene
        ) | (
            HirLifetimeRegistryScope::Flow,
            SyntaxLifetimeRegistryScope::Flow
        ) | (
            HirLifetimeRegistryScope::Session,
            SyntaxLifetimeRegistryScope::Session
        ) | (
            HirLifetimeRegistryScope::Global,
            SyntaxLifetimeRegistryScope::Global
        ) | (
            HirLifetimeRegistryScope::Persistent,
            SyntaxLifetimeRegistryScope::Persistent
        )
    ) || matches!(
        (actual, expected),
        (HirLifetimeRegistryScope::Named(actual), SyntaxLifetimeRegistryScope::Named(expected))
            if actual.as_str() == expected.as_str()
    )
}

fn lifetime_scope_present(scope: &SyntaxLifetimeRegistryScope) -> bool {
    !matches!(
        scope,
        SyntaxLifetimeRegistryScope::Recovered(SyntaxNameIssue::Missing)
    )
}

fn expected_lifetime_issue(path: &SyntaxLifetimeRegistryPath) -> Option<HirLifetimeRegistryIssue> {
    match path.scope() {
        SyntaxLifetimeRegistryScope::Recovered(SyntaxNameIssue::Missing) => {
            Some(HirLifetimeRegistryIssue::MissingScope)
        }
        SyntaxLifetimeRegistryScope::Recovered(_) => {
            Some(HirLifetimeRegistryIssue::InvalidNamedScope)
        }
        _ => path
            .segments()
            .iter()
            .position(Result::is_err)
            .map(|ordinal| HirLifetimeRegistryIssue::InvalidKeySegment {
                ordinal: u32::try_from(ordinal)
                    .expect("attached lifetime segment ordinal fits u32"),
            }),
    }
}

pub(in crate::source_index) fn path_projection_matches(
    actual: &HirPathValue,
    expected: &AttachedPath,
) -> bool {
    let root = attached_path_root(expected);
    let segment_count = expected
        .segments()
        .len()
        .saturating_add(usize::from(expected.missing_name().is_some()));
    let expected_issue = expected_path_issue(expected);
    match actual {
        HirPathValue::Resolved(actual) => {
            expected_issue.is_none()
                && actual.root() == root
                && actual.segments().len() == segment_count
                && actual
                    .segments()
                    .iter()
                    .zip(expected.segments())
                    .all(|(actual, expected)| path_segment_matches(actual, expected))
        }
        HirPathValue::Recovered(actual) => {
            actual.root() == root
                && usize::try_from(actual.segment_count()).ok() == Some(segment_count)
                && expected_issue
                    .as_ref()
                    .is_some_and(|issue| actual.issue() == issue)
        }
    }
}

pub(in crate::source_index) fn resolved_path_projection_matches(
    actual: &HirPath,
    expected: &AttachedPath,
) -> bool {
    expected_path_issue(expected).is_none()
        && actual.root() == attached_path_root(expected)
        && actual.segments().len() == expected.segments().len()
        && actual
            .segments()
            .iter()
            .zip(expected.segments())
            .all(|(actual, expected)| path_segment_matches(actual, expected))
}

/// Matches one grouped-import path projected from an attached module path and
/// one parser-classified terminal reference.
///
/// This mirrors the shared typed path projection without constructing a
/// second path or reopening source text. The attached segment family and its
/// parser-owned spelling remain the comparison authority.
pub(in crate::source_index) fn path_projection_with_terminal_matches(
    actual: &HirPathValue,
    base: &AttachedPath,
    terminal_kind: AttachedPathSegmentKind,
    terminal_spelling: &str,
) -> bool {
    let root = attached_path_root(base);
    let segment_count = base
        .segments()
        .len()
        .saturating_add(usize::from(base.missing_name().is_some()))
        .saturating_add(1);
    let expected_issue = base
        .segments()
        .iter()
        .position(|segment| segment.kind() == AttachedPathSegmentKind::Lifetime)
        .map(|ordinal| HirPathIssue::InvalidSegment {
            ordinal: u32::try_from(ordinal).expect("attached path ordinal fits u32"),
        })
        .or_else(|| {
            base.missing_name()
                .is_some()
                .then(|| HirPathIssue::InvalidSegment {
                    ordinal: u32::try_from(base.segments().len())
                        .expect("attached path ordinal fits u32"),
                })
        })
        .or_else(|| {
            (terminal_kind == AttachedPathSegmentKind::Lifetime).then(|| {
                HirPathIssue::InvalidSegment {
                    ordinal: u32::try_from(
                        base.segments().len() + usize::from(base.missing_name().is_some()),
                    )
                    .expect("attached path ordinal fits u32"),
                }
            })
        });

    match actual {
        HirPathValue::Resolved(actual) => {
            expected_issue.is_none()
                && actual.root() == root
                && actual.segments().len() == segment_count
                && actual
                    .segments()
                    .iter()
                    .take(base.segments().len())
                    .zip(base.segments())
                    .all(|(actual, expected)| path_segment_matches(actual, expected))
                && actual.segments().last().is_some_and(|actual| {
                    path_segment_family_and_spelling_matches(
                        actual,
                        terminal_kind,
                        terminal_spelling,
                    )
                })
        }
        HirPathValue::Recovered(actual) => {
            actual.root() == root
                && usize::try_from(actual.segment_count()).ok() == Some(segment_count)
                && expected_issue
                    .as_ref()
                    .is_some_and(|issue| actual.issue() == issue)
        }
    }
}

pub(in crate::source_index) fn attached_path_is_resolved(path: &AttachedPath) -> bool {
    expected_path_issue(path).is_none()
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

fn expected_path_issue(path: &AttachedPath) -> Option<HirPathIssue> {
    if let Some(ordinal) = path
        .segments()
        .iter()
        .position(|segment| segment.kind() == AttachedPathSegmentKind::Lifetime)
    {
        return Some(HirPathIssue::InvalidSegment {
            ordinal: u32::try_from(ordinal).expect("attached path ordinal fits u32"),
        });
    }
    if path.missing_name().is_some() {
        return Some(HirPathIssue::InvalidSegment {
            ordinal: u32::try_from(path.segments().len()).expect("attached path ordinal fits u32"),
        });
    }
    path.segments().is_empty().then_some(HirPathIssue::Empty)
}

fn path_segment_matches(
    actual: &HirPathSegment,
    expected: &arcweft_lang_syntax::attachment::source_file::AttachedPathSegment,
) -> bool {
    path_segment_family_and_spelling_matches(actual, expected.kind(), expected.source_text())
}

fn path_segment_family_and_spelling_matches(
    actual: &HirPathSegment,
    expected_kind: AttachedPathSegmentKind,
    expected_spelling: &str,
) -> bool {
    matches!(
        (actual, expected_kind),
        (
            HirPathSegment::Identifier(_),
            AttachedPathSegmentKind::Identifier
        ) | (
            HirPathSegment::ProjectSymbol(_),
            AttachedPathSegmentKind::Keyword | AttachedPathSegmentKind::ProjectSymbol
        )
    ) && match actual {
        HirPathSegment::Identifier(actual) => actual.as_str() == expected_spelling,
        HirPathSegment::ProjectSymbol(actual) => actual.as_str() == expected_spelling,
    }
}

pub(super) fn short_variant_projection_matches(
    actual: &HirShortVariantName,
    expected: &Result<arcweft_lang_syntax::name::SyntaxName, SyntaxNameIssue>,
) -> bool {
    match (actual, expected) {
        (HirShortVariantName::Resolved(actual), Ok(expected)) => {
            actual.as_str() == expected.as_str()
        }
        (HirShortVariantName::Recovered(_), Err(_)) => true,
        _ => false,
    }
}
