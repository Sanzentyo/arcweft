use arcweft_lang_hir::lower::lower_document_to_hir;
use arcweft_lang_hir::model::HirModule;
use arcweft_lang_sema::check::{TypeCheckReport, analyze_registered_project_types, analyze_types};
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::registration::RegisteredSemanticWorld;
use arcweft_lang_sema::resolve::{
    registry_from_hir_and_env, registry_from_hir_and_registered, validate_hir_references,
};
use arcweft_lang_syntax::ast::items::TypedSyntaxTree;
use arcweft_source::SourceDocument;

use crate::error::ValidateHirError;

/// Lowers a typed syntax tree while binding every retained range to one source revision.
pub fn lower_source_document(
    document: &SourceDocument,
    tree: &TypedSyntaxTree,
) -> Result<HirModule, Vec<arcweft_lang_hir::model::HirLowerError>> {
    lower_document_to_hir(document, tree)
}
/// Validates and type-checks HIR with a supplied environment.
pub fn validate_hir_with_env(
    hir: &HirModule,
    env: &TypeCheckEnv,
) -> Result<TypeCheckReport, ValidateHirError> {
    resolve_hir_references_with_env(hir, env).map_err(ValidateHirError::Resolve)?;
    validate_hir_typecheck_ready(hir).map_err(ValidateHirError::Readiness)?;
    typecheck_hir_with_env(hir, env).map_err(ValidateHirError::Type)
}

/// Validates HIR entity references against declarations plus supplied semantic symbols.
pub fn resolve_hir_references_with_env(
    hir: &HirModule,
    env: &TypeCheckEnv,
) -> Result<(), Vec<arcweft_lang_sema::resolve::NameResolutionError>> {
    let registry = registry_from_hir_and_env(hir, env);
    validate_hir_references(hir, &registry)
}

/// Validates HIR entity references through the committed project semantic world.
pub fn resolve_registered_hir_references(
    hir: &HirModule,
    registered: &RegisteredSemanticWorld,
) -> Result<(), Vec<arcweft_lang_sema::resolve::NameResolutionError>> {
    let registry = registry_from_hir_and_registered(hir, registered);
    validate_hir_references(hir, &registry)
}

/// Validates that HIR no longer contains raw syntax fragments.
pub fn validate_hir_typecheck_ready(
    hir: &HirModule,
) -> Result<(), Vec<arcweft_lang_sema::diagnostics::TypeCheckReadinessError>> {
    arcweft_lang_sema::check::validate_typecheck_ready(hir)
}

/// Type-checks HIR and returns the full type-check report.
pub fn typecheck_hir_with_env(
    hir: &HirModule,
    env: &TypeCheckEnv,
) -> Result<TypeCheckReport, Vec<arcweft_lang_sema::diagnostics::TypeCheckError>> {
    let report = analyze_types(hir, env);
    report.clone().into_result()?;
    Ok(report)
}

/// Type-checks linked project HIR through the committed registration boundary.
pub fn typecheck_registered_project(
    hir: &HirModule,
    registered: &RegisteredSemanticWorld,
) -> Result<TypeCheckReport, Vec<arcweft_lang_sema::diagnostics::TypeCheckError>> {
    let report = analyze_registered_project_types(hir, registered);
    report.clone().into_result()?;
    Ok(report)
}
