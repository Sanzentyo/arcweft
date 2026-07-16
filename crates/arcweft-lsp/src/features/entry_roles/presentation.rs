//! Ordinary outline, completion, and workspace-symbol presentation for entry role edges.

use std::collections::BTreeMap;

use arcweft_lang_sema::project_index::{ProjectEntryRoleTarget, ProjectSemanticIndex};
use arcweft_lang_syntax::{ast::items::Item, parser::parse_source};
use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{
    CompletionItem, CompletionItemKind, DocumentSymbol, DocumentSymbolResponse, Documentation,
    OneOf, SymbolKind, WorkspaceSymbol, WorkspaceSymbolResponse,
};

use super::{EntryToolSymbol, source_location, source_text};
use crate::{documents::DocumentSnapshot, positions::PositionEncoding, profiles::LspProfile};

#[allow(
    deprecated,
    reason = "the LSP wire type still carries the legacy field"
)]
pub(crate) fn document_symbols(
    profile: &LspProfile,
    document: &DocumentSnapshot,
) -> DocumentSymbolResponse {
    let parsed = parse_source(document.text().to_owned());
    let role_details = exact_role_details(profile, document);
    let mut symbols = Vec::new();
    for item in parsed.typed_tree().items() {
        let descriptor = match item {
            Item::Function(function) => {
                let name_range = function.signature_source().name();
                let detail = role_details
                    .get(&(name_range.start(), name_range.end()))
                    .map_or_else(
                        || function.signature_text().to_owned(),
                        |bindings| {
                            format!("{} — {}", function.signature_text(), bindings.join("; "))
                        },
                    );
                Some((
                    function.signature().name().to_owned(),
                    SymbolKind::FUNCTION,
                    *function.range(),
                    name_range,
                    Some(detail),
                ))
            }
            Item::Struct(item) => Some((
                item.name().to_owned(),
                SymbolKind::STRUCT,
                *item.range(),
                *item.name_range(),
                None,
            )),
            Item::Enum(item) => Some((
                item.name().to_owned(),
                SymbolKind::ENUM,
                *item.range(),
                *item.name_range(),
                None,
            )),
            Item::Flow(item) => item.id().map(|id| {
                (
                    item.name().unwrap_or_else(|| id.body()).to_owned(),
                    SymbolKind::FUNCTION,
                    *item.range(),
                    *id.range(),
                    Some("flow".to_owned()),
                )
            }),
            Item::Entry(item) => Some((
                item.id().body().to_owned(),
                SymbolKind::OBJECT,
                *item.range(),
                *item.id().range(),
                Some(format!("{} entry", item.kind().as_str())),
            )),
            _ => None,
        };
        let Some((name, kind, range, selection, detail)) = descriptor else {
            continue;
        };
        symbols.push(DocumentSymbol {
            name,
            detail,
            kind,
            tags: None,
            deprecated: None,
            range: document
                .line_index()
                .range_from_byte_span(range.start(), range.end()),
            selection_range: document
                .line_index()
                .range_from_byte_span(selection.start(), selection.end()),
            children: None,
        });
    }
    DocumentSymbolResponse::Nested(symbols)
}

