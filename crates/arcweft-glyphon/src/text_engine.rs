//! Deterministic project-font shaping and glyph raster-key preparation.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_text_layout::{
    FontFaceId, FontInventoryHash, LayoutPoint, LayoutRect, LayoutSize, ShapedGlyphKey,
    ShapedTextGlyph, ShapedTextRun, TextShapeRequest, TextShaper,
};
use glyphon::cosmic_text::{CacheKeyFlags, Fallback, FeatureTag, FontFeatures, LineIter};
use glyphon::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
    fontdb,
};
use thiserror::Error;
use unicode_script::Script;

const SHAPING_FEATURE_RECORDS: [&[u8]; 3] = [
    b"cosmic-text-0.18.2",
    b"horizontal:liga,kern",
    b"vertical:vert,vrt2",
];

/// Hard bounds for the reusable shaped-run cache.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextShapeCacheLimits {
    /// Maximum number of shaped runs retained.
    pub max_entries: usize,
    /// Maximum total number of glyph records retained.
    pub max_glyphs: usize,
}

impl Default for TextShapeCacheLimits {
    fn default() -> Self {
        Self {
            max_entries: 2_048,
            max_glyphs: 262_144,
        }
    }
}

/// Observable cache counters used by parity and invalidation tests.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextShapeCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub invalidations: u64,
    pub entries: usize,
    pub glyphs: usize,
}

/// A renderer-local glyphon key prepared from an Arcweft-stable glyph key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GlyphRasterKey {
    pub cache_key: CacheKey,
    pub physical_x: i32,
    pub physical_y: i32,
}

/// Structured failure from project-font registration, shaping, or raster preparation.
#[derive(Debug, Error)]
pub enum GlyphonTextEngineError {
    #[error("project font inventory must contain at least one font resource")]
    EmptyFontInventory,
    #[error("project font resource {index} is empty")]
    EmptyFontResource { index: usize },
    #[error("project font resource {index} contains no decodable font face")]
    InvalidFontResource { index: usize },
    #[error("project font resource {index} repeats stable face {face:?}")]
    DuplicateFontFace { index: usize, face: FontFaceId },
    #[error("shape cache limits must both be greater than zero")]
    InvalidCacheLimits,
    #[error("shape source range has {source_len} bytes but text has {text_len} bytes")]
    SourceLengthMismatch { source_len: usize, text_len: usize },
    #[error("font size or line height is not finite and positive")]
    InvalidMetrics,
    #[error("letter or word spacing is not finite")]
    InvalidSpacing,
    #[error("word spacing makes cluster {start}..{end} have a negative advance")]
    NegativeWordAdvance { start: usize, end: usize },
    #[error("shaper selected a font face outside the project inventory")]
    UnknownProjectFace,
    #[error("font fallback produced missing glyph 0 for source range {start}..{end}")]
    MissingGlyph { start: usize, end: usize },
    #[error("glyph {glyph_id} for source range {start}..{end} could not be rasterized")]
    RasterizationFailed {
        glyph_id: u32,
        start: usize,
        end: usize,
    },
    #[error("shaper produced non-finite or negative glyph geometry")]
    InvalidGlyphGeometry,
    #[error("stable glyph key refers to an unknown project font face")]
    UnknownStableFace,
    #[error("stable glyph key contains unsupported glyph id {glyph_id}")]
    UnsupportedGlyphId { glyph_id: u32 },
    #[error("stable glyph key contains invalid font-size bits")]
    InvalidGlyphFontSize,
    #[error("stable glyph key contains unsupported cache flags {flags:#x}")]
    UnsupportedCacheFlags { flags: u32 },
}

/// Shared CPU text engine used by native, Web, and headless preparation.
///
/// Construction starts from an empty font database and registers only the
/// supplied project bytes. It therefore never performs system-font discovery.
#[derive(Debug)]
pub struct GlyphonTextEngine {
    font_system: FontSystem,
    swash_cache: SwashCache,
    shape_cache: TextShapeCache,
    face_ids: BTreeMap<fontdb::ID, FontFaceId>,
    database_ids: BTreeMap<FontFaceId, fontdb::ID>,
    ordered_faces: Vec<FontFaceId>,
    inventory_hash: FontInventoryHash,
    locale: String,
    font_resource_count: usize,
}

#[derive(Debug)]
struct TextShapeCache {
    limits: TextShapeCacheLimits,
    entries: BTreeMap<[u8; 32], CachedShape>,
    glyphs: usize,
    clock: u64,
    hits: u64,
    misses: u64,
    evictions: u64,
    invalidations: u64,
}

#[derive(Clone, Debug)]
struct CachedShape {
    run: ShapedTextRun,
    glyphs: usize,
    last_used: u64,
}

#[derive(Clone, Debug)]
struct PendingGlyph {
    key: ShapedGlyphKey,
    source_start: usize,
    source_end: usize,
    line_index: u32,
    origin_x: f32,
    origin_y: f32,
    advance: f32,
    ink_bounds: LayoutRect,
}

