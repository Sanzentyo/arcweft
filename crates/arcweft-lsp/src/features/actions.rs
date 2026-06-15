use crate::diagnostics::DocumentAnalysis;
use crate::documents::DocumentSnapshot;
use crate::features::cascade::effective_dialogue_cascade_at;
use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        dialogue::{ContentCall, SpeakerLine},
        flow::{
            BorrowBlock, FlowItem, ForBlock, IfBlock, IfLetBlock, LoopBlock, ScopeBlock,
            SourceLocaleBlock, WhileBlock, WhileLetBlock,
        },
        items::{EntityDeclItem, EntityDeclKind, Item},
    },
    parser::parse_source,
};
use arcweft_render_text::{RichTextAssignOp, RichTextCascadeLayer, RichTextStyleContribution};
use arcweft_verify_lsp::source_code_actions_with_mapper;
use arcweft_verify_lsp::workspace_edit_from_tooling_edit;
use lsp_types::{CodeAction, CodeActionKind, Position, Uri};

use arcweft_tooling::TextEdit;

/// Computes code actions for one open Arcweft document.
pub fn actions(
    uri: &Uri,
    document: &DocumentSnapshot,
    _analysis: &DocumentAnalysis,
    position: Position,
) -> Vec<CodeAction> {
    let mut actions = source_code_actions_with_mapper(uri, document.text(), document.line_index());
    actions.extend(dialogue_override_actions(uri, document, position));
    actions
}

fn dialogue_override_actions(
    uri: &Uri,
    document: &DocumentSnapshot,
    position: Position,
) -> Vec<CodeAction> {
    let offset = document.line_index().byte_offset_from_position(position);
    let Some(insertion) = dialogue_option_insertion_at(document.text(), offset) else {
        return Vec::new();
    };
    let Some(cascade) = effective_dialogue_cascade_at(document, offset) else {
        return Vec::new();
    };

    let mut actions = cascade
        .spec
        .style_contributions
        .iter()
        .filter(|contribution| extractable_line_override(contribution))
        .take(8)
        .map(|contribution| {
            let option = format!("{}={}", contribution.path, contribution.value);
            let edit = insertion.edit_for_option(&option);
            extraction_code_action(
                uri,
                document,
                format!("Extract `{}` override to line options", contribution.path),
                &edit,
            )
        })
        .collect::<Vec<_>>();

    actions.extend(
        cascade
            .spec
            .style_contributions
            .iter()
            .filter(|contribution| extractable_character_override(contribution))
            .take(8)
            .filter_map(|contribution| {
                let edit = character_dialogue_style_edit(
                    document.text(),
                    &cascade.spec.callee,
                    &contribution.path,
                    &contribution.value,
                )?;
                Some(extraction_code_action(
                    uri,
                    document,
                    format!(
                        "Extract `{}` override to character dialogue_style",
                        contribution.path
                    ),
                    &edit,
                ))
            }),
    );
    actions.extend(
        cascade
            .spec
            .style_contributions
            .iter()
            .filter(|contribution| extractable_textbox_theme_override(contribution))
            .take(8)
            .filter_map(|contribution| {
                let edit = textbox_theme_edit(
                    document.text(),
                    cascade.spec.window.as_deref()?,
                    &contribution.path,
                    &contribution.value,
                )?;
                Some(extraction_code_action(
                    uri,
                    document,
                    format!("Extract `{}` override to textbox theme", contribution.path),
                    &edit,
                ))
            }),
    );
    actions.extend(
        cascade
            .spec
            .style_contributions
            .iter()
            .filter(|contribution| extractable_dialogue_defaults_override(contribution))
            .take(8)
            .filter_map(|contribution| {
                let edit = dialogue_defaults_edit(
                    document.text(),
                    &contribution.path,
                    &contribution.value,
                )?;
                Some(extraction_code_action(
                    uri,
                    document,
                    format!(
                        "Extract `{}` override to dialogue defaults",
                        contribution.path
                    ),
                    &edit,
                ))
            }),
    );
    actions
}

