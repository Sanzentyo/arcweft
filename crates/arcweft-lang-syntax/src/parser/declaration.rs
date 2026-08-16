//! Shared declaration-header grammar over the private document cursor.

use arcweft_id::{DeclarationIdentityFamily, PublicId};
use arcweft_source::SourceRange;

use super::cursor::DocumentParser;
use super::expression::{
    CompletedNode, emit_expression, emit_expression_node, emit_parenthesized_call_tail,
};
use super::lexer::{LexToken, typed_entity_reference};
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
    PendingExpressionProjection, SyntaxCallArgumentPart, SyntaxCallArgumentProjection,
};
use crate::grammar::attribute_projection::{
    PendingOuterAttributeIssue, PendingOuterAttributeProjection,
};
use crate::grammar::budget::GrammarBudget;
use crate::grammar::callable_projection::PendingMethodReceiverProjection;
use crate::grammar::contract_projection::{
    PendingFlowContractClauseProjection, PendingFlowContractMode,
};
use crate::grammar::declaration_projection::{
    PendingDeclarationHeaderProjection, PendingDeclarationName, PendingDeclarationPublicId,
    PendingDeclarationPublicIdIssue,
};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::grammar::source_projection::PendingVisibilityKind;
use crate::grammar::source_projection::{
    PendingPathProjection, PendingPathRoot, PendingPathSegmentKind,
};
use crate::id_ref::AuthoredIdRoot;
use crate::literal::SyntaxLiteralValue;
use crate::name::SyntaxName;

use super::recovery::ParseErrorKind;

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

pub(super) fn emit_outer_prefixes(parser: &mut DocumentParser<'_, '_>) {
    emit_outer_prefixes_for_owner(parser, OuterAttributeOwner::Other);
}

