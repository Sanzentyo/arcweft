#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "collect-seq06-13e1-inset-shadow-pinned-golden-evidence"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
publish = false

[dependencies]
serde_json = "1.0.150"
sha2 = "0.10.9"
---

/*
Collects seq06.13e.1 pinned inset box-shadow exact PNG evidence.

The script writes evidence under --out-dir and never copies candidates into
checked-in baseline paths. Baseline promotion remains a documented review action.

Default mode is a dry preflight. Use --run only in the pinned visual-golden job.
The Web --run path builds the wasm player, runs the repo-owned browser/WebGPU
exact readback capture, and then runs the generic Web smoke as a separate sanity
check. The visual source for the exact packet is never a DOM/CSS screenshot.
*/

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const REQUIRED_ENV: &str = "ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_REQUIRED";
const PINNED_ENV: &str = "ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED";
const NATIVE_BACKEND_ENV: &str = "ARW_SEQ06_13E1_INSET_SHADOW_NATIVE_BACKEND";
const WEBGPU_ENV: &str = "ARW_SEQ06_13E1_INSET_SHADOW_WEBGPU";
const NATIVE_BACKEND: &str = "wgpu_offscreen_compositor";
const METRICS: &str = "psnr,ssim,mse,mae,maxae";
const MAX_MSE: f64 = 0.002;
const MAX_MAE: f64 = 0.003;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse(env::args().skip(1))?;
    if args.help {
        print_help();
        return Ok(());
    }

    let root = args
        .root
        .canonicalize()
        .map_err(|error| format!("canonicalize {}: {error}", args.root.display()))?;
    let out_dir = if args.out_dir.is_absolute() {
        args.out_dir
    } else {
        root.join(args.out_dir)
    };
    fs::create_dir_all(&out_dir).map_err(|error| format!("create {}: {error}", out_dir.display()))?;

    let mut classifications = Vec::new();
    for &target in args.mode.targets() {
        write_environment(&root, &out_dir, target)?;
        classifications.push(classify_environment(&root, target));
    }
    write_review_decision(&out_dir, &classifications, args.run)?;

    if !args.run {
        println!("wrote dry-run seq06.13e.1 evidence preflight to {}", out_dir.display());
        return Ok(());
    }

    for classification in &classifications {
        if classification.is_blocking() {
            return Err(format!(
                "{} blocked pinned evidence collection: {}",
                classification.target.as_str(),
                classification.message
            ));
        }
    }

    for &target in args.mode.targets() {
        run_smoke_and_capture_commands(&root, &out_dir, target)?;
        validate_existing_artifact_packet(&root, &out_dir, target)?;
    }

    Ok(())
}

fn print_help() {
    println!(
        "collect-seq06-13e1-inset-shadow-pinned-golden-evidence\n\n\
         Usage:\n  cargo +nightly -Zscript tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs --root . [--out-dir DIR] [--mode native|web|both] [--run]\n\n\
         Options:\n  --root <repo-root>\n  --out-dir <dir>\n  --mode <native|web|both>\n  --run                 Execute pinned commands; default only writes dry-run fingerprints.\n  -h, --help            Print this help."
    );
}

#[derive(Debug)]
struct Args {
    root: PathBuf,
    out_dir: PathBuf,
    mode: Mode,
    run: bool,
    help: bool,
}

impl Args {
    fn parse(values: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut root = None;
        let mut out_dir = PathBuf::from("target/seq06.13e.1-inset-box-shadow-golden");
        let mut mode = Mode::Both;
        let mut run = false;
        let mut help = false;
        let mut values = values.peekable();

        while let Some(arg) = values.next() {
            match arg.as_str() {
                "--root" => root = Some(PathBuf::from(next_arg(&mut values, "--root")?)),
                "--out-dir" => out_dir = PathBuf::from(next_arg(&mut values, "--out-dir")?),
                "--mode" => mode = Mode::parse(&next_arg(&mut values, "--mode")?)?,
                "--run" => run = true,
                "--help" | "-h" => help = true,
                unknown => return Err(format!("unknown argument `{unknown}`")),
            }
        }

        Ok(Self {
            root: if help { root.unwrap_or_else(|| PathBuf::from(".")) } else { root.ok_or_else(|| String::from("missing --root"))? },
            out_dir,
            mode,
            run,
            help,
        })
    }
}

