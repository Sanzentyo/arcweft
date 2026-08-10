use crate::app::project::{
    SelectionSemanticContext, SourceSelection, direct_project_compilation_input,
    print_project_compile_error, profile_project_compilation_context, project_compilation_context,
};
use crate::output::RuntimeProfilePhase;
use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_character::presentation_name::CharacterPresentationCatalogData;
use arcweft_compiler::{
    project::{
        CompiledProject, ProjectCompilationContext, ProjectCompilationSession, compile_project,
    },
    view::CompiledViewProduct,
};
use arcweft_core::{
    aot::{AotProgram, AotProgramStats},
    awbc::schema::AwbcProgram,
    bytecode::{BytecodeProgram, BytecodeStats},
    plan::RuntimePlan,
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    incremental::{ParsedSource, SyntaxParseStats},
};
use arcweft_presentation::fx::FxDefinition;
use arcweft_project::sources::ProjectSources;
use arcweft_runtime_plan::{
    awbc_lower::{AwbcLowerError, AwbcLowerer},
    flow::RuntimePlanLowerStats,
};
use arcweft_source::SourceDocument;
use arcweft_text_model::DialogueContentCatalog;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub(in crate::app) struct ProfileCompiledRuntimePlan {
    pub(in crate::app) compiled: Arc<CompiledProject>,
    pub(in crate::app) execution_diagnostics:
        Arc<arcweft_compiler::runtime_diagnostics::ExecutionDiagnosticContext>,
    pub(in crate::app) fx_definitions: Arc<[FxDefinition]>,
    pub(in crate::app) view_product: CompiledViewProduct,
    pub(in crate::app) plan: RuntimePlan,
    pub(in crate::app) syntax_warnings: usize,
    pub(in crate::app) syntax_stats: SyntaxParseStats,
    pub(in crate::app) line_task_groups: usize,
    pub(in crate::app) runtime_plan_stats: RuntimePlanLowerStats,
    pub(in crate::app) dialogue_content_catalog: DialogueContentCatalog,
    pub(in crate::app) character_presentation_catalog:
        Option<Arc<CharacterPresentationCatalogData>>,
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
    if selection.project_manifest().is_some() {
        return compile_project_runtime_plan(selection, semantic, phases);
    }
    let direct = direct_project_compilation_input(selection, semantic, phases)?;
    compile_project_sources_runtime_plan(
        direct.sources(),
        direct.parsed_sources(),
        None,
        direct.context(),
        selection,
        phases,
    )
}

fn profile_lower_product_awbc(
    selection: &SourceSelection,
    plan: &RuntimePlan,
    dialogue_content: &DialogueContentCatalog,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<AwbcProgram, ExitCode> {
    let source_label = report_path(selection.path());
    let report = run_profile_phase(phases, "product_awbc_lower", || {
        AwbcLowerer::new(plan, dialogue_content, &source_label)
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
    selection: &SourceSelection,
    semantic: &SelectionSemanticContext,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ProfileCompiledRuntimePlan, ExitCode> {
    let loaded = selection
        .loaded_project()
        .expect("project selections retain their accepted loaded project");
    let context = project_compilation_context(loaded, selection, semantic)?;
    compile_loaded_project_runtime_plan(loaded, &context, selection, phases)
}

fn compile_loaded_project_runtime_plan(
    loaded: &arcweft_project_loader::project::LoadedProject,
    context: &ProjectCompilationContext,
    selection: &SourceSelection,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ProfileCompiledRuntimePlan, ExitCode> {
    compile_project_sources_runtime_plan(
        loaded.sources(),
        loaded.module_parsed_source_map(),
        selection.compiler_session(),
        context,
        selection,
        phases,
    )
}

fn compile_project_sources_runtime_plan(
    sources: &ProjectSources,
    parsed_sources: &BTreeMap<CanonicalModulePath, ParsedSource>,
    compiler: Option<&Arc<Mutex<ProjectCompilationSession>>>,
    context: &ProjectCompilationContext,
    selection: &SourceSelection,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ProfileCompiledRuntimePlan, ExitCode> {
    let source_document = Arc::clone(sources.root_module().document());
    let compiled_project = if let Some(compiler) = compiler {
        let mut session = compiler.lock().map_err(|_| {
            eprintln!("error: project compiler session lock is poisoned");
            ExitCode::FAILURE
        })?;
        run_profile_phase(phases, "project_compile", || {
            compile_project(&mut session, sources, parsed_sources, context).map_err(|error| {
                print_project_compile_error(&error);
                ExitCode::FAILURE
            })
        })?
    } else {
        let mut session = ProjectCompilationSession::try_new().map_err(|error| {
            eprintln!("error: failed to create project compiler session: {error}");
            ExitCode::FAILURE
        })?;
        run_profile_phase(phases, "project_compile", || {
            compile_project(&mut session, sources, parsed_sources, context).map_err(|error| {
                print_project_compile_error(&error);
                ExitCode::FAILURE
            })
        })?
    };
    compile_accepted_project_runtime_plan(
        sources,
        selection,
        source_document,
        Arc::new(compiled_project),
        phases,
    )
}

/// Lowers executable products from one already accepted compiler generation.
///
/// Project build uses this entry after its cached compilation transaction so
/// bundle emission cannot compile a second HIR project and silently bind a
/// different runtime artifact identity.
pub(in crate::app) fn compile_accepted_project_runtime_plan(
    sources: &ProjectSources,
    selection: &SourceSelection,
    source_document: Arc<SourceDocument>,
    compiled: Arc<CompiledProject>,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ProfileCompiledRuntimePlan, ExitCode> {
    let execution_diagnostics =
        crate::app::runtime_artifact::bind_execution_diagnostics(selection, sources, &compiled)?;
    let source_map = compiled.view_product().product().source_map().clone();
    let syntax_stats =
        compiled
            .modules()
            .iter()
            .try_fold(SyntaxParseStats::ZERO, |stats, module| {
                stats.checked_add(*module.syntax_stats()).ok_or_else(|| {
                    eprintln!("error: accepted syntax work accounting overflowed usize");
                    ExitCode::FAILURE
                })
            })?;
    let runtime_plan_report = compiled.runtime_plan().clone();
    let plan = runtime_plan_report.plan;
    let dialogue_content_catalog = runtime_plan_report.dialogue_content_catalog;
    let character_presentation_catalog = runtime_plan_report.character_presentation_catalog;
    let runtime_plan_stats = runtime_plan_report.stats;
    let product_awbc =
        profile_lower_product_awbc(selection, &plan, &dialogue_content_catalog, phases)?;
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
    let line_task_groups = plan.line_task_groups.len();
    Ok(ProfileCompiledRuntimePlan {
        execution_diagnostics,
        fx_definitions: Arc::from(compiled.fx_definitions()),
        view_product: compiled.view_product().clone(),
        plan,
        syntax_warnings: compiled.syntax_warnings(),
        syntax_stats,
        line_task_groups,
        runtime_plan_stats,
        dialogue_content_catalog,
        character_presentation_catalog,
        product_awbc,
        bytecode,
        bytecode_stats,
        aot_stats,
        source_document,
        source_map,
        compiled,
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
