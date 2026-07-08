//! Packed uniform contract for the shared View compositor WGSL shader.

use crate::view_blend::ViewBlendShaderMode;
use crate::view_box_shadow::ViewBoxShadowPass;
use crate::view_clip_path::{
    MAX_CLIP_PATH_EDGES, ViewClipGeometryPlan, ViewClipPathEdge, ViewClipVertex,
};
use crate::view_effects::{ViewBlurDirection, ViewColorMatrix, ViewTextureExtent};
use crate::view_mask::{
    MAX_MASK_GRADIENT_STOPS, ViewMaskAxisRepeat, ViewMaskChannel, ViewMaskGradientKind,
    ViewMaskGradientPlan, ViewMaskSamplingPlan,
};
use crate::view_scene::{ViewBoxShadowKind, ViewBoxShadowRadii, ViewColorRgba8, ViewFillRule};
use bytemuck::{Pod, Zeroable};
use num_traits::ToPrimitive;

const PASS_COMPOSITE: u32 = 0;
const PASS_COLOR_MATRIX: u32 = 1;
const PASS_BLUR: u32 = 2;
const PASS_DROP_SHADOW: u32 = 3;
const PASS_MASK: u32 = 4;
const PASS_BLEND: u32 = 5;
const PASS_CLIP: u32 = 6;
const PASS_BOX_SHADOW: u32 = 7;
const PASS_MASK_GRADIENT: u32 = 8;
const PASS_CLIPPED_COMPOSITE: u32 = 9;
const OUTPUT_ENCODING_LINEAR: u32 = 0;
const OUTPUT_ENCODING_SRGB: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct ViewCompositorUniform {
    matrix: [[f32; 4]; 4],
    offset: [f32; 4],
    params0: [f32; 4],
    params1: [f32; 4],
    params2: [f32; 4],
    clip_vertices: [[f32; 4]; MAX_CLIP_PATH_EDGES],
    gradient_stops: [[f32; 4]; MAX_MASK_GRADIENT_STOPS],
    pass_kind: u32,
    output_encoding: u32,
    _padding: [u32; 2],
}

impl ViewCompositorUniform {
    pub(crate) fn composite(opacity: f32, blend: ViewBlendShaderMode) -> Self {
        Self {
            params0: [opacity.clamp(0.0, 1.0), shader_mode_to_f32(blend), 0.0, 0.0],
            pass_kind: if blend == ViewBlendShaderMode::Normal {
                PASS_COMPOSITE
            } else {
                PASS_BLEND
            },
            ..Self::from_matrix(ViewColorMatrix::identity())
        }
    }

    pub(crate) fn composite_to_final_target(
        opacity: f32,
        blend: ViewBlendShaderMode,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let mut uniform = Self::composite(opacity, blend);
        uniform.output_encoding = ViewOutputEncoding::for_target_format(target_format).as_uniform();
        uniform
    }

    pub(crate) fn clipped_composite(
        opacity: f32,
        blend: ViewBlendShaderMode,
        rect_logical: [f32; 4],
        target_logical_extent: [f32; 2],
    ) -> Self {
        Self {
            params0: [opacity.clamp(0.0, 1.0), shader_mode_to_f32(blend), 0.0, 0.0],
            params1: rect_logical,
            params2: [
                target_logical_extent[0].max(0.0001),
                target_logical_extent[1].max(0.0001),
                0.0,
                0.0,
            ],
            pass_kind: if blend == ViewBlendShaderMode::Normal {
                PASS_CLIPPED_COMPOSITE
            } else {
                PASS_BLEND
            },
            ..Self::from_matrix(ViewColorMatrix::identity())
        }
    }

    pub(crate) fn color_matrix(matrix: ViewColorMatrix) -> Self {
        Self {
            pass_kind: PASS_COLOR_MATRIX,
            ..Self::from_matrix(matrix)
        }
    }

    pub(crate) fn blur(
        direction: ViewBlurDirection,
        radius_px: f32,
        extent: ViewTextureExtent,
    ) -> Self {
        let (step_x, step_y) = match direction {
            ViewBlurDirection::Horizontal => (1.0 / dimension_to_f32(extent.width), 0.0),
            ViewBlurDirection::Vertical => (0.0, 1.0 / dimension_to_f32(extent.height)),
        };
        Self {
            params0: [step_x, step_y, radius_px.max(0.0), 0.0],
            pass_kind: PASS_BLUR,
            ..Self::from_matrix(ViewColorMatrix::identity())
        }
    }

