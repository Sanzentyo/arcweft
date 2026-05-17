use arcweft_core::{
    Engine, FlowFiberStatus, FlowRuntimeId, FrameInput, RuntimeBinding, RuntimePlan, RuntimeValue,
};
use arcweft_lang_hir::lower_to_hir;
use arcweft_lang_sema::{
    TypeCheckEnv, registry_from_hir, typecheck_hir, validate_hir_references,
    validate_typecheck_ready,
};
use arcweft_lang_syntax::{lint_id_policy, parse_source};
use arcweft_runtime_plan::{LoweredLineTaskGroup, lower_line_task_groups, lower_runtime_plan};
use arcweft_test::{ScriptStep, ScriptTest, ScriptTestManifest, collect_script_tests};
use arcweft_verify::{
    BackendKind, SmtBackend, VerificationMode, VerificationPolicy, VerificationReport,
    emit_smt_lib, verify_module,
};
use arcweft_verify_oxiz::OxizBackend;
use arcweft_verify_z3::ExternalZ3Backend;
mod output;
use output::{
    RuntimeFrameRunSummary, RuntimePlanReport, RuntimeRunReport, ScriptTestRunReport,
    ScriptTestRunSummary, flow_status_label,
};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run(args: &[OsString]) -> Result<(), ExitCode> {
    match args {
        [] => {
            print_help();
            Err(ExitCode::from(2))
        }
        [flag] if flag == "--help" || flag == "-h" => {
            print_help();
            Ok(())
        }
        [command, path] if command == "check" => check(&PathBuf::from(path)),
        [command, rest @ ..] if command == "verify" => verify_command(rest),
        [command, rest @ ..] if command == "unsafe" => unsafe_command(rest),
        [command, rest @ ..] if command == "plan" => runtime_plan_command(rest),
        [command, rest @ ..] if command == "run" => runtime_run_command(rest),
        [command, rest @ ..] if command == "test" => script_test_command(rest),
        [command, rest @ ..] if command == "bench" => script_bench_command(rest),
        [command, ..] => {
            eprintln!("error: unknown command `{}`", command.to_string_lossy());
            print_help();
            Err(ExitCode::from(2))
        }
    }
}

fn runtime_plan_command(args: &[OsString]) -> Result<(), ExitCode> {
    let options = ScriptPlanOptions::parse(args, "plan")?;
    let checked = load_and_check(&options.path)?;
    let report = RuntimePlanReport::from_checked(&checked);
    if options.json {
        print_json(&report)
    } else {
        for line in &report.lines {
            println!(
                "{} {} {} task_node={} child_task(s)={} effect(s)={}",
                line.flow_id.as_deref().unwrap_or("-"),
                line.line_id.as_deref().unwrap_or("-"),
                line.callee,
                line.root.kind,
                line.child_tasks,
                line.effects
            );
        }
        println!(
            "ok: {} ({} line task group(s), {} verifier obligation(s))",
            options.path.display(),
            report.lines.len(),
            report.verifier_obligations
        );
        Ok(())
    }
}

