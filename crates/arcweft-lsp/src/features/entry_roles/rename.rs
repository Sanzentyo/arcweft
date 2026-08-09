//! Entry-owned rename planning and fail-closed workspace edits.

use std::collections::HashMap;

use arcweft_lang_sema::entry::CheckedEntryId;
use arcweft_source::SourceSpan;
use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{PrepareRenameResponse, TextEdit, Uri, WorkspaceEdit};

use super::{EntryToolSymbol, callable_symbol, entry_declaration, source_text, symbol_at};
use crate::{
    documents::{DocumentSnapshot, DocumentStore},
    positions::{LineIndex, PositionEncoding},
    profiles::{LspProfile, accepted_project::AcceptedProjectSnapshot},
};

pub(crate) fn prepare_rename(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<PrepareRenameResponse> {
    let accepted = profile.accepted_environment()?;
    let index = accepted.executable()?.semantic_index();
    let cursor = symbol_at(profile, document, offset, index)?;
    if !matches!(
        cursor.symbol,
        EntryToolSymbol::Callable(_) | EntryToolSymbol::Entry(_)
    ) {
        return None;
    }
    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: document
            .line_index()
            .range_from_byte_span(cursor.source_range.start, cursor.source_range.end),
        placeholder: cursor.placeholder,
    })
}

#[allow(
    clippy::mutable_key_type,
    clippy::too_many_lines,
    reason = "LSP WorkspaceEdit requires Uri keys and rename keeps atomic edit collection with its stale-document fail-closed guard"
)]
pub(crate) fn rename(
    profile: &LspProfile,
    documents: &DocumentStore,
    document: &DocumentSnapshot,
    offset: usize,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let accepted = profile.accepted_environment()?;
    let index = accepted.executable()?.semantic_index();
    let project = accepted.project();
    let cursor = symbol_at(profile, document, offset, index)?;
    let encoding = document.line_index().position_encoding();
    let mut changes = HashMap::<Uri, Vec<TextEdit>>::new();
    match &cursor.symbol {
        EntryToolSymbol::Callable(declaration) => {
            if !is_identifier(new_name) {
                return None;
            }
            let source = callable_symbol(project, declaration)?;
            push_source_edit(
                project,
                &mut changes,
                source.name_span(),
                new_name,
                encoding,
            )?;
            for edge in index.entry_role_edges() {
                if edge.target().callable().is_some_and(|(candidate, _)| {
                    &arcweft_lang_hir::symbol::CallableDeclarationKey::Existing(candidate.clone())
                        == declaration
                }) && source_text(project, edge.source()) == Some(declaration.name())
                {
                    push_source_edit(project, &mut changes, edge.source(), new_name, encoding)?;
                }
            }
            for reference in project
                .callable_references()
                .iter()
                .filter(|reference| reference.declaration() == declaration)
                .filter(|reference| {
                    source_text(project, reference.source()) == Some(declaration.name())
                })
            {
                push_source_edit(
                    project,
                    &mut changes,
                    reference.source(),
                    new_name,
                    encoding,
                )?;
            }
        }
        EntryToolSymbol::Nominal(_) | EntryToolSymbol::Flow(_) => return None,
        EntryToolSymbol::Entry(id) => {
            let canonical = renamed_entry_id(id, new_name)?;
            let (_, span) = entry_declaration(project, id)?;
            push_source_edit(
                project,
                &mut changes,
                &span,
                &format!("@{canonical}"),
                encoding,
            )?;
            for reference in project
                .entry_references()
                .iter()
                .filter(|reference| reference.entry() == id)
            {
                push_source_edit(
                    project,
                    &mut changes,
                    reference.source(),
                    &canonical,
                    encoding,
                )?;
            }
            for (entry, selection) in profile.entry_selections() {
                if entry == id.public_id().as_str() {
                    let uri = selection.uri()?.clone();
                    let range = selection.value_range();
                    let line_index = LineIndex::new(
                        selection.source().to_owned(),
                        document.line_index().position_encoding(),
                    );
                    changes.entry(uri).or_default().push(TextEdit::new(
                        line_index.range_from_byte_span(range.start, range.end),
                        format!("@{canonical}"),
                    ));
                }
            }
        }
    }
    if !changes.keys().all(|uri| {
        documents.get(uri).is_none_or(|open| {
            project
                .sources()
                .by_uri(uri)
                .is_some_and(|source| source.document().text() == open.text())
                || profile.entry_selections().iter().any(|(_, selection)| {
                    selection.uri() == Some(uri.clone()) && selection.source() == open.text()
                })
        })
    }) {
        return None;
    }
    Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
}

#[allow(
    clippy::mutable_key_type,
    reason = "LSP WorkspaceEdit requires its protocol Uri key type"
)]
fn push_source_edit(
    project: &AcceptedProjectSnapshot,
    changes: &mut HashMap<Uri, Vec<TextEdit>>,
    span: &SourceSpan,
    replacement: &str,
    encoding: PositionEncoding,
) -> Option<()> {
    let source = project.source(span.source())?;
    let uri = source.locator().uri().cloned()?;
    let line_index = LineIndex::new(source.document().text().to_owned(), encoding);
    changes.entry(uri).or_default().push(TextEdit::new(
        line_index.range_from_byte_span(span.range().start(), span.range().end()),
        replacement.to_owned(),
    ));
    Some(())
}

fn renamed_entry_id(id: &CheckedEntryId, new_name: &str) -> Option<String> {
    if new_name.starts_with("entry.") {
        return valid_entry_id(new_name).then(|| new_name.to_owned());
    }
    if !is_identifier(new_name) {
        return None;
    }
    let current = id.public_id().as_str();
    let prefix = current
        .rsplit_once('.')
        .map_or("entry", |(prefix, _)| prefix);
    let candidate = format!("{prefix}.{new_name}");
    valid_entry_id(&candidate).then_some(candidate)
}

fn valid_entry_id(value: &str) -> bool {
    value.starts_with("entry.")
        && value
            .split('.')
            .all(|segment| !segment.is_empty() && is_identifier(segment))
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}
