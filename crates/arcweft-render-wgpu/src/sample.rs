//! Deterministic visual sample assets for native/WebGPU parity checks.

use crate::geometry::{RenderImage, RenderImageFrame, RenderViewport};
use arcweft_presentation::hit::HitRect;

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
            opacity_milli: 1_000,
        },
        RenderImage {
            id: DemoImageKind::CharacterStand.asset_id().to_owned(),
            frame: character_stand_frame(),
            bounds: HitRect::new(width * 0.61, height * 0.14, width * 0.22, height * 0.62),
            opacity_milli: 980,
        },
        RenderImage {
            id: DemoImageKind::GifPulse.asset_id().to_owned(),
            frame: pulse_frame(clock.frame_index(160, 4), [255, 111, 88, 255]),
            bounds: HitRect::new(width * 0.08, height * 0.12, 76.0, 76.0),
            opacity_milli: 950,
        },
        RenderImage {
            id: DemoImageKind::WebPPulse.asset_id().to_owned(),
            frame: pulse_frame(clock.frame_index(130, 5), [98, 205, 255, 255]),
            bounds: HitRect::new(width * 0.17, height * 0.16, 68.0, 68.0),
            opacity_milli: 920,
        },
    ]
}

fn gradient_background_frame() -> RenderImageFrame {
    let width = 96;
    let height = 54;
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let horizon = (x * 90 / width) as u8;
            let dusk = (y * 80 / height) as u8;
            rgba.extend([12 + horizon / 5, 28 + dusk / 3, 58 + horizon / 2, 255]);
        }
    }
    RenderImageFrame {
        width: width as u32,
        height: height as u32,
        rgba,
    }
}

fn character_stand_frame() -> RenderImageFrame {
    let width = 72;
    let height = 128;
    let mut rgba = vec![0; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - width as f32 * 0.5;
            let body_y = y as f32 - height as f32 * 0.55;
            let head = dx * dx / 360.0 + (y as f32 - 28.0).powi(2) / 420.0 <= 1.0;
            let body = dx.abs() < 18.0 + body_y.max(0.0) * 0.04 && y > 45 && y < height - 10;
            let hair = dx.abs() < 23.0 && y > 12 && y < 50;
            let pixel = (y * width + x) * 4;
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
        width: width as u32,
        height: height as u32,
        rgba,
    }
}

fn pulse_frame(frame: u64, color: [u8; 4]) -> RenderImageFrame {
    let width = 32;
    let height = 32;
    let radius = 7.0 + frame as f32 * 2.2;
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - 15.5;
            let dy = y as f32 - 15.5;
            let distance = (dx * dx + dy * dy).sqrt();
            let alpha = if distance <= radius {
                color[3]
            } else if distance <= radius + 3.0 {
                ((radius + 3.0 - distance) * 70.0) as u8
            } else {
                0
            };
            rgba.extend([color[0], color[1], color[2], alpha]);
        }
    }
    RenderImageFrame {
        width: width as u32,
        height: height as u32,
        rgba,
    }
}
