mod agent;
mod bundle;
pub(in crate::app) mod bundle_view;
mod bundle_view_layout;
mod bundle_view_schema;
mod cache;
mod commands;
mod debug;
pub(in crate::app) mod diagnostics;
mod image_declarations;
mod import;
mod inspect;
pub(in crate::app) mod jit;
pub(in crate::app) mod local_embedding;
#[cfg(feature = "native-player")]
mod native_player;
mod progress;
pub(crate) mod project;
mod project_commands;
mod release;
mod release_sign;
mod remote_embedding;
pub(in crate::app) mod runtime;
pub(crate) mod shared;
mod tooling;
pub(in crate::app) mod verify;

use self::agent::agent_command;
use self::bundle::{bundle_command, patch_bundle_command, run_bundle_command};
use self::cache::cache_command;
use self::commands::{Cli, CliCommand};
use self::debug::debug_command;
use self::import::import_command;
use self::inspect::inspect_command;
use self::jit::jit_command;
#[cfg(feature = "native-player")]
use self::native_player::native_player_command;
use self::project_commands::{compile_command, project_build_command, project_check_command};
use self::release::release_command;
use self::release_sign::sign_bundle_command;
use self::runtime::cli::runtime_cli_command;
use self::runtime::plan::runtime_plan_command;
use self::runtime::profile_cmd::runtime_profile_command;
use self::runtime::run::runtime_run_command;
use self::runtime::script_bench::script_bench_command;
use self::runtime::script_test::script_test_command;
use self::runtime::serve::runtime_serve_command;
use self::tooling::{format_command, ids_command};
use self::verify::{unsafe_command, verify_command, verify_types_command};
use crate::toolchain_profile;
use arcweft_host_adapter::{HostAdapterError, HostAdapterRegistryBuilder};
use arcweft_runtime_host::NativeAdapterRegistrar;
use clap::Parser;
use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;

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
    let mut registrars = Vec::with_capacity(adapter_registrars.len() + 1);
    registrars.push(desktop_native_adapter_registrar as NativeAdapterRegistrar);
    registrars.extend_from_slice(adapter_registrars);
    match run_cli(Cli::parse_from(args), &registrars) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn desktop_native_adapter_registrar(
    _source_path: &Path,
    builder: HostAdapterRegistryBuilder,
) -> Result<HostAdapterRegistryBuilder, HostAdapterError> {
    let adapter_set = arcweft_adapter_desktop::DesktopAdapterSet::bind_current_thread(
        arcweft_desktop_native::NativeDesktopBackend::builder().build(),
    );
    adapter_set.register(builder).map(|(builder, _)| builder)
}

fn run_cli(cli: Cli, adapter_registrars: &[NativeAdapterRegistrar]) -> Result<(), ExitCode> {
    match cli.command {
        CliCommand::Check(options) => project_check_command(&options),
        CliCommand::Compile(options) => compile_command(&options),
        CliCommand::Import { command } => import_command(command),
        CliCommand::Inspect(options) => inspect_command(&options),
        CliCommand::Agent { command } => agent_command(command, adapter_registrars),
        CliCommand::Debug { command } => debug_command(command),
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
        CliCommand::Patch(options) => patch_bundle_command(&options),
        CliCommand::SignBundle(options) => sign_bundle_command(&options),
        CliCommand::Release { command } => release_command(command),
        CliCommand::RunBundle(options) => run_bundle_command(&options, adapter_registrars),
        CliCommand::Cache { command } => cache_command(command),
        #[cfg(feature = "native-player")]
        CliCommand::PlayNative(options) => native_player_command(&options),
        CliCommand::Build(options) => project_build_command(&options),
        CliCommand::ToolchainProfile(options) => toolchain_profile::run(&options),
        CliCommand::Jit { command } => jit_command(command),
        CliCommand::Fmt(options) => format_command(&options),
        CliCommand::Ids { command } => ids_command(command),
    }
}
