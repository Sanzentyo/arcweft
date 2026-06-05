use arcweft_runtime_host::{HostSystemInfo, host_system_info};
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
    #[arg(long, default_value_t = 0)]
    pub(crate) warmup: usize,
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
    #[value(name = "bench-003")]
    Bench003,
    #[value(name = "bench-009")]
    Bench009,
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
    warmup: usize,
    elapsed_ns: u128,
    timing: ToolchainTimingReport,
    stdout_lines: usize,
    stderr_lines: usize,
    arcweft_bench: Option<ToolchainArcweftBenchReport>,
    warmup_samples: Vec<ToolchainCommandSample>,
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
    arcweft_bench: Option<ToolchainArcweftBenchSample>,
}

#[derive(Clone, Copy, Debug)]
struct ToolchainCommandSpec {
    label: &'static str,
    args: &'static [&'static str],
    kind: ToolchainCommandKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ToolchainCommandKind {
    Cargo,
    ArcweftBench,
}

#[derive(Clone, Debug, Serialize)]
struct ToolchainArcweftBenchSample {
    source: String,
    bench_id: String,
    bench_status: String,
    executor: String,
    bench_elapsed_ns: u64,
    per_executed_op_ns: u64,
    pure_calls: u64,
    pure_batch_calls: u64,
    pure_batch_items: u64,
    pure_jit_calls: u64,
    pure_aot_calls: u64,
    pure_vm_calls: u64,
    pure_fallbacks: u64,
    pure_arg_vec_allocations: u64,
    pure_arg_bytes_borrowed: u64,
}

#[derive(Serialize)]
struct ToolchainArcweftBenchReport {
    source: String,
    bench_id: String,
    bench_status: String,
    executor: String,
    median_bench_elapsed_ns: u64,
    min_bench_elapsed_ns: u64,
    max_bench_elapsed_ns: u64,
    median_per_executed_op_ns: u64,
    median_pure_calls: u64,
    median_pure_batch_calls: u64,
    median_pure_batch_items: u64,
    median_pure_jit_calls: u64,
    median_pure_aot_calls: u64,
    median_pure_vm_calls: u64,
    median_pure_fallbacks: u64,
    median_pure_arg_vec_allocations: u64,
    median_pure_arg_bytes_borrowed: u64,
}

const CHECK: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_check_workspace",
    args: &["check", "--workspace"],
    kind: ToolchainCommandKind::Cargo,
};

const CHECK_FULL: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_check_workspace_all_targets_all_features",
    args: &["check", "--workspace", "--all-targets", "--all-features"],
    kind: ToolchainCommandKind::Cargo,
};

const FMT: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_fmt_all_check",
    args: &["fmt", "--all", "--check"],
    kind: ToolchainCommandKind::Cargo,
};

const CLIPPY: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_clippy_workspace_all_targets_all_features",
    args: &["clippy", "--workspace", "--all-targets", "--all-features"],
    kind: ToolchainCommandKind::Cargo,
};

const TEST: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_test_workspace",
    args: &["test", "--workspace"],
    kind: ToolchainCommandKind::Cargo,
};

const TEST_BUILD: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_test_workspace_no_run",
    args: &["test", "--workspace", "--no-run"],
    kind: ToolchainCommandKind::Cargo,
};

const BENCH_003: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_bench_003_for_pure_jit",
    args: &[
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/003_for_pure_jit.arcw",
        "--json",
        "--iterations",
        "15",
        "--warmup",
        "3",
        "--samples",
        "9",
        "--steps",
        "64",
        "--max-ops",
        "64",
        "--pure-backend",
        "jit",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const BENCH_009: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_bench_009_nonuniform_map_pure_batch",
    args: &[
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw",
        "--json",
        "--iterations",
        "15",
        "--warmup",
        "3",
        "--samples",
        "9",
        "--steps",
        "64",
        "--max-ops",
        "64",
        "--pure-backend",
        "jit",
        "--pure-workers",
        "4",
        "--pure-batch-min-len",
        "64",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

pub(crate) fn run(options: &ToolchainProfileOptions) -> Result<(), ExitCode> {
    let reports = selected_commands(options)
        .into_iter()
        .map(|spec| profile_command(spec, options.dry_run, options.repeat, options.warmup))
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
            ToolchainProfileCommand::Bench003 => BENCH_003,
            ToolchainProfileCommand::Bench009 => BENCH_009,
        }
    }
}

