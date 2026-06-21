use super::*;

#[test]
fn vertical_column_plan_balances_paragraph_with_dp_cost() {
    let frame = frame_with_run(
        "天地玄黄宇宙",
        vertical_presentation(RichTextWritingMode::VerticalRl),
    );
    let config = TextLayoutConfig {
        size: LayoutSize::new(160.0, 168.0),
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

    assert_eq!(
        plan.break_before,
        vec![false, false, false, true, false, false]
    );
}

#[test]
fn vertical_paragraph_plan_combines_published_jlreq_line_composition_classes() {
    // W3C JLREQ 3.1 groups these as line-head/line-end and
    // separation-prohibited punctuation classes; keep them together in one
    // paragraph plan instead of only proving isolated two-cluster cases.
    let text = "天地春夏秋冬月火、山々人「川」あっいおーえ―中・外………終";
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 90.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert!(
            vertical_layout_column_count(&layout) >= 7,
            "{writing_mode:?} JLREQ paragraph should require a multi-column plan: {layout:?}"
        );

        let fire = nth_laid_out_glyph(&layout, "火", 0);
        let comma = nth_laid_out_glyph(&layout, "、", 0);
        let mountain = nth_laid_out_glyph(&layout, "山", 0);
        assert_vertical_layout_after(fire, comma, "comma should follow body text");
        assert_f32_eq(comma.advance.height, config.font_size * 0.5);
        assert_next_vertical_layout_column(
            comma,
            mountain,
            next_column_moves_right,
            "text after a column-end comma should continue in the next paragraph column",
        );

        let iteration = nth_laid_out_glyph(&layout, "々", 0);
        let person = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_after(
            mountain,
            iteration,
            "iteration mark should stay with the previous cluster in paragraph context",
        );
        assert_vertical_layout_after(
            iteration,
            person,
            "text after an iteration mark should continue in the same paragraph column when it fits",
        );

        let open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        let close = nth_laid_out_glyph(&layout, "」", 0);
        assert_vertical_layout_after(
            open,
            river,
            "opening bracket should not strand its base text",
        );
        assert_vertical_layout_after(
            river,
            close,
            "closing bracket should stay with its base text",
        );

        let large_kana = nth_laid_out_glyph(&layout, "あ", 0);
        let small_kana = nth_laid_out_glyph(&layout, "っ", 0);
        let next_kana = nth_laid_out_glyph(&layout, "い", 0);
        assert_vertical_layout_after(
            large_kana,
            small_kana,
            "small kana should stay out of a paragraph column head",
        );
        assert_vertical_layout_after(
            small_kana,
            next_kana,
            "text after a small kana should continue in the same paragraph column when it fits",
        );

        assert_vertical_paragraph_dash_suffix(&layout, next_column_moves_right);

        let middle_dot = nth_laid_out_glyph(&layout, "・", 0);
        let outside = nth_laid_out_glyph(&layout, "外", 0);
        assert_same_vertical_layout_column(
            middle_dot,
            outside,
            "middle-dot compression should keep following paragraph text in the same column",
        );
        assert!(outside.origin.y > middle_dot.origin.y);

        let first_leader = nth_laid_out_glyph(&layout, "…", 0);
        let second_leader = nth_laid_out_glyph(&layout, "…", 1);
        let third_leader = nth_laid_out_glyph(&layout, "…", 2);
        let ending = nth_laid_out_glyph(&layout, "終", 0);
        assert_vertical_layout_after(
            first_leader,
            second_leader,
            "repeated leaders should stay together in paragraph context",
        );
        assert_vertical_layout_after(
            second_leader,
            third_leader,
            "the full leader chain should stay together in paragraph context",
        );
        assert!(
            third_leader.bounds.bottom() > config.origin.y + config.size.height,
            "leader chain should overhang as one paragraph suffix: {third_leader:?}"
        );
        assert_next_vertical_layout_column(
            third_leader,
            ending,
            next_column_moves_right,
            "text after an overhanging leader chain should continue in the next paragraph column",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_strict_closing_opening_pair_inside_class_mix() {
    let text = "天地春夏秋冬月火、山々人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let loose_config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 147.0),
            jlreq_strictness: JlreqStrictness::Loose,
            ..TextLayoutConfig::default()
        };
        let strict_config = TextLayoutConfig {
            jlreq_strictness: JlreqStrictness::Strict,
            ..loose_config
        };

        let loose = layout_frame(&frame, loose_config).expect("loose layout succeeds");
        let loose_full_stop = nth_laid_out_glyph(&loose, "。", 0);
        let loose_open = nth_laid_out_glyph(&loose, "「", 0);
        assert_vertical_layout_after(
            loose_full_stop,
            loose_open,
            "loose paragraph class mix may keep adjacent closing/opening punctuation when it fits",
        );

        let strict = layout_frame(&frame, strict_config).expect("strict layout succeeds");
        assert!(
            vertical_layout_column_count(&strict) >= 7,
            "{writing_mode:?} strict JLREQ paragraph should still require a multi-column plan: {strict:?}"
        );
        let person = nth_laid_out_glyph(&strict, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&strict, "。", 0);
        let strict_open = nth_laid_out_glyph(&strict, "「", 0);
        let river = nth_laid_out_glyph(&strict, "川", 0);
        let close = nth_laid_out_glyph(&strict, "」", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should keep closing punctuation after its base",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should keep adjacent closing/opening punctuation together",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should not strand its following base",
        );
        assert_vertical_layout_after(
            river,
            close,
            "strict paragraph class mix should keep closing bracket with its base",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_plain_western_word_inside_class_mix() {
    let text = "天地春夏秋冬Web人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with a plain Western word should require a multi-column plan: {layout:?}"
        );

        let word = nth_laid_out_glyph(&layout, "Web", 0);
        assert_eq!(word.orientation, GlyphOrientation::SidewaysCw);

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a plain Western word",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a plain Western word",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a plain Western word",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_hyphenated_word_inside_class_mix() {
    let text = "天地春夏秋冬Web-Test人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with a hyphenated Western word should require a multi-column plan: {layout:?}"
        );

        let first = nth_laid_out_glyph(&layout, "Web", 0);
        let hyphen = nth_laid_out_glyph(&layout, "-", 0);
        let after_hyphen = nth_laid_out_glyph(&layout, "Test", 0);
        assert_vertical_layout_after(
            first,
            hyphen,
            "word-internal hyphen should stay attached inside a paragraph class mix",
        );
        assert_vertical_layout_after(
            hyphen,
            after_hyphen,
            "letters after a word-internal hyphen should stay attached inside a paragraph class mix",
        );
        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after its base after a Western word",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a Western word",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a Western word",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_apostrophe_word_inside_class_mix() {
    for (text, joiner, description) in [
        (
            "天地春夏秋冬O'K人。「川」あっいおーえ―中・外………終",
            "'",
            "ASCII apostrophe Western word",
        ),
        (
            "天地春夏秋冬O’K人。「川」あっいおーえ―中・外………終",
            "’",
            "typographic apostrophe Western word",
        ),
    ] {
        for writing_mode in [
            RichTextWritingMode::VerticalRl,
            RichTextWritingMode::VerticalLr,
        ] {
            let frame = frame_with_run(text, vertical_presentation(writing_mode));
            let config = TextLayoutConfig {
                size: LayoutSize::new(210.0, 210.0),
                jlreq_strictness: JlreqStrictness::Strict,
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");
            assert!(
                vertical_layout_column_count(&layout) >= 4,
                "{writing_mode:?} published JLREQ paragraph with an {description} should require a multi-column plan: {layout:?}"
            );

            let first = nth_laid_out_glyph(&layout, "O", 0);
            let apostrophe = nth_laid_out_glyph(&layout, joiner, 0);
            let after_apostrophe = nth_laid_out_glyph(&layout, "K", 0);
            assert_vertical_layout_after(
                first,
                apostrophe,
                "word-internal apostrophe should stay attached to the preceding letter inside a paragraph class mix",
            );
            assert_vertical_layout_after(
                apostrophe,
                after_apostrophe,
                "letter after a word-internal apostrophe should stay attached inside a paragraph class mix",
            );

            let person = nth_laid_out_glyph(&layout, "人", 0);
            let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
            let strict_open = nth_laid_out_glyph(&layout, "「", 0);
            let river = nth_laid_out_glyph(&layout, "川", 0);
            assert_vertical_layout_after(
                person,
                strict_full_stop,
                "strict paragraph class mix should still keep closing punctuation after an apostrophe Western word",
            );
            assert_vertical_layout_after(
                strict_full_stop,
                strict_open,
                "strict paragraph class mix should still keep adjacent closing/opening punctuation after an apostrophe Western word",
            );
            assert_vertical_layout_after(
                strict_open,
                river,
                "strict opening punctuation should still stay with its base after an apostrophe Western word",
            );
        }
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_accented_latin_word_inside_class_mix() {
    let text = "天地春夏秋冬café人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with an accented Latin word should require a multi-column plan: {layout:?}"
        );

        let word = nth_laid_out_glyph(&layout, "café", 0);
        assert_eq!(word.orientation, GlyphOrientation::SidewaysCw);

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after an accented Latin word",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after an accented Latin word",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after an accented Latin word",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_decomposed_accented_latin_word_inside_class_mix() {
    let text = "天地春夏秋冬cafe\u{301}人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with a decomposed accented Latin word should require a multi-column plan: {layout:?}"
        );

        let word = nth_laid_out_glyph(&layout, "cafe\u{301}", 0);
        assert_eq!(word.orientation, GlyphOrientation::SidewaysCw);

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a decomposed accented Latin word",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a decomposed accented Latin word",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a decomposed accented Latin word",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_zwj_grapheme_inside_strict_class_mix() {
    let text = "天地春夏秋冬👩‍💻人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} strict paragraph with a ZWJ grapheme should require a multi-column plan: {layout:?}"
        );

        let zwj = nth_laid_out_glyph(&layout, "👩‍💻", 0);
        assert_eq!(zwj.range, RichTextRange::new(18, 29));
        assert_eq!(zwj.orientation, GlyphOrientation::Upright);

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a ZWJ grapheme",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a ZWJ grapheme",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a ZWJ grapheme",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_decomposed_kana_inside_strict_class_mix() {
    let text = "天地春夏秋冬か\u{3099}人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} strict paragraph with a decomposed kana grapheme should require a multi-column plan: {layout:?}"
        );

        let kana = nth_laid_out_glyph(&layout, "か\u{3099}", 0);
        assert_eq!(kana.range, RichTextRange::new(18, 24));
        assert_eq!(kana.orientation, GlyphOrientation::Upright);

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a decomposed kana grapheme",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a decomposed kana grapheme",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a decomposed kana grapheme",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_variation_selector_grapheme_inside_strict_class_mix() {
    let text = "天地春夏秋冬漢\u{fe00}人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} strict paragraph with a variation-selector grapheme should require a multi-column plan: {layout:?}"
        );

        let variant = nth_laid_out_glyph(&layout, "漢\u{fe00}", 0);
        assert_eq!(variant.range, RichTextRange::new(18, 24));
        assert_eq!(variant.orientation, GlyphOrientation::Upright);
        assert_eq!(variant.vertical_form, GlyphVerticalForm::None);

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a variation-selector grapheme",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a variation-selector grapheme",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a variation-selector grapheme",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_emoji_modifier_grapheme_inside_strict_class_mix() {
    let text = "天地春夏秋冬👍🏽人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} strict paragraph with an emoji modifier grapheme should require a multi-column plan: {layout:?}"
        );

        let emoji = nth_laid_out_glyph(&layout, "👍🏽", 0);
        assert_eq!(emoji.range, RichTextRange::new(18, 26));
        assert_eq!(emoji.orientation, GlyphOrientation::Upright);
        assert_eq!(emoji.vertical_form, GlyphVerticalForm::None);

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after an emoji modifier grapheme",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after an emoji modifier grapheme",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after an emoji modifier grapheme",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_regional_indicator_grapheme_inside_strict_class_mix() {
    let text = "天地春夏秋冬🇯🇵人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} strict paragraph with a regional-indicator grapheme should require a multi-column plan: {layout:?}"
        );

        let flag = nth_laid_out_glyph(&layout, "🇯🇵", 0);
        assert_eq!(flag.range, RichTextRange::new(18, 26));
        assert_eq!(flag.orientation, GlyphOrientation::Upright);
        assert_eq!(flag.vertical_form, GlyphVerticalForm::None);

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a regional-indicator grapheme",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a regional-indicator grapheme",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a regional-indicator grapheme",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_keycap_grapheme_inside_strict_class_mix() {
    let text = "天地春夏秋冬1️⃣人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} strict paragraph with a keycap grapheme should require a multi-column plan: {layout:?}"
        );

        let keycap = nth_laid_out_glyph(&layout, "1️⃣", 0);
        assert_eq!(keycap.range, RichTextRange::new(18, 25));
        assert_eq!(keycap.orientation, GlyphOrientation::Upright);
        assert_eq!(keycap.vertical_form, GlyphVerticalForm::None);

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a keycap grapheme",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a keycap grapheme",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a keycap grapheme",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_unit_symbol_inside_class_mix() {
    for (text, unit_text, description) in [
        (
            "天地春夏秋冬kg人。「川」あっいおーえ―中・外………終",
            "kg",
            "Latin unit symbol",
        ),
        (
            "天地春夏秋冬μm人。「川」あっいおーえ―中・外………終",
            "μm",
            "Greek+Latin unit symbol",
        ),
    ] {
        for writing_mode in [
            RichTextWritingMode::VerticalRl,
            RichTextWritingMode::VerticalLr,
        ] {
            let frame = frame_with_run(text, vertical_presentation(writing_mode));
            let config = TextLayoutConfig {
                size: LayoutSize::new(210.0, 210.0),
                jlreq_strictness: JlreqStrictness::Strict,
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");
            assert!(
                vertical_layout_column_count(&layout) >= 4,
                "{writing_mode:?} published JLREQ paragraph with a {description} should require a multi-column plan: {layout:?}"
            );

            let unit = nth_laid_out_glyph(&layout, unit_text, 0);
            assert_eq!(unit.orientation, GlyphOrientation::SidewaysCw);

            let person = nth_laid_out_glyph(&layout, "人", 0);
            let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
            let strict_open = nth_laid_out_glyph(&layout, "「", 0);
            let river = nth_laid_out_glyph(&layout, "川", 0);
            assert_vertical_layout_after(
                person,
                strict_full_stop,
                "strict paragraph class mix should still keep closing punctuation after a unit symbol",
            );
            assert_vertical_layout_after(
                strict_full_stop,
                strict_open,
                "strict paragraph class mix should still keep adjacent closing/opening punctuation after a unit symbol",
            );
            assert_vertical_layout_after(
                strict_open,
                river,
                "strict opening punctuation should still stay with its base after a unit symbol",
            );
        }
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_numeric_unit_inside_class_mix() {
    for (text, unit_text, description) in [
        (
            "天地春夏秋冬3kg人。「川」あっいおーえ―中・外………終",
            "kg",
            "numeric Latin unit",
        ),
        (
            "天地春夏秋冬3μm人。「川」あっいおーえ―中・外………終",
            "μm",
            "numeric Greek+Latin unit",
        ),
    ] {
        for writing_mode in [
            RichTextWritingMode::VerticalRl,
            RichTextWritingMode::VerticalLr,
        ] {
            let frame = frame_with_run(text, vertical_presentation(writing_mode));
            let config = TextLayoutConfig {
                size: LayoutSize::new(210.0, 210.0),
                jlreq_strictness: JlreqStrictness::Strict,
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");
            assert!(
                vertical_layout_column_count(&layout) >= 4,
                "{writing_mode:?} published JLREQ paragraph with a {description} should require a multi-column plan: {layout:?}"
            );

            let digit = nth_laid_out_glyph(&layout, "3", 0);
            let unit = nth_laid_out_glyph(&layout, unit_text, 0);
            assert_vertical_layout_after(
                digit,
                unit,
                "numeric unit symbol should stay attached to the preceding digit inside a paragraph class mix",
            );

            let person = nth_laid_out_glyph(&layout, "人", 0);
            let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
            let strict_open = nth_laid_out_glyph(&layout, "「", 0);
            let river = nth_laid_out_glyph(&layout, "川", 0);
            assert_vertical_layout_after(
                person,
                strict_full_stop,
                "strict paragraph class mix should still keep closing punctuation after a numeric unit",
            );
            assert_vertical_layout_after(
                strict_full_stop,
                strict_open,
                "strict paragraph class mix should still keep adjacent closing/opening punctuation after a numeric unit",
            );
            assert_vertical_layout_after(
                strict_open,
                river,
                "strict opening punctuation should still stay with its base after a numeric unit",
            );
        }
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_numeric_separator_inside_class_mix() {
    let text = "天地春夏秋冬1,234.56人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with numeric separators should require a multi-column plan: {layout:?}"
        );

        let first_digit = nth_laid_out_glyph(&layout, "1", 0);
        let comma = nth_laid_out_glyph(&layout, ",", 0);
        let middle_digits = nth_laid_out_glyph(&layout, "234", 0);
        let decimal_point = nth_laid_out_glyph(&layout, ".", 0);
        let final_digits = nth_laid_out_glyph(&layout, "56", 0);
        assert_vertical_layout_after(
            first_digit,
            comma,
            "comma place separator should stay with its preceding digit inside a paragraph class mix",
        );
        assert_vertical_layout_after(
            comma,
            middle_digits,
            "digits after comma place separator should stay attached inside a paragraph class mix",
        );
        assert_vertical_layout_after(
            middle_digits,
            decimal_point,
            "decimal point should stay with its preceding digit chunk inside a paragraph class mix",
        );
        assert_vertical_layout_after(
            decimal_point,
            final_digits,
            "digits after decimal point should stay attached inside a paragraph class mix",
        );

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a numeric separator sequence",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a numeric separator sequence",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a numeric separator sequence",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_prefixed_abbreviation_inside_class_mix() {
    let text = "天地春夏秋冬$123人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with a prefixed abbreviation should require a multi-column plan: {layout:?}"
        );

        let prefix = nth_laid_out_glyph(&layout, "$", 0);
        let digits = nth_laid_out_glyph(&layout, "123", 0);
        assert_vertical_layout_after(
            prefix,
            digits,
            "prefixed abbreviation should stay attached to following digits inside a paragraph class mix",
        );

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a prefixed abbreviation",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a prefixed abbreviation",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a prefixed abbreviation",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_cent_prefixed_abbreviation_inside_class_mix() {
    let text = "天地春夏秋冬¢123人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with a cent-prefixed abbreviation should require a multi-column plan: {layout:?}"
        );

        let prefix = nth_laid_out_glyph(&layout, "¢", 0);
        let digits = nth_laid_out_glyph(&layout, "123", 0);
        assert_vertical_layout_after(
            prefix,
            digits,
            "cent-prefixed abbreviation should stay attached to following digits inside a paragraph class mix",
        );

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a cent-prefixed abbreviation",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a cent-prefixed abbreviation",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a cent-prefixed abbreviation",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_yen_prefixed_abbreviation_inside_class_mix() {
    for (text, prefix, description) in [
        (
            "天地春夏秋冬¥123人。「川」あっいおーえ―中・外………終",
            "¥",
            "yen-prefixed abbreviation",
        ),
        (
            "天地春夏秋冬￥123人。「川」あっいおーえ―中・外………終",
            "￥",
            "fullwidth yen-prefixed abbreviation",
        ),
    ] {
        for writing_mode in [
            RichTextWritingMode::VerticalRl,
            RichTextWritingMode::VerticalLr,
        ] {
            let frame = frame_with_run(text, vertical_presentation(writing_mode));
            let config = TextLayoutConfig {
                size: LayoutSize::new(210.0, 210.0),
                jlreq_strictness: JlreqStrictness::Strict,
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");
            assert!(
                vertical_layout_column_count(&layout) >= 4,
                "{writing_mode:?} published JLREQ paragraph with a {description} should require a multi-column plan: {layout:?}"
            );

            let prefix = nth_laid_out_glyph(&layout, prefix, 0);
            let digits = nth_laid_out_glyph(&layout, "123", 0);
            assert_vertical_layout_after(
                prefix,
                digits,
                "yen-prefixed abbreviation should stay attached to following digits inside a paragraph class mix",
            );

            let person = nth_laid_out_glyph(&layout, "人", 0);
            let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
            let strict_open = nth_laid_out_glyph(&layout, "「", 0);
            let river = nth_laid_out_glyph(&layout, "川", 0);
            assert_vertical_layout_after(
                person,
                strict_full_stop,
                "strict paragraph class mix should still keep closing punctuation after a yen-prefixed abbreviation",
            );
            assert_vertical_layout_after(
                strict_full_stop,
                strict_open,
                "strict paragraph class mix should still keep adjacent closing/opening punctuation after a yen-prefixed abbreviation",
            );
            assert_vertical_layout_after(
                strict_open,
                river,
                "strict opening punctuation should still stay with its base after a yen-prefixed abbreviation",
            );
        }
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_postfixed_abbreviation_inside_class_mix() {
    let text = "天地春夏秋冬50%人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with a postfixed abbreviation should require a multi-column plan: {layout:?}"
        );

        let digits = nth_laid_out_glyph(&layout, "50", 0);
        let suffix = nth_laid_out_glyph(&layout, "%", 0);
        assert_vertical_layout_after(
            digits,
            suffix,
            "postfixed abbreviation should stay attached to preceding digits inside a paragraph class mix",
        );

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a postfixed abbreviation",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a postfixed abbreviation",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a postfixed abbreviation",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_ideographic_abbreviation_inside_class_mix() {
    for (text, leading, trailing, description) in [
        (
            "天地春夏秋冬$五人。「川」あっいおーえ―中・外………終",
            "$",
            "五",
            "prefixed ideographic abbreviation",
        ),
        (
            "天地春夏秋冬五%人。「川」あっいおーえ―中・外………終",
            "五",
            "%",
            "postfixed ideographic abbreviation",
        ),
    ] {
        for writing_mode in [
            RichTextWritingMode::VerticalRl,
            RichTextWritingMode::VerticalLr,
        ] {
            let frame = frame_with_run(text, vertical_presentation(writing_mode));
            let config = TextLayoutConfig {
                size: LayoutSize::new(210.0, 210.0),
                jlreq_strictness: JlreqStrictness::Strict,
                ..TextLayoutConfig::default()
            };
            let layout = layout_frame(&frame, config).expect("layout succeeds");
            assert!(
                vertical_layout_column_count(&layout) >= 4,
                "{writing_mode:?} published JLREQ paragraph with a {description} should require a multi-column plan: {layout:?}"
            );

            let leading = nth_laid_out_glyph(&layout, leading, 0);
            let trailing = nth_laid_out_glyph(&layout, trailing, 0);
            assert_vertical_layout_after(
                leading,
                trailing,
                "ideographic numeric abbreviation should stay attached inside a paragraph class mix",
            );

            let person = nth_laid_out_glyph(&layout, "人", 0);
            let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
            let strict_open = nth_laid_out_glyph(&layout, "「", 0);
            let river = nth_laid_out_glyph(&layout, "川", 0);
            assert_vertical_layout_after(
                person,
                strict_full_stop,
                "strict paragraph class mix should still keep closing punctuation after an ideographic abbreviation",
            );
            assert_vertical_layout_after(
                strict_full_stop,
                strict_open,
                "strict paragraph class mix should still keep adjacent closing/opening punctuation after an ideographic abbreviation",
            );
            assert_vertical_layout_after(
                strict_open,
                river,
                "strict opening punctuation should still stay with its base after an ideographic abbreviation",
            );
        }
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_reference_mark_inside_class_mix() {
    let text = "天地春夏秋冬本¹²。人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with reference marks should require a multi-column plan: {layout:?}"
        );

        let body = nth_laid_out_glyph(&layout, "本", 0);
        let first_mark = nth_laid_out_glyph(&layout, "¹", 0);
        let second_mark = nth_laid_out_glyph(&layout, "²", 0);
        let reference_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        assert_vertical_layout_after(
            body,
            first_mark,
            "reference mark should stay with the preceding main-text cluster inside a paragraph class mix",
        );
        assert_vertical_layout_after(
            first_mark,
            second_mark,
            "reference mark digits should stay together inside a paragraph class mix",
        );
        assert_f32_eq(second_mark.origin.x, reference_full_stop.origin.x);
        assert!(
            reference_full_stop.bounds.bottom() > second_mark.origin.y,
            "full stop after a reference mark should stay attached inside a paragraph class mix: {reference_full_stop:?}"
        );

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 1);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a reference mark sequence",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a reference mark sequence",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a reference mark sequence",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_parenthesized_reference_mark_inside_class_mix() {
    let text = "天地春夏秋冬本⁽¹⁾。人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with parenthesized reference marks should require a multi-column plan: {layout:?}"
        );

        let body = nth_laid_out_glyph(&layout, "本", 0);
        let open = nth_laid_out_glyph(&layout, "⁽", 0);
        let mark = nth_laid_out_glyph(&layout, "¹", 0);
        let close = nth_laid_out_glyph(&layout, "⁾", 0);
        let reference_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        assert_vertical_layout_after(
            body,
            open,
            "parenthesized reference mark should stay with preceding main text inside a paragraph class mix",
        );
        assert_vertical_layout_after(
            open,
            mark,
            "parenthesized reference digit should stay with its opening bracket inside a paragraph class mix",
        );
        assert_f32_eq(mark.origin.x, close.origin.x);
        assert!(
            close.bounds.bottom() > mark.origin.y,
            "parenthesized reference closing bracket should stay attached inside a paragraph class mix: {close:?}"
        );
        assert_f32_eq(close.origin.x, reference_full_stop.origin.x);
        assert!(
            reference_full_stop.bounds.bottom() > close.origin.y,
            "full stop after a parenthesized reference mark should stay attached inside a paragraph class mix: {reference_full_stop:?}"
        );

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 1);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a parenthesized reference mark sequence",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a parenthesized reference mark sequence",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a parenthesized reference mark sequence",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_temperature_suffix_inside_class_mix() {
    let text = "天地春夏秋冬25℃人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with temperature suffix should require a multi-column plan: {layout:?}"
        );

        let digits = nth_laid_out_glyph(&layout, "25", 0);
        let suffix = nth_laid_out_glyph(&layout, "℃", 0);
        assert_vertical_layout_after(
            digits,
            suffix,
            "temperature suffix abbreviation should stay with preceding digits inside a paragraph class mix",
        );

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a temperature suffix",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a temperature suffix",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a temperature suffix",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_decomposed_temperature_inside_class_mix() {
    let text = "天地春夏秋冬25°C人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with decomposed temperature unit should require a multi-column plan: {layout:?}"
        );

        let digits = nth_laid_out_glyph(&layout, "25", 0);
        let degree = nth_laid_out_glyph(&layout, "°", 0);
        let unit = nth_laid_out_glyph(&layout, "C", 0);
        assert_vertical_layout_after(
            digits,
            degree,
            "degree suffix abbreviation should stay with preceding digits inside a paragraph class mix",
        );
        assert_vertical_layout_after(
            degree,
            unit,
            "Latin temperature unit tail should stay with the degree suffix inside a paragraph class mix",
        );

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a decomposed temperature unit",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a decomposed temperature unit",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a decomposed temperature unit",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_subscript_object_inside_class_mix() {
    let text = "天地春夏秋冬H₂O人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with subscript object should require a multi-column plan: {layout:?}"
        );

        let base = nth_laid_out_glyph(&layout, "H", 0);
        let mark = nth_laid_out_glyph(&layout, "₂", 0);
        let following_base = nth_laid_out_glyph(&layout, "O", 0);
        assert_vertical_layout_after(
            base,
            mark,
            "subscript should stay with the preceding base inside a paragraph class mix",
        );
        assert_vertical_layout_after(
            mark,
            following_base,
            "following base should stay attached to the subscript object inside a paragraph class mix",
        );

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a subscript object",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a subscript object",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a subscript object",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_greek_subscript_object_inside_class_mix() {
    let text = "天地春夏秋冬α₂β人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with Greek subscript object should require a multi-column plan: {layout:?}"
        );

        let base = nth_laid_out_glyph(&layout, "α", 0);
        let mark = nth_laid_out_glyph(&layout, "₂", 0);
        let following_base = nth_laid_out_glyph(&layout, "β", 0);
        assert_vertical_layout_after(
            base,
            mark,
            "Greek subscript should stay with the preceding base inside a paragraph class mix",
        );
        assert_vertical_layout_after(
            mark,
            following_base,
            "Greek following base should stay attached to the subscript object inside a paragraph class mix",
        );

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a Greek subscript object",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a Greek subscript object",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a Greek subscript object",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_published_jlreq_greek_superscript_object_inside_class_mix() {
    let text = "天地春夏秋冬α²β人。「川」あっいおーえ―中・外………終";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_run(text, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 210.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        assert!(
            vertical_layout_column_count(&layout) >= 4,
            "{writing_mode:?} published JLREQ paragraph with Greek superscript object should require a multi-column plan: {layout:?}"
        );

        let base = nth_laid_out_glyph(&layout, "α", 0);
        let mark = nth_laid_out_glyph(&layout, "²", 0);
        let following_base = nth_laid_out_glyph(&layout, "β", 0);
        assert_vertical_layout_after(
            base,
            mark,
            "Greek superscript should stay with the preceding base inside a paragraph class mix",
        );
        assert_vertical_layout_after(
            mark,
            following_base,
            "Greek following base should stay attached to the superscript object inside a paragraph class mix",
        );

        let person = nth_laid_out_glyph(&layout, "人", 0);
        let strict_full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let strict_open = nth_laid_out_glyph(&layout, "「", 0);
        let river = nth_laid_out_glyph(&layout, "川", 0);
        assert_vertical_layout_after(
            person,
            strict_full_stop,
            "strict paragraph class mix should still keep closing punctuation after a Greek superscript object",
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict paragraph class mix should still keep adjacent closing/opening punctuation after a Greek superscript object",
        );
        assert_vertical_layout_after(
            strict_open,
            river,
            "strict opening punctuation should still stay with its base after a Greek superscript object",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_strict_pair_after_ruby_text_combine() {
    let text = "夢2026。「人山川海";
    let dream_start = 0;
    let dream_end = "夢".len();
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let mut frame = frame_with_run(text, vertical_presentation(writing_mode));
        push_ruby(&mut frame, dream_start, dream_end, "ゆめ");
        let config = TextLayoutConfig {
            size: LayoutSize::new(260.0, 105.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };

        let strict = layout_frame(&frame, config).expect("strict layout succeeds");
        assert_eq!(strict.ruby.len(), 1);
        let text_combine = nth_laid_out_glyph(&strict, "2026", 0);
        let strict_full_stop = nth_laid_out_glyph(&strict, "。", 0);
        let strict_open = nth_laid_out_glyph(&strict, "「", 0);
        let person = nth_laid_out_glyph(&strict, "人", 0);
        assert_eq!(text_combine.range, RichTextRange::new(3, 7));
        assert_eq!(
            text_combine.orientation,
            GlyphOrientation::TextCombineUpright
        );
        assert_vertical_layout_after(
            strict_full_stop,
            strict_open,
            "strict ruby/text-combine paragraph keeps adjacent closing/opening punctuation together",
        );
        assert_vertical_layout_after(
            strict_open,
            person,
            "strict opening punctuation should not strand its following base after text-combine",
        );
    }
}

#[test]
fn vertical_paragraph_plan_can_restart_after_ruby_run_before_text_combine() {
    let text = "夢2026。「人山川海";
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let presentation = vertical_presentation(writing_mode);
        let mut frame = frame_with_run(text, presentation.clone());
        frame.display_map.text_runs = vec![
            RichTextTextRun {
                range: RichTextRange::new(0, "夢".len()),
                source: RichTextTextSource::RubyBase,
                node_index: 0,
                styles: Vec::new(),
                presentation: presentation.clone(),
            },
            RichTextTextRun {
                range: RichTextRange::new("夢".len(), text.len()),
                source: RichTextTextSource::Text,
                node_index: 1,
                styles: Vec::new(),
                presentation,
            },
        ];
        push_ruby(&mut frame, 0, "夢".len(), "ゆめ");
        let config = TextLayoutConfig {
            size: LayoutSize::new(260.0, 105.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };

        let layout = layout_frame(&frame, config).expect("layout succeeds");
        let text_combine = nth_laid_out_glyph(&layout, "2026", 0);
        let full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let opening = nth_laid_out_glyph(&layout, "「", 0);
        let person = nth_laid_out_glyph(&layout, "人", 0);

        assert_eq!(
            text_combine.orientation,
            GlyphOrientation::TextCombineUpright
        );
        assert_f32_eq(text_combine.origin.y, config.origin.y);
        assert_vertical_layout_after(
            full_stop,
            opening,
            "strict run-start restart should keep closing/opening punctuation together",
        );
        assert_vertical_layout_after(
            opening,
            person,
            "strict run-start restart should keep opening punctuation with its base",
        );
    }
}

#[test]
fn vertical_paragraph_plan_keeps_strict_pair_across_text_run_boundary() {
    let text = "天地。「人山川海";
    let split_at = "天地。".len();
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let frame = frame_with_split_runs(text, split_at, vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(260.0, 105.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };

        let layout = layout_frame(&frame, config).expect("layout succeeds");
        let full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let opening = nth_laid_out_glyph(&layout, "「", 0);
        let person = nth_laid_out_glyph(&layout, "人", 0);
        assert_vertical_layout_after(
            full_stop,
            opening,
            "strict closing/opening punctuation should stay together across text run boundary",
        );
        assert_vertical_layout_after(
            opening,
            person,
            "strict opening punctuation should keep its base across text run boundary",
        );
    }
}

#[test]
fn vertical_hard_line_break_resets_strict_jlreq_paragraph_segment() {
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let frame = frame_with_run("天地。\n「人外", vertical_presentation(writing_mode));
        let config = TextLayoutConfig {
            size: LayoutSize::new(210.0, 105.0),
            jlreq_strictness: JlreqStrictness::Strict,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");
        let full_stop = nth_laid_out_glyph(&layout, "。", 0);
        let opening = nth_laid_out_glyph(&layout, "「", 0);
        let person = nth_laid_out_glyph(&layout, "人", 0);

        assert_next_vertical_layout_column(
            full_stop,
            opening,
            next_column_moves_right,
            "explicit hard line break should start a new strict JLREQ paragraph segment",
        );
        assert_vertical_layout_after(
            opening,
            person,
            "text after the hard-break opening punctuation should stay in its new segment column",
        );
        assert_f32_eq(opening.origin.y, config.origin.y);
    }
}
