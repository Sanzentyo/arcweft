use super::*;
use arcweft_render_text::{
    LineDisplayFrame, Milli, RichTextDisplayMap, RichTextEffectDescriptor, RichTextEffectPhase,
    RichTextEffectTarget, RichTextJlreqStrictness, RichTextLayout, RichTextParam,
    RichTextStateScope, RichTextTextRun, RichTextTextSource, RichTextVec2,
};
use std::collections::BTreeMap;

mod ruby;
mod vertical_class_mix;
mod vertical_sequences;

fn line_id(value: &str) -> arcweft_core::plan::RuntimeLineId {
    arcweft_core::plan::RuntimeLineId::from_runtime_line_value(value)
        .expect("test line ID is valid")
}

fn frame_with_run(text: &str, presentation: RichTextPresentation) -> LineDisplayFrame {
    LineDisplayFrame {
        line: line_id("say.test.001"),
        callee: "alice.say".to_owned(),
        speaker_label: None,
        text: text.to_owned(),
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        nodes: Vec::new(),
        display_map: RichTextDisplayMap {
            text_runs: vec![RichTextTextRun {
                range: RichTextRange::new(0, text.len()),
                source: RichTextTextSource::Text,
                node_index: 0,
                styles: Vec::new(),
                presentation,
            }],
            ruby_annotations: Vec::new(),
            controls: Vec::new(),
            host_events: Vec::new(),
        },
        host_events: Vec::new(),
        inline_failures: Vec::new(),
        unresolved: Vec::new(),
    }
}

fn vertical_presentation(writing_mode: RichTextWritingMode) -> RichTextPresentation {
    RichTextPresentation {
        layout: Some(RichTextLayout {
            writing_mode,
            ..RichTextLayout::default()
        }),
        ..RichTextPresentation::default()
    }
}

fn frame_with_split_runs(
    text: &str,
    split_at: usize,
    presentation: RichTextPresentation,
) -> LineDisplayFrame {
    LineDisplayFrame {
        line: line_id("say.test.001"),
        callee: "alice.say".to_owned(),
        speaker_label: None,
        text: text.to_owned(),
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        nodes: Vec::new(),
        display_map: RichTextDisplayMap {
            text_runs: vec![
                RichTextTextRun {
                    range: RichTextRange::new(0, split_at),
                    source: RichTextTextSource::Text,
                    node_index: 0,
                    styles: Vec::new(),
                    presentation: presentation.clone(),
                },
                RichTextTextRun {
                    range: RichTextRange::new(split_at, text.len()),
                    source: RichTextTextSource::Text,
                    node_index: 1,
                    styles: Vec::new(),
                    presentation,
                },
            ],
            ruby_annotations: Vec::new(),
            controls: Vec::new(),
            host_events: Vec::new(),
        },
        host_events: Vec::new(),
        inline_failures: Vec::new(),
        unresolved: Vec::new(),
    }
}

#[test]
fn horizontal_layout_keeps_source_ranges() {
    let frame = frame_with_run("A夢", RichTextPresentation::default());
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 2);
    assert_eq!(layout.glyphs[0].range, RichTextRange::new(0, 1));
    assert_eq!(layout.glyphs[1].range, RichTextRange::new(1, 4));
    assert_eq!(layout.glyphs[0].orientation, GlyphOrientation::Upright);
    assert_eq!(layout.runs.len(), 1);
}

#[test]
fn horizontal_layout_keeps_cursor_across_style_runs() {
    let frame = frame_with_split_runs("AB", 1, RichTextPresentation::default());
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 2);
    assert_f32_eq(layout.glyphs[0].origin.x, 24.0);
    assert!(layout.glyphs[1].origin.x > layout.glyphs[0].origin.x);
    assert_f32_eq(layout.glyphs[1].origin.y, layout.glyphs[0].origin.y);
}

