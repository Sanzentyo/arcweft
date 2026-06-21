use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_hir::model::HirModule;
use arcweft_lang_sema::check::{TypeCheckReport, analyze_types};
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::resolve::{registry_from_hir, validate_hir_references};
use arcweft_lang_syntax::ast::items::TypedSyntaxTree;

use crate::error::ValidateHirError;

/// Lowers a typed syntax tree into HIR.
pub fn lower_source_tree(
    tree: &TypedSyntaxTree,
) -> Result<HirModule, Vec<arcweft_lang_hir::model::HirLowerError>> {
    lower_to_hir(tree)
}
/// Validates and type-checks HIR with a supplied environment.
pub fn validate_hir_with_env(
    hir: &HirModule,
    env: &TypeCheckEnv,
) -> Result<TypeCheckReport, ValidateHirError> {
    resolve_hir_references(hir).map_err(ValidateHirError::Resolve)?;
    validate_hir_typecheck_ready(hir).map_err(ValidateHirError::Readiness)?;
    typecheck_hir_with_env(hir, env).map_err(ValidateHirError::Type)
}

/// Validates HIR entity references against declarations in the same module.
pub fn resolve_hir_references(
    hir: &HirModule,
) -> Result<(), Vec<arcweft_lang_sema::resolve::NameResolutionError>> {
    let registry = registry_from_hir(hir);
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
