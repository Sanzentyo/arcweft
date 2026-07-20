//! LSP queries for ordinary declarations bound through entry roles.
//!
//! Entry roles never create synthetic reducer, state, or Agent symbols. This
//! module adapts accepted semantic edges back to their original declarations.

use std::collections::BTreeSet;

use arcweft_lang_hir::{
    callable_source::HirCallableSignatureSource, model::HirTopLevelDecl,
    symbol::CallableDeclarationId,
};
use arcweft_lang_sema::{
    entry::{BoundNominalTypeKey, CheckedEntryId, CheckedFlowId},
    project_index::{ProjectEntryRoleTarget, ProjectSemanticIndex},
};
use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_source::{SourceDocumentIdentity, SourceRange, SourceSpan};
use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{GotoDefinitionResponse, Hover, HoverContents, Location, MarkedString};

use crate::{documents::DocumentSnapshot, profiles::LspProfile};

mod presentation;
mod rename;

use presentation::binding_annotations;
#[cfg(test)]
pub(crate) use presentation::workspace_symbols;
pub(crate) use presentation::{
    callable_completions, document_symbols, workspace_symbols_for_profiles,
};
pub(crate) use rename::{prepare_rename, rename};

#[derive(Clone, Debug, Eq, PartialEq)]
enum EntryToolSymbol {
    Callable(CallableDeclarationId),
    Nominal(BoundNominalTypeKey),
    Flow(CheckedFlowId),
    Entry(CheckedEntryId),
}

#[derive(Clone, Debug)]
struct CursorSymbol {
    symbol: EntryToolSymbol,
    source_range: std::ops::Range<usize>,
    placeholder: String,
}

pub(crate) fn definition(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<GotoDefinitionResponse> {
    let accepted = profile.accepted_environment()?;
    let project = accepted.project();
    let cursor = symbol_at(profile, document, offset)?;
    Some(GotoDefinitionResponse::Scalar(declaration_location(
        project,
        &cursor.symbol,
        document.line_index().position_encoding(),
    )?))
}

pub(crate) fn references(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<Vec<Location>> {
    let accepted = profile.accepted_environment()?;
    let project = accepted.project();
    let cursor = symbol_at(profile, document, offset)?;
    let encoding = document.line_index().position_encoding();
    let mut locations = declaration_location(project, &cursor.symbol, encoding)
        .into_iter()
        .collect::<Vec<_>>();
    match &cursor.symbol {
        EntryToolSymbol::Callable(declaration) => {
            locations.extend(
                project
                    .semantic_index()
                    .entry_role_edges()
                    .iter()
                    .filter(|edge| {
                        edge.target()
                            .callable()
                            .is_some_and(|(candidate, _)| candidate == declaration)
                    })
                    .filter_map(|edge| source_location(project, edge.source(), encoding)),
            );
            locations.extend(
                project
                    .callable_references()
                    .iter()
                    .filter(|reference| reference.declaration() == declaration)
                    .filter_map(|reference| source_location(project, reference.source(), encoding)),
            );
        }
        EntryToolSymbol::Nominal(key) => {
            locations.extend(
                project
                    .semantic_index()
                    .entry_role_edges()
                    .iter()
                    .filter(|edge| {
                        edge.target()
                            .nominal()
                            .is_some_and(|(candidate, _)| candidate == key)
                    })
                    .filter_map(|edge| source_location(project, edge.source(), encoding)),
            );
        }
        EntryToolSymbol::Flow(id) => {
            locations.extend(
                project
                    .semantic_index()
                    .entry_role_edges()
                    .iter()
                    .filter(|edge| {
                        edge.target()
                            .flow()
                            .is_some_and(|(candidate, _)| candidate == id)
                    })
                    .filter_map(|edge| source_location(project, edge.source(), encoding)),
            );
        }
        EntryToolSymbol::Entry(id) => {
            locations.extend(
                project
                    .entry_references()
                    .iter()
                    .filter(|reference| reference.entry() == id)
                    .filter_map(|reference| source_location(project, reference.source(), encoding)),
            );
            locations.extend(
                profile
                    .entry_selections()
                    .iter()
                    .filter_map(|(entry, selection)| {
                        (entry == id.public_id().as_str())
                            .then(|| manifest_selection_location(selection, encoding))
                            .flatten()
                    }),
            );
        }
    }
    let mut seen = BTreeSet::new();
    locations.retain(|location| {
        seen.insert((
            location.uri.to_string(),
            location.range.start.line,
            location.range.start.character,
            location.range.end.line,
            location.range.end.character,
        ))
    });
    Some(locations)
}

pub(crate) fn hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<Hover> {
    let accepted = profile.accepted_environment()?;
    let project = accepted.project();
    let cursor = symbol_at(profile, document, offset)?;
    let mut text = match &cursor.symbol {
        EntryToolSymbol::Callable(declaration) => {
            let source = callable_source(project.hir_project(), declaration)?;
            format!(
                "```arcw\n{}\n```",
                source_text(project, source.signature_span())?
            )
        }
        EntryToolSymbol::Nominal(key) => format!("`{}` nominal type", key.name()),
        EntryToolSymbol::Flow(id) => format!("flow `@{}`", id.public_id()),
        EntryToolSymbol::Entry(id) => format!("entry `@{}`", id.public_id()),
    };
    let bindings = binding_annotations(project.semantic_index(), &cursor.symbol);
    if !bindings.is_empty() {
        text.push_str("\n\n");
        text.push_str(&bindings.join("\n"));
    }
    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(text)),
        range: None,
    })
}

