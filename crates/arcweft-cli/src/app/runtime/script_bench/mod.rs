mod run;
mod samples;

use super::options::ScriptBenchOptions;
use super::profile::{compile_profile_runtime_plan, report_path};
use super::source::source_runtime_program_from_compiled;
use crate::app::project::{
    SourceSelection, require_profile_kind, resolve_source_selection,
    runtime_pure_config_for_selection, semantic_context_for_selection,
};
use crate::app::shared::print_json;
use arcweft_launch::LaunchKind;
use arcweft_runtime_accelerator::RuntimePureAcceleratorConfig;
use arcweft_runtime_host::{NativeAdapterRegistrar, NativeFileRoots};
use arcweft_test::collect_script_tests;
use run::run_script_bench;
use std::process::ExitCode;

#[derive(Clone, Copy)]
pub(in crate::app) struct BenchRuntimeContext<'a> {
    pub(in crate::app) pure_config: RuntimePureAcceleratorConfig,
    pub(in crate::app) host_policy: &'a arcweft_host_adapter::HostCallPolicy,
    pub(in crate::app) adapter_registrars: &'a [NativeAdapterRegistrar],
    pub(in crate::app) file_roots: &'a NativeFileRoots,
    pub(in crate::app) bundle_assets: super::steps::RuntimeBundleAssets<'a>,
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
    let source_runtime = source_runtime_program_from_compiled(selection, compiled, None)?;
    let file_roots = selection.native_file_roots();
    let manifest = collect_script_tests(source_runtime.compiled.analysis_lease().hir_project());
    let runtime = BenchRuntimeContext {
        pure_config,
        host_policy: &source_runtime.host_policy,
        adapter_registrars,
        file_roots: &file_roots,
        bundle_assets: source_runtime.bundle_assets(),
        execution_diagnostics: &source_runtime.execution_diagnostics,
    };
    let benches = manifest
        .benches
        .iter()
        .map(|bench| {
            run_script_bench(
                bench,
                &source_runtime.plan,
                selection.path(),
                options,
                runtime,
            )
        })
        .collect();
    let output = crate::output::ScriptBenchRunReport {
        source: report_path(selection.path()),
        syntax_warnings: source_runtime.syntax_warnings,
        line_task_groups: source_runtime.line_task_groups,
        compiler: source_runtime.compiler,
        phases,
        benches,
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
