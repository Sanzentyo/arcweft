#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "capture-seq06-13e1-inset-shadow-native-frame"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
publish = false

[dependencies]
arcweft-player-web = { path = "../crates/arcweft-player-web" }
arcweft-render-wgpu = { path = "../crates/arcweft-render-wgpu" }
png = "0.18.1"
pollster = "0.4.0"
serde_json = "1"
wgpu = { version = "29.0.3", default-features = false, features = ["std", "wgsl", "dx12", "metal", "vulkan"] }

[patch.crates-io]
glyphon = { path = "../vendor/glyphon" }
---

/*
Captures the native seq06.13e.1 inset box-shadow compositor fixture.

This script writes only target/ evidence artifacts. It never promotes checked-in
goldens and must not be used to claim a pinned run unless the caller provides the
seq06.13e.1 pinned environment variables and keeps the command log.

cargo +nightly -Zscript tools/capture-seq06-13e1-inset-shadow-native-frame.rs --root . --out-dir target/seq06.13e.1-inset-box-shadow-golden
*/

use arcweft_player_web::parity::inset_shadow::{
    HEIGHT, InsetShadowExactStats, WIDTH, has_exact_card_pixels, prepare_inset_shadow_exact_frame,
};
use arcweft_render_wgpu::renderer::SharedRenderer;
use arcweft_render_wgpu::view_effects::ViewTextureExtent;
use png::{BitDepth, ColorType, Encoder};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{SystemTime, UNIX_EPOCH};

const REQUIRED_ENV: &str = "ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_REQUIRED";
const PINNED_ENV: &str = "ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED";
const NATIVE_BACKEND_ENV: &str = "ARW_SEQ06_13E1_INSET_SHADOW_NATIVE_BACKEND";
const NATIVE_BACKEND: &str = "wgpu_offscreen_compositor";

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse(env::args().skip(1))?;
    if args.help {
        print_help();
        return Ok(());
    }

    let root = args
        .root
        .canonicalize()
        .map_err(|error| format!("canonicalize {}: {error}", args.root.display()))?;
    let out_dir = if args.out_dir.is_absolute() {
        args.out_dir
    } else {
        root.join(args.out_dir)
    };
    let target_dir = out_dir.join("native");
    fs::create_dir_all(&target_dir)
        .map_err(|error| format!("create {}: {error}", target_dir.display()))?;

    let candidate = target_dir.join("seq06_13e1_inset_box_shadow.candidate.png");
    let observe = target_dir.join("seq06_13e1_inset_box_shadow.observe.json");

    let bundle_path = args
        .bundle
        .map_or_else(|| out_dir.join("fixture.awfb"), |path| root.join(path));
    let bytes = fs::read(&bundle_path)
        .map_err(|error| format!("read {}: {error}", bundle_path.display()))?;
    let capture = render_capture(args.target_format.wgpu(), &bytes)?;
    write_png(&candidate, WIDTH, HEIGHT, &capture.rgba)?;
    write_observe_json(&root, &candidate, &observe, &capture)?;
    println!(
        "wrote seq06.13e.1 native candidate {} and observe {}",
        candidate.display(),
        observe.display()
    );
    Ok(())
}

fn print_help() {
    println!(
        "capture-seq06-13e1-inset-shadow-native-frame\n\n\
         Usage:\n  cargo +nightly -Zscript tools/capture-seq06-13e1-inset-shadow-native-frame.rs --root . [--out-dir DIR] [--bundle FILE] [--target-format rgba8unorm|rgba8unorm-srgb]\n\n\
         Options:\n  --root <repo-root>\n  --out-dir <dir>        Default: target/seq06.13e.1-inset-box-shadow-golden\n  --bundle <file>        Default: fixture.awfb in the output directory\n  --target-format <fmt> Default: rgba8unorm\n  -h, --help            Print this help."
    );
}

#[derive(Clone, Debug)]
struct Args {
    root: PathBuf,
    out_dir: PathBuf,
    bundle: Option<PathBuf>,
    target_format: CaptureTargetFormat,
    help: bool,
}

