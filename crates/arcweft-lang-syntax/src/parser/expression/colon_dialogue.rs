//! Statement-owned `speaker: content` dialogue application grammar.

use arcweft_source::SourceRange;

use super::{CompletedNode, emit_expression_node};
use crate::expressions::{
    ExpressionComponentRole, ExpressionProjection, PendingExpressionComponent,
    PendingExpressionProjection, SyntaxDialogueApplicationForm,
    SyntaxDialogueApplicationProjection, SyntaxDialogueContentRecoveryBoundary,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::parser::cursor::DocumentParser;
use crate::parser::rich_text_grammar::emit_dialogue_content;
use crate::parser::shadow_recovery::{bump_until, find_top_level_boundary};

pub(in crate::parser) fn emit_colon_dialogue_application(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> Option<CompletedNode> {
    let start = parser.cursor();
    let colon = find_top_level_boundary(parser, start, end, &[":"]);
    if colon == end {
        return None;
    }

    let target = emit_expression_node(parser, colon, role);
    let target_range = parser
        .completed_range(target.start_event)
        .expect("completed dialogue target retains one source range");
    let owner = parser.insert_projected_start(
        target.start_event,
        SyntaxKind::DialogueContentApplicationExpression,
        role,
    );
    parser.set_start_role(target.start_event + 1, SyntaxRole::Target);

    bump_until(parser, colon);
    let colon_range = parser
        .current()
        .expect("colon dialogue dispatch retains its colon token")
        .range();
    parser.start(SyntaxKind::ColonNode, SyntaxRole::Colon);
    parser.bump();
    parser.finish();

    let content_start = parser.cursor();
    let content_range = SourceRange::new(
        parser
            .offset_at_token_boundary(content_start)
            .expect("dialogue content starts at a lexer boundary"),
        parser
            .offset_at_token_boundary(end)
            .expect("dialogue content ends at a lexer boundary"),
    );
    let indented = (content_start..end).any(|index| {
        parser
            .token_at(index)
            .is_some_and(|token| token.kind() == SyntaxKind::NewlineToken)
    });
    let missing_boundary = if indented {
        SyntaxDialogueContentRecoveryBoundary::Indented {
            insertion: content_range.end(),
        }
    } else {
        SyntaxDialogueContentRecoveryBoundary::Inline {
            insertion: content_range.end(),
        }
    };
    let emitted = emit_dialogue_content(parser, end, missing_boundary);
    let (content, mut components, _) = emitted.into_parts();
    let mut outer = vec![
        PendingExpressionComponent::new(ExpressionComponentRole::Target, target_range),
        PendingExpressionComponent::new(ExpressionComponentRole::Colon, colon_range),
        PendingExpressionComponent::new(ExpressionComponentRole::Content, content_range),
        PendingExpressionComponent::new(ExpressionComponentRole::ContentBody, content_range),
    ];
    outer.append(&mut components);
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::DialogueContentApplication(
                SyntaxDialogueApplicationProjection::new(
                    SyntaxDialogueApplicationForm::Colon,
                    content,
                    false,
                ),
            ),
            outer,
        ),
    );
    parser.finish();
    Some(CompletedNode {
        start_event: target.start_event,
    })
}