#[derive(Clone, Copy, Debug)]
struct LineGlyphContext<'a> {
    line: &'a str,
    line_source_start: usize,
    line_index: u32,
    prefix_len: usize,
    content_end: usize,
    baseline: f32,
}

#[derive(Clone, Copy, Debug)]
struct ProjectFontFallback;

impl Fallback for ProjectFontFallback {
    fn common_fallback(&self) -> &[&'static str] {
        &[]
    }

    fn forbidden_fallback(&self) -> &[&'static str] {
        &[]
    }

    fn script_fallback(&self, _script: Script, _locale: &str) -> &[&'static str] {
        &[]
    }
}

impl GlyphonTextEngine {
    /// Creates a conformant engine from exact, ordered project font bytes.
    pub fn from_project_fonts(
        locale: impl Into<String>,
        fonts: Vec<Vec<u8>>,
    ) -> Result<Self, GlyphonTextEngineError> {
        Self::with_cache_limits(locale, fonts, TextShapeCacheLimits::default())
    }

    /// Creates a conformant engine with explicit cache bounds.
    pub fn with_cache_limits(
        locale: impl Into<String>,
        fonts: Vec<Vec<u8>>,
        cache_limits: TextShapeCacheLimits,
    ) -> Result<Self, GlyphonTextEngineError> {
        if fonts.is_empty() {
            return Err(GlyphonTextEngineError::EmptyFontInventory);
        }
        let font_resource_count = fonts.len();
        let shape_cache = TextShapeCache::new(cache_limits)?;
        let mut database = fontdb::Database::new();
        let mut face_ids = BTreeMap::new();
        let mut database_ids = BTreeMap::new();
        let mut ordered_faces = Vec::new();

        for (index, bytes) in fonts.into_iter().enumerate() {
            register_font_bytes(
                &mut database,
                bytes,
                index,
                &mut face_ids,
                &mut database_ids,
                &mut ordered_faces,
            )?;
        }
        set_generic_families_to_first_project_face(&mut database)?;
        let inventory_hash =
            FontInventoryHash::derive(ordered_faces.iter().copied(), SHAPING_FEATURE_RECORDS);
        let locale = locale.into();
        let font_system = FontSystem::new_with_locale_and_db_and_fallback(
            locale.clone(),
            database,
            ProjectFontFallback,
        );

        Ok(Self {
            font_system,
            swash_cache: SwashCache::new(),
            shape_cache,
            face_ids,
            database_ids,
            ordered_faces,
            inventory_hash,
            locale,
            font_resource_count,
        })
    }

    /// Adds one canonical project font resource and invalidates shaped/raster caches.
    pub fn register_project_font(&mut self, bytes: Vec<u8>) -> Result<(), GlyphonTextEngineError> {
        let index = self.font_resource_count;
        register_font_bytes(
            self.font_system.db_mut(),
            bytes,
            index,
            &mut self.face_ids,
            &mut self.database_ids,
            &mut self.ordered_faces,
        )?;
        if index == 0 {
            set_generic_families_to_first_project_face(self.font_system.db_mut())?;
        }
        self.font_resource_count = self.font_resource_count.saturating_add(1);
        self.inventory_hash =
            FontInventoryHash::derive(self.ordered_faces.iter().copied(), SHAPING_FEATURE_RECORDS);
        self.shape_cache.invalidate();
        self.swash_cache.image_cache.clear();
        self.swash_cache.outline_command_cache.clear();
        Ok(())
    }

    /// Exact locale fixed for deterministic fallback and shaping cache identity.
    #[must_use]
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// Ordered stable faces derived from the exact project bytes.
    #[must_use]
    pub fn ordered_faces(&self) -> &[FontFaceId] {
        &self.ordered_faces
    }

    /// Current bounded-cache counters.
    #[must_use]
    pub fn cache_stats(&self) -> TextShapeCacheStats {
        self.shape_cache.stats()
    }

