//! Line and item classification heuristics for CST projection.

use super::CstLine;
use super::punctuation::{
    find_matching_punctuation, find_top_level_punctuation, split_top_level_punctuation_once,
    split_top_level_punctuation_sequence_once,
};
use super::{
    CstFlowItemKind, CstLetFlowItemKind, CstLineKind, CstStmtKind, CstStructuredFlowBlockKind,
    CstTopLevelItemKind, CstTopLevelLineKind,
};

pub(crate) fn classify_stmt(trimmed: &str) -> CstStmtKind {
    if looks_like_lifetime_set(trimmed) {
        CstStmtKind::LifetimeSet
    } else if trimmed.starts_with("wait(") {
        CstStmtKind::Wait
    } else if trimmed.starts_with("let ") {
        CstStmtKind::Let
    } else if trimmed.starts_with("defer ") && trimmed.contains('{') {
        CstStmtKind::DeferBlock
    } else if trimmed.starts_with("defer ") {
        CstStmtKind::Defer
    } else if looks_like_control_transfer(trimmed) {
        CstStmtKind::ControlTransfer
    } else if trimmed.starts_with("on ") {
        CstStmtKind::On
    } else if trimmed.starts_with("unsafe lifetime ") && trimmed.contains('{') {
        CstStmtKind::UnsafeLifetime
    } else if looks_like_braced_stmt(trimmed) {
        CstStmtKind::Braced
    } else if matches!(trimmed.split_whitespace().next(), Some("match" | "if")) {
        CstStmtKind::AmbiguousBlockHead
    } else {
        CstStmtKind::Expr
    }
}
fn looks_like_lifetime_set(trimmed: &str) -> bool {
    let Some((target, _)) = split_top_level_punctuation_sequence_once(trimmed, &["<", "-"]) else {
        return false;
    };
    target.trim_start().starts_with('\'')
}

fn looks_like_control_transfer(trimmed: &str) -> bool {
    trimmed == "break"
        || trimmed == "continue"
        || trimmed.starts_with("continue ")
        || trimmed.starts_with("out ")
        || trimmed.starts_with("break ")
        || ["return ", "goto ", "yield ", "close ", "select "]
            .iter()
            .any(|prefix| trimmed.starts_with(prefix))
}

fn looks_like_braced_stmt(trimmed: &str) -> bool {
    find_top_level_punctuation(trimmed, '{').is_some()
}

pub(super) fn classify_line(text: &str) -> CstLineKind {
    let trimmed = text.trim_start();
    if trimmed.trim_end().is_empty() {
        CstLineKind::Blank
    } else if trimmed.starts_with("///") {
        CstLineKind::DocComment
    } else if trimmed.starts_with("//") {
        CstLineKind::Comment
    } else {
        CstLineKind::Code
    }
}

pub(super) fn classify_top_level_line(trimmed: &str) -> CstTopLevelLineKind {
    if trimmed.starts_with("#[") || trimmed.starts_with("#![") {
        CstTopLevelLineKind::Attribute
    } else if trimmed.starts_with("mod ") {
        CstTopLevelLineKind::Module
    } else if looks_like_use_line(trimmed) {
        CstTopLevelLineKind::Use
    } else {
        CstTopLevelLineKind::Item
    }
}

