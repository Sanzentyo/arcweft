//! Private predicate/proof block and statement grammar over the shared cursor.

pub(super) mod choice;
mod indentation;
pub(super) mod keyword;
mod trigger;

use arcweft_source::SourceRange;

use super::cursor::ShadowDocumentParser;
use super::expression::{
    emit_entity_reference, emit_expression, emit_expression_node, expression_is_call,
};
use super::pattern::emit_pattern;
use super::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_missing_delimiter, emit_open_delimiter,
    emit_required_punctuation, find_matching_close_before, find_statement_terminator,
    find_top_level_boundary, first_significant, token_count, token_text, trimmed_end,
};
use crate::assertion::AssertionMode;
use crate::expressions::SyntaxAwaitPropagation;
use crate::grammar::assertion_projection::PendingAssertionProjection;
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::keyword_statement_projection::{
    PendingAwaitBranchProjection, PendingKeywordStatementProjection, PendingSelectBranchProjection,
    SyntaxAwaitBranchKind, SyntaxSelectStatementForm,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

pub(super) fn emit_block_body(
    parser: &mut ShadowDocumentParser<'_, '_>,
    item_kind: SyntaxKind,
    body_kind: SyntaxKind,
    keyword: &str,
) {
    let block_kind = if item_kind == SyntaxKind::PredicateItem {
        SyntaxKind::PredicateBlock
    } else {
        SyntaxKind::ProofBlock
    };
    parser.start(body_kind, SyntaxRole::Body);
    emit_braced_block(
        parser,
        item_kind,
        block_kind,
        SyntaxRole::Body,
        if keyword == "predicate" {
            "syntax.predicate.missing_block_close"
        } else {
            "syntax.proof.missing_block_close"
        },
    );
    parser.finish();
}

pub(super) fn emit_braced_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    item_kind: SyntaxKind,
    block_kind: SyntaxKind,
    role: SyntaxRole,
    missing_close_code: &'static str,
) {
    let end = token_count(parser);
    emit_braced_block_until(parser, end, item_kind, block_kind, role, missing_close_code);
}

pub(super) fn emit_braced_block_until(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    block_kind: SyntaxKind,
    role: SyntaxRole,
    missing_close_code: &'static str,
) {
    let _ = emit_braced_block_until_with_kind(
        parser,
        end,
        item_kind,
        block_kind,
        role,
        missing_close_code,
        BlockSequenceKind::Value,
    );
}

/// Emits a braced body whose final expression is an ordinary statement.
///
/// Source handlers use this boundary because they never own a value tail.
/// The return value records whether the closing brace was authored.
pub(super) fn emit_braced_statement_block_until(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    block_kind: SyntaxKind,
    role: SyntaxRole,
    missing_close_code: &'static str,
) -> bool {
    emit_braced_block_until_with_kind(
        parser,
        end,
        item_kind,
        block_kind,
        role,
        missing_close_code,
        BlockSequenceKind::Statement,
    )
}

pub(super) fn emit_braced_statement_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    item_kind: SyntaxKind,
    block_kind: SyntaxKind,
    role: SyntaxRole,
    missing_close_code: &'static str,
) {
    let end = token_count(parser);
    let _ = emit_braced_block_until_with_kind(
        parser,
        end,
        item_kind,
        block_kind,
        role,
        missing_close_code,
        BlockSequenceKind::Statement,
    );
}

/// Emits the shared statement-only body used by ordinary Flow declarations
/// and Thread expressions.
///
/// Direct Dialogue content application remains an expression root. Every
/// other expression-shaped item is wrapped once as an `ExpressionStatement`;
/// no value tail or omitted-tail marker is produced.
pub(super) fn emit_braced_thread_flow_block_until(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    block_kind: SyntaxKind,
    role: SyntaxRole,
    missing_close_code: &'static str,
) -> bool {
    emit_braced_block_until_with_kind(
        parser,
        end,
        item_kind,
        block_kind,
        role,
        missing_close_code,
        BlockSequenceKind::ThreadFlow,
    )
}

pub(super) fn emit_braced_thread_flow_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    item_kind: SyntaxKind,
    block_kind: SyntaxKind,
    role: SyntaxRole,
    missing_close_code: &'static str,
) {
    let end = token_count(parser);
    let _ = emit_braced_thread_flow_block_until(
        parser,
        end,
        item_kind,
        block_kind,
        role,
        missing_close_code,
    );
}

fn emit_braced_block_until_with_kind(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    block_kind: SyntaxKind,
    role: SyntaxRole,
    missing_close_code: &'static str,
    sequence_kind: BlockSequenceKind,
) -> bool {
    parser.start(block_kind, role);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close_before(parser, parser.cursor(), end, "{").unwrap_or(end);
    emit_block_sequence(parser, close, item_kind, sequence_kind);

    let closed = parser.cursor() == close && parser.at("}");
    if closed {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.block.missing_close",
        );
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            missing_close_code,
            SourceRange::new(at, at),
            "missing closing `}` for block",
        )));
    }
    parser.finish();
    closed
}

