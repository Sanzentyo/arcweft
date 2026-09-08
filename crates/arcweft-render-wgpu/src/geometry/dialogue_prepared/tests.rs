use arcweft_character::{
    id::CharacterId,
    presentation_name::{
        CharacterPresentationCatalogGeneration, CharacterPresentationCatalogRevision,
        CharacterPresentationLocalePolicyDigest, CharacterPresentationSemanticDigest,
    },
};
use arcweft_core::{entry::RuntimeValueDigest, plan::RuntimeLineId};
use arcweft_dialogue::{
    DialoguePresentationProfile, DialogueProfileRevision, InlineFailurePolicy,
    character_presentation::{
        CharacterPresentationTargetEvidence, CheckedCharacterPresentationPlan,
    },
};
use arcweft_id::TextKey;
use arcweft_presentation::{
    fx::{
        Angle, FiniteF32, FxApplication, FxApplicationDraft, FxApplicationResolver, FxAuthoredSeed,
        FxColor, FxContextSlot, FxDefinition, FxDiagnostic, FxEvaluationBinding, FxGraph,
        FxGraphChildPath, FxInstanceActivation, FxInstanceOwnerKey, FxInstanceSnapshot,
        FxLogicalTime, FxNode, FxPhase, FxProperty, FxPropertyId, FxResourceId, FxRuntimeType,
        FxRuntimeValue, FxSamplerProgram, FxShaderStage, FxStaticValue, FxTarget, FxUniformField,
        FxUniformRecord, FxUniformValue, Length, Transform2D, ValueInstruction, ValueProgramSchema,
    },
    hit::HitRect,
};
use arcweft_render_text::{RuntimeLineContext, TextWeight, resolve_frame_with_template};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{
    ProductSourceRef, SourceDocument, SourceDocumentId, SourceName, SourceSetRevision,
};
use arcweft_text_model::{
    CharacterDialoguePresentationConfig, DialogueContentFragmentTemplate, DialogueContentSpec,
    DialoguePresentationCharacter, LineDisplayFrame, RichTextControl, RichTextDocument,
    RichTextLayout, RichTextNode, RichTextStyle, RichTextWritingMode,
};
use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};
use std::collections::BTreeMap;

use super::*;

const TEST_FONT: &[u8] = include_bytes!("../../../../../web/assets/noto-sans-jp-vf.ttf");

fn test_source_ref() -> ProductSourceRef {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("render-wgpu-dialogue-prepared-test").expect("document ID"),
        SourceName::Memory,
        "dialogue prepared test",
    )
    .expect("test document");
    ProductSourceRef::try_for_identity(source.identity()).expect("product source reference")
}

fn test_character_plan() -> CheckedCharacterPresentationPlan {
    CheckedCharacterPresentationPlan::try_new(
        CharacterPresentationTargetEvidence::Exact(
            CharacterId::try_new("character.narrator").expect("character identity"),
        ),
        CharacterPresentationCatalogGeneration::new(
            CharacterPresentationCatalogRevision::INITIAL,
            CharacterPresentationSemanticDigest::from_bytes([1; 32]),
            CharacterPresentationLocalePolicyDigest::from_bytes([2; 32]),
        ),
    )
    .expect("checked Character presentation plan")
}

fn test_dialogue_profile_revision() -> DialogueProfileRevision {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("render-wgpu-dialogue-profile-test").unwrap(),
        SourceName::Memory,
        "schema = 1\n",
    )
    .unwrap();
    let sources = SourceSetRevision::try_for_identities([source.identity()]).unwrap();
    DialogueProfileRevision::from_admitted_parts(
        source.identity().clone(),
        sources,
        sources,
        ViewProgramId::try_new("view_program.render_wgpu_dialogue").unwrap(),
        AcceptedViewProgramRevision::try_from_bytes([0x41; 32]).unwrap(),
        ResourceTypeRegistry::empty().digest(),
    )
}

fn runtime_line_context() -> RuntimeLineContext {
    RuntimeLineContext::new(
        Vec::new(),
        DialoguePresentationCharacter {
            id: CharacterId::try_new("character.narrator").expect("character identity"),
            display_name: "Narrator".to_owned(),
        },
        CharacterDialoguePresentationConfig {
            view: arcweft_view::ViewId::try_new_engine_owned("std.view.dialogue")
                .expect("standard dialogue View id"),
            voice: None,
            look: None,
            stage: None,
            portrait: None,
            focus: None,
            cleanup: None,
            source_locale: None,
            hooks: Vec::new(),
            inline_failure: InlineFailurePolicy::FailLine,
            custom: BTreeMap::new(),
            config_digest: RuntimeValueDigest::ZERO,
        },
        Vec::new(),
        Vec::new(),
    )
}

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

fn owner(bytes: &[u8]) -> FxInstanceOwnerKey {
    FxInstanceOwnerKey::from_dialogue_canonical_bytes(bytes)
}

