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
    let root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DESIGN_REL));
    let bundle = load_bundle(&root)?;
    validate_semantic(&bundle)?;
    validate_manifest(&bundle)?;
    validate_repository(&bundle)?;
    println!(
        "PASS design={} files={} head=9a5d30d25620541c3f2975d31e04e04e3bc9514c inventories=27/8/7/5/38/13/35/5/13 decisions=1-7",
        bundle.root.display(),
        bundle.files.len()
    );
    Ok(())
}