fn profile_command(
    spec: ToolchainCommandSpec,
    dry_run: bool,
    repeat: usize,
    warmup: usize,
) -> ToolchainCommandReport {
    if dry_run {
        let mut warmup_samples = Vec::with_capacity(warmup);
        for index in 0..warmup {
            warmup_samples.push(planned_command_sample(index));
        }
        let mut samples = Vec::with_capacity(repeat);
        for index in 0..repeat {
            samples.push(planned_command_sample(index));
        }
        return command_report_from_samples(spec, warmup_samples, samples);
    }

    let mut warmup_samples = Vec::with_capacity(warmup);
    for index in 0..warmup {
        warmup_samples.push(profile_command_sample(spec, index));
    }
    let mut samples = Vec::with_capacity(repeat);
    for index in 0..repeat {
        samples.push(profile_command_sample(spec, index));
    }
    command_report_from_samples(spec, warmup_samples, samples)
}

const fn planned_command_sample(index: usize) -> ToolchainCommandSample {
    ToolchainCommandSample {
        index,
        status: "planned",
        exit_code: None,
        elapsed_ns: 0,
        stdout_lines: 0,
        stderr_lines: 0,
        arcweft_bench: None,
    }
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
            arcweft_bench: if output.status.success()
                && spec.kind == ToolchainCommandKind::ArcweftBench
            {
                arcweft_bench_sample(&output.stdout)
            } else {
                None
            },
        },
        Err(_) => ToolchainCommandSample {
            index,
            status: "spawn_failed",
            exit_code: None,
            elapsed_ns: start.elapsed().as_nanos(),
            stdout_lines: 0,
            stderr_lines: 0,
            arcweft_bench: None,
        },
    }
}

fn command_report_from_samples(
    spec: ToolchainCommandSpec,
    warmup_samples: Vec<ToolchainCommandSample>,
    samples: Vec<ToolchainCommandSample>,
) -> ToolchainCommandReport {
    let status = aggregate_status(&warmup_samples, &samples);
    let exit_code = warmup_samples
        .iter()
        .chain(samples.iter())
        .find(|sample| sample.exit_code.is_some_and(|code| code != 0))
        .and_then(|sample| sample.exit_code)
        .or_else(|| {
            warmup_samples
                .iter()
                .chain(samples.iter())
                .find_map(|sample| sample.exit_code)
        });
    let stdout_lines = warmup_samples
        .iter()
        .chain(samples.iter())
        .map(|sample| sample.stdout_lines)
        .sum();
    let stderr_lines = warmup_samples
        .iter()
        .chain(samples.iter())
        .map(|sample| sample.stderr_lines)
        .sum();
    let mut elapsed = samples
        .iter()
        .map(|sample| sample.elapsed_ns)
        .collect::<Vec<_>>();
    let timing = timing_report(&mut elapsed);
    let arcweft_bench = arcweft_bench_report(&samples);

    ToolchainCommandReport {
        label: spec.label,
        argv: argv_for(spec),
        status,
        exit_code,
        repeat: samples.len(),
        warmup: warmup_samples.len(),
        elapsed_ns: timing.median,
        timing,
        stdout_lines,
        stderr_lines,
        arcweft_bench,
        warmup_samples,
        samples,
    }
}

