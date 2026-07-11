//! Legacy rich-text source normalization into the shared Fx paint contract.
//!
//! The descriptor types remain source/domain data. This module is their only
//! executable normalization path; native hosts and capture adapters never
//! inspect descriptor IDs or run separate arithmetic.

use arcweft_glyphon::TextGlyphPaint;
use arcweft_presentation::fx::{
    Angle, FiniteF32, FxColor, FxDiagnostic, FxDiagnosticCode, FxDiagnosticContext, FxNamedValue,
    FxPhase, FxRenderResourceTable, FxRendererInterface, FxResolvedValue, FxResourceId,
    FxRuntimeValue, FxSampleContext, FxTarget, FxVec2, Length, Opacity, ResolvedFxDisplacementKind,
    ResolvedFxPostProcess, ResolvedFxResourceOutput, ResolvedTransform2D, ResolvedValueOperation,
    Seconds, Transform2D,
};
use arcweft_render_text::{
    RichTextEffectDescriptor, RichTextEffectPhase, RichTextEffectTarget, RichTextParam,
    RichTextPresentation, RichTextShaderRef, RichTextTransform, RichTextTransformOrigin, TextColor,
};
use arcweft_text_layout::{LayoutRect, LayoutSize};
use num_traits::ToPrimitive;

use super::FramePlanError;

pub(super) fn presentation_transform(
    presentation: &RichTextPresentation,
    glyph_bounds: LayoutRect,
    glyph_advance: LayoutSize,
    run_bounds: LayoutRect,
    logical_ordinal: u32,
    effect_seconds: f32,
    reduce_motion: bool,
) -> Result<ResolvedTransform2D, FramePlanError> {
    let mut resolved = ResolvedTransform2D::identity();
    if let Some(transform) = &presentation.transform {
        resolved = resolved.then(resolve_authored_transform(
            transform,
            glyph_bounds,
            glyph_advance,
            run_bounds,
        )?)?;
    }
    for effect in &presentation.effects {
        if let Some(transform) = builtin_effect_transform(
            effect,
            glyph_bounds,
            glyph_advance,
            run_bounds,
            logical_ordinal,
            effect_seconds,
            reduce_motion,
        )? {
            resolved = resolved.then(transform.resolve()?)?;
        }
    }
    Ok(resolved)
}

pub(super) fn apply_glyph_paint(
    presentation: &RichTextPresentation,
    logical_ordinal: u32,
    run_ordinal: usize,
    glyph_count: usize,
    effect_seconds: f32,
    paint: &mut TextGlyphPaint,
    resources: &FxRenderResourceTable,
) -> Result<Vec<FxDiagnostic>, FramePlanError> {
    let mut diagnostics = Vec::new();
    for shader in &presentation.shaders {
        if !matches!(
            shader.phase,
            RichTextEffectPhase::GlyphColor | RichTextEffectPhase::RunOffscreenPass
        ) {
            continue;
        }
        match resolve_shader(shader, resources) {
            Ok(output) if output.post_processes.is_empty() => {
                paint.effects.extend(output.glyph_passes);
            }
            Ok(_) => diagnostics.push(shader_diagnostic(
                shader,
                FxDiagnosticCode::UnsupportedCapability,
                "glyph shader resolved a post-process pass",
            )),
            Err(message) => diagnostics.push(shader_diagnostic(
                shader,
                FxDiagnosticCode::MissingProvider,
                message,
            )),
        }
    }

    for effect in &presentation.effects {
        match (effect.id.as_str(), effect.phase) {
            ("sparkle", RichTextEffectPhase::GlyphColor) => {
                paint.color = sparkle_color(effect, logical_ordinal, effect_seconds)?;
            }
            ("typewriter", RichTextEffectPhase::GlyphMask) => {
                apply_typewriter(effect, run_ordinal, glyph_count, effect_seconds, paint)?;
            }
            _ => {}
        }
    }
    Ok(diagnostics)
}

