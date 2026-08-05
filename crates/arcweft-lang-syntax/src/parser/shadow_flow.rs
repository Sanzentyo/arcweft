//! Private Flow body grammar over the shared full-source cursor.

use arcweft_id::DeclarationIdentityFamily;
use arcweft_source::SourceRange;

use super::cursor::ShadowDocumentParser;
use super::declaration::{
    FixedParameterGrammar, emit_extra_parameter_group_recovery, emit_fixed_parameters_until,
    emit_flow_contract_clauses, emit_generic_parameters, emit_outer_prefixes, emit_return_type,
    emit_visibility, emit_where_clause, fixed_parameter_group_end,
};
use super::lexer::{LexToken, typed_entity_reference};
use super::shadow_recovery::expected;
use super::statement::emit_braced_thread_flow_block;
use crate::grammar::budget::GrammarBudget;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::flow_projection::{
    PendingFlowDeclarationProjection, PendingFlowIdentity, PendingFlowPublicId,
    PendingFlowPublicIdForm,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::id_ref::SyntaxIdRefIssue;
use crate::name::SyntaxName;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FlowIdProblem {
    WrongFamily,
    Malformed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FlowIdEmission {
    public_id: PendingFlowPublicId,
    requires_name: bool,
}

pub(super) fn emit_declaration(
    source: &str,
    tokens: &[LexToken],
    role: SyntaxRole,
    events: &mut Vec<SyntaxEvent>,
    budget: &mut GrammarBudget,
) {
    let mut parser = ShadowDocumentParser::new(source, tokens, events, budget);
    let owner = parser.start_projected_owner(SyntaxKind::FlowItem, role);
    emit_outer_prefixes(&mut parser);
    parser.bump_trivia();
    emit_visibility(&mut parser);
    parser.bump_trivia();
    let flow_keyword = parser
        .current()
        .filter(|_| parser.at("flow"))
        .expect("Flow declaration dispatch retains its keyword")
        .range();
    parser.bump();
    parser.bump_trivia();
    let identity = emit_flow_identity(&mut parser);
    parser.bump_trivia();

    if parser.at("<") {
        emit_generic_parameters(&mut parser);
        parser.bump_trivia();
    }
    if parser.at("(") {
        let end = fixed_parameter_group_end(&parser);
        emit_fixed_parameters_until(
            &mut parser,
            end,
            FixedParameterGrammar::TypedPatternWithRecoveredDefault {
                diagnostic: "flow.signature.parameter_default_not_admitted",
                message: "Flow parameters do not admit default values",
            },
            "Flow parameters require an authored type",
            "syntax.decl.unclosed_parameters",
        );
        parser.bump_trivia();
    }
    emit_extra_parameter_group_recovery(
        &mut parser,
        "flow",
        FixedParameterGrammar::TypedPatternWithRecoveredDefault {
            diagnostic: "flow.signature.parameter_default_not_admitted",
            message: "Flow parameters do not admit default values",
        },
        "Flow recovery parameters require an authored type",
    );
    if parser.at("->") {
        emit_return_type(&mut parser, SyntaxKind::FlowItem);
        parser.bump_trivia();
    }
    if parser.at("where") {
        emit_where_clause(&mut parser);
        parser.bump_trivia();
    }
    let signature_end = SourceRange::new(parser.current_offset(), parser.current_offset());
    emit_flow_contract_clauses(&mut parser);

    parser.start(SyntaxKind::FlowBody, SyntaxRole::Body);
    if parser.at("{") {
        emit_braced_thread_flow_block(
            &mut parser,
            SyntaxKind::FlowItem,
            SyntaxKind::Block,
            SyntaxRole::Body,
            "syntax.flow.missing_block_close",
        );
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.flow.missing_body",
            SourceRange::new(at, at),
            "missing Flow body",
        )));
    }
    parser.finish();

    while parser.bump().is_some() {}
    parser.set_flow_declaration_projection(
        owner,
        PendingFlowDeclarationProjection::new(flow_keyword, identity, signature_end),
    );
    parser.finish();
}

