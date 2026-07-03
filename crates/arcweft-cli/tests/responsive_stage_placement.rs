use serde_json::Value;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root canonicalizes")
}

fn arcw_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_arcw"))
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after unix epoch")
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "arcweft-seq06-14-{label}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create seq06.14 temp dir");
    root
}

fn run_arcw(args: &[&str]) -> Output {
    Command::new(arcw_bin())
        .args(args)
        .output()
        .expect("spawn arcw")
}

fn assert_success(command: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{command} should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(command: &str, output: &Output) {
    assert!(
        !output.status.success(),
        "{command} should fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn fixture(path: &str) -> PathBuf {
    workspace_root()
        .join("fixtures/responsive-stage-placement")
        .join(path)
}

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn assert_json_f64_bits(value: &Value, expected: f64) {
    let actual = value.as_f64().expect("JSON value is f64");
    assert_eq!(actual.to_bits(), expected.to_bits());
}

#[test]
fn stand_top_right_bundles_and_observes_responsive_bbox() {
    let work = temp_dir("stand-top-right");
    let output = work.join("stand-top-right.awfb");
    let source = fixture("stand-top-right.arcw");

    let bundle = run_arcw(&[
        "bundle",
        &path_arg(&source),
        "--output",
        &path_arg(&output),
        "--format",
        "awfb",
    ]);
    assert_success("arcw bundle", &bundle);
    assert!(
        output.exists(),
        "bundle output should exist: {}",
        output.display()
    );

    let observe = run_arcw(&[
        "agent",
        "observe",
        &path_arg(&source),
        "--viewport-width",
        "1920",
        "--viewport-height",
        "1080",
        "--image",
        "png",
        "--json",
    ]);
    assert_success("arcw agent observe", &observe);

    let report: Value =
        serde_json::from_slice(&observe.stdout).expect("observe stdout is structured JSON");
    let image = report["objects"]
        .as_array()
        .expect("observe report has objects")
        .iter()
        .find(|object| object["content"]["kind"] == "image")
        .expect("observe report has image object");
    let bbox = &image["content"]["resolved_placement"]["output_bbox"];
    assert_json_f64_bits(&bbox["origin"]["x"], 1395.0);
    assert_json_f64_bits(&bbox["origin"]["y"], 30.0);
    assert_json_f64_bits(&bbox["size"]["width"], 375.0);
    assert_json_f64_bits(&bbox["size"]["height"], 645.0);

    let _ = fs::remove_dir_all(work);
}

#[test]
fn conflicting_constraints_are_typed_diagnostics() {
    let work = temp_dir("conflicting");
    let output = work.join("conflicting.awfb");
    let source = fixture("conflicting-placement.arcw");

    let result = run_arcw(&[
        "bundle",
        &path_arg(&source),
        "--output",
        &path_arg(&output),
        "--format",
        "awfb",
    ]);
    assert_failure("arcw bundle conflicting placement", &result);

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("stage_placement.independent_axis_scale_rejected"),
        "stderr should include typed placement diagnostic\nstderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(work);
}
