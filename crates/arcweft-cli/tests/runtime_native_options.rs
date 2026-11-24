use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[test]
fn text_input_trace_rejects_headless_runner() {
    let fixture = fixture();
    assert_run_usage_error(
        &[
            "--runner",
            "headless",
            fixture.to_str().expect("fixture path is UTF-8"),
            "--text-input-trace-out",
            "target/unused-text-input-trace.json",
        ],
        "--text-input-trace-out requires --runner native",
    );
}

#[test]
fn session_io_requires_an_explicit_native_or_headless_runner() {
    let fixture = fixture();
    for runner in ["auto", "web"] {
        for (option, path) in session_options() {
            assert_run_usage_error(
                &[
                    "--runner",
                    runner,
                    fixture.to_str().expect("fixture path is UTF-8"),
                    option,
                    path,
                ],
                "--session-load and --session-save-out require --runner native or headless",
            );
        }
    }
}

#[test]
fn headless_native_session_preserves_restart_point_and_rejects_a_different_entry() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("checkpoint.arcw");
    let saved = directory.path().join("checkpoint.awfs");
    let completed = directory.path().join("completed.awfs");
    std::fs::write(
        &source,
        "flow main() -> i64 {
             let first = 6i64
             let second = first + 1i64
             return second
         }
         entry cli @entry.main { goto @flow.main }
         entry cli @entry.alias { goto @flow.main }",
    )
    .unwrap();
    let save = saved.to_str().unwrap();
    let finished_save = completed.to_str().unwrap();
    let capture = run_headless_entry(
        &source,
        "entry.main",
        &["--json", "--steps", "1", "--session-save-out", save],
    );
    assert!(capture.status.success(), "{capture:?}");
    let initial: serde_json::Value = serde_json::from_slice(&capture.stdout).unwrap();
    assert_eq!(initial["steps"][0]["index"], 0);
    assert_eq!(initial["final_status"], "running");
    let original = std::fs::read(&saved).unwrap();

    let wrong_entry = run_headless_entry(
        &source,
        "entry.alias",
        &["--session-load", save, "--session-save-out", save],
    );
    assert!(!wrong_entry.status.success(), "{wrong_entry:?}");
    assert!(String::from_utf8_lossy(&wrong_entry.stderr).contains("original invocation or entry"));
    assert_eq!(std::fs::read(&saved).unwrap(), original);

    std::fs::write(&completed, b"previous checkpoint").unwrap();
    let resumed = run_headless_entry(
        &source,
        "entry.main",
        &[
            "--executor",
            "aot",
            "--json",
            "--steps",
            "32",
            "--mode",
            "drain",
            "--max-ops",
            "64",
            "--session-load",
            save,
            "--session-save-out",
            finished_save,
        ],
    );
    assert!(resumed.status.success(), "{resumed:?}");
    let report: serde_json::Value = serde_json::from_slice(&resumed.stdout).unwrap();
    assert_eq!(report["steps"][0]["index"], 1);
    assert_eq!(report["final_status"], "done Return(\"7\")");
    let finished = std::fs::read(&completed).unwrap();
    assert!(finished.starts_with(b"AWFS"));
    assert_ne!(finished, original);

    let observed = run_headless_entry(
        &source,
        "entry.main",
        &["--json", "--steps", "0", "--session-load", finished_save],
    );
    assert!(observed.status.success(), "{observed:?}");
    let observation: serde_json::Value = serde_json::from_slice(&observed.stdout).unwrap();
    assert_eq!(observation["steps"], serde_json::json!([]));
    assert_eq!(observation["final_status"], report["final_status"]);
    assert_eq!(std::fs::read(&completed).unwrap(), finished);

    let mut corrupt = finished;
    *corrupt.last_mut().unwrap() ^= 1;
    std::fs::write(&completed, &corrupt).unwrap();
    let corrupted = run_headless_entry(
        &source,
        "entry.main",
        &["--session-load", finished_save, "--session-save-out", save],
    );
    assert!(!corrupted.status.success(), "{corrupted:?}");
    assert!(String::from_utf8_lossy(&corrupted.stderr).contains("source session save codec error"));
    assert_eq!(std::fs::read(&saved).unwrap(), original);
}