/// Emits a callback-body Block after its closure header has consumed the
/// surrounding brace. The Block owns only the ordered body statements and
/// tail; the callback Closure remains the sole delimiter owner.
pub(super) fn emit_unbraced_block_until(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    block_kind: SyntaxKind,
    role: SyntaxRole,
) {
    parser.start(block_kind, role);
    emit_block_sequence(parser, end, item_kind, BlockSequenceKind::Value);
    parser.finish();
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BlockSequenceKind {
    Value,
    Statement,
    ThreadFlow,
    UnsafeAuditStatement,
}

fn emit_block_sequence(
    parser: &mut ShadowDocumentParser<'_, '_>,
    close: usize,
    item_kind: SyntaxKind,
    sequence_kind: BlockSequenceKind,
) {
    parser.start(SyntaxKind::StatementList, SyntaxRole::Element(0));
    let mut statement = 0_u32;
    let mut has_tail = false;

    while parser.cursor() < close {
        if sequence_kind == BlockSequenceKind::UnsafeAuditStatement {
            emit_unsafe_audit_trivia(parser);
        } else {
            parser.bump_trivia();
        }
        if parser.cursor() >= close {
            break;
        }
        let start = parser.cursor();
        let mut terminator = find_statement_terminator(parser, start, close);
        let choice_expression_start = if sequence_kind == BlockSequenceKind::ThreadFlow
            && parser.current_text() == Some("choice")
        {
            Some(start)
        } else {
            let_choice_initializer_start(parser, start, close)
        };
        if let Some(choice_start) = choice_expression_start {
            let choice_end = choice::logical_choice_end(parser, choice_start, start, close);
            let semicolon = first_significant(parser, choice_end, close)
                .filter(|index| token_text(parser, *index) == Some(";"));
            terminator = Some(semicolon.map_or((choice_end, false), |index| (index, true)));
        }
        let segment_end = terminator.map_or(close, |(index, _)| index);
        let significant_end = trimmed_end(parser, start, segment_end);
        let first = first_significant(parser, start, significant_end)
            .and_then(|index| token_text(parser, index));
        let semicolon = terminator.is_some_and(|(_, semicolon)| semicolon);
        let later = terminator
            .is_some_and(|(index, _)| first_significant(parser, index + 1, close).is_some());
        let unterminated_value_head = !semicolon
            && !later
            && first.is_some_and(|spelling| matches!(spelling, "if" | "loop" | "match" | "thread"));
        let statement_shaped = first.is_some_and(is_statement_head) && !unterminated_value_head;
        if sequence_kind != BlockSequenceKind::Value || semicolon || later || statement_shaped {
            let end = if choice_expression_start.is_some() {
                // Choice owns its indentation newline and comment geometry.
                // Trimming here changes MissingIndentedItem into
                // MissingNewline and can expose a dedented sibling to the
                // Choice emitter.
                segment_end
            } else if semicolon {
                terminator.map_or(segment_end, |(index, _)| index + 1)
            } else {
                significant_end
            };
            if sequence_kind == BlockSequenceKind::ThreadFlow {
                emit_thread_flow_item(parser, end, item_kind, statement);
            } else {
                emit_statement(parser, end, item_kind, statement);
            }
            statement = statement.saturating_add(1);
            bump_until(parser, segment_end);
            continue;
        }

        parser.finish();
        emit_item_expression(parser, significant_end, SyntaxRole::Tail, item_kind);
        bump_until(parser, close);
        has_tail = true;
        break;
    }

    if !has_tail {
        parser.finish();
        if sequence_kind == BlockSequenceKind::Value {
            parser.start(SyntaxKind::OmittedBlockTail, SyntaxRole::Tail);
            parser.finish();
        }
    }
}

fn emit_thread_flow_item(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    let role = SyntaxRole::ThreadFlowItem(ordinal);
    let kind = classify_thread_flow_item(parser, end, item_kind);
    if kind != SyntaxKind::ExpressionStatement {
        emit_statement_kind(parser, end, item_kind, role, kind, true);
        return;
    }

    let expression = emit_expression_node(parser, end, role);
    if parser.completed_kind(expression.start_event)
        == Some(SyntaxKind::DialogueContentApplicationExpression)
    {
        return;
    }

    parser.insert_start(
        expression.start_event,
        SyntaxKind::ExpressionStatement,
        role,
    );
    parser.set_start_role(expression.start_event + 1, SyntaxRole::Initializer);
    parser.finish();
}

fn emit_unsafe_audit_trivia(parser: &mut ShadowDocumentParser<'_, '_>) {
    loop {
        match parser.current_kind() {
            Some(SyntaxKind::DocCommentToken)
                if parser.current_text().is_some_and(is_safety_documentation) =>
            {
                parser.start(SyntaxKind::DocBlock, SyntaxRole::Documentation);
                parser.start(SyntaxKind::LogicalLine, SyntaxRole::Element(0));
                parser.bump();
                parser.finish();
                parser.finish();
            }
            Some(
                SyntaxKind::WhitespaceToken
                | SyntaxKind::NewlineToken
                | SyntaxKind::CommentToken
                | SyntaxKind::DocCommentToken,
            ) => {
                parser.bump();
            }
            _ => break,
        }
    }
}

fn is_safety_documentation(spelling: &str) -> bool {
    spelling
        .strip_prefix("///")
        .or_else(|| spelling.strip_prefix("/**"))
        .is_some_and(|body| body.trim_start().starts_with("SAFETY"))
}

fn emit_statement(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
) {
    emit_statement_with_role(parser, end, item_kind, SyntaxRole::Statement(ordinal));
}

/// Emits one ordinary statement fragment without inventing a declaration
/// owner. Proof/predicate restrictions remain owned by their document item.
pub(super) fn emit_statement_fragment(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
) {
    let end = trimmed_end(parser, parser.cursor(), end);
    emit_statement_with_role(parser, end, SyntaxKind::FunctionItem, role);
}

fn emit_statement_with_role(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    role: SyntaxRole,
) {
    let kind = classify_statement(parser, end, item_kind);
    emit_statement_kind(parser, end, item_kind, role, kind, false);
}

fn emit_statement_kind(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    role: SyntaxRole,
    kind: SyntaxKind,
    thread_flow_context: bool,
) {
    let child_end = if end > parser.cursor() && token_text(parser, end - 1) == Some(";") {
        end - 1
    } else {
        end
    };
    let projection_owner = if kind == SyntaxKind::AssertionStatement
        || crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection::kind_requires_projection(kind)
    {
        parser.start_projected_owner(kind, role)
    } else {
        parser.start(kind, role);
        None
    };
    match kind {
        SyntaxKind::LetStatement
        | SyntaxKind::LetElseStatement
        | SyntaxKind::LetChoiceStatement
        | SyntaxKind::LetScopeStatement
        | SyntaxKind::LetLoopStatement
        | SyntaxKind::LetAwaitStatement
        | SyntaxKind::LetActionReceiveStatement => {
            emit_let_children(parser, child_end, kind, item_kind);
        }
        SyntaxKind::AssertionStatement => {
            let projection = emit_assertion_children(parser, child_end);
            parser.set_assertion_projection(projection_owner, projection);
        }
        SyntaxKind::AssignmentStatement | SyntaxKind::LifetimeSetStatement => {
            emit_assignment_children(parser, child_end, item_kind);
        }
        SyntaxKind::WaitStatement => emit_wait_children(parser, child_end),
        SyntaxKind::OnStatement => emit_on_children(parser, child_end, item_kind),
        SyntaxKind::ThreadStatement
        | SyntaxKind::DeferBlockStatement
        | SyntaxKind::UnsafeLifetimeStatement
        | SyntaxKind::IfStatement
        | SyntaxKind::LoopStatement
        | SyntaxKind::WhileStatement
        | SyntaxKind::WhileLetStatement
        | SyntaxKind::ForStatement
        | SyntaxKind::MatchStatement => {
            emit_control_children(parser, child_end, item_kind, kind, thread_flow_context);
        }
        SyntaxKind::ReturnStatement | SyntaxKind::YieldStatement | SyntaxKind::CloseStatement => {
            emit_required_keyword_operand(parser, child_end, item_kind);
        }
        SyntaxKind::SelectStatement => {
            let projection = emit_select_statement_children(parser, child_end, item_kind);
            parser.set_keyword_statement_projection(projection_owner, projection);
        }
        SyntaxKind::OutStatement
        | SyntaxKind::GotoStatement
        | SyntaxKind::DeferStatement
        | SyntaxKind::SignalStatement
        | SyntaxKind::BreakStatement
        | SyntaxKind::ContinueStatement => {
            let projection = keyword::emit_keyword_statement(parser, child_end, item_kind, kind);
            parser.set_keyword_statement_projection(projection_owner, projection);
        }
        SyntaxKind::ProofCallStatement => {
            emit_expression(parser, child_end, SyntaxRole::Callee);
        }
        SyntaxKind::ChoiceStatement => {
            emit_choice_statement_children(parser, child_end, item_kind);
            parser.set_keyword_statement_projection(
                projection_owner,
                PendingKeywordStatementProjection::Choice,
            );
        }
        SyntaxKind::SourceLocaleStatement => {
            let projection = emit_source_locale_statement_children(parser, child_end, item_kind);
            parser.set_keyword_statement_projection(projection_owner, projection);
        }
        SyntaxKind::ScopeStatement => {
            let projection = emit_scope_statement_children(parser, child_end, item_kind);
            parser.set_keyword_statement_projection(projection_owner, projection);
        }
        SyntaxKind::IncludeStatement => {
            emit_include_statement_children(parser, child_end);
            parser.set_keyword_statement_projection(
                projection_owner,
                PendingKeywordStatementProjection::Include,
            );
        }
        SyntaxKind::AwaitWithStatement => {
            let projection = emit_await_with_statement_children(parser, child_end, item_kind);
            parser.set_keyword_statement_projection(projection_owner, projection);
        }
        SyntaxKind::ExpressionStatement => {
            emit_item_expression(parser, child_end, SyntaxRole::Initializer, item_kind);
        }
        _ => bump_until(parser, child_end),
    }
    bump_until(parser, end);
    parser.finish();
}

fn classify_thread_flow_item(
    parser: &ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) -> SyntaxKind {
    let start = parser.cursor();
    match parser.current_text() {
        Some("choice") => SyntaxKind::ChoiceStatement,
        Some("source")
            if next_significant_text(parser, start.saturating_add(1), end) == Some("locale") =>
        {
            SyntaxKind::SourceLocaleStatement
        }
        Some("scope") | Some("{") => SyntaxKind::ScopeStatement,
        Some("include") => SyntaxKind::IncludeStatement,
        Some("await") if top_level_operator(parser, start, end, "with").is_some() => {
            SyntaxKind::AwaitWithStatement
        }
        Some("try")
            if next_significant_text(parser, start.saturating_add(1), end) == Some("await")
                && top_level_operator(parser, start, end, "with").is_some() =>
        {
            SyntaxKind::AwaitWithStatement
        }
        _ => classify_statement(parser, end, item_kind),
    }
}

fn emit_choice_statement_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    _item_kind: SyntaxKind,
) {
    let _ = emit_expression_node(parser, end, SyntaxRole::Initializer);
}