pub(super) fn collect_frame_passes<'a>(
    presentations: impl IntoIterator<Item = &'a RichTextPresentation>,
    effect_seconds: f32,
    resources: &FxRenderResourceTable,
) -> Result<(Vec<ResolvedFxPostProcess>, Vec<FxDiagnostic>), FramePlanError> {
    let mut post_processes = Vec::new();
    let mut diagnostics = Vec::new();
    for presentation in presentations {
        for shader in &presentation.shaders {
            match resolve_shader(shader, resources) {
                Ok(output) => {
                    if shader.phase == RichTextEffectPhase::PostProcess {
                        for pass in output.post_processes {
                            push_unique(&mut post_processes, pass);
                        }
                    }
                }
                Err(message) => push_unique(
                    &mut diagnostics,
                    shader_diagnostic(shader, FxDiagnosticCode::MissingProvider, message),
                ),
            }
        }
        for effect in &presentation.effects {
            if effect.phase == RichTextEffectPhase::HostEvent {
                continue;
            }
            if !legacy_effect_is_known(&effect.id) {
                push_unique(
                    &mut diagnostics,
                    effect_diagnostic(
                        effect,
                        FxDiagnosticCode::MissingProvider,
                        format!("shared Fx provider `{}` is not available", effect.id),
                    ),
                );
                continue;
            }
            if !legacy_effect_phase_supported(effect) {
                push_unique(
                    &mut diagnostics,
                    effect_diagnostic(
                        effect,
                        FxDiagnosticCode::UnsupportedCapability,
                        format!(
                            "shared effect `{}` does not support phase {:?}",
                            effect.id, effect.phase
                        ),
                    ),
                );
                continue;
            }
            if effect.id == "motion" && !motion_function_is_known(effect) {
                push_unique(
                    &mut diagnostics,
                    effect_diagnostic(
                        effect,
                        FxDiagnosticCode::MissingProvider,
                        format!(
                            "shared motion provider `{}` is not available",
                            motion_function(effect)
                        ),
                    ),
                );
                continue;
            }
            if effect.phase == RichTextEffectPhase::PostProcess
                && let Some(pass) = effect_post_process(effect, effect_seconds)?
            {
                push_unique(&mut post_processes, pass);
            }
        }
    }
    Ok((post_processes, diagnostics))
}

fn resolve_authored_transform(
    transform: &RichTextTransform,
    glyph_bounds: LayoutRect,
    glyph_advance: LayoutSize,
    run_bounds: LayoutRect,
) -> Result<ResolvedTransform2D, FramePlanError> {
    let [origin_x, origin_y] = transform_origin(
        transform.origin,
        transform.target,
        glyph_bounds,
        glyph_advance,
        run_bounds,
    );
    Ok(Transform2D {
        translate_x: Length::try_pixels(transform.translate.x.as_f32())?,
        translate_y: Length::try_pixels(transform.translate.y.as_f32())?,
        scale_x: FiniteF32::try_new(transform.scale.x.as_f32())?,
        scale_y: FiniteF32::try_new(transform.scale.y.as_f32())?,
        skew_x: Angle::try_degrees(f64::from(transform.skew.x.as_f32()))?,
        skew_y: Angle::try_degrees(f64::from(transform.skew.y.as_f32()))?,
        rotation: Angle::try_degrees(f64::from(transform.rotate.as_degrees_f32()))?,
        origin_x: Length::try_pixels(origin_x)?,
        origin_y: Length::try_pixels(origin_y)?,
        opacity: FiniteF32::ONE,
    }
    .resolve()?)
}

