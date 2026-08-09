//! Private nested pattern-family events over the shared cursor.

use arcweft_source::SourceRange;

use super::cursor::DocumentParser;
use super::path::emit_path;
use super::pattern_projection::{
    PatternProjectionTransaction, binding_syntax, empty_range, is_trivia, name_syntax,
    project_id_ref, project_literal, project_record_path, project_variant_head, significant_range,
    stage_record_rest, stage_sequence_rest, stage_variant_payload,
};
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, find_matching_close,
    find_top_level_boundary, first_significant, token_text, trimmed_end,
};
use super::type_ref::emit_type;
use crate::grammar::callable_projection::PendingMethodReceiverProjection;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::name::SyntaxName;
use crate::patterns::{
    PatternBindingIssue, PatternBindingSyntax, PatternComponentRole, PatternFieldPart,
    PatternInvalidRecordFieldSyntax, PatternNameSyntax, PatternNodePath, PatternNodeStep,
    PatternRecordFieldIssue, PatternRecordFieldShape, PatternRecordFieldSyntax,
    PatternRecordSyntax, PatternRecoveryIssue, PatternSequenceRestIssue, PatternSequenceRestSyntax,
    PatternSequenceSyntax, PatternSyntaxKind, PatternSyntaxNode, PatternSyntaxState,
    PatternVariantPayloadIssue, PatternVariantPayloadSyntax, PatternVariantSyntax,
};

pub(super) fn emit_pattern(parser: &mut DocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    let mut transaction = PatternProjectionTransaction::new(parser);
    let root_path = PatternNodePath::root();
    let root = emit_pattern_node(parser, end, role, &mut transaction, &root_path);
    transaction.finish(parser, root);
}

/// Emits the shared binding Pattern owned by one parser-selected method receiver.
///
/// Borrow markers remain Parameter-level receiver semantics. The Pattern owns
/// the complete receiver source and the exact `self` binding, but never gains a
/// fabricated type or a borrow-only Pattern family.
pub(super) fn emit_method_receiver_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    receiver: &PendingMethodReceiverProjection,
) {
    let mut transaction = PatternProjectionTransaction::new(parser);
    let root = PatternNodePath::root();
    let mutable_binding = matches!(
        receiver,
        PendingMethodReceiverProjection::Owned {
            mut_keyword: Some(_),
            ..
        }
    );
    transaction.start_node(
        parser,
        if mutable_binding {
            SyntaxKind::MutableBindingPattern
        } else {
            SyntaxKind::BindingPattern
        },
        role,
        &root,
        receiver.whole(),
    );
    if mutable_binding {
        transaction.component(
            &root,
            PatternComponentRole::MutKeyword,
            receiver
                .mut_keyword()
                .expect("mutable owned receiver retains its `mut` source"),
        );
    }
    transaction.component(&root, PatternComponentRole::Name, receiver.self_keyword());

    let self_index = (parser.cursor()..end)
        .find(|index| {
            parser
                .token_at(*index)
                .is_some_and(|token| token.range() == receiver.self_keyword())
        })
        .expect("validated method receiver retains one `self` token");
    bump_until(parser, self_index);
    parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
    parser.bump();
    parser.finish();
    bump_until(parser, end);
    parser.finish();

    let binding = PatternBindingSyntax::Resolved(
        SyntaxName::try_new("self").expect("`self` is a structurally valid binding name"),
    );
    transaction.finish(
        parser,
        PatternSyntaxNode::new(
            if mutable_binding {
                PatternSyntaxKind::MutableBinding(binding)
            } else {
                PatternSyntaxKind::Binding(binding)
            },
            PatternSyntaxState::Valid,
        ),
    );
}

