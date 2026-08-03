//! Shared declaration-header grammar over the private document cursor.

use arcweft_id::{PublicId, RetainedIdentityFamily};
use arcweft_source::SourceRange;

use super::cursor::ShadowDocumentParser;
use super::expression::{
    CompletedNode, emit_expression, emit_expression_node, emit_parenthesized_call_tail,
};
use super::lexer::LexToken;
use super::path::{PathSeparatorGrammar, emit_path};
use super::pattern::{emit_method_receiver_pattern, emit_pattern};
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter,
    emit_required_punctuation, expected, find_header_boundary, find_matching_close,
    find_matching_close_before, find_top_level_boundary, first_significant, token_count,
    token_text, trimmed_end,
};
use super::type_ref::emit_type;
use crate::expressions::{
    ExpressionComponentRole, ExpressionProjection, PendingExpressionComponent,
    PendingExpressionProjection,
};
use crate::grammar::attribute_projection::{
    PendingOuterAttributeIssue, PendingOuterAttributeProjection,
};
use crate::grammar::callable_projection::PendingMethodReceiverProjection;
use crate::grammar::contract_projection::{
    PendingFlowContractClauseProjection, PendingFlowContractMode,
};
use crate::grammar::declaration_projection::{
    PendingRetainedHeaderProjection, PendingRetainedName, PendingRetainedPublicId,
    PendingRetainedPublicIdIssue,
};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::grammar::source_projection::PendingVisibilityKind;
use crate::name::SyntaxName;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OuterPrefixKind {
    Documentation,
    Attribute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FixedParameterGrammar {
    /// Every source entry is a typed pattern; a missing annotation is retained
    /// as canonical type recovery.
    TypedPattern,
    /// Typed patterns remain structurally available, but an authored default
    /// is retained only as typed recovery under the parameter owner.
    TypedPatternWithRecoveredDefault {
        diagnostic: &'static str,
        message: &'static str,
    },
    /// Trait and implementation methods additionally admit the receiver forms
    /// owned by their method grammar.
    MethodReceiver,
}

impl FixedParameterGrammar {
    const fn admits_method_receiver(self) -> bool {
        matches!(self, Self::MethodReceiver)
    }

    const fn recovered_default(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::TypedPatternWithRecoveredDefault {
                diagnostic,
                message,
            } => Some((diagnostic, message)),
            Self::TypedPattern | Self::MethodReceiver => None,
        }
    }
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
                    emit_outer_attribute(parser, attribute_ordinal);
                    bump_outer_prefix_line(parser);
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

fn emit_outer_attribute(parser: &mut ShadowDocumentParser<'_, '_>, ordinal: u16) {
    let owner =
        parser.start_projected_owner(SyntaxKind::OuterAttribute, SyntaxRole::Attribute(ordinal));
    parser.bump_trivia();
    debug_assert!(parser.at("#"));
    parser.bump();
    parser.bump_trivia();
    if parser.at("[") {
        emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::OpenBracketNode,
            SyntaxRole::OpenDelimiter,
        );
    }
    parser.bump_trivia();
    let close = find_matching_close(parser, parser.cursor(), "[")
        .unwrap_or_else(|| outer_attribute_line_end(parser));
    let missing_path = parser.current_kind() != Some(SyntaxKind::IdentifierToken);
    emit_path(
        parser,
        close,
        SyntaxRole::Target,
        PathSeparatorGrammar::DottedIdentifiers,
    );
    parser.bump_trivia();
    let projection = if !missing_path && parser.at("(") {
        let (arguments, terminator, components) =
            emit_parenthesized_call_tail(parser, close).into_parts();
        parser.bump_trivia();
        let issue = emit_outer_attribute_recovery(parser, close)
            .then_some(PendingOuterAttributeIssue::InvalidShape);
        PendingOuterAttributeProjection::parenthesized(arguments, terminator, components, issue)
    } else {
        let issue = if missing_path {
            let _ = emit_outer_attribute_recovery(parser, close);
            Some(PendingOuterAttributeIssue::MissingPath)
        } else {
            emit_outer_attribute_recovery(parser, close)
                .then_some(PendingOuterAttributeIssue::InvalidShape)
        };
        PendingOuterAttributeProjection::marker(issue)
    };
    parser.set_attribute_projection(owner, projection);
    bump_until(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.attribute.missing_close",
    );
    parser.finish();
}

