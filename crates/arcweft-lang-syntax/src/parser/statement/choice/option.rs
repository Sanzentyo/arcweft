//! Choice option-field grammar over the shared statement cursor.

use super::super::super::cursor::DocumentParser;
use super::super::super::expression::{emit_entity_reference, emit_expression};
use super::super::super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter,
    emit_required_punctuation, find_matching_close_before, find_statement_terminator, trimmed_end,
};
use super::super::indentation::bump_trivia_before;
use super::super::{
    emit_braced_thread_flow_block_until, emit_item_expression, emit_statement_with_role,
    top_level_operator,
};
use super::{
    emit_missing_body, emit_missing_expression, emit_missing_token_diagnostic, emit_recovery,
    finish_choice_delimited_body,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn is_choice_option_field_head(
    _kind: Option<SyntaxKind>,
    spelling: Option<&str>,
) -> bool {
    spelling.is_some_and(|spelling| {
        matches!(
            spelling,
            "label"
                | "id"
                | "value"
                | "visible"
                | "enabled"
                | "order"
                | "hotkey"
                | "view"
                | "select"
                | "let"
        )
    })
}

pub(super) fn emit_choice_option_field(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    match parser.current_text() {
        Some("label") => emit_choice_label_field(parser, end, item_kind, ordinal),
        Some("id") => {
            emit_choice_assignment_field(
                parser,
                end,
                item_kind,
                ordinal,
                SyntaxKind::ChoiceIdField,
            );
        }
        Some("value") => emit_choice_assignment_field(
            parser,
            end,
            item_kind,
            ordinal,
            SyntaxKind::ChoiceValueField,
        ),
        Some("visible") => emit_choice_assignment_field(
            parser,
            end,
            item_kind,
            ordinal,
            SyntaxKind::ChoiceVisibleField,
        ),
        Some("enabled") => emit_choice_assignment_field(
            parser,
            end,
            item_kind,
            ordinal,
            SyntaxKind::ChoiceEnabledField,
        ),
        Some("order") => emit_choice_assignment_field(
            parser,
            end,
            item_kind,
            ordinal,
            SyntaxKind::ChoiceOrderField,
        ),
        Some("hotkey") => emit_choice_assignment_field(
            parser,
            end,
            item_kind,
            ordinal,
            SyntaxKind::ChoiceHotkeyField,
        ),
        Some("view") => emit_choice_view_field(parser, end, item_kind, ordinal),
        Some("select") => emit_choice_select_field(parser, end, item_kind, ordinal),
        Some("let") => {
            emit_statement_with_role(
                parser,
                end,
                item_kind,
                SyntaxRole::ChoiceOptionField(ordinal),
            );
        }
        _ => emit_recovery(
            parser,
            end,
            SyntaxRole::ChoiceOptionField(ordinal),
            "syntax.choice.invalid_option_field",
            "unknown Choice option field",
        ),
    }
}

fn emit_choice_assignment_field(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
    kind: SyntaxKind,
) {
    parser.start(kind, SyntaxRole::ChoiceOptionField(ordinal));
    parser.bump();
    bump_trivia_before(parser, end);
    emit_required_punctuation(
        parser,
        SyntaxKind::EqualsNode,
        SyntaxRole::Equals,
        "=",
        "syntax.choice.field_missing_equals",
        "Choice option field requires `=`",
    );
    bump_trivia_before(parser, end);
    emit_item_expression(parser, end, SyntaxRole::Value, item_kind);
    bump_until(parser, end);
    parser.finish();
}

fn emit_choice_label_field(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    parser.start(
        SyntaxKind::ChoiceLabelField,
        SyntaxRole::ChoiceOptionField(ordinal),
    );
    parser.bump();
    bump_trivia_before(parser, end);
    if parser.at("(") {
        emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
        bump_trivia_before(parser, end);
        if parser.at("id") {
            parser.bump();
        } else {
            emit_missing_token_diagnostic(
                parser,
                "syntax.choice.label_missing_id_key",
                "localized Choice label requires `id`",
            );
        }
        bump_trivia_before(parser, end);
        if parser.at("=") {
            parser.bump();
        } else {
            emit_missing_token_diagnostic(
                parser,
                "syntax.choice.label_id_missing_equals",
                "localized Choice label ID requires `=`",
            );
        }
        bump_trivia_before(parser, end);
        if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) {
            emit_entity_reference(parser, SyntaxRole::PublicId);
        } else {
            emit_missing_expression(
                parser,
                SyntaxRole::PublicId,
                "syntax.choice.label_missing_text_key",
                "localized Choice label requires a text-key entity reference",
            );
        }
        bump_trivia_before(parser, end);
        if parser.at(")") {
            emit_close_delimiter(
                parser,
                SyntaxKind::CloseParenNode,
                ")",
                "syntax.choice.label_missing_close",
            );
        } else {
            emit_missing_delimiter(
                parser,
                SyntaxKind::CloseParenNode,
                SyntaxRole::CloseDelimiter,
            );
        }
        bump_trivia_before(parser, end);
    }
    emit_required_punctuation(
        parser,
        SyntaxKind::EqualsNode,
        SyntaxRole::Equals,
        "=",
        "syntax.choice.label_missing_equals",
        "Choice label requires `=`",
    );
    bump_trivia_before(parser, end);
    emit_item_expression(parser, end, SyntaxRole::Value, item_kind);
    bump_until(parser, end);
    parser.finish();
}

