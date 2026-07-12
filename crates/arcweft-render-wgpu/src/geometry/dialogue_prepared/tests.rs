use std::collections::BTreeMap;

use arcweft_core::plan::RuntimeLineId;
use arcweft_presentation::{
    fx::{
        FiniteF32, FxApplication, FxApplicationResolver, FxColor, FxDefinition, FxDiagnostic,
        FxEvaluationBinding, FxGraph, FxGraphChildPath, FxInstanceSnapshot, FxLogicalTime, FxNode,
        FxProperty, FxResourceId, FxRuntimeValue, FxStaticValue, FxTarget, Length, Transform2D,
    },
    hit::HitRect,
};
use arcweft_render_text::{
    InlineFailurePolicy, LineDisplaySpec, Milli, RichTextControl, RichTextDocument,
    RichTextEffectDescriptor, RichTextEffectPhase, RichTextEffectTarget, RichTextLayout,
    RichTextNode, RichTextParam, RichTextShaderRef, RichTextStateScope, RichTextStyle,
    RichTextWritingMode, RuntimeLineContext, TextWeight,
};

use super::*;

const TEST_FONT: &[u8] = include_bytes!("../../../../../web/assets/noto-sans-jp-vf.ttf");

struct NoFxResolver;

impl FxApplicationResolver for NoFxResolver {
    fn resolve<'a>(
        &'a self,
        _application: &FxApplication,
    ) -> Result<FxEvaluationBinding<'a>, Box<FxDiagnostic>> {
        panic!("fixtures without typed Fx never invoke the resolver")
    }
}

struct TestFxResolver {
    definition: FxDefinition,
    instance: FxInstanceSnapshot,
    runtime_time: FxLogicalTime,
}

impl FxApplicationResolver for TestFxResolver {
    fn resolve<'a>(
        &'a self,
        application: &FxApplication,
    ) -> Result<FxEvaluationBinding<'a>, Box<FxDiagnostic>> {
        assert_eq!(application.definition(), self.definition.id());
        Ok(FxEvaluationBinding {
            definition: &self.definition,
            instance: &self.instance,
            runtime_time: self.runtime_time,
        })
    }
}

#[test]
fn vertical_ruby_stage_uses_canonical_layout_and_prepared_glyphs() {
    let frame = frame(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::Layout {
                layout: RichTextLayout {
                    writing_mode: RichTextWritingMode::VerticalRl,
                    ..RichTextLayout::default()
                },
            },
        },
        RichTextNode::Ruby {
            base: "漢字".to_owned(),
            ruby: "かんじ".to_owned(),
        },
        RichTextNode::Text {
            text: "ABC2026".to_owned(),
        },
    ]);
    let (item, complete, diagnostics) = prepare(&frame, 0, false, true, &NoFxResolver);

    assert!(complete);
    assert!(diagnostics.is_empty());
    assert!(!item.layout.ruby.is_empty());
    assert!(
        item.layout
            .runs
            .iter()
            .all(|run| run.writing_mode == RichTextWritingMode::VerticalRl)
    );
    assert_eq!(item.glyphs.len(), item.paint.glyphs.len());
    assert!(item.paint.glyphs.iter().all(|glyph| glyph.visible));
}

#[test]
fn reveal_changes_only_paint() {
    let frame = frame(vec![RichTextNode::Text {
        text: "after".to_owned(),
    }]);
    let (hidden, _, _) = prepare(&frame, 0, false, false, &NoFxResolver);
    let (complete, _, _) = prepare(&frame, 0, false, true, &NoFxResolver);

    assert_eq!(hidden.layout.hash, complete.layout.hash);
    assert_eq!(complete.interaction.text, "after");
    assert!(hidden.paint.glyphs.iter().all(|glyph| !glyph.visible));
    assert!(complete.paint.glyphs.iter().all(|glyph| glyph.visible));
}

