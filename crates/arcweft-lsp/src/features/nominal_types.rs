//! Accepted-project nominal navigation and refactoring from typed resolver edges.

use std::{
    collections::{BTreeSet, HashMap},
    fmt::Write,
};

use arcweft_lang_hir::symbol::{
    ProjectTypeTarget,
    nominal::{ProjectNominalDeclarationId, ProjectNominalDeclarationKind},
};
use arcweft_lang_sema::types::TypeKind;
use arcweft_lang_syntax::ast::module_path::ModuleSegment;
use arcweft_source::SourceSpan;
use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{
    CompletionItem, CompletionItemKind, Documentation, GotoDefinitionResponse, Hover,
    HoverContents, Location, MarkedString, PrepareRenameResponse, TextEdit, Uri, WorkspaceEdit,
};

use crate::{
    documents::{DocumentSnapshot, DocumentStore},
    profiles::{LspProfile, accepted_project::AcceptedProjectSnapshot},
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

pub(crate) fn hover(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<Hover> {
    let accepted = profile.accepted_environment()?;
    let project = exact_project(accepted.project(), document)?;
    let cursor = symbol_at(project, document, offset)?;
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
    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(text)),
        range: Some(
            document
                .line_index()
                .range_from_byte_span(cursor.source.range().start(), cursor.source.range().end()),
        ),
    })
}

pub(crate) fn definition(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<GotoDefinitionResponse> {
    let accepted = profile.accepted_environment()?;
    let project = exact_project(accepted.project(), document)?;
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
            ProjectTypeTarget::External(external) => CompletionItem {
                label: binding.spelling().to_string(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(external.canonical_path().to_string()),
                documentation: Some(Documentation::String(
                    "Accepted source-backed external type.".to_owned(),
                )),
                ..CompletionItem::default()
            },
        })
        .collect::<Vec<_>>();
    items.extend(
        accepted
            .world()
            .environment()
            .nominal_catalog()
            .exact_records()
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
    items
}

pub(crate) fn prepare_rename(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<PrepareRenameResponse> {
    let accepted = profile.accepted_environment()?;
    let project = exact_project(accepted.project(), document)?;
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

    use arcweft_runtime_host::RuntimeHostRunnerKind;
    use lsp_types::{
        DidOpenTextDocumentParams, GotoDefinitionResponse, HoverContents, MarkedString,
        TextDocumentItem, Uri,
    };

    use super::*;
    use crate::{
        diagnostics::{DocumentAnalysis, publish_diagnostics_from_analysis},
        positions::PositionEncoding,
        profiles::LspProfileResolver,
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
}