fn emit_source_locale_statement_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) -> PendingKeywordStatementProjection {
    let open = find_statement_open_brace(parser, parser.cursor(), end);
    parser.bump();
    parser.bump_trivia();
    if parser.at("locale") {
        parser.bump();
    } else {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.source_locale.missing_locale_keyword",
            SourceRange::new(at, at),
            "expected `locale` after `source`",
        )));
    }
    parser.bump_trivia();
    let locale_end = open.unwrap_or(end);
    let locale_end = trimmed_end(parser, parser.cursor(), locale_end);
    let locale = if parser.cursor() < locale_end {
        let start = parser.current_offset();
        let finish = parser
            .token_at(locale_end - 1)
            .expect("trimmed locale interval retains its final token")
            .range()
            .end();
        let locale = arcweft_id::LocaleTag::try_new(&parser.source()[start..finish]);
        parser.start(SyntaxKind::NameReference, SyntaxRole::Value);
        bump_until(parser, locale_end);
        parser.finish();
        Some(locale)
    } else {
        parser.start(SyntaxKind::MissingName, SyntaxRole::Value);
        parser.finish();
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.source_locale.missing_locale",
            SourceRange::new(at, at),
            "missing source LocaleTag",
        )));
        None
    };
    bump_until(parser, open.unwrap_or(end));
    if open.is_some() {
        emit_braced_thread_flow_block_until(
            parser,
            end,
            item_kind,
            SyntaxKind::Block,
            SyntaxRole::Body,
            "syntax.source_locale.missing_block_close",
        );
    } else {
        emit_required_statement_body_recovery(
            parser,
            "syntax.source_locale.missing_body",
            "missing source-locale body",
        );
    }
    PendingKeywordStatementProjection::SourceLocale { locale }
}

fn emit_scope_statement_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) -> PendingKeywordStatementProjection {
    let open = find_statement_open_brace(parser, parser.cursor(), end);
    let mut name = None;
    if parser.at("scope") {
        parser.bump();
        parser.bump_trivia();
        if parser.cursor() < open.unwrap_or(end) {
            let token = parser
                .current()
                .expect("authored Scope name retains one source token");
            name = Some(crate::name::SyntaxName::try_new(parser.text_of(token)));
            parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
            parser.bump();
            parser.finish();
            parser.bump_trivia();
            if parser.cursor() < open.unwrap_or(end) {
                parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
                bump_until(
                    parser,
                    trimmed_end(parser, parser.cursor(), open.unwrap_or(end)),
                );
                parser.finish();
            }
        }
        bump_until(parser, open.unwrap_or(end));
    }
    if open.is_some() {
        emit_braced_thread_flow_block_until(
            parser,
            end,
            item_kind,
            SyntaxKind::Block,
            SyntaxRole::Body,
            "syntax.scope.missing_block_close",
        );
    } else {
        emit_required_statement_body_recovery(
            parser,
            "syntax.scope.missing_body",
            "missing lexical Scope body",
        );
    }
    PendingKeywordStatementProjection::Scope { name }
}

fn emit_include_statement_children(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.bump();
    parser.bump_trivia();
    if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) {
        emit_entity_reference(parser, SyntaxRole::Target);
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingExpression, SyntaxRole::Target);
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.include.missing_target",
            SourceRange::new(at, at),
            "missing Flow entity reference after `include`",
        )));
    }
    parser.bump_trivia();
    if parser.cursor() < end {
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        bump_until(parser, end);
        parser.finish();
    }
}