fn emit_pattern_node(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    let end = trimmed_end(parser, parser.cursor(), end);
    if parser.cursor() >= end {
        let at = parser.current_offset();
        return emit_missing_pattern_node(
            parser,
            role,
            transaction,
            path,
            SourceRange::new(at, at),
        );
    }

    if boundary(parser, parser.cursor(), end, &["|"]).is_some() {
        return emit_or_pattern(parser, end, role, transaction, path);
    }
    if let Some(colon) = typed_binding_colon(parser, parser.cursor(), end) {
        return emit_typed_binding_pattern(parser, colon, end, role, transaction, path);
    }
    if is_variant_pattern(parser, parser.cursor(), end) {
        return emit_variant_pattern(parser, end, role, transaction, path);
    }
    if let Some(rest) = whole_binding_rest(parser, parser.cursor(), end) {
        return emit_whole_binding_pattern(parser, rest, end, role, transaction, path);
    }
    if boundary(parser, parser.cursor(), end, &["{"]).is_some() {
        return emit_record_pattern(parser, end, role, transaction, path);
    }

    match parser.current_text() {
        Some("_") => emit_discard_pattern(parser, end, role, transaction, path),
        Some("mut") => emit_mutable_binding_pattern(parser, end, role, transaction, path),
        Some("true" | "false") => emit_literal_pattern(parser, end, role, transaction, path),
        Some("(") => emit_tuple_pattern(parser, end, role, transaction, path),
        Some("[") => emit_sequence_pattern(parser, end, role, transaction, path),
        Some("..") => emit_invalid_rest_pattern(parser, end, role, transaction, path),
        _ if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) => {
            emit_entity_reference_pattern(parser, end, role, transaction, path)
        }
        _ if parser.current_kind().is_some_and(is_literal) => {
            emit_literal_pattern(parser, end, role, transaction, path)
        }
        _ if matches!(
            parser.current_kind(),
            Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
        ) =>
        {
            emit_binding_pattern(parser, end, role, transaction, path)
        }
        _ => emit_error_pattern(parser, end, role, transaction, path),
    }
}

fn emit_or_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    start_pattern(parser, end, SyntaxKind::OrPattern, role, transaction, path);
    let mut ordinal = 0_u32;
    let mut alternatives = Vec::new();
    let mut issues = Vec::new();
    loop {
        parser.bump_trivia();
        if parser.cursor() >= end {
            break;
        }
        let alternative_end = find_top_level_boundary(parser, parser.cursor(), end, &["|"]);
        transaction.component(
            path,
            PatternComponentRole::Element { ordinal },
            significant_range(parser, parser.cursor(), alternative_end),
        );
        alternatives.push(emit_pattern_node(
            parser,
            alternative_end,
            SyntaxRole::Element(ordinal),
            transaction,
            &path.child(PatternNodeStep::Element(ordinal)),
        ));
        bump_until(parser, alternative_end);
        ordinal = ordinal
            .checked_add(1)
            .expect("grammar limits fit Pattern element ordinals");
        if parser.at("|") {
            parser.bump();
            if trimmed_end(parser, parser.cursor(), end) == parser.cursor() {
                // Emit the recovery node before consuming trailing trivia so
                // its parser event and typed source projection share the
                // exact insertion boundary immediately after `|`.
                let at = parser.current_offset();
                let missing = SourceRange::new(at, at);
                transaction.component(path, PatternComponentRole::Element { ordinal }, missing);
                alternatives.push(emit_missing_pattern_node(
                    parser,
                    SyntaxRole::Element(ordinal),
                    transaction,
                    &path.child(PatternNodeStep::Element(ordinal)),
                    missing,
                ));
                issues.push(PatternRecoveryIssue::MissingOrAlternative { ordinal });
                break;
            }
            parser.bump_trivia();
        } else {
            break;
        }
    }
    parser.finish();
    PatternSyntaxNode::new(
        PatternSyntaxKind::Or(alternatives.into_boxed_slice()),
        PatternSyntaxState::from_issues(issues),
    )
}

fn emit_missing_pattern_node(
    parser: &mut DocumentParser<'_, '_>,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
    source: SourceRange,
) -> PatternSyntaxNode {
    transaction.start_node(parser, SyntaxKind::MissingPattern, role, path, source);
    transaction.component(path, PatternComponentRole::Recovery, source);
    parser.finish();
    PatternSyntaxNode::new(
        PatternSyntaxKind::Error,
        PatternSyntaxState::from_issues(vec![PatternRecoveryIssue::MissingPattern]),
    )
}