    pub(crate) fn drop_shadow(
        horizontal_offset_px: f32,
        vertical_offset_px: f32,
        blur_radius_px: f32,
        tint: ViewColorRgba8,
        extent: ViewTextureExtent,
    ) -> Self {
        Self {
            params0: [
                horizontal_offset_px / dimension_to_f32(extent.width),
                vertical_offset_px / dimension_to_f32(extent.height),
                blur_radius_px.max(0.0),
                0.0,
            ],
            params1: rgba_to_unit(tint),
            pass_kind: PASS_DROP_SHADOW,
            ..Self::from_matrix(ViewColorMatrix::identity())
        }
    }

    pub(crate) fn mask(
        channel: ViewMaskChannel,
        sampling: ViewMaskSamplingPlan,
        source_extent: ViewTextureExtent,
    ) -> Self {
        Self {
            params0: [
                mask_channel_to_f32(channel),
                repeat_mode_to_f32(sampling.repeat_mode_x),
                repeat_mode_to_f32(sampling.repeat_mode_y),
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
                sampling.tile_stride_px[0],
                sampling.tile_stride_px[1],
            ],
            offset: [
                sampling.tile_count[0].to_f32().unwrap_or(0.0),
                sampling.tile_count[1].to_f32().unwrap_or(0.0),
                0.0,
                0.0,
            ],
            pass_kind: PASS_MASK,
            ..Self::from_matrix(ViewColorMatrix::identity())
        }
    }

    pub(crate) fn gradient_mask(
        channel: ViewMaskChannel,
        sampling: ViewMaskSamplingPlan,
        gradient: &ViewMaskGradientPlan,
        source_extent: ViewTextureExtent,
    ) -> Self {
        let mut uniform = Self::mask(channel, sampling, source_extent);
        uniform.pass_kind = PASS_MASK_GRADIENT;
        uniform.params0[3] = gradient_kind_to_f32(gradient.kind);
        uniform.matrix[0] = gradient_header_0(gradient.kind);
        uniform.matrix[1] = gradient_header_1(gradient.kind, gradient.stops.len());
        for (index, stop) in gradient.stops.iter().copied().enumerate() {
            uniform.gradient_stops[index] = [
                stop.offset,
                stop.alpha_coverage,
                stop.luminance_coverage,
                0.0,
            ];
        }
        uniform
    }

