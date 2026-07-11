use super::{
    Milli, NativeAnimationSample, NativeEffectExecution, NativeGlyphPlacement,
    NativeResolvedShaderFilter, NativeShaderGlyphPass, NativeVisualRun, RichTextEffectClass,
    RichTextMotionRegistry, TextEffectGlyphContext, TextEffectPostProcessContext,
    TextMotionContext, TextShaderContext, TextShaderPostProcessContext, WindowRubyBuffer,
    native_default_motion_registry, rounded_u8, typewriter_cursor_opacity,
    typewriter_visible_count, usize_to_f32_saturating,
};
use arcweft_glyphon::OwnedGlyphArea;
use arcweft_render_text::{
    RichTextEffectDescriptor, RichTextEffectPhase, RichTextEffectTarget, RichTextParam,
    RichTextPresentation, RichTextShaderRef, RichTextStateScope, RichTextTransformOrigin,
    parse_decimal_milli,
};
use arcweft_text_layout::LaidOutText;
use glyphon::Color;
use std::collections::BTreeMap;

pub(super) fn apply_presentation_to_placement_with_effects(
    line_id: &str,
    run: &NativeVisualRun,
    glyph_count: usize,
    time_seconds: f32,
    effects: &mut NativeEffectExecution<'_>,
    placement: &mut NativeGlyphPlacement,
) {
    apply_presentation_effects_to_placement_with_execution(
        line_id,
        &run.presentation,
        glyph_count,
        time_seconds,
        effects,
        placement,
    );
}

pub(super) fn apply_presentation_effects_to_placement(
    line_id: &str,
    presentation: &RichTextPresentation,
    glyph_count: usize,
    time_seconds: f32,
    placement: &mut NativeGlyphPlacement,
) {
    if let Some(transform) = &presentation.transform {
        placement.x += transform.translate.x.as_f32();
        placement.y += transform.translate.y.as_f32();
        placement.rotate_degrees += transform.rotate.as_degrees_f32();
        placement.scale_x *= transform.scale.x.as_f32();
        placement.scale_y *= transform.scale.y.as_f32();
        placement.skew_x_degrees += transform.skew.x.as_f32();
        placement.skew_y_degrees += transform.skew.y.as_f32();
        placement.affine_origin = Some(transform.origin);
        placement.affine_target = Some(transform.target);
    }
    if let Some(opacity) = presentation.opacity {
        placement.opacity *= opacity.as_f32();
    }
    for effect in &presentation.effects {
        apply_builtin_descriptor(line_id, effect, glyph_count, time_seconds, placement);
    }
}