fn emit_choice_view_field(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    parser.start(
        SyntaxKind::ChoiceViewField,
        SyntaxRole::ChoiceOptionField(ordinal),
    );
    parser.bump();
    bump_trivia_before(parser, end);
    if parser.at("{") {
        emit_choice_view_body(parser, end, item_kind);
    } else {
        emit_missing_body(
            parser,
            SyntaxRole::Body,
            "syntax.choice.view_missing_body",
            "missing Choice option view body",
        );
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_choice_view_body(parser: &mut DocumentParser<'_, '_>, end: usize, item_kind: SyntaxKind) {
    parser.start(SyntaxKind::ChoiceViewBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close_before(parser, parser.cursor(), end, "{").unwrap_or(end);
    let mut ordinal = 0_u32;
    while parser.cursor() < close {
        bump_trivia_before(parser, close);
        if parser.cursor() >= close {
            break;
        }
        let start = parser.cursor();
        let terminator = find_statement_terminator(parser, start, close);
        let segment_end = terminator.map_or(close, |(index, _)| index);
        let significant_end = trimmed_end(parser, start, segment_end);
        parser.start(
            SyntaxKind::ChoiceViewField,
            SyntaxRole::ChoiceViewField(ordinal),
        );
        let equals =
            top_level_operator(parser, start, significant_end, "=").unwrap_or(significant_end);
        emit_expression(parser, equals, SyntaxRole::Key);
        bump_until(parser, equals);
        emit_required_punctuation(
            parser,
            SyntaxKind::EqualsNode,
            SyntaxRole::Equals,
            "=",
            "syntax.choice.view_field_missing_equals",
            "Choice view field requires `=`",
        );
        bump_trivia_before(parser, significant_end);
        emit_item_expression(parser, significant_end, SyntaxRole::Value, item_kind);
        bump_until(parser, significant_end);
        parser.finish();
        let consumed_end = if terminator.is_some_and(|(_, semicolon)| semicolon) {
            segment_end.saturating_add(1)
        } else {
            segment_end
        };
        bump_until(parser, consumed_end);
        ordinal = ordinal
            .checked_add(1)
            .expect("the grammar budget keeps Choice view fields within u32");
    }
    finish_choice_delimited_body(
        parser,
        "syntax.choice.view_missing_close",
        "missing closing `}` for Choice view body",
    );
}

fn emit_choice_select_field(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    parser.start(
        SyntaxKind::ChoiceSelectField,
        SyntaxRole::ChoiceOptionField(ordinal),
    );
    parser.bump();
    bump_trivia_before(parser, end);
    if parser.at("{") {
        emit_braced_thread_flow_block_until(
            parser,
            end,
            item_kind,
            SyntaxKind::Block,
            SyntaxRole::Body,
            "syntax.choice.select_missing_close",
        );
    } else {
        emit_missing_body(
            parser,
            SyntaxRole::Body,
            "syntax.choice.select_missing_body",
            "missing Choice option select body",
        );
    }
    bump_until(parser, end);
    parser.finish();
}