fn runtime_run_command(args: &[OsString]) -> Result<(), ExitCode> {
    let options = RuntimeRunOptions::parse(args)?;
    let checked = load_and_check(&options.path)?;
    let plan = lower_runtime_plan(&checked.hir).map_err(|errors| {
        for error in errors {
            eprintln!("error: {}", error.message());
        }
        ExitCode::FAILURE
    })?;
    let mut engine = Engine::new(plan);
    let mut frames = Vec::new();
    for frame_index in 0..options.frames {
        let output = engine.step(FrameInput {
            external_values: options.values.clone(),
            ..FrameInput::default()
        });
        let summary = RuntimeFrameRunSummary::from_output(frame_index, output, engine.fiber());
        let done = matches!(
            engine.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        frames.push(summary);
        if done {
            break;
        }
    }
    let report = RuntimeRunReport {
        frames,
        final_status: flow_status_label(&engine.fiber().status),
    };
    if options.json {
        print_json(&report)
    } else {
        for frame in &report.frames {
            println!(
                "frame {}: {} flow event(s), {} effect(s), {} task request(s), {} diagnostic(s)",
                frame.index,
                frame.flow_events.len(),
                frame.line_effects.len(),
                frame.task_requests.len(),
                frame.diagnostics.len()
            );
            for event in &frame.flow_events {
                println!("  event {event}");
            }
            for effect in &frame.line_effects {
                println!("  effect {effect}");
            }
        }
        println!(
            "ok: {} ({} frame(s), final_status={})",
            options.path.display(),
            report.frames.len(),
            report.final_status
        );
        Ok(())
    }
}

fn script_test_command(args: &[OsString]) -> Result<(), ExitCode> {
    let options = ScriptPlanOptions::parse(args, "test")?;
    let checked = load_and_check(&options.path)?;
    let manifest = collect_script_tests(&checked.hir);
    let plan = lower_runtime_plan(&checked.hir).map_err(|errors| {
        for error in errors {
            eprintln!("error: {}", error.message());
        }
        ExitCode::FAILURE
    })?;
    let output = ScriptTestRunReport {
        tests: manifest
            .tests
            .iter()
            .map(|test| run_script_test(test, &plan, &options))
            .collect(),
    };
    let failed = output.tests.iter().any(|test| test.status == "failed");
    if options.json {
        print_json(&output)?;
    } else {
        for test in &output.tests {
            println!(
                "{} {} {} ({} frame(s))",
                test.id, test.kind, test.status, test.frames_run
            );
            for diagnostic in &test.diagnostics {
                println!("  diagnostic {diagnostic}");
            }
        }
        println!(
            "ok: {} ({} script test(s))",
            options.path.display(),
            output.tests.len()
        );
    }
    if failed {
        Err(ExitCode::FAILURE)
    } else {
        Ok(())
    }
}

fn run_script_test(
    test: &ScriptTest,
    plan: &RuntimePlan,
    options: &ScriptPlanOptions,
) -> ScriptTestRunSummary {
    if test.kind != "scenario" {
        return ScriptTestRunSummary::skipped(
            test,
            format!(
                "headless execution for `{}` tests is not implemented",
                test.kind
            ),
        );
    }
    let Some(start) = test_start_flow(test) else {
        return ScriptTestRunSummary::completed(
            test,
            false,
            "not_started".to_owned(),
            vec!["scenario test requires `start @flow.id`".to_owned()],
            Vec::new(),
        );
    };
    let mut plan = plan.clone();
    plan.entry_flow = Some(FlowRuntimeId(start));
    let mut engine = Engine::new(plan);
    let mut frames = Vec::new();
    for frame_index in 0..options.frames {
        let output = engine.step(FrameInput {
            external_values: options.values.clone(),
            ..FrameInput::default()
        });
        let summary = RuntimeFrameRunSummary::from_output(frame_index, output, engine.fiber());
        let done = matches!(
            engine.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        frames.push(summary);
        if done {
            break;
        }
    }
    let final_status = flow_status_label(&engine.fiber().status);
    let mut diagnostics = frames
        .iter()
        .flat_map(|frame| frame.diagnostics.iter().cloned())
        .collect::<Vec<_>>();
    diagnostics.extend(test_expectation_failures(test, &engine, &frames));
    match engine.fiber().status {
        FlowFiberStatus::Done(_) => {}
        FlowFiberStatus::Failed(ref message) => {
            diagnostics.push(format!("runtime failed: {message}"));
        }
        FlowFiberStatus::Running | FlowFiberStatus::Waiting(_) | FlowFiberStatus::Choice(_) => {
            diagnostics.push(format!(
                "scenario did not finish within {} frame(s): {final_status}",
                options.frames
            ));
        }
    }
    let passed = diagnostics.is_empty();
    ScriptTestRunSummary::completed(test, passed, final_status, diagnostics, frames)
}

fn test_start_flow(test: &ScriptTest) -> Option<String> {
    test.steps.iter().find_map(|step| {
        let rest = step.text.strip_prefix("start ")?;
        let id = rest.split_whitespace().next()?.trim_start_matches('@');
        Some(id.to_owned())
    })
}

fn test_expectation_failures(
    test: &ScriptTest,
    engine: &Engine,
    frames: &[RuntimeFrameRunSummary],
) -> Vec<String> {
    test.steps
        .iter()
        .filter(|step| step.command == "expect")
        .filter_map(|step| evaluate_test_expectation(step, engine, frames).err())
        .collect()
}

fn evaluate_test_expectation(
    step: &ScriptStep,
    engine: &Engine,
    frames: &[RuntimeFrameRunSummary],
) -> Result<(), String> {
    let text = step.text.trim();
    if text == "expect no_assertion_failures" {
        if frames.iter().all(|frame| frame.diagnostics.is_empty()) {
            return Ok(());
        }
        return Err("expected no assertion/runtime diagnostics".to_owned());
    }
    if let Some((target, expected)) = parse_signal_expectation(text) {
        let actual = engine.fiber().observations.signals.get(&target);
        if actual == Some(&expected) {
            return Ok(());
        }
        return Err(format!(
            "expected signal {target} == {expected}, found {}",
            actual.cloned().unwrap_or_else(|| "<missing>".to_owned())
        ));
    }
    if let Some((level, needle)) = parse_log_contains_expectation(text) {
        if engine
            .fiber()
            .observations
            .logs
            .iter()
            .any(|log| log.level == level && log.message.contains(&needle))
        {
            return Ok(());
        }
        return Err(format!("expected log.{level} containing `{needle}`"));
    }
    Err(format!("unsupported scenario expectation `{text}`"))
}

fn parse_signal_expectation(text: &str) -> Option<(String, String)> {
    let rest = text.strip_prefix("expect signal ")?;
    let (target, expected) = rest.split_once(" == ")?;
    Some((target.trim().to_owned(), expected.trim().to_owned()))
}

fn parse_log_contains_expectation(text: &str) -> Option<(String, String)> {
    let rest = text.strip_prefix("expect log.")?;
    let (level, needle) = rest.split_once(" contains ")?;
    Some((
        level.trim().to_owned(),
        needle.trim().trim_matches('"').to_owned(),
    ))
}

fn script_bench_command(args: &[OsString]) -> Result<(), ExitCode> {
    let options = ScriptPlanOptions::parse(args, "bench")?;
    let checked = load_and_check(&options.path)?;
    let manifest = collect_script_tests(&checked.hir);
    let output = ScriptTestManifest {
        tests: Vec::new(),
        benches: manifest.benches,
    };
    if options.json {
        print_json(&output)
    } else {
        for bench in &output.benches {
            println!("{} ({} section(s))", bench.id, bench.sections.len());
        }
        println!(
            "ok: {} ({} script bench(es))",
            options.path.display(),
            output.benches.len()
        );
        Ok(())
    }
}

fn check(path: &Path) -> Result<(), ExitCode> {
    let checked = load_and_check(path)?;
    let report = verify_module(
        &checked.hir,
        VerificationPolicy {
            mode: VerificationMode::Dev,
            backend: BackendKind::Emit,
        },
    );
    print_human_diagnostics(&report);
    if report.has_errors() {
        return Err(ExitCode::FAILURE);
    }

    println!(
        "ok: {} ({} flow(s), {} line task group(s), {} warning(s), {} obligation(s))",
        path.display(),
        checked.hir.flows().len(),
        checked.line_task_groups.len(),
        checked.syntax_warnings,
        report.obligations.len()
    );
    Ok(())
}

fn verify_command(args: &[OsString]) -> Result<(), ExitCode> {
    let options = VerifyOptions::parse(args)?;
    let checked = load_and_check(&options.path)?;
    let report = verify_module(
        &checked.hir,
        VerificationPolicy {
            mode: options.mode,
            backend: options.backend,
        },
    );

    if let Some(path) = options.emit_obligations.as_ref() {
        write_json(path, &report.obligations)?;
    }
    if let Some(path) = options.emit_smt.as_ref() {
        emit_smt(path, &report)?;
    }
    if matches!(options.backend, BackendKind::Oxiz | BackendKind::Z3) {
        solve_report(&report, options.backend, options.z3_command.as_deref());
    }
    if options.json {
        print_json(&report)?;
    } else {
        print_human_diagnostics(&report);
        println!(
            "ok: {} ({} obligation(s), {} unsafe audit(s))",
            options.path.display(),
            report.obligations.len(),
            report.unsafe_audit_count()
        );
    }
    if report.has_errors() {
        Err(ExitCode::FAILURE)
    } else {
        Ok(())
    }
}

fn unsafe_command(args: &[OsString]) -> Result<(), ExitCode> {
    let options = VerifyOptions::parse_with_default_mode(args, VerificationMode::Dev)?;
    let checked = load_and_check(&options.path)?;
    let report = verify_module(
        &checked.hir,
        VerificationPolicy {
            mode: options.mode,
            backend: BackendKind::Emit,
        },
    );
    if options.json {
        print_json(&report.unsafe_audits)?;
    } else {
        for audit in &report.unsafe_audits {
            println!(
                "{} reason={} safety_doc={}",
                audit.id, audit.has_reason, audit.has_safety_doc
            );
        }
    }
    Ok(())
}

struct CheckedModule {
    hir: arcweft_lang_hir::HirModule,
    syntax_warnings: usize,
    line_task_groups: Vec<LoweredLineTaskGroup>,
}

fn load_and_check(path: &Path) -> Result<CheckedModule, ExitCode> {
    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!("error: failed to read {}: {error}", path.display());
        ExitCode::FAILURE
    })?;

    let Ok(parsed) = catch_unwind(AssertUnwindSafe(|| parse_source(source))) else {
        eprintln!("error: parser panicked while checking {}", path.display());
        return Err(ExitCode::FAILURE);
    };
    if !parsed.errors().is_empty() {
        for error in parsed.errors() {
            eprintln!("error: {}", error.message());
        }
        return Err(ExitCode::FAILURE);
    }

    let tree = parsed.into_typed_tree();
    let lints = lint_id_policy(&tree);
    for lint in &lints {
        eprintln!("warning[{:?}]: {}", lint.code(), lint.message());
    }

    let hir = lower_to_hir(&tree).map_err(|errors| {
        for error in errors {
            eprintln!("error: {}", error.message());
        }
        ExitCode::FAILURE
    })?;

    let registry = registry_from_hir(&hir);
    if let Err(errors) = validate_hir_references(&hir, &registry) {
        for error in errors {
            eprintln!("error: {}", error.message());
        }
        return Err(ExitCode::FAILURE);
    }

    if let Err(errors) = validate_typecheck_ready(&hir) {
        for error in errors {
            eprintln!("error: {}", error.message());
        }
        return Err(ExitCode::FAILURE);
    }

    if let Err(errors) = typecheck_hir(&hir, &TypeCheckEnv::new()) {
        for error in errors {
            eprintln!("error: {}", error.message());
        }
        return Err(ExitCode::FAILURE);
    }

    let line_task_groups = match lower_line_task_groups(&hir) {
        Ok(groups) => groups,
        Err(errors) => {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            return Err(ExitCode::FAILURE);
        }
    };

    Ok(CheckedModule {
        hir,
        syntax_warnings: lints.len(),
        line_task_groups,
    })
}

