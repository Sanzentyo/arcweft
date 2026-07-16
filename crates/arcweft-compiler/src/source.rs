use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_syntax::parser::parse_source;

use crate::error::{CompileSourceError, ValidateHirError};
use crate::hir::validate_hir_with_env;
use crate::lower::lower_source_runtime_plan_with_typecheck_stats_and_options;
use crate::style::lower_source_view_styles;
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
    let style = lower_source_view_styles(&hir, &typecheck_report.style_catalog, parsed.document())?;
    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck_report,
        &arcweft_runtime_plan::flow::RuntimePlanLowerOptions::default(),
    )
    .map_err(CompileSourceError::RuntimePlan)?;
    Ok(CompiledSource {
        plan: report.plan,
        display: report.line_display_catalog,
        hir,
        typecheck_report,
        style,
        runtime_plan_stats: report.stats,
    })
}
