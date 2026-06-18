//! Source-to-runtime-plan compiler driver for Arcweft.

use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::check::{TypeCheckReport, analyze_types};
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_syntax::parser::parse_source;
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_plan::flow::{RuntimePlanLowerStats, lower_runtime_plan_with_stats};
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
    #[error("type errors: {0:?}")]
    Type(Vec<arcweft_lang_sema::diagnostics::TypeCheckError>),
    #[error("runtime-plan lowering errors: {0:?}")]
    RuntimePlan(Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>),
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
    let typecheck_report = analyze_types(&hir, env);
    typecheck_report
        .clone()
        .into_result()
        .map_err(CompileSourceError::Type)?;
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
