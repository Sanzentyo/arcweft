//! Project-aware semantic inventory construction for exact open documents.

use std::{fs, path::Path};

use arcweft_lang_hir::{
    lower::lower_to_hir,
    project::{HirProject, HirProjectModule},
};
use arcweft_lang_sema::{
    canonicalization::{
        CanonicalizationSourceSet, CheckedCanonicalizationInventory, SemanticDataUnavailable,
        SemanticDocumentId, SemanticSourceIdentity,
    },
    check::analyze_project_types_for_canonicalization,
    env::TypeCheckEnv,
};
use arcweft_lang_syntax::parser::parse_source;
use lsp_types::Uri;

use crate::profiles::file_path_from_uri;

pub(crate) fn checked_inventory_for_document(
    uri: &Uri,
    source: &str,
    env: &TypeCheckEnv,
) -> Result<CheckedCanonicalizationInventory, SemanticDataUnavailable> {
    let document = SemanticDocumentId::new(uri.to_string());
    let path = file_path_from_uri(uri).ok_or_else(|| {
        SemanticDataUnavailable::new(document.clone(), "document URI is not a local file URI")
    })?;
    let loaded = arcweft_project_loader::project::load_discovered(&path)
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
    let package_name = loaded.sources().manifest().package().name().as_str();

    for project_source in loaded.sources().modules() {
        let selected_source = project_source.module() == &selected_module;
        let exact_source = if selected_source {
            source
        } else {
            project_source.source()
        };
        let parsed = parse_source(exact_source.to_owned());
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
        let hir = lower_to_hir(parsed.typed_tree()).map_err(|errors| {
            SemanticDataUnavailable::new(
                document.clone(),
                format!(
                    "HIR lowering failed for `{}`: {errors:?}",
                    project_source.path().display()
                ),
            )
        })?;
        modules.push(HirProjectModule::new(project_source.module().clone(), hir));
    }

    let hir_project = HirProject::new(package_name, modules)
        .map_err(|error| SemanticDataUnavailable::new(document.clone(), error.to_string()))?;
    for project_source in loaded.sources().modules() {
        let selected_source = project_source.module() == &selected_module;
        let exact_source = if selected_source {
            source
        } else {
            project_source.source()
        };
        let source_document = if selected_source {
            document.clone()
        } else {
            SemanticDocumentId::new(normalized_path(project_source.path()).display().to_string())
        };
        identities.push(SemanticSourceIdentity::from_source(
            hir_project.package().clone(),
            source_document,
            project_source.module().clone(),
            exact_source,
        ));
    }
    let sources = CanonicalizationSourceSet::try_new(hir_project.package().clone(), identities)
        .map_err(|error| SemanticDataUnavailable::new(document.clone(), error.to_string()))?;
    let report = analyze_project_types_for_canonicalization(&hir_project, env, &sources)?;
    let selected_identity = sources.source(&selected_module).ok_or_else(|| {
        SemanticDataUnavailable::new(
            document.clone(),
            "open module has no exact semantic source identity",
        )
    })?;
    report
        .canonicalization_inventory(selected_identity)
        .cloned()
        .ok_or_else(|| {
            SemanticDataUnavailable::new(
                document,
                "checked report has no exact inventory for the open module",
            )
        })
}

fn normalized_path(path: &Path) -> std::path::PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
