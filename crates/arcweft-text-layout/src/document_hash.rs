//! Stable identity for completed font-shaped document layout.

use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextStyle, TextFontFamily, TextSlant, TextWeight,
};
use arcweft_text_model::{RichTextInlineDirection, RichTextRange, RichTextWritingMode};

use crate::{
    FontInventoryHash, GlyphOrientation, GlyphVerticalForm, HorizontalWrap, JlreqStrictness,
    LayoutPoint, LayoutRect, LayoutSize, TextLayoutGlyph, TextLayoutHash, TextLayoutRequest,
    TextLayoutRuby,
};

pub(crate) fn layout_hash(
    document: &ResolvedTextDocument<'_>,
    request: TextLayoutRequest,
    inventory: FontInventoryHash,
    glyphs: &[TextLayoutGlyph],
    ruby: &[TextLayoutRuby],
) -> TextLayoutHash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.text-layout.v1\0");
    put_bytes(&mut hasher, document.text().as_bytes());
    hasher.update(&document.revision().get().to_le_bytes());
    hasher.update(&inventory.as_bytes());
    hash_point(&mut hasher, request.origin);
    hash_size(&mut hasher, request.size);
    hasher.update(&[horizontal_wrap_tag(request.horizontal_wrap)]);
    hasher.update(&[writing_mode_tag(request.default_writing_mode)]);
    hasher.update(&[jlreq_strictness_tag(request.jlreq_strictness)]);
    put_bytes(
        &mut hasher,
        request.vertical_break_policy.stable_id().as_bytes(),
    );
    for run in document.runs() {
        hash_range(&mut hasher, run.source_range());
        hash_layout_style(&mut hasher, run.style());
    }
    for glyph in glyphs {
        hash_glyph(&mut hasher, glyph);
    }
    for annotation in ruby {
        hasher.update(&annotation.ruby_index.to_le_bytes());
        hash_range(&mut hasher, annotation.base_range);
        put_bytes(&mut hasher, annotation.text.as_bytes());
        hash_rect(&mut hasher, annotation.base_bounds);
        hash_rect(&mut hasher, annotation.ruby_bounds);
        hasher.update(&[writing_mode_tag(annotation.writing_mode)]);
        hash_layout_style(&mut hasher, &annotation.style);
        for glyph in &annotation.glyphs {
            hash_range(&mut hasher, glyph.text_range);
            hasher.update(&glyph.cluster_index.to_le_bytes());
            hash_point(&mut hasher, glyph.origin);
            hash_size(&mut hasher, glyph.advance);
            hash_rect(&mut hasher, glyph.layout_bounds);
            hash_rect(&mut hasher, glyph.ink_bounds);
            hasher.update(&[orientation_tag(glyph.orientation)]);
            hasher.update(&glyph.inline_scale.to_bits().to_le_bytes());
            hash_shape_key(&mut hasher, glyph.shape_key);
        }
    }
    TextLayoutHash::from_bytes(*hasher.finalize().as_bytes())
}

fn hash_glyph(hasher: &mut blake3::Hasher, glyph: &TextLayoutGlyph) {
    hasher.update(&glyph.run_index.to_le_bytes());
    hash_range(hasher, glyph.source_range);
    hasher.update(&glyph.line_index.to_le_bytes());
    hasher.update(&glyph.cluster_index.to_le_bytes());
    hasher.update(&glyph.logical_ordinal.to_le_bytes());
    hash_point(hasher, glyph.origin);
    hash_size(hasher, glyph.advance);
    hash_rect(hasher, glyph.layout_bounds);
    hash_rect(hasher, glyph.ink_bounds);
    hasher.update(&[orientation_tag(glyph.orientation)]);
    hasher.update(&[vertical_form_tag(glyph.vertical_form)]);
    hasher.update(&glyph.inline_scale.to_bits().to_le_bytes());
    hash_shape_key(hasher, glyph.shape_key);
}

fn hash_shape_key(hasher: &mut blake3::Hasher, key: crate::ShapedGlyphKey) {
    hasher.update(&key.face.as_bytes());
    hasher.update(&key.glyph_id.to_le_bytes());
    hasher.update(&key.font_size_bits.to_le_bytes());
    hasher.update(&key.font_weight.to_le_bytes());
    hasher.update(&key.flags.to_le_bytes());
}