#[allow(clippy::too_many_arguments)]
#[expect(
    clippy::too_many_lines,
    reason = "This closed dispatcher keeps every legacy transform formula in one auditable shared path."
)]
fn builtin_effect_transform(
    effect: &RichTextEffectDescriptor,
    glyph_bounds: LayoutRect,
    glyph_advance: LayoutSize,
    run_bounds: LayoutRect,
    logical_ordinal: u32,
    effect_seconds: f32,
    reduce_motion: bool,
) -> Result<Option<Transform2D>, FramePlanError> {
    if effect.phase != RichTextEffectPhase::GlyphTransform {
        return Ok(None);
    }
    let mut transform = Transform2D::default();
    let mut origin = RichTextTransformOrigin::GlyphCenter;
    match effect.id.as_str() {
        "wave" => {
            let amplitude = effect_value(effect, "amp", 4.0)?;
            let period = effect_value(effect, "period", 12.0)?;
            if period <= 0.0 {
                return Err(invalid_effect_parameter(effect, "period"));
            }
            let speed = effect_value_alias(effect, "speed", "freq", 1.0)?;
            let authored_phase = effect_value(effect, "phase", 0.0)?;
            let direction = effect_direction(effect, [0.0, 1.0])?;
            let phase = (logical_ordinal.to_f32().unwrap_or(f32::MAX) / period
                + effect_seconds * speed
                + authored_phase)
                * std::f32::consts::TAU;
            let delta = amplitude * phase.sin();
            transform.translate_x = Length::try_pixels(direction[0] * delta)?;
            transform.translate_y = Length::try_pixels(direction[1] * delta)?;
        }
        "shake" | "jitter" => {
            let amplitude = effect_value(effect, "amp", 2.0)?;
            let speed = effect_value(effect, "speed", 16.0)?;
            let bucket = if effect.id == "jitter" || reduce_motion {
                0
            } else {
                (effect_seconds * speed)
                    .floor()
                    .to_i32()
                    .ok_or_else(|| invalid_effect_parameter(effect, "speed"))?
            };
            let context = FxSampleContext::from_elapsed(
                Seconds::try_seconds(effect_seconds)?,
                logical_ordinal,
                effect_seed(effect),
                reduce_motion,
            );
            let x = context.deterministic_noise(bucket)?.get() * 2.0 - 1.0;
            let y = context
                .deterministic_noise(bucket.wrapping_add(0x51f1_5e5d))?
                .get()
                * 2.0
                - 1.0;
            transform.translate_x = Length::try_pixels(x * amplitude)?;
            transform.translate_y = Length::try_pixels(y * amplitude)?;
        }
        "arc" => {
            let radius = effect_value(effect, "radius", 120.0)?;
            let start = effect_value(effect, "start", 0.0)?;
            let step = effect_value(effect, "step", 8.0)?;
            let angle = (start + step * logical_ordinal.to_f32().unwrap_or(f32::MAX)).to_radians();
            transform.translate_x = Length::try_pixels(radius * angle.cos())?;
            transform.translate_y = Length::try_pixels(radius * angle.sin())?;
            transform.rotation = Angle::try_radians(angle + std::f32::consts::FRAC_PI_2)?;
        }
        "spin" => {
            let angle = effect_value_alias(effect, "angle", "amp", 6.0)?;
            let speed = effect_value(effect, "speed", 1.0)?;
            let phase = effect_value(effect, "phase", 0.0)?;
            let sample = (effect_seconds * speed + phase) * std::f32::consts::TAU;
            transform.rotation = Angle::try_degrees(f64::from(angle * sample.sin()))?;
            origin = effect_origin(effect)?.unwrap_or(RichTextTransformOrigin::Center);
        }
        "pulse" => {
            let amplitude = effect_value_alias(effect, "amp", "amount", 0.08)?;
            if amplitude < 0.0 {
                return Err(invalid_effect_parameter(effect, "amp"));
            }
            let speed = effect_value(effect, "speed", 1.0)?;
            let phase = effect_value(effect, "phase", 0.0)?;
            let sample = (effect_seconds * speed + phase) * std::f32::consts::TAU;
            let scale = 1.0 + amplitude * (sample.sin() * 0.5 + 0.5);
            transform.scale_x = FiniteF32::try_new(scale)?;
            transform.scale_y = FiniteF32::try_new(scale)?;
            origin = effect_origin(effect)?.unwrap_or(RichTextTransformOrigin::Center);
        }
        "sparkle" => apply_sparkle_transform(
            effect,
            logical_ordinal,
            effect_seconds,
            reduce_motion,
            &mut transform,
        )?,
        "motion" => apply_motion_transform(
            effect,
            logical_ordinal,
            effect_seconds,
            reduce_motion,
            &mut transform,
        )?,
        _ => return Ok(None),
    }
    let [origin_x, origin_y] = transform_origin(
        origin,
        effect.target,
        glyph_bounds,
        glyph_advance,
        run_bounds,
    );
    transform.origin_x = Length::try_pixels(origin_x)?;
    transform.origin_y = Length::try_pixels(origin_y)?;
    Ok(Some(transform))
}

fn apply_sparkle_transform(
    effect: &RichTextEffectDescriptor,
    logical_ordinal: u32,
    effect_seconds: f32,
    reduce_motion: bool,
    transform: &mut Transform2D,
) -> Result<(), FramePlanError> {
    let amplitude = effect_value(effect, "amp", 1.6)?;
    if amplitude < 0.0 {
        return Err(invalid_effect_parameter(effect, "amp"));
    }
    let speed = effect_value(effect, "speed", 2.2)?;
    if speed <= 0.0 {
        return Err(invalid_effect_parameter(effect, "speed"));
    }
    let context = FxSampleContext::from_elapsed(
        Seconds::try_seconds(effect_seconds)?,
        logical_ordinal,
        effect_seed(effect) ^ 0x51A7_61E5,
        reduce_motion,
    );
    let noise_x = context.deterministic_noise(0)?.get();
    let noise_y = context.deterministic_noise(1)?.get();
    let phase =
        (effect_seconds * speed + noise_x + logical_ordinal.to_f32().unwrap_or(f32::MAX) * 0.071)
            * std::f32::consts::TAU;
    let shimmer = phase.sin() * 0.5 + 0.5;
    let drift = (phase * 0.73 + noise_y * std::f32::consts::TAU).cos();
    transform.translate_x = Length::try_pixels(drift * amplitude * 0.18)?;
    transform.translate_y = Length::try_pixels(-shimmer * amplitude * 0.35)?;
    let scale = 1.0 + shimmer * 0.035;
    transform.scale_x = FiniteF32::try_new(scale)?;
    transform.scale_y = FiniteF32::try_new(scale)?;
    transform.opacity = FiniteF32::try_new(0.82 + shimmer * 0.18)?;
    Ok(())
}