fn emit_whole_binding_pattern(
    parser: &mut DocumentParser<'_, '_>,
    rest: usize,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    start_pattern(
        parser,
        end,
        SyntaxKind::WholeBindingPattern,
        role,
        transaction,
        path,
    );
    let name = parser
        .current()
        .map_or_else(|| empty_range(parser), super::lexer::LexToken::range);
    transaction.component(path, PatternComponentRole::WholeBindingName, name);
    let (binding, issues) = binding_syntax(parser, parser.cursor(), rest);
    parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
    parser.bump();
    parser.finish();
    bump_until(parser, rest);
    transaction.component(
        path,
        PatternComponentRole::NestedPattern,
        significant_range(parser, rest, end),
    );
    let nested = emit_pattern_node(
        parser,
        end,
        SyntaxRole::Pattern,
        transaction,
        &path.child(PatternNodeStep::NestedPattern),
    );
    parser.finish();
    PatternSyntaxNode::new(
        PatternSyntaxKind::WholeBinding {
            binding,
            pattern: Box::new(nested),
        },
        PatternSyntaxState::from_issues(issues),
    )
}

fn emit_variant_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    let payload = boundary(parser, parser.cursor(), end, &["(", "{"]);
    let head_end = payload.unwrap_or(end);
    let node_end = payload
        .and_then(|open| {
            parser
                .token_at(open)
                .and_then(|token| find_matching_close(parser, open + 1, parser.text_of(token)))
                .and_then(|close| close.checked_add(1))
        })
        .unwrap_or(end)
        .min(end);
    start_pattern(
        parser,
        node_end,
        SyntaxKind::VariantPattern,
        role,
        transaction,
        path,
    );
    let (head, name, mut issues) =
        project_variant_head(parser, transaction, path, parser.cursor(), head_end);
    if parser.at(".") {
        parser.bump();
        parser.bump_trivia();
    }
    emit_path(
        parser,
        head_end,
        SyntaxRole::Target,
        super::path::PathSeparatorGrammar::DottedOrQualified,
    );
    bump_until(parser, head_end);
    let payload = match payload.and_then(|_| parser.current_text()) {
        Some("(") => emit_variant_tuple_payload(parser, node_end, transaction, path),
        Some("{") => emit_variant_record_payload(parser, node_end, transaction, path),
        _ => PatternVariantPayloadSyntax::Absent,
    };
    if let PatternVariantPayloadSyntax::Recovered { issue, .. } = &payload {
        issues.push(PatternRecoveryIssue::VariantPayload(issue.clone()));
    }
    parser.finish();
    PatternSyntaxNode::new(
        PatternSyntaxKind::Variant(PatternVariantSyntax::new(head, name, payload)),
        PatternSyntaxState::from_issues(issues),
    )
}

fn emit_record_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    let open = boundary(parser, parser.cursor(), end, &["{"]).expect("classified record pattern");
    start_pattern(
        parser,
        end,
        SyntaxKind::RecordPattern,
        role,
        transaction,
        path,
    );
    let path_syntax = project_record_path(parser, transaction, path, parser.cursor(), open);
    if parser.cursor() < open {
        emit_path(
            parser,
            open,
            SyntaxRole::Target,
            super::path::PathSeparatorGrammar::DottedOrQualified,
        );
        bump_until(parser, open);
    }
    let fields = emit_record_fields(parser, end, transaction, path);
    parser.finish();
    let mut issues = fields.issues;
    if fields.missing_close {
        issues.push(PatternRecoveryIssue::MissingCloseDelimiter);
    }
    PatternSyntaxNode::new(
        PatternSyntaxKind::Record(PatternRecordSyntax::new(path_syntax, fields.fields)),
        PatternSyntaxState::from_issues(issues),
    )
}

fn emit_variant_tuple_payload(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
) -> PatternVariantPayloadSyntax {
    stage_variant_payload(parser, transaction, owner, end);
    let path = owner.child(PatternNodeStep::VariantPayload);
    start_pattern(
        parser,
        end,
        SyntaxKind::TuplePattern,
        SyntaxRole::Pattern,
        transaction,
        &path,
    );
    let missing_close =
        find_matching_close(parser, parser.cursor() + 1, "(").is_none_or(|close| close >= end);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    let close = find_matching_close(parser, parser.cursor(), "(")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    let elements = emit_pattern_list(parser, close, ")", transaction, &path);
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.pattern.missing_variant_close",
    );
    parser.finish();
    let child = PatternSyntaxNode::new(
        PatternSyntaxKind::Tuple(elements.into_boxed_slice()),
        PatternSyntaxState::from_issues(
            missing_close
                .then_some(PatternRecoveryIssue::MissingCloseDelimiter)
                .into_iter()
                .collect(),
        ),
    );
    if missing_close {
        PatternVariantPayloadSyntax::Recovered {
            value: Some(Box::new(child)),
            issue: PatternVariantPayloadIssue::MissingCloseDelimiter,
        }
    } else if child.state().is_valid() {
        PatternVariantPayloadSyntax::Resolved(Box::new(child))
    } else {
        PatternVariantPayloadSyntax::Recovered {
            value: Some(Box::new(child)),
            issue: PatternVariantPayloadIssue::InvalidPattern,
        }
    }
}