fn exact_role_details(
    profile: &LspProfile,
    document: &DocumentSnapshot,
) -> BTreeMap<(usize, usize), Vec<String>> {
    let Some(accepted) = profile.accepted_environment() else {
        return BTreeMap::new();
    };
    let project = accepted.project();
    let Some(source) = project.sources().by_uri(document.uri()) else {
        return BTreeMap::new();
    };
    if source.document().text() != document.text() {
        return BTreeMap::new();
    }
    let identity = source.document().identity();
    project
        .hir_project()
        .callable_signature_sources()
        .filter(|source| source.name_span().source() == identity)
        .map(|source| {
            let range = source.name_span().range();
            (
                (range.start(), range.end()),
                binding_annotations(
                    project.semantic_index(),
                    &EntryToolSymbol::Callable(source.declaration().clone()),
                ),
            )
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn workspace_symbols(
    profile: &LspProfile,
    query: &str,
    encoding: PositionEncoding,
) -> Option<WorkspaceSymbolResponse> {
    workspace_symbols_for_profiles(std::iter::once(profile), query, encoding)
}

pub(crate) fn workspace_symbols_for_profiles<'a>(
    profiles: impl IntoIterator<Item = &'a LspProfile>,
    query: &str,
    encoding: PositionEncoding,
) -> Option<WorkspaceSymbolResponse> {
    let query = query.to_ascii_lowercase();
    let mut accepted_profile_found = false;
    let mut symbols = BTreeMap::new();
    for profile in profiles {
        let Some(accepted) = profile.accepted_environment() else {
            continue;
        };
        accepted_profile_found = true;
        let project = accepted.project();
        for source in project
            .hir_project()
            .callable_signature_sources()
            .filter(|source| {
                source
                    .declaration()
                    .name()
                    .to_ascii_lowercase()
                    .contains(&query)
            })
        {
            let symbol = EntryToolSymbol::Callable(source.declaration().clone());
            let bindings = binding_annotations(project.semantic_index(), &symbol);
            let module = source
                .declaration()
                .module()
                .segments()
                .iter()
                .map(arcweft_lang_syntax::ast::module_path::ModuleSegment::as_str)
                .collect::<Vec<_>>()
                .join(".");
            let location = source_location(project, source.name_span(), encoding)?;
            let key = (
                source.declaration().clone(),
                location.uri.to_string(),
                location.range.start.line,
                location.range.start.character,
                location.range.end.line,
                location.range.end.character,
            );
            symbols.insert(
                key,
                WorkspaceSymbol {
                    name: source.declaration().name().to_owned(),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    container_name: Some(if bindings.is_empty() {
                        module
                    } else if module.is_empty() {
                        bindings.join("; ")
                    } else {
                        format!("{module} — {}", bindings.join("; "))
                    }),
                    location: OneOf::Left(location),
                    data: None,
                },
            );
        }
    }
    accepted_profile_found.then(|| WorkspaceSymbolResponse::Nested(symbols.into_values().collect()))
}

pub(crate) fn callable_completions(profile: &LspProfile) -> Vec<CompletionItem> {
    let Some(accepted) = profile.accepted_environment() else {
        return Vec::new();
    };
    let project = accepted.project();
    project
        .hir_project()
        .callable_signature_sources()
        .map(|source| {
            let symbol = EntryToolSymbol::Callable(source.declaration().clone());
            let bindings = binding_annotations(project.semantic_index(), &symbol);
            let signature = source_text(project, source.signature_span()).unwrap_or_default();
            let detail = if bindings.is_empty() {
                signature.to_owned()
            } else {
                format!("{signature} — {}", bindings.join("; "))
            };
            CompletionItem {
                label: source.declaration().name().to_owned(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(detail.clone()),
                documentation: Some(Documentation::String(detail)),
                ..CompletionItem::default()
            }
        })
        .collect()
}

pub(super) fn binding_annotations(
    index: &ProjectSemanticIndex,
    symbol: &EntryToolSymbol,
) -> Vec<String> {
    index
        .entry_role_edges()
        .iter()
        .filter(|edge| match (symbol, edge.target()) {
            (
                EntryToolSymbol::Callable(expected),
                ProjectEntryRoleTarget::Callable { declaration, .. },
            ) => expected == declaration,
            (EntryToolSymbol::Nominal(expected), ProjectEntryRoleTarget::Nominal { key, .. }) => {
                expected == key
            }
            (EntryToolSymbol::Flow(expected), ProjectEntryRoleTarget::Flow { id, .. }) => {
                expected == id
            }
            _ => false,
        })
        .map(|edge| {
            format!(
                "bound as `{}` by entry `@{}`",
                edge.role().as_str(),
                edge.entry().public_id()
            )
        })
        .collect()
}
