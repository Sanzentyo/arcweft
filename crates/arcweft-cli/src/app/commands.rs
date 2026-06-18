use super::ToolchainProfileOptions;
use super::agent::{AgentHitTestOptions, AgentMcpOptions, AgentObserveOptions};
use super::bundle::{BundleOptions, RunBundleOptions};
use super::jit::JitCheckOptions;
use super::runtime::{
    CliRunOptions, PlanOptions, RuntimeProfileOptions, RuntimeRunOptions, ScriptBenchOptions,
    ScriptTestOptions, ServeOptions,
};
use super::tooling::ToolingCommandOptions;
use super::verify::{CheckOptions, UnsafeOptions, VerifyOptions, VerifyTypesOptions};
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
#[allow(clippy::large_enum_variant)]
pub(super) enum AgentCommand {
    Observe(AgentObserveOptions),
    HitTest(AgentHitTestOptions),
    Mcp(AgentMcpOptions),
}

#[derive(Debug, Subcommand)]
pub(super) enum JitCommand {
    Check(JitCheckOptions),
}

#[derive(Debug, Subcommand)]
pub(super) enum BuildCommand {
    Bundle(BundleOptions),
}
