//! Exact Dialogue/RichText payload projection shared by source-backed and E34 expressions.

use arcweft_lang_syntax::expressions::{
    SyntaxDialogueApplicationProjection, SyntaxDialogueContentIssue,
    SyntaxDialogueContentProjection, SyntaxDialogueNodeProjection, SyntaxLineBreakKind,
    SyntaxProjectSymbolPath, SyntaxRichTextArgumentProjection, SyntaxRichTextEndTagProjection,
    SyntaxRichTextIssue, SyntaxRichTextTagIdentity, SyntaxRichTextTagPayloadProjection,
};
use arcweft_lang_syntax::name::SyntaxNameIssue;
use arcweft_lang_syntax::text::RichTextArgumentIssue;

use crate::dialogue_application::{
    HirDialogueContentApplication, HirDialogueContentError, HirDialogueNodeKind, HirLineBreakKind,
    HirRichTextArgument, HirRichTextArgumentIssue, HirRichTextEndTag, HirRichTextIssue,
    HirRichTextTagIdentity, HirRichTextTagPayload,
};

pub(super) fn dialogue_application_projection_matches(
    actual: &HirDialogueContentApplication,
    expected: &SyntaxDialogueApplicationProjection,
) -> bool {
    if actual.plan().is_some() != expected.has_plan() {
        return false;
    }
    match expected.content() {
        SyntaxDialogueContentProjection::Missing { .. } => {
            actual.content().nodes().is_empty() && actual.content().tags().is_empty()
        }
        SyntaxDialogueContentProjection::Present(expected) => {
            actual.content().nodes().len() == expected.nodes().len()
                && actual.content().tags().len() == expected.tags().len()
                && actual.content().nodes().iter().zip(expected.nodes()).all(
                    |(actual, expected)| dialogue_node_projection_matches(actual.kind(), expected),
                )
                && actual
                    .content()
                    .tags()
                    .iter()
                    .zip(expected.tags())
                    .all(|(actual, expected)| {
                        rich_text_tag_identity_projection_matches(
                            actual.identity(),
                            expected.identity(),
                        ) && actual.arguments().len() == expected.arguments().len()
                            && actual.arguments().iter().zip(expected.arguments()).all(
                                |(actual, expected)| {
                                    rich_text_argument_projection_matches(actual, expected)
                                },
                            )
                            && rich_text_payload_projection_matches(
                                actual.payload(),
                                expected.payload(),
                            )
                    })
        }
    }
}

pub(super) fn dialogue_node_projection_matches(
    actual: &HirDialogueNodeKind,
    expected: &SyntaxDialogueNodeProjection,
) -> bool {
    match (actual, expected) {
        (HirDialogueNodeKind::Text(actual), SyntaxDialogueNodeProjection::Text(expected))
        | (HirDialogueNodeKind::Raw(actual), SyntaxDialogueNodeProjection::Raw(expected)) => {
            actual.as_str() == expected.as_ref()
        }
        (HirDialogueNodeKind::Escape(actual), SyntaxDialogueNodeProjection::Escape(expected)) => {
            actual == expected
        }
        (
            HirDialogueNodeKind::Ruby(actual),
            SyntaxDialogueNodeProjection::Ruby {
                base: expected_base,
                ruby: expected_ruby,
            },
        ) => actual.base() == expected_base.as_ref() && actual.ruby() == expected_ruby.as_ref(),
        (
            HirDialogueNodeKind::AuthoredStartTag(actual),
            SyntaxDialogueNodeProjection::AuthoredStartTag { tag: expected },
        )
        | (
            HirDialogueNodeKind::InferredStartTag(actual),
            SyntaxDialogueNodeProjection::InferredStartTag { tag: expected },
        ) => actual.ordinal() == *expected,
        (
            HirDialogueNodeKind::AuthoredEndTag(actual),
            SyntaxDialogueNodeProjection::AuthoredEndTag(expected),
        )
        | (
            HirDialogueNodeKind::InferredEndTag(actual),
            SyntaxDialogueNodeProjection::InferredEndTag(expected),
        ) => rich_text_end_tag_projection_matches(actual, expected),
        (HirDialogueNodeKind::Interpolation(_), SyntaxDialogueNodeProjection::Interpolation(_)) => {
            true
        }
        (
            HirDialogueNodeKind::LineBreak(actual),
            SyntaxDialogueNodeProjection::LineBreak(expected),
        ) => line_break_projection_matches(*actual, *expected),
        (HirDialogueNodeKind::Error(actual), SyntaxDialogueNodeProjection::Error(expected)) => {
            dialogue_content_issue_projection_matches(actual, expected)
        }
        _ => false,
    }
}

const fn line_break_projection_matches(
    actual: HirLineBreakKind,
    expected: SyntaxLineBreakKind,
) -> bool {
    matches!(
        (actual, expected),
        (HirLineBreakKind::Line, SyntaxLineBreakKind::Line)
            | (HirLineBreakKind::Paragraph, SyntaxLineBreakKind::Paragraph)
            | (HirLineBreakKind::Page, SyntaxLineBreakKind::Page)
    )
}

