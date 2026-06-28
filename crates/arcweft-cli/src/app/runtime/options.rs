use crate::app::project::ProfileOptions;
use crate::app::runtime::parse::{parse_runtime_binding_arg, parse_runtime_pure_workers};
use crate::output::RuntimeExecutorTier;
use arcweft_core::value::RuntimeBinding;
use arcweft_runtime_accelerator::{
    RuntimePureBackendMode, RuntimePureWorkerCount, math::RuntimeMathBackend,
};
use arcweft_runtime_host::{BundleRunnerExecutor, BundleRunnerStepMode};
use clap::{Args, ValueEnum};
use std::net::SocketAddr;
use std::path::PathBuf;

impl From<BundleRunnerExecutor> for CliRuntimeExecutorTier {
    fn from(value: BundleRunnerExecutor) -> Self {
        match value {
            BundleRunnerExecutor::AwbcProduct => Self::AwbcProduct,
            BundleRunnerExecutor::BytecodeVm => Self::BytecodeVm,
            BundleRunnerExecutor::Aot => Self::Aot,
        }
    }
}

impl From<CliRuntimeExecutorTier> for BundleRunnerExecutor {
    fn from(value: CliRuntimeExecutorTier) -> Self {
        match value {
            CliRuntimeExecutorTier::AwbcProduct => Self::AwbcProduct,
            CliRuntimeExecutorTier::BytecodeVm => Self::BytecodeVm,
            CliRuntimeExecutorTier::Aot => Self::Aot,
        }
    }
}

impl From<BundleRunnerStepMode> for CliRuntimeStepMode {
    fn from(value: BundleRunnerStepMode) -> Self {
        match value {
            BundleRunnerStepMode::OneOp => Self::OneOp,
            BundleRunnerStepMode::Drain => Self::Drain,
            BundleRunnerStepMode::Game => Self::Game,
            BundleRunnerStepMode::Server => Self::Server,
        }
    }
}

