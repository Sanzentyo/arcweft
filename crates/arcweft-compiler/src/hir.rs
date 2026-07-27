use arcweft_lang_hir::lower::lower_document_to_hir;
use arcweft_lang_hir::model::HirModule;
use arcweft_lang_sema::check::{TypeCheckReport, analyze_registered_project_types};
use arcweft_lang_sema::registration::RegisteredSemanticWorld;
use arcweft_lang_sema::resolve::{registry_from_hir_and_registered, validate_hir_references};
use arcweft_lang_syntax::ast::items::TypedSyntaxTree;
use arcweft_source::SourceDocument;

/// Lowers a typed syntax tree while binding every retained range to one source revision.
pub fn lower_source_document(
    document: &SourceDocument,
    tree: &TypedSyntaxTree,
) -> Result<HirModule, Vec<arcweft_lang_hir::model::HirLowerError>> {
    lower_document_to_hir(document, tree)
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

/// Type-checks linked project HIR through the committed registration boundary.
pub fn typecheck_registered_project(
    hir: &HirModule,
    registered: &RegisteredSemanticWorld,
) -> Result<TypeCheckReport, Vec<arcweft_lang_sema::diagnostics::TypeCheckError>> {
    let report = analyze_registered_project_types(hir, registered);
    report.clone().into_result()?;
    Ok(report)
}