fn arcweft_bench_sample(bytes: &[u8]) -> Option<ToolchainArcweftBenchSample> {
    let json = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    let section = json
        .get("benches")?
        .as_array()?
        .first()?
        .get("sections")?
        .as_array()?
        .first()?;
    let measurement = section.get("measurement")?;
    let deterministic = measurement.get("deterministic")?;
    Some(ToolchainArcweftBenchSample {
        source: json.get("source")?.as_str()?.to_owned(),
        bench_id: json
            .get("benches")?
            .as_array()?
            .first()?
            .get("id")?
            .as_str()?
            .to_owned(),
        bench_status: section.get("status")?.as_str()?.to_owned(),
        executor: measurement.get("executor")?.as_str()?.to_owned(),
        bench_elapsed_ns: measurement.get("elapsed_ns")?.get("median")?.as_u64()?,
        per_executed_op_ns: measurement.get("per_executed_op_ns")?.as_u64()?,
        pure_calls: deterministic.get("pure_calls_median")?.as_u64()?,
        pure_batch_calls: deterministic.get("pure_batch_calls_median")?.as_u64()?,
        pure_batch_items: deterministic.get("pure_batch_items_median")?.as_u64()?,
        pure_jit_calls: deterministic.get("pure_jit_calls_median")?.as_u64()?,
        pure_aot_calls: deterministic.get("pure_aot_calls_median")?.as_u64()?,
        pure_vm_calls: deterministic.get("pure_vm_calls_median")?.as_u64()?,
        pure_fallbacks: deterministic.get("pure_fallbacks_median")?.as_u64()?,
        pure_arg_vec_allocations: deterministic
            .get("pure_arg_vec_allocations_median")?
            .as_u64()?,
        pure_arg_bytes_borrowed: deterministic
            .get("pure_arg_bytes_borrowed_median")?
            .as_u64()?,
    })
}

fn arcweft_bench_report(samples: &[ToolchainCommandSample]) -> Option<ToolchainArcweftBenchReport> {
    let bench_samples = samples
        .iter()
        .filter_map(|sample| sample.arcweft_bench.as_ref())
        .collect::<Vec<_>>();
    let first = bench_samples.first()?;
    Some(ToolchainArcweftBenchReport {
        source: first.source.clone(),
        bench_id: first.bench_id.clone(),
        bench_status: first.bench_status.clone(),
        executor: first.executor.clone(),
        median_bench_elapsed_ns: median_bench_sample_by(&bench_samples, |sample| {
            sample.bench_elapsed_ns
        }),
        min_bench_elapsed_ns: bench_samples
            .iter()
            .map(|sample| sample.bench_elapsed_ns)
            .min()
            .unwrap_or_default(),
        max_bench_elapsed_ns: bench_samples
            .iter()
            .map(|sample| sample.bench_elapsed_ns)
            .max()
            .unwrap_or_default(),
        median_per_executed_op_ns: median_bench_sample_by(&bench_samples, |sample| {
            sample.per_executed_op_ns
        }),
        median_pure_calls: median_bench_sample_by(&bench_samples, |sample| sample.pure_calls),
        median_pure_batch_calls: median_bench_sample_by(&bench_samples, |sample| {
            sample.pure_batch_calls
        }),
        median_pure_batch_items: median_bench_sample_by(&bench_samples, |sample| {
            sample.pure_batch_items
        }),
        median_pure_jit_calls: median_bench_sample_by(&bench_samples, |sample| {
            sample.pure_jit_calls
        }),
        median_pure_aot_calls: median_bench_sample_by(&bench_samples, |sample| {
            sample.pure_aot_calls
        }),
        median_pure_vm_calls: median_bench_sample_by(&bench_samples, |sample| sample.pure_vm_calls),
        median_pure_fallbacks: median_bench_sample_by(&bench_samples, |sample| {
            sample.pure_fallbacks
        }),
        median_pure_arg_vec_allocations: median_bench_sample_by(&bench_samples, |sample| {
            sample.pure_arg_vec_allocations
        }),
        median_pure_arg_bytes_borrowed: median_bench_sample_by(&bench_samples, |sample| {
            sample.pure_arg_bytes_borrowed
        }),
    })
}

fn median_bench_sample_by(
    samples: &[&ToolchainArcweftBenchSample],
    field: impl Fn(&ToolchainArcweftBenchSample) -> u64,
) -> u64 {
    let mut values = samples
        .iter()
        .map(|sample| field(sample))
        .collect::<Vec<_>>();
    values.sort_unstable();
    values.get(values.len() / 2).copied().unwrap_or_default()
}

