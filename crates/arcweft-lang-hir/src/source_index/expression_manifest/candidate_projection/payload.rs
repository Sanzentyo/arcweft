//! Candidate-only payload and source projection helpers.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::{
    AttachedCandidateDialogueOwner, AttachedCandidateNode, AttachedCandidatePathProjection,
    AttachedExpressionNode,
};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, SyntaxDialogueContent, SyntaxDialogueContentProjection,
    SyntaxDialogueNodeProjection, SyntaxExpressionSlot, SyntaxRecordField,
};
use arcweft_lang_syntax::incremental::ParsedSource;

use super::super::dialogue_projection::dialogue_node_projection_matches;
use super::CandidateChild;
use crate::arena::ArenaSnapshot;
use crate::dialogue_application::{HirDialogueCoordinate, HirDialogueNodeKind};
use crate::expr::{
    HirBinaryOp, HirCallChildPoison, HirExpr, HirExprKind, HirExpressionRecoveryIssue,
    HirRecordField, HirRecordFieldIssue, HirRecoveryIssue,
};
use crate::identity::ExprId;
use crate::leaf::{HirPathIssue, HirPathRoot, HirPathSegment, HirPathValue};
use crate::slot::SlotSnapshot;
use crate::source_index::{
    HirDialogueNodeSourcePart, HirDialoguePointActionSourcePart, HirExprSourceRole,
    HirInsertionPoint, HirSourceSite,
};

pub(super) fn candidate_role_map<K: Ord, V>(
    entries: impl IntoIterator<Item = (K, V)>,
) -> Option<BTreeMap<K, V>> {
    let mut roles = BTreeMap::new();
    for (role, value) in entries {
        if roles.insert(role, value).is_some() {
            return None;
        }
    }
    Some(roles)
}

pub(super) fn outer_root_site(
    parsed: &ParsedSource,
    attached: &AttachedExpressionNode,
) -> Option<HirSourceSite> {
    let content = attached.component(ExpressionComponentRole::Content)?;
    HirInsertionPoint::try_new(parsed.document(), content.range().start())
        .ok()
        .map(HirSourceSite::Insertion)
}

pub(super) fn candidate_node_root_site(
    parsed: &ParsedSource,
    node: AttachedCandidateNode<'_>,
) -> Option<HirSourceSite> {
    let content = node
        .expression_components()?
        .find(|component| component.role() == ExpressionComponentRole::Content)?;
    HirInsertionPoint::try_new(parsed.document(), content.source_span().range().start())
        .ok()
        .map(HirSourceSite::Insertion)
}

pub(super) fn dialogue_coordinates_match(
    actual: &[HirDialogueCoordinate],
    target: &HirExpr,
) -> bool {
    match target.kind() {
        HirExprKind::Call(call) => {
            HirDialogueCoordinate::from_immediate_arguments(call.arguments())
                .is_ok_and(|expected| expected.as_ref() == actual)
        }
        _ => actual.is_empty(),
    }
}

pub(super) fn dialogue_slot_role(
    owner: AttachedCandidateDialogueOwner,
    content: &SyntaxDialogueContent,
) -> Option<HirExprSourceRole> {
    let AttachedCandidateDialogueOwner::Node { ordinal } = owner;
    let node = content.nodes().get(ordinal as usize)?;
    match node {
        SyntaxDialogueNodeProjection::Interpolation(_) => Some(HirExprSourceRole::DialogueNode {
            ordinal,
            part: HirDialogueNodeSourcePart::Interpolation,
        }),
        SyntaxDialogueNodeProjection::ContentApplication(_) => {
            Some(HirExprSourceRole::DialogueNode {
                ordinal,
                part: HirDialogueNodeSourcePart::Expression,
            })
        }
        SyntaxDialogueNodeProjection::PointAction(action)
            if !matches!(
                action.payload(),
                arcweft_lang_syntax::expressions::SyntaxDialoguePointActionPayload::None
            ) =>
        {
            Some(HirExprSourceRole::DialoguePointAction {
                ordinal,
                part: HirDialoguePointActionSourcePart::Payload,
            })
        }
        _ => None,
    }
}

