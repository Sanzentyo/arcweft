#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "arcweft-structure-audit"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
publish = false

[dependencies]
---

#[path = "arcweft-structure-audit/src/args.rs"]
mod args;
#[path = "arcweft-structure-audit/src/cargo_manifest.rs"]
mod cargo_manifest;
#[path = "arcweft-structure-audit/src/metrics.rs"]
mod metrics;
#[path = "arcweft-structure-audit/src/report.rs"]
mod report;
#[path = "arcweft-structure-audit/src/rules.rs"]
mod rules;
#[path = "arcweft-structure-audit/src/walk.rs"]
mod walk;

use args::ParseOutcome;
use std::error::Error;

fn main() {
    match run() {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32, Box<dyn Error>> {
    let arguments = match args::parse()
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?
    {
        ParseOutcome::Help => {
            print!("{}", args::help());
            return Ok(0);
        }
        ParseOutcome::Run(arguments) => arguments,
    };

    let root = arguments.root.canonicalize()?;
    let paths = walk::collect_files(&root)?;
    let files = metrics::analyze_files(&root, &paths)?;
    let manifests = cargo_manifest::parse_manifests(&root, &paths)?;
    let violations = rules::evaluate(&files, &manifests);

    if let Some(directory) = arguments.write_dir.as_deref() {
        report::write_reports(directory, &files, &manifests, &violations)?;
    }
    report::print_summary(
        &files,
        &manifests,
        &violations,
        arguments.write_dir.as_deref(),
    );

    Ok(if arguments.fail_on_violations && !violations.is_empty() {
        2
    } else {
        0
    })
}
