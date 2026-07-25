use crate::diagnostics::DocumentAnalysis;
use crate::documents::DocumentSnapshot;
use crate::features::cascade::effective_dialogue_cascade_at;
use crate::profiles::LspProfile;
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::{
    check::analyze_types,
    effect_diagnostics::{EffectDiagnosticKind, EffectSeverity},
    effect_model::CallableId,
    effects::EffectSet,
};
use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        dialogue::{ContentCall, SpeakerLine},
        flow::{
            FlowItem, ForBlock, IfBlock, IfLetBlock, LoopBlock, ScopeBlock, SourceLocaleBlock,
            Stmt, WhileBlock, WhileLetBlock,
        },
        items::{EntityDeclItem, EntityDeclKind, FunctionItem, Item},
        pattern::Pattern,
    },
    expr::Expr,
    parser::parse_source,
};
use arcweft_render_text::{
    LineDisplaySpec, RichTextAssignOp, RichTextCascadeLayer, RichTextStyleContribution,
};
use arcweft_verify_lsp::code_actions_from_report_with_mapper;
use arcweft_verify_lsp::source_code_actions_with_mapper;
use arcweft_verify_lsp::workspace_edit_from_tooling_edit;
use lsp_types::{CodeAction, CodeActionKind, Position, Uri};

use arcweft_tooling::model::TextEdit;
use arcweft_tooling::model::ToolingError;

/// Computes code actions for one open Arcweft document.
pub fn actions(
    profile: &LspProfile,
    uri: &Uri,
    document: &DocumentSnapshot,
    analysis: &DocumentAnalysis,
    position: Position,
) -> Result<Vec<CodeAction>, ToolingError> {
    let Ok(offset) = document
        .line_index()
        .try_byte_offset_from_position(position)
    else {
        return Ok(Vec::new());
    };
    let mut actions = source_code_actions_with_mapper(
        uri,
        document.source_document(),
        document.line_index(),
        analysis.canonicalization_input(),
    )?;
    if let Some(report) = analysis.verification_report() {
        actions.extend(code_actions_from_report_with_mapper(
            uri,
            report,
            document.line_index(),
        ));
    }
    actions.extend(effect_contract_actions(profile, uri, document));
    actions.extend(dialogue_override_actions(profile, uri, document, offset));
    Ok(actions)
}

fn effect_contract_actions(
    profile: &LspProfile,
    uri: &Uri,
    document: &DocumentSnapshot,
) -> Vec<CodeAction> {
    let parsed = parse_source(document.text().to_owned());
    if !parsed.errors().is_empty() {
        return Vec::new();
    }
    let Ok(hir) = lower_to_hir(parsed.typed_tree()) else {
        return Vec::new();
    };
    let report = analyze_types(&hir, &profile.typecheck_env());
    report
        .effects
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity() == EffectSeverity::Error)
        .filter_map(|diagnostic| match diagnostic.kind() {
            EffectDiagnosticKind::UpperBoundExceeded {
                excess,
                upper_bound,
            } => {
                let effects = upper_bound.union(excess);
                effect_set_edit(
                    document.text(),
                    parsed.typed_tree().items(),
                    diagnostic.callable(),
                    &effects,
                )
                .and_then(|edit| {
                    quickfix_code_action(
                        uri,
                        document,
                        format!("Expand effect upper bound for `{}`", diagnostic.callable()),
                        &edit,
                    )
                })
            }
            EffectDiagnosticKind::ForbiddenEffect { .. }
            | EffectDiagnosticKind::PureCallableEffect { .. }
            | EffectDiagnosticKind::UnknownLocalCallable { .. }
            | EffectDiagnosticKind::DynamicSignatureRequired { .. }
            | EffectDiagnosticKind::CapabilityUnavailable { .. } => None,
        })
        .collect()
}

fn dialogue_override_actions(
    profile: &LspProfile,
    uri: &Uri,
    document: &DocumentSnapshot,
    offset: usize,
) -> Vec<CodeAction> {
    let Some(insertion) = dialogue_option_insertion_at(document.text(), offset) else {
        return Vec::new();
    };
    let Some(cascade) = effective_dialogue_cascade_at(profile, document, offset) else {
        return Vec::new();
    };

    let mut actions =
        line_override_actions(uri, document, &cascade.spec.style_contributions, insertion);
    actions.extend(character_override_actions(uri, document, &cascade.spec));
    actions.extend(speaker_preset_actions(uri, document, &cascade.spec));
    actions
}

