#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "regenerate-windowed-live-patch-fixtures"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
publish = false

[dependencies]
arcweft-bundle = { path = "../crates/arcweft-bundle" }
arcweft-core = { path = "../crates/arcweft-core" }
arcweft-player-native = { path = "../crates/arcweft-player-native" }
arcweft-player-scene = { path = "../crates/arcweft-player-scene" }
arcweft-render-text = { path = "../crates/arcweft-render-text" }
arcweft-runtime-driver = { path = "../crates/arcweft-runtime-driver" }
arcweft-runtime-plan = { path = "../crates/arcweft-runtime-plan" }
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.150"

[patch.crates-io]
glyphon = { path = "../vendor/glyphon" }
---

#[path = "../crates/arcweft-player-native/tests/support/windowed_live_patch_fixtures.rs"]
mod windowed_live_patch_fixtures;

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use windowed_live_patch_fixtures::{
    GENERATED_DIR, GeneratedFixtureFile, all_smoke_reports, assert_fixture_compatibility,
    build_windowed_live_patch_fixtures, generated_fixture_files, summarize_report,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Check,
    Apply,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mode = parse_mode(env::args().skip(1))?;
    let root = env::current_dir().map_err(|error| error.to_string())?;
    let fixtures = build_windowed_live_patch_fixtures();
    assert_fixture_compatibility(&fixtures);
    let reports = all_smoke_reports(&fixtures)?;
    let files = generated_fixture_files(&fixtures, &reports).map_err(|error| error.to_string())?;

    match mode {
        Mode::Check => check_files(&root, &files),
        Mode::Apply => {
            write_files(&root, &files)?;
            remove_stale_generated_files(&root, &files)?;
            println!("regenerated {} seq-03.7 windowed live-patch fixture files", files.len());
            for report in &reports {
                print!("{}", summarize_report(report));
            }
            Ok(())
        }
    }
}

fn parse_mode(args: impl Iterator<Item = String>) -> Result<Mode, String> {
    let mut mode = Mode::Check;
    for arg in args {
        match arg.as_str() {
            "--check" => mode = Mode::Check,
            "--apply" | "-a" => mode = Mode::Apply,
            "-h" | "--help" => {
                println!(
                    "Usage: cargo +nightly -Zscript tools/regenerate-windowed-live-patch-fixtures.rs [--check|--apply]"
                );
                println!("Default mode is --check; --apply writes AWFB bundles, patch bundles, and JSON smoke reports.");
                process::exit(0);
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(mode)
}

fn check_files(root: &Path, files: &[GeneratedFixtureFile]) -> Result<(), String> {
    let expected = files
        .iter()
        .map(|file| normalize_relative_path(&file.relative_path))
        .collect::<BTreeSet<_>>();
    let mut stale = files
        .iter()
        .filter_map(|file| {
            let path = root.join(&file.relative_path);
            match fs::read(&path) {
                Ok(current) if current == file.bytes => None,
                Ok(_) => Some(format!("stale {}", file.relative_path)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Some(format!("missing {}", file.relative_path))
                }
                Err(error) => Some(format!("failed to read {}: {error}", path.display())),
            }
        })
        .collect::<Vec<_>>();

    let generated = root.join(GENERATED_DIR);
    if generated.exists() {
        for path in generated_files(&generated)? {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| format!("failed to relativize {}: {error}", path.display()))?;
            let relative = normalize_path(relative);
            if generated_file_is_managed(&path) && !expected.contains(&relative) {
                stale.push(format!("unexpected {relative}"));
            }
        }
    }

    if stale.is_empty() {
        println!("seq-03.7 windowed live-patch generated fixtures are current");
        Ok(())
    } else {
        for item in &stale {
            eprintln!("{item}");
        }
        Err("generated fixtures are stale; run with --apply".to_owned())
    }
}

fn write_files(root: &Path, files: &[GeneratedFixtureFile]) -> Result<(), String> {
    for file in files {
        let path = root.join(&file.relative_path);
        let parent = path
            .parent()
            .ok_or_else(|| format!("generated fixture path has no parent: {}", path.display()))?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        fs::write(&path, &file.bytes)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }
    Ok(())
}

fn remove_stale_generated_files(
    root: &Path,
    files: &[GeneratedFixtureFile],
) -> Result<(), String> {
    let generated = root.join(GENERATED_DIR);
    if !generated.exists() {
        return Ok(());
    }
    let expected = files
        .iter()
        .map(|file| normalize_relative_path(&file.relative_path))
        .collect::<BTreeSet<_>>();
    for path in generated_files(&generated)? {
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("failed to relativize {}: {error}", path.display()))?;
        let relative = normalize_path(relative);
        if generated_file_is_managed(&path) && !expected.contains(&relative) {
            fs::remove_file(&path)
                .map_err(|error| format!("failed to remove stale {}: {error}", path.display()))?;
        }
    }
    Ok(())
}

fn generated_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_generated_files(root, &mut files)?;
    Ok(files)
}

fn collect_generated_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root)
        .map_err(|error| format!("failed to read {}: {error}", root.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect_generated_files(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn generated_file_is_managed(path: &Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| matches!(extension, "awfb" | "patch" | "json"))
}

fn normalize_relative_path(path: &str) -> String {
    normalize_path(Path::new(path))
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