fn emit_outer_attribute_recovery(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) -> bool {
    if parser.cursor() >= end {
        return false;
    }
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, end);
    parser.finish();
    let range = SourceRange::new(start, parser.current_offset());
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.attribute.invalid_shape",
        range,
        "an attribute must contain a dotted identifier path and optional parenthesized arguments",
    )));
    true
}

fn outer_attribute_line_end(parser: &ShadowDocumentParser<'_, '_>) -> usize {
    (parser.cursor()..token_count(parser))
        .find(|index| {
            parser
                .token_at(*index)
                .is_some_and(|token| token.kind() == SyntaxKind::NewlineToken)
        })
        .unwrap_or_else(|| token_count(parser))
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
    let start = parser.current_offset();
    let owner = parser.start_projected_owner(SyntaxKind::Visibility, SyntaxRole::Visibility);
    parser.bump();
    let projection = if parser.at("(") {
        emit_scoped_visibility(parser)
    } else {
        PendingVisibilityKind::Public
    };
    if projection == PendingVisibilityKind::Recovery {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.visibility.invalid_scope",
            SourceRange::new(start, parser.current_offset()),
            "visibility must be `pub`, `pub(crate)`, or `pub(super)`",
        )));
    }
    parser.set_visibility_projection(owner, projection);
    parser.finish();
}

fn emit_scoped_visibility(parser: &mut ShadowDocumentParser<'_, '_>) -> PendingVisibilityKind {
    debug_assert!(parser.at("("));
    parser.bump();
    let mut depth = 1_usize;
    let mut selected = None;
    let mut invalid = false;
    let mut closed = false;
    while let Some(text) = parser.current_text() {
        match text {
            "(" => {
                depth += 1;
                invalid = true;
            }
            ")" if depth == 1 => {
                parser.bump();
                closed = true;
                break;
            }
            ")" => {
                depth = depth.saturating_sub(1);
                invalid = true;
            }
            "crate" if depth == 1 && selected.is_none() => {
                selected = Some(PendingVisibilityKind::Crate);
            }
            "super" if depth == 1 && selected.is_none() => {
                selected = Some(PendingVisibilityKind::Super);
            }
            _ if parser.current_kind().is_some_and(is_visibility_trivia) => {}
            _ => invalid = true,
        }
        parser.bump();
    }
    if closed && !invalid {
        selected.unwrap_or(PendingVisibilityKind::Recovery)
    } else {
        PendingVisibilityKind::Recovery
    }
}

fn is_visibility_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::WhitespaceToken
            | SyntaxKind::NewlineToken
            | SyntaxKind::CommentToken
            | SyntaxKind::DocCommentToken
    )
}

pub(super) fn emit_retained_declaration_header<T>(
    parser: &mut ShadowDocumentParser<'_, '_>,
    family: RetainedIdentityFamily,
    emit_family_tail: impl FnOnce(&mut ShadowDocumentParser<'_, '_>) -> T,
) -> T {
    let owner = parser.start_projected_owner(SyntaxKind::DeclarationHeader, SyntaxRole::Element(0));
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
    let public_id = emit_retained_declaration_public_id(parser, family, keyword_range);
    parser.bump_trivia();
    let name = emit_retained_declaration_name(parser);
    parser.bump_trivia();
    let family_tail = emit_family_tail(parser);
    parser.set_retained_header_projection(
        owner,
        PendingRetainedHeaderProjection::new(public_id, name),
    );
    parser.finish();
    family_tail
}

pub(super) fn emit_metric_declaration_header(
    parser: &mut ShadowDocumentParser<'_, '_>,
    emit_kind: impl FnOnce(&mut ShadowDocumentParser<'_, '_>),
    emit_family_tail: impl FnOnce(&mut ShadowDocumentParser<'_, '_>),
) {
    let owner = parser.start_projected_owner(SyntaxKind::DeclarationHeader, SyntaxRole::Element(0));
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
    let public_id =
        emit_retained_declaration_public_id(parser, RetainedIdentityFamily::Metric, keyword_range);
    parser.bump_trivia();
    let name = emit_retained_declaration_name(parser);
    parser.bump_trivia();
    emit_family_tail(parser);
    parser.set_retained_header_projection(
        owner,
        PendingRetainedHeaderProjection::new(public_id, name),
    );
    parser.finish();
}

