#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"
---

use std::env;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse(env::args().skip(1).collect())?;
    let mut failures = Vec::new();

    for checkpoint in Checkpoint::all() {
        run_text_raster_gate(&args, checkpoint, &mut failures);
    }
    for checkpoint in Checkpoint::all() {
        run_full_image_gate(&args, checkpoint, &mut failures);
    }
    if args.run_imq {
        for checkpoint in Checkpoint::all() {
            run_imq_gate(&args, checkpoint, &mut failures);
        }
    }

    if failures.is_empty() {
        println!("css-style parity gates passed after writing all evidence");
        Ok(())
    } else {
        for failure in &failures {
            eprintln!("css-style parity gate failed: {failure}");
        }
        Err(format!("{} css-style parity gate(s) failed", failures.len()).into())
    }
}

#[derive(Clone, Debug)]
struct Args {
    dir: PathBuf,
    font: PathBuf,
    run_imq: bool,
}

impl Args {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut parsed = Self {
            dir: PathBuf::from("target/css-style-parity"),
            font: PathBuf::from("web/assets/arcweft-demo.ttf"),
            run_imq: true,
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--dir" => {
                    index += 1;
                    parsed.dir = path_arg(&args, index, "--dir")?;
                }
                "--font" => {
                    index += 1;
                    parsed.font = path_arg(&args, index, "--font")?;
                }
                "--no-imq" => parsed.run_imq = false,
                "--help" | "-h" => return Err(Self::usage()),
                unknown => return Err(format!("unknown argument `{unknown}`\n{}", Self::usage())),
            }
            index += 1;
        }
        Ok(parsed)
    }

    fn usage() -> String {
        "usage: cargo +nightly -Zscript tools/run-css-style-parity-gates.rs \
         [--dir target/css-style-parity] [--font web/assets/arcweft-demo.ttf] [--no-imq]"
            .to_owned()
    }
}

fn path_arg(args: &[String], index: usize, name: &str) -> Result<PathBuf, String> {
    args.get(index)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{name} requires a path"))
}

#[derive(Clone, Copy, Debug)]
enum Checkpoint {
    Default,
    Compact,
    Hidpi,
}

impl Checkpoint {
    const fn all() -> [Self; 3] {
        [Self::Default, Self::Compact, Self::Hidpi]
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Compact => "compact",
            Self::Hidpi => "hidpi",
        }
    }

    const fn full_image_thresholds(self) -> FullImageThresholds {
        match self {
            Self::Default => FullImageThresholds {
                min_psnr: "25.0",
                min_ssim: "0.60",
                max_mse: "0.0030",
                max_mae: "0.0048",
                max_changed_pixel_ratio: "0.011",
            },
            Self::Compact => FullImageThresholds {
                min_psnr: "24.0",
                min_ssim: "0.55",
                max_mse: "0.0039",
                max_mae: "0.0063",
                max_changed_pixel_ratio: "0.0145",
            },
            Self::Hidpi => FullImageThresholds {
                min_psnr: "19.8",
                min_ssim: "0.44",
                max_mse: "0.0102",
                max_mae: "0.0148",
                max_changed_pixel_ratio: "0.028",
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct FullImageThresholds {
    min_psnr: &'static str,
    min_ssim: &'static str,
    max_mse: &'static str,
    max_mae: &'static str,
    max_changed_pixel_ratio: &'static str,
}

fn run_text_raster_gate(args: &Args, checkpoint: Checkpoint, failures: &mut Vec<String>) {
    let name = checkpoint.name();
    let command = CommandSpec::new("cargo")
        .args(["+nightly", "-Zscript", "tools/verify-text-raster-parity.rs"])
        .args(["--checkpoint", name])
        .arg("--native")
        .arg(args.dir.join(format!("native-{name}.png")))
        .arg("--web")
        .arg(args.dir.join(format!("web-{name}.png")))
        .arg("--native-frame")
        .arg(args.dir.join(format!("native-{name}.frame.json")))
        .arg("--web-frame")
        .arg(args.dir.join(format!("web-{name}.frame.json")))
        .arg("--font")
        .arg(&args.font)
        .arg("--report")
        .arg(args.dir.join(format!("text-raster-{name}.json")));
    run_gate(format!("text-raster-{name}"), command, failures);
}

fn run_full_image_gate(args: &Args, checkpoint: Checkpoint, failures: &mut Vec<String>) {
    let name = checkpoint.name();
    let thresholds = checkpoint.full_image_thresholds();
    let command = CommandSpec::new("cargo")
        .args(["+nightly", "-Zscript", "tools/verify-webgpu-parity.rs"])
        .arg("--native")
        .arg(args.dir.join(format!("native-{name}.png")))
        .arg("--web")
        .arg(args.dir.join(format!("web-{name}.png")))
        .arg("--report")
        .arg(args.dir.join(format!("parity-{name}.json")))
        .args(["--min-psnr", thresholds.min_psnr])
        .args(["--min-ssim", thresholds.min_ssim])
        .args(["--max-mse", thresholds.max_mse])
        .args(["--max-mae", thresholds.max_mae])
        .args([
            "--max-changed-pixel-ratio",
            thresholds.max_changed_pixel_ratio,
        ]);
    run_gate(format!("full-image-{name}"), command, failures);
}

fn run_imq_gate(args: &Args, checkpoint: Checkpoint, failures: &mut Vec<String>) {
    let name = checkpoint.name();
    let command = CommandSpec::new("imq")
        .arg("compare")
        .arg(args.dir.join(format!("native-{name}.png")))
        .arg(args.dir.join(format!("web-{name}.png")))
        .args(["--metrics", "psnr,ssim,mse,mae,maxae"])
        .args(["--format", "json"])
        .arg("--output")
        .arg(args.dir.join(format!("imq-{name}.json")));
    run_gate(format!("imq-{name}"), command, failures);
}

fn run_gate(label: String, command: CommandSpec, failures: &mut Vec<String>) {
    println!("running {label}: {}", command.display());
    match command.status() {
        Ok(status) if status.success() => println!("passed {label}"),
        Ok(status) => failures.push(format!("{label} exited with {status}")),
        Err(error) => failures.push(format!("{label} could not start: {error}")),
    }
}

#[derive(Clone, Debug)]
struct CommandSpec {
    program: OsString,
    args: Vec<OsString>,
}

impl CommandSpec {
    fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_owned(),
            args: Vec::new(),
        }
    }

    fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_owned());
        self
    }

    fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
        self
    }

    fn status(&self) -> Result<std::process::ExitStatus, std::io::Error> {
        Command::new(&self.program).args(&self.args).status()
    }

    fn display(&self) -> String {
        std::iter::once(self.program.as_os_str())
            .chain(self.args.iter().map(OsString::as_os_str))
            .map(display_arg)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn display_arg(arg: &OsStr) -> String {
    let value = arg.to_string_lossy();
    if value.contains(' ') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.into_owned()
    }
}
