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
        dialogue::{
            DialogueContent, DialogueDefaultAssignOp, DialogueDefaultAssignment,
            DialogueDefaultsItem,
        },
        flow::{AwaitBranch, FlowItem, Stmt},
        items::{Attribute, EntityDeclKind, Item},
        line_plan::LinePlanItem,
        pattern::Pattern,
    },
    cst::{CstLineEvents, CstLineKind, cst_lines},
    expr::Expr,
    lint::{SyntaxLintCode, lint_id_policy},
    parser::parse_source,
    source::ParsedSource,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::ops::Range;
use thiserror::Error;

/// Formatting and source normalization options.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FormatOptions {
    /// Rewrite script-friendly sugar into canonical block/call forms.
    pub expand_sugar: bool,
    /// Rewrite inferred rich-text tags into explicit style/layout/transform/effect/object spans.
    pub canonical_rich_text: bool,
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
    #[error("text edit range {start}..{end} does not align to UTF-8 character boundaries")]
    InvalidCharBoundary { start: usize, end: usize },
}

/// Formats source while preserving authoring sugar by default.
pub fn format_source(
    source: &str,
    options: FormatOptions,
) -> Result<ToolingEditReport, ToolingError> {
    let mut edits = Vec::new();
    if options.expand_sugar {
        edits.extend(sugar_expansion_edits(source));
    } else if options.canonical_rich_text {
        edits.extend(rich_text_canonical_edits(source));
    }
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
    if let Ok(report) = format_source(
        source,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    ) && report.changed
    {
        actions.push(rewrite_action(
            "arcweft.expandSugar",
            "Expand Arcweft sugar",
            source,
            report.output,
        ));
    }
    if let Ok(report) = format_source(
        source,
        FormatOptions {
            expand_sugar: false,
            canonical_rich_text: true,
        },
    ) && report.changed
    {
        actions.push(rewrite_action(
            "arcweft.canonicalRichText",
            "Canonicalize inferred rich-text tags",
            source,
            report.output,
        ));
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

fn rewrite_action(
    id: impl Into<String>,
    label: impl Into<String>,
    source: &str,
    output: String,
) -> ToolingCodeAction {
    ToolingCodeAction {
        id: id.into(),
        label: label.into(),
        edit: Some(TextEdit {
            start: 0,
            end: source.len(),
            replacement: output,
        }),
    }
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
        if !source.is_char_boundary(edit.start) || !source.is_char_boundary(edit.end) {
            return Err(ToolingError::InvalidCharBoundary {
                start: edit.start,
                end: edit.end,
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
    let dialogue_content_ranges = collect_dialogue_content_ranges(source, &parsed);
    let mut edits = Vec::new();
    edits.extend(declaration_identity_edits(source, &parsed));
    edits.extend(dialogue_defaults_nested_assignment_edits(source, &parsed));

    for line in lines.iter() {
        if line.kind() == CstLineKind::Comment {
            continue;
        }
        if line_starts_inside_any_dialogue_content(
            line.start(),
            line.text(),
            &dialogue_content_ranges,
        ) {
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
    let context = DialogueSugarContext::from_parsed(&parsed);
    for edit in dialogue_text_sugar_edits(source, &parsed, DialogueSugarMode::All, &context) {
        if !edits.iter().any(|existing| edits_overlap(existing, &edit)) {
            edits.push(edit);
        }
    }
    edits
}

fn line_starts_inside_any_dialogue_content(
    line_start: usize,
    line_text: &str,
    ranges: &[Range<usize>],
) -> bool {
    let trimmed_start = line_start + line_text.len() - line_text.trim_start().len();
    ranges
        .iter()
        .any(|range| range.contains(&line_start) || range.contains(&trimmed_start))
}

fn collect_dialogue_content_ranges(source: &str, parsed: &ParsedSource) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    for item in parsed.typed_tree().items() {
        collect_dialogue_content_ranges_from_item(source, item, &mut ranges);
    }
    ranges
}

fn collect_dialogue_content_ranges_from_item(
    source: &str,
    item: &Item,
    ranges: &mut Vec<Range<usize>>,
) {
    match item {
        Item::Flow(flow) => {
            for item in flow.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        Item::FlowItem(item) => {
            collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
        }
        _ => {}
    }
}

fn collect_dialogue_content_ranges_from_flow_item(
    source: &str,
    item: &FlowItem,
    ranges: &mut Vec<Range<usize>>,
) {
    match item {
        FlowItem::SpeakerLine(line) => push_dialogue_content_range(source, line.content(), ranges),
        FlowItem::ContentCall(call) => push_dialogue_content_range(source, call.content(), ranges),
        FlowItem::Scope(scope) => {
            for item in scope.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::If(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::IfLet(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::Match(block) => {
            for arm in block.arms() {
                for item in arm.body() {
                    collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
                }
            }
        }
        FlowItem::Loop(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::While(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::WhileLet(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::For(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::Select(block) => {
            for branch in block.branches() {
                for item in branch.body() {
                    collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
                }
            }
        }
        FlowItem::BorrowBlock(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::SourceLocale(block) => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        FlowItem::AwaitWith(await_with) => {
            for branch in await_with.branches() {
                for item in branch.body() {
                    collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
                }
            }
        }
        FlowItem::Stmt(stmt) => collect_dialogue_content_ranges_from_stmt(source, stmt, ranges),
        FlowItem::Choice(_) | FlowItem::Include(_) | FlowItem::Raw(_) => {}
    }
}

fn collect_dialogue_content_ranges_from_stmt(
    source: &str,
    stmt: &Stmt,
    ranges: &mut Vec<Range<usize>>,
) {
    match stmt {
        Stmt::Let {
            expr,
            expr_source,
            expr_range,
            ..
        } => collect_dialogue_content_ranges_from_expr(
            expr,
            expr_source.as_deref(),
            expr_range.as_ref(),
            ranges,
        ),
        Stmt::LetElse { else_body, .. } => {
            for stmt in else_body {
                collect_dialogue_content_ranges_from_stmt(source, stmt, ranges);
            }
        }
        Stmt::LetScope { scope, .. } => {
            for stmt in scope.statements() {
                collect_dialogue_content_ranges_from_stmt(source, stmt, ranges);
            }
        }
        Stmt::LetLoop { block, .. } => {
            for item in block.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        Stmt::LetAwait { await_with, .. } => {
            for branch in await_with.branches() {
                for item in branch.body() {
                    collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
                }
            }
        }
        Stmt::Thread(thread) => {
            for item in thread.body() {
                collect_dialogue_content_ranges_from_flow_item(source, item, ranges);
            }
        }
        Stmt::DeferBlock { statements, .. }
        | Stmt::On {
            body: statements, ..
        }
        | Stmt::UnsafeLifetime {
            body: statements, ..
        }
        | Stmt::If {
            body: statements, ..
        }
        | Stmt::Loop {
            body: statements, ..
        }
        | Stmt::While {
            body: statements, ..
        }
        | Stmt::WhileLet {
            body: statements, ..
        }
        | Stmt::For {
            body: statements, ..
        } => {
            for stmt in statements {
                collect_dialogue_content_ranges_from_stmt(source, stmt, ranges);
            }
        }
        Stmt::Match { arms, .. } => {
            for arm in arms {
                for stmt in arm.body() {
                    collect_dialogue_content_ranges_from_stmt(source, stmt, ranges);
                }
            }
        }
        Stmt::LetChoice { .. }
        | Stmt::Return(_)
        | Stmt::Out { .. }
        | Stmt::Goto(_)
        | Stmt::Defer { .. }
        | Stmt::Yield(_)
        | Stmt::Signal { .. }
        | Stmt::LifetimeSet { .. }
        | Stmt::Wait(_)
        | Stmt::Close(_)
        | Stmt::Select(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Expr(_)
        | Stmt::Raw(_) => {}
    }
}

fn collect_dialogue_content_ranges_from_expr(
    expr: &Expr,
    expr_source: Option<&str>,
    expr_range: Option<&arcweft_lang_syntax::ast::common::TextRange>,
    ranges: &mut Vec<Range<usize>>,
) {
    match expr {
        Expr::DialogueCall { content, .. } => {
            let (Some(expr_source), Some(expr_range)) = (expr_source, expr_range) else {
                return;
            };
            let Some(content_start) = expr_source.find(content) else {
                return;
            };
            let start = expr_range.start() + content_start;
            ranges.push(start..start + content.len());
        }
        Expr::Try { expr } => {
            collect_dialogue_content_ranges_from_expr(expr, expr_source, expr_range, ranges);
        }
        _ => {}
    }
}

fn push_dialogue_content_range(
    source: &str,
    content: &DialogueContent,
    ranges: &mut Vec<Range<usize>>,
) {
    let Some(base) = dialogue_content_source_base(source, content) else {
        return;
    };
    ranges.push(base..base + content.raw().len());
}

fn dialogue_defaults_nested_assignment_edits(source: &str, parsed: &ParsedSource) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    for defaults in parsed
        .typed_tree()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::DialogueDefaults(defaults) => Some(defaults),
            _ => None,
        })
    {
        edits.extend(dialogue_defaults_nested_assignment_runs(source, defaults));
    }
    edits
}

fn dialogue_defaults_nested_assignment_runs(
    source: &str,
    defaults: &DialogueDefaultsItem,
) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    let mut pending = NestedAssignmentRun::default();
    for assignment in defaults.assignments() {
        if let Some(dotted) = DottedAssignment::new(source, assignment) {
            if !pending.can_append(source, &dotted)
                && let Some(edit) = pending.finish()
            {
                edits.push(edit);
            }
            pending.push(&dotted);
        } else if let Some(edit) = pending.finish() {
            edits.push(edit);
        }
    }
    if let Some(edit) = pending.finish() {
        edits.push(edit);
    }
    edits
}

#[derive(Debug)]
struct DottedAssignment {
    line_start: usize,
    end: usize,
    indent: String,
    parts: Vec<String>,
    op: &'static str,
    value: String,
}

impl DottedAssignment {
    fn new(source: &str, assignment: &DialogueDefaultAssignment) -> Option<Self> {
        let path_source = source.get(assignment.path_range().as_range())?.trim();
        let parts = path_source
            .split('.')
            .filter(|part| !part.trim().is_empty())
            .map(|part| part.trim().to_owned())
            .collect::<Vec<_>>();
        if parts.len() <= 1 {
            return None;
        }
        let line_start = source[..assignment.range().start()]
            .rfind('\n')
            .map_or(0, |offset| offset + 1);
        Some(Self {
            line_start,
            end: assignment.range().end(),
            indent: source
                .get(line_start..assignment.range().start())?
                .to_owned(),
            parts,
            op: assignment_op_label(assignment.op()),
            value: assignment.raw_value().to_owned(),
        })
    }
}

#[derive(Debug, Default)]
struct NestedAssignmentRun {
    start: Option<usize>,
    end: usize,
    indent: String,
    tree: Vec<NestedAssignmentNode>,
}

impl NestedAssignmentRun {
    fn can_append(&self, source: &str, assignment: &DottedAssignment) -> bool {
        let Some(start) = self.start else {
            return true;
        };
        start <= assignment.line_start
            && self.end <= assignment.line_start
            && source
                .get(self.end..assignment.line_start)
                .is_some_and(|between| between.trim().is_empty())
    }

    fn push(&mut self, assignment: &DottedAssignment) {
        if self.start.is_none() {
            self.start = Some(assignment.line_start);
            self.indent.clone_from(&assignment.indent);
        }
        self.end = assignment.end;
        insert_nested_assignment(
            &mut self.tree,
            &assignment.parts,
            assignment.op,
            &assignment.value,
        );
    }

    fn finish(&mut self) -> Option<TextEdit> {
        let start = self.start?;
        let replacement = nested_assignment_tree_text(&self.indent, &self.tree);
        let edit = TextEdit {
            start,
            end: self.end,
            replacement,
        };
        *self = Self::default();
        Some(edit)
    }
}

#[derive(Debug)]
enum NestedAssignmentNode {
    Block {
        name: String,
        children: Vec<NestedAssignmentNode>,
    },
    Leaf {
        name: String,
        op: &'static str,
        value: String,
    },
}

fn insert_nested_assignment(
    nodes: &mut Vec<NestedAssignmentNode>,
    parts: &[String],
    op: &'static str,
    value: &str,
) {
    if let Some((head, tail)) = parts.split_first() {
        if tail.is_empty() {
            nodes.push(NestedAssignmentNode::Leaf {
                name: head.clone(),
                op,
                value: value.to_owned(),
            });
            return;
        }
        if let Some(NestedAssignmentNode::Block { children, .. }) =
            nodes.iter_mut().find(|node| match node {
                NestedAssignmentNode::Block { name, .. } => name == head,
                NestedAssignmentNode::Leaf { .. } => false,
            })
        {
            insert_nested_assignment(children, tail, op, value);
            return;
        }
        let mut children = Vec::new();
        insert_nested_assignment(&mut children, tail, op, value);
        nodes.push(NestedAssignmentNode::Block {
            name: head.clone(),
            children,
        });
    }
}

fn nested_assignment_tree_text(indent: &str, nodes: &[NestedAssignmentNode]) -> String {
    let mut output = String::new();
    for (index, node) in nodes.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        write_nested_assignment_node(&mut output, indent, 0, node);
    }
    output
}

fn write_nested_assignment_node(
    output: &mut String,
    indent: &str,
    depth: usize,
    node: &NestedAssignmentNode,
) {
    output.push_str(indent);
    output.push_str(&"    ".repeat(depth));
    match node {
        NestedAssignmentNode::Block { name, children } => {
            output.push_str(name);
            output.push_str(" {");
            for child in children {
                output.push('\n');
                write_nested_assignment_node(output, indent, depth + 1, child);
            }
            output.push('\n');
            output.push_str(indent);
            output.push_str(&"    ".repeat(depth));
            output.push('}');
        }
        NestedAssignmentNode::Leaf { name, op, value } => {
            output.push_str(name);
            output.push(' ');
            output.push_str(op);
            output.push(' ');
            output.push_str(value);
        }
    }
}

fn assignment_op_label(op: DialogueDefaultAssignOp) -> &'static str {
    match op {
        DialogueDefaultAssignOp::Replace => "=",
        DialogueDefaultAssignOp::Append => "+=",
    }
}

fn declaration_identity_edits(source: &str, parsed: &ParsedSource) -> Vec<TextEdit> {
    lint_id_policy(parsed.typed_tree())
        .into_iter()
        .filter(|lint| lint.code() == SyntaxLintCode::RedundantDeclIdentity)
        .filter_map(|lint| {
            redundant_decl_identity_edit(source, lint.range().start(), lint.range().end())
        })
        .collect()
}

fn redundant_decl_identity_edit(source: &str, id_start: usize, id_end: usize) -> Option<TextEdit> {
    let edit_start = id_start
        .checked_sub(1)
        .filter(|start| source.as_bytes().get(*start) == Some(&b'@'))
        .unwrap_or(id_start);
    let id_token = source.get(id_start..id_end)?;
    let line_end = source[id_end..]
        .find('\n')
        .map_or(source.len(), |offset| id_end + offset);
    let after_id_raw = source.get(id_end..line_end)?;
    let after_id = after_id_raw.trim_start();
    let id_gap = after_id_raw.len() - after_id.len();
    let name_len = after_id
        .find(|ch: char| ch.is_whitespace() || ch == '(' || ch == ':' || ch == '{')
        .unwrap_or(after_id.len());
    if name_len == 0 {
        return None;
    }
    let name = &after_id[..name_len];
    let id_tail = id_token.trim_start_matches('@').rsplit('.').next()?;
    if id_tail != name {
        return None;
    }
    let end = id_end + id_gap + name_len;
    Some(TextEdit {
        start: edit_start,
        end,
        replacement: name.to_owned(),
    })
}

fn rich_text_canonical_edits(source: &str) -> Vec<TextEdit> {
    let parsed = parse_source(source);
    let context = DialogueSugarContext::from_parsed(&parsed);
    dialogue_text_sugar_edits(source, &parsed, DialogueSugarMode::RichTextOnly, &context)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DialogueSugarMode {
    All,
    RichTextOnly,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DialogueSugarContext {
    text_proxy_types: BTreeSet<String>,
}

impl DialogueSugarContext {
    fn from_parsed(parsed: &ParsedSource) -> Self {
        Self {
            text_proxy_types: collect_text_proxy_type_names(parsed),
        }
    }
}

fn edits_overlap(lhs: &TextEdit, rhs: &TextEdit) -> bool {
    lhs.start < rhs.end && rhs.start < lhs.end
}

fn dialogue_text_sugar_edits(
    source: &str,
    parsed: &ParsedSource,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    for item in parsed.typed_tree().items() {
        collect_dialogue_text_sugar_edits_from_item(source, item, &mut edits, mode, context);
    }
    edits
}

fn collect_dialogue_text_sugar_edits_from_item(
    source: &str,
    item: &Item,
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) {
    match item {
        Item::Flow(flow) => {
            for item in flow.body() {
                collect_dialogue_text_sugar_edits_from_flow_item(
                    source, item, edits, mode, context,
                );
            }
        }
        Item::FlowItem(item) => {
            collect_dialogue_text_sugar_edits_from_flow_item(source, item, edits, mode, context);
        }
        _ => {}
    }
}

fn collect_dialogue_text_sugar_edits_from_flow_item(
    source: &str,
    item: &FlowItem,
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) {
    match item {
        FlowItem::SpeakerLine(line) => {
            collect_dialogue_content_sugar_edits(source, line.content(), edits, mode, context);
        }
        FlowItem::ContentCall(call) => {
            collect_dialogue_content_sugar_edits(source, call.content(), edits, mode, context);
        }
        FlowItem::Stmt(stmt) => {
            collect_dialogue_text_sugar_edits_from_stmt(source, stmt, edits, mode, context);
        }
        FlowItem::Choice(_) | FlowItem::Include(_) | FlowItem::Raw(_) => {}
        _ => collect_dialogue_text_sugar_edits_from_flow_item_children(
            source, item, edits, mode, context,
        ),
    }
}

fn collect_dialogue_text_sugar_edits_from_flow_item_children(
    source: &str,
    item: &FlowItem,
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) {
    if let Some(body) = flow_item_body(item) {
        collect_dialogue_text_sugar_edits_from_flow_items(source, body, edits, mode, context);
        return;
    }
    match item {
        FlowItem::Match(block) => {
            for arm in block.arms() {
                collect_dialogue_text_sugar_edits_from_flow_items(
                    source,
                    arm.body(),
                    edits,
                    mode,
                    context,
                );
            }
        }
        FlowItem::Select(block) => {
            for branch in block.branches() {
                collect_dialogue_text_sugar_edits_from_flow_items(
                    source,
                    branch.body(),
                    edits,
                    mode,
                    context,
                );
            }
        }
        FlowItem::AwaitWith(await_with) => {
            for branch in await_with.branches() {
                collect_dialogue_text_sugar_edits_from_flow_items(
                    source,
                    branch.body(),
                    edits,
                    mode,
                    context,
                );
            }
        }
        FlowItem::Scope(_)
        | FlowItem::If(_)
        | FlowItem::IfLet(_)
        | FlowItem::Loop(_)
        | FlowItem::While(_)
        | FlowItem::WhileLet(_)
        | FlowItem::For(_)
        | FlowItem::BorrowBlock(_)
        | FlowItem::SourceLocale(_)
        | FlowItem::SpeakerLine(_)
        | FlowItem::ContentCall(_)
        | FlowItem::Stmt(_)
        | FlowItem::Choice(_)
        | FlowItem::Include(_)
        | FlowItem::Raw(_) => {}
    }
}

fn flow_item_body(item: &FlowItem) -> Option<&[FlowItem]> {
    match item {
        FlowItem::Scope(block) => Some(block.body()),
        FlowItem::If(block) => Some(block.body()),
        FlowItem::IfLet(block) => Some(block.body()),
        FlowItem::Loop(block) => Some(block.body()),
        FlowItem::While(block) => Some(block.body()),
        FlowItem::WhileLet(block) => Some(block.body()),
        FlowItem::For(block) => Some(block.body()),
        FlowItem::BorrowBlock(block) => Some(block.body()),
        FlowItem::SourceLocale(block) => Some(block.body()),
        _ => None,
    }
}

fn collect_dialogue_text_sugar_edits_from_flow_items(
    source: &str,
    items: &[FlowItem],
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) {
    for item in items {
        collect_dialogue_text_sugar_edits_from_flow_item(source, item, edits, mode, context);
    }
}

fn collect_dialogue_text_sugar_edits_from_stmt(
    source: &str,
    stmt: &Stmt,
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) {
    match stmt {
        Stmt::Let {
            expr,
            expr_source,
            expr_range,
            ..
        } => collect_dialogue_text_sugar_edits_from_expr(
            expr,
            expr_source.as_deref(),
            expr_range.as_ref(),
            edits,
            mode,
            context,
        ),
        Stmt::LetElse { else_body, .. } => {
            for stmt in else_body {
                collect_dialogue_text_sugar_edits_from_stmt(source, stmt, edits, mode, context);
            }
        }
        Stmt::LetScope { scope, .. } => {
            for stmt in scope.statements() {
                collect_dialogue_text_sugar_edits_from_stmt(source, stmt, edits, mode, context);
            }
        }
        Stmt::LetLoop { block, .. } => {
            for item in block.body() {
                collect_dialogue_text_sugar_edits_from_flow_item(
                    source, item, edits, mode, context,
                );
            }
        }
        Stmt::LetAwait { await_with, .. } => {
            for branch in await_with.branches() {
                for item in branch.body() {
                    collect_dialogue_text_sugar_edits_from_flow_item(
                        source, item, edits, mode, context,
                    );
                }
            }
        }
        Stmt::Thread(thread) => {
            for item in thread.body() {
                collect_dialogue_text_sugar_edits_from_flow_item(
                    source, item, edits, mode, context,
                );
            }
        }
        Stmt::DeferBlock { statements, .. }
        | Stmt::On {
            body: statements, ..
        }
        | Stmt::UnsafeLifetime {
            body: statements, ..
        }
        | Stmt::If {
            body: statements, ..
        }
        | Stmt::Loop {
            body: statements, ..
        }
        | Stmt::While {
            body: statements, ..
        }
        | Stmt::WhileLet {
            body: statements, ..
        }
        | Stmt::For {
            body: statements, ..
        } => {
            for stmt in statements {
                collect_dialogue_text_sugar_edits_from_stmt(source, stmt, edits, mode, context);
            }
        }
        Stmt::Match { arms, .. } => {
            for arm in arms {
                for stmt in arm.body() {
                    collect_dialogue_text_sugar_edits_from_stmt(source, stmt, edits, mode, context);
                }
            }
        }
        Stmt::LetChoice { .. }
        | Stmt::Return(_)
        | Stmt::Out { .. }
        | Stmt::Goto(_)
        | Stmt::Defer { .. }
        | Stmt::Yield(_)
        | Stmt::Signal { .. }
        | Stmt::LifetimeSet { .. }
        | Stmt::Wait(_)
        | Stmt::Close(_)
        | Stmt::Select(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Expr(_)
        | Stmt::Raw(_) => {}
    }
}

fn collect_dialogue_text_sugar_edits_from_expr(
    expr: &Expr,
    expr_source: Option<&str>,
    expr_range: Option<&arcweft_lang_syntax::ast::common::TextRange>,
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) {
    match expr {
        Expr::DialogueCall { content, .. } => {
            let (Some(expr_source), Some(expr_range)) = (expr_source, expr_range) else {
                return;
            };
            let Some(content_start) = expr_source.find(content) else {
                return;
            };
            edits.extend(dialogue_text_canonical_edits(
                content,
                expr_range.start() + content_start,
                mode,
                context,
            ));
        }
        Expr::Try { expr } => {
            collect_dialogue_text_sugar_edits_from_expr(
                expr,
                expr_source,
                expr_range,
                edits,
                mode,
                context,
            );
        }
        _ => {}
    }
}

fn collect_dialogue_content_sugar_edits(
    source: &str,
    content: &DialogueContent,
    edits: &mut Vec<TextEdit>,
    mode: DialogueSugarMode,
    sugar_context: &DialogueSugarContext,
) {
    let Some(base) = dialogue_content_source_base(source, content) else {
        return;
    };
    edits.extend(dialogue_text_canonical_edits(
        content.raw(),
        base,
        mode,
        sugar_context,
    ));
}

fn dialogue_content_source_base(source: &str, content: &DialogueContent) -> Option<usize> {
    let range = content.range();
    if let Some(slice) = source.get(range.start()..range.end()) {
        if slice == content.raw() {
            return Some(range.start());
        }
        if let Some(relative) = slice.find(content.raw()) {
            return Some(range.start() + relative);
        }
    }
    // Nested parser ranges are currently local to their flow body. Use a
    // source search fallback so formatter sugar expansion can still operate
    // until syntax ranges are fully rebased in the typed tree.
    source.find(content.raw())
}

fn dialogue_text_canonical_edits(
    raw: &str,
    base: usize,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) -> Vec<TextEdit> {
    let mut edits = Vec::new();
    let mut cursor = 0;
    let mut inferred_span_stack = Vec::new();
    while cursor < raw.len() {
        let Some(ch) = raw[cursor..].chars().next() else {
            break;
        };
        match ch {
            '\\' => {
                cursor += ch.len_utf8();
                if let Some(escaped) = raw[cursor..].chars().next() {
                    cursor += escaped.len_utf8();
                }
            }
            '｜' if mode == DialogueSugarMode::All => {
                if let Some((end, replacement)) = natural_ruby_edit(raw, cursor) {
                    edits.push(TextEdit {
                        start: base + cursor,
                        end: base + end,
                        replacement,
                    });
                    cursor = end;
                } else {
                    cursor += ch.len_utf8();
                }
            }
            '|' if mode == DialogueSugarMode::All => {
                if let Some((end, replacement)) = compact_ruby_edit(raw, cursor) {
                    edits.push(TextEdit {
                        start: base + cursor,
                        end: base + end,
                        replacement,
                    });
                    cursor = end;
                } else {
                    cursor += ch.len_utf8();
                }
            }
            '$' if mode == DialogueSugarMode::All => {
                if let Some((end, replacement)) = dollar_expr_edit(raw, cursor) {
                    edits.push(TextEdit {
                        start: base + cursor,
                        end: base + end,
                        replacement,
                    });
                    cursor = end;
                } else {
                    cursor += ch.len_utf8();
                }
            }
            '[' => {
                if let Some((end, replacement)) =
                    bracket_dialogue_edit(raw, cursor, &mut inferred_span_stack, mode, context)
                {
                    edits.push(TextEdit {
                        start: base + cursor,
                        end: base + end,
                        replacement,
                    });
                    cursor = end;
                } else if let Some(end) = raw_span_end(raw, cursor) {
                    cursor = end;
                } else {
                    cursor += ch.len_utf8();
                }
            }
            _ => cursor += ch.len_utf8(),
        }
    }
    edits
}

fn natural_ruby_edit(raw: &str, start: usize) -> Option<(usize, String)> {
    let after_marker = start + '｜'.len_utf8();
    let tail = raw.get(after_marker..)?;
    let open = tail.find('《')?;
    let base_text = &tail[..open];
    let ruby_start = after_marker + open + '《'.len_utf8();
    let ruby_tail = raw.get(ruby_start..)?;
    let close = ruby_tail.find('》')?;
    let ruby = &ruby_tail[..close];
    if base_text.is_empty() || ruby.is_empty() {
        return None;
    }
    Some((
        ruby_start + close + '》'.len_utf8(),
        format!("|[{base_text}]({ruby})"),
    ))
}

fn compact_ruby_edit(raw: &str, start: usize) -> Option<(usize, String)> {
    let after_marker = start + '|'.len_utf8();
    let tail = raw.get(after_marker..)?;
    if tail.starts_with('[') {
        return None;
    }
    let open = tail.find('{')?;
    let base_text = &tail[..open];
    if base_text.is_empty()
        || base_text
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '[' | ']' | '{' | '}' | '#' | '|'))
    {
        return None;
    }
    let ruby_start = after_marker + open + '{'.len_utf8();
    let ruby_tail = raw.get(ruby_start..)?;
    let close = ruby_tail.find('}')?;
    let ruby = &ruby_tail[..close];
    if ruby.is_empty() {
        return None;
    }
    Some((
        ruby_start + close + '}'.len_utf8(),
        format!("|[{base_text}]({ruby})"),
    ))
}

fn dollar_expr_edit(raw: &str, start: usize) -> Option<(usize, String)> {
    let expr_start = start + "$(".len();
    let end = balanced_close(raw, expr_start, '(', ')')?;
    let expr = raw.get(expr_start..end - ')'.len_utf8())?;
    Some((end, format!("#[{expr}]")))
}

fn bracket_dialogue_edit(
    raw: &str,
    start: usize,
    inferred_span_stack: &mut Vec<Option<&'static str>>,
    mode: DialogueSugarMode,
    context: &DialogueSugarContext,
) -> Option<(usize, String)> {
    if mode == DialogueSugarMode::All
        && let Some(body) = raw.get(start..)?.strip_prefix("[raw:")
    {
        let close_relative = raw_colon_close(body)?;
        let raw_body = body[..close_relative].trim_start();
        return Some((
            start + "[raw:".len() + close_relative + ']'.len_utf8(),
            format!("[raw]{raw_body}[/raw]"),
        ));
    }
    let close = raw.get(start + '['.len_utf8()..)?.find(']')? + start + '['.len_utf8();
    let inside = raw.get(start + '['.len_utf8()..close)?.trim();
    let end = close + ']'.len_utf8();
    if inside == "/" {
        return inferred_span_stack
            .pop()
            .map(|family| family.map_or_else(String::new, |family| format!("[/{family}]")))
            .map(|replacement| (end, replacement));
    }
    if mode == DialogueSugarMode::All && inside == "page" {
        return Some((end, "[p]".to_owned()));
    }
    if mode == DialogueSugarMode::All && inside == "wait" {
        return Some((end, "[l]".to_owned()));
    }
    if mode == DialogueSugarMode::All && inside == "nl" {
        return Some((end, "[r]".to_owned()));
    }
    if mode == DialogueSugarMode::All
        && let Some(rest) = inside.strip_prefix("! ")
    {
        return Some((end, format!("[call {rest}]")));
    }
    if inside.starts_with('.') && inside.len() > 1 {
        let (selector, attrs) = split_dialogue_tag_head(inside);
        let selector_name = selector.trim_start_matches('.');
        let family = inferred_rich_text_family(selector_name, attrs, context);
        if let Some(family) = family {
            inferred_span_stack.push(Some(family));
            let attrs = canonical_object_attrs(selector_name, attrs, context)
                .filter(|_| family == "object")
                .unwrap_or_else(|| attrs.to_owned());
            let replacement = if attrs.is_empty() {
                format!("[{family} {selector}]")
            } else {
                format!("[{family} {selector} {attrs}]")
            };
            return Some((end, replacement));
        }
        inferred_span_stack.push(None);
        return Some((end, format!("[mark {selector}]")));
    }
    if mode == DialogueSugarMode::All
        && let Some(time) = inside.strip_prefix("w ")
        && !time.contains('=')
    {
        return Some((end, format!("[w time={}]", time.trim())));
    }
    if mode == DialogueSugarMode::All
        && let Some((tag, body)) = inside.split_once(':')
    {
        let body = body.trim_start();
        if tag == "em" || tag == "strong" {
            return Some((end, format!("[{tag}]{body}[/{tag}]")));
        }
        if let Some(color) = tag.strip_prefix("color ") {
            return Some((
                end,
                format!("[color value=\"{}\"]{body}[/color]", color.trim()),
            ));
        }
    }
    if mode == DialogueSugarMode::All {
        rb_tag_edit(raw, start, inside, end)
    } else {
        None
    }
}

fn raw_colon_close(body: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut cursor = 0usize;
    while cursor < body.len() {
        let ch = body[cursor..].chars().next()?;
        match ch {
            '\\' => {
                cursor += ch.len_utf8();
                if let Some(escaped) = body[cursor..].chars().next() {
                    cursor += escaped.len_utf8();
                }
            }
            '[' => {
                depth += 1;
                cursor += ch.len_utf8();
            }
            ']' if depth == 0 => return Some(cursor),
            ']' => {
                depth = depth.saturating_sub(1);
                cursor += ch.len_utf8();
            }
            _ => cursor += ch.len_utf8(),
        }
    }
    None
}

fn split_dialogue_tag_head(source: &str) -> (&str, &str) {
    let mut parts = source.splitn(2, char::is_whitespace);
    (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default().trim(),
    )
}

fn inferred_rich_text_family(
    selector: &str,
    attrs: &str,
    context: &DialogueSugarContext,
) -> Option<&'static str> {
    match selector {
        "italic" | "oblique" | "opacity" | "alpha" | "layer" | "object_layer" | "z" | "z_index" => {
            Some("style")
        }
        "horizontal_tb"
        | "vertical_rl"
        | "vertical_lr"
        | "dir"
        | "ruby_over"
        | "ruby_under"
        | "ruby_inter_character" => Some("layout"),
        "offset" | "pos" | "rotate" | "scale" | "skew" => Some("transform"),
        "wave" | "shake" | "arc" | "spin" | "pulse" | "motion" | "typewriter" | "jitter"
        | "shader" | "host" => Some("effect"),
        _ if inferred_text_proxy_type(selector, attrs, context).is_some() => Some("object"),
        _ if !attrs.trim().is_empty() => Some("effect"),
        _ => None,
    }
}

fn inferred_text_proxy_type<'a>(
    selector: &'a str,
    attrs: &'a str,
    context: &'a DialogueSugarContext,
) -> Option<&'a str> {
    text_proxy_type_attr(attrs)
        .filter(|name| context.text_proxy_types.contains(*name))
        .or_else(|| {
            context
                .text_proxy_types
                .contains(selector)
                .then_some(selector)
        })
}

fn canonical_object_attrs(
    selector: &str,
    attrs: &str,
    context: &DialogueSugarContext,
) -> Option<String> {
    let proxy_type = inferred_text_proxy_type(selector, attrs, context)?;
    if text_proxy_type_attr(attrs).is_some() {
        Some(attrs.to_owned())
    } else if attrs.trim().is_empty() {
        Some(format!("type={proxy_type}"))
    } else {
        Some(format!("type={proxy_type} {}", attrs.trim()))
    }
}

fn text_proxy_type_attr(attrs: &str) -> Option<&str> {
    find_tag_attr(attrs, "type")
        .or_else(|| find_tag_attr(attrs, "struct"))
        .or_else(|| find_tag_attr(attrs, "proxy"))
}

fn find_tag_attr<'a>(attrs: &'a str, key: &str) -> Option<&'a str> {
    attrs.split_whitespace().find_map(|part| {
        let (name, value) = part.split_once('=')?;
        (name == key).then(|| value.trim_matches('"'))
    })
}

fn collect_text_proxy_type_names(parsed: &ParsedSource) -> BTreeSet<String> {
    parsed
        .typed_tree()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Struct(item)
                if item.attrs().iter().any(is_text_proxy_attribute) && !item.name().is_empty() =>
            {
                Some(item.name().to_owned())
            }
            _ => None,
        })
        .collect()
}

fn is_text_proxy_attribute(attr: &Attribute) -> bool {
    matches!(attr.name(), "text_proxy" | "rich_text_proxy")
}

fn rb_tag_edit(raw: &str, _start: usize, inside: &str, open_end: usize) -> Option<(usize, String)> {
    let attrs = inside.strip_prefix("rb")?.trim();
    let ruby = ruby_attr_value(attrs)?;
    let tail = raw.get(open_end..)?;
    let close = tail.find("[/rb]")?;
    let body = raw.get(open_end..open_end + close)?;
    Some((
        open_end + close + "[/rb]".len(),
        format!("[ruby rt=\"{ruby}\"]{body}[/ruby]"),
    ))
}

fn ruby_attr_value(attrs: &str) -> Option<&str> {
    let value = attrs.trim().strip_prefix("rt")?.trim_start();
    let value = value.strip_prefix('=')?.trim_start();
    if let Some(quoted) = value.strip_prefix('"') {
        return quoted.find('"').map(|end| &quoted[..end]);
    }
    let end = value.find(char::is_whitespace).unwrap_or(value.len());
    (end > 0).then_some(&value[..end])
}

fn raw_span_end(raw: &str, start: usize) -> Option<usize> {
    let body_start = start + "[raw]".len();
    raw.get(start..)?.starts_with("[raw]").then_some(())?;
    let close = raw.get(body_start..)?.find("[/raw]")?;
    Some(body_start + close + "[/raw]".len())
}

fn balanced_close(raw: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 1_u32;
    for (relative, ch) in raw.get(start..)?.char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(start + relative + close.len_utf8());
            }
        }
    }
    None
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
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
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
        | Stmt::Close(expr)
        | Stmt::Select(expr)
        | Stmt::Expr(expr) => {
            collect_speaker_presets_from_expr(expr, character_aliases, presets);
        }
        Stmt::Thread(block) => {
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        Stmt::DeferBlock { statements, .. } => {
            collect_speaker_presets_from_stmts(statements, character_aliases, presets);
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
                collect_speaker_presets_from_expr(arg.value(), character_aliases, presets);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_speaker_presets_from_expr(receiver, character_aliases, presets);
            for arg in args {
                collect_speaker_presets_from_expr(arg.value(), character_aliases, presets);
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
            collect_speaker_presets_from_flow_items(block.body(), character_aliases, presets);
        }
        Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::Placeholder(_)
        | Expr::NumericBracketSeq(_)
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
    let text = canonical_dialogue_text_for_speaker_line(text.trim_start());
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

fn canonical_dialogue_text_for_speaker_line(text: &str) -> String {
    let edits = dialogue_text_canonical_edits(
        text,
        0,
        DialogueSugarMode::All,
        &DialogueSugarContext::default(),
    );
    apply_text_edits(text, &edits).unwrap_or_else(|_| text.to_owned())
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
        let report = format_source(
            source,
            FormatOptions {
                expand_sugar: true,
                canonical_rich_text: false,
            },
        )
        .expect("format report");
        assert!(report.output.contains("alice.say()[hi[p]]"));
        assert!(report.output.contains("with {"));
        assert!(report.output.contains("    }"));
        assert!(report.output.contains("goto super::next"));
    }

    #[test]
    fn expand_sugar_canonicalizes_redundant_decl_identity_only() {
        let source = "flow @flow.opening opening {\n}\nflow @flow.opening start {\n}\nsource @source.http_requests http_requests: Source<HttpRequest, HttpError> {\n}\ncharacter @character.alice alice {\n}\n";
        let report = format_source(
            source,
            FormatOptions {
                expand_sugar: true,
                canonical_rich_text: false,
            },
        )
        .expect("format report");

        assert!(report.output.contains("flow opening {"));
        assert!(report.output.contains("flow @flow.opening start {"));
        assert!(
            report
                .output
                .contains("source http_requests: Source<HttpRequest, HttpError>")
        );
        assert!(report.output.contains("character alice {"));
    }

    #[test]
    fn expand_sugar_preserves_generated_decl_identity_surface() {
        let source = "#[generated]\nflow @flow.opening opening {\n}\n#[allow(style::redundant_decl_identity)]\nsource @source.http_requests http_requests: Source<HttpRequest, HttpError> {\n}\n";
        let report = format_source(
            source,
            FormatOptions {
                expand_sugar: true,
                canonical_rich_text: false,
            },
        )
        .expect("format report");

        assert!(!report.changed);
        assert!(report.output.contains("flow @flow.opening opening {"));
        assert!(
            report
                .output
                .contains("source @source.http_requests http_requests")
        );
    }

    #[test]
    fn expand_sugar_preserves_source_generated_decl_identity_surface() {
        let source = "#![generated(tool)]\nflow @flow.generated generated {\n    alice: hi[p]\n}\n";
        let report = format_source(
            source,
            FormatOptions {
                expand_sugar: true,
                canonical_rich_text: false,
            },
        )
        .expect("format report");

        assert!(report.changed);
        assert!(report.output.contains("flow @flow.generated generated {"));
        assert!(report.output.contains("alice.say()[hi[p]]"));
    }

    #[test]
    fn expand_sugar_preserves_source_allowed_decl_identity_surface() {
        let source = "#![allow(style::redundant_decl_identity)]\nflow @flow.generated generated {\n    alice: hi[p]\n}\n";
        let report = format_source(
            source,
            FormatOptions {
                expand_sugar: true,
                canonical_rich_text: false,
            },
        )
        .expect("format report");

        assert!(report.changed);
        assert!(report.output.contains("flow @flow.generated generated {"));
        assert!(report.output.contains("alice.say()[hi[p]]"));
    }

    #[test]
    fn expand_sugar_nests_dotted_dialogue_defaults_assignments() {
        let source = "pub dialogue defaults @dialogue.defaults {\n    rich_text.ruby.size = 14px\n    rich_text.ruby.gap += 1px\n}\n";
        let report = format_source(
            source,
            FormatOptions {
                expand_sugar: true,
                canonical_rich_text: false,
            },
        )
        .expect("format report");

        assert!(report.changed);
        assert!(report.output.contains(
            "    rich_text {\n        ruby {\n            size = 14px\n            gap += 1px\n        }\n    }"
        ));
        assert!(!report.output.contains("rich_text.ruby.size"));
        assert!(!report.output.contains("rich_text.ruby.gap"));
    }

    #[test]
    fn expands_speaker_presets_from_typed_tree_without_helper_false_positive() {
        let source = "pub surface character @character.alice Alice as alice {}\nflow @flow.opening opening {\n    let alice2 = alice(voice=auto)\n    let helper = compute()\n    alice2: preset[p]\n    helper: helper[p]\n}\n";
        let report = format_source(
            source,
            FormatOptions {
                expand_sugar: true,
                canonical_rich_text: false,
            },
        )
        .expect("format report");

        assert!(report.output.contains("alice2[preset[p]]"));
        assert!(report.output.contains("helper.say()[helper[p]]"));
        assert!(!report.output.contains("helper[helper[p]]"));
    }

    #[test]
    fn expands_chained_speaker_presets_from_typed_tree() {
        let source = "pub surface character @character.alice Alice as alice {}\nflow @flow.opening opening {\n    let alice2 = alice(voice=auto)\n    let alice3 = alice2(face=smile)\n    alice3: chained[p]\n}\n";
        let report = format_source(
            source,
            FormatOptions {
                expand_sugar: true,
                canonical_rich_text: false,
            },
        )
        .expect("format report");

        assert!(report.output.contains("alice3[chained[p]]"));
    }

    #[test]
    fn expands_dialogue_authoring_sugar_only_when_requested() {
        let source = "flow @flow.opening opening {\n    alice.say()[今日は｜変な夢《へんなゆめ》と|悪夢{あくむ}。$(name)[! flash()][.mark][w 500ms][page][em:夢][raw: [p]]]\n}\n";
        let preserved = format_source(source, FormatOptions::default()).expect("format report");
        assert_eq!(preserved.output, source);

        let expanded = format_source(
            source,
            FormatOptions {
                expand_sugar: true,
                canonical_rich_text: false,
            },
        )
        .expect("format report");
        assert!(expanded.output.contains("|[変な夢](へんなゆめ)"));
        assert!(expanded.output.contains("|[悪夢](あくむ)"));
        assert!(expanded.output.contains("#[name]"));
        assert!(expanded.output.contains("[call flash()]"));
        assert!(expanded.output.contains("[mark .mark]"));
        assert!(expanded.output.contains("[w time=500ms]"));
        assert!(expanded.output.contains("[p]"));
        assert!(expanded.output.contains("[em]夢[/em]"));
        assert!(expanded.output.contains("[raw][p][/raw]"));
    }

    #[test]
    fn expand_sugar_does_not_treat_dialogue_content_lines_as_speaker_sugar() {
        let source = "flow @flow.opening opening {\n    alice.say()[\n        cue: [raw: [p]や#[expr]をそのまま表示] と [! flash()][p]\n    ]\n}\n";
        let expanded = format_source(
            source,
            FormatOptions {
                expand_sugar: true,
                canonical_rich_text: false,
            },
        )
        .expect("format report");

        assert!(!expanded.output.contains("cue.say()"));
        assert!(
            expanded
                .output
                .contains("cue: [raw][p]や#[expr]をそのまま表示[/raw] と [call flash()][p]")
        );
    }

    #[test]
    fn canonical_rich_text_expands_dot_inference_without_other_sugar() {
        let source = "flow @flow.opening opening {\n    alice: hi $(name)[.keyword][.sparkle amp=2px pattern=a,b,c]there[/][.host id=sparkle amp=1px]hosted[/][page]\n    let handles = alice.say()[[.vertical_rl]縦[/][p]] with: out handles\n}\n";
        let report = format_source(
            source,
            FormatOptions {
                expand_sugar: false,
                canonical_rich_text: true,
            },
        )
        .expect("format report");

        assert!(report.output.contains("$(name)"));
        assert!(report.output.contains("[mark .keyword]"));
        assert!(
            report
                .output
                .contains("[effect .sparkle amp=2px pattern=a,b,c]there[/effect]")
        );
        assert!(
            report
                .output
                .contains("[effect .host id=sparkle amp=1px]hosted[/effect]")
        );
        assert!(
            report
                .output
                .contains("[layout .vertical_rl]縦[/layout][p]")
        );
        assert!(report.output.contains("[page]"));
    }

    #[test]
    fn canonical_rich_text_expands_inferred_text_proxy_objects() {
        let source = "#[text_proxy(kind=\"keyword\", default_hit=true)]\npub struct KeywordHit {\n    channel: String\n}\n\nflow @flow.opening opening {\n    alice: [.hotspot type=KeywordHit channel=choice]proxy[/][.KeywordHit]typed[/][.sparkle amp=2px]effect[/][p]\n}\n";
        let report = format_source(
            source,
            FormatOptions {
                expand_sugar: false,
                canonical_rich_text: true,
            },
        )
        .expect("format report");

        assert!(
            report
                .output
                .contains("[object .hotspot type=KeywordHit channel=choice]proxy[/object]")
        );
        assert!(
            report
                .output
                .contains("[object .KeywordHit type=KeywordHit]typed[/object]")
        );
        assert!(
            report
                .output
                .contains("[effect .sparkle amp=2px]effect[/effect]")
        );
        assert!(!report.output.contains("[effect .hotspot"));
        assert!(!report.output.contains("[effect .KeywordHit"));
    }

    #[test]
    fn canonical_rich_text_expands_nested_inferred_text_proxy_objects() {
        let source = "#[text_proxy(kind=\"keyword\", default_hit=true)]\npub struct KeywordHit {\n    channel: String\n}\n\n#[text_proxy(kind=\"hover\", default_hit=false)]\npub struct HoverHit {\n    layer: String\n}\n\nflow @flow.opening opening {\n    alice: [.hotspot type=KeywordHit channel=inventory][.HoverHit tone=alert]multi[/][/][.sparkle amp=2px]effect[/][p]\n}\n";
        let report = format_source(
            source,
            FormatOptions {
                expand_sugar: false,
                canonical_rich_text: true,
            },
        )
        .expect("format report");

        assert!(report.output.contains(
            "[object .hotspot type=KeywordHit channel=inventory][object .HoverHit type=HoverHit tone=alert]multi[/object][/object]"
        ));
        assert!(
            report
                .output
                .contains("[effect .sparkle amp=2px]effect[/effect]")
        );
        assert!(!report.output.contains("[effect .hotspot"));
        assert!(!report.output.contains("[effect .HoverHit"));
        assert!(!report.output.contains("[/]"));
    }

    #[test]
    fn canonical_rich_text_removes_marker_like_inferred_close() {
        let source =
            "flow @flow.opening opening {\n    alice: [.keyword]word[/][.shake]there[/][p]\n}\n";
        let report = format_source(
            source,
            FormatOptions {
                expand_sugar: false,
                canonical_rich_text: true,
            },
        )
        .expect("format report");

        assert!(
            report
                .output
                .contains("[mark .keyword]word[effect .shake]there[/effect]")
        );
        assert!(!report.output.contains("[/]"));
    }

    #[test]
    fn source_code_actions_include_canonical_rich_text_edits() {
        let source = "flow @flow.opening opening {\n    alice: [.keyword][.vertical_rl]縦[/]\n}\n";
        let actions = source_code_actions(source);

        let action = actions
            .iter()
            .find(|action| action.id == "arcweft.canonicalRichText")
            .expect("canonical rich-text action");
        let edit = action.edit.as_ref().expect("canonical action has edit");

        assert_eq!(action.label, "Canonicalize inferred rich-text tags");
        assert_eq!(edit.start, 0);
        assert_eq!(edit.end, source.len());
        assert!(
            edit.replacement
                .contains("[layout .vertical_rl]縦[/layout]")
        );
        assert!(edit.replacement.contains("[mark .keyword]"));
        assert!(!edit.replacement.contains("[/]"));
    }

    #[test]
    fn source_code_actions_group_expand_sugar_rewrites() {
        let source =
            "flow @flow.opening opening {\n    alice: hi $(name)[.shake]there[/][page]\n}\n";
        let actions = source_code_actions(source);

        let action = actions
            .iter()
            .find(|action| action.id == "arcweft.expandSugar")
            .expect("expand action");
        let edit = action.edit.as_ref().expect("expand action has edit");

        assert_eq!(edit.start, 0);
        assert_eq!(edit.end, source.len());
        assert!(edit.replacement.contains("alice.say()["));
        assert!(edit.replacement.contains("#[name]"));
        assert!(edit.replacement.contains("[effect .shake]there[/effect]"));
        assert!(edit.replacement.contains("[p]"));
    }

    #[test]
    fn source_code_actions_include_decl_identity_rewrite_only_when_linted() {
        let source =
            "flow @flow.opening opening {\n}\n#[generated]\nflow @flow.generated generated {\n}\n";
        let actions = source_code_actions(source);
        let action = actions
            .iter()
            .find(|action| action.id == "arcweft.expandSugar")
            .expect("expand action");
        let edit = action.edit.as_ref().expect("expand action has edit");

        assert_eq!(edit.start, 0);
        assert_eq!(edit.end, source.len());
        assert!(edit.replacement.contains("flow opening {"));
        assert!(
            edit.replacement
                .contains("#[generated]\nflow @flow.generated generated {")
        );
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
        let source = "flow @flow.opening opening {\n    alice:\n        Hi[p]\n    alice.say()[\n        Again[p]\n    ]\n=== scope rain ===\n=== line 地の文 ===\n雨。[p]\n=== with ===\nwait(mark(.done))\n=== /with ===\n=== /line ===\n=== /scope ===\n}\n";
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
