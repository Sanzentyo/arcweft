//! Sans I/O source-edit helpers for Arcweft tooling.
//!
//! This crate produces deterministic text edits and lightweight tooling data.
//! It does not read files, write files, watch paths, or run an LSP transport.

use arcweft_lang_hir::id_context::{
    IdContextEntry, IdContextMaterialization, IdContextOption, collect_id_context,
};
use arcweft_lang_syntax::{
    ast::{
        choice::{ChoiceAction, ChoiceItem},
        flow::{AwaitBranch, FlowItem, Stmt},
        items::{EntityDeclKind, Item},
        line_plan::LinePlanItem,
        pattern::Pattern,
    },
    cst::{CstLineEvents, CstLineKind, cst_lines},
    expr::Expr,
    parser::parse_source,
    source::ParsedSource,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// Formatting and source normalization options.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FormatOptions {
    /// Rewrite script-friendly sugar into canonical block/call forms.
    pub expand_sugar: bool,
}

/// A half-open source edit over UTF-8 byte offsets.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextEdit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

/// One diagnostic produced while computing tooling edits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolingDiagnostic {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

/// Inlay hint data independent from any concrete LSP transport.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InlayHint {
    pub position: usize,
    pub label: String,
}

/// Tooling code action data independent from any concrete LSP transport.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolingCodeAction {
    pub id: String,
    pub label: String,
    pub edit: Option<TextEdit>,
}

/// A complete source-edit report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolingEditReport {
    pub status: String,
    pub changed: bool,
    pub edits: Vec<TextEdit>,
    pub output: String,
    pub diagnostics: Vec<ToolingDiagnostic>,
}

/// Error returned when edit application would corrupt source coordinates.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ToolingError {
    #[error("text edit range {start}..{end} is outside source length {len}")]
    RangeOutOfBounds {
        start: usize,
        end: usize,
        len: usize,
    },
    #[error("text edit range {start}..{end} overlaps a later edit")]
    OverlappingEdit { start: usize, end: usize },
}

/// Formats source while preserving authoring sugar by default.
pub fn format_source(
    source: &str,
    options: FormatOptions,
) -> Result<ToolingEditReport, ToolingError> {
    let edits = if options.expand_sugar {
        sugar_expansion_edits(source)
    } else {
        Vec::new()
    };
    report_from_edits(source, edits)
}

/// Rewrites ID-context relative IDs to normalized explicit IDs.
pub fn materialize_ids(source: &str) -> Result<ToolingEditReport, ToolingError> {
    let edits = id_context_edits(source);
    report_from_edits(source, edits)
}

/// Computes inferred-ID inlay hints for relative ID positions.
pub fn inferred_id_hints(source: &str) -> Vec<InlayHint> {
    id_context_hints(source)
}

fn id_context_edits(source: &str) -> Vec<TextEdit> {
    collect_id_context(source)
        .entries()
        .iter()
        .map(id_context_edit)
        .collect()
}

fn id_context_edit(entry: &IdContextEntry) -> TextEdit {
    match entry.materialization() {
        IdContextMaterialization::Replace { range, normalized } => TextEdit {
            start: range.start(),
            end: range.end(),
            replacement: format!("@{normalized}"),
        },
        IdContextMaterialization::InsertDialogueOptions {
            insert,
            call_has_options,
            options_has_any,
            options,
        } => {
            let joined = options
                .iter()
                .map(IdContextOption::as_assignment)
                .collect::<Vec<_>>()
                .join(", ");
            let replacement = if *call_has_options {
                if *options_has_any {
                    format!(", {joined}")
                } else {
                    joined
                }
            } else {
                format!("({joined})")
            };
            TextEdit {
                start: insert.start(),
                end: insert.end(),
                replacement,
            }
        }
    }
}

fn id_context_hints(source: &str) -> Vec<InlayHint> {
    collect_id_context(source)
        .entries()
        .iter()
        .map(|entry| match entry.materialization() {
            IdContextMaterialization::Replace { range, normalized } => InlayHint {
                position: range.end(),
                label: format!("@{normalized}"),
            },
            IdContextMaterialization::InsertDialogueOptions {
                insert, options, ..
            } => InlayHint {
                position: insert.start(),
                label: options
                    .iter()
                    .map(IdContextOption::as_assignment)
                    .collect::<Vec<_>>()
                    .join(", "),
            },
        })
        .collect()
}