fn apply_motion_transform(
    effect: &RichTextEffectDescriptor,
    logical_ordinal: u32,
    effect_seconds: f32,
    reduce_motion: bool,
    transform: &mut Transform2D,
) -> Result<(), FramePlanError> {
    if !motion_function_is_known(effect) {
        return Ok(());
    }
    let function = motion_function(effect);
    let speed = effect_value(effect, "speed", 1.0)?;
    let phase = effect_value(effect, "phase", 0.0)?;
    let amplitude = effect_value_alias(effect, "amp", "radius", 4.0)?;
    let angle = effect_value(effect, "angle", 6.0)?;
    let scale_amplitude = effect_value_alias(effect, "scale", "scale_amp", 0.08)?;
    if scale_amplitude < 0.0 {
        return Err(invalid_effect_parameter(effect, "scale"));
    }
    let context = FxSampleContext::from_elapsed(
        Seconds::try_seconds(effect_seconds)?,
        logical_ordinal,
        effect_seed(effect) ^ stable_text_hash(&function),
        reduce_motion,
    );
    let noise = [
        context.deterministic_noise(0)?.get(),
        context.deterministic_noise(1)?.get(),
    ];
    let sample_time = effect_seconds.mul_add(speed, phase)
        + logical_ordinal.to_f32().unwrap_or(f32::MAX) * 0.037
        + noise[0] * 0.11;
    let sample = if function.ends_with("elastic_bloom") {
        sample_elastic_bloom(sample_time, noise)
    } else {
        sample_breath_orbit(sample_time, noise)
    };
    transform.translate_x = Length::try_pixels(amplitude * sample.translate[0])?;
    transform.translate_y = Length::try_pixels(amplitude * sample.translate[1])?;
    transform.rotation = Angle::try_degrees(f64::from(angle * sample.rotate))?;
    let scale = 1.0 + scale_amplitude * sample.scale.max(0.0);
    transform.scale_x = FiniteF32::try_new(scale)?;
    transform.scale_y = FiniteF32::try_new(scale)?;
    Ok(())
}

#[derive(Clone, Copy)]
struct MotionSample {
    translate: [f32; 2],
    rotate: f32,
    scale: f32,
}

fn sample_breath_orbit(time_seconds: f32, noise: [f32; 2]) -> MotionSample {
    let tau = std::f32::consts::TAU;
    let primary = (time_seconds * tau).sin();
    let secondary = (time_seconds.mul_add(2.0, noise[0]) * tau).sin();
    let orbit = time_seconds.mul_add(tau, secondary * 0.32);
    let bloom = (primary * 0.5 + 0.5).powf(1.35) * 0.72 + (secondary * 0.5 + 0.5) * 0.28;
    MotionSample {
        translate: [
            orbit.cos() * (0.65 + bloom * 0.35),
            orbit.sin().mul_add(0.48, secondary * 0.18),
        ],
        rotate: primary.mul_add(0.72, secondary * 0.28),
        scale: bloom,
    }
}

fn sample_elastic_bloom(time_seconds: f32, noise: [f32; 2]) -> MotionSample {
    let tau = std::f32::consts::TAU;
    let primary = (time_seconds * tau).sin();
    let snap = (time_seconds.mul_add(3.0, noise[1] * 0.25) * tau)
        .sin()
        .max(0.0)
        .powf(2.2);
    MotionSample {
        translate: [primary * 0.25, -snap * 0.55],
        rotate: primary.mul_add(0.35, snap * 0.65),
        scale: snap,
    }
}

fn transform_origin(
    origin: RichTextTransformOrigin,
    target: RichTextEffectTarget,
    glyph_bounds: LayoutRect,
    glyph_advance: LayoutSize,
    run_bounds: LayoutRect,
) -> [f32; 2] {
    let target_bounds = match target {
        RichTextEffectTarget::Glyph => glyph_bounds,
        RichTextEffectTarget::Document
        | RichTextEffectTarget::Line
        | RichTextEffectTarget::Sentence
        | RichTextEffectTarget::Run
        | RichTextEffectTarget::TextBox
        | RichTextEffectTarget::Screen => run_bounds,
    };
    let global = match origin {
        RichTextTransformOrigin::BaselineStart => [target_bounds.x, target_bounds.y],
        RichTextTransformOrigin::BaselineCenter => [
            glyph_bounds.x + glyph_advance.width * 0.5,
            glyph_bounds.y + glyph_advance.height * 0.5,
        ],
        RichTextTransformOrigin::Center | RichTextTransformOrigin::GlyphCenter => [
            target_bounds.x + target_bounds.width * 0.5,
            target_bounds.y + target_bounds.height * 0.5,
        ],
    };
    [global[0] - glyph_bounds.x, global[1] - glyph_bounds.y]
}