pub(super) fn dialogue_content_matches(
    actual: &crate::dialogue_application::HirDialogueContent,
    expected: &SyntaxDialogueContentProjection,
    node_values: &BTreeMap<u32, ExprId>,
    _action_values: &BTreeMap<u32, ExprId>,
) -> bool {
    match expected {
        SyntaxDialogueContentProjection::Missing { .. } => {
            actual.nodes().is_empty()
                && actual.raw_literal().is_none()
                && node_values.is_empty()
        }
        SyntaxDialogueContentProjection::RawLiteral(literal) => {
            actual
                .raw_literal()
                .is_some_and(|actual| actual.as_str() == literal.value())
                && actual.nodes().is_empty()
                && node_values.is_empty()
        }
        SyntaxDialogueContentProjection::Present(expected) => {
            actual.raw_literal().is_none()
                && actual.nodes().len() == expected.nodes().len()
                && actual
                    .nodes()
                    .iter()
                    .zip(expected.nodes())
                    .enumerate()
                    .all(|(ordinal, (actual, expected))| {
                        let Ok(ordinal) = u32::try_from(ordinal) else {
                            return false;
                        };
                        if actual.id().ordinal() != ordinal
                            || !dialogue_node_projection_matches(actual.kind(), expected)
                        {
                            return false;
                        }
                        match (actual.kind(), expected) {
                            (
                                HirDialogueNodeKind::Interpolation(value),
                                SyntaxDialogueNodeProjection::Interpolation(_),
                            )
                            | (
                                HirDialogueNodeKind::ContentApplication(value),
                                SyntaxDialogueNodeProjection::ContentApplication(_),
                            ) => node_values.get(&ordinal) == Some(value),
                            (
                                HirDialogueNodeKind::PointAction(action),
                                SyntaxDialogueNodeProjection::PointAction(source),
                            ) if !matches!(
                                source.payload(),
                                arcweft_lang_syntax::expressions::SyntaxDialoguePointActionPayload::None
                            ) => action.payload().expression().is_some_and(|value| {
                                node_values.get(&ordinal) == Some(&value)
                            }),
                            _ => !node_values.contains_key(&ordinal),
                        }
                    })
                && node_values.len()
                    == expected
                        .nodes()
                        .iter()
                        .filter(|node| match node {
                            SyntaxDialogueNodeProjection::Interpolation(_)
                            | SyntaxDialogueNodeProjection::ContentApplication(_) => true,
                            SyntaxDialogueNodeProjection::PointAction(action) => !matches!(
                                action.payload(),
                                arcweft_lang_syntax::expressions::SyntaxDialoguePointActionPayload::None
                            ),
                            _ => false,
                        })
                        .count()
        }
    }
}

pub(super) fn dialogue_intrinsic_recovery(
    source: &SyntaxDialogueContentProjection,
    actual: &crate::dialogue_application::HirAttachedContentApplication,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    recovery: &mut Option<HirRecoveryIssue>,
) -> Option<()> {
    for node in actual.content().nodes() {
        let HirDialogueNodeKind::PointAction(action) = node.kind() else {
            continue;
        };
        for argument in action.arguments() {
            if let Some(issue) = argument.issue() {
                recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(
                    crate::dialogue_application::HirRichTextIssue::Argument(issue),
                ));
            }
        }
        if let Some(expression) = action.payload().expression() {
            let expression = expressions.resolve_prepared(slots, expression).ok()?;
            if !matches!(expression.kind(), HirExprKind::Call(_)) {
                recovery.get_or_insert(HirRecoveryIssue::InvalidRichText(
                    crate::dialogue_application::HirRichTextIssue::InvalidPayload,
                ));
            }
        }
    }
    if let SyntaxDialogueContentProjection::Present(source) = source {
        for (actual, source) in actual.content().nodes().iter().zip(source.nodes()) {
            if matches!(source, SyntaxDialogueNodeProjection::Error(_))
                || matches!(actual.kind(), HirDialogueNodeKind::Error(_))
            {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::Generic(
                        crate::expr::HirGenericExprIssue::TransactionalChildFailure,
                    ),
                ));
            }
        }
    }
    Some(())
}

pub(super) fn child_ids(children: &[CandidateChild]) -> Vec<ExprId> {
    children.iter().map(|child| child.id).collect()
}