/// Returns source-level code actions that are safe to expose through LSP.
pub fn source_code_actions(source: &str) -> Vec<ToolingCodeAction> {
    let mut actions = Vec::new();
    for edit in sugar_expansion_edits(source) {
        actions.push(ToolingCodeAction {
            id: "arcweft.expandSugar".to_owned(),
            label: "Expand Arcweft sugar".to_owned(),
            edit: Some(edit),
        });
    }
    if let Ok(report) = materialize_ids(source) {
        actions.extend(report.edits.into_iter().map(|edit| ToolingCodeAction {
            id: "arcweft.materializeId".to_owned(),
            label: "Materialize inferred Arcweft ID".to_owned(),
            edit: Some(edit),
        }));
    }
    actions
}

/// Applies edits to source. Edits may be unsorted, but must not overlap.
pub fn apply_text_edits(source: &str, edits: &[TextEdit]) -> Result<String, ToolingError> {
    let mut sorted = edits.to_vec();
    sorted.sort_by_key(|edit| (edit.start, edit.end));
    let mut previous_end = 0;
    for edit in &sorted {
        if edit.start > edit.end || edit.end > source.len() {
            return Err(ToolingError::RangeOutOfBounds {
                start: edit.start,
                end: edit.end,
                len: source.len(),
            });
        }
        if edit.start < previous_end {
            return Err(ToolingError::OverlappingEdit {
                start: edit.start,
                end: edit.end,
            });
        }
        previous_end = edit.end;
    }
    let mut output = source.to_owned();
    for edit in sorted.iter().rev() {
        output.replace_range(edit.start..edit.end, &edit.replacement);
    }
    Ok(output)
}

fn report_from_edits(
    source: &str,
    mut edits: Vec<TextEdit>,
) -> Result<ToolingEditReport, ToolingError> {
    dedupe_edits(&mut edits);
    let output = apply_text_edits(source, &edits)?;
    Ok(ToolingEditReport {
        status: "ok".to_owned(),
        changed: output != source,
        edits,
        output,
        diagnostics: Vec::new(),
    })
}

fn sugar_expansion_edits(source: &str) -> Vec<TextEdit> {
    let parsed = parse_source(source);
    let lines = cst_lines(parsed.syntax());
    let character_aliases = collect_character_aliases(&parsed);
    let speaker_presets =
        collect_speaker_preset_locals_from_typed_tree(&parsed, &character_aliases);
    let mut edits = Vec::new();

    for line in lines.iter() {
        if line.kind() == CstLineKind::Comment {
            continue;
        }
        edits.extend(parent_path_edits(line.text(), line.start()));
        if line.trimmed() == "with:" {
            edits.push(TextEdit {
                start: line.end() - 1,
                end: line.end(),
                replacement: " {".to_owned(),
            });
            if let Some(close) = closing_brace_insert(&lines, line.start()) {
                edits.push(close);
            }
            continue;
        }
        if let Some(edit) = speaker_line_edit(
            line.text(),
            line.start(),
            &speaker_presets,
            &character_aliases,
        ) {
            edits.push(edit);
        }
        if let Some(edit) = await_question_edit(line.text(), line.start()) {
            edits.push(edit);
        }
    }
    edits
}

fn collect_character_aliases(parsed: &ParsedSource) -> BTreeSet<String> {
    parsed
        .typed_tree()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::EntityDecl(entity) if entity.kind() == EntityDeclKind::Character => {
                entity.surface_alias().map(str::to_owned)
            }
            _ => None,
        })
        .collect()
}

fn collect_speaker_preset_locals_from_typed_tree(
    parsed: &ParsedSource,
    character_aliases: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut presets = BTreeSet::new();
    for item in parsed.typed_tree().items() {
        collect_speaker_presets_from_item(item, character_aliases, &mut presets);
    }
    presets
}

fn collect_speaker_presets_from_item(
    item: &Item,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match item {
        Item::Flow(flow) => {
            collect_speaker_presets_from_flow_items(flow.body(), character_aliases, presets);
        }
        Item::Function(function) => {
            collect_speaker_presets_from_stmts(
                function.body_statements(),
                character_aliases,
                presets,
            );
            if let Some(value) = function.body_value() {
                collect_speaker_presets_from_expr(value, character_aliases, presets);
            }
        }
        Item::MemoFn(memo) => {
            collect_speaker_presets_from_stmts(memo.body_statements(), character_aliases, presets);
            if let Some(value) = memo.body_value() {
                collect_speaker_presets_from_expr(value, character_aliases, presets);
            }
        }
        Item::Parser(parser) => {
            collect_speaker_presets_from_stmts(
                parser.body_statements(),
                character_aliases,
                presets,
            );
            if let Some(value) = parser.body_value() {
                collect_speaker_presets_from_expr(value, character_aliases, presets);
            }
        }
        Item::Source(source) => {
            for handler in source.handlers() {
                collect_speaker_presets_from_stmts(handler.body(), character_aliases, presets);
            }
        }
        Item::FlowItem(item) => {
            collect_speaker_presets_from_flow_item(item, character_aliases, presets);
        }
        _ => {}
    }
}

