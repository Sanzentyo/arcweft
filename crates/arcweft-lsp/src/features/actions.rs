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
            CodeAction {
                title: format!("Extract `{}` override to line options", contribution.path),
                kind: Some(CodeActionKind::REFACTOR_EXTRACT),
                edit: Some(workspace_edit_from_tooling_edit(
                    uri,
                    &edit,
                    document.line_index(),
                )),
                ..CodeAction::default()
            }
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
                Some(CodeAction {
                    title: format!(
                        "Extract `{}` override to character dialogue_style",
                        contribution.path
                    ),
                    kind: Some(CodeActionKind::REFACTOR_EXTRACT),
                    edit: Some(workspace_edit_from_tooling_edit(
                        uri,
                        &edit,
                        document.line_index(),
                    )),
                    ..CodeAction::default()
                })
            }),
    );
    actions
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
                entity_dialogue_style_edit(entity, path, value)
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
    entity: &EntityDeclItem,
    path: &str,
    value: &str,
) -> Option<TextEdit> {
    let body = entity.body()?;
    let body_range = entity.body_range()?;
    if let Some(insertion) = existing_dialogue_style_insertion(body, body_range, path, value) {
        return Some(insertion);
    }
    Some(TextEdit {
        start: body_range.end(),
        end: body_range.end(),
        replacement: format!("\n    dialogue_style {{\n        {path} = {value}\n    }}"),
    })
}

fn existing_dialogue_style_insertion(
    body: &str,
    body_range: &TextRange,
    path: &str,
    value: &str,
) -> Option<TextEdit> {
    let start = body.find("dialogue_style")?;
    let open = body[start..].find('{')? + start;
    let close = matching_brace(body, open)?;
    let line_start = body[..close].rfind('\n').map_or(0, |offset| offset + 1);
    let close_indent = &body[line_start..close];
    Some(TextEdit {
        start: body_range.start() + line_start,
        end: body_range.start() + line_start,
        replacement: format!("{close_indent}    {path} = {value}\n"),
    })
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
            "\n    dialogue_style {\n        rich_text.ruby.size = 14px\n    }"
        );
    }

    #[test]
    fn character_dialogue_style_edit_appends_existing_block() {
        let source = "pub character alice {\n    dialogue_style {\n        text_color = rgb(\"#202122\")\n    }\n}\n";
        let edit = character_dialogue_style_edit(source, "alice", "rich_text.ruby.size", "14px")
            .expect("character style edit");

        assert_eq!(edit.start, source.find("    }\n}").expect("style close"));
        assert_eq!(edit.replacement, "        rich_text.ruby.size = 14px\n");
    }
}