fn next_arg(
    values: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    name: &str,
) -> Result<String, String> {
    values.next().ok_or_else(|| format!("{name} requires a value"))
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    Native,
    Web,
    Both,
}

impl Mode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "native" => Ok(Self::Native),
            "web" => Ok(Self::Web),
            "both" => Ok(Self::Both),
            _ => Err(format!("unknown mode `{value}`; expected native, web, or both")),
        }
    }

    const fn targets(self) -> &'static [Target] {
        match self {
            Self::Native => &[Target::Native],
            Self::Web => &[Target::Web],
            Self::Both => &[Target::Native, Target::Web],
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Target {
    Native,
    Web,
}

impl Target {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Web => "web",
        }
    }

    const fn schema(self) -> &'static str {
        match self {
            Self::Native => "arcweft.seq06.13e1.inset_box_shadow.native_environment.v1",
            Self::Web => "arcweft.seq06.13e1.inset_box_shadow.web_environment.v1",
        }
    }

    fn dir(self, out_dir: &Path) -> PathBuf {
        out_dir.join(self.as_str())
    }
}

#[derive(Clone, Debug)]
struct Classification {
    target: Target,
    status: &'static str,
    code: &'static str,
    message: String,
}

impl Classification {
    fn is_blocking(&self) -> bool {
        matches!(
            self.status,
            "environment_not_pinned" | "environment_blocker" | "baseline_missing" | "hard_visual_regression"
        )
    }
}

fn classify_environment(root: &Path, target: Target) -> Classification {
    if env_present(REQUIRED_ENV) && !env_present(PINNED_ENV) {
        return Classification {
            target,
            status: "environment_not_pinned",
            code: "missing_required_pin",
            message: format!("{PINNED_ENV}=1 is required when {REQUIRED_ENV} is set"),
        };
    }
    if !command_available("imq") {
        return Classification {
            target,
            status: if env_present(REQUIRED_ENV) { "environment_blocker" } else { "expected_skip" },
            code: "missing_imq",
            message: String::from("imq is required for exact PNG metrics"),
        };
    }
    match target {
        Target::Native => classify_native_environment(),
        Target::Web => classify_web_environment(root),
    }
}

fn classify_native_environment() -> Classification {
    if !cfg!(windows) {
        return Classification {
            target: Target::Native,
            status: if env_present(REQUIRED_ENV) { "environment_blocker" } else { "expected_skip" },
            code: "unsupported_os",
            message: String::from("native exact inset box-shadow golden requires the pinned Windows native GPU job"),
        };
    }
    if env::var(NATIVE_BACKEND_ENV).ok().as_deref() != Some(NATIVE_BACKEND) {
        return Classification {
            target: Target::Native,
            status: if env_present(REQUIRED_ENV) { "environment_blocker" } else { "expected_skip" },
            code: "unsupported_backend",
            message: format!("{NATIVE_BACKEND_ENV} must be {NATIVE_BACKEND}"),
        };
    }
    if !pinned_font_available() {
        return Classification {
            target: Target::Native,
            status: if env_present(REQUIRED_ENV) { "environment_blocker" } else { "expected_skip" },
            code: "missing_pinned_font_probe",
            message: String::from("MS Mincho font probe is required to preserve the existing native exact visual policy"),
        };
    }
    Classification {
        target: Target::Native,
        status: "preflight_passed",
        code: "ready",
        message: String::from("native pinned preflight passed; artifact packet still must be validated"),
    }
}

fn classify_web_environment(root: &Path) -> Classification {
    if !env_present(WEBGPU_ENV) {
        return Classification {
            target: Target::Web,
            status: if env_present(REQUIRED_ENV) { "environment_blocker" } else { "expected_skip" },
            code: "webgpu_pin_missing",
            message: format!("{WEBGPU_ENV}=1 is required for pinned WebGPU exact evidence"),
        };
    }
    for command in ["node", "npm", "wasm-bindgen"] {
        if !command_available(command) {
            return Classification {
                target: Target::Web,
                status: if env_present(REQUIRED_ENV) { "environment_blocker" } else { "expected_skip" },
                code: "missing_web_runtime_tool",
                message: format!("required Web runtime command `{command}` is unavailable"),
            };
        }
    }
    if npm_playwright_version(root).is_none() {
        return Classification {
            target: Target::Web,
            status: if env_present(REQUIRED_ENV) { "environment_blocker" } else { "expected_skip" },
            code: "missing_browser_runtime",
            message: String::from("Playwright browser runtime is unavailable; run npm --prefix web install and install the pinned browser channel"),
        };
    }
    Classification {
        target: Target::Web,
        status: "preflight_passed",
        code: "ready",
        message: String::from("web pinned preflight passed; exact WebGPU readback packet still must be validated"),
    }
}

