//! Accepted-project nominal navigation and refactoring from typed resolver edges.

use std::{
    collections::{BTreeSet, HashMap},
    fmt::Write,
};

use arcweft_lang_hir::symbol::{
    ProjectTypeTarget,
    nominal::{ProjectNominalDeclarationId, ProjectNominalDeclarationKind},
};
use arcweft_lang_sema::{
    env::nominal::{AcceptedNominalId, AcceptedNominalOwnerId},
    nominal::{
        BuiltinTypeConstructor, ExternalNominalResolution, TypeArgumentExpectation,
        TypeNameResolution,
    },
    registration::{RegisteredExternalOwner, RegisteredExternalOwnerKind},
    types::{AcceptedNominalType, EntityKind, TypeKind},
};
use arcweft_lang_syntax::ast::module_path::ModuleSegment;
use arcweft_source::SourceSpan;
use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{
    CompletionItem, CompletionItemKind, Documentation, GotoDefinitionResponse, Hover,
    HoverContents, Location, MarkedString, PrepareRenameResponse, TextEdit, Uri, WorkspaceEdit,
};

use crate::{
    documents::{DocumentSnapshot, DocumentStore},
    profiles::{
        LspProfile, accepted_project::AcceptedProjectSnapshot, state::AcceptedProfileEnvironment,
    },
};

#[derive(Clone)]
struct NominalCursor {
    declaration: ProjectNominalDeclarationId,
    source: SourceSpan,
    terminal_source: SourceSpan,
    arguments: Box<[TypeKind]>,
    normalized: Option<TypeKind>,
    alias_trace: Box<[ProjectNominalDeclarationId]>,
}

#[derive(Clone)]
enum LanguageNominalOwner {
    Builtin(BuiltinTypeConstructor),
    EntityFamily(EntityKind),
}

#[derive(Clone)]
struct LanguageNominalCursor {
    owner: LanguageNominalOwner,
    source: SourceSpan,
    normalized: Option<TypeKind>,
}

#[derive(Clone)]
struct AcceptedNominalCursor {
    nominal: AcceptedNominalType,
    source: SourceSpan,
}

pub(crate) fn hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<Hover> {
    let accepted = profile.accepted_environment()?;
    let project = exact_project(accepted.project(), document)?;
    if let Some(cursor) = language_symbol_at(project, document, offset) {
        return Some(language_nominal_hover(document, cursor));
    }
    if let Some(cursor) = accepted_nominal_at(project, document, offset) {
        return accepted_nominal_hover(&accepted, document, &cursor);
    }
    let cursor = symbol_at(project, document, offset)?;
    project_nominal_hover(project, document, cursor)
}

fn language_nominal_hover(document: &DocumentSnapshot, cursor: LanguageNominalCursor) -> Hover {
    let text = match cursor.owner {
        LanguageNominalOwner::Builtin(constructor) => {
            let mut text = format!(
                "language-owned type constructor `{}<EntityFamily>`",
                constructor.spelling()
            );
            if let Some(normalized) = cursor
                .normalized
                .filter(|normalized| !matches!(normalized, TypeKind::Error(_)))
            {
                write!(text, "\n\nnormalized type: `{}`", normalized.source_label())
                    .expect("writing to a String cannot fail");
            }
            text
        }
        LanguageNominalOwner::EntityFamily(family) => format!(
            "entity family `{}`",
            family
                .authored_type_name()
                .expect("resolver facts expose only authored fixed families")
        ),
    };
    hover_at(document, &cursor.source, text)
}

fn accepted_nominal_hover(
    accepted: &AcceptedProfileEnvironment,
    document: &DocumentSnapshot,
    cursor: &AcceptedNominalCursor,
) -> Option<Hover> {
    let environment = accepted.world().environment();
    let id = cursor.nominal.declaration();
    let record = environment
        .nominal_catalog()
        .exact(id.canonical_path())
        .filter(|record| record.id() == id)?;
    let metadata = environment.rust_metadata().get(id);
    let mut text = if metadata.is_some() {
        format!("accepted Rust nominal `{}`", id.source_label())
    } else {
        format!("accepted environment nominal `{}`", id.source_label())
    };
    write!(
        text,
        "\n\nowner: `{}`\n\nmounted path: `{}`\n\narity: `{}`",
        id.owner().source_label(),
        id.canonical_path().canonical_string(),
        record.arity()
    )
    .expect("writing to a String cannot fail");
    if let Some(metadata) = metadata {
        write!(
            text,
            "\n\nRust package: `{}`\n\nRust item: `{}`",
            metadata.package(),
            metadata.rust_item().as_str()
        )
        .expect("writing to a String cannot fail");
    }
    if !cursor.nominal.arguments().is_empty() {
        let arguments = cursor
            .nominal
            .arguments()
            .iter()
            .map(TypeKind::source_label)
            .collect::<Vec<_>>()
            .join(", ");
        write!(text, "\n\napplied arguments: `{arguments}`")
            .expect("writing to a String cannot fail");
    }
    Some(hover_at(document, &cursor.source, text))
}