#[derive(Clone, Debug)]
struct VerifyOptions {
    path: PathBuf,
    mode: VerificationMode,
    backend: BackendKind,
    json: bool,
    emit_obligations: Option<PathBuf>,
    emit_smt: Option<PathBuf>,
    z3_command: Option<String>,
}

#[derive(Clone, Debug)]
struct ScriptPlanOptions {
    path: PathBuf,
    frames: usize,
    values: Vec<RuntimeBinding>,
    json: bool,
}

#[derive(Clone, Debug)]
struct RuntimeRunOptions {
    path: PathBuf,
    frames: usize,
    values: Vec<RuntimeBinding>,
    json: bool,
}

impl ScriptPlanOptions {
    fn parse(args: &[OsString], command: &str) -> Result<Self, ExitCode> {
        let Some(path) = args.first() else {
            eprintln!("error: {command} requires <file.awft>");
            print_help();
            return Err(ExitCode::from(2));
        };
        let mut options = Self {
            path: PathBuf::from(path),
            frames: 32,
            values: Vec::new(),
            json: false,
        };
        let mut index = 1;
        while index < args.len() {
            let flag = args[index].to_string_lossy();
            match flag.as_ref() {
                "--json" => options.json = true,
                "--frames" => {
                    index += 1;
                    options.frames = parse_usize_arg(args.get(index), "--frames")?;
                }
                "--value" => {
                    index += 1;
                    options
                        .values
                        .push(parse_runtime_binding(args.get(index), "--value")?);
                }
                other => {
                    eprintln!("error: unknown {command} option `{other}`");
                    return Err(ExitCode::from(2));
                }
            }
            index += 1;
        }
        Ok(options)
    }
}