/// Emits the outer prefix of a Proof declaration while selecting
/// `verify.trusted` exactly once from the ordinary attribute grammar.
pub(super) fn emit_proof_outer_prefixes(parser: &mut DocumentParser<'_, '_>) {
    emit_outer_prefixes_for_owner(parser, OuterAttributeOwner::Proof);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OuterAttributeOwner {
    Proof,
    Other,
}

fn emit_outer_prefixes_for_owner(
    parser: &mut DocumentParser<'_, '_>,
    attribute_owner: OuterAttributeOwner,
) {
    let mut attribute_ordinal = 0_u16;
    let mut first_trusted_attribute = None;
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
                    let trusted_attribute = emit_attribute(
                        parser,
                        SyntaxKind::OuterAttribute,
                        SyntaxRole::Attribute(attribute_ordinal),
                        attribute_owner,
                        first_trusted_attribute,
                    );
                    first_trusted_attribute = first_trusted_attribute.or(trusted_attribute);
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

pub(super) fn emit_inner_attribute(
    source: &str,
    tokens: &[LexToken],
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    let mut parser = DocumentParser::new(source, tokens, events, budget);
    emit_attribute(
        &mut parser,
        SyntaxKind::InnerAttribute,
        role,
        OuterAttributeOwner::Other,
        None,
    );
    while parser.bump().is_some() {}
}

fn emit_attribute(
    parser: &mut DocumentParser<'_, '_>,
    kind: SyntaxKind,
    role: SyntaxRole,
    attribute_owner: OuterAttributeOwner,
    first_trusted_attribute: Option<SourceRange>,
) -> Option<SourceRange> {
    debug_assert!(matches!(
        kind,
        SyntaxKind::InnerAttribute | SyntaxKind::OuterAttribute
    ));
    let attribute_start = parser.current_offset();
    let owner = parser.start_projected_owner(kind, role);
    parser.bump_trivia();
    debug_assert!(parser.at("#"));
    parser.bump();
    parser.bump_trivia();
    if kind == SyntaxKind::InnerAttribute {
        debug_assert!(parser.at("!"));
        parser.bump();
        parser.bump_trivia();
    }
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
        .unwrap_or_else(|| attribute_line_end(parser));
    let missing_path = parser.current_kind() != Some(SyntaxKind::IdentifierToken);
    let path = emit_path(
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
        let issue = emit_attribute_recovery(parser, close)
            .then_some(PendingOuterAttributeIssue::InvalidShape);
        PendingOuterAttributeProjection::parenthesized(arguments, terminator, components, issue)
    } else {
        let issue = if missing_path {
            let _ = emit_attribute_recovery(parser, close);
            Some(PendingOuterAttributeIssue::MissingPath)
        } else {
            emit_attribute_recovery(parser, close)
                .then_some(PendingOuterAttributeIssue::InvalidShape)
        };
        PendingOuterAttributeProjection::marker(issue)
    };
    parser.set_attribute_projection(owner, projection.clone());
    bump_until(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.attribute.missing_close",
    );
    let attribute_range = SourceRange::new(attribute_start, parser.current_offset());
    let trusted_attribute = is_verify_trusted_path(parser, &path).then_some(attribute_range);
    if trusted_attribute.is_some() {
        emit_trusted_attribute_diagnostics(
            parser,
            attribute_owner,
            first_trusted_attribute,
            attribute_range,
            &projection,
        );
    }
    parser.finish();
    trusted_attribute
}

fn is_verify_trusted_path(parser: &DocumentParser<'_, '_>, path: &PendingPathProjection) -> bool {
    matches!(path.root(), PendingPathRoot::ImplicitCrate)
        && matches!(
            path.segments(),
            [verify, trusted]
                if verify.kind() == PendingPathSegmentKind::Identifier
                    && trusted.kind() == PendingPathSegmentKind::Identifier
                    && &parser.source()[verify.source().as_range()] == "verify"
                    && &parser.source()[trusted.source().as_range()] == "trusted"
        )
}

#[allow(
    clippy::too_many_lines,
    reason = "the ordered trusted-attribute diagnostic matrix is one closed grammar validation"
)]
fn emit_trusted_attribute_diagnostics(
    parser: &mut DocumentParser<'_, '_>,
    owner: OuterAttributeOwner,
    first_trusted_attribute: Option<SourceRange>,
    attribute_range: SourceRange,
    projection: &PendingOuterAttributeProjection,
) {
    if owner == OuterAttributeOwner::Other {
        emit_proof_trusted_diagnostic(
            parser,
            ParseErrorKind::ProofTrustedNotProof,
            attribute_range,
        );
        return;
    }
    if let Some(first) = first_trusted_attribute {
        parser.push(SyntaxEvent::Diagnostic(
            PendingSyntaxDiagnostic::new(
                ParseErrorKind::ProofTrustedDuplicate.code(),
                attribute_range,
                ParseErrorKind::ProofTrustedDuplicate.label(),
            )
            .with_related_range(first)
            .with_related_message("the first trusted proof attribute is here"),
        ));
        return;
    }

    let crate::grammar::attribute_projection::PendingOuterAttributeForm::Parenthesized {
        arguments,
        terminator: _,
    } = projection.form()
    else {
        emit_proof_trusted_diagnostic(
            parser,
            ParseErrorKind::ProofTrustedReasonMissing,
            attribute_range,
        );
        return;
    };
    if arguments.is_empty() {
        emit_proof_trusted_diagnostic(
            parser,
            ParseErrorKind::ProofTrustedReasonMissing,
            attribute_range,
        );
        return;
    }

    let mut saw_reason = false;
    for (ordinal, argument) in arguments.iter().enumerate() {
        let ordinal = u16::try_from(ordinal).expect("attribute argument limit fits u16");
        let whole = trusted_argument_component(projection, ordinal, SyntaxCallArgumentPart::Whole)
            .unwrap_or(attribute_range);
        match argument {
            SyntaxCallArgumentProjection::Positional { .. }
            | SyntaxCallArgumentProjection::Spread { .. } => emit_proof_trusted_diagnostic(
                parser,
                ParseErrorKind::ProofTrustedPositionalArgument,
                whole,
            ),
            SyntaxCallArgumentProjection::Named { name, value, .. } => {
                let name_range =
                    trusted_argument_component(projection, ordinal, SyntaxCallArgumentPart::Name)
                        .unwrap_or(whole);
                if !matches!(name, Ok(name) if name.as_str() == "reason") {
                    emit_proof_trusted_diagnostic(
                        parser,
                        ParseErrorKind::ProofTrustedUnknownArgument,
                        name_range,
                    );
                    continue;
                }
                if saw_reason {
                    emit_proof_trusted_diagnostic(
                        parser,
                        ParseErrorKind::ProofTrustedReasonDuplicate,
                        name_range,
                    );
                    continue;
                }
                saw_reason = true;
                let value_range =
                    trusted_argument_component(projection, ordinal, SyntaxCallArgumentPart::Value)
                        .unwrap_or(whole);
                if value.is_missing() {
                    emit_proof_trusted_diagnostic(
                        parser,
                        ParseErrorKind::ProofTrustedReasonNotString,
                        value_range,
                    );
                    continue;
                }
                let Some(expression) = parser.expression_projection_for_range(value_range) else {
                    emit_proof_trusted_diagnostic(
                        parser,
                        ParseErrorKind::ProofTrustedReasonNotString,
                        value_range,
                    );
                    continue;
                };
                match expression.projection() {
                    ExpressionProjection::Literal(literal) => match literal.value() {
                        SyntaxLiteralValue::String { value, .. } if value.trim().is_empty() => {
                            emit_proof_trusted_diagnostic(
                                parser,
                                ParseErrorKind::ProofTrustedReasonEmpty,
                                value_range,
                            );
                        }
                        SyntaxLiteralValue::String { .. } => {}
                        _ => emit_proof_trusted_diagnostic(
                            parser,
                            ParseErrorKind::ProofTrustedReasonNotString,
                            value_range,
                        ),
                    },
                    _ => emit_proof_trusted_diagnostic(
                        parser,
                        ParseErrorKind::ProofTrustedReasonNotString,
                        value_range,
                    ),
                }
            }
        }
    }
}