fn write_environment(root: &Path, out_dir: &Path, target: Target) -> Result<(), String> {
    let dir = target.dir(out_dir);
    fs::create_dir_all(&dir).map_err(|error| format!("create {}: {error}", dir.display()))?;
    let classification = classify_environment(root, target);
    let environment = match target {
        Target::Native => native_environment(root, &classification),
        Target::Web => web_environment(root, &classification),
    };
    fs::write(
        dir.join("seq06_13e1_inset_box_shadow.environment.json"),
        serde_json::to_string_pretty(&environment).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| format!("write {} environment: {error}", target.as_str()))
}

fn native_environment(root: &Path, classification: &Classification) -> Value {
    json!({
        "schema": Target::Native.schema(),
        "generated_unix_seconds": unix_seconds(),
        "target": "native",
        "status": classification.status,
        "classification_code": classification.code,
        "message": classification.message.as_str(),
        "environment": {
            "required": env_present(REQUIRED_ENV),
            "pinned": env_present(PINNED_ENV),
            "os": {"family": env::consts::OS, "arch": env::consts::ARCH, "version_family": os_version()},
            "arcweft": {"commit": git_head(root), "dirty": git_dirty(root)},
            "imq": {"available": command_available("imq"), "version": command_stdout(tool_command("imq").arg("--version")), "metrics": METRICS},
            "renderer": {"backend_path": "wgpu_offscreen_compositor", "backend_env": env::var(NATIVE_BACKEND_ENV).ok()},
            "font": {"requested_family": "MS Mincho", "windows_font_file_exists": pinned_font_available()},
            "viewport": {"width": 320, "height": 180, "device_pixel_ratio": 1.0}
        },
        "artifacts": artifact_packet_json(Target::Native),
    })
}

fn web_environment(root: &Path, classification: &Classification) -> Value {
    json!({
        "schema": Target::Web.schema(),
        "generated_unix_seconds": unix_seconds(),
        "target": "web",
        "status": classification.status,
        "classification_code": classification.code,
        "message": classification.message.as_str(),
        "environment": {
            "required": env_present(REQUIRED_ENV),
            "pinned": env_present(PINNED_ENV),
            "os": {"family": env::consts::OS, "arch": env::consts::ARCH, "version_family": os_version()},
            "arcweft": {"commit": git_head(root), "dirty": git_dirty(root)},
            "runtime": {
                "name": "Playwright browser test harness",
                "node_version": command_stdout(tool_command("node").arg("--version")),
                "npm_version": command_stdout(tool_command("npm").arg("--version")),
                "playwright_version": npm_playwright_version(root),
                "wasm_bindgen_version": command_stdout(tool_command("wasm-bindgen").arg("--version")),
            },
            "browser": {"name": null, "version": null, "channel": env::var("ARW_PLAYWRIGHT_CHANNEL").ok().unwrap_or_else(|| "chrome".to_owned())},
            "webgpu": {"required_env": env::var(WEBGPU_ENV).ok(), "adapter_label": null, "backend": null, "driver": null},
            "canvas": {"width": 320, "height": 180, "device_pixel_ratio": 1.0},
            "feature_flags": ["WebGPU enabled", "deterministic Arcweft WGPU texture copy/readback enabled"],
            "imq": {"available": command_available("imq"), "version": command_stdout(tool_command("imq").arg("--version")), "metrics": METRICS}
        },
        "artifacts": artifact_packet_json(Target::Web),
    })
}