fn symbol_at(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<CursorSymbol> {
    if let Some(symbol) = manifest_entry_at(profile, document, offset) {
        return Some(symbol);
    }
    let accepted = profile.accepted_environment()?;
    let project = accepted.project();
    let accepted_source = project.sources().by_uri(document.uri())?;
    if accepted_source.document().text() != document.text() {
        return None;
    }
    let identity = accepted_source.document().identity();
    for edge in project.semantic_index().entry_role_edges() {
        if edge.source().source() == identity && contains_source(edge.source().range(), offset) {
            let symbol = target_symbol(edge.target());
            let (source_range, placeholder) = match &symbol {
                EntryToolSymbol::Callable(_) => (
                    edge.source().range().start()..edge.source().range().end(),
                    source_text(project, edge.source())?.to_owned(),
                ),
                EntryToolSymbol::Nominal(key) => (
                    edge.source().range().start()..edge.source().range().end(),
                    key.name().to_owned(),
                ),
                EntryToolSymbol::Flow(id) => (
                    edge.source().range().start()..edge.source().range().end(),
                    id.public_id().as_str().to_owned(),
                ),
                EntryToolSymbol::Entry(_) => unreachable!("entry roles do not target entries"),
            };
            return Some(CursorSymbol {
                symbol,
                source_range,
                placeholder,
            });
        }
    }
    for reference in project.callable_references() {
        if reference.source().source() == identity
            && contains_source(reference.source().range(), offset)
        {
            let placeholder = source_text(project, reference.source())?.to_owned();
            return Some(CursorSymbol {
                symbol: EntryToolSymbol::Callable(reference.declaration().clone()),
                source_range: reference.source().range().start()..reference.source().range().end(),
                placeholder,
            });
        }
    }
    for reference in project.entry_references() {
        if reference.source().source() == identity
            && contains_source(reference.source().range(), offset)
        {
            return Some(CursorSymbol {
                symbol: EntryToolSymbol::Entry(reference.entry().clone()),
                source_range: reference.source().range().start()..reference.source().range().end(),
                placeholder: reference.entry().public_id().as_str().to_owned(),
            });
        }
    }
    for callable in project.hir_project().callable_signature_sources() {
        if callable.name_span().source() == identity
            && contains_source(callable.name_span().range(), offset)
        {
            return Some(CursorSymbol {
                symbol: EntryToolSymbol::Callable(callable.declaration().clone()),
                source_range: callable.name_span().range().start()
                    ..callable.name_span().range().end(),
                placeholder: callable.declaration().name().to_owned(),
            });
        }
    }
    declaration_symbol_at(project, identity, offset)
}

fn manifest_entry_at(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<CursorSymbol> {
    let accepted = profile.accepted_environment()?;
    for (entry, selection) in profile.entry_selections() {
        if selection.uri().as_ref() != Some(document.uri()) || selection.source() != document.text()
        {
            continue;
        }
        let range = selection.value_range();
        if range.start <= offset && offset <= range.end {
            let id = accepted
                .project()
                .semantic_index()
                .entry_records()
                .keys()
                .find(|id| id.public_id().as_str() == entry)?
                .clone();
            return Some(CursorSymbol {
                symbol: EntryToolSymbol::Entry(id),
                source_range: range,
                placeholder: entry.clone(),
            });
        }
    }
    None
}

fn declaration_symbol_at(
    project: &crate::profiles::accepted_project::AcceptedProjectSnapshot,
    identity: &SourceDocumentIdentity,
    offset: usize,
) -> Option<CursorSymbol> {
    for (module, hir) in project.hir_project().modules() {
        if project.hir_project().source(module) != Some(identity) {
            continue;
        }
        for flow in hir.flows() {
            let Some(id) = flow.id() else { continue };
            if contains(id.range(), offset) {
                let checked = project
                    .semantic_index()
                    .entry_role_edges()
                    .iter()
                    .filter_map(|edge| edge.target().flow().map(|(id, _)| id))
                    .find(|checked| checked.public_id().as_str() == id.body())?
                    .clone();
                return Some(CursorSymbol {
                    symbol: EntryToolSymbol::Flow(checked),
                    source_range: id.range().start()..id.range().end(),
                    placeholder: id.body().to_owned(),
                });
            }
        }
        for declaration in hir.declarations() {
            match declaration {
                HirTopLevelDecl::Entry(entry) if contains(entry.id().range(), offset) => {
                    let name_range = entry.id().authored_body_range()?;
                    let id = project
                        .semantic_index()
                        .entry_records()
                        .keys()
                        .find(|id| id.public_id().as_str() == entry.id().body())?
                        .clone();
                    return Some(CursorSymbol {
                        symbol: EntryToolSymbol::Entry(id),
                        source_range: name_range.start()..name_range.end(),
                        placeholder: entry.id().body().to_owned(),
                    });
                }
                HirTopLevelDecl::Struct(item) => {
                    if contains(item.name_range(), offset) {
                        let key = nominal_key(project.semantic_index(), module, item.name())?;
                        return Some(CursorSymbol {
                            symbol: EntryToolSymbol::Nominal(key),
                            source_range: item.name_range().start()..item.name_range().end(),
                            placeholder: item.name().to_owned(),
                        });
                    }
                }
                HirTopLevelDecl::Enum(item) if contains(item.name_range(), offset) => {
                    let key = nominal_key(project.semantic_index(), module, item.name())?;
                    return Some(CursorSymbol {
                        symbol: EntryToolSymbol::Nominal(key),
                        source_range: item.name_range().start()..item.name_range().end(),
                        placeholder: item.name().to_owned(),
                    });
                }
                _ => {}
            }
        }
    }
    None
}

fn declaration_location(
    project: &crate::profiles::accepted_project::AcceptedProjectSnapshot,
    symbol: &EntryToolSymbol,
    encoding: crate::positions::PositionEncoding,
) -> Option<Location> {
    match symbol {
        EntryToolSymbol::Callable(declaration) => source_location(
            project,
            callable_source(project.hir_project(), declaration)?.name_span(),
            encoding,
        ),
        EntryToolSymbol::Nominal(key) => nominal_declaration(project, key)
            .and_then(|span| source_location(project, &span, encoding)),
        EntryToolSymbol::Flow(id) => {
            flow_declaration(project, id).and_then(|span| source_location(project, &span, encoding))
        }
        EntryToolSymbol::Entry(id) => entry_declaration(project, id)
            .and_then(|(_, span)| source_location(project, &span, encoding)),
    }
}

fn callable_source<'a>(
    hir: &'a arcweft_lang_hir::project::HirProject,
    declaration: &CallableDeclarationId,
) -> Option<&'a HirCallableSignatureSource> {
    hir.callable_signature_sources()
        .find(|source| source.declaration() == declaration)
}

