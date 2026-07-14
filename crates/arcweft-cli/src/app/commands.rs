use super::agent::{AgentHitTestOptions, AgentMcpOptions, AgentObserveOptions, AgentReplOptions};
use super::bundle::{BundleOptions, PatchBundleOptions, RunBundleOptions};
use super::cache::CacheCommand;
use super::debug::DebugCommand;
use super::import::ImportCommand;
use super::inspect::InspectOptions;
use super::jit::JitCheckOptions;
#[cfg(feature = "native-player")]
use super::native_player::NativePlayerOptions;
use super::project_commands::{CompileOptions, ProjectBuildOptions, ProjectCheckOptions};
use super::release::ReleaseCommand;
use super::release_sign::SignBundleOptions;
use super::runtime::options::{
    CliRunOptions, PlanOptions, RuntimeProfileOptions, RuntimeRunOptions, ScriptBenchOptions,
    ScriptTestOptions, ServeOptions,
};
use super::tooling::{CanonicalizeCommandOptions, ToolingCommandOptions};
use super::verify::{UnsafeOptions, VerifyOptions, VerifyTypesOptions};
use crate::toolchain_profile::ToolchainProfileOptions;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "arcw", about = "Arcweft language and runtime tooling")]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: CliCommand,
}

#[derive(Debug, Subcommand)]
pub(super) enum CliCommand {
    /// Checks the package selected by `arcw.toml`.
    Check(ProjectCheckOptions),
    /// Compiles one `.arcw` source directly without package discovery.
    Compile(CompileOptions),
    Import {
        #[command(subcommand)]
        command: ImportCommand,
    },
    /// Inspects an AWFB bundle header, manifest, and section index.
    Inspect(InspectOptions),
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    Debug {
        #[command(subcommand)]
        command: DebugCommand,
    },
    Verify(VerifyOptions),
    VerifyTypes(VerifyTypesOptions),
    Unsafe(UnsafeOptions),
    Plan(PlanOptions),
    Run(RuntimeRunOptions),
    Profile(RuntimeProfileOptions),
    Cli(CliRunOptions),
    Serve(ServeOptions),
    Test(ScriptTestOptions),
    Bench(ScriptBenchOptions),
    Bundle(BundleOptions),
    Patch(PatchBundleOptions),
    /// Appends a release signature envelope to an AWFB bundle.
    SignBundle(SignBundleOptions),
    /// Publishes and verifies AWFR release artifacts.
    Release {
        #[command(subcommand)]
        command: ReleaseCommand,
    },
    RunBundle(RunBundleOptions),
    /// Inspects and verifies the Arcweft filesystem cache.
    Cache {
        #[command(subcommand)]
        command: CacheCommand,
    },
    #[cfg(feature = "native-player")]
    PlayNative(NativePlayerOptions),
    /// Builds the package and writes project/plan artifacts under `target`.
    Build(ProjectBuildOptions),
    ToolchainProfile(ToolchainProfileOptions),
    Jit {
        #[command(subcommand)]
        command: JitCommand,
    },
    Fmt(ToolingCommandOptions),
    /// Canonicalizes semantic Arcweft sugar using the containing checked project.
    Canonicalize(CanonicalizeCommandOptions),
    Ids {
        #[command(subcommand)]
        command: IdsCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum IdsCommand {
    Materialize(ToolingCommandOptions),
}

#[derive(Debug, Subcommand)]
pub(super) enum AgentCommand {
    Observe(Box<AgentObserveOptions>),
    HitTest(Box<AgentHitTestOptions>),
    Mcp(Box<AgentMcpOptions>),
    Repl(Box<AgentReplOptions>),
    Rag {
        #[command(subcommand)]
        command: Box<AgentRagCommand>,
    },
    Script {
        #[command(subcommand)]
        command: Box<AgentScriptCommand>,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum AgentRagCommand {
    Index(super::agent::AgentRagIndexOptions),
    Query(super::agent::AgentRagQueryOptions),
    Explain(super::agent::AgentRagExplainOptions),
    ContextRead(super::agent::AgentRagContextReadOptions),
}

#[derive(Debug, Subcommand)]
pub(super) enum AgentScriptCommand {
    Build(super::agent::AgentScriptBuildOptions),
    Check(super::agent::AgentScriptCheckOptions),
    Replay(super::agent::AgentScriptReplayOptions),
    Run(Box<super::agent::AgentScriptRunOptions>),
    Trace(super::agent::AgentScriptTraceOptions),
}

#[derive(Debug, Subcommand)]
pub(super) enum JitCommand {
    Check(JitCheckOptions),
}
