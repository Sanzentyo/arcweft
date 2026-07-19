//! Compilation of the built-in rich-text shorthand into ordinary typed Fx graphs.

mod inventory;
mod program;

use std::collections::BTreeMap;

use arcweft_presentation::fx::{FxDefinition, FxId, FxPhase, FxTarget};
use arcweft_presentation::rich_text::{BuiltinRichTextFx, BuiltinRichTextFxPhase};

use crate::{errors::RuntimePlanLowerError, render_text::attrs::parse_attrs};

pub(crate) use inventory::{builtin_rich_text_fx_definitions, builtin_selector};

/// Result of classifying one authored rich-text effect opener.
#[derive(Clone, Debug)]
pub(crate) enum CompiledBuiltinRichTextFx {
    /// The tag remains a runtime host event and does not create a visual Fx graph.
    HostEvent,
    /// Arcweft owns the complete executable graph.
    Definition(FxDefinition),
    /// An external effect has no bundled definition and must diagnose visibly.
    MissingDefinition(FxId),
}

/// Compiles one legacy-surface effect selector directly into the final Fx model.
pub(crate) fn compile_builtin_rich_text_fx(
    selector: &str,
    raw_attrs: &str,
) -> Result<CompiledBuiltinRichTextFx, RuntimePlanLowerError> {
    let selector = selector.trim().trim_start_matches('.');
    let attrs = parse_attrs(raw_attrs);
    let builtin = BuiltinRichTextFx::from_selector(selector);
    let phase = effect_phase(builtin, &attrs)?;
    if phase == BuiltinRichTextFxPhase::HostEvent {
        return Ok(CompiledBuiltinRichTextFx::HostEvent);
    }
    let effect = selector.to_owned();
    let presentation_phase = presentation_phase(phase);
    let target = effect_target(&attrs, presentation_phase)?;
    let key = canonical_semantic_key(&effect, presentation_phase, target, &attrs);
    let id = FxId::derive_builtin(&format!("rich_text.{effect}"), &key).map_err(|error| {
        fx_error(format!(
            "invalid built-in Fx identity for `{effect}`: {error}"
        ))
    })?;
    let Some(builtin) = builtin else {
        return Ok(CompiledBuiltinRichTextFx::MissingDefinition(id));
    };
    let graph = if builtin == BuiltinRichTextFx::Shader {
        program::shader_graph(&id, builtin, phase, target, &attrs)?
    } else {
        program::effect_graph(&id, builtin, phase, target, &attrs)?
    };
    let definition = FxDefinition::new(id, Vec::new(), graph)
        .map_err(|error| fx_error(format!("invalid built-in `{effect}` Fx graph: {error}")))?;
    Ok(CompiledBuiltinRichTextFx::Definition(definition))
}

fn effect_phase(
    builtin: Option<BuiltinRichTextFx>,
    attrs: &BTreeMap<String, String>,
) -> Result<BuiltinRichTextFxPhase, RuntimePlanLowerError> {
    let Some(value) = attrs.get("phase") else {
        return Ok(builtin.map_or(
            BuiltinRichTextFxPhase::GlyphTransform,
            BuiltinRichTextFx::default_phase,
        ));
    };
    BuiltinRichTextFxPhase::from_source_name(value).ok_or_else(|| {
        fx_error(format!(
            "rich-text effect phase `{value}` is not in the closed Fx phase set"
        ))
    })
}

pub(super) const fn presentation_phase(phase: BuiltinRichTextFxPhase) -> FxPhase {
    match phase {
        BuiltinRichTextFxPhase::BeforeLayout => FxPhase::BeforeLayout,
        BuiltinRichTextFxPhase::LayoutTransform => FxPhase::LayoutTransform,
        BuiltinRichTextFxPhase::GlyphTransform => FxPhase::GlyphTransform,
        BuiltinRichTextFxPhase::GlyphColor => FxPhase::GlyphColor,
        BuiltinRichTextFxPhase::GlyphMask => FxPhase::GlyphMask,
        BuiltinRichTextFxPhase::OffscreenPass => FxPhase::OffscreenPass,
        BuiltinRichTextFxPhase::PostProcess => FxPhase::PostProcess,
        BuiltinRichTextFxPhase::HostEvent => FxPhase::Transition,
    }
}

fn effect_target(
    attrs: &BTreeMap<String, String>,
    phase: FxPhase,
) -> Result<FxTarget, RuntimePlanLowerError> {
    let authored = attrs
        .get("target")
        .map(|value| value.trim().trim_start_matches('.'));
    let target = match authored {
        None if phase == FxPhase::PostProcess => FxTarget::Viewport,
        None | Some("content") => FxTarget::Content,
        Some("node") => FxTarget::Node,
        Some("background") => FxTarget::Background,
        Some("line") => FxTarget::Line,
        Some("glyph") => FxTarget::Glyph,
        Some("viewport") => FxTarget::Viewport,
        Some(value) => {
            return Err(fx_error(format!(
                "rich-text effect target `{value}` is not in the closed Fx target set: node, content, background, line, glyph, viewport"
            )));
        }
    };
    if phase == FxPhase::PostProcess && target != FxTarget::Viewport {
        return Err(fx_error(
            "rich-text post-process effects must target `viewport`",
        ));
    }
    if phase != FxPhase::PostProcess && target == FxTarget::Viewport {
        return Err(fx_error(
            "only post-process rich-text effects may target `viewport`",
        ));
    }
    Ok(target)
}

