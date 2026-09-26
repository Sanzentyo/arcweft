use super::profile::{ProfileCompiledRuntimePlan, compile_profile_runtime_plan};
use super::steps::RuntimeBundleAssets;
use crate::app::bundle::compile_bundle_from_profile_runtime_plan_with_adapter;
use crate::app::project::{
    SourceSelection, native_host_policy_for_selection_with_adapter, semantic_context_for_selection,
};
use crate::output::{
    AotProfileStats, AwbcProfileStats, FinalSemanticProfileStats, RuntimePlanProfileStats,
    RuntimeProfileCompiler, RuntimeProfilePhase,
};
use arcweft_adapter_context::standard;
use arcweft_bundle::{ArcweftBundle, BundleArtifactIdentity, BundleVirtualFileSpace};
use arcweft_compiler::project::CompiledProject;
use arcweft_core::task::GenerationId;
use arcweft_host_adapter::HostCallPolicy;
use arcweft_source::SourceDocument;
use std::process::ExitCode;
use std::sync::Arc;

/// One accepted source compilation and the logical bundle catalog used to
/// resolve its generation-owned asset tasks.
pub(in crate::app) struct SourceRuntimeProgram {
    pub(in crate::app) compiled: Arc<CompiledProject>,
    pub(in crate::app) source_document: Arc<SourceDocument>,
    pub(in crate::app) execution_diagnostics:
        Arc<arcweft_compiler::runtime_diagnostics::ExecutionDiagnosticContext>,
    pub(in crate::app) plan: arcweft_core::plan::RuntimePlan,
    pub(in crate::app) bundle: Arc<ArcweftBundle>,
    pub(in crate::app) bundle_artifact_identity: BundleArtifactIdentity,
    pub(in crate::app) bundle_asset_context: arcweft_core::value::RuntimeBundleAssetContext,
    pub(in crate::app) host_policy: HostCallPolicy,
    pub(in crate::app) compiler: RuntimeProfileCompiler,
    pub(in crate::app) syntax_warnings: usize,
    pub(in crate::app) line_task_groups: usize,
}

impl SourceRuntimeProgram {
    pub(in crate::app) fn bundle_assets(&self) -> RuntimeBundleAssets<'_> {
        RuntimeBundleAssets {
            bundle: &self.bundle,
            artifact_identity: self.bundle_artifact_identity,
            context: self.bundle_asset_context,
        }
    }
}

