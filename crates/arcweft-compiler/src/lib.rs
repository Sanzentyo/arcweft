//! Source-to-runtime-plan compiler driver for Arcweft.

use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_hir::model::HirModule;
use arcweft_lang_sema::check::{TypeCheckReport, analyze_types};
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::resolve::{registry_from_hir, validate_hir_references};
use arcweft_lang_syntax::ast::items::TypedSyntaxTree;
use arcweft_lang_syntax::lint::{SyntaxLint, SyntaxLintSeverity, lint_id_policy};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_lang_syntax::source::ParsedSource;
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_plan::flow::{RuntimePlanLowerStats, lower_runtime_plan_with_stats};
use arcweft_runtime_plan::line_task::{LoweredLineTaskGroup, lower_line_task_groups};
use thiserror::Error;

/// Source compilation result shared by developer tooling and player hosts.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledSource {
    pub plan: arcweft_core::plan::RuntimePlan,
    pub display: LineDisplayCatalog,
    pub hir: arcweft_lang_hir::model::HirModule,
    pub typecheck_report: TypeCheckReport,
    pub runtime_plan_stats: RuntimePlanLowerStats,
}

/// Source compiler diagnostics for the shared driver.
#[derive(Debug, Error)]
pub enum CompileSourceError {
    #[error("parse errors: {0:?}")]
    Parse(Vec<arcweft_lang_syntax::parser::recovery::ParseError>),
    #[error("HIR lowering errors: {0:?}")]
    Hir(Vec<arcweft_lang_hir::model::HirLowerError>),
    #[error("reference resolution errors: {0:?}")]
    Resolve(Vec<arcweft_lang_sema::resolve::NameResolutionError>),
    #[error("type-check readiness errors: {0:?}")]
    Readiness(Vec<arcweft_lang_sema::diagnostics::TypeCheckReadinessError>),
    #[error("type errors: {0:?}")]
    Type(Vec<arcweft_lang_sema::diagnostics::TypeCheckError>),
    #[error("runtime-plan lowering errors: {0:?}")]
    RuntimePlan(Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>),
}

/// HIR semantic validation diagnostics for the shared compiler driver.
#[derive(Debug, Error)]
pub enum ValidateHirError {
    #[error("reference resolution errors: {0:?}")]
    Resolve(Vec<arcweft_lang_sema::resolve::NameResolutionError>),
    #[error("type-check readiness errors: {0:?}")]
    Readiness(Vec<arcweft_lang_sema::diagnostics::TypeCheckReadinessError>),
    #[error("type errors: {0:?}")]
    Type(Vec<arcweft_lang_sema::diagnostics::TypeCheckError>),
}

/// Parses source text into the shared syntax parser output.
pub fn parse_source_text(source: impl Into<String>) -> ParsedSource {
    parse_source(source)
}

/// Runs syntax-level source lints on a typed syntax tree.
pub fn lint_source_tree(tree: &TypedSyntaxTree) -> Vec<SyntaxLint> {
    lint_id_policy(tree)
}

/// Counts source lints that should be reported as warnings.
pub fn count_warning_lints(lints: &[SyntaxLint]) -> usize {
    lints
        .iter()
        .filter(|lint| matches!(lint.severity(), SyntaxLintSeverity::Warning))
        .count()
}

/// Returns whether any source lint should stop compilation.
pub fn has_error_lints(lints: &[SyntaxLint]) -> bool {
    lints
        .iter()
        .any(|lint| matches!(lint.severity(), SyntaxLintSeverity::Error))
}

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

/// Lowers dialogue line plans from HIR into runtime task groups.
pub fn lower_source_line_tasks(
    hir: &HirModule,
) -> Result<Vec<LoweredLineTaskGroup>, Vec<arcweft_runtime_plan::errors::LinePlanLowerError>> {
    lower_line_task_groups(hir)
}

/// Compiles an Arcweft source string with the standard type-checking environment.
pub fn compile_source(source: &str) -> Result<CompiledSource, CompileSourceError> {
    compile_source_with_env(source, &TypeCheckEnv::standard())
}

/// Compiles an Arcweft source string with a supplied type-checking environment.
pub fn compile_source_with_env(
    source: &str,
    env: &TypeCheckEnv,
) -> Result<CompiledSource, CompileSourceError> {
    let parsed = parse_source(source.to_owned());
    if !parsed.errors().is_empty() {
        return Err(CompileSourceError::Parse(parsed.errors().to_vec()));
    }
    let hir = lower_to_hir(parsed.typed_tree()).map_err(CompileSourceError::Hir)?;
    let typecheck_report = validate_hir_with_env(&hir, env).map_err(|error| match error {
        ValidateHirError::Resolve(errors) => CompileSourceError::Resolve(errors),
        ValidateHirError::Readiness(errors) => CompileSourceError::Readiness(errors),
        ValidateHirError::Type(errors) => CompileSourceError::Type(errors),
    })?;
    let report = lower_runtime_plan_with_stats(&hir).map_err(CompileSourceError::RuntimePlan)?;
    Ok(CompiledSource {
        plan: report.plan,
        display: report.line_display_catalog,
        hir,
        typecheck_report,
        runtime_plan_stats: report.stats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_dialogue_source_to_plan_and_display_catalog() {
        let source = r"
character @character.alice Alice as alice {}

entry game @entry.main {
    start(@flow.main)
}

flow @flow.main main {
    alice: Hello
}
";

        let compiled = compile_source(source).expect("source compiles");

        assert!(!compiled.plan.entries.is_empty());
        assert!(!compiled.display.lines().is_empty());
    }
}
