//! Exact dialogue-content payload projection shared by source-backed and E34 expressions.

use arcweft_lang_syntax::expressions::{
    SyntaxAttachedContentApplicationProjection, SyntaxDialogueActionArgumentProjection,
    SyntaxDialogueContentIssue, SyntaxDialogueContentProjection, SyntaxDialogueNodeProjection,
    SyntaxDialoguePointActionIdentity, SyntaxDialoguePointActionPayload,
    SyntaxDialoguePointActionProjection, SyntaxLineBreakKind,
};

use crate::dialogue_application::{
    HirAttachedContentApplication, HirAttachedContentBodyPresence, HirDialogueContentError,
    HirDialogueNodeKind, HirDialoguePointActionIdentity,
};

pub(super) fn dialogue_application_projection_matches(
    actual: &HirAttachedContentApplication,
    expected: &SyntaxAttachedContentApplicationProjection,
) -> bool {
    let expected_body_presence = match expected.content() {
        SyntaxDialogueContentProjection::Missing { .. } => HirAttachedContentBodyPresence::Absent,
        SyntaxDialogueContentProjection::Present(_)
        | SyntaxDialogueContentProjection::RawLiteral(_) => HirAttachedContentBodyPresence::Present,
    };
    if actual.body_presence() != expected_body_presence {
        return false;
    }
    if match actual.family() {
        crate::dialogue_application::HirAttachedContentApplicationFamily::DialogueLine {
            plan,
            ..
        } => plan.is_some() != expected.has_plan(),
        crate::dialogue_application::HirAttachedContentApplicationFamily::ContentCall {
            ..
        } => expected.has_plan(),
    } {
        return false;
    }
    match expected.content() {
        SyntaxDialogueContentProjection::Missing { .. } => {
            actual.content().nodes().is_empty() && actual.content().raw_literal().is_none()
        }
        SyntaxDialogueContentProjection::RawLiteral(literal) => {
            actual
                .content()
                .raw_literal()
                .is_some_and(|actual| actual.as_str() == literal.value())
                && actual.content().nodes().is_empty()
        }
        SyntaxDialogueContentProjection::Present(expected) => {
            actual.content().raw_literal().is_none()
                && actual.content().nodes().len() == expected.nodes().len()
                && actual.content().nodes().iter().zip(expected.nodes()).all(
                    |(actual, expected)| dialogue_node_projection_matches(actual.kind(), expected),
                )
        }
    }
}

pub(super) fn dialogue_node_projection_matches(
    actual: &HirDialogueNodeKind,
    expected: &SyntaxDialogueNodeProjection,
) -> bool {
    match (actual, expected) {
        (HirDialogueNodeKind::Text(actual), SyntaxDialogueNodeProjection::Text(expected)) => {
            actual.as_str() == expected.as_ref()
        }
        (HirDialogueNodeKind::Escape(actual), SyntaxDialogueNodeProjection::Escape(expected)) => {
            actual == expected
        }
        (HirDialogueNodeKind::Interpolation(_), SyntaxDialogueNodeProjection::Interpolation(_))
        | (
            HirDialogueNodeKind::ContentApplication(_),
            SyntaxDialogueNodeProjection::ContentApplication(_),
        ) => true,
        (
            HirDialogueNodeKind::PointAction(actual),
            SyntaxDialogueNodeProjection::PointAction(expected),
        ) => point_action_projection_matches(actual, expected),
        (
            HirDialogueNodeKind::LineBreak(actual),
            SyntaxDialogueNodeProjection::LineBreak(expected),
        ) => line_break_projection_matches_new(*actual, *expected),
        (HirDialogueNodeKind::Error(actual), SyntaxDialogueNodeProjection::Error(expected)) => {
            dialogue_content_issue_projection_matches_new(actual, expected)
        }
        // Retained Ruby sugar is lowered through the same canonical
        // content-call path as `#ruby("reading")[body]`.
        (HirDialogueNodeKind::ContentApplication(_), SyntaxDialogueNodeProjection::Ruby { .. }) => {
            true
        }
        _ => false,
    }
}