fn emit_retained_declaration_public_id(
    parser: &mut ShadowDocumentParser<'_, '_>,
    family: RetainedIdentityFamily,
    keyword_range: SourceRange,
) -> PendingRetainedPublicId {
    if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) {
        let token = parser.current().expect("checked declaration ID token");
        let token_text = parser.text_of(token);
        let value = token_text
            .strip_prefix('@')
            .expect("entity-reference token begins with @");

        parser.start(SyntaxKind::DeclarationPublicId, SyntaxRole::PublicId);
        if value.is_empty() {
            parser.start(SyntaxKind::MissingDeclarationId, SyntaxRole::Recovery(0));
            parser.bump();
            parser.finish();
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.declaration.malformed_id",
                token.range(),
                "retained declaration ID is missing after `@`",
            )));
            parser.finish();
            return PendingRetainedPublicId::Recovered {
                issue: PendingRetainedPublicIdIssue::Missing,
                source: token.range(),
            };
        } else if value.starts_with('.') || value.contains(":.") {
            parser.bump();
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.declaration.relative_id",
                token.range(),
                "retained declaration IDs must be plain absolute references",
            )));
            parser.finish();
            return PendingRetainedPublicId::Recovered {
                issue: PendingRetainedPublicIdIssue::Relative,
                source: token.range(),
            };
        } else if value.starts_with('{') || value.contains(':') || value.contains('/') {
            parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
            parser.bump();
            parser.finish();
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.declaration.malformed_id",
                token.range(),
                "retained declaration ID is not a plain absolute public-ID reference",
            )));
            parser.finish();
            return PendingRetainedPublicId::Recovered {
                issue: PendingRetainedPublicIdIssue::Malformed,
                source: token.range(),
            };
        } else {
            match PublicId::try_new(value) {
                Ok(public_id) if family.validate_public_id(&public_id).is_ok() => {
                    parser.bump();
                    parser.finish();
                    return PendingRetainedPublicId::Explicit {
                        value: public_id,
                        source: token.range(),
                    };
                }
                Ok(public_id) => {
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
                    parser.finish();
                    return PendingRetainedPublicId::Recovered {
                        issue: PendingRetainedPublicIdIssue::WrongFamily(public_id),
                        source: token.range(),
                    };
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
                    parser.finish();
                    return PendingRetainedPublicId::Recovered {
                        issue: PendingRetainedPublicIdIssue::Malformed,
                        source: token.range(),
                    };
                }
            }
        }
    }
    PendingRetainedPublicId::Derived
}

fn emit_retained_declaration_name(
    parser: &mut ShadowDocumentParser<'_, '_>,
) -> PendingRetainedName {
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) && !current_name_is_dotted(parser)
    {
        let token = parser
            .current()
            .expect("checked retained declaration name token");
        let name = SyntaxName::try_new(parser.text_of(token))
            .expect("identifier token is a validated declaration name");
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
        return PendingRetainedName::Resolved {
            value: name,
            source: token.range(),
        };
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
        return PendingRetainedName::Missing {
            insertion: SourceRange::new(at, at),
        };
    }

    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_invalid_declaration_name(parser);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.declaration.invalid_name",
        SourceRange::new(at, parser.current_offset()),
        "retained declaration name must be one non-keyword identifier",
    )));
    PendingRetainedName::Invalid {
        insertion: SourceRange::new(at, at),
        recovery: SourceRange::new(at, parser.current_offset()),
    }
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
        let name_is_valid = first
            .and_then(|index| parser.token_at(index))
            .is_some_and(|token| match kind {
                SyntaxKind::LifetimeParameter => token.kind() == SyntaxKind::LifetimeToken,
                SyntaxKind::TypeParameter => token.kind() == SyntaxKind::IdentifierToken,
                _ => false,
            });
        if name_is_valid {
            let name = first.expect("validated generic parameter name position");
            parser.bump_through(name.saturating_sub(1));
            parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
            parser.bump();
            parser.finish();
        } else {
            let at = first
                .and_then(|index| parser.token_at(index))
                .map_or_else(|| parser.current_offset(), |token| token.range().start());
            parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
            parser.push(SyntaxEvent::MissingToken {
                expected: expected(match kind {
                    SyntaxKind::LifetimeParameter => SyntaxKind::LifetimeToken,
                    _ => SyntaxKind::IdentifierToken,
                }),
                at,
            });
            parser.finish();
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.generic.missing_name",
                SourceRange::new(at, at),
                "generic parameter requires a name",
            )));
        }
        parser.bump_trivia();
        match kind {
            SyntaxKind::TypeParameter if parser.at(":") => {
                emit_required_punctuation(
                    parser,
                    SyntaxKind::ColonNode,
                    SyntaxRole::Colon,
                    ":",
                    "syntax.generic.missing_bound_separator",
                    "generic type parameter bounds require `:`",
                );
                parser.bump_trivia();
                emit_generic_bounds(parser, end);
            }
            SyntaxKind::TypeParameter if parser.cursor() < end => {
                let start = parser.current_offset();
                parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
                bump_until(parser, end);
                parser.finish();
                parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                    "syntax.generic.invalid_parameter",
                    SourceRange::new(start, parser.current_offset()),
                    "unexpected syntax after generic type parameter name",
                )));
            }
            SyntaxKind::LifetimeParameter if parser.cursor() < end => {
                let start = parser.current_offset();
                parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
                bump_until(parser, end);
                parser.finish();
                parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                    "syntax.generic.invalid_lifetime_parameter",
                    SourceRange::new(start, parser.current_offset()),
                    "lifetime parameters do not accept inline bounds",
                )));
            }
            _ => {}
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

