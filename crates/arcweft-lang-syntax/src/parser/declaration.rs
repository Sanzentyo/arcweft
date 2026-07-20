//! Shared declaration-header grammar over the private document cursor.

use arcweft_id::{PublicId, RetainedIdentityFamily};
use arcweft_source::SourceRange;

use super::document::ShadowDocumentParser;
use super::expression::emit_expression;
use super::lexer::LexToken;
use super::pattern::emit_pattern;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter, expected,
    find_header_boundary, find_top_level_boundary, first_significant, token_count, token_text,
    trimmed_end,
};
use super::type_ref::emit_type;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OuterPrefixKind {
    Documentation,
    Attribute,
}

pub(super) fn emit_outer_prefixes(parser: &mut ShadowDocumentParser<'_, '_>) {
    let mut attribute_ordinal = 0_u16;
    loop {
        match outer_prefix_kind(parser) {
            Some(OuterPrefixKind::Documentation) => {
                parser.start(SyntaxKind::DocBlock, SyntaxRole::Documentation);
                let mut line_ordinal = 0_u32;
                while outer_prefix_kind(parser) == Some(OuterPrefixKind::Documentation) {
                    parser.start(SyntaxKind::LogicalLine, SyntaxRole::Element(line_ordinal));
                    bump_outer_prefix_line(parser);
                    parser.finish();
                    line_ordinal = line_ordinal.saturating_add(1);
                }
                parser.finish();
            }
            Some(OuterPrefixKind::Attribute) => {
                parser.start(SyntaxKind::AttributeList, SyntaxRole::Element(0));
                let mut line_ordinal = 0_u32;
                while outer_prefix_kind(parser) == Some(OuterPrefixKind::Attribute) {
                    parser.start(SyntaxKind::LogicalLine, SyntaxRole::Element(line_ordinal));
                    parser.start(
                        SyntaxKind::OuterAttribute,
                        SyntaxRole::Attribute(attribute_ordinal),
                    );
                    bump_outer_prefix_line(parser);
                    parser.finish();
                    parser.finish();
                    attribute_ordinal = attribute_ordinal.saturating_add(1);
                    line_ordinal = line_ordinal.saturating_add(1);
                }
                parser.finish();
            }
            None => break,
        }
    }
}

fn outer_prefix_kind(parser: &ShadowDocumentParser<'_, '_>) -> Option<OuterPrefixKind> {
    let mut cursor = parser.cursor();
    loop {
        let token = parser.token_at(cursor)?;
        match token.kind() {
            SyntaxKind::WhitespaceToken => cursor += 1,
            SyntaxKind::DocCommentToken => return Some(OuterPrefixKind::Documentation),
            SyntaxKind::PunctuationToken if parser.text_of(token) == "#" => {
                cursor += 1;
                while parser
                    .token_at(cursor)
                    .is_some_and(|token| token.kind() == SyntaxKind::WhitespaceToken)
                {
                    cursor += 1;
                }
                return parser
                    .token_at(cursor)
                    .filter(|token| parser.text_of(*token) == "[")
                    .map(|_| OuterPrefixKind::Attribute);
            }
            _ => return None,
        }
    }
}

fn bump_outer_prefix_line(parser: &mut ShadowDocumentParser<'_, '_>) {
    let mut delimiter_depth = 0_usize;
    while let Some(token) = parser.current() {
        let is_line_end = token.kind() == SyntaxKind::NewlineToken && delimiter_depth == 0;
        if token.kind() == SyntaxKind::PunctuationToken {
            match parser.text_of(token) {
                "(" | "[" | "{" => delimiter_depth += 1,
                ")" | "]" | "}" => delimiter_depth = delimiter_depth.saturating_sub(1),
                _ => {}
            }
        }
        parser.bump();
        if is_line_end {
            break;
        }
    }
}

pub(super) fn emit_visibility(parser: &mut ShadowDocumentParser<'_, '_>) {
    if !parser.at("pub") {
        return;
    }
    parser.start(SyntaxKind::Visibility, SyntaxRole::Visibility);
    parser.bump();
    if parser.at("(") {
        let mut depth = 0_usize;
        while let Some(text) = parser.current_text() {
            match text {
                "(" => depth += 1,
                ")" if depth == 1 => {
                    parser.bump();
                    break;
                }
                ")" => depth = depth.saturating_sub(1),
                _ => {}
            }
            parser.bump();
        }
    }
    parser.finish();
}