    /// Renderer-owned font system containing only registered project fonts.
    pub fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }

    /// Raster cache paired with the engine font system.
    pub fn swash_cache_mut(&mut self) -> &mut SwashCache {
        &mut self.swash_cache
    }

    /// Resolves an Arcweft-stable key to a renderer-local glyphon cache key.
    pub fn prepare_raster_key(
        &self,
        key: ShapedGlyphKey,
        position: LayoutPoint,
    ) -> Result<GlyphRasterKey, GlyphonTextEngineError> {
        let font_id = self
            .database_ids
            .get(&key.face)
            .copied()
            .ok_or(GlyphonTextEngineError::UnknownStableFace)?;
        let glyph_id = u16::try_from(key.glyph_id).map_err(|_| {
            GlyphonTextEngineError::UnsupportedGlyphId {
                glyph_id: key.glyph_id,
            }
        })?;
        let font_size = f32::from_bits(key.font_size_bits);
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err(GlyphonTextEngineError::InvalidGlyphFontSize);
        }
        let flags = CacheKeyFlags::from_bits(key.flags)
            .ok_or(GlyphonTextEngineError::UnsupportedCacheFlags { flags: key.flags })?;
        let (cache_key, physical_x, physical_y) = CacheKey::new(
            font_id,
            glyph_id,
            font_size,
            (position.x, position.y),
            Weight(key.font_weight),
            flags,
        );
        Ok(GlyphRasterKey {
            cache_key,
            physical_x,
            physical_y,
        })
    }

    fn shape_uncached(
        &mut self,
        request: TextShapeRequest<'_>,
    ) -> Result<ShapedTextRun, GlyphonTextEngineError> {
        let font_size = milli_u32_to_f32(request.style.font_size_milli())
            .ok_or(GlyphonTextEngineError::InvalidMetrics)?;
        let line_height = milli_u32_to_f32(request.style.line_height_milli())
            .ok_or(GlyphonTextEngineError::InvalidMetrics)?;
        if !font_size.is_finite()
            || font_size <= 0.0
            || !line_height.is_finite()
            || line_height <= 0.0
        {
            return Err(GlyphonTextEngineError::InvalidMetrics);
        }
        let letter_spacing = milli_i32_to_f32(request.style.letter_spacing_milli())
            .ok_or(GlyphonTextEngineError::InvalidSpacing)?;
        let word_spacing = milli_i32_to_f32(request.style.word_spacing_milli())
            .ok_or(GlyphonTextEngineError::InvalidSpacing)?;
        if !letter_spacing.is_finite() || !word_spacing.is_finite() {
            return Err(GlyphonTextEngineError::InvalidSpacing);
        }

        let family = selected_family(request.style.font_families(), self.font_system.db());
        let mut features = FontFeatures::new();
        if !matches!(
            request.writing_mode,
            arcweft_render_text::RichTextWritingMode::HorizontalTb
        ) {
            features.enable(FeatureTag::new(b"vert"));
            features.enable(FeatureTag::new(b"vrt2"));
        }
        let attrs = Attrs::new()
            .family(family)
            .weight(text_weight(request.style.weight()))
            .style(text_style(request.style.slant()))
            .letter_spacing(letter_spacing / font_size)
            .font_features(features);

        let mut pending = Vec::new();
        let mut max_advance = 0.0_f32;
        let mut total_height = 0.0_f32;
        for (line_index, (line_range, _ending)) in LineIter::new(request.text).enumerate() {
            let line = &request.text[line_range.clone()];
            let mut line_glyphs = self.shape_line(
                line,
                line_range.start,
                u32::try_from(line_index).unwrap_or(u32::MAX),
                font_size,
                line_height,
                word_spacing,
                request.direction,
                &attrs,
            )?;
            let line_advance = line_glyphs.iter().map(|glyph| glyph.advance).sum::<f32>();
            max_advance = max_advance.max(line_advance);
            total_height += line_height;
            pending.append(&mut line_glyphs);
        }

        let cluster_ids = logical_cluster_ids(&pending);
        let mut glyphs = Vec::with_capacity(pending.len());
        let mut ink_bounds = None;
        let mut line_cursor = BTreeMap::<u32, f32>::new();
        for glyph in pending {
            let cursor = line_cursor.entry(glyph.line_index).or_default();
            let offset = LayoutPoint::new(glyph.origin_x - *cursor, glyph.origin_y);
            *cursor += glyph.advance;
            let cluster_index = cluster_ids
                .get(&(glyph.line_index, glyph.source_start, glyph.source_end))
                .copied()
                .unwrap_or(u32::MAX);
            let shaped = ShapedTextGlyph {
                key: glyph.key,
                source_range: arcweft_render_text::RichTextRange::new(
                    glyph.source_start,
                    glyph.source_end,
                ),
                line_index: glyph.line_index,
                cluster_index,
                offset,
                advance: LayoutSize::new(glyph.advance, 0.0),
                ink_bounds: glyph.ink_bounds,
            };
            let absolute_ink = LayoutRect::new(
                shaped.offset.x + shaped.ink_bounds.x,
                f32::from(u16::try_from(shaped.line_index).unwrap_or(u16::MAX)) * line_height
                    + shaped.offset.y
                    + shaped.ink_bounds.y,
                shaped.ink_bounds.width,
                shaped.ink_bounds.height,
            );
            ink_bounds = Some(ink_bounds.map_or(absolute_ink, |bounds: LayoutRect| {
                bounds.union(absolute_ink)
            }));
            glyphs.push(shaped);
        }
        Ok(ShapedTextRun::new(
            glyphs,
            LayoutSize::new(max_advance, total_height),
            ink_bounds,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn shape_line(
        &mut self,
        line: &str,
        line_source_start: usize,
        line_index: u32,
        font_size: f32,
        line_height: f32,
        word_spacing: f32,
        direction: arcweft_render_text::RichTextInlineDirection,
        attrs: &Attrs<'_>,
    ) -> Result<Vec<PendingGlyph>, GlyphonTextEngineError> {
        if line.is_empty() {
            return Ok(Vec::new());
        }
        let (shaping_text, prefix_len) = text_with_explicit_direction(line, direction);
        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(font_size, line_height));
        buffer.set_wrap(&mut self.font_system, Wrap::None);
        buffer.set_text(
            &mut self.font_system,
            &shaping_text,
            attrs,
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        let content_end = prefix_len + line.len();
        let mut out = Vec::new();
        for run in buffer.layout_runs() {
            let context = LineGlyphContext {
                line,
                line_source_start,
                line_index,
                prefix_len,
                content_end,
                baseline: run.line_y,
            };
            for glyph in run.glyphs {
                if let Some(pending) = self.pending_glyph(glyph, context)? {
                    out.push(pending);
                }
            }
        }
        apply_word_spacing(&mut out, line, line_source_start, word_spacing)?;
        Ok(out)
    }

    fn pending_glyph(
        &mut self,
        glyph: &glyphon::LayoutGlyph,
        context: LineGlyphContext<'_>,
    ) -> Result<Option<PendingGlyph>, GlyphonTextEngineError> {
        let mapped_start = glyph.start.max(context.prefix_len);
        let mapped_end = glyph.end.min(context.content_end);
        if mapped_start >= mapped_end {
            return Ok(None);
        }
        let local_start = mapped_start - context.prefix_len;
        let local_end = mapped_end - context.prefix_len;
        let source_start = context.line_source_start + local_start;
        let source_end = context.line_source_start + local_end;
        if glyph.glyph_id == 0 {
            return Err(GlyphonTextEngineError::MissingGlyph {
                start: source_start,
                end: source_end,
            });
        }
        let face = self
            .face_ids
            .get(&glyph.font_id)
            .copied()
            .ok_or(GlyphonTextEngineError::UnknownProjectFace)?;
        let logical_x = glyph.x + glyph.font_size * glyph.x_offset;
        let logical_y = context.baseline + glyph.y - glyph.font_size * glyph.y_offset;
        let physical = glyph.physical((0.0, context.baseline), 1.0);
        let image = self
            .swash_cache
            .get_image(&mut self.font_system, physical.cache_key);
        let ink_bounds = if let Some(image) = image {
            let physical_x =
                exact_pixel_i32(physical.x).ok_or(GlyphonTextEngineError::InvalidGlyphGeometry)?;
            let physical_y =
                exact_pixel_i32(physical.y).ok_or(GlyphonTextEngineError::InvalidGlyphGeometry)?;
            let image_left = exact_pixel_i32(image.placement.left)
                .ok_or(GlyphonTextEngineError::InvalidGlyphGeometry)?;
            let image_top = exact_pixel_i32(image.placement.top)
                .ok_or(GlyphonTextEngineError::InvalidGlyphGeometry)?;
            let image_width = exact_pixel_u32(image.placement.width)
                .ok_or(GlyphonTextEngineError::InvalidGlyphGeometry)?;
            let image_height = exact_pixel_u32(image.placement.height)
                .ok_or(GlyphonTextEngineError::InvalidGlyphGeometry)?;
            LayoutRect::new(
                physical_x + image_left - logical_x,
                physical_y - image_top - logical_y,
                image_width,
                image_height,
            )
        } else if context.line[local_start..local_end]
            .chars()
            .all(char::is_whitespace)
        {
            LayoutRect::new(0.0, 0.0, 0.0, 0.0)
        } else {
            return Err(GlyphonTextEngineError::RasterizationFailed {
                glyph_id: u32::from(glyph.glyph_id),
                start: source_start,
                end: source_end,
            });
        };
        let values = [
            logical_x,
            logical_y,
            glyph.w,
            ink_bounds.x,
            ink_bounds.y,
            ink_bounds.width,
            ink_bounds.height,
        ];
        if values.iter().any(|value| !value.is_finite())
            || glyph.w < 0.0
            || ink_bounds.width < 0.0
            || ink_bounds.height < 0.0
        {
            return Err(GlyphonTextEngineError::InvalidGlyphGeometry);
        }
        Ok(Some(PendingGlyph {
            key: ShapedGlyphKey {
                face,
                glyph_id: u32::from(glyph.glyph_id),
                font_size_bits: glyph.font_size.to_bits(),
                font_weight: glyph.font_weight.0,
                flags: glyph.cache_key_flags.bits(),
            },
            source_start,
            source_end,
            line_index: context.line_index,
            origin_x: logical_x,
            origin_y: logical_y,
            advance: glyph.w,
            ink_bounds,
        }))
    }
}

impl TextShaper for GlyphonTextEngine {
    type Error = GlyphonTextEngineError;

    fn font_inventory_hash(&self) -> FontInventoryHash {
        self.inventory_hash
    }

    fn shape_run(&mut self, request: TextShapeRequest<'_>) -> Result<ShapedTextRun, Self::Error> {
        let source_len = request
            .source_range
            .end
            .checked_sub(request.source_range.start)
            .unwrap_or(usize::MAX);
        if source_len != request.text.len() {
            return Err(GlyphonTextEngineError::SourceLengthMismatch {
                source_len,
                text_len: request.text.len(),
            });
        }
        let key = shape_cache_key(self.inventory_hash, &self.locale, request);
        let relative = if let Some(cached) = self.shape_cache.get(key) {
            cached
        } else {
            let shaped = self.shape_uncached(TextShapeRequest {
                source_range: arcweft_render_text::RichTextRange::new(0, request.text.len()),
                ..request
            })?;
            self.shape_cache.insert(key, shaped.clone());
            shaped
        };
        Ok(rebase_run(&relative, request.source_range.start))
    }
}

impl TextShapeCache {
    fn new(limits: TextShapeCacheLimits) -> Result<Self, GlyphonTextEngineError> {
        if limits.max_entries == 0 || limits.max_glyphs == 0 {
            return Err(GlyphonTextEngineError::InvalidCacheLimits);
        }
        Ok(Self {
            limits,
            entries: BTreeMap::new(),
            glyphs: 0,
            clock: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            invalidations: 0,
        })
    }

    fn get(&mut self, key: [u8; 32]) -> Option<ShapedTextRun> {
        self.clock = self.clock.saturating_add(1);
        if let Some(entry) = self.entries.get_mut(&key) {
            self.hits = self.hits.saturating_add(1);
            entry.last_used = self.clock;
            return Some(entry.run.clone());
        }
        self.misses = self.misses.saturating_add(1);
        None
    }

    fn insert(&mut self, key: [u8; 32], run: ShapedTextRun) {
        let glyphs = run.glyphs().len();
        if glyphs > self.limits.max_glyphs {
            return;
        }
        if let Some(previous) = self.entries.remove(&key) {
            self.glyphs = self.glyphs.saturating_sub(previous.glyphs);
        }
        self.clock = self.clock.saturating_add(1);
        self.glyphs = self.glyphs.saturating_add(glyphs);
        self.entries.insert(
            key,
            CachedShape {
                run,
                glyphs,
                last_used: self.clock,
            },
        );
        while self.entries.len() > self.limits.max_entries || self.glyphs > self.limits.max_glyphs {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(entry_key, entry)| (entry.last_used, **entry_key))
                .map(|(entry_key, _)| *entry_key)
            else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.glyphs = self.glyphs.saturating_sub(removed.glyphs);
                self.evictions = self.evictions.saturating_add(1);
            }
        }
    }

    fn invalidate(&mut self) {
        self.entries.clear();
        self.glyphs = 0;
        self.invalidations = self.invalidations.saturating_add(1);
    }

    fn stats(&self) -> TextShapeCacheStats {
        TextShapeCacheStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            invalidations: self.invalidations,
            entries: self.entries.len(),
            glyphs: self.glyphs,
        }
    }
}

