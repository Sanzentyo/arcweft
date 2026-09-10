#[path = "support/cli.rs"]
mod cli;

use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;

use cli::CommandOutput;

fn temp_arcw(name: &str, source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "arcweft-check-core-{name}-{}.arcw",
        std::process::id()
    ));
    fs::write(&path, source).expect("write temporary Arcweft source");
    path
}

#[test]
fn help_is_available_without_project_files() {
    let output = CommandOutput::run(["--help"]).expect("run arcw --help");
    output.assert_success();
    assert!(output.stdout().contains("Usage"));
}

#[test]
fn unknown_top_level_option_fails_with_diagnostic() {
    let output = CommandOutput::run(["--definitely-not-an-arcweft-option"])
        .expect("run arcw with invalid option");
    output.assert_failure();
    assert!(!output.stderr().trim().is_empty());
}

#[test]
fn check_reports_typed_statement_unknown_mode_once() {
    let path = temp_arcw(
        "assertion-unknown-mode",
        "flow demo {\n    assert.assume(true)\n}\n",
    );
    let output = CommandOutput::run([OsStr::new("check"), path.as_os_str()])
        .expect("run arcw check with malformed assertion");

    output.assert_failure();
    let stderr = output.stderr();
    assert_eq!(stderr.matches("syntax.assert.unknown_mode").count(), 1);
    assert_eq!(stderr.matches("unknown assertion mode").count(), 1);
    fs::remove_file(path).expect("remove temporary Arcweft source");
}

#[test]
fn check_reports_final_call_diagnostics_once() {
    for (name, source, code) in [
        (
            "surplus-call",
            "fn identity(value: i64) -> i64 { value }\nflow main() -> Unit { identity(1i64, 2i64); }\nentry cli @entry.main { goto @flow.main }\n",
            "sema.call.no_viable_signature",
        ),
        (
            "non-callable",
            "flow main() -> Unit { let value = 1i64; value(); }\nentry cli @entry.main { goto @flow.main }\n",
            "sema.call.non_callable_target",
        ),
    ] {
        let path = temp_arcw(name, source);
        let output = CommandOutput::run([OsStr::new("check"), path.as_os_str()])
            .expect("run check on a rejected call");
        output.assert_failure();
        assert_eq!(
            output.stderr().matches(code).count(),
            1,
            "{}",
            output.stderr()
        );
        fs::remove_file(path).expect("remove temporary source");
    }
}

#[test]
fn check_keeps_explicit_id_lint_on_the_independent_lint_path() {
    let path = temp_arcw("explicit-id-lint", "flow @flow.opening {\n}\n");
    let output = CommandOutput::run([OsStr::new("check"), path.as_os_str()])
        .expect("run arcw check with explicit declaration id");

    output.assert_success();
    let stderr = output.stderr();
    assert!(stderr.contains("AWF0103"));
    assert!(stderr.contains("style::explicit_decl_id"));
    assert!(!stderr.contains("syntax.assert."));
    fs::remove_file(path).expect("remove temporary Arcweft source");
}
