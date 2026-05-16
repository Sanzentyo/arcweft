use arcweft_lang_syntax::{
    TypeCheckEnv, lint_id_policy, lower_line_task_groups, lower_to_hir, parse_source,
    registry_from_hir, typecheck_hir, validate_hir_references, validate_typecheck_ready,
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
        [command, ..] => {
            eprintln!("error: unknown command `{}`", command.to_string_lossy());
            print_help();
            Err(ExitCode::from(2))
        }
    }
}

fn check(path: &Path) -> Result<(), ExitCode> {
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

    println!(
        "ok: {} ({} flow(s), {} line task group(s), {} warning(s))",
        path.display(),
        hir.flows().len(),
        line_task_groups.len(),
        lints.len()
    );
    Ok(())
}

fn print_help() {
    eprintln!("Usage:");
    eprintln!("  arcw check <file.awft>");
}
