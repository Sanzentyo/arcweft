use arcweft_layout::stage_placement::ResolvedStagePlacement;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::image::{ImageObjectAlignment, ImageObjectFit, ImageObjectTransform};
use num_traits::ToPrimitive;

/// One decoded RGBA image frame ready for GPU upload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderImageFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One textured image quad in logical viewport coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderImage {
    pub id: String,
    pub frame: RenderImageFrame,
    pub bounds: HitRect,
    pub placement: Option<ResolvedStagePlacement>,
    pub fit: ImageObjectFit,
    pub alignment: ImageObjectAlignment,
    pub transform: ImageObjectTransform,
    pub opacity_milli: u16,
}

/// Resolved textured image quad before viewport normalization.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderImageQuad {
    pub rect: HitRect,
    pub uv_left: f32,
    pub uv_top: f32,
    pub uv_right: f32,
    pub uv_bottom: f32,
}

/// Resolved image transform matrix in renderer `f32` coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderImageTransformMatrix {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub tx: f32,
    pub ty: f32,
}

impl RenderImage {
    #[must_use]
    pub fn quad(&self) -> RenderImageQuad {
        let bounds = self.bounds;
        let source_width = self.frame.width.max(1).to_f32().unwrap_or(f32::MAX);
        let source_height = self.frame.height.max(1).to_f32().unwrap_or(f32::MAX);
        let align_x = alignment_factor(self.alignment.x_milli());
        let align_y = alignment_factor(self.alignment.y_milli());
        match self.fit {
            ImageObjectFit::Stretch => RenderImageQuad {
                rect: bounds,
                uv_left: 0.0,
                uv_top: 0.0,
                uv_right: 1.0,
                uv_bottom: 1.0,
            },
            ImageObjectFit::Intrinsic => {
                aligned_image_quad(bounds, source_width, source_height, align_x, align_y)
            }
            ImageObjectFit::Contain => {
                let scale = (bounds.width / source_width)
                    .min(bounds.height / source_height)
                    .max(f32::EPSILON);
                aligned_image_quad(
                    bounds,
                    source_width * scale,
                    source_height * scale,
                    align_x,
                    align_y,
                )
            }
            ImageObjectFit::Cover => {
                cover_image_quad(bounds, source_width, source_height, align_x, align_y)
            }
        }
    }

    #[must_use]
    pub fn transform_matrix(&self) -> RenderImageTransformMatrix {
        let transform = self.transform;
        RenderImageTransformMatrix {
            m11: milli_i32_to_f32(transform.m11_milli),
            m12: milli_i32_to_f32(transform.m12_milli),
            m21: milli_i32_to_f32(transform.m21_milli),
            m22: milli_i32_to_f32(transform.m22_milli),
            tx: milli_i32_to_f32(transform.tx_milli),
            ty: milli_i32_to_f32(transform.ty_milli),
        }
    }

    #[must_use]
    pub fn transform_point(&self, x: f32, y: f32) -> [f32; 2] {
        let transform = self.transform_matrix();
        [
            transform.m11 * x + transform.m12 * y + transform.tx,
            transform.m21 * x + transform.m22 * y + transform.ty,
        ]
    }
}

fn aligned_image_quad(
    bounds: HitRect,
    width: f32,
    height: f32,
    align_x: f32,
    align_y: f32,
) -> RenderImageQuad {
    RenderImageQuad {
        rect: HitRect::new(
            bounds.x + (bounds.width - width) * align_x,
            bounds.y + (bounds.height - height) * align_y,
            width,
            height,
        ),
        uv_left: 0.0,
        uv_top: 0.0,
        uv_right: 1.0,
        uv_bottom: 1.0,
    }
}

fn cover_image_quad(
    bounds: HitRect,
    source_width: f32,
    source_height: f32,
    align_x: f32,
    align_y: f32,
) -> RenderImageQuad {
    let source_ratio = source_width / source_height;
    let target_ratio = bounds.width / bounds.height;
    if source_ratio > target_ratio {
        let visible_width = (target_ratio / source_ratio).clamp(0.0, 1.0);
        let uv_left = (1.0 - visible_width) * align_x;
        RenderImageQuad {
            rect: bounds,
            uv_left,
            uv_top: 0.0,
            uv_right: uv_left + visible_width,
            uv_bottom: 1.0,
        }
    } else {
        let visible_height = (source_ratio / target_ratio).clamp(0.0, 1.0);
        let uv_top = (1.0 - visible_height) * align_y;
        RenderImageQuad {
            rect: bounds,
            uv_left: 0.0,
            uv_top,
            uv_right: 1.0,
            uv_bottom: uv_top + visible_height,
        }
    }
}

fn alignment_factor(milli: i32) -> f32 {
    milli.clamp(0, 1_000).to_f32().unwrap_or(0.0) / 1_000.0
}

fn milli_i32_to_f32(value: i32) -> f32 {
    value.to_f32().unwrap_or(0.0) / 1_000.0
}