pub(super) fn emit_retained_declaration_header(
    parser: &mut ShadowDocumentParser<'_, '_>,
    family: RetainedIdentityFamily,
    emit_family_tail: impl FnOnce(&mut ShadowDocumentParser<'_, '_>),
) {
    parser.start(SyntaxKind::DeclarationHeader, SyntaxRole::Element(0));
    emit_outer_prefixes(parser);
    parser.bump_trivia();
    emit_visibility(parser);
    parser.bump_trivia();

    let keyword_range = parser
        .current()
        .filter(|token| parser.text_of(*token) == family.prefix())
        .map_or_else(
            || SourceRange::new(parser.current_offset(), parser.current_offset()),
            LexToken::range,
        );
    if parser.at(family.prefix()) {
        parser.bump();
    }
    parser.bump_trivia();
    emit_retained_declaration_public_id(parser, family, keyword_range);
    parser.bump_trivia();
    emit_retained_declaration_name(parser);
    parser.bump_trivia();
    emit_family_tail(parser);
    parser.finish();
}

pub(super) fn emit_metric_declaration_header(
    parser: &mut ShadowDocumentParser<'_, '_>,
    emit_kind: impl FnOnce(&mut ShadowDocumentParser<'_, '_>),
    emit_family_tail: impl FnOnce(&mut ShadowDocumentParser<'_, '_>),
) {
    parser.start(SyntaxKind::DeclarationHeader, SyntaxRole::Element(0));
    emit_outer_prefixes(parser);
    parser.bump_trivia();
    emit_visibility(parser);
    parser.bump_trivia();

    let keyword_range = parser
        .current()
        .filter(|token| parser.text_of(*token) == RetainedIdentityFamily::Metric.prefix())
        .map_or_else(
            || SourceRange::new(parser.current_offset(), parser.current_offset()),
            LexToken::range,
        );
    if parser.at(RetainedIdentityFamily::Metric.prefix()) {
        parser.bump();
    }
    parser.bump_trivia();
    emit_kind(parser);
    parser.bump_trivia();
    emit_retained_declaration_public_id(parser, RetainedIdentityFamily::Metric, keyword_range);
    parser.bump_trivia();
    emit_retained_declaration_name(parser);
    parser.bump_trivia();
    emit_family_tail(parser);
    parser.finish();
}

fn emit_retained_declaration_public_id(
    parser: &mut ShadowDocumentParser<'_, '_>,
    family: RetainedIdentityFamily,
    keyword_range: SourceRange,
) {
    if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) {
        let token = parser.current().expect("checked declaration ID token");
        let token_text = parser.text_of(token);
        let value = token_text
            .strip_prefix('@')
            .expect("entity-reference token begins with @");

        parser.start(SyntaxKind::DeclarationPublicId, SyntaxRole::PublicId);
        if value.starts_with('.') || value.contains(":.") {
            parser.bump();
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.declaration.relative_id",
                token.range(),
                "retained declaration IDs must be plain absolute references",
            )));
        } else if value.starts_with('{') || value.contains(':') || value.contains('/') {
            parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
            parser.bump();
            parser.finish();
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.declaration.malformed_id",
                token.range(),
                "retained declaration ID is not a plain absolute public-ID reference",
            )));
        } else {
            match PublicId::try_new(value) {
                Ok(public_id) if family.validate_public_id(&public_id).is_ok() => {
                    parser.bump();
                }
                Ok(_) => {
                    parser.start(SyntaxKind::WrongFamilyReference, SyntaxRole::Reference(0));
                    parser.bump();
                    parser.finish();
                    parser.push(SyntaxEvent::Diagnostic(
                        PendingSyntaxDiagnostic::new(
                            "syntax.declaration.wrong_family_id",
                            token.range(),
                            format!(
                                "declaration ID must belong to the `{}` family",
                                family.prefix()
                            ),
                        )
                        .with_related_range(keyword_range),
                    ));
                }
                Err(_) => {
                    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
                    parser.bump();
                    parser.finish();
                    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                        "syntax.declaration.malformed_id",
                        token.range(),
                        "retained declaration ID is malformed",
                    )));
                }
            }
        }
        parser.finish();
        return;
    }

    if !parser.at("@") {
        return;
    }
    let start = parser.current_offset();
    parser.start(SyntaxKind::DeclarationPublicId, SyntaxRole::PublicId);
    parser.start(SyntaxKind::MissingDeclarationId, SyntaxRole::Recovery(0));
    bump_contiguous_declaration_spelling(parser);
    parser.finish();
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.declaration.malformed_id",
        SourceRange::new(start, parser.current_offset()),
        "retained declaration ID is malformed",
    )));
}