fn canonical_semantic_key(
    effect: &str,
    phase: FxPhase,
    target: FxTarget,
    attrs: &BTreeMap<String, String>,
) -> Vec<u8> {
    let mut key = format!("effect={effect}\nphase={phase:?}\ntarget={target:?}\n").into_bytes();
    for (name, value) in attrs {
        key.extend_from_slice(name.as_bytes());
        key.push(b'=');
        key.extend_from_slice(value.trim().as_bytes());
        key.push(b'\n');
    }
    key
}

pub(super) fn fx_error(message: impl Into<String>) -> RuntimePlanLowerError {
    RuntimePlanLowerError::new(format!("rich-text built-in Fx: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use arcweft_presentation::fx::{FxNode, FxPhase, FxStaticValue, FxTarget};
    use arcweft_presentation::rich_text::BuiltinRichTextFx;

    use super::{CompiledBuiltinRichTextFx, compile_builtin_rich_text_fx};

    #[test]
    fn wave_shorthand_compiles_to_one_typed_sampler_definition() {
        let CompiledBuiltinRichTextFx::Definition(definition) = compile_builtin_rich_text_fx(
            "wave",
            "amp=4px period=8 speed=1.6 target=glyph phase=glyph_transform",
        )
        .expect("wave compiles") else {
            panic!("wave is an Arcweft builtin");
        };
        let [FxNode::Transform { properties, .. }] = definition.graph().nodes() else {
            panic!("wave compiles to a transform node");
        };
        assert!(properties.iter().any(|property| {
            property.name() == "sampler" && matches!(property.value(), FxStaticValue::Sampler(_))
        }));
        assert!(properties.iter().any(|property| {
            property.name() == "target"
                && property.value() == &FxStaticValue::Target(FxTarget::Glyph)
        }));
    }

    #[test]
    fn shader_resource_and_phase_are_closed_graph_properties() {
        let CompiledBuiltinRichTextFx::Definition(definition) = compile_builtin_rich_text_fx(
            "shader",
            "id=soft_glow amount=0.6 dir=1,0 phase=run_offscreen_pass",
        )
        .expect("shader compiles") else {
            panic!("shader shorthand has a typed definition");
        };
        let [FxNode::Shader { properties, .. }] = definition.graph().nodes() else {
            panic!("shader compiles to a shader node");
        };
        assert!(properties.iter().any(|property| {
            property.name() == "phase"
                && property.value() == &FxStaticValue::Phase(FxPhase::OffscreenPass)
        }));
    }

    #[test]
    fn target_values_outside_the_closed_current_set_fail() {
        let error = compile_builtin_rich_text_fx("wave", "target=elsewhere")
            .expect_err("unknown target value must diagnose");
        assert!(
            error
                .to_string()
                .contains("not in the closed Fx target set")
        );
    }

    #[test]
    fn host_event_and_unknown_provider_remain_visibly_distinct() {
        assert!(matches!(
            compile_builtin_rich_text_fx("host", "id=beat phase=host_event channel=debug")
                .expect("host event classifies"),
            CompiledBuiltinRichTextFx::HostEvent
        ));
        assert!(matches!(
            compile_builtin_rich_text_fx("missing_fx", "amp=2px")
                .expect("unknown provider remains a runtime definition miss"),
            CompiledBuiltinRichTextFx::MissingDefinition(_)
        ));
        assert!(matches!(
            compile_builtin_rich_text_fx("host", "id=sparkle amp=2px")
                .expect("visual host selector must not fall back by basename"),
            CompiledBuiltinRichTextFx::MissingDefinition(_)
        ));
    }

    #[test]
    fn every_owned_builtin_compiles_with_its_default_phase() {
        for effect in BuiltinRichTextFx::ALL {
            let attrs = if effect == BuiltinRichTextFx::Shader {
                "id=soft_glow"
            } else {
                ""
            };
            assert!(matches!(
                compile_builtin_rich_text_fx(effect.selector(), attrs)
                    .unwrap_or_else(|error| panic!("{} failed: {error}", effect.selector())),
                CompiledBuiltinRichTextFx::Definition(_)
            ));
        }
    }

    #[test]
    fn every_owned_builtin_compiles_with_authored_attributes() {
        for effect in BuiltinRichTextFx::ALL {
            let phase = effect.default_phase().source_name();
            let attrs = if effect == BuiltinRichTextFx::Shader {
                format!("id=soft_glow phase={phase}")
            } else {
                format!("phase={phase}")
            };
            assert!(matches!(
                compile_builtin_rich_text_fx(effect.selector(), &attrs)
                    .unwrap_or_else(|error| panic!("{} failed: {error}", effect.selector())),
                CompiledBuiltinRichTextFx::Definition(_)
            ));
        }
    }

    #[test]
    fn owned_property_schema_rejects_unknown_names() {
        for effect in BuiltinRichTextFx::ALL {
            let attrs = if effect == BuiltinRichTextFx::Shader {
                "id=soft_glow invented=1"
            } else {
                "invented=1"
            };
            let error = compile_builtin_rich_text_fx(effect.selector(), attrs)
                .expect_err("unknown owned property must diagnose");
            assert!(error.to_string().contains("no property named `invented`"));
        }
    }
}
