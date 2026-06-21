use super::*;

#[test]
fn ruby_uses_base_geometry() {
    let mut frame = frame_with_run("夢", RichTextPresentation::default());
    frame
        .display_map
        .ruby_annotations
        .push(RichTextRubyAnnotation {
            base_range: RichTextRange::new(0, "夢".len()),
            ruby: "ゆめ".to_owned(),
            node_index: 0,
            styles: Vec::new(),
            presentation: RichTextPresentation::default(),
        });
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(layout.ruby.len(), 1);
    assert_eq!(layout.ruby[0].base_bounds, layout.glyphs[0].bounds);
    assert!(layout.ruby[0].ruby_bounds.y < layout.glyphs[0].bounds.y);
}

#[test]
fn horizontal_glyph_bounds_track_font_size_inside_line_advance() {
    let frame = frame_with_run("夢", RichTextPresentation::default());
    let config = TextLayoutConfig::default();
    let layout = layout_frame(&frame, config).expect("layout succeeds");
    let glyph = &layout.glyphs[0];

    assert_f32_eq(glyph.origin.y, config.origin.y);
    assert_f32_eq(glyph.bounds.height, config.font_size);
    assert_f32_eq(
        glyph.bounds.y,
        config.origin.y + (config.line_advance - config.font_size) * 0.5,
    );
}

#[test]
fn horizontal_latin_uses_proportional_deterministic_advance() {
    let frame = frame_with_run("ialwm.", RichTextPresentation::default());
    let config = TextLayoutConfig::default();
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 6);
    assert!(layout.glyphs[0].advance.width < layout.glyphs[3].advance.width);
    assert!(layout.glyphs[5].advance.width < layout.glyphs[1].advance.width);
    let total = layout
        .glyphs
        .iter()
        .map(|glyph| glyph.advance.width)
        .sum::<f32>();
    assert!(total < config.font_size * 6.0);
}

#[test]
fn vertical_glyph_bounds_track_font_size_inside_line_advance() {
    let frame = frame_with_run("夢", vertical_presentation(RichTextWritingMode::VerticalRl));
    let config = TextLayoutConfig::default();
    let layout = layout_frame(&frame, config).expect("layout succeeds");
    let glyph = &layout.glyphs[0];

    assert_f32_eq(glyph.origin.x, glyph.bounds.x);
    assert_f32_eq(glyph.bounds.width, config.font_size);
    assert_f32_eq(glyph.bounds.height, config.font_size);
}

#[test]
fn default_horizontal_ruby_gap_applies_after_html_overlap() {
    let mut frame = frame_with_run("夢", RichTextPresentation::default());
    push_ruby(&mut frame, 0, "夢".len(), "ゆめ");
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    let base = layout.ruby[0].base_bounds;
    let annotation = layout.ruby[0].ruby_bounds;
    let metrics =
        ruby_metrics_from_presentation(&layout.ruby[0].presentation, TextLayoutConfig::default());
    assert_f32_near(
        base.y - annotation.bottom(),
        DEFAULT_RUBY_GAP - horizontal_ruby_html_overlap(metrics),
        0.001,
    );
}

#[test]
fn horizontal_ruby_zero_gap_matches_html_like_overlap() {
    let mut frame = frame_with_run("夢", RichTextPresentation::default());
    push_ruby(&mut frame, 0, "夢".len(), "ゆめ");
    frame.display_map.ruby_annotations[0].presentation.layout = Some(RichTextLayout {
        ruby_gap: Some(Milli(0)),
        ..RichTextLayout::default()
    });
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    let base = layout.ruby[0].base_bounds;
    let annotation = layout.ruby[0].ruby_bounds;
    let metrics =
        ruby_metrics_from_presentation(&layout.ruby[0].presentation, TextLayoutConfig::default());
    assert_f32_near(
        base.y - annotation.bottom(),
        -horizontal_ruby_html_overlap(metrics),
        0.001,
    );
}