fn emit_retained_declaration_name(parser: &mut ShadowDocumentParser<'_, '_>) {
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) && !current_name_is_dotted(parser)
    {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
        return;
    }

    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.finish();

    if parser.is_at_end()
        || matches!(
            parser.current_text(),
            Some("(" | ":" | "{" | ";" | "\r" | "\n")
        )
    {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.declaration.missing_name",
            SourceRange::new(at, at),
            "retained declaration requires one ordinary local name",
        )));
        return;
    }

    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_invalid_declaration_name(parser);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.declaration.invalid_name",
        SourceRange::new(at, parser.current_offset()),
        "retained declaration name must be one non-keyword identifier",
    )));
}

fn current_name_is_dotted(parser: &ShadowDocumentParser<'_, '_>) -> bool {
    let Some(current) = parser.current() else {
        return false;
    };
    parser.token_at(parser.cursor() + 1).is_some_and(|next| {
        next.range().start() == current.range().end() && parser.text_of(next) == "."
    })
}

fn bump_invalid_declaration_name(parser: &mut ShadowDocumentParser<'_, '_>) {
    let Some(first) = parser.bump() else {
        return;
    };
    let mut end = first.range().end();
    while parser.current().is_some_and(|token| {
        token.range().start() == end
            && (parser.text_of(token) == "." || token.kind() == SyntaxKind::IdentifierToken)
    }) {
        end = parser
            .bump()
            .expect("checked invalid-name continuation token")
            .range()
            .end();
    }
}

fn bump_contiguous_declaration_spelling(parser: &mut ShadowDocumentParser<'_, '_>) {
    let mut end = parser.current_offset();
    while let Some(token) = parser.current() {
        if token.range().start() != end
            || matches!(
                token.kind(),
                SyntaxKind::WhitespaceToken
                    | SyntaxKind::NewlineToken
                    | SyntaxKind::CommentToken
                    | SyntaxKind::DocCommentToken
            )
        {
            break;
        }
        end = token.range().end();
        parser.bump();
    }
}

pub(super) fn emit_name(parser: &mut ShadowDocumentParser<'_, '_>, keyword: &str) {
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
        return;
    }

    parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
    let at = parser.current_offset();
    parser.push(SyntaxEvent::MissingToken {
        expected: expected(SyntaxKind::IdentifierToken),
        at,
    });
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        match keyword {
            "predicate" => "syntax.predicate.missing_name",
            "proof" => "syntax.proof.missing_name",
            _ => "syntax.decl.missing_name",
        },
        SourceRange::new(at, at),
        format!("missing ordinary name after `{keyword}`"),
    )));
    parser.finish();
}

pub(super) fn emit_generic_parameters(parser: &mut ShadowDocumentParser<'_, '_>) {
    parser.start(SyntaxKind::GenericParameterGroup, SyntaxRole::GenericGroup);
    emit_open_delimiter(parser, SyntaxKind::OpenAngleNode, "<");
    parser.start(SyntaxKind::GenericParameterList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.is_at_end() || parser.at(">") {
            break;
        }
        let end = find_top_level_boundary(parser, parser.cursor(), &[",", ">"]);
        let first = first_significant(parser, parser.cursor(), end);
        let kind = first.and_then(|index| parser.token_at(index)).map_or(
            SyntaxKind::TypeParameter,
            |token| {
                if token.kind() == SyntaxKind::LifetimeToken {
                    SyntaxKind::LifetimeParameter
                } else {
                    SyntaxKind::TypeParameter
                }
            },
        );
        parser.start(
            SyntaxKind::GenericParameter,
            SyntaxRole::GenericParameter(ordinal),
        );
        parser.start(kind, SyntaxRole::Element(0));
        if let Some(name) = first {
            parser.bump_through(name.saturating_sub(1));
            parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
            parser.bump();
            parser.finish();
        }
        bump_until(parser, end);
        parser.finish();
        parser.finish();
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseAngleNode,
        ">",
        "syntax.generic.missing_close",
    );
    parser.finish();
}

