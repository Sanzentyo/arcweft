//! Frame-local text preparation shared by native, Web, and headless paths.

use std::collections::BTreeMap;

use arcweft_presentation::{
    fx::{
        FiniteF32, ResolvedFxGlyphPass, ResolvedFxMask, ResolvedFxOffscreenPass,
        ResolvedFxPostProcess, ResolvedTransform2D,
    },
    input::InteractionTarget,
};
use arcweft_render_text::{RichTextRange, TextColor};
use arcweft_text_layout::{
    GlyphOrientation, LayoutPoint, LayoutRect, LayoutSize, TextLayout, TextLayoutGlyph,
    TextLayoutRubyGlyph,
};
use glyphon::{
    Affine2, CacheKey, Color, GlyphArea, GlyphInstance, GlyphSource, GlyphTransform, Point, Rect,
    TextBounds, TextCluster, Vector,
};
use thiserror::Error;

use crate::{GlyphonTextEngine, GlyphonTextEngineError};

/// Frame-local stable index into one [`PreparedTextBatch`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PreparedTextId(u32);

/// All text prepared for one frame in painter order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreparedTextBatch {
    items: Vec<PreparedTextItem>,
}

/// Body or ruby origin of one prepared raster glyph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedGlyphSource {
    Body { glyph_index: u32 },
    Ruby { ruby_index: u32, glyph_index: u32 },
}

/// One pre-shaped glyph with renderer-local raster identity.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedGlyph {
    pub source: PreparedGlyphSource,
    pub origin: LayoutPoint,
    pub advance: LayoutSize,
    pub layout_bounds: LayoutRect,
    pub ink_bounds: LayoutRect,
    pub source_range: RichTextRange,
    pub cluster_index: u32,
    pub orientation: GlyphOrientation,
    pub inline_scale: f32,
    cache_key: CacheKey,
}

/// One canonical layout plus paint, interaction, clipping, and raster keys.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedTextItem {
    pub layout: TextLayout,
    pub glyphs: Vec<PreparedGlyph>,
    pub paint: TextPaintPlan,
    pub interaction: TextInteractionPlan,
    pub clip: Option<LayoutRect>,
    raster_scale: f32,
}

/// Paint-only plan. Updating it never changes [`TextLayout::hash`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TextPaintPlan {
    pub glyphs: Vec<TextGlyphPaint>,
    pub offscreen_passes: Vec<ResolvedFxOffscreenPass>,
    pub post_processes: Vec<ResolvedFxPostProcess>,
}

/// Paint for one body or ruby glyph in prepared order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextGlyphPaint {
    pub visible: bool,
    pub opacity_milli: u16,
    pub color: TextColor,
    pub transform: TextGlyphTransform,
    /// Closed additional raster passes emitted before the main glyph.
    pub effects: Vec<ResolvedFxGlyphPass>,
    pub masks: Vec<ResolvedFxMask>,
}

/// Resolved finite affine/opacity transform applied after layout orientation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextGlyphTransform(ResolvedTransform2D);

/// Character geometry produced from the same layout used for rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextCharacterBounds {
    pub source_range: RichTextRange,
    pub bounds: LayoutRect,
}

/// Caret paint in layout-local coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextCaretPaint {
    pub bounds: LayoutRect,
    pub color: TextColor,
    pub visible: bool,
}

/// One IME composition underline fragment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextCompositionUnderline {
    pub source_range: RichTextRange,
    pub bounds: LayoutRect,
    pub color: TextColor,
    pub thickness: f32,
}

/// Selection, caret, composition, and character geometry for one item.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextInteractionPlan {
    pub target: Option<InteractionTarget>,
    pub selection_enabled: bool,
    pub text: String,
    pub container_bounds: Option<LayoutRect>,
    pub selection_rects: Vec<LayoutRect>,
    pub selection_rgba: [f32; 4],
    pub caret: Option<TextCaretPaint>,
    pub composition_underlines: Vec<TextCompositionUnderline>,
    pub character_bounds: Vec<TextCharacterBounds>,
}

/// Owned glyph instances and scale borrowed by a glyphon submission view.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedTextSubmission {
    glyphs: Vec<GlyphInstance>,
    raster_scale: f32,
}