#[test]
fn horizontal_ruby_under_zero_gap_matches_html_like_overlap() {
    let mut frame = frame_with_run("夢", RichTextPresentation::default());
    push_ruby(&mut frame, 0, "夢".len(), "ゆめ");
    frame.display_map.ruby_annotations[0].presentation.layout = Some(RichTextLayout {
        ruby_gap: Some(Milli(0)),
        ruby_position: RichTextRubyPosition::Under,
        ..RichTextLayout::default()
    });
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    let base = layout.ruby[0].base_bounds;
    let annotation = layout.ruby[0].ruby_bounds;
    let metrics =
        ruby_metrics_from_presentation(&layout.ruby[0].presentation, TextLayoutConfig::default());
    assert_f32_near(
        annotation.y - base.bottom(),
        -horizontal_ruby_html_overlap(metrics),
        0.001,
    );
}

#[test]
fn horizontal_ruby_collision_shifts_adjacent_annotations() {
    let mut frame = frame_with_run("夢星", RichTextPresentation::default());
    push_ruby(&mut frame, 0, "夢".len(), "ながいよみ");
    push_ruby(&mut frame, "夢".len(), "夢星".len(), "ながいよみ");
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(layout.ruby.len(), 2);
    assert_eq!(
        layout.ruby[0].writing_mode,
        RichTextWritingMode::HorizontalTb
    );
    assert!(
        !layout.ruby[0]
            .ruby_bounds
            .intersects(layout.ruby[1].ruby_bounds)
    );
    assert!(
        layout.ruby[1].ruby_bounds.x >= layout.ruby[0].ruby_bounds.right(),
        "second horizontal ruby should move after the first annotation"
    );
}

#[test]
fn long_horizontal_ruby_expands_base_allocation_before_overhang() {
    let mut frame = frame_with_run("夢", RichTextPresentation::default());
    push_ruby(&mut frame, 0, "夢".len(), "ながいよみ");
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(layout.ruby.len(), 1);
    assert!(
        layout.ruby[0].base_bounds.width > layout.glyphs[0].bounds.width,
        "long ruby should expand the base allocation before using overhang"
    );
    assert_f32_eq(
        layout.ruby[0].base_bounds.width,
        layout.ruby[0].ruby_bounds.width,
    );
    assert_f32_eq(layout.ruby[0].ruby_bounds.x, layout.ruby[0].base_bounds.x);
}

#[test]
fn long_horizontal_ruby_reserves_inline_advance_before_following_text() {
    let mut frame = frame_with_run("政を", RichTextPresentation::default());
    push_ruby(&mut frame, 0, "政".len(), "まつりごと");
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(layout.ruby.len(), 1);
    assert!(
        layout.glyphs[1].bounds.x >= layout.ruby[0].base_bounds.right(),
        "following body glyph should start after the expanded long-ruby base"
    );
    assert_f32_eq(
        layout.glyphs[0].bounds.x + layout.glyphs[0].bounds.width * 0.5,
        layout.ruby[0].base_bounds.x + layout.ruby[0].base_bounds.width * 0.5,
    );
}

#[test]
fn short_horizontal_ruby_centers_over_wide_base() {
    let base = "中央の帝国将官たち";
    let mut frame = frame_with_run(base, RichTextPresentation::default());
    push_ruby(&mut frame, 0, base.len(), "ぐん");
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(layout.ruby.len(), 1);
    let base_center = layout.ruby[0].base_bounds.x + layout.ruby[0].base_bounds.width * 0.5;
    let ruby_center = layout.ruby[0].ruby_bounds.x + layout.ruby[0].ruby_bounds.width * 0.5;
    assert_f32_eq(base_center, ruby_center);
    assert!(
        layout.ruby[0].ruby_bounds.width < layout.ruby[0].base_bounds.width,
        "short ruby should remain centered instead of expanding the base"
    );
}

#[test]
fn long_vertical_ruby_reserves_inline_advance_before_following_text() {
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let mut frame = frame_with_run("政を", vertical_presentation(writing_mode));
        push_ruby(&mut frame, 0, "政".len(), "まつりごと");
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 160.0),
            ruby_font_size: 13.0,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 2);
        assert_eq!(layout.ruby.len(), 1);
        assert!(
            layout.ruby[0].base_bounds.height > layout.glyphs[0].bounds.height,
            "long {writing_mode:?} ruby should expand base allocation along inline progression"
        );
        assert!(
            layout.glyphs[1].bounds.y >= layout.ruby[0].base_bounds.bottom(),
            "following {writing_mode:?} body glyph should start after the expanded ruby base"
        );
        assert_f32_eq(layout.glyphs[0].bounds.y, layout.ruby[0].base_bounds.y);
    }
}