fn trusted_argument_component(
    projection: &PendingOuterAttributeProjection,
    argument: u16,
    part: SyntaxCallArgumentPart,
) -> Option<SourceRange> {
    projection
        .components()
        .iter()
        .find(|component| {
            component.role() == ExpressionComponentRole::CallArgument { argument, part }
        })
        .map(|component| component.range())
}

fn emit_proof_trusted_diagnostic(
    parser: &mut DocumentParser<'_, '_>,
    kind: ParseErrorKind,
    range: SourceRange,
) {
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        kind.code(),
        range,
        kind.label(),
    )));
}

fn emit_attribute_recovery(parser: &mut DocumentParser<'_, '_>, end: usize) -> bool {
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

fn attribute_line_end(parser: &DocumentParser<'_, '_>) -> usize {
    (parser.cursor()..token_count(parser))
        .find(|index| {
            parser
                .token_at(*index)
                .is_some_and(|token| token.kind() == SyntaxKind::NewlineToken)
        })
        .unwrap_or_else(|| token_count(parser))
}

fn outer_prefix_kind(parser: &DocumentParser<'_, '_>) -> Option<OuterPrefixKind> {
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

fn bump_outer_prefix_line(parser: &mut DocumentParser<'_, '_>) {
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

pub(super) fn emit_visibility(parser: &mut DocumentParser<'_, '_>) {
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

fn emit_scoped_visibility(parser: &mut DocumentParser<'_, '_>) -> PendingVisibilityKind {
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
    parser: &mut DocumentParser<'_, '_>,
    family: DeclarationIdentityFamily,
    emit_family_tail: impl FnOnce(&mut DocumentParser<'_, '_>) -> T,
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
    let public_id = emit_declaration_public_id(parser, family, keyword_range);
    parser.bump_trivia();
    let name = emit_declaration_name(parser, family.prefix());
    parser.bump_trivia();
    let family_tail = emit_family_tail(parser);
    parser.set_declaration_header_projection(
        owner,
        PendingDeclarationHeaderProjection::new(public_id, name),
    );
    parser.finish();
    family_tail
}

pub(super) fn emit_metric_declaration_header(
    parser: &mut DocumentParser<'_, '_>,
    emit_kind: impl FnOnce(&mut DocumentParser<'_, '_>),
    emit_family_tail: impl FnOnce(&mut DocumentParser<'_, '_>),
) {
    let owner = parser.start_projected_owner(SyntaxKind::DeclarationHeader, SyntaxRole::Element(0));
    emit_outer_prefixes(parser);
    parser.bump_trivia();
    emit_visibility(parser);
    parser.bump_trivia();

    let keyword_range = parser
        .current()
        .filter(|token| parser.text_of(*token) == DeclarationIdentityFamily::Metric.prefix())
        .map_or_else(
            || SourceRange::new(parser.current_offset(), parser.current_offset()),
            LexToken::range,
        );
    if parser.at(DeclarationIdentityFamily::Metric.prefix()) {
        parser.bump();
    }
    parser.bump_trivia();
    emit_kind(parser);
    parser.bump_trivia();
    let public_id =
        emit_declaration_public_id(parser, DeclarationIdentityFamily::Metric, keyword_range);
    parser.bump_trivia();
    let name = emit_declaration_name(parser, DeclarationIdentityFamily::Metric.prefix());
    parser.bump_trivia();
    emit_family_tail(parser);
    parser.set_declaration_header_projection(
        owner,
        PendingDeclarationHeaderProjection::new(public_id, name),
    );
    parser.finish();
}

/// Emits one shared declaration identity and its required local name.
///
/// All declaration producers accept the same relative identity spellings and
/// normalize them to the declaration family before the name and lint layers
/// observe them.
pub(super) fn emit_declaration_identity(
    parser: &mut DocumentParser<'_, '_>,
    family: DeclarationIdentityFamily,
    keyword_range: SourceRange,
) -> PendingDeclarationHeaderProjection {
    let public_id = emit_declaration_public_id(parser, family, keyword_range);
    parser.bump_trivia();
    let name = emit_declaration_name(parser, family.prefix());
    PendingDeclarationHeaderProjection::new(public_id, name)
}

fn emit_declaration_public_id(
    parser: &mut DocumentParser<'_, '_>,
    family: DeclarationIdentityFamily,
    keyword_range: SourceRange,
) -> PendingDeclarationPublicId {
    if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) {
        let token = parser.current().expect("checked declaration ID token");
        let projection = typed_entity_reference(token, parser.text_of(token));
        let syntax = projection.into_syntax();
        let source = token.range();
        let family_name = SyntaxName::try_new(family.prefix())
            .expect("declaration identity family prefixes are valid names");

        parser.start(SyntaxKind::DeclarationPublicId, SyntaxRole::PublicId);
        let result = match syntax.value() {
            Err(crate::id_ref::SyntaxIdRefIssue::MissingSuffix) => {
                parser.start(SyntaxKind::MissingDeclarationId, SyntaxRole::Recovery(0));
                parser.bump();
                parser.finish();
                parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                    "syntax.declaration.malformed_id",
                    source,
                    "declaration ID is missing after `@`",
                )));
                PendingDeclarationPublicId::Recovered {
                    issue: PendingDeclarationPublicIdIssue::Missing,
                    source,
                }
            }
            Err(_) => {
                parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
                parser.bump();
                parser.finish();
                parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                    "syntax.declaration.malformed_id",
                    source,
                    "declaration ID is malformed",
                )));
                PendingDeclarationPublicId::Recovered {
                    issue: PendingDeclarationPublicIdIssue::Malformed,
                    source,
                }
            }
            Ok(_reference) => {
                let (normalized, canonical) = syntax.normalized_for_family(&family_name);
                if !canonical {
                    let public_id = public_id_from_syntax(&syntax, family)
                        .expect("validated entity reference has a public-ID spelling");
                    parser.start(SyntaxKind::WrongFamilyReference, SyntaxRole::Reference(0));
                    parser.bump();
                    parser.finish();
                    parser.push(SyntaxEvent::Diagnostic(
                        PendingSyntaxDiagnostic::new(
                            "syntax.declaration.wrong_family_id",
                            source,
                            format!(
                                "declaration ID must belong to the `{}` family",
                                family.prefix()
                            ),
                        )
                        .with_related_range(keyword_range),
                    ));
                    PendingDeclarationPublicId::Recovered {
                        issue: PendingDeclarationPublicIdIssue::WrongFamily(public_id),
                        source,
                    }
                } else if let Some(public_id) = public_id_from_syntax(&normalized, family) {
                    parser.bump();
                    PendingDeclarationPublicId::Explicit {
                        value: public_id,
                        source,
                    }
                } else {
                    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
                    parser.bump();
                    parser.finish();
                    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                        "syntax.declaration.malformed_id",
                        source,
                        "declaration ID is malformed",
                    )));
                    PendingDeclarationPublicId::Recovered {
                        issue: PendingDeclarationPublicIdIssue::Malformed,
                        source,
                    }
                }
            }
        };
        parser.finish();
        return result;
    }
    PendingDeclarationPublicId::Derived
}

