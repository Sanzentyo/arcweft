//! Ordinary outline, completion, and workspace-symbol presentation for entry role edges.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    identity::ItemId,
    item::{HirEntryKind, HirItemKind},
    leaf::{HirIdRef, HirIdRefValue},
    module::HirModule,
    source_index::{
        HirCallableSourceOwner, HirCallableSourceRole, HirDeclarationSourceRole,
        HirEntrySourcePart, HirFlowSourceRole, HirItemSourceRole, HirSourcePresence,
        HirSourceQuery, HirSourceSite,
    },
    symbol::CallableSymbol,
};
use arcweft_lang_sema::project_index::{ProjectEntryRoleTarget, ProjectSemanticIndex};
use arcweft_source::SourceSpan;
use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{
    CompletionItem, CompletionItemKind, DocumentSymbol, DocumentSymbolResponse, Documentation,
    OneOf, SymbolKind, WorkspaceSymbol, WorkspaceSymbolResponse,
};

use super::{EntryToolSymbol, entry_tool_symbol_for_declaration, source_location, source_text};
use crate::{documents::DocumentSnapshot, positions::PositionEncoding, profiles::LspProfile};

#[allow(
    deprecated,
    reason = "the LSP wire type still carries the legacy field"
)]
#[expect(
    clippy::too_many_lines,
    reason = "document-symbol publication exhaustively maps the closed entry-tool symbol families"
)]
pub(crate) fn document_symbols(
    profile: &LspProfile,
    document: &DocumentSnapshot,
) -> DocumentSymbolResponse {
    let Some(accepted) = profile.accepted_environment() else {
        return DocumentSymbolResponse::Nested(Vec::new());
    };
    let index = accepted
        .executable()
        .map(|compiled| compiled.semantic_index().as_ref());
    let project = accepted.project();
    let Some(module) = project.hir_for_open_document(document.uri(), document.source_document())
    else {
        return DocumentSymbolResponse::Nested(Vec::new());
    };
    let mut symbols = Vec::new();
    for owner in module.source_ordered_items() {
        let Ok(item) = module.resolve_item(*owner) else {
            continue;
        };
        let descriptor = (|| match item.kind() {
            HirItemKind::Function(function) => callable_document_symbol(
                project,
                module,
                *owner,
                "fn",
                function.name().resolved()?.as_str(),
                index,
            ),
            HirItemKind::Predicate(predicate) => callable_document_symbol(
                project,
                module,
                *owner,
                "predicate",
                predicate.name().resolved()?.as_str(),
                index,
            ),
            HirItemKind::Proof(proof) => callable_document_symbol(
                project,
                module,
                *owner,
                "proof",
                proof.name().resolved()?.as_str(),
                index,
            ),
            HirItemKind::Struct(item) => named_declaration_symbol(
                module,
                *owner,
                item.name().resolved()?.as_str(),
                SymbolKind::STRUCT,
            ),
            HirItemKind::Enum(item) => named_declaration_symbol(
                module,
                *owner,
                item.name().resolved()?.as_str(),
                SymbolKind::ENUM,
            ),
            HirItemKind::Flow(flow) => {
                let (name, selection_role) = if let Some(name) = flow.identity().name() {
                    (name.as_str().to_owned(), HirFlowSourceRole::Name)
                } else {
                    (
                        flow.identity()
                            .public_id()
                            .and_then(absolute_id)
                            .map(str::to_owned)?,
                        HirFlowSourceRole::PublicId,
                    )
                };
                Some((
                    name,
                    SymbolKind::FUNCTION,
                    item_span(
                        module,
                        *owner,
                        HirItemSourceRole::Flow(HirFlowSourceRole::Whole),
                    )?,
                    item_span(module, *owner, HirItemSourceRole::Flow(selection_role))?,
                    Some("flow".to_owned()),
                ))
            }
            HirItemKind::Entry(entry) => Some((
                entry
                    .id()
                    .value()
                    .and_then(resolved_absolute_id)?
                    .to_owned(),
                SymbolKind::OBJECT,
                item_span(
                    module,
                    *owner,
                    HirItemSourceRole::Entry(HirEntrySourcePart::Whole),
                )?,
                item_span(
                    module,
                    *owner,
                    HirItemSourceRole::Entry(HirEntrySourcePart::Id),
                )?,
                Some(format!("{} entry", entry_kind_label(entry.kind())?)),
            )),
            _ => None,
        })();
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
                .range_from_byte_span(range.range().start(), range.range().end()),
            selection_range: document
                .line_index()
                .range_from_byte_span(selection.range().start(), selection.range().end()),
            children: None,
        });
    }
    DocumentSymbolResponse::Nested(symbols)
}