impl Args {
    fn parse(values: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut root = None;
        let mut out_dir = PathBuf::from("target/seq06.13e.1-inset-box-shadow-golden");
        let mut bundle = None;
        let mut target_format = CaptureTargetFormat::Rgba8Unorm;
        let mut help = false;
        let mut values = values.peekable();

        while let Some(arg) = values.next() {
            match arg.as_str() {
                "--root" => root = Some(PathBuf::from(next_arg(&mut values, "--root")?)),
                "--out-dir" => out_dir = PathBuf::from(next_arg(&mut values, "--out-dir")?),
                "--bundle" => bundle = Some(PathBuf::from(next_arg(&mut values, "--bundle")?)),
                "--target-format" => {
                    target_format =
                        CaptureTargetFormat::parse(&next_arg(&mut values, "--target-format")?)?;
                }
                "--help" | "-h" => help = true,
                unknown => return Err(format!("unknown argument `{unknown}`")),
            }
        }

        Ok(Self {
            root: if help {
                root.unwrap_or_else(|| PathBuf::from("."))
            } else {
                root.ok_or_else(|| String::from("missing --root"))?
            },
            out_dir,
            bundle,
            target_format,
            help,
        })
    }
}

fn next_arg(
    values: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    name: &str,
) -> Result<String, String> {
    values
        .next()
        .ok_or_else(|| format!("{name} requires a value"))
}

#[derive(Clone, Copy, Debug)]
enum CaptureTargetFormat {
    Rgba8Unorm,
    Rgba8UnormSrgb,
}

impl CaptureTargetFormat {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "rgba8unorm" => Ok(Self::Rgba8Unorm),
            "rgba8unorm-srgb" | "rgba8unorm_srgb" => Ok(Self::Rgba8UnormSrgb),
            _ => Err(format!("unsupported target format `{value}`")),
        }
    }

    const fn wgpu(self) -> wgpu::TextureFormat {
        match self {
            Self::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            Self::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }
}

struct CaptureOutput {
    rgba: Vec<u8>,
    stats: InsetShadowExactStats,
    bundle_artifact: String,
    adapter: wgpu::AdapterInfo,
    content: ContentStats,
}

#[derive(Clone, Copy, Debug)]
struct ContentStats {
    non_transparent_pixels: u32,
    max_alpha: u8,
    max_channel: u8,
}

fn render_capture(format: wgpu::TextureFormat, bytes: &[u8]) -> Result<CaptureOutput, String> {
    let fixture = prepare_inset_shadow_exact_frame(bytes).map_err(|error| error.to_string())?;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .map_err(|error| format!("request native wgpu adapter: {error}"))?;
    let adapter_info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("arcweft-seq06-13e1-inset-shadow-capture"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|error| format!("request native wgpu device: {error}"))?;

    let extent = ViewTextureExtent::new(WIDTH, HEIGHT);
    let final_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("arcweft-seq06-13e1-inset-shadow-capture-target"),
        size: wgpu::Extent3d {
            width: extent.width,
            height: extent.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let final_view = final_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut renderer = SharedRenderer::new(&device, &queue, format);
    renderer
        .render_to_view(&device, &queue, &final_view, &fixture.frame)
        .map_err(|error| format!("render retained inset-shadow frame: {error}"))?;
    let rgba = readback_texture_rgba(&device, &queue, &final_texture, WIDTH, HEIGHT)?;
    if !has_exact_card_pixels(&rgba) {
        return Err("native exact PNG is missing one or both card fills".to_owned());
    }
    let content = content_stats(&rgba);
    Ok(CaptureOutput {
        rgba,
        stats: fixture.stats,
        bundle_artifact: fixture.bundle_identity.digest().to_hex(),
        adapter: adapter_info,
        content,
    })
}

fn readback_texture_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let padded_row_bytes = padded_rgba_row_bytes(width);
    let buffer_size = u64::from(padded_row_bytes).saturating_mul(u64::from(height));
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("arcweft-seq06-13e1-inset-shadow-readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("arcweft-seq06-13e1-inset-shadow-readback-encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (sender, receiver) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result.map_err(|error| error.to_string()));
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| format!("poll readback device: {error}"))?;
    receiver
        .recv()
        .map_err(|error| format!("receive readback map status: {error}"))?
        .map_err(|error| format!("map readback buffer: {error}"))?;

    let mapped = slice.get_mapped_range();
    let rgba = unpad_rgba_rows(&mapped, width, height, padded_row_bytes);
    drop(mapped);
    readback.unmap();
    Ok(rgba)
}