/// Structured prepared-text construction failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PreparedTextError {
    #[error("prepared text batch contains more than u32::MAX items")]
    TooManyItems,
    #[error("paint plan has {actual} glyphs but layout requires {expected}")]
    PaintGlyphCountMismatch { expected: usize, actual: usize },
    #[error("glyph paint {glyph_index} opacity {opacity_milli} exceeds 1000")]
    InvalidOpacity {
        glyph_index: usize,
        opacity_milli: u16,
    },
    #[error("prepared text clip contains invalid geometry")]
    InvalidClip,
    #[error(transparent)]
    RasterKey(#[from] GlyphonTextEngineError),
}

impl PreparedTextId {
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

impl PreparedTextBatch {
    pub fn push(&mut self, item: PreparedTextItem) -> Result<PreparedTextId, PreparedTextError> {
        let index = u32::try_from(self.items.len()).map_err(|_| PreparedTextError::TooManyItems)?;
        self.items.push(item);
        Ok(PreparedTextId(index))
    }

    #[must_use]
    pub fn get(&self, id: PreparedTextId) -> Option<&PreparedTextItem> {
        usize::try_from(id.0)
            .ok()
            .and_then(|index| self.items.get(index))
    }

    pub fn get_mut(&mut self, id: PreparedTextId) -> Option<&mut PreparedTextItem> {
        usize::try_from(id.0)
            .ok()
            .and_then(|index| self.items.get_mut(index))
    }