pub(super) fn apply_presentation_effects_to_placement_with_execution(
    line_id: &str,
    presentation: &RichTextPresentation,
    glyph_count: usize,
    time_seconds: f32,
    effects: &mut NativeEffectExecution<'_>,
    placement: &mut NativeGlyphPlacement,
) {
    if let Some(transform) = &presentation.transform {
        placement.x += transform.translate.x.as_f32();
        placement.y += transform.translate.y.as_f32();
        placement.rotate_degrees += transform.rotate.as_degrees_f32();
        placement.scale_x *= transform.scale.x.as_f32();
        placement.scale_y *= transform.scale.y.as_f32();
        placement.skew_x_degrees += transform.skew.x.as_f32();
        placement.skew_y_degrees += transform.skew.y.as_f32();
        placement.affine_origin = Some(transform.origin);
        placement.affine_target = Some(transform.target);
    }
    if let Some(opacity) = presentation.opacity {
        placement.opacity *= opacity.as_f32();
    }
    for effect in &presentation.effects {
        if !apply_builtin_descriptor_with_execution(
            line_id,
            effect,
            glyph_count,
            time_seconds,
            effects,
            placement,
        ) {
            effects.apply_custom_effect(line_id, effect, glyph_count, time_seconds, placement);
        }
    }
    apply_presentation_shader_color_to_placement(presentation, effects, placement);
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn apply_builtin_descriptor(
    line_id: &str,
    effect: &RichTextEffectDescriptor,
    glyph_count: usize,
    time_seconds: f32,
    placement: &mut NativeGlyphPlacement,
) -> bool {
    let mut motion_registry = native_default_motion_registry();
    apply_builtin_descriptor_inner(
        line_id,
        effect,
        glyph_count,
        time_seconds,
        Some(&mut motion_registry),
        None,
        placement,
    )
}

pub(super) fn apply_builtin_descriptor_with_execution(
    line_id: &str,
    effect: &RichTextEffectDescriptor,
    glyph_count: usize,
    time_seconds: f32,
    effects: &mut NativeEffectExecution<'_>,
    placement: &mut NativeGlyphPlacement,
) -> bool {
    if is_builtin_effect_id(&effect.id) && !effects.observe_builtin_effect_phase(effect) {
        return true;
    }
    apply_builtin_descriptor_inner(
        line_id,
        effect,
        glyph_count,
        time_seconds,
        None,
        Some(effects),
        placement,
    )
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn apply_builtin_descriptor_inner(
    line_id: &str,
    effect: &RichTextEffectDescriptor,
    glyph_count: usize,
    time_seconds: f32,
    motion_registry: Option<&mut RichTextMotionRegistry>,
    effects: Option<&mut NativeEffectExecution<'_>>,
    placement: &mut NativeGlyphPlacement,
) -> bool {
    match effect.id.as_str() {
        "wave" => {
            if !effect_applies_to_glyph_transform(effect) {
                return true;
            }
            let amplitude = param_milli(effect, "amp").unwrap_or(Milli(4000)).as_f32();
            let period = param_milli(effect, "period")
                .unwrap_or(Milli(12000))
                .as_f32()
                .max(0.001);
            let speed = param_milli(effect, "speed").unwrap_or(Milli::ONE).as_f32();
            let phase = param_milli(effect, "phase").unwrap_or_default().as_f32();
            let direction = param_vec2(effect, "dir")
                .or_else(|| axis_direction(effect))
                .unwrap_or([0.0, 1.0]);
            let target_index = effect_target_wave_index(effect.target, placement);
            let t = (usize_to_f32_saturating(target_index) / period + time_seconds * speed + phase)
                * std::f32::consts::TAU;
            let delta = amplitude * t.sin();
            placement.x += direction[0] * delta;
            placement.y += direction[1] * delta;
        }
        "shake" | "jitter" => {
            if !effect_applies_to_glyph_transform(effect) {
                return true;
            }
            let amplitude = param_milli(effect, "amp").unwrap_or(Milli(2000)).as_f32();
            let speed = param_milli(effect, "speed")
                .unwrap_or(Milli(16000))
                .as_f32();
            let noise_seed = param_seed(effect, "seed").unwrap_or(0);
            let time_bucket = if effect.id == "jitter" {
                0.0
            } else {
                time_seconds * speed
            };
            let noise = deterministic_noise(
                noise_seed,
                line_id,
                shake_noise_index(effect.state_scope, placement),
                time_bucket,
            );
            placement.x += (noise[0] * 2.0 - 1.0) * amplitude;
            placement.y += (noise[1] * 2.0 - 1.0) * amplitude;
        }
        "arc" => {
            if !effect_applies_to_glyph_transform(effect) {
                return true;
            }
            let radius = param_milli(effect, "radius")
                .unwrap_or(Milli(120_000))
                .as_f32();
            let start = param_milli(effect, "start").unwrap_or_default().as_f32();
            let step = param_milli(effect, "step").unwrap_or(Milli(8000)).as_f32();
            let angle =
                (start + step * usize_to_f32_saturating(placement.glyph_index)).to_radians();
            placement.x += radius * angle.cos();
            placement.y += radius * angle.sin();
            placement.rotate_degrees += angle.to_degrees() + 90.0;
        }
        "spin" => {
            if !effect_applies_to_glyph_transform(effect) {
                return true;
            }
            apply_builtin_spin(effect, time_seconds, placement);
        }
        "pulse" => {
            if !effect_applies_to_glyph_transform(effect) {
                return true;
            }
            apply_builtin_pulse(effect, time_seconds, placement);
        }
        "motion" => {
            if !effect_applies_to_glyph_transform(effect) {
                return true;
            }
            apply_builtin_motion(
                line_id,
                effect,
                glyph_count,
                time_seconds,
                motion_registry,
                effects,
                placement,
            );
        }
        "typewriter" => {
            if !effect_applies_to_glyph_mask(effect) {
                return true;
            }
            let visible = typewriter_visible_count(effect, time_seconds, glyph_count);
            if placement.glyph_index >= visible.min(glyph_count) {
                placement.opacity *=
                    typewriter_cursor_opacity(effect, placement.glyph_index, visible, glyph_count);
            }
        }
        _ => return false,
    }
    true
}

pub(super) fn is_builtin_effect_id(id: &str) -> bool {
    matches!(
        id,
        "wave" | "shake" | "jitter" | "arc" | "spin" | "pulse" | "motion" | "typewriter"
    )
}

pub(super) fn builtin_effect_phase_supported(effect: &RichTextEffectDescriptor) -> bool {
    match effect.id.as_str() {
        "wave" | "shake" | "jitter" | "arc" | "spin" | "pulse" | "motion" => {
            effect.phase == RichTextEffectPhase::PostProcess
                || effect_applies_to_glyph_transform(effect)
        }
        "typewriter" => effect_applies_to_glyph_mask(effect),
        _ => true,
    }
}

pub(super) fn apply_builtin_spin(
    effect: &RichTextEffectDescriptor,
    time_seconds: f32,
    placement: &mut NativeGlyphPlacement,
) {
    let angle = param_milli(effect, "angle")
        .or_else(|| param_milli(effect, "amp"))
        .unwrap_or(Milli(6000))
        .as_f32();
    let speed = param_milli(effect, "speed").unwrap_or(Milli::ONE).as_f32();
    let phase = param_milli(effect, "phase").unwrap_or_default().as_f32();
    let t = (time_seconds * speed + phase) * std::f32::consts::TAU;
    placement.rotate_degrees += angle * t.sin();
    apply_effect_affine_pivot(effect, RichTextTransformOrigin::Center, placement);
}

pub(super) fn apply_builtin_pulse(
    effect: &RichTextEffectDescriptor,
    time_seconds: f32,
    placement: &mut NativeGlyphPlacement,
) {
    let amplitude = param_milli(effect, "amp")
        .or_else(|| param_milli(effect, "amount"))
        .unwrap_or(Milli(80))
        .as_f32()
        .max(0.0);
    let speed = param_milli(effect, "speed").unwrap_or(Milli::ONE).as_f32();
    let phase = param_milli(effect, "phase").unwrap_or_default().as_f32();
    let t = (time_seconds * speed + phase) * std::f32::consts::TAU;
    let scale = 1.0 + amplitude * (t.sin() * 0.5 + 0.5);
    placement.scale_x *= scale;
    placement.scale_y *= scale;
    apply_effect_affine_pivot(effect, RichTextTransformOrigin::Center, placement);
}

pub(super) fn apply_builtin_motion(
    line_id: &str,
    effect: &RichTextEffectDescriptor,
    glyph_count: usize,
    time_seconds: f32,
    motion_registry: Option<&mut RichTextMotionRegistry>,
    effects: Option<&mut NativeEffectExecution<'_>>,
    placement: &mut NativeGlyphPlacement,
) {
    let function = param_label(effect, "fn")
        .or_else(|| param_label(effect, "curve"))
        .unwrap_or_else(|| "breath_orbit".to_owned());
    let function = normalize_motion_function_label(&function);
    let speed = param_milli(effect, "speed").unwrap_or(Milli::ONE).as_f32();
    let phase = param_milli(effect, "phase").unwrap_or_default().as_f32();
    let amplitude = param_milli(effect, "amp")
        .or_else(|| param_milli(effect, "radius"))
        .unwrap_or(Milli(4000))
        .as_f32();
    let angle = param_milli(effect, "angle").unwrap_or(Milli(6000)).as_f32();
    let scale_amplitude = param_milli(effect, "scale")
        .or_else(|| param_milli(effect, "scale_amp"))
        .or_else(|| param_milli(effect, "amount"))
        .unwrap_or(Milli(80))
        .as_f32()
        .max(0.0);
    let motion_seed = param_seed(effect, "seed").unwrap_or(0) ^ stable_text_hash(&function);
    let noise = deterministic_noise(motion_seed, line_id, placement.glyph_index, 0.0);
    let target_index = effect_target_wave_index(effect.target, placement);
    let sample_time = time_seconds.mul_add(speed, phase)
        + usize_to_f32_saturating(target_index) * 0.037
        + noise[0] * 0.11;
    let ctx = TextMotionContext {
        effect,
        function: &function,
        sample_time,
        line_id,
        run_index: placement.run_index,
        glyph_index: placement.glyph_index,
        glyph_count,
        noise,
    };
    let sample = if let Some(effects) = effects {
        effects.sample_motion_function(effect, &function, &ctx)
    } else {
        motion_registry.and_then(|registry| registry.sample(&function, &ctx))
    };
    let Some(sample) = sample else {
        return;
    };
    apply_parametric_motion_sample_with_params(
        effect,
        sample,
        amplitude,
        angle,
        scale_amplitude,
        placement,
    );
}

pub(super) fn sample_breath_orbit(time_seconds: f32, noise: [f32; 2]) -> NativeAnimationSample {
    let tau = std::f32::consts::TAU;
    let primary = (time_seconds * tau).sin();
    let secondary = (time_seconds.mul_add(2.0, noise[0]) * tau).sin();
    let orbit = time_seconds.mul_add(tau, secondary * 0.32);
    let bloom = (primary * 0.5 + 0.5).powf(1.35) * 0.72 + (secondary * 0.5 + 0.5) * 0.28;
    NativeAnimationSample {
        translate: [
            orbit.cos() * (0.65 + bloom * 0.35),
            orbit.sin().mul_add(0.48, secondary * 0.18),
        ],
        rotate: primary.mul_add(0.72, secondary * 0.28),
        scale: bloom,
    }
}

pub(super) fn apply_parametric_motion_sample_with_params(
    effect: &RichTextEffectDescriptor,
    sample: NativeAnimationSample,
    amplitude: f32,
    angle: f32,
    scale_amplitude: f32,
    placement: &mut NativeGlyphPlacement,
) {
    placement.x += amplitude * sample.translate[0];
    placement.y += amplitude * sample.translate[1];
    placement.rotate_degrees += angle * sample.rotate;
    let scale = 1.0 + scale_amplitude * sample.scale.max(0.0);
    placement.scale_x *= scale;
    placement.scale_y *= scale;
    apply_effect_affine_pivot(effect, RichTextTransformOrigin::GlyphCenter, placement);
}

pub(super) fn sample_elastic_bloom(time_seconds: f32, noise: [f32; 2]) -> NativeAnimationSample {
    let tau = std::f32::consts::TAU;
    let primary = (time_seconds * tau).sin();
    let snap = (time_seconds.mul_add(3.0, noise[1] * 0.25) * tau)
        .sin()
        .max(0.0)
        .powf(2.2);
    NativeAnimationSample {
        translate: [primary * 0.25, -snap * 0.55],
        rotate: primary.mul_add(0.35, snap * 0.65),
        scale: snap,
    }
}

pub(super) fn normalize_motion_function_label(function: &str) -> String {
    function
        .trim()
        .trim_start_matches('@')
        .trim_start_matches('.')
        .to_owned()
}

pub(super) fn apply_effect_affine_pivot(
    effect: &RichTextEffectDescriptor,
    default_origin: RichTextTransformOrigin,
    placement: &mut NativeGlyphPlacement,
) {
    placement.affine_origin = Some(param_origin(effect).unwrap_or(default_origin));
    placement.affine_target = Some(effect.target);
}

pub(super) struct NativeSparkleEffect;

impl RichTextEffectClass for NativeSparkleEffect {
    fn apply_glyph(&mut self, ctx: &mut TextEffectGlyphContext<'_>) {
        native_sparkle_effect(ctx);
    }

    fn post_process(
        &mut self,
        ctx: &mut TextEffectPostProcessContext<'_>,
        rgba: &mut [u8],
    ) -> bool {
        native_sparkle_post_process(ctx, rgba);
        true
    }
}

pub(super) fn native_sparkle_effect(ctx: &mut TextEffectGlyphContext<'_>) {
    if !effect_applies_to_renderer_glyph(ctx.effect) {
        return;
    }
    let amplitude = param_milli(ctx.effect, "amp")
        .unwrap_or(Milli(1600))
        .as_f32()
        .max(0.0);
    let cycles_per_second = param_milli(ctx.effect, "speed")
        .unwrap_or(Milli(2200))
        .as_f32()
        .max(0.001);
    let sparkle_seed = param_seed(ctx.effect, "seed").unwrap_or(0x51A7_61E5);
    let noise = deterministic_noise(sparkle_seed, ctx.line_id, ctx.glyph_index, 0.0);
    let ordinal = usize_to_f32_saturating(ctx.glyph_index);
    let phase =
        (ctx.time_seconds * cycles_per_second + noise[0] + ordinal * 0.071) * std::f32::consts::TAU;
    let shimmer = (phase.sin() * 0.5 + 0.5).clamp(0.0, 1.0);
    let drift = (phase * 0.73 + noise[1] * std::f32::consts::TAU).cos();

    if ctx.effect.phase == RichTextEffectPhase::GlyphColor {
        let blue = rounded_u8(190.0 + shimmer * 65.0);
        let green = rounded_u8(150.0 + shimmer * 80.0);
        ctx.placement.color = Some([255, green, blue, 255]);
        return;
    }

    ctx.placement.x += drift * amplitude * 0.18;
    ctx.placement.y -= shimmer * amplitude * 0.35;
    ctx.placement.scale_x *= 1.0 + shimmer * 0.035;
    ctx.placement.scale_y *= 1.0 + shimmer * 0.035;
    ctx.placement.opacity *= (0.82 + shimmer * 0.18).clamp(0.0, 1.0);
}

pub(super) fn native_sparkle_post_process(
    ctx: &mut TextEffectPostProcessContext<'_>,
    rgba: &mut [u8],
) {
    let amount = param_milli(ctx.effect, "amount")
        .or_else(|| param_milli(ctx.effect, "amp"))
        .unwrap_or(Milli(350))
        .as_f32()
        .clamp(0.0, 1.0);
    if amount <= f32::EPSILON {
        return;
    }
    let sparkle_seed = param_seed(ctx.effect, "seed").unwrap_or(0x51A7_61E5);
    let seed_phase = f32::from(u8::try_from(sparkle_seed & 0xff).unwrap_or(0)) * 0.001;
    let shimmer = ((ctx.time_seconds * 2.2 + seed_phase) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    for (index, pixel) in rgba.chunks_exact_mut(4).enumerate() {
        if pixel[3] == 0 || (pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0) {
            continue;
        }
        let noise = deterministic_noise(sparkle_seed, ctx.line_id, index, ctx.time_seconds * 13.0);
        let pulse = ((noise[0] + shimmer) * amount).clamp(0.0, 1.0);
        pixel[0] = blend_channel(pixel[0], 255, pulse * 0.45);
        pixel[1] = blend_channel(pixel[1], 225, pulse * 0.35);
        pixel[2] = blend_channel(pixel[2], 255, pulse * 0.55);
    }
}

pub(super) fn apply_builtin_effect_post_process(
    effect: &RichTextEffectDescriptor,
    width: u32,
    height: u32,
    time_seconds: f32,
    rgba: &mut [u8],
) {
    match effect.id.as_str() {
        "wave" | "shake" | "jitter" => {
            apply_displacement_post_process(effect, width, height, time_seconds, rgba);
        }
        "arc" | "spin" | "pulse" | "motion" => {
            apply_tint_post_process(effect, rgba);
        }
        _ => {}
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub(super) fn apply_displacement_post_process(
    effect: &RichTextEffectDescriptor,
    width: u32,
    height: u32,
    time_seconds: f32,
    rgba: &mut [u8],
) {
    if width == 0 || height == 0 || rgba.is_empty() {
        return;
    }
    let source = rgba.to_vec();
    let amplitude = param_milli(effect, "amp")
        .or_else(|| param_milli(effect, "amount"))
        .unwrap_or(Milli(3000))
        .as_f32();
    let period = param_milli(effect, "period")
        .unwrap_or(Milli(64000))
        .as_f32()
        .max(1.0);
    let speed = param_milli(effect, "speed").unwrap_or(Milli::ONE).as_f32();
    let phase = param_milli(effect, "phase").unwrap_or_default().as_f32();
    let direction = param_vec2(effect, "dir")
        .or_else(|| axis_direction(effect))
        .unwrap_or([1.0, 0.0]);
    let effect_seed = param_seed(effect, "seed").unwrap_or(0);
    for y in 0..height {
        for x in 0..width {
            let wave_index = if direction[0].abs() >= direction[1].abs() {
                y as f32
            } else {
                x as f32
            };
            let delta = match effect.id.as_str() {
                "shake" => {
                    let noise = deterministic_noise(
                        effect_seed,
                        "post_process",
                        y as usize,
                        time_seconds * speed,
                    );
                    (noise[0] * 2.0 - 1.0) * amplitude
                }
                "jitter" => {
                    let noise = deterministic_noise(effect_seed, "post_process", y as usize, 0.0);
                    (noise[0] * 2.0 - 1.0) * amplitude
                }
                _ => {
                    let t = (wave_index / period + time_seconds * speed + phase)
                        * std::f32::consts::TAU;
                    t.sin() * amplitude
                }
            };
            let sample_x = (x as f32 - direction[0] * delta)
                .round()
                .clamp(0.0, width.saturating_sub(1) as f32) as u32;
            let sample_y = (y as f32 - direction[1] * delta)
                .round()
                .clamp(0.0, height.saturating_sub(1) as f32) as u32;
            let dst = ((y * width + x) * 4) as usize;
            let src = ((sample_y * width + sample_x) * 4) as usize;
            rgba[dst..dst + 4].copy_from_slice(&source[src..src + 4]);
        }
    }
}

pub(super) fn apply_tint_post_process(effect: &RichTextEffectDescriptor, rgba: &mut [u8]) {
    let amount = param_milli(effect, "amount")
        .or_else(|| param_milli(effect, "amp"))
        .unwrap_or(Milli(180))
        .as_f32()
        .clamp(0.0, 1.0);
    if amount <= f32::EPSILON {
        return;
    }
    let color = match effect.id.as_str() {
        "pulse" => [255, 220, 150],
        "spin" => [170, 220, 255],
        "arc" => [210, 190, 255],
        "motion" => [255, 170, 220],
        _ => [220, 220, 255],
    };
    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[3] == 0 || (pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0) {
            continue;
        }
        pixel[0] = blend_channel(pixel[0], color[0], amount);
        pixel[1] = blend_channel(pixel[1], color[1], amount);
        pixel[2] = blend_channel(pixel[2], color[2], amount);
    }
}

pub(super) const fn effect_target_wave_index(
    target: RichTextEffectTarget,
    placement: &NativeGlyphPlacement,
) -> usize {
    match target {
        RichTextEffectTarget::Glyph => placement.glyph_index,
        RichTextEffectTarget::Run => placement.run_index,
        RichTextEffectTarget::Document
        | RichTextEffectTarget::Line
        | RichTextEffectTarget::Sentence
        | RichTextEffectTarget::TextBox
        | RichTextEffectTarget::Screen => 0,
    }
}

pub(super) const fn effect_applies_to_glyph_transform(effect: &RichTextEffectDescriptor) -> bool {
    matches!(
        effect.phase,
        RichTextEffectPhase::BeforeLayout
            | RichTextEffectPhase::LayoutTransform
            | RichTextEffectPhase::GlyphTransform
    )
}

pub(super) const fn effect_applies_to_glyph_mask(effect: &RichTextEffectDescriptor) -> bool {
    matches!(effect.phase, RichTextEffectPhase::GlyphMask)
}

pub(super) const fn effect_applies_to_renderer_glyph(effect: &RichTextEffectDescriptor) -> bool {
    effect_phase_applies_to_renderer_glyph(effect.phase)
}

pub(super) const fn effect_phase_applies_to_renderer_glyph(phase: RichTextEffectPhase) -> bool {
    matches!(
        phase,
        RichTextEffectPhase::BeforeLayout
            | RichTextEffectPhase::LayoutTransform
            | RichTextEffectPhase::GlyphTransform
            | RichTextEffectPhase::GlyphColor
            | RichTextEffectPhase::GlyphMask
    )
}

pub(super) const fn shader_phase_known(phase: RichTextEffectPhase) -> bool {
    matches!(
        phase,
        RichTextEffectPhase::RunOffscreenPass
            | RichTextEffectPhase::GlyphColor
            | RichTextEffectPhase::PostProcess
    )
}

pub(super) fn apply_presentation_shader_color_to_placement(
    presentation: &RichTextPresentation,
    effects: &mut NativeEffectExecution<'_>,
    placement: &mut NativeGlyphPlacement,
) {
    for shader in &presentation.shaders {
        if let Some(color) = effects.shader_glyph_color(shader) {
            placement.color = Some(color);
        }
    }
}

pub(super) const fn shake_noise_index(
    scope: RichTextStateScope,
    placement: &NativeGlyphPlacement,
) -> usize {
    match scope {
        RichTextStateScope::Glyph => placement.glyph_index,
        RichTextStateScope::Run => placement.run_index,
        RichTextStateScope::Line
        | RichTextStateScope::Sentence
        | RichTextStateScope::Paragraph
        | RichTextStateScope::Document
        | RichTextStateScope::DialogueLine
        | RichTextStateScope::Speaker
        | RichTextStateScope::Window
        | RichTextStateScope::Global => 0,
    }
}

pub(super) fn resolve_shader_filter(shader: &RichTextShaderRef) -> NativeResolvedShaderFilter {
    NativeResolvedShaderFilter {
        id: shader.id.clone(),
        phase: shader.phase,
        amount: shader_param_milli(shader, "amount")
            .unwrap_or(Milli::ONE)
            .as_f32(),
        direction: shader_param_vec2(shader, "dir").unwrap_or([0.0, 1.0]),
    }
}

pub(super) fn observe_layout_shaders<'a>(
    effects: &mut NativeEffectExecution<'_>,
    layout: &LaidOutText,
    ruby_presentations: impl IntoIterator<Item = &'a RichTextPresentation>,
) {
    effects.observe_shaders(
        layout
            .runs
            .iter()
            .flat_map(|run| run.presentation.shaders.iter()),
    );
    effects.observe_shaders(
        ruby_presentations
            .into_iter()
            .flat_map(|presentation| presentation.shaders.iter()),
    );
}

pub(super) fn shader_glyph_areas_for_text(
    glyph_area: &OwnedGlyphArea,
    layout: &LaidOutText,
    effects: &mut NativeEffectExecution<'_>,
) -> Vec<OwnedGlyphArea> {
    shader_glyph_areas(glyph_area, |metadata| {
        layout.glyphs.get(metadata).map_or_else(Vec::new, |glyph| {
            shader_glyph_passes_for_presentation(&glyph.presentation, effects)
        })
    })
}

pub(super) fn shader_glyph_areas_for_ruby(
    ruby_glyph_areas: &[OwnedGlyphArea],
    ruby_buffers: &[WindowRubyBuffer],
    effects: &mut NativeEffectExecution<'_>,
) -> Vec<OwnedGlyphArea> {
    ruby_glyph_areas
        .iter()
        .zip(ruby_buffers)
        .flat_map(|(glyph_area, ruby)| {
            let passes = shader_glyph_passes_for_presentation(&ruby.presentation, effects);
            shader_glyph_areas(glyph_area, move |_metadata| passes.clone())
        })
        .collect()
}

pub(super) fn shader_glyph_passes_for_presentation(
    presentation: &RichTextPresentation,
    effects: &mut NativeEffectExecution<'_>,
) -> Vec<NativeShaderGlyphPass> {
    presentation
        .shaders
        .iter()
        .flat_map(|shader| effects.shader_glyph_passes(shader))
        .collect()
}

pub(super) fn shader_glyph_areas(
    glyph_area: &OwnedGlyphArea,
    mut passes_for_metadata: impl FnMut(usize) -> Vec<NativeShaderGlyphPass>,
) -> Vec<OwnedGlyphArea> {
    let mut passes_by_metadata = BTreeMap::<usize, Vec<NativeShaderGlyphPass>>::new();
    for glyph in glyph_area.glyphs() {
        passes_by_metadata
            .entry(glyph.metadata)
            .or_insert_with(|| passes_for_metadata(glyph.metadata));
    }
    let pass_count = passes_by_metadata
        .values()
        .map(Vec::len)
        .max()
        .unwrap_or_default();

    (0..pass_count)
        .filter_map(|pass| {
            let mut area = glyph_area.clone();
            let mut has_visible_shader_glyph = false;
            area.set_default_color(Color::rgba(0, 0, 0, 0));
            for glyph in area.glyphs_mut() {
                let Some(shader_pass) = passes_by_metadata
                    .get(&glyph.metadata)
                    .and_then(|passes| passes.get(pass))
                else {
                    glyph.color = Some(Color::rgba(0, 0, 0, 0));
                    continue;
                };
                glyph.origin.x += shader_pass.offset[0];
                glyph.origin.y += shader_pass.offset[1];
                let [red, green, blue, alpha] = shader_pass.color;
                glyph.color = Some(Color::rgba(red, green, blue, alpha));
                has_visible_shader_glyph = true;
            }
            has_visible_shader_glyph.then_some(area)
        })
        .collect()
}

#[derive(Clone, Copy)]
pub(super) enum SoftGlowPass {
    Forward,
    Backward,
    SideA,
    SideB,
}

pub(super) fn native_soft_glow_shader(ctx: &TextShaderContext<'_>) -> Vec<NativeShaderGlyphPass> {
    native_glow_shader(ctx, [155, 205, 255])
}

pub(super) fn native_warm_glow_shader(ctx: &TextShaderContext<'_>) -> Vec<NativeShaderGlyphPass> {
    native_glow_shader(ctx, [255, 178, 112])
}

pub(super) fn native_screen_tint_post_process(
    ctx: &TextShaderPostProcessContext<'_>,
    rgba: &mut [u8],
) {
    let color = shader_param_color(ctx.shader, "color").unwrap_or([120, 160, 255]);
    let amount = shader_param_milli(ctx.shader, "amount")
        .unwrap_or(Milli(250))
        .as_f32()
        .clamp(0.0, 1.0);
    if amount <= f32::EPSILON {
        return;
    }
    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[3] == 0 || (pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0) {
            continue;
        }
        pixel[0] = blend_channel(pixel[0], color[0], amount);
        pixel[1] = blend_channel(pixel[1], color[1], amount);
        pixel[2] = blend_channel(pixel[2], color[2], amount);
    }
}

pub(super) fn native_glow_shader(
    ctx: &TextShaderContext<'_>,
    color: [u8; 3],
) -> Vec<NativeShaderGlyphPass> {
    let shader = resolve_shader_filter(ctx.shader);
    if ctx.shader.phase == RichTextEffectPhase::GlyphColor {
        let alpha = rounded_u8((shader.amount * 255.0).clamp(0.0, 255.0));
        return vec![NativeShaderGlyphPass {
            offset: [0.0, 0.0],
            color: [color[0], color[1], color[2], alpha],
        }];
    }
    [
        SoftGlowPass::Forward,
        SoftGlowPass::Backward,
        SoftGlowPass::SideA,
        SoftGlowPass::SideB,
    ]
    .into_iter()
    .map(|pass| NativeShaderGlyphPass {
        offset: soft_glow_offset(&shader, pass),
        color: glow_color(&shader, pass, color),
    })
    .collect()
}

pub(super) fn soft_glow_offset(
    shader: &NativeResolvedShaderFilter,
    pass: SoftGlowPass,
) -> [f32; 2] {
    let direction = normalize_shader_direction(shader.direction);
    let side = [-direction[1], direction[0]];
    let radius = (shader.amount * 6.0).clamp(1.0, 12.0);
    match pass {
        SoftGlowPass::Forward => [direction[0] * radius, direction[1] * radius],
        SoftGlowPass::Backward => [direction[0] * radius * -0.5, direction[1] * radius * -0.5],
        SoftGlowPass::SideA => [side[0] * radius * 0.5, side[1] * radius * 0.5],
        SoftGlowPass::SideB => [side[0] * radius * -0.5, side[1] * radius * -0.5],
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn glow_color(
    shader: &NativeResolvedShaderFilter,
    pass: SoftGlowPass,
    [red, green, blue]: [u8; 3],
) -> [u8; 4] {
    let alpha_scale = match pass {
        SoftGlowPass::Forward => 72.0,
        SoftGlowPass::Backward => 44.0,
        SoftGlowPass::SideA | SoftGlowPass::SideB => 32.0,
    };
    let alpha = (shader.amount * alpha_scale).round().clamp(8.0, 96.0) as u8;
    [red, green, blue, alpha]
}

pub(super) fn normalize_shader_direction(direction: [f32; 2]) -> [f32; 2] {
    let length = direction[0].hypot(direction[1]);
    if length <= f32::EPSILON {
        [0.0, 1.0]
    } else {
        [direction[0] / length, direction[1] / length]
    }
}

pub(super) fn param_milli(effect: &RichTextEffectDescriptor, name: &str) -> Option<Milli> {
    param_as_milli(effect.params.get(name)?)
}

pub(super) fn param_seed(effect: &RichTextEffectDescriptor, name: &str) -> Option<u64> {
    effect.params.get(name).map(param_as_seed)
}

pub(super) fn param_vec2(effect: &RichTextEffectDescriptor, name: &str) -> Option<[f32; 2]> {
    param_as_vec2(effect.params.get(name)?)
}

pub(super) fn param_label(effect: &RichTextEffectDescriptor, name: &str) -> Option<String> {
    let (RichTextParam::Raw { value }
    | RichTextParam::Text { value }
    | RichTextParam::Selector { value }
    | RichTextParam::Expr { source: value }) = effect.params.get(name)?
    else {
        return None;
    };
    let label = value.trim().trim_matches('"').trim_matches('\'');
    (!label.is_empty()).then(|| label.trim_start_matches('@').to_owned())
}

pub(super) fn param_bool(effect: &RichTextEffectDescriptor, name: &str) -> Option<bool> {
    match effect.params.get(name)? {
        RichTextParam::Bool { value } => Some(*value),
        RichTextParam::Int { value } => Some(*value != 0),
        RichTextParam::Milli { value } => Some(value.0 != 0),
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value } => match value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim_start_matches('.')
        {
            "true" | "on" | "yes" | "cursor" | "ghost" | "preview" => Some(true),
            "false" | "off" | "no" | "none" | "hidden" | "0" => Some(false),
            _ => None,
        },
        RichTextParam::Expr { .. } | RichTextParam::Vec2 { .. } => None,
    }
}

pub(super) fn param_origin(effect: &RichTextEffectDescriptor) -> Option<RichTextTransformOrigin> {
    let value = match effect.params.get("origin")? {
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value } => value.as_str(),
        _ => return None,
    };
    match value.trim().trim_start_matches('.') {
        "baseline_start" | "start" => Some(RichTextTransformOrigin::BaselineStart),
        "baseline_center" => Some(RichTextTransformOrigin::BaselineCenter),
        "center" => Some(RichTextTransformOrigin::Center),
        "glyph_center" | "glyph" => Some(RichTextTransformOrigin::GlyphCenter),
        _ => None,
    }
}

pub(super) fn shader_param_milli(shader: &RichTextShaderRef, name: &str) -> Option<Milli> {
    param_as_milli(shader.params.get(name)?)
}

pub(super) fn shader_param_vec2(shader: &RichTextShaderRef, name: &str) -> Option<[f32; 2]> {
    param_as_vec2(shader.params.get(name)?)
}

pub(super) fn shader_param_color(shader: &RichTextShaderRef, name: &str) -> Option<[u8; 3]> {
    param_as_color(shader.params.get(name)?)
}

pub(super) fn param_as_color(param: &RichTextParam) -> Option<[u8; 3]> {
    let value = match param {
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value }
        | RichTextParam::Expr { source: value } => {
            value.trim().trim_matches('"').trim_matches('\'')
        }
        _ => return None,
    };
    parse_hex_rgb(value)
}

pub(super) fn parse_hex_rgb(value: &str) -> Option<[u8; 3]> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some([red, green, blue])
}

