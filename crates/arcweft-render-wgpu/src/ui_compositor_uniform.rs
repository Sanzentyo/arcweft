//! Packed uniform contract for the shared UI compositor WGSL shader.

use crate::ui_blend::UiBlendShaderMode;
use crate::ui_box_shadow::UiBoxShadowPass;
use crate::ui_clip_path::{MAX_CLIP_PATH_EDGES, UiClipGeometryPlan, UiClipPathEdge, UiClipVertex};
use crate::ui_effects::{UiBlurDirection, UiColorMatrix, UiTextureExtent};
use crate::ui_mask::{
    MAX_MASK_GRADIENT_STOPS, UiMaskAxisRepeat, UiMaskChannel, UiMaskGradientKind,
    UiMaskGradientPlan, UiMaskSamplingPlan,
};
use crate::ui_scene::{UiBoxShadowKind, UiBoxShadowRadii, UiColorRgba8, UiFillRule};
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

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct UiCompositorUniform {
    matrix: [[f32; 4]; 4],
    offset: [f32; 4],
    params0: [f32; 4],
    params1: [f32; 4],
    params2: [f32; 4],
    clip_vertices: [[f32; 4]; MAX_CLIP_PATH_EDGES],
    gradient_stops: [[f32; 4]; MAX_MASK_GRADIENT_STOPS],
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

    pub(crate) fn clipped_composite(
        opacity: f32,
        blend: UiBlendShaderMode,
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
            pass_kind: if blend == UiBlendShaderMode::Normal {
                PASS_CLIPPED_COMPOSITE
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
            params1: rgba_to_unit(tint),
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
            ..Self::from_matrix(UiColorMatrix::identity())
        }
    }

    pub(crate) fn gradient_mask(
        channel: UiMaskChannel,
        sampling: UiMaskSamplingPlan,
        gradient: &UiMaskGradientPlan,
        source_extent: UiTextureExtent,
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
        plan: &UiClipGeometryPlan,
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
            UiClipGeometryPlan::Path {
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
        pass: &UiBoxShadowPass,
        origin_logical: [f32; 2],
        logical_extent: [f32; 2],
    ) -> Self {
        let shadow_kind = match pass.shadow.kind {
            UiBoxShadowKind::Outer => 0.0,
            UiBoxShadowKind::Inset => 1.0,
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
            ..Self::from_matrix(UiColorMatrix::identity())
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
    fn from_matrix(matrix: UiColorMatrix) -> Self {
        Self {
            matrix: matrix.matrix,
            offset: matrix.offset,
            params0: [0.0; 4],
            params1: [0.0; 4],
            params2: [0.0; 4],
            clip_vertices: [[0.0; 4]; MAX_CLIP_PATH_EDGES],
            gradient_stops: [[0.0; 4]; MAX_MASK_GRADIENT_STOPS],
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

fn mask_channel_to_f32(channel: UiMaskChannel) -> f32 {
    match channel {
        UiMaskChannel::Alpha => 0.0,
        UiMaskChannel::Luminance => 1.0,
    }
}

fn repeat_mode_to_f32(mode: UiMaskAxisRepeat) -> f32 {
    match mode {
        UiMaskAxisRepeat::NoRepeat => 0.0,
        UiMaskAxisRepeat::Repeat => 1.0,
        UiMaskAxisRepeat::Space => 2.0,
        UiMaskAxisRepeat::Round => 3.0,
    }
}

fn gradient_kind_to_f32(kind: UiMaskGradientKind) -> f32 {
    match kind {
        UiMaskGradientKind::Linear { .. } => 1.0,
        UiMaskGradientKind::Radial { .. } => 2.0,
        UiMaskGradientKind::Conic { .. } => 3.0,
    }
}

fn gradient_header_0(kind: UiMaskGradientKind) -> [f32; 4] {
    match kind {
        UiMaskGradientKind::Linear { angle_degrees } => [angle_degrees, 0.0, 0.0, 0.0],
        UiMaskGradientKind::Radial { center_px, .. } => [0.0, center_px[0], center_px[1], 0.0],
        UiMaskGradientKind::Conic {
            center_px,
            from_degrees,
        } => [0.0, center_px[0], center_px[1], from_degrees],
    }
}

fn gradient_header_1(kind: UiMaskGradientKind, stop_count: usize) -> [f32; 4] {
    match kind {
        UiMaskGradientKind::Linear { .. } | UiMaskGradientKind::Conic { .. } => {
            [0.0, 0.0, stop_count.to_f32().unwrap_or(0.0), 0.0]
        }
        UiMaskGradientKind::Radial { radius_px, .. } => [
            radius_px[0],
            radius_px[1],
            stop_count.to_f32().unwrap_or(0.0),
            0.0,
        ],
    }
}

fn clip_vertex_uniform(vertex: UiClipVertex) -> [f32; 4] {
    [vertex.x, vertex.y, 0.0, 0.0]
}

fn clip_edge_uniform(edge: UiClipPathEdge) -> [f32; 4] {
    [edge.from.x, edge.from.y, edge.to.x, edge.to.y]
}

fn radii_head_uniform(radii: UiBoxShadowRadii) -> [f32; 4] {
    [
        radii.top_left.x_px,
        radii.top_left.y_px,
        radii.top_right.x_px,
        radii.top_right.y_px,
    ]
}

fn radii_tail_uniform(radii: UiBoxShadowRadii) -> [f32; 4] {
    [
        radii.bottom_right.x_px,
        radii.bottom_right.y_px,
        radii.bottom_left.x_px,
        radii.bottom_left.y_px,
    ]
}

fn rgba_to_unit(color: UiColorRgba8) -> [f32; 4] {
    [
        f32::from(color.red) / 255.0,
        f32::from(color.green) / 255.0,
        f32::from(color.blue) / 255.0,
        f32::from(color.alpha) / 255.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::UiCompositorUniform;
    use crate::ui_box_shadow::UiBoxShadowPassPlan;
    use crate::ui_clip_path::UiClipGeometryPlan;
    use crate::ui_scene::{UiBoxShadow, UiBoxShadowList, UiColorRgba8};
    use arcweft_presentation::hit::HitRect;

    #[test]
    fn clip_uniform_uses_explicit_logical_extent() {
        let plan = UiClipGeometryPlan::Inset {
            rect: HitRect::new(80.0, 240.0, 672.0, 220.0),
            radii_px: [24.0, 24.0, 24.0, 24.0],
        };

        let uniform = UiCompositorUniform::clip(&plan, [1280.0, 720.0], [80.0, 240.0]);

        assert_uniform_value(uniform.params1[0], 80.0);
        assert_uniform_value(uniform.params1[1], 240.0);
        assert_uniform_value(uniform.params2[0], 1280.0);
        assert_uniform_value(uniform.params2[1], 720.0);
        assert_uniform_row(uniform.matrix[0], [80.0, 240.0, 672.0, 220.0]);
    }

    #[test]
    fn box_shadow_uniform_uses_explicit_logical_extent() {
        let shadows = UiBoxShadowList::new([UiBoxShadow::outer(
            0.0,
            18.0,
            42.0,
            0.0,
            9.0,
            UiColorRgba8 {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 128,
            },
        )]);
        let plan =
            UiBoxShadowPassPlan::from_shadows(&shadows, HitRect::new(80.0, 240.0, 672.0, 220.0))
                .expect("shadow plans");

        let uniform =
            UiCompositorUniform::box_shadow(&plan.passes()[0], [0.0, 0.0], [1280.0, 720.0]);

        assert_uniform_value(uniform.params2[0], 1280.0);
        assert_uniform_value(uniform.params2[1], 720.0);
        assert_uniform_row(uniform.matrix[1], [80.0, 258.0, 672.0, 220.0]);
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
