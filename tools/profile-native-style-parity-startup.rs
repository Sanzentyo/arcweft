#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.145"
---

use serde::Serialize;
use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse(env::args().skip(1))?;
    fs::create_dir_all("web/local")?;
    fs::create_dir_all("target/native-style-parity")?;

    let mut samples = Vec::new();
    run_step(
        &mut samples,
        "bundle native-style-parity",
        CommandSpec::new("cargo").args([
            "run",
            "-p",
            "arcweft-cli",
            "--",
            "bundle",
            "samples/native-style-parity/main.arcw",
            "--output",
            "web/local/native-style-parity.awfb",
        ]),
    )?;
    run_step(
        &mut samples,
        "build player web wasm",
        CommandSpec::new("cargo").args([
            "build",
            "-p",
            "arcweft-player-web",
            "--target",
            "wasm32-unknown-unknown",
            "--quiet",
        ]),
    )?;
    run_step(
        &mut samples,
        "wasm-bindgen player web",
        CommandSpec::new("wasm-bindgen").args([
            "--target",
            "web",
            "--out-dir",
            "web/pkg",
            "--out-name",
            "arcweft_player_web",
            "target/wasm32-unknown-unknown/debug/arcweft_player_web.wasm",
        ]),
    )?;

    for viewport in ["default", "compact", "hidpi"] {
        run_step(
            &mut samples,
            format!("native capture {viewport}"),
            CommandSpec::new("cargo").args([
                "+nightly".to_owned(),
                "-Zscript".to_owned(),
                "tools/capture-text-parity-frame.rs".to_owned(),
                "--bundle".to_owned(),
                "web/local/native-style-parity.awfb".to_owned(),
                "--checkpoint".to_owned(),
                viewport.to_owned(),
                "--output".to_owned(),
                format!("target/native-style-parity/native-{viewport}.png"),
                "--no-frame-report".to_owned(),
                "--viewport".to_owned(),
                viewport.to_owned(),
                "--visual-time-millis".to_owned(),
                "9000".to_owned(),
                "--target-format".to_owned(),
                "rgba8unorm".to_owned(),
            ]),
        )?;
    }

    run_step(
        &mut samples,
        "web capture default compact hidpi",
        CommandSpec::new("node")
            .env(
                "ARW_TEXT_PARITY_DIR",
                PathBuf::from("target/native-style-parity")
                    .canonicalize()?
                    .display()
                    .to_string(),
            )
            .env("ARW_TEXT_PARITY_CHECKPOINTS", "default,compact,hidpi")
            .env("ARW_TEXT_PARITY_VISUAL_TIME_MILLIS", "9000")
            .env(
                "ARW_TEXT_PARITY_REQUIRED_TEXT",
                "DSL-styled text|wave motion",
            )
            .args(["web/tests/text-parity-smoke.mjs"]),
    )?;

    let json = serde_json::to_vec_pretty(&samples)?;
    if let Some(parent) = args.output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, json)?;
    println!(
        "wrote native-style parity startup profile {}",
        args.output.display()
    );
    Ok(())
}

fn run_step(
    samples: &mut Vec<StartupProfileSample>,
    name: impl Into<String>,
    spec: CommandSpec,
) -> Result<(), Box<dyn Error>> {
    let name = name.into();
    println!("{:>12} {name}", "Profiling");
    let started = Instant::now();
    let status = spec.to_command().status()?;
    let elapsed_ms = started.elapsed().as_millis();
    let sample_status = if status.success() { "ok" } else { "failed" };
    samples.push(StartupProfileSample {
        step: name.clone(),
        status: sample_status,
        exit_code: status.code(),
        millis: elapsed_ms,
    });
    println!("{:>12} {name} in {elapsed_ms}ms", "Finished");
    if status.success() {
        Ok(())
    } else {
        Err(format!("profile step `{name}` exited with {status}").into())
    }
}

#[derive(Clone, Debug)]
struct CommandSpec {
    program: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
}

impl CommandSpec {
    fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            envs: Vec::new(),
        }
    }

    fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.envs.push((name.into(), value.into()));
        self
    }

    fn to_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .env_remove("RUSTUP_TOOLCHAIN")
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        for (name, value) in &self.envs {
            command.env(name, value);
        }
        command
    }
}

#[derive(Clone, Debug)]
struct Args {
    output: PathBuf,
}

impl Args {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, Box<dyn Error>> {
        let mut output = PathBuf::from("target/native-style-parity/startup-profile.json");
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--output" => {
                    let Some(value) = args.next() else {
                        return Err("--output requires a path".into());
                    };
                    output = PathBuf::from(value);
                }
                "--help" | "-h" => {
                    println!(
                        "Usage: cargo +nightly -Zscript tools/profile-native-style-parity-startup.rs [--output <path>]"
                    );
                    std::process::exit(0);
                }
                _ => return Err(format!("unknown argument `{arg}`").into()),
            }
        }
        Ok(Self { output })
    }
}

#[derive(Clone, Debug, Serialize)]
struct StartupProfileSample {
    step: String,
    status: &'static str,
    exit_code: Option<i32>,
    millis: u128,
}
