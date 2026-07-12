#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
arcweft-bundle = { path = "../crates/arcweft-bundle" }
arcweft-player-web = { path = "../crates/arcweft-player-web" }
arcweft-render-wgpu = { path = "../crates/arcweft-render-wgpu" }
png = "0.18.1"
pollster = "0.4.0"
wgpu = { version = "29.0.3", default-features = false, features = ["std", "wgsl", "dx12", "metal", "vulkan"] }

[patch.crates-io]
glyphon = { path = "../vendor/glyphon" }
---

use arcweft_bundle::ArcweftBundle;
use arcweft_player_web::parity::{WebGpuParityCheckpoint, prepare_bundle_parity_frame};
use arcweft_render_wgpu::offscreen::{CaptureAttachment, CaptureRequest, SharedOffscreenCapture};
use png::{BitDepth, ColorType, Encoder};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse(env::args().skip(1).collect())?;
    let bundle = ArcweftBundle::from_json_slice(&fs::read(&args.bundle)?)?;
    let frame =
        prepare_bundle_parity_frame(&bundle, args.checkpoint.options(args.visual_time_millis))?;
    let mut capture = pollster::block_on(SharedOffscreenCapture::new(args.target_format.wgpu()))?;
    capture.register_font_bytes(fs::read(&args.font)?)?;
    let image = capture.capture(&frame, &CaptureRequest::whole_frame_color())?;
    let rgba = image
        .attachment_rgba(CaptureAttachment::Color)
        .ok_or("offscreen capture omitted the requested color attachment")?;
    write_png(&args.output, image.width, image.height, rgba)?;
    println!(
        "wrote native shared-renderer capture {} ({}x{}, checkpoint={}, visual_time_millis={}, target_format={})",
        args.output.display(),
        image.width,
        image.height,
        args.checkpoint.as_str(),
        args.visual_time_millis,
        args.target_format.as_str()
    );
    Ok(())
}

struct Args {
    bundle: PathBuf,
    font: PathBuf,
    output: PathBuf,
    visual_time_millis: u64,
    target_format: CaptureTargetFormat,
    checkpoint: WebGpuParityCheckpoint,
}

impl Args {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut parsed = Self {
            bundle: PathBuf::from("web/demo.awfb"),
            font: PathBuf::from("web/assets/arcweft-demo.ttf"),
            output: PathBuf::from("target/webgpu-parity/native.png"),
            visual_time_millis: 160,
            target_format: CaptureTargetFormat::Rgba8Unorm,
            checkpoint: WebGpuParityCheckpoint::default(),
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--bundle" => {
                    index += 1;
                    parsed.bundle = PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "--bundle requires a path".to_owned())?,
                    );
                }
                "--font" => {
                    index += 1;
                    parsed.font = PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "--font requires a path".to_owned())?,
                    );
                }
                "--output" => {
                    index += 1;
                    parsed.output = PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "--output requires a path".to_owned())?,
                    );
                }
                "--visual-time-millis" => {
                    index += 1;
                    parsed.visual_time_millis = args
                        .get(index)
                        .ok_or_else(|| "--visual-time-millis requires an integer".to_owned())?
                        .parse()
                        .map_err(|error| {
                            format!("--visual-time-millis must be an integer: {error}")
                        })?;
                }
                "--target-format" => {
                    index += 1;
                    parsed.target_format = args
                        .get(index)
                        .ok_or_else(|| "--target-format requires a format".to_owned())?
                        .parse()?;
                }
                "--checkpoint" => {
                    index += 1;
                    parsed.checkpoint = args
                        .get(index)
                        .ok_or_else(|| "--checkpoint requires a checkpoint".to_owned())?
                        .parse::<WebGpuParityCheckpoint>()
                        .map_err(|error| error.to_string())?;
                }
                "--help" | "-h" => return Err(Self::usage()),
                unknown => return Err(format!("unknown argument `{unknown}`\n{}", Self::usage())),
            }
            index += 1;
        }
        Ok(parsed)
    }

    fn usage() -> String {
        "usage: cargo +nightly -Zscript tools/capture-webgpu-native-frame.rs \
         [--bundle web/demo.awfb] [--font web/assets/arcweft-demo.ttf] \
         [--output target/webgpu-parity/native.png] [--visual-time-millis 160] \
         [--target-format rgba8unorm|rgba8unorm-srgb|bgra8unorm|bgra8unorm-srgb] \
         [--checkpoint neutral|focus-first-choice|hover-first-choice|hover-second-choice|press-first-choice|compact-focus-first-choice|hidpi-focus-first-choice]"
            .to_owned()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaptureTargetFormat {
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Bgra8Unorm,
    Bgra8UnormSrgb,
}

impl CaptureTargetFormat {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Rgba8Unorm => "rgba8unorm",
            Self::Rgba8UnormSrgb => "rgba8unorm-srgb",
            Self::Bgra8Unorm => "bgra8unorm",
            Self::Bgra8UnormSrgb => "bgra8unorm-srgb",
        }
    }

    const fn wgpu(self) -> wgpu::TextureFormat {
        match self {
            Self::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            Self::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            Self::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
            Self::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
        }
    }
}

impl std::str::FromStr for CaptureTargetFormat {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "rgba8unorm" => Ok(Self::Rgba8Unorm),
            "rgba8unorm-srgb" => Ok(Self::Rgba8UnormSrgb),
            "bgra8unorm" => Ok(Self::Bgra8Unorm),
            "bgra8unorm-srgb" => Ok(Self::Bgra8UnormSrgb),
            unknown => Err(format!("unknown target format `{unknown}`")),
        }
    }
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(path)?;
    let mut encoder = Encoder::new(file, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}
