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
---

use arcweft_bundle::ArcweftBundle;
use arcweft_player_web::parity::{WebGpuParityFrameOptions, prepare_bundle_parity_frame};
use arcweft_render_wgpu::offscreen::SharedOffscreenCapture;
use png::{BitDepth, ColorType, Encoder};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse(env::args().skip(1).collect())?;
    let bundle = ArcweftBundle::from_json_slice(&fs::read(&args.bundle)?)?;
    let frame = prepare_bundle_parity_frame(
        &bundle,
        WebGpuParityFrameOptions {
            visual_time_millis: args.visual_time_millis,
            ..WebGpuParityFrameOptions::default()
        },
    )?;
    let mut capture = pollster::block_on(SharedOffscreenCapture::new(
        wgpu_format(),
    ))?;
    capture.register_font_bytes(fs::read(&args.font)?)?;
    let image = capture.capture_frame(&frame)?;
    write_png(&args.output, image.width, image.height, &image.rgba)?;
    println!(
        "wrote native shared-renderer capture {} ({}x{}, visual_time_millis={})",
        args.output.display(),
        image.width,
        image.height,
        args.visual_time_millis
    );
    Ok(())
}

fn wgpu_format() -> wgpu::TextureFormat {
    wgpu::TextureFormat::Rgba8Unorm
}

struct Args {
    bundle: PathBuf,
    font: PathBuf,
    output: PathBuf,
    visual_time_millis: u64,
}

impl Args {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut parsed = Self {
            bundle: PathBuf::from("web/demo.awfb"),
            font: PathBuf::from("web/assets/arcweft-demo.ttf"),
            output: PathBuf::from("target/webgpu-parity/native.png"),
            visual_time_millis: 160,
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
         [--output target/webgpu-parity/native.png] [--visual-time-millis 160]"
            .to_owned()
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