fn emit_variant_record_payload(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
) -> PatternVariantPayloadSyntax {
    stage_variant_payload(parser, transaction, owner, end);
    let path = owner.child(PatternNodeStep::VariantPayload);
    start_pattern(
        parser,
        end,
        SyntaxKind::RecordPattern,
        SyntaxRole::Pattern,
        transaction,
        &path,
    );
    let fields = emit_record_fields(parser, end, transaction, &path);
    parser.finish();
    let mut issues = fields.issues;
    if fields.missing_close {
        issues.push(PatternRecoveryIssue::MissingCloseDelimiter);
    }
    let child = PatternSyntaxNode::new(
        PatternSyntaxKind::Record(PatternRecordSyntax::new(
            crate::patterns::PatternPathSyntax::Absent,
            fields.fields,
        )),
        PatternSyntaxState::from_issues(issues),
    );
    if fields.missing_close {
        PatternVariantPayloadSyntax::Recovered {
            value: Some(Box::new(child)),
            issue: PatternVariantPayloadIssue::MissingCloseDelimiter,
        }
    } else if child.state().is_valid() {
        PatternVariantPayloadSyntax::Resolved(Box::new(child))
    } else {
        PatternVariantPayloadSyntax::Recovered {
            value: Some(Box::new(child)),
            issue: PatternVariantPayloadIssue::InvalidPattern,
        }
    }
}

fn emit_tuple_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    start_pattern(
        parser,
        end,
        SyntaxKind::TuplePattern,
        role,
        transaction,
        path,
    );
    let missing_close =
        find_matching_close(parser, parser.cursor() + 1, "(").is_none_or(|close| close >= end);
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    let close = find_matching_close(parser, parser.cursor(), "(")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    let elements = emit_pattern_list(parser, close, ")", transaction, path);
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.pattern.missing_tuple_close",
    );
    parser.finish();
    PatternSyntaxNode::new(
        PatternSyntaxKind::Tuple(elements.into_boxed_slice()),
        PatternSyntaxState::from_issues(
            missing_close
                .then_some(PatternRecoveryIssue::MissingCloseDelimiter)
                .into_iter()
                .collect(),
        ),
    )
}

fn emit_sequence_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    start_pattern(
        parser,
        end,
        SyntaxKind::SequencePattern,
        role,
        transaction,
        path,
    );
    let missing_close =
        find_matching_close(parser, parser.cursor() + 1, "[").is_none_or(|close| close >= end);
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    let close = find_matching_close(parser, parser.cursor(), "[")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::ParameterList, SyntaxRole::Element(0));
    let mut ordinal = 0_u32;
    let mut elements = Vec::new();
    let mut rest = PatternSequenceRestSyntax::Absent;
    let mut rest_ordinal = 0_u32;
    let mut issues = Vec::new();
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at("]") {
            break;
        }
        let element_end = find_top_level_boundary(parser, parser.cursor(), close, &[",", "]"]);
        if parser.at("..") {
            if matches!(rest, PatternSequenceRestSyntax::Absent) {
                let (projected, rest_issues) =
                    stage_sequence_rest(parser, transaction, path, parser.cursor(), element_end);
                rest = projected;
                issues.extend(rest_issues);
            } else {
                let issue = PatternSequenceRestIssue::MultipleRest {
                    ordinal: rest_ordinal,
                };
                rest = rest.recover(issue.clone());
                issues.push(PatternRecoveryIssue::SequenceRest(issue));
            }
            rest_ordinal = rest_ordinal
                .checked_add(1)
                .expect("grammar limits fit sequence-rest ordinals");
            emit_rest_pattern(parser, element_end, SyntaxRole::Element(ordinal));
        } else {
            transaction.component(
                path,
                PatternComponentRole::Element { ordinal },
                significant_range(parser, parser.cursor(), element_end),
            );
            elements.push(emit_pattern_node(
                parser,
                element_end,
                SyntaxRole::Element(ordinal),
                transaction,
                &path.child(PatternNodeStep::Element(ordinal)),
            ));
            ordinal = ordinal
                .checked_add(1)
                .expect("grammar limits fit Pattern element ordinals");
        }
        bump_until(parser, element_end);
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.pattern.missing_sequence_close",
    );
    parser.finish();
    if missing_close {
        issues.push(PatternRecoveryIssue::MissingCloseDelimiter);
    }
    PatternSyntaxNode::new(
        PatternSyntaxKind::BracketSequence(PatternSequenceSyntax::new(elements, rest)),
        PatternSyntaxState::from_issues(issues),
    )
}