fn register_font_bytes(
    database: &mut fontdb::Database,
    bytes: Vec<u8>,
    index: usize,
    face_ids: &mut BTreeMap<fontdb::ID, FontFaceId>,
    database_ids: &mut BTreeMap<FontFaceId, fontdb::ID>,
    ordered_faces: &mut Vec<FontFaceId>,
) -> Result<(), GlyphonTextEngineError> {
    if bytes.is_empty() {
        return Err(GlyphonTextEngineError::EmptyFontResource { index });
    }
    let bytes = Arc::new(bytes);
    let source: Arc<dyn AsRef<[u8]> + Send + Sync> = bytes.clone();
    let ids = database.load_font_source(fontdb::Source::Binary(source));
    if ids.is_empty() {
        return Err(GlyphonTextEngineError::InvalidFontResource { index });
    }
    for database_id in ids {
        let face_index = database
            .face(database_id)
            .map(|face| face.index)
            .ok_or(GlyphonTextEngineError::InvalidFontResource { index })?;
        let stable_id = FontFaceId::derive(bytes.as_slice(), face_index, &[]);
        if database_ids.contains_key(&stable_id) {
            return Err(GlyphonTextEngineError::DuplicateFontFace {
                index,
                face: stable_id,
            });
        }
        face_ids.insert(database_id, stable_id);
        database_ids.insert(stable_id, database_id);
        ordered_faces.push(stable_id);
    }
    Ok(())
}

