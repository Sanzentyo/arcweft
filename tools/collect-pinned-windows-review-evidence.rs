#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "collect-pinned-windows-review-evidence"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
publish = false

[dependencies]
sha2 = "0.10.9"
---

/*
Collects the seq06.7.1 exact native golden baseline-promotion review packet.

This tool intentionally refuses to collect evidence outside Windows. Exact native
goldens depend on the pinned Windows native text path, MS Mincho, imq, and the
seq06.7 environment variables. The first command may fail with baseline_drift;
that is still review input. The artifact command must succeed.

```powershell
cargo +nightly -Zscript tools/collect-pinned-windows-review-evidence.rs `
  --root . `
  --out-dir seq06.7.1-pinned-review-evidence
```
*/

use sha2::{Digest, Sha256};
use std::env;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const PINNED_REQUIRED: &str = "1";
const PINNED_BACKEND: &str = "native_rich_text_observer";
const REQUIRED_ARTIFACTS: &[&str] = &[
    "vertical_tutr_golden.candidate.png",
    "vertical_tutr_golden.observe.json",
    "vertical_tutr_golden.imq.json",
    "exact-native-golden.environment.json",
];

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

    ensure_windows()?;
    let root = args
        .root
        .canonicalize()
        .map_err(|error| format!("canonicalize {}: {error}", args.root.display()))?;
    let out_dir = if args.out_dir.is_absolute() {
        args.out_dir
    } else {
        root.join(args.out_dir)
    };

    ensure_command_available("just")?;
    ensure_command_available("imq")?;
    ensure_pinned_font()?;

    if args.check_env_only {
        println!("seq06.7.1 pinned Windows review environment is available");
        return Ok(());
    }

    collect_review_packet(&root, &out_dir)
}

#[cfg(windows)]
fn ensure_windows() -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
fn ensure_windows() -> Result<(), String> {
    Err(String::from(
        "seq06.7.1 exact native golden review evidence must be collected on Windows",
    ))
}

fn print_help() {
    println!(
        "collect-pinned-windows-review-evidence\n\n\
         Usage:\n  cargo +nightly -Zscript tools/collect-pinned-windows-review-evidence.rs --root . [--out-dir DIR]\n\n\
         Options:\n  --root <repo-root>      Repository root to run just from.\n  --out-dir <dir>        Output directory, relative to root unless absolute.\n  --check-env-only       Validate Windows, just, imq, and MS Mincho without running captures.\n  -h, --help             Print this help.\n"
    );
}

#[derive(Debug)]
struct Args {
    root: PathBuf,
    out_dir: PathBuf,
    check_env_only: bool,
    help: bool,
}