pub(in crate::app) fn compile_source_runtime_program(
    selection: &SourceSelection,
    adapter_override: Option<&str>,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<SourceRuntimeProgram, ExitCode> {
    let semantic = semantic_context_for_selection(selection, adapter_override)?;
    let compiled = compile_profile_runtime_plan(selection, &semantic, phases)?;
    source_runtime_program_from_compiled(selection, compiled, adapter_override)
}

pub(in crate::app) fn source_runtime_program_from_compiled(
    selection: &SourceSelection,
    compiled: ProfileCompiledRuntimePlan,
    adapter_override: Option<&str>,
) -> Result<SourceRuntimeProgram, ExitCode> {
    let compiled_project = Arc::clone(&compiled.compiled);
    let source_document = Arc::clone(&compiled.source_document);
    let execution_diagnostics = Arc::clone(&compiled.execution_diagnostics);
    let syntax_warnings = compiled.syntax_warnings;
    let line_task_groups = compiled.line_task_groups;
    let plan = compiled.plan.clone();
    let compiler = RuntimeProfileCompiler {
        syntax: compiled.syntax_stats.into(),
        semantic: FinalSemanticProfileStats::from(
            compiled.compiled.analysis_lease().final_analysis().as_ref(),
        ),
        runtime_plan: RuntimePlanProfileStats::from(compiled.runtime_plan_stats),
        awbc: AwbcProfileStats::from(&compiled.product_awbc),
        aot: AotProfileStats::from(&compiled.aot_stats),
    };
    let bundle = compile_bundle_from_profile_runtime_plan_with_adapter(
        selection,
        compiled,
        vec![BundleVirtualFileSpace::Asset],
        adapter_override,
    )?
    .bundle;
    let bundle_artifact_identity = bundle
        .logical_identity()
        .map(|identity| BundleArtifactIdentity::LogicalBundle { identity })
        .map_err(|error| {
            eprintln!("error: failed to identify the source runtime bundle: {error}");
            ExitCode::FAILURE
        })?;
    let bundle_asset_context = bundle_artifact_identity
        .bundle_asset_context(GenerationId::new(0))
        .map_err(|error| {
            eprintln!("error: failed to bind source bundle asset context: {error}");
            ExitCode::FAILURE
        })?;
    let mut host_policy =
        native_host_policy_for_selection_with_adapter(selection, adapter_override)?;
    if bundle
        .manifest
        .required_host_calls
        .iter()
        .any(|call| matches!(call.as_str(), "asset.image" | "asset.voice"))
    {
        host_policy = host_policy.union(HostCallPolicy::from_manifests([
            standard::bundle_asset_manifest(),
        ]));
    }

    Ok(SourceRuntimeProgram {
        compiled: compiled_project,
        source_document,
        execution_diagnostics,
        plan,
        bundle: Arc::new(bundle),
        bundle_artifact_identity,
        bundle_asset_context,
        host_policy,
        compiler,
        syntax_warnings,
        line_task_groups,
    })
}

#[cfg(test)]
mod tests {
    use super::compile_source_runtime_program;
    use crate::app::project::{ProfileOptions, resolve_source_selection};
    use crate::app::runtime::invocation::seal_named_flow_invocation;
    use crate::app::runtime::options::{CliRuntimeExecutorTier, CliRuntimeStepMode};
    use crate::app::runtime::steps::{
        NativeRunHost, NativeRunSource, RuntimeStepRunConfig, run_runtime_flow_steps,
    };
    use arcweft_core::engine::{FlowExit, FlowFiberStatus};
    use arcweft_core::value::{RuntimeBinding, RuntimeEntityReference, RuntimeValue};
    use arcweft_id::{DeclarationIdentityFamily, PublicId};
    use arcweft_runtime_accelerator::RuntimePureAcceleratorConfig;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_PROJECT_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempProject(PathBuf);

    impl TempProject {
        fn new() -> Self {
            let index = TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "arcweft-source-asset-load-{}-{index}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("temporary source project directory is created");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn source_headless_run_resolves_a_bundled_png_asset_load() {
        let project = TempProject::new();
        let source_path = project.path().join("asset_load.arcw");
        fs::write(
            &source_path,
            r#"
entry cli @entry.main {
    goto @flow.launch
}

flow launch() -> String {
    return "unused_entry"
}

flow main(background: Ref<Asset>) -> String effects { asset.read } {
    return match (await asset.image(background)) {
        .Ok(_) => "asset_ready"
        .Err(_) => "asset_error"
    }
}
"#,
        )
        .expect("source program is written");
        let asset_directory = project.path().join("assets/bg");
        fs::create_dir_all(&asset_directory).expect("asset directory is created");
        fs::write(asset_directory.join("room.png"), one_pixel_png()).expect("PNG asset is written");
        let selection = resolve_source_selection(Some(&source_path), &ProfileOptions::default())
            .expect("direct source selection loads sibling authored assets");

        let mut phases = Vec::new();
        let runtime = compile_source_runtime_program(&selection, None, &mut phases)
            .expect("source runtime plan and bundle compile");
        let invocation = seal_named_flow_invocation(
            runtime.plan.clone(),
            "flow.main",
            &[RuntimeBinding {
                name: "background".to_owned(),
                value: RuntimeValue::EntityRef(
                    RuntimeEntityReference::try_project(
                        DeclarationIdentityFamily::Asset,
                        PublicId::try_new("asset.bg.room").expect("asset reference ID is valid"),
                    )
                    .expect("asset reference has the Asset identity family"),
                ),
            }],
        )
        .expect("runtime flow invocation binds the logical asset reference");
        let file_roots = selection.native_file_roots();
        let trace = run_runtime_flow_steps(
            invocation,
            NativeRunHost {
                source: Some(NativeRunSource::new(selection.path(), &file_roots)),
                bundle_assets: Some(runtime.bundle_assets()),
                policy: &runtime.host_policy,
                adapter_registrars: &[crate::app::desktop_native_adapter_registrar],
                cli_args: &[],
            },
            RuntimeStepRunConfig {
                steps: 8,
                mode: CliRuntimeStepMode::Drain,
                max_ops: 32,
                executor: CliRuntimeExecutorTier::BytecodeVm,
                pure_config: RuntimePureAcceleratorConfig::default(),
            },
            &runtime.execution_diagnostics,
        )
        .expect("native source host completes the asset task");

        assert_eq!(
            trace.final_status,
            FlowFiberStatus::Done(FlowExit::Return("asset_ready".to_owned()))
        );
        assert_eq!(trace.native_io.completed_tasks, 1);
        assert_eq!(trace.native_io.failed_tasks, 0);
    }

    fn one_pixel_png() -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("PNG header writes");
            writer
                .write_image_data(&[255, 0, 0, 255])
                .expect("PNG pixel data writes");
        }
        bytes
    }
}