fn collect_speaker_presets_from_flow_items(
    items: &[FlowItem],
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    for item in items {
        collect_speaker_presets_from_flow_item(item, character_aliases, presets);
    }
}

fn collect_speaker_presets_from_flow_item(
    item: &FlowItem,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match item {
        FlowItem::Stmt(stmt) => collect_speaker_presets_from_stmt(stmt, character_aliases, presets),
        FlowItem::If(block) => {
            collect_speaker_presets_from_expr(block.condition(), character_aliases, presets);
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::IfLet(block) => {
            collect_speaker_presets_from_expr(block.expr(), character_aliases, presets);
            if let Some(guard) = block.guard() {
                collect_speaker_presets_from_expr(guard, character_aliases, presets);
            }
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::Match(block) => {
            collect_speaker_presets_from_expr(block.expr(), character_aliases, presets);
            for arm in block.arms() {
                if let Some(guard) = arm.guard() {
                    collect_speaker_presets_from_expr(guard, character_aliases, presets);
                }
                collect_speaker_presets_from_flow_items(arm.body(), character_aliases, presets);
            }
        }
        FlowItem::Loop(block) => {
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::While(block) => {
            collect_speaker_presets_from_expr(block.condition(), character_aliases, presets);
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::WhileLet(block) => {
            collect_speaker_presets_from_expr(block.expr(), character_aliases, presets);
            if let Some(guard) = block.guard() {
                collect_speaker_presets_from_expr(guard, character_aliases, presets);
            }
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::For(block) => {
            collect_speaker_presets_from_expr(block.source(), character_aliases, presets);
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::Select(block) => {
            for branch in block.branches() {
                collect_speaker_presets_from_flow_items(branch.body(), character_aliases, presets);
            }
        }
        FlowItem::BorrowBlock(block) => {
            collect_speaker_presets_from_expr(block.source(), character_aliases, presets);
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::SourceLocale(block) => {
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::Scope(block) => {
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        FlowItem::AwaitWith(await_with) => {
            collect_speaker_presets_from_expr(await_with.expr(), character_aliases, presets);
            for branch in await_with.branches() {
                collect_speaker_presets_from_await_branch(branch, character_aliases, presets);
            }
        }
        FlowItem::Choice(choice) => {
            collect_speaker_presets_from_choice_items(choice.items(), character_aliases, presets);
            if let Some(plan) = choice.plan() {
                for item in plan.items() {
                    collect_speaker_presets_from_choice_plan_item(item, character_aliases, presets);
                }
            }
        }
        FlowItem::ScenarioCommand(command) => {
            for arg in command.args() {
                collect_speaker_presets_from_expr(arg, character_aliases, presets);
            }
        }
        FlowItem::SpeakerLine(line) => {
            if let Some(plan) = line.plan() {
                for item in plan.items() {
                    collect_speaker_presets_from_line_plan_item(item, character_aliases, presets);
                }
            }
        }
        FlowItem::ContentCall(call) => {
            if let Some(plan) = call.plan() {
                for item in plan.items() {
                    collect_speaker_presets_from_line_plan_item(item, character_aliases, presets);
                }
            }
        }
        FlowItem::Include(_) | FlowItem::Raw(_) => {}
    }
}

fn collect_speaker_presets_from_await_branch(
    branch: &AwaitBranch,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    collect_speaker_presets_from_flow_items(branch.body(), character_aliases, presets);
}

fn collect_speaker_presets_from_choice_items(
    items: &[ChoiceItem],
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    for item in items {
        match item {
            ChoiceItem::Let { pattern, expr } => {
                collect_speaker_preset_binding(pattern, expr, character_aliases, presets);
            }
            ChoiceItem::If { condition, items } => {
                collect_speaker_presets_from_expr(condition, character_aliases, presets);
                collect_speaker_presets_from_choice_items(items, character_aliases, presets);
            }
            ChoiceItem::For { source, items, .. } => {
                collect_speaker_presets_from_expr(source, character_aliases, presets);
                collect_speaker_presets_from_choice_items(items, character_aliases, presets);
            }
            ChoiceItem::Match { expr, arms } => {
                collect_speaker_presets_from_expr(expr, character_aliases, presets);
                for arm in arms {
                    if let Some(guard) = arm.guard() {
                        collect_speaker_presets_from_expr(guard, character_aliases, presets);
                    }
                    collect_speaker_presets_from_choice_items(
                        arm.items(),
                        character_aliases,
                        presets,
                    );
                }
            }
            ChoiceItem::Option(option) => {
                if let Some(expr) = option.id_expr() {
                    collect_speaker_presets_from_expr(expr, character_aliases, presets);
                }
                if let Some(value) = option.value() {
                    collect_speaker_presets_from_expr(value, character_aliases, presets);
                }
                if let Some(condition) = option.condition() {
                    collect_speaker_presets_from_expr(condition, character_aliases, presets);
                }
                if let Some(visible) = option.visible() {
                    collect_speaker_presets_from_expr(visible, character_aliases, presets);
                }
                if let Some(order) = option.order() {
                    collect_speaker_presets_from_expr(order, character_aliases, presets);
                }
                if let Some(hotkey) = option.hotkey() {
                    collect_speaker_presets_from_expr(hotkey, character_aliases, presets);
                }
                for field in option.ui_fields() {
                    collect_speaker_presets_from_expr(field.value(), character_aliases, presets);
                }
                match option.action() {
                    ChoiceAction::Out(expr) => {
                        collect_speaker_presets_from_expr(expr, character_aliases, presets);
                    }
                    ChoiceAction::SelectBlock(stmts) => {
                        collect_speaker_presets_from_stmts(stmts, character_aliases, presets);
                    }
                    ChoiceAction::Goto(_) | ChoiceAction::None => {}
                }
            }
            ChoiceItem::Raw(_) => {}
        }
    }
}

fn collect_speaker_presets_from_choice_plan_item(
    item: &arcweft_lang_syntax::ast::choice::ChoicePlanItem,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match item {
        arcweft_lang_syntax::ast::choice::ChoicePlanItem::Option { value, .. } => {
            collect_speaker_presets_from_expr(value, character_aliases, presets);
        }
        arcweft_lang_syntax::ast::choice::ChoicePlanItem::Timeout { duration, body } => {
            collect_speaker_presets_from_expr(duration, character_aliases, presets);
            collect_speaker_presets_from_stmts(body, character_aliases, presets);
        }
        arcweft_lang_syntax::ast::choice::ChoicePlanItem::Cancel { body, .. }
        | arcweft_lang_syntax::ast::choice::ChoicePlanItem::OnSelect { body, .. } => {
            collect_speaker_presets_from_stmts(body, character_aliases, presets);
        }
        arcweft_lang_syntax::ast::choice::ChoicePlanItem::Raw(_) => {}
    }
}

fn collect_speaker_presets_from_line_plan_item(
    item: &LinePlanItem,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match item {
        LinePlanItem::Init(stmts) | LinePlanItem::On { body: stmts, .. } => {
            collect_speaker_presets_from_stmts(stmts, character_aliases, presets);
        }
        LinePlanItem::CancelRule(rule) => {
            collect_speaker_presets_from_stmts(rule.action(), character_aliases, presets);
        }
        LinePlanItem::Thread(block) => {
            collect_speaker_presets_from_stmts(block.body(), character_aliases, presets);
        }
        LinePlanItem::Option { value, .. }
        | LinePlanItem::Let { expr: value, .. }
        | LinePlanItem::Out(value)
        | LinePlanItem::TimedCue { anchor: value, .. }
        | LinePlanItem::Assert { expr: value, .. }
        | LinePlanItem::Expr(value) => {
            collect_speaker_presets_from_expr(value, character_aliases, presets);
        }
        LinePlanItem::Stmt(stmt) => {
            collect_speaker_presets_from_stmt(stmt, character_aliases, presets);
        }
        LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
            for item in items {
                collect_speaker_presets_from_line_plan_item(item, character_aliases, presets);
            }
        }
        LinePlanItem::Memo { options, .. } => {
            for (_, value) in options {
                collect_speaker_presets_from_expr(value, character_aliases, presets);
            }
        }
        LinePlanItem::Raw(_) => {}
    }
}

fn collect_speaker_presets_from_stmts(
    stmts: &[Stmt],
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    for stmt in stmts {
        collect_speaker_presets_from_stmt(stmt, character_aliases, presets);
    }
}

fn collect_speaker_presets_from_stmt(
    stmt: &Stmt,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match stmt {
        Stmt::Let { pattern, expr, .. } => {
            collect_speaker_preset_binding(pattern, expr, character_aliases, presets);
        }
        Stmt::LetElse {
            pattern,
            expr,
            else_body,
            ..
        } => {
            collect_speaker_preset_binding(pattern, expr, character_aliases, presets);
            collect_speaker_presets_from_stmts(else_body, character_aliases, presets);
        }
        Stmt::LetChoice { pattern: _, choice } => {
            collect_speaker_presets_from_choice_items(choice.items(), character_aliases, presets);
        }
        Stmt::LetScope { scope, .. } => {
            collect_speaker_presets_from_stmts(scope.statements(), character_aliases, presets);
            if let Some(value) = scope.value() {
                collect_speaker_presets_from_expr(value, character_aliases, presets);
            }
        }
        Stmt::LetLoop { block, .. } => {
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        Stmt::LetAwait { await_with, .. } => {
            collect_speaker_presets_from_expr(await_with.expr(), character_aliases, presets);
            for branch in await_with.branches() {
                collect_speaker_presets_from_await_branch(branch, character_aliases, presets);
            }
        }
        Stmt::Return(expr)
        | Stmt::Out { expr, .. }
        | Stmt::Goto(expr)
        | Stmt::Defer { expr, .. }
        | Stmt::Yield(expr)
        | Stmt::Panic(expr)
        | Stmt::Fail(expr)
        | Stmt::Bail(expr)
        | Stmt::Close(expr)
        | Stmt::Select(expr)
        | Stmt::Expr(expr) => {
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
        }
        Stmt::Thread(block) => {
            collect_speaker_presets_from_stmts(block.body(), character_aliases, presets);
        }
        Stmt::DeferBlock { statements, .. } => {
            collect_speaker_presets_from_stmts(statements, character_aliases, presets);
        }
        Stmt::Ensure { condition, message } => {
            collect_speaker_presets_from_expr(condition, character_aliases, presets);
            collect_speaker_presets_from_expr(message, character_aliases, presets);
        }
        Stmt::Signal { target, value }
        | Stmt::LifetimeSet {
            target,
            expr: value,
        } => {
            collect_speaker_presets_from_expr(target, character_aliases, presets);
            collect_speaker_presets_from_expr(value, character_aliases, presets);
        }
        Stmt::On { body, .. } | Stmt::UnsafeLifetime { body, .. } | Stmt::Loop { body } => {
            collect_speaker_presets_from_stmts(body, character_aliases, presets);
        }
        Stmt::Command(command) => {
            for arg in command.args() {
                collect_speaker_presets_from_expr(arg, character_aliases, presets);
            }
        }
        Stmt::If { .. }
        | Stmt::While { .. }
        | Stmt::WhileLet { .. }
        | Stmt::For { .. }
        | Stmt::Match { .. }
        | Stmt::Break { .. } => {
            collect_speaker_presets_from_control_stmt(stmt, character_aliases, presets);
        }
        Stmt::Wait(_) | Stmt::Continue { .. } | Stmt::Raw(_) => {}
    }
}

fn collect_speaker_presets_from_control_stmt(
    stmt: &Stmt,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match stmt {
        Stmt::If { condition, body } | Stmt::While { condition, body } => {
            collect_speaker_presets_from_expr(condition, character_aliases, presets);
            collect_speaker_presets_from_stmts(body, character_aliases, presets);
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
            if let Some(guard) = guard {
                collect_speaker_presets_from_expr(guard, character_aliases, presets);
            }
            collect_speaker_presets_from_stmts(body, character_aliases, presets);
        }
        Stmt::For { source, body, .. } => {
            collect_speaker_presets_from_expr(source, character_aliases, presets);
            collect_speaker_presets_from_stmts(body, character_aliases, presets);
        }
        Stmt::Match { expr, arms } => {
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
            for arm in arms {
                if let Some(guard) = arm.guard() {
                    collect_speaker_presets_from_expr(guard, character_aliases, presets);
                }
                collect_speaker_presets_from_stmts(arm.body(), character_aliases, presets);
            }
        }
        Stmt::Break {
            expr: Some(expr), ..
        } => {
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
        }
        _ => {}
    }
}

fn collect_speaker_preset_binding(
    pattern: &Pattern,
    expr: &Expr,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    if let Some(name) = pattern_binding_name(pattern)
        && is_speaker_preset_expr(expr, character_aliases, presets)
    {
        presets.insert(name.to_owned());
    }
    collect_speaker_presets_from_expr(expr, character_aliases, presets);
}

fn pattern_binding_name(pattern: &Pattern) -> Option<&str> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            Some(name.as_str())
        }
        _ => None,
    }
    .filter(|name| is_identifier(name))
}

fn collect_speaker_presets_from_expr(
    expr: &Expr,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match expr {
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            for item in items {
                collect_speaker_presets_from_expr(item, character_aliases, presets);
            }
        }
        Expr::ArrayRepeat { value, len } => {
            collect_speaker_presets_from_expr(value, character_aliases, presets);
            collect_speaker_presets_from_expr(len, character_aliases, presets);
        }
        Expr::Call { callee, args } => {
            collect_speaker_presets_from_expr(callee, character_aliases, presets);
            for arg in args {
                collect_speaker_presets_from_expr(arg, character_aliases, presets);
            }
        }
        Expr::NamedArg { value, .. } => {
            collect_speaker_presets_from_expr(value, character_aliases, presets);
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_speaker_presets_from_expr(receiver, character_aliases, presets);
            for arg in args {
                collect_speaker_presets_from_expr(arg, character_aliases, presets);
            }
        }
        Expr::Field { target, .. } | Expr::Try { expr: target } => {
            collect_speaker_presets_from_expr(target, character_aliases, presets);
        }
        Expr::DialogueCall { callee, plan, .. } => {
            collect_speaker_presets_from_expr(callee, character_aliases, presets);
            if let Some(plan) = plan {
                for item in plan.items() {
                    collect_speaker_presets_from_line_plan_item(item, character_aliases, presets);
                }
            }
        }
        Expr::Index { target, index } => {
            collect_speaker_presets_from_expr(target, character_aliases, presets);
            collect_speaker_presets_from_expr(index, character_aliases, presets);
        }
        Expr::Pipe { lhs, rhs } | Expr::Binary { lhs, rhs, .. } => {
            collect_speaker_presets_from_expr(lhs, character_aliases, presets);
            collect_speaker_presets_from_expr(rhs, character_aliases, presets);
        }
        Expr::Await { expr, .. } | Expr::Unary { expr, .. } => {
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
        }
        Expr::Range { start, end, .. } => {
            if let Some(start) = start {
                collect_speaker_presets_from_expr(start, character_aliases, presets);
            }
            if let Some(end) = end {
                collect_speaker_presets_from_expr(end, character_aliases, presets);
            }
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            for (_, value) in fields {
                collect_speaker_presets_from_expr(value, character_aliases, presets);
            }
        }
        Expr::Closure { body, .. } => {
            collect_speaker_presets_from_expr(body, character_aliases, presets);
        }
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::MemoBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => {
            collect_speaker_presets_from_expr_block(
                statements,
                value.as_deref(),
                character_aliases,
                presets,
            );
        }
        Expr::If { .. } | Expr::IfLet { .. } | Expr::Match { .. } => {
            collect_speaker_presets_from_control_expr(expr, character_aliases, presets);
        }
        Expr::Thread { block } => {
            collect_speaker_presets_from_stmts(block.body(), character_aliases, presets);
        }
        Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::Placeholder(_)
        | Expr::Raw(_) => {}
    }
}

fn collect_speaker_presets_from_expr_block(
    statements: &[Stmt],
    value: Option<&Expr>,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    collect_speaker_presets_from_stmts(statements, character_aliases, presets);
    if let Some(value) = value {
        collect_speaker_presets_from_expr(value, character_aliases, presets);
    }
}

fn collect_speaker_presets_from_control_expr(
    expr: &Expr,
    character_aliases: &BTreeSet<String>,
    presets: &mut BTreeSet<String>,
) {
    match expr {
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_speaker_presets_from_expr(condition, character_aliases, presets);
            collect_speaker_presets_from_expr(then_branch, character_aliases, presets);
            if let Some(else_branch) = else_branch {
                collect_speaker_presets_from_expr(else_branch, character_aliases, presets);
            }
        }
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => {
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
            if let Some(guard) = guard {
                collect_speaker_presets_from_expr(guard, character_aliases, presets);
            }
            collect_speaker_presets_from_expr(then_branch, character_aliases, presets);
            if let Some(else_branch) = else_branch {
                collect_speaker_presets_from_expr(else_branch, character_aliases, presets);
            }
        }
        Expr::Match { scrutinee, arms } => {
            collect_speaker_presets_from_expr(scrutinee, character_aliases, presets);
            for arm in arms {
                if let Some(guard) = arm.guard() {
                    collect_speaker_presets_from_expr(guard, character_aliases, presets);
                }
                collect_speaker_presets_from_expr(arm.value(), character_aliases, presets);
            }
        }
        _ => {}
    }
}

fn is_speaker_preset_expr(
    expr: &Expr,
    character_aliases: &BTreeSet<String>,
    presets: &BTreeSet<String>,
) -> bool {
    match expr {
        Expr::Call { callee, .. } => speaker_preset_callee(callee, character_aliases, presets),
        Expr::MethodCall { receiver, .. } => {
            is_speaker_preset_expr(receiver, character_aliases, presets)
        }
        Expr::Block { value, .. }
        | Expr::ComputationBlock { value, .. }
        | Expr::MemoBlock { value, .. }
        | Expr::NamedBlock { value, .. } => value
            .as_deref()
            .is_some_and(|value| is_speaker_preset_expr(value, character_aliases, presets)),
        _ => false,
    }
}

fn speaker_preset_callee(
    callee: &Expr,
    character_aliases: &BTreeSet<String>,
    presets: &BTreeSet<String>,
) -> bool {
    match callee {
        Expr::Path(path) if character_aliases.contains(path) || presets.contains(path) => true,
        Expr::Field { target, field } if field == "new" => {
            matches!(target.as_ref(), Expr::Path(path) if path == "SpeakerPreset")
        }
        _ => false,
    }
}

fn parent_path_edits(line: &str, base: usize) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    let mut search = 0;
    while let Some(offset) = line[search..].find("parent::") {
        let start = search + offset;
        edits.push(TextEdit {
            start: base + start,
            end: base + start + "parent".len(),
            replacement: "super".to_owned(),
        });
        search = start + "parent::".len();
    }
    edits
}

fn await_question_edit(line: &str, base: usize) -> Option<TextEdit> {
    let leading = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("await? ")?;
    Some(TextEdit {
        start: base + leading,
        end: base + line.len(),
        replacement: format!("try await {rest}"),
    })
}

fn speaker_line_edit(
    line: &str,
    base: usize,
    speaker_presets: &BTreeSet<String>,
    character_aliases: &BTreeSet<String>,
) -> Option<TextEdit> {
    let leading = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    if trimmed.starts_with("//")
        || trimmed.starts_with("///")
        || trimmed.starts_with("with:")
        || trimmed.starts_with("case ")
    {
        return None;
    }
    let (head, text) = trimmed.split_once(':')?;
    if head.contains(' ') || text.trim().is_empty() || head.starts_with('@') {
        return None;
    }
    let (base_name, args) = split_call_head(head.trim());
    if !is_identifier(base_name) {
        return None;
    }
    let text = text.trim_start();
    let callee = if speaker_presets.contains(base_name) {
        args.map_or_else(
            || base_name.to_owned(),
            |args| format!("{base_name}({args})"),
        )
    } else if args.is_some() || character_aliases.contains(base_name) {
        args.map_or_else(
            || format!("{base_name}.say()"),
            |args| format!("{base_name}.say({args})"),
        )
    } else {
        format!("{base_name}.say()")
    };
    Some(TextEdit {
        start: base + leading,
        end: base + line.len(),
        replacement: format!("{callee}[{text}]"),
    })
}

fn split_call_head(head: &str) -> (&str, Option<&str>) {
    let Some(open) = head.find('(') else {
        return (head, None);
    };
    if !head.ends_with(')') {
        return (head, None);
    }
    (&head[..open], Some(&head[open + 1..head.len() - 1]))
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn closing_brace_insert(lines: &CstLineEvents, with_start: usize) -> Option<TextEdit> {
    let index = lines.iter().position(|line| line.start() == with_start)?;
    let line = lines.get(index)?;
    let indent = leading_whitespace(line.text());
    let mut last_body = line;
    for candidate in lines.iter().skip(index + 1) {
        if candidate.trimmed().is_empty() {
            last_body = candidate;
            continue;
        }
        if leading_whitespace(candidate.text()).len() <= indent.len() {
            break;
        }
        last_body = candidate;
    }
    let insert_at = last_body.end();
    Some(TextEdit {
        start: insert_at,
        end: insert_at,
        replacement: format!("\n{indent}}}"),
    })
}

fn leading_whitespace(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

fn dedupe_edits(edits: &mut Vec<TextEdit>) {
    edits.sort_by_key(|edit| (edit.start, edit.end, edit.replacement.clone()));
    edits.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_format_preserves_sugar() {
        let source = "flow @flow.opening opening {\n    alice: hi[p]\n}\n";
        let report = format_source(source, FormatOptions::default()).expect("format report");
        assert!(!report.changed);
        assert_eq!(report.output, source);
    }

    #[test]
    fn expands_speaker_with_and_parent_sugar() {
        let source = "pub surface character @character.alice Alice as alice {}\nflow @flow.opening opening {\n    alice: hi[p]\n    with:\n        log.info(\"x\")\n    goto parent::next\n}\n";
        let report =
            format_source(source, FormatOptions { expand_sugar: true }).expect("format report");
        assert!(report.output.contains("alice.say()[hi[p]]"));
        assert!(report.output.contains("with {"));
        assert!(report.output.contains("    }"));
        assert!(report.output.contains("goto super::next"));
    }

    #[test]
    fn expands_speaker_presets_from_typed_tree_without_helper_false_positive() {
        let source = "pub surface character @character.alice Alice as alice {}\nflow @flow.opening opening {\n    let alice2 = alice(voice=auto)\n    let helper = compute()\n    alice2: preset[p]\n    helper: helper[p]\n}\n";
        let report =
            format_source(source, FormatOptions { expand_sugar: true }).expect("format report");

        assert!(report.output.contains("alice2[preset[p]]"));
        assert!(report.output.contains("helper.say()[helper[p]]"));
        assert!(!report.output.contains("helper[helper[p]]"));
    }

    #[test]
    fn expands_chained_speaker_presets_from_typed_tree() {
        let source = "pub surface character @character.alice Alice as alice {}\nflow @flow.opening opening {\n    let alice2 = alice(voice=auto)\n    let alice3 = alice2(face=smile)\n    alice3: chained[p]\n}\n";
        let report =
            format_source(source, FormatOptions { expand_sugar: true }).expect("format report");

        assert!(report.output.contains("alice3[chained[p]]"));
    }

    #[test]
    fn materializes_top_level_and_choice_ids() {
        let source = "flow @flow.opening opening {\n    choice @.first {\n        @.listen \"Listen\" -> @flow.next\n    }\n}\ntest @.smoke scenario {}\n";
        let report = materialize_ids(source).expect("materialize report");
        assert!(report.output.contains("choice @choice.opening.first"));
        assert!(report.output.contains("@choice.opening.first.listen"));
        assert!(report.output.contains("test @test.smoke scenario"));
    }

    #[test]
    fn materializes_dialogue_line_option_ids() {
        let source = "flow @flow.opening opening {\n    scope outer {\n        scope rain {\n            地の文(id=@say:.sound):\n                雨の音。[p]\n            alice(id=@.comment, text_key=@.comment_text):\n                Good morning.[p]\n            alice.say(id=@...shared, text_key=@super.inner_text)[\n                Shared.[p]\n            ]\n        }\n    }\n}\n";
        let report = materialize_ids(source).expect("materialize report");

        assert!(report.output.contains(
            "地の文(id=@say.opening.narrator.outer.rain.sound, text_key=@text.opening.narrator.outer.rain.sound):"
        ));
        assert!(report.output.contains(
            "alice(id=@say.opening.alice.outer.rain.comment, text_key=@text.opening.alice.outer.rain.comment_text):"
        ));
        assert!(report.output.contains(
            "alice.say(id=@say.opening.alice.shared, text_key=@text.opening.alice.outer.inner_text)["
        ));
    }

    #[test]
    fn materializes_omitted_dialogue_ids_in_colon_call_and_flat_fences() {
        let source = "flow @flow.opening opening {\n    alice:\n        Hi[p]\n    alice.say()[\n        Again[p]\n    ]\n=== scope rain ===\n=== line 地の文 ===\n雨。[p]\n=== with ===\nwait mark .done\n=== /with ===\n=== /line ===\n=== /scope ===\n}\n";
        let report = materialize_ids(source).expect("materialize report");

        assert!(
            report
                .output
                .contains("alice(id=@say.opening.alice.001, text_key=@text.opening.alice.001):")
        );
        assert!(
            report.output.contains(
                "alice.say(id=@say.opening.alice.002, text_key=@text.opening.alice.002)["
            )
        );
        assert!(report.output.contains(
            "=== line 地の文(id=@say.opening.narrator.rain.001, text_key=@text.opening.narrator.rain.001) ==="
        ));
        assert!(report.output.contains("=== with ==="));
    }
}
