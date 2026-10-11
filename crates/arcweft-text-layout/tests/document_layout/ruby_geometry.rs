use super::*;
use arcweft_text_model::{Milli, PreparedTextDocumentContext};

fn ruby_document(
    text: &'static str,
    annotation: &'static str,
    mode: RichTextWritingMode,
    overhang: Option<Milli>,
    gap: Option<Milli>,
) -> ResolvedTextDocument<'static> {
    let base_style = style().with_flow(mode, RichTextInlineDirection::Auto);
    let base = RichTextRange::new("天地".len(), "天地夢".len());
    let presentation = RichTextPresentation {
        layout: Some(RichTextLayout {
            writing_mode: mode,
            ruby_overhang: overhang,
            ruby_collision_gap: gap,
            ..Default::default()
        }),
        ..Default::default()
    };
    let ruby = ResolvedTextRuby::new(
        RichTextNodeIndex::new(0),
        RichTextNodeRange::new(RichTextNodeIndex::new(1), RichTextNodeIndex::new(2)),
        RichTextTextRunRange::new(1, 2),
        base,
        base,
        annotation,
        base_style.clone(),
        presentation.clone(),
    )
    .unwrap();
    document_with_presentation(text, base_style, presentation, vec![ruby], 80)
}

#[test]
fn long_vertical_annotation_allocates_before_breaking_without_stretching_body_cells() {
    for mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let document = ruby_document("天地夢人外", "ながいながいよみ", mode, None, None);
        let layout = layout_document(&document, request(), &mut MockShaper::new(b"font")).unwrap();
        let annotation = &layout.ruby[0];
        let base = layout
            .glyphs
            .iter()
            .find(|glyph| glyph.source_range == annotation.base_range)
            .unwrap();
        assert_eq!(base.layout_bounds.width, 20.0);
        assert_eq!(base.layout_bounds.height, 20.0);
        assert_eq!(base.advance, LayoutSize::new(0.0, 20.0));
        assert!(annotation.base_bounds.height > base.layout_bounds.height * 2.0);
        assert!(annotation.base_bounds.y < base.layout_bounds.y);
        assert!(annotation.ruby_bounds.height >= annotation.base_bounds.height);
        assert_eq!(annotation.base_allocations.len(), 1);
        let allocation = &annotation.base_allocations[0];
        assert_eq!(allocation.bounds, annotation.base_bounds);
        assert!(allocation.bounds.y <= base.layout_bounds.y);
        assert!(allocation.bounds.bottom() >= base.layout_bounds.bottom());
        assert!(
            layout
                .lines
                .iter()
                .all(|line| line.bounds.height <= request().size.height)
        );
        let context = PreparedTextDocumentContext::try_new(0, document.text().len()).unwrap();
        layout
            .validate_text_context(context.ranges(), document.text())
            .unwrap();
        assert_eq!(
            layout.source_map.glyphs()[usize::try_from(allocation.glyph_range.start).unwrap()]
                .source_range,
            annotation.base_range
        );
        let second = layout_document(&document, request(), &mut MockShaper::new(b"font")).unwrap();
        assert_eq!(
            layout, second,
            "measured placement and identities remain deterministic"
        );
    }
}