impl RuntimeRunOptions {
    fn parse(args: &[OsString]) -> Result<Self, ExitCode> {
        let Some(path) = args.first() else {
            eprintln!("error: run requires <file.awft>");
            print_help();
            return Err(ExitCode::from(2));
        };
        let mut options = Self {
            path: PathBuf::from(path),
            frames: 1,
            values: Vec::new(),
            json: false,
        };
        let mut index = 1;
        while index < args.len() {
            let flag = args[index].to_string_lossy();
            match flag.as_ref() {
                "--json" => options.json = true,
                "--frames" => {
                    index += 1;
                    options.frames = parse_usize_arg(args.get(index), "--frames")?;
                }
                "--value" => {
                    index += 1;
                    options
                        .values
                        .push(parse_runtime_binding(args.get(index), "--value")?);
                }
                other => {
                    eprintln!("error: unknown run option `{other}`");
                    return Err(ExitCode::from(2));
                }
            }
            index += 1;
        }
        Ok(options)
    }
}

fn parse_runtime_binding(value: Option<&OsString>, flag: &str) -> Result<RuntimeBinding, ExitCode> {
    let Some(value) = value else {
        eprintln!("error: {flag} requires name=value");
        return Err(ExitCode::from(2));
    };
    let value = value.to_string_lossy();
    let Some((name, raw)) = value.split_once('=') else {
        eprintln!("error: {flag} requires name=value");
        return Err(ExitCode::from(2));
    };
    if name.is_empty() {
        eprintln!("error: {flag} binding name must not be empty");
        return Err(ExitCode::from(2));
    }
    Ok(RuntimeBinding {
        name: name.to_owned(),
        value: parse_runtime_value(raw),
    })
}

