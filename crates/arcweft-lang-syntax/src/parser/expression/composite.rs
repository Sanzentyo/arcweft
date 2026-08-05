//! Parenthesized and closure-expression events over the shared cursor.

use super::{CompletedNode, control, emit_expression};
use arcweft_source::SourceRange;

use crate::expressions::{
    ExpressionComponentRole, ExpressionProjection, ExpressionRecordFieldPart,
    PendingExpressionComponent, PendingExpressionProjection, SyntaxClosureParameterPart,
    SyntaxClosureParameterProjection, SyntaxClosureProjection, SyntaxClosureSyntax,
    SyntaxClosureTerminator, SyntaxComputationBlockKind, SyntaxExpressionSlot,
    SyntaxNumericSequence, SyntaxNumericSequenceElement, SyntaxNumericSequenceRecovery,
    SyntaxRecordField, SyntaxThreadMode, SyntaxThreadProjection,
};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::literal::{SyntaxLiteralIssue, SyntaxLiteralValue};
use crate::name::{SyntaxName, SyntaxNameIssue};
use crate::parser::cursor::ShadowDocumentParser;
use crate::parser::lexer::{LiteralLexemePart, typed_literal};
use crate::parser::pattern::emit_pattern;
use crate::parser::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, emit_required_punctuation,
    find_matching_close, find_top_level_boundary, first_significant, token_text, trimmed_end,
};
use crate::parser::type_ref::emit_type;

pub(super) fn emit_parenthesized(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let close = find_matching_close(parser, parser.cursor() + 1, "(")
        .unwrap_or(end)
        .min(end);
    let first = first_significant(parser, parser.cursor() + 1, close);
    let comma = find_top_level_boundary(parser, parser.cursor() + 1, &[",", ")"]);
    if first.is_some() && comma >= close {
        return emit_delimited_group(parser, close, role);
    }
    emit_tuple(parser, close, role, first.is_none())
}

fn emit_delimited_group(
    parser: &mut ShadowDocumentParser<'_, '_>,
    close: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    parser.start_transparent_expression_group(role);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    parser.bump_trivia();
    emit_expression(parser, close, SyntaxRole::Operand);
    bump_until(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.expression.missing_parenthesis_close",
    );
    parser.finish();
    CompletedNode { start_event }
}

fn emit_tuple(
    parser: &mut ShadowDocumentParser<'_, '_>,
    close: usize,
    role: SyntaxRole,
    unit: bool,
) -> CompletedNode {
    let start_event = parser.event_position();
    let owner = parser.start_projected_owner(SyntaxKind::TupleExpression, role);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    parser.start(SyntaxKind::ExpressionList, SyntaxRole::Element(0));
    let mut slots = Vec::new();
    let mut components = Vec::new();
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at(")") {
            break;
        }
        let element_end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(close);
        let (slot, range) = expression_slot(parser, element_end);
        emit_expression(parser, element_end, SyntaxRole::Element(ordinal));
        bump_until(parser, element_end);
        slots.push(slot);
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::Element { ordinal },
            range,
        ));
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.expression.missing_parenthesis_close",
    );
    parser.set_expression_projection(
        owner,
        if unit {
            PendingExpressionProjection::new(ExpressionProjection::Unit, Vec::new())
        } else {
            PendingExpressionProjection::new(
                ExpressionProjection::Tuple(slots.into_boxed_slice()),
                components,
            )
        },
    );
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn emit_bracket_sequence(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let close = find_matching_close(parser, parser.cursor() + 1, "[")
        .unwrap_or(end)
        .min(end);
    let content_start = parser.cursor() + 1;
    let separator = find_top_level_boundary(parser, content_start, &[";", "]"]);
    let has_repeat_separator = separator < close && token_text(parser, separator) == Some(";");
    let numeric = (!has_repeat_separator)
        .then(|| numeric_sequence_projection(parser, content_start, close))
        .flatten();
    let kind = if has_repeat_separator {
        SyntaxKind::ArrayRepeatExpression
    } else if numeric.is_some() {
        SyntaxKind::NumericBracketSequenceExpression
    } else {
        SyntaxKind::BracketSequenceExpression
    };
    let owner = parser.start_projected_owner(kind, role);
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    parser.start(SyntaxKind::ExpressionList, SyntaxRole::Element(0));
    let projection = match kind {
        SyntaxKind::NumericBracketSequenceExpression => {
            bump_until(parser, close);
            let (sequence, components) = numeric.expect("numeric kind retains its projection");
            PendingExpressionProjection::new(
                ExpressionProjection::NumericBracketSequence(sequence),
                components,
            )
        }
        SyntaxKind::ArrayRepeatExpression => {
            let (slots, components) = emit_array_repeat_elements(parser, separator, close);
            PendingExpressionProjection::new(ExpressionProjection::ArrayRepeat(slots), components)
        }
        SyntaxKind::BracketSequenceExpression => {
            let (slots, components) = emit_bracket_elements(parser, close);
            PendingExpressionProjection::new(
                ExpressionProjection::BracketSequence(slots.into_boxed_slice()),
                components,
            )
        }
        _ => unreachable!("bracket parser selects one exact bracket expression family"),
    };
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.expression.missing_bracket_close",
    );
    parser.set_expression_projection(owner, projection);
    parser.finish();
    CompletedNode { start_event }
}