#[test]
fn vertical_zero_overhang_changes_owned_base_extent_without_changing_physical_font_cells() {
    let automatic = ruby_document(
        "天地夢人外",
        "ながいながいよみ",
        RichTextWritingMode::VerticalRl,
        None,
        None,
    );
    let zero = ruby_document(
        "天地夢人外",
        "ながいながいよみ",
        RichTextWritingMode::VerticalRl,
        Some(Milli(0)),
        None,
    );
    let automatic = layout_document(&automatic, request(), &mut MockShaper::new(b"font")).unwrap();
    let zero = layout_document(&zero, request(), &mut MockShaper::new(b"font")).unwrap();
    assert!(automatic.ruby[0].base_bounds.height < zero.ruby[0].base_bounds.height);
    assert_eq!(
        zero.ruby[0].base_bounds.height,
        zero.ruby[0].ruby_bounds.height
    );
    let physical = |layout: &TextLayout| {
        layout
            .glyphs
            .iter()
            .map(|glyph| {
                (
                    glyph.source_range,
                    glyph.layout_bounds.width,
                    glyph.layout_bounds.height,
                    glyph.advance,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(physical(&automatic), physical(&zero));
    assert_ne!(automatic.hash, zero.hash);
}

#[test]
fn overheight_vertical_annotation_uses_measured_continuation_tracks_and_owned_collision_gap() {
    for mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let document = ruby_document(
            "天地夢人外",
            "あいうえおかきくけこさしすせそたちつてと",
            mode,
            None,
            Some(Milli(2_000)),
        );
        let request = TextLayoutRequest {
            size: LayoutSize::new(180.0, 60.0),
            ..request()
        };
        let layout = layout_document(&document, request, &mut MockShaper::new(b"font")).unwrap();
        let annotation = &layout.ruby[0];
        assert_eq!(annotation.glyphs.len(), 20);
        assert!(annotation.ruby_bounds.width > annotation.glyphs[0].layout_bounds.width);
        let mut tracks = annotation
            .glyphs
            .iter()
            .map(|glyph| glyph.layout_bounds.x)
            .collect::<Vec<_>>();
        tracks.sort_by(f32::total_cmp);
        tracks.dedup();
        assert_eq!(tracks.len(), 4);
        for pair in tracks.windows(2) {
            assert_eq!(pair[1] - pair[0], 12.0);
        }
        for glyph in &annotation.glyphs {
            assert!(glyph.layout_bounds.y >= request.origin.y);
            assert!(glyph.layout_bounds.bottom() <= request.origin.y + request.size.height);
            assert!(glyph.ink_bounds.y >= request.origin.y);
            assert!(glyph.ink_bounds.bottom() <= request.origin.y + request.size.height);
        }
        let indices = annotation
            .glyphs
            .iter()
            .map(|glyph| glyph.cluster_index)
            .collect::<Vec<_>>();
        assert_eq!(indices, (0..20).collect::<Vec<_>>());
        assert!(annotation.ruby_bounds.x >= request.origin.x);
        assert!(annotation.ruby_bounds.right() <= request.origin.x + request.size.width);
        let context = PreparedTextDocumentContext::try_new(0, document.text().len()).unwrap();
        layout
            .validate_text_context(context.ranges(), document.text())
            .unwrap();
    }
}

#[test]
fn ruby_allocation_codec_rejects_missing_duplicate_foreign_and_mismatched_body_partitions() {
    let document = ruby_document(
        "天地夢人外",
        "ながいながいよみ",
        RichTextWritingMode::VerticalRl,
        None,
        None,
    );
    let layout = layout_document(&document, request(), &mut MockShaper::new(b"font")).unwrap();
    let context = PreparedTextDocumentContext::try_new(0, document.text().len()).unwrap();
    let bytes = serde_json::to_vec(&layout).unwrap();
    let decoded: TextLayout = serde_json::from_slice(&bytes).unwrap();
    decoded
        .validate_text_context(context.ranges(), document.text())
        .unwrap();
    assert_eq!(decoded, layout);
    let mut wire = serde_json::to_value(&layout).unwrap();
    wire["ruby"][0]
        .as_object_mut()
        .unwrap()
        .remove("base_allocations");
    assert!(serde_json::from_value::<TextLayout>(wire).is_err());
    for defect in 0..4 {
        let mut changed = layout.clone();
        match defect {
            0 => changed.ruby[0].base_allocations.clear(),
            1 => {
                let duplicate = changed.ruby[0].base_allocations[0].clone();
                changed.ruby[0].base_allocations.push(duplicate);
            }
            2 => changed.ruby[0].base_allocations[0].glyph_range = 0..1,
            3 => changed.ruby[0].base_allocations[0].bounds.height = 0.0,
            _ => unreachable!(),
        }
        assert_eq!(
            changed.validate_text_context(context.ranges(), document.text()),
            Err(arcweft_text_layout::TextLayoutContextError::RubyAllocation { index: 0 })
        );
    }
}

struct CountingShaper {
    inner: MockShaper,
    calls: usize,
}
impl TextShaper for CountingShaper {
    type Error = MockShapeError;
    fn font_inventory_hash(&self) -> FontInventoryHash {
        self.inner.font_inventory_hash()
    }
    fn shape_run(&mut self, request: TextShapeRequest<'_>) -> Result<ShapedTextRun, Self::Error> {
        self.calls += 1;
        self.inner.shape_run(request)
    }
}

#[test]
fn measured_ruby_finalization_does_not_shape_body_or_annotation_twice() {
    let document = ruby_document(
        "天地夢人外",
        "ながいながいよみ",
        RichTextWritingMode::VerticalRl,
        None,
        None,
    );
    let mut shaper = CountingShaper {
        inner: MockShaper::new(b"font"),
        calls: 0,
    };
    let layout = layout_document(&document, request(), &mut shaper).unwrap();
    assert_eq!(shaper.calls, document.runs().len() + document.ruby().len());
    assert_eq!(layout.glyphs.len(), 5);
    assert_eq!(layout.ruby[0].glyphs.len(), 8);
}

#[test]
fn ruby_base_run_mode_and_source_origin_remain_exact_across_unannotated_runs() {
    let text = "a夢b";
    let source_origin = 91;
    for mode in [
        RichTextWritingMode::HorizontalTb,
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let base_style = style().with_flow(mode, RichTextInlineDirection::Auto);
        let foreign_mode = if mode == RichTextWritingMode::HorizontalTb {
            RichTextWritingMode::VerticalRl
        } else {
            RichTextWritingMode::HorizontalTb
        };
        let foreign_style = style().with_flow(foreign_mode, RichTextInlineDirection::Auto);
        let base_range = RichTextRange::new(1, 1 + "夢".len());
        let source_base = RichTextRange::new(
            source_origin + base_range.start,
            source_origin + base_range.end,
        );
        let runs = [
            (RichTextRange::new(0, 1), foreign_style.clone()),
            (base_range, base_style),
            (
                RichTextRange::new(base_range.end, text.len()),
                foreign_style.clone(),
            ),
        ]
        .into_iter()
        .map(|(range, style)| {
            ResolvedTextRun::new(
                range,
                RichTextRange::new(source_origin + range.start, source_origin + range.end),
                style,
                RichTextPresentation::default(),
                ResolvedTextRunSource::Plain,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
        let annotation = ResolvedTextRuby::new(
            RichTextNodeIndex::new(0),
            RichTextNodeRange::new(RichTextNodeIndex::new(1), RichTextNodeIndex::new(2)),
            RichTextTextRunRange::new(1, 2),
            base_range,
            source_base,
            "ながいよみ",
            foreign_style,
            RichTextPresentation::default(),
        )
        .unwrap();
        let document = ResolvedTextDocument::new(
            text,
            source_origin,
            runs,
            vec![annotation],
            TextDocumentRevision::new(92),
        )
        .unwrap();
        let layout = layout_document(&document, request(), &mut MockShaper::new(b"font")).unwrap();
        assert_eq!(layout.ruby.len(), 1);
        let annotation = &layout.ruby[0];
        assert_eq!(annotation.writing_mode, mode);
        assert_eq!(annotation.base_range, source_base);
        assert_eq!(annotation.base_runs, RichTextTextRunRange::new(1, 2));
        assert_eq!(annotation.base_allocations.len(), 1);
        for index in annotation.base_allocations[0].glyph_range.clone() {
            let glyph = &layout.glyphs[usize::try_from(index).unwrap()];
            assert_eq!(glyph.run_index, 1);
            assert_eq!(glyph.source_range, source_base);
        }
        for run_index in [0_usize, 2] {
            let run = &layout.runs[run_index];
            let physical = layout.glyphs[usize::try_from(run.glyph_range.start).unwrap()
                ..usize::try_from(run.glyph_range.end).unwrap()]
                .iter()
                .map(|glyph| glyph.layout_bounds.union(glyph.ink_bounds))
                .reduce(LayoutRect::union)
                .unwrap();
            assert_eq!(
                run.bounds, physical,
                "unannotated runs retain their physical geometry"
            );
        }
        let context = PreparedTextDocumentContext::try_new(source_origin, text.len()).unwrap();
        layout
            .validate_text_context(context.ranges(), text)
            .unwrap();
    }
}

#[test]
fn unannotated_body_runs_and_lines_keep_physical_geometry_without_ruby_metadata() {
    for mode in [
        RichTextWritingMode::HorizontalTb,
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let document = document(
            "天地人",
            style().with_flow(mode, RichTextInlineDirection::Auto),
            93,
        );
        let layout = layout_document(&document, request(), &mut MockShaper::new(b"font")).unwrap();
        assert!(layout.ruby.is_empty());
        for (interval, bounds) in layout
            .runs
            .iter()
            .map(|run| (&run.glyph_range, run.bounds))
            .chain(
                layout
                    .lines
                    .iter()
                    .map(|line| (&line.glyph_range, line.bounds)),
            )
        {
            let physical = layout.glyphs
                [usize::try_from(interval.start).unwrap()..usize::try_from(interval.end).unwrap()]
                .iter()
                .map(|glyph| glyph.layout_bounds.union(glyph.ink_bounds))
                .reduce(LayoutRect::union)
                .unwrap();
            assert_eq!(bounds, physical);
        }
        let context = PreparedTextDocumentContext::try_new(0, document.text().len()).unwrap();
        layout
            .validate_text_context(context.ranges(), document.text())
            .unwrap();
    }
}