fn padded_rgba_row_bytes(width: u32) -> u32 {
    let row_bytes = width.saturating_mul(4);
    row_bytes.saturating_add(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1)
        / wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}

fn unpad_rgba_rows(mapped: &[u8], width: u32, height: u32, padded_row_bytes: u32) -> Vec<u8> {
    let row_bytes = usize::try_from(width.saturating_mul(4)).unwrap_or(0);
    let padded = usize::try_from(padded_row_bytes).unwrap_or(row_bytes);
    (0..usize::try_from(height).unwrap_or(0))
        .flat_map(|row| {
            let start = row.saturating_mul(padded);
            let end = start.saturating_add(row_bytes).min(mapped.len());
            mapped[start..end].iter().copied()
        })
        .collect()
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("write PNG header: {error}"))?;
    writer
        .write_image_data(rgba)
        .map_err(|error| format!("write PNG data: {error}"))?;
    writer
        .finish()
        .map_err(|error| format!("finish PNG: {error}"))?;
    fs::write(path, bytes).map_err(|error| format!("write {}: {error}", path.display()))
}

fn write_observe_json(
    root: &Path,
    candidate: &Path,
    observe: &Path,
    capture: &CaptureOutput,
) -> Result<(), String> {
    let mut json = String::new();
    writeln!(&mut json, "{{").unwrap();
    writeln!(
        &mut json,
        "  \"schema\": \"arcweft.seq06.13e1.inset_box_shadow.native_observe.v1\","
    )
    .unwrap();
    writeln!(
        &mut json,
        "  \"generated_unix_seconds\": {},",
        unix_seconds()
    )
    .unwrap();
    writeln!(&mut json, "  \"target\": \"native\",").unwrap();
    writeln!(&mut json, "  \"fixture\": \"seq06_13e1_inset_box_shadow\",").unwrap();
    writeln!(
        &mut json,
        "  \"candidate_png\": {},",
        json_string(&display_path(candidate))
    )
    .unwrap();
    writeln!(
        &mut json,
        "  \"bundle_artifact\": {},",
        json_string(&capture.bundle_artifact)
    )
    .unwrap();
    writeln!(&mut json, "  \"viewport\": {{").unwrap();
    writeln!(&mut json, "    \"width\": {WIDTH},").unwrap();
    writeln!(&mut json, "    \"height\": {HEIGHT},").unwrap();
    writeln!(&mut json, "    \"device_pixel_ratio\": 1.0").unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(&mut json, "  \"route\": [").unwrap();
    let route = [
        "BundleSession::from_awfb_bytes",
        "PlayerFramePlanner::prepare",
        "SharedRenderer::render_to_view",
        "ViewCompositingEffects::box_shadows",
        "ViewBoxShadowPassPlan unified outer/inset pass list",
        "ViewCompositor::render_group",
        "PASS_BOX_SHADOW WGSL kind flag",
    ];
    for (index, label) in route.iter().enumerate() {
        let comma = if index + 1 == route.len() { "" } else { "," };
        writeln!(&mut json, "    {}{comma}", json_string(label)).unwrap();
    }
    writeln!(&mut json, "  ],").unwrap();
    writeln!(&mut json, "  \"scene\": {{").unwrap();
    writeln!(
        &mut json,
        "    \"cards\": [\"rounded_inset_shadow_card\", \"mixed_outer_inset_shadow_card\"],"
    )
    .unwrap();
    writeln!(&mut json, "    \"box_shadow\": \"outer and inset\",").unwrap();
    writeln!(&mut json, "    \"inset\": true").unwrap();
    writeln!(&mut json, "  }},").unwrap();
    let stats = serde_json::to_string(&capture.stats).map_err(|error| error.to_string())?;
    writeln!(&mut json, "  \"stats\": {stats},").unwrap();
    writeln!(&mut json, "  \"content\": {{").unwrap();
    writeln!(
        &mut json,
        "    \"non_transparent_pixels\": {},",
        capture.content.non_transparent_pixels
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"max_alpha\": {},",
        capture.content.max_alpha
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"max_channel\": {}",
        capture.content.max_channel
    )
    .unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(&mut json, "  \"adapter\": {{").unwrap();
    writeln!(
        &mut json,
        "    \"name\": {},",
        json_string(&capture.adapter.name)
    )
    .unwrap();
    writeln!(&mut json, "    \"vendor\": {},", capture.adapter.vendor).unwrap();
    writeln!(&mut json, "    \"device\": {},", capture.adapter.device).unwrap();
    writeln!(
        &mut json,
        "    \"device_type\": {},",
        json_string(&format!("{:?}", capture.adapter.device_type))
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"backend\": {},",
        json_string(&format!("{:?}", capture.adapter.backend))
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"driver\": {},",
        json_string(&capture.adapter.driver)
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"driver_info\": {}",
        json_string(&capture.adapter.driver_info)
    )
    .unwrap();
    writeln!(&mut json, "  }},").unwrap();
    writeln!(&mut json, "  \"environment\": {{").unwrap();
    writeln!(
        &mut json,
        "    \"required\": {},",
        env_present(REQUIRED_ENV)
    )
    .unwrap();
    writeln!(&mut json, "    \"pinned\": {},", env_present(PINNED_ENV)).unwrap();
    writeln!(
        &mut json,
        "    \"native_backend_env\": {},",
        json_option(env::var(NATIVE_BACKEND_ENV).ok().as_deref())
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"native_backend_expected\": {},",
        json_string(NATIVE_BACKEND)
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"arcweft_commit\": {},",
        json_option(
            command_stdout(
                Command::new("git")
                    .arg("-C")
                    .arg(display_path(root))
                    .arg("rev-parse")
                    .arg("HEAD")
            )
            .as_deref()
        )
    )
    .unwrap();
    writeln!(
        &mut json,
        "    \"arcweft_dirty\": {}",
        command_stdout(
            Command::new("git")
                .arg("-C")
                .arg(display_path(root))
                .arg("status")
                .arg("--short")
        )
        .map_or(String::from("null"), |status| (!status.is_empty())
            .to_string())
    )
    .unwrap();
    writeln!(&mut json, "  }}").unwrap();
    writeln!(&mut json, "}}").unwrap();
    fs::write(observe, json).map_err(|error| format!("write {}: {error}", observe.display()))
}

fn content_stats(rgba: &[u8]) -> ContentStats {
    rgba.chunks_exact(4).fold(
        ContentStats {
            non_transparent_pixels: 0,
            max_alpha: 0,
            max_channel: 0,
        },
        |mut stats, pixel| {
            let alpha = pixel[3];
            if alpha != 0 {
                stats.non_transparent_pixels = stats.non_transparent_pixels.saturating_add(1);
            }
            stats.max_alpha = stats.max_alpha.max(alpha);
            stats.max_channel = stats.max_channel.max(pixel[0]).max(pixel[1]).max(pixel[2]);
            stats
        },
    )
}

fn env_present(name: &str) -> bool {
    env::var_os(name).is_some()
}

fn command_stdout(command: &mut Command) -> Option<String> {
    let output = command.stderr(Stdio::null()).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn json_string(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn json_option(value: Option<&str>) -> String {
    value.map_or_else(|| String::from("null"), json_string)
}

fn display_path(path: &Path) -> String {
    let text = path.display().to_string();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned()
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => write!(&mut escaped, "\\u{:04x}", c as u32).unwrap(),
            c => escaped.push(c),
        }
    }
    escaped
}