fn dialogue_content_issue_projection_matches(
    actual: &HirDialogueContentError,
    expected: &SyntaxDialogueContentIssue,
) -> bool {
    matches!(
        (actual, expected),
        (
            HirDialogueContentError::UnclassifiedToken,
            SyntaxDialogueContentIssue::UnclassifiedToken
        ) | (
            HirDialogueContentError::InvalidEscape,
            SyntaxDialogueContentIssue::InvalidEscape
        ) | (
            HirDialogueContentError::InvalidRuby,
            SyntaxDialogueContentIssue::InvalidRuby
        ) | (
            HirDialogueContentError::UnmatchedEndTag,
            SyntaxDialogueContentIssue::UnmatchedEndTag
        ) | (
            HirDialogueContentError::UnclosedTag,
            SyntaxDialogueContentIssue::UnclosedTag
        )
    )
}

pub(super) fn rich_text_argument_projection_matches(
    actual: &HirRichTextArgument,
    expected: &SyntaxRichTextArgumentProjection,
) -> bool {
    match (actual, expected) {
        (
            HirRichTextArgument::Positional { value: actual, .. },
            SyntaxRichTextArgumentProjection::Positional { value: expected },
        ) => actual.as_str() == expected.decoded(),
        (
            HirRichTextArgument::Named {
                name: actual_name,
                value: actual_value,
                ..
            },
            SyntaxRichTextArgumentProjection::Named {
                name: Ok(expected_name),
                value: expected_value,
            },
        ) => {
            actual_name.as_str() == expected_name.as_str()
                && actual_value.as_str() == expected_value.decoded()
        }
        (
            HirRichTextArgument::Invalid { issue: actual, .. },
            SyntaxRichTextArgumentProjection::Named {
                name: Err(expected),
                ..
            },
        ) => invalid_named_argument_issue_matches(*actual, expected),
        (
            HirRichTextArgument::Invalid { issue: actual, .. },
            SyntaxRichTextArgumentProjection::Invalid {
                issue: expected, ..
            },
        ) => rich_text_argument_issue_projection_matches(*actual, *expected),
        _ => false,
    }
}

fn invalid_named_argument_issue_matches(
    actual: HirRichTextArgumentIssue,
    _expected: &SyntaxNameIssue,
) -> bool {
    actual == HirRichTextArgumentIssue::InvalidKey
}

pub(super) fn rich_text_tag_identity_projection_matches(
    actual: &HirRichTextTagIdentity,
    expected: &SyntaxRichTextTagIdentity,
) -> bool {
    match (actual, expected) {
        (HirRichTextTagIdentity::Builtin(actual), SyntaxRichTextTagIdentity::Builtin(expected)) => {
            *actual == (*expected).into()
        }
        (
            HirRichTextTagIdentity::Unresolved(actual),
            SyntaxRichTextTagIdentity::DotSelector(Ok(expected)),
        ) => {
            actual.issue() == &HirRichTextIssue::UnknownRegisteredTag
                && actual.name().as_str() == expected.as_str()
        }
        (
            HirRichTextTagIdentity::Unresolved(actual),
            SyntaxRichTextTagIdentity::DotSelector(Err(expected)),
        ) => {
            actual.issue() == &HirRichTextIssue::UnknownRegisteredTag
                && attempted_name_spelling(expected)
                    .is_some_and(|expected| actual.name().as_str() == expected)
        }
        (
            HirRichTextTagIdentity::Unresolved(actual),
            SyntaxRichTextTagIdentity::ProjectSymbol(expected),
        ) => {
            actual.issue() == &HirRichTextIssue::UnknownRegisteredTag
                && project_symbol_terminal_spelling(expected)
                    .is_some_and(|expected| actual.name().as_str() == expected)
        }
        _ => false,
    }
}

fn rich_text_end_tag_projection_matches(
    actual: &HirRichTextEndTag,
    expected: &SyntaxRichTextEndTagProjection,
) -> bool {
    rich_text_end_tag_fields_match(
        actual,
        expected.identity(),
        expected.is_inferred(),
        expected.issue(),
    )
}

fn rich_text_end_tag_fields_match(
    actual: &HirRichTextEndTag,
    expected_identity: Option<&SyntaxRichTextTagIdentity>,
    expected_inferred: bool,
    expected_issue: Option<&SyntaxRichTextIssue>,
) -> bool {
    actual.is_inferred() == expected_inferred
        && match (actual.identity(), expected_identity) {
            (Some(actual), Some(expected)) => {
                rich_text_tag_identity_projection_matches(actual, expected)
            }
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        }
        && actual.issue().cloned() == expected_issue.cloned().map(Into::into)
}

fn attempted_name_spelling(issue: &SyntaxNameIssue) -> Option<&str> {
    match issue {
        SyntaxNameIssue::Missing => None,
        SyntaxNameIssue::InvalidStart { spelling }
        | SyntaxNameIssue::InvalidContinuation { spelling } => Some(spelling),
    }
}

