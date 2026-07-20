//! Project-aware semantic inventory construction for exact open documents.

use std::{fs, path::Path, sync::Arc};

use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
};
use arcweft_lang_sema::{
    canonicalization::{
        CanonicalizationSourceSet, CheckedCanonicalizationInventory, SemanticDataUnavailable,
    },
    check::analyze_registered_project_types_for_canonicalization,
    registration::{CharacterRegistrar, CharacterRegistrationRequest},
};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceRange};
use lsp_types::Uri;

use crate::profiles::LspProfile;
use crate::profiles::file_path_from_uri;

#[allow(
    clippy::too_many_lines,
    reason = "one fail-closed transaction binds the open text, project modules, registration world, and checked inventory"
)]
pub(crate) fn checked_inventory_for_document(
    uri: &Uri,
    source: &str,
    profile: &LspProfile,
) -> Result<CheckedCanonicalizationInventory, SemanticDataUnavailable> {
    let document = SourceDocumentId::try_new(uri.to_string()).map_err(|error| {
        SemanticDataUnavailable::new(
            lsp_fallback_document(),
            format!("document URI cannot identify a source document: {error}"),
        )
    })?;
    let path = file_path_from_uri(uri).ok_or_else(|| {
        SemanticDataUnavailable::new(document.clone(), "document URI is not a local file URI")
    })?;
    let limits = arcweft_lang_sema::registration::CharacterRegistrationLimits::PRODUCTION;
    let loaded = arcweft_project_loader::project::load_discovered_with_limits(
        &path,
        arcweft_project_loader::project_limits::ProjectLoadLimits::new(
            limits.documents(),
            limits.source_bytes(),
        ),
    )
    .map_err(|error| SemanticDataUnavailable::new(document.clone(), error.to_string()))?;
    let selected_path = normalized_path(&path);
    let selected = loaded
        .sources()
        .modules()
        .find(|candidate| normalized_path(candidate.path()) == selected_path)
        .ok_or_else(|| {
            SemanticDataUnavailable::new(
                document.clone(),
                "open document is not a module in the discovered project",
            )
        })?;
    let selected_module = selected.module().clone();

    let mut identities = Vec::with_capacity(loaded.sources().modules().len());
    let mut modules = Vec::with_capacity(loaded.sources().modules().len());
    let mut additional_documents = Vec::new();
    let package_name = loaded.sources().package().id.as_str();

    for project_source in loaded.sources().modules() {
        let selected_source = project_source.module() == &selected_module;
        let loaded_document = loaded
            .module_document(project_source.module())
            .ok_or_else(|| {
                SemanticDataUnavailable::new(
                    document.clone(),
                    format!(
                        "loaded project has no source document for module `{}`",
                        project_source.module()
                    ),
                )
            })?;
        let exact_document = if selected_source {
            let document = Arc::new(
                SourceDocument::try_new(
                    document.clone(),
                    loaded_document.display_name().clone(),
                    source,
                )
                .map_err(|error| {
                    SemanticDataUnavailable::new(document.clone(), error.to_string())
                })?,
            );
            additional_documents.push(Arc::clone(&document));
            document
        } else {
            Arc::clone(loaded_document)
        };
        let parsed = parse_source(exact_document.text());
        if !parsed.errors().is_empty() {
            return Err(SemanticDataUnavailable::new(
                document.clone(),
                format!(
                    "source `{}` has syntax errors: {:?}",
                    project_source.path().display(),
                    parsed.errors()
                ),
            ));
        }
        let hir =
            lower_document_to_hir(&exact_document, parsed.typed_tree()).map_err(|errors| {
                SemanticDataUnavailable::new(
                    document.clone(),
                    format!(
                        "HIR lowering failed for `{}`: {errors:?}",
                        project_source.path().display()
                    ),
                )
            })?;
        let identity = exact_document.identity().clone();
        let source_span = exact_document
            .span(SourceRange::new(0, exact_document.text().len()))
            .map_err(|error| SemanticDataUnavailable::new(document.clone(), error.to_string()))?;
        modules.push(
            HirProjectModule::try_new(project_source.module().clone(), identity, hir).map_err(
                |error| SemanticDataUnavailable::new(document.clone(), error.to_string()),
            )?,
        );
        identities.push((project_source.module().clone(), source_span));
    }

    let hir_project = HirProject::new(package_name, modules)
        .map_err(|error| SemanticDataUnavailable::new(document.clone(), error.to_string()))?;
    let sources = CanonicalizationSourceSet::try_new(hir_project.package().clone(), identities)
        .map_err(|error| SemanticDataUnavailable::new(document.clone(), error.to_string()))?;
    let previous = profile.accepted_environment();
    let request = arcweft_project_loader::environment::ProjectLoadRequest::new(
        &loaded,
        additional_documents,
        Vec::new(),
    )
    .with_adapter_manifests(profile.declared_manifests().iter().cloned());
    let registration = arcweft_project_loader::environment::load_project_registration(&request)
        .map_err(|error| SemanticDataUnavailable::new(document.clone(), error.to_string()))?;
    let (facts, _) = registration.into_parts();
    let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(profile.typecheck_env()),
        &hir_project,
        &facts,
        previous
            .as_ref()
            .map(|accepted| accepted.world().environment()),
    ))
    .map_err(|report| {
        SemanticDataUnavailable::new(
            document.clone(),
            report
                .diagnostics()
                .iter()
                .map(|diagnostic| diagnostic.diagnostic().message().to_owned())
                .collect::<Vec<_>>()
                .join("; "),
        )
    })?;
    let report =
        analyze_registered_project_types_for_canonicalization(&hir_project, &registered, &sources)?;
    let selected_identity = sources.source(&selected_module).ok_or_else(|| {
        SemanticDataUnavailable::new(
            document.clone(),
            "open module has no exact semantic source identity",
        )
    })?;
    report
        .canonicalization_inventory(&selected_module, selected_identity)
        .cloned()
        .ok_or_else(|| {
            SemanticDataUnavailable::new(
                document,
                "checked report has no exact inventory for the open module",
            )
        })
}

fn lsp_fallback_document() -> SourceDocumentId {
    SourceDocumentId::try_new("arcweft-generated://lsp-invalid-document/0")
        .expect("generated fallback document id is valid")
}

fn normalized_path(path: &Path) -> std::path::PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