impl From<CliRuntimeStepMode> for BundleRunnerStepMode {
    fn from(value: CliRuntimeStepMode) -> Self {
        match value {
            CliRuntimeStepMode::OneOp => Self::OneOp,
            CliRuntimeStepMode::Drain => Self::Drain,
            CliRuntimeStepMode::Game => Self::Game,
            CliRuntimeStepMode::Server => Self::Server,
        }
    }
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct PlanOptions {
    pub(in crate::app) path: Option<PathBuf>,
    #[command(flatten)]
    pub(in crate::app) profile: ProfileOptions,
    #[arg(long)]
    pub(in crate::app) json: bool,
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct RuntimeRunOptions {
    pub(in crate::app) path: Option<PathBuf>,
    #[command(flatten)]
    pub(in crate::app) profile: ProfileOptions,
    #[arg(long, value_enum, default_value_t = CliRuntimeRunner::Auto)]
    pub(in crate::app) runner: CliRuntimeRunner,
    #[arg(long, conflicts_with = "flow")]
    pub(in crate::app) entry: Option<String>,
    #[arg(long, conflicts_with = "entry")]
    pub(in crate::app) flow: Option<String>,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    pub(in crate::app) executor: CliRuntimeExecutorTier,
    #[arg(long, value_enum)]
    pub(in crate::app) pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pub(in crate::app) pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pub(in crate::app) pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pub(in crate::app) pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    pub(in crate::app) math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    pub(in crate::app) math_wgpu_min_elements: Option<usize>,
    #[arg(long, default_value_t = 1)]
    pub(in crate::app) steps: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::OneOp)]
    pub(in crate::app) mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 1)]
    pub(in crate::app) max_ops: usize,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    pub(in crate::app) values: Vec<RuntimeBinding>,
    #[arg(long)]
    pub(in crate::app) watch: bool,
    #[arg(long, default_value_t = 500)]
    pub(in crate::app) watch_poll_ms: u64,
    #[arg(long, hide = true, default_value_t = 0)]
    pub(in crate::app) watch_iterations: usize,
    #[arg(long = "text-input-trace-out")]
    pub(in crate::app) text_input_trace_out: Option<PathBuf>,
    #[arg(long)]
    pub(in crate::app) json: bool,
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct RuntimeProfileOptions {
    pub(in crate::app) path: Option<PathBuf>,
    #[command(flatten)]
    pub(in crate::app) profile: ProfileOptions,
    #[arg(long, conflicts_with = "flow")]
    pub(in crate::app) entry: Option<String>,
    #[arg(long, conflicts_with = "entry")]
    pub(in crate::app) flow: Option<String>,
    #[arg(long)]
    pub(in crate::app) adapter: Option<String>,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    pub(in crate::app) executor: CliRuntimeExecutorTier,
    #[arg(long, value_enum)]
    pub(in crate::app) pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pub(in crate::app) pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pub(in crate::app) pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pub(in crate::app) pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    pub(in crate::app) math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    pub(in crate::app) math_wgpu_min_elements: Option<usize>,
    #[arg(long, default_value_t = 1)]
    pub(in crate::app) steps: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    pub(in crate::app) mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 32)]
    pub(in crate::app) max_ops: usize,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    pub(in crate::app) values: Vec<RuntimeBinding>,
    #[arg(long)]
    pub(in crate::app) json: bool,
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct CliRunOptions {
    pub(in crate::app) path: Option<PathBuf>,
    #[command(flatten)]
    pub(in crate::app) profile: ProfileOptions,
    #[arg(long)]
    pub(in crate::app) entry: Option<String>,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    pub(in crate::app) executor: CliRuntimeExecutorTier,
    #[arg(long, value_enum)]
    pub(in crate::app) pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pub(in crate::app) pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pub(in crate::app) pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pub(in crate::app) pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    pub(in crate::app) math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    pub(in crate::app) math_wgpu_min_elements: Option<usize>,
    #[arg(long, default_value_t = 1)]
    pub(in crate::app) steps: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    pub(in crate::app) mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 32)]
    pub(in crate::app) max_ops: usize,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    pub(in crate::app) values: Vec<RuntimeBinding>,
    #[arg(long)]
    pub(in crate::app) json: bool,
    #[arg(last = true)]
    pub(in crate::app) args: Vec<String>,
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct ServeOptions {
    pub(in crate::app) path: Option<PathBuf>,
    #[command(flatten)]
    pub(in crate::app) profile: ProfileOptions,
    #[arg(long)]
    pub(in crate::app) entry: Option<String>,
    #[arg(long)]
    pub(in crate::app) adapter: Option<String>,
    #[arg(long)]
    pub(in crate::app) listen: Option<SocketAddr>,
    #[arg(long)]
    pub(in crate::app) once: bool,
    #[arg(long, value_enum)]
    pub(in crate::app) pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pub(in crate::app) pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pub(in crate::app) pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pub(in crate::app) pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    pub(in crate::app) math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    pub(in crate::app) math_wgpu_min_elements: Option<usize>,
    #[arg(long, default_value_t = 128)]
    pub(in crate::app) max_ops: usize,
    #[arg(long)]
    pub(in crate::app) json: bool,
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct ScriptTestOptions {
    pub(in crate::app) path: Option<PathBuf>,
    #[command(flatten)]
    pub(in crate::app) profile: ProfileOptions,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    pub(in crate::app) executor: CliRuntimeExecutorTier,
    #[arg(long, value_enum)]
    pub(in crate::app) pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pub(in crate::app) pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pub(in crate::app) pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pub(in crate::app) pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    pub(in crate::app) math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    pub(in crate::app) math_wgpu_min_elements: Option<usize>,
    #[arg(long, default_value_t = 32)]
    pub(in crate::app) steps: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    pub(in crate::app) mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 32)]
    pub(in crate::app) max_ops: usize,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    pub(in crate::app) values: Vec<RuntimeBinding>,
    #[arg(long)]
    pub(in crate::app) json: bool,
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct ScriptBenchOptions {
    pub(in crate::app) path: Option<PathBuf>,
    #[command(flatten)]
    pub(in crate::app) profile: ProfileOptions,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    pub(in crate::app) executor: CliRuntimeExecutorTier,
    #[arg(long, value_enum)]
    pub(in crate::app) pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pub(in crate::app) pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pub(in crate::app) pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pub(in crate::app) pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    pub(in crate::app) math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    pub(in crate::app) math_wgpu_min_elements: Option<usize>,
    #[arg(long, default_value_t = 32)]
    pub(in crate::app) steps: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    pub(in crate::app) mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 32)]
    pub(in crate::app) max_ops: usize,
    #[arg(long, default_value_t = 1)]
    pub(in crate::app) iterations: usize,
    #[arg(long, default_value_t = 0)]
    pub(in crate::app) warmup: usize,
    #[arg(long, default_value_t = 5)]
    pub(in crate::app) samples: usize,
    #[arg(long, default_value_t = 0)]
    pub(in crate::app) input_seed: u64,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    pub(in crate::app) values: Vec<RuntimeBinding>,
    #[arg(long)]
    pub(in crate::app) json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(in crate::app) enum CliRuntimeStepMode {
    OneOp,
    Drain,
    Game,
    Server,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(in crate::app) enum CliRuntimeRunner {
    Auto,
    Native,
    Headless,
    Web,
}

impl CliRuntimeRunner {
    pub(in crate::app) const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Native => "native",
            Self::Headless => "headless",
            Self::Web => "web",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(in crate::app) enum CliRuntimeExecutorTier {
    AwbcProduct,
    BytecodeVm,
    Aot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(in crate::app) enum CliRuntimePureBackend {
    Auto,
    Vm,
    Aot,
    Jit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(in crate::app) enum CliRuntimeMathBackend {
    Auto,
    Scalar,
    Glam,
    Ndarray,
    Wgpu,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum CliRuntimePureWorkers {
    Auto,
    Fixed(usize),
}

impl From<CliRuntimeExecutorTier> for RuntimeExecutorTier {
    fn from(tier: CliRuntimeExecutorTier) -> Self {
        match tier {
            CliRuntimeExecutorTier::AwbcProduct => Self::AwbcProduct,
            CliRuntimeExecutorTier::BytecodeVm => Self::BytecodeVm,
            CliRuntimeExecutorTier::Aot => Self::Aot,
        }
    }
}

impl From<CliRuntimePureBackend> for RuntimePureBackendMode {
    fn from(value: CliRuntimePureBackend) -> Self {
        match value {
            CliRuntimePureBackend::Auto => Self::Auto,
            CliRuntimePureBackend::Vm => Self::Vm,
            CliRuntimePureBackend::Aot => Self::Aot,
            CliRuntimePureBackend::Jit => Self::Jit,
        }
    }
}

impl From<CliRuntimeMathBackend> for RuntimeMathBackend {
    fn from(value: CliRuntimeMathBackend) -> Self {
        match value {
            CliRuntimeMathBackend::Auto => Self::Auto,
            CliRuntimeMathBackend::Scalar => Self::Scalar,
            CliRuntimeMathBackend::Glam => Self::Glam,
            CliRuntimeMathBackend::Ndarray => Self::Ndarray,
            CliRuntimeMathBackend::Wgpu => Self::Wgpu,
        }
    }
}

impl From<CliRuntimePureWorkers> for RuntimePureWorkerCount {
    fn from(value: CliRuntimePureWorkers) -> Self {
        match value {
            CliRuntimePureWorkers::Auto => Self::Auto,
            CliRuntimePureWorkers::Fixed(value) => Self::Fixed(value),
        }
    }
}
