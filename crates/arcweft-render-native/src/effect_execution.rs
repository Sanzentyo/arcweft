use super::{
    NativeAnimationSample, NativeGlyphPlacement, NativeShaderGlyphPass, NativeVisualDiagnostic,
    NativeVisualDiagnosticSeverity, RichTextEffectRegistry, RichTextMotionRegistry,
    RichTextShaderRegistry, RichTextStateStore, TextEffectGlyphContext,
    TextEffectPostProcessContext, TextMotionContext, TextShaderContext,
    TextShaderPostProcessContext, apply_builtin_effect_post_process,
    builtin_effect_phase_supported, effect_applies_to_renderer_glyph, is_builtin_effect_id,
    shader_phase_known,
};
use arcweft_render_text::{RichTextEffectDescriptor, RichTextEffectPhase, RichTextShaderRef};
use std::collections::BTreeSet;

pub(super) struct NativeEffectExecution<'a> {
    pub(super) registry: Option<&'a mut RichTextEffectRegistry>,
    pub(super) shader_registry: Option<&'a mut RichTextShaderRegistry>,
    pub(super) motion_registry: Option<&'a mut RichTextMotionRegistry>,
    pub(super) state: &'a mut RichTextStateStore,
    pub(super) diagnostics: Vec<NativeVisualDiagnostic>,
    pub(super) seen_diagnostics: BTreeSet<String>,
}

impl<'a> NativeEffectExecution<'a> {
    pub(super) fn new(
        registry: Option<&'a mut RichTextEffectRegistry>,
        shader_registry: Option<&'a mut RichTextShaderRegistry>,
        motion_registry: Option<&'a mut RichTextMotionRegistry>,
        state: &'a mut RichTextStateStore,
    ) -> Self {
        Self {
            registry,
            shader_registry,
            motion_registry,
            state,
            diagnostics: Vec::new(),
            seen_diagnostics: BTreeSet::new(),
        }
    }

    pub(super) fn into_diagnostics(self) -> Vec<NativeVisualDiagnostic> {
        self.diagnostics
    }

    pub(super) fn apply_custom_effect(
        &mut self,
        line_id: &str,
        effect: &RichTextEffectDescriptor,
        glyph_count: usize,
        time_seconds: f32,
        placement: &mut NativeGlyphPlacement,
    ) {
        if effect.phase == RichTextEffectPhase::PostProcess {
            return;
        }
        if !effect_applies_to_renderer_glyph(effect) {
            self.push_diagnostic(
                "unsupported_custom_effect_phase",
                NativeVisualDiagnosticSeverity::Warning,
                effect,
                format!(
                    "custom rich-text effect `{}` uses unsupported native glyph phase {:?}",
                    effect.id, effect.phase
                ),
            );
            return;
        }
        let Some(registry) = self.registry.as_deref_mut() else {
            self.push_diagnostic(
                "missing_custom_effect_registry",
                NativeVisualDiagnosticSeverity::Warning,
                effect,
                format!(
                    "custom rich-text effect `{}` has no native effect registry",
                    effect.id
                ),
            );
            return;
        };
        if !registry.contains(&effect.id) {
            self.push_diagnostic(
                "missing_custom_effect",
                NativeVisualDiagnosticSeverity::Warning,
                effect,
                format!(
                    "custom rich-text effect `{}` is not registered in the native effect registry",
                    effect.id
                ),
            );
            return;
        }
        let mut ctx = TextEffectGlyphContext {
            effect,
            time_seconds,
            line_id,
            run_index: placement.run_index,
            glyph_index: placement.glyph_index,
            glyph_count,
            state: self.state,
            placement,
        };
        registry.apply_host_effect(&effect.id, &mut ctx);
    }