fn set_generic_families_to_first_project_face(
    database: &mut fontdb::Database,
) -> Result<(), GlyphonTextEngineError> {
    let family = database
        .faces()
        .next()
        .and_then(|face| face.families.first())
        .map(|family| family.0.clone())
        .ok_or(GlyphonTextEngineError::EmptyFontInventory)?;
    database.set_serif_family(family.clone());
    database.set_sans_serif_family(family.clone());
    database.set_monospace_family(family.clone());
    database.set_cursive_family(family.clone());
    database.set_fantasy_family(family);
    Ok(())
}

fn selected_family<'a>(
    families: &'a [arcweft_render_text::TextFontFamily],
    database: &fontdb::Database,
) -> Family<'a> {
    for family in families {
        match family {
            arcweft_render_text::TextFontFamily::Named(name)
                if database
                    .faces()
                    .any(|face| face.families.iter().any(|family| family.0 == *name)) =>
            {
                return Family::Name(name);
            }
            arcweft_render_text::TextFontFamily::Serif => return Family::Serif,
            arcweft_render_text::TextFontFamily::SansSerif => return Family::SansSerif,
            arcweft_render_text::TextFontFamily::Monospace => return Family::Monospace,
            arcweft_render_text::TextFontFamily::Cursive => return Family::Cursive,
            arcweft_render_text::TextFontFamily::Fantasy => return Family::Fantasy,
            arcweft_render_text::TextFontFamily::Named(_) => {}
        }
    }
    Family::SansSerif
}

