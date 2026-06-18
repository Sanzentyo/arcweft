mod agent;
mod bundle;
mod commands;
pub(in crate::app) mod jit;
pub(crate) mod project;
pub(in crate::app) mod runtime;
pub(crate) mod shared;
mod tooling;
pub(in crate::app) mod verify;

use self::agent::agent_command;
use self::bundle::{bundle_command, run_bundle_command};
use self::commands::{AgentCommand, BuildCommand, Cli, CliCommand, JitCommand};
use self::jit::jit_command;
use self::runtime::{
    runtime_cli_command, runtime_plan_command, runtime_profile_command, runtime_run_command,
    runtime_serve_command, script_bench_command, script_test_command,
};
use self::tooling::{format_command, ids_command};
use self::verify::{check_command, unsafe_command, verify_command, verify_types_command};
use crate::output::{
    AotProfileStats, BorrowCheckProfileStats, BytecodeProfileStats, RuntimeExecutorTier,
    RuntimePlanProfileStats, RuntimePlanReport, RuntimeProfileCompiler, RuntimeProfilePhase,
    RuntimeProfileReport, RuntimeProfileRuntime, RuntimePureCallStatsSummary, RuntimeRunReport,
    RuntimeStepRunSummary, RuntimeTypeValidationProfileStats, ScriptBenchDeterministicSummary,
    ScriptBenchElapsedSummary, ScriptBenchMeasurementSummary, ScriptBenchPureHelperBatchSummary,
    ScriptBenchPureHelperDeterministicSummary, ScriptBenchPureHelperMeasurementSummary,
    ScriptBenchPureHelperRuntimeBatchSummary, ScriptBenchPureHelperStatsSummary,
    ScriptBenchPureHelperTimingSamples, ScriptBenchPureHelperTimingSummary, ScriptBenchRunReport,
    ScriptBenchRunSummary, ScriptBenchSectionRunSummary, ScriptTestFinalStatus,
    ScriptTestRunReport, ScriptTestRunSummary, ScriptTestStatus, TypeCheckProfileStats,
    flow_status_label,
};
use crate::server_adapter::{NativeHttpServerConfig, serve_native_http};
use crate::toolchain_profile::ToolchainProfileOptions;
use crate::{server_adapter, toolchain_profile};
use arcweft_bundle::BundleRuntimeSummary;
use arcweft_core::aot::{AotProgram, AotProgramStats};
use arcweft_core::bytecode::{BytecodeProgram, BytecodeStats};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::executor::{AotExecutor, BytecodeVmExecutor, RuntimeExecutor};
use arcweft_core::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
use arcweft_core::plan::{
    FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget, RuntimePlan,
    RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin, RuntimePureInputType,
    RuntimePureOutputType, RuntimeRouteSpec,
};
use arcweft_core::step::{
    RuntimePureCallStats, RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
    RuntimeStepResult, RuntimeStepStats,
};
use arcweft_core::{
    pure::{
        AotPureFunctionBackend, AotPureI64Plan, PureFunctionBackendKind, PureFunctionRequest,
        PureFunctionResult, PureFunctionStats, RuntimeI64Args, RuntimePureCallBackend,
        VmPureFunctionBackend, VmPureFunctionScratch, compare_pure_function_backend,
    },
    value::{
        DenseSeq, RuntimeBinaryOp, RuntimeBinding, RuntimeCallTarget, RuntimeExpr,
        RuntimeIntrinsic, RuntimeSeq, RuntimeUnaryOp, RuntimeValue, runtime_sequence_dense_f32,
        runtime_sequence_values,
    },
};
use arcweft_host_adapter::HostCallPolicy;
use arcweft_lang_jit_cranelift::{
    CompiledPureI64Batch, CompiledPureI64Inputs, CraneliftPureFunctionBackend,
};
use arcweft_lang_sema::check::TypeCheckReport;
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_syntax::expr::{CallArg, Expr, Literal, parse_expr};
use arcweft_launch::LaunchKind;
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_accelerator::{
    RuntimePureAccelerator, RuntimePureAcceleratorConfig, RuntimePureBackendMode,
    RuntimePureWorkerCount, math::RuntimeMathBackend,
};
use arcweft_runtime_host::{
    BundleRunnerExecutor, BundleRunnerPhase, BundleRunnerStepMode, BundleRunnerStepSummary,
    HostSystemInfo, NativeAdapterRegistrar, NativeSchedulerStats, NativeTaskBridge,
    NativeTaskClassCounts, NativeTaskStats, RuntimeExecutorMathStatsSummary, RuntimeExecutorStats,
    host_system_info, runtime_executor_stats,
};
use arcweft_runtime_plan::flow::{
    RuntimePlanLowerReport, RuntimePlanLowerStats, lower_runtime_plan_with_options,
    lower_runtime_plan_with_stats_and_options,
};
use arcweft_runtime_plan::pure::{
    PureHelperCandidate, PureHelperLowerError, lower_pure_helper_candidates,
};
use arcweft_test::{BenchSection, ScriptBench, ScriptStep, ScriptTest, collect_script_tests};
use arcweft_verify::{RuntimeTypeValidationStats, validate_runtime_plan_types};
use clap::{Args, Parser, ValueEnum};
use std::ffi::OsString;
use std::fs;
use std::net::SocketAddr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

/// Runs the Arcweft CLI with the standard native adapters.
pub fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    run_with_native_adapters(args, &[])
}

/// Runs the Arcweft CLI and registers additional native host adapters.
pub fn run_with_native_adapters<I, T>(
    args: I,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match run_cli(Cli::parse_from(args), adapter_registrars) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run_cli(cli: Cli, adapter_registrars: &[NativeAdapterRegistrar]) -> Result<(), ExitCode> {
    match cli.command {
        CliCommand::Check(options) => check_command(&options),
        CliCommand::Agent { command } => agent_command(command, adapter_registrars),
        CliCommand::Verify(options) => verify_command(&options),
        CliCommand::VerifyTypes(options) => verify_types_command(&options, adapter_registrars),
        CliCommand::Unsafe(options) => unsafe_command(&options),
        CliCommand::Plan(options) => runtime_plan_command(&options),
        CliCommand::Run(options) => runtime_run_command(&options, adapter_registrars),
        CliCommand::Profile(options) => runtime_profile_command(&options, adapter_registrars),
        CliCommand::Cli(options) => runtime_cli_command(&options, adapter_registrars),
        CliCommand::Serve(options) => runtime_serve_command(&options, adapter_registrars),
        CliCommand::Test(options) => script_test_command(&options, adapter_registrars),
        CliCommand::Bench(options) => script_bench_command(&options, adapter_registrars),
        CliCommand::Bundle(options) => bundle_command(&options),
        CliCommand::RunBundle(options) => run_bundle_command(&options, adapter_registrars),
        CliCommand::Build { command } => match command {
            BuildCommand::Bundle(options) => bundle_command(&options),
        },
        CliCommand::ToolchainProfile(options) => toolchain_profile::run(&options),
        CliCommand::Jit { command } => jit_command(command),
        CliCommand::Fmt(options) => format_command(&options),
        CliCommand::Ids { command } => ids_command(command),
    }
}
