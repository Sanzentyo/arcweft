//! Frame-local text preparation shared by native, Web, and headless paths.

use std::{collections::BTreeMap, sync::Arc};

use arcweft_presentation::{
    fx::{
        FiniteF32, ResolvedFxGlyphPass, ResolvedFxMask, ResolvedFxOffscreenPass,
        ResolvedFxPostProcess, ResolvedTransform2D,
    },
    input::InteractionTarget,
};
use arcweft_render_text::TextColor;
use arcweft_text_layout::{
    GlyphOrientation, LayoutPoint, LayoutRect, LayoutSize, TextLayout, TextLayoutGlyph,
    TextLayoutRubyGlyph,
};
use arcweft_text_model::RichTextRange;
use glyphon::{
    Affine2, CacheKey, Color, GlyphArea, GlyphInstance, GlyphSource, GlyphTransform, Point, Rect,
    TextBounds, TextCluster, Vector,
};
use thiserror::Error;

use crate::{
    GlyphonTextEngine, GlyphonTextEngineError, PreparedTextPhysicalBounds,
    PreparedTextPhysicalBoundsError,
};

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
    pub layout: Arc<TextLayout>,
    pub glyphs: Vec<PreparedGlyph>,
    pub paint: TextPaintPlan,
    pub interaction: TextInteractionPlan,
    pub clip: Option<LayoutRect>,
    physical_clip_bounds: Option<PreparedTextPhysicalBounds>,
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

/// Validated affine and opacity applied to an immutable prepared item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedTextAffine {
    matrix: [f32; 6],
    opacity: f32,
}

