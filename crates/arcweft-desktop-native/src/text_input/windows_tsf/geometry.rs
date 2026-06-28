use arcweft_presentation::hit::HitRect;
use arcweft_presentation::text_input::{
    TextByteOffset, TextInputGeometrySnapshot, TextInputSecurityPolicy, TextRange, TextRevision,
};
use num_traits::ToPrimitive;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TsfScreenRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TsfLayoutResult {
    Available {
        rect: TsfScreenRect,
        clipped: bool,
    },
    NoLayout,
    StaleRevision {
        expected: TextRevision,
        actual: TextRevision,
    },
    InvalidRange,
    SecureRedacted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowsTsfGeometry {
    security: TextInputSecurityPolicy,
}

impl TsfScreenRect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn enclosing(rect: HitRect) -> Self {
        Self {
            left: floor_to_i32_saturating(rect.x),
            top: floor_to_i32_saturating(rect.y),
            right: ceil_to_i32_saturating(rect.x + rect.width),
            bottom: ceil_to_i32_saturating(rect.y + rect.height),
        }
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            left: min_i32(self.left, other.left),
            top: min_i32(self.top, other.top),
            right: max_i32(self.right, other.right),
            bottom: max_i32(self.bottom, other.bottom),
        }
    }
}

impl WindowsTsfGeometry {
    pub const fn new(security: TextInputSecurityPolicy) -> Self {
        Self { security }
    }

    pub fn candidate_anchor(&self, snapshot: &TextInputGeometrySnapshot) -> TsfScreenRect {
        let rect = if self.security == TextInputSecurityPolicy::SecureRedacted {
            snapshot.screen_control_rect()
        } else {
            snapshot.candidate_anchor_rect()
        };
        TsfScreenRect::enclosing(rect)
    }

    pub fn text_ext(
        &self,
        snapshot: &TextInputGeometrySnapshot,
        expected_revision: TextRevision,
        range: TextRange<TextByteOffset>,
        clipped: bool,
    ) -> TsfLayoutResult {
        if snapshot.revision() != expected_revision {
            return TsfLayoutResult::StaleRevision {
                expected: expected_revision,
                actual: snapshot.revision(),
            };
        }
        if self.security == TextInputSecurityPolicy::SecureRedacted {
            return TsfLayoutResult::SecureRedacted;
        }
        if range.start().0 >= range.end().0 {
            return TsfLayoutResult::InvalidRange;
        }

        let rect = snapshot
            .screen_character_bounds()
            .iter()
            .filter(|bounds| bounds.range.start().0 >= range.start().0)
            .filter(|bounds| bounds.range.end().0 <= range.end().0)
            .map(|bounds| TsfScreenRect::enclosing(bounds.bounds))
            .reduce(TsfScreenRect::union);

        rect.map_or(TsfLayoutResult::NoLayout, |rect| {
            TsfLayoutResult::Available { rect, clipped }
        })
    }
}

const fn min_i32(a: i32, b: i32) -> i32 {
    if a < b { a } else { b }
}

const fn max_i32(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}

fn floor_to_i32_saturating(value: f32) -> i32 {
    rounded_to_i32_saturating(value.floor(), value)
}

fn ceil_to_i32_saturating(value: f32) -> i32 {
    rounded_to_i32_saturating(value.ceil(), value)
}

fn rounded_to_i32_saturating(rounded: f32, original: f32) -> i32 {
    rounded.to_i32().unwrap_or_else(|| {
        if original.is_sign_negative() {
            i32::MIN
        } else {
            i32::MAX
        }
    })
}