fn emit_generic_bounds(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        let bound_end = find_top_level_boundary(parser, parser.cursor(), &["+"]).min(end);
        emit_type(parser, bound_end, SyntaxRole::Element(ordinal));
        bump_until(parser, bound_end);
        ordinal = ordinal.saturating_add(1);
        if parser.at("+") && parser.cursor() < end {
            parser.bump();
        } else {
            break;
        }
    }
}

pub(super) fn emit_fixed_parameters(
    parser: &mut ShadowDocumentParser<'_, '_>,
    grammar: FixedParameterGrammar,
    missing_type_message: &'static str,
    missing_close_code: &'static str,
) {
    emit_fixed_parameters_until(
        parser,
        token_count(parser),
        grammar,
        missing_type_message,
        missing_close_code,
    );
}

pub(super) fn emit_fixed_parameters_until(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    grammar: FixedParameterGrammar,
    missing_type_message: &'static str,
    missing_close_code: &'static str,
) {
    parser.start(SyntaxKind::FixedParameterGroup, SyntaxRole::ParameterGroup);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    let close = find_matching_close_before(parser, parser.cursor(), end, "(");
    let content_end = close.unwrap_or(end);
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    while parser.cursor() < content_end {
        parser.bump_trivia();
        if parser.cursor() >= content_end || parser.is_at_end() || parser.at(")") {
            break;
        }
        let parameter_end =
            find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(content_end);
        emit_parameter(
            parser,
            parameter_end,
            ordinal,
            grammar,
            missing_type_message,
        );
        ordinal = ordinal.saturating_add(1);
        if parser.at(",") && parser.cursor() < content_end {
            parser.bump();
        }
    }
    bump_until(parser, content_end);
    parser.finish();
    emit_close_delimiter(parser, SyntaxKind::CloseParenNode, ")", missing_close_code);
    parser.finish();
}

