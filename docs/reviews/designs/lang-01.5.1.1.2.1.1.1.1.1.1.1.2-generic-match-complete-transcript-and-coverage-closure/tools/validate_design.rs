#!/usr/bin/env -S cargo +nightly -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
serde_json = "1"
sha2 = "0.10"
syn = { version = "2", features = ["full"] }
walkdir = "2"
---

mod validation_support;

use std::path::PathBuf;
use validation_support::{
    load_bundle, validate_manifest, validate_repository, validate_semantic, DESIGN_REL,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("FAIL {}", error);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut design_only = false;
    let mut root = None;
    for argument in std::env::args_os().skip(1) {
        if argument == "--design-only" {
            design_only = true;
        } else if root.replace(PathBuf::from(argument)).is_some() {
            return Err("more than one design path supplied".into());
        }
    }
    let root = root.unwrap_or_else(|| PathBuf::from(DESIGN_REL));
    let bundle = load_bundle(&root)?;
    validate_semantic(&bundle)?;
    validate_manifest(&bundle)?;
    if !design_only {
        validate_repository(&bundle)?;
    }
    println!(
        "PASS design={} files={} repository={} inventories=27/8/7/5/38/13/35/5/13 decisions=1-7",
        bundle.root.display(),
        bundle.files.len(),
        if design_only { "NOT_RUN" } else { "PASS" }
    );
    Ok(())
}