fn line_override_actions(
    uri: &Uri,
    document: &DocumentSnapshot,
    contributions: &[RichTextStyleContribution],
    insertion: DialogueOptionInsertion,
) -> Vec<CodeAction> {
    contributions
        .iter()
        .filter(|contribution| extractable_line_override(contribution))
        .take(8)
        .filter_map(|contribution| {
            let option = format!("{}={}", contribution.path, contribution.value);
            let edit = insertion.edit_for_option(&option);
            extraction_code_action(
                uri,
                document,
                format!("Extract `{}` override to line options", contribution.path),
                &edit,
            )
        })
        .collect()
}

fn character_override_actions(
    uri: &Uri,
    document: &DocumentSnapshot,
    spec: &LineDisplaySpec,
) -> Vec<CodeAction> {
    spec.style_contributions
        .iter()
        .filter(|contribution| extractable_character_override(contribution))
        .take(8)
        .filter_map(|contribution| {
            let edit = character_dialogue_style_edit(
                document.text(),
                &spec.callee,
                &contribution.path,
                &contribution.value,
            )?;
            extraction_code_action(
                uri,
                document,
                format!(
                    "Extract `{}` override to character dialogue_style",
                    contribution.path
                ),
                &edit,
            )
        })
        .collect()
}

fn speaker_preset_actions(
    uri: &Uri,
    document: &DocumentSnapshot,
    spec: &LineDisplaySpec,
) -> Vec<CodeAction> {
    spec.style_contributions
        .iter()
        .filter(|contribution| extractable_speaker_preset_override(contribution))
        .take(8)
        .filter_map(|contribution| {
            let edit = speaker_preset_edit(
                document.text(),
                &spec.callee,
                &contribution.path,
                &contribution.value,
            )?;
            extraction_code_action(
                uri,
                document,
                format!("Extract `{}` override to speaker preset", contribution.path),
                &edit,
            )
        })
        .collect()
}

fn extraction_code_action(
    uri: &Uri,
    document: &DocumentSnapshot,
    title: String,
    edit: &TextEdit,
) -> Option<CodeAction> {
    Some(CodeAction {
        title,
        kind: Some(CodeActionKind::REFACTOR_EXTRACT),
        edit: Some(
            workspace_edit_from_tooling_edit(
                uri,
                edit,
                document.source_document(),
                document.line_index(),
            )
            .ok()?,
        ),
        ..CodeAction::default()
    })
}

fn quickfix_code_action(
    uri: &Uri,
    document: &DocumentSnapshot,
    title: String,
    edit: &TextEdit,
) -> Option<CodeAction> {
    Some(CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        edit: Some(
            workspace_edit_from_tooling_edit(
                uri,
                edit,
                document.source_document(),
                document.line_index(),
            )
            .ok()?,
        ),
        ..CodeAction::default()
    })
}

fn effect_set_edit(
    source: &str,
    items: &[Item],
    callable: &CallableId,
    effects: &EffectSet,
) -> Option<TextEdit> {
    let item_range = callable_item_range(items, callable)?;
    let replacement = format_effects_clause(effects);
    if let Some(range) = find_effects_clause(source, item_range) {
        return Some(TextEdit {
            start: range.start(),
            end: range.end(),
            replacement,
        });
    }
    let body_open = find_body_open(source, item_range)?;
    Some(TextEdit {
        start: body_open,
        end: body_open,
        replacement: format!("\n{replacement}\n"),
    })
}

fn callable_item_range(items: &[Item], callable: &CallableId) -> Option<TextRange> {
    items.iter().find_map(|item| match item {
        Item::Function(function) if callable.as_str() == function_callable_name(function) => {
            Some(*function.range())
        }
        Item::Flow(flow)
            if flow
                .name()
                .is_some_and(|name| callable.as_str() == flow_callable_name(name)) =>
        {
            Some(*flow.range())
        }
        Item::Flow(_)
        | Item::Function(_)
        | Item::Trait(_)
        | Item::Impl(_)
        | Item::Enum(_)
        | Item::Struct(_)
        | Item::TypeAlias(_)
        | Item::EntityDecl(_)
        | Item::Entry(_)
        | Item::ExternCapability(_)
        | Item::Proof(_)
        | Item::Test(_)
        | Item::Bench(_)
        | Item::Source(_)
        | Item::Style(_)
        | Item::Raw(_) => None,
    })
}

fn function_callable_name(function: &FunctionItem) -> String {
    format!("fn.{}", function.signature().name())
}

fn flow_callable_name(name: &str) -> String {
    format!("flow.{name}")
}

fn format_effects_clause(effects: &EffectSet) -> String {
    let labels = effects.to_labels();
    if labels.is_empty() {
        "effects { }".to_owned()
    } else {
        format!("effects {{ {} }}", labels.join(", "))
    }
}