fn artifact_packet_json(target: Target) -> Value {
    let prefix = target.as_str();
    json!({
        "candidate_png": format!("target/seq06.13e.1-inset-box-shadow-golden/{prefix}/seq06_13e1_inset_box_shadow.candidate.png"),
        "reference_png": format!("fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow/{prefix}/seq06_13e1_inset_box_shadow.png"),
        "observation_json": format!("target/seq06.13e.1-inset-box-shadow-golden/{prefix}/seq06_13e1_inset_box_shadow.observe.json"),
        "environment_json": format!("target/seq06.13e.1-inset-box-shadow-golden/{prefix}/seq06_13e1_inset_box_shadow.environment.json"),
        "imq_json": format!("target/seq06.13e.1-inset-box-shadow-golden/{prefix}/seq06_13e1_inset_box_shadow.imq.json"),
        "command_logs": format!("target/seq06.13e.1-inset-box-shadow-golden/{prefix}/command-logs/")
    })
}

fn write_review_decision(
    out_dir: &Path,
    classifications: &[Classification],
    run_requested: bool,
) -> Result<(), String> {
    let review_dir = out_dir.join("review");
    fs::create_dir_all(&review_dir).map_err(|error| format!("create {}: {error}", review_dir.display()))?;
    let status = if classifications.iter().any(Classification::is_blocking) {
        "no_promotion"
    } else if run_requested {
        "awaiting_artifact_packet_validation"
    } else {
        "dry_run_preflight"
    };
    let decision = json!({
        "schema": "arcweft.seq06.13e1.inset_box_shadow.promotion_decision.v1",
        "status": status,
        "run_requested": run_requested,
        "promoted": false,
        "classifications": classifications.iter().map(|classification| json!({
            "target": classification.target.as_str(),
            "status": classification.status,
            "code": classification.code,
            "message": classification.message.as_str(),
        })).collect::<Vec<_>>()
    });
    fs::write(
        review_dir.join("seq06_13e1_promotion_decision.json"),
        serde_json::to_string_pretty(&decision).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| format!("write review decision: {error}"))
}

fn run_smoke_and_capture_commands(root: &Path, out_dir: &Path, target: Target) -> Result<(), String> {
    let log_dir = target.dir(out_dir).join("command-logs");
    fs::create_dir_all(&log_dir).map_err(|error| format!("create {}: {error}", log_dir.display()))?;
    match target {
        Target::Native => run_native_commands(root, out_dir, &log_dir),
        Target::Web => run_web_commands(root, out_dir, &log_dir),
    }
}

fn run_native_commands(root: &Path, out_dir: &Path, log_dir: &Path) -> Result<(), String> {
    let capture_output = cargo_command()
        .current_dir(root)
        .args([
            "+nightly",
            "-Zscript",
            "tools/capture-seq06-13e1-inset-shadow-native-frame.rs",
            "--root",
        ])
        .arg(root)
        .arg("--out-dir")
        .arg(out_dir)
        .env(NATIVE_BACKEND_ENV, NATIVE_BACKEND)
        .output()
        .map_err(|error| format!("run native exact PNG capture: {error}"))?;
    write_command_log(
        &log_dir.join("native-exact-png-capture.log"),
        "cargo +nightly -Zscript tools/capture-seq06-13e1-inset-shadow-native-frame.rs --root . --out-dir target/seq06.13e.1-inset-box-shadow-golden",
        &capture_output,
    )?;
    if !capture_output.status.success() {
        return Err(String::from("native exact PNG capture failed; exact PNG packet is not valid"));
    }

    let output = cargo_command()
        .current_dir(root)
        .args([
            "test",
            "-p",
            "arcweft-render-wgpu",
            "--test",
            "ui_box_shadow_gpu_smoke",
            "per_corner_outer_and_elliptical_inset_shadow_cards_execute_gpu_compositor_path",
            "--all-features",
            "--",
            "--ignored",
            "--exact",
            "--nocapture",
        ])
        .output()
        .map_err(|error| format!("run native compositor smoke: {error}"))?;
    write_command_log(&log_dir.join("native-compositor-smoke.log"), "cargo test -p arcweft-render-wgpu --test ui_box_shadow_gpu_smoke per_corner_outer_and_elliptical_inset_shadow_cards_execute_gpu_compositor_path --all-features -- --ignored --exact --nocapture", &output)?;
    if !output.status.success() {
        return Err(String::from("native compositor smoke failed; exact PNG capture is not valid"));
    }
    Ok(())
}

