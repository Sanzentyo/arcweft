#[path = "support/cli.rs"]
mod cli;
#[path = "support/json.rs"]
mod json;
#[path = "support/temp.rs"]
mod temp;
#[path = "support/workspace.rs"]
mod workspace;

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

#[test]
fn support_helpers_work_for_temp_files_and_json_envelopes() {
    let temp = temp::TempDir::new("check-core-cli").expect("create temp dir");
    assert!(temp.path().exists());

    let workspace = workspace::TestWorkspace::new("check-core-cli").expect("create workspace");
    let written = workspace
        .write("nested/input.txt", "hello from arcweft")
        .expect("write workspace file");
    assert_eq!(
        std::fs::read_to_string(&written).expect("read workspace file"),
        "hello from arcweft"
    );

    let json = json::parse_json(r#"{"error":{"code":"arcw.invalid_option"}}"#);
    json::assert_json_error_code(&json, "arcw.invalid_option");
}
