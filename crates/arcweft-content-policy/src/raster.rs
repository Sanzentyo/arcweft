use crate::types::{FindingTarget, ObjectId, PixelRect, PolicyError, PolicyFinding};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Validated RGBA8 image.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RgbaImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// Dense one-byte-per-pixel mask. Zero is visible; non-zero is replaced.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PixelMask {
    width: u32,
    height: u32,
    values: Vec<u8>,
}

/// Object-id attachment aligned with a color image.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ObjectIdBuffer {
    width: u32,
    height: u32,
    values: Vec<ObjectId>,
}

/// Irreversible mask style used for agent publication.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MaskStyle {
    Solid { rgba: [u8; 4] },
}

impl Default for MaskStyle {
    fn default() -> Self {
        Self::Solid {
            rgba: [32, 32, 32, 255],
        }
    }
}

impl RgbaImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, PolicyError> {
        let expected = rgba_len(width, height)?;
        if pixels.len() != expected {
            return Err(PolicyError::InvalidRgbaLength {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }

    pub fn content_digest(&self) -> crate::types::ContentDigest {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.rgba8.v1");
        hasher.update(&self.width.to_le_bytes());
        hasher.update(&self.height.to_le_bytes());
        hasher.update(&self.pixels);
        crate::types::ContentDigest::from_hasher(&hasher)
    }

    pub fn mask_for_findings(
        &self,
        findings: &[PolicyFinding],
        object_ids: Option<&ObjectIdBuffer>,
        whole_if_unlocalized: bool,
    ) -> Result<PixelMask, PolicyError> {
        let mut mask = PixelMask::empty(self.width, self.height)?;
        let mut localized = false;
        for finding in findings {
            match &finding.target {
                FindingTarget::Whole => {
                    mask.fill();
                    localized = true;
                }
                FindingTarget::ImageRect { rect } => {
                    mask.set_rect(*rect);
                    localized = true;
                }
                FindingTarget::ImageMask { mask: finding_mask } => {
                    mask.union_assign(finding_mask)?;
                    localized = true;
                }
                FindingTarget::ObjectIds { ids } => {
                    if let Some(object_ids) = object_ids {
                        mask.union_assign(&object_ids.mask_for(ids)?)?;
                    } else if whole_if_unlocalized {
                        mask.fill();
                    } else {
                        return Err(PolicyError::ObjectIdDimensionMismatch);
                    }
                    localized = true;
                }
                FindingTarget::Text { .. }
                | FindingTarget::SceneViewRect { .. }
                | FindingTarget::SceneViewMask { .. } => {}
            }
        }
        if whole_if_unlocalized && !localized {
            mask.fill();
        }
        Ok(mask)
    }

    pub fn masked(&self, mask: &PixelMask, style: MaskStyle) -> Result<Self, PolicyError> {
        mask.require_dimensions(self.width, self.height)?;
        let replacement = match style {
            MaskStyle::Solid { rgba } => rgba,
        };
        let pixels = self
            .pixels
            .chunks_exact(4)
            .zip(mask.values.iter())
            .flat_map(|(pixel, masked)| {
                if *masked == 0 {
                    [pixel[0], pixel[1], pixel[2], pixel[3]]
                } else {
                    replacement
                }
            })
            .collect();
        Self::new(self.width, self.height, pixels)
    }
}

impl PixelMask {
    pub fn new(width: u32, height: u32, values: Vec<u8>) -> Result<Self, PolicyError> {
        let expected = pixel_len(width, height)?;
        if values.len() != expected {
            return Err(PolicyError::MaskDimensionMismatch);
        }
        Ok(Self {
            width,
            height,
            values,
        })
    }

    pub fn empty(width: u32, height: u32) -> Result<Self, PolicyError> {
        Ok(Self {
            width,
            height,
            values: vec![0; pixel_len(width, height)?],
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn values(&self) -> &[u8] {
        &self.values
    }

    pub fn masked_pixels(&self) -> usize {
        self.values.iter().filter(|value| **value != 0).count()
    }

    pub fn fill(&mut self) {
        self.values.fill(u8::MAX);
    }

    pub fn set_rect(&mut self, rect: PixelRect) {
        let rect = rect.clamped(self.width, self.height);
        if rect.is_empty() {
            return;
        }
        let (Ok(width), Ok(x), Ok(row_width), Ok(start_y), Ok(end_y)) = (
            usize::try_from(self.width),
            usize::try_from(rect.x),
            usize::try_from(rect.width),
            usize::try_from(rect.y),
            usize::try_from(rect.y.saturating_add(rect.height)),
        ) else {
            return;
        };
        let end_x = x + row_width;
        self.values
            .chunks_exact_mut(width)
            .skip(start_y)
            .take(end_y - start_y)
            .for_each(|row| row[x..end_x].fill(u8::MAX));
    }

    pub fn union_assign(&mut self, other: &Self) -> Result<(), PolicyError> {
        self.require_dimensions(other.width, other.height)?;
        self.values
            .iter_mut()
            .zip(other.values.iter())
            .for_each(|(target, source)| *target = (*target).max(*source));
        Ok(())
    }

    pub fn require_dimensions(&self, width: u32, height: u32) -> Result<(), PolicyError> {
        if self.width == width && self.height == height {
            Ok(())
        } else {
            Err(PolicyError::MaskDimensionMismatch)
        }
    }
}

impl ObjectIdBuffer {
    pub fn new(width: u32, height: u32, values: Vec<ObjectId>) -> Result<Self, PolicyError> {
        if values.len() != pixel_len(width, height)? {
            return Err(PolicyError::ObjectIdDimensionMismatch);
        }
        Ok(Self {
            width,
            height,
            values,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn values(&self) -> &[ObjectId] {
        &self.values
    }

    pub fn mask_for(&self, ids: &BTreeSet<ObjectId>) -> Result<PixelMask, PolicyError> {
        PixelMask::new(
            self.width,
            self.height,
            self.values
                .iter()
                .map(|id| if ids.contains(id) { u8::MAX } else { 0 })
                .collect(),
        )
    }
}

fn pixel_len(width: u32, height: u32) -> Result<usize, PolicyError> {
    if width == 0 || height == 0 {
        return Err(PolicyError::EmptyImage);
    }
    u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(PolicyError::EmptyImage)
}

fn rgba_len(width: u32, height: u32) -> Result<usize, PolicyError> {
    pixel_len(width, height)?
        .checked_mul(4)
        .ok_or(PolicyError::EmptyImage)
}