fn emit_bracket_elements(
    parser: &mut ShadowDocumentParser<'_, '_>,
    close: usize,
) -> (Vec<SyntaxExpressionSlot>, Vec<PendingExpressionComponent>) {
    let mut slots = Vec::new();
    let mut components = Vec::new();
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at("]") {
            break;
        }
        let element_end =
            find_top_level_boundary(parser, parser.cursor(), &[",", ";", "]"]).min(close);
        let (slot, range) = expression_slot(parser, element_end);
        emit_expression(parser, element_end, SyntaxRole::Element(ordinal));
        bump_until(parser, element_end);
        slots.push(slot);
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::Element { ordinal },
            range,
        ));
        ordinal = ordinal.saturating_add(1);
        if matches!(parser.current_text(), Some("," | ";")) {
            parser.bump();
        } else {
            break;
        }
    }
    (slots, components)
}

fn emit_array_repeat_elements(
    parser: &mut ShadowDocumentParser<'_, '_>,
    separator: usize,
    close: usize,
) -> ([SyntaxExpressionSlot; 2], Vec<PendingExpressionComponent>) {
    parser.bump_trivia();
    let (value, value_range) = expression_slot(parser, separator);
    emit_expression(parser, separator, SyntaxRole::Element(0));
    bump_until(parser, separator);
    if parser.at(";") {
        parser.bump();
    }

    parser.bump_trivia();
    let (length, length_range) = expression_slot(parser, close);
    emit_expression(parser, close, SyntaxRole::Element(1));
    bump_until(parser, close);

    (
        [value, length],
        vec![
            PendingExpressionComponent::new(ExpressionComponentRole::RepeatValue, value_range),
            PendingExpressionComponent::new(ExpressionComponentRole::RepeatLength, length_range),
        ],
    )
}

pub(in crate::parser) fn expression_slot(
    parser: &ShadowDocumentParser<'_, '_>,
    end: usize,
) -> (SyntaxExpressionSlot, SourceRange) {
    let start = parser.cursor();
    let end = trimmed_end(parser, start, end);
    let start_offset = parser.current_offset();
    if start >= end {
        return (
            SyntaxExpressionSlot::Missing,
            SourceRange::new(start_offset, start_offset),
        );
    }
    let end_offset = parser
        .token_at(end - 1)
        .expect("non-empty expression slot retains its terminal token")
        .range()
        .end();
    (
        SyntaxExpressionSlot::Authored,
        SourceRange::new(start_offset, end_offset),
    )
}

