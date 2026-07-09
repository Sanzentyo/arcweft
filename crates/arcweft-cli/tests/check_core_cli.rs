#[path = "support/cli.rs"]
mod cli;

use cli::CommandOutput;

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