fn public_id_from_syntax(
    syntax: &crate::id_ref::SyntaxIdRefSyntax,
    family: DeclarationIdentityFamily,
) -> Option<PublicId> {
    let reference = syntax.value().ok()?;
    let mut components = Vec::new();
    match reference.root() {
        AuthoredIdRoot::Relative { .. } => components.push(family.prefix().to_owned()),
        AuthoredIdRoot::FamilyRelative {
            family: authored, ..
        } => {
            components.push(authored.as_str().to_owned());
        }
        AuthoredIdRoot::Absolute { .. } => {}
    }
    components.extend(
        reference
            .segments()
            .iter()
            .map(|segment| segment.as_str().to_owned()),
    );
    PublicId::try_new(components.join(".")).ok()
}

fn emit_declaration_name(
    parser: &mut DocumentParser<'_, '_>,
    family: &str,
) -> PendingDeclarationName {
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) && !current_name_is_dotted(parser)
    {
        let token = parser.current().expect("checked declaration name token");
        let name = SyntaxName::try_new(parser.text_of(token))
            .expect("identifier token is a validated declaration name");
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        parser.bump();
        parser.finish();
        return PendingDeclarationName::Resolved {
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
            format!("{family} declaration requires one ordinary local name"),
        )));
        return PendingDeclarationName::Missing {
            insertion: SourceRange::new(at, at),
        };
    }

    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_invalid_declaration_name(parser);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.declaration.invalid_name",
        SourceRange::new(at, parser.current_offset()),
        format!("{family} declaration name must be one non-keyword identifier"),
    )));
    PendingDeclarationName::Invalid {
        insertion: SourceRange::new(at, at),
        recovery: SourceRange::new(at, parser.current_offset()),
    }
}