struct RecordFieldsResult {
    fields: Vec<PatternRecordFieldSyntax>,
    issues: Vec<PatternRecoveryIssue>,
    missing_close: bool,
}

fn emit_record_fields(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
) -> RecordFieldsResult {
    let missing_close =
        find_matching_close(parser, parser.cursor() + 1, "{").is_none_or(|close| close >= end);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close(parser, parser.cursor(), "{")
        .unwrap_or(end)
        .min(end);
    parser.start(SyntaxKind::FieldList, SyntaxRole::Element(0));
    let mut ordinal = 0_u32;
    let mut fields = Vec::new();
    let mut issues = Vec::new();
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at("}") {
            break;
        }
        let field_end = find_top_level_boundary(parser, parser.cursor(), close, &[",", "}"]);
        if parser.at("..") {
            let role_ordinal =
                u16::try_from(ordinal).expect("grammar limits fit record-field role ordinals");
            let (field, issue) = stage_record_rest(
                parser,
                transaction,
                owner,
                ordinal,
                parser.cursor(),
                field_end,
            );
            fields.push(field);
            if let Some(issue) = issue {
                issues.push(PatternRecoveryIssue::InvalidRecordField { ordinal, issue });
            }
            emit_rest_pattern(parser, field_end, SyntaxRole::Field(role_ordinal));
        } else {
            let (field, issue) = emit_record_field(parser, field_end, ordinal, transaction, owner);
            fields.push(field);
            if let Some(issue) = issue {
                issues.push(PatternRecoveryIssue::InvalidRecordField { ordinal, issue });
            }
        }
        bump_until(parser, field_end);
        ordinal = ordinal
            .checked_add(1)
            .expect("grammar limits fit record-field ordinals");
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.pattern.missing_record_close",
    );
    RecordFieldsResult {
        fields,
        issues,
        missing_close,
    }
}

fn emit_record_field(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    field: u32,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
) -> (PatternRecordFieldSyntax, Option<PatternRecordFieldIssue>) {
    let ordinal = u16::try_from(field).expect("grammar limits fit record-field role ordinals");
    let whole = significant_range(parser, parser.cursor(), end);
    transaction.component(
        owner,
        PatternComponentRole::PatternField {
            field,
            part: PatternFieldPart::Whole,
        },
        whole,
    );
    parser.start(SyntaxKind::RecordPatternField, SyntaxRole::Field(ordinal));
    match boundary(parser, parser.cursor(), end, &[":"]) {
        Some(colon) => emit_explicit_record_field(parser, end, field, colon, transaction, owner),
        None => emit_shorthand_record_field(parser, end, field, whole, transaction, owner),
    }
}