pub(super) fn fixed_parameter_group_end(parser: &ShadowDocumentParser<'_, '_>) -> usize {
    let start = parser.cursor().saturating_add(1);
    if let Some(close) = find_matching_close(parser, start, "(") {
        return close.saturating_add(1);
    }
    let boundary = find_top_level_boundary(
        parser,
        start,
        &[
            "->",
            "where",
            "requires",
            "ensures",
            "invariant",
            "assume",
            "reads",
            "effects",
            "modifies",
            "decreases",
            "{",
        ],
    );
    boundary
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
    grammar: FixedParameterGrammar,
    missing_type_message: &'static str,
) {
    let mut ordinal = 0_u32;
    while parser.at("(") {
        let start = parser.current_offset();
        let end = fixed_parameter_group_end(parser);
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(ordinal));
        emit_fixed_parameters_until(
            parser,
            end,
            grammar,
            missing_type_message,
            "syntax.decl.unclosed_recovered_parameters",
        );
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            match keyword {
                "predicate" => "syntax.predicate.malformed_header",
                "proof" => "syntax.proof.malformed_header",
                "flow" => "flow.signature.curried_flow",
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
    grammar: FixedParameterGrammar,
    missing_type_message: &'static str,
) {
    let parameter_owner =
        parser.start_projected_owner(SyntaxKind::Parameter, SyntaxRole::Parameter(ordinal));
    let colon = find_top_level_boundary(parser, parser.cursor(), &[":"]);
    let colon = (colon < end && token_text(parser, colon) == Some(":")).then_some(colon);
    if colon.is_none()
        && grammar.admits_method_receiver()
        && let Some(receiver) = method_receiver_projection(parser, parser.cursor(), end)
    {
        emit_method_receiver_pattern(parser, end, SyntaxRole::ParameterPattern, &receiver);
        parser.set_method_receiver_projection(parameter_owner, receiver);
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
        if parser.at("...") {
            parser.start(SyntaxKind::RestParameterMarker, SyntaxRole::Kind);
            parser.bump();
            parser.finish();
            parser.bump_trivia();
        }
        let default = find_top_level_boundary(parser, parser.cursor(), &["="]).min(end);
        emit_type(parser, default, SyntaxRole::ParameterType);
        bump_until(parser, default);
        if parser.at("=") && parser.cursor() < end {
            emit_parameter_default(parser, end, grammar);
        }
    } else {
        let at = parser.current_offset();
        emit_type(parser, parser.cursor(), SyntaxRole::ParameterType);
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.parameter.missing_type",
            SourceRange::new(at, at),
            missing_type_message,
        )));
    }
    bump_until(parser, end);
    parser.finish();
}

fn emit_parameter_default(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    grammar: FixedParameterGrammar,
) {
    let start = parser.current_offset();
    let recovered = grammar.recovered_default();
    if recovered.is_some() {
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    }
    parser.start(SyntaxKind::EqualsNode, SyntaxRole::Equals);
    parser.bump();
    parser.finish();
    parser.bump_trivia();
    if parser.cursor() >= end {
        let at = parser.current_offset();
        let owner = parser.start_projected_owner(SyntaxKind::MissingExpression, SyntaxRole::Value);
        parser.set_expression_projection(
            owner,
            PendingExpressionProjection::new(
                ExpressionProjection::Error,
                vec![PendingExpressionComponent::new(
                    ExpressionComponentRole::Recovery,
                    SourceRange::new(at, at),
                )],
            ),
        );
        parser.push(SyntaxEvent::MissingToken {
            expected: expected(SyntaxKind::TextToken),
            at,
        });
        parser.finish();
        if recovered.is_none() {
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.parameter.missing_default",
                SourceRange::new(at, at),
                "parameter default requires an expression",
            )));
        }
    } else {
        emit_expression(parser, end, SyntaxRole::Value);
    }
    if let Some((diagnostic, message)) = recovered {
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            diagnostic,
            SourceRange::new(start, parser.current_offset()),
            message,
        )));
    }
}