pub(super) fn emit_fixed_parameters(
    parser: &mut ShadowDocumentParser<'_, '_>,
    missing_type_message: &'static str,
    missing_close_code: &'static str,
) {
    parser.start(SyntaxKind::FixedParameterGroup, SyntaxRole::ParameterGroup);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.is_at_end() || parser.at(")") {
            break;
        }
        let end = find_top_level_boundary(parser, parser.cursor(), &[",", ")"]);
        emit_parameter(parser, end, ordinal, missing_type_message);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") {
            parser.bump();
        }
    }
    parser.finish();
    emit_close_delimiter(parser, SyntaxKind::CloseParenNode, ")", missing_close_code);
    parser.finish();
}

pub(super) fn emit_missing_parameter_group(
    parser: &mut ShadowDocumentParser<'_, '_>,
    keyword: &str,
    requirement: &str,
) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::FixedParameterGroup, SyntaxRole::ParameterGroup);
    emit_missing_delimiter(parser, SyntaxKind::OpenParenNode, SyntaxRole::OpenDelimiter);
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    parser.finish();
    emit_missing_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        SyntaxRole::CloseDelimiter,
    );
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        match keyword {
            "predicate" => "syntax.predicate.missing_parameters",
            "proof" => "syntax.proof.missing_parameters",
            _ => "syntax.decl.invalid_header",
        },
        SourceRange::new(at, at),
        format!("`{keyword}` requires {requirement}"),
    )));
    parser.finish();
}

pub(super) fn emit_extra_parameter_group_recovery(
    parser: &mut ShadowDocumentParser<'_, '_>,
    keyword: &str,
) {
    let mut ordinal = 0_u32;
    while parser.at("(") {
        let start = parser.current_offset();
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(ordinal));
        parser.bump();
        let close = super::shadow_recovery::find_matching_close(parser, parser.cursor(), "(");
        if let Some(close) = close {
            bump_until(parser, close + 1);
        } else {
            bump_until(parser, token_count(parser));
        }
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            match keyword {
                "predicate" => "syntax.predicate.malformed_header",
                "proof" => "syntax.proof.malformed_header",
                _ => "syntax.decl.invalid_header",
            },
            SourceRange::new(start, parser.current_offset()),
            format!("`{keyword}` accepts exactly one fixed parameter group"),
        )));
        ordinal = ordinal.saturating_add(1);
        parser.bump_trivia();
    }
}

fn emit_parameter(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    ordinal: u16,
    missing_type_message: &'static str,
) {
    parser.start(SyntaxKind::Parameter, SyntaxRole::Parameter(ordinal));
    let colon = find_top_level_boundary(parser, parser.cursor(), &[":"]);
    let colon = (colon < end && token_text(parser, colon) == Some(":")).then_some(colon);
    if let (None, Some(kind)) = (colon, receiver_pattern_kind(parser, parser.cursor(), end)) {
        parser.start(kind, SyntaxRole::ParameterPattern);
        bump_until(parser, end);
        parser.finish();
        parser.finish();
        return;
    }
    let pattern_end = colon.unwrap_or(end);
    emit_pattern(parser, pattern_end, SyntaxRole::ParameterPattern);
    bump_until(parser, pattern_end);
    if let Some(colon) = colon {
        debug_assert_eq!(parser.cursor(), colon);
        parser.bump();
        parser.bump_trivia();
        emit_type(parser, end, SyntaxRole::ParameterType);
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingType, SyntaxRole::ParameterType);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.parameter.missing_type",
            SourceRange::new(at, at),
            missing_type_message,
        )));
    }
    bump_until(parser, end);
    parser.finish();
}

