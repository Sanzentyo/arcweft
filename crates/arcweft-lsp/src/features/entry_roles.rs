//! LSP queries for ordinary declarations bound through entry roles.
//!
//! Entry roles never create synthetic reducer, state, or Agent symbols. This
//! module adapts accepted semantic edges back to their original declarations.

use std::{collections::BTreeSet, sync::Arc};

use arcweft_lang_hir::{
    identity::ItemId,
    item::HirItemKind,
    leaf::{HirIdRef, HirIdRefValue},
    module::HirModule,
    source_index::{
        HirCallableSourceRole, HirEntrySourcePart, HirItemSourceRole, HirSourcePresence,
        HirSourceQuery, HirSourceSite,
    },
    symbol::nominal::{ProjectNominalDeclaration, ProjectNominalDeclarationKind},
    symbol::{CallableDeclarationKey, CallableSymbol, FlowDeclarationId},
};
use arcweft_lang_sema::{
    entry::{BoundNominalKind, BoundNominalTypeKey, CheckedEntryId, CheckedFlowId},
    project_index::{ProjectEntryRoleTarget, ProjectSemanticIndex},
};
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
    Callable(CallableDeclarationKey),
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
    let index = accepted.executable()?.semantic_index();
    let project = accepted.project();
    let cursor = symbol_at(profile, document, offset, index)?;
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
    let index = accepted.executable()?.semantic_index();
    let project = accepted.project();
    let cursor = symbol_at(profile, document, offset, index)?;
    let encoding = document.line_index().position_encoding();
    let mut locations = declaration_location(project, &cursor.symbol, encoding)
        .into_iter()
        .collect::<Vec<_>>();
    match &cursor.symbol {
        EntryToolSymbol::Callable(declaration) => {
            locations.extend(
                index
                    .entry_role_edges()
                    .iter()
                    .filter(|edge| {
                        edge.target().callable().is_some_and(|(candidate, _)| {
                            &CallableDeclarationKey::Existing(candidate.clone()) == declaration
                        })
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
                index
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
                index
                    .entry_role_edges()
                    .iter()
                    .filter(|edge| {
                        edge.target()
                            .flow()
                            .is_some_and(|(candidate, _)| candidate == id)
                    })
                    .filter_map(|edge| source_location(project, edge.source(), encoding)),
            );
            locations.extend(
                project
                    .callable_references()
                    .iter()
                    .filter(|reference| {
                        matches!(
                            reference.declaration(),
                            CallableDeclarationKey::Flow(declaration)
                                if declaration.semantic_digest() == *id.declaration_digest()
                        )
                    })
                    .filter_map(|reference| source_location(project, reference.source(), encoding)),
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
    let index = accepted.executable()?.semantic_index();
    let project = accepted.project();
    let cursor = symbol_at(profile, document, offset, index)?;
    let mut text = match &cursor.symbol {
        EntryToolSymbol::Callable(declaration) => {
            let source = callable_symbol(project, declaration)?;
            let signature = callable_source_span(
                project,
                source,
                HirCallableSourceRole::Signature {
                    owner: source.source_owner(),
                },
            )?;
            format!("```arcw\n{}\n```", source_text(project, &signature)?)
        }
        EntryToolSymbol::Nominal(key) => format!("`{}` nominal type", key.name()),
        EntryToolSymbol::Flow(id) => format!("flow `@{}`", id.public_id()),
        EntryToolSymbol::Entry(id) => format!("entry `@{}`", id.public_id()),
    };
    let bindings = binding_annotations(index, &cursor.symbol);
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
    index: &ProjectSemanticIndex,
) -> Option<CursorSymbol> {
    if let Some(symbol) = manifest_entry_at(profile, document, offset, index) {
        return Some(symbol);
    }
    let accepted = profile.accepted_environment()?;
    let project = accepted.project();
    let accepted_source = project.sources().by_uri(document.uri())?;
    if !Arc::ptr_eq(accepted_source.document(), document.source_document()) {
        return None;
    }
    let identity = accepted_source.document().identity();
    for edge in index.entry_role_edges() {
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
                symbol: entry_tool_symbol_for_declaration(Some(index), reference.declaration())?,
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
    for callable in project.project_symbols().callable_symbols() {
        if callable.name_span().source() == identity
            && contains_source(callable.name_span().range(), offset)
        {
            let Some(symbol) =
                entry_tool_symbol_for_declaration(Some(index), callable.declaration())
            else {
                continue;
            };
            return Some(CursorSymbol {
                symbol,
                source_range: callable.name_span().range().start()
                    ..callable.name_span().range().end(),
                placeholder: callable.declaration().name().to_owned(),
            });
        }
    }
    declaration_symbol_at(project, identity, offset, index)
}

fn manifest_entry_at(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
    index: &ProjectSemanticIndex,
) -> Option<CursorSymbol> {
    for (entry, selection) in profile.entry_selections() {
        if selection.uri().as_ref() != Some(document.uri()) || selection.source() != document.text()
        {
            continue;
        }
        let range = selection.value_range();
        if range.start <= offset && offset <= range.end {
            let id = index
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
    index: &ProjectSemanticIndex,
) -> Option<CursorSymbol> {
    for nominal in project.project_symbols().nominal_symbols() {
        let span = nominal.source().name();
        if span.source() != identity || !contains_source(span.range(), offset) {
            continue;
        }
        let key = nominal_key(index, nominal)?;
        return Some(CursorSymbol {
            symbol: EntryToolSymbol::Nominal(key),
            source_range: span.range().start()..span.range().end(),
            placeholder: nominal.id().name().as_str().to_owned(),
        });
    }

    for (_, hir) in project.hir_project().view().modules() {
        if hir.provenance().source_identity() != identity {
            continue;
        }
        for owner in hir.source_ordered_items() {
            let item = hir.resolve_item(*owner).ok()?;
            if let HirItemKind::Entry(entry) = item.kind() {
                let Some(public_id) = entry.id().value().and_then(resolved_absolute_id) else {
                    continue;
                };
                let Some(span) = item_source_span(
                    hir,
                    *owner,
                    HirItemSourceRole::Entry(HirEntrySourcePart::Id),
                ) else {
                    continue;
                };
                if !contains_source(span.range(), offset) {
                    continue;
                }
                let checked = index
                    .entry_records()
                    .keys()
                    .find(|id| id.public_id().as_str() == public_id)?
                    .clone();
                return Some(CursorSymbol {
                    symbol: EntryToolSymbol::Entry(checked),
                    source_range: span.range().start()..span.range().end(),
                    placeholder: public_id.to_owned(),
                });
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
            callable_symbol(project, declaration)?.name_span(),
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

fn callable_symbol<'a>(
    project: &'a crate::profiles::accepted_project::AcceptedProjectSnapshot,
    declaration: &CallableDeclarationKey,
) -> Option<&'a CallableSymbol> {
    project.project_symbols().callable(declaration)
}

fn callable_source_span(
    project: &crate::profiles::accepted_project::AcceptedProjectSnapshot,
    symbol: &CallableSymbol,
    role: HirCallableSourceRole,
) -> Option<SourceSpan> {
    let hir = project
        .hir_project()
        .view()
        .module(symbol.declaration().module())?;
    if hir.snapshot_id() != symbol.source_snapshot() || role.owner() != symbol.source_owner() {
        return None;
    }
    item_source_span(hir, symbol.source_item(), HirItemSourceRole::Callable(role))
}

fn nominal_key(
    index: &ProjectSemanticIndex,
    nominal: &ProjectNominalDeclaration,
) -> Option<BoundNominalTypeKey> {
    let expected_kind = match nominal.id().kind() {
        ProjectNominalDeclarationKind::Struct => BoundNominalKind::Struct,
        ProjectNominalDeclarationKind::Enum => BoundNominalKind::Enum,
        ProjectNominalDeclarationKind::TypeAlias => return None,
    };
    index
        .entry_role_edges()
        .iter()
        .filter_map(|edge| edge.target().nominal().map(|(key, _)| key))
        .find(|key| {
            key.module() == nominal.id().module()
                && key.name() == nominal.id().name().as_str()
                && key.kind() == expected_kind
        })
        .cloned()
}

fn nominal_symbol<'a>(
    project: &'a crate::profiles::accepted_project::AcceptedProjectSnapshot,
    key: &BoundNominalTypeKey,
) -> Option<&'a ProjectNominalDeclaration> {
    if project.project_symbols().world().package() != key.package() {
        return None;
    }
    project.project_symbols().nominal_symbols().find(|nominal| {
        let kind_matches = matches!(
            (nominal.id().kind(), key.kind()),
            (
                ProjectNominalDeclarationKind::Struct,
                BoundNominalKind::Struct
            ) | (ProjectNominalDeclarationKind::Enum, BoundNominalKind::Enum)
        );
        nominal.id().module() == key.module()
            && nominal.id().name().as_str() == key.name()
            && kind_matches
    })
}

fn nominal_declaration(
    project: &crate::profiles::accepted_project::AcceptedProjectSnapshot,
    key: &BoundNominalTypeKey,
) -> Option<SourceSpan> {
    Some(nominal_symbol(project, key)?.source().name().clone())
}

fn flow_declaration(
    project: &crate::profiles::accepted_project::AcceptedProjectSnapshot,
    checked: &CheckedFlowId,
) -> Option<SourceSpan> {
    project
        .project_symbols()
        .callable_symbols()
        .find(|symbol| {
            matches!(
                symbol.declaration(),
                CallableDeclarationKey::Flow(declaration)
                    if declaration.semantic_digest() == *checked.declaration_digest()
            )
        })
        .map(|symbol| symbol.name_span().clone())
}

fn checked_flow_for_declaration<'a>(
    index: &'a ProjectSemanticIndex,
    declaration: &FlowDeclarationId,
) -> Option<&'a CheckedFlowId> {
    let digest = declaration.semantic_digest();
    index
        .entry_role_edges()
        .iter()
        .filter_map(|edge| edge.target().flow().map(|(id, _)| id))
        .find(|checked| *checked.declaration_digest() == digest)
}

fn entry_tool_symbol_for_declaration(
    index: Option<&ProjectSemanticIndex>,
    declaration: &CallableDeclarationKey,
) -> Option<EntryToolSymbol> {
    match declaration {
        CallableDeclarationKey::Flow(declaration) => {
            checked_flow_for_declaration(index?, declaration)
                .cloned()
                .map(EntryToolSymbol::Flow)
        }
        declaration => Some(EntryToolSymbol::Callable(declaration.clone())),
    }
}

fn entry_declaration(
    project: &crate::profiles::accepted_project::AcceptedProjectSnapshot,
    checked: &CheckedEntryId,
) -> Option<(String, SourceSpan)> {
    for (_, hir) in project.hir_project().view().modules() {
        for owner in hir.source_ordered_items() {
            let item = hir.resolve_item(*owner).ok()?;
            let HirItemKind::Entry(entry) = item.kind() else {
                continue;
            };
            let Some(public_id) = entry.id().value().and_then(resolved_absolute_id) else {
                continue;
            };
            if public_id == checked.public_id().as_str() {
                let span = item_source_span(
                    hir,
                    *owner,
                    HirItemSourceRole::Entry(HirEntrySourcePart::Id),
                )?;
                return Some((public_id.to_owned(), span));
            }
        }
    }
    None
}

fn item_source_span(hir: &HirModule, owner: ItemId, role: HirItemSourceRole) -> Option<SourceSpan> {
    let lookup = hir
        .source_site(
            hir.provenance().source_identity(),
            HirSourceQuery::Item { owner, role },
        )
        .ok()?;
    let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence() else {
        return None;
    };
    Some(span.clone())
}

fn absolute_id(reference: &HirIdRef) -> Option<&str> {
    match reference {
        HirIdRef::Absolute(reference) => Some(reference.as_str()),
        HirIdRef::Relative(_) | HirIdRef::FamilyRelative(_) => None,
    }
}

fn resolved_absolute_id(reference: &HirIdRefValue) -> Option<&str> {
    reference.as_resolved().and_then(absolute_id)
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
            EntryToolSymbol::Callable(CallableDeclarationKey::Existing(declaration.clone()))
        }
        ProjectEntryRoleTarget::Nominal { key, .. } => EntryToolSymbol::Nominal(key.clone()),
        ProjectEntryRoleTarget::Flow { id, .. } => EntryToolSymbol::Flow(id.clone()),
    }
}

fn contains_source(range: SourceRange, offset: usize) -> bool {
    range.start() <= offset && offset < range.end()
}

#[cfg(test)]
mod tests;