#[test]
fn clear_projects_the_remaining_stage_to_the_textbox_origin() {
    let frame = frame(vec![
        RichTextNode::Text {
            text: "before".to_owned(),
        },
        RichTextNode::Control {
            control: RichTextControl::Clear,
        },
        RichTextNode::Text {
            text: "after".to_owned(),
        },
    ]);
    let (item, complete, _) = prepare(&frame, 0, false, true, &NoFxResolver);

    assert!(complete);
    assert_eq!(item.interaction.text, "after");
    assert!(
        item.layout
            .glyphs
            .iter()
            .all(|glyph| glyph.layout_bounds.x >= 20.0)
    );
}

#[test]
fn wave_uses_logical_glyph_ordinal_and_time_only_changes_paint() {
    let frame = frame(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::Effect {
                effect: RichTextEffectDescriptor {
                    id: "wave".to_owned(),
                    params: BTreeMap::from([
                        (
                            "amp".to_owned(),
                            RichTextParam::Milli {
                                value: Milli(4_000),
                            },
                        ),
                        (
                            "period".to_owned(),
                            RichTextParam::Milli {
                                value: Milli(8_000),
                            },
                        ),
                    ]),
                    target: RichTextEffectTarget::Glyph,
                    phase: RichTextEffectPhase::GlyphTransform,
                    state_scope: RichTextStateScope::Glyph,
                },
            },
        },
        RichTextNode::Text {
            text: "漢字".to_owned(),
        },
    ]);
    let (at_zero, _, _) = prepare(&frame, 0, false, true, &NoFxResolver);
    let (later, _, _) = prepare(&frame, 500, false, true, &NoFxResolver);

    assert_eq!(at_zero.layout.hash, later.layout.hash);
    assert_ne!(at_zero.paint, later.paint);
    let first_y = at_zero.paint.glyphs[0].transform.resolved().translation()[1].pixels();
    let second_y = at_zero.paint.glyphs[1].transform.resolved().translation()[1].pixels();
    assert!(first_y.abs() <= 0.001);
    assert!((second_y - std::f32::consts::FRAC_1_SQRT_2 * 4.0).abs() <= 0.001);
}

#[test]
fn typed_fx_application_changes_layout_style_and_post_layout_transform() {
    let id = arcweft_presentation::fx::FxId::try_new("test", "dialogue.emphasis").expect("Fx id");
    let transform = Transform2D {
        translate_x: Length::try_pixels(3.0).expect("translation"),
        ..Transform2D::default()
    };
    let graph = FxGraph::try_new(vec![
        FxNode::Text {
            properties: vec![FxProperty::new("weight", FxRuntimeValue::I32(700).into())],
        },
        FxNode::Transform {
            fx: id.clone(),
            properties: vec![
                FxProperty::new("target", FxStaticValue::Target(FxTarget::Glyph)),
                FxProperty::new("transform", FxRuntimeValue::Transform2D(transform).into()),
            ],
        },
    ])
    .expect("typed graph");
    let definition = FxDefinition::new(id.clone(), Vec::new(), graph).expect("definition");
    let application =
        FxApplication::try_new(id.clone(), Vec::new(), 0, None).expect("typed application");
    let instance = FxInstanceSnapshot {
        instance: application.derive_instance_id(["dialogue", "fixture", "occurrence.1"]),
        definition: id,
        abi_hash: definition.abi_hash(),
        activation_logical_time: FxLogicalTime::zero(),
        deterministic_seed: 17,
        parameters: Vec::new(),
        child_path: FxGraphChildPath::default(),
        provider_state: Vec::new(),
    };
    let resolver = TestFxResolver {
        definition,
        instance,
        runtime_time: FxLogicalTime::zero(),
    };
    let frame = frame(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::Fx { application },
        },
        RichTextNode::Text {
            text: "typed".to_owned(),
        },
        RichTextNode::StyleEnd {
            name: "fx".to_owned(),
        },
    ]);
    let (item, _, diagnostics) = prepare(&frame, 0, false, true, &resolver);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(
        item.layout
            .runs
            .iter()
            .all(|run| run.style.weight() == TextWeight::Bold)
    );
    assert!(item.paint.glyphs.iter().all(|glyph| {
        (glyph.transform.resolved().translation()[0].pixels() - 3.0).abs() <= 0.001
    }));
}