/// Geometry derived once from one prepared item's evaluated paint. It shares
/// the shaped layout and remains independent of glyph visibility, so a hidden
/// typewriter stage has the same selection/capture extent as its revealed stage.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedTextGeometry {
    body: Vec<PreparedGlyphGeometry>,
    lines: Vec<Option<LayoutRect>>,
    runs: Vec<Option<LayoutRect>>,
    ruby: BTreeMap<u32, PreparedRubyGeometry>,
    bounds: Option<LayoutRect>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedGlyphGeometry {
    pub logical_bounds: LayoutRect,
    /// Physical glyph geometry remains separate from its allocated Ruby space.
    pub allocated_bounds: LayoutRect,
    pub visual_bounds: LayoutRect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedRubyGeometry {
    pub base_bounds: LayoutRect,
    pub annotation_bounds: LayoutRect,
}

impl PreparedTextGeometry {
    #[must_use]
    pub fn glyph(&self, index: usize) -> Option<PreparedGlyphGeometry> {
        self.body.get(index).copied()
    }

    #[must_use]
    pub fn line(&self, index: usize) -> Option<LayoutRect> {
        self.lines.get(index).copied().flatten()
    }

    #[must_use]
    pub fn run(&self, index: usize) -> Option<LayoutRect> {
        self.runs.get(index).copied().flatten()
    }

    #[must_use]
    pub fn ruby(&self, index: u32) -> Option<PreparedRubyGeometry> {
        self.ruby.get(&index).copied()
    }

    #[must_use]
    pub const fn bounds(&self) -> Option<LayoutRect> {
        self.bounds
    }
}

/// Structured prepared-text construction failure.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum PreparedTextError {
    #[error("prepared glyph {index} has no admitted paint geometry")]
    MissingGlyphGeometry { index: usize },
    #[error("prepared glyph {index} has non-finite transformed geometry")]
    InvalidGlyphGeometry { index: usize },
    #[error("prepared text geometry has no actual painter context")]
    MissingPainterContext,
    #[error("prepared text batch contains more than u32::MAX items")]
    TooManyItems,
    #[error("paint plan has {actual} glyphs but layout requires {expected}")]
    PaintGlyphCountMismatch { expected: usize, actual: usize },
    #[error("glyph paint {glyph_index} opacity {opacity_milli} exceeds 1000")]
    InvalidOpacity {
        glyph_index: usize,
        opacity_milli: u16,
    },
    #[error(transparent)]
    PhysicalClipBounds(#[from] PreparedTextPhysicalBoundsError),
    #[error("prepared text submission affine contains a non-finite value")]
    InvalidSubmissionAffine,
    #[error("prepared text submission opacity {value} is outside [0, 1]")]
    InvalidSubmissionOpacity { value: String },
    #[error(transparent)]
    RasterKey(#[from] GlyphonTextEngineError),
}

impl PreparedTextId {
    #[must_use]
    pub const fn from_index(index: u32) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

impl PreparedTextAffine {
    pub const IDENTITY: Self = Self {
        matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        opacity: 1.0,
    };

    pub fn try_new(matrix: [f32; 6], opacity: f32) -> Result<Self, PreparedTextError> {
        if matrix.iter().any(|value| !value.is_finite()) {
            return Err(PreparedTextError::InvalidSubmissionAffine);
        }
        if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
            return Err(PreparedTextError::InvalidSubmissionOpacity {
                value: opacity.to_string(),
            });
        }
        Ok(Self { matrix, opacity })
    }

    #[must_use]
    pub const fn matrix(self) -> [f32; 6] {
        self.matrix
    }

    #[must_use]
    pub const fn opacity(self) -> f32 {
        self.opacity
    }

    fn point(self, point: Point) -> Point {
        let [m11, m12, m21, m22, tx, ty] = self.matrix;
        Point::new(
            m11.mul_add(point.x, m21.mul_add(point.y, tx)),
            m12.mul_add(point.x, m22.mul_add(point.y, ty)),
        )
    }

    fn vector(self, vector: Vector) -> Vector {
        let [m11, m12, m21, m22, _, _] = self.matrix;
        Vector::new(
            m11.mul_add(vector.x, m21 * vector.y),
            m12.mul_add(vector.x, m22 * vector.y),
        )
    }

    fn linear_affine(self) -> Affine2 {
        let [m11, m12, m21, m22, _, _] = self.matrix;
        // View/CSS stores the two column vectors; glyphon's six values are
        // row-major. Origin, advance, and raster shape must use one transform.
        Affine2::new([m11, m21, m12, m22, 0.0, 0.0])
    }
}

impl Default for PreparedTextAffine {
    fn default() -> Self {
        Self::IDENTITY
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

    /// Effective local glyph opacity after the authored transform and closed
    /// mask chain, before a containing View's opacity is applied.
    #[must_use]
    pub fn effective_opacity(&self) -> f32 {
        let mask_coverage = self.masks.iter().fold(1.0, |coverage, mask| {
            coverage * mask.effective_coverage().value().get()
        });
        f32::from(self.opacity_milli) / 1_000.0
            * self.transform.resolved().opacity().value().get()
            * mask_coverage
    }

    /// Effective local glyph opacity in the frame-observation milli contract.
    #[must_use]
    pub fn effective_opacity_milli(&self) -> u16 {
        let milli = (self.effective_opacity() * 1_000.0).round();
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "closed paint opacity is finite and constrained to [0, 1]"
        )]
        {
            milli as u16
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
        let physical_clip_bounds = clip
            .map(|clip| PreparedTextPhysicalBounds::try_from_logical(clip, raster_scale))
            .transpose()?;
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
            layout: Arc::new(layout),
            glyphs,
            paint,
            interaction,
            clip,
            physical_clip_bounds,
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
    /// Derives bounds under every actual View painter occurrence. The layout
    /// already owns orientation and text-combine placement; evaluated paint is
    /// applied about the same raster origin as `submission_with_affine`.
    pub fn painted_geometry(
        &self,
        affines: impl IntoIterator<Item = PreparedTextAffine>,
    ) -> Result<PreparedTextGeometry, PreparedTextError> {
        self.paint.validate(self.glyphs.len())?;
        let mut geometry = Vec::<PreparedGlyphGeometry>::new();
        let mut ruby_base_allocations = BTreeMap::<u32, LayoutRect>::new();
        for affine in affines {
            for (index, glyph) in self.glyphs.iter().enumerate() {
                let logical_bounds =
                    self.painted_glyph_bounds(index, glyph.layout_bounds, affine, [0.0, 0.0])?;
                let mut visual_bounds = logical_bounds.union(self.painted_glyph_bounds(
                    index,
                    glyph.ink_bounds,
                    affine,
                    [0.0, 0.0],
                )?);
                for pass in &self.paint.glyphs[index].effects {
                    visual_bounds = visual_bounds.union(self.painted_glyph_bounds(
                        index,
                        glyph.ink_bounds,
                        affine,
                        [pass.offset_x.pixels(), pass.offset_y.pixels()],
                    )?);
                }
                if let Some(existing) = geometry.get_mut(index) {
                    existing.logical_bounds = existing.logical_bounds.union(logical_bounds);
                    existing.allocated_bounds = existing.allocated_bounds.union(logical_bounds);
                    existing.visual_bounds = existing.visual_bounds.union(visual_bounds);
                } else {
                    geometry.push(PreparedGlyphGeometry {
                        logical_bounds,
                        allocated_bounds: logical_bounds,
                        visual_bounds,
                    });
                }
            }
            for annotation in &self.layout.ruby {
                for allocation in &annotation.base_allocations {
                    for body_index in allocation.glyph_range.clone() {
                        let index = usize::try_from(body_index).map_err(|_| {
                            PreparedTextError::MissingGlyphGeometry { index: usize::MAX }
                        })?;
                        let glyph = self
                            .layout
                            .glyphs
                            .get(index)
                            .ok_or(PreparedTextError::MissingGlyphGeometry { index })?;
                        let bounds = self
                            .painted_glyph_bounds(index, allocation.bounds, affine, [0.0, 0.0])?
                            .union(self.painted_glyph_bounds(
                                index,
                                glyph.ink_bounds,
                                affine,
                                [0.0, 0.0],
                            )?);
                        let body_geometry = geometry
                            .get_mut(index)
                            .ok_or(PreparedTextError::MissingGlyphGeometry { index })?;
                        body_geometry.allocated_bounds =
                            body_geometry.allocated_bounds.union(bounds);
                        ruby_base_allocations
                            .entry(annotation.ruby_index)
                            .and_modify(|current| *current = current.union(bounds))
                            .or_insert(bounds);
                    }
                }
            }
        }
        if geometry.len() != self.glyphs.len() {
            return Err(PreparedTextError::MissingPainterContext);
        }
        let body = geometry[..self.layout.glyphs.len()].to_vec();
        let mut lines = vec![None; self.layout.lines.len()];
        let mut runs = vec![None; self.layout.runs.len()];
        for (glyph, bounds) in self.layout.glyphs.iter().zip(&body) {
            for slot in
                [
                    lines.get_mut(usize::try_from(glyph.line_index).map_err(|_| {
                        PreparedTextError::MissingGlyphGeometry { index: usize::MAX }
                    })?),
                    runs.get_mut(usize::try_from(glyph.run_index).map_err(|_| {
                        PreparedTextError::MissingGlyphGeometry { index: usize::MAX }
                    })?),
                ]
            {
                let slot =
                    slot.ok_or(PreparedTextError::MissingGlyphGeometry { index: usize::MAX })?;
                let allocated = bounds.visual_bounds.union(bounds.allocated_bounds);
                *slot =
                    Some(slot.map_or(allocated, |current: LayoutRect| current.union(allocated)));
            }
        }
        let mut ruby = BTreeMap::new();
        let mut annotation_glyphs = BTreeMap::<u32, LayoutRect>::new();
        for (glyph, bounds) in self.glyphs.iter().zip(&geometry) {
            if let PreparedGlyphSource::Ruby { ruby_index, .. } = glyph.source {
                annotation_glyphs
                    .entry(ruby_index)
                    .and_modify(|current| *current = current.union(bounds.visual_bounds))
                    .or_insert(bounds.visual_bounds);
            }
        }
        for annotation in &self.layout.ruby {
            let mut base_bounds = ruby_base_allocations.get(&annotation.ruby_index).copied();
            // Offscreen/raster passes remain the actual body glyph's geometry;
            // allocation whitespace is projected only through its owning paint.
            for allocation in &annotation.base_allocations {
                for index in allocation.glyph_range.clone() {
                    let index = usize::try_from(index).map_err(|_| {
                        PreparedTextError::MissingGlyphGeometry { index: usize::MAX }
                    })?;
                    let bounds = body
                        .get(index)
                        .ok_or(PreparedTextError::MissingGlyphGeometry { index })?
                        .visual_bounds;
                    base_bounds = Some(base_bounds.map_or(bounds, |current| current.union(bounds)));
                }
            }
            if let (Some(base_bounds), Some(annotation_bounds)) = (
                base_bounds,
                annotation_glyphs.get(&annotation.ruby_index).copied(),
            ) {
                if let Some(allocation) = annotation.base_allocations.first() {
                    let index = usize::try_from(allocation.glyph_range.start).map_err(|_| {
                        PreparedTextError::MissingGlyphGeometry { index: usize::MAX }
                    })?;
                    let first = self
                        .layout
                        .glyphs
                        .get(index)
                        .ok_or(PreparedTextError::MissingGlyphGeometry { index })?;
                    let line_index = usize::try_from(first.line_index).map_err(|_| {
                        PreparedTextError::MissingGlyphGeometry { index: usize::MAX }
                    })?;
                    let line = lines
                        .get_mut(line_index)
                        .ok_or(PreparedTextError::MissingGlyphGeometry { index })?;
                    let bounds = base_bounds.union(annotation_bounds);
                    *line = Some(line.map_or(bounds, |current| current.union(bounds)));
                }
                ruby.insert(
                    annotation.ruby_index,
                    PreparedRubyGeometry {
                        base_bounds,
                        annotation_bounds,
                    },
                );
            }
        }
        let bounds = geometry
            .iter()
            .map(|glyph| glyph.visual_bounds)
            .chain(ruby.values().map(|ruby| ruby.base_bounds))
            .reduce(LayoutRect::union);
        Ok(PreparedTextGeometry {
            body,
            lines,
            runs,
            ruby,
            bounds,
        })
    }

    /// Logical cell geometry after the already-evaluated glyph paint and View
    /// affine. Layout bounds already include the glyph's orientation; it must
    /// not be applied a second time while deriving observation geometry.
    pub fn painted_glyph_layout_bounds(
        &self,
        index: usize,
        affine: PreparedTextAffine,
    ) -> Result<LayoutRect, PreparedTextError> {
        let glyph = self
            .glyphs
            .get(index)
            .ok_or(PreparedTextError::MissingGlyphGeometry { index })?;
        self.painted_glyph_bounds(index, glyph.layout_bounds, affine, [0.0, 0.0])
    }

    fn painted_glyph_bounds(
        &self,
        index: usize,
        bounds: LayoutRect,
        affine: PreparedTextAffine,
        effect_offset: [f32; 2],
    ) -> Result<LayoutRect, PreparedTextError> {
        let glyph = self
            .glyphs
            .get(index)
            .ok_or(PreparedTextError::MissingGlyphGeometry { index })?;
        let paint = self
            .paint
            .glyphs
            .get(index)
            .ok_or(PreparedTextError::MissingGlyphGeometry { index })?;
        let [m11, m12, m21, m22] = paint.transform.resolved().matrix().map(FiniteF32::get);
        let [tx, ty] = paint
            .transform
            .resolved()
            .translation()
            .map(|value| value.pixels());
        let corners = [
            Point::new(bounds.x, bounds.y),
            Point::new(bounds.right(), bounds.y),
            Point::new(bounds.x, bounds.bottom()),
            Point::new(bounds.right(), bounds.bottom()),
        ]
        .map(|point| {
            let x = point.x - glyph.origin.x;
            let y = point.y - glyph.origin.y;
            affine.point(Point::new(
                glyph.origin.x + effect_offset[0] + m11.mul_add(x, m12.mul_add(y, tx)),
                glyph.origin.y + effect_offset[1] + m21.mul_add(x, m22.mul_add(y, ty)),
            ))
        });
        if corners
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(PreparedTextError::InvalidGlyphGeometry { index });
        }
        let min_x = corners.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let min_y = corners.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let max_x = corners
            .iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let max_y = corners
            .iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);
        let width = max_x - min_x;
        let height = max_y - min_y;
        if !width.is_finite() || !height.is_finite() {
            return Err(PreparedTextError::InvalidGlyphGeometry { index });
        }
        Ok(LayoutRect::new(min_x, min_y, width, height))
    }

    /// Checked physical clip prepared alongside this item's logical clip.
    #[must_use]
    pub const fn physical_clip_bounds(&self) -> Option<PreparedTextPhysicalBounds> {
        self.physical_clip_bounds
    }

    /// Physical raster scale used by glyph instances and clip conversion.
    #[must_use]
    pub const fn raster_scale(&self) -> f32 {
        self.raster_scale
    }

    /// Builds glyphon instances from the current paint-only state.
    #[must_use]
    pub fn submission(&self) -> PreparedTextSubmission {
        self.submission_with_affine(PreparedTextAffine::IDENTITY)
    }

    /// Builds glyphon instances under one validated View/compositor context.
    #[must_use]
    pub fn submission_with_affine(&self, affine: PreparedTextAffine) -> PreparedTextSubmission {
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
                    .map(|pass| glyph_instance(*metadata, glyph, paint, Some(*pass), affine))
            }));
        }
        glyphs.extend(visible.into_iter().map(|(metadata, (glyph, paint))| {
            glyph_instance(metadata, glyph, paint, None, affine)
        }));
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
    affine: PreparedTextAffine,
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
    if affine != PreparedTextAffine::IDENTITY {
        transform = transform.then_affine(affine.linear_affine());
    }
    let [offset_x, offset_y] = effect.map_or([0.0, 0.0], |effect| {
        [effect.offset_x.pixels(), effect.offset_y.pixels()]
    });
    GlyphInstance {
        source: GlyphSource::Text {
            cache_key: glyph.cache_key,
        },
        origin: affine.point(Point::new(
            glyph.origin.x + offset_x,
            glyph.origin.y + offset_y,
        )),
        advance: affine.vector(Vector::new(glyph.advance.width, glyph.advance.height)),
        ink_bounds: Rect::new(
            glyph.ink_bounds.x - glyph.origin.x,
            glyph.ink_bounds.y - glyph.origin.y,
            glyph.ink_bounds.right() - glyph.origin.x,
            glyph.ink_bounds.bottom() - glyph.origin.y,
        ),
        transform,
        color: Some(glyph_color(
            paint,
            effect.map(|effect| effect.color),
            affine.opacity,
        )),
        metadata,
        cluster: Some(TextCluster {
            start: glyph.source_range.start,
            end: glyph.source_range.end,
            index: glyph.cluster_index,
        }),
    }
}