fn emit_explicit_record_field(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    field: u32,
    colon: usize,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
) -> (PatternRecordFieldSyntax, Option<PatternRecordFieldIssue>) {
    let name = significant_range(parser, parser.cursor(), colon);
    let name_syntax = name_syntax(parser, first_significant(parser, parser.cursor(), colon));
    transaction.component(
        owner,
        PatternComponentRole::PatternField {
            field,
            part: PatternFieldPart::Name,
        },
        name,
    );
    if let Some(token) = parser.token_at(colon) {
        transaction.component(
            owner,
            PatternComponentRole::PatternField {
                field,
                part: PatternFieldPart::Colon,
            },
            token.range(),
        );
    }
    let nested_start = first_significant(parser, colon + 1, end).unwrap_or(end);
    let nested_range = if nested_start < end {
        significant_range(parser, nested_start, end)
    } else {
        let at = parser
            .offset_at_token_boundary(end)
            .unwrap_or_else(|| parser.current_offset());
        SourceRange::new(at, at)
    };
    transaction.component(
        owner,
        PatternComponentRole::PatternField {
            field,
            part: PatternFieldPart::Pattern,
        },
        nested_range,
    );
    parser.start(SyntaxKind::NameReference, SyntaxRole::Name);
    bump_until(parser, trimmed_end(parser, parser.cursor(), colon));
    parser.finish();
    bump_until(parser, colon);
    parser.bump();
    parser.bump_trivia();
    if nested_start >= end {
        parser.start(SyntaxKind::MissingPattern, SyntaxRole::Pattern);
        parser.finish();
        let issue = PatternRecordFieldIssue::MissingPattern;
        parser.finish();
        return (
            PatternRecordFieldSyntax::Invalid(PatternInvalidRecordFieldSyntax::new(
                name_syntax,
                issue.clone(),
                PatternRecordFieldShape::explicit(),
            )),
            Some(issue),
        );
    }
    let nested = emit_pattern_node(
        parser,
        end,
        SyntaxRole::Pattern,
        transaction,
        &owner.child(PatternNodeStep::RecordField(field)),
    );
    let issue = match &name_syntax {
        PatternNameSyntax::Resolved(_) => None,
        PatternNameSyntax::Recovered(issue) => {
            Some(PatternRecordFieldIssue::InvalidName(issue.clone()))
        }
        PatternNameSyntax::Absent => Some(PatternRecordFieldIssue::MissingName),
    };
    parser.finish();
    (
        PatternRecordFieldSyntax::Explicit {
            name: name_syntax,
            pattern: Box::new(nested),
        },
        issue,
    )
}

fn emit_shorthand_record_field(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    field: u32,
    whole: SourceRange,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
) -> (PatternRecordFieldSyntax, Option<PatternRecordFieldIssue>) {
    transaction.component(
        owner,
        PatternComponentRole::PatternField {
            field,
            part: PatternFieldPart::Name,
        },
        whole,
    );
    parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
    let (binding, binding_issues) = binding_syntax(parser, parser.cursor(), end);
    bump_until(parser, end);
    parser.finish();
    let issue = binding_issues.into_iter().find_map(|issue| match issue {
        PatternRecoveryIssue::Binding(PatternBindingIssue::MissingName) => {
            Some(PatternRecordFieldIssue::MissingName)
        }
        PatternRecoveryIssue::Binding(PatternBindingIssue::InvalidName(issue)) => {
            Some(PatternRecordFieldIssue::InvalidName(issue))
        }
        PatternRecoveryIssue::Binding(
            issue @ (PatternBindingIssue::ReservedBindingKeyword { .. }
            | PatternBindingIssue::UnexpectedTrailingInput { .. }),
        ) => Some(PatternRecordFieldIssue::InvalidBinding(issue)),
        _ => None,
    });
    parser.finish();
    (PatternRecordFieldSyntax::Shorthand(binding), issue)
}

fn emit_pattern_list(
    parser: &mut DocumentParser<'_, '_>,
    close: usize,
    delimiter: &str,
    transaction: &mut PatternProjectionTransaction,
    owner: &PatternNodePath,
) -> Vec<PatternSyntaxNode> {
    let mut ordinal = 0_u32;
    let mut elements = Vec::new();
    loop {
        parser.bump_trivia();
        if parser.cursor() >= close || parser.at(delimiter) {
            break;
        }
        let element_end =
            find_top_level_boundary(parser, parser.cursor(), close, &[",", delimiter]);
        transaction.component(
            owner,
            PatternComponentRole::Element { ordinal },
            significant_range(parser, parser.cursor(), element_end),
        );
        elements.push(emit_pattern_node(
            parser,
            element_end,
            SyntaxRole::Element(ordinal),
            transaction,
            &owner.child(PatternNodeStep::Element(ordinal)),
        ));
        bump_until(parser, element_end);
        ordinal = ordinal
            .checked_add(1)
            .expect("grammar limits fit Pattern element ordinals");
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    elements
}

fn emit_binding_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    start_pattern(
        parser,
        end,
        SyntaxKind::BindingPattern,
        role,
        transaction,
        path,
    );
    transaction.component(
        path,
        PatternComponentRole::Name,
        significant_range(parser, parser.cursor(), end),
    );
    let (binding, issues) = binding_syntax(parser, parser.cursor(), end);
    parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
    bump_until(parser, end);
    parser.finish();
    parser.finish();
    PatternSyntaxNode::new(
        PatternSyntaxKind::Binding(binding),
        PatternSyntaxState::from_issues(issues),
    )
}

