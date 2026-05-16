use arcweft_core::{
    LineEffectRequest, LineTaskGroup, LineTaskNode, LineTaskScope, LineTaskTrigger,
};
use arcweft_lang_syntax::{
    LoweredLineTaskGroup, TypeCheckEnv, lint_id_policy, lower_line_task_groups, lower_to_hir,
    parse_source, registry_from_hir, typecheck_hir, validate_hir_references,
    validate_typecheck_ready,
};
use arcweft_test::{ScriptTestManifest, collect_script_tests};
use arcweft_verify::{
    BackendKind, SmtBackend, VerificationMode, VerificationPolicy, VerificationReport,
    emit_smt_lib, verify_module,
};
use arcweft_verify_oxiz::OxizBackend;
use arcweft_verify_z3::ExternalZ3Backend;
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

fn script_test_command(args: &[OsString]) -> Result<(), ExitCode> {
    let options = ScriptPlanOptions::parse(args, "test")?;
    let checked = load_and_check(&options.path)?;
    let manifest = collect_script_tests(&checked.hir);
    let output = ScriptTestManifest {
        tests: manifest.tests,
        benches: Vec::new(),
    };
    if options.json {
        print_json(&output)
    } else {
        for test in &output.tests {
            println!("{} {} ({} step(s))", test.id, test.kind, test.steps.len());
        }
        println!(
            "ok: {} ({} script test(s))",
            options.path.display(),
            output.tests.len()
        );
        Ok(())
    }
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
            json: false,
        };
        for flag in &args[1..] {
            match flag.to_string_lossy().as_ref() {
                "--json" => options.json = true,
                other => {
                    eprintln!("error: unknown {command} option `{other}`");
                    return Err(ExitCode::from(2));
                }
            }
        }
        Ok(options)
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
    eprintln!("  arcw test <file.awft> [--json]");
    eprintln!("  arcw bench <file.awft> [--json]");
}

#[derive(serde::Serialize)]
struct RuntimePlanReport {
    lines: Vec<RuntimeLinePlanSummary>,
    verifier_diagnostics: usize,
    verifier_obligations: usize,
}

#[derive(serde::Serialize)]
struct RuntimeLinePlanSummary {
    flow_id: Option<String>,
    line_id: Option<String>,
    callee: String,
    child_tasks: usize,
    effects: usize,
    root: RuntimeNodeSummary,
    options: usize,
    bindings: usize,
    out: usize,
    cancel_rules: usize,
    memo: usize,
    assertions: usize,
}

#[derive(serde::Serialize)]
struct RuntimeScopeSummary {
    node: Box<RuntimeNodeSummary>,
    defer_count: usize,
    completed_defer_count: usize,
    cancelled_defer_count: usize,
    failed_defer_count: usize,
}

#[derive(serde::Serialize)]
struct RuntimeNodeSummary {
    kind: String,
    children: Vec<RuntimeNodeSummary>,
    task: Option<Box<RuntimeTaskSummary>>,
    effect: Option<String>,
}

#[derive(serde::Serialize)]
struct RuntimeTaskSummary {
    id: String,
    key: Option<String>,
    name: Option<String>,
    trigger: String,
    priority: i32,
    join_policy: String,
    cancel_policy: String,
    scope: Box<RuntimeScopeSummary>,
}

impl RuntimePlanReport {
    fn from_checked(checked: &CheckedModule) -> Self {
        let verification = verify_module(
            &checked.hir,
            VerificationPolicy {
                mode: VerificationMode::Dev,
                backend: BackendKind::Emit,
            },
        );
        Self {
            lines: checked
                .line_task_groups
                .iter()
                .map(RuntimeLinePlanSummary::from_lowered)
                .collect(),
            verifier_diagnostics: verification.diagnostics.len(),
            verifier_obligations: verification.obligations.len(),
        }
    }
}

impl RuntimeLinePlanSummary {
    fn from_lowered(line: &LoweredLineTaskGroup) -> Self {
        let group = line.group();
        let root = node_summary(&group.root.node);
        Self {
            flow_id: line.flow_id().map(|id| id.body().to_owned()),
            line_id: line.line_id().map(|id| id.body().to_owned()),
            callee: line.callee().to_owned(),
            child_tasks: count_child_tasks(group),
            effects: count_effects(group),
            root,
            options: group.options.len(),
            bindings: group.bindings.len(),
            out: group.out.len(),
            cancel_rules: group.cancel_rules.len(),
            memo: group.memo.len(),
            assertions: group.assertions.len(),
        }
    }
}

fn scope_summary(scope: &LineTaskScope) -> RuntimeScopeSummary {
    RuntimeScopeSummary {
        node: Box::new(node_summary(&scope.node)),
        defer_count: scope.defer_stack.len(),
        completed_defer_count: scope.completed_defer_stack.len(),
        cancelled_defer_count: scope.cancelled_defer_stack.len(),
        failed_defer_count: scope.failed_defer_stack.len(),
    }
}