fn sparkle_color(
    effect: &RichTextEffectDescriptor,
    logical_ordinal: u32,
    effect_seconds: f32,
) -> Result<TextColor, FramePlanError> {
    let speed = effect_value(effect, "speed", 2.2)?;
    let context = FxSampleContext::from_elapsed(
        Seconds::try_seconds(effect_seconds)?,
        logical_ordinal,
        effect_seed(effect) ^ 0x51A7_61E5,
        false,
    );
    let phase = (effect_seconds * speed
        + context.deterministic_noise(0)?.get()
        + logical_ordinal.to_f32().unwrap_or(f32::MAX) * 0.071)
        * std::f32::consts::TAU;
    let shimmer = phase.sin() * 0.5 + 0.5;
    Ok(TextColor::rgba(
        255,
        rounded_u8(150.0 + shimmer * 80.0),
        rounded_u8(190.0 + shimmer * 65.0),
        255,
    ))
}

fn apply_typewriter(
    effect: &RichTextEffectDescriptor,
    glyph_ordinal: usize,
    glyph_count: usize,
    effect_seconds: f32,
    paint: &mut TextGlyphPaint,
) -> Result<(), FramePlanError> {
    let cps = effect_value(effect, "cps", 28.0)?;
    let delay = effect_value_alias(effect, "delay", "start", 0.0)?;
    if cps < 0.0 || delay < 0.0 {
        return Err(invalid_effect_parameter(
            effect,
            if cps < 0.0 { "cps" } else { "delay" },
        ));
    }
    let elapsed = (effect_seconds.max(0.0) - delay).max(0.0);
    let visible = (elapsed * cps)
        .floor()
        .to_usize()
        .unwrap_or(usize::MAX)
        .min(glyph_count);
    if glyph_ordinal < visible {
        return Ok(());
    }
    let cursor = effect_bool(effect, "cursor")?.unwrap_or(false);
    if cursor && glyph_ordinal == visible && visible < glyph_count {
        let opacity = effect_value_alias(effect, "cursor_alpha", "cursor_opacity", 0.35)?;
        if !(0.0..=1.0).contains(&opacity) {
            return Err(invalid_effect_parameter(effect, "cursor_alpha"));
        }
        paint.opacity_milli = scale_milli(paint.opacity_milli, opacity);
    } else {
        paint.visible = false;
    }
    Ok(())
}

