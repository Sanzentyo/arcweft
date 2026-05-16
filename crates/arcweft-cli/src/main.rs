use arcweft_lang_syntax::{
    TypeCheckEnv, lint_id_policy, lower_line_task_groups, lower_to_hir, parse_source,
    registry_from_hir, typecheck_hir, validate_hir_references, validate_typecheck_ready,
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
        [command, rest @ ..] if command == "test" => script_test_command(rest),
        [command, rest @ ..] if command == "bench" => script_bench_command(rest),
        [command, ..] => {
            eprintln!("error: unknown command `{}`", command.to_string_lossy());
            print_help();
            Err(ExitCode::from(2))
        }
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
        checked.line_task_groups,
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
    line_task_groups: usize,
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
        Ok(groups) => groups.len(),
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
    eprintln!("  arcw test <file.awft> [--json]");
    eprintln!("  arcw bench <file.awft> [--json]");
}