#[test]
fn horizontal_layout_wraps_inside_textbox_width() {
    let frame = frame_with_run("AAAA", RichTextPresentation::default());
    let config = TextLayoutConfig {
        size: LayoutSize::new(40.0, 120.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 4);
    assert_f32_eq(layout.glyphs[0].origin.x, config.origin.x);
    assert_f32_eq(layout.glyphs[1].origin.y, config.origin.y);
    assert_f32_eq(layout.glyphs[2].origin.x, config.origin.x);
    assert_f32_eq(
        layout.glyphs[2].origin.y,
        config.origin.y + config.line_advance,
    );
    assert!(layout.bounds.unwrap().height > config.line_advance);
}

#[test]
fn horizontal_layout_transform_effect_reserves_inline_advance_for_wrapping() {
    let presentation = RichTextPresentation {
        effects: vec![RichTextEffectDescriptor {
            id: "wave".to_owned(),
            params: BTreeMap::from([
                (
                    "amp".to_owned(),
                    RichTextParam::Milli {
                        value: Milli(10000),
                    },
                ),
                (
                    "dir".to_owned(),
                    RichTextParam::Vec2 {
                        value: RichTextVec2::new(Milli::ONE, Milli::ZERO),
                    },
                ),
            ]),
            target: RichTextEffectTarget::Run,
            phase: RichTextEffectPhase::LayoutTransform,
            state_scope: RichTextStateScope::Run,
        }],
        ..RichTextPresentation::default()
    };
    let frame = frame_with_run("AA", presentation);
    let config = TextLayoutConfig {
        origin: LayoutPoint::new(0.0, 0.0),
        size: LayoutSize::new(54.0, 120.0),
        font_size: 30.0,
        line_advance: 42.0,
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 2);
    assert!(
        layout.glyphs[1].origin.y > layout.glyphs[0].origin.y,
        "layout_transform wave should reserve inline motion before wrapping"
    );
    assert!(layout.glyphs[0].advance.width > horizontal_advance('A', config.font_size));
}

#[test]
fn horizontal_layout_wraps_across_style_run_boundary() {
    let frame = frame_with_split_runs("AAAA", 2, RichTextPresentation::default());
    let config = TextLayoutConfig {
        size: LayoutSize::new(40.0, 120.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.runs.len(), 2);
    assert_f32_eq(layout.glyphs[1].origin.y, config.origin.y);
    assert_f32_eq(layout.glyphs[2].origin.x, config.origin.x);
    assert_f32_eq(
        layout.glyphs[2].origin.y,
        config.origin.y + config.line_advance,
    );
}

#[test]
fn vertical_rl_lays_out_top_to_bottom_then_right_to_left() {
    let frame = frame_with_run(
        "天地人",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(120.0, 60.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_f32_eq(layout.glyphs[0].origin.x, 102.0);
    assert_f32_eq(layout.glyphs[0].origin.y, 24.0);
    assert_f32_eq(layout.glyphs[1].origin.x, 102.0);
    assert_f32_eq(layout.glyphs[1].origin.y, 54.0);
    assert!(layout.glyphs[2].origin.x < layout.glyphs[1].origin.x);
    assert_f32_eq(layout.glyphs[2].origin.y, 24.0);
}

#[test]
fn vertical_lr_lays_out_top_to_bottom_then_left_to_right() {
    let frame = frame_with_run(
        "天地人",
        vertical_presentation(RichTextWritingMode::VerticalLr),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(120.0, 60.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_f32_eq(layout.glyphs[0].origin.x, 24.0);
    assert_f32_eq(layout.glyphs[0].origin.y, 24.0);
    assert_f32_eq(layout.glyphs[1].origin.x, 24.0);
    assert_f32_eq(layout.glyphs[1].origin.y, 54.0);
    assert!(layout.glyphs[2].origin.x > layout.glyphs[1].origin.x);
    assert_f32_eq(layout.glyphs[2].origin.y, 24.0);
}

#[test]
fn vertical_layout_transform_effect_reserves_inline_advance_for_column_breaking() {
    let presentation = RichTextPresentation {
        layout: Some(RichTextLayout {
            writing_mode: RichTextWritingMode::VerticalRl,
            ..RichTextLayout::default()
        }),
        effects: vec![RichTextEffectDescriptor {
            id: "wave".to_owned(),
            params: BTreeMap::from([
                (
                    "amp".to_owned(),
                    RichTextParam::Milli {
                        value: Milli(10000),
                    },
                ),
                (
                    "dir".to_owned(),
                    RichTextParam::Vec2 {
                        value: RichTextVec2::new(Milli::ZERO, Milli::ONE),
                    },
                ),
            ]),
            target: RichTextEffectTarget::Run,
            phase: RichTextEffectPhase::LayoutTransform,
            state_scope: RichTextStateScope::Run,
        }],
        ..RichTextPresentation::default()
    };
    let frame = frame_with_run("天地", presentation);
    let config = TextLayoutConfig {
        origin: LayoutPoint::new(0.0, 0.0),
        size: LayoutSize::new(120.0, 70.0),
        font_size: 30.0,
        line_advance: 42.0,
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 2);
    assert!(
        layout.glyphs[1].origin.x < layout.glyphs[0].origin.x,
        "layout_transform wave should reserve vertical inline motion before column breaking"
    );
    assert!(layout.glyphs[0].advance.height > config.font_size);
}

#[test]
fn vertical_layout_keeps_cursor_across_style_runs() {
    let frame = frame_with_split_runs(
        "天地",
        "天".len(),
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 2);
    assert_f32_eq(layout.glyphs[1].origin.x, layout.glyphs[0].origin.x);
    assert!(layout.glyphs[1].origin.y > layout.glyphs[0].origin.y);
}

#[test]
fn vertical_mixed_rotates_latin_and_combines_short_digits() {
    let frame = frame_with_run(
        "吾A12",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 3);
    assert_eq!(layout.glyphs[0].orientation, GlyphOrientation::Upright);
    assert_eq!(layout.glyphs[1].orientation, GlyphOrientation::SidewaysCw);
    assert_eq!(layout.glyphs[2].text, "12");
    assert_eq!(
        layout.glyphs[2].orientation,
        GlyphOrientation::TextCombineUpright
    );
}

#[test]
fn vertical_mixed_groups_sideways_latin_runs() {
    let frame = frame_with_run(
        "吾ABC12Ａ",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let config = TextLayoutConfig::default();
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 4);
    assert_eq!(layout.glyphs[0].text, "吾");
    assert_eq!(layout.glyphs[1].text, "ABC");
    assert_eq!(layout.glyphs[1].orientation, GlyphOrientation::SidewaysCw);
    assert_eq!(layout.glyphs[1].vertical_form, GlyphVerticalForm::None);
    assert!(
        layout.glyphs[1].advance.height > config.font_size,
        "sideways Latin run should use horizontal shaping advance as vertical inline extent"
    );
    assert_eq!(layout.glyphs[2].text, "12");
    assert_eq!(
        layout.glyphs[2].orientation,
        GlyphOrientation::TextCombineUpright
    );
    assert_eq!(layout.glyphs[3].text, "Ａ");
    assert_eq!(layout.glyphs[3].orientation, GlyphOrientation::Upright);
}

#[test]
fn vertical_layout_uses_grapheme_clusters_for_mixed_orientation() {
    let text = "e\u{301}👨‍👩‍👧‍👦A";
    let frame = frame_with_run(text, vertical_presentation(RichTextWritingMode::VerticalRl));
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 3);
    assert_eq!(layout.glyphs[0].text, "e\u{301}");
    assert_eq!(
        layout.glyphs[0].range,
        RichTextRange::new(0, "e\u{301}".len())
    );
    assert_eq!(layout.glyphs[0].orientation, GlyphOrientation::SidewaysCw);
    assert_eq!(layout.glyphs[1].text, "👨‍👩‍👧‍👦");
    assert_eq!(layout.glyphs[1].orientation, GlyphOrientation::Upright);
    assert_eq!(layout.glyphs[2].text, "A");
    assert_eq!(layout.glyphs[2].orientation, GlyphOrientation::SidewaysCw);
}

#[test]
fn vertical_mixed_orientation_uses_unicode_vertical_orientation_data() {
    let frame = frame_with_run(
        "AＡ。ー",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(UNICODE_VERTICAL_ORIENTATION_VERSION, "17.0.0");
    assert_eq!(layout.glyphs.len(), 4);
    assert_eq!(layout.glyphs[0].text, "A");
    assert_eq!(layout.glyphs[0].orientation, GlyphOrientation::SidewaysCw);
    assert_eq!(layout.glyphs[1].text, "Ａ");
    assert_eq!(layout.glyphs[1].orientation, GlyphOrientation::Upright);
    assert_eq!(layout.glyphs[1].vertical_form, GlyphVerticalForm::None);
    assert_eq!(layout.glyphs[2].text, "。");
    assert_eq!(layout.glyphs[2].orientation, GlyphOrientation::Upright);
    assert_eq!(
        layout.glyphs[2].vertical_form,
        GlyphVerticalForm::UprightAlternate
    );
    assert_eq!(layout.glyphs[3].text, "ー");
    assert_eq!(layout.glyphs[3].orientation, GlyphOrientation::SidewaysCw);
    assert_eq!(
        layout.glyphs[3].vertical_form,
        GlyphVerticalForm::RotatedAlternate
    );
}

#[test]
fn vertical_text_combine_uses_at_most_four_ascii_digits() {
    let frame = frame_with_run(
        "20265",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 2);
    assert_eq!(layout.glyphs[0].text, "2026");
    assert_eq!(
        layout.glyphs[0].orientation,
        GlyphOrientation::TextCombineUpright
    );
    assert_eq!(layout.glyphs[1].text, "5");
    assert_eq!(layout.glyphs[1].orientation, GlyphOrientation::SidewaysCw);
    assert_eq!(layout.glyphs[0].vertical_form, GlyphVerticalForm::None);
}

#[test]
fn vertical_crlf_advances_to_next_column_without_emitting_glyph() {
    let frame = frame_with_run(
        "天\r\n地",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 2);
    assert_eq!(layout.glyphs[0].text, "天");
    assert_eq!(layout.glyphs[1].text, "地");
    assert!(layout.glyphs[1].origin.x < layout.glyphs[0].origin.x);
    assert_f32_eq(layout.glyphs[1].origin.y, layout.glyphs[0].origin.y);
}

#[test]
fn vertical_column_breaks_use_uax14_opportunities() {
    let frame = frame_with_run(
        "天地。人",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 84.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 4);
    assert_eq!(layout.glyphs[2].text, "。");
    assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
    assert_f32_eq(
        layout.glyphs[2].origin.y,
        config.origin.y + config.size.height - config.font_size * 0.5,
    );
    assert_f32_eq(
        layout.glyphs[2].bounds.bottom(),
        config.origin.y + config.size.height + config.font_size * 0.5,
    );
    assert!(
        layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
        "closing punctuation may hang past the current column instead of violating kinsoku"
    );
    assert_eq!(layout.glyphs[3].text, "人");
    assert!(
        layout.glyphs[3].origin.x < layout.glyphs[2].origin.x,
        "the next breakable cluster should start the next vertical_rl column"
    );
    assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
}

#[test]
fn vertical_column_plan_records_jlreq_break_decisions_before_placement() {
    let frame = frame_with_run(
        "天地。人",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 84.0),
        ..TextLayoutConfig::default()
    };
    let clusters = vertical_clusters(&frame.text, RichTextVerticalLatinMode::Mixed);
    let context = RunLayoutContext {
        run_index: 0,
        range_start: 0,
        source: RichTextTextSource::Text,
        presentation: &frame.display_map.text_runs[0].presentation,
        ruby_annotations: &frame.display_map.ruby_annotations,
        config,
    };
    let plan = plan_vertical_columns(
        &clusters,
        context,
        LayoutCursor::new(
            vertical_column_start(RichTextWritingMode::VerticalRl, config),
            config.origin.y,
        ),
        None,
    );

    assert_eq!(plan.break_before, vec![false, false, false, true]);
}

#[test]
fn vertical_column_plan_pushes_line_end_prohibited_opening_punctuation() {
    let frame = frame_with_run(
        "天（地",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 84.0),
        ..TextLayoutConfig::default()
    };
    let clusters = vertical_clusters(&frame.text, RichTextVerticalLatinMode::Mixed);
    let context = RunLayoutContext {
        run_index: 0,
        range_start: 0,
        source: RichTextTextSource::Text,
        presentation: &frame.display_map.text_runs[0].presentation,
        ruby_annotations: &frame.display_map.ruby_annotations,
        config,
    };
    let plan = plan_vertical_columns(
        &clusters,
        context,
        LayoutCursor::new(
            vertical_column_start(RichTextWritingMode::VerticalRl, config),
            config.origin.y,
        ),
        None,
    );

    assert_eq!(plan.break_before, vec![false, true, false]);
}

#[test]
fn vertical_column_keeps_vertical_presentation_bracket_pair_together() {
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        for (opening_mark, closing_mark, description) in [
            ("︵", "︶", "vertical presentation parenthesis"),
            ("︷", "︸", "vertical presentation curly bracket"),
            ("︹", "︺", "vertical presentation tortoise shell bracket"),
            ("︻", "︼", "vertical presentation lenticular bracket"),
            ("︽", "︾", "vertical presentation double angle bracket"),
            ("︿", "﹀", "vertical presentation angle bracket"),
            ("﹁", "﹂", "vertical presentation corner bracket"),
            ("﹃", "﹄", "vertical presentation white corner bracket"),
            ("﹇", "﹈", "vertical presentation square bracket"),
        ] {
            let frame = frame_with_run(
                &format!("天{opening_mark}{closing_mark}人"),
                vertical_presentation(writing_mode),
            );
            let config = TextLayoutConfig {
                size: LayoutSize::new(160.0, 84.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");
            let opening = nth_laid_out_glyph(&layout, opening_mark, 0);
            let closing = nth_laid_out_glyph(&layout, closing_mark, 0);
            let person = nth_laid_out_glyph(&layout, "人", 0);

            assert_vertical_layout_after(
                &layout.glyphs[0],
                opening,
                &format!("{description} opening mark should sit after the previous cluster"),
            );
            assert_vertical_layout_after(
                opening,
                closing,
                &format!("{description} pair should stay together"),
            );
            assert!(
                closing.bounds.bottom() > config.origin.y + config.size.height,
                "{description} pair may overhang as one suffix"
            );
            assert_next_vertical_layout_column(
                closing,
                person,
                next_column_moves_right,
                &format!("ordinary text after {description} pair should start the next column"),
            );
        }
    }
}

#[test]
fn vertical_column_keeps_halfwidth_corner_bracket_pair_together() {
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run("天｢｣人", vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        let opening = nth_laid_out_glyph(&layout, "｢", 0);
        let closing = nth_laid_out_glyph(&layout, "｣", 0);
        let person = nth_laid_out_glyph(&layout, "人", 0);

        assert_vertical_layout_after(
            &layout.glyphs[0],
            opening,
            "halfwidth opening corner bracket should sit after the previous cluster",
        );
        assert_vertical_layout_after(
            opening,
            closing,
            "halfwidth corner bracket pair should stay together",
        );
        assert!(
            closing.bounds.bottom() > config.origin.y + config.size.height,
            "halfwidth corner bracket pair may overhang as one suffix"
        );
        assert_next_vertical_layout_column(
            closing,
            person,
            next_column_moves_right,
            "ordinary text after halfwidth corner bracket pair should start the next column",
        );
    }
}

#[test]
fn vertical_column_keeps_fullwidth_bracket_pairs_together() {
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        for (opening_mark, closing_mark, description) in [
            ("（", "）", "fullwidth parenthesis"),
            ("［", "］", "fullwidth square bracket"),
            ("｛", "｝", "fullwidth curly bracket"),
            ("｟", "｠", "fullwidth white parenthesis"),
        ] {
            let frame = frame_with_run(
                &format!("天{opening_mark}{closing_mark}人"),
                vertical_presentation(writing_mode),
            );
            let config = TextLayoutConfig {
                size: LayoutSize::new(160.0, 84.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");
            let opening = nth_laid_out_glyph(&layout, opening_mark, 0);
            let closing = nth_laid_out_glyph(&layout, closing_mark, 0);
            let person = nth_laid_out_glyph(&layout, "人", 0);

            assert_vertical_layout_after(
                &layout.glyphs[0],
                opening,
                &format!("{description} opening mark should sit after the previous cluster"),
            );
            assert_vertical_layout_after(
                opening,
                closing,
                &format!("{description} pair should stay together"),
            );
            assert!(
                closing.bounds.bottom() > config.origin.y + config.size.height,
                "{description} pair may overhang as one suffix"
            );
            assert_next_vertical_layout_column(
                closing,
                person,
                next_column_moves_right,
                &format!("ordinary text after {description} pair should start the next column"),
            );
        }
    }
}

#[test]
fn vertical_column_keeps_small_bracket_pairs_together() {
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        for (opening_mark, closing_mark, description) in [
            ("﹙", "﹚", "small parenthesis"),
            ("﹛", "﹜", "small curly bracket"),
            ("﹝", "﹞", "small tortoise shell bracket"),
        ] {
            let frame = frame_with_run(
                &format!("天{opening_mark}{closing_mark}人"),
                vertical_presentation(writing_mode),
            );
            let config = TextLayoutConfig {
                size: LayoutSize::new(160.0, 84.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");
            let opening = nth_laid_out_glyph(&layout, opening_mark, 0);
            let closing = nth_laid_out_glyph(&layout, closing_mark, 0);
            let person = nth_laid_out_glyph(&layout, "人", 0);

            assert_vertical_layout_after(
                &layout.glyphs[0],
                opening,
                &format!("{description} opening mark should sit after the previous cluster"),
            );
            assert_vertical_layout_after(
                opening,
                closing,
                &format!("{description} pair should stay together"),
            );
            assert!(
                closing.bounds.bottom() > config.origin.y + config.size.height,
                "{description} pair may overhang as one suffix"
            );
            assert_next_vertical_layout_column(
                closing,
                person,
                next_column_moves_right,
                &format!("ordinary text after {description} pair should start the next column"),
            );
        }
    }
}

fn assert_vertical_paragraph_dash_suffix(layout: &LaidOutText, next_column_moves_right: bool) {
    let syllable = nth_laid_out_glyph(layout, "え", 0);
    let dash = nth_laid_out_glyph(layout, "―", 0);
    let center = nth_laid_out_glyph(layout, "中", 0);
    assert_vertical_layout_after(
        syllable,
        dash,
        "dash mark should stay with the previous paragraph cluster",
    );
    assert_next_vertical_layout_column(
        dash,
        center,
        next_column_moves_right,
        "text after an overhanging dash-mark suffix should continue in the next paragraph column",
    );
}

#[test]
fn vertical_column_plan_reads_generated_pair_break_penalty() {
    let clusters = vertical_clusters("天地。「人", RichTextVerticalLatinMode::Mixed);
    let penalty = vertical_column_pair_break_penalty(&clusters, 0, 3, JlreqStrictness::Normal);

    assert_f32_eq(penalty, 25.0);
}

#[test]
fn vertical_column_pair_break_penalty_uses_jlreq_strictness() {
    let clusters = vertical_clusters("天地。「人", RichTextVerticalLatinMode::Mixed);

    assert_f32_eq(
        vertical_column_pair_break_penalty(&clusters, 0, 3, JlreqStrictness::Loose),
        5.0,
    );
    assert_f32_eq(
        vertical_column_pair_break_penalty(&clusters, 0, 3, JlreqStrictness::Strict),
        100.0,
    );
}

#[test]
fn vertical_column_plan_applies_closing_opening_penalty_to_paragraph_dp() {
    let text = "天地。「人山川海";
    let presentation = vertical_presentation(RichTextWritingMode::VerticalRl);
    let frame = frame_with_run(text, presentation);
    let clusters = vertical_clusters(text, RichTextVerticalLatinMode::Mixed);
    let loose_config = TextLayoutConfig {
        size: LayoutSize::new(260.0, 105.0),
        jlreq_strictness: JlreqStrictness::Loose,
        ..TextLayoutConfig::default()
    };
    let strict_config = TextLayoutConfig {
        jlreq_strictness: JlreqStrictness::Strict,
        ..loose_config
    };
    let loose_context = RunLayoutContext {
        run_index: 0,
        range_start: 0,
        source: RichTextTextSource::Text,
        presentation: &frame.display_map.text_runs[0].presentation,
        ruby_annotations: &frame.display_map.ruby_annotations,
        config: loose_config,
    };
    let strict_context = RunLayoutContext {
        config: strict_config,
        ..loose_context
    };
    let start = LayoutCursor::new(
        vertical_column_start(RichTextWritingMode::VerticalRl, loose_config),
        loose_config.origin.y,
    );

    let loose_plan = plan_vertical_columns(&clusters, loose_context, start, None);
    let strict_plan = plan_vertical_columns(&clusters, strict_context, start, None);

    assert_eq!(
        loose_plan.break_before,
        vec![false, false, false, true, false, false, true, false],
        "loose composition may break between adjacent closing and opening punctuation"
    );
    assert_eq!(
        strict_plan.break_before,
        vec![false, true, false, false, false, true, false, false],
        "strict composition should choose a different paragraph plan to avoid the weak closing/opening break"
    );

    let loose_layout = layout_frame(&frame, loose_config).expect("loose layout succeeds");
    let loose_full_stop = nth_laid_out_glyph(&loose_layout, "。", 0);
    let loose_open = nth_laid_out_glyph(&loose_layout, "「", 0);
    assert_next_vertical_layout_column(
        loose_full_stop,
        loose_open,
        false,
        "loose layout should expose the weaker closing/opening break in geometry",
    );

    let strict_layout = layout_frame(&frame, strict_config).expect("strict layout succeeds");
    let strict_full_stop = nth_laid_out_glyph(&strict_layout, "。", 0);
    let strict_open = nth_laid_out_glyph(&strict_layout, "「", 0);
    assert_vertical_layout_after(
        strict_full_stop,
        strict_open,
        "strict layout should keep adjacent closing/opening punctuation in one column",
    );
}

#[test]
fn vertical_column_plan_applies_middle_dot_opening_strict_pair_to_paragraph_dp() {
    let text = "天地・「人山川海";
    let presentation = vertical_presentation(RichTextWritingMode::VerticalRl);
    let frame = frame_with_run(text, presentation);
    let clusters = vertical_clusters(text, RichTextVerticalLatinMode::Mixed);
    let loose_config = TextLayoutConfig {
        size: LayoutSize::new(260.0, 105.0),
        jlreq_strictness: JlreqStrictness::Loose,
        ..TextLayoutConfig::default()
    };
    let strict_config = TextLayoutConfig {
        jlreq_strictness: JlreqStrictness::Strict,
        ..loose_config
    };
    let loose_context = RunLayoutContext {
        run_index: 0,
        range_start: 0,
        source: RichTextTextSource::Text,
        presentation: &frame.display_map.text_runs[0].presentation,
        ruby_annotations: &frame.display_map.ruby_annotations,
        config: loose_config,
    };
    let strict_context = RunLayoutContext {
        config: strict_config,
        ..loose_context
    };
    let start = LayoutCursor::new(
        vertical_column_start(RichTextWritingMode::VerticalRl, loose_config),
        loose_config.origin.y,
    );

    let loose_plan = plan_vertical_columns(&clusters, loose_context, start, None);
    let strict_plan = plan_vertical_columns(&clusters, strict_context, start, None);

    assert_eq!(
        loose_plan.break_before,
        vec![false, false, false, true, false, false, true, false],
        "loose composition may break between a middle dot and opening punctuation"
    );
    assert_eq!(
        strict_plan.break_before,
        vec![false, true, false, false, false, true, false, false],
        "strict composition should choose a different paragraph plan to keep the middle-dot/opening pair together"
    );

    let loose_layout = layout_frame(&frame, loose_config).expect("loose layout succeeds");
    let loose_middle_dot = nth_laid_out_glyph(&loose_layout, "・", 0);
    let loose_open = nth_laid_out_glyph(&loose_layout, "「", 0);
    assert_next_vertical_layout_column(
        loose_middle_dot,
        loose_open,
        false,
        "loose layout should expose the middle-dot/opening weak break in geometry",
    );

    let strict_layout = layout_frame(&frame, strict_config).expect("strict layout succeeds");
    let strict_middle_dot = nth_laid_out_glyph(&strict_layout, "・", 0);
    let strict_open = nth_laid_out_glyph(&strict_layout, "「", 0);
    assert_vertical_layout_after(
        strict_middle_dot,
        strict_open,
        "strict layout should keep middle-dot/opening punctuation in one column",
    );
}

#[test]
fn vertical_column_pair_break_penalty_reads_expanded_jlreq_pairs() {
    let leader_clusters = vertical_clusters("天…人", RichTextVerticalLatinMode::Mixed);
    assert_f32_eq(
        vertical_column_pair_break_penalty(&leader_clusters, 0, 1, JlreqStrictness::Loose),
        50.0,
    );
    assert_f32_eq(
        vertical_column_pair_break_penalty(&leader_clusters, 0, 1, JlreqStrictness::Normal),
        500.0,
    );

    let middle_dot_clusters = vertical_clusters("天・人", RichTextVerticalLatinMode::Mixed);
    assert_f32_eq(
        vertical_column_pair_break_penalty(&middle_dot_clusters, 0, 1, JlreqStrictness::Strict),
        1000.0,
    );

    let middle_dot_open_clusters = vertical_clusters("天・「人", RichTextVerticalLatinMode::Mixed);
    assert_f32_eq(
        vertical_column_pair_break_penalty(&middle_dot_open_clusters, 0, 2, JlreqStrictness::Loose),
        0.0,
    );
    assert_f32_eq(
        vertical_column_pair_break_penalty(
            &middle_dot_open_clusters,
            0,
            2,
            JlreqStrictness::Normal,
        ),
        15.0,
    );
    assert_f32_eq(
        vertical_column_pair_break_penalty(
            &middle_dot_open_clusters,
            0,
            2,
            JlreqStrictness::Strict,
        ),
        1000.0,
    );

    let bracket_clusters = vertical_clusters("「」人", RichTextVerticalLatinMode::Mixed);
    assert_f32_eq(
        vertical_column_pair_break_penalty(&bracket_clusters, 0, 1, JlreqStrictness::Normal),
        1000.0,
    );
}

#[test]
fn rich_text_layout_jlreq_strictness_overrides_host_config_when_explicit() {
    let mut presentation = vertical_presentation(RichTextWritingMode::VerticalRl);
    presentation
        .layout
        .as_mut()
        .expect("vertical presentation has layout")
        .jlreq_strictness = RichTextJlreqStrictness::Strict;
    let config = TextLayoutConfig {
        jlreq_strictness: JlreqStrictness::Loose,
        ..TextLayoutConfig::default()
    };

    let resolved = text_layout_config_for_presentation(config, &presentation);

    assert_eq!(resolved.jlreq_strictness, JlreqStrictness::Strict);
}

#[test]
fn rich_text_layout_jlreq_auto_inherits_host_config() {
    let presentation = vertical_presentation(RichTextWritingMode::VerticalRl);
    let config = TextLayoutConfig {
        jlreq_strictness: JlreqStrictness::Strict,
        ..TextLayoutConfig::default()
    };

    let resolved = text_layout_config_for_presentation(config, &presentation);

    assert_eq!(resolved.jlreq_strictness, JlreqStrictness::Strict);
}

#[test]
fn vertical_hanging_punctuation_limits_column_overhang_to_half_cell() {
    let frame = frame_with_run(
        "天地、人人",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 84.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");
    let column_end = config.origin.y + config.size.height;

    assert_eq!(layout.glyphs.len(), 5);
    assert_eq!(layout.glyphs[2].text, "、");
    assert_f32_eq(layout.glyphs[2].advance.height, config.font_size * 0.5);
    assert_f32_eq(
        layout.glyphs[2].origin.y,
        column_end - config.font_size * 0.5,
    );
    assert_f32_eq(
        layout.glyphs[2].bounds.bottom(),
        column_end + config.font_size * 0.5,
    );
    assert_eq!(layout.glyphs[3].text, "人");
    assert!(
        layout.glyphs[3].origin.x < layout.glyphs[2].origin.x,
        "ordinary text after hanging punctuation should start the next column"
    );
    assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
}

#[test]
fn vertical_fullwidth_and_halfwidth_closing_punctuation_hangs() {
    for (mark, label) in [
        ("？", "fullwidth question mark"),
        ("：", "fullwidth colon"),
        ("；", "fullwidth semicolon"),
        ("｡", "halfwidth full stop"),
        ("､", "halfwidth ideographic comma"),
    ] {
        for (writing_mode, next_column_moves_right) in [
            (RichTextWritingMode::VerticalRl, false),
            (RichTextWritingMode::VerticalLr, true),
        ] {
            let frame = frame_with_run(
                &format!("天地{mark}人"),
                vertical_presentation(writing_mode),
            );
            let config = TextLayoutConfig {
                size: LayoutSize::new(160.0, 84.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");
            let punctuation = nth_laid_out_glyph(&layout, mark, 0);
            let person = nth_laid_out_glyph(&layout, "人", 0);

            assert_eq!(punctuation.text, mark);
            assert_f32_eq(punctuation.advance.height, config.font_size * 0.5);
            assert_f32_eq(punctuation.origin.x, layout.glyphs[1].origin.x);
            assert!(
                punctuation.bounds.bottom() > config.origin.y + config.size.height,
                "{label} should hang past the {writing_mode:?} column end"
            );
            assert_next_vertical_layout_column(
                punctuation,
                person,
                next_column_moves_right,
                "ordinary text after closing punctuation should start the next column",
            );
        }
    }
}

#[test]
fn vertical_punctuation_compression_keeps_following_text_in_column() {
    let frame = frame_with_run(
        "天、。人",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 90.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 4);
    assert_eq!(layout.glyphs[1].text, "、");
    assert_eq!(layout.glyphs[2].text, "。");
    assert_f32_eq(layout.glyphs[1].advance.height, config.font_size * 0.5);
    assert_f32_eq(layout.glyphs[2].advance.height, config.font_size * 0.5);
    assert_f32_eq(layout.glyphs[1].bounds.height, config.font_size);
    assert_f32_eq(layout.glyphs[2].bounds.height, config.font_size);
    assert_eq!(layout.glyphs[3].text, "人");
    assert_f32_eq(layout.glyphs[3].origin.x, layout.glyphs[0].origin.x);
    assert!(
        layout.glyphs[3].origin.y < config.origin.y + config.size.height,
        "compressed punctuation should leave room for the following cluster"
    );
}

#[test]
fn vertical_consecutive_punctuation_compression_uses_half_cell_advances() {
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run("天、。・人", vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 120.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        let body = nth_laid_out_glyph(&layout, "天", 0);
        let comma = nth_laid_out_glyph(&layout, "、", 0);
        let period = nth_laid_out_glyph(&layout, "。", 0);
        let middle_dot = nth_laid_out_glyph(&layout, "・", 0);
        let person = nth_laid_out_glyph(&layout, "人", 0);

        assert_same_vertical_layout_column(
            body,
            person,
            "consecutive compressed punctuation should leave the following text in the same column",
        );
        for punctuation in [comma, period, middle_dot] {
            assert_f32_eq(punctuation.advance.height, config.font_size * 0.5);
            assert_f32_eq(punctuation.bounds.height, config.font_size);
        }
        assert_vertical_layout_after(body, comma, "comma should follow body text");
        assert_vertical_layout_after(comma, period, "full stop should follow comma");
        assert_vertical_layout_after(period, middle_dot, "middle dot should follow full stop");
        assert_vertical_layout_after(
            middle_dot,
            person,
            "body text should follow the compressed punctuation chain",
        );
    }
}

#[test]
fn vertical_column_keeps_small_kana_out_of_column_heads() {
    for (text, mark) in [("天地ぁ人", "ぁ"), ("天地ｯ人", "ｯ"), ("天地ㇰ人", "ㇰ")]
    {
        assert_vertical_rl_no_column_head_mark(
            text,
            mark,
            "small kana may overhang the current column instead of starting the next column",
        );
    }
}

fn assert_vertical_rl_no_column_head_mark(text: &str, mark: &str, message: &str) {
    let frame = frame_with_run(text, vertical_presentation(RichTextWritingMode::VerticalRl));
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 84.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 4);
    assert_eq!(layout.glyphs[2].text, mark);
    assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
    assert!(
        layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
        "{message}"
    );
    assert_eq!(layout.glyphs[3].text, "人");
    assert!(
        layout.glyphs[3].origin.x < layout.glyphs[2].origin.x,
        "the next ordinary cluster should start the next vertical_rl column"
    );
    assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
}

#[test]
fn vertical_column_keeps_jlreq_leaders_together() {
    let frame = frame_with_run(
        "天……人",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 84.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 4);
    assert_eq!(layout.glyphs[1].text, "…");
    assert_eq!(layout.glyphs[2].text, "…");
    assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
    assert!(
        layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
        "the second leader mark should stay with the first instead of starting a new column"
    );
    assert_eq!(layout.glyphs[3].text, "人");
    assert!(layout.glyphs[3].origin.x < layout.glyphs[2].origin.x);
    assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
}

#[test]
fn vertical_column_keeps_vertical_presentation_leaders_together() {
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        for leader in ["︙", "︰"] {
            let frame = frame_with_run(
                &format!("天{leader}{leader}人"),
                vertical_presentation(writing_mode),
            );
            let config = TextLayoutConfig {
                size: LayoutSize::new(160.0, 84.0),
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");

            assert_eq!(layout.glyphs.len(), 4);
            assert_eq!(layout.glyphs[1].text, leader);
            assert_eq!(layout.glyphs[2].text, leader);
            assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
            assert!(
                layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
                "second vertical presentation leader should stay with the first"
            );
            assert_eq!(layout.glyphs[3].text, "人");
            assert_next_vertical_layout_column(
                &layout.glyphs[2],
                &layout.glyphs[3],
                next_column_moves_right,
                "ordinary text after vertical presentation leaders should start the next column",
            );
        }
    }
}

#[test]
fn vertical_column_keeps_jlreq_leader_chain_together_before_next_column() {
    let frame = frame_with_run(
        "天………人",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 84.0),
        ..TextLayoutConfig::default()
    };
    let clusters = vertical_clusters(&frame.text, RichTextVerticalLatinMode::Mixed);
    let context = RunLayoutContext {
        run_index: 0,
        range_start: 0,
        source: RichTextTextSource::Text,
        presentation: &frame.display_map.text_runs[0].presentation,
        ruby_annotations: &frame.display_map.ruby_annotations,
        config,
    };
    let plan = plan_vertical_columns(
        &clusters,
        context,
        LayoutCursor::new(
            vertical_column_start(RichTextWritingMode::VerticalRl, config),
            config.origin.y,
        ),
        None,
    );
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(plan.break_before, vec![false, false, false, false, true]);
    assert_eq!(layout.glyphs.len(), 5);
    assert_eq!(layout.glyphs[1].text, "…");
    assert_eq!(layout.glyphs[2].text, "…");
    assert_eq!(layout.glyphs[3].text, "…");
    assert_f32_eq(layout.glyphs[1].origin.x, layout.glyphs[0].origin.x);
    assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[0].origin.x);
    assert_f32_eq(layout.glyphs[3].origin.x, layout.glyphs[0].origin.x);
    assert!(
        layout.glyphs[3].bounds.bottom() > config.origin.y + config.size.height,
        "the leader chain may overhang as one unbreakable trailing suffix"
    );
    assert_eq!(layout.glyphs[4].text, "人");
    assert!(
        layout.glyphs[4].origin.x < layout.glyphs[3].origin.x,
        "ordinary text after the leader chain should start the next vertical_rl column"
    );
    assert_f32_eq(layout.glyphs[4].origin.y, config.origin.y);
}

#[test]
fn vertical_lr_column_keeps_jlreq_leader_chain_together_before_next_column() {
    let frame = frame_with_run(
        "天………人",
        vertical_presentation(RichTextWritingMode::VerticalLr),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 84.0),
        ..TextLayoutConfig::default()
    };
    let clusters = vertical_clusters(&frame.text, RichTextVerticalLatinMode::Mixed);
    let context = RunLayoutContext {
        run_index: 0,
        range_start: 0,
        source: RichTextTextSource::Text,
        presentation: &frame.display_map.text_runs[0].presentation,
        ruby_annotations: &frame.display_map.ruby_annotations,
        config,
    };
    let plan = plan_vertical_columns(
        &clusters,
        context,
        LayoutCursor::new(
            vertical_column_start(RichTextWritingMode::VerticalLr, config),
            config.origin.y,
        ),
        None,
    );
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(plan.break_before, vec![false, false, false, false, true]);
    assert_eq!(layout.glyphs.len(), 5);
    assert_eq!(layout.glyphs[1].text, "…");
    assert_eq!(layout.glyphs[2].text, "…");
    assert_eq!(layout.glyphs[3].text, "…");
    assert_f32_eq(layout.glyphs[1].origin.x, layout.glyphs[0].origin.x);
    assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[0].origin.x);
    assert_f32_eq(layout.glyphs[3].origin.x, layout.glyphs[0].origin.x);
    assert!(
        layout.glyphs[3].bounds.bottom() > config.origin.y + config.size.height,
        "the vertical_lr leader chain may overhang as one unbreakable trailing suffix"
    );
    assert_eq!(layout.glyphs[4].text, "人");
    assert!(
        layout.glyphs[4].origin.x > layout.glyphs[3].origin.x,
        "ordinary text after the leader chain should start the next vertical_lr column"
    );
    assert_f32_eq(layout.glyphs[4].origin.y, config.origin.y);
}

#[test]
fn vertical_column_keeps_jlreq_dashes_together() {
    let frame = frame_with_run(
        "天――人",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 84.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 4);
    assert_eq!(layout.glyphs[1].text, "―");
    assert_eq!(layout.glyphs[2].text, "―");
    assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
    assert!(
        layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
        "the second dash should stay with the first instead of starting a new column"
    );
    assert_eq!(layout.glyphs[3].text, "人");
    assert!(layout.glyphs[3].origin.x < layout.glyphs[2].origin.x);
    assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
}

#[test]
fn vertical_lr_column_keeps_jlreq_dashes_together() {
    let frame = frame_with_run(
        "天――人",
        vertical_presentation(RichTextWritingMode::VerticalLr),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 84.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 4);
    assert_eq!(layout.glyphs[1].text, "―");
    assert_eq!(layout.glyphs[2].text, "―");
    assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
    assert!(
        layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
        "the second vertical_lr dash should stay with the first instead of starting a new column"
    );
    assert_eq!(layout.glyphs[3].text, "人");
    assert!(layout.glyphs[3].origin.x > layout.glyphs[2].origin.x);
    assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
}

#[test]
fn vertical_column_keeps_prolonged_sound_mark_out_of_column_heads() {
    for (text, mark) in [("天地ー人", "ー"), ("天地ｰ人", "ｰ")] {
        assert_vertical_rl_no_column_head_mark(
            text,
            mark,
            "prolonged sound marks should overhang instead of starting the next column",
        );
    }
}

#[test]
fn vertical_column_keeps_middle_dot_out_of_column_heads() {
    for (text, mark) in [("天地・人", "・"), ("天地･人", "･")] {
        assert_vertical_rl_no_column_head_mark(
            text,
            mark,
            "middle dots should overhang instead of starting the next column",
        );
    }
}

#[test]
fn vertical_column_keeps_jlreq_iteration_marks_with_previous_cluster() {
    let frame = frame_with_run(
        "天地々人",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 84.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 4);
    assert_eq!(layout.glyphs[2].text, "々");
    assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
    assert!(
        layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
        "iteration marks should stay with the previous cluster instead of starting a new column"
    );
    assert_eq!(layout.glyphs[3].text, "人");
    assert!(layout.glyphs[3].origin.x < layout.glyphs[2].origin.x);
    assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
}

#[test]
fn vertical_lr_column_keeps_small_kana_out_of_column_heads() {
    assert_vertical_lr_no_column_head_mark("天地ぁ人", "ぁ");
    assert_vertical_lr_no_column_head_mark("天地ｯ人", "ｯ");
    assert_vertical_lr_no_column_head_mark("天地ㇰ人", "ㇰ");
}

#[test]
fn vertical_lr_column_keeps_prolonged_sound_mark_out_of_column_heads() {
    assert_vertical_lr_no_column_head_mark("天地ー人", "ー");
    assert_vertical_lr_no_column_head_mark("天地ｰ人", "ｰ");
}

#[test]
fn vertical_lr_column_keeps_middle_dot_out_of_column_heads() {
    assert_vertical_lr_no_column_head_mark("天地・人", "・");
    assert_vertical_lr_no_column_head_mark("天地･人", "･");
}

#[test]
fn vertical_lr_column_keeps_jlreq_iteration_marks_with_previous_cluster() {
    assert_vertical_lr_no_column_head_mark("天地々人", "々");
}

fn assert_vertical_lr_no_column_head_mark(text: &str, mark: &str) {
    let frame = frame_with_run(text, vertical_presentation(RichTextWritingMode::VerticalLr));
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 84.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 4);
    assert_eq!(layout.glyphs[2].text, mark);
    assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
    assert!(
        layout.glyphs[2].bounds.bottom() > config.origin.y + config.size.height,
        "{mark} should overhang the current vertical_lr column instead of starting the next column"
    );
    assert_eq!(layout.glyphs[3].text, "人");
    assert!(
        layout.glyphs[3].origin.x > layout.glyphs[2].origin.x,
        "the next ordinary cluster should start the next vertical_lr column"
    );
    assert_f32_eq(layout.glyphs[3].origin.y, config.origin.y);
}

#[test]
fn vertical_column_breaks_before_jlreq_line_end_prohibited_opening_punctuation() {
    let frame = frame_with_run(
        "天（地",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 84.0),
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 3);
    assert_eq!(layout.glyphs[1].text, "（");
    assert!(
        layout.glyphs[1].origin.x < layout.glyphs[0].origin.x,
        "opening punctuation should not remain at the previous column end"
    );
    assert_f32_eq(layout.glyphs[1].origin.y, config.origin.y);
    assert_eq!(layout.glyphs[2].text, "地");
    assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
    assert!(layout.glyphs[2].origin.y > layout.glyphs[1].origin.y);
}

fn push_ruby(frame: &mut LineDisplayFrame, start: usize, end: usize, ruby: &str) {
    frame
        .display_map
        .ruby_annotations
        .push(RichTextRubyAnnotation {
            base_range: RichTextRange::new(start, end),
            ruby: ruby.to_owned(),
            node_index: frame.display_map.ruby_annotations.len(),
            styles: Vec::new(),
            presentation: RichTextPresentation::default(),
        });
}

fn nth_laid_out_glyph<'layout>(
    layout: &'layout LaidOutText,
    text: &str,
    occurrence: usize,
) -> &'layout LaidOutGlyph {
    layout
        .glyphs
        .iter()
        .filter(|glyph| glyph.text == text)
        .nth(occurrence)
        .unwrap_or_else(|| panic!("missing laid-out glyph {text:?} occurrence {occurrence}"))
}

fn vertical_layout_column_count(layout: &LaidOutText) -> usize {
    layout
        .glyphs
        .iter()
        .map(|glyph| glyph.origin.x.to_bits())
        .collect::<HashSet<_>>()
        .len()
}

fn assert_same_vertical_layout_column(
    previous: &LaidOutGlyph,
    current: &LaidOutGlyph,
    message: &str,
) {
    assert_f32_eq(previous.origin.x, current.origin.x);
    assert!(
        current.origin.y > previous.origin.y,
        "{message}: expected {current:?} to advance after {previous:?}"
    );
}

fn assert_vertical_layout_after(previous: &LaidOutGlyph, current: &LaidOutGlyph, message: &str) {
    assert_same_vertical_layout_column(previous, current, message);
}

fn assert_next_vertical_layout_column(
    previous: &LaidOutGlyph,
    current: &LaidOutGlyph,
    next_column_moves_right: bool,
    message: &str,
) {
    if next_column_moves_right {
        assert!(
            current.origin.x > previous.origin.x,
            "{message}: expected {current:?} to move right after {previous:?}"
        );
    } else {
        assert!(
            current.origin.x < previous.origin.x,
            "{message}: expected {current:?} to move left after {previous:?}"
        );
    }
    assert!(
        current.origin.y < previous.origin.y,
        "{message}: expected {current:?} to restart above {previous:?}"
    );
}

fn assert_vertical_layout_column_restart(
    previous: &LaidOutGlyph,
    current: &LaidOutGlyph,
    next_column_moves_right: bool,
    message: &str,
) {
    if next_column_moves_right {
        assert!(
            current.origin.x > previous.origin.x,
            "{message}: expected {current:?} to move right after {previous:?}"
        );
    } else {
        assert!(
            current.origin.x < previous.origin.x,
            "{message}: expected {current:?} to move left after {previous:?}"
        );
    }
}

fn assert_f32_eq(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < f32::EPSILON,
        "expected {actual} to equal {expected}"
    );
}

fn assert_f32_near(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {actual} to be within {tolerance} of {expected}"
    );
}