#[test]
fn legacy_shader_typewriter_and_post_process_share_prepared_paint() {
    let frame = frame(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::Shader {
                shader: RichTextShaderRef {
                    id: "soft_glow".to_owned(),
                    params: BTreeMap::from([
                        (
                            "amount".to_owned(),
                            RichTextParam::Milli { value: Milli(650) },
                        ),
                        (
                            "dir".to_owned(),
                            RichTextParam::Vec2 {
                                value: arcweft_render_text::RichTextVec2 {
                                    x: Milli::ONE,
                                    y: Milli::default(),
                                },
                            },
                        ),
                    ]),
                    phase: RichTextEffectPhase::RunOffscreenPass,
                },
            },
        },
        RichTextNode::StyleStart {
            style: RichTextStyle::Shader {
                shader: RichTextShaderRef {
                    id: "screen_tint".to_owned(),
                    params: BTreeMap::from([
                        (
                            "amount".to_owned(),
                            RichTextParam::Milli { value: Milli(550) },
                        ),
                        (
                            "color".to_owned(),
                            RichTextParam::Raw {
                                value: "#ff4050".to_owned(),
                            },
                        ),
                    ]),
                    phase: RichTextEffectPhase::PostProcess,
                },
            },
        },
        RichTextNode::StyleStart {
            style: RichTextStyle::Effect {
                effect: RichTextEffectDescriptor {
                    id: "typewriter".to_owned(),
                    params: BTreeMap::from([(
                        "cps".to_owned(),
                        RichTextParam::Milli { value: Milli::ONE },
                    )]),
                    target: RichTextEffectTarget::Glyph,
                    phase: RichTextEffectPhase::GlyphMask,
                    state_scope: RichTextStateScope::Run,
                },
            },
        },
        RichTextNode::Text {
            text: "AB".to_owned(),
        },
    ]);

    let (item, _, diagnostics) = prepare(&frame, 1_000, false, true, &NoFxResolver);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(item.paint.post_processes.len(), 1);
    assert!(
        item.paint
            .glyphs
            .iter()
            .all(|glyph| glyph.effects.len() == 4)
    );
    assert_eq!(
        item.paint
            .glyphs
            .iter()
            .filter(|glyph| glyph.visible)
            .count(),
        1
    );
    assert_eq!(item.submission().glyphs().len(), 5);
}

