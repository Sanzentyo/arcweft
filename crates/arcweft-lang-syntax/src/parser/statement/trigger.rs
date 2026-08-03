//! Shared one-pass trigger-pattern grammar.

use arcweft_source::SourceRange;

use super::super::cursor::ShadowDocumentParser;
use super::super::expression::emit_expression;
use super::super::pattern::emit_pattern;
use super::super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, find_matching_close_before,
    find_top_level_boundary, first_significant, token_text,
};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TriggerCallKind {
    Input,
    Event,
    Signal,
    Timeout,
    Mark,
    Select,
    Task,
    Scope,
}

pub(super) fn emit_trigger_pattern(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    let Some((kind, close)) = trigger_call_shape(parser, end) else {
        emit_expression(parser, end, role);
        return;
    };

    match kind {
        TriggerCallKind::Input => {
            emit_unary_pattern_call(parser, end, close, role, SyntaxKind::InputTriggerPattern)
        }
        TriggerCallKind::Event => {
            emit_unary_pattern_call(parser, end, close, role, SyntaxKind::EventTriggerPattern)
        }
        TriggerCallKind::Signal => emit_signal_call(parser, end, close, role),
        TriggerCallKind::Timeout => {
            emit_unary_expression_call(parser, end, close, role, SyntaxKind::TimeoutTriggerPattern)
        }
        TriggerCallKind::Mark => {
            emit_unary_pattern_call(parser, end, close, role, SyntaxKind::MarkTriggerPattern)
        }
        TriggerCallKind::Select => {
            emit_unary_pattern_call(parser, end, close, role, SyntaxKind::SelectTriggerPattern)
        }
        TriggerCallKind::Task => {
            emit_unary_pattern_call(parser, end, close, role, SyntaxKind::TaskTriggerPattern)
        }
        TriggerCallKind::Scope => {
            emit_unary_pattern_call(parser, end, close, role, SyntaxKind::ScopeTriggerPattern)
        }
    }
}

fn trigger_call_shape(
    parser: &ShadowDocumentParser<'_, '_>,
    end: usize,
) -> Option<(TriggerCallKind, Option<usize>)> {
    let kind = match parser.current_text()? {
        "input" => TriggerCallKind::Input,
        "event" | "item" | "error" => TriggerCallKind::Event,
        "signal" => TriggerCallKind::Signal,
        "timeout" => TriggerCallKind::Timeout,
        "mark" => TriggerCallKind::Mark,
        "select" => TriggerCallKind::Select,
        "task" => TriggerCallKind::Task,
        "scope" => TriggerCallKind::Scope,
        _ => return None,
    };
    let open = first_significant(parser, parser.cursor().saturating_add(1), end)?;
    if token_text(parser, open) != Some("(") {
        return None;
    }
    let close = find_matching_close_before(parser, open.saturating_add(1), end, "(");
    if close.is_some_and(|close| first_significant(parser, close.saturating_add(1), end).is_some())
    {
        return None;
    }
    Some((kind, close))
}

fn emit_unary_pattern_call(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    close: Option<usize>,
    role: SyntaxRole,
    kind: SyntaxKind,
) {
    parser.start(kind, role);
    emit_trigger_call_open(parser);
    let close = close.unwrap_or(end);
    parser.bump_trivia();
    let argument_end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(close);
    if parser.cursor() < argument_end {
        emit_pattern(parser, argument_end, SyntaxRole::Pattern);
    } else {
        emit_missing_pattern(parser, "trigger pattern requires one pattern argument");
    }
    bump_until(parser, argument_end);
    emit_extra_arguments_recovery(parser, close);
    finish_trigger_call(parser, end, close);
    parser.finish();
}

fn emit_unary_expression_call(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    close: Option<usize>,
    role: SyntaxRole,
    kind: SyntaxKind,
) {
    parser.start(kind, role);
    emit_trigger_call_open(parser);
    let close = close.unwrap_or(end);
    parser.bump_trivia();
    let argument_end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(close);
    if parser.cursor() < argument_end {
        emit_expression(parser, argument_end, SyntaxRole::Operand);
    } else {
        emit_missing_expression(
            parser,
            SyntaxRole::Operand,
            "trigger pattern requires one expression argument",
        );
    }
    bump_until(parser, argument_end);
    emit_extra_arguments_recovery(parser, close);
    finish_trigger_call(parser, end, close);
    parser.finish();
}

fn emit_signal_call(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    close: Option<usize>,
    role: SyntaxRole,
) {
    parser.start(SyntaxKind::SignalTriggerPattern, role);
    emit_trigger_call_open(parser);
    let close = close.unwrap_or(end);
    parser.bump_trivia();
    let target_end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(close);
    if parser.cursor() < target_end {
        emit_expression(parser, target_end, SyntaxRole::Target);
    } else {
        emit_missing_expression(
            parser,
            SyntaxRole::Target,
            "signal trigger requires a target expression",
        );
    }
    bump_until(parser, target_end);

    if parser.cursor() < close && parser.at(",") {
        parser.bump();
        parser.bump_trivia();
        let pattern_end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(close);
        if parser.cursor() < pattern_end {
            emit_pattern(parser, pattern_end, SyntaxRole::Pattern);
        } else {
            emit_missing_pattern(parser, "signal trigger value requires a pattern");
        }
        bump_until(parser, pattern_end);
    }
    emit_extra_arguments_recovery(parser, close);
    finish_trigger_call(parser, end, close);
    parser.finish();
}

fn emit_trigger_call_open(parser: &mut ShadowDocumentParser<'_, '_>) {
    parser.bump();
    parser.bump_trivia();
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
}

fn emit_extra_arguments_recovery(parser: &mut ShadowDocumentParser<'_, '_>, close: usize) {
    parser.bump_trivia();
    if parser.cursor() < close {
        let start = parser.current_offset();
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        bump_until(parser, close);
        parser.finish();
        let finish = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.trigger.extra_arguments",
            SourceRange::new(start, finish),
            "trigger pattern has extra arguments",
        )));
    }
}

fn finish_trigger_call(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, close: usize) {
    bump_until(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.trigger.missing_close",
    );
    if let Some(trailing) = first_significant(parser, parser.cursor(), end) {
        bump_until(parser, trailing);
        let start = parser.current_offset();
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::TrailingRecovery(0));
        bump_until(parser, end);
        parser.finish();
        let finish = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.trigger.trailing_tokens",
            SourceRange::new(start, finish),
            "unexpected tokens after trigger pattern",
        )));
    }
}

fn emit_missing_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    role: SyntaxRole,
    message: &'static str,
) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingExpression, role);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.trigger.missing_expression",
        SourceRange::new(at, at),
        message,
    )));
}

fn emit_missing_pattern(parser: &mut ShadowDocumentParser<'_, '_>, message: &'static str) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingPattern, SyntaxRole::Pattern);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.trigger.missing_pattern",
        SourceRange::new(at, at),
        message,
    )));
}