#[allow(
    clippy::match_same_arms,
    clippy::too_many_lines,
    reason = "one candidate record matrix keeps every field shape, shorthand case, source role, and recovery explicit"
)]
pub(super) fn candidate_record_fields_match(
    actual: &[HirRecordField],
    expected: &[SyntaxRecordField],
    children: &[CandidateChild],
    node: AttachedCandidateNode<'_>,
    local_resolver: &crate::module::HirLocalResolver<'_>,
    scope: crate::identity::ScopeId,
    recovery: &mut Option<HirRecoveryIssue>,
) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    let mut names = BTreeSet::new();
    let mut used_children = BTreeSet::new();
    for (field, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let Ok(field) = u32::try_from(field) else {
            return false;
        };
        let role = HirExprSourceRole::RecordField {
            field,
            part: crate::source_index::HirRecordFieldSourcePart::Value,
        };
        let child = children.iter().find(|child| child.role == role);
        if child.is_some_and(|child| !used_children.insert(child.id)) {
            return false;
        }
        let source_name = expected.name().as_ref().ok();
        let duplicate = source_name.is_some_and(|name| !names.insert(name.as_str()));
        let matched = match (actual, expected, duplicate, child) {
            (
                HirRecordField::Explicit {
                    name: actual_name,
                    value: actual_value,
                },
                SyntaxRecordField::Explicit {
                    name: Ok(expected_name),
                    value: SyntaxExpressionSlot::Authored,
                },
                false,
                Some(child),
            ) => {
                !child.missing
                    && actual_name.as_str() == expected_name.as_str()
                    && *actual_value == child.id
            }
            (
                HirRecordField::Shorthand {
                    name: actual_name,
                    local,
                },
                SyntaxRecordField::Shorthand {
                    name: Ok(expected_name),
                },
                false,
                None,
            ) => {
                let Some(use_start) = node.expression_components().and_then(|mut components| {
                    components
                        .find(|component| {
                            component.role()
                                == ExpressionComponentRole::RecordField {
                                    field,
                                    part: arcweft_lang_syntax::expressions::ExpressionRecordFieldPart::Name,
                                }
                        })
                        .map(|component| component.source_span().range().start())
                }) else {
                    return false;
                };
                actual_name.as_str() == expected_name.as_str()
                    && matches!(
                        local_resolver.lookup(scope, actual_name.as_str(), use_start),
                        Some(crate::scope::LocalLookup::Found(found)) if found == *local
                    )
            }
            (
                HirRecordField::Invalid {
                    issue: HirRecordFieldIssue::MissingName,
                },
                SyntaxRecordField::Explicit { name: Err(_), .. },
                false,
                Some(_),
            ) => {
                recovery.get_or_insert(HirRecoveryIssue::InvalidName(
                    crate::leaf::HirNameInvariantError::InvalidIdentifier,
                ));
                true
            }
            (
                HirRecordField::Invalid {
                    issue: HirRecordFieldIssue::MissingValue,
                },
                SyntaxRecordField::Explicit {
                    name: Ok(_),
                    value: SyntaxExpressionSlot::Missing,
                },
                false,
                Some(child),
            ) => child.missing,
            (
                HirRecordField::Invalid {
                    issue: HirRecordFieldIssue::DuplicateName,
                },
                SyntaxRecordField::Explicit { name: Ok(_), .. },
                true,
                Some(_),
            )
            | (
                HirRecordField::Invalid {
                    issue: HirRecordFieldIssue::DuplicateName,
                },
                SyntaxRecordField::Shorthand { name: Ok(_) },
                true,
                None,
            ) => {
                recovery.get_or_insert(HirRecoveryIssue::InvalidName(
                    crate::leaf::HirNameInvariantError::InvalidIdentifier,
                ));
                true
            }
            _ => false,
        };
        if !matched {
            return false;
        }
    }
    used_children.len() == children.len()
}