fn emit_await_with_statement_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) -> PendingKeywordStatementProjection {
    let with = top_level_operator(parser, parser.cursor(), end, "with").unwrap_or(end);
    let mut propagation = SyntaxAwaitPropagation::PreserveResult;
    if parser.at("try") {
        propagation = SyntaxAwaitPropagation::PropagateError;
        parser.bump();
        parser.bump_trivia();
    }
    if parser.at("await") {
        parser.bump();
        if parser.at("?") {
            propagation = SyntaxAwaitPropagation::PropagateError;
            parser.bump();
        }
    }
    parser.bump_trivia();
    emit_expression(parser, with, SyntaxRole::Operand);
    bump_until(parser, with);
    if parser.at("with") {
        parser.bump();
        parser.bump_trivia();
    }
    if parser.at("{") {
        let branches = emit_await_with_branch_block(parser, end, item_kind);
        return PendingKeywordStatementProjection::AwaitWith {
            propagation,
            branches: branches.into_boxed_slice(),
        };
    } else {
        emit_required_statement_body_recovery(
            parser,
            "syntax.await_with.missing_body",
            "missing AwaitWith branch body",
        );
    }
    PendingKeywordStatementProjection::AwaitWith {
        propagation,
        branches: Box::new([]),
    }
}

fn emit_select_statement_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) -> PendingKeywordStatementProjection {
    let open = find_statement_open_brace(parser, parser.cursor(), end);
    if open.is_none() {
        emit_required_keyword_operand(parser, end, item_kind);
        return PendingKeywordStatementProjection::Select {
            form: SyntaxSelectStatementForm::Operand,
            branches: Box::new([]),
        };
    }
    parser.bump();
    parser.bump_trivia();
    bump_until(parser, open.expect("checked Select opening brace"));
    let branches = emit_select_branch_block(parser, end, item_kind);
    PendingKeywordStatementProjection::Select {
        form: SyntaxSelectStatementForm::BranchBlock,
        branches: branches.into_boxed_slice(),
    }
}

fn emit_select_branch_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) -> Vec<PendingSelectBranchProjection> {
    parser.start(SyntaxKind::Block, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close_before(parser, parser.cursor(), end, "{").unwrap_or(end);
    let mut projections = Vec::new();

    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }
        let ordinal = u32::try_from(projections.len()).unwrap_or(u32::MAX);
        let start = parser.cursor();
        let Some(arrow) = find_branch_separator(parser, start, close) else {
            projections.push(emit_invalid_branch(
                parser,
                close,
                SyntaxKind::SelectBranch,
                ordinal,
                "syntax.select.invalid_branch",
                "select branch requires a typed head and `=>` body separator",
            ));
            continue;
        };

        parser.start(SyntaxKind::SelectBranch, SyntaxRole::Branch(ordinal));
        let projection = emit_select_branch_head(parser, arrow, item_kind);
        bump_until(parser, arrow);
        parser.bump();
        parser.bump_trivia();
        emit_required_branch_body(
            parser,
            close,
            item_kind,
            "syntax.select.missing_branch_body",
            "missing Select branch body",
            "syntax.select.missing_branch_close",
        );
        parser.finish();
        projections.push(projection);
    }

    finish_thread_branch_block(
        parser,
        close,
        "syntax.select.missing_block_close",
        "missing closing `}` for Select branch block",
    );
    projections
}

fn emit_select_branch_head(
    parser: &mut ShadowDocumentParser<'_, '_>,
    arrow: usize,
    item_kind: SyntaxKind,
) -> PendingSelectBranchProjection {
    match parser.current_text() {
        Some("frame") => {
            parser.bump();
            parser.bump_trivia();
            emit_pattern(parser, arrow, SyntaxRole::Pattern);
            PendingSelectBranchProjection::Frame
        }
        Some("event") => {
            parser.bump();
            parser.bump_trivia();
            emit_pattern(parser, arrow, SyntaxRole::Pattern);
            PendingSelectBranchProjection::Event
        }
        _ => {
            let Some(equals) = top_level_operator(parser, parser.cursor(), arrow, "=") else {
                parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
                bump_until(parser, arrow);
                parser.finish();
                return PendingSelectBranchProjection::Error;
            };
            let name_end = trimmed_end(parser, parser.cursor(), equals);
            let name = if parser.cursor() < name_end {
                let start = parser.current_offset();
                let finish = parser
                    .token_at(name_end - 1)
                    .expect("Select binding name retains its final token")
                    .range()
                    .end();
                let name = crate::name::SyntaxName::try_new(&parser.source()[start..finish]);
                parser.start(SyntaxKind::NameDefinition, SyntaxRole::Name);
                bump_until(parser, name_end);
                parser.finish();
                name
            } else {
                parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
                parser.finish();
                Err(crate::name::SyntaxNameIssue::Missing)
            };
            bump_until(parser, equals);
            emit_required_punctuation(
                parser,
                SyntaxKind::EqualsNode,
                SyntaxRole::Equals,
                "=",
                "syntax.select.missing_binding_equals",
                "missing `=` in Select binding branch",
            );
            parser.bump_trivia();
            let source_end = trimmed_end(parser, parser.cursor(), arrow);
            let propagates_error =
                source_end > parser.cursor() && token_text(parser, source_end - 1) == Some("?");
            let expression_end = if propagates_error {
                trimmed_end(parser, parser.cursor(), source_end - 1)
            } else {
                source_end
            };
            emit_item_expression(parser, expression_end, SyntaxRole::Initializer, item_kind);
            bump_until(parser, arrow);
            PendingSelectBranchProjection::Bind {
                name,
                propagates_error,
            }
        }
    }
}

fn emit_await_with_branch_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) -> Vec<PendingAwaitBranchProjection> {
    parser.start(SyntaxKind::Block, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close_before(parser, parser.cursor(), end, "{").unwrap_or(end);
    let mut projections = Vec::new();

    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }
        let ordinal = u32::try_from(projections.len()).unwrap_or(u32::MAX);
        let start = parser.cursor();
        let Some(arrow) = find_branch_separator(parser, start, close) else {
            emit_invalid_await_branch(
                parser,
                close,
                ordinal,
                "syntax.await_with.invalid_branch",
                "AwaitWith branch requires a known head and `=>` body separator",
            );
            projections.push(PendingAwaitBranchProjection::recovered());
            continue;
        };
        let kind = match parser.current_text() {
            Some("pending") => SyntaxAwaitBranchKind::Pending,
            Some("ready") => SyntaxAwaitBranchKind::Ready,
            Some("error") => SyntaxAwaitBranchKind::Error,
            Some("denied") => SyntaxAwaitBranchKind::Denied,
            _ => {
                emit_invalid_await_branch(
                    parser,
                    close,
                    ordinal,
                    "syntax.await_with.unknown_branch",
                    "unknown AwaitWith branch kind",
                );
                projections.push(PendingAwaitBranchProjection::recovered());
                continue;
            }
        };
        parser.start(SyntaxKind::AwaitWithBranch, SyntaxRole::Branch(ordinal));
        parser.bump();
        parser.bump_trivia();
        emit_pattern(parser, arrow, SyntaxRole::Pattern);
        bump_until(parser, arrow);
        parser.bump();
        parser.bump_trivia();
        emit_required_branch_body(
            parser,
            close,
            item_kind,
            "syntax.await_with.missing_branch_body",
            "missing AwaitWith branch body",
            "syntax.await_with.missing_branch_close",
        );
        parser.finish();
        projections.push(PendingAwaitBranchProjection::new(kind));
    }

    finish_thread_branch_block(
        parser,
        close,
        "syntax.await_with.missing_block_close",
        "missing closing `}` for AwaitWith branch block",
    );
    projections
}