fn nominal_key(
    index: &ProjectSemanticIndex,
    module: &arcweft_lang_syntax::ast::module_path::CanonicalModulePath,
    name: &str,
) -> Option<BoundNominalTypeKey> {
    index
        .entry_role_edges()
        .iter()
        .filter_map(|edge| edge.target().nominal().map(|(key, _)| key))
        .find(|key| key.module() == module && key.name() == name)
        .cloned()
}

fn nominal_declaration(
    project: &crate::profiles::accepted_project::AcceptedProjectSnapshot,
    key: &BoundNominalTypeKey,
) -> Option<SourceSpan> {
    let identity = project.hir_project().source(key.module())?.clone();
    let hir = project.hir_project().module(key.module())?;
    for declaration in hir.declarations() {
        let range = match declaration {
            HirTopLevelDecl::Struct(item) if item.name() == key.name() => item.name_range(),
            HirTopLevelDecl::Enum(item) if item.name() == key.name() => item.name_range(),
            _ => continue,
        };
        return project
            .source(&identity)?
            .document()
            .span(SourceRange::new(range.start(), range.end()))
            .ok();
    }
    None
}

fn flow_declaration(
    project: &crate::profiles::accepted_project::AcceptedProjectSnapshot,
    checked: &CheckedFlowId,
) -> Option<SourceSpan> {
    for (module, hir) in project.hir_project().modules() {
        let identity = project.hir_project().source(module)?.clone();
        for flow in hir.flows() {
            let Some(id) = flow.id() else { continue };
            if id.body() == checked.public_id().as_str() {
                return project
                    .source(&identity)?
                    .document()
                    .span(SourceRange::new(id.range().start(), id.range().end()))
                    .ok();
            }
        }
    }
    None
}

