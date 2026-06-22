#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "generate-webgpu-demo-assets"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
publish = false

[dependencies]
arcweft-image = { path = "../crates/arcweft-image" }
image = { version = "0.25.10", default-features = false, features = ["png", "gif", "webp"] }
---

use arcweft_image::{ImageDecodeOptions, ImageFormat, decode_image_bytes};
use image::codecs::gif::{GifEncoder, Repeat};
use image::codecs::png::PngEncoder;
use image::codecs::webp::WebPEncoder;
use image::{Delay, ExtendedColorType, Frame, ImageEncoder, Rgba, RgbaImage};
use std::error::Error;
use std::fs;
use std::path::Path;

const WEB_ASSET_DIR: &str = "web/assets";
const BUNDLE_ASSET_DIR: &str = "web/.arcweft/asset/generated";
const PULSE_SIZE: u32 = 96;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let web_asset_dir = Path::new(WEB_ASSET_DIR);
    let bundle_asset_dir = Path::new(BUNDLE_ASSET_DIR);
    fs::create_dir_all(web_asset_dir)?;
    fs::create_dir_all(bundle_asset_dir)?;

    let background = background_image();
    write_png(&web_asset_dir.join("generated-background.png"), &background)?;
    write_png(&bundle_asset_dir.join("background.png"), &background)?;
    validate_static(
        ImageFormat::Png,
        &fs::read(web_asset_dir.join("generated-background.png"))?,
    )?;

    let character = character_image();
    write_png(&web_asset_dir.join("generated-character.png"), &character)?;
    write_png(&bundle_asset_dir.join("character_stand.png"), &character)?;
    validate_static(
        ImageFormat::Png,
        &fs::read(web_asset_dir.join("generated-character.png"))?,
    )?;

    let frames = pulse_frames();
    write_gif(&web_asset_dir.join("generated-pulse.gif"), &frames)?;
    write_gif(&bundle_asset_dir.join("gif_pulse.gif"), &frames)?;
    validate_animation(
        ImageFormat::Gif,
        &fs::read(web_asset_dir.join("generated-pulse.gif"))?,
    )?;

    let webp = animated_webp(&frames)?;
    fs::write(web_asset_dir.join("generated-pulse.webp"), &webp)?;
    fs::write(bundle_asset_dir.join("webp_pulse.webp"), &webp)?;
    validate_animation(ImageFormat::WebP, &webp)?;

    println!(
        "generated WebGPU demo assets in {} and {}",
        web_asset_dir.display(),
        bundle_asset_dir.display()
    );
    Ok(())
}

fn background_image() -> RgbaImage {
    let width = 640;
    let height = 360;
    let mut image = RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let sky = 72 + (y * 70 / height) as u8;
            let warm = 36 + (x * 32 / width) as u8;
            image.put_pixel(x, y, Rgba([24 + warm / 3, sky, 122 + warm / 2, 255]));
        }
    }
    for y in 214..height {
        for x in 0..width {
            let shade = 42 + ((x + y) % 38) as u8;
            image.put_pixel(x, y, Rgba([shade, 58 + shade / 3, 72 + shade / 2, 255]));
        }
    }
    for x in 0..width {
        let y = 180 + ((x as f32 / 32.0).sin() * 18.0) as i32;
        for dy in 0..34 {
            let row = y + dy;
            if (0..height as i32).contains(&row) {
                image.put_pixel(x, row as u32, Rgba([76, 96, 116, 255]));
            }
        }
    }
    image
}

fn character_image() -> RgbaImage {
    let width = 180;
    let height = 300;
    let mut image = RgbaImage::new(width, height);
    fill_ellipse(&mut image, 90, 76, 44, 50, Rgba([244, 204, 170, 255]));
    fill_ellipse(&mut image, 68, 66, 26, 34, Rgba([66, 45, 58, 255]));
    fill_ellipse(&mut image, 112, 66, 26, 34, Rgba([66, 45, 58, 255]));
    fill_rect(&mut image, 56, 126, 68, 136, Rgba([78, 118, 170, 255]));
    fill_rect(&mut image, 40, 160, 32, 92, Rgba([55, 82, 128, 255]));
    fill_rect(&mut image, 108, 160, 32, 92, Rgba([55, 82, 128, 255]));
    fill_rect(&mut image, 64, 262, 24, 34, Rgba([42, 50, 70, 255]));
    fill_rect(&mut image, 96, 262, 24, 34, Rgba([42, 50, 70, 255]));
    fill_ellipse(&mut image, 76, 82, 4, 5, Rgba([20, 22, 34, 255]));
    fill_ellipse(&mut image, 104, 82, 4, 5, Rgba([20, 22, 34, 255]));
    fill_rect(&mut image, 78, 106, 24, 3, Rgba([166, 74, 86, 255]));
    image
}

