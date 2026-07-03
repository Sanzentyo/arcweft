//! Packed uniform contract for the shared UI compositor WGSL shader.

use crate::ui_blend::UiBlendShaderMode;
use crate::ui_clip_path::{MAX_CLIP_POLYGON_VERTICES, UiClipGeometryPlan, UiClipVertex};
use crate::ui_effects::{UiBlurDirection, UiColorMatrix, UiTextureExtent};
use crate::ui_mask::{UiMaskChannel, UiMaskSamplingPlan};
use crate::ui_scene::{UiColorRgba8, UiFillRule};
use bytemuck::{Pod, Zeroable};
use num_traits::ToPrimitive;

const PASS_COMPOSITE: u32 = 0;
const PASS_COLOR_MATRIX: u32 = 1;
const PASS_BLUR: u32 = 2;
const PASS_DROP_SHADOW: u32 = 3;
const PASS_MASK: u32 = 4;
const PASS_BLEND: u32 = 5;
const PASS_CLIP: u32 = 6;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct UiCompositorUniform {
    matrix: [[f32; 4]; 4],
    offset: [f32; 4],
    params0: [f32; 4],
    params1: [f32; 4],
    params2: [f32; 4],
    clip_vertices: [[f32; 4]; MAX_CLIP_POLYGON_VERTICES],
    pass_kind: u32,
    _padding: [u32; 3],
}

impl UiCompositorUniform {
    pub(crate) fn composite(opacity: f32, blend: UiBlendShaderMode) -> Self {
        Self {
            params0: [opacity.clamp(0.0, 1.0), shader_mode_to_f32(blend), 0.0, 0.0],
            pass_kind: if blend == UiBlendShaderMode::Normal {
                PASS_COMPOSITE
            } else {
                PASS_BLEND
            },
            ..Self::from_matrix(UiColorMatrix::identity())
        }
    }

    pub(crate) fn color_matrix(matrix: UiColorMatrix) -> Self {
        Self {
            pass_kind: PASS_COLOR_MATRIX,
            ..Self::from_matrix(matrix)
        }
    }

    pub(crate) fn blur(
        direction: UiBlurDirection,
        radius_px: f32,
        extent: UiTextureExtent,
    ) -> Self {
        let (step_x, step_y) = match direction {
            UiBlurDirection::Horizontal => (1.0 / dimension_to_f32(extent.width), 0.0),
            UiBlurDirection::Vertical => (0.0, 1.0 / dimension_to_f32(extent.height)),
        };
        Self {
            params0: [step_x, step_y, radius_px.max(0.0), 0.0],
            pass_kind: PASS_BLUR,
            ..Self::from_matrix(UiColorMatrix::identity())
        }
    }

    pub(crate) fn drop_shadow(
        horizontal_offset_px: f32,
        vertical_offset_px: f32,
        blur_radius_px: f32,
        tint: UiColorRgba8,
        extent: UiTextureExtent,
    ) -> Self {
        Self {
            params0: [
                horizontal_offset_px / dimension_to_f32(extent.width),
                vertical_offset_px / dimension_to_f32(extent.height),
                blur_radius_px.max(0.0),
                0.0,
            ],
            params1: [
                f32::from(tint.red) / 255.0,
                f32::from(tint.green) / 255.0,
                f32::from(tint.blue) / 255.0,
                f32::from(tint.alpha) / 255.0,
            ],
            pass_kind: PASS_DROP_SHADOW,
            ..Self::from_matrix(UiColorMatrix::identity())
        }
    }

    pub(crate) fn mask(
        channel: UiMaskChannel,
        sampling: UiMaskSamplingPlan,
        source_extent: UiTextureExtent,
    ) -> Self {
        Self {
            params0: [
                match channel {
                    UiMaskChannel::Alpha => 0.0,
                    UiMaskChannel::Luminance => 1.0,
                },
                f32::from(u8::from(sampling.repeat_x)),
                f32::from(u8::from(sampling.repeat_y)),
                0.0,
            ],
            params1: [
                sampling.tile_origin_px[0],
                sampling.tile_origin_px[1],
                sampling.tile_size_px[0],
                sampling.tile_size_px[1],
            ],
            params2: [
                dimension_to_f32(source_extent.width),
                dimension_to_f32(source_extent.height),
                0.0,
                0.0,
            ],
            pass_kind: PASS_MASK,
            ..Self::from_matrix(UiColorMatrix::identity())
        }
    }

    pub(crate) fn clip(
        plan: &UiClipGeometryPlan,
        source_extent: UiTextureExtent,
        origin_logical: [f32; 2],
    ) -> Self {
        let mut uniform = Self {
            params1: [origin_logical[0], origin_logical[1], 0.0, 0.0],
            params2: [
                dimension_to_f32(source_extent.width),
                dimension_to_f32(source_extent.height),
                0.0,
                0.0,
            ],
            pass_kind: PASS_CLIP,
            ..Self::from_matrix(UiColorMatrix::identity())
        };
        match plan {
            UiClipGeometryPlan::None => {}
            UiClipGeometryPlan::Inset { rect, radii_px } => {
                uniform.params0 = [1.0, 0.0, 0.0, 0.0];
                uniform.matrix[0] = [rect.x, rect.y, rect.width, rect.height];
                uniform.matrix[1] = *radii_px;
            }
            UiClipGeometryPlan::Ellipse {
                center,
                radius_x_px,
                radius_y_px,
            } => {
                uniform.params0 = [2.0, 0.0, 0.0, 0.0];
                uniform.matrix[0] = [center.x, center.y, *radius_x_px, *radius_y_px];
            }
            UiClipGeometryPlan::Polygon {
                fill_rule,
                vertices,
            } => {
                uniform.params0 = [
                    3.0,
                    fill_rule_to_f32(*fill_rule),
                    vertices.len().to_f32().unwrap_or(0.0),
                    0.0,
                ];
                for (index, vertex) in vertices.iter().copied().enumerate() {
                    uniform.clip_vertices[index] = clip_vertex_uniform(vertex);
                }
            }
        }
        uniform
    }

    fn from_matrix(matrix: UiColorMatrix) -> Self {
        Self {
            matrix: matrix.matrix,
            offset: matrix.offset,
            params0: [0.0; 4],
            params1: [0.0; 4],
            params2: [0.0; 4],
            clip_vertices: [[0.0; 4]; MAX_CLIP_POLYGON_VERTICES],
            pass_kind: PASS_COMPOSITE,
            _padding: [0; 3],
        }
    }
}

fn dimension_to_f32(value: u32) -> f32 {
    value.max(1).to_f32().unwrap_or(f32::MAX)
}

fn shader_mode_to_f32(mode: UiBlendShaderMode) -> f32 {
    mode.as_shader_u32().to_f32().unwrap_or(0.0)
}

fn fill_rule_to_f32(fill_rule: UiFillRule) -> f32 {
    match fill_rule {
        UiFillRule::NonZero => 0.0,
        UiFillRule::EvenOdd => 1.0,
    }
}

fn clip_vertex_uniform(vertex: UiClipVertex) -> [f32; 4] {
    [vertex.x, vertex.y, 0.0, 0.0]
}
