//! Canonical semantic encoding for native specified Style values.

use super::{
    ViewAlignment, ViewBlendMode, ViewBorderRadii, ViewClip, ViewColorValue, ViewFilter,
    ViewFlexDirection, ViewFlexWrap, ViewFontFamily, ViewFontStyle, ViewMask, ViewShadow,
    ViewSpecifiedValue, ViewSystemFontFamily,
};
use arcweft_presentation::appearance::PresentationColor;

const DOMAIN: &[u8] = b"arcweft.view.specified-value-semantic.v1\0";

/// One-way semantic identity of one native specified Style value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewSpecifiedValueSemanticDigest([u8; 32]);

impl ViewSpecifiedValueSemanticDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

struct SemanticEncoder {
    hasher: blake3::Hasher,
}

impl SemanticEncoder {
    fn new() -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(DOMAIN);
        Self { hasher }
    }

    fn finish(self) -> ViewSpecifiedValueSemanticDigest {
        ViewSpecifiedValueSemanticDigest(*self.hasher.finalize().as_bytes())
    }

    fn u8(&mut self, value: u8) {
        self.hasher.update(&[value]);
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u16(&mut self, value: u16) {
        self.hasher.update(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.hasher.update(&value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.hasher.update(&value.to_le_bytes());
    }

    fn len(&mut self, value: usize) {
        let value =
            u64::try_from(value).expect("Rust collection lengths fit the semantic u64 grammar");
        self.hasher.update(&value.to_le_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.len(value.len());
        self.hasher.update(value);
    }
}

impl ViewSystemFontFamily {
    /// Stable semantic tag in declaration order.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Ui => 0,
            Self::Serif => 1,
            Self::Sans => 2,
            Self::Monospace => 3,
            Self::Emoji => 4,
        }
    }
}

impl ViewFlexDirection {
    /// Stable semantic tag in declaration order.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Row => 0,
            Self::RowReverse => 1,
            Self::Column => 2,
            Self::ColumnReverse => 3,
        }
    }
}

impl ViewFlexWrap {
    /// Stable semantic tag in declaration order.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::NoWrap => 0,
            Self::Wrap => 1,
            Self::WrapReverse => 2,
        }
    }
}

impl ViewFontStyle {
    /// Stable semantic tag in declaration order.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Italic => 1,
            Self::Oblique => 2,
        }
    }
}

impl ViewAlignment {
    /// Stable semantic tag in declaration order.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Start => 0,
            Self::End => 1,
            Self::Center => 2,
            Self::Stretch => 3,
            Self::SpaceBetween => 4,
            Self::SpaceAround => 5,
            Self::SpaceEvenly => 6,
        }
    }
}

impl ViewFilter {
    /// Stable semantic tag in declaration order.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Blur { .. } => 0,
            Self::Brightness { .. } => 1,
            Self::Contrast { .. } => 2,
            Self::Opacity { .. } => 3,
        }
    }
}

impl ViewClip {
    /// Stable semantic tag in declaration order.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::None => 0,
            Self::RoundedRect(_) => 1,
        }
    }
}

impl ViewMask {
    /// Stable semantic tag in declaration order.
    pub const fn semantic_tag(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::Resource(_) => 1,
        }
    }
}

impl ViewBlendMode {
    /// Stable semantic tag in declaration order.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Multiply => 1,
            Self::Screen => 2,
            Self::Overlay => 3,
            Self::Darken => 4,
            Self::Lighten => 5,
        }
    }
}

impl ViewSpecifiedValue {
    /// Computes the canonical owner-defined semantic digest.
    #[must_use]
    pub fn semantic_digest(&self) -> ViewSpecifiedValueSemanticDigest {
        let mut encoder = SemanticEncoder::new();
        encoder.u8(self.semantic_tag());
        self.encode_payload(&mut encoder);
        encoder.finish()
    }

    const fn semantic_tag(&self) -> u8 {
        match self {
            Self::Token { .. } => 0,
            Self::BoxAxes { .. } => 1,
            Self::Bool { .. } => 2,
            Self::Integer { .. } => 3,
            Self::Ratio { .. } => 4,
            Self::Scalar { .. } => 5,
            Self::Length { .. } => 6,
            Self::Angle { .. } => 7,
            Self::Color { .. } => 8,
            Self::FontFamilyList { .. } => 9,
            Self::FontWeight { .. } => 10,
            Self::FontStyle { .. } => 11,
            Self::Display { .. } => 12,
            Self::Position { .. } => 13,
            Self::Overflow { .. } => 14,
            Self::FlexDirection { .. } => 15,
            Self::FlexWrap { .. } => 16,
            Self::Alignment { .. } => 17,
            Self::BorderRadii { .. } => 18,
            Self::ShadowList { .. } => 19,
            Self::FilterList { .. } => 20,
            Self::Clip { .. } => 21,
            Self::Mask { .. } => 22,
            Self::BlendMode { .. } => 23,
            Self::Transition { .. } => 24,
            Self::Resource { .. } => 25,
        }
    }

