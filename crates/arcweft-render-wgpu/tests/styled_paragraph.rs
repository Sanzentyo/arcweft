use arcweft_render_text::{
    Milli, RichTextColor, RichTextControl, RichTextControlMarker, RichTextEffectDescriptor,
    RichTextEffectPhase, RichTextEffectTarget, RichTextFontFamily, RichTextParam, RichTextRange,
    RichTextStateScope, RichTextStyle, RichTextTextRun, RichTextTextSource,
    presentation_from_styles,
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderChoiceItem, RenderDialogue, RenderFontFamily,
    RenderGlyphTransformKind, RenderPreferences, RenderScene, RenderTextWeight, RenderViewport,
    SharedFramePlanner,
};
use arcweft_render_wgpu::renderer::{
    StyledParagraphRevealState, StyledParagraphTransformSupport, styled_paragraph_layout_evidence,
};
use glyphon::FontSystem;
use std::collections::BTreeMap;

fn viewport() -> RenderViewport {
    RenderViewport {
        logical_width: 960.0,
        logical_height: 540.0,
        physical_width: 960,
        physical_height: 540,
        scale_factor: 1.0,
    }
}

fn scene(dialogue: RenderDialogue) -> RenderScene {
    RenderScene {
        dialogue: Some(dialogue),
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: Vec::new(),
        viewport: viewport(),
        visual_time_millis: 1_000,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    }
}

fn run(start: usize, end: usize, node_index: usize, styles: Vec<RichTextStyle>) -> RichTextTextRun {
    RichTextTextRun {
        range: RichTextRange::new(start, end),
        source: RichTextTextSource::Text,
        node_index,
        presentation: presentation_from_styles(&styles),
        styles,
    }
}

fn control(
    text_offset: usize,
    node_index: usize,
    control: RichTextControl,
) -> RichTextControlMarker {
    RichTextControlMarker {
        node_index,
        text_offset,
        control,
        range: None,
    }
}

fn color(red: u8, green: u8, blue: u8) -> RichTextStyle {
    RichTextStyle::Color {
        value: RichTextColor::Rgb { red, green, blue },
    }
}

fn size(points: u16) -> RichTextStyle {
    RichTextStyle::Size {
        points: Some(points),
        raw: points.to_string(),
    }
}

#[test]
fn dialogue_body_text_runs_create_one_styled_paragraph_not_blocks() {
    let text = "Alpha bravo charlie delta echo".to_owned();
    let alpha_end = "Alpha bravo ".len();
    let dialogue = RenderDialogue {
        speaker: "Narrator".to_owned(),
        text: text.clone(),
        base_styles: Vec::new(),
        text_runs: vec![
            run(0, alpha_end, 0, vec![color(100, 210, 200)]),
            run(
                alpha_end,
                text.len(),
                1,
                vec![RichTextStyle::Strong {
                    attrs: String::new(),
                }],
            ),
        ],
        controls: Vec::new(),
        reveal_start: 0,
        reveal_complete: false,
    };

    let frame = SharedFramePlanner::prepare(&scene(dialogue)).expect("frame plans");

    assert_eq!(frame.styled_paragraphs.len(), 1);
    assert_eq!(frame.styled_paragraphs[0].text, text);
    assert_eq!(frame.styled_paragraphs[0].spans.len(), 2);
    assert!(
        frame
            .text
            .iter()
            .all(|block| !block.text.contains("Alpha bravo"))
    );
}

#[test]
fn styled_paragraph_preserves_mixed_span_styles() {
    let text = "Cyan bold large".to_owned();
    let cyan_end = "Cyan ".len();
    let bold_end = "Cyan bold ".len();
    let dialogue = RenderDialogue {
        speaker: "Guide".to_owned(),
        text: text.clone(),
        base_styles: vec![RichTextStyle::Font {
            family: RichTextFontFamily::Named {
                name: "Arcweft Demo".to_owned(),
            },
        }],
        text_runs: vec![
            run(0, cyan_end, 0, vec![color(100, 210, 200)]),
            run(
                cyan_end,
                bold_end,
                1,
                vec![RichTextStyle::Strong {
                    attrs: String::new(),
                }],
            ),
            run(bold_end, text.len(), 2, vec![size(36)]),
        ],
        controls: Vec::new(),
        reveal_start: 0,
        reveal_complete: false,
    };

    let frame = SharedFramePlanner::prepare(&scene(dialogue)).expect("frame plans");
    let paragraph = frame.styled_paragraphs.first().expect("styled paragraph");

    assert_eq!(paragraph.spans[0].style.color, [100, 210, 200, 255]);
    assert_eq!(
        paragraph.spans[0].style.font_family,
        RenderFontFamily::Named("Arcweft Demo".to_owned())
    );
    assert_eq!(paragraph.spans[1].style.weight, RenderTextWeight::Bold);
    assert!((paragraph.spans[2].style.font_size - 36.0).abs() < f32::EPSILON);
    assert!((paragraph.spans[2].style.line_height - 48.6).abs() < 0.001);
}

