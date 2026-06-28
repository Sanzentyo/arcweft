#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "write-native-golden-fingerprint"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
publish = false
---

/*
Writes an exact-native-golden environment fingerprint JSON file.

This script does not modify repository fixtures. It writes only the path supplied
through `--out`, so callers can use it before and after native visual artifact
generation.

```bash
cargo +nightly -Zscript tools/write-native-golden-fingerprint.rs \
  --root . \
  --out target/arcweft-native-capture-artifacts/exact-native-golden.environment.json \
  --status artifacts_complete
```
*/

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const METRIC_SET: &[&str] = &["psnr", "ssim", "mse", "mae", "maxae"];
const FIXTURES: &[Fixture] = &[
    Fixture {
        id: "vertical_tutr_golden",
        source: "tests/fixtures/native_capture/vertical_tutr_golden.arcw",
        reference: "tests/fixtures/native_capture/vertical_tutr_golden.png",
    },
    Fixture {
        id: "vertical_jlreq_preset_loose_golden",
        source: "tests/fixtures/native_capture/vertical_jlreq_preset_loose_golden.arcw",
        reference: "tests/fixtures/native_capture/vertical_jlreq_preset_loose_golden.png",
    },
    Fixture {
        id: "vertical_jlreq_preset_normal_golden",
        source: "tests/fixtures/native_capture/vertical_jlreq_preset_normal_golden.arcw",
        reference: "tests/fixtures/native_capture/vertical_jlreq_preset_normal_golden.png",
    },
    Fixture {
        id: "vertical_lr_ruby_text_combine_golden",
        source: "tests/fixtures/native_capture/vertical_lr_ruby_text_combine_golden.arcw",
        reference: "tests/fixtures/native_capture/vertical_lr_ruby_text_combine_golden.png",
    },
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

    let root = args.root.canonicalize().unwrap_or(args.root);
    let artifact_dir = args
        .artifact_dir
        .unwrap_or_else(|| args.out.parent().unwrap_or_else(|| Path::new(".")).to_path_buf());
    if let Some(parent) = args.out.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;
    }

    let json = fingerprint_json(&root, &artifact_dir, &args.status, args.blocker.as_deref());
    fs::write(&args.out, json).map_err(|error| format!("write {}: {error}", args.out.display()))?;
    println!("wrote {}", args.out.display());
    Ok(())
}

fn print_help() {
    println!(
        "write-native-golden-fingerprint\n\n\
         Required arguments:\n  --root <repo-root>\n  --out <fingerprint-json>\n\n\
         Optional arguments:\n  --artifact-dir <dir>\n  --status <status>\n  --blocker <code>\n"
    );
}

#[derive(Debug)]
struct Args {
    root: PathBuf,
    out: PathBuf,
    artifact_dir: Option<PathBuf>,
    status: String,
    blocker: Option<String>,
    help: bool,
}