fn parse_runtime_value(raw: &str) -> RuntimeValue {
    match raw {
        "true" => RuntimeValue::Bool(true),
        "false" => RuntimeValue::Bool(false),
        "()" => RuntimeValue::Unit,
        value if value.starts_with('@') => RuntimeValue::EntityRef(value[1..].to_owned()),
        value => value.parse::<i64>().map_or_else(
            |_| RuntimeValue::String(value.to_owned()),
            RuntimeValue::Int,
        ),
    }
}

impl VerifyOptions {
    fn parse(args: &[OsString]) -> Result<Self, ExitCode> {
        Self::parse_with_default_mode(args, VerificationMode::Test)
    }

    fn parse_with_default_mode(
        args: &[OsString],
        default_mode: VerificationMode,
    ) -> Result<Self, ExitCode> {
        let Some(path) = args.first() else {
            eprintln!("error: verify requires <file.awft>");
            print_help();
            return Err(ExitCode::from(2));
        };
        let mut options = Self {
            path: PathBuf::from(path),
            mode: default_mode,
            backend: BackendKind::Emit,
            json: false,
            emit_obligations: None,
            emit_smt: None,
            z3_command: None,
        };
        let mut index = 1;
        while index < args.len() {
            let flag = args[index].to_string_lossy();
            match flag.as_ref() {
                "--json" => options.json = true,
                "--mode" => {
                    index += 1;
                    options.mode = parse_mode(args.get(index))?;
                }
                "--backend" | "--solver" => {
                    index += 1;
                    options.backend = parse_backend(args.get(index))?;
                }
                "--emit-obligations" => {
                    index += 1;
                    options.emit_obligations =
                        Some(parse_path_arg(args.get(index), flag.as_ref())?);
                }
                "--emit-smt" => {
                    index += 1;
                    options.emit_smt = Some(parse_path_arg(args.get(index), flag.as_ref())?);
                }
                "--solver-command" | "--z3-command" => {
                    index += 1;
                    options.z3_command = Some(parse_string_arg(args.get(index), flag.as_ref())?);
                }
                other => {
                    eprintln!("error: unknown verify option `{other}`");
                    return Err(ExitCode::from(2));
                }
            }
            index += 1;
        }
        Ok(options)
    }
}

fn parse_mode(arg: Option<&OsString>) -> Result<VerificationMode, ExitCode> {
    match arg.map(|arg| arg.to_string_lossy()).as_deref() {
        Some("dev") => Ok(VerificationMode::Dev),
        Some("test") => Ok(VerificationMode::Test),
        Some("release") => Ok(VerificationMode::Release),
        Some(other) => {
            eprintln!("error: unknown verification mode `{other}`");
            Err(ExitCode::from(2))
        }
        None => {
            eprintln!("error: --mode requires a value");
            Err(ExitCode::from(2))
        }
    }
}