fn entry_declaration(
    project: &crate::profiles::accepted_project::AcceptedProjectSnapshot,
    checked: &CheckedEntryId,
) -> Option<(String, SourceSpan)> {
    for (module, hir) in project.hir_project().modules() {
        let identity = project.hir_project().source(module)?.clone();
        for declaration in hir.declarations() {
            let HirTopLevelDecl::Entry(entry) = declaration else {
                continue;
            };
            if entry.id().body() == checked.public_id().as_str() {
                let span = project
                    .source(&identity)?
                    .document()
                    .span(SourceRange::new(
                        entry.id().range().start(),
                        entry.id().range().end(),
                    ))
                    .ok()?;
                return Some((entry.id().body().to_owned(), span));
            }
        }
    }
    None
}

fn source_location(
    project: &crate::profiles::accepted_project::AcceptedProjectSnapshot,
    span: &SourceSpan,
    encoding: crate::positions::PositionEncoding,
) -> Option<Location> {
    let source = project.source(span.source())?;
    let line_index =
        crate::positions::LineIndex::new(source.document().text().to_owned(), encoding);
    Some(Location::new(
        source.locator().uri()?.clone(),
        line_index.range_from_byte_span(span.range().start(), span.range().end()),
    ))
}

fn manifest_selection_location(
    selection: &crate::profiles::ProfileSourceSelection,
    encoding: crate::positions::PositionEncoding,
) -> Option<Location> {
    let uri = selection.uri()?;
    let line_index = crate::positions::LineIndex::new(selection.source().to_owned(), encoding);
    let range = selection.value_range();
    Some(Location::new(
        uri,
        line_index.range_from_byte_span(range.start, range.end),
    ))
}

fn source_text<'a>(
    project: &'a crate::profiles::accepted_project::AcceptedProjectSnapshot,
    span: &SourceSpan,
) -> Option<&'a str> {
    project
        .source(span.source())?
        .document()
        .text()
        .get(span.range().start()..span.range().end())
}

fn target_symbol(target: &ProjectEntryRoleTarget) -> EntryToolSymbol {
    match target {
        ProjectEntryRoleTarget::Callable { declaration, .. } => {
            EntryToolSymbol::Callable(declaration.clone())
        }
        ProjectEntryRoleTarget::Nominal { key, .. } => EntryToolSymbol::Nominal(key.clone()),
        ProjectEntryRoleTarget::Flow { id, .. } => EntryToolSymbol::Flow(id.clone()),
    }
}

fn contains(range: &TextRange, offset: usize) -> bool {
    range.start() <= offset && offset <= range.end()
}

fn contains_source(range: SourceRange, offset: usize) -> bool {
    range.start() <= offset && offset <= range.end()
}

#[cfg(test)]
mod tests;
