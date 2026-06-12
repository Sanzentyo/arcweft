//! Adapter from Arcweft Sans I/O text layout geometry to glyphon `GlyphArea`.

use arcweft_text_layout::{GlyphOrientation, LaidOutGlyph, LaidOutText};
use glyphon::{
    Affine2, Buffer, CacheKey, Color, GlyphArea, GlyphInstance, GlyphSource, GlyphTransform, Point,
    Rect, TextBounds, TextCluster, Vector,
};
use thiserror::Error;

/// Error raised while adapting laid-out text to glyphon glyph areas.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum GlyphonAdapterError {
    /// A laid-out glyph could not be mapped to a glyphon cache key.
    #[error("missing glyphon cache key for glyph at layout index {glyph_index}")]
    MissingCacheKey {
        /// Index in `LaidOutText::glyphs`.
        glyph_index: usize,
    },
}

/// Static options used to wrap one Arcweft layout in a glyphon area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphonAreaOptions {
    /// Area left offset.
    pub left: f32,
    /// Area top offset.
    pub top: f32,
    /// Area scale.
    pub scale: f32,
    /// Offset applied to each layout glyph origin before submitting to glyphon.
    pub origin_offset: Vector,
    /// Clip bounds.
    pub bounds: TextBounds,
    /// Default glyph color.
    pub default_color: Color,
    /// Whether missing cache keys should skip glyphs instead of erroring.
    pub skip_missing_glyphs: bool,
}

impl Default for GlyphonAreaOptions {
    fn default() -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            scale: 1.0,
            origin_offset: Vector::new(0.0, 0.0),
            bounds: TextBounds::default(),
            default_color: Color::rgb(245, 245, 245),
            skip_missing_glyphs: false,
        }
    }
}

/// Owned glyph area data whose borrowed view can be submitted to glyphon.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnedGlyphArea {
    glyphs: Vec<GlyphInstance>,
    left: f32,
    top: f32,
    scale: f32,
    bounds: TextBounds,
    default_color: Color,
    skipped_glyphs: usize,
}

impl OwnedGlyphArea {
    /// Returns a borrowed glyphon area view.
    pub fn as_glyph_area(&self) -> GlyphArea<'_> {
        GlyphArea {
            glyphs: &self.glyphs,
            left: self.left,
            top: self.top,
            scale: self.scale,
            bounds: self.bounds,
            default_color: self.default_color,
        }
    }

    /// Adapted glyph instances.
    pub fn glyphs(&self) -> &[GlyphInstance] {
        &self.glyphs
    }

    /// Assigns an explicit color to every glyph instance emitted for one
    /// Arcweft layout glyph index.
    pub fn set_color_for_layout_glyph(&mut self, glyph_index: usize, color: Color) {
        for glyph in &mut self.glyphs {
            if glyph.metadata == glyph_index {
                glyph.color = Some(color);
            }
        }
    }

    /// Number of laid-out glyphs skipped because cache keys were unavailable.
    pub const fn skipped_glyphs(&self) -> usize {
        self.skipped_glyphs
    }

    /// Number of adapted glyph instances.
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    /// Whether the area contains no glyph instances.
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }
}

/// One shaped renderer glyph resolved from a laid-out Arcweft source cluster.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedGlyph {
    /// glyphon/cosmic-text cache key for the shaped glyph.
    pub cache_key: CacheKey,
    /// Shaped advance before Arcweft vertical/text-combine transforms.
    pub advance: Vector,
}

