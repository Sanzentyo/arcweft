#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"
---

use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse(env::args().skip(1).collect())?;
    check_fixture_set(&args.fixtures)?;
    println!("css layout/cascade coverage fixture gates passed");
    Ok(())
}

#[derive(Clone, Debug)]
struct Args {
    fixtures: PathBuf,
}

impl Args {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut parsed = Self {
            fixtures: PathBuf::from("fixtures/css-layout-cascade-coverage"),
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--fixtures" => {
                    index += 1;
                    parsed.fixtures = args
                        .get(index)
                        .map(PathBuf::from)
                        .ok_or_else(|| "--fixtures requires a path".to_owned())?;
                }
                "--help" | "-h" => return Err(Self::usage()),
                unknown => return Err(format!("unknown argument `{unknown}`\n{}", Self::usage())),
            }
            index += 1;
        }
        Ok(parsed)
    }

    fn usage() -> String {
        "usage: cargo +nightly -Zscript tools/run-css-layout-cascade-coverage-gates.rs [--fixtures fixtures/css-layout-cascade-coverage]".to_owned()
    }
}

fn check_fixture_set(fixtures: &Path) -> Result<(), Box<dyn Error>> {
    let css = read_required(fixtures.join("coverage.css"))?;
    require_contains(&css, "display: flex", "coverage.css flex declaration")?;
    require_contains(&css, "grid-template-columns", "coverage.css grid diagnostic probe")?;
    require_contains(&css, "@container", "coverage.css container diagnostic probe")?;
    require_contains(&css, "::before", "coverage.css pseudo-element diagnostic probe")?;
    require_contains(
        &css,
        "overflow-y: auto",
        "coverage.css interactive overflow diagnostic probe",
    )?;
    require_contains(
        &css,
        "var(--missing-token)",
        "coverage.css unresolved variable diagnostic probe",
    )?;

    for checkpoint in ["default", "compact", "hidpi"] {
        let json = read_required(fixtures.join(format!("computed-style-{checkpoint}.json")))?;
        require_contains(
            &json,
            "arcweft.css-layout-cascade-coverage.v1",
            &format!("computed-style-{checkpoint}.json schema"),
        )?;
        require_contains(&json, "computed_styles", &format!("computed-style-{checkpoint}.json styles"))?;
        require_contains(&json, "layout_boxes", &format!("computed-style-{checkpoint}.json layout"))?;
        require_contains(&json, "invalidation", &format!("computed-style-{checkpoint}.json invalidation"))?;
    }

    let diagnostics = read_required(fixtures.join("unsupported-diagnostics.expected.json"))?;
    for code in [
        "UnsupportedCssSelector",
        "CssCoverageGap",
        "UnresolvedCssVariable",
    ] {
        require_contains(&diagnostics, code, &format!("diagnostic code {code}"))?;
    }

    let visual = read_required(fixtures.join("visual-smoke-manifest.json"))?;
    for checkpoint in ["default", "compact", "hidpi"] {
        require_contains(&visual, checkpoint, &format!("visual checkpoint {checkpoint}"))?;
    }
    require_contains(&visual, "scale", "visual smoke high-DPI scale evidence")?;

    Ok(())
}

fn read_required(path: impl AsRef<Path>) -> Result<String, Box<dyn Error>> {
    let path = path.as_ref();
    fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()).into())
}

fn require_contains(haystack: &str, needle: &str, label: &str) -> Result<(), Box<dyn Error>> {
    if haystack.contains(needle) {
        Ok(())
    } else {
        Err(format!("missing {label}: expected `{needle}`").into())
    }
}