#[test]
fn headless_native_session_requires_the_original_asset_before_dispatch() {
    let directory = tempfile::tempdir().unwrap();
    let (source, image) = asset_checkpoint_source(directory.path());
    let saved = directory.path().join("before-asset.awfs");
    let original_image = single_pixel_png([255, 0, 0, 255]);
    std::fs::write(&image, &original_image).unwrap();
    let save = saved.to_str().unwrap();
    let capture = run_headless_entry(
        &source,
        "entry.main",
        &["--steps", "1", "--session-save-out", save],
    );
    assert!(capture.status.success(), "{capture:?}");
    let original_save = std::fs::read(&saved).unwrap();

    std::fs::write(&image, single_pixel_png([0, 0, 255, 255])).unwrap();
    let changed = run_headless_entry(
        &source,
        "entry.main",
        &["--session-load", save, "--session-save-out", save],
    );
    assert!(!changed.status.success(), "{changed:?}");
    assert!(
        String::from_utf8_lossy(&changed.stderr).contains("source session bundle asset context")
    );
    assert_eq!(std::fs::read(&saved).unwrap(), original_save);

    std::fs::write(&image, original_image).unwrap();
    let resumed = run_headless_entry(
        &source,
        "entry.main",
        &[
            "--executor",
            "aot",
            "--json",
            "--steps",
            "8",
            "--mode",
            "drain",
            "--max-ops",
            "64",
            "--session-load",
            save,
        ],
    );
    assert!(resumed.status.success(), "{resumed:?}");
    let report: serde_json::Value = serde_json::from_slice(&resumed.stdout).unwrap();
    assert_eq!(report["steps"][0]["index"], 1);
    assert_eq!(report["final_status"], "done Return(\"asset_ready\")");
    assert_eq!(report["native_io"]["completed_tasks"], 1);
}

#[test]
fn headless_native_session_reissues_an_in_flight_asset_across_processes() {
    let directory = tempfile::tempdir().unwrap();
    let (source, image) = asset_checkpoint_source(directory.path());
    std::fs::write(&image, single_pixel_png([255, 0, 0, 255])).unwrap();
    let first_save = directory.path().join("first.awfs");
    let next_save = directory.path().join("next.awfs");
    let first_path = first_save.to_str().unwrap();
    let next_path = next_save.to_str().unwrap();

    let first = run_headless_asset_steps(
        &source,
        "bytecode-vm",
        "1",
        &["--session-save-out", first_path],
    );
    let original_requests = first["steps"][0]["task_requests"].as_array().unwrap();
    assert_eq!(original_requests.len(), 1);
    assert_eq!(first["steps"][0]["index"], 0);
    assert_eq!(first["native_io"]["scheduler"]["in_flight"], 1);
    assert_eq!(first["native_io"]["completed_tasks"], 0);

    let resumed = run_headless_asset_steps(
        &source,
        "aot",
        "1",
        &[
            "--session-load",
            first_path,
            "--session-save-out",
            next_path,
        ],
    );
    assert_eq!(resumed["steps"][0]["index"], 1);
    assert_eq!(
        resumed["steps"][0]["task_requests"],
        first["steps"][0]["task_requests"]
    );
    assert_eq!(resumed["native_io"]["scheduler"]["in_flight"], 1);
    assert_eq!(resumed["native_io"]["completed_tasks"], 0);

    let completed = run_headless_asset_steps(&source, "aot", "8", &["--session-load", next_path]);
    assert_eq!(completed["steps"][0]["index"], 2);
    assert_eq!(completed["final_status"], "done Return(\"asset_ready\")");
    assert_eq!(completed["native_io"]["scheduler"]["dispatched"], 1);
    assert_eq!(completed["native_io"]["scheduler"]["in_flight"], 0);
    assert_eq!(completed["native_io"]["completed_tasks"], 1);
}

#[test]
fn headless_native_session_restores_a_retained_asset_result_and_reissues_only_pending_work() {
    let directory = tempfile::tempdir().unwrap();
    let (source, image) = write_asset_checkpoint_source(
        directory.path(),
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String effects { asset.read } {
    let background = @asset:.bg.room
    let first = await asset.image(background)
    let second = await asset.image(background)
    return match (first, second) {
        (.Ok(_), .Ok(_)) => "assets_ready"
        _ => "asset_error"
    }
}
"#,
    );
    std::fs::write(image, single_pixel_png([255, 0, 0, 255])).unwrap();
    let save = directory.path().join("retained-asset.awfs");
    let save = save.to_str().unwrap();

    let stopped =
        run_headless_asset_steps(&source, "bytecode-vm", "2", &["--session-save-out", save]);
    assert_eq!(stopped["native_io"]["completed_tasks"], 1);
    assert_eq!(stopped["native_io"]["scheduler"]["in_flight"], 1);
    let first_request = &stopped["steps"][0]["task_requests"];
    let pending_request = &stopped["steps"][1]["task_requests"];
    assert_eq!(first_request.as_array().unwrap().len(), 1);
    assert_eq!(pending_request.as_array().unwrap().len(), 1);
    assert_ne!(first_request, pending_request);

    let resumed = run_headless_asset_steps(&source, "aot", "8", &["--session-load", save]);
    assert_eq!(resumed["steps"][0]["index"], 2);
    assert_eq!(&resumed["steps"][0]["task_requests"], pending_request);
    assert_eq!(resumed["steps"][1]["task_requests"], serde_json::json!([]));
    assert_eq!(resumed["native_io"]["scheduler"]["dispatched"], 1);
    assert_eq!(resumed["native_io"]["completed_tasks"], 1);
    assert_eq!(resumed["final_status"], "done Return(\"assets_ready\")");
}