fn numeric_sequence_projection(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<(SyntaxNumericSequence, Vec<PendingExpressionComponent>)> {
    let significant = (start..end)
        .filter(|index| {
            parser.token_at(*index).is_some_and(|token| {
                !matches!(
                    token.kind(),
                    SyntaxKind::WhitespaceToken
                        | SyntaxKind::NewlineToken
                        | SyntaxKind::CommentToken
                )
            })
        })
        .collect::<Vec<_>>();
    if significant.is_empty()
        || significant.iter().enumerate().any(|(position, index)| {
            let token = parser
                .token_at(*index)
                .expect("significant token index remains in the shared cursor");
            if position % 2 == 0 {
                token.kind() != SyntaxKind::NumberToken
            } else {
                parser.text_of(token) != ","
            }
        })
    {
        return None;
    }

    let trailing_separator = significant.len() % 2 == 0;
    let mut elements = Vec::new();
    let mut components = Vec::new();
    let mut common_suffix = None;
    let mut common_suffix_range = None;
    let mut recovery = None;

    for index in significant.iter().step_by(2).copied() {
        let token = parser
            .token_at(index)
            .expect("numeric element token remains in the shared cursor");
        let projection = typed_literal(token, parser.text_of(token));
        let ordinal = u32::try_from(elements.len()).ok()?;
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::NumericElement { ordinal },
            token.range(),
        ));
        let suffix_range = projection
            .components()
            .iter()
            .find(|component| component.part() == LiteralLexemePart::Suffix)
            .map(|component| component.range());
        let digit_count = projection.syntax().numeric_digit_count().unwrap_or(0);
        match projection.syntax().value() {
            SyntaxLiteralValue::Integer(integer) => {
                let suffix = integer.suffix();
                elements.push(SyntaxNumericSequenceElement::new(integer.clone()));
                if let Some(suffix) = suffix {
                    match common_suffix {
                        None => {
                            common_suffix = Some(suffix);
                            common_suffix_range = suffix_range;
                        }
                        Some(first) if first != suffix => {
                            recovery = Some(SyntaxNumericSequenceRecovery::ConflictingSuffix {
                                ordinal,
                                first,
                                conflicting: suffix,
                            });
                            break;
                        }
                        Some(_) => {}
                    }
                }
            }
            SyntaxLiteralValue::Invalid(SyntaxLiteralIssue::Integer(issue)) => {
                recovery = Some(SyntaxNumericSequenceRecovery::InvalidElement {
                    ordinal,
                    issue: issue.clone(),
                    digit_count,
                });
                break;
            }
            _ => return None,
        }
    }

    if recovery.is_none() && trailing_separator {
        let ordinal = u32::try_from(elements.len()).ok()?;
        let insertion = parser
            .token_at(end)
            .map_or_else(|| parser.current_offset(), |token| token.range().start());
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::NumericElement { ordinal },
            SourceRange::new(insertion, insertion),
        ));
        recovery = Some(SyntaxNumericSequenceRecovery::MissingFinalElement { ordinal });
    }
    let recovery = recovery.unwrap_or(SyntaxNumericSequenceRecovery::Complete);
    if let Some(range) = common_suffix_range {
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::NumericCommonSuffix,
            range,
        ));
    }
    let sequence = SyntaxNumericSequence::try_new(elements, common_suffix, recovery)
        .expect("lexer-owned numeric projection satisfies sequence invariants");
    Some((sequence, components))
}

pub(super) fn emit_closure(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let owner = parser.start_projected_owner(SyntaxKind::ClosureExpression, role);
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    let opening = parser
        .current()
        .expect("closure dispatch retains its opening delimiter")
        .range();
    let (parameters, mut components, terminator) = if parser.at("||") {
        let delimiter = parser
            .bump()
            .expect("zero-parameter closure retains its paired pipes")
            .range();
        (
            Vec::new(),
            vec![
                PendingExpressionComponent::new(
                    ExpressionComponentRole::ClosureOpenDelimiter,
                    delimiter,
                ),
                PendingExpressionComponent::new(
                    ExpressionComponentRole::ClosureCloseDelimiter,
                    delimiter,
                ),
            ],
            SyntaxClosureTerminator::Closed,
        )
    } else {
        parser.bump();
        let (parameters, mut components) = emit_closure_parameters(parser, end);
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::ClosureOpenDelimiter,
            opening,
        ));
        let terminator = if parser.at("|") {
            let close = parser
                .bump()
                .expect("closure retains its closing pipe")
                .range();
            components.push(PendingExpressionComponent::new(
                ExpressionComponentRole::ClosureCloseDelimiter,
                close,
            ));
            SyntaxClosureTerminator::Closed
        } else {
            let at = parser.current_offset();
            components.push(PendingExpressionComponent::new(
                ExpressionComponentRole::ClosureRecoveryEnd,
                SourceRange::new(at, at),
            ));
            SyntaxClosureTerminator::RecoveredMissing
        };
        (parameters, components, terminator)
    };
    parser.finish();
    parser.bump_trivia();

    let result_type = if parser.at("->") {
        let source = emit_closure_return_type(parser, end);
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::ReturnType,
            source,
        ));
        parser.bump_trivia();
        true
    } else {
        false
    };
    let (body, body_range) = expression_slot(parser, end);
    if parser.at("{") {
        control::emit_block_expression(parser, end, SyntaxRole::Body);
    } else {
        emit_expression(parser, end, SyntaxRole::Body);
    }
    components.push(PendingExpressionComponent::new(
        ExpressionComponentRole::Body,
        body_range,
    ));
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Closure(SyntaxClosureProjection::new(
                parameters,
                result_type,
                body,
                SyntaxClosureSyntax::Pipe { terminator },
            )),
            components,
        ),
    );
    parser.finish();
    CompletedNode { start_event }
}