fn node_summary(node: &LineTaskNode) -> RuntimeNodeSummary {
    match node {
        LineTaskNode::Seq(children) => node_children_summary("seq", children),
        LineTaskNode::Start(children) => node_children_summary("start", children),
        LineTaskNode::Parallel { children, .. } => node_children_summary("parallel", children),
        LineTaskNode::Child(task) => RuntimeNodeSummary {
            kind: "child".to_owned(),
            children: Vec::new(),
            task: Some(Box::new(RuntimeTaskSummary {
                id: task.id.0.clone(),
                key: task.key.as_ref().map(|key| key.0.clone()),
                name: task.name.clone(),
                trigger: trigger_label(&task.trigger),
                priority: task.priority.0,
                join_policy: format!("{:?}", task.join_policy),
                cancel_policy: format!("{:?}", task.cancel_policy),
                scope: Box::new(scope_summary(&task.scope)),
            })),
            effect: None,
        },
        LineTaskNode::Effect(effect) => RuntimeNodeSummary {
            kind: "effect".to_owned(),
            children: Vec::new(),
            task: None,
            effect: Some(effect_label(effect)),
        },
    }
}

fn node_children_summary(kind: &str, children: &[LineTaskNode]) -> RuntimeNodeSummary {
    RuntimeNodeSummary {
        kind: kind.to_owned(),
        children: children.iter().map(node_summary).collect(),
        task: None,
        effect: None,
    }
}

fn trigger_label(trigger: &LineTaskTrigger) -> String {
    match trigger {
        LineTaskTrigger::Immediate => "immediate".to_owned(),
        LineTaskTrigger::Mark(name) => format!("mark {name}"),
        LineTaskTrigger::Delay(duration) => format!("delay {}ns", duration.as_nanos()),
    }
}

fn effect_label(effect: &LineEffectRequest) -> String {
    match effect {
        LineEffectRequest::RegisterHandle { key, .. } => format!("register {key}"),
        LineEffectRequest::DropHandle { key } => format!("drop {key}"),
        LineEffectRequest::WaitMark(mark) => format!("wait mark {mark}"),
        LineEffectRequest::Wait(duration) => format!("wait {}ns", duration.as_nanos()),
        LineEffectRequest::Call(call) => format!("call {}", call.callee),
        LineEffectRequest::Log(log) => format!("log.{}", log.level),
        LineEffectRequest::SignalWrite(write) => format!("signal.set {}", write.target),
        LineEffectRequest::MetricWrite(write) => format!("metric.set {}", write.target),
        LineEffectRequest::EmitEvent(event) => format!("event.emit {}", event.event),
        LineEffectRequest::Command(command) => format!("command {}", command.name),
        LineEffectRequest::Out(_) => "out".to_owned(),
        LineEffectRequest::Return(_) => "return".to_owned(),
        LineEffectRequest::Goto(_) => "goto".to_owned(),
        LineEffectRequest::Yield(_) => "yield".to_owned(),
        LineEffectRequest::Panic(_) => "panic".to_owned(),
        LineEffectRequest::Fail(_) => "fail".to_owned(),
        LineEffectRequest::Bail(_) => "bail".to_owned(),
        LineEffectRequest::Ensure { .. } => "ensure".to_owned(),
        LineEffectRequest::Close(_) => "close".to_owned(),
        LineEffectRequest::Select(_) => "select".to_owned(),
        LineEffectRequest::Break { .. } => "break".to_owned(),
        LineEffectRequest::Continue { .. } => "continue".to_owned(),
    }
}

fn count_child_tasks(group: &LineTaskGroup) -> usize {
    count_child_tasks_in_node(&group.root.node)
}

fn count_child_tasks_in_node(node: &LineTaskNode) -> usize {
    match node {
        LineTaskNode::Seq(children)
        | LineTaskNode::Start(children)
        | LineTaskNode::Parallel { children, .. } => {
            children.iter().map(count_child_tasks_in_node).sum()
        }
        LineTaskNode::Child(task) => 1 + count_child_tasks_in_node(&task.scope.node),
        LineTaskNode::Effect(_) => 0,
    }
}

fn count_effects(group: &LineTaskGroup) -> usize {
    count_effects_in_scope(&group.root)
}

fn count_effects_in_scope(scope: &LineTaskScope) -> usize {
    count_effects_in_node(&scope.node)
        + scope.defer_stack.iter().map(Vec::len).sum::<usize>()
        + scope
            .completed_defer_stack
            .iter()
            .map(Vec::len)
            .sum::<usize>()
        + scope
            .cancelled_defer_stack
            .iter()
            .map(Vec::len)
            .sum::<usize>()
        + scope.failed_defer_stack.iter().map(Vec::len).sum::<usize>()
}

fn count_effects_in_node(node: &LineTaskNode) -> usize {
    match node {
        LineTaskNode::Seq(children) | LineTaskNode::Start(children) => {
            children.iter().map(count_effects_in_node).sum()
        }
        LineTaskNode::Parallel { children, .. } => children.iter().map(count_effects_in_node).sum(),
        LineTaskNode::Child(task) => count_effects_in_scope(&task.scope),
        LineTaskNode::Effect(_) => 1,
    }
}
