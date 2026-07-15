use super::expectations::{parse_goto_flow_in_text, parse_goto_flow_statement};
use crate::app::diagnostics::{DiagnosticEmitter, DiagnosticSource};
use crate::app::project::{
    SelectionSemanticContext, SourceSelection, print_project_compile_error,
    project_compilation_context, runtime_plan_options_for_selection, source_document_for_path,
};
use crate::output::RuntimeProfilePhase;
use arcweft_compiler::{
    hir, lower, parse,
    project::compile_project,
    style::{CompiledViewStyleArtifact, lower_source_view_styles},
};
use arcweft_core::{
    aot::{AotProgram, AotProgramStats},
    awbc::schema::AwbcProgram,
    bytecode::{BytecodeProgram, BytecodeStats},
    plan::{FlowRuntimeId, RuntimePlan},
};
use arcweft_lang_sema::{check::TypeCheckReport, env::TypeCheckEnv};
use arcweft_lang_syntax::cst::SyntaxParseStats;
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_plan::{
    awbc_lower::{AwbcLowerError, AwbcLowerer},
    flow::{RuntimePlanLowerReport, RuntimePlanLowerStats},
};
use arcweft_source::SourceDocument;
use arcweft_test::collect_script_tests;
use arcweft_verify::{RuntimeTypeValidationStats, validate_runtime_plan_types};
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

pub(in crate::app) struct ProfileCompiledRuntimePlan {
    pub(in crate::app) hir: arcweft_lang_hir::model::HirModule,
    pub(in crate::app) style: CompiledViewStyleArtifact,
    pub(in crate::app) plan: RuntimePlan,
    pub(in crate::app) syntax_warnings: usize,
    pub(in crate::app) syntax_stats: arcweft_lang_syntax::cst::SyntaxParseStats,
    pub(in crate::app) line_task_groups: usize,
    pub(in crate::app) typecheck_report: TypeCheckReport,
    pub(in crate::app) runtime_plan_stats: RuntimePlanLowerStats,
    pub(in crate::app) line_display_catalog: LineDisplayCatalog,
    pub(in crate::app) runtime_type_validation_stats: RuntimeTypeValidationStats,
    pub(in crate::app) product_awbc: AwbcProgram,
    pub(in crate::app) bytecode: BytecodeProgram,
    pub(in crate::app) bytecode_stats: BytecodeStats,
    pub(in crate::app) aot_stats: AotProgramStats,
    pub(in crate::app) source_document: Arc<SourceDocument>,
}

struct ProfileParsedSource {
    document: Arc<SourceDocument>,
    tree: arcweft_lang_syntax::ast::items::TypedSyntaxTree,
    syntax_warnings: usize,
    syntax_stats: SyntaxParseStats,
}

struct ProfileCheckedSource {
    hir: arcweft_lang_hir::model::HirModule,
    style: CompiledViewStyleArtifact,
    line_task_groups: usize,
    typecheck_report: TypeCheckReport,
}

pub(in crate::app) fn compile_profile_runtime_plan(
    selection: &SourceSelection,
    semantic: &SelectionSemanticContext,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ProfileCompiledRuntimePlan, ExitCode> {
    if let Some(manifest) = selection.manifest() {
        return compile_project_runtime_plan(manifest, selection, semantic, phases);
    }
    let env = semantic.base();
    let parsed = profile_parse_single_source(selection, phases)?;
    let checked = profile_lower_checked_source(&parsed, env, phases)?;
    let runtime_plan_report =
        profile_lower_runtime_plan(selection, &checked.hir, &checked.typecheck_report, phases)?;
    let mut plan = runtime_plan_report.plan;
    apply_script_manifest_entry_fallback(&mut plan, &checked.hir);
    let line_display_catalog = runtime_plan_report.line_display_catalog;
    let runtime_plan_stats = runtime_plan_report.stats;
    let runtime_type_validation_stats = run_profile_phase(phases, "runtime_type_validate", || {
        let report = validate_runtime_plan_types(&plan, &checked.typecheck_report);
        if report.has_errors() {
            for diagnostic in report.diagnostics {
                eprintln!("error: {}: {}", diagnostic.path, diagnostic.message);
            }
            Err(ExitCode::FAILURE)
        } else {
            Ok(report.stats)
        }
    })?;
    let product_awbc = profile_lower_product_awbc(selection, &plan, &line_display_catalog, phases)?;
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
        hir: checked.hir,
        style: checked.style,
        plan,
        syntax_warnings: parsed.syntax_warnings,
        syntax_stats: parsed.syntax_stats,
        line_task_groups: checked.line_task_groups,
        typecheck_report: checked.typecheck_report,
        runtime_plan_stats,
        line_display_catalog,
        runtime_type_validation_stats,
        product_awbc,
        bytecode,
        bytecode_stats,
        aot_stats,
        source_document: Arc::clone(&parsed.document),
    })
}