fn pulse_frames() -> Vec<RgbaImage> {
    [0_u8, 1, 2, 3]
        .into_iter()
        .map(|index| {
            let mut image = RgbaImage::new(PULSE_SIZE, PULSE_SIZE);
            let radius = 22 + u32::from(index) * 7;
            fill_ellipse(
                &mut image,
                48,
                48,
                radius as i32,
                radius as i32,
                Rgba([248, 214 - index * 24, 84 + index * 26, 210]),
            );
            fill_ellipse(&mut image, 48, 48, 12, 12, Rgba([255, 255, 255, 235]));
            image
        })
        .collect()
}

fn fill_rect(image: &mut RgbaImage, x: u32, y: u32, width: u32, height: u32, color: Rgba<u8>) {
    for row in y..(y + height).min(image.height()) {
        for col in x..(x + width).min(image.width()) {
            image.put_pixel(col, row, color);
        }
    }
}

fn fill_ellipse(image: &mut RgbaImage, cx: i32, cy: i32, rx: i32, ry: i32, color: Rgba<u8>) {
    for y in (cy - ry).max(0)..=(cy + ry).min(image.height() as i32 - 1) {
        for x in (cx - rx).max(0)..=(cx + rx).min(image.width() as i32 - 1) {
            let dx = (x - cx) as f32 / rx as f32;
            let dy = (y - cy) as f32 / ry as f32;
            if dx * dx + dy * dy <= 1.0 {
                image.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn write_png(path: &Path, image: &RgbaImage) -> Result<(), Box<dyn Error>> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ExtendedColorType::Rgba8,
    )?;
    fs::write(path, bytes)?;
    Ok(())
}

fn write_gif(path: &Path, frames: &[RgbaImage]) -> Result<(), Box<dyn Error>> {
    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        encoder.set_repeat(Repeat::Infinite)?;
        for frame in frames {
            encoder.encode_frame(Frame::from_parts(
                frame.clone(),
                0,
                0,
                Delay::from_numer_denom_ms(90, 1),
            ))?;
        }
    }
    fs::write(path, bytes)?;
    Ok(())
}

fn animated_webp(frames: &[RgbaImage]) -> Result<Vec<u8>, Box<dyn Error>> {
    let width = frames.first().ok_or("animated webp requires frames")?.width();
    let height = frames[0].height();
    let frame_chunks = frames
        .iter()
        .map(|frame| {
            let static_webp = static_webp(frame)?;
            extract_chunk(&static_webp, b"VP8L").ok_or_else(|| "encoded WebP did not contain VP8L".into())
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;

    let mut chunks = Vec::new();
    let mut vp8x = Vec::new();
    vp8x.extend_from_slice(&[0x12, 0, 0, 0]);
    write_u24(&mut vp8x, width - 1);
    write_u24(&mut vp8x, height - 1);
    write_chunk(&mut chunks, b"VP8X", &vp8x);

    let mut anim = Vec::new();
    anim.extend_from_slice(&[0, 0, 0, 0]);
    anim.extend_from_slice(&0_u16.to_le_bytes());
    write_chunk(&mut chunks, b"ANIM", &anim);

    for chunk in frame_chunks {
        let mut frame = Vec::new();
        write_u24(&mut frame, 0);
        write_u24(&mut frame, 0);
        write_u24(&mut frame, width - 1);
        write_u24(&mut frame, height - 1);
        write_u24(&mut frame, 90);
        frame.push(0b0000_0010);
        write_chunk(&mut frame, b"VP8L", &chunk);
        write_chunk(&mut chunks, b"ANMF", &frame);
    }

    let riff_size = u32::try_from(chunks.len() + 4)?;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(&chunks);
    Ok(out)
}

fn static_webp(image: &RgbaImage) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    WebPEncoder::new_lossless(&mut bytes).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ExtendedColorType::Rgba8,
    )?;
    Ok(bytes)
}

fn extract_chunk(bytes: &[u8], fourcc: &[u8; 4]) -> Option<Vec<u8>> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    let mut offset = 12;
    while offset + 8 <= bytes.len() {
        let name = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
        let start = offset + 8;
        let end = start.checked_add(size)?;
        if end > bytes.len() {
            return None;
        }
        if name == fourcc {
            return Some(bytes[start..end].to_vec());
        }
        offset = end + (size % 2);
    }
    None
}

fn write_chunk(out: &mut Vec<u8>, fourcc: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(fourcc);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        out.push(0);
    }
}

fn write_u24(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes()[..3]);
}

fn validate_static(format: ImageFormat, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let decoded = decode_image_bytes(format, bytes, ImageDecodeOptions::default())?;
    if decoded.is_animated() {
        return Err("expected static image".into());
    }
    Ok(())
}

fn validate_animation(format: ImageFormat, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let decoded = decode_image_bytes(format, bytes, ImageDecodeOptions::default())?;
    if !decoded.is_animated() || decoded.frames().len() < 2 {
        return Err("expected animated image".into());
    }
    Ok(())
}