fn find_effects_clause(source: &str, range: TextRange) -> Option<TextRange> {
    let mut cursor = range.start();
    while cursor < range.end() {
        let offset = source[cursor..range.end()].find("effects")?;
        let start = cursor + offset;
        let after_keyword = start + "effects".len();
        if is_identifier_boundary(source, start, after_keyword) {
            let open = skip_ascii_whitespace(source, after_keyword, range.end());
            if source.as_bytes().get(open) == Some(&b'{') {
                let end = matching_brace_end(source, open, range.end())?;
                return Some(TextRange::new(start, end));
            }
        }
        cursor = after_keyword;
    }
    None
}

fn find_body_open(source: &str, range: TextRange) -> Option<usize> {
    source[range.start()..range.end()]
        .find('{')
        .map(|offset| range.start() + offset)
}

fn skip_ascii_whitespace(source: &str, mut offset: usize, end: usize) -> usize {
    while offset < end && source.as_bytes()[offset].is_ascii_whitespace() {
        offset += 1;
    }
    offset
}

fn matching_brace_end(source: &str, open: usize, end: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in source[open..end].char_indices() {
        match ch {
            '{' => depth = depth.saturating_add(1),
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset + ch.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

fn is_identifier_boundary(source: &str, start: usize, end: usize) -> bool {
    !source[..start]
        .chars()
        .next_back()
        .is_some_and(is_identifier_continue)
        && !source[end..]
            .chars()
            .next()
            .is_some_and(is_identifier_continue)
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn extractable_line_override(contribution: &RichTextStyleContribution) -> bool {
    contribution.active
        && contribution.op == RichTextAssignOp::Replace
        && contribution.layer != RichTextCascadeLayer::LineOptions
        && !contribution.path.is_empty()
        && !contribution.value.is_empty()
}

fn extractable_character_override(contribution: &RichTextStyleContribution) -> bool {
    contribution.active
        && contribution.op == RichTextAssignOp::Replace
        && !matches!(
            contribution.layer,
            RichTextCascadeLayer::LineOptions | RichTextCascadeLayer::CharacterDialogueStyle
        )
        && !contribution.path.is_empty()
        && !contribution.value.is_empty()
}

fn extractable_speaker_preset_override(contribution: &RichTextStyleContribution) -> bool {
    contribution.active
        && contribution.op == RichTextAssignOp::Replace
        && contribution.layer != RichTextCascadeLayer::SpeakerPreset
        && !contribution.path.is_empty()
        && !contribution.value.is_empty()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DialogueOptionInsertion {
    offset: usize,
    has_options: bool,
    options_have_values: bool,
}

impl DialogueOptionInsertion {
    fn edit_for_option(self, option: &str) -> TextEdit {
        let replacement = if self.has_options {
            if self.options_have_values {
                format!(", {option}")
            } else {
                option.to_owned()
            }
        } else {
            format!("({option})")
        };
        TextEdit {
            start: self.offset,
            end: self.offset,
            replacement,
        }
    }
}

fn dialogue_option_insertion_at(source: &str, offset: usize) -> Option<DialogueOptionInsertion> {
    let parsed = parse_source(source);
    if !parsed.errors().is_empty() {
        return None;
    }
    parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| item_dialogue_option_insertion(source, item, offset))
}

fn item_dialogue_option_insertion(
    source: &str,
    item: &Item,
    offset: usize,
) -> Option<DialogueOptionInsertion> {
    match item {
        Item::Flow(flow) => flow_item_dialogue_option_insertion(source, flow.body(), offset),
        _ => None,
    }
}

fn flow_item_dialogue_option_insertion(
    source: &str,
    items: &[FlowItem],
    offset: usize,
) -> Option<DialogueOptionInsertion> {
    items.iter().find_map(|item| match item {
        FlowItem::SpeakerLine(line) if line.content().content_offset(offset).is_some() => {
            speaker_line_option_insertion(source, line)
        }
        FlowItem::ContentCall(call) if call.content().content_offset(offset).is_some() => {
            content_call_option_insertion(source, call)
        }
        FlowItem::If(block) => nested_body_insertion(source, block, offset),
        FlowItem::IfLet(block) => nested_body_insertion(source, block, offset),
        FlowItem::Match(block) => block
            .arms()
            .iter()
            .find_map(|arm| flow_item_dialogue_option_insertion(source, arm.body(), offset)),
        FlowItem::Loop(block) => nested_body_insertion(source, block, offset),
        FlowItem::While(block) => nested_body_insertion(source, block, offset),
        FlowItem::WhileLet(block) => nested_body_insertion(source, block, offset),
        FlowItem::For(block) => nested_body_insertion(source, block, offset),
        FlowItem::Select(block) => block
            .branches()
            .iter()
            .find_map(|branch| flow_item_dialogue_option_insertion(source, branch.body(), offset)),
        FlowItem::SourceLocale(block) => nested_body_insertion(source, block, offset),
        FlowItem::Scope(block) => nested_body_insertion(source, block, offset),
        FlowItem::AwaitWith(await_with) => await_with
            .branches()
            .iter()
            .find_map(|branch| flow_item_dialogue_option_insertion(source, branch.body(), offset)),
        FlowItem::SpeakerLine(_)
        | FlowItem::ContentCall(_)
        | FlowItem::Choice(_)
        | FlowItem::Stmt(_)
        | FlowItem::Include(_)
        | FlowItem::Raw(_) => None,
    })
}

trait HasFlowBody {
    fn body(&self) -> &[FlowItem];
}

impl HasFlowBody for IfBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for IfLetBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for LoopBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for WhileBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for WhileLetBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for ForBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for SourceLocaleBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

impl HasFlowBody for ScopeBlock {
    fn body(&self) -> &[FlowItem] {
        self.body()
    }
}

fn nested_body_insertion(
    source: &str,
    block: &impl HasFlowBody,
    offset: usize,
) -> Option<DialogueOptionInsertion> {
    flow_item_dialogue_option_insertion(source, block.body(), offset)
}

fn speaker_line_option_insertion(
    source: &str,
    line: &SpeakerLine,
) -> Option<DialogueOptionInsertion> {
    let header_end = line.content().range().start();
    let colon = source.get(line.range().start()..header_end)?.rfind(':')?;
    call_header_option_insertion(source, line.range().start(), line.range().start() + colon)
}

fn content_call_option_insertion(
    source: &str,
    call: &ContentCall,
) -> Option<DialogueOptionInsertion> {
    let header_end = call.content().range().start();
    let bracket = source.get(call.range().start()..header_end)?.rfind('[')?;
    call_header_option_insertion(source, call.range().start(), call.range().start() + bracket)
}

fn call_header_option_insertion(
    source: &str,
    header_start: usize,
    header_end: usize,
) -> Option<DialogueOptionInsertion> {
    let header = source.get(header_start..header_end)?;
    let trimmed_end = header.trim_end().len();
    if trimmed_end == 0 {
        return None;
    }
    let insert = header_start + trimmed_end;
    if header[..trimmed_end].ends_with(')') {
        let close = trimmed_end - ')'.len_utf8();
        let open = matching_open_paren(&header[..trimmed_end], close)?;
        let options = &header[open + '('.len_utf8()..close];
        Some(DialogueOptionInsertion {
            offset: header_start + close,
            has_options: true,
            options_have_values: !options.trim().is_empty(),
        })
    } else {
        Some(DialogueOptionInsertion {
            offset: insert,
            has_options: false,
            options_have_values: false,
        })
    }
}

fn matching_open_paren(source: &str, close: usize) -> Option<usize> {
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source[..=close].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '(' if !in_string => stack.push(offset),
            ')' if !in_string => {
                let open = stack.pop()?;
                if offset == close {
                    return Some(open);
                }
            }
            _ => {}
        }
    }
    None
}

fn character_dialogue_style_edit(
    source: &str,
    callee: &str,
    path: &str,
    value: &str,
) -> Option<TextEdit> {
    let parsed = parse_source(source);
    if !parsed.errors().is_empty() {
        return None;
    }
    parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(entity)
                if entity.kind() == EntityDeclKind::Character
                    && character_matches_callee(entity, callee) =>
            {
                entity_dialogue_style_edit(source, entity, path, value)
            }
            _ => None,
        })
}