fn method_receiver_projection(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<PendingMethodReceiverProjection> {
    let significant = (start..end)
        .filter(|index| {
            parser.token_at(*index).is_some_and(|token| {
                !matches!(
                    token.kind(),
                    SyntaxKind::WhitespaceToken
                        | SyntaxKind::NewlineToken
                        | SyntaxKind::CommentToken
                        | SyntaxKind::DocCommentToken
                )
            })
        })
        .collect::<Vec<_>>();
    let token = |index: usize| parser.token_at(index);
    let spelling = |index: usize| token_text(parser, index);
    let whole = || {
        let first = token(*significant.first()?)?;
        let last = token(*significant.last()?)?;
        Some(SourceRange::new(first.range().start(), last.range().end()))
    };
    match significant.as_slice() {
        [self_index] if spelling(*self_index) == Some("self") => {
            Some(PendingMethodReceiverProjection::Owned {
                whole: whole()?,
                mut_keyword: None,
                self_keyword: token(*self_index)?.range(),
            })
        }
        [mut_index, self_index]
            if spelling(*mut_index) == Some("mut") && spelling(*self_index) == Some("self") =>
        {
            Some(PendingMethodReceiverProjection::Owned {
                whole: whole()?,
                mut_keyword: Some(token(*mut_index)?.range()),
                self_keyword: token(*self_index)?.range(),
            })
        }
        [ampersand_index, self_index]
            if spelling(*ampersand_index) == Some("&") && spelling(*self_index) == Some("self") =>
        {
            Some(PendingMethodReceiverProjection::SharedReference {
                whole: whole()?,
                ampersand: token(*ampersand_index)?.range(),
                self_keyword: token(*self_index)?.range(),
            })
        }
        [ampersand_index, mut_index, self_index]
            if spelling(*ampersand_index) == Some("&")
                && spelling(*mut_index) == Some("mut")
                && spelling(*self_index) == Some("self") =>
        {
            Some(PendingMethodReceiverProjection::MutableReference {
                whole: whole()?,
                ampersand: token(*ampersand_index)?.range(),
                mut_keyword: token(*mut_index)?.range(),
                self_keyword: token(*self_index)?.range(),
            })
        }
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
        emit_type(parser, end, SyntaxRole::LeftOperand);
        bump_until(parser, end);
        emit_required_punctuation(
            parser,
            SyntaxKind::ColonNode,
            SyntaxRole::Colon,
            ":",
            "syntax.where.missing_colon",
            "where predicate requires `:` before its bounds",
        );
        emit_type(parser, end, SyntaxRole::Element(0));
        return;
    }

    emit_type(parser, colon, SyntaxRole::LeftOperand);
    bump_until(parser, colon);
    emit_required_punctuation(
        parser,
        SyntaxKind::ColonNode,
        SyntaxRole::Colon,
        ":",
        "syntax.where.missing_colon",
        "where predicate requires `:` before its bounds",
    );
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= end {
            emit_type(parser, end, SyntaxRole::Element(ordinal));
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContractClauseFamily {
    Requires,
    Ensures,
    Invariant,
    Assume,
    Reads,
    Effects,
    NoEffect,
    Modifies,
    Decreases,
}

impl ContractClauseFamily {
    fn from_flow_keyword(keyword: &str) -> Option<Self> {
        match keyword {
            "requires" => Some(Self::Requires),
            "ensures" => Some(Self::Ensures),
            "invariant" => Some(Self::Invariant),
            "assume" => Some(Self::Assume),
            "reads" => Some(Self::Reads),
            "effects" => Some(Self::Effects),
            "modifies" => Some(Self::Modifies),
            "decreases" => Some(Self::Decreases),
            _ => None,
        }
    }

    const fn syntax_kind(self) -> SyntaxKind {
        match self {
            Self::Requires => SyntaxKind::RequiresClause,
            Self::Ensures => SyntaxKind::EnsuresClause,
            Self::Invariant => SyntaxKind::InvariantClause,
            Self::Assume => SyntaxKind::AssumeClause,
            Self::Reads => SyntaxKind::ReadsClause,
            Self::Effects => SyntaxKind::EffectsClause,
            Self::NoEffect => SyntaxKind::NoEffectClause,
            Self::Modifies => SyntaxKind::ModifiesClause,
            Self::Decreases => SyntaxKind::DecreasesClause,
        }
    }

    const fn admits_mode(self) -> bool {
        matches!(self, Self::Requires | Self::Ensures | Self::Invariant)
    }

    const fn has_operand_list(self) -> bool {
        matches!(self, Self::Reads | Self::Effects | Self::Modifies)
    }
}

/// Emits the maintained requires/ensures sequence shared by ordinary
/// callable declarations. Family meaning stays in the node kind while the
/// one heterogeneous role owns source order.
pub(super) fn emit_callable_contract_clauses(parser: &mut ShadowDocumentParser<'_, '_>) {
    let mut ordinal = 0_u16;
    let mut saw_ensures = false;
    while matches!(parser.current_text(), Some("requires" | "ensures")) {
        let family = if parser.at("requires") {
            ContractClauseFamily::Requires
        } else {
            ContractClauseFamily::Ensures
        };
        if family == ContractClauseFamily::Requires {
            let clause_start = parser.current_offset();
            emit_contract_clause(parser, family, SyntaxRole::ContractClause(ordinal), false);
            if saw_ensures {
                parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                    "syntax.contract.invalid_clause_order",
                    SourceRange::new(clause_start, parser.current_offset()),
                    "`requires` clauses must precede every `ensures` clause",
                )));
            }
        } else {
            saw_ensures = true;
            emit_contract_clause(parser, family, SyntaxRole::ContractClause(ordinal), false);
        }
        ordinal = ordinal
            .checked_add(1)
            .expect("contract clause budget is below the role index range");
        parser.bump_trivia();
    }
}

