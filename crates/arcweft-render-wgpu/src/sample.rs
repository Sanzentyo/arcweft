//! Deterministic visual sample assets for native/WebGPU parity checks.

use crate::convert::{nonnegative_alpha_byte, saturating_u32_as_f32, saturating_u64_as_f32};
use crate::geometry::{RenderImage, RenderImageFrame, RenderViewport};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::image::{ImageObjectAlignment, ImageObjectFit, ImageObjectTransform};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DemoImageKind {
    Background,
    CharacterStand,
    GifPulse,
    WebPPulse,
}

impl DemoImageKind {
    pub const fn asset_id(self) -> &'static str {
        match self {
            Self::Background => "asset.generated.background",
            Self::CharacterStand => "asset.generated.character_stand",
            Self::GifPulse => "asset.generated.gif_pulse",
            Self::WebPPulse => "asset.generated.webp_pulse",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DemoAnimationClock {
    pub elapsed_millis: u64,
}

impl DemoAnimationClock {
    pub const fn from_millis(elapsed_millis: u64) -> Self {
        Self { elapsed_millis }
    }

    const fn frame_index(self, frame_millis: u64, frame_count: u64) -> u64 {
        if frame_millis == 0 || frame_count == 0 {
            0
        } else {
            (self.elapsed_millis / frame_millis) % frame_count
        }
    }
}

pub fn generated_demo_images(
    viewport: RenderViewport,
    clock: DemoAnimationClock,
) -> Vec<RenderImage> {
    let width = viewport.logical_width;
    let height = viewport.logical_height;
    vec![
        RenderImage {
            id: DemoImageKind::Background.asset_id().to_owned(),
            frame: gradient_background_frame(),
            bounds: HitRect::new(0.0, 0.0, width, height),
            fit: ImageObjectFit::Cover,
            alignment: ImageObjectAlignment::center(),
            transform: ImageObjectTransform::identity(),
            opacity_milli: 1_000,
        },
        RenderImage {
            id: DemoImageKind::CharacterStand.asset_id().to_owned(),
            frame: character_stand_frame(),
            bounds: HitRect::new(width * 0.61, height * 0.14, width * 0.22, height * 0.62),
            fit: ImageObjectFit::Contain,
            alignment: ImageObjectAlignment::center(),
            transform: ImageObjectTransform::identity(),
            opacity_milli: 980,
        },
        RenderImage {
            id: DemoImageKind::GifPulse.asset_id().to_owned(),
            frame: pulse_frame(clock.frame_index(160, 4), [255, 111, 88, 255]),
            bounds: HitRect::new(width * 0.08, height * 0.12, 76.0, 76.0),
            fit: ImageObjectFit::Stretch,
            alignment: ImageObjectAlignment::center(),
            transform: ImageObjectTransform::identity(),
            opacity_milli: 950,
        },
        RenderImage {
            id: DemoImageKind::WebPPulse.asset_id().to_owned(),
            frame: pulse_frame(clock.frame_index(130, 5), [98, 205, 255, 255]),
            bounds: HitRect::new(width * 0.17, height * 0.16, 68.0, 68.0),
            fit: ImageObjectFit::Stretch,
            alignment: ImageObjectAlignment::center(),
            transform: ImageObjectTransform::identity(),
            opacity_milli: 920,
        },
    ]
}

fn gradient_background_frame() -> RenderImageFrame {
    const WIDTH: u32 = 96;
    const HEIGHT: u32 = 54;
    let mut rgba = Vec::with_capacity(usize::try_from(WIDTH * HEIGHT * 4).unwrap_or(0));
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let horizon = u8::try_from(x * 90 / WIDTH).unwrap_or(0);
            let dusk = u8::try_from(y * 80 / HEIGHT).unwrap_or(0);
            rgba.extend([12 + horizon / 5, 28 + dusk / 3, 58 + horizon / 2, 255]);
        }
    }
    RenderImageFrame {
        width: WIDTH,
        height: HEIGHT,
        rgba,
    }
}

fn character_stand_frame() -> RenderImageFrame {
    const WIDTH: u32 = 72;
    const HEIGHT: u32 = 128;
    let width = saturating_u32_as_f32(WIDTH);
    let height = saturating_u32_as_f32(HEIGHT);
    let mut rgba = vec![0; usize::try_from(WIDTH * HEIGHT * 4).unwrap_or(0)];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let xf = saturating_u32_as_f32(x);
            let yf = saturating_u32_as_f32(y);
            let dx = xf - width * 0.5;
            let body_y = yf - height * 0.55;
            let head = dx * dx / 360.0 + (yf - 28.0).powi(2) / 420.0 <= 1.0;
            let body = dx.abs() < 18.0 + body_y.max(0.0) * 0.04 && y > 45 && y < HEIGHT - 10;
            let hair = dx.abs() < 23.0 && y > 12 && y < 50;
            let pixel = usize::try_from((y * WIDTH + x) * 4).unwrap_or(0);
            if hair {
                rgba[pixel..pixel + 4].copy_from_slice(&[33, 39, 78, 245]);
            }
            if body {
                rgba[pixel..pixel + 4].copy_from_slice(&[82, 136, 178, 238]);
            }
            if head {
                rgba[pixel..pixel + 4].copy_from_slice(&[248, 209, 185, 255]);
            }
            if (x == 29 || x == 43) && (26..31).contains(&y) {
                rgba[pixel..pixel + 4].copy_from_slice(&[31, 37, 56, 255]);
            }
        }
    }
    RenderImageFrame {
        width: WIDTH,
        height: HEIGHT,
        rgba,
    }
}

fn pulse_frame(frame: u64, color: [u8; 4]) -> RenderImageFrame {
    const WIDTH: u32 = 32;
    const HEIGHT: u32 = 32;
    let radius = 7.0 + saturating_u64_as_f32(frame) * 2.2;
    let mut rgba = Vec::with_capacity(usize::try_from(WIDTH * HEIGHT * 4).unwrap_or(0));
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let dx = saturating_u32_as_f32(x) - 15.5;
            let dy = saturating_u32_as_f32(y) - 15.5;
            let distance = (dx * dx + dy * dy).sqrt();
            let alpha = if distance <= radius {
                color[3]
            } else if distance <= radius + 3.0 {
                nonnegative_alpha_byte((radius + 3.0 - distance) * 70.0)
            } else {
                0
            };
            rgba.extend([color[0], color[1], color[2], alpha]);
        }
    }
    RenderImageFrame {
        width: WIDTH,
        height: HEIGHT,
        rgba,
    }
}
