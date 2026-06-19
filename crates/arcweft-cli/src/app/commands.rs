use super::agent::{AgentHitTestOptions, AgentMcpOptions, AgentObserveOptions, AgentReplOptions};
use super::bundle::{BundleOptions, RunBundleOptions};
use super::debug::DebugCommand;
use super::jit::JitCheckOptions;
use super::runtime::{
    CliRunOptions, PlanOptions, RuntimeProfileOptions, RuntimeRunOptions, ScriptBenchOptions,
    ScriptTestOptions, ServeOptions,
};
use super::tooling::ToolingCommandOptions;
use super::verify::{CheckOptions, UnsafeOptions, VerifyOptions, VerifyTypesOptions};
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
    Check(CheckOptions),
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
    RunBundle(RunBundleOptions),
    Build {
        #[command(subcommand)]
        command: BuildCommand,
    },
    ToolchainProfile(ToolchainProfileOptions),
    Jit {
        #[command(subcommand)]
        command: JitCommand,
    },
    Fmt(ToolingCommandOptions),
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
        command: AgentRagCommand,
    },
    Script {
        #[command(subcommand)]
        command: Box<AgentScriptCommand>,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum AgentRagCommand {
    Query(super::agent::AgentRagQueryOptions),
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

#[derive(Debug, Subcommand)]
pub(super) enum BuildCommand {
    Bundle(BundleOptions),
}
