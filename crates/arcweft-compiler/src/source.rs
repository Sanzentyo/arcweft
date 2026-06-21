use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_syntax::parser::parse_source;
use arcweft_runtime_plan::flow::lower_runtime_plan_with_stats;

use crate::error::{CompileSourceError, ValidateHirError};
use crate::hir::validate_hir_with_env;
use crate::types::CompiledSource;

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