fn bind_empty(definition: &FxDefinition, authored_ordinal: u32) -> FxApplication {
    let draft = FxApplicationDraft::try_new(
        definition.id().clone(),
        vec![None; definition.parameters().len()],
        authored_ordinal,
        None,
    )
    .expect("empty Fx application draft");
    FxApplication::bind(definition, draft).expect("empty Fx application binds")
}

fn snapshot(
    application: &FxApplication,
    definition: &FxDefinition,
    owner_bytes: &[u8],
    activation_logical_time: FxLogicalTime,
    authored_seed: u32,
) -> FxInstanceSnapshot {
    FxInstanceSnapshot::try_new(
        application.instance_identity(owner(owner_bytes)),
        definition,
        FxInstanceActivation::new(
            activation_logical_time,
            Some(FxAuthoredSeed::new(authored_seed)),
            FxGraphChildPath::default(),
        ),
        application.template().clone(),
        application
            .template()
            .initial_runtime()
            .to_vec()
            .into_boxed_slice(),
        Vec::new(),
    )
    .expect("Fx snapshot")
}

fn resolver(
    definition: FxDefinition,
    application: &FxApplication,
    runtime_millis: u64,
) -> TestFxResolver {
    let instance = snapshot(
        application,
        &definition,
        b"dialogue-typed-fixture",
        FxLogicalTime::zero(),
        17,
    );
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
    let frame = frame(vec![RichTextNode::Scope {
        style: Box::new(RichTextStyle::Layout {
            layout: RichTextLayout {
                writing_mode: RichTextWritingMode::VerticalRl,
                ..RichTextLayout::default()
            },
        }),
        body: vec![
            RichTextNode::Ruby {
                body: vec![RichTextNode::Text {
                    text: "漢字".to_owned(),
                }],
                ruby: "かんじ".to_owned(),
            },
            RichTextNode::Text {
                text: "ABC2026".to_owned(),
            },
        ],
    }]);
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
            FxProperty::new(FxPropertyId::Target, FxStaticValue::Target(FxTarget::Glyph)),
            FxProperty::new(
                FxPropertyId::Phase,
                FxStaticValue::Phase(FxPhase::GlyphTransform),
            ),
            FxProperty::new(FxPropertyId::Sampler, FxStaticValue::Sampler(sampler)),
        ],
    }])
    .expect("wave graph");
    let definition = FxDefinition::new(id.clone(), Vec::new(), graph).expect("definition");
    let application = bind_empty(&definition, 0);
    let frame = frame(vec![
        RichTextNode::Text {
            text: "前".to_owned(),
        },
        RichTextNode::Scope {
            style: Box::new(RichTextStyle::Fx {
                application: application.clone(),
            }),
            body: vec![RichTextNode::Text {
                text: "漢字".to_owned(),
            }],
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
            properties: vec![FxProperty::new(
                FxPropertyId::Weight,
                FxRuntimeValue::I32(700).into(),
            )],
        },
        FxNode::Transform {
            fx: id.clone(),
            properties: vec![
                FxProperty::new(FxPropertyId::Target, FxStaticValue::Target(FxTarget::Glyph)),
                FxProperty::new(
                    FxPropertyId::Transform,
                    FxRuntimeValue::Transform2D(transform).into(),
                ),
            ],
        },
    ])
    .expect("typed graph");
    let definition = FxDefinition::new(id.clone(), Vec::new(), graph).expect("definition");
    let application = bind_empty(&definition, 0);
    let instance = snapshot(
        &application,
        &definition,
        b"dialogue-fixture-occurrence",
        FxLogicalTime::zero(),
        17,
    );
    let resolver = TestFxResolver {
        definition,
        instance,
        runtime_time: FxLogicalTime::zero(),
    };
    let frame = frame(vec![RichTextNode::Scope {
        style: Box::new(RichTextStyle::Fx { application }),
        body: vec![RichTextNode::Text {
            text: "typed".to_owned(),
        }],
    }]);
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
    let shader_node = |stage: FxShaderStage, amount: f32, color: [u8; 4]| FxNode::Shader {
        fx: id.clone(),
        properties: vec![
            FxProperty::new(
                FxPropertyId::Resource,
                FxStaticValue::Resource(
                    FxResourceId::try_new("shader.source_glow").expect("resource"),
                ),
            ),
            FxProperty::new(FxPropertyId::Stage, FxStaticValue::ShaderStage(stage)),
            FxProperty::new(
                FxPropertyId::Uniforms,
                FxStaticValue::UniformRecord(
                    FxUniformRecord::try_new(vec![
                        FxUniformField::try_new(
                            "amount",
                            FxUniformValue::constant(FxRuntimeValue::F32(
                                FiniteF32::try_new(amount).expect("finite"),
                            ))
                            .expect("amount uniform"),
                        )
                        .expect("amount field"),
                        FxUniformField::try_new(
                            "color",
                            FxUniformValue::constant(FxRuntimeValue::Color(FxColor::from_rgba8(
                                color,
                            )))
                            .expect("color uniform"),
                        )
                        .expect("color field"),
                    ])
                    .expect("uniform record"),
                ),
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
        shader_node(FxShaderStage::GlyphColor, 0.9, [96, 64, 255, 255]),
        FxNode::Mask {
            fx: id.clone(),
            properties: vec![
                FxProperty::new(FxPropertyId::Target, FxStaticValue::Target(FxTarget::Glyph)),
                FxProperty::new(
                    FxPropertyId::Phase,
                    FxStaticValue::Phase(FxPhase::GlyphMask),
                ),
                FxProperty::new(FxPropertyId::Coverage, FxStaticValue::Sampler(coverage)),
            ],
        },
        shader_node(FxShaderStage::PostProcess, 0.65, [64, 176, 255, 255]),
    ])
    .expect("typed graph");
    let definition = FxDefinition::new(id.clone(), Vec::new(), graph).expect("definition");
    let application = bind_empty(&definition, 0);
    let instance = snapshot(
        &application,
        &definition,
        b"dialogue-fixture-source",
        FxLogicalTime::zero(),
        91,
    );
    let resolver = TestFxResolver {
        definition,
        instance,
        runtime_time: FxLogicalTime::zero(),
    };
    let frame = frame(vec![RichTextNode::Scope {
        style: Box::new(RichTextStyle::Fx { application }),
        body: vec![RichTextNode::Text {
            text: "source".to_owned(),
        }],
    }]);

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
            FxProperty::new(FxPropertyId::Target, FxStaticValue::Target(FxTarget::Glyph)),
            FxProperty::new(
                FxPropertyId::Phase,
                FxStaticValue::Phase(FxPhase::GlyphMask),
            ),
            FxProperty::new(FxPropertyId::Coverage, FxStaticValue::Sampler(coverage)),
        ],
    }])
    .expect("typed mask graph");
    let definition = FxDefinition::new(id.clone(), Vec::new(), graph).expect("definition");
    let application = bind_empty(&definition, 0);
    let activation_logical_time = FxLogicalTime::zero()
        .try_advance_millis(7_000)
        .expect("activation time");
    let instance = snapshot(
        &application,
        &definition,
        b"dialogue-stage-time",
        activation_logical_time,
        5,
    );
    let frame = frame(vec![RichTextNode::Scope {
        style: Box::new(RichTextStyle::Fx {
            application: application.clone(),
        }),
        body: vec![RichTextNode::Text {
            text: "時".to_owned(),
        }],
    }]);
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
            FxProperty::new(FxPropertyId::Target, FxStaticValue::Target(FxTarget::Glyph)),
            FxProperty::new(
                FxPropertyId::Phase,
                FxStaticValue::Phase(FxPhase::GlyphColor),
            ),
            FxProperty::new(
                FxPropertyId::Resource,
                FxStaticValue::Resource(
                    FxResourceId::try_new("missing.shader").expect("resource id"),
                ),
            ),
            FxProperty::new(
                FxPropertyId::Uniforms,
                FxStaticValue::UniformRecord(
                    FxUniformRecord::try_new(Vec::new()).expect("empty uniform record"),
                ),
            ),
        ],
    }])
    .expect("shader graph");
    let definition = FxDefinition::new(id.clone(), Vec::new(), graph).expect("definition");
    let application = bind_empty(&definition, 0);
    let resolver = resolver(definition, &application, 0);
    let frame = frame(vec![RichTextNode::Scope {
        style: Box::new(RichTextStyle::Fx {
            application: application.clone(),
        }),
        body: vec![RichTextNode::Text {
            text: "missing".to_owned(),
        }],
    }]);

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
    frame: &LineDisplayFrame,
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
        reveal_elapsed: arcweft_text_model::DialogueRevealElapsed::from_nanos(
            visual_time_millis * 1_000_000,
        ),
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

fn frame(nodes: Vec<RichTextNode>) -> LineDisplayFrame {
    let template = DialogueContentFragmentTemplate::try_new_canonical(
        arcweft_core::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        RichTextDocument::new(nodes),
    )
    .expect("dialogue template");
    let spec = DialogueContentSpec::try_new(
        RuntimeLineId::canonical("prepared.dialogue.test").expect("line id"),
        TextKey::try_new("text.prepared.dialogue.test").expect("text key"),
        &template,
        test_character_plan(),
        arcweft_text_model::DialoguePresentationSnapshot::new(
            DialoguePresentationProfile::engine_default(),
            test_dialogue_profile_revision(),
        ),
        Vec::new(),
        test_source_ref(),
    )
    .expect("dialogue spec");
    let content = arcweft_core::value::RuntimeDialogueContentValue::try_new(
        arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x71; 32])
            .expect("fixture artifact"),
        template.id(),
        template.digest(),
        [],
    )
    .expect("fixture Content envelope");
    resolve_frame_with_template(&spec, &template, &content, &runtime_line_context())
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