fn callable_document_symbol(
    project: &crate::profiles::accepted_project::AcceptedProjectSnapshot,
    module: &HirModule,
    owner: ItemId,
    source_label: &str,
    name: &str,
    index: Option<&ProjectSemanticIndex>,
) -> Option<(String, SymbolKind, SourceSpan, SourceSpan, Option<String>)> {
    let whole = item_span(
        module,
        owner,
        HirItemSourceRole::Declaration(HirDeclarationSourceRole::Whole),
    )?;
    let selection = item_span(
        module,
        owner,
        HirItemSourceRole::Callable(HirCallableSourceRole::Name {
            owner: HirCallableSourceOwner::Item,
        }),
    )?;
    let signature = item_span(
        module,
        owner,
        HirItemSourceRole::Callable(HirCallableSourceRole::Signature {
            owner: HirCallableSourceOwner::Item,
        }),
    )
    .and_then(|span| source_text(project, &span))
    .unwrap_or_default();
    let bindings = project
        .project_symbols()
        .callable_symbols()
        .find(|symbol| {
            symbol.source_item() == owner && symbol.source_owner() == HirCallableSourceOwner::Item
        })
        .map(|symbol| {
            let Some(binding_symbol) =
                entry_tool_symbol_for_declaration(index, symbol.declaration())
            else {
                return Vec::new();
            };
            index.map_or_else(Vec::new, |index| {
                binding_annotations(index, &binding_symbol)
            })
        })
        .unwrap_or_default();
    let mut detail = format!("{source_label} {signature}");
    if !bindings.is_empty() {
        detail.push_str(" — ");
        detail.push_str(&bindings.join("; "));
    }
    Some((
        name.to_owned(),
        SymbolKind::FUNCTION,
        whole,
        selection,
        Some(detail),
    ))
}

fn named_declaration_symbol(
    module: &HirModule,
    owner: ItemId,
    name: &str,
    kind: SymbolKind,
) -> Option<(String, SymbolKind, SourceSpan, SourceSpan, Option<String>)> {
    Some((
        name.to_owned(),
        kind,
        item_span(
            module,
            owner,
            HirItemSourceRole::Declaration(HirDeclarationSourceRole::Whole),
        )?,
        item_span(
            module,
            owner,
            HirItemSourceRole::Declaration(HirDeclarationSourceRole::Name),
        )?,
        None,
    ))
}

fn item_span(module: &HirModule, owner: ItemId, role: HirItemSourceRole) -> Option<SourceSpan> {
    let lookup = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Item { owner, role },
        )
        .ok()?;
    let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence() else {
        return None;
    };
    Some(span.clone())
}

fn callable_signature_span(
    project: &crate::profiles::accepted_project::AcceptedProjectSnapshot,
    symbol: &CallableSymbol,
) -> Option<SourceSpan> {
    let module = project
        .hir_project()
        .view()
        .module(symbol.declaration().module())?;
    if module.snapshot_id() != symbol.source_snapshot() {
        return None;
    }
    item_span(
        module,
        symbol.source_item(),
        HirItemSourceRole::Callable(HirCallableSourceRole::Signature {
            owner: symbol.source_owner(),
        }),
    )
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

fn entry_kind_label(kind: &HirEntryKind) -> Option<&str> {
    match kind {
        HirEntryKind::Game => Some("game"),
        HirEntryKind::Editor => Some("editor"),
        HirEntryKind::Cli => Some("cli"),
        HirEntryKind::Server => Some("server"),
        HirEntryKind::Activity => Some("activity"),
        HirEntryKind::Test => Some("test"),
        HirEntryKind::Bench => Some("bench"),
        HirEntryKind::Agent => Some("agent"),
        HirEntryKind::Custom(name) => Some(name.as_str()),
        HirEntryKind::Recovered(_) => None,
    }
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
        let index = accepted
            .executable()
            .map(|compiled| compiled.semantic_index().as_ref());
        for source in project
            .project_symbols()
            .callable_symbols()
            .filter(|source| {
                source
                    .declaration()
                    .name()
                    .to_ascii_lowercase()
                    .contains(&query)
            })
        {
            let binding_symbol = entry_tool_symbol_for_declaration(index, source.declaration());
            let bindings = index.map_or_else(Vec::new, |index| {
                binding_symbol
                    .as_ref()
                    .map_or_else(Vec::new, |symbol| binding_annotations(index, symbol))
            });
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
    let index = accepted
        .executable()
        .map(|compiled| compiled.semantic_index().as_ref());
    project
        .project_symbols()
        .callable_symbols()
        .filter_map(|source| {
            let symbol = entry_tool_symbol_for_declaration(index, source.declaration())?;
            if !matches!(&symbol, EntryToolSymbol::Callable(_)) {
                return None;
            }
            let bindings = index.map_or_else(Vec::new, |index| binding_annotations(index, &symbol));
            let signature = callable_signature_span(project, source)
                .and_then(|span| source_text(project, &span))
                .unwrap_or_default();
            let detail = if bindings.is_empty() {
                signature.to_owned()
            } else {
                format!("{signature} — {}", bindings.join("; "))
            };
            Some(CompletionItem {
                label: source.declaration().name().to_owned(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(detail.clone()),
                documentation: Some(Documentation::String(detail)),
                ..CompletionItem::default()
            })
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
            ) => {
                expected
                    == &arcweft_lang_hir::symbol::CallableDeclarationKey::Existing(
                        declaration.clone(),
                    )
            }
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
