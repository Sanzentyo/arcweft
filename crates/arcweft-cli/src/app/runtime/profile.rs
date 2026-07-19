use crate::app::project::{
    SelectionSemanticContext, SourceSelection, direct_project_compilation_input,
    print_project_compile_error, profile_project_compilation_context, project_compilation_context,
    runtime_plan_options_for_selection,
};
use crate::output::RuntimeProfilePhase;
use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_compiler::{
    project::{ProjectCompilationContext, compile_project},
    style::CompiledViewStyleArtifact,
};
use arcweft_core::{
    aot::{AotProgram, AotProgramStats},
    awbc::schema::AwbcProgram,
    bytecode::{BytecodeProgram, BytecodeStats},
    plan::RuntimePlan,
};
use arcweft_lang_sema::check::TypeCheckReport;
use arcweft_lang_syntax::cst::SyntaxParseStats;
use arcweft_project::sources::ProjectSources;
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_plan::{
    awbc_lower::{AwbcLowerError, AwbcLowerer},
    flow::RuntimePlanLowerStats,
};
use arcweft_source::SourceDocument;
use arcweft_verify::{RuntimeTypeValidationStats, validate_runtime_plan_types};
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
    pub(in crate::app) source_map: SourceMapSection,
}

pub(in crate::app) fn compile_profile_runtime_plan(
    selection: &SourceSelection,
    semantic: &SelectionSemanticContext,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ProfileCompiledRuntimePlan, ExitCode> {
    if let Some(topology) = semantic.profile_topology() {
        let context = profile_project_compilation_context(topology, semantic)?;
        return compile_loaded_project_runtime_plan(
            topology.loaded_project(),
            &context,
            selection,
            phases,
        );
    }
    if let Some(manifest) = selection.project_manifest() {
        return compile_project_runtime_plan(manifest, selection, semantic, phases);
    }
    let direct = direct_project_compilation_input(selection, semantic, phases)?;
    compile_project_sources_runtime_plan(direct.sources(), direct.context(), selection, phases)
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
    let context = project_compilation_context(&loaded, selection, semantic)?;
    compile_loaded_project_runtime_plan(&loaded, &context, selection, phases)
}

fn compile_loaded_project_runtime_plan(
    loaded: &arcweft_project_loader::project::LoadedProject,
    context: &ProjectCompilationContext,
    selection: &SourceSelection,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ProfileCompiledRuntimePlan, ExitCode> {
    compile_project_sources_runtime_plan(loaded.sources(), context, selection, phases)
}

fn compile_project_sources_runtime_plan(
    sources: &ProjectSources,
    context: &ProjectCompilationContext,
    selection: &SourceSelection,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ProfileCompiledRuntimePlan, ExitCode> {
    let source_document = Arc::clone(sources.root_module().document());
    let source_map = SourceMapSection::try_from_documents(
        &sources
            .modules()
            .map(|source| source.document().as_ref())
            .collect::<Vec<_>>(),
    )
    .map_err(|error| {
        eprintln!("error: failed to build project source map: {error}");
        ExitCode::FAILURE
    })?;
    let runtime_options = runtime_plan_options_for_selection(selection)?;
    let compiled = run_profile_phase(phases, "project_compile", || {
        compile_project(sources, context, &runtime_options).map_err(|error| {
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
        source_map,
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
    total.dialogue_rescue_expr_parse_attempts += item.dialogue_rescue_expr_parse_attempts;
    total.numeric_seq_summaries += item.numeric_seq_summaries;
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
