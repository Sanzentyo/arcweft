use std::collections::BTreeSet;

use arcweft_glyphon::{
    GlyphonTextEngine, PreparedTextBatch, PreparedTextBoundsEdge, PreparedTextError,
    PreparedTextPhysicalBoundsError,
};
use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextRuby, ResolvedTextRun, ResolvedTextRunSource,
    ResolvedTextStyle, TextDocumentRevision, TextFontFamily,
};
use arcweft_text_layout::{
    GlyphOrientation, LayoutPoint, LayoutRect, LayoutSize, TextLayoutRequest, layout_document,
};
use arcweft_text_model::{
    RichTextInlineDirection, RichTextNodeIndex, RichTextNodeRange, RichTextPresentation,
    RichTextRange, RichTextTextRunRange, RichTextWritingMode,
};

const JAPANESE_FONT: &[u8] = include_bytes!("../../../web/assets/noto-sans-jp-vf.ttf");
const EMOJI_FONT: &[u8] = include_bytes!("../../../web/assets/noto-emoji-regular.ttf");
const LATIN_FONT: &[u8] = include_bytes!("../../../web/assets/arcweft-demo.ttf");

fn engine() -> GlyphonTextEngine {
    GlyphonTextEngine::from_project_fonts(
        "ja",
        vec![
            JAPANESE_FONT.to_vec(),
            EMOJI_FONT.to_vec(),
            LATIN_FONT.to_vec(),
        ],
    )
    .expect("bundled project fonts load")
}

fn horizontal_style() -> ResolvedTextStyle {
    ResolvedTextStyle::new(vec![TextFontFamily::SansSerif], 24_000, 32_000)
        .expect("test style is valid")
}

fn one_run_document(
    text: &str,
    style: ResolvedTextStyle,
    ruby: Vec<ResolvedTextRuby>,
    revision: u64,
) -> ResolvedTextDocument<'_> {
    let mut boundaries = vec![0, text.len()];
    boundaries.extend(
        ruby.iter()
            .flat_map(|annotation| [annotation.base_range().start, annotation.base_range().end]),
    );
    boundaries.sort_unstable();
    boundaries.dedup();
    let runs = boundaries
        .windows(2)
        .filter(|window| window[0] < window[1])
        .map(|window| {
            let range = RichTextRange::new(window[0], window[1]);
            ResolvedTextRun::new(
                range,
                range,
                style.clone(),
                RichTextPresentation::default(),
                ResolvedTextRunSource::Plain,
            )
            .expect("test run is valid")
        })
        .collect();
    ResolvedTextDocument::new(text, 0, runs, ruby, TextDocumentRevision::new(revision))
        .expect("test document is valid")
}

fn request(width: f32, height: f32) -> TextLayoutRequest {
    TextLayoutRequest {
        origin: LayoutPoint::default(),
        size: LayoutSize::new(width, height),
        ..TextLayoutRequest::default()
    }
}

#[test]
fn exact_project_fonts_shape_ligature_combining_cjk_emoji_with_stable_hash() {
    let text = "office e\u{301} 日本語 😀";
    let document = one_run_document(text, horizontal_style(), Vec::new(), 1);
    let mut first_engine = engine();
    let project_faces = first_engine.ordered_faces().to_vec();
    let first = layout_document(&document, request(600.0, 120.0), &mut first_engine)
        .expect("mixed text lays out");
    let second = layout_document(&document, request(600.0, 120.0), &mut engine())
        .expect("same mixed text lays out again");

    assert_eq!(first.hash, second.hash);
    assert_eq!(first.font_inventory, second.font_inventory);
    assert!(
        first
            .glyphs
            .iter()
            .any(|glyph| glyph.source_range.end - glyph.source_range.start > 1)
    );
    assert!(
        first
            .glyphs
            .iter()
            .any(|glyph| glyph.ink_bounds.width > 0.0 && glyph.ink_bounds.height > 0.0)
    );
    let used_faces = first
        .glyphs
        .iter()
        .map(|glyph| glyph.shape_key.face)
        .collect::<BTreeSet<_>>();
    assert!(used_faces.len() >= 2, "emoji must use project fallback");
    assert!(used_faces.iter().all(|face| project_faces.contains(face)));
}