fn hash_layout_style(hasher: &mut blake3::Hasher, style: &ResolvedTextStyle) {
    hasher.update(&saturating_u32(style.font_families().len()).to_le_bytes());
    for family in style.font_families() {
        match family {
            TextFontFamily::Serif => {
                hasher.update(&[0]);
            }
            TextFontFamily::SansSerif => {
                hasher.update(&[1]);
            }
            TextFontFamily::Monospace => {
                hasher.update(&[2]);
            }
            TextFontFamily::Cursive => {
                hasher.update(&[3]);
            }
            TextFontFamily::Fantasy => {
                hasher.update(&[4]);
            }
            TextFontFamily::Named(name) => {
                hasher.update(&[5]);
                put_bytes(hasher, name.as_bytes());
            }
        }
    }
    hasher.update(&style.font_size_milli().to_le_bytes());
    hasher.update(&style.line_height_milli().to_le_bytes());
    hasher.update(&[weight_tag(style.weight())]);
    match style.slant() {
        TextSlant::Upright => {
            hasher.update(&[0]);
        }
        TextSlant::Italic => {
            hasher.update(&[1]);
        }
        TextSlant::Oblique { angle } => {
            hasher.update(&[2]);
            hasher.update(&angle.degrees.0.to_le_bytes());
        }
    }
    hasher.update(&style.letter_spacing_milli().to_le_bytes());
    hasher.update(&style.word_spacing_milli().to_le_bytes());
    hasher.update(&[writing_mode_tag(style.writing_mode())]);
    hasher.update(&[direction_tag(style.direction())]);
    if let Some(language) = style.language() {
        hasher.update(&[1]);
        put_bytes(hasher, language.as_str().as_bytes());
    } else {
        hasher.update(&[0]);
    }
}

fn hash_point(hasher: &mut blake3::Hasher, point: LayoutPoint) {
    hasher.update(&point.x.to_bits().to_le_bytes());
    hasher.update(&point.y.to_bits().to_le_bytes());
}

fn hash_size(hasher: &mut blake3::Hasher, size: LayoutSize) {
    hasher.update(&size.width.to_bits().to_le_bytes());
    hasher.update(&size.height.to_bits().to_le_bytes());
}

fn hash_rect(hasher: &mut blake3::Hasher, rect: LayoutRect) {
    for value in [rect.x, rect.y, rect.width, rect.height] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
}

fn hash_range(hasher: &mut blake3::Hasher, range: RichTextRange) {
    hasher.update(&saturating_u64(range.start).to_le_bytes());
    hasher.update(&saturating_u64(range.end).to_le_bytes());
}

fn orientation_tag(value: GlyphOrientation) -> u8 {
    match value {
        GlyphOrientation::Upright => 0,
        GlyphOrientation::SidewaysCw => 1,
        GlyphOrientation::TextCombineUpright => 2,
    }
}

fn vertical_form_tag(value: GlyphVerticalForm) -> u8 {
    match value {
        GlyphVerticalForm::None => 0,
        GlyphVerticalForm::UprightAlternate => 1,
        GlyphVerticalForm::RotatedAlternate => 2,
    }
}

fn writing_mode_tag(value: RichTextWritingMode) -> u8 {
    match value {
        RichTextWritingMode::HorizontalTb => 0,
        RichTextWritingMode::VerticalRl => 1,
        RichTextWritingMode::VerticalLr => 2,
    }
}

fn horizontal_wrap_tag(value: HorizontalWrap) -> u8 {
    match value {
        HorizontalWrap::Wrap => 0,
        HorizontalWrap::NoWrap => 1,
    }
}

fn direction_tag(value: RichTextInlineDirection) -> u8 {
    match value {
        RichTextInlineDirection::Auto => 0,
        RichTextInlineDirection::Ltr => 1,
        RichTextInlineDirection::Rtl => 2,
    }
}

fn jlreq_strictness_tag(value: JlreqStrictness) -> u8 {
    match value {
        JlreqStrictness::Loose => 0,
        JlreqStrictness::Normal => 1,
        JlreqStrictness::Strict => 2,
    }
}

fn weight_tag(value: TextWeight) -> u8 {
    match value {
        TextWeight::Thin => 0,
        TextWeight::ExtraLight => 1,
        TextWeight::Light => 2,
        TextWeight::Normal => 3,
        TextWeight::Medium => 4,
        TextWeight::SemiBold => 5,
        TextWeight::Bold => 6,
        TextWeight::ExtraBold => 7,
        TextWeight::Black => 8,
    }
}

fn put_bytes(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&saturating_u64(value.len()).to_le_bytes());
    hasher.update(value);
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn saturating_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