pub(super) fn blend_channel(source: u8, target: u8, amount: f32) -> u8 {
    rounded_u8(f32::from(source) * (1.0 - amount) + f32::from(target) * amount)
}

pub(super) fn param_as_milli(param: &RichTextParam) -> Option<Milli> {
    match param {
        RichTextParam::Milli { value } => Some(*value),
        RichTextParam::Int { value } => {
            Some(Milli(i32::try_from(*value).ok()?.saturating_mul(1000)))
        }
        RichTextParam::Raw { value } | RichTextParam::Text { value } => parse_raw_milli(value),
        _ => None,
    }
}

pub(super) fn param_as_seed(param: &RichTextParam) -> u64 {
    match param {
        RichTextParam::Bool { value } => u64::from(*value),
        RichTextParam::Int { value } => u64::from_ne_bytes(value.to_ne_bytes()),
        RichTextParam::Milli { value } => u64::from_ne_bytes(i64::from(value.0).to_ne_bytes()),
        RichTextParam::Vec2 { value } => {
            u64::from_ne_bytes(i64::from(value.x.0).to_ne_bytes())
                ^ u64::from_ne_bytes(i64::from(value.y.0).to_ne_bytes()).rotate_left(17)
        }
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value } => stable_text_hash(value),
        RichTextParam::Expr { source } => stable_text_hash(source),
    }
}