fn emit_closure_parameters(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
) -> (
    Vec<SyntaxClosureParameterProjection>,
    Vec<PendingExpressionComponent>,
) {
    let close = find_top_level_boundary(parser, parser.cursor(), &["|"]).min(end);
    emit_closure_parameters_until(parser, close)
}

pub(super) fn emit_closure_parameters_until(
    parser: &mut ShadowDocumentParser<'_, '_>,
    close: usize,
) -> (
    Vec<SyntaxClosureParameterProjection>,
    Vec<PendingExpressionComponent>,
) {
    let mut ordinal = 0_u16;
    let mut parameters = Vec::new();
    let mut components = Vec::new();
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at("|") {
            break;
        }
        let parameter_end =
            find_top_level_boundary(parser, parser.cursor(), &[",", "|"]).min(close);
        let type_separator =
            find_top_level_boundary(parser, parser.cursor(), &[":", ",", "|"]).min(parameter_end);
        let parameter_range = token_interval_range(parser, parser.cursor(), parameter_end);
        let pattern_range = token_interval_range(parser, parser.cursor(), type_separator);
        parser.start(SyntaxKind::ClosureParameter, SyntaxRole::Parameter(ordinal));
        emit_pattern(parser, type_separator, SyntaxRole::ParameterPattern);
        bump_until(parser, type_separator);
        let has_type = parser.at(":");
        components.extend([
            PendingExpressionComponent::new(
                ExpressionComponentRole::ClosureParameter {
                    parameter: ordinal,
                    part: SyntaxClosureParameterPart::Whole,
                },
                parameter_range,
            ),
            PendingExpressionComponent::new(
                ExpressionComponentRole::ClosureParameter {
                    parameter: ordinal,
                    part: SyntaxClosureParameterPart::Pattern,
                },
                pattern_range,
            ),
        ]);
        if has_type {
            let colon = parser
                .current()
                .expect("typed closure parameter retains its colon")
                .range();
            parser.bump();
            parser.bump_trivia();
            let type_range = token_interval_range(parser, parser.cursor(), parameter_end);
            emit_type(parser, parameter_end, SyntaxRole::ParameterType);
            bump_until(parser, parameter_end);
            components.extend([
                PendingExpressionComponent::new(
                    ExpressionComponentRole::ClosureParameter {
                        parameter: ordinal,
                        part: SyntaxClosureParameterPart::Colon,
                    },
                    colon,
                ),
                PendingExpressionComponent::new(
                    ExpressionComponentRole::ClosureParameter {
                        parameter: ordinal,
                        part: SyntaxClosureParameterPart::Type,
                    },
                    type_range,
                ),
            ]);
        }
        parser.finish();
        parameters.push(SyntaxClosureParameterProjection::new(has_type));
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            let separator = parser
                .bump()
                .expect("closure parameter separator retains its comma")
                .range();
            if first_significant(parser, parser.cursor(), close).is_some() {
                components.push(PendingExpressionComponent::new(
                    ExpressionComponentRole::ClosureParameterSeparator { following: ordinal },
                    separator,
                ));
            }
        } else {
            break;
        }
    }
    bump_until(parser, close);
    (parameters, components)
}