fn emit_invalid_await_branch(
    parser: &mut ShadowDocumentParser<'_, '_>,
    close: usize,
    ordinal: u32,
    code: &'static str,
    message: &'static str,
) {
    parser.start(SyntaxKind::AwaitWithBranch, SyntaxRole::Branch(ordinal));
    emit_invalid_branch_contents(parser, close, code, message);
    parser.finish();
}

fn find_branch_separator(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    block_end: usize,
) -> Option<usize> {
    let head_end =
        find_statement_terminator(parser, start, block_end).map_or(block_end, |(end, _)| end);
    top_level_operator(parser, start, head_end, "=>")
}

fn emit_invalid_branch(
    parser: &mut ShadowDocumentParser<'_, '_>,
    close: usize,
    kind: SyntaxKind,
    ordinal: u32,
    code: &'static str,
    message: &'static str,
) -> PendingSelectBranchProjection {
    parser.start(kind, SyntaxRole::Branch(ordinal));
    emit_invalid_branch_contents(parser, close, code, message);
    parser.finish();
    PendingSelectBranchProjection::Error
}

fn emit_invalid_branch_contents(
    parser: &mut ShadowDocumentParser<'_, '_>,
    close: usize,
    code: &'static str,
    message: &'static str,
) {
    let terminator = find_statement_terminator(parser, parser.cursor(), close);
    let end = terminator.map_or(close, |(index, _)| index);
    let start = parser.current_offset();
    parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
    bump_until(parser, trimmed_end(parser, parser.cursor(), end));
    parser.finish();
    bump_until(parser, end);
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(start, at),
        message,
    )));
    if terminator.is_some_and(|(_, semicolon)| semicolon) && parser.at(";") {
        parser.bump();
    }
}

fn emit_required_branch_body(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    missing_code: &'static str,
    missing_message: &'static str,
    missing_close_code: &'static str,
) {
    if parser.at("{") {
        emit_braced_thread_flow_block_until(
            parser,
            end,
            item_kind,
            SyntaxKind::Block,
            SyntaxRole::Body,
            missing_close_code,
        );
    } else {
        emit_required_statement_body_recovery(parser, missing_code, missing_message);
    }
}

fn finish_thread_branch_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    close: usize,
    missing_close_code: &'static str,
    missing_close_message: &'static str,
) {
    if parser.cursor() == close && parser.at("}") {
        emit_close_delimiter(parser, SyntaxKind::CloseBraceNode, "}", missing_close_code);
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            missing_close_code,
            SourceRange::new(at, at),
            missing_close_message,
        )));
    }
    parser.finish();
}

fn emit_required_statement_body_recovery(
    parser: &mut ShadowDocumentParser<'_, '_>,
    code: &'static str,
    message: &'static str,
) {
    let at = parser.current_offset();
    parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
    parser.finish();
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code,
        SourceRange::new(at, at),
        message,
    )));
}

fn classify_statement(
    parser: &ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) -> SyntaxKind {
    let start = parser.cursor();
    if matches!(item_kind, SyntaxKind::PredicateItem | SyntaxKind::ProofItem) {
        return match parser.current_text() {
            Some("let") => SyntaxKind::LetStatement,
            Some("assert") => SyntaxKind::AssertionStatement,
            _ if item_kind == SyntaxKind::ProofItem && expression_is_call(parser, start, end) => {
                SyntaxKind::ProofCallStatement
            }
            _ => SyntaxKind::ErrorStatement,
        };
    }
    match parser.current_text() {
        Some("assert") => SyntaxKind::AssertionStatement,
        Some("let") => classify_let_statement(parser, end),
        Some("return") => SyntaxKind::ReturnStatement,
        Some("out") => SyntaxKind::OutStatement,
        Some("goto") => SyntaxKind::GotoStatement,
        Some("thread") => SyntaxKind::ThreadStatement,
        Some("defer") if find_statement_open_brace(parser, start, end).is_some() => {
            SyntaxKind::DeferBlockStatement
        }
        Some("defer") => SyntaxKind::DeferStatement,
        Some("yield") => SyntaxKind::YieldStatement,
        // A path or postfix continuation belongs to the Pratt expression
        // owner. Other `signal` heads retain statement recovery, including a
        // misspelled or omitted `<-` separator.
        Some("signal")
            if !matches!(
                next_significant_text(parser, start + 1, end),
                Some("(" | "[" | "." | "?" | "::")
            ) =>
        {
            SyntaxKind::SignalStatement
        }
        Some("wait") => SyntaxKind::WaitStatement,
        Some("on") => SyntaxKind::OnStatement,
        Some("unsafe") => SyntaxKind::UnsafeLifetimeStatement,
        Some("if") => SyntaxKind::IfStatement,
        Some("loop") => SyntaxKind::LoopStatement,
        Some("while") if next_significant_text(parser, start + 1, end) == Some("let") => {
            SyntaxKind::WhileLetStatement
        }
        Some("while") => SyntaxKind::WhileStatement,
        Some("for") => SyntaxKind::ForStatement,
        Some("match") => SyntaxKind::MatchStatement,
        Some("close") => SyntaxKind::CloseStatement,
        Some("select") => SyntaxKind::SelectStatement,
        Some("break") => SyntaxKind::BreakStatement,
        Some("continue") => SyntaxKind::ContinueStatement,
        _ if top_level_operator(parser, start, end, "<-").is_some() => {
            SyntaxKind::LifetimeSetStatement
        }
        _ if top_level_operator(parser, start, end, "=").is_some() => {
            SyntaxKind::AssignmentStatement
        }
        _ if item_kind == SyntaxKind::ProofItem && expression_is_call(parser, start, end) => {
            SyntaxKind::ProofCallStatement
        }
        _ if expression_statement_start(parser) => SyntaxKind::ExpressionStatement,
        _ => SyntaxKind::ErrorStatement,
    }
}