pub(super) fn param_as_vec2(param: &RichTextParam) -> Option<[f32; 2]> {
    match param {
        RichTextParam::Vec2 { value } => Some([value.x.as_f32(), value.y.as_f32()]),
        RichTextParam::Raw { value } | RichTextParam::Text { value } => parse_raw_vec2(value),
        _ => None,
    }
}

pub(super) fn parse_raw_milli(value: &str) -> Option<Milli> {
    let trimmed = value.trim();
    if let Some(milliseconds) = trimmed.strip_suffix("ms") {
        return parse_decimal_milli(milliseconds.trim()).map(|value| Milli(value.0 / 1000));
    }
    if let Some(seconds) = trimmed.strip_suffix('s') {
        return parse_decimal_milli(seconds.trim());
    }
    let numeric = trimmed
        .strip_suffix("px")
        .or_else(|| trimmed.strip_suffix("deg"))
        .or_else(|| trimmed.strip_suffix("ch"))
        .unwrap_or(trimmed)
        .trim();
    parse_decimal_milli(numeric)
}

pub(super) fn parse_raw_vec2(value: &str) -> Option<[f32; 2]> {
    let (x, y) = value.split_once(',')?;
    Some([parse_raw_milli(x)?.as_f32(), parse_raw_milli(y)?.as_f32()])
}

pub(super) fn axis_direction(effect: &RichTextEffectDescriptor) -> Option<[f32; 2]> {
    match effect.params.get("axis")? {
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value } => match value.as_str() {
            "x" | ".x" => Some([1.0, 0.0]),
            "y" | ".y" => Some([0.0, 1.0]),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn stable_text_hash(value: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub(super) fn deterministic_noise(
    seed: u64,
    line_id: &str,
    glyph_index: usize,
    time_bucket: f32,
) -> [f32; 2] {
    let mut hash =
        seed ^ glyph_index as u64 ^ (time_bucket.floor() as u64).wrapping_mul(0x9E37_79B9);
    for byte in line_id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x1000_0000_01B3);
    }
    let x = ((hash & 0xffff) as f32) / 65535.0;
    hash = hash.rotate_left(17).wrapping_mul(0xD6E8_FD50_9A2C_8395);
    let y = ((hash & 0xffff) as f32) / 65535.0;
    [x, y]
}