fn emit_closure_return_type(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) -> SourceRange {
    parser.start(SyntaxKind::ReturnType, SyntaxRole::ReturnType);
    emit_required_punctuation(
        parser,
        SyntaxKind::ThinArrowNode,
        SyntaxRole::Token,
        "->",
        "syntax.return.missing_arrow",
        "authored return type requires `->`",
    );
    parser.bump_trivia();
    let body = find_top_level_boundary(parser, parser.cursor(), &["{"]).min(end);
    let type_end = trimmed_end(parser, parser.cursor(), body);
    let source = token_interval_range(parser, parser.cursor(), type_end);
    emit_type(parser, type_end, SyntaxRole::Type);
    bump_until(parser, body);
    parser.finish();
    source
}

fn token_interval_range(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> SourceRange {
    let start = first_significant(parser, start, end).unwrap_or(end);
    let end = trimmed_end(parser, start, end);
    if start >= end {
        let at = parser
            .offset_at_token_boundary(start)
            .unwrap_or_else(|| parser.source().len());
        return SourceRange::new(at, at);
    }
    SourceRange::new(
        parser
            .token_at(start)
            .expect("component starts at one token")
            .range()
            .start(),
        parser
            .token_at(end - 1)
            .expect("component ends at one token")
            .range()
            .end(),
    )
}

pub(super) fn has_braced_body(parser: &ShadowDocumentParser<'_, '_>, end: usize) -> bool {
    block_open(parser, end).is_some()
}

pub(super) fn is_nominal_record_head(parser: &ShadowDocumentParser<'_, '_>, end: usize) -> bool {
    let Some(open) = block_open(parser, end) else {
        return false;
    };
    (parser.cursor()..open)
        .rev()
        .find_map(|index| {
            let token = parser.token_at(index)?;
            (!matches!(
                token.kind(),
                SyntaxKind::WhitespaceToken | SyntaxKind::NewlineToken | SyntaxKind::CommentToken
            ))
            .then(|| parser.text_of(token))
        })
        .and_then(|name| name.chars().next())
        .is_some_and(char::is_uppercase)
}

pub(super) fn emit_braced_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let close = find_matching_close(parser, parser.cursor() + 1, "{")
        .unwrap_or(end)
        .min(end);
    if looks_like_record_literal(parser, parser.cursor() + 1, close) {
        emit_record_literal(parser, close, role)
    } else {
        control::emit_block_expression(parser, end, role)
    }
}

pub(super) fn emit_record_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let open = block_open(parser, end).unwrap_or(end);
    let (_, path_range) = expression_slot(parser, open);
    let owner = parser.start_projected_owner(SyntaxKind::RecordExpression, role);
    crate::parser::path::emit_path(
        parser,
        open,
        SyntaxRole::Target,
        crate::parser::path::PathSeparatorGrammar::DottedOrQualified,
    );
    bump_until(parser, open);
    let (fields, mut components) = emit_record_fields(parser, end);
    components.insert(
        0,
        PendingExpressionComponent::new(ExpressionComponentRole::RecordPath, path_range),
    );
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Record(fields.into_boxed_slice()),
            components,
        ),
    );
    parser.finish();
    CompletedNode { start_event }
}

fn emit_record_literal(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let owner = parser.start_projected_owner(SyntaxKind::RecordLiteralExpression, role);
    let (fields, components) = emit_record_fields(parser, end);
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::RecordLiteral(fields.into_boxed_slice()),
            components,
        ),
    );
    parser.finish();
    CompletedNode { start_event }
}

fn emit_record_fields(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
) -> (Vec<SyntaxRecordField>, Vec<PendingExpressionComponent>) {
    debug_assert!(parser.at("{"));
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::FieldList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    let mut fields = Vec::new();
    let mut components = Vec::new();
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at("}") {
            break;
        }
        let field_end = record_field_boundary(parser, parser.cursor(), close);
        let field = emit_record_field(parser, field_end, ordinal);
        fields.push(field.0);
        components.extend(field.1);
        bump_until(parser, field_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        } else if parser.cursor() >= close
            || parser.current_kind() != Some(SyntaxKind::NewlineToken)
        {
            break;
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.expression.missing_record_close",
    );
    (fields, components)
}