#[test]
fn authored_voice_load_restarts_native_and_resolves_the_same_awfb_resource() {
    assert_voice_load_survives_native_restart(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String effects { asset.read } {
    let recording = @voice.alice.001
    let loaded = await voice.load(recording)
    let missing = await voice.load(@voice:.alice.missing)
    return match (loaded, missing) {
        (.Ok(_), .Err(_)) => "voice_ready"
        _ => "voice_result_mismatch"
    }
}
"#,
    );
}

#[test]
fn retained_prefix_in_nested_voice_load_arguments_survives_native_restart() {
    assert_voice_load_survives_native_restart(
        r#"
entry cli @entry.main { goto @flow.main }
fn choose<A, B>(first: A)(second: B) -> B { second }
flow main() -> String effects { asset.read } {
    let prefix = choose(7i64)
    let before = prefix("kept")
    let loaded = await voice.load(prefix(@voice.alice.001))
    let missing = await voice.load(prefix(@voice:.alice.missing))
    let after = prefix(42i64)
    return match (loaded, missing, before == "kept", after == 42i64) {
        (.Ok(_), .Err(_), true, true) => "voice_ready"
        _ => "voice_result_mismatch"
    }
}
"#,
    );
}

fn assert_voice_load_survives_native_restart(source_text: &str) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("voice-checkpoint.arcw");
    let assets = directory.path().join("assets/audio/voice/alice");
    std::fs::create_dir_all(&assets).unwrap();
    let samples = mono_voice_wav();
    std::fs::write(assets.join("001.wav"), &samples).unwrap();
    std::fs::write(&source, source_text).unwrap();

    let output_bundle = directory.path().join("voice.awfb");
    let packaged = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bundle")
        .arg(&source)
        .arg("--output")
        .arg(&output_bundle)
        .output()
        .unwrap();
    assert!(packaged.status.success(), "{packaged:?}");
    let bytes = std::fs::read(&output_bundle).unwrap();
    let bundle = arcweft_bundle::ArcweftBundle::from_product_path_slice(&output_bundle, &bytes)
        .expect("the published AWFB decodes");
    assert_eq!(
        bundle.audio_asset_bytes("asset.voice.alice.001").unwrap(),
        Some(samples.as_slice())
    );

    let save = directory.path().join("voice.awfs");
    let save = save.to_str().unwrap();
    let stopped =
        run_headless_asset_steps(&source, "bytecode-vm", "2", &["--session-save-out", save]);
    assert_eq!(stopped["native_io"]["completed_tasks"], 1);
    assert_eq!(stopped["native_io"]["scheduler"]["in_flight"], 1);
    let pending = &stopped["steps"][1]["task_requests"];
    assert_eq!(pending.as_array().unwrap().len(), 1);

    let resumed = run_headless_asset_steps(&source, "aot", "8", &["--session-load", save]);
    assert_eq!(resumed["steps"][0]["index"], 2);
    assert_eq!(&resumed["steps"][0]["task_requests"], pending);
    assert_eq!(resumed["native_io"]["completed_tasks"], 1);
    assert_eq!(resumed["final_status"], "done Return(\"voice_ready\")");

    let product = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run-bundle")
        .arg(&output_bundle)
        .args(["--entry", "entry.main", "--max-ops", "64", "--json"])
        .output()
        .unwrap();
    assert!(product.status.success(), "{product:?}");
    let report: serde_json::Value = serde_json::from_slice(&product.stdout).unwrap();
    assert_eq!(report["native_io"]["completed_tasks"], 2);
    assert_eq!(report["final_status"], "done return voice_ready");
}