    pub fn items(&self) -> &[PreparedTextItem] {
        &self.items
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (PreparedTextId, &PreparedTextItem)> {
        self.items.iter().enumerate().map(|(index, item)| {
            (
                PreparedTextId(u32::try_from(index).unwrap_or(u32::MAX)),
                item,
            )
        })
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl TextGlyphTransform {
    pub const IDENTITY: Self = Self(ResolvedTransform2D::identity());

    #[must_use]
    pub const fn new(transform: ResolvedTransform2D) -> Self {
        Self(transform)
    }

    #[must_use]
    pub const fn resolved(self) -> ResolvedTransform2D {
        self.0
    }

    fn affine(self) -> Affine2 {
        let [m11, m12, m21, m22] = self.0.matrix();
        let [translate_x, translate_y] = self.0.translation();
        Affine2::new([
            m11.get(),
            m12.get(),
            m21.get(),
            m22.get(),
            translate_x.pixels(),
            translate_y.pixels(),
        ])
    }

    fn matrix_is_identity(self) -> bool {
        let [m11, m12, m21, m22] = self.0.matrix();
        let [translate_x, translate_y] = self.0.translation();
        m11 == FiniteF32::ONE
            && m12 == FiniteF32::ZERO
            && m21 == FiniteF32::ZERO
            && m22 == FiniteF32::ONE
            && translate_x.value() == FiniteF32::ZERO
            && translate_y.value() == FiniteF32::ZERO
    }
}

impl Default for TextGlyphTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl TextGlyphPaint {
    #[must_use]
    pub fn opaque(color: TextColor) -> Self {
        Self {
            visible: true,
            opacity_milli: 1_000,
            color,
            transform: TextGlyphTransform::IDENTITY,
            effects: Vec::new(),
            masks: Vec::new(),
        }
    }
}

impl TextPaintPlan {
    #[must_use]
    pub fn from_layout(layout: &TextLayout) -> Self {
        let mut glyphs = layout
            .glyphs
            .iter()
            .map(|glyph| {
                let color = usize::try_from(glyph.run_index)
                    .ok()
                    .and_then(|index| layout.runs.get(index))
                    .map_or_else(TextColor::default, |run| run.style.color());
                TextGlyphPaint::opaque(color)
            })
            .collect::<Vec<_>>();
        glyphs.extend(layout.ruby.iter().flat_map(|annotation| {
            annotation
                .glyphs
                .iter()
                .map(|_| TextGlyphPaint::opaque(annotation.style.color()))
        }));
        Self {
            glyphs,
            offscreen_passes: Vec::new(),
            post_processes: Vec::new(),
        }
    }

    fn validate(&self, expected: usize) -> Result<(), PreparedTextError> {
        if self.glyphs.len() != expected {
            return Err(PreparedTextError::PaintGlyphCountMismatch {
                expected,
                actual: self.glyphs.len(),
            });
        }
        for (glyph_index, paint) in self.glyphs.iter().enumerate() {
            if paint.opacity_milli > 1_000 {
                return Err(PreparedTextError::InvalidOpacity {
                    glyph_index,
                    opacity_milli: paint.opacity_milli,
                });
            }
        }
        Ok(())
    }
}

impl TextInteractionPlan {
    #[must_use]
    pub fn from_layout(layout: &TextLayout, target: Option<InteractionTarget>) -> Self {
        let mut character_bounds = BTreeMap::<(usize, usize), LayoutRect>::new();
        for glyph in &layout.glyphs {
            character_bounds
                .entry((glyph.source_range.start, glyph.source_range.end))
                .and_modify(|bounds| *bounds = bounds.union(glyph.layout_bounds))
                .or_insert(glyph.layout_bounds);
        }
        Self {
            target,
            selection_enabled: false,
            text: String::new(),
            container_bounds: layout.bounds,
            selection_rects: Vec::new(),
            selection_rgba: [0.2, 0.4, 0.8, 0.5],
            caret: None,
            composition_underlines: Vec::new(),
            character_bounds: character_bounds
                .into_iter()
                .map(|((start, end), bounds)| TextCharacterBounds {
                    source_range: RichTextRange::new(start, end),
                    bounds,
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn with_selection(mut self, source_range: RichTextRange) -> Self {
        self.selection_rects = self
            .character_bounds
            .iter()
            .filter(|character| ranges_overlap(character.source_range, source_range))
            .map(|character| character.bounds)
            .collect();
        self
    }

    #[must_use]
    pub fn with_text_and_selection_color(
        mut self,
        text: impl Into<String>,
        selection_rgba: [f32; 4],
    ) -> Self {
        self.text = text.into();
        self.selection_rgba = selection_rgba;
        self
    }

    #[must_use]
    pub const fn with_container_bounds(mut self, bounds: LayoutRect) -> Self {
        self.container_bounds = Some(bounds);
        self
    }

    #[must_use]
    pub const fn with_selection_enabled(mut self, enabled: bool) -> Self {
        self.selection_enabled = enabled;
        self
    }
}

impl GlyphonTextEngine {
    /// Resolves stable layout glyph keys once for one frame's raster scale.
    pub fn prepare_text_item(
        &self,
        layout: TextLayout,
        paint: TextPaintPlan,
        interaction: TextInteractionPlan,
        clip: Option<LayoutRect>,
        raster_scale: f32,
    ) -> Result<PreparedTextItem, PreparedTextError> {
        validate_clip(clip)?;
        let expected_glyphs = layout
            .ruby
            .iter()
            .map(|annotation| annotation.glyphs.len())
            .sum::<usize>()
            .saturating_add(layout.glyphs.len());
        paint.validate(expected_glyphs)?;
        let mut glyphs = Vec::with_capacity(expected_glyphs);
        for (glyph_index, glyph) in layout.glyphs.iter().enumerate() {
            glyphs.push(self.prepare_body_glyph(glyph_index, glyph, raster_scale)?);
        }
        for annotation in &layout.ruby {
            for (glyph_index, glyph) in annotation.glyphs.iter().enumerate() {
                glyphs.push(self.prepare_ruby_glyph(
                    annotation.ruby_index,
                    glyph_index,
                    glyph,
                    raster_scale,
                )?);
            }
        }
        Ok(PreparedTextItem {
            layout,
            glyphs,
            paint,
            interaction,
            clip,
            raster_scale,
        })
    }

    /// Convenience preparation with resolved style colors and layout-derived
    /// character geometry.
    pub fn prepare_layout(
        &self,
        layout: TextLayout,
        target: Option<InteractionTarget>,
        clip: Option<LayoutRect>,
        raster_scale: f32,
    ) -> Result<PreparedTextItem, PreparedTextError> {
        let paint = TextPaintPlan::from_layout(&layout);
        let interaction = TextInteractionPlan::from_layout(&layout, target);
        self.prepare_text_item(layout, paint, interaction, clip, raster_scale)
    }

    fn prepare_body_glyph(
        &self,
        glyph_index: usize,
        glyph: &TextLayoutGlyph,
        raster_scale: f32,
    ) -> Result<PreparedGlyph, PreparedTextError> {
        let key = self.prepare_raster_key_for_scale(glyph.shape_key, glyph.origin, raster_scale)?;
        Ok(PreparedGlyph {
            source: PreparedGlyphSource::Body {
                glyph_index: u32::try_from(glyph_index).unwrap_or(u32::MAX),
            },
            origin: glyph.origin,
            advance: glyph.advance,
            layout_bounds: glyph.layout_bounds,
            ink_bounds: glyph.ink_bounds,
            source_range: glyph.source_range,
            cluster_index: glyph.cluster_index,
            orientation: glyph.orientation,
            inline_scale: glyph.inline_scale,
            cache_key: key.cache_key,
        })
    }

    fn prepare_ruby_glyph(
        &self,
        ruby_index: u32,
        glyph_index: usize,
        glyph: &TextLayoutRubyGlyph,
        raster_scale: f32,
    ) -> Result<PreparedGlyph, PreparedTextError> {
        let key = self.prepare_raster_key_for_scale(glyph.shape_key, glyph.origin, raster_scale)?;
        Ok(PreparedGlyph {
            source: PreparedGlyphSource::Ruby {
                ruby_index,
                glyph_index: u32::try_from(glyph_index).unwrap_or(u32::MAX),
            },
            origin: glyph.origin,
            advance: glyph.advance,
            layout_bounds: glyph.layout_bounds,
            ink_bounds: glyph.ink_bounds,
            source_range: glyph.text_range,
            cluster_index: glyph.cluster_index,
            orientation: glyph.orientation,
            inline_scale: glyph.inline_scale,
            cache_key: key.cache_key,
        })
    }
}

impl PreparedTextItem {
    /// Builds glyphon instances from the current paint-only state.
    #[must_use]
    pub fn submission(&self) -> PreparedTextSubmission {
        let visible = self
            .glyphs
            .iter()
            .zip(&self.paint.glyphs)
            .enumerate()
            .filter(|(_, (_, paint))| paint.visible && paint.opacity_milli > 0)
            .collect::<Vec<_>>();
        let effect_pass_count = visible
            .iter()
            .map(|(_, (_, paint))| paint.effects.len())
            .max()
            .unwrap_or_default();
        let mut glyphs = Vec::with_capacity(
            visible
                .len()
                .saturating_mul(effect_pass_count.saturating_add(1)),
        );
        for pass_index in 0..effect_pass_count {
            glyphs.extend(visible.iter().filter_map(|(metadata, (glyph, paint))| {
                paint
                    .effects
                    .get(pass_index)
                    .map(|pass| glyph_instance(*metadata, glyph, paint, Some(*pass)))
            }));
        }
        glyphs.extend(
            visible
                .into_iter()
                .map(|(metadata, (glyph, paint))| glyph_instance(metadata, glyph, paint, None)),
        );
        PreparedTextSubmission {
            glyphs,
            raster_scale: self.raster_scale,
        }
    }
}

impl PreparedTextSubmission {
    pub fn glyphs(&self) -> &[GlyphInstance] {
        &self.glyphs
    }

    #[must_use]
    pub const fn raster_scale(&self) -> f32 {
        self.raster_scale
    }

    pub fn glyph_area(&self, bounds: TextBounds) -> GlyphArea<'_> {
        GlyphArea {
            glyphs: &self.glyphs,
            left: 0.0,
            top: 0.0,
            scale: self.raster_scale,
            bounds,
            default_color: Color::rgba(255, 255, 255, 255),
            force_alpha_mask: false,
        }
    }
}

fn glyph_instance(
    metadata: usize,
    glyph: &PreparedGlyph,
    paint: &TextGlyphPaint,
    effect: Option<ResolvedFxGlyphPass>,
) -> GlyphInstance {
    let mut transform = match glyph.orientation {
        GlyphOrientation::Upright => GlyphTransform::Identity,
        GlyphOrientation::SidewaysCw => GlyphTransform::Rotate90Cw,
        GlyphOrientation::TextCombineUpright => {
            GlyphTransform::Affine(Affine2::new([glyph.inline_scale, 0.0, 0.0, 1.0, 0.0, 0.0]))
        }
    };
    if !paint.transform.matrix_is_identity() {
        transform = transform.then_affine(paint.transform.affine());
    }
    let [offset_x, offset_y] = effect.map_or([0.0, 0.0], |effect| {
        [effect.offset_x.pixels(), effect.offset_y.pixels()]
    });
    GlyphInstance {
        source: GlyphSource::Text {
            cache_key: glyph.cache_key,
        },
        origin: Point::new(glyph.origin.x + offset_x, glyph.origin.y + offset_y),
        advance: Vector::new(glyph.advance.width, glyph.advance.height),
        ink_bounds: Rect::new(
            glyph.ink_bounds.x - glyph.origin.x,
            glyph.ink_bounds.y - glyph.origin.y,
            glyph.ink_bounds.right() - glyph.origin.x,
            glyph.ink_bounds.bottom() - glyph.origin.y,
        ),
        transform,
        color: Some(glyph_color(paint, effect.map(|effect| effect.color))),
        metadata,
        cluster: Some(TextCluster {
            start: glyph.source_range.start,
            end: glyph.source_range.end,
            index: glyph.cluster_index,
        }),
    }
}

fn glyph_color(paint: &TextGlyphPaint, effect: Option<arcweft_presentation::fx::FxColor>) -> Color {
    let [red, green, blue, alpha] = effect.map_or_else(
        || paint.color.channels(),
        |color| {
            [
                unit_to_u8(color.red().value().get()),
                unit_to_u8(color.green().value().get()),
                unit_to_u8(color.blue().value().get()),
                unit_to_u8(color.alpha().value().get()),
            ]
        },
    );
    let mask_coverage = paint.masks.iter().fold(1.0, |coverage, mask| {
        coverage * mask.effective_coverage().value().get()
    });
    let opacity = f32::from(paint.opacity_milli) / 1_000.0
        * paint.transform.resolved().opacity().value().get()
        * mask_coverage;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let alpha = (f32::from(alpha) * opacity).round() as u8;
    Color::rgba(red, green, blue, alpha)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn unit_to_u8(value: f32) -> u8 {
    (value * 255.0).round() as u8
}

fn validate_clip(clip: Option<LayoutRect>) -> Result<(), PreparedTextError> {
    if let Some(clip) = clip {
        let values = [clip.x, clip.y, clip.width, clip.height];
        if values.iter().any(|value| !value.is_finite()) || clip.width < 0.0 || clip.height < 0.0 {
            return Err(PreparedTextError::InvalidClip);
        }
    }
    Ok(())
}

fn ranges_overlap(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

#[cfg(test)]
mod tests {
    use arcweft_presentation::fx::{FiniteF32, Opacity, ResolvedFxMask};

    use super::{PreparedTextError, TextGlyphPaint, TextPaintPlan};

    #[test]
    fn paint_validation_accepts_only_closed_mask_payloads() {
        let paint = TextPaintPlan {
            glyphs: vec![TextGlyphPaint {
                masks: vec![ResolvedFxMask {
                    coverage: Opacity::try_new(FiniteF32::try_new(0.5).expect("finite"))
                        .expect("opacity"),
                    invert: false,
                }],
                ..TextGlyphPaint::opaque(arcweft_render_text::TextColor::default())
            }],
            offscreen_passes: Vec::new(),
            post_processes: Vec::new(),
        };

        assert_eq!(paint.validate(1), Ok(()));

        let mut invalid = paint;
        invalid.glyphs[0].opacity_milli = 1_001;
        assert!(matches!(
            invalid.validate(1),
            Err(PreparedTextError::InvalidOpacity {
                glyph_index: 0,
                opacity_milli: 1_001,
            })
        ));
    }
}