#[test]
fn typewriter_reveal_keeps_full_paragraph_text_and_ranges() {
    let text = "The paragraph must wrap before it is fully revealed.".to_owned();
    let mut params = BTreeMap::new();
    params.insert("cps".to_owned(), RichTextParam::Int { value: 4 });
    let effect = RichTextEffectDescriptor {
        id: "typewriter".to_owned(),
        params,
        target: RichTextEffectTarget::default(),
        phase: RichTextEffectPhase::GlyphMask,
        state_scope: RichTextStateScope::default(),
    };
    let mut text_run = run(0, text.len(), 0, Vec::new());
    text_run.presentation.effects.push(effect);
    let dialogue = RenderDialogue {
        speaker: "Guide".to_owned(),
        text: text.clone(),
        base_styles: Vec::new(),
        text_runs: vec![text_run],
        controls: Vec::new(),
        reveal_start: 0,
        reveal_complete: false,
    };
    let mut test_scene = scene(dialogue);
    test_scene.visual_time_millis = 250;

    let frame = SharedFramePlanner::prepare(&test_scene).expect("frame plans");
    let paragraph = frame.styled_paragraphs.first().expect("styled paragraph");

    assert_eq!(paragraph.text, text);
    assert_eq!(
        paragraph.spans[0].range,
        RichTextRange::new(0, paragraph.text.len())
    );
    assert!(paragraph.reveal.visible_end < paragraph.text.len());
}

#[test]
fn timed_wait_starts_after_the_marker_is_revealed() {
    let text = "AB".to_owned();
    let dialogue = RenderDialogue {
        speaker: "Guide".to_owned(),
        text: text.clone(),
        base_styles: Vec::new(),
        text_runs: vec![run(
            0,
            text.len(),
            0,
            vec![RichTextStyle::Speed {
                value: "10".to_owned(),
            }],
        )],
        controls: vec![control(
            1,
            1,
            RichTextControl::TimedWait {
                duration_millis: 500,
            },
        )],
        reveal_start: 0,
        reveal_complete: false,
    };
    let mut before_wait_finishes = scene(dialogue.clone());
    before_wait_finishes.visual_time_millis = 599;
    let mut after_wait_finishes = scene(dialogue);
    after_wait_finishes.visual_time_millis = 700;

    let before = SharedFramePlanner::prepare(&before_wait_finishes).expect("frame plans");
    let after = SharedFramePlanner::prepare(&after_wait_finishes).expect("frame plans");

    assert_eq!(before.styled_paragraphs[0].reveal.visible_end, 1);
    assert!(!before.styled_paragraphs[0].reveal_complete());
    assert_eq!(after.styled_paragraphs[0].reveal.visible_end, text.len());
    assert!(after.styled_paragraphs[0].reveal_complete());
}

#[test]
fn reduce_motion_reveals_characters_instantly_but_preserves_wait_and_clear_order() {
    let text = "AB".to_owned();
    let dialogue = RenderDialogue {
        speaker: "Guide".to_owned(),
        text: text.clone(),
        base_styles: Vec::new(),
        text_runs: vec![run(0, text.len(), 0, Vec::new())],
        controls: vec![
            control(
                1,
                1,
                RichTextControl::TimedWait {
                    duration_millis: 500,
                },
            ),
            control(1, 2, RichTextControl::Clear),
        ],
        reveal_start: 0,
        reveal_complete: false,
    };
    let mut waiting = scene(dialogue.clone());
    waiting.preferences.reduce_motion = true;
    waiting.visual_time_millis = 499;
    let mut after_wait = scene(dialogue);
    after_wait.preferences.reduce_motion = true;
    after_wait.visual_time_millis = 500;

    let waiting = SharedFramePlanner::prepare(&waiting).expect("frame plans");
    let after_wait = SharedFramePlanner::prepare(&after_wait).expect("frame plans");

    assert_eq!(waiting.styled_paragraphs[0].text, text);
    assert_eq!(waiting.styled_paragraphs[0].reveal.visible_end, 1);
    assert!(!waiting.styled_paragraphs[0].reveal_complete());
    assert_eq!(after_wait.styled_paragraphs[0].text, "B");
    assert_eq!(after_wait.styled_paragraphs[0].reveal.visible_end, 1);
    assert!(after_wait.styled_paragraphs[0].reveal_complete());
}