pub(super) fn classify_top_level_item(trimmed: &str) -> CstTopLevelItemKind {
    if looks_like_flow(trimmed) {
        CstTopLevelItemKind::Flow
    } else if looks_like_function_item(trimmed) {
        CstTopLevelItemKind::Function
    } else if looks_like_agent_item(trimmed) {
        CstTopLevelItemKind::Agent
    } else if looks_like_callable_item(trimmed) {
        CstTopLevelItemKind::Callable
    } else if looks_like_state_item(trimmed) {
        CstTopLevelItemKind::State
    } else if looks_like_trait_item(trimmed) {
        CstTopLevelItemKind::Trait
    } else if looks_like_impl_item(trimmed) {
        CstTopLevelItemKind::Impl
    } else if looks_like_enum_item(trimmed) {
        CstTopLevelItemKind::Enum
    } else if looks_like_struct_item(trimmed) {
        CstTopLevelItemKind::Struct
    } else if looks_like_type_alias(trimmed) {
        CstTopLevelItemKind::TypeAlias
    } else if looks_like_entity_decl_item(trimmed) {
        CstTopLevelItemKind::EntityDecl
    } else if looks_like_entry_item(trimmed) {
        CstTopLevelItemKind::Entry
    } else if looks_like_extern_capability_item(trimmed) {
        CstTopLevelItemKind::ExternCapability
    } else if looks_like_extern_mod_item(trimmed) {
        CstTopLevelItemKind::ExternMod
    } else if looks_like_hook(trimmed) {
        CstTopLevelItemKind::Hook
    } else if looks_like_dialogue_defaults(trimmed) {
        CstTopLevelItemKind::DialogueDefaults
    } else if looks_like_memo_fn(trimmed) {
        CstTopLevelItemKind::MemoFn
    } else if looks_like_proof_item(trimmed) {
        CstTopLevelItemKind::Proof
    } else if looks_like_trusted_axiom_item(trimmed) {
        CstTopLevelItemKind::TrustedAxiom
    } else if looks_like_test_item(trimmed) {
        CstTopLevelItemKind::Test
    } else if looks_like_bench_item(trimmed) {
        CstTopLevelItemKind::Bench
    } else if looks_like_parser_item(trimmed) {
        CstTopLevelItemKind::Parser
    } else if looks_like_source_item(trimmed) {
        CstTopLevelItemKind::Source
    } else if looks_like_ui_text_input(trimmed) {
        CstTopLevelItemKind::UiTextInput
    } else if looks_like_ui_style(trimmed) {
        CstTopLevelItemKind::UiStyle
    } else {
        CstTopLevelItemKind::FlowBodyItemOrRaw
    }
}

fn visible_tail(input: &str) -> &str {
    let trimmed = input.trim_start();
    if let Some(rest) = trimmed.strip_prefix("pub(crate)") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("pub(super)") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("pub ") {
        rest
    } else {
        input
    }
}

fn visible_head(input: &str) -> &str {
    visible_tail(input).trim_start()
}

fn looks_like_ui_text_input(trimmed: &str) -> bool {
    let tail = visible_tail(trimmed).trim_start();
    ["ui text_input", "ui text_area", "ui secure_field"]
        .iter()
        .any(|prefix| {
            tail.strip_prefix(prefix)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
        })
}

fn looks_like_ui_style(trimmed: &str) -> bool {
    let tail = visible_tail(trimmed).trim_start();
    tail.strip_prefix("ui style")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
}

fn looks_like_use_line(trimmed: &str) -> bool {
    let rest = visible_head(trimmed);
    let rest = rest.strip_prefix("surface ").unwrap_or(rest);
    rest.starts_with("use ") || rest.starts_with("lazy use ") || rest.starts_with("eager use ")
}

fn looks_like_flow(trimmed: &str) -> bool {
    let rest = visible_head(trimmed);
    rest.starts_with("flow ") || rest.starts_with("fragment ")
}

pub(super) fn looks_like_function_item(trimmed: &str) -> bool {
    let rest = visible_head(trimmed);
    rest.starts_with("fn ")
        || rest.starts_with("task fn ")
        || rest.starts_with("dialogue fn ")
        || rest.starts_with("stream fn ")
}

pub(super) fn looks_like_agent_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("agent ")
}

fn looks_like_callable_item(trimmed: &str) -> bool {
    let rest = visible_head(trimmed);
    rest.starts_with("reducer ") || rest.starts_with("view ")
}

fn looks_like_state_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("state ")
}

fn looks_like_trait_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("trait ")
}

fn looks_like_impl_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("impl")
}

fn looks_like_enum_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("enum ")
}

fn looks_like_struct_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("struct ")
}

fn looks_like_type_alias(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("type ")
}

fn looks_like_entity_decl_item(trimmed: &str) -> bool {
    let rest = visible_head(trimmed);
    let rest = rest.strip_prefix("surface ").unwrap_or(rest);
    [
        "audio bus",
        "mixer snapshot",
        "voice profile",
        "asset",
        "image",
        "character",
        "component",
        "activity",
        "content",
        "metric counter",
        "metric gauge",
        "metric",
        "signal",
        "layer",
        "textbox",
        "voice",
        "se",
        "bgm",
        "ducking",
        "motion",
        "rig",
    ]
    .into_iter()
    .any(|keyword| {
        rest.strip_prefix(keyword)
            .is_some_and(|tail| tail.starts_with(char::is_whitespace))
    })
}

fn looks_like_extern_mod_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("extern ")
}

fn looks_like_entry_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("entry ")
}

fn looks_like_extern_capability_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("extern capability ")
}

fn looks_like_hook(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("hook ")
}