/// Emits the full heterogeneous Flow contract sequence. Authored order is
/// recorded directly and is never reconstructed from family-specific rows.
pub(super) fn emit_flow_contract_clauses(parser: &mut ShadowDocumentParser<'_, '_>) {
    let mut ordinal = 0_u16;
    while let Some(family) = current_flow_contract_family(parser) {
        emit_contract_clause(parser, family, SyntaxRole::ContractClause(ordinal), true);
        ordinal = ordinal
            .checked_add(1)
            .expect("contract clause budget is below the role index range");
        parser.bump_trivia();
    }
}

fn current_flow_contract_family(
    parser: &ShadowDocumentParser<'_, '_>,
) -> Option<ContractClauseFamily> {
    let family = ContractClauseFamily::from_flow_keyword(parser.current_text()?)?;
    if family == ContractClauseFamily::Ensures && ensures_starts_no_effect(parser) {
        Some(ContractClauseFamily::NoEffect)
    } else {
        Some(family)
    }
}

fn ensures_starts_no_effect(parser: &ShadowDocumentParser<'_, '_>) -> bool {
    let start = parser.cursor().saturating_add(1);
    let end = find_header_boundary(parser, start);
    let Some(next) = first_significant(parser, start, end) else {
        return false;
    };
    !(start..next).any(|index| {
        parser
            .token_at(index)
            .is_some_and(|token| token.kind() == SyntaxKind::NewlineToken)
    }) && token_text(parser, next) == Some("no_effect")
}

fn emit_contract_clause(
    parser: &mut ShadowDocumentParser<'_, '_>,
    family: ContractClauseFamily,
    role: SyntaxRole,
    flow_contract: bool,
) {
    let owner = parser.start_projected_owner(family.syntax_kind(), role);
    let clause_keyword = parser.current().expect("checked contract keyword").range();
    parser.bump();
    let mut payload_starts_on_new_line = bump_contract_trivia(parser);
    let mut mode = PendingFlowContractMode::Default;
    let mut no_effect_keyword = None;
    if family == ContractClauseFamily::NoEffect {
        debug_assert!(parser.at("no_effect"));
        no_effect_keyword = parser.current().map(LexToken::range);
        parser.bump();
        payload_starts_on_new_line |= bump_contract_trivia(parser);
    } else if flow_contract
        && !payload_starts_on_new_line
        && family.admits_mode()
        && matches!(parser.current_text(), Some("prove" | "check" | "debug"))
    {
        // The authored mode remains the direct token owned by this clause.
        // It must not allocate a semantic name identity merely to retain a
        // closed contract-mode spelling.
        let token = parser.current().expect("checked Flow contract mode");
        mode = match parser.text_of(token) {
            "prove" => PendingFlowContractMode::Prove(token.range()),
            "check" => PendingFlowContractMode::Check(token.range()),
            "debug" => PendingFlowContractMode::Debug(token.range()),
            _ => unreachable!("mode guard admits the closed Flow mode vocabulary"),
        };
        parser.bump();
        payload_starts_on_new_line |= bump_contract_trivia(parser);
    }
    if family.has_operand_list() {
        emit_contract_operand_list(parser, payload_starts_on_new_line);
    } else {
        let end = find_header_boundary(parser, parser.cursor());
        emit_contract_operand_until(parser, end, 0);
    }
    if flow_contract {
        parser.set_flow_contract_clause_projection(
            owner,
            PendingFlowContractClauseProjection::new(clause_keyword, mode, no_effect_keyword),
        );
    }
    parser.finish();
}

fn bump_contract_trivia(parser: &mut ShadowDocumentParser<'_, '_>) -> bool {
    let mut saw_newline = false;
    while parser.current_kind().is_some_and(|kind| {
        matches!(
            kind,
            SyntaxKind::WhitespaceToken
                | SyntaxKind::NewlineToken
                | SyntaxKind::CommentToken
                | SyntaxKind::DocCommentToken
        )
    }) {
        saw_newline |= parser.current_kind() == Some(SyntaxKind::NewlineToken);
        parser.bump();
    }
    saw_newline
}

pub(super) fn emit_contract_clause_until(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    kind: SyntaxKind,
    role: SyntaxRole,
) -> CompletedNode {
    parser.start(kind, role);
    parser.bump();
    parser.bump_trivia();
    let condition = emit_contract_operand_until(parser, end, 0);
    parser.finish();
    condition
}