#[test]
fn clear_removes_the_revealed_prefix_when_the_timeline_reaches_it() {
    let text = "AB".to_owned();
    let dialogue = RenderDialogue {
        speaker: "Guide".to_owned(),
        text: text.clone(),
        base_styles: Vec::new(),
        text_runs: vec![run(
            0,
            text.len(),
            0,
            vec![RichTextStyle::Speed {
                value: "10".to_owned(),
            }],
        )],
        controls: vec![control(1, 1, RichTextControl::Clear)],
        reveal_start: 0,
        reveal_complete: false,
    };
    let mut at_clear = scene(dialogue.clone());
    at_clear.visual_time_millis = 100;
    let mut complete = scene(dialogue);
    complete.visual_time_millis = 200;

    let at_clear = SharedFramePlanner::prepare(&at_clear).expect("frame plans");
    let complete = SharedFramePlanner::prepare(&complete).expect("frame plans");

    assert_eq!(at_clear.styled_paragraphs[0].text, "B");
    assert_eq!(at_clear.styled_paragraphs[0].reveal.visible_end, 0);
    assert_eq!(complete.styled_paragraphs[0].text, "B");
    assert_eq!(complete.styled_paragraphs[0].reveal.visible_end, 1);
    assert!(complete.styled_paragraphs[0].reveal_complete());
}

#[test]
fn explicit_reveal_completion_finishes_waits_and_applies_clear() {
    let text = "AB".to_owned();
    let dialogue = RenderDialogue {
        speaker: "Guide".to_owned(),
        text: text.clone(),
        base_styles: Vec::new(),
        text_runs: vec![run(0, text.len(), 0, Vec::new())],
        controls: vec![
            control(
                1,
                1,
                RichTextControl::TimedWait {
                    duration_millis: u64::MAX,
                },
            ),
            control(1, 2, RichTextControl::Clear),
        ],
        reveal_start: 0,
        reveal_complete: false,
    }
    .with_reveal_complete(true);
    let mut complete = scene(dialogue);
    complete.visual_time_millis = 0;

    let frame = SharedFramePlanner::prepare(&complete).expect("frame plans");
    let paragraph = &frame.styled_paragraphs[0];

    assert_eq!(paragraph.text, "B");
    assert_eq!(paragraph.reveal.visible_end, 1);
    assert!(paragraph.reveal_complete());
    assert!(frame.dialogue_reveal_complete());
}

#[test]
fn glyph_transform_effect_is_range_metadata_not_character_blocks() {
    let text = "Wavy words stay in one paragraph".to_owned();
    let mut params = BTreeMap::new();
    params.insert(
        "amp".to_owned(),
        RichTextParam::Milli {
            value: Milli(6_000),
        },
    );
    let effect = RichTextEffectDescriptor {
        id: "wave".to_owned(),
        params,
        target: RichTextEffectTarget::default(),
        phase: RichTextEffectPhase::GlyphTransform,
        state_scope: RichTextStateScope::default(),
    };
    let mut text_run = run(0, text.len(), 0, Vec::new());
    text_run.presentation.effects.push(effect);
    let dialogue = RenderDialogue {
        speaker: "Guide".to_owned(),
        text,
        base_styles: Vec::new(),
        text_runs: vec![text_run],
        controls: Vec::new(),
        reveal_start: 0,
        reveal_complete: false,
    };

    let frame = SharedFramePlanner::prepare(&scene(dialogue)).expect("frame plans");
    let paragraph = frame.styled_paragraphs.first().expect("styled paragraph");

    assert_eq!(paragraph.glyph_transforms.len(), 1);
    assert_eq!(
        paragraph.glyph_transforms[0].motion.kind,
        RenderGlyphTransformKind::Wave
    );
    assert!(
        frame
            .text
            .iter()
            .all(|block| block.text.chars().count() != 1)
    );
}