#[cfg(test)]
pub(super) fn parse_test_statement_block(
    document: &arcweft_source::SourceDocument,
) -> Result<crate::grammar::build::GrammarBuild, crate::grammar::build::GrammarBuildError> {
    let tokens = super::lexer::DocumentLexer::new(document.text()).lex();
    let mut events = Vec::with_capacity(tokens.len() + 8);
    let mut budget = crate::grammar::budget::GrammarBudget::default();
    assert!(budget.start(SyntaxKind::SourceFile, SyntaxRole::Root));
    events.push(SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root));
    {
        let mut parser =
            ShadowDocumentParser::new(document.text(), &tokens, &mut events, &mut budget);
        emit_braced_block(
            &mut parser,
            SyntaxKind::FunctionItem,
            SyntaxKind::Block,
            SyntaxRole::Body,
            "syntax.statement.missing_block_close",
        );
        while parser.bump().is_some() {}
    }
    let eof = SyntaxEvent::token(
        SyntaxKind::EofToken,
        SourceRange::new(document.text().len(), document.text().len()),
    );
    assert!(budget.event(&eof));
    events.push(eof);
    assert!(budget.finish());
    events.push(SyntaxEvent::FinishNode);
    crate::grammar::build::build_grammar(document, &events)
}

fn classify_let_statement(parser: &ShadowDocumentParser<'_, '_>, end: usize) -> SyntaxKind {
    let Some(equals) = top_level_operator(parser, parser.cursor(), end, "=") else {
        return SyntaxKind::LetStatement;
    };
    let initializer =
        first_significant(parser, equals + 1, end).and_then(|index| token_text(parser, index));
    if initializer != Some("if") && top_level_operator(parser, equals + 1, end, "else").is_some() {
        return SyntaxKind::LetElseStatement;
    }
    match initializer {
        Some("choice") => SyntaxKind::LetChoiceStatement,
        Some("scope") => SyntaxKind::LetScopeStatement,
        Some("loop") => SyntaxKind::LetLoopStatement,
        Some("await" | "try") => SyntaxKind::LetAwaitStatement,
        Some("receive") => SyntaxKind::LetActionReceiveStatement,
        _ => SyntaxKind::LetStatement,
    }
}

fn let_choice_initializer_start(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    if token_text(parser, start) != Some("let") {
        return None;
    }
    let equals = top_level_operator(parser, start.saturating_add(1), end, "=")?;
    let initializer = first_significant(parser, equals.saturating_add(1), end)?;
    (token_text(parser, initializer) == Some("choice")).then_some(initializer)
}

fn emit_let_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    kind: SyntaxKind,
    item_kind: SyntaxKind,
) {
    parser.bump();
    let equals = find_top_level_boundary(parser, parser.cursor(), &["="]).min(end);
    indentation::bump_trivia_before(parser, equals);
    emit_pattern(parser, equals, SyntaxRole::Pattern);
    bump_until(parser, equals);
    if equals < end && parser.cursor() == equals {
        parser.bump();
        let initializer_end = if kind == SyntaxKind::LetElseStatement {
            top_level_operator(parser, parser.cursor(), end, "else").unwrap_or(end)
        } else {
            end
        };
        indentation::bump_trivia_before(parser, initializer_end);
        emit_item_expression(parser, initializer_end, SyntaxRole::Initializer, item_kind);
        bump_until(parser, initializer_end);
        if parser.at("else") {
            parser.bump();
            parser.bump_trivia();
            if parser.at("{") {
                emit_braced_statement_block(
                    parser,
                    item_kind,
                    SyntaxKind::Block,
                    SyntaxRole::ElseBranch,
                    "syntax.statement.missing_let_else_close",
                );
            }
        }
    }
}

fn emit_assertion_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
) -> PendingAssertionProjection {
    let mode = assertion_mode(parser, end);
    parser.bump();
    parser.bump_trivia();
    if parser.at(".") {
        parser.bump();
    } else {
        parser.start(SyntaxKind::ErrorNode, SyntaxRole::Recovery(0));
        parser.finish();
    }
    parser.bump_trivia();
    if parser.current_kind() == Some(SyntaxKind::IdentifierToken) {
        parser.start(SyntaxKind::NameReference, SyntaxRole::Name);
        parser.bump();
        parser.finish();
    } else {
        parser.start(SyntaxKind::MissingName, SyntaxRole::Name);
        parser.finish();
    }
    parser.bump_trivia();

    let has_open = parser.at("(");
    if has_open {
        emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    } else {
        emit_missing_delimiter(parser, SyntaxKind::OpenParenNode, SyntaxRole::OpenDelimiter);
    }
    let close = if has_open {
        find_matching_close_before(parser, parser.cursor(), end, "(").unwrap_or(end)
    } else {
        end
    };
    parser.start(SyntaxKind::ExpressionList, SyntaxRole::Element(0));
    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }
        let condition_end =
            find_top_level_boundary(parser, parser.cursor(), &[",", ")"]).min(close);
        parser.charge_assertion_condition();
        emit_expression(parser, condition_end, SyntaxRole::Condition);
        bump_until(parser, condition_end);
        if parser.at(",") {
            parser.bump();
        } else {
            break;
        }
    }
    parser.finish();
    if parser.cursor() == close && parser.at(")") {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseParenNode,
            ")",
            "syntax.assert.unclosed_arguments",
        );
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseParenNode,
            SyntaxRole::CloseDelimiter,
        );
    }
    bump_until(parser, end);
    PendingAssertionProjection::new(mode)
}

fn assertion_mode(parser: &ShadowDocumentParser<'_, '_>, end: usize) -> Option<AssertionMode> {
    let first = first_significant(parser, parser.cursor().checked_add(1)?, end)?;
    let mode = if token_text(parser, first) == Some(".") {
        first_significant(parser, first.checked_add(1)?, end)?
    } else {
        first
    };
    AssertionMode::from_keyword(token_text(parser, mode)?)
}

fn emit_assignment_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) {
    let operator = top_level_operator(parser, parser.cursor(), end, "<-")
        .or_else(|| top_level_operator(parser, parser.cursor(), end, "="))
        .unwrap_or(end);
    emit_expression(parser, operator, SyntaxRole::Target);
    bump_until(parser, operator);
    if parser.cursor() < end {
        parser.bump();
        parser.bump_trivia();
    }
    emit_item_expression(parser, end, SyntaxRole::Initializer, item_kind);
}

fn emit_required_keyword_operand(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
) {
    parser.bump();
    parser.bump_trivia();
    emit_item_expression(parser, end, SyntaxRole::Operand, item_kind);
}

fn emit_wait_children(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.bump();
    parser.bump_trivia();
    emit_required_punctuation(
        parser,
        SyntaxKind::OpenParenNode,
        SyntaxRole::OpenDelimiter,
        "(",
        "syntax.statement.missing_wait_open",
        "wait requires an opening `(`",
    );
    parser.bump_trivia();
    let close = find_matching_close_before(parser, parser.cursor(), end, "(").unwrap_or(end);
    emit_expression(parser, close, SyntaxRole::Operand);
    bump_until(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.statement.missing_wait_close",
    );
}