fn text_weight(weight: arcweft_render_text::TextWeight) -> Weight {
    match weight {
        arcweft_render_text::TextWeight::Thin => Weight::THIN,
        arcweft_render_text::TextWeight::ExtraLight => Weight::EXTRA_LIGHT,
        arcweft_render_text::TextWeight::Light => Weight::LIGHT,
        arcweft_render_text::TextWeight::Normal => Weight::NORMAL,
        arcweft_render_text::TextWeight::Medium => Weight::MEDIUM,
        arcweft_render_text::TextWeight::SemiBold => Weight::SEMIBOLD,
        arcweft_render_text::TextWeight::Bold => Weight::BOLD,
        arcweft_render_text::TextWeight::ExtraBold => Weight::EXTRA_BOLD,
        arcweft_render_text::TextWeight::Black => Weight::BLACK,
    }
}

fn text_style(slant: arcweft_render_text::TextSlant) -> Style {
    match slant {
        arcweft_render_text::TextSlant::Upright => Style::Normal,
        arcweft_render_text::TextSlant::Italic => Style::Italic,
        arcweft_render_text::TextSlant::Oblique { .. } => Style::Oblique,
    }
}

fn text_with_explicit_direction(
    text: &str,
    direction: arcweft_render_text::RichTextInlineDirection,
) -> (String, usize) {
    let control = match direction {
        arcweft_render_text::RichTextInlineDirection::Auto => return (text.to_owned(), 0),
        arcweft_render_text::RichTextInlineDirection::Ltr => '\u{202a}',
        arcweft_render_text::RichTextInlineDirection::Rtl => '\u{202b}',
    };
    let mut wrapped = String::with_capacity(text.len() + 6);
    wrapped.push(control);
    let prefix_len = wrapped.len();
    wrapped.push_str(text);
    wrapped.push('\u{202c}');
    (wrapped, prefix_len)
}

fn apply_word_spacing(
    glyphs: &mut [PendingGlyph],
    line: &str,
    line_source_start: usize,
    word_spacing: f32,
) -> Result<(), GlyphonTextEngineError> {
    if word_spacing == 0.0 || glyphs.is_empty() {
        return Ok(());
    }
    let mut extra_before = 0.0_f32;
    let mut start = 0;
    while start < glyphs.len() {
        let source = (glyphs[start].source_start, glyphs[start].source_end);
        let end = glyphs[start + 1..]
            .iter()
            .position(|glyph| (glyph.source_start, glyph.source_end) != source)
            .map_or(glyphs.len(), |offset| start + 1 + offset);
        for glyph in &mut glyphs[start..end] {
            glyph.origin_x += extra_before;
        }
        let local_start = source.0.saturating_sub(line_source_start);
        let local_end = source.1.saturating_sub(line_source_start);
        let is_space = line
            .get(local_start..local_end)
            .is_some_and(|cluster| cluster.chars().all(char::is_whitespace));
        if is_space {
            let cluster_advance = glyphs[start..end]
                .iter()
                .map(|glyph| glyph.advance)
                .sum::<f32>();
            if cluster_advance + word_spacing < 0.0 {
                return Err(GlyphonTextEngineError::NegativeWordAdvance {
                    start: source.0,
                    end: source.1,
                });
            }
            if let Some(last) = glyphs.get_mut(end.saturating_sub(1)) {
                last.advance += word_spacing;
            }
            extra_before += word_spacing;
        }
        start = end;
    }
    Ok(())
}

fn logical_cluster_ids(glyphs: &[PendingGlyph]) -> BTreeMap<(u32, usize, usize), u32> {
    let ranges = glyphs
        .iter()
        .map(|glyph| (glyph.line_index, glyph.source_start, glyph.source_end))
        .collect::<BTreeSet<_>>();
    ranges
        .into_iter()
        .enumerate()
        .map(|(index, range)| (range, u32::try_from(index).unwrap_or(u32::MAX)))
        .collect()
}

fn rebase_run(run: &ShapedTextRun, source_start: usize) -> ShapedTextRun {
    let glyphs = run
        .glyphs()
        .iter()
        .cloned()
        .map(|mut glyph| {
            glyph.source_range.start = glyph.source_range.start.saturating_add(source_start);
            glyph.source_range.end = glyph.source_range.end.saturating_add(source_start);
            glyph
        })
        .collect();
    ShapedTextRun::new(glyphs, run.advance(), run.ink_bounds())
}

