use std::collections::BTreeSet;

use arcweft_glyphon::{
    GlyphonTextEngine, PreparedTextBatch, PreparedTextBoundsEdge, PreparedTextError,
    PreparedTextPhysicalBoundsError,
};
use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextRuby, ResolvedTextRun, ResolvedTextRunSource,
    ResolvedTextStyle, RichTextInlineDirection, RichTextPresentation, RichTextRange,
    RichTextWritingMode, TextDocumentRevision, TextFontFamily,
};
use arcweft_text_layout::{
    GlyphOrientation, LayoutPoint, LayoutRect, LayoutSize, TextLayoutRequest, layout_document,
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
    let range = RichTextRange::new(0, text.len());
    let run = ResolvedTextRun::new(
        range,
        range,
        style,
        RichTextPresentation::default(),
        ResolvedTextRunSource::Plain,
    )
    .expect("test run is valid");
    ResolvedTextDocument::new(
        text,
        0,
        vec![run],
        ruby,
        TextDocumentRevision::new(revision),
    )
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