#[test]
fn explicit_rtl_uses_bidi_visual_order_but_keeps_logical_ordinals() {
    let style = horizontal_style().with_flow(
        RichTextWritingMode::HorizontalTb,
        RichTextInlineDirection::Rtl,
    );
    let document = one_run_document("123 - 456", style, Vec::new(), 2);
    let layout = layout_document(&document, request(300.0, 100.0), &mut engine())
        .expect("explicit RTL lays out");
    let source_starts = layout
        .glyphs
        .iter()
        .map(|glyph| u32::try_from(glyph.source_range.start).expect("test source fits u32"))
        .collect::<Vec<_>>();
    let logical_ordinals = layout
        .glyphs
        .iter()
        .map(|glyph| glyph.logical_ordinal)
        .collect::<Vec<_>>();

    assert!(
        source_starts.windows(2).any(|pair| pair[0] > pair[1]),
        "explicit RTL visual order stayed logical: {source_starts:?}"
    );
    assert_eq!(source_starts, logical_ordinals);
}

#[test]
fn hard_break_uses_distinct_visual_lines_with_real_baselines() {
    let document = one_run_document("im\n日本", horizontal_style(), Vec::new(), 3);
    let layout = layout_document(&document, request(300.0, 120.0), &mut engine())
        .expect("hard-break text lays out");

    assert_eq!(layout.lines.len(), 2);
    assert!(layout.lines[1].bounds.y > layout.lines[0].bounds.y);
    assert!(layout.glyphs.iter().any(|glyph| glyph.line_index == 1));
}

#[test]
fn vertical_text_combine_sideways_latin_and_ruby_share_shaped_geometry() {
    let text = "日本2026ABC";
    let style = ResolvedTextStyle::new(vec![TextFontFamily::SansSerif], 30_000, 42_000)
        .expect("test style is valid")
        .with_flow(
            RichTextWritingMode::VerticalRl,
            RichTextInlineDirection::Auto,
        );
    let base = RichTextRange::new(0, "日本".len());
    let ruby = ResolvedTextRuby::new(
        RichTextNodeIndex::new(0),
        RichTextNodeRange::new(RichTextNodeIndex::new(1), RichTextNodeIndex::new(2)),
        RichTextTextRunRange::new(0, 1),
        base,
        base,
        "にほん",
        style.clone(),
        RichTextPresentation::default(),
    )
    .expect("ruby is valid");
    let document = one_run_document(text, style, vec![ruby], 4);
    let layout = layout_document(&document, request(180.0, 100.0), &mut engine())
        .expect("vertical ruby text lays out");

    assert!(
        layout.lines.len() >= 2,
        "content must wrap to another column"
    );
    assert!(
        layout
            .glyphs
            .iter()
            .any(|glyph| glyph.orientation == GlyphOrientation::TextCombineUpright)
    );
    assert!(
        layout
            .glyphs
            .iter()
            .any(|glyph| glyph.orientation == GlyphOrientation::SidewaysCw)
    );
    assert!(layout.glyphs.iter().any(|glyph| glyph.inline_scale < 1.0));
    assert_eq!(layout.ruby.len(), 1);
    assert!(!layout.ruby[0].glyphs.is_empty());
    assert!(
        layout.ruby[0].ruby_bounds.x > layout.ruby[0].base_bounds.x,
        "vertical_rl ruby must occupy the reserved physical-right track"
    );
    assert!(
        layout.ruby[0]
            .glyphs
            .iter()
            .all(|glyph| glyph.ink_bounds.width.is_finite() && glyph.ink_bounds.height.is_finite())
    );
    assert!(
        layout
            .bounds
            .is_some_and(|bounds| bounds.intersects(layout.ruby[0].ruby_bounds))
    );
}

#[test]
fn prepared_batch_reuses_layout_for_paint_and_interaction() {
    let document = one_run_document("office 日本", horizontal_style(), Vec::new(), 5);
    let mut engine = engine();
    let layout =
        layout_document(&document, request(400.0, 100.0), &mut engine).expect("text lays out");
    let layout_hash = layout.hash;
    let mut item = engine
        .prepare_layout(layout, None, None, 2.0)
        .expect("layout prepares");

    assert_eq!(item.glyphs.len(), item.paint.glyphs.len());
    assert!(!item.interaction.character_bounds.is_empty());
    let original_count = item.submission().glyphs().len();
    item.paint.glyphs[0].visible = false;
    let submission = item.submission();
    assert_eq!(submission.glyphs().len() + 1, original_count);
    assert_eq!(item.layout.hash, layout_hash);
    let first_font_size = match submission.glyphs()[0].source {
        glyphon::GlyphSource::Text { cache_key } => f32::from_bits(cache_key.font_size_bits),
        glyphon::GlyphSource::Custom { .. } => panic!("text preparation emitted custom glyph"),
    };
    assert!((first_font_size - 48.0).abs() < f32::EPSILON);

    let mut batch = PreparedTextBatch::default();
    let id = batch.push(item).expect("batch index fits");
    assert_eq!(id.index(), 0);
    assert_eq!(
        batch.get(id).map(|item| item.layout.hash),
        Some(layout_hash)
    );
}