fn receiver_pattern_kind(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<SyntaxKind> {
    let spellings = (start..end)
        .filter_map(|index| {
            let token = parser.token_at(index)?;
            (!matches!(
                token.kind(),
                SyntaxKind::WhitespaceToken
                    | SyntaxKind::NewlineToken
                    | SyntaxKind::CommentToken
                    | SyntaxKind::DocCommentToken
            ))
            .then(|| parser.text_of(token))
        })
        .collect::<Vec<_>>();
    match spellings.as_slice() {
        ["self"] | ["&", "self"] | ["&", "mut", "self"] => Some(SyntaxKind::BindingPattern),
        ["mut", "self"] => Some(SyntaxKind::MutableBindingPattern),
        _ => None,
    }
}

pub(super) fn emit_return_type(parser: &mut ShadowDocumentParser<'_, '_>, item_kind: SyntaxKind) {
    let start = parser.current_offset();
    parser.start(SyntaxKind::ReturnType, SyntaxRole::ReturnType);
    parser.bump();
    parser.bump_trivia();
    let end = find_header_boundary(parser, parser.cursor());
    emit_type(parser, end, SyntaxRole::Type);
    bump_until(parser, end);
    parser.finish();
    if item_kind == SyntaxKind::PredicateItem {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.predicate.return_not_allowed",
            SourceRange::new(start, parser.current_offset()),
            "predicates have an implicit `Bool` return type",
        )));
    }
}

pub(super) fn emit_where_clause(parser: &mut ShadowDocumentParser<'_, '_>) {
    parser.start(SyntaxKind::WhereClause, SyntaxRole::WhereClause);
    parser.bump();
    parser.bump_trivia();
    let clause_end = find_header_boundary(parser, parser.cursor());
    parser.start(SyntaxKind::WherePredicateList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    while parser.cursor() < clause_end {
        parser.bump_trivia();
        if parser.cursor() >= clause_end {
            break;
        }
        let end = find_top_level_boundary(parser, parser.cursor(), &[","]).min(clause_end);
        parser.start(
            SyntaxKind::WherePredicate,
            SyntaxRole::WherePredicate(ordinal),
        );
        emit_where_predicate_children(parser, trimmed_end(parser, parser.cursor(), end));
        parser.finish();
        bump_until(parser, end);
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") && parser.cursor() < clause_end {
            parser.bump();
        }
    }
    parser.finish();
    bump_until(parser, clause_end);
    parser.finish();
}

fn emit_where_predicate_children(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    let colon = find_top_level_boundary(parser, parser.cursor(), &[":"]).min(end);
    if colon == end {
        emit_type(parser, end, SyntaxRole::Type);
        return;
    }

    emit_type(parser, colon, SyntaxRole::LeftOperand);
    bump_until(parser, colon);
    parser.bump();
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= end {
            break;
        }
        let bound_end = find_top_level_boundary(parser, parser.cursor(), &["+"]).min(end);
        emit_type(parser, bound_end, SyntaxRole::Element(ordinal));
        bump_until(parser, bound_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at("+") {
            parser.bump();
        } else {
            break;
        }
    }
}

pub(super) fn emit_contract_clauses(parser: &mut ShadowDocumentParser<'_, '_>) {
    let mut requires = 0_u16;
    let mut ensures = 0_u16;
    let mut saw_ensures = false;
    while matches!(parser.current_text(), Some("requires" | "ensures")) {
        if parser.at("requires") {
            let clause_start = parser.current_offset();
            emit_contract_clause(
                parser,
                SyntaxKind::RequiresClause,
                SyntaxRole::RequiresClause(requires),
            );
            if saw_ensures {
                parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                    "syntax.contract.invalid_clause_order",
                    SourceRange::new(clause_start, parser.current_offset()),
                    "`requires` clauses must precede every `ensures` clause",
                )));
            }
            requires = requires.saturating_add(1);
        } else {
            saw_ensures = true;
            emit_contract_clause(
                parser,
                SyntaxKind::EnsuresClause,
                SyntaxRole::EnsuresClause(ensures),
            );
            ensures = ensures.saturating_add(1);
        }
        parser.bump_trivia();
    }
}

fn emit_contract_clause(
    parser: &mut ShadowDocumentParser<'_, '_>,
    kind: SyntaxKind,
    role: SyntaxRole,
) {
    parser.start(kind, role);
    parser.bump();
    parser.bump_trivia();
    let end = find_header_boundary(parser, parser.cursor());
    let expression_start = parser.cursor();
    emit_expression(parser, end, SyntaxRole::Condition);
    if trimmed_end(parser, expression_start, end) == expression_start {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.contract.missing_expression",
            SourceRange::new(at, at),
            "contract clause requires an expression",
        )));
    }
    bump_until(parser, end);
    parser.finish();
}