fn run_web_commands(root: &Path, out_dir: &Path, log_dir: &Path) -> Result<(), String> {
    let build_output = cargo_command()
        .current_dir(root)
        .args(["build", "-p", "arcweft-player-web", "--target", "wasm32-unknown-unknown"])
        .output()
        .map_err(|error| format!("build web wasm player: {error}"))?;
    write_command_log(&log_dir.join("web-wasm-cargo-build.log"), "cargo build -p arcweft-player-web --target wasm32-unknown-unknown", &build_output)?;
    if !build_output.status.success() {
        return Err(String::from("web wasm player build failed; exact PNG packet is not valid"));
    }

    let bindgen_output = tool_command("wasm-bindgen")
        .current_dir(root)
        .args(["--target", "web", "--out-dir", "web/pkg", "--out-name", "arcweft_player_web"])
        .arg(root.join("target/wasm32-unknown-unknown/debug/arcweft_player_web.wasm"))
        .output()
        .map_err(|error| format!("run wasm-bindgen for web player: {error}"))?;
    write_command_log(&log_dir.join("web-wasm-bindgen.log"), "wasm-bindgen --target web --out-dir web/pkg --out-name arcweft_player_web target/wasm32-unknown-unknown/debug/arcweft_player_web.wasm", &bindgen_output)?;
    if !bindgen_output.status.success() {
        return Err(String::from("web wasm-bindgen failed; exact PNG packet is not valid"));
    }

    let capture_output = tool_command("node")
        .current_dir(root)
        .arg("web/tests/seq06-13e1-inset-shadow-exact-capture.mjs")
        .arg("--root")
        .arg(display_path(root))
        .arg("--out-dir")
        .arg(display_path(out_dir))
        .env(WEBGPU_ENV, "1")
        .output()
        .map_err(|error| format!("run web exact PNG capture: {error}"))?;
    write_command_log(&log_dir.join("web-exact-png-capture.log"), "node web/tests/seq06-13e1-inset-shadow-exact-capture.mjs --root . --out-dir target/seq06.13e.1-inset-box-shadow-golden", &capture_output)?;
    if !capture_output.status.success() {
        return Err(format!(
            "web exact PNG capture failed; classification={}; exact PNG packet is not valid",
            web_capture_failure_code(&capture_output)
        ));
    }

    let smoke_output = tool_command("npm")
        .current_dir(root.join("web"))
        .args(["test"])
        .env(WEBGPU_ENV, "1")
        .output()
        .map_err(|error| format!("run web smoke: {error}"))?;
    write_command_log(&log_dir.join("webgpu-smoke.log"), "npm --prefix web test", &smoke_output)?;
    if !smoke_output.status.success() {
        return Err(String::from("webgpu smoke failed; exact PNG capture is not valid"));
    }
    Ok(())
}

fn web_capture_failure_code(output: &Output) -> &'static str {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if text.contains("fully transparent seq06.13e.1 candidate") {
        "transparent_candidate"
    } else if text.contains("navigator.gpu is unavailable") || text.contains("WebGPU") {
        "missing_webgpu"
    } else if text.contains("Executable doesn't exist")
        || text.contains("browserType.launch")
        || text.contains("Playwright")
    {
        "missing_browser_runtime"
    } else {
        "web_capture_failed"
    }
}