#[test]
fn typed_shader_resource_resolves_glyph_and_post_process_passes() {
    let id =
        arcweft_presentation::fx::FxId::try_new("test", "dialogue.source_glow").expect("Fx id");
    let shader_node = |stage: &str, amount: f32, color: [u8; 4]| FxNode::Shader {
        fx: id.clone(),
        properties: vec![
            FxProperty::new(
                "resource",
                FxStaticValue::Resource(
                    FxResourceId::try_new("shader.source_glow").expect("resource"),
                ),
            ),
            FxProperty::new("stage", FxStaticValue::Selector(stage.to_owned())),
            FxProperty::new(
                "uniforms",
                FxStaticValue::Record(vec![
                    FxProperty::new(
                        "amount",
                        FxRuntimeValue::F32(FiniteF32::try_new(amount).expect("finite")).into(),
                    ),
                    FxProperty::new(
                        "color",
                        FxRuntimeValue::Color(FxColor::from_rgba8(color)).into(),
                    ),
                ]),
            ),
        ],
    };
    let graph = FxGraph::try_new(vec![
        shader_node("glyph_color", 0.9, [96, 64, 255, 255]),
        shader_node("post_process", 0.65, [64, 176, 255, 255]),
    ])
    .expect("typed graph");
    let definition = FxDefinition::new(id.clone(), Vec::new(), graph).expect("definition");
    let application =
        FxApplication::try_new(id.clone(), Vec::new(), 0, None).expect("typed application");
    let instance = FxInstanceSnapshot {
        instance: application.derive_instance_id(["dialogue", "fixture", "source.glow"]),
        definition: id,
        abi_hash: definition.abi_hash(),
        activation_logical_time: FxLogicalTime::zero(),
        deterministic_seed: 91,
        parameters: Vec::new(),
        child_path: FxGraphChildPath::default(),
        provider_state: Vec::new(),
    };
    let resolver = TestFxResolver {
        definition,
        instance,
        runtime_time: FxLogicalTime::zero(),
    };
    let frame = frame(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::Fx { application },
        },
        RichTextNode::Text {
            text: "source".to_owned(),
        },
        RichTextNode::StyleEnd {
            name: "fx".to_owned(),
        },
    ]);

    let (item, _, diagnostics) = prepare(&frame, 0, false, true, &resolver);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(
        item.paint
            .glyphs
            .iter()
            .all(|glyph| glyph.effects.len() == 1)
    );
    assert_eq!(item.paint.post_processes.len(), 1);
}

#[test]
fn missing_legacy_shader_is_a_typed_diagnostic() {
    let frame = frame(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::Shader {
                shader: RichTextShaderRef {
                    id: "missing.shader".to_owned(),
                    params: BTreeMap::new(),
                    phase: RichTextEffectPhase::GlyphColor,
                },
            },
        },
        RichTextNode::Text {
            text: "missing".to_owned(),
        },
    ]);

    let (item, _, diagnostics) = prepare(&frame, 0, false, true, &NoFxResolver);

    assert!(
        item.paint
            .glyphs
            .iter()
            .all(|glyph| glyph.effects.is_empty())
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == arcweft_presentation::fx::FxDiagnosticCode::MissingProvider
            && diagnostic.message.contains("missing.shader")
    }));
}

fn prepare(
    frame: &arcweft_render_text::LineDisplayFrame,
    visual_time_millis: u64,
    reduce_motion: bool,
    reveal_complete: bool,
    resolver: &dyn FxApplicationResolver,
) -> (PreparedTextItem, bool, Vec<FxDiagnostic>) {
    let stage = frame.stage(0).expect("stage");
    let mut engine =
        GlyphonTextEngine::from_project_fonts("ja", vec![TEST_FONT.to_vec()]).expect("font engine");
    let request = PreparedRichTextStageRequest {
        bounds: HitRect::new(20.0, 30.0, 360.0, 180.0),
        default_style: ResolvedTextStyle::new(vec![TextFontFamily::SansSerif], 24_000, 32_000)
            .expect("paragraph style resolves")
            .with_color(TextColor::rgba(245, 245, 245, 255)),
        visual_time_millis,
        reveal_complete,
    };
    let (item, complete, diagnostics, _) = prepare_stage(
        &mut engine,
        stage,
        &request,
        viewport(),
        reduce_motion,
        resolver,
    )
    .expect("stage prepares");
    (item, complete, diagnostics)
}

fn frame(nodes: Vec<RichTextNode>) -> arcweft_render_text::LineDisplayFrame {
    LineDisplaySpec {
        line: RuntimeLineId::canonical("prepared.dialogue.test").expect("line id"),
        callee: "narrator".to_owned(),
        speaker_label: None,
        text_key: None,
        window: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: Some(InlineFailurePolicy::FailLine),
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(nodes),
    }
    .resolve_frame(&RuntimeLineContext::default())
    .expect("frame resolves")
}

const fn viewport() -> RenderViewport {
    RenderViewport {
        logical_width: 640.0,
        logical_height: 360.0,
        physical_width: 1_280,
        physical_height: 720,
        scale_factor: 2.0,
    }
}