fn emit_on_children(parser: &mut ShadowDocumentParser<'_, '_>, end: usize, item_kind: SyntaxKind) {
    parser.bump();
    parser.bump_trivia();
    let arrow = top_level_operator(parser, parser.cursor(), end, "=>").unwrap_or(end);
    emit_expression(parser, arrow, SyntaxRole::Condition);
    bump_until(parser, arrow);
    if parser.cursor() < end {
        parser.bump();
        parser.bump_trivia();
        emit_statement(parser, end, item_kind, 0);
    }
}

fn emit_control_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    kind: SyntaxKind,
    thread_flow_context: bool,
) {
    if kind == SyntaxKind::UnsafeLifetimeStatement {
        let open = find_statement_open_brace(parser, parser.cursor(), end);
        emit_unsafe_lifetime_children(parser, open, end, item_kind);
        return;
    }
    let open = find_statement_open_brace(parser, parser.cursor(), end);
    if kind == SyntaxKind::MatchStatement {
        let head_end = open.unwrap_or(end);
        emit_expression_head(parser, head_end, "match", SyntaxRole::Scrutinee);
        bump_until(parser, head_end);
        if open.is_some() {
            emit_match_block(parser, end, item_kind, thread_flow_context);
        } else {
            parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
            parser.finish();
        }
        return;
    }
    let Some(open) = open else {
        bump_until(parser, end);
        return;
    };

    match kind {
        SyntaxKind::IfStatement => {
            emit_if_children(parser, open, end, item_kind, thread_flow_context);
            return;
        }
        SyntaxKind::WhileLetStatement => {
            emit_pattern_condition_head(parser, open, "while");
        }
        SyntaxKind::WhileStatement => {
            emit_expression_head(parser, open, "while", SyntaxRole::Condition);
        }
        SyntaxKind::ForStatement => emit_for_head(parser, open),
        _ => bump_until(parser, open),
    }
    bump_until(parser, open);
    emit_nested_control_block(
        parser,
        end,
        item_kind,
        SyntaxRole::Body,
        "syntax.statement.missing_block_close",
        thread_flow_context,
    );
}

fn emit_unsafe_lifetime_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    open: Option<usize>,
    end: usize,
    item_kind: SyntaxKind,
) {
    let head_end = open.unwrap_or(end);
    debug_assert!(parser.at("unsafe"));
    parser.bump();
    parser.bump_trivia();
    if parser.at("lifetime") {
        parser.bump();
    } else {
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.statement.missing_unsafe_lifetime_keyword",
            SourceRange::new(at, at),
            "expected `lifetime` after `unsafe`",
        )));
    }
    parser.bump_trivia();

    if parser.current_kind() == Some(SyntaxKind::EntityReferenceToken) {
        emit_entity_reference(parser, SyntaxRole::Reference(0));
    } else {
        let at = parser.current_offset();
        parser.start(SyntaxKind::MissingExpression, SyntaxRole::Reference(0));
        parser.finish();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.statement.missing_unsafe_audit_id",
            SourceRange::new(at, at),
            "expected an unsafe-audit entity reference",
        )));
    }
    parser.bump_trivia();

    if parser.at("reason") {
        parser.bump();
        parser.bump_trivia();
        if parser.at("=") {
            parser.bump();
            parser.bump_trivia();
            emit_item_expression(parser, head_end, SyntaxRole::Initializer, item_kind);
            bump_until(parser, head_end);
        } else {
            let at = parser.current_offset();
            parser.start(SyntaxKind::MissingExpression, SyntaxRole::Initializer);
            parser.finish();
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                "syntax.statement.missing_unsafe_reason_equals",
                SourceRange::new(at, at),
                "expected `=` after unsafe-audit `reason`",
            )));
            bump_until(parser, head_end);
        }
    } else {
        bump_until(parser, head_end);
    }

    if open.is_some() {
        emit_braced_unsafe_audit_statement_block(parser, item_kind, end);
    } else {
        parser.start(SyntaxKind::MissingBody, SyntaxRole::Body);
        parser.finish();
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.statement.missing_unsafe_audit_body",
            SourceRange::new(at, at),
            "expected a braced unsafe lifetime body",
        )));
    }
}

fn emit_braced_unsafe_audit_statement_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    item_kind: SyntaxKind,
    end: usize,
) {
    parser.start(SyntaxKind::Block, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close_before(parser, parser.cursor(), end, "{").unwrap_or(end);
    emit_block_sequence(
        parser,
        close,
        item_kind,
        BlockSequenceKind::UnsafeAuditStatement,
    );
    if close < end {
        emit_close_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            "}",
            "syntax.statement.missing_unsafe_audit_close",
        );
    } else {
        emit_missing_delimiter(
            parser,
            SyntaxKind::CloseBraceNode,
            SyntaxRole::CloseDelimiter,
        );
        let at = parser.current_offset();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.statement.missing_unsafe_audit_close",
            SourceRange::new(at, at),
            "missing closing `}` for unsafe lifetime block",
        )));
    }
    parser.finish();
}

fn emit_if_children(
    parser: &mut ShadowDocumentParser<'_, '_>,
    open: usize,
    end: usize,
    item_kind: SyntaxKind,
    thread_flow_context: bool,
) {
    if next_significant_text(parser, parser.cursor() + 1, open) == Some("let") {
        emit_pattern_condition_head(parser, open, "if");
    } else {
        emit_expression_head(parser, open, "if", SyntaxRole::Condition);
    }
    bump_until(parser, open);
    emit_nested_control_block(
        parser,
        end,
        item_kind,
        SyntaxRole::ThenBranch,
        "syntax.statement.missing_if_close",
        thread_flow_context,
    );
    parser.bump_trivia();
    if parser.cursor() >= end || !parser.at("else") {
        return;
    }
    parser.bump();
    parser.bump_trivia();
    if parser.at("if") {
        parser.start(SyntaxKind::IfStatement, SyntaxRole::ElseBranch);
        if let Some(nested_open) = find_statement_open_brace(parser, parser.cursor(), end) {
            emit_if_children(parser, nested_open, end, item_kind, thread_flow_context);
        } else {
            bump_until(parser, end);
        }
        parser.finish();
    } else if parser.at("{") {
        emit_nested_control_block(
            parser,
            end,
            item_kind,
            SyntaxRole::ElseBranch,
            "syntax.statement.missing_else_close",
            thread_flow_context,
        );
    } else {
        bump_until(parser, end);
    }
}

fn emit_expression_head(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    keyword: &str,
    role: SyntaxRole,
) {
    debug_assert!(parser.at(keyword));
    parser.bump();
    parser.bump_trivia();
    emit_expression(parser, end, role);
}