fn validate_existing_artifact_packet(root: &Path, out_dir: &Path, target: Target) -> Result<(), String> {
    let dir = target.dir(out_dir);
    let candidate = dir.join("seq06_13e1_inset_box_shadow.candidate.png");
    let observe = dir.join("seq06_13e1_inset_box_shadow.observe.json");
    let metrics = dir.join("seq06_13e1_inset_box_shadow.imq.json");
    let environment = dir.join("seq06_13e1_inset_box_shadow.environment.json");
    let reference = root
        .join("fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow")
        .join(target.as_str())
        .join("seq06_13e1_inset_box_shadow.png");
    let paths = PacketPaths { candidate: &candidate, reference: &reference, observe: &observe, metrics: &metrics, environment: &environment };

    let mut required = vec![&candidate, &observe, &environment];
    let web_capture_log = dir.join("command-logs/web-exact-png-capture.log");
    let native_capture_log = dir.join("command-logs/native-exact-png-capture.log");
    match target {
        Target::Native => required.push(&native_capture_log),
        Target::Web => required.push(&web_capture_log),
    }
    let missing = required
        .iter()
        .filter(|path| !path.exists())
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        let missing_code = if !candidate.exists() {
            "missing_candidate_png"
        } else {
            "missing_packet_artifact"
        };
        write_packet_review_decision(
            root,
            out_dir,
            target,
            "hard_visual_regression",
            &format!("{missing_code}: required packet artifacts are missing: {}", missing.join(", ")),
            &paths,
            None,
            None,
            None,
        )?;
        return Err(format!("{} pinned packet is incomplete; {missing_code}; missing: {}", target.as_str(), missing.join(", ")));
    }

    validate_route_evidence(&observe)?;
    let candidate_dimensions = png_dimensions(&candidate)?;
    if candidate_dimensions.width != 320 || candidate_dimensions.height != 180 {
        write_packet_review_decision(root, out_dir, target, "hard_visual_regression", "candidate PNG dimensions are not 320x180", &paths, Some(candidate_dimensions), None, None)?;
        return Err(format!("{} exact PNG candidate dimensions are {}x{}; expected 320x180", target.as_str(), candidate_dimensions.width, candidate_dimensions.height));
    }

    if !reference.exists() {
        write_baseline_missing_metrics(root, target, &paths, candidate_dimensions)?;
        write_packet_review_decision(root, out_dir, target, "ready_for_first_promotion_review", "candidate, observation, environment, command logs, and baseline-missing metrics are present; checked-in reference PNG is still absent", &paths, Some(candidate_dimensions), None, None)?;
        println!("{} exact PNG packet is ready for first-promotion review; reference baseline is absent", target.as_str());
        return Ok(());
    }

    let reference_dimensions = png_dimensions(&reference)?;
    if candidate_dimensions != reference_dimensions {
        write_packet_review_decision(root, out_dir, target, "hard_visual_regression", "candidate and reference PNG dimensions differ", &paths, Some(candidate_dimensions), Some(reference_dimensions), None)?;
        return Err(format!(
            "{} exact PNG dimensions differ: candidate={}x{}, reference={}x{}",
            target.as_str(),
            candidate_dimensions.width,
            candidate_dimensions.height,
            reference_dimensions.width,
            reference_dimensions.height
        ));
    }

    let imq_output = Command::new("imq")
        .arg("image")
        .arg(&reference)
        .arg(&candidate)
        .arg("--metrics")
        .arg(METRICS)
        .arg("--format")
        .arg("json")
        .output()
        .map_err(|error| format!("run imq for {} packet: {error}", target.as_str()))?;
    fs::write(&metrics, &imq_output.stdout).map_err(|error| format!("write {}: {error}", metrics.display()))?;
    if !imq_output.status.success() {
        write_packet_review_decision(root, out_dir, target, "hard_visual_regression", "imq comparison failed", &paths, Some(candidate_dimensions), Some(reference_dimensions), None)?;
        return Err(format!("{} imq comparison failed; metrics={}, stderr={}", target.as_str(), metrics.display(), String::from_utf8_lossy(&imq_output.stderr)));
    }

    let imq_json: Value = serde_json::from_slice(&imq_output.stdout).map_err(|error| format!("parse {} imq JSON: {error}", metrics.display()))?;
    let mse = metric_score(&imq_json, "mse")?;
    let mae = metric_score(&imq_json, "mae")?;
    let metric_summary = MetricSummary { mse, mae };
    if mse <= MAX_MSE && mae <= MAX_MAE {
        write_packet_review_decision(root, out_dir, target, "passed_existing_baseline_gate", "candidate matches the existing checked-in baseline within seq06.13e.1 thresholds", &paths, Some(candidate_dimensions), Some(reference_dimensions), Some(metric_summary))?;
        Ok(())
    } else {
        write_packet_review_decision(root, out_dir, target, "baseline_drift", "candidate exceeds seq06.13e.1 MSE/MAE thresholds against the existing checked-in baseline", &paths, Some(candidate_dimensions), Some(reference_dimensions), Some(metric_summary))?;
        Err(format!("{} exact PNG baseline drift: mse={mse}, mae={mae}, max_mse={MAX_MSE}, max_mae={MAX_MAE}", target.as_str()))
    }
}