fn aggregate_status(
    warmup_samples: &[ToolchainCommandSample],
    samples: &[ToolchainCommandSample],
) -> &'static str {
    let mut all_samples = warmup_samples.iter().chain(samples.iter());
    if all_samples.any(|sample| sample.status == "spawn_failed") {
        return "spawn_failed";
    }
    let mut all_samples = warmup_samples.iter().chain(samples.iter());
    if all_samples.any(|sample| sample.status == "failed") {
        return "failed";
    }
    let mut all_samples = warmup_samples.iter().chain(samples.iter());
    if all_samples.all(|sample| sample.status == "planned") {
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
    if bytes.is_empty() {
        return 0;
    }
    let segments = bytes.split(|byte| *byte == b'\n').count();
    if bytes.ends_with(b"\n") {
        segments.saturating_sub(1)
    } else {
        segments
    }
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
            "{}: {} (median {} ns, min {} ns, max {} ns, warmup {}, repeat {}, stdout lines {}, stderr lines {})",
            command.label,
            command.status,
            command.elapsed_ns,
            command.timing.min,
            command.timing.max,
            command.warmup,
            command.repeat,
            command.stdout_lines,
            command.stderr_lines
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{arcweft_bench_report, arcweft_bench_sample, count_lines, planned_command_sample};

    #[test]
    fn count_lines_does_not_allocate_utf8_strings() {
        assert_eq!(count_lines(b""), 0);
        assert_eq!(count_lines(b"one"), 1);
        assert_eq!(count_lines(b"one\n"), 1);
        assert_eq!(count_lines(b"one\ntwo"), 2);
        assert_eq!(count_lines(b"one\r\ntwo\r\n"), 2);
    }

    #[test]
    fn arcweft_bench_sample_extracts_path_free_runtime_counters() {
        let sample = arcweft_bench_sample(
            br#"{
  "source": "tests/fixtures/arcw/spec_should_pass/bench/003_for_pure_jit.arcw",
  "benches": [{
    "id": "bench.for_pure",
    "sections": [{
      "status": "measured",
      "measurement": {
        "executor": "bytecode_vm",
        "per_executed_op_ns": 700,
        "elapsed_ns": { "min": 10000, "median": 20000, "max": 30000 },
        "deterministic": {
          "pure_calls_median": 16,
          "pure_batch_calls_median": 0,
          "pure_batch_items_median": 0,
          "pure_jit_calls_median": 16,
          "pure_aot_calls_median": 0,
          "pure_vm_calls_median": 0,
          "pure_fallbacks_median": 0,
          "pure_arg_vec_allocations_median": 0,
          "pure_arg_bytes_borrowed_median": 256
        }
      }
    }]
  }]
}"#,
        )
        .expect("sample should parse");

        assert_eq!(sample.bench_id, "bench.for_pure");
        assert_eq!(sample.bench_status, "measured");
        assert_eq!(sample.executor, "bytecode_vm");
        assert_eq!(sample.bench_elapsed_ns, 20000);
        assert_eq!(sample.pure_jit_calls, 16);
        assert_eq!(sample.pure_arg_vec_allocations, 0);
    }

    #[test]
    fn arcweft_bench_report_summarizes_measured_samples_only() {
        let mut planned = planned_command_sample(0);
        let mut first = planned_command_sample(1);
        first.arcweft_bench = Some(super::ToolchainArcweftBenchSample {
            source: "003_for_pure_jit.arcw".to_owned(),
            bench_id: "bench.for_pure".to_owned(),
            bench_status: "measured".to_owned(),
            executor: "bytecode_vm".to_owned(),
            bench_elapsed_ns: 300,
            per_executed_op_ns: 30,
            pure_calls: 16,
            pure_batch_calls: 0,
            pure_batch_items: 0,
            pure_jit_calls: 16,
            pure_aot_calls: 0,
            pure_vm_calls: 0,
            pure_fallbacks: 0,
            pure_arg_vec_allocations: 0,
            pure_arg_bytes_borrowed: 256,
        });
        let mut second = first.clone();
        second
            .arcweft_bench
            .as_mut()
            .expect("bench sample")
            .bench_elapsed_ns = 100;
        let mut third = first.clone();
        third
            .arcweft_bench
            .as_mut()
            .expect("bench sample")
            .bench_elapsed_ns = 200;
        planned.arcweft_bench = None;

        let report = arcweft_bench_report(&[planned, first, second, third])
            .expect("report should summarize bench samples");

        assert_eq!(report.source, "003_for_pure_jit.arcw");
        assert_eq!(report.median_bench_elapsed_ns, 200);
        assert_eq!(report.min_bench_elapsed_ns, 100);
        assert_eq!(report.max_bench_elapsed_ns, 300);
        assert_eq!(report.median_pure_jit_calls, 16);
    }
}