impl Args {
    fn parse(values: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut root = None;
        let mut out_dir = PathBuf::from("seq06.7.1-pinned-review-evidence");
        let mut check_env_only = false;
        let mut help = false;
        let mut values = values.peekable();

        while let Some(arg) = values.next() {
            match arg.as_str() {
                "--root" => root = Some(PathBuf::from(next_arg(&mut values, "--root")?)),
                "--out-dir" => out_dir = PathBuf::from(next_arg(&mut values, "--out-dir")?),
                "--check-env-only" => check_env_only = true,
                "--help" | "-h" => help = true,
                _ => return Err(format!("unknown argument `{arg}`")),
            }
        }

        Ok(Self {
            root: if help {
                root.unwrap_or_else(|| PathBuf::from("."))
            } else {
                root.ok_or_else(|| String::from("missing --root"))?
            },
            out_dir,
            check_env_only,
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

fn collect_review_packet(root: &Path, out_dir: &Path) -> Result<(), String> {
    let command_logs = out_dir.join("command-logs");
    let target_artifacts = out_dir.join("target-artifacts");
    fs::create_dir_all(&command_logs)
        .map_err(|error| format!("create {}: {error}", command_logs.display()))?;
    fs::create_dir_all(&target_artifacts)
        .map_err(|error| format!("create {}: {error}", target_artifacts.display()))?;

    let started_unix_seconds = unix_seconds();
    let readme = out_dir.join("README.md");
    fs::write(
        &readme,
        format!(
            "# seq06.7.1 pinned Windows evidence collection\n\nStarted unix seconds: {started_unix_seconds}\n"
        ),
    )
    .map_err(|error| format!("write {}: {error}", readme.display()))?;

    let test_visual = run_just(root, ["test-visual-golden"])?;
    write_command_log(
        &command_logs.join("just-test-visual-golden.log"),
        "just test-visual-golden",
        &test_visual,
    )?;

    if !test_visual.status.success() {
        append_file(
            &command_logs.join("just-test-visual-golden.log"),
            "\njust test-visual-golden failed; baseline_drift can be review input, environment_blocker cannot be promotion evidence.\n",
        )?;
    }

    let native_artifacts = run_just(root, ["native-visual-artifacts"])?;
    write_command_log(
        &command_logs.join("just-native-visual-artifacts.log"),
        "just native-visual-artifacts",
        &native_artifacts,
    )?;
    if !native_artifacts.status.success() {
        return Err(format!(
            "just native-visual-artifacts failed with exit code {}",
            exit_code_text(&native_artifacts)
        ));
    }

    let artifact_root = root.join("target/arcweft-native-capture-artifacts");
    for artifact in REQUIRED_ARTIFACTS {
        let source = artifact_root.join(artifact);
        let destination = target_artifacts.join(artifact);
        if !source.exists() {
            return Err(format!(
                "required exact native golden artifact is missing: {}",
                source.display()
            ));
        }
        fs::copy(&source, &destination).map_err(|error| {
            format!(
                "copy {} to {}: {error}",
                source.display(),
                destination.display()
            )
        })?;
    }

    write_sha256sums(
        &target_artifacts,
        REQUIRED_ARTIFACTS,
        &out_dir.join("SHA256SUMS.windows-artifacts.txt"),
    )?;
    append_file(
        &readme,
        &format!(
            "Completed unix seconds: {}\ntest_visual_exit={}\nnative_artifacts_exit={}\n",
            unix_seconds(),
            exit_code_text(&test_visual),
            exit_code_text(&native_artifacts)
        ),
    )?;
    println!("wrote {}", out_dir.display());
    Ok(())
}

fn run_just<const N: usize>(root: &Path, args: [&str; N]) -> Result<Output, String> {
    Command::new("just")
        .current_dir(root)
        .args(args)
        .env("ARW_EXACT_NATIVE_GOLDEN_REQUIRED", PINNED_REQUIRED)
        .env("ARW_EXACT_NATIVE_GOLDEN_PINNED", PINNED_REQUIRED)
        .env("ARW_EXACT_NATIVE_GOLDEN_BACKEND", PINNED_BACKEND)
        .output()
        .map_err(|error| format!("run just: {error}"))
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

fn append_file(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;

    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    file.write_all(text.as_bytes())
        .map_err(|error| format!("append {}: {error}", path.display()))
}

fn write_sha256sums(path: &Path, artifacts: &[&str], out: &Path) -> Result<(), String> {
    let mut text = String::new();
    for artifact in artifacts {
        let artifact_path = path.join(artifact);
        let digest = sha256_file(&artifact_path)
            .map_err(|error| format!("hash {}: {error}", artifact_path.display()))?;
        writeln!(&mut text, "{digest}  target-artifacts/{artifact}").unwrap();
    }
    fs::write(out, text).map_err(|error| format!("write {}: {error}", out.display()))
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

fn ensure_command_available(command: &str) -> Result<(), String> {
    Command::new(command)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|_| ())
        .map_err(|error| format!("required command `{command}` is not available: {error}"))
}

#[cfg(windows)]
fn ensure_pinned_font() -> Result<(), String> {
    let windir = env::var_os("WINDIR").ok_or_else(|| String::from("WINDIR is not set"))?;
    let font = PathBuf::from(windir).join("Fonts").join("msmincho.ttc");
    if font.exists() {
        Ok(())
    } else {
        Err(format!(
            "required pinned MS Mincho font file is missing: {}",
            font.display()
        ))
    }
}

#[cfg(not(windows))]
fn ensure_pinned_font() -> Result<(), String> {
    Err(String::from(
        "MS Mincho pinned font probe is only meaningful on Windows",
    ))
}

fn exit_code_text(output: &Output) -> String {
    output.status.code().map_or_else(
        || String::from("terminated-by-signal"),
        |code| code.to_string(),
    )
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