fn effect_post_process(
    effect: &RichTextEffectDescriptor,
    effect_seconds: f32,
) -> Result<Option<ResolvedFxPostProcess>, FramePlanError> {
    let pass = match effect.id.as_str() {
        "wave" | "shake" | "jitter" => {
            let amplitude = effect_value_alias(effect, "amp", "amount", 3.0)?;
            let period = effect_value(effect, "period", 64.0)?;
            if period <= 0.0 {
                return Err(invalid_effect_parameter(effect, "period"));
            }
            let speed = effect_value(effect, "speed", 1.0)?;
            let authored_phase = effect_value(effect, "phase", 0.0)?;
            let direction = normalized_direction(effect_direction(effect, [1.0, 0.0])?)?;
            let phase = if effect.id == "jitter" {
                0.0
            } else {
                (effect_seconds * speed + authored_phase) * std::f32::consts::TAU
            };
            ResolvedFxPostProcess::Displacement {
                displacement: match effect.id.as_str() {
                    "wave" => ResolvedFxDisplacementKind::Wave,
                    "shake" => ResolvedFxDisplacementKind::Shake,
                    "jitter" => ResolvedFxDisplacementKind::Jitter,
                    _ => unreachable!(),
                },
                amplitude: Length::try_pixels(amplitude)?,
                period: Length::try_pixels(period)?,
                phase_radians: FiniteF32::try_new(phase)?,
                direction: FxVec2 {
                    x: FiniteF32::try_new(direction[0])?,
                    y: FiniteF32::try_new(direction[1])?,
                },
                seed: effect_seed(effect),
            }
        }
        "sparkle" => {
            let amount = effect_value_alias(effect, "amount", "amp", 0.35)?;
            ResolvedFxPostProcess::Sparkle {
                amount: checked_opacity(effect, "amount", amount)?,
                phase_radians: FiniteF32::try_new(effect_seconds * 2.2 * std::f32::consts::TAU)?,
                seed: effect_seed(effect) ^ 0x51A7_61E5,
            }
        }
        "arc" | "spin" | "pulse" | "motion" => {
            let amount = effect_value_alias(effect, "amount", "amp", 0.18)?;
            let color = match effect.id.as_str() {
                "pulse" => [255, 220, 150, 255],
                "spin" => [170, 220, 255, 255],
                "arc" => [210, 190, 255, 255],
                "motion" => [255, 170, 220, 255],
                _ => unreachable!(),
            };
            ResolvedFxPostProcess::Tint {
                color: FxColor::from_rgba8(color),
                amount: checked_opacity(effect, "amount", amount)?,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(pass))
}

fn resolve_shader(
    shader: &RichTextShaderRef,
    resources: &FxRenderResourceTable,
) -> Result<ResolvedFxResourceOutput, String> {
    let phase = shader_phase(shader)?;
    let resource = FxResourceId::try_new(shader.id.clone())?;
    let uniforms = shader
        .params
        .iter()
        .map(|(name, value)| shader_uniform(name, value))
        .collect::<Result<Vec<_>, _>>()?;
    let operation = ResolvedValueOperation::new(
        FxRendererInterface::ShaderUniform,
        phase,
        if phase == FxPhase::PostProcess {
            FxTarget::Viewport
        } else {
            FxTarget::Content
        },
        vec![
            FxNamedValue::new("resource", FxResolvedValue::Resource(resource)),
            FxNamedValue::new("uniforms", FxResolvedValue::Record(uniforms)),
        ],
    );
    resources
        .resolve_shader(&operation)
        .map_err(|error| error.to_string())
}

fn shader_phase(shader: &RichTextShaderRef) -> Result<FxPhase, String> {
    match shader.phase {
        RichTextEffectPhase::GlyphColor => Ok(FxPhase::GlyphColor),
        RichTextEffectPhase::RunOffscreenPass => Ok(FxPhase::OffscreenPass),
        RichTextEffectPhase::PostProcess => Ok(FxPhase::PostProcess),
        phase => Err(format!(
            "shared shader `{}` does not support phase {phase:?}",
            shader.id
        )),
    }
}

fn shader_uniform(name: &str, value: &RichTextParam) -> Result<FxNamedValue, String> {
    let value = match name {
        "amount" => FxRuntimeValue::F32(
            FiniteF32::try_new(param_number(value, name)?).map_err(|error| error.to_string())?,
        ),
        "dir" => {
            let direction = param_vec2(value, name)?;
            FxRuntimeValue::Vec2(FxVec2 {
                x: FiniteF32::try_new(direction[0]).map_err(|error| error.to_string())?,
                y: FiniteF32::try_new(direction[1]).map_err(|error| error.to_string())?,
            })
        }
        "color" => FxRuntimeValue::Color(param_color(value)?),
        _ => return Err(format!("shared shader has no uniform named `{name}`")),
    };
    Ok(FxNamedValue::runtime(name, value))
}

fn param_color(value: &RichTextParam) -> Result<FxColor, String> {
    let value = match value {
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value } => value.trim(),
        _ => return Err("shader color must be a closed color literal".to_owned()),
    };
    let channels = match value.to_ascii_lowercase().as_str() {
        "red" => [255, 0, 0, 255],
        "green" => [0, 128, 0, 255],
        "blue" => [0, 0, 255, 255],
        "white" => [255, 255, 255, 255],
        "black" => [0, 0, 0, 255],
        _ => parse_hex_color(value)?,
    };
    Ok(FxColor::from_rgba8(channels))
}

fn parse_hex_color(value: &str) -> Result<[u8; 4], String> {
    let hex = value
        .strip_prefix('#')
        .ok_or_else(|| format!("unsupported shader color `{value}`"))?;
    if hex.len() != 6 {
        return Err(format!("shader color `{value}` must use #RRGGBB"));
    }
    let channel = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&hex[range], 16)
            .map_err(|_| format!("shader color `{value}` contains non-hex digits"))
    };
    Ok([channel(0..2)?, channel(2..4)?, channel(4..6)?, 255])
}

fn shader_diagnostic(
    shader: &RichTextShaderRef,
    code: FxDiagnosticCode,
    message: impl Into<String>,
) -> FxDiagnostic {
    let phase = shader_phase(shader).ok();
    FxDiagnostic::error(
        code,
        FxDiagnosticContext {
            target: Some(if phase == Some(FxPhase::PostProcess) {
                FxTarget::Viewport
            } else {
                FxTarget::Content
            }),
            interface: Some(FxRendererInterface::ShaderUniform),
            ..FxDiagnosticContext::default()
        },
        message,
    )
}

fn effect_diagnostic(
    effect: &RichTextEffectDescriptor,
    code: FxDiagnosticCode,
    message: impl Into<String>,
) -> FxDiagnostic {
    FxDiagnostic::error(
        code,
        FxDiagnosticContext {
            target: Some(effect_target(effect.target)),
            interface: Some(effect_interface(effect.phase)),
            ..FxDiagnosticContext::default()
        },
        message,
    )
}

fn effect_target(target: RichTextEffectTarget) -> FxTarget {
    match target {
        RichTextEffectTarget::Glyph => FxTarget::Glyph,
        RichTextEffectTarget::Line | RichTextEffectTarget::Sentence => FxTarget::Line,
        RichTextEffectTarget::Screen => FxTarget::Viewport,
        RichTextEffectTarget::Document
        | RichTextEffectTarget::Run
        | RichTextEffectTarget::TextBox => FxTarget::Content,
    }
}

