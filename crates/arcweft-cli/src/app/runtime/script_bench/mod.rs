mod run;
mod samples;

use super::options::ScriptBenchOptions;
use super::profile::{compile_profile_runtime_plan, report_path};
use crate::app::project::{
    SourceSelection, native_host_policy_for_selection, require_profile_kind,
    resolve_source_selection, runtime_pure_config_for_selection, semantic_context_for_selection,
};
use crate::app::shared::print_json;
use crate::output::{
    AotProfileStats, BytecodeProfileStats, FinalSemanticProfileStats, RuntimePlanProfileStats,
    RuntimeProfileCompiler,
};
use arcweft_host_adapter::HostCallPolicy;
use arcweft_launch::LaunchKind;
use arcweft_runtime_accelerator::RuntimePureAcceleratorConfig;
use arcweft_runtime_host::{NativeAdapterRegistrar, NativeFileRoots};
use arcweft_test::collect_script_tests;
use run::run_script_bench;
use std::process::ExitCode;

#[derive(Clone, Copy)]
pub(in crate::app) struct BenchRuntimeContext<'a> {
    pub(in crate::app) pure_config: RuntimePureAcceleratorConfig,
    pub(in crate::app) host_policy: &'a HostCallPolicy,
    pub(in crate::app) adapter_registrars: &'a [NativeAdapterRegistrar],
    pub(in crate::app) file_roots: &'a NativeFileRoots,
    pub(in crate::app) execution_diagnostics:
        &'a arcweft_compiler::runtime_diagnostics::ExecutionDiagnosticContext,
}

pub(in crate::app) fn script_bench_command(
    options: &ScriptBenchOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    require_profile_kind(&selection, LaunchKind::Bench, "bench")?;
    script_bench_selection(&selection, options, adapter_registrars)
}

pub(in crate::app) fn script_bench_selection(
    selection: &SourceSelection,
    options: &ScriptBenchOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let pure_config = runtime_pure_config_for_selection(
        selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.pure_object_artifacts,
        options.math_backend,
        options.math_wgpu_min_elements,
    );
    let mut phases = Vec::new();
    let semantic = semantic_context_for_selection(selection, None)?;
    let compiled = compile_profile_runtime_plan(selection, &semantic, &mut phases)?;
    let host_policy = native_host_policy_for_selection(selection)?;
    let file_roots = selection.native_file_roots();
    let manifest = collect_script_tests(compiled.compiled.hir_project());
    let pure_helpers = compiled.plan.pure_helpers.clone();
    let runtime = BenchRuntimeContext {
        pure_config,
        host_policy: &host_policy,
        adapter_registrars,
        file_roots: &file_roots,
        execution_diagnostics: &compiled.execution_diagnostics,
    };
    let output = crate::output::ScriptBenchRunReport {
        source: report_path(selection.path()),
        syntax_warnings: compiled.syntax_warnings,
        line_task_groups: compiled.line_task_groups,
        compiler: RuntimeProfileCompiler {
            syntax: compiled.syntax_stats.into(),
            semantic: FinalSemanticProfileStats::from(compiled.compiled.final_analysis().as_ref()),
            runtime_plan: RuntimePlanProfileStats::from(compiled.runtime_plan_stats),
            bytecode: BytecodeProfileStats::from(&compiled.bytecode_stats),
            aot: AotProfileStats::from(&compiled.aot_stats),
        },
        phases,
        benches: manifest
            .benches
            .iter()
            .map(|bench| {
                run_script_bench(
                    bench,
                    &compiled.plan,
                    &pure_helpers,
                    selection.path(),
                    options,
                    runtime,
                )
            })
            .collect(),
    };
    let failed = output.benches.iter().any(|bench| bench.status == "failed");
    if options.json {
        print_json(&output)?;
    } else {
        for bench in &output.benches {
            println!(
                "{} {} ({} section(s))",
                bench.id,
                bench.status,
                bench.sections.len()
            );
            for diagnostic in &bench.diagnostics {
                println!("  diagnostic {diagnostic}");
            }
        }
        println!(
            "ok: {} ({} script bench(es))",
            selection.path().display(),
            output.benches.len()
        );
    }
    if failed {
        Err(ExitCode::FAILURE)
    } else {
        Ok(())
    }
}