pub(super) fn candidate_path_matches(
    actual: &HirPathValue,
    expected: AttachedCandidatePathProjection<'_>,
) -> bool {
    let root = match expected.root() {
        arcweft_lang_syntax::attachment::source_file::AttachedPathRoot::ImplicitCrate => {
            HirPathRoot::ImplicitCrate
        }
        arcweft_lang_syntax::attachment::source_file::AttachedPathRoot::Crate { .. } => {
            HirPathRoot::Crate
        }
        arcweft_lang_syntax::attachment::source_file::AttachedPathRoot::SelfModule { .. } => {
            HirPathRoot::SelfModule
        }
        arcweft_lang_syntax::attachment::source_file::AttachedPathRoot::Super { levels } => {
            HirPathRoot::Super {
                depth: levels.len(),
            }
        }
    };
    let segments = expected.segments().collect::<Vec<_>>();
    let issue = if let Some(ordinal) = segments.iter().position(|segment| {
        segment.kind()
            == arcweft_lang_syntax::attachment::source_file::AttachedPathSegmentKind::Lifetime
    }) {
        Some(HirPathIssue::InvalidSegment {
            ordinal: match u32::try_from(ordinal) {
                Ok(ordinal) => ordinal,
                Err(_) => return false,
            },
        })
    } else if expected.missing_name().is_some() {
        Some(HirPathIssue::InvalidSegment {
            ordinal: match u32::try_from(segments.len()) {
                Ok(ordinal) => ordinal,
                Err(_) => return false,
            },
        })
    } else if segments.is_empty() {
        Some(HirPathIssue::Empty)
    } else {
        None
    };
    match actual {
        HirPathValue::Resolved(actual) => {
            issue.is_none()
                && actual.root() == root
                && actual.segments().len() == segments.len()
                && actual.segments().iter().zip(segments).all(|(actual, expected)| {
                    (matches!((actual, expected.kind()),
                        (HirPathSegment::Identifier(_), arcweft_lang_syntax::attachment::source_file::AttachedPathSegmentKind::Identifier)
                        | (HirPathSegment::ProjectSymbol(_), arcweft_lang_syntax::attachment::source_file::AttachedPathSegmentKind::Keyword | arcweft_lang_syntax::attachment::source_file::AttachedPathSegmentKind::ProjectSymbol))
                        || matches!((actual, expected.kind()),
                            (HirPathSegment::Identifier(_), arcweft_lang_syntax::attachment::source_file::AttachedPathSegmentKind::Keyword)
                        ) && expected.source_text() == "self")
                        && match actual {
                            HirPathSegment::Identifier(actual) => actual.as_str() == expected.source_text(),
                            HirPathSegment::ProjectSymbol(actual) => actual.as_str() == expected.source_text(),
                        }
                })
        }
        HirPathValue::Recovered(actual) => {
            actual.root() == root
                && usize::try_from(actual.segment_count()).ok()
                    == Some(segments.len() + usize::from(expected.missing_name().is_some()))
                && issue.as_ref() == Some(actual.issue())
        }
    }
}

pub(super) fn candidate_resolved_path_matches(
    actual: &crate::leaf::HirPath,
    expected: AttachedCandidatePathProjection<'_>,
) -> bool {
    candidate_path_matches(&HirPathValue::Resolved(actual.clone()), expected)
}

pub(super) const fn child_poison(poisoned: bool) -> HirCallChildPoison {
    if poisoned {
        HirCallChildPoison::Poisoned
    } else {
        HirCallChildPoison::Clean
    }
}

pub(super) const fn recovered_child(role: HirExprSourceRole) -> HirRecoveryIssue {
    HirRecoveryIssue::InvalidExpression(HirExpressionRecoveryIssue::RecoveredChild { role })
}

pub(super) const fn binary_operator_matches(
    actual: HirBinaryOp,
    expected: arcweft_lang_syntax::expressions::SyntaxBinaryOperator,
) -> bool {
    use arcweft_lang_syntax::expressions::SyntaxBinaryOperator as Syntax;
    matches!(
        (actual, expected),
        (HirBinaryOp::Implies, Syntax::Implies)
            | (HirBinaryOp::Or, Syntax::Or)
            | (HirBinaryOp::And, Syntax::And)
            | (HirBinaryOp::In, Syntax::In)
            | (HirBinaryOp::Equal, Syntax::Equal)
            | (HirBinaryOp::NotEqual, Syntax::NotEqual)
            | (HirBinaryOp::GreaterOrEqual, Syntax::GreaterOrEqual)
            | (HirBinaryOp::LessOrEqual, Syntax::LessOrEqual)
            | (HirBinaryOp::Greater, Syntax::Greater)
            | (HirBinaryOp::Less, Syntax::Less)
            | (HirBinaryOp::Merge, Syntax::Merge)
            | (HirBinaryOp::Add, Syntax::Add)
            | (HirBinaryOp::Subtract, Syntax::Subtract)
            | (HirBinaryOp::Multiply, Syntax::Multiply)
            | (HirBinaryOp::Divide, Syntax::Divide)
            | (HirBinaryOp::Remainder, Syntax::Remainder)
    )
}