fn validate_route_evidence(observe: &Path) -> Result<(), String> {
    let text = fs::read_to_string(observe).map_err(|error| format!("read {}: {error}", observe.display()))?;
    for required in [
        "UiCompositingEffects::box_shadows",
        "UiBoxShadowPassPlan",
        "UiCompositor::render_group",
        "PASS_BOX_SHADOW",
        "inset",
    ] {
        if !text.contains(required) {
            return Err(format!("{} is missing required route evidence `{required}`", observe.display()));
        }
    }
    for forbidden in ["browser DOM CSS box-shadow screenshots", "SVG filters", "canvas 2D fallback", "CPU raster fallback"] {
        if text.contains(forbidden) {
            return Err(format!("{} contains forbidden fallback evidence `{forbidden}`", observe.display()));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct PacketPaths<'a> {
    candidate: &'a Path,
    reference: &'a Path,
    observe: &'a Path,
    metrics: &'a Path,
    environment: &'a Path,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PngDimensions {
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, Debug)]
struct MetricSummary {
    mse: f64,
    mae: f64,
}

fn write_baseline_missing_metrics(
    root: &Path,
    target: Target,
    paths: &PacketPaths<'_>,
    candidate_dimensions: PngDimensions,
) -> Result<(), String> {
    let report = json!({
        "schema": "arcweft.seq06.13e1.inset_box_shadow.imq_report.v1",
        "status": "baseline_missing",
        "target": target.as_str(),
        "reason": "checked-in reference PNG is absent; imq comparison is deferred until first-promotion review copies the candidate into the reference path",
        "metric_set": ["psnr", "ssim", "mse", "mae", "maxae"],
        "metrics": null,
        "thresholds": {"max_mse": MAX_MSE, "max_mae": MAX_MAE},
        "candidate_dimensions": {"width": candidate_dimensions.width, "height": candidate_dimensions.height},
        "artifacts": {"candidate": artifact_review_json(root, paths.candidate), "reference": artifact_review_json(root, paths.reference)},
    });
    fs::write(paths.metrics, serde_json::to_string_pretty(&report).map_err(|error| error.to_string())? + "\n")
        .map_err(|error| format!("write {}: {error}", paths.metrics.display()))
}

fn write_packet_review_decision(
    root: &Path,
    out_dir: &Path,
    target: Target,
    status: &str,
    reason: &str,
    paths: &PacketPaths<'_>,
    candidate_dimensions: Option<PngDimensions>,
    reference_dimensions: Option<PngDimensions>,
    metrics: Option<MetricSummary>,
) -> Result<(), String> {
    let review_dir = out_dir.join("review");
    fs::create_dir_all(&review_dir).map_err(|error| format!("create {}: {error}", review_dir.display()))?;
    let decision = json!({
        "schema": "arcweft.seq06.13e1.inset_box_shadow.promotion_decision.v1",
        "target": target.as_str(),
        "status": status,
        "reason": reason,
        "promoted": false,
        "thresholds": {"max_mse": MAX_MSE, "max_mae": MAX_MAE},
        "metrics": metrics.map(|summary| json!({"mse": summary.mse, "mae": summary.mae, "passed": summary.mse <= MAX_MSE && summary.mae <= MAX_MAE})),
        "dimensions": {
            "candidate": candidate_dimensions.map(|dimensions| json!({"width": dimensions.width, "height": dimensions.height})),
            "reference": reference_dimensions.map(|dimensions| json!({"width": dimensions.width, "height": dimensions.height})),
        },
        "source_hashes": {
            "policy_git": git_hash_object_path(root, &root.join("fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow-exact-png-policy.json")),
            "fixture_doc_git": git_hash_object_path(root, &fixture_doc_path(root, target)),
            "source_css_git": git_hash_object_path(root, &root.join("docs/fixtures/css/seq06.13e-inset-box-shadow-card.css")),
        },
        "artifacts": {
            "candidate": artifact_review_json(root, paths.candidate),
            "reference": artifact_review_json(root, paths.reference),
            "observe_json": artifact_review_json(root, paths.observe),
            "metrics_json": artifact_review_json(root, paths.metrics),
            "environment_json": artifact_review_json(root, paths.environment),
        },
    });
    fs::write(
        review_dir.join(format!("seq06_13e1_{}_promotion_decision.json", target.as_str())),
        serde_json::to_string_pretty(&decision).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| format!("write packet review decision: {error}"))
}

fn fixture_doc_path(root: &Path, target: Target) -> PathBuf {
    root.join("docs/fixtures").join(target.as_str()).join("seq06_13e1_inset_box_shadow_exact_golden.json")
}

fn artifact_review_json(root: &Path, path: &Path) -> Value {
    json!({
        "path": display_path(path),
        "exists": path.exists(),
        "sha256": sha256_file(path).ok(),
        "git_hash_object": git_hash_object_path(root, path),
    })
}

fn png_dimensions(path: &Path) -> Result<PngDimensions, String> {
    let bytes = fs::read(path).map_err(|error| format!("read PNG {}: {error}", path.display()))?;
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err(format!("{} is not a PNG with an IHDR chunk", path.display()));
    }
    Ok(PngDimensions {
        width: u32::from_be_bytes(bytes[16..20].try_into().expect("PNG width bytes")),
        height: u32::from_be_bytes(bytes[20..24].try_into().expect("PNG height bytes")),
    })
}