    fn encode_payload(&self, encoder: &mut SemanticEncoder) {
        match self {
            Self::Token { token, value_kind } => {
                encoder.bytes(token.public_id().as_str().as_bytes());
                encoder.u8(value_kind.semantic_tag());
            }
            Self::BoxAxes { value } => encoder.u8(value.canonical_tag()),
            Self::Bool { value } => encoder.bool(*value),
            Self::Integer { value } => encoder.i32(*value),
            Self::Ratio { value } => encoder.u16(value.value()),
            Self::Scalar { value } => encoder.u32(value.value()),
            Self::Length { value } => encoder.i32(value.value()),
            Self::Angle { value } => encoder.i32(value.value()),
            Self::Color { value } => encode_color(encoder, *value),
            Self::FontFamilyList { value } => {
                encoder.len(value.families.len());
                for family in &value.families {
                    match family {
                        ViewFontFamily::Named(name) => {
                            encoder.u8(0);
                            encoder.bytes(name.as_bytes());
                        }
                        ViewFontFamily::System(system) => {
                            encoder.u8(1);
                            encoder.u8(system.semantic_tag());
                        }
                    }
                }
            }
            Self::FontWeight { value } => encoder.u16(value.value()),
            Self::FontStyle { value } => encoder.u8(value.semantic_tag()),
            Self::Display { value } => encoder.u8(value.canonical_tag()),
            Self::Position { value } => encoder.u8(value.canonical_tag()),
            Self::Overflow { value } => encoder.u8(value.canonical_tag()),
            Self::FlexDirection { value } => encoder.u8(value.semantic_tag()),
            Self::FlexWrap { value } => encoder.u8(value.semantic_tag()),
            Self::Alignment { value } => encoder.u8(value.semantic_tag()),
            Self::BorderRadii { value } => encode_radii(encoder, *value),
            Self::ShadowList { value } => {
                encoder.len(value.len());
                for shadow in value {
                    encode_shadow(encoder, *shadow);
                }
            }
            Self::FilterList { value } => {
                encoder.len(value.len());
                for filter in value {
                    encode_filter(encoder, *filter);
                }
            }
            Self::Clip { value } => {
                encoder.u8(value.semantic_tag());
                if let ViewClip::RoundedRect(radii) = value {
                    encode_radii(encoder, *radii);
                }
            }
            Self::Mask { value } => {
                encoder.u8(value.semantic_tag());
                if let ViewMask::Resource(resource) = value {
                    encoder.bytes(resource.as_str().as_bytes());
                }
            }
            Self::BlendMode { value } => encoder.u8(value.semantic_tag()),
            Self::Transition { value } => {
                encoder.len(value.len());
                for transition in value {
                    encoder.u8(transition.property.semantic_tag());
                    encoder.u32(transition.duration_millis);
                    encoder.u32(transition.delay_millis);
                }
            }
            Self::Resource { value } => encoder.bytes(value.as_str().as_bytes()),
        }
    }
}

fn encode_color(encoder: &mut SemanticEncoder, color: ViewColorValue) {
    match color {
        ViewColorValue::Literal { color } => {
            encoder.u8(0);
            encode_literal_color(encoder, color);
        }
        ViewColorValue::System { role } => {
            encoder.u8(1);
            encoder.u8(role.semantic_tag());
        }
    }
}

fn encode_literal_color(encoder: &mut SemanticEncoder, color: PresentationColor) {
    encoder.u8(color.red);
    encoder.u8(color.green);
    encoder.u8(color.blue);
    encoder.u8(color.alpha);
}

fn encode_radii(encoder: &mut SemanticEncoder, radii: ViewBorderRadii) {
    encoder.i32(radii.top_left.value());
    encoder.i32(radii.top_right.value());
    encoder.i32(radii.bottom_right.value());
    encoder.i32(radii.bottom_left.value());
}

fn encode_shadow(encoder: &mut SemanticEncoder, shadow: ViewShadow) {
    encoder.i32(shadow.x.value());
    encoder.i32(shadow.y.value());
    encoder.i32(shadow.blur.value());
    encoder.i32(shadow.spread.value());
    encode_color(encoder, shadow.color);
    encoder.bool(shadow.inset);
}

fn encode_filter(encoder: &mut SemanticEncoder, filter: ViewFilter) {
    encoder.u8(filter.semantic_tag());
    match filter {
        ViewFilter::Blur { radius } => encoder.i32(radius.value()),
        ViewFilter::Brightness { amount } | ViewFilter::Contrast { amount } => {
            encoder.u32(amount.value());
        }
        ViewFilter::Opacity { amount } => encoder.u16(amount.value()),
    }
}

#[cfg(test)]
#[path = "semantic/tests.rs"]
mod tests;
