use crate::app::project::{SourceSelection, runtime_plan_options_for_selection};
use crate::output::RuntimeProfilePhase;
use arcweft_compiler::{hir, lower, parse};
use arcweft_core::aot::{AotProgram, AotProgramStats};
use arcweft_core::bytecode::{BytecodeProgram, BytecodeStats};
use arcweft_core::plan::RuntimePlan;
use arcweft_lang_sema::{check::TypeCheckReport, env::TypeCheckEnv};
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_plan::flow::{RuntimePlanLowerReport, RuntimePlanLowerStats};
use arcweft_verify::{RuntimeTypeValidationStats, validate_runtime_plan_types};
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

pub(in crate::app) struct ProfileCompiledRuntimePlan {
    pub(in crate::app) hir: arcweft_lang_hir::model::HirModule,
    pub(in crate::app) plan: RuntimePlan,
    pub(in crate::app) syntax_warnings: usize,
    pub(in crate::app) syntax_stats: arcweft_lang_syntax::cst::SyntaxParseStats,
    pub(in crate::app) line_task_groups: usize,
    pub(in crate::app) typecheck_report: TypeCheckReport,
    pub(in crate::app) runtime_plan_stats: RuntimePlanLowerStats,
    pub(in crate::app) line_display_catalog: LineDisplayCatalog,
    pub(in crate::app) runtime_type_validation_stats: RuntimeTypeValidationStats,
    pub(in crate::app) bytecode: BytecodeProgram,
    pub(in crate::app) bytecode_stats: BytecodeStats,
    pub(in crate::app) aot_stats: AotProgramStats,
}

pub(in crate::app) fn compile_profile_runtime_plan(
    selection: &SourceSelection,
    env: &TypeCheckEnv,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ProfileCompiledRuntimePlan, ExitCode> {
    let source = run_profile_phase(phases, "read_source", || {
        fs::read_to_string(selection.path()).map_err(|error| {
            eprintln!(
                "error: failed to read {}: {error}",
                selection.path().display()
            );
            ExitCode::FAILURE
        })
    })?;
    let parsed = run_profile_phase(phases, "parse", || {
        catch_unwind(AssertUnwindSafe(|| parse::parse_source_text(source))).map_err(|_| {
            eprintln!(
                "error: parser panicked while profiling {}",
                selection.path().display()
            );
            ExitCode::FAILURE
        })
    })?;
    if !parsed.errors().is_empty() {
        for error in parsed.errors() {
            eprintln!("error: {}", error.message());
        }
        return Err(ExitCode::FAILURE);
    }
    let syntax_stats = parsed.syntax_stats();
    let tree = parsed.into_typed_tree();
    let syntax_warnings = run_profile_phase(phases, "lint", || {
        let lints = parse::lint_source_tree(&tree);
        for lint in &lints {
            eprintln!(
                "{}[{} {}]: {}",
                lint.severity().label(),
                lint.code().stable_code(),
                lint.code().domain_name(),
                lint.message()
            );
        }
        if parse::has_error_lints(&lints) {
            return Err(ExitCode::FAILURE);
        }
        Ok::<usize, ExitCode>(parse::count_warning_lints(&lints))
    })?;
    let hir = profile_lower_hir(&tree, phases)?;
    let typecheck_report = profile_validate_hir(&hir, env, phases)?;
    let line_task_groups = run_profile_phase(phases, "line_task_lower", || {
        lower::lower_source_line_tasks(&hir).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;
    let runtime_plan_report = profile_lower_runtime_plan(selection, &hir, phases)?;
    let plan = runtime_plan_report.plan;
    let line_display_catalog = runtime_plan_report.line_display_catalog;
    let runtime_plan_stats = runtime_plan_report.stats;
    let runtime_type_validation_stats = run_profile_phase(phases, "runtime_type_validate", || {
        let report = validate_runtime_plan_types(&plan, &typecheck_report);
        if report.has_errors() {
            for diagnostic in report.diagnostics {
                eprintln!("error: {}: {}", diagnostic.path, diagnostic.message);
            }
            Err(ExitCode::FAILURE)
        } else {
            Ok(report.stats)
        }
    })?;
    let aot = run_profile_phase(phases, "aot_lower", || {
        Ok::<AotProgram, ExitCode>(AotProgram::from_runtime_plan(&plan))
    })?;
    let aot_stats = aot.stats().clone();
    let bytecode = run_profile_phase(phases, "bytecode_lower", || {
        Ok::<BytecodeProgram, ExitCode>(BytecodeProgram::from_runtime_plan(plan))
    })?;
    let bytecode_stats = bytecode.stats();
    let plan = bytecode.clone().into_runtime_plan().map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })?;
    Ok(ProfileCompiledRuntimePlan {
        hir,
        plan,
        syntax_warnings,
        syntax_stats,
        line_task_groups: line_task_groups.len(),
        typecheck_report,
        runtime_plan_stats,
        line_display_catalog,
        runtime_type_validation_stats,
        bytecode,
        bytecode_stats,
        aot_stats,
    })
}

fn profile_lower_runtime_plan(
    selection: &SourceSelection,
    hir: &arcweft_lang_hir::model::HirModule,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<RuntimePlanLowerReport, ExitCode> {
    run_profile_phase(phases, "runtime_plan_lower", || {
        let runtime_options = runtime_plan_options_for_selection(selection);
        lower::lower_source_runtime_plan_with_stats_and_options(hir, &runtime_options).map_err(
            |errors| {
                for error in errors {
                    eprintln!("error: {}", error.message());
                }
                ExitCode::FAILURE
            },
        )
    })
}

pub(in crate::app) fn profile_lower_hir(
    tree: &arcweft_lang_syntax::ast::items::TypedSyntaxTree,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<arcweft_lang_hir::model::HirModule, ExitCode> {
    run_profile_phase(phases, "lower_hir", || {
        hir::lower_source_tree(tree).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })
}

pub(in crate::app) fn profile_validate_hir(
    hir: &arcweft_lang_hir::model::HirModule,
    env: &TypeCheckEnv,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<TypeCheckReport, ExitCode> {
    run_profile_phase(phases, "resolve", || {
        hir::resolve_hir_references(hir).map_err(|errors| {
            for error in errors {
                eprintln!("error: {error}");
            }
            ExitCode::FAILURE
        })
    })?;
    run_profile_phase(phases, "readiness", || {
        hir::validate_hir_typecheck_ready(hir).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;
    run_profile_phase(phases, "typecheck", || {
        hir::typecheck_hir_with_env(hir, env).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })
}

pub(in crate::app) fn run_profile_phase<T>(
    phases: &mut Vec<RuntimeProfilePhase>,
    name: &'static str,
    run: impl FnOnce() -> Result<T, ExitCode>,
) -> Result<T, ExitCode> {
    let started = Instant::now();
    let result = run();
    phases.push(RuntimeProfilePhase {
        name,
        elapsed_ns: started.elapsed().as_nanos(),
    });
    result
}

pub(in crate::app) fn report_path(path: &Path) -> String {
    if let Ok(cwd) = std::env::current_dir()
        && let Ok(relative) = path.strip_prefix(cwd)
    {
        return relative.display().to_string();
    }
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