fn current_name_is_dotted(parser: &DocumentParser<'_, '_>) -> bool {
    let Some(current) = parser.current() else {
        return false;
    };
    parser.token_at(parser.cursor() + 1).is_some_and(|next| {
        next.range().start() == current.range().end() && parser.text_of(next) == "."
    })
}

fn bump_invalid_declaration_name(parser: &mut DocumentParser<'_, '_>) {
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

pub(super) fn emit_name(parser: &mut DocumentParser<'_, '_>, keyword: &str) {
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

pub(super) fn emit_generic_parameters(parser: &mut DocumentParser<'_, '_>) {
    parser.start(SyntaxKind::GenericParameterGroup, SyntaxRole::GenericGroup);
    emit_open_delimiter(parser, SyntaxKind::OpenAngleNode, "<");
    parser.start(SyntaxKind::GenericParameterList, SyntaxRole::Element(0));
    let mut ordinal = 0_u16;
    loop {
        parser.bump_trivia();
        if parser.is_at_end() || parser.at(">") {
            break;
        }
        let end =
            find_top_level_boundary(parser, parser.cursor(), token_count(parser), &[",", ">"]);
        emit_generic_parameter(parser, end, ordinal);
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

fn emit_generic_parameter(parser: &mut DocumentParser<'_, '_>, end: usize, ordinal: u16) {
    let first = first_significant(parser, parser.cursor(), end);
    let kind =
        first
            .and_then(|index| parser.token_at(index))
            .map_or(SyntaxKind::TypeParameter, |token| {
                if token.kind() == SyntaxKind::LifetimeToken {
                    SyntaxKind::LifetimeParameter
                } else {
                    SyntaxKind::TypeParameter
                }
            });
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
        emit_missing_generic_parameter_name(parser, first, kind);
    }
    parser.bump_trivia();
    emit_generic_parameter_tail(parser, end, kind);
    bump_until(parser, end);
    parser.finish();
    parser.finish();
}

fn emit_missing_generic_parameter_name(
    parser: &mut DocumentParser<'_, '_>,
    first: Option<usize>,
    kind: SyntaxKind,
) {
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

fn emit_generic_parameter_tail(parser: &mut DocumentParser<'_, '_>, end: usize, kind: SyntaxKind) {
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
        SyntaxKind::TypeParameter if parser.cursor() < end => emit_invalid_generic_parameter_tail(
            parser,
            end,
            "syntax.generic.invalid_parameter",
            "unexpected syntax after generic type parameter name",
        ),
        SyntaxKind::LifetimeParameter if parser.cursor() < end => {
            emit_invalid_generic_parameter_tail(
                parser,
                end,
                "syntax.generic.invalid_lifetime_parameter",
                "lifetime parameters do not accept inline bounds",
            );
        }
        _ => {}
    }
}

fn emit_invalid_generic_parameter_tail(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    code: &'static str,
    message: &'static str,
) {
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, end);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(start, parser.current_offset()),
        message,
    )));
}

