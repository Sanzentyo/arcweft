//! Source-component validation for typed Dialogue application projections.
//!
//! Dialogue content has one source-ordered authority. Bracket actions retain
//! their point-action parts and arguments on the node that owns them; there is
//! no tag table, end-tag record, or pairing inventory for this validator to
//! reconcile.

use std::collections::HashSet;

use super::super::{
    ExpressionComponentRole, SyntaxAttachedContentApplicationForm,
    SyntaxAttachedContentApplicationProjection, SyntaxDialogueActionArgumentProjection,
    SyntaxDialogueActionArgumentSourcePart, SyntaxDialogueContent, SyntaxDialogueContentProjection,
    SyntaxDialogueNodeProjection, SyntaxDialogueNodeSourcePart, SyntaxDialoguePointActionIdentity,
    SyntaxDialoguePointActionPayload, SyntaxDialoguePointActionSourcePart,
};
use super::PendingExpressionComponent;

pub(super) fn components_validate(
    application: &SyntaxAttachedContentApplicationProjection,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> bool {
    let outer = dialogue_application_outer_roles(application);
    if !outer.iter().all(|role| roles.contains(role))
        || roles.contains(&ExpressionComponentRole::Plan) != application.has_plan()
    {
        return false;
    }

    let mut expected = outer;
    if application.has_plan() {
        expected.push(ExpressionComponentRole::Plan);
    }

    match application.content() {
        SyntaxDialogueContentProjection::Present(content) => {
            let Some(content_roles) = dialogue_content_roles(content) else {
                return false;
            };
            expected.extend(content_roles);
        }
        SyntaxDialogueContentProjection::RawLiteral(_)
        | SyntaxDialogueContentProjection::Missing { .. } => {}
    }

    expected.len() == components.len()
        && expected.iter().all(|role| roles.contains(role))
        && components
            .iter()
            .all(|component| expected.contains(&component.role()))
}

fn dialogue_content_roles(content: &SyntaxDialogueContent) -> Option<Vec<ExpressionComponentRole>> {
    let mut roles = Vec::new();
    for (ordinal, node) in content.nodes().iter().enumerate() {
        let ordinal = u32::try_from(ordinal).ok()?;
        roles.extend(
            dialogue_node_source_parts(node)
                .iter()
                .copied()
                .map(|part| ExpressionComponentRole::DialogueNode { ordinal, part }),
        );
        if let SyntaxDialogueNodeProjection::PointAction(action) = node {
            roles.extend(point_action_roles(ordinal, action)?);
        }
    }
    Some(roles)
}

fn point_action_roles(
    ordinal: u32,
    action: &super::super::SyntaxDialoguePointActionProjection,
) -> Option<Vec<ExpressionComponentRole>> {
    let mut roles = vec![
        ExpressionComponentRole::DialoguePointAction {
            ordinal,
            part: SyntaxDialoguePointActionSourcePart::Whole,
        },
        ExpressionComponentRole::DialoguePointAction {
            ordinal,
            part: SyntaxDialoguePointActionSourcePart::OpenDelimiter,
        },
        ExpressionComponentRole::DialoguePointAction {
            ordinal,
            part: SyntaxDialoguePointActionSourcePart::Name,
        },
        ExpressionComponentRole::DialoguePointAction {
            ordinal,
            part: SyntaxDialoguePointActionSourcePart::CloseDelimiter,
        },
    ];
    if !matches!(action.payload(), SyntaxDialoguePointActionPayload::None) {
        roles.push(ExpressionComponentRole::DialoguePointAction {
            ordinal,
            part: SyntaxDialoguePointActionSourcePart::Payload,
        });
    }
    if let SyntaxDialoguePointActionIdentity::Mark(selector) = action.identity() {
        roles.extend(selector.components().iter().map(|component| {
            ExpressionComponentRole::DialoguePointAction {
                ordinal,
                part: SyntaxDialoguePointActionSourcePart::Marker(component.part()),
            }
        }));
    }
    for (argument, value) in action.arguments().iter().enumerate() {
        let argument = u16::try_from(argument).ok()?;
        roles.extend(
            rich_text_argument_source_parts(value)
                .into_iter()
                .map(
                    |part| ExpressionComponentRole::DialoguePointActionArgument {
                        action: ordinal,
                        argument,
                        part,
                    },
                ),
        );
    }
    Some(roles)
}

fn dialogue_application_outer_roles(
    application: &SyntaxAttachedContentApplicationProjection,
) -> Vec<ExpressionComponentRole> {
    match application.form() {
        SyntaxAttachedContentApplicationForm::Bracket { .. } => vec![
            ExpressionComponentRole::Target,
            ExpressionComponentRole::OpenBracket,
            ExpressionComponentRole::CloseBracket,
            ExpressionComponentRole::Content,
            ExpressionComponentRole::ContentBody,
        ],
        SyntaxAttachedContentApplicationForm::Colon => vec![
            ExpressionComponentRole::Target,
            ExpressionComponentRole::Colon,
            ExpressionComponentRole::Content,
            ExpressionComponentRole::ContentBody,
        ],
        SyntaxAttachedContentApplicationForm::Hash => {
            let mut roles = vec![
                ExpressionComponentRole::Hash,
                ExpressionComponentRole::Target,
            ];
            if !matches!(
                application.content(),
                SyntaxDialogueContentProjection::Missing {
                    boundary: super::super::SyntaxDialogueContentRecoveryBoundary::Inline { .. },
                }
            ) {
                roles.extend([
                    ExpressionComponentRole::OpenBracket,
                    ExpressionComponentRole::CloseBracket,
                    ExpressionComponentRole::Content,
                    ExpressionComponentRole::ContentBody,
                ]);
            }
            roles
        }
    }
}

fn dialogue_node_source_parts(
    node: &SyntaxDialogueNodeProjection,
) -> &'static [SyntaxDialogueNodeSourcePart] {
    match node {
        SyntaxDialogueNodeProjection::Text(_) => &[
            SyntaxDialogueNodeSourcePart::Whole,
            SyntaxDialogueNodeSourcePart::Text,
        ],
        SyntaxDialogueNodeProjection::Escape(_) => &[
            SyntaxDialogueNodeSourcePart::Whole,
            SyntaxDialogueNodeSourcePart::Escape,
        ],
        SyntaxDialogueNodeProjection::Ruby { .. } => &[
            SyntaxDialogueNodeSourcePart::Whole,
            SyntaxDialogueNodeSourcePart::Ruby,
        ],
        SyntaxDialogueNodeProjection::Interpolation(_) => &[
            SyntaxDialogueNodeSourcePart::Whole,
            SyntaxDialogueNodeSourcePart::Interpolation,
        ],
        SyntaxDialogueNodeProjection::ContentApplication(_) => &[
            SyntaxDialogueNodeSourcePart::Whole,
            SyntaxDialogueNodeSourcePart::Hash,
            SyntaxDialogueNodeSourcePart::Expression,
        ],
        SyntaxDialogueNodeProjection::PointAction(_) => &[
            SyntaxDialogueNodeSourcePart::Whole,
            SyntaxDialogueNodeSourcePart::PointAction,
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
    argument: &SyntaxDialogueActionArgumentProjection,
) -> Vec<SyntaxDialogueActionArgumentSourcePart> {
    match argument {
        SyntaxDialogueActionArgumentProjection::Positional { .. } => vec![
            SyntaxDialogueActionArgumentSourcePart::Whole,
            SyntaxDialogueActionArgumentSourcePart::Value,
        ],
        SyntaxDialogueActionArgumentProjection::Named { .. } => vec![
            SyntaxDialogueActionArgumentSourcePart::Whole,
            SyntaxDialogueActionArgumentSourcePart::Name,
            SyntaxDialogueActionArgumentSourcePart::Equals,
            SyntaxDialogueActionArgumentSourcePart::Value,
        ],
        SyntaxDialogueActionArgumentProjection::Invalid { authored_parts, .. } => {
            let mut parts = vec![SyntaxDialogueActionArgumentSourcePart::Whole];
            if authored_parts.has_name() {
                parts.push(SyntaxDialogueActionArgumentSourcePart::Name);
            }
            if authored_parts.has_equals() {
                parts.push(SyntaxDialogueActionArgumentSourcePart::Equals);
            }
            if authored_parts.has_value() {
                parts.push(SyntaxDialogueActionArgumentSourcePart::Value);
            }
            parts
        }
    }
}