fn project_nominal_hover(
    project: &AcceptedProjectSnapshot,
    document: &DocumentSnapshot,
    cursor: NominalCursor,
) -> Option<Hover> {
    let record = project
        .semantic_index()
        .project_nominal(&cursor.declaration)?;
    let declaration = record.declaration();
    let type_parameters = declaration
        .type_parameters()
        .iter()
        .map(|parameter| parameter.name().as_str())
        .collect::<Vec<_>>();
    let applied = cursor
        .arguments
        .iter()
        .map(TypeKind::source_label)
        .collect::<Vec<_>>();
    let mut text = format!(
        "{} `{}`",
        declaration_kind_label(declaration.id().kind()),
        declaration.id().qualified_name()
    );
    if !type_parameters.is_empty() {
        write!(
            text,
            "\n\ntype parameters: `{}`",
            type_parameters.join(", ")
        )
        .expect("writing to a String cannot fail");
    }
    if !applied.is_empty() {
        write!(text, "\n\napplied arguments: `{}`", applied.join(", "))
            .expect("writing to a String cannot fail");
    }
    if let Some(normalized) = cursor.normalized {
        write!(text, "\n\nnormalized type: `{}`", normalized.source_label())
            .expect("writing to a String cannot fail");
    }
    if !cursor.alias_trace.is_empty() {
        text.push_str("\n\nalias expansion:");
        for alias in cursor.alias_trace {
            write!(text, "\n- `{}`", alias.qualified_name())
                .expect("writing to a String cannot fail");
        }
    }
    if !record.poisons().is_empty() {
        write!(
            text,
            "\n\nchecked with {} retained poison fact(s)",
            record.poisons().len()
        )
        .expect("writing to a String cannot fail");
    }
    Some(hover_at(document, &cursor.source, text))
}

fn hover_at(document: &DocumentSnapshot, source: &SourceSpan, text: String) -> Hover {
    Hover {
        contents: HoverContents::Scalar(MarkedString::String(text)),
        range: Some(
            document
                .line_index()
                .range_from_byte_span(source.range().start(), source.range().end()),
        ),
    }
}

