use arcweft_player_native::{NativePlayerCaptureMetadata, compile_source, run_headless};
use arcweft_render_native as native;
use clap::{Parser, ValueEnum};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(name = "arcweft-player-native")]
struct Args {
    /// Run without opening a native window.
    #[arg(long)]
    headless: bool,
    /// Emit JSON in headless mode.
    #[arg(long)]
    json: bool,
    /// Maximum runtime steps.
    #[arg(long, default_value_t = 64)]
    steps: usize,
    /// Capture the first resolved frame through native offscreen readback.
    #[arg(long, value_enum)]
    capture: Option<NativeCaptureFormat>,
    /// Capture output path for --capture.
    #[arg(long)]
    capture_out: Option<PathBuf>,
    /// Native capture width in pixels.
    #[arg(long, default_value_t = 960)]
    capture_width: u32,
    /// Native capture height in pixels.
    #[arg(long, default_value_t = 540)]
    capture_height: u32,
    /// Arcweft source file.
    path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum NativeCaptureFormat {
    Png,
    RawRgba,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args) -> Result<(), String> {
    let source = fs::read_to_string(&args.path)
        .map_err(|error| format!("failed to read source file: {error}"))?;
    let program = compile_source(&source).map_err(|error| error.to_string())?;
    if args.headless {
        let mut report = run_headless(program, args.steps).map_err(|error| error.to_string())?;
        if let Some(format) = args.capture {
            let capture_out = args
                .capture_out
                .as_ref()
                .ok_or_else(|| "--capture requires --capture-out".to_owned())?;
            let frame = report
                .frames
                .first()
                .ok_or_else(|| "no display frame was produced for native capture".to_owned())?;
            let capture =
                native::capture_frame_rgba(frame, args.capture_width, args.capture_height)
                    .map_err(|error| error.to_string())?;
            let bytes = native_capture_bytes(format, &capture)?;
            fs::write(capture_out, bytes)
                .map_err(|error| format!("failed to write capture: {error}"))?;
            report.native_capture = Some(NativePlayerCaptureMetadata {
                renderer: "native_offscreen_wgpu_glyphon".to_owned(),
                format: format.resource_name().to_owned(),
                width: capture.width,
                height: capture.height,
                pixel_format: "rgba8_unorm".to_owned(),
                row_stride_bytes: capture.width.saturating_mul(4),
                content_bbox: capture.content_bbox,
                content_pixels: capture.content_pixels,
                written: capture_out.display().to_string(),
            });
        }
        if args.json {
            serde_json::to_writer_pretty(std::io::stdout(), &report)
                .map_err(|error| format!("failed to write JSON: {error}"))?;
            println!();
        } else {
            for frame in &report.frames {
                println!("{} {}", frame.line.0, frame.text);
            }
        }
        return Ok(());
    }
    let report = run_headless(program, args.steps).map_err(|error| error.to_string())?;
    native::run_frames_window("Arcweft Player", &report.frames).map_err(|error| error.to_string())
}

fn native_capture_bytes(
    format: NativeCaptureFormat,
    capture: &native::NativeFrameCapture,
) -> Result<Vec<u8>, String> {
    match format {
        NativeCaptureFormat::RawRgba => Ok(capture.rgba.clone()),
        NativeCaptureFormat::Png => encode_png(capture),
    }
}

fn encode_png(capture: &native::NativeFrameCapture) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, capture.width, capture.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| format!("failed to encode PNG capture: {error}"))?;
        writer
            .write_image_data(&capture.rgba)
            .map_err(|error| format!("failed to encode PNG capture: {error}"))?;
        writer
            .finish()
            .map_err(|error| format!("failed to encode PNG capture: {error}"))?;
    }
    Ok(bytes)
}

impl NativeCaptureFormat {
    const fn resource_name(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::RawRgba => "raw-rgba",
        }
    }
}