fn profile_parse_single_source(
    selection: &SourceSelection,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ProfileParsedSource, ExitCode> {
    let source = run_profile_phase(phases, "read_source", || {
        fs::read_to_string(selection.path()).map_err(|error| {
            eprintln!(
                "error: failed to read {}: {error}",
                selection.path().display()
            );
            ExitCode::FAILURE
        })
    })?;
    let document = Arc::new(source_document_for_path(selection.path(), source)?);
    let parsed = run_profile_phase(phases, "parse", || {
        catch_unwind(AssertUnwindSafe(|| {
            parse::parse_source_text(document.text().to_owned())
        }))
        .map_err(|_| {
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

    debug_assert_eq!(parsed.source(), document.text());
    let diagnostic_source = DiagnosticSource::new(&document);
    let emitter = DiagnosticEmitter::stderr();
    let syntax_stats = parsed.syntax_stats();
    let tree = parsed.into_typed_tree();
    let syntax_warnings = run_profile_phase(phases, "lint", || {
        let lints = parse::lint_source_tree(&tree);
        for lint in &lints {
            emitter.emit(&lint.diagnostic(&document), &diagnostic_source);
        }
        if parse::has_error_lints(&lints) {
            return Err(ExitCode::FAILURE);
        }
        Ok::<usize, ExitCode>(parse::count_warning_lints(&lints))
    })?;

    Ok(ProfileParsedSource {
        document,
        tree,
        syntax_warnings,
        syntax_stats,
    })
}

fn profile_lower_checked_source(
    parsed: &ProfileParsedSource,
    env: &TypeCheckEnv,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ProfileCheckedSource, ExitCode> {
    let hir = profile_lower_hir(&parsed.document, &parsed.tree, phases)?;
    let typecheck_report = profile_validate_hir(&hir, env, phases)?;
    let style = run_profile_phase(phases, "style_lower", || {
        lower_source_view_styles(
            &hir,
            &typecheck_report.style_catalog,
            parsed.document.text(),
        )
        .map_err(|error| {
            eprintln!("error: failed to lower checked View Style: {error}");
            ExitCode::FAILURE
        })
    })?;
    let line_task_groups = run_profile_phase(phases, "line_task_lower", || {
        lower::lower_source_line_tasks(&hir).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;

    Ok(ProfileCheckedSource {
        hir,
        style,
        line_task_groups: line_task_groups.len(),
        typecheck_report,
    })
}

fn profile_lower_product_awbc(
    selection: &SourceSelection,
    plan: &RuntimePlan,
    display: &LineDisplayCatalog,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<AwbcProgram, ExitCode> {
    let source_label = report_path(selection.path());
    let report = run_profile_phase(phases, "product_awbc_lower", || {
        AwbcLowerer::new(plan, display, &source_label)
            .lower()
            .map_err(|error| {
                match error {
                    AwbcLowerError::Lowering(diagnostics) => {
                        for diagnostic in diagnostics {
                            eprintln!(
                                "error[product-awbc {}]: {}",
                                diagnostic.path, diagnostic.message
                            );
                        }
                    }
                    AwbcLowerError::Verify(error) => {
                        eprintln!("error: product AWBC verification failed: {error}");
                    }
                }
                ExitCode::FAILURE
            })
    })?;
    for diagnostic in report.diagnostics {
        eprintln!(
            "warning[product-awbc {}]: {}",
            diagnostic.path, diagnostic.message
        );
    }
    Ok(report.program)
}

fn apply_script_manifest_entry_fallback(
    plan: &mut RuntimePlan,
    hir: &arcweft_lang_hir::model::HirModule,
) {
    if plan.entry_flow.is_some() || !plan.entries.is_empty() {
        return;
    }
    if let Some(flow) = script_manifest_goto_flow(hir) {
        match FlowRuntimeId::from_runtime_target_value(&flow) {
            Ok(flow) => plan.entry_flow = Some(flow),
            Err(error) => {
                eprintln!(
                    "warning: script manifest fallback ignored invalid goto target `{flow}`: {error}"
                );
            }
        }
    }
}

fn script_manifest_goto_flow(hir: &arcweft_lang_hir::model::HirModule) -> Option<String> {
    let manifest = collect_script_tests(hir);
    manifest
        .tests
        .iter()
        .filter(|test| test.kind == "scenario")
        .flat_map(|test| test.steps.iter().map(|step| step.text.as_str()))
        .find_map(parse_goto_flow_statement)
        .or_else(|| {
            manifest
                .benches
                .iter()
                .flat_map(|bench| bench.sections.iter().map(|section| section.text.as_str()))
                .find_map(parse_goto_flow_in_text)
        })
}

fn compile_project_runtime_plan(
    manifest: &Path,
    selection: &SourceSelection,
    semantic: &SelectionSemanticContext,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ProfileCompiledRuntimePlan, ExitCode> {
    let loaded = run_profile_phase(phases, "load_project", || {
        arcweft_project_loader::project::load(manifest).map_err(|error| {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        })
    })?;
    let source_document = Arc::clone(
        loaded
            .module_document(loaded.sources().root_module().module())
            .expect("loaded projects retain their root source document"),
    );
    let runtime_options = runtime_plan_options_for_selection(selection)?;
    let context = project_compilation_context(&loaded, selection, semantic)?;
    let compiled = run_profile_phase(phases, "project_compile", || {
        compile_project(loaded.sources(), &context, &runtime_options).map_err(|error| {
            print_project_compile_error(&error);
            ExitCode::FAILURE
        })
    })?;
    let syntax_stats =
        compiled
            .modules()
            .iter()
            .fold(SyntaxParseStats::default(), |mut stats, module| {
                add_syntax_stats(&mut stats, module.syntax_stats());
                stats
            });
    let runtime_plan_report = compiled.runtime_plan().clone();
    let plan = runtime_plan_report.plan;
    let line_display_catalog = runtime_plan_report.line_display_catalog;
    let runtime_plan_stats = runtime_plan_report.stats;
    let runtime_type_validation_stats = run_profile_phase(phases, "runtime_type_validate", || {
        let report = validate_runtime_plan_types(&plan, compiled.typecheck_report());
        if report.has_errors() {
            for diagnostic in report.diagnostics {
                eprintln!("error: {}: {}", diagnostic.path, diagnostic.message);
            }
            Err(ExitCode::FAILURE)
        } else {
            Ok(report.stats)
        }
    })?;
    let product_awbc = profile_lower_product_awbc(selection, &plan, &line_display_catalog, phases)?;
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
        hir: compiled.linked_hir().clone(),
        style: compiled.style().clone(),
        plan,
        syntax_warnings: compiled.syntax_warnings(),
        syntax_stats,
        line_task_groups: compiled.line_task_groups().len(),
        typecheck_report: compiled.typecheck_report().clone(),
        runtime_plan_stats,
        line_display_catalog,
        runtime_type_validation_stats,
        product_awbc,
        bytecode,
        bytecode_stats,
        aot_stats,
        source_document,
    })
}

fn add_syntax_stats(total: &mut SyntaxParseStats, item: &SyntaxParseStats) {
    total.cst_lex_passes += item.cst_lex_passes;
    total.punctuation_scans += item.punctuation_scans;
    total.punctuation_scan_bytes += item.punctuation_scan_bytes;
    total.line_owned_bytes += item.line_owned_bytes;
    total.block_owned_bytes += item.block_owned_bytes;
    total.raw_owned_bytes += item.raw_owned_bytes;
    total.wiki_scan_performed += item.wiki_scan_performed;
    total.dot_normalization_owned += item.dot_normalization_owned;
    total.dialogue_rescue_expr_parse_attempts += item.dialogue_rescue_expr_parse_attempts;
    total.numeric_seq_summaries += item.numeric_seq_summaries;
}

fn profile_lower_runtime_plan(
    selection: &SourceSelection,
    hir: &arcweft_lang_hir::model::HirModule,
    typecheck: &TypeCheckReport,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<RuntimePlanLowerReport, ExitCode> {
    run_profile_phase(phases, "runtime_plan_lower", || {
        let runtime_options = runtime_plan_options_for_selection(selection)?;
        lower::lower_source_runtime_plan_with_typecheck_stats_and_options(
            hir,
            typecheck,
            &runtime_options,
        )
        .map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })
}

pub(in crate::app) fn profile_lower_hir(
    document: &SourceDocument,
    tree: &arcweft_lang_syntax::ast::items::TypedSyntaxTree,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<arcweft_lang_hir::model::HirModule, ExitCode> {
    run_profile_phase(phases, "lower_hir", || {
        hir::lower_source_document(document, tree).map_err(|errors| {
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
        hir::resolve_hir_references_with_env(hir, env).map_err(|errors| {
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
