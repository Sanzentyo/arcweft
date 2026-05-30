use crate::native_system::{HostSystemInfo, host_system_info};
use clap::{Args, ValueEnum};
use serde::Serialize;
use std::process::{Command, ExitCode};
use std::time::Instant;

#[derive(Args, Clone, Debug)]
pub(crate) struct ToolchainProfileOptions {
    #[arg(long = "command", value_enum)]
    pub(crate) commands: Vec<ToolchainProfileCommand>,
    #[arg(long, default_value_t = 1, value_parser = parse_positive_usize)]
    pub(crate) repeat: usize,
    #[arg(long)]
    pub(crate) dry_run: bool,
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum ToolchainProfileCommand {
    Fmt,
    Check,
    CheckFull,
    TestBuild,
    Clippy,
    Test,
}

#[derive(Serialize)]
struct ToolchainProfileReport {
    status: String,
    host_system: HostSystemInfo,
    commands: Vec<ToolchainCommandReport>,
}

#[derive(Serialize)]
struct ToolchainCommandReport {
    label: &'static str,
    argv: Vec<&'static str>,
    status: &'static str,
    exit_code: Option<i32>,
    repeat: usize,
    elapsed_ns: u128,
    timing: ToolchainTimingReport,
    stdout_lines: usize,
    stderr_lines: usize,
    samples: Vec<ToolchainCommandSample>,
}

#[derive(Serialize)]
struct ToolchainTimingReport {
    min: u128,
    median: u128,
    max: u128,
}

#[derive(Clone, Debug, Serialize)]
struct ToolchainCommandSample {
    index: usize,
    status: &'static str,
    exit_code: Option<i32>,
    elapsed_ns: u128,
    stdout_lines: usize,
    stderr_lines: usize,
}

#[derive(Clone, Copy, Debug)]
struct ToolchainCommandSpec {
    label: &'static str,
    args: &'static [&'static str],
}

const CHECK: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_check_workspace",
    args: &["check", "--workspace"],
};

const CHECK_FULL: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_check_workspace_all_targets_all_features",
    args: &["check", "--workspace", "--all-targets", "--all-features"],
};

const FMT: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_fmt_all_check",
    args: &["fmt", "--all", "--check"],
};

const CLIPPY: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_clippy_workspace_all_targets_all_features",
    args: &["clippy", "--workspace", "--all-targets", "--all-features"],
};

const TEST: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_test_workspace",
    args: &["test", "--workspace"],
};

const TEST_BUILD: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_test_workspace_no_run",
    args: &["test", "--workspace", "--no-run"],
};

pub(crate) fn run(options: &ToolchainProfileOptions) -> Result<(), ExitCode> {
    let reports = selected_commands(options)
        .into_iter()
        .map(|spec| profile_command(spec, options.dry_run, options.repeat))
        .collect::<Vec<_>>();
    let failed = reports
        .iter()
        .any(|report| matches!(report.status, "failed" | "spawn_failed"));
    let report = ToolchainProfileReport {
        status: if failed { "failed" } else { "ok" }.to_owned(),
        host_system: host_system_info(),
        commands: reports,
    };

    if options.json {
        crate::print_json(&report)?;
    } else {
        print_human_report(&report);
    }

    if failed {
        Err(ExitCode::FAILURE)
    } else {
        Ok(())
    }
}

fn selected_commands(options: &ToolchainProfileOptions) -> Vec<ToolchainCommandSpec> {
    if options.commands.is_empty() {
        return vec![CHECK];
    }
    options
        .commands
        .iter()
        .copied()
        .map(ToolchainCommandSpec::from)
        .collect()
}

impl From<ToolchainProfileCommand> for ToolchainCommandSpec {
    fn from(command: ToolchainProfileCommand) -> Self {
        match command {
            ToolchainProfileCommand::Fmt => FMT,
            ToolchainProfileCommand::Check => CHECK,
            ToolchainProfileCommand::CheckFull => CHECK_FULL,
            ToolchainProfileCommand::TestBuild => TEST_BUILD,
            ToolchainProfileCommand::Clippy => CLIPPY,
            ToolchainProfileCommand::Test => TEST,
        }
    }
}