fn project_symbol_terminal_spelling(path: &SyntaxProjectSymbolPath) -> Option<&str> {
    match path.segments().last()? {
        Ok(name) => Some(name.as_str()),
        Err(issue) => attempted_name_spelling(issue),
    }
}

fn rich_text_argument_issue_projection_matches(
    actual: HirRichTextArgumentIssue,
    expected: RichTextArgumentIssue,
) -> bool {
    actual == expected.into()
}

pub(super) const fn rich_text_payload_projection_matches(
    actual: &HirRichTextTagPayload,
    expected: &SyntaxRichTextTagPayloadProjection,
) -> bool {
    matches!(
        (actual, expected),
        (
            HirRichTextTagPayload::Arguments,
            SyntaxRichTextTagPayloadProjection::Arguments
        ) | (
            HirRichTextTagPayload::FxCall(_),
            SyntaxRichTextTagPayloadProjection::FxCall(_)
        ) | (
            HirRichTextTagPayload::DialogueCall(_),
            SyntaxRichTextTagPayloadProjection::DialogueCall(_)
        ) | (
            HirRichTextTagPayload::Condition(_),
            SyntaxRichTextTagPayloadProjection::Condition(_)
        ) | (
            HirRichTextTagPayload::None,
            SyntaxRichTextTagPayloadProjection::None
        )
    )
}

#[cfg(test)]
mod tests {
    use arcweft_lang_syntax::expressions::{SyntaxBuiltinRichTextTag, SyntaxRichTextTagIdentity};
    use arcweft_lang_syntax::name::SyntaxNameIssue;

    use crate::dialogue_application::{
        HirBuiltinRichTextTag, HirRichTextArgumentIssue, HirRichTextEndTag, HirRichTextIssue,
        HirRichTextTagIdentity, HirUnresolvedRichTextTag,
    };
    use crate::leaf::HirProjectSymbolSegment;

    use super::{
        invalid_named_argument_issue_matches, rich_text_end_tag_fields_match,
        rich_text_tag_identity_projection_matches,
    };

    #[test]
    fn rich_text_identity_projection_accepts_only_the_exact_builtin() {
        let expected = SyntaxRichTextTagIdentity::Builtin(SyntaxBuiltinRichTextTag::Page);

        assert!(rich_text_tag_identity_projection_matches(
            &HirRichTextTagIdentity::Builtin(HirBuiltinRichTextTag::Page),
            &expected,
        ));
        assert!(!rich_text_tag_identity_projection_matches(
            &HirRichTextTagIdentity::Builtin(HirBuiltinRichTextTag::Clear),
            &expected,
        ));
    }

    #[test]
    fn rich_text_identity_projection_freezes_unresolved_dot_selector_spelling_and_issue() {
        let expected =
            SyntaxRichTextTagIdentity::DotSelector(Err(SyntaxNameIssue::InvalidContinuation {
                spelling: "bad-name".into(),
            }));
        let segment =
            HirProjectSymbolSegment::try_new("bad-name".into()).expect("attempted marker segment");

        assert!(rich_text_tag_identity_projection_matches(
            &HirRichTextTagIdentity::Unresolved(HirUnresolvedRichTextTag::new(
                segment.clone(),
                HirRichTextIssue::UnknownRegisteredTag,
            )),
            &expected,
        ));
        assert!(!rich_text_tag_identity_projection_matches(
            &HirRichTextTagIdentity::Unresolved(HirUnresolvedRichTextTag::new(
                segment,
                HirRichTextIssue::UnknownTag,
            )),
            &expected,
        ));
    }

    #[test]
    fn rich_text_end_tag_projection_freezes_identity_and_inferred_role() {
        let actual = HirRichTextEndTag::new(
            None,
            Some(HirRichTextTagIdentity::Builtin(HirBuiltinRichTextTag::Page)),
            false,
            None,
        );
        let matching = SyntaxRichTextTagIdentity::Builtin(SyntaxBuiltinRichTextTag::Page);
        let mismatched = SyntaxRichTextTagIdentity::Builtin(SyntaxBuiltinRichTextTag::Clear);

        assert!(rich_text_end_tag_fields_match(
            &actual,
            Some(&matching),
            false,
            None,
        ));
        assert!(!rich_text_end_tag_fields_match(
            &actual,
            Some(&mismatched),
            false,
            None,
        ));
        assert!(!rich_text_end_tag_fields_match(
            &actual,
            Some(&matching),
            true,
            None,
        ));
    }

    #[test]
    fn malformed_named_argument_maps_only_to_invalid_key() {
        let source = SyntaxNameIssue::InvalidStart {
            spelling: "Bad".into(),
        };

        assert!(invalid_named_argument_issue_matches(
            HirRichTextArgumentIssue::InvalidKey,
            &source,
        ));
        assert!(!invalid_named_argument_issue_matches(
            HirRichTextArgumentIssue::MissingValue,
            &source,
        ));
    }
}