#[test]
fn short_vertical_ruby_centers_beside_tall_base() {
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let base = "中央の帝国将官たち";
        let mut frame = frame_with_run(base, vertical_presentation(writing_mode));
        push_ruby(&mut frame, 0, base.len(), "ぐん");
        let config = TextLayoutConfig {
            size: LayoutSize::new(240.0, 520.0),
            ruby_font_size: 13.0,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.ruby.len(), 1);
        let base_center = layout.ruby[0].base_bounds.y + layout.ruby[0].base_bounds.height * 0.5;
        let ruby_center = layout.ruby[0].ruby_bounds.y + layout.ruby[0].ruby_bounds.height * 0.5;
        assert_f32_eq(base_center, ruby_center);
        assert!(
            layout.ruby[0].ruby_bounds.height < layout.ruby[0].base_bounds.height,
            "short {writing_mode:?} ruby should remain centered instead of expanding the base"
        );
    }
}

#[test]
fn horizontal_ruby_uses_limited_overhang_after_base_expansion() {
    let mut frame = frame_with_run("夢", RichTextPresentation::default());
    push_ruby(&mut frame, 0, "夢".len(), "ながいよみか");
    let config = TextLayoutConfig {
        size: LayoutSize::new(60.0, 120.0),
        ruby_font_size: 12.0,
        ..TextLayoutConfig::default()
    };
    let layout = layout_frame(&frame, config).expect("layout succeeds");

    assert_eq!(layout.ruby.len(), 1);
    assert_f32_eq(layout.ruby[0].base_bounds.width, 60.0);
    assert_f32_eq(
        layout.ruby[0].base_bounds.x - layout.ruby[0].ruby_bounds.x,
        config.ruby_font_size * 0.5,
    );
    assert_f32_eq(
        layout.ruby[0].ruby_bounds.right() - layout.ruby[0].base_bounds.right(),
        config.ruby_font_size * 0.5,
    );
    assert!(
        layout.ruby[0].base_bounds.x - layout.ruby[0].ruby_bounds.x <= config.ruby_font_size * 0.5
    );
}

#[test]
fn vertical_ruby_collision_shifts_adjacent_annotations_inline() {
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let mut frame = frame_with_run("夢星", vertical_presentation(writing_mode));
        push_ruby(&mut frame, 0, "夢".len(), "ながいよみ");
        push_ruby(&mut frame, "夢".len(), "夢星".len(), "ながいよみ");
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.ruby.len(), 2);
        assert_eq!(layout.ruby[0].writing_mode, writing_mode);
        assert!(
            !layout.ruby[0]
                .ruby_bounds
                .intersects(layout.ruby[1].ruby_bounds)
        );
        assert!(
            layout.ruby[1].ruby_bounds.y >= layout.ruby[0].ruby_bounds.bottom(),
            "second {writing_mode:?} ruby should move below the first annotation"
        );
        assert_f32_eq(layout.ruby[1].ruby_bounds.x, layout.ruby[0].ruby_bounds.x);
    }
}

#[test]
fn vertical_lr_ruby_uses_left_annotation_track_with_base_expansion() {
    let mut frame = frame_with_run("夢", vertical_presentation(RichTextWritingMode::VerticalLr));
    push_ruby(&mut frame, 0, "夢".len(), "ながいよみ");
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    assert_eq!(layout.ruby.len(), 1);
    assert_eq!(layout.ruby[0].writing_mode, RichTextWritingMode::VerticalLr);
    assert!(
        layout.ruby[0].base_bounds.height > layout.glyphs[0].bounds.height,
        "long vertical ruby should expand the base allocation along inline progression"
    );
    assert!(
        layout.ruby[0].ruby_bounds.x + layout.ruby[0].ruby_bounds.width * 0.5
            < layout.ruby[0].base_bounds.x + layout.ruby[0].base_bounds.width * 0.5,
        "vertical_lr ruby annotation should be placed on the left side of the base"
    );
}

