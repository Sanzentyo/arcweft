//! Source-component validation for typed Dialogue application projections.

use std::collections::HashSet;

use super::super::{
    ExpressionComponentRole, SyntaxDialogueApplicationForm, SyntaxDialogueApplicationProjection,
    SyntaxDialogueContent, SyntaxDialogueContentProjection, SyntaxDialogueNodeProjection,
    SyntaxDialogueNodeSourcePart, SyntaxRichTextArgumentProjection,
    SyntaxRichTextArgumentSourcePart, SyntaxRichTextTagIdentity, SyntaxRichTextTagProjection,
    SyntaxRichTextTagSourcePart,
};
use super::PendingExpressionComponent;
use crate::expressions::SyntaxDialogueMarkName;
use crate::id_ref::SyntaxIdRefPart;

pub(super) fn components_validate(
    application: &SyntaxDialogueApplicationProjection,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> bool {
    let outer = match application.form() {
        SyntaxDialogueApplicationForm::Bracket { .. } => [
            Some(ExpressionComponentRole::Target),
            Some(ExpressionComponentRole::OpenBracket),
            Some(ExpressionComponentRole::CloseBracket),
            Some(ExpressionComponentRole::Content),
            Some(ExpressionComponentRole::ContentBody),
        ],
        SyntaxDialogueApplicationForm::Colon => [
            Some(ExpressionComponentRole::Target),
            Some(ExpressionComponentRole::Colon),
            Some(ExpressionComponentRole::Content),
            Some(ExpressionComponentRole::ContentBody),
            None,
        ],
    };
    if !outer.iter().flatten().all(|role| roles.contains(role))
        || roles.contains(&ExpressionComponentRole::Plan) != application.has_plan()
    {
        return false;
    }
    let outer_count = outer.iter().flatten().count() + usize::from(application.has_plan());
    let SyntaxDialogueContentProjection::Present(content) = application.content() else {
        return components.len() == outer_count
            && components.iter().all(|component| {
                outer.iter().flatten().any(|role| *role == component.role())
                    || application.has_plan() && component.role() == ExpressionComponentRole::Plan
            });
    };

    let Some(expected) = validate_dialogue_nodes(content, roles, outer_count) else {
        return false;
    };
    let Some(expected) = validate_rich_text_tags(content, roles, expected) else {
        return false;
    };
    components.len() == expected
        && components.iter().all(|component| {
            dialogue_component_is_expected(application, content, &outer, component.role())
        })
}

fn validate_dialogue_nodes(
    content: &SyntaxDialogueContent,
    roles: &HashSet<ExpressionComponentRole>,
    mut expected: usize,
) -> Option<usize> {
    for (ordinal, node) in content.nodes().iter().enumerate() {
        let ordinal = u32::try_from(ordinal).ok()?;
        let parts = dialogue_node_source_parts(node);
        if !parts.iter().all(|part| {
            roles.contains(&ExpressionComponentRole::DialogueNode {
                ordinal,
                part: *part,
            })
        }) {
            return None;
        }
        match node {
            SyntaxDialogueNodeProjection::AuthoredStartTag { tag }
            | SyntaxDialogueNodeProjection::InferredStartTag { tag }
                if content.tags().get(*tag as usize).is_none() =>
            {
                return None;
            }
            SyntaxDialogueNodeProjection::AuthoredEndTag(end)
            | SyntaxDialogueNodeProjection::InferredEndTag(end)
                if end.identity().is_none() && !end.has_recovery() =>
            {
                return None;
            }
            _ => {}
        }
        expected = expected.checked_add(parts.len())?;
    }
    Some(expected)
}

fn validate_rich_text_tags(
    content: &SyntaxDialogueContent,
    roles: &HashSet<ExpressionComponentRole>,
    mut expected: usize,
) -> Option<usize> {
    for (tag, projection) in content.tags().iter().enumerate() {
        let tag = u32::try_from(tag).ok()?;
        let inferred = content.nodes().iter().any(|node| {
            matches!(node, SyntaxDialogueNodeProjection::InferredStartTag { tag: node_tag } if *node_tag == tag)
        });
        let mut tag_parts = vec![
            SyntaxRichTextTagSourcePart::Whole,
            SyntaxRichTextTagSourcePart::OpenDelimiter,
            SyntaxRichTextTagSourcePart::Name,
            SyntaxRichTextTagSourcePart::Payload,
            SyntaxRichTextTagSourcePart::CloseDelimiter,
        ];
        if inferred {
            tag_parts.push(SyntaxRichTextTagSourcePart::InferenceInsertion);
        }
        if let Some(end) = projection.paired_end_node() {
            if content.nodes().get(end as usize).is_none_or(|node| {
                !matches!(
                    node,
                    SyntaxDialogueNodeProjection::AuthoredEndTag(_)
                        | SyntaxDialogueNodeProjection::InferredEndTag(_)
                )
            }) {
                return None;
            }
            tag_parts.push(SyntaxRichTextTagSourcePart::EndTag);
        }
        if !tag_parts
            .iter()
            .all(|part| roles.contains(&ExpressionComponentRole::RichTextTag { tag, part: *part }))
        {
            return None;
        }
        expected = expected.checked_add(tag_parts.len())?;
        if let SyntaxRichTextTagIdentity::Marker(selector) = projection.identity() {
            for part in marker_source_parts(selector) {
                if !roles.contains(&ExpressionComponentRole::RichTextTag {
                    tag,
                    part: SyntaxRichTextTagSourcePart::Marker(part),
                }) {
                    return None;
                }
                expected = expected.checked_add(1)?;
            }
        }
        expected = validate_tag_arguments(tag, projection, roles, expected)?;
    }
    Some(expected)
}

fn validate_tag_arguments(
    tag: u32,
    projection: &SyntaxRichTextTagProjection,
    roles: &HashSet<ExpressionComponentRole>,
    mut expected: usize,
) -> Option<usize> {
    for (argument, value) in projection.arguments().iter().enumerate() {
        let argument = u16::try_from(argument).ok()?;
        let parts = rich_text_argument_source_parts(value);
        if !parts.iter().all(|part| {
            roles.contains(&ExpressionComponentRole::RichTextArgument {
                tag,
                argument,
                part: *part,
            })
        }) {
            return None;
        }
        expected = expected.checked_add(parts.len())?;
    }
    Some(expected)
}

fn dialogue_component_is_expected(
    application: &SyntaxDialogueApplicationProjection,
    content: &SyntaxDialogueContent,
    outer: &[Option<ExpressionComponentRole>; 5],
    role: ExpressionComponentRole,
) -> bool {
    match role {
            role if outer.iter().flatten().any(|outer| *outer == role) => true,
            ExpressionComponentRole::Plan => application.has_plan(),
            ExpressionComponentRole::DialogueNode { ordinal, part } => content
                .nodes()
                .get(ordinal as usize)
                .is_some_and(|node| dialogue_node_source_parts(node).contains(&part)),
            ExpressionComponentRole::RichTextTag { tag, part } => content
                .tags()
                .get(tag as usize)
                .is_some_and(|projection| {
                    matches!(
                        part,
                        SyntaxRichTextTagSourcePart::Whole
                            | SyntaxRichTextTagSourcePart::OpenDelimiter
                            | SyntaxRichTextTagSourcePart::Name
                            | SyntaxRichTextTagSourcePart::Payload
                            | SyntaxRichTextTagSourcePart::CloseDelimiter
                    ) || part == SyntaxRichTextTagSourcePart::InferenceInsertion
                        && content.nodes().iter().any(|node| {
                            matches!(node, SyntaxDialogueNodeProjection::InferredStartTag { tag: node_tag } if *node_tag == tag)
                        })
                        || part == SyntaxRichTextTagSourcePart::EndTag
                            && projection.paired_end_node().is_some()
                        || matches!(
                            (part, projection.identity()),
                            (
                                SyntaxRichTextTagSourcePart::Marker(marker_part),
                                super::super::SyntaxRichTextTagIdentity::Marker(selector)
                            ) if marker_source_parts(selector).contains(&marker_part)
                        )
                }),
            ExpressionComponentRole::RichTextArgument {
                tag,
                argument,
                part,
            } => content
                .tags()
                .get(tag as usize)
                .and_then(|projection| projection.arguments().get(argument as usize))
                .is_some_and(|argument| rich_text_argument_source_parts(argument).contains(&part)),
            _ => false,
        }
}

fn marker_source_parts(selector: &SyntaxDialogueMarkName) -> Vec<SyntaxIdRefPart> {
    selector
        .components()
        .iter()
        .map(|component| component.part())
        .collect()
}

fn dialogue_node_source_parts(
    node: &SyntaxDialogueNodeProjection,
) -> &'static [SyntaxDialogueNodeSourcePart] {
    match node {
        SyntaxDialogueNodeProjection::Text(_) => &[
            SyntaxDialogueNodeSourcePart::Whole,
            SyntaxDialogueNodeSourcePart::Text,
        ],
        SyntaxDialogueNodeProjection::Raw(_) => &[
            SyntaxDialogueNodeSourcePart::Whole,
            SyntaxDialogueNodeSourcePart::Raw,
        ],
        SyntaxDialogueNodeProjection::Escape(_) => &[
            SyntaxDialogueNodeSourcePart::Whole,
            SyntaxDialogueNodeSourcePart::Escape,
        ],
        SyntaxDialogueNodeProjection::Ruby { .. } => &[
            SyntaxDialogueNodeSourcePart::Whole,
            SyntaxDialogueNodeSourcePart::RubyBase,
            SyntaxDialogueNodeSourcePart::RubyText,
        ],
        SyntaxDialogueNodeProjection::AuthoredStartTag { .. }
        | SyntaxDialogueNodeProjection::InferredStartTag { .. }
        | SyntaxDialogueNodeProjection::AuthoredEndTag(_)
        | SyntaxDialogueNodeProjection::InferredEndTag(_) => &[SyntaxDialogueNodeSourcePart::Whole],
        SyntaxDialogueNodeProjection::Interpolation(_) => &[
            SyntaxDialogueNodeSourcePart::Whole,
            SyntaxDialogueNodeSourcePart::Interpolation,
        ],
        SyntaxDialogueNodeProjection::LineBreak(_) => &[
            SyntaxDialogueNodeSourcePart::Whole,
            SyntaxDialogueNodeSourcePart::LineBreak,
        ],
        SyntaxDialogueNodeProjection::Error(_) => &[
            SyntaxDialogueNodeSourcePart::Whole,
            SyntaxDialogueNodeSourcePart::Error,
        ],
    }
}

fn rich_text_argument_source_parts(
    argument: &SyntaxRichTextArgumentProjection,
) -> Vec<SyntaxRichTextArgumentSourcePart> {
    match argument {
        SyntaxRichTextArgumentProjection::Positional { .. } => vec![
            SyntaxRichTextArgumentSourcePart::Whole,
            SyntaxRichTextArgumentSourcePart::Value,
        ],
        SyntaxRichTextArgumentProjection::Named { .. } => vec![
            SyntaxRichTextArgumentSourcePart::Whole,
            SyntaxRichTextArgumentSourcePart::Name,
            SyntaxRichTextArgumentSourcePart::Equals,
            SyntaxRichTextArgumentSourcePart::Value,
        ],
        SyntaxRichTextArgumentProjection::Invalid { authored_parts, .. } => {
            let mut parts = vec![SyntaxRichTextArgumentSourcePart::Whole];
            if authored_parts.has_name() {
                parts.push(SyntaxRichTextArgumentSourcePart::Name);
            }
            if authored_parts.has_equals() {
                parts.push(SyntaxRichTextArgumentSourcePart::Equals);
            }
            if authored_parts.has_value() {
                parts.push(SyntaxRichTextArgumentSourcePart::Value);
            }
            parts
        }
    }
}