fn emit_flow_identity(parser: &mut ShadowDocumentParser<'_, '_>) -> PendingFlowIdentity {
    let public_id = emit_flow_public_id(parser);
    if public_id.is_some() {
        parser.bump_trivia();
    }
    let name = emit_optional_flow_name(parser);

    match (public_id, name) {
        (None, Some((value, source))) => PendingFlowIdentity::Name { value, source },
        (Some(public_id), Some((name, name_source))) => PendingFlowIdentity::PublicIdAndName {
            public_id: public_id.public_id,
            name,
            name_source,
        },
        (Some(public_id), None) if !public_id.requires_name => {
            PendingFlowIdentity::PublicId(public_id.public_id)
        }
        (public_id, None) => {
            let at = parser.current_offset();
            parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
            parser.push(SyntaxEvent::MissingToken {
                expected: expected(SyntaxKind::IdentifierToken),
                at,
            });
            parser.finish();
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "flow.identity.missing",
                SourceRange::new(at, at),
                "Flow declaration requires a name or complete public ID",
            )));
            PendingFlowIdentity::Missing {
                insertion: SourceRange::new(at, at),
                public_id_recovery: public_id.map(|emission| emission.public_id),
            }
        }
    }
}

fn emit_flow_public_id(parser: &mut ShadowDocumentParser<'_, '_>) -> Option<FlowIdEmission> {
    let token = parser
        .current()
        .filter(|token| token.kind() == SyntaxKind::EntityReferenceToken)?;
    let source = token.range();
    let projection = typed_entity_reference(token, parser.text_of(token));
    let marker_family = projection.empty_marker_family().cloned();
    let malformed_delimited_absolute = projection.has_unclosed_delimited_absolute();
    let components = projection.components().to_vec().into_boxed_slice();
    let syntax = projection.into_syntax();
    let flow_family = SyntaxName::try_new(DeclarationIdentityFamily::Flow.prefix())
        .expect("fixed Flow family is an identifier");
    let canonical_flow_family = !malformed_delimited_absolute
        && match syntax.value() {
            Ok(_) => syntax.normalized_for_family(&flow_family).1,
            Err(SyntaxIdRefIssue::MissingSuffix) if marker_family.is_some() => {
                marker_family.as_ref().is_none_or(|family| {
                    family.as_ref().is_none_or(|family| {
                        family.as_str() == DeclarationIdentityFamily::Flow.prefix()
                    })
                })
            }
            _ => false,
        };
    let problem = if malformed_delimited_absolute {
        Some(FlowIdProblem::Malformed)
    } else {
        match syntax.value() {
            Err(SyntaxIdRefIssue::MissingSuffix) if marker_family.is_some() => {
                (!canonical_flow_family).then_some(FlowIdProblem::WrongFamily)
            }
            Err(_) => Some(FlowIdProblem::Malformed),
            Ok(_) if !canonical_flow_family => Some(FlowIdProblem::WrongFamily),
            Ok(_) => None,
        }
    };
    let form = match marker_family {
        Some(family) => PendingFlowPublicIdForm::DerivedFromEmptyMarker { family },
        None => PendingFlowPublicIdForm::Authored,
    };
    let requires_name = matches!(form, PendingFlowPublicIdForm::DerivedFromEmptyMarker { .. });

    parser.start(SyntaxKind::DeclarationPublicId, SyntaxRole::PublicId);
    if let Some(problem) = problem {
        parser.start(
            match problem {
                FlowIdProblem::WrongFamily => SyntaxKind::WrongFamilyReference,
                FlowIdProblem::Malformed => SyntaxKind::ErrorNode,
            },
            match problem {
                FlowIdProblem::WrongFamily => SyntaxRole::Reference(0),
                FlowIdProblem::Malformed => SyntaxRole::Recovery(0),
            },
        );
    }
    parser.bump();
    if problem.is_some() {
        parser.finish();
    }
    parser.finish();
    if let Some(problem) = problem {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            match problem {
                FlowIdProblem::WrongFamily => "flow.identity.wrong_family",
                FlowIdProblem::Malformed => "flow.identity.malformed_public_id",
            },
            source,
            match problem {
                FlowIdProblem::WrongFamily => {
                    "Flow declaration public ID must belong to the `flow` family"
                }
                FlowIdProblem::Malformed => "Flow declaration public ID is malformed",
            },
        )));
    }

    Some(FlowIdEmission {
        public_id: PendingFlowPublicId::new(
            syntax,
            source,
            components,
            form,
            canonical_flow_family,
        ),
        requires_name,
    })
}

fn emit_optional_flow_name(
    parser: &mut ShadowDocumentParser<'_, '_>,
) -> Option<(SyntaxName, SourceRange)> {
    let token = parser
        .current()
        .filter(|token| token.kind() == SyntaxKind::IdentifierToken)?;
    let value = SyntaxName::try_new(parser.text_of(token))
        .expect("identifier token is a validated Flow name");
    let source = token.range();
    parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
    parser.bump();
    parser.finish();
    Some((value, source))
}