fn emit_generic_bounds(parser: &mut DocumentParser<'_, '_>, end: usize) {
    let mut ordinal = 0_u32;
    loop {
        parser.bump_trivia();
        let bound_end = find_top_level_boundary(parser, parser.cursor(), end, &["+"]);
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
    parser: &mut DocumentParser<'_, '_>,
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
    parser: &mut DocumentParser<'_, '_>,
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
            find_top_level_boundary(parser, parser.cursor(), content_end, &[",", ")"]);
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

pub(super) fn fixed_parameter_group_end(parser: &DocumentParser<'_, '_>) -> usize {
    let start = parser.cursor().saturating_add(1);
    if let Some(close) = find_matching_close(parser, start, "(") {
        return close.saturating_add(1);
    }

    find_top_level_boundary(
        parser,
        start,
        token_count(parser),
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
    )
}

pub(super) fn emit_missing_parameter_group(
    parser: &mut DocumentParser<'_, '_>,
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
    parser: &mut DocumentParser<'_, '_>,
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
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    ordinal: u16,
    grammar: FixedParameterGrammar,
    missing_type_message: &'static str,
) {
    let parameter_owner =
        parser.start_projected_owner(SyntaxKind::Parameter, SyntaxRole::Parameter(ordinal));
    let colon = find_top_level_boundary(parser, parser.cursor(), end, &[":"]);
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
        emit_required_punctuation(
            parser,
            SyntaxKind::ColonNode,
            SyntaxRole::Colon,
            ":",
            "syntax.parameter.missing_colon",
            "typed parameter requires `:` before its type",
        );
        parser.bump_trivia();
        if parser.at("...") {
            parser.start(SyntaxKind::RestParameterMarker, SyntaxRole::Kind);
            parser.bump();
            parser.finish();
            parser.bump_trivia();
        }
        let default = find_top_level_boundary(parser, parser.cursor(), end, &["="]);
        emit_type(parser, default, SyntaxRole::ParameterType);
        bump_until(parser, default);
        if parser.at("=") && parser.cursor() < end {
            emit_parameter_default(parser, end, grammar);
        }
    } else {
        let at = parser.current_offset();
        emit_required_punctuation(
            parser,
            SyntaxKind::ColonNode,
            SyntaxRole::Colon,
            ":",
            "syntax.parameter.missing_colon",
            "typed parameter requires `:` before its type",
        );
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
    parser: &mut DocumentParser<'_, '_>,
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
    parser: &DocumentParser<'_, '_>,
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

pub(super) fn emit_return_type(parser: &mut DocumentParser<'_, '_>, item_kind: SyntaxKind) {
    let start = parser.current_offset();
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
    let end = find_header_boundary(parser, parser.cursor());
    let type_end = trimmed_end(parser, parser.cursor(), end);
    emit_type(parser, type_end, SyntaxRole::Type);
    bump_until(parser, type_end);
    parser.finish();
    if item_kind == SyntaxKind::PredicateItem {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.predicate.return_not_allowed",
            SourceRange::new(start, parser.current_offset()),
            "predicates have an implicit `Bool` return type",
        )));
    }
}

