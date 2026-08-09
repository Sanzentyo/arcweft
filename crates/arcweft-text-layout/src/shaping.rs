//! Renderer-independent shaping requests and stable project-font identities.

use std::{error::Error, fmt};

use arcweft_render_text::{LanguageTag, ResolvedTextStyle};
use arcweft_text_model::{RichTextInlineDirection, RichTextRange, RichTextWritingMode};
use serde::{Deserialize, Serialize};

use crate::{LayoutPoint, LayoutRect, LayoutSize};

/// Stable identity of one canonical project font face and variation instance.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FontFaceId([u8; 32]);

/// Hash of the ordered project font inventory and its shaping features.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FontInventoryHash([u8; 32]);

/// Arcweft-owned stable glyph key, independent from a process-local fontdb ID.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ShapedGlyphKey {
    pub face: FontFaceId,
    pub glyph_id: u32,
    pub font_size_bits: u32,
    pub font_weight: u16,
    pub flags: u32,
}

/// One resolved run request passed to a shared CPU shaper.
#[derive(Clone, Copy, Debug)]
pub struct TextShapeRequest<'a> {
    pub text: &'a str,
    pub source_range: RichTextRange,
    pub style: &'a ResolvedTextStyle,
    pub locale: Option<&'a LanguageTag>,
    pub direction: RichTextInlineDirection,
    pub writing_mode: RichTextWritingMode,
}

/// One raster glyph produced by shaping, before line/column placement.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapedTextGlyph {
    pub key: ShapedGlyphKey,
    pub source_range: RichTextRange,
    /// Source hard-line index. Visual glyph order is retained within each line.
    pub line_index: u32,
    pub cluster_index: u32,
    pub offset: LayoutPoint,
    pub advance: LayoutSize,
    pub ink_bounds: LayoutRect,
}

/// Complete shaped output for one resolved text run in visual glyph order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapedTextRun {
    glyphs: Vec<ShapedTextGlyph>,
    advance: LayoutSize,
    ink_bounds: Option<LayoutRect>,
}

/// Shared font-aware shaping boundary used by native, Web, and headless paths.
pub trait TextShaper {
    type Error: Error + Send + Sync + 'static;

    /// Hash of the exact ordered font inventory used by [`Self::shape_run`].
    fn font_inventory_hash(&self) -> FontInventoryHash;

    fn shape_run(&mut self, request: TextShapeRequest<'_>) -> Result<ShapedTextRun, Self::Error>;
}

impl FontFaceId {
    /// Derives a stable face identity from canonical font bytes, face index,
    /// and sorted variation coordinates represented by canonical `f32` bits.
    #[must_use]
    pub fn derive(
        canonical_font_bytes: &[u8],
        face_index: u32,
        variation_coordinates: &[(u32, u32)],
    ) -> Self {
        let mut coordinates = variation_coordinates.to_vec();
        coordinates.sort_unstable();
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.font-face.v1\0");
        put_bytes(&mut hasher, canonical_font_bytes);
        hasher.update(&face_index.to_le_bytes());
        hasher.update(&(coordinates.len() as u64).to_le_bytes());
        for (tag, value_bits) in coordinates {
            hasher.update(&tag.to_le_bytes());
            hasher.update(&value_bits.to_le_bytes());
        }
        Self(*hasher.finalize().as_bytes())
    }

    pub const fn from_bytes(value: [u8; 32]) -> Self {
        Self(value)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl FontInventoryHash {
    /// Hashes ordered face identities and canonical shaping-feature records.
    #[must_use]
    pub fn derive<'a>(
        faces: impl IntoIterator<Item = FontFaceId>,
        shaping_features: impl IntoIterator<Item = &'a [u8]>,
    ) -> Self {
        let faces = faces.into_iter().collect::<Vec<_>>();
        let features = shaping_features.into_iter().collect::<Vec<_>>();
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.font-inventory.v1\0");
        hasher.update(&(faces.len() as u64).to_le_bytes());
        for face in faces {
            hasher.update(&face.0);
        }
        hasher.update(&(features.len() as u64).to_le_bytes());
        for feature in features {
            put_bytes(&mut hasher, feature);
        }
        Self(*hasher.finalize().as_bytes())
    }

    pub const fn from_bytes(value: [u8; 32]) -> Self {
        Self(value)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl ShapedTextRun {
    #[must_use]
    pub fn new(
        glyphs: Vec<ShapedTextGlyph>,
        advance: LayoutSize,
        ink_bounds: Option<LayoutRect>,
    ) -> Self {
        Self {
            glyphs,
            advance,
            ink_bounds,
        }
    }

    pub fn glyphs(&self) -> &[ShapedTextGlyph] {
        &self.glyphs
    }

    pub const fn advance(&self) -> LayoutSize {
        self.advance
    }

    pub const fn ink_bounds(&self) -> Option<LayoutRect> {
        self.ink_bounds
    }
}

impl fmt::Debug for FontFaceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "FontFaceId({})", HexDigest(&self.0))
    }
}

impl fmt::Debug for FontInventoryHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "FontInventoryHash({})", HexDigest(&self.0))
    }
}

struct HexDigest<'a>(&'a [u8; 32]);

impl fmt::Display for HexDigest<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

fn put_bytes(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

#[cfg(test)]
mod tests {
    use super::{FontFaceId, FontInventoryHash};

    #[test]
    fn face_identity_is_stable_and_sorts_variation_coordinates() {
        let first = FontFaceId::derive(
            b"font bytes",
            2,
            &[
                (u32::from_be_bytes(*b"wght"), 700.0_f32.to_bits()),
                (u32::from_be_bytes(*b"wdth"), 100.0_f32.to_bits()),
            ],
        );
        let second = FontFaceId::derive(
            b"font bytes",
            2,
            &[
                (u32::from_be_bytes(*b"wdth"), 100.0_f32.to_bits()),
                (u32::from_be_bytes(*b"wght"), 700.0_f32.to_bits()),
            ],
        );

        assert_eq!(first, second);
        assert_ne!(first, FontFaceId::derive(b"font bytes", 3, &[]));
    }

    #[test]
    fn inventory_hash_preserves_face_and_feature_order() {
        let first = FontFaceId::derive(b"first", 0, &[]);
        let second = FontFaceId::derive(b"second", 0, &[]);
        let ordered = FontInventoryHash::derive([first, second], [b"liga=0".as_slice()]);
        let reordered = FontInventoryHash::derive([second, first], [b"liga=0".as_slice()]);
        let different_features = FontInventoryHash::derive([first, second], [b"liga=1".as_slice()]);

        assert_ne!(ordered, reordered);
        assert_ne!(ordered, different_features);
    }
}