fn character_matches_callee(entity: &EntityDeclItem, callee: &str) -> bool {
    let key = callee.strip_suffix(".say").unwrap_or(callee);
    [
        Some(entity.id().body()),
        entity.name(),
        entity.surface_alias(),
        entity.id().body().rsplit('.').next(),
    ]
    .into_iter()
    .flatten()
    .any(|candidate| candidate == key || format!("@<{candidate}>") == key)
}

fn entity_dialogue_style_edit(
    source: &str,
    entity: &EntityDeclItem,
    path: &str,
    value: &str,
) -> Option<TextEdit> {
    let body = entity.body()?;
    let body_range = entity.body_range()?;
    if let Some(insertion) =
        existing_dialogue_style_insertion(source, body, body_range, path, value)
    {
        return Some(insertion);
    }
    let replacement = if let Some(parts) = nested_path_parts(path) {
        format!(
            "\n    dialogue_style {{\n{}    }}",
            nested_assignment_text("    ", &parts, value)
        )
    } else {
        format!("\n    dialogue_style {{\n        {path} = {value}\n    }}")
    };
    Some(TextEdit {
        start: body_range.end(),
        end: body_range.end(),
        replacement,
    })
}

fn existing_dialogue_style_insertion(
    source: &str,
    body: &str,
    body_range: &TextRange,
    path: &str,
    value: &str,
) -> Option<TextEdit> {
    existing_named_block_insertion(source, body, body_range, "dialogue_style", path, value)
}