#[test]
fn vertical_ruby_under_flips_annotation_track() {
    for (writing_mode, under_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let mut frame = frame_with_run("夢", vertical_presentation(writing_mode));
        push_ruby(&mut frame, 0, "夢".len(), "ゆめ");
        frame.display_map.ruby_annotations[0].presentation.layout = Some(RichTextLayout {
            writing_mode,
            ruby_position: RichTextRubyPosition::Under,
            ..RichTextLayout::default()
        });
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        let base = layout.ruby[0].base_bounds;
        let annotation = layout.ruby[0].ruby_bounds;
        let base_center = base.x + base.width * 0.5;
        let annotation_center = annotation.x + annotation.width * 0.5;
        if under_moves_right {
            assert!(
                annotation_center > base_center,
                "{writing_mode:?} ruby_under should place annotation on the right side of the base"
            );
        } else {
            assert!(
                annotation_center < base_center,
                "{writing_mode:?} ruby_under should place annotation on the left side of the base"
            );
        }
    }
}

#[test]
fn vertical_ruby_side_tracks_apply_html_like_overlap() {
    for (writing_mode, position, expected_right_side) in [
        (
            RichTextWritingMode::VerticalRl,
            RichTextRubyPosition::Over,
            true,
        ),
        (
            RichTextWritingMode::VerticalRl,
            RichTextRubyPosition::Under,
            false,
        ),
        (
            RichTextWritingMode::VerticalLr,
            RichTextRubyPosition::Over,
            false,
        ),
        (
            RichTextWritingMode::VerticalLr,
            RichTextRubyPosition::Under,
            true,
        ),
    ] {
        let mut frame = frame_with_run("夢", vertical_presentation(writing_mode));
        push_ruby(&mut frame, 0, "夢".len(), "ゆめ");
        frame.display_map.ruby_annotations[0].presentation.layout = Some(RichTextLayout {
            writing_mode,
            ruby_gap: Some(Milli(0)),
            ruby_position: position,
            ..RichTextLayout::default()
        });
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        let base = layout.ruby[0].base_bounds;
        let annotation = layout.ruby[0].ruby_bounds;
        let metrics = ruby_metrics_from_presentation(
            &layout.ruby[0].presentation,
            TextLayoutConfig::default(),
        );
        if expected_right_side {
            assert_f32_near(
                annotation.x - base.right(),
                -vertical_ruby_html_overlap(metrics),
                0.001,
            );
        } else {
            assert_f32_near(
                base.x - annotation.right(),
                -vertical_ruby_html_overlap(metrics),
                0.001,
            );
        }
    }
}

#[test]
fn vertical_inter_character_ruby_inserts_annotation_between_base_clusters() {
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let mut frame = frame_with_run("夢星", vertical_presentation(writing_mode));
        push_ruby(&mut frame, 0, "夢星".len(), "ゆめ");
        frame.display_map.ruby_annotations[0].presentation.layout = Some(RichTextLayout {
            writing_mode,
            ruby_position: RichTextRubyPosition::InterCharacter,
            ..RichTextLayout::default()
        });
        let config = TextLayoutConfig::default();
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 2);
        assert_eq!(layout.ruby.len(), 1);
        assert_eq!(layout.ruby[0].writing_mode, writing_mode);
        assert_f32_eq(layout.ruby[0].ruby_bounds.x, layout.glyphs[0].bounds.x);
        assert_f32_eq(
            layout.ruby[0].ruby_bounds.y,
            layout.glyphs[0].bounds.bottom(),
        );
        assert_f32_eq(
            layout.ruby[0].ruby_bounds.height,
            config.ruby_font_size * 2.0,
        );
        assert!(
            layout.glyphs[1].bounds.y >= layout.ruby[0].ruby_bounds.bottom(),
            "{writing_mode:?} inter-character ruby should push the following base cluster after the annotation"
        );
        assert_f32_eq(layout.glyphs[1].origin.x, layout.glyphs[0].origin.x);
    }
}