fn emit_mutable_binding_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    start_pattern(
        parser,
        end,
        SyntaxKind::MutableBindingPattern,
        role,
        transaction,
        path,
    );
    if let Some(token) = parser.current() {
        transaction.component(path, PatternComponentRole::MutKeyword, token.range());
    }
    parser.bump();
    parser.bump_trivia();
    transaction.component(
        path,
        PatternComponentRole::Name,
        significant_range(parser, parser.cursor(), end),
    );
    let (binding, issues) = binding_syntax(parser, parser.cursor(), end);
    parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
    bump_until(parser, end);
    parser.finish();
    parser.finish();
    PatternSyntaxNode::new(
        PatternSyntaxKind::MutableBinding(binding),
        PatternSyntaxState::from_issues(issues),
    )
}

fn emit_rest_pattern(parser: &mut DocumentParser<'_, '_>, end: usize, role: SyntaxRole) {
    parser.start(SyntaxKind::RestPattern, role);
    parser.bump();
    parser.bump_trivia();
    if parser.cursor() < end {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        bump_until(parser, end);
        parser.finish();
    }
    parser.finish();
}

fn emit_invalid_rest_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    start_pattern(
        parser,
        end,
        SyntaxKind::RestPattern,
        role,
        transaction,
        path,
    );
    transaction.component(
        path,
        PatternComponentRole::Recovery,
        significant_range(parser, parser.cursor(), end),
    );
    parser.bump();
    parser.bump_trivia();
    if parser.cursor() < end {
        parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
        bump_until(parser, end);
        parser.finish();
    }
    parser.finish();
    PatternSyntaxNode::new(
        PatternSyntaxKind::Error,
        PatternSyntaxState::from_issues(vec![PatternRecoveryIssue::UnexpectedPattern]),
    )
}

fn emit_discard_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    start_pattern(
        parser,
        end,
        SyntaxKind::WildcardPattern,
        role,
        transaction,
        path,
    );
    bump_until(parser, end);
    parser.finish();
    PatternSyntaxNode::valid(PatternSyntaxKind::Discard)
}

fn emit_literal_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    let token = parser.current().expect("classified literal Pattern token");
    start_pattern(
        parser,
        end,
        SyntaxKind::LiteralPattern,
        role,
        transaction,
        path,
    );
    let (literal, issues) = project_literal(parser, transaction, path, token);
    bump_until(parser, end);
    parser.finish();
    PatternSyntaxNode::new(
        PatternSyntaxKind::Literal(literal),
        PatternSyntaxState::from_issues(issues),
    )
}

fn emit_entity_reference_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    let token = parser
        .current()
        .expect("classified entity-reference Pattern token");
    start_pattern(
        parser,
        end,
        SyntaxKind::EntityReferencePattern,
        role,
        transaction,
        path,
    );
    let (reference, issues) = project_id_ref(parser, transaction, path, token);
    bump_until(parser, end);
    parser.finish();
    PatternSyntaxNode::new(
        PatternSyntaxKind::EntityReference(reference),
        PatternSyntaxState::from_issues(issues),
    )
}

fn emit_error_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    start_pattern(
        parser,
        end,
        SyntaxKind::ErrorPattern,
        role,
        transaction,
        path,
    );
    transaction.component(
        path,
        PatternComponentRole::Recovery,
        significant_range(parser, parser.cursor(), end),
    );
    bump_until(parser, end);
    parser.finish();
    PatternSyntaxNode::new(
        PatternSyntaxKind::Error,
        PatternSyntaxState::from_issues(vec![PatternRecoveryIssue::UnexpectedPattern]),
    )
}

fn emit_typed_binding_pattern(
    parser: &mut DocumentParser<'_, '_>,
    colon: usize,
    end: usize,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) -> PatternSyntaxNode {
    start_pattern(
        parser,
        end,
        SyntaxKind::TypedBindingPattern,
        role,
        transaction,
        path,
    );
    transaction.component(
        path,
        PatternComponentRole::Name,
        significant_range(parser, parser.cursor(), colon),
    );
    let (binding, mut issues) = binding_syntax(parser, parser.cursor(), colon);
    parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
    bump_until(parser, colon);
    parser.finish();
    if let Some(token) = parser.token_at(colon) {
        transaction.component(path, PatternComponentRole::TypedBindingColon, token.range());
    }
    bump_until(parser, colon);
    parser.bump();
    parser.bump_trivia();
    transaction.component(
        path,
        PatternComponentRole::TypedBindingType,
        significant_range(parser, parser.cursor(), end),
    );
    let projection = emit_type(parser, end, SyntaxRole::Type);
    if matches!(
        projection.authored().value(),
        crate::types::TypeRef::Recovery(_)
    ) {
        issues.push(PatternRecoveryIssue::InvalidType);
    }
    transaction.type_child(path, &projection);
    parser.finish();
    PatternSyntaxNode::new(
        PatternSyntaxKind::TypedBinding(binding),
        PatternSyntaxState::from_issues(issues),
    )
}

