//! Native Style rule, selector, and property grammar.

use super::{
    DocumentParser, PendingStylePredicate, PendingStylePropertyProjection,
    PendingStyleRuleProjection, PendingStyleSelectorPart, PendingStyleSelectorProjection,
    PendingStyleSelectorRelation, PendingStyleSelectorSequence, PendingSyntaxDiagnostic,
    SourceRange, StylePropertyOperation, StyleSelectorRelation, SyntaxEvent, SyntaxKind,
    SyntaxRole, bump_member_separators, bump_selector_trivia, bump_trivia_before, bump_until,
    emit_assignment, emit_close_delimiter, emit_expression, emit_open_delimiter, emit_style_name,
    expected, find_matching_close, find_top_level_boundary, member_boundary, next_nontrivia,
    selector_sequence_end, token_range,
};

pub(super) fn emit_rule(
    parser: &mut DocumentParser<'_, '_>,
    close: usize,
    source_ordinal: u32,
) -> PendingStyleRuleProjection {
    let start = parser.cursor();
    let open = find_top_level_boundary(parser, start, close, &["{"]);
    parser.start(SyntaxKind::StyleRule, SyntaxRole::Element(source_ordinal));
    let selector = emit_selector(parser, open);
    bump_until(parser, open);
    let (declarations, body_closed) = emit_style_rule_body(parser, close);
    parser.finish();
    PendingStyleRuleProjection {
        source_ordinal,
        selector,
        declarations: declarations.into_boxed_slice(),
        body_closed,
    }
}

fn emit_selector(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
) -> PendingStyleSelectorProjection {
    parser.start(SyntaxKind::StyleSelector, SyntaxRole::Target);
    let mut sequences = Vec::new();
    let mut recovery_count = 0_u32;
    let mut previous = false;
    let mut relation = None;

    while parser.cursor() < end {
        let trivia = bump_selector_trivia(parser, end);
        if parser.cursor() >= end {
            break;
        }
        if parser.at(">") {
            let source = parser.current().expect("selector combinator").range();
            let next = next_nontrivia(parser, parser.cursor() + 1, end);
            if !previous
                || next.is_none_or(|index| {
                    parser
                        .token_at(index)
                        .is_some_and(|token| parser.text_of(token) == ">")
                })
            {
                parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(recovery_count));
                parser.bump();
                parser.finish();
                parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                    "syntax.style.invalid_selector_combinator",
                    source,
                    "style selector child combinator requires a sequence on both sides",
                )));
                recovery_count = recovery_count.saturating_add(1);
                continue;
            }
            parser.bump();
            relation = Some(PendingStyleSelectorRelation {
                value: StyleSelectorRelation::Child,
                source,
            });
            continue;
        }
        if previous && relation.is_none() {
            relation = trivia.map(|source| PendingStyleSelectorRelation {
                value: StyleSelectorRelation::Descendant,
                source,
            });
        }
        let sequence_end = selector_sequence_end(parser, end);
        let source_ordinal = u32::try_from(sequences.len()).unwrap_or(u32::MAX);
        let sequence =
            emit_selector_sequence(parser, sequence_end, source_ordinal, relation.take());
        sequences.push(sequence);
        previous = true;
    }

    let missing = sequences.is_empty();
    if missing {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::IdentifierToken),
            at,
        });
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.missing_selector",
            SourceRange::new(at, at),
            "style rule requires a selector before `{`",
        )));
    }
    parser.finish();
    PendingStyleSelectorProjection {
        sequences: sequences.into_boxed_slice(),
        recovery_count,
        missing,
    }
}