fn extraction_code_action(
    uri: &Uri,
    document: &DocumentSnapshot,
    title: String,
    edit: &TextEdit,
) -> CodeAction {
    CodeAction {
        title,
        kind: Some(CodeActionKind::REFACTOR_EXTRACT),
        edit: Some(workspace_edit_from_tooling_edit(
            uri,
            edit,
            document.line_index(),
        )),
        ..CodeAction::default()
    }
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

fn extractable_dialogue_defaults_override(contribution: &RichTextStyleContribution) -> bool {
    contribution.active
        && contribution.op == RichTextAssignOp::Replace
        && contribution.layer != RichTextCascadeLayer::DialogueDefaults
        && !contribution.path.is_empty()
        && !contribution.value.is_empty()
}

fn extractable_textbox_theme_override(contribution: &RichTextStyleContribution) -> bool {
    contribution.active
        && contribution.op == RichTextAssignOp::Replace
        && contribution.layer != RichTextCascadeLayer::DialogueWindowTheme
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
        Item::FlowItem(item) => {
            flow_item_dialogue_option_insertion(source, std::slice::from_ref(item.as_ref()), offset)
        }
        _ => None,
    }
}

fn flow_item_dialogue_option_insertion(
    source: &str,
    items: &[FlowItem],
    offset: usize,
) -> Option<DialogueOptionInsertion> {
    items.iter().find_map(|item| match item {
        FlowItem::SpeakerLine(line) if range_contains(line.content().range(), offset) => {
            speaker_line_option_insertion(source, line)
        }
        FlowItem::ContentCall(call) if range_contains(call.content().range(), offset) => {
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
        FlowItem::BorrowBlock(block) => nested_body_insertion(source, block, offset),
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

impl HasFlowBody for BorrowBlock {
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

fn range_contains(range: &TextRange, offset: usize) -> bool {
    range.start() <= offset && offset <= range.end()
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

fn dialogue_defaults_edit(source: &str, path: &str, value: &str) -> Option<TextEdit> {
    let parsed = parse_source(source);
    if !parsed.errors().is_empty() {
        return None;
    }
    let defaults = parsed
        .typed_tree()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::DialogueDefaults(defaults) => Some(defaults),
            _ => None,
        })
        .collect::<Vec<_>>();
    let target = defaults
        .iter()
        .copied()
        .find(|defaults| {
            defaults
                .id()
                .is_some_and(|id| id.body() == "dialogue.defaults")
        })
        .or_else(|| (defaults.len() == 1).then(|| defaults[0]))?;
    block_path_assignment_insertion(source, target.range(), path, value)
}

fn textbox_theme_edit(source: &str, window: &str, path: &str, value: &str) -> Option<TextEdit> {
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
                if entity.kind() == EntityDeclKind::Textbox
                    && entity_matches_ref(entity, window) =>
            {
                entity_textbox_theme_edit(source, entity, path, value)
            }
            _ => None,
        })
}

fn entity_matches_ref(entity: &EntityDeclItem, raw_ref: &str) -> bool {
    let key = raw_ref
        .trim()
        .strip_prefix("@<")
        .and_then(|inner| inner.strip_suffix('>'))
        .or_else(|| raw_ref.trim().strip_prefix('@'))
        .unwrap_or(raw_ref)
        .trim();
    [
        Some(entity.id().body()),
        entity.name(),
        entity.surface_alias(),
        entity
            .id()
            .body()
            .rsplit_once('.')
            .map(|(_, suffix)| suffix),
    ]
    .into_iter()
    .flatten()
    .any(|candidate| candidate == key || format!("@<{candidate}>") == key)
}

fn entity_textbox_theme_edit(
    source: &str,
    entity: &EntityDeclItem,
    path: &str,
    value: &str,
) -> Option<TextEdit> {
    let body = entity.body()?;
    let body_range = entity.body_range()?;
    if let Some(rest) = path.strip_prefix("rich_text.") {
        if let Some(insertion) =
            existing_named_block_insertion(source, body, body_range, "rich_text", rest, value)
        {
            return Some(insertion);
        }
        let parts = assignment_path_parts(rest);
        if parts.is_empty() {
            return None;
        }
        return Some(TextEdit {
            start: body_range.end(),
            end: body_range.end(),
            replacement: format!(
                "\n    rich_text {{\n{}    }}",
                nested_assignment_text("    ", &parts, value)
            ),
        });
    }
    if let Some(insertion) =
        existing_named_block_insertion(source, body, body_range, "dialogue_style", path, value)
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
    fn textbox_theme_edit_creates_rich_text_block_for_window() {
        let source = "pub textbox @textbox.phone PhoneBox {}\nflow opening {\n    alice(window=@textbox.phone): Hello[p]\n}\n";
        let edit = textbox_theme_edit(source, "textbox.phone", "rich_text.ruby.size", "14px")
            .expect("textbox theme edit");

        assert_eq!(
            edit.start,
            source.find("{}").expect("empty textbox body") + 1
        );
        assert_eq!(
            edit.replacement,
            "\n    rich_text {\n        ruby {\n            size = 14px\n        }\n    }"
        );
    }

    #[test]
    fn textbox_theme_edit_appends_existing_rich_text_leaf_block() {
        let source = "pub textbox @textbox.phone PhoneBox {\n    rich_text {\n        ruby {\n            gap = 1px\n        }\n    }\n}\n";
        let edit = textbox_theme_edit(source, "@textbox.phone", "rich_text.ruby.size", "14px")
            .expect("textbox theme edit");

        assert_eq!(
            edit.start,
            source.find("        }\n    }\n}\n").expect("ruby close")
        );
        assert_eq!(edit.replacement, "            size = 14px\n");
    }

    #[test]
    fn textbox_theme_edit_uses_dialogue_style_for_non_rich_text_paths() {
        let source = "pub textbox @textbox.phone PhoneBox {}\n";
        let edit = textbox_theme_edit(source, "phone", "text_color", "rgb(\"#202122\")")
            .expect("textbox dialogue style edit");

        assert_eq!(
            edit.start,
            source.find("{}").expect("empty textbox body") + 1
        );
        assert_eq!(
            edit.replacement,
            "\n    dialogue_style {\n        text_color = rgb(\"#202122\")\n    }"
        );
    }

    #[test]
    fn dialogue_defaults_edit_uses_canonical_profile() {
        let source = "pub dialogue defaults @dialogue.defaults.debug {\n}\n\npub dialogue defaults @dialogue.defaults {\n}\n";
        let edit = dialogue_defaults_edit(source, "text_color", "rgb(\"#202122\")")
            .expect("defaults edit");
        let expected = source
            .rfind("}\n")
            .expect("canonical defaults closing brace");

        assert_eq!(edit.start, expected);
        assert_eq!(edit.replacement, "    text_color = rgb(\"#202122\")\n");
    }

    #[test]
    fn dialogue_defaults_edit_creates_nested_rich_text_blocks() {
        let source = "pub dialogue defaults @dialogue.defaults {\n}\n";
        let edit =
            dialogue_defaults_edit(source, "rich_text.ruby.size", "14px").expect("defaults edit");
        let expected = source.find("}\n").expect("defaults close");

        assert_eq!(edit.start, expected);
        assert_eq!(
            edit.replacement,
            "    rich_text {\n        ruby {\n            size = 14px\n        }\n    }\n"
        );
    }

    #[test]
    fn dialogue_defaults_edit_appends_missing_nested_child_block() {
        let source = "pub dialogue defaults @dialogue.defaults {\n    rich_text {\n        text {\n            color = rgb(\"#202122\")\n        }\n    }\n}\n";
        let edit =
            dialogue_defaults_edit(source, "rich_text.ruby.size", "14px").expect("defaults edit");
        let expected = source.find("    }\n}\n").expect("rich_text close");

        assert_eq!(edit.start, expected);
        assert_eq!(
            edit.replacement,
            "        ruby {\n            size = 14px\n        }\n"
        );
    }

    #[test]
    fn dialogue_defaults_edit_appends_existing_nested_leaf_block() {
        let source = "pub dialogue defaults @dialogue.defaults {\n    rich_text {\n        ruby {\n            gap = 1px\n        }\n    }\n}\n";
        let edit =
            dialogue_defaults_edit(source, "rich_text.ruby.size", "14px").expect("defaults edit");
        let expected = source.find("        }\n    }\n}\n").expect("ruby close");

        assert_eq!(edit.start, expected);
        assert_eq!(edit.replacement, "            size = 14px\n");
    }
}