fn profile_command(
    spec: ToolchainCommandSpec,
    dry_run: bool,
    repeat: usize,
) -> ToolchainCommandReport {
    if dry_run {
        let samples = (0..repeat)
            .map(|index| ToolchainCommandSample {
                index,
                status: "planned",
                exit_code: None,
                elapsed_ns: 0,
                stdout_lines: 0,
                stderr_lines: 0,
            })
            .collect::<Vec<_>>();
        return command_report_from_samples(spec, samples);
    }

    let samples = (0..repeat)
        .map(|index| profile_command_sample(spec, index))
        .collect::<Vec<_>>();
    command_report_from_samples(spec, samples)
}

fn profile_command_sample(spec: ToolchainCommandSpec, index: usize) -> ToolchainCommandSample {
    let start = Instant::now();
    match Command::new("cargo").args(spec.args).output() {
        Ok(output) => ToolchainCommandSample {
            index,
            status: if output.status.success() {
                "ok"
            } else {
                "failed"
            },
            exit_code: output.status.code(),
            elapsed_ns: start.elapsed().as_nanos(),
            stdout_lines: count_lines(&output.stdout),
            stderr_lines: count_lines(&output.stderr),
        },
        Err(_) => ToolchainCommandSample {
            index,
            status: "spawn_failed",
            exit_code: None,
            elapsed_ns: start.elapsed().as_nanos(),
            stdout_lines: 0,
            stderr_lines: 0,
        },
    }
}

fn command_report_from_samples(
    spec: ToolchainCommandSpec,
    samples: Vec<ToolchainCommandSample>,
) -> ToolchainCommandReport {
    let status = aggregate_status(&samples);
    let exit_code = samples
        .iter()
        .find(|sample| sample.exit_code.is_some_and(|code| code != 0))
        .and_then(|sample| sample.exit_code)
        .or_else(|| samples.iter().find_map(|sample| sample.exit_code));
    let stdout_lines = samples.iter().map(|sample| sample.stdout_lines).sum();
    let stderr_lines = samples.iter().map(|sample| sample.stderr_lines).sum();
    let mut elapsed = samples
        .iter()
        .map(|sample| sample.elapsed_ns)
        .collect::<Vec<_>>();
    let timing = timing_report(&mut elapsed);

    ToolchainCommandReport {
        label: spec.label,
        argv: argv_for(spec),
        status,
        exit_code,
        repeat: samples.len(),
        elapsed_ns: timing.median,
        timing,
        stdout_lines,
        stderr_lines,
        samples,
    }
}

fn aggregate_status(samples: &[ToolchainCommandSample]) -> &'static str {
    if samples.iter().any(|sample| sample.status == "spawn_failed") {
        return "spawn_failed";
    }
    if samples.iter().any(|sample| sample.status == "failed") {
        return "failed";
    }
    if samples.iter().all(|sample| sample.status == "planned") {
        return "planned";
    }
    "ok"
}

fn timing_report(elapsed: &mut [u128]) -> ToolchainTimingReport {
    elapsed.sort_unstable();
    ToolchainTimingReport {
        min: elapsed.first().copied().unwrap_or_default(),
        median: elapsed.get(elapsed.len() / 2).copied().unwrap_or_default(),
        max: elapsed.last().copied().unwrap_or_default(),
    }
}

fn argv_for(spec: ToolchainCommandSpec) -> Vec<&'static str> {
    std::iter::once("cargo")
        .chain(spec.args.iter().copied())
        .collect()
}

fn count_lines(bytes: &[u8]) -> usize {
    String::from_utf8_lossy(bytes).lines().count()
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(parsed) if parsed > 0 => Ok(parsed),
        Ok(_) => Err("value must be greater than zero".to_owned()),
        Err(error) => Err(error.to_string()),
    }
}

fn print_human_report(report: &ToolchainProfileReport) {
    for command in &report.commands {
        println!(
            "{}: {} (median {} ns, min {} ns, max {} ns, repeat {}, stdout lines {}, stderr lines {})",
            command.label,
            command.status,
            command.elapsed_ns,
            command.timing.min,
            command.timing.max,
            command.repeat,
            command.stdout_lines,
            command.stderr_lines
        );
    }
}