    pub(super) fn apply_effect_post_processes<'b>(
        &mut self,
        line_id: &str,
        effects: impl IntoIterator<Item = &'b RichTextEffectDescriptor>,
        width: u32,
        height: u32,
        time_seconds: f32,
        rgba: &mut [u8],
    ) {
        for effect in effects {
            self.apply_effect_post_process(line_id, effect, width, height, time_seconds, rgba);
        }
    }

    pub(super) fn apply_effect_post_process(
        &mut self,
        line_id: &str,
        effect: &RichTextEffectDescriptor,
        width: u32,
        height: u32,
        time_seconds: f32,
        rgba: &mut [u8],
    ) {
        if effect.phase != RichTextEffectPhase::PostProcess {
            return;
        }
        if is_builtin_effect_id(&effect.id) {
            apply_builtin_effect_post_process(effect, width, height, time_seconds, rgba);
            return;
        }
        let Some(registry) = self.registry.as_deref_mut() else {
            self.push_diagnostic(
                "missing_custom_effect_registry",
                NativeVisualDiagnosticSeverity::Warning,
                effect,
                format!(
                    "custom rich-text effect `{}` has no native effect registry",
                    effect.id
                ),
            );
            return;
        };
        if !registry.contains(&effect.id) {
            self.push_diagnostic(
                "missing_custom_effect",
                NativeVisualDiagnosticSeverity::Warning,
                effect,
                format!(
                    "custom rich-text effect `{}` is not registered in the native effect registry",
                    effect.id
                ),
            );
            return;
        }
        if !registry.supports_phase(&effect.id, effect.phase) {
            self.push_diagnostic(
                "unsupported_custom_effect_phase",
                NativeVisualDiagnosticSeverity::Warning,
                effect,
                format!(
                    "custom rich-text effect `{}` uses unsupported native phase {:?}",
                    effect.id, effect.phase
                ),
            );
            return;
        }
        let mut ctx = TextEffectPostProcessContext {
            effect,
            time_seconds,
            line_id,
            width,
            height,
            state: self.state,
        };
        let _ = registry.post_process(&effect.id, &mut ctx, rgba);
    }

    pub(super) fn observe_builtin_effect_phase(
        &mut self,
        effect: &RichTextEffectDescriptor,
    ) -> bool {
        if builtin_effect_phase_supported(effect) {
            return true;
        }
        self.push_diagnostic(
            "unsupported_builtin_effect_phase",
            NativeVisualDiagnosticSeverity::Warning,
            effect,
            format!(
                "builtin rich-text effect `{}` uses unsupported native phase {:?}",
                effect.id, effect.phase
            ),
        );
        false
    }

    pub(super) fn observe_shaders<'b>(
        &mut self,
        shaders: impl IntoIterator<Item = &'b RichTextShaderRef>,
    ) {
        for shader in shaders {
            self.observe_shader(shader);
        }
    }

    pub(super) fn observe_shader(&mut self, shader: &RichTextShaderRef) {
        if !shader_phase_known(shader.phase) {
            self.push_shader_diagnostic(
                "unsupported_shader_phase",
                NativeVisualDiagnosticSeverity::Warning,
                shader,
                format!(
                    "rich-text shader `{}` uses unsupported native phase {:?}",
                    shader.id, shader.phase
                ),
            );
            return;
        }
        let Some(registry) = self.shader_registry.as_deref() else {
            self.push_shader_diagnostic(
                "missing_shader_registry",
                NativeVisualDiagnosticSeverity::Warning,
                shader,
                format!(
                    "rich-text shader `{}` has no native shader registry",
                    shader.id
                ),
            );
            return;
        };
        if registry.contains(&shader.id) {
            if !registry.supports_phase(&shader.id, shader.phase) {
                self.push_shader_diagnostic(
                    "unsupported_shader_phase",
                    NativeVisualDiagnosticSeverity::Warning,
                    shader,
                    format!(
                        "rich-text shader `{}` uses unsupported native phase {:?}",
                        shader.id, shader.phase
                    ),
                );
            }
            return;
        }
        self.push_shader_diagnostic(
            "missing_shader",
            NativeVisualDiagnosticSeverity::Warning,
            shader,
            format!(
                "rich-text shader `{}` is not registered in the native shader registry",
                shader.id
            ),
        );
    }

    pub(super) fn shader_glyph_passes(
        &mut self,
        shader: &RichTextShaderRef,
    ) -> Vec<NativeShaderGlyphPass> {
        if shader.phase != RichTextEffectPhase::RunOffscreenPass {
            return Vec::new();
        }
        let Some(registry) = self.shader_registry.as_deref_mut() else {
            return Vec::new();
        };
        registry
            .glyph_passes(&shader.id, &TextShaderContext { shader })
            .unwrap_or_default()
    }

    pub(super) fn shader_glyph_color(&mut self, shader: &RichTextShaderRef) -> Option<[u8; 4]> {
        if shader.phase != RichTextEffectPhase::GlyphColor {
            return None;
        }
        let registry = self.shader_registry.as_deref_mut()?;
        registry
            .glyph_passes(&shader.id, &TextShaderContext { shader })
            .and_then(|passes| passes.into_iter().next().map(|pass| pass.color))
    }

    pub(super) fn apply_shader_post_processes<'b>(
        &mut self,
        shaders: impl IntoIterator<Item = &'b RichTextShaderRef>,
        width: u32,
        height: u32,
        time_seconds: f32,
        rgba: &mut [u8],
    ) {
        for shader in shaders {
            self.apply_shader_post_process(shader, width, height, time_seconds, rgba);
        }
    }

    pub(super) fn apply_shader_post_process(
        &mut self,
        shader: &RichTextShaderRef,
        width: u32,
        height: u32,
        time_seconds: f32,
        rgba: &mut [u8],
    ) {
        if shader.phase != RichTextEffectPhase::PostProcess {
            return;
        }
        self.observe_shader(shader);
        let Some(registry) = self.shader_registry.as_deref_mut() else {
            return;
        };
        if !registry.supports_phase(&shader.id, shader.phase) {
            return;
        }
        let ctx = TextShaderPostProcessContext {
            shader,
            width,
            height,
            time_seconds,
        };
        let _ = registry.post_process(&shader.id, &ctx, rgba);
    }

    pub(super) fn sample_motion_function(
        &mut self,
        effect: &RichTextEffectDescriptor,
        function: &str,
        ctx: &TextMotionContext<'_>,
    ) -> Option<NativeAnimationSample> {
        let Some(registry) = self.motion_registry.as_deref_mut() else {
            self.push_motion_diagnostic(
                "missing_motion_registry",
                NativeVisualDiagnosticSeverity::Warning,
                effect,
                function,
                format!("rich-text motion function `{function}` has no native motion registry"),
            );
            return None;
        };
        let Some(sample) = registry.sample(function, ctx) else {
            self.push_motion_diagnostic(
                "missing_motion_function",
                NativeVisualDiagnosticSeverity::Warning,
                effect,
                function,
                format!(
                    "rich-text motion function `{function}` is not registered in the native motion registry"
                ),
            );
            return None;
        };
        Some(sample)
    }

    pub(super) fn push_diagnostic(
        &mut self,
        code: &str,
        severity: NativeVisualDiagnosticSeverity,
        effect: &RichTextEffectDescriptor,
        message: String,
    ) {
        let key = format!("{code}:{}:{:?}", effect.id, effect.phase);
        if !self.seen_diagnostics.insert(key) {
            return;
        }
        self.diagnostics.push(NativeVisualDiagnostic {
            severity,
            code: code.to_owned(),
            message,
            effect_id: Some(effect.id.clone()),
        });
    }

    pub(super) fn push_shader_diagnostic(
        &mut self,
        code: &str,
        severity: NativeVisualDiagnosticSeverity,
        shader: &RichTextShaderRef,
        message: String,
    ) {
        let key = format!("{code}:{}:{:?}", shader.id, shader.phase);
        if !self.seen_diagnostics.insert(key) {
            return;
        }
        self.diagnostics.push(NativeVisualDiagnostic {
            severity,
            code: code.to_owned(),
            message,
            effect_id: Some(shader.id.clone()),
        });
    }

    pub(super) fn push_motion_diagnostic(
        &mut self,
        code: &str,
        severity: NativeVisualDiagnosticSeverity,
        effect: &RichTextEffectDescriptor,
        function: &str,
        message: String,
    ) {
        let key = format!("{code}:{}:{function}:{:?}", effect.id, effect.phase);
        if !self.seen_diagnostics.insert(key) {
            return;
        }
        self.diagnostics.push(NativeVisualDiagnostic {
            severity,
            code: code.to_owned(),
            message,
            effect_id: Some(effect.id.clone()),
        });
    }
}