fn parse_backend(arg: Option<&OsString>) -> Result<BackendKind, ExitCode> {
    match arg.map(|arg| arg.to_string_lossy()).as_deref() {
        Some("emit") => Ok(BackendKind::Emit),
        Some("oxiz") => Ok(BackendKind::Oxiz),
        Some("z3") => Ok(BackendKind::Z3),
        Some(other) => {
            eprintln!("error: unknown verifier backend `{other}`");
            Err(ExitCode::from(2))
        }
        None => {
            eprintln!("error: --backend requires a value");
            Err(ExitCode::from(2))
        }
    }
}

fn parse_path_arg(arg: Option<&OsString>, flag: &str) -> Result<PathBuf, ExitCode> {
    arg.map(PathBuf::from).ok_or_else(|| {
        eprintln!("error: {flag} requires a path");
        ExitCode::from(2)
    })
}

fn parse_string_arg(arg: Option<&OsString>, flag: &str) -> Result<String, ExitCode> {
    arg.map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(|| {
            eprintln!("error: {flag} requires a value");
            ExitCode::from(2)
        })
}

fn parse_usize_arg(arg: Option<&OsString>, flag: &str) -> Result<usize, ExitCode> {
    let value = parse_string_arg(arg, flag)?;
    value.parse::<usize>().map_err(|error| {
        eprintln!("error: {flag} requires a positive integer: {error}");
        ExitCode::from(2)
    })
}

fn print_human_diagnostics(report: &VerificationReport) {
    for diagnostic in &report.diagnostics {
        eprintln!("{:?}: {}", diagnostic.severity, diagnostic.message);
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<(), ExitCode> {
    serde_json::to_writer_pretty(std::io::stdout(), value).map_err(|error| {
        eprintln!("error: failed to write JSON: {error}");
        ExitCode::FAILURE
    })?;
    println!();
    Ok(())
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), ExitCode> {
    let json = serde_json::to_string_pretty(value).map_err(|error| {
        eprintln!("error: failed to encode JSON: {error}");
        ExitCode::FAILURE
    })?;
    fs::write(path, json).map_err(|error| {
        eprintln!("error: failed to write {}: {error}", path.display());
        ExitCode::FAILURE
    })
}

fn emit_smt(path: &Path, report: &VerificationReport) -> Result<(), ExitCode> {
    fs::create_dir_all(path).map_err(|error| {
        eprintln!("error: failed to create {}: {error}", path.display());
        ExitCode::FAILURE
    })?;
    for obligation in &report.obligations {
        let Some(problem) = &obligation.smt else {
            continue;
        };
        let file = path.join(format!("{}.smt2", obligation.id));
        fs::write(&file, emit_smt_lib(problem)).map_err(|error| {
            eprintln!("error: failed to write {}: {error}", file.display());
            ExitCode::FAILURE
        })?;
    }
    Ok(())
}

fn solve_report(report: &VerificationReport, backend: BackendKind, z3_command: Option<&str>) {
    for obligation in &report.obligations {
        let Some(problem) = &obligation.smt else {
            continue;
        };
        let outcome = match backend {
            BackendKind::Emit => continue,
            BackendKind::Oxiz => OxizBackend.check(problem),
            BackendKind::Z3 => {
                let backend =
                    z3_command.map_or_else(ExternalZ3Backend::default, ExternalZ3Backend::new);
                backend.check(problem)
            }
        };
        match outcome {
            Ok(outcome) => eprintln!("solver[{backend:?}] {}: {outcome:?}", obligation.id),
            Err(error) => eprintln!("solver[{backend:?}] {}: {error}", obligation.id),
        }
    }
}

fn print_help() {
    eprintln!("Usage:");
    eprintln!("  arcw check <file.awft>");
    eprintln!(
        "  arcw verify <file.awft> [--mode dev|test|release] [--backend emit|oxiz|z3] [--json]"
    );
    eprintln!(
        "  arcw verify <file.awft> --emit-obligations obligations.json --emit-smt out/proofs"
    );
    eprintln!("  arcw unsafe <file.awft> [--json]");
    eprintln!("  arcw plan <file.awft> [--json]");
    eprintln!("  arcw run <file.awft> [--frames N] [--value name=value] [--json]");
    eprintln!("  arcw test <file.awft> [--json]");
    eprintln!("  arcw bench <file.awft> [--json]");
}
