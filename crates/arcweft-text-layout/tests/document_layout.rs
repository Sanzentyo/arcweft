use std::{error::Error, fmt};

use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextRun, ResolvedTextRunSource, ResolvedTextStyle,
    RichTextInlineDirection, RichTextPresentation, RichTextRange, TextColor, TextDocumentRevision,
    TextFontFamily,
};
use arcweft_text_layout::{
    FontFaceId, FontInventoryHash, LayoutPoint, LayoutRect, LayoutSize, ShapedGlyphKey,
    ShapedTextGlyph, ShapedTextRun, TextLayoutError, TextLayoutRequest, TextShapeRequest,
    TextShaper, layout_document,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MockShapeError;

impl fmt::Display for MockShapeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("mock shaping failed")
    }
}

impl Error for MockShapeError {}

struct MockShaper {
    face: FontFaceId,
    inventory: FontInventoryHash,
    reverse_visual_order: bool,
    invalid_range: bool,
}

impl MockShaper {
    fn new(font_bytes: &[u8]) -> Self {
        let face = FontFaceId::derive(font_bytes, 0, &[]);
        Self {
            face,
            inventory: FontInventoryHash::derive([face], std::iter::empty()),
            reverse_visual_order: false,
            invalid_range: false,
        }
    }

    fn reverse_visual_order(mut self) -> Self {
        self.reverse_visual_order = true;
        self
    }

    fn invalid_range(mut self) -> Self {
        self.invalid_range = true;
        self
    }
}

impl TextShaper for MockShaper {
    type Error = MockShapeError;

    fn font_inventory_hash(&self) -> FontInventoryHash {
        self.inventory
    }

    fn shape_run(&mut self, request: TextShapeRequest<'_>) -> Result<ShapedTextRun, Self::Error> {
        let mut glyphs = request
            .text
            .char_indices()
            .enumerate()
            .map(|(logical_index, (start, character))| {
                let end = start + character.len_utf8();
                let width = match character {
                    'i' => 3.0,
                    'm' => 9.0,
                    _ => 6.0,
                };
                ShapedTextGlyph {
                    key: ShapedGlyphKey {
                        face: self.face,
                        glyph_id: u32::from(character),
                        font_size_bits: 20.0_f32.to_bits(),
                        font_weight: 400,
                        flags: 0,
                    },
                    source_range: RichTextRange::new(
                        request.source_range.start + start,
                        request.source_range.start + end,
                    ),
                    line_index: 0,
                    cluster_index: u32::try_from(logical_index).unwrap_or(u32::MAX),
                    offset: LayoutPoint::default(),
                    advance: LayoutSize::new(width, 0.0),
                    ink_bounds: LayoutRect::new(0.0, 0.0, width, 10.0),
                }
            })
            .collect::<Vec<_>>();
        if self.reverse_visual_order {
            glyphs.reverse();
            for (visual_index, glyph) in glyphs.iter_mut().enumerate() {
                glyph.cluster_index = u32::try_from(visual_index).unwrap_or(u32::MAX);
            }
        }
        if self.invalid_range
            && let Some(first) = glyphs.first_mut()
        {
            first.source_range.end = request.source_range.end.saturating_add(1);
        }
        let width = glyphs.iter().map(|glyph| glyph.advance.width).sum();
        Ok(ShapedTextRun::new(
            glyphs,
            LayoutSize::new(width, 0.0),
            Some(LayoutRect::new(0.0, 0.0, width, 10.0)),
        ))
    }
}

fn document(text: &str, style: ResolvedTextStyle, revision: u64) -> ResolvedTextDocument<'_> {
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
        Vec::new(),
        TextDocumentRevision::new(revision),
    )
    .expect("test document is valid")
}

fn style() -> ResolvedTextStyle {
    ResolvedTextStyle::new(vec![TextFontFamily::SansSerif], 20_000, 27_500)
        .expect("test style is valid")
}