impl Args {
    fn parse(values: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut root = None;
        let mut out = None;
        let mut artifact_dir = None;
        let mut status = String::from("fingerprint_written");
        let mut blocker = None;
        let mut help = false;
        let mut values = values.peekable();

        while let Some(arg) = values.next() {
            match arg.as_str() {
                "--root" => root = Some(PathBuf::from(next_arg(&mut values, "--root")?)),
                "--out" => out = Some(PathBuf::from(next_arg(&mut values, "--out")?)),
                "--artifact-dir" => {
                    artifact_dir = Some(PathBuf::from(next_arg(&mut values, "--artifact-dir")?));
                }
                "--status" => status = next_arg(&mut values, "--status")?,
                "--blocker" => blocker = Some(next_arg(&mut values, "--blocker")?),
                "--help" | "-h" => help = true,
                _ => return Err(format!("unknown argument `{arg}`")),
            }
        }

        if help {
            return Ok(Self {
                root: PathBuf::from("."),
                out: PathBuf::from("exact-native-golden.environment.json"),
                artifact_dir,
                status,
                blocker,
                help,
            });
        }

        Ok(Self {
            root: root.ok_or_else(|| String::from("missing --root"))?,
            out: out.ok_or_else(|| String::from("missing --out"))?,
            artifact_dir,
            status,
            blocker,
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

#[derive(Clone, Copy)]
struct Fixture {
    id: &'static str,
    source: &'static str,
    reference: &'static str,
}

fn fingerprint_json(
    root: &Path,
    artifact_dir: &Path,
    status: &str,
    blocker: Option<&str>,
) -> String {
    let generated_unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let font_path = pinned_font_path();
    let font_path_exists = font_path.as_ref().is_some_and(|path| path.exists());
    let mut json = String::new();

    writeln!(&mut json, "{{").unwrap();
    writeln!(
        &mut json,
        "  \"schema\": \"arcweft.exact_native_golden.environment.v1\","
    )
    .unwrap();
    writeln!(
        &mut json,
        "  \"generated_unix_seconds\": {generated_unix_seconds},"
    )
    .unwrap();
    writeln!(&mut json, "  \"status\": {},", json_string(status)).unwrap();
    writeln!(&mut json, "  \"blocker\": {},", json_option(blocker)).unwrap();
    writeln!(&mut json, "  \"environment_required\": {},", env_present("ARW_EXACT_NATIVE_GOLDEN_REQUIRED")).unwrap();
    writeln!(&mut json, "  \"environment_pinned\": {},", env_present("ARW_EXACT_NATIVE_GOLDEN_PINNED")).unwrap();
    writeln!(&mut json, "  \"os\": {{").unwrap();
    writeln!(
        &mut json,
        "    \"family\": {},",
        json_string(env::consts::OS)
    )
    .unwrap();
    writeln!(&mut json, "    \"arch\": {},", json_string(env::consts::ARCH)).unwrap();
    writeln!(
        &mut json,
        "    \"version_family\": {}",
        json_option(os_version().as_deref())
    )
    .unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(&mut json, "  \"renderer\": {{").unwrap();
    writeln!(
        &mut json,
        "    \"backend_path\": \"native_rich_text_observer\","
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"backend_env\": {},",
        json_option(env::var("ARW_EXACT_NATIVE_GOLDEN_BACKEND").ok().as_deref())
    )
    .unwrap();
    writeln!(&mut json, "    \"arcw_binary\": \"target/release/arcw.exe\"").unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(&mut json, "  \"font\": {{").unwrap();
    writeln!(&mut json, "    \"requested_family\": \"MS Mincho\",").unwrap();
    writeln!(
        &mut json,
        "    \"fallback_policy\": \"fixture source pins MS Mincho; exact baseline acceptance is blocked if the pinned family probe fails\","
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"windows_font_file\": {},",
        json_option(font_path.as_ref().map(|path| path.display().to_string()).as_deref())
    )
    .unwrap();
    writeln!(&mut json, "    \"windows_font_file_exists\": {font_path_exists}").unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(&mut json, "  \"viewport\": {{").unwrap();
    writeln!(&mut json, "    \"width\": 1280,").unwrap();
    writeln!(&mut json, "    \"height\": 720,").unwrap();
    writeln!(&mut json, "    \"device_scale\": 1.0").unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(&mut json, "  \"png\": {{").unwrap();
    writeln!(&mut json, "    \"format\": \"png\",").unwrap();
    writeln!(&mut json, "    \"capture_command_format\": \"arcw agent observe --image png\",").unwrap();
    writeln!(&mut json, "    \"color_format\": \"PNG bytes emitted by native Agent capture\"").unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(&mut json, "  \"arcweft\": {{").unwrap();
    writeln!(&mut json, "    \"commit\": {},", json_option(git_commit(root).as_deref())).unwrap();
    writeln!(&mut json, "    \"dirty\": {}", git_dirty(root).map_or(String::from("null"), |dirty| dirty.to_string())).unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(&mut json, "  \"imq\": {{").unwrap();
    writeln!(&mut json, "    \"available\": {},", imq_available()).unwrap();
    writeln!(&mut json, "    \"version\": {},", json_option(imq_version().as_deref())).unwrap();
    writeln!(&mut json, "    \"metrics\": [{}]", json_string_list(METRIC_SET)).unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(
        &mut json,
        "  \"artifact_dir\": {},",
        json_string(&artifact_dir.display().to_string())
    )
    .unwrap();
    write_fixture_list(&mut json, root, artifact_dir);
    writeln!(&mut json, "}}").unwrap();
    json
}

fn write_fixture_list(json: &mut String, root: &Path, artifact_dir: &Path) {
    writeln!(json, "  \"fixtures\": [").unwrap();
    for (index, fixture) in FIXTURES.iter().enumerate() {
        let comma = if index + 1 == FIXTURES.len() { "" } else { "," };
        let candidate_png = artifact_dir.join(format!("{}.candidate.png", fixture.id));
        let refresh_png = artifact_dir.join(format!("{}.png", fixture.id));
        let observe_json = artifact_dir.join(format!("{}.observe.json", fixture.id));
        let metrics_json = artifact_dir.join(format!("{}.imq.json", fixture.id));
        writeln!(json, "    {{").unwrap();
        writeln!(json, "      \"id\": {},", json_string(fixture.id)).unwrap();
        writeln!(json, "      \"source\": {},", json_string(fixture.source)).unwrap();
        writeln!(json, "      \"source_hash\": {},", json_option(git_hash_object(root, fixture.source).as_deref())).unwrap();
        writeln!(json, "      \"reference\": {},", json_string(fixture.reference)).unwrap();
        writeln!(json, "      \"reference_hash\": {},", json_option(git_hash_object(root, fixture.reference).as_deref())).unwrap();
        writeln!(json, "      \"candidate_png\": {},", artifact_json(&candidate_png)).unwrap();
        writeln!(json, "      \"refresh_png\": {},", artifact_json(&refresh_png)).unwrap();
        writeln!(json, "      \"observe_json\": {},", artifact_json(&observe_json)).unwrap();
        writeln!(json, "      \"metrics_json\": {}", artifact_json(&metrics_json)).unwrap();
        writeln!(json, "    }}{comma}").unwrap();
    }
    writeln!(json, "  ]").unwrap();
}

fn artifact_json(path: &Path) -> String {
    format!(
        "{{\"path\": {}, \"exists\": {}}}",
        json_string(&path.display().to_string()),
        path.exists()
    )
}

fn env_present(name: &str) -> bool {
    env::var_os(name).is_some()
}

fn pinned_font_path() -> Option<PathBuf> {
    env::var_os("WINDIR").map(|windir| PathBuf::from(windir).join("Fonts").join("msmincho.ttc"))
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

fn git_commit(root: &Path) -> Option<String> {
    command_stdout(Command::new("git").arg("-C").arg(root).arg("rev-parse").arg("HEAD"))
}

fn git_dirty(root: &Path) -> Option<bool> {
    command_stdout(Command::new("git").arg("-C").arg(root).arg("status").arg("--short"))
        .map(|status| !status.is_empty())
}

fn git_hash_object(root: &Path, relative_path: &str) -> Option<String> {
    command_stdout(
        Command::new("git")
            .arg("-C")
            .arg(root)
            .arg("hash-object")
            .arg(root.join(relative_path)),
    )
}

fn imq_available() -> bool {
    Command::new("imq")
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn imq_version() -> Option<String> {
    command_stdout(Command::new("imq").arg("--version")).or_else(|| {
        command_stdout(Command::new("imq").arg("--help"))
            .and_then(|help| help.lines().next().map(str::to_owned))
    })
}

fn command_stdout(command: &mut Command) -> Option<String> {
    let output = command.stderr(Stdio::null()).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    Some(trimmed.to_owned())
}

fn json_string(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn json_option(value: Option<&str>) -> String {
    value.map_or_else(|| String::from("null"), json_string)
}

fn json_string_list(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| json_string(value))
        .collect::<Vec<_>>()
        .join(", ")
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
