//! Dialogue-line navigation and explicit-ID rename over one accepted project.

use std::{collections::HashMap, sync::Arc};

use arcweft_id::dialogue::DialogueLineId;
use arcweft_lang_hir::project::{AcceptedDialogueLine, HirProject};
use arcweft_lang_sema::project_index::ProjectSemanticIndex;
use arcweft_source::{SourceDocumentIdentity, SourceSpan};
use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{
    GotoDefinitionResponse, Location, PrepareRenameResponse, TextEdit, Uri, WorkspaceEdit,
};

use crate::{
    documents::{DocumentSnapshot, DocumentStore},
    profiles::{
        LspProfile, accepted_project::AcceptedProjectSnapshot, state::AcceptedProfileEnvironment,
    },
};

#[derive(Clone, Copy)]
struct DialogueLineCursor<'a> {
    line: &'a AcceptedDialogueLine,
    source: &'a SourceSpan,
}

pub(crate) fn definition(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<GotoDefinitionResponse> {
    let accepted = exact_environment(profile, document)?;
    let executable = accepted.executable()?;
    let project = accepted.project();
    let hir = project.hir_project();
    let index = executable.semantic_index();
    let cursor = symbol_at(project, hir, index, document, offset)?;
    Some(GotoDefinitionResponse::Scalar(location(
        project,
        declaration_source(cursor.line),
    )?))
}

pub(crate) fn references(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<Vec<Location>> {
    let accepted = exact_environment(profile, document)?;
    let executable = accepted.executable()?;
    let project = accepted.project();
    let hir = project.hir_project();
    let index = executable.semantic_index();
    let cursor = symbol_at(project, hir, index, document, offset)?;
    let mut locations = vec![location(project, declaration_source(cursor.line))?];
    locations.extend(
        index
            .dialogue_line_references()
            .iter()
            .filter(|reference| reference.target() == cursor.line.id())
            .filter_map(|reference| location(project, reference.source())),
    );
    locations.sort_by(|left, right| {
        left.uri
            .as_str()
            .cmp(right.uri.as_str())
            .then_with(|| left.range.start.cmp(&right.range.start))
            .then_with(|| left.range.end.cmp(&right.range.end))
    });
    locations.dedup();
    Some(locations)
}

pub(crate) fn prepare_rename(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<PrepareRenameResponse> {
    let accepted = exact_environment(profile, document)?;
    let executable = accepted.executable()?;
    let project = accepted.project();
    let hir = project.hir_project();
    let index = executable.semantic_index();
    let cursor = symbol_at(project, hir, index, document, offset)?;
    cursor.line.source().id_coordinate_span()?;
    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: document
            .line_index()
            .range_from_byte_span(cursor.source.range().start(), cursor.source.range().end()),
        placeholder: format!("@{}", cursor.line.id()),
    })
}

#[allow(
    clippy::mutable_key_type,
    reason = "LSP WorkspaceEdit requires Uri keys"
)]
pub(crate) fn rename(
    profile: &LspProfile,
    documents: &DocumentStore,
    document: &DocumentSnapshot,
    offset: usize,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let accepted = exact_environment(profile, document)?;
    let executable = accepted.executable()?;
    let project = accepted.project();
    let hir = project.hir_project();
    let index = executable.semantic_index();
    let cursor = symbol_at(project, hir, index, document, offset)?;
    let declaration = cursor.line.source().id_coordinate_span()?;
    let replacement = DialogueLineId::try_new(new_name.to_owned()).ok()?;
    if matches!(
        cursor.line.text_key_origin(),
        arcweft_lang_hir::line_identity::DialogueTextKeyOrigin::Derived
    ) && replacement.generated_text_key().is_err()
    {
        return None;
    }
    if replacement != *cursor.line.id() && hir.dialogue_lines().get(&replacement).is_some() {
        return None;
    }

    let mut changes = HashMap::<Uri, Vec<TextEdit>>::new();
    push_edit(
        project,
        &mut changes,
        declaration,
        &format!("@{replacement}"),
    )?;
    for reference in index
        .dialogue_line_references()
        .iter()
        .filter(|reference| reference.target() == cursor.line.id())
    {
        push_edit(
            project,
            &mut changes,
            reference.source(),
            &format!("@{replacement}"),
        )?;
    }
    if !changes.keys().all(|uri| {
        documents.get(uri).is_none_or(|open| {
            project
                .sources()
                .by_uri(uri)
                .is_some_and(|source| source.document().text() == open.text())
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

fn exact_environment(
    profile: &LspProfile,
    document: &DocumentSnapshot,
) -> Option<Arc<AcceptedProfileEnvironment>> {
    let accepted = profile.accepted_environment()?;
    let project = accepted.project();
    project
        .sources()
        .by_uri(document.uri())
        .is_some_and(|source| Arc::ptr_eq(source.document(), document.source_document()))
        .then_some(accepted)
}

fn symbol_at<'a>(
    project: &AcceptedProjectSnapshot,
    hir: &'a HirProject,
    index: &'a ProjectSemanticIndex,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<DialogueLineCursor<'a>> {
    let identity = project
        .sources()
        .by_uri(document.uri())?
        .document()
        .identity();
    hir.dialogue_lines()
        .records()
        .iter()
        .filter_map(|line| {
            line.source()
                .id_coordinate_span()
                .filter(|source| span_contains(source, identity, offset))
                .map(|source| DialogueLineCursor { line, source })
        })
        .chain(
            index
                .dialogue_line_references()
                .iter()
                .filter_map(|reference| {
                    let source = reference.source();
                    if !span_contains(source, identity, offset) {
                        return None;
                    }
                    Some(DialogueLineCursor {
                        line: hir.dialogue_lines().get(reference.target())?,
                        source,
                    })
                }),
        )
        .min_by_key(|cursor| cursor.source.range().end() - cursor.source.range().start())
}

fn declaration_source(line: &AcceptedDialogueLine) -> &SourceSpan {
    line.source()
        .id_coordinate_span()
        .unwrap_or_else(|| line.source().application_span())
}

fn span_contains(source: &SourceSpan, identity: &SourceDocumentIdentity, offset: usize) -> bool {
    source.source() == identity && source.range().start() <= offset && offset < source.range().end()
}

fn location(project: &AcceptedProjectSnapshot, source: &SourceSpan) -> Option<Location> {
    let accepted = project.source(source.source())?;
    Some(Location::new(
        accepted.locator().uri()?.clone(),
        accepted
            .line_index()
            .range_from_byte_span(source.range().start(), source.range().end()),
    ))
}

#[allow(
    clippy::mutable_key_type,
    reason = "LSP WorkspaceEdit requires Uri keys"
)]
fn push_edit(
    project: &AcceptedProjectSnapshot,
    changes: &mut HashMap<Uri, Vec<TextEdit>>,
    source: &SourceSpan,
    replacement: &str,
) -> Option<()> {
    let accepted = project.source(source.source())?;
    let uri = accepted.locator().uri()?.clone();
    let range = accepted
        .line_index()
        .range_from_byte_span(source.range().start(), source.range().end());
    changes
        .entry(uri)
        .or_default()
        .push(TextEdit::new(range, replacement.to_owned()));
    Some(())
}