fn emit_selector_sequence(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    source_ordinal: u32,
    relation: Option<PendingStyleSelectorRelation>,
) -> PendingStyleSelectorSequence {
    parser.start(
        SyntaxKind::StyleSelectorSequence,
        SyntaxRole::Element(source_ordinal),
    );
    let mut element = None;
    let mut part = None;
    let mut predicates = Vec::new();
    let mut has_recovery = false;

    if parser.at(".") {
        let separator = parser.current().expect("selector part separator").range();
        parser.bump();
        part = Some(PendingStyleSelectorPart {
            separator,
            name: emit_style_name(
                parser,
                end,
                SyntaxKind::NameReference,
                SyntaxRole::Target,
                false,
                "syntax.style.selector_part",
                "style selector part requires a name",
            ),
        });
    } else {
        element = Some(emit_style_name(
            parser,
            end,
            SyntaxKind::NameReference,
            SyntaxRole::Name,
            false,
            "syntax.style.selector_element",
            "style selector sequence requires an element or part",
        ));
        if parser.at(".") {
            let separator = parser.current().expect("selector part separator").range();
            parser.bump();
            part = Some(PendingStyleSelectorPart {
                separator,
                name: emit_style_name(
                    parser,
                    end,
                    SyntaxKind::NameReference,
                    SyntaxRole::Target,
                    false,
                    "syntax.style.selector_part",
                    "style selector part requires a name",
                ),
            });
        }
    }

    while parser.cursor() < end && parser.at(":") {
        let colon = parser.current().expect("selector predicate colon").range();
        parser.bump();
        let ordinal = u16::try_from(predicates.len()).unwrap_or(u16::MAX);
        predicates.push(PendingStylePredicate {
            source_ordinal: ordinal,
            colon,
            name: emit_style_name(
                parser,
                end,
                SyntaxKind::NameReference,
                SyntaxRole::Label(ordinal),
                false,
                "syntax.style.selector_predicate",
                "style selector predicate requires a name",
            ),
        });
    }

    if parser.cursor() < end {
        let recovery = token_range(parser, parser.cursor(), end);
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        bump_until(parser, end);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.style.invalid_selector",
            recovery,
            "style selector contains an invalid sequence suffix",
        )));
        has_recovery = true;
    }
    parser.finish();
    PendingStyleSelectorSequence {
        source_ordinal,
        relation,
        element,
        part,
        predicates: predicates.into_boxed_slice(),
        has_recovery,
    }
}

fn emit_style_rule_body(
    parser: &mut DocumentParser<'_, '_>,
    enclosing_close: usize,
) -> (Vec<PendingStylePropertyProjection>, bool) {
    parser.start(SyntaxKind::StyleBody, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let end = find_matching_close(parser, parser.cursor(), "{")
        .unwrap_or(enclosing_close)
        .min(enclosing_close);
    parser.start(SyntaxKind::FieldList, SyntaxRole::Element(0));
    let mut declarations = Vec::new();
    while parser.cursor() < end {
        bump_member_separators(parser, end);
        if parser.cursor() >= end {
            break;
        }
        let member_end = member_boundary(parser, parser.cursor(), end);
        let source_ordinal = u32::try_from(declarations.len()).unwrap_or(u32::MAX);
        declarations.push(emit_property_declaration(
            parser,
            member_end,
            source_ordinal,
        ));
        bump_until(parser, member_end);
    }
    parser.finish();
    bump_until(parser, end);
    let closed = parser.at("}");
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.style.missing_rule_close",
    );
    parser.finish();
    (declarations, closed)
}

fn emit_property_declaration(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    source_ordinal: u32,
) -> PendingStylePropertyProjection {
    parser.start(
        SyntaxKind::StylePropertyDeclaration,
        SyntaxRole::Element(source_ordinal),
    );
    let append_keyword = if parser.at("append") {
        let source = parser.current().expect("append keyword").range();
        parser.start(SyntaxKind::NameReference, SyntaxRole::Kind);
        parser.bump();
        parser.finish();
        bump_trivia_before(parser, end);
        Some(source)
    } else {
        None
    };
    let name = emit_style_name(
        parser,
        end,
        SyntaxKind::NameDefinition,
        SyntaxRole::Name,
        true,
        "syntax.style.member_name",
        "style property requires a name",
    );
    bump_trivia_before(parser, end);
    let assignment = emit_assignment(parser, "syntax.style.property_initializer");
    bump_trivia_before(parser, end);
    emit_expression(parser, end, SyntaxRole::Initializer);
    bump_until(parser, end);
    parser.finish();
    PendingStylePropertyProjection {
        source_ordinal,
        name,
        operation: if append_keyword.is_some() {
            StylePropertyOperation::Append
        } else {
            StylePropertyOperation::Replace
        },
        append_keyword,
        assignment,
    }
}
