use clap::{Args, ValueEnum};
use serde::Serialize;
use std::process::{Command, ExitCode};
use std::time::Instant;

#[derive(Args, Clone, Debug)]
pub(crate) struct ToolchainProfileOptions {
    #[arg(long = "command", value_enum)]
    pub(crate) commands: Vec<ToolchainProfileCommand>,
    #[arg(long)]
    pub(crate) dry_run: bool,
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum ToolchainProfileCommand {
    Fmt,
    Check,
    Clippy,
    Test,
}

#[derive(Serialize)]
struct ToolchainProfileReport {
    status: String,
    commands: Vec<ToolchainCommandReport>,
}

#[derive(Serialize)]
struct ToolchainCommandReport {
    label: &'static str,
    argv: Vec<&'static str>,
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

pub(crate) fn run(options: &ToolchainProfileOptions) -> Result<(), ExitCode> {
    let reports = selected_commands(options)
        .into_iter()
        .map(|spec| profile_command(spec, options.dry_run))
        .collect::<Vec<_>>();
    let failed = reports
        .iter()
        .any(|report| matches!(report.status, "failed" | "spawn_failed"));
    let report = ToolchainProfileReport {
        status: if failed { "failed" } else { "ok" }.to_owned(),
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
            ToolchainProfileCommand::Clippy => CLIPPY,
            ToolchainProfileCommand::Test => TEST,
        }
    }
}

fn profile_command(spec: ToolchainCommandSpec, dry_run: bool) -> ToolchainCommandReport {
    if dry_run {
        return ToolchainCommandReport {
            label: spec.label,
            argv: argv_for(spec),
            status: "planned",
            exit_code: None,
            elapsed_ns: 0,
            stdout_lines: 0,
            stderr_lines: 0,
        };
    }

    let start = Instant::now();
    match Command::new("cargo").args(spec.args).output() {
        Ok(output) => ToolchainCommandReport {
            label: spec.label,
            argv: argv_for(spec),
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
        Err(_) => ToolchainCommandReport {
            label: spec.label,
            argv: argv_for(spec),
            status: "spawn_failed",
            exit_code: None,
            elapsed_ns: start.elapsed().as_nanos(),
            stdout_lines: 0,
            stderr_lines: 0,
        },
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

fn print_human_report(report: &ToolchainProfileReport) {
    for command in &report.commands {
        println!(
            "{}: {} ({} ns, stdout lines {}, stderr lines {})",
            command.label,
            command.status,
            command.elapsed_ns,
            command.stdout_lines,
            command.stderr_lines
        );
    }
}
