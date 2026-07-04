#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "collect-seq06-13e1-inset-shadow-pinned-golden-evidence"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
publish = false
---

/*
Collects seq06.13e.1 pinned inset box-shadow exact PNG evidence.

The script writes evidence under --out-dir and never copies candidates into
checked-in baseline paths. Baseline promotion remains a documented review action.

Default mode is a dry preflight. Use --run only in the pinned visual-golden job.

Example:

cargo +nightly -Zscript tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs \
  --root . \
  --out-dir target/seq06.13e.1-inset-box-shadow-golden \
  --mode both \
  --run
*/

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const REQUIRED_ENV: &str = "ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_REQUIRED";
const PINNED_ENV: &str = "ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED";
const NATIVE_BACKEND_ENV: &str = "ARW_SEQ06_13E1_INSET_SHADOW_NATIVE_BACKEND";
const WEBGPU_ENV: &str = "ARW_SEQ06_13E1_INSET_SHADOW_WEBGPU";
const NATIVE_BACKEND: &str = "wgpu_offscreen_compositor";
const METRICS: &str = "psnr,ssim,mse,mae,maxae";

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
    for &mode in args.mode.targets() {
        write_environment(&root, &out_dir, mode)?;
        classifications.push(classify_environment(mode));
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

    for &mode in args.mode.targets() {
        run_smoke_and_capture_commands(&root, &out_dir, mode)?;
        validate_existing_artifact_packet(&root, &out_dir, mode)?;
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
            root: if help {
                root.unwrap_or_else(|| PathBuf::from("."))
            } else {
                root.ok_or_else(|| String::from("missing --root"))?
            },
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
    values
        .next()
        .ok_or_else(|| format!("{name} requires a value"))
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

fn classify_environment(target: Target) -> Classification {
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
        Target::Web => classify_web_environment(),
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

fn classify_web_environment() -> Classification {
    if !env_present(WEBGPU_ENV) {
        return Classification {
            target: Target::Web,
            status: if env_present(REQUIRED_ENV) { "environment_blocker" } else { "expected_skip" },
            code: "webgpu_pin_missing",
            message: format!("{WEBGPU_ENV}=1 is required for pinned WebGPU exact evidence"),
        };
    }
    for command in ["node", "npm"] {
        if !command_available(command) {
            return Classification {
                target: Target::Web,
                status: if env_present(REQUIRED_ENV) { "environment_blocker" } else { "expected_skip" },
                code: "missing_web_runtime_tool",
                message: format!("required Web runtime command `{command}` is unavailable"),
            };
        }
    }
    Classification {
        target: Target::Web,
        status: "preflight_passed",
        code: "ready",
        message: String::from("web pinned preflight passed; artifact packet still must be validated"),
    }
}

fn write_environment(root: &Path, out_dir: &Path, target: Target) -> Result<(), String> {
    let dir = target.dir(out_dir);
    fs::create_dir_all(&dir).map_err(|error| format!("create {}: {error}", dir.display()))?;
    let path = dir.join("seq06_13e1_inset_box_shadow.environment.json");
    let classification = classify_environment(target);
    let mut json = String::new();
    writeln!(&mut json, "{{").unwrap();
    writeln!(&mut json, "  \"schema\": {},", json_string(target.schema())).unwrap();
    writeln!(&mut json, "  \"generated_unix_seconds\": {},", unix_seconds()).unwrap();
    writeln!(&mut json, "  \"target\": {},", json_string(target.as_str())).unwrap();
    writeln!(&mut json, "  \"status\": {},", json_string(classification.status)).unwrap();
    writeln!(&mut json, "  \"classification_code\": {},", json_string(classification.code)).unwrap();
    writeln!(&mut json, "  \"message\": {},", json_string(&classification.message)).unwrap();
    write_env_object(&mut json, root, target)?;
    writeln!(&mut json, "}}").unwrap();
    fs::write(&path, json).map_err(|error| format!("write {}: {error}", path.display()))
}

fn write_env_object(json: &mut String, root: &Path, target: Target) -> Result<(), String> {
    writeln!(json, "  \"environment\": {{").unwrap();
    writeln!(json, "    \"required\": {},", env_present(REQUIRED_ENV)).unwrap();
    writeln!(json, "    \"pinned\": {},", env_present(PINNED_ENV)).unwrap();
    writeln!(json, "    \"os\": {{").unwrap();
    writeln!(json, "      \"family\": {},", json_string(env::consts::OS)).unwrap();
    writeln!(json, "      \"arch\": {},", json_string(env::consts::ARCH)).unwrap();
    writeln!(json, "      \"version_family\": {}", json_option(os_version().as_deref())).unwrap();
    writeln!(json, "    }},").unwrap();
    writeln!(json, "    \"arcweft\": {{").unwrap();
    writeln!(json, "      \"commit\": {},", json_option(command_stdout(Command::new("git").arg("-C").arg(root).arg("rev-parse").arg("HEAD")).as_deref())).unwrap();
    writeln!(json, "      \"dirty\": {}", command_stdout(Command::new("git").arg("-C").arg(root).arg("status").arg("--short")).map_or(String::from("null"), |status| (!status.is_empty()).to_string())).unwrap();
    writeln!(json, "    }},").unwrap();
    writeln!(json, "    \"imq\": {{").unwrap();
    writeln!(json, "      \"available\": {},", command_available("imq")).unwrap();
    writeln!(json, "      \"version\": {},", json_option(command_stdout(Command::new("imq").arg("--version")).as_deref())).unwrap();
    writeln!(json, "      \"metrics\": {}", json_string(METRICS)).unwrap();
    writeln!(json, "    }},").unwrap();
    match target {
        Target::Native => write_native_env(json),
        Target::Web => write_web_env(json),
    }
}

fn write_native_env(json: &mut String) -> Result<(), String> {
    writeln!(json, "    \"renderer\": {{").unwrap();
    writeln!(json, "      \"backend_path\": \"wgpu_offscreen_compositor\",").unwrap();
    writeln!(json, "      \"backend_env\": {}", json_option(env::var(NATIVE_BACKEND_ENV).ok().as_deref())).unwrap();
    writeln!(json, "    }},").unwrap();
    writeln!(json, "    \"font\": {{").unwrap();
    writeln!(json, "      \"requested_family\": \"MS Mincho\",").unwrap();
    writeln!(json, "      \"windows_font_file_exists\": {}", pinned_font_available()).unwrap();
    writeln!(json, "    }},").unwrap();
    writeln!(json, "    \"viewport\": {{\"width\": 320, \"height\": 180, \"device_pixel_ratio\": 1.0}}").unwrap();
    writeln!(json, "  }},").unwrap();
    writeln!(json, "  \"artifacts\": {}", artifact_packet_json(Target::Native)).unwrap();
    Ok(())
}

fn write_web_env(json: &mut String) -> Result<(), String> {
    writeln!(json, "    \"browser\": {{").unwrap();
    writeln!(json, "      \"runtime\": \"Playwright/WebGPU pinned browser harness\",").unwrap();
    writeln!(json, "      \"node_version\": {},", json_option(command_stdout(Command::new("node").arg("--version")).as_deref())).unwrap();
    writeln!(json, "      \"npm_version\": {}", json_option(command_stdout(Command::new("npm").arg("--version")).as_deref())).unwrap();
    writeln!(json, "    }},").unwrap();
    writeln!(json, "    \"webgpu\": {{").unwrap();
    writeln!(json, "      \"required_env\": {},", json_option(env::var(WEBGPU_ENV).ok().as_deref())).unwrap();
    writeln!(json, "      \"adapter_label\": null,").unwrap();
    writeln!(json, "      \"backend\": null,").unwrap();
    writeln!(json, "      \"driver\": null").unwrap();
    writeln!(json, "    }},").unwrap();
    writeln!(json, "    \"canvas\": {{\"width\": 320, \"height\": 180, \"device_pixel_ratio\": 1.0}}").unwrap();
    writeln!(json, "  }},").unwrap();
    writeln!(json, "  \"artifacts\": {}", artifact_packet_json(Target::Web)).unwrap();
    Ok(())
}

fn artifact_packet_json(target: Target) -> String {
    let prefix = target.as_str();
    format!(
        "{{\n    \"candidate_png\": \"target/seq06.13e.1-inset-box-shadow-golden/{prefix}/seq06_13e1_inset_box_shadow.candidate.png\",\n    \"reference_png\": \"fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow/{prefix}/seq06_13e1_inset_box_shadow.png\",\n    \"observation_json\": \"target/seq06.13e.1-inset-box-shadow-golden/{prefix}/seq06_13e1_inset_box_shadow.observe.json\",\n    \"imq_json\": \"target/seq06.13e.1-inset-box-shadow-golden/{prefix}/seq06_13e1_inset_box_shadow.imq.json\"\n  }}"
    )
}

fn write_review_decision(
    out_dir: &Path,
    classifications: &[Classification],
    run_requested: bool,
) -> Result<(), String> {
    let review_dir = out_dir.join("review");
    fs::create_dir_all(&review_dir).map_err(|error| format!("create {}: {error}", review_dir.display()))?;
    let path = review_dir.join("seq06_13e1_promotion_decision.json");
    let status = if classifications.iter().any(Classification::is_blocking) {
        "no_promotion"
    } else if run_requested {
        "awaiting_artifact_packet_validation"
    } else {
        "dry_run_preflight"
    };
    let mut json = String::new();
    writeln!(&mut json, "{{").unwrap();
    writeln!(&mut json, "  \"schema\": \"arcweft.seq06.13e1.inset_box_shadow.promotion_decision.v1\",").unwrap();
    writeln!(&mut json, "  \"status\": {},", json_string(status)).unwrap();
    writeln!(&mut json, "  \"run_requested\": {},", run_requested).unwrap();
    writeln!(&mut json, "  \"promoted\": false,").unwrap();
    writeln!(&mut json, "  \"classifications\": [").unwrap();
    for (index, classification) in classifications.iter().enumerate() {
        let comma = if index + 1 == classifications.len() { "" } else { "," };
        writeln!(&mut json, "    {{\"target\": {}, \"status\": {}, \"code\": {}, \"message\": {}}}{comma}",
            json_string(classification.target.as_str()),
            json_string(classification.status),
            json_string(classification.code),
            json_string(&classification.message)).unwrap();
    }
    writeln!(&mut json, "  ]").unwrap();
    writeln!(&mut json, "}}").unwrap();
    fs::write(&path, json).map_err(|error| format!("write {}: {error}", path.display()))
}

fn run_smoke_and_capture_commands(root: &Path, out_dir: &Path, target: Target) -> Result<(), String> {
    let log_dir = target.dir(out_dir).join("command-logs");
    fs::create_dir_all(&log_dir).map_err(|error| format!("create {}: {error}", log_dir.display()))?;
    match target {
        Target::Native => {
            let output = Command::new("cargo")
                .current_dir(root)
                .args([
                    "test",
                    "-p",
                    "arcweft-render-wgpu",
                    "--test",
                    "ui_box_shadow_gpu_smoke",
                    "rounded_inset_and_mixed_shadow_cards_execute_gpu_compositor_path",
                    "--all-features",
                    "--",
                    "--ignored",
                    "--exact",
                    "--nocapture",
                ])
                .output()
                .map_err(|error| format!("run native compositor smoke: {error}"))?;
            write_command_log(&log_dir.join("native-compositor-smoke.log"), "cargo test -p arcweft-render-wgpu --test ui_box_shadow_gpu_smoke rounded_inset_and_mixed_shadow_cards_execute_gpu_compositor_path --all-features -- --ignored --exact --nocapture", &output)?;
            if !output.status.success() {
                return Err(String::from("native compositor smoke failed; exact PNG capture is not valid"));
            }
        }
        Target::Web => {
            let output = Command::new("npm")
                .current_dir(root.join("web"))
                .args(["test"])
                .env(WEBGPU_ENV, "1")
                .output()
                .map_err(|error| format!("run web smoke: {error}"))?;
            write_command_log(&log_dir.join("webgpu-smoke.log"), "npm --prefix web test", &output)?;
            if !output.status.success() {
                return Err(String::from("webgpu smoke failed; exact PNG capture is not valid"));
            }
        }
    }
    Ok(())
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
    let required = [&candidate, &observe, &environment, &reference];
    let missing = required
        .iter()
        .filter(|path| !path.exists())
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "{} pinned packet is incomplete or baseline_missing; missing: {}",
            target.as_str(),
            missing.join(", ")
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
    fs::write(&metrics, &imq_output.stdout)
        .map_err(|error| format!("write {}: {error}", metrics.display()))?;
    if imq_output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} imq comparison failed; metrics={}, stderr={}",
            target.as_str(),
            metrics.display(),
            String::from_utf8_lossy(&imq_output.stderr)
        ))
    }
}

fn write_command_log(path: &Path, command: &str, output: &Output) -> Result<(), String> {
    let mut log = String::new();
    writeln!(&mut log, "# command: {command}").unwrap();
    writeln!(&mut log, "# exit: {}", exit_code_text(output)).unwrap();
    writeln!(&mut log, "\n## stdout\n").unwrap();
    log.push_str(&String::from_utf8_lossy(&output.stdout));
    writeln!(&mut log, "\n## stderr\n").unwrap();
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

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn command_stdout(command: &mut Command) -> Option<String> {
    let output = command.stderr(Stdio::null()).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
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
    output
        .status
        .code()
        .map_or_else(|| String::from("terminated-by-signal"), |code| code.to_string())
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn json_string(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn json_option(value: Option<&str>) -> String {
    value.map_or_else(|| String::from("null"), json_string)
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => write!(&mut escaped, "\\u{:04x}", c as u32).unwrap(),
            c => escaped.push(c),
        }
    }
    escaped
}