fn looks_like_dialogue_defaults(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("dialogue defaults")
}

fn looks_like_memo_fn(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("memo fn ")
}

fn looks_like_proof_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("proof ")
}

fn looks_like_trusted_axiom_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("trusted axiom ")
}

fn looks_like_test_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("test ")
}

fn looks_like_bench_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("bench ")
}

fn looks_like_parser_item(trimmed: &str) -> bool {
    visible_head(trimmed).starts_with("parser ")
}

fn looks_like_source_item(trimmed: &str) -> bool {
    let rest = visible_head(trimmed);
    rest.starts_with("source ") && !rest.starts_with("source locale ")
}

pub(super) fn classify_flow_item(trimmed: &str) -> CstFlowItemKind {
    if let Some(kind) = classify_structured_flow_block(trimmed) {
        CstFlowItemKind::StructuredBlock(kind)
    } else if trimmed.starts_with("include ") {
        CstFlowItemKind::Include
    } else if is_await_with_head(trimmed) {
        CstFlowItemKind::AwaitWith
    } else if let Some(kind) = classify_let_flow_item(trimmed) {
        CstFlowItemKind::Let(kind)
    } else if is_typed_stmt(trimmed) || looks_like_assignment_stmt(trimmed) {
        CstFlowItemKind::TypedStmt
    } else {
        CstFlowItemKind::Other
    }
}

fn classify_structured_flow_block(trimmed: &str) -> Option<CstStructuredFlowBlockKind> {
    if trimmed.starts_with("choice ") {
        Some(CstStructuredFlowBlockKind::Choice)
    } else if trimmed.starts_with("if let ") {
        Some(CstStructuredFlowBlockKind::IfLet)
    } else if trimmed.starts_with("if ") {
        Some(CstStructuredFlowBlockKind::If)
    } else if trimmed.starts_with("match ") {
        Some(CstStructuredFlowBlockKind::Match)
    } else if is_loop_head(trimmed) {
        Some(CstStructuredFlowBlockKind::Loop)
    } else if trimmed.starts_with("while let ") {
        Some(CstStructuredFlowBlockKind::WhileLet)
    } else if trimmed.starts_with("while ") {
        Some(CstStructuredFlowBlockKind::While)
    } else if trimmed.starts_with("for ") {
        Some(CstStructuredFlowBlockKind::For)
    } else if trimmed.starts_with("select") {
        Some(CstStructuredFlowBlockKind::Select)
    } else if trimmed.starts_with("thread ") || matches!(trimmed, "thread" | "thread:") {
        Some(CstStructuredFlowBlockKind::Thread)
    } else if trimmed.starts_with("defer ") || matches!(trimmed, "defer" | "defer:") {
        Some(CstStructuredFlowBlockKind::Defer)
    } else if trimmed.starts_with("borrow ") {
        Some(CstStructuredFlowBlockKind::Borrow)
    } else if trimmed.starts_with("unsafe lifetime ") {
        Some(CstStructuredFlowBlockKind::UnsafeLifetime)
    } else if trimmed.starts_with("source locale ") {
        Some(CstStructuredFlowBlockKind::SourceLocale)
    } else if trimmed.starts_with('{') {
        Some(CstStructuredFlowBlockKind::BareScope)
    } else if trimmed.starts_with("scope ") || matches!(trimmed, "scope" | "scope:") {
        Some(CstStructuredFlowBlockKind::Scope)
    } else {
        None
    }
}

fn classify_let_flow_item(trimmed: &str) -> Option<CstLetFlowItemKind> {
    let value = let_binding_value(trimmed)?;
    Some(if value.trim_start().starts_with("choice ") {
        CstLetFlowItemKind::Choice
    } else if is_dialogue_call_value(value) {
        CstLetFlowItemKind::DialogueCall
    } else if parse_scope_head(value.trim_start()) {
        CstLetFlowItemKind::Scope
    } else if matches!(value.trim(), "result {" | "task {" | "seq {" | "stream {") {
        CstLetFlowItemKind::ComputationBlock
    } else if value.trim_start().starts_with("memo(") {
        CstLetFlowItemKind::MemoBlock
    } else if value.trim().starts_with('{') {
        CstLetFlowItemKind::Block
    } else if is_loop_head(value.trim_start()) {
        CstLetFlowItemKind::Loop
    } else if is_await_with_head(value.trim())
        || value.trim().starts_with("(await ") && value.contains(" with")
    {
        CstLetFlowItemKind::AwaitWith
    } else if is_await_start_head(value.trim()) {
        CstLetFlowItemKind::AwaitStart
    } else if value.trim_start().starts_with("if let ") {
        CstLetFlowItemKind::IfLet
    } else if value.trim_start().starts_with("if ") {
        CstLetFlowItemKind::If
    } else if value.trim_start().starts_with("match ") {
        CstLetFlowItemKind::Match
    } else if trimmed.starts_with("let ") && trimmed.contains(" else") && trimmed.contains('{') {
        CstLetFlowItemKind::LetElse
    } else {
        CstLetFlowItemKind::Plain
    })
}