fn metric_score(report: &Value, metric_name: &str) -> Result<f64, String> {
    report["metrics"]
        .as_array()
        .and_then(|metrics| metrics.iter().find(|metric| metric["name"].as_str() == Some(metric_name)))
        .and_then(|metric| metric["score"].as_f64())
        .ok_or_else(|| format!("{metric_name} score should be present in imq JSON: {report}"))
}

fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn git_hash_object_path(root: &Path, path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    command_stdout(Command::new("git").arg("-C").arg(display_path(root)).arg("hash-object").arg(display_path(path)))
}

fn write_command_log(path: &Path, command: &str, output: &Output) -> Result<(), String> {
    let mut log = String::new();
    log.push_str(&format!("# command: {command}\n"));
    log.push_str(&format!("# exit: {}\n\n## stdout\n\n", exit_code_text(output)));
    log.push_str(&String::from_utf8_lossy(&output.stdout));
    log.push_str("\n## stderr\n\n");
    log.push_str(&String::from_utf8_lossy(&output.stderr));
    fs::write(path, log).map_err(|error| format!("write {}: {error}", path.display()))
}

fn os_version() -> Option<String> {
    if cfg!(windows) {
        command_stdout(Command::new("cmd").arg("/C").arg("ver"))
    } else if cfg!(target_os = "macos") {
        command_stdout(Command::new("sw_vers").arg("-productVersion"))
    } else {
        command_stdout(Command::new("uname").arg("-srv"))
    }
}

fn npm_playwright_version(root: &Path) -> Option<String> {
    command_stdout(
        tool_command("npm")
            .arg("--prefix")
            .arg(display_path(&root.join("web")))
            .arg("exec")
            .arg("--")
            .arg("playwright")
            .arg("--version"),
    )
}

fn command_available(command: &str) -> bool {
    tool_command(command)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn tool_command(command: &str) -> Command {
    Command::new(tool_program(command))
}

fn cargo_command() -> Command {
    let mut command = tool_command("cargo");
    command.env_remove("RUSTUP_TOOLCHAIN");
    command
}

fn tool_program(command: &str) -> String {
    if cfg!(windows) {
        match command {
            "cargo" => String::from("cargo.exe"),
            "node" => String::from("node.exe"),
            "npm" => String::from("npm.cmd"),
            "wasm-bindgen" => String::from("wasm-bindgen.exe"),
            "imq" => String::from("imq.exe"),
            other => other.to_owned(),
        }
    } else {
        command.to_owned()
    }
}

fn command_stdout(command: &mut Command) -> Option<String> {
    let output = command.stderr(Stdio::null()).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}

fn git_head(root: &Path) -> Option<String> {
    command_stdout(Command::new("git").arg("-C").arg(display_path(root)).arg("rev-parse").arg("HEAD"))
}

fn git_dirty(root: &Path) -> Option<bool> {
    command_stdout(Command::new("git").arg("-C").arg(display_path(root)).arg("status").arg("--short")).map(|status| !status.is_empty())
}

fn env_present(name: &str) -> bool {
    env::var_os(name).is_some()
}

#[cfg(windows)]
fn pinned_font_available() -> bool {
    env::var_os("WINDIR")
        .map(|windir| PathBuf::from(windir).join("Fonts").join("msmincho.ttc"))
        .is_some_and(|path| path.exists())
}

#[cfg(not(windows))]
fn pinned_font_available() -> bool {
    false
}

fn exit_code_text(output: &Output) -> String {
    output.status.code().map_or_else(|| String::from("terminated-by-signal"), |code| code.to_string())
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn display_path(path: &Path) -> String {
    let text = path.display().to_string();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned()
}
