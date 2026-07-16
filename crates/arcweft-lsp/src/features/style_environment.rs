//! Thin LSP projections for Sans-I/O native Style environment tooling.

use crate::positions::LineIndex;
use arcweft_tooling::style_environment::{
    StyleEnvironmentCodeAction, StyleEnvironmentCompletionItem, StyleEnvironmentCompletionKind,
    StyleEnvironmentHover, StyleEnvironmentIntrinsicTarget, StyleEnvironmentNavigationResult,
    StyleEnvironmentSemanticKind, StyleEnvironmentSemanticSpan,
};
use lsp_types::{
    CodeAction, CodeActionKind, CompletionItem, CompletionItemKind, CompletionTextEdit, Hover,
    HoverContents, MarkupContent, MarkupKind, Range, TextEdit, Uri, WorkspaceEdit,
};
use std::collections::HashMap;

/// Absolute LSP range plus repository-owned semantic-token legend index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StyleEnvironmentLspSemanticSpan {
    pub range: Range,
    pub token_type: u32,
}

/// LSP origin retained alongside a typed intrinsic target, never a fabricated URI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StyleEnvironmentLspNavigation {
    pub origin: Range,
    pub target: StyleEnvironmentIntrinsicTarget,
}

/// Projects tooling completions without re-evaluating their field rules.
pub fn completion_items(
    items: &[StyleEnvironmentCompletionItem],
    index: &LineIndex,
) -> Vec<CompletionItem> {
    items
        .iter()
        .map(|item| CompletionItem {
            label: item.label.to_owned(),
            kind: Some(completion_kind(item.kind)),
            insert_text: Some(item.insert_text.to_owned()),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range: byte_range(index, item.replace.start(), item.replace.end()),
                new_text: item.insert_text.to_owned(),
            })),
            ..CompletionItem::default()
        })
        .collect()
}

/// Projects one tooling hover to Markdown LSP content.
pub fn hover(hover: &StyleEnvironmentHover, index: &LineIndex) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: hover.markdown.clone(),
        }),
        range: Some(byte_range(index, hover.range.start(), hover.range.end())),
    }
}

/// Projects typed semantic ranges to stable legend indexes.
pub fn semantic_spans(
    spans: &[StyleEnvironmentSemanticSpan],
    index: &LineIndex,
) -> Vec<StyleEnvironmentLspSemanticSpan> {
    spans
        .iter()
        .map(|span| StyleEnvironmentLspSemanticSpan {
            range: byte_range(index, span.range.start(), span.range.end()),
            token_type: semantic_token_index(span.kind),
        })
        .collect()
}

/// Projects tooling actions into edits for the exact open-document URI.
pub fn code_actions(
    actions: &[StyleEnvironmentCodeAction],
    uri: &Uri,
    index: &LineIndex,
) -> Vec<CodeAction> {
    actions
        .iter()
        .map(|action| {
            let edits = action
                .edits
                .iter()
                .map(|edit| TextEdit {
                    range: byte_range(index, edit.start, edit.end),
                    new_text: edit.replacement.clone(),
                })
                .collect();
            CodeAction {
                title: action.title.to_owned(),
                kind: Some(CodeActionKind::QUICKFIX),
                edit: Some(WorkspaceEdit {
                    changes: Some(HashMap::from([(uri.clone(), edits)])),
                    ..WorkspaceEdit::default()
                }),
                ..CodeAction::default()
            }
        })
        .collect()
}

/// Projects intrinsic navigation while preserving the non-URI target contract.
pub fn navigation(
    navigation: StyleEnvironmentNavigationResult,
    index: &LineIndex,
) -> StyleEnvironmentLspNavigation {
    StyleEnvironmentLspNavigation {
        origin: byte_range(index, navigation.origin.start(), navigation.origin.end()),
        target: navigation.target,
    }
}

const fn completion_kind(kind: StyleEnvironmentCompletionKind) -> CompletionItemKind {
    match kind {
        StyleEnvironmentCompletionKind::Field => CompletionItemKind::FIELD,
        StyleEnvironmentCompletionKind::Operator => CompletionItemKind::OPERATOR,
        StyleEnvironmentCompletionKind::EnumValue => CompletionItemKind::ENUM_MEMBER,
        StyleEnvironmentCompletionKind::Boolean | StyleEnvironmentCompletionKind::Number => {
            CompletionItemKind::VALUE
        }
        StyleEnvironmentCompletionKind::Punctuation => CompletionItemKind::KEYWORD,
    }
}

const fn semantic_token_index(kind: StyleEnvironmentSemanticKind) -> u32 {
    match kind {
        StyleEnvironmentSemanticKind::Keyword => 0,
        StyleEnvironmentSemanticKind::Intrinsic => 1,
        StyleEnvironmentSemanticKind::Field => 2,
        StyleEnvironmentSemanticKind::Operator => 3,
        StyleEnvironmentSemanticKind::EnumValue => 4,
        StyleEnvironmentSemanticKind::Boolean => 5,
        StyleEnvironmentSemanticKind::Number => 6,
        StyleEnvironmentSemanticKind::Unit => 7,
        StyleEnvironmentSemanticKind::Punctuation => 8,
        StyleEnvironmentSemanticKind::Recovered => 9,
    }
}

fn byte_range(index: &LineIndex, start: usize, end: usize) -> Range {
    Range {
        start: index.position_from_byte_offset(start),
        end: index.position_from_byte_offset(end),
    }
}