fn effect_interface(phase: RichTextEffectPhase) -> FxRendererInterface {
    match phase {
        RichTextEffectPhase::BeforeLayout => FxRendererInterface::TextStyle,
        RichTextEffectPhase::LayoutTransform | RichTextEffectPhase::GlyphTransform => {
            FxRendererInterface::Transform
        }
        RichTextEffectPhase::GlyphColor => FxRendererInterface::Color,
        RichTextEffectPhase::GlyphMask => FxRendererInterface::Mask,
        RichTextEffectPhase::RunOffscreenPass => FxRendererInterface::OffscreenPass,
        RichTextEffectPhase::PostProcess => FxRendererInterface::PostProcess,
        RichTextEffectPhase::HostEvent => FxRendererInterface::Transition,
    }
}

fn legacy_effect_is_known(id: &str) -> bool {
    matches!(
        id,
        "wave"
            | "shake"
            | "jitter"
            | "arc"
            | "spin"
            | "pulse"
            | "motion"
            | "typewriter"
            | "sparkle"
    )
}

fn legacy_effect_phase_supported(effect: &RichTextEffectDescriptor) -> bool {
    match effect.id.as_str() {
        "wave" | "shake" | "jitter" | "arc" | "spin" | "pulse" | "motion" => matches!(
            effect.phase,
            RichTextEffectPhase::GlyphTransform | RichTextEffectPhase::PostProcess
        ),
        "sparkle" => matches!(
            effect.phase,
            RichTextEffectPhase::GlyphTransform
                | RichTextEffectPhase::GlyphColor
                | RichTextEffectPhase::PostProcess
        ),
        "typewriter" => effect.phase == RichTextEffectPhase::GlyphMask,
        _ => false,
    }
}

fn motion_function_is_known(effect: &RichTextEffectDescriptor) -> bool {
    matches!(
        motion_function(effect).as_str(),
        "breath_orbit" | "fx.breath_orbit" | "elastic_bloom" | "fx.elastic_bloom"
    )
}

fn motion_function(effect: &RichTextEffectDescriptor) -> String {
    effect
        .params
        .get("fn")
        .or_else(|| effect.params.get("curve"))
        .and_then(param_text)
        .unwrap_or_else(|| "breath_orbit".to_owned())
        .trim()
        .trim_start_matches('@')
        .trim_start_matches('.')
        .to_owned()
}

fn effect_value(
    effect: &RichTextEffectDescriptor,
    name: &'static str,
    default: f32,
) -> Result<f32, FramePlanError> {
    effect
        .params
        .get(name)
        .map(|value| param_number(value, name).map_err(|_| invalid_effect_parameter(effect, name)))
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn effect_value_alias(
    effect: &RichTextEffectDescriptor,
    name: &'static str,
    alias: &'static str,
    default: f32,
) -> Result<f32, FramePlanError> {
    if effect.params.contains_key(name) {
        effect_value(effect, name, default)
    } else {
        effect_value(effect, alias, default)
    }
}

fn param_number(value: &RichTextParam, name: &str) -> Result<f32, String> {
    let parsed = match value {
        RichTextParam::Int { value } => value.to_f32(),
        RichTextParam::Milli { value } => Some(value.as_f32()),
        RichTextParam::Raw { value } | RichTextParam::Text { value } => {
            let value = value.trim();
            let numeric = ["px", "deg", "ms", "s", "ch"]
                .iter()
                .find_map(|suffix| value.strip_suffix(suffix))
                .unwrap_or(value)
                .trim();
            numeric
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite())
        }
        RichTextParam::Bool { .. }
        | RichTextParam::Vec2 { .. }
        | RichTextParam::Selector { .. }
        | RichTextParam::Expr { .. } => None,
    };
    parsed.ok_or_else(|| format!("parameter `{name}` is not a finite number"))
}

fn effect_direction(
    effect: &RichTextEffectDescriptor,
    default: [f32; 2],
) -> Result<[f32; 2], FramePlanError> {
    if let Some(value) = effect.params.get("dir") {
        return param_vec2(value, "dir").map_err(|_| invalid_effect_parameter(effect, "dir"));
    }
    if let Some(value) = effect.params.get("axis") {
        let axis = param_text(value)
            .ok_or_else(|| invalid_effect_parameter(effect, "axis"))?
            .trim()
            .trim_start_matches('.')
            .to_owned();
        return match axis.as_str() {
            "x" => Ok([1.0, 0.0]),
            "y" => Ok([0.0, 1.0]),
            _ => Err(invalid_effect_parameter(effect, "axis")),
        };
    }
    Ok(default)
}