#[test]
fn prepared_item_rejects_finite_clip_scale_overflow_with_numeric_context() {
    let document = one_run_document("A", horizontal_style(), Vec::new(), 6);
    let mut engine = engine();
    let layout =
        layout_document(&document, request(100.0, 100.0), &mut engine).expect("text lays out");

    let error = engine
        .prepare_layout(
            layout,
            None,
            Some(LayoutRect::new(f32::MAX, 0.0, 0.0, 0.0)),
            2.0,
        )
        .expect_err("finite operands whose product overflows must fail preparation");

    assert_eq!(
        error,
        PreparedTextError::PhysicalClipBounds(
            PreparedTextPhysicalBoundsError::PhysicalScaleOverflow {
                edge: PreparedTextBoundsEdge::Left,
                logical: f32::MAX,
                raster_scale: 2.0,
            }
        )
    );
}

fn measured_vertical_ruby_item(
    mode: RichTextWritingMode,
    annotation: &str,
    height: f32,
) -> arcweft_glyphon::PreparedTextItem {
    let text = "天地夢人外";
    let style = ResolvedTextStyle::new(vec![TextFontFamily::SansSerif], 25_000, 34_000)
        .unwrap()
        .with_flow(mode, RichTextInlineDirection::Auto);
    let base = RichTextRange::new("天地".len(), "天地夢".len());
    let ruby = ResolvedTextRuby::new(
        RichTextNodeIndex::new(0),
        RichTextNodeRange::new(RichTextNodeIndex::new(1), RichTextNodeIndex::new(2)),
        RichTextTextRunRange::new(1, 2),
        base,
        base,
        annotation,
        style.clone(),
        RichTextPresentation::default(),
    )
    .unwrap();
    let document = one_run_document(text, style, vec![ruby], 90);
    let mut engine = engine();
    let layout = layout_document(
        &document,
        TextLayoutRequest {
            origin: LayoutPoint::new(10.0, 20.0),
            size: LayoutSize::new(220.0, height),
            ..TextLayoutRequest::default()
        },
        &mut engine,
    )
    .unwrap();
    engine.prepare_layout(layout, None, None, 1.0).unwrap()
}

fn assert_measured_rect(actual: LayoutRect, expected: LayoutRect) {
    for (actual, expected) in [actual.x, actual.y, actual.width, actual.height]
        .into_iter()
        .zip([expected.x, expected.y, expected.width, expected.height])
    {
        // The owning paint/submission projection uses f32 fused operations.
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }
}