fn glyph_color(
    paint: &TextGlyphPaint,
    effect: Option<arcweft_presentation::fx::FxColor>,
    context_opacity: f32,
) -> Color {
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
    let opacity = paint.effective_opacity() * context_opacity;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let alpha = (f32::from(alpha) * opacity).round() as u8;
    Color::rgba(red, green, blue, alpha)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn unit_to_u8(value: f32) -> u8 {
    (value * 255.0).round() as u8
}

fn ranges_overlap(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

#[cfg(test)]
mod tests {
    use arcweft_presentation::fx::{FiniteF32, Opacity, ResolvedFxMask};
    use arcweft_render_text::TextColor;
    use arcweft_text_layout::{GlyphOrientation, LayoutPoint, LayoutRect, LayoutSize};
    use arcweft_text_model::RichTextRange;
    use glyphon::{
        CacheKey, Color, GlyphTransform, Point, Vector, Weight,
        cosmic_text::{CacheKeyFlags, fontdb},
    };

    use super::{
        PreparedGlyph, PreparedGlyphSource, PreparedTextAffine, PreparedTextError, TextGlyphPaint,
        TextPaintPlan, glyph_instance,
    };

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

    #[test]
    fn effective_opacity_includes_the_closed_mask_chain() {
        let mut paint = TextGlyphPaint::opaque(TextColor::default());
        paint.opacity_milli = 500;
        paint.masks.push(ResolvedFxMask {
            coverage: Opacity::try_new(FiniteF32::ZERO).expect("zero opacity"),
            invert: false,
        });

        assert!(paint.effective_opacity().abs() <= f32::EPSILON);
        assert_eq!(paint.effective_opacity_milli(), 0);
    }

    #[test]
    fn view_affine_transforms_origin_advance_shape_and_opacity_once() {
        let (cache_key, _, _) = CacheKey::new(
            fontdb::ID::dummy(),
            1,
            30.0,
            (0.0, 0.0),
            Weight::NORMAL,
            CacheKeyFlags::empty(),
        );
        let glyph = PreparedGlyph {
            source: PreparedGlyphSource::Body { glyph_index: 0 },
            origin: LayoutPoint::new(10.0, 20.0),
            advance: LayoutSize::new(4.0, 6.0),
            layout_bounds: LayoutRect::new(10.0, 20.0, 4.0, 6.0),
            ink_bounds: LayoutRect::new(9.0, 18.0, 7.0, 9.0),
            source_range: RichTextRange::new(0, 1),
            cluster_index: 0,
            orientation: GlyphOrientation::Upright,
            inline_scale: 1.0,
            cache_key,
        };
        let paint = TextGlyphPaint::opaque(TextColor::rgba(100, 120, 140, 200));
        let affine = PreparedTextAffine::try_new([2.0, 0.0, 0.0, 3.0, 5.0, 7.0], 0.5)
            .expect("finite affine");

        let instance = glyph_instance(0, &glyph, &paint, None, affine);

        assert_eq!(instance.origin, Point::new(25.0, 67.0));
        assert_eq!(instance.advance, Vector::new(8.0, 18.0));
        assert_eq!(instance.color, Some(Color::rgba(100, 120, 140, 100)));
        let GlyphTransform::Affine(transform) = instance.transform else {
            panic!("View scale must reach the glyph raster transform");
        };
        for (actual, expected) in transform
            .values
            .into_iter()
            .zip([2.0, 0.0, 0.0, 3.0, 0.0, 0.0])
        {
            assert!((actual - expected).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn view_affine_rejects_non_finite_geometry_and_out_of_range_opacity() {
        assert_eq!(
            PreparedTextAffine::try_new([f32::NAN, 0.0, 0.0, 1.0, 0.0, 0.0], 1.0),
            Err(PreparedTextError::InvalidSubmissionAffine)
        );
        assert!(matches!(
            PreparedTextAffine::try_new([1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 1.1),
            Err(PreparedTextError::InvalidSubmissionOpacity { .. })
        ));
    }

    #[test]
    fn view_shear_uses_the_same_matrix_for_origin_and_raster_shape() {
        let affine = PreparedTextAffine::try_new([2.0, 3.0, 5.0, 7.0, 11.0, 13.0], 1.0).unwrap();
        assert_eq!(
            affine.point(Point::new(10.0, 20.0)),
            Point::new(131.0, 183.0)
        );
        assert_eq!(
            affine.vector(Vector::new(4.0, 6.0)),
            Vector::new(38.0, 54.0)
        );
        assert_eq!(
            affine.linear_affine().values,
            [2.0, 5.0, 3.0, 7.0, 0.0, 0.0]
        );
    }
}