fn emit_contract_operand_until(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    ordinal: u16,
) -> CompletedNode {
    let expression_start = parser.cursor();
    let condition = if trimmed_end(parser, expression_start, end) == expression_start {
        let at = parser.current_offset();
        let start_event = parser.event_position();
        let owner = parser.start_projected_owner(
            SyntaxKind::MissingExpression,
            SyntaxRole::ContractOperand(ordinal),
        );
        parser.set_expression_projection(
            owner,
            PendingExpressionProjection::new(
                ExpressionProjection::Error,
                vec![PendingExpressionComponent::new(
                    ExpressionComponentRole::Recovery,
                    SourceRange::new(at, at),
                )],
            ),
        );
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.contract.missing_expression",
            SourceRange::new(at, at),
            "contract clause requires an expression",
        )));
        CompletedNode { start_event }
    } else {
        emit_expression_node(parser, end, SyntaxRole::ContractOperand(ordinal))
    };
    bump_until(parser, end);
    condition
}

fn emit_contract_operand_list(
    parser: &mut ShadowDocumentParser<'_, '_>,
    payload_starts_on_new_line: bool,
) {
    let clause_end = find_header_boundary(parser, parser.cursor());
    if !parser.at("{") || payload_starts_on_new_line {
        emit_unbraced_contract_operands(parser, clause_end);
        return;
    }

    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let recovery_end = find_flow_contract_list_recovery_boundary(parser, parser.cursor());
    let close = find_matching_close_before(parser, parser.cursor(), recovery_end, "{");
    let list_end = close.unwrap_or(recovery_end);
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.cursor() >= list_end {
            break;
        }
        let operand_end = find_top_level_boundary(parser, parser.cursor(), &[","]).min(list_end);
        emit_contract_operand_until(parser, operand_end, ordinal);
        ordinal = ordinal
            .checked_add(1)
            .expect("contract operand budget is below the role index range");
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    bump_until(parser, list_end);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.contract.missing_list_close",
    );
}

/// Stops an unclosed braced list before the next top-level Flow clause or
/// body. This is a grammar synchronization rule over the current token cursor,
/// not a second parser or source-text reconstruction path.
fn find_flow_contract_list_recovery_boundary(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
) -> usize {
    let end = token_count(parser);
    let mut delimiters = Vec::new();
    let mut line_start = false;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            break;
        };
        if token.kind() == SyntaxKind::NewlineToken {
            if delimiters.is_empty() {
                line_start = true;
            }
            continue;
        }
        if matches!(
            token.kind(),
            SyntaxKind::WhitespaceToken | SyntaxKind::CommentToken | SyntaxKind::DocCommentToken
        ) {
            continue;
        }

        let text = parser.text_of(token);
        if line_start
            && delimiters.is_empty()
            && (text == "{" || ContractClauseFamily::from_flow_keyword(text).is_some())
        {
            return index;
        }
        line_start = false;

        match text {
            "(" | "[" | "{" => delimiters.push(text),
            ")" if delimiters.last() == Some(&"(") => {
                delimiters.pop();
            }
            "]" if delimiters.last() == Some(&"[") => {
                delimiters.pop();
            }
            "}" if delimiters.last() == Some(&"{") => {
                delimiters.pop();
            }
            // A depth-zero close belongs to the already-consumed outer brace;
            // leave it visible to `find_matching_close_before`.
            "}" if delimiters.is_empty() => return end,
            _ => {}
        }
    }
    end
}

fn emit_unbraced_contract_operands(parser: &mut ShadowDocumentParser<'_, '_>, clause_end: usize) {
    if trimmed_end(parser, parser.cursor(), clause_end) == parser.cursor() {
        emit_contract_operand_until(parser, clause_end, 0);
        return;
    }

    let mut ordinal = 0_u16;
    loop {
        let operand_end = find_top_level_boundary(parser, parser.cursor(), &[","]).min(clause_end);
        emit_contract_operand_until(parser, operand_end, ordinal);
        ordinal = ordinal
            .checked_add(1)
            .expect("contract operand budget is below the role index range");
        if parser.at(",") && parser.cursor() < clause_end {
            parser.bump();
            parser.bump_trivia();
        } else {
            break;
        }
    }
}