fn point_action_projection_matches(
    actual: &crate::dialogue_application::HirDialoguePointAction,
    expected: &SyntaxDialoguePointActionProjection,
) -> bool {
    let identity_matches = match (actual.identity(), expected.identity()) {
        (
            HirDialoguePointActionIdentity::Control(actual),
            SyntaxDialoguePointActionIdentity::Control(expected),
        ) => *actual == (*expected).into(),
        (
            HirDialoguePointActionIdentity::Host(actual),
            SyntaxDialoguePointActionIdentity::Host(expected),
        ) => *actual == (*expected).into(),
        (
            HirDialoguePointActionIdentity::Mark(actual),
            SyntaxDialoguePointActionIdentity::Mark(expected),
        ) => expected
            .name()
            .is_some_and(|name| actual.as_str() == name.as_str()),
        _ => false,
    };
    if !identity_matches || actual.arguments().len() != expected.arguments().len() {
        return false;
    }
    if !actual
        .arguments()
        .iter()
        .zip(expected.arguments())
        .all(|(actual, expected)| point_action_argument_projection_matches(actual, expected))
    {
        return false;
    }
    matches!(
        (actual.payload(), expected.payload()),
        (
            crate::dialogue_application::HirDialoguePointActionPayload::None,
            SyntaxDialoguePointActionPayload::None
        ) | (
            crate::dialogue_application::HirDialoguePointActionPayload::Call(_),
            SyntaxDialoguePointActionPayload::Call(_)
        ) | (
            crate::dialogue_application::HirDialoguePointActionPayload::TimedCue(_),
            SyntaxDialoguePointActionPayload::TimedCue(_)
        )
    )
}

fn point_action_argument_projection_matches(
    actual: &crate::dialogue_application::HirDialoguePointActionArgument,
    expected: &SyntaxDialogueActionArgumentProjection,
) -> bool {
    match (actual, expected) {
        (
            crate::dialogue_application::HirDialoguePointActionArgument::Positional {
                value: actual,
                ..
            },
            SyntaxDialogueActionArgumentProjection::Positional { value: expected },
        ) => actual.as_str() == expected.decoded(),
        (
            crate::dialogue_application::HirDialoguePointActionArgument::Named {
                name: actual_name,
                value: actual_value,
                ..
            },
            SyntaxDialogueActionArgumentProjection::Named {
                name: Ok(expected_name),
                value: expected_value,
            },
        ) => {
            actual_name.as_ref() == expected_name.as_str()
                && actual_value.as_str() == expected_value.decoded()
        }
        (
            crate::dialogue_application::HirDialoguePointActionArgument::Invalid {
                issue: actual,
                ..
            },
            SyntaxDialogueActionArgumentProjection::Invalid {
                issue: expected, ..
            },
        ) => *actual == (*expected).into(),
        (
            crate::dialogue_application::HirDialoguePointActionArgument::Invalid { .. },
            SyntaxDialogueActionArgumentProjection::Named { name: Err(_), .. },
        ) => true,
        _ => false,
    }
}

fn line_break_projection_matches_new(
    actual: crate::dialogue_application::HirLineBreakKind,
    expected: SyntaxLineBreakKind,
) -> bool {
    matches!(
        (actual, expected),
        (
            crate::dialogue_application::HirLineBreakKind::Line,
            SyntaxLineBreakKind::Line
        ) | (
            crate::dialogue_application::HirLineBreakKind::Paragraph,
            SyntaxLineBreakKind::Paragraph
        ) | (
            crate::dialogue_application::HirLineBreakKind::Page,
            SyntaxLineBreakKind::Page
        )
    )
}

fn dialogue_content_issue_projection_matches_new(
    actual: &HirDialogueContentError,
    expected: &SyntaxDialogueContentIssue,
) -> bool {
    matches!(
        (actual, expected),
        (
            HirDialogueContentError::UnclassifiedToken,
            SyntaxDialogueContentIssue::UnclassifiedToken
        ) | (
            HirDialogueContentError::InvalidPointAction,
            SyntaxDialogueContentIssue::InvalidPointAction
        )
    )
}