fn start_pattern(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    kind: SyntaxKind,
    role: SyntaxRole,
    transaction: &mut PatternProjectionTransaction,
    path: &PatternNodePath,
) {
    transaction.start_node(
        parser,
        kind,
        role,
        path,
        significant_range(parser, parser.cursor(), end),
    );
}

fn typed_binding_colon(parser: &DocumentParser<'_, '_>, start: usize, end: usize) -> Option<usize> {
    let first = first_significant(parser, start, end)?;
    matches!(
        parser.token_at(first).map(super::lexer::LexToken::kind),
        Some(SyntaxKind::IdentifierToken | SyntaxKind::KeywordToken)
    )
    .then(|| boundary(parser, first + 1, end, &[":"]))
    .flatten()
}

fn whole_binding_rest(parser: &DocumentParser<'_, '_>, start: usize, end: usize) -> Option<usize> {
    let first = first_significant(parser, start, end)?;
    let token = parser.token_at(first)?;
    if token.kind() != SyntaxKind::IdentifierToken {
        return None;
    }
    let rest = first_significant(parser, first + 1, end)?;
    let rest_token = parser.token_at(rest)?;
    let rest_text = parser.text_of(rest_token);
    (matches!(rest_text, "." | "(" | "[")
        || rest_token.kind() == SyntaxKind::EntityReferenceToken
        || is_literal(rest_token.kind()))
    .then_some(rest)
}

fn is_variant_pattern(parser: &DocumentParser<'_, '_>, start: usize, end: usize) -> bool {
    if token_text(parser, start) == Some(".") {
        return true;
    }
    if has_adjacent_variant_separator(parser, start, end) {
        return true;
    }
    first_significant(parser, start, end)
        .and_then(|index| parser.token_at(index))
        .and_then(|token| BareExpectedTypeVariantGrammar::from_spelling(parser.text_of(token)))
        .is_some()
}

fn has_adjacent_variant_separator(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> bool {
    let Some(separator) = boundary(parser, start, end, &["."]) else {
        return false;
    };
    let previous = (start..separator).rev().find_map(|index| {
        parser
            .token_at(index)
            .filter(|token| !is_trivia(token.kind()))
    });
    let next = (separator + 1..end).find_map(|index| {
        parser
            .token_at(index)
            .filter(|token| !is_trivia(token.kind()))
    });
    let Some(separator) = parser.token_at(separator) else {
        return false;
    };
    previous.is_some_and(|token| token.range().end() == separator.range().start())
        && next.is_none_or(|token| token.range().start() == separator.range().end())
}

/// Parser-owned inventory of the four accepted bare expected-type spellings.
/// The semantic Pattern projection intentionally erases this grammar-only
/// classification to `BareExpectedType`; HIR and sema resolve the retained
/// authored name through the expected type instead of selecting Option/Result
/// here.
#[derive(Clone, Copy)]
enum BareExpectedTypeVariantGrammar {
    Some,
    None,
    Ok,
    Err,
}

impl BareExpectedTypeVariantGrammar {
    const fn from_spelling(spelling: &str) -> Option<Self> {
        Some(match spelling.as_bytes() {
            b"Some" => Self::Some,
            b"None" => Self::None,
            b"Ok" => Self::Ok,
            b"Err" => Self::Err,
            _ => return None,
        })
    }
}

fn boundary(
    parser: &DocumentParser<'_, '_>,
    start: usize,
    end: usize,
    spellings: &[&str],
) -> Option<usize> {
    let found = find_top_level_boundary(parser, start, end, spellings);
    (found < end).then_some(found)
}

const fn is_literal(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::NumberToken
            | SyntaxKind::StringToken
            | SyntaxKind::RawStringToken
            | SyntaxKind::CharacterToken
            | SyntaxKind::UnterminatedStringToken
    )
}

#[cfg(test)]
mod tests;