#[test]
fn vertical_inter_character_ruby_does_not_reserve_side_track() {
    let mut side_track =
        frame_with_run("夢", vertical_presentation(RichTextWritingMode::VerticalRl));
    push_ruby(&mut side_track, 0, "夢".len(), "ゆめ");
    let side_track_layout =
        layout_frame(&side_track, TextLayoutConfig::default()).expect("layout succeeds");

    let mut inter_character =
        frame_with_run("夢", vertical_presentation(RichTextWritingMode::VerticalRl));
    push_ruby(&mut inter_character, 0, "夢".len(), "ゆめ");
    inter_character.display_map.ruby_annotations[0]
        .presentation
        .layout = Some(RichTextLayout {
        writing_mode: RichTextWritingMode::VerticalRl,
        ruby_position: RichTextRubyPosition::InterCharacter,
        ..RichTextLayout::default()
    });
    let inter_character_layout =
        layout_frame(&inter_character, TextLayoutConfig::default()).expect("layout succeeds");

    assert!(
        inter_character_layout.glyphs[0].origin.x > side_track_layout.glyphs[0].origin.x,
        "inter-character ruby should keep the body column at the normal start instead of reserving an external side track"
    );
    assert_f32_eq(
        inter_character_layout.ruby[0].ruby_bounds.x,
        inter_character_layout.glyphs[0].bounds.x,
    );
}

#[test]
fn horizontal_ruby_typography_attrs_control_size_gap_and_overhang() {
    let mut frame = frame_with_run("夢", RichTextPresentation::default());
    push_ruby(&mut frame, 0, "夢".len(), "ゆめ");
    frame.display_map.ruby_annotations[0].presentation.layout = Some(RichTextLayout {
        ruby_font_size: Some(arcweft_render_text::Milli(10000)),
        ruby_gap: Some(arcweft_render_text::Milli(1000)),
        ruby_overhang: Some(arcweft_render_text::Milli(3000)),
        ruby_collision_gap: Some(arcweft_render_text::Milli(4000)),
        ..RichTextLayout::default()
    });
    let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

    let base = layout.ruby[0].base_bounds;
    let annotation = layout.ruby[0].ruby_bounds;
    assert_f32_eq(annotation.height, 10.0);
    assert_f32_eq(
        annotation.y,
        base.y - 10.0 - 1.0
            + horizontal_ruby_html_overlap(ruby_metrics_from_presentation(
                &layout.ruby[0].presentation,
                TextLayoutConfig::default(),
            )),
    );
    assert!(
        annotation.x >= base.x - 3.0,
        "overhang should constrain annotation start near the base: {annotation:?}"
    );
}

#[test]
fn vertical_ruby_reserves_annotation_track_inside_layout_width() {
    for (writing_mode, annotation_on_right) in [
        (RichTextWritingMode::VerticalRl, true),
        (RichTextWritingMode::VerticalLr, false),
    ] {
        let mut frame = frame_with_run("夢", vertical_presentation(writing_mode));
        push_ruby(&mut frame, 0, "夢".len(), "ゆめ");
        let config = TextLayoutConfig {
            origin: LayoutPoint::new(0.0, 0.0),
            size: LayoutSize::new(84.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.ruby.len(), 1);
        let base = layout.ruby[0].base_bounds;
        let annotation = layout.ruby[0].ruby_bounds;
        assert!(
            annotation.x >= config.origin.x
                && annotation.right() <= config.origin.x + config.size.width,
            "{writing_mode:?} ruby annotation should stay inside the layout width: {annotation:?}"
        );
        if annotation_on_right {
            assert!(annotation.x + annotation.width * 0.5 > base.x + base.width * 0.5);
        } else {
            assert!(annotation.x + annotation.width * 0.5 < base.x + base.width * 0.5);
        }
    }
}

#[test]
fn vertical_ruby_layout_survives_typewriter_visibility_effect() {
    for writing_mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let mut presentation = vertical_presentation(writing_mode);
        presentation.effects.push(RichTextEffectDescriptor {
            id: "typewriter".to_owned(),
            params: BTreeMap::from([(
                "cps".to_owned(),
                RichTextParam::Milli { value: Milli::ONE },
            )]),
            target: RichTextEffectTarget::Run,
            phase: RichTextEffectPhase::GlyphMask,
            state_scope: RichTextStateScope::Run,
        });
        let mut frame = frame_with_run("夢", presentation);
        frame.display_map.text_runs[0].source = RichTextTextSource::RubyBase;
        push_ruby(&mut frame, 0, "夢".len(), "ゆめ");

        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 1);
        assert_eq!(layout.ruby.len(), 1);
        assert_eq!(layout.ruby[0].base_range, RichTextRange::new(0, "夢".len()));
        assert_eq!(layout.ruby[0].writing_mode, writing_mode);
    }
}