fn existing_named_block_insertion(
    source: &str,
    body: &str,
    body_range: &TextRange,
    block_name: &str,
    path: &str,
    value: &str,
) -> Option<TextEdit> {
    let start = body.find(block_name)?;
    let open = body[start..].find('{')? + start;
    let close = matching_brace(body, open)?;
    let block_range = TextRange::new(body_range.start() + start, body_range.start() + close + 1);
    block_path_assignment_insertion(source, &block_range, path, value)
}

fn matching_brace(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source[open..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn speaker_preset_edit(source: &str, callee: &str, path: &str, value: &str) -> Option<TextEdit> {
    let preset_name = callee.strip_suffix(".say").unwrap_or(callee).trim();
    if preset_name.is_empty() {
        return None;
    }
    let parsed = parse_source(source);
    if !parsed.errors().is_empty() {
        return None;
    }
    let option = format!("{path}={value}");
    parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| speaker_preset_edit_from_item(source, item, preset_name, &option))
}

fn speaker_preset_edit_from_item(
    source: &str,
    item: &Item,
    preset_name: &str,
    option: &str,
) -> Option<TextEdit> {
    match item {
        Item::Flow(flow) => {
            speaker_preset_edit_from_flow_items(source, flow.body(), preset_name, option)
        }
        _ => None,
    }
}

fn speaker_preset_edit_from_flow_items(
    source: &str,
    items: &[FlowItem],
    preset_name: &str,
    option: &str,
) -> Option<TextEdit> {
    items.iter().find_map(|item| match item {
        FlowItem::Stmt(stmt) => speaker_preset_edit_from_stmt(source, stmt, preset_name, option),
        FlowItem::If(block) => nested_speaker_preset_edit(source, block, preset_name, option),
        FlowItem::IfLet(block) => nested_speaker_preset_edit(source, block, preset_name, option),
        FlowItem::Match(block) => block.arms().iter().find_map(|arm| {
            speaker_preset_edit_from_flow_items(source, arm.body(), preset_name, option)
        }),
        FlowItem::Loop(block) => nested_speaker_preset_edit(source, block, preset_name, option),
        FlowItem::While(block) => nested_speaker_preset_edit(source, block, preset_name, option),
        FlowItem::WhileLet(block) => nested_speaker_preset_edit(source, block, preset_name, option),
        FlowItem::For(block) => nested_speaker_preset_edit(source, block, preset_name, option),
        FlowItem::Select(block) => block.branches().iter().find_map(|branch| {
            speaker_preset_edit_from_flow_items(source, branch.body(), preset_name, option)
        }),
        FlowItem::SourceLocale(block) => {
            nested_speaker_preset_edit(source, block, preset_name, option)
        }
        FlowItem::Scope(block) => nested_speaker_preset_edit(source, block, preset_name, option),
        FlowItem::AwaitWith(await_with) => await_with.branches().iter().find_map(|branch| {
            speaker_preset_edit_from_flow_items(source, branch.body(), preset_name, option)
        }),
        FlowItem::SpeakerLine(_)
        | FlowItem::ContentCall(_)
        | FlowItem::Choice(_)
        | FlowItem::Include(_)
        | FlowItem::Raw(_) => None,
    })
}

fn nested_speaker_preset_edit(
    source: &str,
    block: &impl HasFlowBody,
    preset_name: &str,
    option: &str,
) -> Option<TextEdit> {
    speaker_preset_edit_from_flow_items(source, block.body(), preset_name, option)
}

fn speaker_preset_edit_from_stmt(
    source: &str,
    stmt: &Stmt,
    preset_name: &str,
    option: &str,
) -> Option<TextEdit> {
    match stmt {
        Stmt::Let {
            pattern,
            expr,
            expr_source,
            expr_range,
            ..
        } if pattern_ident(pattern).is_some_and(|name| name == preset_name) => {
            speaker_preset_expr_edit(
                source,
                expr,
                expr_source.as_deref(),
                expr_range.as_ref(),
                option,
            )
        }
        Stmt::LetElse { else_body, .. } => {
            speaker_preset_edit_from_stmts(source, else_body, preset_name, option)
        }
        Stmt::LetScope { scope, .. } => {
            speaker_preset_edit_from_stmts(source, scope.statements(), preset_name, option)
        }
        Stmt::LetLoop { block, .. } => {
            speaker_preset_edit_from_flow_items(source, block.body(), preset_name, option)
        }
        Stmt::LetAwait { await_with, .. } => await_with.branches().iter().find_map(|branch| {
            speaker_preset_edit_from_flow_items(source, branch.body(), preset_name, option)
        }),
        Stmt::Thread(thread) => {
            speaker_preset_edit_from_flow_items(source, thread.body(), preset_name, option)
        }
        Stmt::DeferBlock { statements, .. }
        | Stmt::On {
            body: statements, ..
        }
        | Stmt::UnsafeLifetime {
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
        } => speaker_preset_edit_from_stmts(source, statements, preset_name, option),
        Stmt::If {
            body, else_body, ..
        } => speaker_preset_edit_from_stmts(source, body, preset_name, option)
            .or_else(|| speaker_preset_edit_from_stmts(source, else_body, preset_name, option)),
        Stmt::Match { arms, .. } => arms.iter().find_map(|arm| {
            speaker_preset_edit_from_stmts(source, arm.body(), preset_name, option)
        }),
        Stmt::Assertion(_)
        | Stmt::Let { .. }
        | Stmt::Assign { .. }
        | Stmt::LetChoice { .. }
        | Stmt::LetActionReceive { .. }
        | Stmt::Return { expr: _, .. }
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
        | Stmt::Expr { expr: _, .. }
        | Stmt::Raw(_) => None,
    }
}

fn speaker_preset_edit_from_stmts(
    source: &str,
    statements: &[Stmt],
    preset_name: &str,
    option: &str,
) -> Option<TextEdit> {
    statements
        .iter()
        .find_map(|stmt| speaker_preset_edit_from_stmt(source, stmt, preset_name, option))
}

fn speaker_preset_expr_edit(
    source: &str,
    expr: &Expr,
    expr_source: Option<&str>,
    expr_range: Option<&TextRange>,
    option: &str,
) -> Option<TextEdit> {
    if !matches!(expr, Expr::Call(_)) {
        return None;
    }
    let expr_range = expr_range?;
    let expr_source = expr_source?;
    let insertion = call_header_option_insertion(
        source,
        expr_range.start(),
        expr_range.start() + expr_source.len(),
    )?;
    Some(insertion.edit_for_option(option))
}

fn pattern_ident(pattern: &Pattern) -> Option<&str> {
    match pattern {
        Pattern::Ident(name)
        | Pattern::MutIdent(name)
        | Pattern::Typed { name, .. }
        | Pattern::Whole { name, .. } => Some(name),
        Pattern::Literal(_)
        | Pattern::Entity(_)
        | Pattern::Variant { .. }
        | Pattern::Discard
        | Pattern::Tuple(_)
        | Pattern::Record { .. }
        | Pattern::BracketSeq { .. }
        | Pattern::Raw(_) => None,
    }
}

fn block_path_assignment_insertion(
    source: &str,
    range: &TextRange,
    path: &str,
    value: &str,
) -> Option<TextEdit> {
    let Some(parts) = nested_path_parts(path) else {
        return block_assignment_insertion(source, range, path, value);
    };

    let mut block_range = *range;
    let mut consumed = 0usize;
    for part in &parts[..parts.len() - 1] {
        let Some(child) = find_direct_child_block(source, &block_range, part) else {
            break;
        };
        block_range = child;
        consumed += 1;
    }

    if consumed == parts.len() - 1 {
        block_assignment_insertion(source, &block_range, parts[parts.len() - 1], value)
    } else {
        block_nested_assignment_insertion(source, &block_range, &parts[consumed..], value)
    }
}

fn nested_path_parts(path: &str) -> Option<Vec<&str>> {
    let parts = assignment_path_parts(path);
    (parts.len() > 1).then_some(parts)
}

fn assignment_path_parts(path: &str) -> Vec<&str> {
    path.split('.')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

fn block_assignment_insertion(
    source: &str,
    range: &TextRange,
    path: &str,
    value: &str,
) -> Option<TextEdit> {
    let block = source.get(range.as_range())?;
    let open = block.find('{')?;
    let close = matching_brace(block, open)?;
    let line_start = block[..close]
        .rfind('\n')
        .map_or(open + 1, |offset| offset + 1);
    let close_indent = &block[line_start..close];
    Some(TextEdit {
        start: range.start() + line_start,
        end: range.start() + line_start,
        replacement: format!("{close_indent}    {path} = {value}\n"),
    })
}

fn block_nested_assignment_insertion(
    source: &str,
    range: &TextRange,
    parts: &[&str],
    value: &str,
) -> Option<TextEdit> {
    let (_, close) = block_braces(source, range)?;
    let line_start = source[..close].rfind('\n').map_or(0, |offset| offset + 1);
    let close_indent = &source[line_start..close];
    Some(TextEdit {
        start: line_start,
        end: line_start,
        replacement: nested_assignment_text(close_indent, parts, value),
    })
}

fn nested_assignment_text(close_indent: &str, parts: &[&str], value: &str) -> String {
    let mut output = String::new();
    let base_indent = format!("{close_indent}    ");
    for (depth, part) in parts.iter().take(parts.len().saturating_sub(1)).enumerate() {
        output.push_str(&base_indent);
        output.push_str(&"    ".repeat(depth));
        output.push_str(part);
        output.push_str(" {\n");
    }
    output.push_str(&base_indent);
    output.push_str(&"    ".repeat(parts.len().saturating_sub(1)));
    output.push_str(parts.last().copied().unwrap_or_default());
    output.push_str(" = ");
    output.push_str(value);
    output.push('\n');
    for (depth, _) in parts
        .iter()
        .take(parts.len().saturating_sub(1))
        .enumerate()
        .rev()
    {
        output.push_str(&base_indent);
        output.push_str(&"    ".repeat(depth));
        output.push_str("}\n");
    }
    output
}

fn find_direct_child_block(source: &str, range: &TextRange, name: &str) -> Option<TextRange> {
    let (open, close) = block_braces(source, range)?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (relative, ch) in source[open + '{'.len_utf8()..close].char_indices() {
        let offset = open + '{'.len_utf8() + relative;
        if escaped {
            escaped = false;
            continue;
        }
        if depth == 0 && starts_block_head(source, offset, name) {
            let open =
                source[offset + name.len()..]
                    .char_indices()
                    .find_map(|(relative, ch)| {
                        if ch == '{' {
                            Some(offset + name.len() + relative)
                        } else if ch.is_whitespace() {
                            None
                        } else {
                            Some(usize::MAX)
                        }
                    })?;
            if open == usize::MAX {
                continue;
            }
            let close = matching_brace(source, open)?;
            return Some(TextRange::new(offset, close + '}'.len_utf8()));
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn starts_block_head(source: &str, offset: usize, name: &str) -> bool {
    source[offset..].starts_with(name)
        && source[..offset]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_block_head_symbol(ch))
        && source[offset + name.len()..]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_whitespace() || ch == '{')
}

fn is_block_head_symbol(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | ':')
}

fn block_braces(source: &str, range: &TextRange) -> Option<(usize, usize)> {
    let block = source.get(range.as_range())?;
    let open = range.start() + block.find('{')?;
    let close = matching_brace(source, open)?;
    Some((open, close))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::DocumentStore;
    use crate::positions::PositionEncoding;
    use arcweft_runtime_host::RuntimeHostRunnerKind;
    use lsp_types::{
        DidChangeTextDocumentParams, TextDocumentContentChangeEvent,
        VersionedTextDocumentIdentifier,
    };

    #[test]
    fn dialogue_option_insertion_adds_new_speaker_options() {
        let source = "flow opening {\n    alice: Hello[p]\n}\n";
        let offset = source.find("Hello").expect("content");
        let insertion = dialogue_option_insertion_at(source, offset).expect("dialogue insertion");
        let edit = insertion.edit_for_option("text_color=rgb(\"#202122\")");

        assert_eq!(
            edit,
            TextEdit {
                start: source.find(": Hello").expect("colon"),
                end: source.find(": Hello").expect("colon"),
                replacement: "(text_color=rgb(\"#202122\"))".to_owned(),
            }
        );
    }

    #[test]
    fn dialogue_option_insertion_maps_indented_multiline_lf_and_crlf_content() {
        let source_lf = "flow opening {\n    alice:\n        Intro\n        Styled content[p]\n}\n";
        for source in [source_lf.to_owned(), source_lf.replace('\n', "\r\n")] {
            let offset = source.find("Styled").expect("later dialogue content");
            let insertion =
                dialogue_option_insertion_at(&source, offset).expect("dialogue insertion");
            let edit = insertion.edit_for_option("rich_text.ruby.size=14px");

            assert_eq!(edit.start, source.find(':').expect("speaker colon"));
            assert_eq!(edit.end, edit.start);
            assert_eq!(edit.replacement, "(rich_text.ruby.size=14px)");

            let line_start = source
                .find("        Styled")
                .expect("indented content line");
            assert!(
                dialogue_option_insertion_at(&source, line_start + 1).is_none(),
                "removed indentation must not alias a normalized content byte"
            );
        }
    }

    #[test]
    fn dialogue_option_insertion_appends_existing_speaker_options() {
        let source = "flow opening {\n    alice(voice=auto, note=\")\"): Hello[p]\n}\n";
        let offset = source.find("Hello").expect("content");
        let insertion = dialogue_option_insertion_at(source, offset).expect("dialogue insertion");
        let edit = insertion.edit_for_option("rich_text.ruby.size=14px");

        assert_eq!(
            edit,
            TextEdit {
                start: source.find("): Hello").expect("close paren"),
                end: source.find("): Hello").expect("close paren"),
                replacement: ", rich_text.ruby.size=14px".to_owned(),
            }
        );
    }

    #[test]
    fn character_dialogue_style_edit_creates_missing_block() {
        let source = "pub character alice {}\nflow opening {\n    alice: Hello[p]\n}\n";
        let edit = character_dialogue_style_edit(source, "alice", "rich_text.ruby.size", "14px")
            .expect("character style edit");

        assert_eq!(edit.start, source.find("{}").expect("empty body") + 1);
        assert_eq!(
            edit.replacement,
            "\n    dialogue_style {\n        rich_text {\n            ruby {\n                size = 14px\n            }\n        }\n    }"
        );
    }

    #[test]
    fn character_dialogue_style_edit_appends_nested_block_to_existing_style() {
        let source = "pub character alice {\n    dialogue_style {\n        text_color = rgb(\"#202122\")\n    }\n}\n";
        let edit = character_dialogue_style_edit(source, "alice", "rich_text.ruby.size", "14px")
            .expect("character style edit");

        assert_eq!(edit.start, source.find("    }\n}").expect("style close"));
        assert_eq!(
            edit.replacement,
            "        rich_text {\n            ruby {\n                size = 14px\n            }\n        }\n"
        );
    }

    #[test]
    fn character_dialogue_style_edit_appends_existing_nested_leaf_block() {
        let source = "pub character alice {\n    dialogue_style {\n        rich_text {\n            ruby {\n                gap = 1px\n            }\n        }\n    }\n}\n";
        let edit = character_dialogue_style_edit(source, "alice", "rich_text.ruby.size", "14px")
            .expect("character style edit");

        assert_eq!(
            edit.start,
            source
                .find("            }\n        }\n    }\n}\n")
                .expect("ruby close")
        );
        assert_eq!(edit.replacement, "                size = 14px\n");
    }

    #[test]
    fn speaker_preset_edit_appends_existing_call_options() {
        let source =
            "flow opening {\n    let alice_side = alice(voice=auto)\n    alice_side: Hello[p]\n}\n";
        let edit = speaker_preset_edit(source, "alice_side", "rich_text.ruby.size", "14px")
            .expect("speaker preset edit");

        assert_eq!(
            edit.start,
            source.find(")\n    alice_side").expect("call close")
        );
        assert_eq!(edit.replacement, ", rich_text.ruby.size=14px");
    }

    #[test]
    fn speaker_preset_edit_creates_call_options() {
        let source = "flow opening {\n    let alice_side = alice()\n    alice_side: Hello[p]\n}\n";
        let edit = speaker_preset_edit(source, "alice_side", "text_color", "rgb(\"#202122\")")
            .expect("speaker preset edit");

        assert_eq!(
            edit.start,
            source.find(")\n    alice_side").expect("call close")
        );
        assert_eq!(edit.replacement, "text_color=rgb(\"#202122\")");
    }

    #[test]
    fn verifier_report_actions_are_included_in_code_actions() {
        let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
        let source = "flow @flow.opening opening {\n    let summary = promote('flow)\n}\n";
        let mut store = DocumentStore::default();
        let document = store
            .change(
                DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: uri.clone(),
                        version: 1,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: source.to_owned(),
                    }],
                },
                PositionEncoding::Utf16,
            )
            .expect("document opens");
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = DocumentAnalysis::analyze(
            document.text(),
            document.line_index().position_encoding(),
            &profile,
        );

        assert!(analysis.verification_report().is_some());
        let code_actions = actions(&profile, &uri, &document, &analysis, Position::new(1, 4))
            .expect("code actions");

        assert!(code_actions.iter().any(|action| {
            action.command.as_ref().is_some_and(|command| {
                command.command == "arcweft.verify.generateProofStub"
                    || command.command == "arcweft.verify.showObligation"
            })
        }));
    }
}