pub(super) fn emit_where_clause(parser: &mut DocumentParser<'_, '_>) {
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
        let end = find_top_level_boundary(parser, parser.cursor(), clause_end, &[","]);
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

fn emit_where_predicate_children(parser: &mut DocumentParser<'_, '_>, end: usize) {
    let colon = find_top_level_boundary(parser, parser.cursor(), end, &[":"]);
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
        let bound_end = find_top_level_boundary(parser, parser.cursor(), end, &["+"]);
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
pub(super) fn emit_callable_contract_clauses(parser: &mut DocumentParser<'_, '_>) {
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

/// Emits the maintained ordinary-Function contract sequence.
///
/// Predicate/Proof declarations retain the scalar requires/ensures grammar.
/// Ordinary functions additionally own explicit effect upper-bound lists in
/// the same source-ordered contract row; the body parser never reclassifies an
/// `effects { ... }` clause as a block body.
pub(super) fn emit_function_contract_clauses(parser: &mut DocumentParser<'_, '_>) {
    let mut ordinal = 0_u16;
    let mut saw_ensures = false;
    while matches!(
        parser.current_text(),
        Some("requires" | "ensures" | "effects")
    ) {
        let family = match parser.current_text() {
            Some("requires") => ContractClauseFamily::Requires,
            Some("ensures") => ContractClauseFamily::Ensures,
            Some("effects") => ContractClauseFamily::Effects,
            _ => unreachable!("ordinary Function contract guard is exhaustive"),
        };
        let clause_start = parser.current_offset();
        emit_contract_clause(
            parser,
            family,
            SyntaxRole::ContractClause(ordinal),
            family == ContractClauseFamily::Effects,
        );
        if family == ContractClauseFamily::Requires && saw_ensures {
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.contract.invalid_clause_order",
                SourceRange::new(clause_start, parser.current_offset()),
                "`requires` clauses must precede every `ensures` clause",
            )));
        }
        saw_ensures |= family == ContractClauseFamily::Ensures;
        ordinal = ordinal
            .checked_add(1)
            .expect("contract clause budget is below the role index range");
        parser.bump_trivia();
    }
}

/// Emits the full heterogeneous Flow contract sequence. Authored order is
/// recorded directly and is never reconstructed from family-specific rows.
pub(super) fn emit_flow_contract_clauses(parser: &mut DocumentParser<'_, '_>) {
    let mut ordinal = 0_u16;
    while let Some(family) = current_flow_contract_family(parser) {
        emit_contract_clause(parser, family, SyntaxRole::ContractClause(ordinal), true);
        ordinal = ordinal
            .checked_add(1)
            .expect("contract clause budget is below the role index range");
        parser.bump_trivia();
    }
}