#[test]
fn layout_evidence_wraps_across_style_boundaries() {
    let text = "one two three four five six seven eight nine ten".to_owned();
    let split = "one two three four ".len();
    let dialogue = RenderDialogue {
        speaker: "Guide".to_owned(),
        text: text.clone(),
        base_styles: Vec::new(),
        text_runs: vec![
            run(0, split, 0, vec![color(120, 220, 150)]),
            run(split, text.len(), 1, vec![size(34)]),
        ],
        controls: Vec::new(),
        reveal_start: 0,
        reveal_complete: false,
    };
    let mut test_scene = scene(dialogue);
    test_scene.viewport.logical_width = 360.0;
    test_scene.viewport.physical_width = 360;

    let frame = SharedFramePlanner::prepare(&test_scene).expect("frame plans");
    let paragraph = frame.styled_paragraphs.first().expect("styled paragraph");
    let mut font_system = FontSystem::new();
    let evidence = styled_paragraph_layout_evidence(&mut font_system, paragraph);

    assert!(evidence.line_boxes.len() >= 2);
    assert_eq!(evidence.spans.len(), 2);
    assert!(
        evidence
            .glyph_bounds
            .iter()
            .any(|glyph| glyph.source_range.start < split)
    );
    assert!(
        evidence
            .glyph_bounds
            .iter()
            .any(|glyph| glyph.source_range.start >= split)
    );
}

#[test]
fn layout_evidence_records_reveal_state_per_glyph() {
    let text = "Reveal slowly across glyphs".to_owned();
    let mut params = BTreeMap::new();
    params.insert("cps".to_owned(), RichTextParam::Int { value: 8 });
    let effect = RichTextEffectDescriptor {
        id: "typewriter".to_owned(),
        params,
        target: RichTextEffectTarget::default(),
        phase: RichTextEffectPhase::GlyphMask,
        state_scope: RichTextStateScope::default(),
    };
    let mut text_run = run(0, text.len(), 0, Vec::new());
    text_run.presentation.effects.push(effect);
    let dialogue = RenderDialogue {
        speaker: "Guide".to_owned(),
        text,
        base_styles: Vec::new(),
        text_runs: vec![text_run],
        controls: Vec::new(),
        reveal_start: 0,
        reveal_complete: false,
    };
    let mut test_scene = scene(dialogue);
    test_scene.visual_time_millis = 250;

    let frame = SharedFramePlanner::prepare(&test_scene).expect("frame plans");
    let paragraph = frame.styled_paragraphs.first().expect("styled paragraph");
    let mut font_system = FontSystem::new();
    let evidence = styled_paragraph_layout_evidence(&mut font_system, paragraph);

    assert!(evidence.visible_end < evidence.text_len);
    assert!(
        evidence
            .glyph_bounds
            .iter()
            .any(|glyph| glyph.reveal_state == StyledParagraphRevealState::Visible)
    );
    assert!(
        evidence
            .glyph_bounds
            .iter()
            .any(|glyph| glyph.reveal_state == StyledParagraphRevealState::Hidden)
    );
}

#[test]
fn layout_evidence_records_rendered_transform_support() {
    let text = "Wavy words stay deterministic".to_owned();
    let mut params = BTreeMap::new();
    params.insert(
        "amp".to_owned(),
        RichTextParam::Milli {
            value: Milli(6_000),
        },
    );
    let effect = RichTextEffectDescriptor {
        id: "wave".to_owned(),
        params,
        target: RichTextEffectTarget::default(),
        phase: RichTextEffectPhase::GlyphTransform,
        state_scope: RichTextStateScope::default(),
    };
    let mut text_run = run(0, text.len(), 0, Vec::new());
    text_run.presentation.effects.push(effect);
    let dialogue = RenderDialogue {
        speaker: "Guide".to_owned(),
        text,
        base_styles: Vec::new(),
        text_runs: vec![text_run],
        controls: Vec::new(),
        reveal_start: 0,
        reveal_complete: false,
    };

    let frame = SharedFramePlanner::prepare(&scene(dialogue)).expect("frame plans");
    let paragraph = frame.styled_paragraphs.first().expect("styled paragraph");
    let mut font_system = FontSystem::new();
    let evidence = styled_paragraph_layout_evidence(&mut font_system, paragraph);

    assert_eq!(
        evidence.transform_support,
        StyledParagraphTransformSupport::Rendered
    );
    assert_eq!(evidence.glyph_transforms.len(), 1);
    assert!(evidence.glyph_transforms[0].rendered);
}

#[test]
fn choice_labels_remain_single_style_blocks() {
    let mut test_scene = scene(RenderDialogue::plain("Guide", "A body paragraph"));
    test_scene.choices = vec![RenderChoiceItem {
        id: "inspect".to_owned(),
        label: "Inspect parity metrics".to_owned(),
    }];

    let frame = SharedFramePlanner::prepare(&test_scene).expect("frame plans");

    assert!(
        frame
            .text
            .iter()
            .any(|block| block.text == "Inspect parity metrics")
    );
    assert_eq!(frame.styled_paragraphs.len(), 1);
}