#[test]
fn catalog_need_callables_execute_through_values_and_parallel_traversal() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("catalog-callables.arcw");
    let voices = directory.path().join("assets/audio/voice/alice");
    let images = directory.path().join("assets/bg");
    std::fs::create_dir_all(&voices).unwrap();
    std::fs::create_dir_all(&images).unwrap();
    for voice in 1..=7 {
        std::fs::write(voices.join(format!("{voice:03}.wav")), mono_voice_wav()).unwrap();
    }
    std::fs::write(images.join("blue.png"), single_pixel_png([0, 0, 255, 255])).unwrap();
    std::fs::write(images.join("red.png"), single_pixel_png([255, 0, 0, 255])).unwrap();
    std::fs::write(
        &source,
        r#"
fn load_voice(recording: Ref<Voice>) -> Need<Result<AudioHandle, VoiceError>> {
    voice.load(recording)
}
entry cli @entry.main { goto @flow.main }
flow main() -> String effects { asset.read } {
    let load = voice.load
    let single = await load(@voice.alice.001)
    let named_argument = await load(voice = @voice.alice.007)
    let images = await [@asset.bg.blue, @asset.bg.red].traverse(asset.image).parallel(limit = 2u32)
    let voices = await [@voice.alice.002, @voice.alice.003].traverse(voice.load).parallel(limit = 2u32)
    let named = await [@voice.alice.004, @voice.alice.005].traverse(load_voice).parallel(limit = 2u32)
    let closure = await [@voice.alice.006, @voice.alice.missing].traverse(|item| voice.load(item)).parallel(limit = 2u32)
    return match (single, named_argument, images, voices, named, closure) {
        (.Ok(_), .Ok(_), .Ok(_), .Ok(_), .Ok(_), .Err(_)) => "catalog_ready"
        _ => "catalog_error"
    }
}
"#,
    )
    .unwrap();

    let native = run_headless_asset_steps(&source, "bytecode-vm", "32", &[]);
    assert_eq!(native["final_status"], "done Return(\"catalog_ready\")");
    assert_eq!(
        native["native_io"]["scheduler"]["dispatched_by_class"]["asset_decode"],
        10
    );

    let bundle = directory.path().join("catalog-callables.awfb");
    let packaged = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bundle")
        .arg(&source)
        .arg("--output")
        .arg(&bundle)
        .output()
        .unwrap();
    assert!(packaged.status.success(), "{packaged:?}");
    let product = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run-bundle")
        .arg(&bundle)
        .args([
            "--entry",
            "entry.main",
            "--steps",
            "32",
            "--max-ops",
            "64",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(product.status.success(), "{product:?}");
    let report: serde_json::Value = serde_json::from_slice(&product.stdout).unwrap();
    assert_eq!(report["final_status"], "done return catalog_ready");
    assert_eq!(
        report["native_io"]["scheduler"]["dispatched_by_class"]["asset_decode"],
        10
    );
}

fn mono_voice_wav() -> Vec<u8> {
    let mut bytes = b"RIFF\xc4\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00".to_vec();
    bytes.extend_from_slice(&8_000_u32.to_le_bytes());
    bytes.extend_from_slice(&16_000_u32.to_le_bytes());
    bytes.extend_from_slice(b"\x02\x00\x10\x00data");
    bytes.extend_from_slice(&160_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 160]);
    bytes
}

fn run_headless_asset_steps(
    source: &Path,
    executor: &str,
    steps: &str,
    options: &[&str],
) -> serde_json::Value {
    let mut arguments = vec![
        "--executor",
        executor,
        "--json",
        "--steps",
        steps,
        "--mode",
        "drain",
        "--max-ops",
        "64",
    ];
    arguments.extend_from_slice(options);
    let output = run_headless_entry(source, "entry.main", &arguments);
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn asset_checkpoint_source(directory: &Path) -> (PathBuf, PathBuf) {
    write_asset_checkpoint_source(
        directory,
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String effects { asset.read } {
    let background = @asset:.bg.room
    return match (await asset.image(background)) {
        .Ok(_) => "asset_ready"
        .Err(_) => "asset_error"
    }
}
"#,
    )
}

fn write_asset_checkpoint_source(directory: &Path, text: &str) -> (PathBuf, PathBuf) {
    let source = directory.join("asset-checkpoint.arcw");
    let assets = directory.join("assets/bg");
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::write(&source, text).unwrap();
    (source, assets.join("room.png"))
}

fn single_pixel_png(pixel: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&pixel).unwrap();
    writer.finish().unwrap();
    bytes
}

#[test]
fn session_io_rejects_watch_mode() {
    let fixture = fixture();
    for (option, path) in session_options() {
        assert_run_usage_error(
            &[
                "--runner",
                "native",
                fixture.to_str().expect("fixture path is UTF-8"),
                "--watch",
                option,
                path,
            ],
            "--session-load and --session-save-out cannot be combined with --watch",
        );
    }
}

fn session_options() -> [(&'static str, &'static str); 2] {
    [
        ("--session-load", "target/missing-session-load.json"),
        ("--session-save-out", "target/unused-session-save.json"),
    ]
}

fn run_headless_entry(source: &Path, entry: &str, options: &[&str]) -> Output {
    let mut arguments = vec![
        "--runner",
        "headless",
        source.to_str().unwrap(),
        "--entry",
        entry,
    ];
    arguments.extend_from_slice(options);
    run(&arguments)
}

fn assert_run_usage_error(arguments: &[&str], expected: &str) {
    let output = run(arguments);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(
        stderr.contains(expected),
        "stderr did not contain {expected:?}: {stderr}"
    );
}

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .args(arguments)
        .output()
        .expect("arcw run starts")
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/arcw/spec_should_pass/run/001_cli_stdout_entry.arcw")
}