fn current_flow_contract_family(parser: &DocumentParser<'_, '_>) -> Option<ContractClauseFamily> {
    let family = ContractClauseFamily::from_flow_keyword(parser.current_text()?)?;
    if family == ContractClauseFamily::Ensures && ensures_starts_no_effect(parser) {
        Some(ContractClauseFamily::NoEffect)
    } else {
        Some(family)
    }
}

fn ensures_starts_no_effect(parser: &DocumentParser<'_, '_>) -> bool {
    let start = parser.cursor().saturating_add(1);
    let end = find_contract_expression_boundary(parser, start);
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
    parser: &mut DocumentParser<'_, '_>,
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
        let end = find_contract_expression_boundary(parser, parser.cursor());
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

fn bump_contract_trivia(parser: &mut DocumentParser<'_, '_>) -> bool {
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

fn emit_contract_operand_until(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    ordinal: u16,
) -> CompletedNode {
    let expression_start = parser.cursor();
    let expression_end = trimmed_end(parser, expression_start, end);
    let condition = if expression_end == expression_start {
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
        emit_expression_node(parser, expression_end, SyntaxRole::ContractOperand(ordinal))
    };
    bump_until(parser, expression_end);
    condition
}

fn emit_contract_operand_list(
    parser: &mut DocumentParser<'_, '_>,
    payload_starts_on_new_line: bool,
) {
    let clause_end = find_contract_expression_boundary(parser, parser.cursor());
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
        let operand_end = find_top_level_boundary(parser, parser.cursor(), list_end, &[","]);
        emit_contract_operand_until(parser, operand_end, ordinal);
        ordinal = ordinal
            .checked_add(1)
            .expect("contract operand budget is below the role index range");
        parser.bump_trivia();
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

/// Finds the next declaration boundary while scanning a contract expression.
///
/// Unlike a declaration header, an expression uses `<` and `>` as ordinary
/// comparison operators. Treating those tokens as generic delimiters causes a
/// condition such as `a < c` to absorb every following contract clause and the
/// callable body. Parentheses, brackets, and nested braces still protect their
/// contents from the outer contract boundary.
fn find_contract_expression_boundary(parser: &DocumentParser<'_, '_>, start: usize) -> usize {
    let mut delimiters = Vec::new();
    let end = token_count(parser);
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            break;
        };
        let text = parser.text_of(token);
        if delimiters.is_empty()
            && matches!(
                text,
                "where"
                    | "requires"
                    | "ensures"
                    | "invariant"
                    | "assume"
                    | "reads"
                    | "effects"
                    | "modifies"
                    | "decreases"
                    | "="
                    | "{"
            )
        {
            return index;
        }
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
            _ => {}
        }
    }
    end
}

/// Stops an unclosed braced list before the next top-level Flow clause or
/// body. This is a grammar synchronization rule over the current token cursor,
/// not a second parser or source-text reconstruction path.
fn find_flow_contract_list_recovery_boundary(
    parser: &DocumentParser<'_, '_>,
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

fn emit_unbraced_contract_operands(parser: &mut DocumentParser<'_, '_>, clause_end: usize) {
    if trimmed_end(parser, parser.cursor(), clause_end) == parser.cursor() {
        emit_contract_operand_until(parser, clause_end, 0);
        return;
    }

    let mut ordinal = 0_u16;
    loop {
        let operand_end = find_top_level_boundary(parser, parser.cursor(), clause_end, &[","]);
        emit_contract_operand_until(parser, operand_end, ordinal);
        ordinal = ordinal
            .checked_add(1)
            .expect("contract operand budget is below the role index range");
        parser.bump_trivia();
        if parser.at(",") && parser.cursor() < clause_end {
            parser.bump();
            parser.bump_trivia();
        } else {
            break;
        }
    }
}