fn param_vec2(value: &RichTextParam, name: &str) -> Result<[f32; 2], String> {
    match value {
        RichTextParam::Vec2 { value } => Ok([value.x.as_f32(), value.y.as_f32()]),
        RichTextParam::Raw { value } | RichTextParam::Text { value } => {
            let (x, y) = value
                .split_once(',')
                .ok_or_else(|| format!("parameter `{name}` must contain x,y"))?;
            Ok([
                x.trim()
                    .parse()
                    .map_err(|_| format!("parameter `{name}` x is invalid"))?,
                y.trim()
                    .parse()
                    .map_err(|_| format!("parameter `{name}` y is invalid"))?,
            ])
        }
        _ => Err(format!("parameter `{name}` is not a vector")),
    }
}

fn effect_origin(
    effect: &RichTextEffectDescriptor,
) -> Result<Option<RichTextTransformOrigin>, FramePlanError> {
    let Some(value) = effect.params.get("origin") else {
        return Ok(None);
    };
    let value = param_text(value)
        .ok_or_else(|| invalid_effect_parameter(effect, "origin"))?
        .trim()
        .trim_start_matches('.')
        .to_owned();
    match value.as_str() {
        "baseline_start" | "start" => Ok(Some(RichTextTransformOrigin::BaselineStart)),
        "baseline_center" => Ok(Some(RichTextTransformOrigin::BaselineCenter)),
        "center" => Ok(Some(RichTextTransformOrigin::Center)),
        "glyph_center" | "glyph" => Ok(Some(RichTextTransformOrigin::GlyphCenter)),
        _ => Err(invalid_effect_parameter(effect, "origin")),
    }
}

fn effect_bool(
    effect: &RichTextEffectDescriptor,
    name: &'static str,
) -> Result<Option<bool>, FramePlanError> {
    effect
        .params
        .get(name)
        .map(|value| match value {
            RichTextParam::Bool { value } => Ok(*value),
            RichTextParam::Raw { value } | RichTextParam::Text { value } => match value.as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(invalid_effect_parameter(effect, name)),
            },
            _ => Err(invalid_effect_parameter(effect, name)),
        })
        .transpose()
}

fn param_text(value: &RichTextParam) -> Option<String> {
    match value {
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value } => Some(value.clone()),
        RichTextParam::Expr { source } => Some(source.clone()),
        RichTextParam::Bool { .. }
        | RichTextParam::Int { .. }
        | RichTextParam::Milli { .. }
        | RichTextParam::Vec2 { .. } => None,
    }
}

fn effect_seed(effect: &RichTextEffectDescriptor) -> u64 {
    let Some(seed) = effect.params.get("seed") else {
        return 0;
    };
    match seed {
        RichTextParam::Bool { value } => u64::from(*value),
        RichTextParam::Int { value } => u64::from_ne_bytes(value.to_ne_bytes()),
        RichTextParam::Milli { value } => u64::from_ne_bytes(i64::from(value.0).to_ne_bytes()),
        RichTextParam::Vec2 { value } => {
            u64::from_ne_bytes(i64::from(value.x.0).to_ne_bytes())
                ^ u64::from_ne_bytes(i64::from(value.y.0).to_ne_bytes()).rotate_left(17)
        }
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value }
        | RichTextParam::Expr { source: value } => stable_text_hash(value),
    }
}

fn stable_text_hash(value: &str) -> u64 {
    value
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

fn normalized_direction(value: [f32; 2]) -> Result<[f32; 2], FramePlanError> {
    let length = value[0].hypot(value[1]);
    if !length.is_finite() || length <= f32::EPSILON {
        return Err(FramePlanError::InvalidRichTextEffectParameter {
            effect: "post_process".to_owned(),
            parameter: "dir",
        });
    }
    Ok([value[0] / length, value[1] / length])
}

fn checked_opacity(
    effect: &RichTextEffectDescriptor,
    parameter: &'static str,
    value: f32,
) -> Result<Opacity, FramePlanError> {
    let value = FiniteF32::try_new(value)?;
    Opacity::try_new(value).map_err(|_| invalid_effect_parameter(effect, parameter))
}

fn scale_milli(value: u16, factor: f32) -> u16 {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "validated opacity and milli input keep the rounded product in u16 range"
    )]
    {
        (f32::from(value) * factor).round() as u16
    }
}

fn rounded_u8(value: f32) -> u8 {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "sparkle channel formulas are bounded to the u8 domain"
    )]
    {
        value.round().clamp(0.0, 255.0) as u8
    }
}

fn invalid_effect_parameter(
    effect: &RichTextEffectDescriptor,
    parameter: &'static str,
) -> FramePlanError {
    FramePlanError::InvalidRichTextEffectParameter {
        effect: effect.id.clone(),
        parameter,
    }
}

fn push_unique<T: PartialEq>(target: &mut Vec<T>, value: T) {
    if !target.contains(&value) {
        target.push(value);
    }
}
