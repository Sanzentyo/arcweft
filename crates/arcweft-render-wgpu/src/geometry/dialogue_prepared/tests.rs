use arcweft_core::plan::RuntimeLineId;
use arcweft_dialogue::InlineFailurePolicy;
use arcweft_presentation::{
    fx::{
        Angle, FiniteF32, FxApplication, FxApplicationResolver, FxColor, FxContextSlot,
        FxDefinition, FxDiagnostic, FxEvaluationBinding, FxGraph, FxGraphChildPath,
        FxInstanceSnapshot, FxLogicalTime, FxNode, FxPhase, FxProperty, FxResourceId,
        FxRuntimeType, FxRuntimeValue, FxSamplerProgram, FxStaticValue, FxTarget, Length,
        Transform2D, ValueInstruction, ValueProgramSchema,
    },
    hit::HitRect,
};
use arcweft_render_text::{
    LineDisplaySpec, RichTextControl, RichTextDocument, RichTextLayout, RichTextNode,
    RichTextStyle, RichTextWritingMode, RuntimeLineContext, TextWeight,
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

fn constant(value: FxRuntimeValue) -> ValueInstruction {
    ValueInstruction::Constant { value }
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::try_new(value).expect("test value is finite")
}

fn resolver(
    definition: FxDefinition,
    application: &FxApplication,
    runtime_millis: u64,
) -> TestFxResolver {
    let instance = FxInstanceSnapshot {
        instance: application.derive_instance_id(["dialogue", "typed-fixture"]),
        definition: definition.id().clone(),
        abi_hash: definition.abi_hash(),
        activation_logical_time: FxLogicalTime::zero(),
        deterministic_seed: 17,
        parameters: application.parameters().to_vec(),
        child_path: FxGraphChildPath::default(),
        provider_state: Vec::new(),
    };
    TestFxResolver {
        definition,
        instance,
        runtime_time: FxLogicalTime::zero()
            .try_advance_millis(runtime_millis)
            .expect("test logical time is finite"),
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
fn clear_projects_the_remaining_stage_to_the_dialogue_view_origin() {
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
fn typed_sampler_uses_logical_glyph_ordinal_and_time_only_changes_paint() {
    let id = arcweft_presentation::fx::FxId::try_new("test", "dialogue.wave").expect("Fx id");
    let sampler = FxSamplerProgram::validate(
        ValueProgramSchema::new(Vec::new(), Vec::new(), FxRuntimeType::Transform2D),
        vec![
            constant(FxRuntimeValue::Length(Length::ZERO)),
            ValueInstruction::LoadContext {
                slot: FxContextSlot::Ordinal,
            },
            constant(FxRuntimeValue::F32(finite(8.0))),
            ValueInstruction::Div,
            ValueInstruction::LoadContext {
                slot: FxContextSlot::Time,
            },
            ValueInstruction::Add,
            constant(FxRuntimeValue::F32(finite(std::f32::consts::TAU))),
            ValueInstruction::Mul,
            ValueInstruction::Sin,
            constant(FxRuntimeValue::Length(
                Length::try_pixels(4.0).expect("amplitude"),
            )),
            ValueInstruction::Mul,
            constant(FxRuntimeValue::F32(FiniteF32::ONE)),
            constant(FxRuntimeValue::F32(FiniteF32::ONE)),
            constant(FxRuntimeValue::Angle(Angle::ZERO)),
            constant(FxRuntimeValue::Angle(Angle::ZERO)),
            constant(FxRuntimeValue::Angle(Angle::ZERO)),
            constant(FxRuntimeValue::Length(Length::ZERO)),
            constant(FxRuntimeValue::Length(Length::ZERO)),
            constant(FxRuntimeValue::F32(FiniteF32::ONE)),
            ValueInstruction::MakeTransform2D,
            ValueInstruction::Return,
        ],
    )
    .expect("wave sampler validates");
    let graph = FxGraph::try_new(vec![FxNode::Transform {
        fx: id.clone(),
        properties: vec![
            FxProperty::new("target", FxStaticValue::Target(FxTarget::Glyph)),
            FxProperty::new("phase", FxStaticValue::Phase(FxPhase::GlyphTransform)),
            FxProperty::new("sampler", FxStaticValue::Sampler(sampler)),
        ],
    }])
    .expect("wave graph");
    let definition = FxDefinition::new(id.clone(), Vec::new(), graph).expect("definition");
    let application = FxApplication::try_new(id, Vec::new(), 0, None).expect("application");
    let frame = frame(vec![
        RichTextNode::Text {
            text: "前".to_owned(),
        },
        RichTextNode::StyleStart {
            style: RichTextStyle::Fx {
                application: application.clone(),
            },
        },
        RichTextNode::Text {
            text: "漢字".to_owned(),
        },
    ]);
    let at_zero_resolver = resolver(definition.clone(), &application, 0);
    let later_resolver = resolver(definition, &application, 500);
    let (at_zero, _, diagnostics) = prepare(&frame, 0, false, true, &at_zero_resolver);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let (later, _, diagnostics) = prepare(&frame, 0, false, true, &later_resolver);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    assert_eq!(at_zero.layout.hash, later.layout.hash);
    assert_ne!(at_zero.paint, later.paint);
    let prefix_y = at_zero.paint.glyphs[0].transform.resolved().translation()[1].pixels();
    let first_y = at_zero.paint.glyphs[1].transform.resolved().translation()[1].pixels();
    let second_y = at_zero.paint.glyphs[2].transform.resolved().translation()[1].pixels();
    assert!(prefix_y.abs() <= 0.001);
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
fn typed_shader_and_mask_resolve_glyph_and_post_process_passes() {
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
    let coverage = FxSamplerProgram::validate(
        ValueProgramSchema::new(Vec::new(), Vec::new(), FxRuntimeType::F32),
        vec![
            constant(FxRuntimeValue::F32(finite(0.5))),
            ValueInstruction::Return,
        ],
    )
    .expect("mask sampler");
    let graph = FxGraph::try_new(vec![
        shader_node("glyph_color", 0.9, [96, 64, 255, 255]),
        FxNode::Mask {
            fx: id.clone(),
            properties: vec![
                FxProperty::new("target", FxStaticValue::Target(FxTarget::Glyph)),
                FxProperty::new("phase", FxStaticValue::Phase(FxPhase::GlyphMask)),
                FxProperty::new("coverage", FxStaticValue::Sampler(coverage)),
            ],
        },
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
    assert!(item.paint.glyphs.iter().all(|glyph| glyph.masks.len() == 1));
    assert_eq!(item.paint.post_processes.len(), 1);
}

#[test]
fn stage_local_fx_time_reaches_dialogue_prepared_glyph_mask() {
    let id = arcweft_presentation::fx::FxId::try_new("test", "dialogue.stage_time").expect("Fx id");
    let coverage = FxSamplerProgram::validate(
        ValueProgramSchema::new(Vec::new(), Vec::new(), FxRuntimeType::F32),
        vec![
            ValueInstruction::LoadContext {
                slot: FxContextSlot::Time,
            },
            ValueInstruction::Return,
        ],
    )
    .expect("stage-time coverage sampler");
    let graph = FxGraph::try_new(vec![FxNode::Mask {
        fx: id.clone(),
        properties: vec![
            FxProperty::new("target", FxStaticValue::Target(FxTarget::Glyph)),
            FxProperty::new("phase", FxStaticValue::Phase(FxPhase::GlyphMask)),
            FxProperty::new("coverage", FxStaticValue::Sampler(coverage)),
        ],
    }])
    .expect("typed mask graph");
    let definition = FxDefinition::new(id.clone(), Vec::new(), graph).expect("definition");
    let application = FxApplication::try_new(id.clone(), Vec::new(), 0, None).expect("application");
    let activation_logical_time = FxLogicalTime::zero()
        .try_advance_millis(7_000)
        .expect("activation time");
    let instance = FxInstanceSnapshot {
        instance: application.derive_instance_id(["dialogue", "stage-time"]),
        definition: id,
        abi_hash: definition.abi_hash(),
        activation_logical_time,
        deterministic_seed: 5,
        parameters: Vec::new(),
        child_path: FxGraphChildPath::default(),
        provider_state: Vec::new(),
    };
    let frame = frame(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::Fx {
                application: application.clone(),
            },
        },
        RichTextNode::Text {
            text: "時".to_owned(),
        },
        RichTextNode::StyleEnd {
            name: "fx".to_owned(),
        },
    ]);
    let at_stage_start = TestFxResolver {
        definition: definition.clone(),
        instance: instance.clone(),
        runtime_time: activation_logical_time,
    };
    let after_one_second = TestFxResolver {
        definition,
        instance,
        runtime_time: activation_logical_time
            .try_advance_millis(1_000)
            .expect("stage-local sample time"),
    };

    let (hidden, _, diagnostics) = prepare(&frame, 0, false, true, &at_stage_start);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let (visible, _, diagnostics) = prepare(&frame, 1_000, false, true, &after_one_second);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let hidden_coverage = hidden.paint.glyphs[0].masks[0]
        .effective_coverage()
        .value()
        .get();
    let visible_coverage = visible.paint.glyphs[0].masks[0]
        .effective_coverage()
        .value()
        .get();
    assert!(hidden_coverage.abs() <= f32::EPSILON);
    assert!((visible_coverage - 1.0).abs() <= f32::EPSILON);
    assert_eq!(hidden.layout.hash, visible.layout.hash);
}

#[test]
fn missing_typed_shader_is_a_typed_diagnostic() {
    let id =
        arcweft_presentation::fx::FxId::try_new("test", "dialogue.missing_shader").expect("Fx id");
    let graph = FxGraph::try_new(vec![FxNode::Shader {
        fx: id.clone(),
        properties: vec![
            FxProperty::new("target", FxStaticValue::Target(FxTarget::Glyph)),
            FxProperty::new("phase", FxStaticValue::Phase(FxPhase::GlyphColor)),
            FxProperty::new(
                "resource",
                FxStaticValue::Resource(
                    FxResourceId::try_new("missing.shader").expect("resource id"),
                ),
            ),
            FxProperty::new("uniforms", FxStaticValue::Record(Vec::new())),
        ],
    }])
    .expect("shader graph");
    let definition = FxDefinition::new(id.clone(), Vec::new(), graph).expect("definition");
    let application = FxApplication::try_new(id, Vec::new(), 0, None).expect("application");
    let resolver = resolver(definition, &application, 0);
    let frame = frame(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::Fx {
                application: application.clone(),
            },
        },
        RichTextNode::Text {
            text: "missing".to_owned(),
        },
    ]);

    let (item, _, diagnostics) = prepare(&frame, 0, false, true, &resolver);

    assert!(
        item.paint
            .glyphs
            .iter()
            .all(|glyph| glyph.effects.is_empty())
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == arcweft_presentation::fx::FxDiagnosticCode::MissingProvider
                && diagnostic.message.contains("missing.shader")
        }),
        "{diagnostics:#?}"
    );
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
        view: arcweft_view::ViewId::try_new_engine_owned("std.view.dialogue")
            .expect("standard dialogue View id"),
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