fn request() -> TextLayoutRequest {
    TextLayoutRequest {
        origin: LayoutPoint::default(),
        size: LayoutSize::new(100.0, 100.0),
        ..TextLayoutRequest::default()
    }
}

#[test]
fn proportional_shaped_advances_drive_placement_and_one_visual_line() {
    let document = document("im", style(), 1);
    let layout = layout_document(&document, request(), &mut MockShaper::new(b"font"))
        .expect("layout succeeds");

    assert_eq!(layout.glyphs.len(), 2);
    assert!(layout.glyphs[0].origin.x.abs() < f32::EPSILON);
    assert!((layout.glyphs[1].origin.x - 3.0).abs() < f32::EPSILON);
    assert!((layout.glyphs[1].advance.width - 9.0).abs() < f32::EPSILON);
    assert_eq!(layout.lines.len(), 1);
    assert_eq!(layout.lines[0].glyph_range, 0..2);
}

#[test]
fn logical_ordinal_is_independent_from_rtl_visual_order() {
    let rtl = style().with_flow(
        arcweft_render_text::RichTextWritingMode::HorizontalTb,
        RichTextInlineDirection::Rtl,
    );
    let document = document("abc", rtl, 2);
    let layout = layout_document(
        &document,
        request(),
        &mut MockShaper::new(b"font").reverse_visual_order(),
    )
    .expect("layout succeeds");

    assert_eq!(
        layout
            .glyphs
            .iter()
            .map(|glyph| glyph.source_range)
            .collect::<Vec<_>>(),
        vec![
            RichTextRange::new(2, 3),
            RichTextRange::new(1, 2),
            RichTextRange::new(0, 1),
        ]
    );
    assert_eq!(
        layout
            .glyphs
            .iter()
            .map(|glyph| glyph.logical_ordinal)
            .collect::<Vec<_>>(),
        vec![2, 1, 0]
    );
}

#[test]
fn shaped_ranges_outside_the_requested_run_are_rejected() {
    let document = document("a", style(), 3);
    let error = layout_document(
        &document,
        request(),
        &mut MockShaper::new(b"font").invalid_range(),
    )
    .expect_err("invalid shaper range must fail");

    assert!(matches!(
        error,
        TextLayoutError::InvalidShapedRange {
            run_index: 0,
            glyph_index: 0,
            range: RichTextRange { start: 0, end: 2 },
        }
    ));
}

#[test]
fn layout_hash_tracks_revision_inventory_and_constraints() {
    let first = document("a", style(), 4);
    let revised = document("a", style(), 5);
    let first_hash = layout_document(&first, request(), &mut MockShaper::new(b"font-a"))
        .expect("layout succeeds")
        .hash;
    let revised_hash = layout_document(&revised, request(), &mut MockShaper::new(b"font-a"))
        .expect("layout succeeds")
        .hash;
    let other_inventory = layout_document(&first, request(), &mut MockShaper::new(b"font-b"))
        .expect("layout succeeds")
        .hash;
    let constrained_hash = layout_document(
        &first,
        TextLayoutRequest {
            size: LayoutSize::new(60.0, 100.0),
            ..request()
        },
        &mut MockShaper::new(b"font-a"),
    )
    .expect("layout succeeds")
    .hash;

    assert_ne!(first_hash, revised_hash);
    assert_ne!(first_hash, other_inventory);
    assert_ne!(first_hash, constrained_hash);
}

#[test]
fn paint_only_color_is_excluded_from_layout_hash() {
    let pale = document(
        "a",
        style().with_color(TextColor::rgba(245, 245, 245, 255)),
        6,
    );
    let red = document("a", style().with_color(TextColor::rgba(255, 0, 0, 255)), 6);

    let pale_hash = layout_document(&pale, request(), &mut MockShaper::new(b"font"))
        .expect("layout succeeds")
        .hash;
    let red_hash = layout_document(&red, request(), &mut MockShaper::new(b"font"))
        .expect("layout succeeds")
        .hash;

    assert_eq!(pale_hash, red_hash);
}