#[test]
fn measured_ruby_allocation_follows_the_same_real_glyph_paint_and_view_affine() {
    use arcweft_glyphon::{PreparedTextAffine, TextGlyphTransform};
    use arcweft_presentation::fx::{Length, Transform2D};

    for mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let mut item = measured_vertical_ruby_item(mode, "ながいながいよみ", 125.6);
        let annotation = &item.layout.ruby[0];
        assert_eq!(annotation.style.font_size_milli(), 12_500);
        let allocation = &annotation.base_allocations[0];
        let index = usize::try_from(allocation.glyph_range.start).unwrap();
        assert_eq!(allocation.glyph_range.end - allocation.glyph_range.start, 1);
        let physical = &item.layout.glyphs[index];
        assert_eq!(physical.shape_key.font_size_bits, 25.0_f32.to_bits());
        assert_eq!(physical.layout_bounds.height, 25.0);
        assert_eq!(physical.advance.height, 25.0);
        assert!(annotation.base_bounds.height > physical.layout_bounds.height * 2.0);
        let original = annotation.base_bounds;
        let raster_origin = item.glyphs[index].origin;
        let hash = item.layout.hash;
        item.paint.glyphs[index].transform = TextGlyphTransform::new(
            Transform2D {
                translate_x: Length::try_pixels(4.0).unwrap(),
                translate_y: Length::try_pixels(17.0).unwrap(),
                ..Transform2D::default()
            }
            .resolve()
            .unwrap(),
        );
        let view = PreparedTextAffine::try_new([1.0, 0.0, 0.25, 1.0, 11.0, 13.0], 1.0).unwrap();
        let shifted = LayoutRect::new(
            original.x + 4.0,
            original.y + 17.0,
            original.width,
            original.height,
        );
        let expected = LayoutRect::new(
            shifted.x + shifted.y * 0.25 + 11.0,
            shifted.y + 13.0,
            shifted.width + shifted.height * 0.25,
            shifted.height,
        );
        let geometry = item.painted_geometry([view]).unwrap();
        assert_measured_rect(geometry.ruby(0).unwrap().base_bounds, expected);
        let submission = item.submission_with_affine(view);
        let submitted = &submission.glyphs()[index];
        let expected_origin = LayoutPoint::new(
            raster_origin.x + 4.0 + (raster_origin.y + 17.0) * 0.25 + 11.0,
            raster_origin.y + 17.0 + 13.0,
        );
        let expected_raster_origin = LayoutPoint::new(
            raster_origin.x + raster_origin.y * 0.25 + 11.0,
            raster_origin.y + 13.0,
        );
        assert!((submitted.origin.x - expected_raster_origin.x).abs() < 0.001);
        assert!((submitted.origin.y - expected_raster_origin.y).abs() < 0.001);
        let glyphon::GlyphTransform::Affine(local_transform) = submitted.transform else {
            panic!("upright body glyph must retain the composed paint and View affine");
        };
        assert_eq!(&local_transform.values[..4], &[1.0, 0.25, 0.0, 1.0]);
        // The renderer adds the glyph-local affine translation to the raster
        // origin. Check that physical painted position, not just its anchor.
        let painted_origin = LayoutPoint::new(
            submitted.origin.x + local_transform.values[4],
            submitted.origin.y + local_transform.values[5],
        );
        assert!((painted_origin.x - expected_origin.x).abs() < 0.001);
        assert!((painted_origin.y - expected_origin.y).abs() < 0.001);
        assert_eq!(
            item.layout.hash, hash,
            "paint does not rerun shaping or mutate allocated layout"
        );
        let shifted_only = item
            .painted_geometry([PreparedTextAffine::IDENTITY])
            .unwrap()
            .ruby(0)
            .unwrap()
            .base_bounds;
        let both = item
            .painted_geometry([PreparedTextAffine::IDENTITY, view])
            .unwrap();
        assert_measured_rect(
            both.ruby(0).unwrap().base_bounds,
            shifted_only.union(expected),
        );
    }
}

#[test]
fn real_overheight_ruby_tracks_are_complete_and_fit_the_same_issued_container() {
    for mode in [
        RichTextWritingMode::VerticalRl,
        RichTextWritingMode::VerticalLr,
    ] {
        let item =
            measured_vertical_ruby_item(mode, "あいうえおかきくけこさしすせそたちつてと", 125.6);
        let annotation = &item.layout.ruby[0];
        assert_eq!(annotation.glyphs.len(), 20);
        assert_eq!(annotation.style.font_size_milli(), 12_500);
        let mut tracks = annotation
            .glyphs
            .iter()
            .map(|glyph| glyph.layout_bounds.x)
            .collect::<Vec<_>>();
        tracks.sort_by(f32::total_cmp);
        tracks.dedup();
        assert_eq!(tracks.len(), 2);
        assert!(annotation.ruby_bounds.width > 24.0);
        for glyph in &annotation.glyphs {
            assert!(glyph.layout_bounds.y >= 20.0);
            assert!(glyph.layout_bounds.bottom() <= 20.0 + 125.6);
            assert!(glyph.ink_bounds.y >= 20.0);
            assert!(glyph.ink_bounds.bottom() <= 20.0 + 125.6);
        }
        let geometry = item
            .painted_geometry([arcweft_glyphon::PreparedTextAffine::IDENTITY])
            .unwrap();
        assert_measured_rect(
            geometry.ruby(0).unwrap().base_bounds,
            annotation.base_bounds,
        );
        assert_measured_rect(
            geometry.ruby(0).unwrap().annotation_bounds,
            annotation.ruby_bounds,
        );
        assert_eq!(
            item.submission().glyphs().len(),
            item.layout.glyphs.len() + annotation.glyphs.len()
        );
    }
}
