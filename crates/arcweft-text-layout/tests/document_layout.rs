use std::{error::Error, fmt};

use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextRuby, ResolvedTextRun, ResolvedTextRunSource,
    ResolvedTextStyle, RichTextInlineDirection, RichTextJlreqStrictness, RichTextLayout,
    RichTextPresentation, RichTextRange, RichTextRubyPosition, RichTextWritingMode, TextColor,
    TextDocumentRevision, TextFontFamily,
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
    document_with_presentation(
        text,
        style,
        RichTextPresentation::default(),
        Vec::new(),
        revision,
    )
}

fn document_with_presentation(
    text: &str,
    style: ResolvedTextStyle,
    presentation: RichTextPresentation,
    ruby: Vec<ResolvedTextRuby>,
    revision: u64,
) -> ResolvedTextDocument<'_> {
    let range = RichTextRange::new(0, text.len());
    let run = ResolvedTextRun::new(
        range,
        range,
        style,
        presentation,
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
fn sideways_vertical_raster_origins_retain_shaped_bearings_inside_reported_ink() {
    let vertical = style().with_flow(
        RichTextWritingMode::VerticalRl,
        RichTextInlineDirection::Auto,
    );
    let document = document("ABC", vertical, 11);
    let layout = layout_document(&document, request(), &mut MockShaper::new(b"font"))
        .expect("sideways vertical layout succeeds");

    assert_eq!(layout.glyphs.len(), 3);
    assert!(
        layout
            .glyphs
            .iter()
            .all(|glyph| glyph.orientation == arcweft_text_layout::GlyphOrientation::SidewaysCw)
    );
    for glyph in &layout.glyphs {
        assert!((glyph.origin.x - glyph.ink_bounds.x).abs() < f32::EPSILON);
        assert!((glyph.origin.y - glyph.ink_bounds.y).abs() < f32::EPSILON);
    }
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

#[test]
fn vertical_lr_reserves_the_left_ruby_track_before_body_layout() {
    let text = "夢";
    let style = style().with_flow(
        RichTextWritingMode::VerticalLr,
        RichTextInlineDirection::Auto,
    );
    let range = RichTextRange::new(0, text.len());
    let ruby = ResolvedTextRuby::new(
        range,
        range,
        "ゆめ",
        style.clone(),
        RichTextPresentation::default(),
    )
    .expect("ruby is valid");
    let document =
        document_with_presentation(text, style, RichTextPresentation::default(), vec![ruby], 7);
    let layout = layout_document(&document, request(), &mut MockShaper::new(b"font"))
        .expect("vertical ruby lays out");
    let ruby = &layout.ruby[0];

    assert!(ruby.ruby_bounds.x < ruby.base_bounds.x);
    assert!(ruby.ruby_bounds.right() <= ruby.base_bounds.x + f32::EPSILON);
    assert!(ruby.ruby_bounds.x >= request().origin.x);
    assert!(ruby.ruby_bounds.right() <= request().origin.x + request().size.width);
}

#[test]
fn horizontal_over_ruby_stacks_outside_the_base_cell() {
    let text = "夢";
    let range = RichTextRange::new(0, text.len());
    let ruby = ResolvedTextRuby::new(
        range,
        range,
        "ゆめ",
        style(),
        RichTextPresentation::default(),
    )
    .expect("ruby is valid");
    let document = document_with_presentation(
        text,
        style(),
        RichTextPresentation::default(),
        vec![ruby],
        10,
    );
    let layout = layout_document(&document, request(), &mut MockShaper::new(b"font"))
        .expect("horizontal ruby lays out");
    let ruby = &layout.ruby[0];

    assert!(ruby.ruby_bounds.bottom() <= ruby.base_bounds.y + f32::EPSILON);
}

#[test]
fn vertical_inter_character_has_the_same_effect_as_over() {
    let text = "夢星人";
    let style = style().with_flow(
        RichTextWritingMode::VerticalRl,
        RichTextInlineDirection::Auto,
    );
    let base_range = RichTextRange::new(0, "夢星".len());
    let layout_with = |position, revision| {
        let presentation = RichTextPresentation {
            layout: Some(RichTextLayout {
                writing_mode: RichTextWritingMode::VerticalRl,
                ruby_position: position,
                ..RichTextLayout::default()
            }),
            ..RichTextPresentation::default()
        };
        let ruby = ResolvedTextRuby::new(
            base_range,
            base_range,
            "ゆめ",
            style.clone(),
            presentation.clone(),
        )
        .expect("ruby is valid");
        let document =
            document_with_presentation(text, style.clone(), presentation, vec![ruby], revision);
        layout_document(&document, request(), &mut MockShaper::new(b"font"))
            .expect("vertical ruby lays out")
    };
    let inter_character = layout_with(RichTextRubyPosition::InterCharacter, 11);
    let over = layout_with(RichTextRubyPosition::Over, 12);
    let ruby = &inter_character.ruby[0];

    assert_eq!(ruby.base_bounds, over.ruby[0].base_bounds);
    assert_eq!(ruby.ruby_bounds, over.ruby[0].ruby_bounds);
    assert!(ruby.ruby_bounds.x >= ruby.base_bounds.right());
    assert!(
        (inter_character.glyphs[2].layout_bounds.y
            - inter_character.glyphs[1].layout_bounds.bottom())
        .abs()
            < f32::EPSILON
    );
}

#[test]
fn horizontal_inter_character_is_inserted_to_the_right_of_its_base() {
    let text = "夢人";
    let base_range = RichTextRange::new(0, "夢".len());
    let presentation = RichTextPresentation {
        layout: Some(RichTextLayout {
            ruby_position: RichTextRubyPosition::InterCharacter,
            ..RichTextLayout::default()
        }),
        ..RichTextPresentation::default()
    };
    let ruby = ResolvedTextRuby::new(
        base_range,
        base_range,
        "ゆめ",
        style(),
        presentation.clone(),
    )
    .expect("inter-character ruby is valid");
    let document = document_with_presentation(text, style(), presentation, vec![ruby], 13);
    let layout = layout_document(&document, request(), &mut MockShaper::new(b"font"))
        .expect("horizontal inter-character ruby lays out");
    let ruby = &layout.ruby[0];

    assert_eq!(ruby.writing_mode, RichTextWritingMode::HorizontalTb);
    assert!(ruby.ruby_bounds.x >= ruby.base_bounds.right());
    assert!(layout.glyphs[1].layout_bounds.x >= ruby.ruby_bounds.right());
}

#[test]
fn authored_jlreq_strictness_changes_the_shaped_vertical_column_plan() {
    let text = "天地。「人山川海";
    let vertical = style().with_flow(
        RichTextWritingMode::VerticalRl,
        RichTextInlineDirection::Auto,
    );
    let with_strictness = |strictness| {
        document_with_presentation(
            text,
            vertical.clone(),
            RichTextPresentation {
                layout: Some(RichTextLayout {
                    writing_mode: RichTextWritingMode::VerticalRl,
                    jlreq_strictness: strictness,
                    ..RichTextLayout::default()
                }),
                ..RichTextPresentation::default()
            },
            Vec::new(),
            8,
        )
    };
    let request = TextLayoutRequest {
        size: LayoutSize::new(180.0, 70.0),
        ..request()
    };
    let loose = layout_document(
        &with_strictness(RichTextJlreqStrictness::Loose),
        request,
        &mut MockShaper::new(b"font"),
    )
    .expect("loose layout succeeds");
    let strict = layout_document(
        &with_strictness(RichTextJlreqStrictness::Strict),
        request,
        &mut MockShaper::new(b"font"),
    )
    .expect("strict layout succeeds");

    assert_ne!(loose.hash, strict.hash);
    assert_ne!(
        loose
            .glyphs
            .iter()
            .map(|glyph| (glyph.source_range, glyph.line_index))
            .collect::<Vec<_>>(),
        strict
            .glyphs
            .iter()
            .map(|glyph| (glyph.source_range, glyph.line_index))
            .collect::<Vec<_>>()
    );
}

#[test]
fn vertical_column_plan_is_independent_of_paint_run_boundaries() {
    let text = "縦夢へ2026XYZ。";
    let vertical = style().with_flow(
        RichTextWritingMode::VerticalLr,
        RichTextInlineDirection::Auto,
    );
    let presentation = RichTextPresentation {
        layout: Some(RichTextLayout {
            writing_mode: RichTextWritingMode::VerticalLr,
            jlreq_strictness: RichTextJlreqStrictness::Strict,
            ..RichTextLayout::default()
        }),
        ..RichTextPresentation::default()
    };
    let unsplit =
        document_with_presentation(text, vertical.clone(), presentation.clone(), Vec::new(), 9);
    let split_at = "縦夢へ".len();
    let split = ResolvedTextDocument::new(
        text,
        0,
        vec![
            ResolvedTextRun::new(
                RichTextRange::new(0, split_at),
                RichTextRange::new(0, split_at),
                vertical.clone(),
                presentation.clone(),
                ResolvedTextRunSource::Plain,
            )
            .expect("first split run is valid"),
            ResolvedTextRun::new(
                RichTextRange::new(split_at, text.len()),
                RichTextRange::new(split_at, text.len()),
                vertical,
                RichTextPresentation {
                    z_index: 1,
                    ..presentation
                },
                ResolvedTextRunSource::Plain,
            )
            .expect("paint-only split run is valid"),
        ],
        Vec::new(),
        TextDocumentRevision::new(9),
    )
    .expect("split document is valid");
    let request = TextLayoutRequest {
        size: LayoutSize::new(180.0, 70.0),
        ..request()
    };
    let unsplit = layout_document(&unsplit, request, &mut MockShaper::new(b"font"))
        .expect("unsplit vertical layout succeeds");
    let split = layout_document(&split, request, &mut MockShaper::new(b"font"))
        .expect("split vertical layout succeeds");

    assert_eq!(
        unsplit
            .glyphs
            .iter()
            .map(|glyph| (glyph.source_range, glyph.line_index, glyph.layout_bounds))
            .collect::<Vec<_>>(),
        split
            .glyphs
            .iter()
            .map(|glyph| (glyph.source_range, glyph.line_index, glyph.layout_bounds))
            .collect::<Vec<_>>()
    );
}