fn record_field_boundary(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> usize {
    let mut depth = 0_usize;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            return index;
        };
        let text = parser.text_of(token);
        if depth == 0 && (text == "," || token.kind() == SyntaxKind::NewlineToken) {
            return index;
        }
        match text {
            "(" | "[" | "{" | "<" => depth += 1,
            ")" | "]" | "}" | ">" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    end
}

fn emit_record_field(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    ordinal: u16,
) -> (SyntaxRecordField, Vec<PendingExpressionComponent>) {
    let (_, whole_range) = expression_slot(parser, end);
    let field = u32::from(ordinal);
    parser.start(SyntaxKind::RecordField, SyntaxRole::Field(ordinal));
    let separator = find_top_level_boundary(parser, parser.cursor(), &["=", ":"]).min(end);
    let name_end = if separator < end { separator } else { end };
    let (name, name_range) = source_name(parser, parser.cursor(), name_end);
    parser.start(
        if matches!(&name, Err(SyntaxNameIssue::Missing)) {
            SyntaxKind::MissingName
        } else {
            SyntaxKind::NameReference
        },
        SyntaxRole::Name,
    );
    bump_until(parser, trimmed_end(parser, parser.cursor(), name_end));
    parser.finish();
    bump_until(parser, separator);
    let mut components = vec![
        PendingExpressionComponent::new(
            ExpressionComponentRole::RecordField {
                field,
                part: ExpressionRecordFieldPart::Whole,
            },
            whole_range,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RecordField {
                field,
                part: ExpressionRecordFieldPart::Name,
            },
            name_range,
        ),
    ];
    let projection = if separator < end {
        let separator_range = parser
            .current()
            .expect("record separator was found in the token cursor")
            .range();
        parser.bump();
        parser.bump_trivia();
        let (value, value_range) = expression_slot(parser, end);
        emit_expression(parser, end, SyntaxRole::Initializer);
        bump_until(parser, end);
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::RecordField {
                field,
                part: ExpressionRecordFieldPart::Colon,
            },
            separator_range,
        ));
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::RecordField {
                field,
                part: ExpressionRecordFieldPart::Value,
            },
            value_range,
        ));
        SyntaxRecordField::explicit(name, value)
    } else {
        SyntaxRecordField::shorthand(name)
    };
    parser.finish();
    (projection, components)
}

fn source_name(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> (Result<SyntaxName, SyntaxNameIssue>, SourceRange) {
    let end = trimmed_end(parser, start, end);
    let start_offset = parser.current_offset();
    if start >= end {
        return (
            Err(SyntaxNameIssue::Missing),
            SourceRange::new(start_offset, start_offset),
        );
    }
    let end_offset = parser
        .token_at(end - 1)
        .expect("record field name retains its terminal token")
        .range()
        .end();
    let spelling = (start..end)
        .filter_map(|index| parser.token_at(index))
        .map(|token| parser.text_of(token))
        .collect::<String>();
    (
        SyntaxName::try_new(&spelling),
        SourceRange::new(start_offset, end_offset),
    )
}

pub(super) fn emit_computation_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let open = block_open(parser, end).unwrap_or(end);
    let kind = match parser
        .token_at(parser.cursor())
        .map(|token| parser.text_of(token))
    {
        Some("result") => SyntaxComputationBlockKind::Result,
        Some("task") => SyntaxComputationBlockKind::Task,
        Some("seq") => SyntaxComputationBlockKind::Seq,
        Some("stream") => SyntaxComputationBlockKind::Stream,
        _ => unreachable!("computation-block emission requires a selected keyword"),
    };
    let owner = parser.start_projected_owner(SyntaxKind::ComputationBlockExpression, role);
    bump_until(parser, open);
    if parser.at("{") {
        control::emit_block_contents(parser, SyntaxRole::Body);
    }
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(ExpressionProjection::ComputationBlock(kind), Vec::new()),
    );
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn emit_named_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let open = block_open(parser, end).unwrap_or(end);
    let owner = parser.start_projected_owner(SyntaxKind::NamedBlockExpression, role);
    parser.bump();
    parser.bump_trivia();
    let name_start = parser.cursor();
    let name_end = trimmed_end(parser, name_start, open);
    let (name, name_range) = source_name(parser, name_start, name_end);
    if parser.cursor() < open {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        bump_until(parser, name_end);
        parser.finish();
        bump_until(parser, open);
    }
    if parser.at("{") {
        control::emit_block_contents(parser, SyntaxRole::Body);
    }
    let projection = match name {
        Err(crate::name::SyntaxNameIssue::Missing) => {
            PendingExpressionProjection::new(ExpressionProjection::Block, Vec::new())
        }
        name => PendingExpressionProjection::new(
            ExpressionProjection::NamedBlock(name),
            vec![PendingExpressionComponent::new(
                ExpressionComponentRole::Name,
                name_range,
            )],
        ),
    };
    parser.set_expression_projection(owner, projection);
    parser.finish();
    CompletedNode { start_event }
}