    pub(crate) fn clip(
        plan: &ViewClipGeometryPlan,
        logical_extent: [f32; 2],
        origin_logical: [f32; 2],
    ) -> Self {
        let mut uniform = Self {
            params1: [origin_logical[0], origin_logical[1], 0.0, 0.0],
            params2: [
                logical_extent[0].max(0.0001),
                logical_extent[1].max(0.0001),
                0.0,
                0.0,
            ],
            pass_kind: PASS_CLIP,
            ..Self::from_matrix(ViewColorMatrix::identity())
        };
        match plan {
            ViewClipGeometryPlan::None => {}
            ViewClipGeometryPlan::Inset { rect, radii_px } => {
                uniform.params0 = [1.0, 0.0, 0.0, 0.0];
                uniform.matrix[0] = [rect.x, rect.y, rect.width, rect.height];
                uniform.matrix[1] = *radii_px;
            }
            ViewClipGeometryPlan::Ellipse {
                center,
                radius_x_px,
                radius_y_px,
            } => {
                uniform.params0 = [2.0, 0.0, 0.0, 0.0];
                uniform.matrix[0] = [center.x, center.y, *radius_x_px, *radius_y_px];
            }
            ViewClipGeometryPlan::Polygon {
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
            ViewClipGeometryPlan::Path {
                fill_rule, edges, ..
            } => {
                uniform.params0 = [
                    4.0,
                    fill_rule_to_f32(*fill_rule),
                    edges.len().to_f32().unwrap_or(0.0),
                    0.0,
                ];
                for (index, edge) in edges.iter().copied().enumerate() {
                    uniform.clip_vertices[index] = clip_edge_uniform(edge);
                }
            }
        }
        uniform
    }

    pub(crate) fn box_shadow(
        pass: &ViewBoxShadowPass,
        origin_logical: [f32; 2],
        logical_extent: [f32; 2],
    ) -> Self {
        let shadow_kind = match pass.shadow.kind {
            ViewBoxShadowKind::Outer => 0.0,
            ViewBoxShadowKind::Inset => 1.0,
        };
        let mut uniform = Self {
            offset: rgba_to_unit(pass.shadow.color),
            params0: [pass.shadow.blur_radius_px.max(0.0), 0.0, 0.0, shadow_kind],
            params1: [origin_logical[0], origin_logical[1], 0.0, 0.0],
            params2: [
                logical_extent[0].max(0.0001),
                logical_extent[1].max(0.0001),
                0.0,
                0.0,
            ],
            pass_kind: PASS_BOX_SHADOW,
            ..Self::from_matrix(ViewColorMatrix::identity())
        };
        uniform.matrix[0] = [
            pass.body_rect.x,
            pass.body_rect.y,
            pass.body_rect.width,
            pass.body_rect.height,
        ];
        uniform.matrix[1] = [
            pass.shadow_rect.x,
            pass.shadow_rect.y,
            pass.shadow_rect.width,
            pass.shadow_rect.height,
        ];
        uniform.matrix[2] = radii_head_uniform(pass.body_radii);
        uniform.matrix[3] = radii_tail_uniform(pass.body_radii);
        uniform.clip_vertices[0] = radii_head_uniform(pass.shadow_radii);
        uniform.clip_vertices[1] = radii_tail_uniform(pass.shadow_radii);
        uniform
    }
    fn from_matrix(matrix: ViewColorMatrix) -> Self {
        Self {
            matrix: matrix.matrix,
            offset: matrix.offset,
            params0: [0.0; 4],
            params1: [0.0; 4],
            params2: [0.0; 4],
            clip_vertices: [[0.0; 4]; MAX_CLIP_PATH_EDGES],
            gradient_stops: [[0.0; 4]; MAX_MASK_GRADIENT_STOPS],
            pass_kind: PASS_COMPOSITE,
            output_encoding: OUTPUT_ENCODING_LINEAR,
            _padding: [0; 2],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ViewOutputEncoding {
    Linear,
    Srgb,
}

impl ViewOutputEncoding {
    fn for_target_format(format: wgpu::TextureFormat) -> Self {
        if format.is_srgb() {
            return Self::Linear;
        }
        match format {
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm => Self::Srgb,
            _ => Self::Linear,
        }
    }

    const fn as_uniform(self) -> u32 {
        match self {
            Self::Linear => OUTPUT_ENCODING_LINEAR,
            Self::Srgb => OUTPUT_ENCODING_SRGB,
        }
    }
}

fn dimension_to_f32(value: u32) -> f32 {
    value.max(1).to_f32().unwrap_or(f32::MAX)
}

fn shader_mode_to_f32(mode: ViewBlendShaderMode) -> f32 {
    mode.as_shader_u32().to_f32().unwrap_or(0.0)
}

fn fill_rule_to_f32(fill_rule: ViewFillRule) -> f32 {
    match fill_rule {
        ViewFillRule::NonZero => 0.0,
        ViewFillRule::EvenOdd => 1.0,
    }
}

fn mask_channel_to_f32(channel: ViewMaskChannel) -> f32 {
    match channel {
        ViewMaskChannel::Alpha => 0.0,
        ViewMaskChannel::Luminance => 1.0,
    }
}

fn repeat_mode_to_f32(mode: ViewMaskAxisRepeat) -> f32 {
    match mode {
        ViewMaskAxisRepeat::NoRepeat => 0.0,
        ViewMaskAxisRepeat::Repeat => 1.0,
        ViewMaskAxisRepeat::Space => 2.0,
        ViewMaskAxisRepeat::Round => 3.0,
    }
}

fn gradient_kind_to_f32(kind: ViewMaskGradientKind) -> f32 {
    match kind {
        ViewMaskGradientKind::Linear { .. } => 1.0,
        ViewMaskGradientKind::Radial { .. } => 2.0,
        ViewMaskGradientKind::Conic { .. } => 3.0,
    }
}

fn gradient_header_0(kind: ViewMaskGradientKind) -> [f32; 4] {
    match kind {
        ViewMaskGradientKind::Linear { angle_degrees } => [angle_degrees, 0.0, 0.0, 0.0],
        ViewMaskGradientKind::Radial { center_px, .. } => [0.0, center_px[0], center_px[1], 0.0],
        ViewMaskGradientKind::Conic {
            center_px,
            from_degrees,
        } => [0.0, center_px[0], center_px[1], from_degrees],
    }
}

fn gradient_header_1(kind: ViewMaskGradientKind, stop_count: usize) -> [f32; 4] {
    match kind {
        ViewMaskGradientKind::Linear { .. } | ViewMaskGradientKind::Conic { .. } => {
            [0.0, 0.0, stop_count.to_f32().unwrap_or(0.0), 0.0]
        }
        ViewMaskGradientKind::Radial { radius_px, .. } => [
            radius_px[0],
            radius_px[1],
            stop_count.to_f32().unwrap_or(0.0),
            0.0,
        ],
    }
}

fn clip_vertex_uniform(vertex: ViewClipVertex) -> [f32; 4] {
    [vertex.x, vertex.y, 0.0, 0.0]
}

fn clip_edge_uniform(edge: ViewClipPathEdge) -> [f32; 4] {
    [edge.from.x, edge.from.y, edge.to.x, edge.to.y]
}

fn radii_head_uniform(radii: ViewBoxShadowRadii) -> [f32; 4] {
    [
        radii.top_left.x_px,
        radii.top_left.y_px,
        radii.top_right.x_px,
        radii.top_right.y_px,
    ]
}

fn radii_tail_uniform(radii: ViewBoxShadowRadii) -> [f32; 4] {
    [
        radii.bottom_right.x_px,
        radii.bottom_right.y_px,
        radii.bottom_left.x_px,
        radii.bottom_left.y_px,
    ]
}

fn rgba_to_unit(color: ViewColorRgba8) -> [f32; 4] {
    [
        f32::from(color.red) / 255.0,
        f32::from(color.green) / 255.0,
        f32::from(color.blue) / 255.0,
        f32::from(color.alpha) / 255.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::ViewCompositorUniform;
    use crate::view_blend::ViewBlendShaderMode;
    use crate::view_box_shadow::ViewBoxShadowPassPlan;
    use crate::view_clip_path::ViewClipGeometryPlan;
    use crate::view_scene::{ViewBoxShadow, ViewBoxShadowList, ViewColorRgba8};
    use arcweft_presentation::hit::HitRect;

    #[test]
    fn clip_uniform_uses_explicit_logical_extent() {
        let plan = ViewClipGeometryPlan::Inset {
            rect: HitRect::new(80.0, 240.0, 672.0, 220.0),
            radii_px: [24.0, 24.0, 24.0, 24.0],
        };

        let uniform = ViewCompositorUniform::clip(&plan, [1280.0, 720.0], [80.0, 240.0]);

        assert_uniform_value(uniform.params1[0], 80.0);
        assert_uniform_value(uniform.params1[1], 240.0);
        assert_uniform_value(uniform.params2[0], 1280.0);
        assert_uniform_value(uniform.params2[1], 720.0);
        assert_uniform_row(uniform.matrix[0], [80.0, 240.0, 672.0, 220.0]);
    }

    #[test]
    fn box_shadow_uniform_uses_explicit_logical_extent() {
        let shadows = ViewBoxShadowList::new([ViewBoxShadow::outer(
            0.0,
            18.0,
            42.0,
            0.0,
            9.0,
            ViewColorRgba8 {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 128,
            },
        )]);
        let plan =
            ViewBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(80.0, 240.0, 672.0, 220.0))
                .expect("shadow plans");

        let uniform =
            ViewCompositorUniform::box_shadow(&plan.passes()[0], [0.0, 0.0], [1280.0, 720.0]);

        assert_uniform_value(uniform.params2[0], 1280.0);
        assert_uniform_value(uniform.params2[1], 720.0);
        assert_uniform_row(uniform.matrix[1], [80.0, 258.0, 672.0, 220.0]);
    }

    #[test]
    fn final_target_composite_encodes_display_unorm_formats() {
        let rgba = ViewCompositorUniform::composite_to_final_target(
            1.0,
            ViewBlendShaderMode::Normal,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let bgra = ViewCompositorUniform::composite_to_final_target(
            1.0,
            ViewBlendShaderMode::Normal,
            wgpu::TextureFormat::Bgra8Unorm,
        );
        let rgba_srgb = ViewCompositorUniform::composite_to_final_target(
            1.0,
            ViewBlendShaderMode::Normal,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );
        let rgba_float = ViewCompositorUniform::composite_to_final_target(
            1.0,
            ViewBlendShaderMode::Normal,
            wgpu::TextureFormat::Rgba16Float,
        );

        assert_eq!(rgba.output_encoding, super::OUTPUT_ENCODING_SRGB);
        assert_eq!(bgra.output_encoding, super::OUTPUT_ENCODING_SRGB);
        assert_eq!(rgba_srgb.output_encoding, super::OUTPUT_ENCODING_LINEAR);
        assert_eq!(rgba_float.output_encoding, super::OUTPUT_ENCODING_LINEAR);
    }

    fn assert_uniform_row(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert_uniform_value(actual, expected);
        }
    }

    fn assert_uniform_value(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "expected {expected}, got {actual}"
        );
    }
}