/// Adapts Arcweft layout geometry to a glyphon `GlyphArea`.
///
/// The resolver boundary keeps font shaping and cache-key ownership in the
/// renderer adapter. This crate only maps Arcweft geometry and orientation into
/// glyphon renderer instances.
pub fn glyph_area_from_layout(
    layout: &LaidOutText,
    options: GlyphonAreaOptions,
    mut resolve_glyphs: impl FnMut(usize, &LaidOutGlyph) -> Vec<ResolvedGlyph>,
) -> Result<OwnedGlyphArea, GlyphonAdapterError> {
    let mut skipped_glyphs = 0;
    let mut glyphs = Vec::with_capacity(layout.glyphs.len());
    for (glyph_index, glyph) in layout.glyphs.iter().enumerate() {
        let resolved = resolve_glyphs(glyph_index, glyph);
        if resolved.is_empty() {
            if options.skip_missing_glyphs {
                skipped_glyphs += 1;
                continue;
            }
            return Err(GlyphonAdapterError::MissingCacheKey { glyph_index });
        }
        append_glyph_instances(
            &mut glyphs,
            glyph_index,
            glyph,
            &resolved,
            options.origin_offset,
        );
    }
    Ok(OwnedGlyphArea {
        glyphs,
        left: options.left,
        top: options.top,
        scale: options.scale,
        bounds: options.bounds,
        default_color: options.default_color,
        skipped_glyphs,
    })
}

/// Adapts an already-shaped glyphon text buffer to a pre-laid `GlyphArea`.
///
/// This is used by renderer adapters for secondary text streams, such as ruby,
/// whose placement is owned by Arcweft layout but whose glyph shaping still
/// comes from glyphon/cosmic-text. The emitted glyph origins are absolute
/// physical positions, matching glyphon's `TextArea` path cache-key generation.
pub fn glyph_area_from_shaped_buffer(
    buffer: &Buffer,
    options: GlyphonAreaOptions,
) -> OwnedGlyphArea {
    let glyphs = buffer
        .layout_runs()
        .filter(|run| is_buffer_run_visible(run.line_top, run.line_height, options))
        .flat_map(|run| {
            run.glyphs.iter().map(move |glyph| {
                let physical = glyph.physical((options.left, options.top), options.scale);
                GlyphInstance {
                    source: GlyphSource::Text {
                        cache_key: physical.cache_key,
                    },
                    origin: Point::new(i32_to_f32(physical.x), i32_to_f32(physical.y)),
                    advance: Vector::new(glyph.w * options.scale, 0.0),
                    ink_bounds: Rect::new(0.0, 0.0, glyph.w * options.scale, run.line_height),
                    transform: GlyphTransform::Identity,
                    color: glyph.color_opt,
                    metadata: glyph.metadata,
                    cluster: Some(TextCluster {
                        start: glyph.start,
                        end: glyph.end,
                        index: u32::try_from(glyph.start).unwrap_or(u32::MAX),
                    }),
                }
            })
        })
        .collect();

    OwnedGlyphArea {
        glyphs,
        left: 0.0,
        top: 0.0,
        scale: 1.0,
        bounds: options.bounds,
        default_color: options.default_color,
        skipped_glyphs: 0,
    }
}

fn is_buffer_run_visible(line_top: f32, line_height: f32, options: GlyphonAreaOptions) -> bool {
    let start_y = options.top + (line_top * options.scale);
    let end_y = start_y + (line_height * options.scale);
    start_y <= i32_to_f32(options.bounds.bottom) && i32_to_f32(options.bounds.top) <= end_y
}

fn append_glyph_instances(
    instances: &mut Vec<GlyphInstance>,
    glyph_index: usize,
    glyph: &LaidOutGlyph,
    resolved: &[ResolvedGlyph],
    origin_offset: Vector,
) {
    if glyph.orientation == GlyphOrientation::TextCombineUpright {
        append_text_combine_instances(instances, glyph_index, glyph, resolved, origin_offset);
        return;
    }
    if let Some(resolved) = resolved.first().copied() {
        instances.push(glyph_instance(
            glyph_index,
            glyph,
            resolved.cache_key,
            Point::new(glyph.origin.x, glyph.origin.y),
            Vector::new(glyph.advance.width, glyph.advance.height),
            Rect::new(0.0, 0.0, glyph.bounds.width, glyph.bounds.height),
            origin_offset,
        ));
    }
}