pub(crate) fn definition(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<GotoDefinitionResponse> {
    let accepted = profile.accepted_environment()?;
    let project = exact_project(accepted.project(), document)?;
    if language_symbol_at(project, document, offset).is_some() {
        return None;
    }
    if let Some(cursor) = accepted_nominal_at(project, document, offset) {
        let environment = accepted.world().environment();
        let id = cursor.nominal.declaration();
        let source = environment
            .rust_metadata()
            .get(id)
            .map(arcweft_lang_sema::env::AcceptedRustTypeMetadata::source)
            .or_else(|| {
                environment
                    .nominal_world()
                    .visibility()
                    .visible(id)
                    .map(arcweft_lang_sema::registration::AcceptedNominalSource::declaration)
            })
            .or_else(|| {
                environment
                    .nominal_catalog()
                    .exact(id.canonical_path())
                    .filter(|record| record.id() == id)
                    .and_then(|record| record.source())
            })?;
        return location(project, source).map(GotoDefinitionResponse::Scalar);
    }
    let cursor = symbol_at(project, document, offset)?;
    let source = project
        .semantic_index()
        .project_nominal(&cursor.declaration)?
        .declaration()
        .source()
        .name();
    location(project, source).map(GotoDefinitionResponse::Scalar)
}

pub(crate) fn references(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<Vec<Location>> {
    let accepted = profile.accepted_environment()?;
    let project = exact_project(accepted.project(), document)?;
    if language_symbol_at(project, document, offset).is_some() {
        return Some(Vec::new());
    }
    let cursor = symbol_at(project, document, offset)?;
    let index = project.semantic_index();
    let mut spans = BTreeSet::new();
    spans.insert(
        index
            .project_nominal(&cursor.declaration)?
            .declaration()
            .source()
            .name()
            .clone(),
    );
    spans.extend(
        index
            .project_nominal_references()
            .iter()
            .filter(|edge| edge.declaration() == &cursor.declaration)
            .map(|edge| edge.source().clone()),
    );
    for module in accepted.world().symbols().modules() {
        for binding in accepted.world().symbols().visible_type_bindings(module) {
            if matches!(binding.target(), ProjectTypeTarget::Nominal(declaration) if declaration.id() == &cursor.declaration)
            {
                spans.extend(binding.reference_sites().iter().cloned());
            }
        }
    }
    Some(
        spans
            .iter()
            .filter_map(|span| location(project, span))
            .collect(),
    )
}

pub(crate) fn completions(
    profile: &LspProfile,
    document: &DocumentSnapshot,
) -> Vec<CompletionItem> {
    let Some(accepted) = profile.accepted_environment() else {
        return Vec::new();
    };
    let Some(project) = exact_project(accepted.project(), document) else {
        return Vec::new();
    };
    let Some(source) = project.sources().by_uri(document.uri()) else {
        return Vec::new();
    };
    let Some(module) = project.module_key(source.document().identity()) else {
        return Vec::new();
    };
    let environment = accepted.world().environment();
    let mut items = accepted
        .world()
        .symbols()
        .visible_type_bindings(module.module())
        .map(|binding| match binding.target() {
            ProjectTypeTarget::Nominal(declaration) => CompletionItem {
                label: binding.spelling().to_string(),
                kind: Some(completion_kind(declaration.id().kind())),
                detail: Some(declaration.id().qualified_name()),
                documentation: Some(Documentation::String(format!(
                    "Accepted project {} with {} type parameter(s).",
                    declaration_kind_label(declaration.id().kind()),
                    declaration.type_parameters().len()
                ))),
                ..CompletionItem::default()
            },
            ProjectTypeTarget::External(external) => {
                let detail = environment
                    .external_owner(
                        accepted.world().symbols(),
                        external.declaration(),
                        RegisteredExternalOwnerKind::Environment,
                    )
                    .ok()
                    .and_then(|owner| match owner {
                        RegisteredExternalOwner::Environment(owner) => {
                            environment.environment_binding(owner.value_binding())
                        }
                        RegisteredExternalOwner::Character(_) => None,
                    })
                    .and_then(|ty| match ty {
                        TypeKind::AcceptedNominal(nominal) => {
                            Some(nominal.declaration().source_label())
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| external.canonical_path().to_string());
                CompletionItem {
                    label: binding.spelling().to_string(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(detail),
                    documentation: Some(Documentation::String(
                        "Accepted source-backed external type.".to_owned(),
                    )),
                    ..CompletionItem::default()
                }
            }
        })
        .collect::<Vec<_>>();
    items.extend(
        environment
            .nominal_catalog()
            .exact_records()
            .filter(|record| {
                accepted_nominal_is_visible(environment.nominal_world().visibility(), record.id())
            })
            .map(|record| CompletionItem {
                label: record.id().canonical_path().canonical_string(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(record.id().source_label()),
                documentation: Some(Documentation::String(format!(
                    "Accepted environment type with arity {}.",
                    record.arity()
                ))),
                ..CompletionItem::default()
            }),
    );
    items.extend(
        BuiltinTypeConstructor::ALL
            .iter()
            .copied()
            .map(|constructor| CompletionItem {
                label: constructor.spelling().to_owned(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(format!(
                    "language-owned type constructor (arity {})",
                    constructor.arity()
                )),
                documentation: Some(Documentation::String(
                    "Arcweft language-owned type constructor.".to_owned(),
                )),
                ..CompletionItem::default()
            }),
    );
    items
}

fn accepted_nominal_is_visible(
    visibility: &arcweft_lang_sema::registration::AcceptedNominalVisibilityIndex,
    id: &AcceptedNominalId,
) -> bool {
    match id.owner() {
        AcceptedNominalOwnerId::Environment(_) | AcceptedNominalOwnerId::RustPackage(_) => {
            visibility.visible(id).is_some()
        }
        AcceptedNominalOwnerId::Standard | AcceptedNominalOwnerId::Character(_) => true,
    }
}

pub(crate) fn contextual_completions(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Vec<CompletionItem> {
    let Some(accepted) = profile.accepted_environment() else {
        return Vec::new();
    };
    let Some(project) = exact_project(accepted.project(), document) else {
        return Vec::new();
    };
    let identity = project
        .sources()
        .by_uri(document.uri())
        .map(|source| source.document().identity());
    let Some(identity) = identity else {
        return Vec::new();
    };
    let is_entity_family_slot = project
        .typecheck()
        .nominal_resolutions
        .nodes()
        .any(|(_, node)| {
            let TypeNameResolution::Builtin(constructor) = node.outcome() else {
                return false;
            };
            constructor.argument_expectation(0) == Some(TypeArgumentExpectation::EntityFamily)
                && node
                    .source()
                    .project()
                    .is_some_and(|source| span_contains_offset(source, identity, offset))
                && !node
                    .terminal_source()
                    .and_then(|source| source.project())
                    .is_some_and(|source| span_contains_offset(source, identity, offset))
        });
    if !is_entity_family_slot {
        return Vec::new();
    }
    EntityKind::AUTHORED_FAMILIES
        .iter()
        .filter_map(EntityKind::authored_type_name)
        .map(|name| CompletionItem {
            label: name.to_owned(),
            kind: Some(CompletionItemKind::TYPE_PARAMETER),
            detail: Some("Arcweft entity family".to_owned()),
            documentation: Some(Documentation::String(
                "Fixed language-owned entity family accepted by this constructor.".to_owned(),
            )),
            ..CompletionItem::default()
        })
        .collect()
}

pub(crate) fn prepare_rename(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<PrepareRenameResponse> {
    let accepted = profile.accepted_environment()?;
    let project = exact_project(accepted.project(), document)?;
    if language_symbol_at(project, document, offset).is_some() {
        return None;
    }
    let cursor = symbol_at(project, document, offset)?;
    let record = project
        .semantic_index()
        .project_nominal(&cursor.declaration)?;
    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: document.line_index().range_from_byte_span(
            cursor.terminal_source.range().start(),
            cursor.terminal_source.range().end(),
        ),
        placeholder: record.id().name().as_str().to_owned(),
    })
}

#[allow(
    clippy::mutable_key_type,
    reason = "LSP WorkspaceEdit requires Uri keys and the accepted snapshot provides stale guards"
)]
pub(crate) fn rename(
    profile: &LspProfile,
    documents: &DocumentStore,
    document: &DocumentSnapshot,
    offset: usize,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let new_name = ModuleSegment::new(new_name).ok()?;
    let accepted = profile.accepted_environment()?;
    let project = exact_project(accepted.project(), document)?;
    if language_symbol_at(project, document, offset).is_some() {
        return None;
    }
    let cursor = symbol_at(project, document, offset)?;
    let index = project.semantic_index();
    if index.project_nominals().keys().any(|candidate| {
        candidate != &cursor.declaration
            && candidate.module() == cursor.declaration.module()
            && candidate.owner_path() == cursor.declaration.owner_path()
            && candidate.name() == &new_name
    }) {
        return None;
    }
    let mut edits = BTreeSet::<SourceSpan>::new();
    edits.insert(
        index
            .project_nominal(&cursor.declaration)?
            .declaration()
            .source()
            .name()
            .clone(),
    );
    for edge in index
        .project_nominal_references()
        .iter()
        .filter(|edge| edge.declaration() == &cursor.declaration)
        .filter(|edge| {
            edge.use_path()
                .segments()
                .last()
                .is_some_and(|segment| segment.as_str() == cursor.declaration.name().as_str())
        })
    {
        edits.insert(edge.terminal_source().clone());
    }
    for module in accepted.world().symbols().modules() {
        for binding in accepted.world().symbols().visible_type_bindings(module) {
            if matches!(binding.target(), ProjectTypeTarget::Nominal(declaration) if declaration.id() == &cursor.declaration)
            {
                edits.extend(binding.reference_sites().iter().cloned());
            }
        }
    }

    let mut changes = HashMap::<Uri, Vec<TextEdit>>::new();
    for span in edits {
        push_edit(project, &mut changes, &span, new_name.as_str())?;
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

fn exact_project<'a>(
    project: &'a AcceptedProjectSnapshot,
    document: &DocumentSnapshot,
) -> Option<&'a AcceptedProjectSnapshot> {
    project
        .sources()
        .by_uri(document.uri())
        .is_some_and(|source| source.document().text() == document.text())
        .then_some(project)
}

fn symbol_at(
    project: &AcceptedProjectSnapshot,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<NominalCursor> {
    let identity = project
        .sources()
        .by_uri(document.uri())?
        .document()
        .identity();
    if let Some(record) = project
        .semantic_index()
        .project_nominals()
        .values()
        .find(|record| span_contains_offset(record.declaration().source().name(), identity, offset))
    {
        return Some(NominalCursor {
            declaration: record.id().clone(),
            source: record.declaration().source().name().clone(),
            terminal_source: record.declaration().source().name().clone(),
            arguments: Box::new([]),
            normalized: None,
            alias_trace: Box::new([]),
        });
    }
    project
        .semantic_index()
        .project_nominal_references()
        .iter()
        .filter(|edge| span_contains_offset(edge.source(), identity, offset))
        .min_by_key(|edge| edge.source().range().end() - edge.source().range().start())
        .map(|edge| NominalCursor {
            declaration: edge.declaration().clone(),
            source: edge.source().clone(),
            terminal_source: edge.terminal_source().clone(),
            arguments: edge.arguments().to_vec().into_boxed_slice(),
            normalized: Some(edge.normalized().clone()),
            alias_trace: edge
                .alias_expansions()
                .iter()
                .map(|fact| fact.alias().clone())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        })
}

fn language_symbol_at(
    project: &AcceptedProjectSnapshot,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<LanguageNominalCursor> {
    let identity = project
        .sources()
        .by_uri(document.uri())?
        .document()
        .identity();
    project
        .typecheck()
        .nominal_resolutions
        .nodes()
        .filter_map(|(_, node)| {
            let (owner, source) = match node.outcome() {
                TypeNameResolution::Builtin(constructor)
                    if BuiltinTypeConstructor::ENTITY_FAMILY_PROJECTIONS.contains(constructor) =>
                {
                    (
                        LanguageNominalOwner::Builtin(*constructor),
                        node.terminal_source()?.project()?,
                    )
                }
                TypeNameResolution::EntityFamily(family) => (
                    LanguageNominalOwner::EntityFamily(family.clone()),
                    node.terminal_source()
                        .and_then(|source| source.project())
                        .or_else(|| node.source().project())?,
                ),
                _ => return None,
            };
            span_contains_offset(source, identity, offset).then(|| LanguageNominalCursor {
                owner,
                source: source.clone(),
                normalized: node.recovered().cloned(),
            })
        })
        .min_by_key(|cursor| cursor.source.range().end() - cursor.source.range().start())
}

fn accepted_nominal_at(
    project: &AcceptedProjectSnapshot,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<AcceptedNominalCursor> {
    let identity = project
        .sources()
        .by_uri(document.uri())?
        .document()
        .identity();
    project
        .typecheck()
        .nominal_resolutions
        .nodes()
        .filter_map(|(_, node)| {
            let (TypeNameResolution::Accepted(nominal)
            | TypeNameResolution::External(ExternalNominalResolution::Accepted {
                nominal, ..
            })) = node.outcome()
            else {
                return None;
            };
            let source = node
                .terminal_source()
                .and_then(|source| source.project())
                .filter(|source| span_contains_offset(source, identity, offset))
                .or_else(|| {
                    node.source()
                        .project()
                        .filter(|source| span_contains_offset(source, identity, offset))
                })?;
            Some(AcceptedNominalCursor {
                nominal: nominal.clone(),
                source: source.clone(),
            })
        })
        .min_by_key(|cursor| cursor.source.range().end() - cursor.source.range().start())
}

fn span_contains_offset(
    span: &SourceSpan,
    identity: &arcweft_source::SourceDocumentIdentity,
    offset: usize,
) -> bool {
    span.source() == identity && span.range().start() <= offset && offset < span.range().end()
}

fn location(project: &AcceptedProjectSnapshot, span: &SourceSpan) -> Option<Location> {
    let source = project.source(span.source())?;
    Some(Location::new(
        source.locator().uri()?.clone(),
        source
            .line_index()
            .range_from_byte_span(span.range().start(), span.range().end()),
    ))
}

#[allow(
    clippy::mutable_key_type,
    reason = "LSP WorkspaceEdit requires Uri keys"
)]
fn push_edit(
    project: &AcceptedProjectSnapshot,
    changes: &mut HashMap<Uri, Vec<TextEdit>>,
    span: &SourceSpan,
    replacement: &str,
) -> Option<()> {
    let source = project.source(span.source())?;
    let uri = source.locator().uri()?.clone();
    let range = source
        .line_index()
        .range_from_byte_span(span.range().start(), span.range().end());
    changes
        .entry(uri)
        .or_default()
        .push(TextEdit::new(range, replacement.to_owned()));
    Some(())
}

const fn completion_kind(kind: ProjectNominalDeclarationKind) -> CompletionItemKind {
    match kind {
        ProjectNominalDeclarationKind::Struct => CompletionItemKind::STRUCT,
        ProjectNominalDeclarationKind::Enum => CompletionItemKind::ENUM,
        ProjectNominalDeclarationKind::TypeAlias => CompletionItemKind::TYPE_PARAMETER,
    }
}

const fn declaration_kind_label(kind: ProjectNominalDeclarationKind) -> &'static str {
    match kind {
        ProjectNominalDeclarationKind::Struct => "struct",
        ProjectNominalDeclarationKind::Enum => "enum",
        ProjectNominalDeclarationKind::TypeAlias => "type alias",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use arcweft_adapter_context::manifest::{
        AdapterManifest, AdapterNominalDeclaration, AdapterNominalPath, AdapterNominalPathPrefix,
        AdapterNominalPathSegment, AdapterNominalVisibility, AdapterRegistry,
    };
    use arcweft_launch::LaunchProfileSelection;
    use arcweft_project_loader::topology::{
        ProfileTopologyLoadRequest, ProfileTopologyOwnerId, load_profile_topology,
    };
    use arcweft_runtime_host::RuntimeHostRunnerKind;
    use arcweft_rust_abi::{
        ArcweftRustManifest, ArcweftRustPackage, ArcweftRustPackageId, ArcweftRustTypeDecl,
        ArcweftRustTypeKind, ArcweftRustTypeParameter, ArcweftRustTypeParameterIndex,
        ArcweftRustTypeParameterName, ArcweftRustTypePath, ArcweftRustTypePathSegment,
        ArcweftRustTypeRef, ArcweftRustVariant, ArcweftRustVariantPayload,
    };
    use lsp_types::{
        DidOpenTextDocumentParams, GotoDefinitionResponse, HoverContents, MarkedString,
        TextDocumentItem, Uri,
    };

    use super::*;
    use crate::{
        diagnostics::{DocumentAnalysis, publish_diagnostics_from_analysis},
        positions::PositionEncoding,
        profiles::{
            LspProfile, LspProfileResolver, register_loaded_environment, state::AcceptedOverlaySet,
        },
    };

    const MAIN: &str = r"
use crate.models.Record as LocalRecord

fn smoke() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

fn identity(value: LocalRecord) -> crate.models.Record {
    value
}

entry agent @entry.agent.main {
    controller = smoke
}
";

    const MODELS: &str = r"
mod crate.models

pub struct Record {}
";

    const ALIAS_MODELS: &str = r"
mod crate.models

pub struct Record {}

pub type PublicAlias = Record
";

    const ALIAS_MAIN: &str = r"
use crate.models.PublicAlias as ImportedAlias

fn smoke() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

fn identity(value: ImportedAlias) -> crate.models.PublicAlias {
    value
}

entry agent @entry.agent.main {
    controller = smoke
}
";

    #[test]
    fn accepted_nominal_features_share_exact_resolver_edges() {
        let project = TestProject::new("nominal-tooling");
        project.write_manifest();
        project.write("src/main.arcw", MAIN);
        project.write("src/models.arcw", MODELS);
        let main_path = project.path("src/main.arcw");
        let profile =
            LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("agent".to_owned()))
                .resolve_for_document_path(&main_path);
        assert!(
            profile.diagnostics().is_empty(),
            "{:?}",
            profile.diagnostics()
        );
        let document = open(&main_path, MAIN);
        let analysis = DocumentAnalysis::analyze_project(
            MAIN,
            PositionEncoding::Utf16,
            &profile,
            document.uri(),
        );
        let published = publish_diagnostics_from_analysis(&document, &profile, &analysis);
        assert!(published.diagnostics.iter().all(|diagnostic| {
            !matches!(
                diagnostic.code.as_ref(),
                Some(lsp_types::NumberOrString::String(code))
                    if code.starts_with("sema.nominal.")
            )
        }));
        let offset = MAIN.find("value: LocalRecord").expect("alias type use") + "value: ".len() + 1;

        let GotoDefinitionResponse::Scalar(location) =
            definition(&profile, &document, offset).expect("nominal definition")
        else {
            panic!("expected one nominal definition");
        };
        assert!(location.uri.as_str().ends_with("/src/models.arcw"));
        let HoverContents::Scalar(MarkedString::String(text)) = hover(&profile, &document, offset)
            .expect("nominal hover")
            .contents
        else {
            panic!("expected nominal string hover");
        };
        assert!(text.contains("struct `models.Record`"));
        assert!(
            completions(&profile, &document)
                .iter()
                .any(|item| item.label == "LocalRecord")
        );
        assert_eq!(
            references(&profile, &document, offset)
                .expect("nominal references")
                .len(),
            4,
            "declaration, import target, alias type use, and qualified type use"
        );

        let edit = rename(
            &profile,
            &DocumentStore::default(),
            &document,
            offset,
            "Renamed",
        )
        .expect("nominal rename edits");
        assert_eq!(
            edit.changes
                .as_ref()
                .expect("nominal rename changes")
                .values()
                .map(Vec::len)
                .sum::<usize>(),
            3
        );
        let main_edits = edit
            .changes
            .as_ref()
            .and_then(|changes| changes.get(document.uri()))
            .expect("main source edits");
        assert_eq!(main_edits.len(), 2);
        let edited_tokens = main_edits
            .iter()
            .filter_map(|edit| {
                document
                    .line_index()
                    .try_byte_offset_from_position(edit.range.start)
                    .ok()
                    .zip(
                        document
                            .line_index()
                            .try_byte_offset_from_position(edit.range.end)
                            .ok(),
                    )
                    .and_then(|(start, end)| MAIN.get(start..end))
            })
            .collect::<Vec<_>>();
        assert_eq!(edited_tokens, ["Record", "Record"]);
    }

    #[test]
    fn accepted_project_alias_tooling_uses_the_alias_declaration_id() {
        const TEST_ID: &str = "TOOL-DEFINITION-ALIAS/TOOL-HOVER-ALIAS/TOOL-RENAME-ALIAS";
        let project = TestProject::new("nominal-alias-tooling");
        project.write_manifest();
        project.write("src/main.arcw", ALIAS_MAIN);
        project.write("src/models.arcw", ALIAS_MODELS);
        let main_path = project.path("src/main.arcw");
        let profile =
            LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("agent".to_owned()))
                .resolve_for_document_path(&main_path);
        assert!(
            profile.diagnostics().is_empty(),
            "{TEST_ID}: accepted project diagnostics: {:?}",
            profile.diagnostics()
        );
        let document = open(&main_path, ALIAS_MAIN);
        let offset = ALIAS_MAIN
            .find("value: ImportedAlias")
            .expect("TOOL-DEFINITION-ALIAS: imported alias type use")
            + "value: ".len()
            + 1;
        let position = document.line_index().position_from_byte_offset(offset);

        let GotoDefinitionResponse::Scalar(location) = crate::features::definition::definition(
            &profile,
            document.uri(),
            &DocumentStore::default(),
            &document,
            position,
        )
        .expect("TOOL-DEFINITION-ALIAS: definition request")
        .expect("TOOL-DEFINITION-ALIAS: alias definition") else {
            panic!("TOOL-DEFINITION-ALIAS: expected scalar alias definition");
        };
        assert!(
            location.uri.as_str().ends_with("/src/models.arcw"),
            "TOOL-DEFINITION-ALIAS: definition URI was {:?}",
            location.uri
        );

        let HoverContents::Scalar(MarkedString::String(text)) =
            crate::features::hover::hover(&profile, &document, position)
                .expect("TOOL-HOVER-ALIAS: alias hover")
                .contents
        else {
            panic!("TOOL-HOVER-ALIAS: expected string hover");
        };
        assert!(
            text.contains("type alias `models.PublicAlias`"),
            "TOOL-HOVER-ALIAS: hover did not identify alias: {text}"
        );
        assert!(
            text.contains("normalized type: `models.Record`"),
            "TOOL-HOVER-ALIAS: hover omitted normalized type: {text}"
        );
        assert!(
            text.contains("alias expansion:\n- `models.PublicAlias`"),
            "TOOL-HOVER-ALIAS: hover omitted typed alias expansion: {text}"
        );

        let labels = crate::features::completion::completions(&profile, Some(&document))
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();
        assert_eq!(
            labels
                .iter()
                .filter(|label| label.as_str() == "ImportedAlias")
                .count(),
            1,
            "TOOL-COMPLETION: imported alias completion must be unique: {labels:?}"
        );

        let references =
            crate::features::references::references(&profile, document.uri(), &document, position);
        assert_eq!(
            references.len(),
            4,
            "TOOL-REPEATED: alias declaration, import target, alias use, and qualified use"
        );

        let edit = crate::features::rename::rename(
            &profile,
            &DocumentStore::default(),
            &document,
            position,
            "RenamedAlias",
        )
        .expect("TOOL-RENAME-ALIAS: alias rename edits");
        assert_eq!(
            edit.changes
                .as_ref()
                .expect("TOOL-RENAME-ALIAS: rename changes")
                .values()
                .map(Vec::len)
                .sum::<usize>(),
            3,
            "TOOL-RENAME-ALIAS: original alias declaration and original-spelling uses only"
        );
    }

    #[test]
    fn entity_family_constructor_tooling_uses_checked_language_owned_nodes() {
        const SOURCE: &str = r"
pub struct RouteInfo {
    route: Ref<Flow>,
    speaker: Ref<Character>,
}

fn smoke() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

entry agent @entry.agent.main {
    controller = smoke
}
";
        let project = TestProject::new("entity-family-tooling");
        project.write_manifest();
        project.write("src/main.arcw", SOURCE);
        let main_path = project.path("src/main.arcw");
        let profile =
            LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("agent".to_owned()))
                .resolve_for_document_path(&main_path);
        assert!(
            profile.diagnostics().is_empty(),
            "accepted project diagnostics: {:?}",
            profile.diagnostics()
        );
        let document = open(&main_path, SOURCE);
        let ref_offset = SOURCE.find("Ref<Flow>").expect("Ref use") + 1;
        let family_offset = SOURCE.find("Flow>").expect("Flow family") + 1;

        let HoverContents::Scalar(MarkedString::String(ref_hover)) =
            hover(&profile, &document, ref_offset)
                .expect("Ref hover")
                .contents
        else {
            panic!("expected Ref string hover");
        };
        assert!(ref_hover.contains("language-owned type constructor `Ref<EntityFamily>`"));
        assert!(ref_hover.contains("normalized type: `Ref<Flow>`"));

        let HoverContents::Scalar(MarkedString::String(family_hover)) =
            hover(&profile, &document, family_offset)
                .expect("entity-family hover")
                .contents
        else {
            panic!("expected entity-family string hover");
        };
        assert_eq!(family_hover, "entity family `Flow`");
        assert!(definition(&profile, &document, ref_offset).is_none());
        assert!(definition(&profile, &document, family_offset).is_none());
        assert_eq!(
            references(&profile, &document, ref_offset),
            Some(Vec::new())
        );
        assert!(prepare_rename(&profile, &document, ref_offset).is_none());
        assert!(prepare_rename(&profile, &document, family_offset).is_none());
        assert!(
            rename(
                &profile,
                &DocumentStore::default(),
                &document,
                family_offset,
                "Scene"
            )
            .is_none()
        );

        let global = crate::features::completion::completions(&profile, Some(&document));
        for constructor in ["Ref", "Speaker", "SpeakerPreset"] {
            assert_eq!(
                global
                    .iter()
                    .filter(|item| item.label == constructor)
                    .count(),
                1,
                "{constructor} is published once from the typed builtin inventory"
            );
        }
        let contextual = crate::features::completion::completions_at(
            &profile,
            Some(&document),
            document
                .line_index()
                .position_from_byte_offset(family_offset),
        );
        for family in ["Character", "Flow", "View"] {
            assert!(
                contextual.iter().any(|item| item.label == family),
                "{family} is an authored entity-family completion"
            );
        }
        assert!(!contextual.iter().any(|item| item.label == "Other"));
    }

    const ACCEPTED_RUST_NOMINAL_SOURCE: &str = r"
fn smoke() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

fn identity(value: Envelope<Rank>) -> Envelope<Rank> {
    value
}

entry agent @entry.agent.main {
    controller = smoke
}
";

    struct AcceptedRustNominalToolingFixture {
        _project: TestProject,
        profile: LspProfile,
        document: DocumentSnapshot,
        offset: usize,
    }

    fn accepted_rust_nominal_tooling_fixture() -> AcceptedRustNominalToolingFixture {
        let project = TestProject::new("accepted-rust-nominal-tooling");
        let adapter = rust_nominal_adapter();
        project.write(
            "arcw.toml",
            r#"schema = 1

[package]
id = "org.arcweft.tests.accepted-rust-nominal-tooling"
version = "0.1.0"

[profiles.agent]
kind = "agent"
entry = "@entry.agent.main"
source = "src/main.arcw"
adapter = "rust-nominal-tooling"
"#,
        );
        project.write("src/main.arcw", ACCEPTED_RUST_NOMINAL_SOURCE);
        let manifest_path = project.path("arcw.toml");
        let owner = ProfileTopologyOwnerId::workspace(
            file_uri(&project.root).to_string(),
            file_uri(&manifest_path).to_string(),
        )
        .expect("workspace owner");
        let topology = load_profile_topology(ProfileTopologyLoadRequest::new(
            &manifest_path,
            owner,
            LaunchProfileSelection::Explicit("agent"),
            &[],
            AdapterRegistry::from_manifests([adapter.clone()]),
        ))
        .expect("custom adapter topology");
        let (candidate, _) =
            register_loaded_environment(&topology, AcceptedOverlaySet::default(), None)
                .expect("registered custom adapter environment");
        let profile = LspProfile::new(adapter, RuntimeHostRunnerKind::Native);
        profile
            .state()
            .replace_accepted(candidate)
            .expect("accepted custom adapter environment");

        let main_path = project.path("src/main.arcw");
        let document = open(&main_path, ACCEPTED_RUST_NOMINAL_SOURCE);
        let offset = ACCEPTED_RUST_NOMINAL_SOURCE
            .find("Envelope<Rank>")
            .expect("generic Rust type")
            + 1;
        AcceptedRustNominalToolingFixture {
            _project: project,
            profile,
            document,
            offset,
        }
    }

    #[test]
    fn accepted_rust_nominal_tooling_retains_identity_metadata_and_source() {
        let AcceptedRustNominalToolingFixture {
            _project,
            profile,
            document,
            offset,
        } = accepted_rust_nominal_tooling_fixture();
        let accepted = profile.accepted_environment().expect("accepted profile");
        let cursor = accepted_nominal_at(accepted.project(), &document, offset)
            .expect("typed accepted nominal cursor");
        assert_eq!(
            cursor.nominal.declaration().owner().source_label(),
            "rust:tooling-types"
        );
        assert_eq!(
            cursor
                .nominal
                .declaration()
                .canonical_path()
                .canonical_string(),
            "Envelope"
        );
        let [TypeKind::AcceptedNominal(argument)] = cursor.nominal.arguments() else {
            panic!("Envelope retains its accepted Rank argument")
        };
        assert_eq!(
            argument.declaration().canonical_path().canonical_string(),
            "Rank"
        );

        let HoverContents::Scalar(MarkedString::String(text)) = hover(&profile, &document, offset)
            .expect("accepted Rust nominal hover")
            .contents
        else {
            panic!("expected accepted Rust nominal string hover")
        };
        assert!(text.contains("accepted Rust nominal `rust:tooling-types::Envelope`"));
        assert!(text.contains("Rust package: `tooling-types`"));
        assert!(text.contains("mounted path: `Envelope`"));
        assert!(text.contains("arity: `1`"));
        assert!(text.contains("Rust item: `tooling_types::Envelope`"));
        assert!(text.contains("applied arguments: `Rank`"));

        let GotoDefinitionResponse::Scalar(actual) =
            definition(&profile, &document, offset).expect("accepted Rust metadata definition")
        else {
            panic!("expected one accepted Rust metadata definition")
        };
        let metadata = accepted
            .world()
            .environment()
            .rust_metadata()
            .get(cursor.nominal.declaration())
            .expect("accepted Rust metadata");
        assert_eq!(
            actual,
            location(accepted.project(), metadata.source())
                .expect("generated metadata source has a typed URI")
        );

        let completions = completions(&profile, &document);
        assert!(completions.iter().any(|item| {
            item.label == "Envelope"
                && item.detail.as_deref() == Some("rust:tooling-types::Envelope")
        }));
        assert!(
            !completions.iter().any(|item| item.label == "PrivateOnly"),
            "inaccessible accepted nominals are not completion candidates"
        );
    }

    #[test]
    fn accepted_project_nominal_tooling_rejects_a_stale_document_snapshot() {
        const TEST_ID: &str = "TOOL-STALE";
        let project = TestProject::new("nominal-stale-tooling");
        project.write_manifest();
        project.write("src/main.arcw", MAIN);
        project.write("src/models.arcw", MODELS);
        let main_path = project.path("src/main.arcw");
        let profile =
            LspProfileResolver::new(RuntimeHostRunnerKind::Native, Some("agent".to_owned()))
                .resolve_for_document_path(&main_path);
        assert!(
            profile.diagnostics().is_empty(),
            "{TEST_ID}: accepted project diagnostics: {:?}",
            profile.diagnostics()
        );
        let stale_source = MAIN.replace("LocalRecord", "StaleRecord");
        let document = open(&main_path, &stale_source);
        let offset = stale_source
            .find("value: StaleRecord")
            .expect("TOOL-STALE: stale nominal type use")
            + "value: ".len()
            + 1;
        let position = document.line_index().position_from_byte_offset(offset);

        assert!(
            crate::features::definition::definition(
                &profile,
                document.uri(),
                &DocumentStore::default(),
                &document,
                position,
            )
            .is_err(),
            "{TEST_ID}: stale text must reject the accepted-project definition snapshot"
        );
        assert!(
            crate::features::hover::hover(&profile, &document, position).is_none(),
            "{TEST_ID}: stale text must not receive accepted-project hover"
        );
        assert!(
            crate::features::references::references(&profile, document.uri(), &document, position,)
                .is_empty(),
            "{TEST_ID}: stale text must not receive accepted-project references"
        );
        assert!(
            crate::features::rename::rename(
                &profile,
                &DocumentStore::default(),
                &document,
                position,
                "Renamed",
            )
            .is_none(),
            "{TEST_ID}: stale text must not receive accepted-project rename edits"
        );
        assert!(
            !crate::features::completion::completions(&profile, Some(&document))
                .iter()
                .any(|item| item.label == "LocalRecord"),
            "{TEST_ID}: stale text must not receive accepted-project nominal completions"
        );
    }

    fn open(path: &Path, source: &str) -> DocumentSnapshot {
        let mut store = DocumentStore::default();
        store.open(
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: file_uri(path),
                    language_id: "arcweft".to_owned(),
                    version: 1,
                    text: source.to_owned(),
                },
            },
            PositionEncoding::Utf16,
        )
    }

    fn file_uri(path: &Path) -> Uri {
        format!(
            "file:///{}",
            path.to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches('/')
        )
        .parse()
        .expect("file URI")
    }

    struct TestProject {
        root: PathBuf,
    }

    impl TestProject {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let root = std::env::temp_dir().join(format!("{label}-{unique}"));
            fs::create_dir_all(&root).expect("root");
            Self { root }
        }

        fn path(&self, path: impl AsRef<Path>) -> PathBuf {
            self.root.join(path)
        }

        fn write_manifest(&self) {
            self.write(
                "arcw.toml",
                r#"schema = 1

[package]
id = "org.arcweft.tests.nominal-tooling"
version = "0.1.0"

[profiles.agent]
kind = "agent"
entry = "@entry.agent.main"
source = "src/main.arcw"
"#,
            );
        }

        fn write(&self, path: &str, contents: &str) {
            let path = self.path(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent");
            }
            fs::write(path, contents).expect("write");
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn rust_nominal_adapter() -> AdapterManifest {
        let package = ArcweftRustPackageId::try_new("tooling-types").expect("Rust package ID");
        let type_path = |name: &str| {
            ArcweftRustTypePath::try_new([
                ArcweftRustTypePathSegment::try_new(name).expect("Rust type path segment")
            ])
            .expect("Rust type path")
        };
        let parameter_index =
            ArcweftRustTypeParameterIndex::try_from_usize(0).expect("generic parameter index");
        let rust = ArcweftRustManifest::new(ArcweftRustPackage {
            id: package.clone(),
            version: "1.0.0".to_owned(),
            metadata_hash: None,
        })
        .with_type(ArcweftRustTypeDecl {
            path: type_path("Rank"),
            rust_path: "tooling_types::Rank".to_owned(),
            parameters: Vec::new(),
            kind: ArcweftRustTypeKind::Enum {
                variants: vec![ArcweftRustVariant {
                    name: "First".to_owned(),
                    payload: ArcweftRustVariantPayload::Unit,
                }],
            },
        })
        .with_type(ArcweftRustTypeDecl {
            path: type_path("Envelope"),
            rust_path: "tooling_types::Envelope".to_owned(),
            parameters: vec![ArcweftRustTypeParameter {
                index: parameter_index,
                name: ArcweftRustTypeParameterName::try_new("T").expect("generic parameter name"),
            }],
            kind: ArcweftRustTypeKind::Newtype {
                inner: ArcweftRustTypeRef::TypeParameter {
                    index: parameter_index,
                },
            },
        });
        let private_path =
            AdapterNominalPath::try_new([AdapterNominalPathSegment::try_new("PrivateOnly")
                .expect("private nominal path segment")])
            .expect("private nominal path");
        AdapterManifest::new("rust-nominal-tooling", "Rust Nominal Tooling")
            .try_with_nominal_declaration(
                AdapterNominalDeclaration::try_new(
                    private_path,
                    0,
                    AdapterNominalVisibility::Private,
                    "PrivateOnly",
                )
                .expect("private nominal declaration"),
            )
            .expect("private nominal path is unique")
            .try_with_rust_package_mount(
                package,
                AdapterNominalPathPrefix::try_new([]).expect("empty package mount"),
            )
            .expect("Rust package mount")
            .try_with_rust_manifest(&rust)
            .expect("Rust nominal metadata")
    }
}