fn emit_pattern_condition_head(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    keyword: &str,
) {
    debug_assert!(parser.at(keyword));
    parser.bump();
    parser.bump_trivia();
    if parser.at("let") {
        parser.bump();
        parser.bump_trivia();
    }
    let equals = top_level_operator(parser, parser.cursor(), end, "=").unwrap_or(end);
    emit_pattern(parser, equals, SyntaxRole::Pattern);
    bump_until(parser, equals);
    if parser.cursor() >= end {
        emit_expression(parser, end, SyntaxRole::Scrutinee);
        return;
    }
    parser.bump();
    parser.bump_trivia();
    let guard = top_level_operator(parser, parser.cursor(), end, "when").unwrap_or(end);
    emit_expression(parser, guard, SyntaxRole::Scrutinee);
    bump_until(parser, guard);
    if parser.at("when") {
        parser.bump();
        parser.bump_trivia();
        emit_expression(parser, end, SyntaxRole::Guard);
    }
}

fn emit_for_head(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
    parser.bump();
    parser.bump_trivia();
    let separator = top_level_operator(parser, parser.cursor(), end, "in").unwrap_or(end);
    emit_pattern(parser, separator, SyntaxRole::Pattern);
    bump_until(parser, separator);
    if parser.cursor() < end {
        parser.bump();
        parser.bump_trivia();
        emit_expression(parser, end, SyntaxRole::Scrutinee);
    }
}

fn emit_match_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    thread_flow_context: bool,
) {
    parser.start(SyntaxKind::Block, SyntaxRole::Body);
    emit_open_delimiter(parser, SyntaxKind::OpenBraceNode, "{");
    let close = find_matching_close_before(parser, parser.cursor(), end, "{").unwrap_or(end);
    parser.start(SyntaxKind::MatchArmList, SyntaxRole::Element(0));
    let mut ordinal = 0_u32;
    while parser.cursor() < close {
        parser.bump_trivia();
        if parser.cursor() >= close {
            break;
        }
        let arm_end = find_match_arm_end(parser, parser.cursor(), close);
        emit_match_arm(parser, arm_end, item_kind, ordinal, thread_flow_context);
        bump_until(parser, arm_end);
        ordinal = ordinal
            .checked_add(1)
            .expect("the statement grammar budget keeps Match arm ordinals within u32");
        if matches!(parser.current_text(), Some("," | ";")) {
            parser.bump();
        }
    }
    parser.finish();
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBraceNode,
        "}",
        "syntax.statement.missing_match_close",
    );
    parser.finish();
}

fn emit_match_arm(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    ordinal: u32,
    thread_flow_context: bool,
) {
    parser.start(SyntaxKind::MatchArm, SyntaxRole::MatchArm(ordinal));
    let arrow = top_level_operator(parser, parser.cursor(), end, "=>").unwrap_or(end);
    let guard = top_level_operator(parser, parser.cursor(), arrow, "when").unwrap_or(arrow);
    emit_pattern(parser, guard, SyntaxRole::Pattern);
    bump_until(parser, guard);
    if parser.at("when") {
        parser.bump();
        parser.bump_trivia();
        emit_expression(parser, arrow, SyntaxRole::Guard);
        bump_until(parser, arrow);
    }
    if parser.at("=>") {
        parser.bump();
        parser.bump_trivia();
        if parser.at("{") {
            emit_nested_control_block(
                parser,
                end,
                item_kind,
                SyntaxRole::Body,
                "syntax.statement.missing_match_arm_close",
                thread_flow_context,
            );
        } else {
            emit_item_expression(parser, end, SyntaxRole::Body, item_kind);
        }
    } else {
        emit_item_expression(parser, end, SyntaxRole::Body, item_kind);
    }
    parser.finish();
}

fn emit_nested_control_block(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    item_kind: SyntaxKind,
    role: SyntaxRole,
    missing_close_code: &'static str,
    thread_flow_context: bool,
) {
    if thread_flow_context {
        emit_braced_thread_flow_block_until(
            parser,
            end,
            item_kind,
            SyntaxKind::Block,
            role,
            missing_close_code,
        );
    } else {
        emit_braced_statement_block_until(
            parser,
            end,
            item_kind,
            SyntaxKind::Block,
            role,
            missing_close_code,
        );
    }
}

fn emit_item_expression(
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    role: SyntaxRole,
    _item_kind: SyntaxKind,
) {
    emit_expression(parser, end, role);
}

fn find_match_arm_end(parser: &ShadowDocumentParser<'_, '_>, start: usize, end: usize) -> usize {
    let mut depth = 0_usize;
    for index in start..end {
        let Some(token) = parser.token_at(index) else {
            return end;
        };
        let text = parser.text_of(token);
        if depth == 0 && (matches!(text, "," | ";") || token.kind() == SyntaxKind::NewlineToken) {
            return index;
        }
        match text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    end
}

fn top_level_operator(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
    spelling: &str,
) -> Option<usize> {
    let mut depth = 0_usize;
    for index in start..end {
        let token = parser.token_at(index)?;
        let text = parser.text_of(token);
        if depth == 0 && text == spelling {
            return Some(index);
        }
        match text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn next_significant_text<'a>(
    parser: &'a ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<&'a str> {
    first_significant(parser, start, end).and_then(|index| token_text(parser, index))
}

fn find_statement_open_brace(
    parser: &ShadowDocumentParser<'_, '_>,
    start: usize,
    end: usize,
) -> Option<usize> {
    let mut paren = 0_usize;
    let mut bracket = 0_usize;
    for index in start..end {
        let text = token_text(parser, index)?;
        if text == "{" && paren == 0 && bracket == 0 {
            return Some(index);
        }
        match text {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "[" => bracket += 1,
            "]" => bracket = bracket.saturating_sub(1),
            _ => {}
        }
    }
    None
}

pub(super) fn is_statement_head(spelling: &str) -> bool {
    matches!(
        spelling,
        "assert"
            | "break"
            | "close"
            | "continue"
            | "defer"
            | "for"
            | "goto"
            | "if"
            | "let"
            | "loop"
            | "match"
            | "on"
            | "out"
            | "return"
            | "select"
            | "signal"
            | "thread"
            | "unsafe"
            | "wait"
            | "while"
            | "yield"
    )
}

fn expression_statement_start(parser: &ShadowDocumentParser<'_, '_>) -> bool {
    matches!(
        parser.current_kind(),
        Some(
            SyntaxKind::IdentifierToken
                | SyntaxKind::KeywordToken
                | SyntaxKind::LifetimeToken
                | SyntaxKind::NumberToken
                | SyntaxKind::StringToken
                | SyntaxKind::RawStringToken
                | SyntaxKind::CharacterToken
                | SyntaxKind::EntityReferenceToken
        )
    ) || matches!(
        parser.current_text(),
        Some("(" | "[" | "{" | "." | "_" | "&" | "*" | "!" | "-" | "+")
    )
}