fn append_text_combine_instances(
    instances: &mut Vec<GlyphInstance>,
    glyph_index: usize,
    glyph: &LaidOutGlyph,
    resolved: &[ResolvedGlyph],
    origin_offset: Vector,
) {
    let placement = text_combine_placement(glyph, resolved);
    let mut cursor_x = placement.start_x;
    for glyph_resolved in resolved.iter().copied() {
        let advance_x = (glyph_resolved.advance.x * placement.scale_x).max(1.0);
        let origin = Point::new(cursor_x, placement.origin_y);
        let mut instance = glyph_instance(
            glyph_index,
            glyph,
            glyph_resolved.cache_key,
            origin,
            Vector::new(advance_x, 0.0),
            Rect::new(0.0, 0.0, advance_x, placement.cell_height),
            origin_offset,
        );
        instance.transform = placement.transform;
        instances.push(instance);
        cursor_x += advance_x;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TextCombinePlacement {
    start_x: f32,
    origin_y: f32,
    scale_x: f32,
    cell_height: f32,
    transform: GlyphTransform,
}

fn text_combine_placement(
    glyph: &LaidOutGlyph,
    resolved: &[ResolvedGlyph],
) -> TextCombinePlacement {
    let cell_width = glyph.bounds.width.max(1.0);
    let cell_height = glyph.bounds.height.max(1.0);
    let em = cell_width.min(cell_height).max(1.0);
    let uncompressed_width = resolved
        .iter()
        .map(|resolved| resolved.advance.x.max(1.0))
        .sum::<f32>()
        .max(em * 0.5);
    let scale_x = (cell_width / uncompressed_width).min(1.0);
    let compressed_width = uncompressed_width * scale_x;
    let start_x = glyph.origin.x + (cell_width - compressed_width).max(0.0) * 0.5;
    TextCombinePlacement {
        start_x,
        origin_y: glyph.origin.y,
        scale_x,
        cell_height,
        transform: GlyphTransform::Affine(Affine2::new([scale_x, 0.0, 0.0, 1.0, 0.0, 0.0])),
    }
}

fn glyph_instance(
    glyph_index: usize,
    glyph: &LaidOutGlyph,
    cache_key: CacheKey,
    origin: Point,
    advance: Vector,
    ink_bounds: Rect,
    origin_offset: Vector,
) -> GlyphInstance {
    GlyphInstance {
        source: GlyphSource::Text { cache_key },
        origin: Point::new(origin.x + origin_offset.x, origin.y + origin_offset.y),
        advance,
        ink_bounds,
        transform: glyph_transform(glyph.orientation),
        color: None,
        metadata: glyph_index,
        cluster: Some(TextCluster {
            start: glyph.range.start,
            end: glyph.range.end,
            index: u32::try_from(glyph_index).unwrap_or(u32::MAX),
        }),
    }
}

fn i32_to_f32(value: i32) -> f32 {
    f32::from(i16::try_from(value).unwrap_or(if value.is_negative() {
        i16::MIN
    } else {
        i16::MAX
    }))
}

const fn glyph_transform(orientation: GlyphOrientation) -> GlyphTransform {
    match orientation {
        GlyphOrientation::Upright | GlyphOrientation::TextCombineUpright => {
            GlyphTransform::Identity
        }
        GlyphOrientation::SidewaysCw => GlyphTransform::Rotate90Cw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_text_layout::{
        GlyphVerticalForm, LaidOutGlyph, LayoutPoint, LayoutRect, LayoutSize, TextLayoutConfig,
        layout_frame,
    };
    use glyphon::{
        Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight, cosmic_text::CacheKeyFlags,
        fontdb,
    };

    fn fake_cache_key(glyph_id: u16) -> CacheKey {
        let (key, _, _) = CacheKey::new(
            fontdb::ID::dummy(),
            glyph_id,
            30.0,
            (0.0, 0.0),
            Weight::NORMAL,
            CacheKeyFlags::empty(),
        );
        key
    }

    fn fake_resolved_glyph(glyph_id: u16, advance_x: f32) -> ResolvedGlyph {
        ResolvedGlyph {
            cache_key: fake_cache_key(glyph_id),
            advance: Vector::new(advance_x, 0.0),
        }
    }

    fn assert_f32_eq(left: f32, right: f32) {
        assert!(
            (left - right).abs() <= 0.0001,
            "expected {left} to equal {right}"
        );
    }

    fn assert_affine_scale_x(transform: GlyphTransform, expected: f32) {
        let GlyphTransform::Affine(affine) = transform else {
            panic!("expected affine transform, got {transform:?}");
        };
        assert_f32_eq(affine.values[0], expected);
        assert_f32_eq(affine.values[1], 0.0);
        assert_f32_eq(affine.values[2], 0.0);
        assert_f32_eq(affine.values[3], 1.0);
        assert_f32_eq(affine.values[4], 0.0);
        assert_f32_eq(affine.values[5], 0.0);
    }

    #[test]
    fn maps_layout_glyphs_to_glyphon_instances() {
        let layout = LaidOutText {
            glyphs: vec![
                LaidOutGlyph {
                    run_index: 0,
                    range: arcweft_render_text::RichTextRange::new(0, 3),
                    text: "夢".to_owned(),
                    origin: LayoutPoint::new(10.0, 20.0),
                    advance: LayoutSize::new(0.0, 42.0),
                    bounds: LayoutRect::new(10.0, 20.0, 42.0, 42.0),
                    writing_mode: arcweft_render_text::RichTextWritingMode::VerticalRl,
                    orientation: GlyphOrientation::Upright,
                    vertical_form: GlyphVerticalForm::None,
                    presentation: arcweft_render_text::RichTextPresentation::default(),
                },
                LaidOutGlyph {
                    run_index: 0,
                    range: arcweft_render_text::RichTextRange::new(3, 4),
                    text: "A".to_owned(),
                    origin: LayoutPoint::new(10.0, 62.0),
                    advance: LayoutSize::new(0.0, 42.0),
                    bounds: LayoutRect::new(10.0, 62.0, 42.0, 42.0),
                    writing_mode: arcweft_render_text::RichTextWritingMode::VerticalRl,
                    orientation: GlyphOrientation::SidewaysCw,
                    vertical_form: GlyphVerticalForm::None,
                    presentation: arcweft_render_text::RichTextPresentation::default(),
                },
            ],
            runs: Vec::new(),
            ruby: Vec::new(),
            bounds: None,
        };

        let area =
            glyph_area_from_layout(&layout, GlyphonAreaOptions::default(), |index, _glyph| {
                vec![fake_resolved_glyph(
                    u16::try_from(index).unwrap_or(u16::MAX),
                    16.0,
                )]
            })
            .expect("area adapts");

        assert_eq!(area.len(), 2);
        assert_eq!(area.glyphs()[0].transform, GlyphTransform::Identity);
        assert_eq!(area.glyphs()[1].transform, GlyphTransform::Rotate90Cw);
        assert_eq!(
            area.glyphs()[1].cluster,
            Some(TextCluster {
                start: 3,
                end: 4,
                index: 1
            })
        );
    }

    #[test]
    fn origin_offset_moves_submitted_glyph_origin() {
        let layout = LaidOutText {
            glyphs: vec![LaidOutGlyph {
                run_index: 0,
                range: arcweft_render_text::RichTextRange::new(0, 1),
                text: "A".to_owned(),
                origin: LayoutPoint::new(10.0, 20.0),
                advance: LayoutSize::new(16.0, 0.0),
                bounds: LayoutRect::new(10.0, 20.0, 16.0, 42.0),
                writing_mode: arcweft_render_text::RichTextWritingMode::HorizontalTb,
                orientation: GlyphOrientation::Upright,
                vertical_form: GlyphVerticalForm::None,
                presentation: arcweft_render_text::RichTextPresentation::default(),
            }],
            runs: Vec::new(),
            ruby: Vec::new(),
            bounds: None,
        };

        let area = glyph_area_from_layout(
            &layout,
            GlyphonAreaOptions {
                origin_offset: Vector::new(2.0, 30.0),
                ..GlyphonAreaOptions::default()
            },
            |_index, _glyph| vec![fake_resolved_glyph(1, 16.0)],
        )
        .expect("area adapts");

        assert_eq!(area.glyphs()[0].origin, Point::new(12.0, 50.0));
    }

    #[test]
    fn can_assign_color_by_layout_glyph_index() {
        let layout = LaidOutText {
            glyphs: vec![LaidOutGlyph {
                run_index: 0,
                range: arcweft_render_text::RichTextRange::new(0, 4),
                text: "2026".to_owned(),
                origin: LayoutPoint::new(10.0, 20.0),
                advance: LayoutSize::new(0.0, 42.0),
                bounds: LayoutRect::new(10.0, 20.0, 42.0, 42.0),
                writing_mode: arcweft_render_text::RichTextWritingMode::VerticalRl,
                orientation: GlyphOrientation::TextCombineUpright,
                vertical_form: GlyphVerticalForm::None,
                presentation: arcweft_render_text::RichTextPresentation::default(),
            }],
            runs: Vec::new(),
            ruby: Vec::new(),
            bounds: None,
        };

        let mut area =
            glyph_area_from_layout(&layout, GlyphonAreaOptions::default(), |_index, _glyph| {
                vec![fake_resolved_glyph(1, 16.0), fake_resolved_glyph(2, 16.0)]
            })
            .expect("area adapts");
        area.set_color_for_layout_glyph(0, Color::rgb(255, 0, 0));

        assert_eq!(area.glyphs().len(), 2);
        assert!(
            area.glyphs()
                .iter()
                .all(|glyph| glyph.color == Some(Color::rgb(255, 0, 0)))
        );
    }

    #[test]
    fn text_combine_clusters_expand_to_compressed_glyph_instances() {
        let layout = LaidOutText {
            glyphs: vec![LaidOutGlyph {
                run_index: 0,
                range: arcweft_render_text::RichTextRange::new(0, 2),
                text: "12".to_owned(),
                origin: LayoutPoint::new(100.0, 24.0),
                advance: LayoutSize::new(0.0, 42.0),
                bounds: LayoutRect::new(100.0, 24.0, 42.0, 42.0),
                writing_mode: arcweft_render_text::RichTextWritingMode::VerticalRl,
                orientation: GlyphOrientation::TextCombineUpright,
                vertical_form: GlyphVerticalForm::None,
                presentation: arcweft_render_text::RichTextPresentation::default(),
            }],
            runs: Vec::new(),
            ruby: Vec::new(),
            bounds: None,
        };

        let area =
            glyph_area_from_layout(&layout, GlyphonAreaOptions::default(), |_index, _glyph| {
                vec![fake_resolved_glyph(1, 18.0), fake_resolved_glyph(2, 24.0)]
            })
            .expect("area adapts");

        assert_eq!(area.len(), 2);
        assert_affine_scale_x(area.glyphs()[0].transform, 1.0);
        assert_eq!(area.glyphs()[1].transform, area.glyphs()[0].transform);
        assert_f32_eq(area.glyphs()[0].origin.x, 100.0);
        assert_f32_eq(area.glyphs()[0].origin.y, 24.0);
        assert_f32_eq(area.glyphs()[1].origin.x, 118.0);
        assert_f32_eq(area.glyphs()[1].origin.y, 24.0);
        assert_f32_eq(area.glyphs()[0].advance.x, 18.0);
        assert_f32_eq(area.glyphs()[1].advance.x, 24.0);
        assert_eq!(
            area.glyphs()[0].cluster,
            Some(TextCluster {
                start: 0,
                end: 2,
                index: 0
            })
        );
        assert_eq!(area.glyphs()[0].metadata, area.glyphs()[1].metadata);
    }

    #[test]
    fn text_combine_four_digits_fit_inside_one_cell() {
        let layout = LaidOutText {
            glyphs: vec![LaidOutGlyph {
                run_index: 0,
                range: arcweft_render_text::RichTextRange::new(0, 4),
                text: "2026".to_owned(),
                origin: LayoutPoint::new(100.0, 24.0),
                advance: LayoutSize::new(0.0, 42.0),
                bounds: LayoutRect::new(100.0, 24.0, 42.0, 42.0),
                writing_mode: arcweft_render_text::RichTextWritingMode::VerticalRl,
                orientation: GlyphOrientation::TextCombineUpright,
                vertical_form: GlyphVerticalForm::None,
                presentation: arcweft_render_text::RichTextPresentation::default(),
            }],
            runs: Vec::new(),
            ruby: Vec::new(),
            bounds: None,
        };

        let area =
            glyph_area_from_layout(&layout, GlyphonAreaOptions::default(), |_index, _glyph| {
                vec![
                    fake_resolved_glyph(2, 15.0),
                    fake_resolved_glyph(0, 13.0),
                    fake_resolved_glyph(2, 15.0),
                    fake_resolved_glyph(6, 17.0),
                ]
            })
            .expect("area adapts");

        assert_eq!(area.len(), 4);
        for glyph in area.glyphs() {
            assert_affine_scale_x(glyph.transform, 42.0 / 60.0);
        }
        assert_f32_eq(area.glyphs()[0].origin.x, 100.0);
        assert_f32_eq(area.glyphs()[0].origin.y, 24.0);
        assert_f32_eq(area.glyphs()[0].advance.x, 10.5);
        assert_f32_eq(area.glyphs()[1].advance.x, 9.1);
        assert_f32_eq(area.glyphs()[2].advance.x, 10.5);
        assert_f32_eq(area.glyphs()[3].advance.x, 11.9);
        assert_f32_eq(area.glyphs()[3].origin.x, 130.1);
        assert_f32_eq(area.glyphs()[3].origin.y, 24.0);
        assert!(
            area.glyphs()[3].origin.x + area.glyphs()[3].advance.x <= 142.0,
            "compressed text-combine should stay inside the 1em cell"
        );
    }

    #[test]
    fn shaped_buffer_maps_to_absolute_glyph_instances() {
        let mut font_system = FontSystem::new();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(16.0, 20.0));
        buffer.set_size(&mut font_system, Some(400.0), Some(100.0));
        let attrs = Attrs::new().family(Family::SansSerif);
        buffer.set_rich_text(
            &mut font_system,
            [("ruby", attrs.clone())],
            &attrs,
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut font_system, false);

        let area = glyph_area_from_shaped_buffer(
            &buffer,
            GlyphonAreaOptions {
                left: 12.0,
                top: 34.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: 400,
                    bottom: 200,
                },
                default_color: Color::rgb(170, 190, 220),
                ..GlyphonAreaOptions::default()
            },
        );

        assert!(!area.is_empty());
        assert_f32_eq(area.as_glyph_area().left, 0.0);
        assert_f32_eq(area.as_glyph_area().top, 0.0);
        assert!(area.glyphs()[0].origin.x >= 12.0);
        assert!(area.glyphs()[0].origin.y >= 34.0);
    }

    #[test]
    fn missing_cache_key_can_skip_or_error() {
        let frame = arcweft_render_text::LineDisplayFrame {
            line: arcweft_core::plan::RuntimeLineId("say.test.001".to_owned()),
            callee: "alice.say".to_owned(),
            text: "A".to_owned(),
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            nodes: Vec::new(),
            display_map: arcweft_render_text::RichTextDisplayMap {
                text_runs: vec![arcweft_render_text::RichTextTextRun {
                    range: arcweft_render_text::RichTextRange::new(0, 1),
                    source: arcweft_render_text::RichTextTextSource::Text,
                    node_index: 0,
                    styles: Vec::new(),
                    presentation: arcweft_render_text::RichTextPresentation::default(),
                }],
                ruby_annotations: Vec::new(),
                controls: Vec::new(),
                host_events: Vec::new(),
            },
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
        };
        let layout = layout_frame(&frame, TextLayoutConfig::default()).expect("layout succeeds");

        assert_eq!(
            glyph_area_from_layout(
                &layout,
                GlyphonAreaOptions::default(),
                |_index, _glyph| vec![]
            )
            .expect_err("missing key errors"),
            GlyphonAdapterError::MissingCacheKey { glyph_index: 0 }
        );

        let area = glyph_area_from_layout(
            &layout,
            GlyphonAreaOptions {
                skip_missing_glyphs: true,
                ..GlyphonAreaOptions::default()
            },
            |_index, _glyph| Vec::new(),
        )
        .expect("missing key skips");
        assert_eq!(area.skipped_glyphs(), 1);
        assert!(area.is_empty());
    }
}
