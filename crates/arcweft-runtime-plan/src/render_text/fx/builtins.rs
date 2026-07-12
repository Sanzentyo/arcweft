//! Compilation of the built-in rich-text shorthand into ordinary typed Fx graphs.

mod inventory;
mod program;

use std::collections::BTreeMap;

use arcweft_presentation::fx::{FxDefinition, FxId, FxPhase, FxTarget};

use crate::{errors::RuntimePlanLowerError, render_text::attrs::parse_attrs};

pub(crate) use inventory::{builtin_rich_text_fx_definitions, builtin_selector};

/// Result of classifying one authored rich-text effect opener.
#[derive(Clone, Debug)]
pub(crate) enum BuiltinRichTextFx {
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
) -> Result<BuiltinRichTextFx, RuntimePlanLowerError> {
    let selector = selector.trim().trim_start_matches('.');
    let attrs = parse_attrs(raw_attrs);
    let phase = effect_phase(selector, &attrs)?;
    if phase == FxPhase::Transition {
        return Ok(BuiltinRichTextFx::HostEvent);
    }
    reject_removed_state_scope(&attrs)?;
    let effect = selector.to_owned();
    let target = effect_target(&attrs, phase)?;
    let key = canonical_semantic_key(&effect, phase, target, &attrs);
    let id = FxId::derive_builtin(&format!("rich_text.{effect}"), &key).map_err(|error| {
        fx_error(format!(
            "invalid built-in Fx identity for `{effect}`: {error}"
        ))
    })?;
    if selector != "shader" && !program::is_builtin_effect(&effect) {
        return Ok(BuiltinRichTextFx::MissingDefinition(id));
    }
    let graph = if selector == "shader" {
        program::shader_graph(&id, phase, target, &attrs)?
    } else {
        program::effect_graph(&id, &effect, phase, target, &attrs)?
    };
    let definition = FxDefinition::new(id, Vec::new(), graph)
        .map_err(|error| fx_error(format!("invalid built-in `{effect}` Fx graph: {error}")))?;
    Ok(BuiltinRichTextFx::Definition(definition))
}

fn effect_phase(
    selector: &str,
    attrs: &BTreeMap<String, String>,
) -> Result<FxPhase, RuntimePlanLowerError> {
    let Some(value) = attrs.get("phase") else {
        return Ok(if selector == "typewriter" {
            FxPhase::GlyphMask
        } else if selector == "shader" {
            FxPhase::OffscreenPass
        } else {
            FxPhase::GlyphTransform
        });
    };
    match value.trim().trim_start_matches('.') {
        "before_layout" => Ok(FxPhase::BeforeLayout),
        "layout_transform" => Ok(FxPhase::LayoutTransform),
        "glyph_transform" => Ok(FxPhase::GlyphTransform),
        "glyph_color" => Ok(FxPhase::GlyphColor),
        "glyph_mask" => Ok(FxPhase::GlyphMask),
        "offscreen_pass" | "run_offscreen_pass" => Ok(FxPhase::OffscreenPass),
        "post_process" => Ok(FxPhase::PostProcess),
        "host_event" => Ok(FxPhase::Transition),
        value => Err(fx_error(format!(
            "rich-text effect phase `{value}` is not in the closed Fx phase set"
        ))),
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
                "rich-text effect target `{value}` was removed; use node, content, background, line, glyph, or viewport"
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

fn reject_removed_state_scope(
    attrs: &BTreeMap<String, String>,
) -> Result<(), RuntimePlanLowerError> {
    if let Some((name, _)) = attrs
        .iter()
        .find(|(name, _)| matches!(name.as_str(), "state" | "scope" | "state_scope"))
    {
        return Err(fx_error(format!(
            "rich-text effect `{name}` was removed; Fx state is owned by the stable per-occurrence FxInstanceId"
        )));
    }
    Ok(())
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

    use super::{BuiltinRichTextFx, compile_builtin_rich_text_fx};

    #[test]
    fn wave_shorthand_compiles_to_one_typed_sampler_definition() {
        let BuiltinRichTextFx::Definition(definition) = compile_builtin_rich_text_fx(
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
        let BuiltinRichTextFx::Definition(definition) = compile_builtin_rich_text_fx(
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
    fn removed_target_and_state_scope_fail_instead_of_aliasing() {
        assert!(
            compile_builtin_rich_text_fx("wave", "target=run")
                .expect_err("run target was replaced")
                .to_string()
                .contains("target `run` was removed")
        );
        assert!(
            compile_builtin_rich_text_fx("shake", "state_scope=glyph")
                .expect_err("state scope was replaced by stable identity")
                .to_string()
                .contains("stable per-occurrence FxInstanceId")
        );
    }

    #[test]
    fn host_event_and_unknown_provider_remain_visibly_distinct() {
        assert!(matches!(
            compile_builtin_rich_text_fx("host", "id=beat phase=host_event channel=debug")
                .expect("host event classifies"),
            BuiltinRichTextFx::HostEvent
        ));
        assert!(matches!(
            compile_builtin_rich_text_fx("missing_fx", "amp=2px")
                .expect("unknown provider remains a runtime definition miss"),
            BuiltinRichTextFx::MissingDefinition(_)
        ));
        assert!(matches!(
            compile_builtin_rich_text_fx("host", "id=sparkle amp=2px")
                .expect("visual host selector must not fall back by basename"),
            BuiltinRichTextFx::MissingDefinition(_)
        ));
    }
}