fn let_binding_value(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix("let ")?;
    split_top_level_punctuation_once(rest, '=').map(|(_, value)| value)
}

fn is_dialogue_call_value(value: &str) -> bool {
    let value = value.trim_start();
    let Some(open) = find_content_bracket(value) else {
        return false;
    };
    if value.starts_with('[') {
        return false;
    }
    let target = value[..open].trim();
    let Some(close) = find_matching_punctuation(value, open, '[', ']') else {
        // Multiline dialogue result bindings start as `let x = speaker.say()[`
        // on the first CST line. Classify those as dialogue so the AST parser
        // can collect the remaining content lines and the following `with:`.
        return target.contains('(');
    };
    let content = value[open + '['.len_utf8()..close].trim();
    target.contains('(') || crate::expr::parse_expr(content).is_err()
}

fn parse_scope_head(source: &str) -> bool {
    let Some(rest) = source.strip_prefix("scope") else {
        return false;
    };
    if rest
        .chars()
        .next()
        .is_some_and(|ch| !(ch.is_whitespace() || ch == '{'))
    {
        return false;
    }
    true
}

fn is_loop_head(head: &str) -> bool {
    head == "loop"
        || head.starts_with("loop ")
        || labeled_head_tail(head).is_some_and(|tail| tail == "loop" || tail.starts_with("loop "))
}

fn labeled_head_tail(head: &str) -> Option<&str> {
    let rest = head.trim_start().strip_prefix('\'')?;
    let (_, tail) = split_top_level_punctuation_once(rest, ':')?;
    Some(tail.trim_start())
}

fn is_await_with_head(trimmed: &str) -> bool {
    (trimmed.starts_with("await ")
        || trimmed.starts_with("try await ")
        || trimmed.starts_with("await? "))
        && (trimmed.contains(" with ") || trimmed.ends_with("with:"))
}

fn is_await_start_head(trimmed: &str) -> bool {
    trimmed.starts_with("await ")
        || trimmed.starts_with("try await ")
        || trimmed.starts_with("await? ")
        || trimmed.starts_with("(await ")
}

fn find_content_bracket(text: &str) -> Option<usize> {
    let open = find_top_level_punctuation(text, '[')?;
    (!text[..open].trim_end().ends_with('#')).then_some(open)
}

fn is_typed_stmt(trimmed: &str) -> bool {
    if trimmed.starts_with('\'') && (trimmed.contains("<-") || trimmed.contains("|>")) {
        return true;
    }
    matches!(
        trimmed.split_whitespace().next(),
        Some(
            "let"
                | "match"
                | "if"
                | "for"
                | "return"
                | "out"
                | "goto"
                | "thread"
                | "scope"
                | "defer"
                | "yield"
                | "unsafe"
                | "signal"
                | "close"
                | "break"
                | "continue"
        )
    )
}

fn looks_like_assignment_stmt(trimmed: &str) -> bool {
    let Some((target, expr)) = split_top_level_punctuation_once(trimmed, '=') else {
        return false;
    };
    !target.trim().is_empty()
        && !expr.trim().is_empty()
        && !target.trim_end().ends_with(['!', '<', '>', '='])
        && !expr.trim_start().starts_with('=')
}

pub(super) fn flow_line_starts_body(line: &CstLine, is_first_line: bool) -> bool {
    let trimmed = line.trimmed();
    trimmed == "{"
        || (is_first_line && line.has_top_level_brace_open() && !trimmed.starts_with("effects"))
}

pub(super) fn function_body_line_starts_body(line: &CstLine) -> bool {
    let trimmed = line.trimmed();
    trimmed == "{"
        || (line.has_unclosed_top_level_brace_open() && !trimmed.starts_with("effects"))
        || ((looks_like_function_item(trimmed) || looks_like_agent_item(trimmed))
            && line.has_top_level_brace_open())
}