#[test]
fn vertical_ruby_base_expansion_feeds_back_into_column_breaks() {
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let mut frame = frame_with_run("天夢", vertical_presentation(writing_mode));
        push_ruby(&mut frame, "天".len(), "天夢".len(), "ながいよみ");
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 2);
        assert_eq!(layout.glyphs[0].text, "天");
        assert_eq!(layout.glyphs[1].text, "夢");
        assert_vertical_layout_column_restart(
            &layout.glyphs[0],
            &layout.glyphs[1],
            next_column_moves_right,
            "long ruby base allocation should force the annotated cluster to the next column",
        );
        assert_eq!(layout.ruby.len(), 1);
        assert_f32_eq(layout.ruby[0].base_bounds.y, config.origin.y);
        assert_f32_eq(layout.glyphs[1].origin.y, config.origin.y);
        assert!(
            layout.ruby[0].base_bounds.bottom() <= config.origin.y + config.size.height,
            "expanded {writing_mode:?} ruby base should fit inside the column after feedback"
        );
    }
}

#[test]
fn vertical_ruby_multi_cluster_base_breaks_before_the_base_start() {
    for (writing_mode, next_column_moves_right) in [
        (RichTextWritingMode::VerticalRl, false),
        (RichTextWritingMode::VerticalLr, true),
    ] {
        let mut frame = frame_with_run("天夢星", vertical_presentation(writing_mode));
        push_ruby(&mut frame, "天".len(), "天夢星".len(), "ゆめ");
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 84.0),
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.glyphs.len(), 3);
        assert_eq!(layout.glyphs[1].text, "夢");
        assert_eq!(layout.glyphs[2].text, "星");
        assert_vertical_layout_column_restart(
            &layout.glyphs[0],
            &layout.glyphs[1],
            next_column_moves_right,
            "multi-cluster ruby base should move as a unit before it is split by overflow",
        );
        assert_f32_eq(layout.glyphs[1].origin.y, config.origin.y);
        assert_f32_eq(layout.glyphs[2].origin.x, layout.glyphs[1].origin.x);
        assert!(layout.glyphs[2].origin.y > layout.glyphs[1].origin.y);
        assert_eq!(layout.ruby.len(), 1);
        assert_eq!(
            layout.ruby[0].base_range,
            RichTextRange::new("天".len(), "天夢星".len())
        );
        assert_f32_eq(layout.ruby[0].base_bounds.x, layout.glyphs[1].bounds.x);
    }
}

#[test]
fn overheight_vertical_ruby_splits_into_column_segments() {
    for (writing_mode, continuation_moves_right) in [
        (RichTextWritingMode::VerticalRl, true),
        (RichTextWritingMode::VerticalLr, false),
    ] {
        let mut frame = frame_with_run("夢", vertical_presentation(writing_mode));
        push_ruby(&mut frame, 0, "夢".len(), "あいうえお");
        let config = TextLayoutConfig {
            size: LayoutSize::new(160.0, 42.0),
            ruby_font_size: 14.0,
            ..TextLayoutConfig::default()
        };
        let layout = layout_frame(&frame, config).expect("layout succeeds");

        assert_eq!(layout.ruby.len(), 2);
        assert_eq!(layout.ruby[0].writing_mode, writing_mode);
        assert_eq!(layout.ruby[0].ruby_index, layout.ruby[1].ruby_index);
        assert_eq!(layout.ruby[0].ruby, "あいう");
        assert_eq!(layout.ruby[1].ruby, "えお");
        assert!(layout.ruby[0].ruby_bounds.height <= config.size.height);
        assert!(layout.ruby[1].ruby_bounds.height <= config.size.height);
        if continuation_moves_right {
            assert!(layout.ruby[1].ruby_bounds.x > layout.ruby[0].ruby_bounds.x);
        } else {
            assert!(layout.ruby[1].ruby_bounds.x < layout.ruby[0].ruby_bounds.x);
        }
        assert_f32_eq(layout.ruby[0].ruby_bounds.y, config.origin.y);
        assert_f32_eq(layout.ruby[1].ruby_bounds.y, config.origin.y);
    }
}