fn shape_cache_key(
    inventory: FontInventoryHash,
    engine_locale: &str,
    request: TextShapeRequest<'_>,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.glyphon-shape.v1\0");
    hasher.update(&inventory.as_bytes());
    put_bytes(&mut hasher, engine_locale.as_bytes());
    put_bytes(&mut hasher, request.text.as_bytes());
    hasher.update(&request.style.font_size_milli().to_le_bytes());
    hasher.update(&request.style.line_height_milli().to_le_bytes());
    hasher.update(&[weight_tag(request.style.weight())]);
    match request.style.slant() {
        arcweft_render_text::TextSlant::Upright => {
            hasher.update(&[0]);
        }
        arcweft_render_text::TextSlant::Italic => {
            hasher.update(&[1]);
        }
        arcweft_render_text::TextSlant::Oblique { angle } => {
            hasher.update(&[2]);
            hasher.update(&angle.degrees.0.to_le_bytes());
        }
    }
    hasher.update(&request.style.letter_spacing_milli().to_le_bytes());
    hasher.update(&request.style.word_spacing_milli().to_le_bytes());
    hasher.update(&[writing_mode_tag(request.writing_mode)]);
    hasher.update(&[direction_tag(request.direction)]);
    hasher.update(
        &u32::try_from(request.style.font_families().len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for family in request.style.font_families() {
        match family {
            arcweft_render_text::TextFontFamily::Serif => {
                hasher.update(&[0]);
            }
            arcweft_render_text::TextFontFamily::SansSerif => {
                hasher.update(&[1]);
            }
            arcweft_render_text::TextFontFamily::Monospace => {
                hasher.update(&[2]);
            }
            arcweft_render_text::TextFontFamily::Cursive => {
                hasher.update(&[3]);
            }
            arcweft_render_text::TextFontFamily::Fantasy => {
                hasher.update(&[4]);
            }
            arcweft_render_text::TextFontFamily::Named(name) => {
                hasher.update(&[5]);
                put_bytes(&mut hasher, name.as_bytes());
            }
        }
    }
    if let Some(locale) = request.locale {
        hasher.update(&[1]);
        put_bytes(&mut hasher, locale.as_str().as_bytes());
    } else {
        hasher.update(&[0]);
    }
    *hasher.finalize().as_bytes()
}

fn weight_tag(value: arcweft_render_text::TextWeight) -> u8 {
    match value {
        arcweft_render_text::TextWeight::Thin => 0,
        arcweft_render_text::TextWeight::ExtraLight => 1,
        arcweft_render_text::TextWeight::Light => 2,
        arcweft_render_text::TextWeight::Normal => 3,
        arcweft_render_text::TextWeight::Medium => 4,
        arcweft_render_text::TextWeight::SemiBold => 5,
        arcweft_render_text::TextWeight::Bold => 6,
        arcweft_render_text::TextWeight::ExtraBold => 7,
        arcweft_render_text::TextWeight::Black => 8,
    }
}

fn writing_mode_tag(value: arcweft_render_text::RichTextWritingMode) -> u8 {
    match value {
        arcweft_render_text::RichTextWritingMode::HorizontalTb => 0,
        arcweft_render_text::RichTextWritingMode::VerticalRl => 1,
        arcweft_render_text::RichTextWritingMode::VerticalLr => 2,
    }
}

fn direction_tag(value: arcweft_render_text::RichTextInlineDirection) -> u8 {
    match value {
        arcweft_render_text::RichTextInlineDirection::Auto => 0,
        arcweft_render_text::RichTextInlineDirection::Ltr => 1,
        arcweft_render_text::RichTextInlineDirection::Rtl => 2,
    }
}

fn put_bytes(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(value);
}

fn milli_u32_to_f32(value: u32) -> Option<f32> {
    let whole = u16::try_from(value / 1_000).ok()?;
    let fractional = u16::try_from(value % 1_000).ok()?;
    Some(f32::from(whole) + f32::from(fractional) / 1_000.0)
}

fn milli_i32_to_f32(value: i32) -> Option<f32> {
    let whole = i16::try_from(value / 1_000).ok()?;
    let fractional = i16::try_from(value % 1_000).ok()?;
    Some(f32::from(whole) + f32::from(fractional) / 1_000.0)
}

fn exact_pixel_i32(value: i32) -> Option<f32> {
    const MAX_EXACT_INTEGER: i32 = 1 << 24;
    if (-MAX_EXACT_INTEGER..=MAX_EXACT_INTEGER).contains(&value) {
        #[allow(clippy::cast_precision_loss)]
        Some(value as f32)
    } else {
        None
    }
}

fn exact_pixel_u32(value: u32) -> Option<f32> {
    const MAX_EXACT_INTEGER: u32 = 1 << 24;
    if value <= MAX_EXACT_INTEGER {
        #[allow(clippy::cast_precision_loss)]
        Some(value as f32)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{GlyphonTextEngine, GlyphonTextEngineError, TextShapeCacheLimits};
    use arcweft_render_text::{
        ResolvedTextStyle, RichTextInlineDirection, RichTextRange, RichTextWritingMode, TextColor,
        TextFontFamily,
    };
    use arcweft_text_layout::{TextShapeRequest, TextShaper};

    const TEST_FONT: &[u8] = include_bytes!("../../../vendor/glyphon/examples/Inter-Bold.ttf");

    fn style() -> ResolvedTextStyle {
        ResolvedTextStyle::new(vec![TextFontFamily::SansSerif], 20_000, 26_000)
            .expect("test style is valid")
    }

    fn request<'a>(
        text: &'a str,
        source_start: usize,
        style: &'a ResolvedTextStyle,
    ) -> TextShapeRequest<'a> {
        TextShapeRequest {
            text,
            source_range: RichTextRange::new(source_start, source_start + text.len()),
            style,
            locale: None,
            direction: RichTextInlineDirection::Auto,
            writing_mode: RichTextWritingMode::HorizontalTb,
        }
    }

    #[test]
    fn conformant_engine_rejects_empty_and_invalid_font_inventory() {
        assert!(matches!(
            GlyphonTextEngine::from_project_fonts("en-US", Vec::new()),
            Err(GlyphonTextEngineError::EmptyFontInventory)
        ));
        assert!(matches!(
            GlyphonTextEngine::from_project_fonts("en-US", vec![b"not a font".to_vec()]),
            Err(GlyphonTextEngineError::InvalidFontResource { index: 0 })
        ));
    }

    #[test]
    fn ligatures_and_combining_marks_use_shaped_clusters_and_real_ink() {
        let mut engine = GlyphonTextEngine::from_project_fonts("en-US", vec![TEST_FONT.to_vec()])
            .expect("test font loads");
        let style = style();
        let text = "office e\u{301}";
        let shaped = engine
            .shape_run(request(text, 0, &style))
            .expect("text shapes");

        assert!(shaped.glyphs().len() < text.chars().count());
        assert!(
            shaped
                .glyphs()
                .iter()
                .any(|glyph| glyph.source_range.end - glyph.source_range.start > 1)
        );
        assert!(
            shaped
                .glyphs()
                .iter()
                .any(|glyph| { glyph.ink_bounds.width > 0.0 && glyph.ink_bounds.height > 0.0 })
        );
    }

    #[test]
    fn cache_rebases_source_ranges_and_excludes_paint_color() {
        let mut engine = GlyphonTextEngine::from_project_fonts("en-US", vec![TEST_FONT.to_vec()])
            .expect("test font loads");
        let pale = style().with_color(TextColor::rgba(245, 245, 245, 255));
        let red = style().with_color(TextColor::rgba(255, 0, 0, 255));

        let first = engine
            .shape_run(request("cache", 0, &pale))
            .expect("first shape succeeds");
        let second = engine
            .shape_run(request("cache", 40, &red))
            .expect("cached shape succeeds");

        assert_eq!(engine.cache_stats().misses, 1);
        assert_eq!(engine.cache_stats().hits, 1);
        assert_eq!(first.glyphs().len(), second.glyphs().len());
        assert!(
            second
                .glyphs()
                .iter()
                .all(|glyph| glyph.source_range.start >= 40)
        );
    }

    #[test]
    fn cache_is_bounded_and_flow_changes_miss() {
        let mut engine = GlyphonTextEngine::with_cache_limits(
            "en-US",
            vec![TEST_FONT.to_vec()],
            TextShapeCacheLimits {
                max_entries: 1,
                max_glyphs: 100,
            },
        )
        .expect("test font loads");
        let horizontal = style();
        let vertical = style().with_flow(
            RichTextWritingMode::VerticalRl,
            RichTextInlineDirection::Auto,
        );

        engine
            .shape_run(request("one", 0, &horizontal))
            .expect("horizontal shape succeeds");
        engine
            .shape_run(TextShapeRequest {
                writing_mode: RichTextWritingMode::VerticalRl,
                ..request("one", 0, &vertical)
            })
            .expect("vertical shape succeeds");

        assert_eq!(engine.cache_stats().entries, 1);
        assert_eq!(engine.cache_stats().misses, 2);
        assert_eq!(engine.cache_stats().evictions, 1);
    }

    #[test]
    fn negative_word_spacing_never_silently_clamps() {
        let mut engine = GlyphonTextEngine::from_project_fonts("en-US", vec![TEST_FONT.to_vec()])
            .expect("test font loads");
        let style = style().with_spacing(0, -100_000);
        let error = engine
            .shape_run(request("a b", 0, &style))
            .expect_err("negative whitespace advance must fail");

        assert!(matches!(
            error,
            GlyphonTextEngineError::NegativeWordAdvance { .. }
        ));
    }
}
