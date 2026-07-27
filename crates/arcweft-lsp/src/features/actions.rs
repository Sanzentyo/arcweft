use crate::diagnostics::DocumentAnalysis;
use crate::documents::DocumentSnapshot;
use crate::profiles::LspProfile;
use arcweft_lang_hir::lower::lower_document_to_hir;
use arcweft_lang_sema::{
    check::analyze_types,
    effect_diagnostics::{EffectDiagnosticKind, EffectSeverity},
    effect_model::CallableId,
    effects::EffectSet,
};
use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        items::{FunctionItem, Item},
    },
    parser::{ParseOptions, parse_document_with_source},
};
use arcweft_tooling::model::{TextEdit, ToolingError};
use arcweft_verify_lsp::{
    code_actions_from_report_with_mapper, source_code_actions_with_mapper,
    workspace_edit_from_tooling_edit,
};
use lsp_types::{CodeAction, CodeActionKind, Position, Uri};
use std::sync::Arc;

/// Computes code actions for one open Arcweft document.
pub fn actions(
    profile: &LspProfile,
    uri: &Uri,
    document: &DocumentSnapshot,
    analysis: &DocumentAnalysis,
    _position: Position,
) -> Result<Vec<CodeAction>, ToolingError> {
    let mut actions =
        source_code_actions_with_mapper(uri, document.source_document(), document.line_index())?;
    if let Some(report) = analysis.verification_report() {
        actions.extend(code_actions_from_report_with_mapper(
            uri,
            report,
            document.line_index(),
        ));
    }
    actions.extend(effect_contract_actions(profile, uri, document));
    Ok(actions)
}

fn effect_contract_actions(
    profile: &LspProfile,
    uri: &Uri,
    document: &DocumentSnapshot,
) -> Vec<CodeAction> {
    let parsed = parse_document_with_source(
        Arc::clone(document.source_document()),
        ParseOptions::default(),
    );
    if !parsed.errors().is_empty() {
        return Vec::new();
    }
    let Ok(hir) = lower_document_to_hir(parsed.document().as_ref(), parsed.typed_tree()) else {
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
        let analysis = DocumentAnalysis::analyze_snapshot(&document, &profile);

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