/// Emits a named block whose owner has already admitted the current name token.
pub(super) fn emit_owner_named_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let name_token = parser
        .current()
        .expect("a selected owner-named block retains its name token");
    let name_range = name_token.range();
    let name = SyntaxName::try_new(parser.text_of(name_token));
    let open = block_open(parser, end).unwrap_or(end);
    let owner = parser.start_projected_owner(SyntaxKind::NamedBlockExpression, role);

    parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
    parser.bump();
    parser.finish();
    parser.bump_trivia();
    bump_until(parser, open);
    if parser.at("{") {
        control::emit_block_contents(parser, SyntaxRole::Body);
    }

    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::NamedBlock(name),
            vec![PendingExpressionComponent::new(
                ExpressionComponentRole::Name,
                name_range,
            )],
        ),
    );
    parser.finish();
    CompletedNode { start_event }
}

pub(super) fn emit_thread_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) -> CompletedNode {
    let start_event = parser.event_position();
    let open = block_open(parser, end).unwrap_or(end);
    let owner = parser.start_projected_owner(SyntaxKind::ThreadExpression, role);
    parser.bump();
    parser.bump_trivia();
    let mut components = Vec::with_capacity(2);
    let mode = if parser.at("detached") {
        let source = parser
            .current()
            .expect("detached Thread mode retains its token")
            .range();
        parser.bump();
        parser.bump_trivia();
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::ThreadMode,
            source,
        ));
        SyntaxThreadMode::Detached
    } else {
        SyntaxThreadMode::Attached
    };
    let name = if parser.cursor() < open {
        let token = parser
            .current()
            .expect("named Thread retains one header token");
        let source = token.range();
        let name = SyntaxName::try_new(parser.text_of(token));
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::Name,
            source,
        ));
        parser.bump_trivia();
        if parser.cursor() < open {
            parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
            bump_until(parser, trimmed_end(parser, parser.cursor(), open));
            parser.finish();
        }
        bump_until(parser, open);
        Some(name)
    } else {
        None
    };
    if parser.at("{") {
        crate::parser::statement::emit_braced_thread_flow_block_until(
            parser,
            end,
            SyntaxKind::FunctionItem,
            SyntaxKind::Block,
            SyntaxRole::Body,
            "syntax.thread.missing_block_close",
        );
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.thread.missing_body",
            SourceRange::new(at, at),
            "missing Thread body",
        )));
    }
    parser.set_expression_projection(
        owner,
        PendingExpressionProjection::new(
            ExpressionProjection::Thread(SyntaxThreadProjection::new(mode, name)),
            components,
        ),
    );
    parser.finish();
    CompletedNode { start_event }
}

fn block_open(parser: &ShadowDocumentParser<'_, '_>, end: usize) -> Option<usize> {
    let open = find_top_level_boundary(parser, parser.cursor(), &["{"]).min(end);
    (open < end && token_text(parser, open) == Some("{")).then_some(open)
}

fn looks_like_record_literal(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> bool {
    let Some(first) = first_significant(parser, start, end) else {
        return false;
    };
    if token_text(parser, first).is_some_and(crate::parser::statement::is_statement_head) {
        return false;
    }
    let boundary = find_top_level_boundary(parser, first, &["=", ":", ",", ";"]);
    boundary < end && token_text(parser, boundary) != Some(";")
}
