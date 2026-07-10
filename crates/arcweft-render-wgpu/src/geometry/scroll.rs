use super::{PaintRect, Palette, RenderScene};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::ViewportPoint;
use std::time::Duration;

const INDICATOR_AUTO_FULL_MILLIS: u64 = 700;
const INDICATOR_AUTO_FADE_MILLIS: u64 = 300;
const INDICATOR_TRACK_INSET_PX: f32 = 3.0;
const INDICATOR_THICKNESS_PX: f32 = 6.0;
const INDICATOR_MIN_THUMB_PX: f32 = 20.0;

/// One scrollable retained View region in logical viewport coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderScrollRegion {
    pub id: String,
    pub bounds: HitRect,
    pub content_width: f32,
    pub content_height: f32,
    /// Persisted, clamped content offset along the horizontal axis.
    pub offset_x: f32,
    /// Persisted, clamped content offset along the vertical axis.
    pub offset_y: f32,
    /// Non-persistent elastic displacement applied only while rendering.
    pub overscroll_x: f32,
    /// Non-persistent elastic displacement applied only while rendering.
    pub overscroll_y: f32,
    pub axis: RenderScrollAxis,
    pub overflow: RenderScrollOverflow,
    pub indicators: RenderScrollIndicatorsPolicy,
    pub overscroll: RenderScrollOverscrollPolicy,
    pub auto_scroll_focus: RenderFocusAutoScrollPolicy,
    /// Last user/focus interaction used by the deterministic `.auto` indicator policy.
    pub indicator_activity_millis: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderScrollAxis {
    #[default]
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderScrollOverflow {
    #[default]
    Auto,
    Scroll,
    Hidden,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderScrollIndicatorsPolicy {
    #[default]
    Auto,
    Visible,
    Hidden,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderScrollOverscrollPolicy {
    #[default]
    Clamp,
    Contain,
    Elastic,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderFocusAutoScrollPolicy {
    #[default]
    Nearest,
    Start,
    End,
    Disabled,
}

/// Observable track/thumb geometry produced for a retained scroll region.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedScrollIndicator {
    pub region_id: String,
    pub axis: RenderScrollAxis,
    pub track_bounds: HitRect,
    pub thumb_bounds: HitRect,
    pub opacity: f32,
}

impl RenderScrollRegion {
    #[must_use]
    pub fn max_offset_x(&self) -> f32 {
        if self.axis != RenderScrollAxis::Horizontal || !self.overflow.scroll_enabled() {
            return 0.0;
        }
        if self.content_width.is_finite() && self.bounds.width.is_finite() {
            (self.content_width - self.bounds.width).max(0.0)
        } else {
            0.0
        }
    }

    #[must_use]
    pub fn max_offset_y(&self) -> f32 {
        if self.axis != RenderScrollAxis::Vertical || !self.overflow.scroll_enabled() {
            return 0.0;
        }
        if self.content_height.is_finite() && self.bounds.height.is_finite() {
            (self.content_height - self.bounds.height).max(0.0)
        } else {
            0.0
        }
    }

    #[must_use]
    pub fn clamped_offset_x(&self, offset_x: f32) -> f32 {
        if offset_x.is_finite() {
            offset_x.clamp(0.0, self.max_offset_x())
        } else {
            0.0
        }
    }

    #[must_use]
    pub fn clamped_offset_y(&self, offset_y: f32) -> f32 {
        if offset_y.is_finite() {
            offset_y.clamp(0.0, self.max_offset_y())
        } else {
            0.0
        }
    }

    /// Content offset including the transient elastic displacement.
    #[must_use]
    pub fn visual_offset_x(&self) -> f32 {
        self.clamped_offset_x(self.offset_x) + self.elastic_displacement(self.overscroll_x)
    }

    /// Content offset including the transient elastic displacement.
    #[must_use]
    pub fn visual_offset_y(&self) -> f32 {
        self.clamped_offset_y(self.offset_y) + self.elastic_displacement(self.overscroll_y)
    }

    #[must_use]
    pub fn contains(&self, point: ViewportPoint) -> bool {
        point.x >= self.bounds.x
            && point.x <= self.bounds.x + self.bounds.width
            && point.y >= self.bounds.y
            && point.y <= self.bounds.y + self.bounds.height
    }

    /// Whether `outer` can act as the next scroll-chain ancestor of this region.
    #[must_use]
    pub fn is_contained_by(&self, outer: &Self) -> bool {
        self.bounds.x >= outer.bounds.x
            && self.bounds.y >= outer.bounds.y
            && self.bounds.x + self.bounds.width <= outer.bounds.x + outer.bounds.width
            && self.bounds.y + self.bounds.height <= outer.bounds.y + outer.bounds.height
    }

    #[must_use]
    pub fn indicator_opacity(&self, visual_time_millis: u64, reduce_motion: bool) -> f32 {
        if !self.overflow.scroll_enabled() || self.primary_max_offset() <= f32::EPSILON {
            return 0.0;
        }
        match self.indicators {
            RenderScrollIndicatorsPolicy::Visible => 1.0,
            RenderScrollIndicatorsPolicy::Hidden => 0.0,
            RenderScrollIndicatorsPolicy::Auto => {
                let Some(activity) = self.indicator_activity_millis else {
                    return 0.0;
                };
                let age = visual_time_millis.saturating_sub(activity);
                if age <= INDICATOR_AUTO_FULL_MILLIS {
                    return 1.0;
                }
                if reduce_motion {
                    return 0.0;
                }
                let fade_age = age - INDICATOR_AUTO_FULL_MILLIS;
                if fade_age >= INDICATOR_AUTO_FADE_MILLIS {
                    0.0
                } else {
                    1.0 - Duration::from_millis(fade_age).as_secs_f32()
                        / Duration::from_millis(INDICATOR_AUTO_FADE_MILLIS).as_secs_f32()
                }
            }
        }
    }

    fn elastic_displacement(&self, displacement: f32) -> f32 {
        if self.overscroll != RenderScrollOverscrollPolicy::Elastic || !displacement.is_finite() {
            return 0.0;
        }
        let limit = match self.axis {
            RenderScrollAxis::Vertical => self.bounds.height,
            RenderScrollAxis::Horizontal => self.bounds.width,
        }
        .max(0.0)
            * 0.25;
        displacement.clamp(-limit.min(96.0), limit.min(96.0))
    }

    fn primary_max_offset(&self) -> f32 {
        match self.axis {
            RenderScrollAxis::Vertical => self.max_offset_y(),
            RenderScrollAxis::Horizontal => self.max_offset_x(),
        }
    }
}

impl RenderScrollOverflow {
    pub const fn scroll_enabled(self) -> bool {
        matches!(self, Self::Auto | Self::Scroll)
    }
}

pub(super) fn build_scroll_indicators(
    scene: &RenderScene,
    rectangles: &mut Vec<PaintRect>,
    palette: &Palette,
) -> Vec<PreparedScrollIndicator> {
    scene
        .scroll_regions
        .iter()
        .filter_map(|region| {
            let opacity =
                region.indicator_opacity(scene.visual_time_millis, scene.preferences.reduce_motion);
            (opacity > f32::EPSILON).then(|| prepare_indicator(region, opacity))
        })
        .flatten()
        .inspect(|indicator| {
            let radius = INDICATOR_THICKNESS_PX * 0.5;
            rectangles.extend([
                PaintRect::rounded(
                    indicator.track_bounds,
                    with_opacity(palette.scroll_track, indicator.opacity),
                    radius,
                )
                .clipped_to(indicator.track_bounds, radius),
                PaintRect::rounded(
                    indicator.thumb_bounds,
                    with_opacity(palette.scroll_thumb, indicator.opacity),
                    radius,
                )
                .clipped_to(indicator.track_bounds, radius),
            ]);
        })
        .collect()
}

fn prepare_indicator(region: &RenderScrollRegion, opacity: f32) -> Option<PreparedScrollIndicator> {
    let thickness = INDICATOR_THICKNESS_PX.min(match region.axis {
        RenderScrollAxis::Vertical => region.bounds.width,
        RenderScrollAxis::Horizontal => region.bounds.height,
    });
    let track_bounds = match region.axis {
        RenderScrollAxis::Vertical => HitRect::new(
            region.bounds.x + region.bounds.width - INDICATOR_TRACK_INSET_PX - thickness,
            region.bounds.y + INDICATOR_TRACK_INSET_PX,
            thickness,
            (region.bounds.height - INDICATOR_TRACK_INSET_PX * 2.0).max(0.0),
        ),
        RenderScrollAxis::Horizontal => HitRect::new(
            region.bounds.x + INDICATOR_TRACK_INSET_PX,
            region.bounds.y + region.bounds.height - INDICATOR_TRACK_INSET_PX - thickness,
            (region.bounds.width - INDICATOR_TRACK_INSET_PX * 2.0).max(0.0),
            thickness,
        ),
    };
    let (track_length, viewport_length, content_length, offset, max_offset) = match region.axis {
        RenderScrollAxis::Vertical => (
            track_bounds.height,
            region.bounds.height,
            region.content_height,
            region.clamped_offset_y(region.offset_y),
            region.max_offset_y(),
        ),
        RenderScrollAxis::Horizontal => (
            track_bounds.width,
            region.bounds.width,
            region.content_width,
            region.clamped_offset_x(region.offset_x),
            region.max_offset_x(),
        ),
    };
    if track_length <= f32::EPSILON
        || viewport_length <= f32::EPSILON
        || content_length <= viewport_length
        || max_offset <= f32::EPSILON
    {
        return None;
    }
    let thumb_length = (track_length * viewport_length / content_length)
        .clamp(INDICATOR_MIN_THUMB_PX.min(track_length), track_length);
    let thumb_position = (track_length - thumb_length) * (offset / max_offset).clamp(0.0, 1.0);
    let thumb_bounds = match region.axis {
        RenderScrollAxis::Vertical => HitRect::new(
            track_bounds.x,
            track_bounds.y + thumb_position,
            track_bounds.width,
            thumb_length,
        ),
        RenderScrollAxis::Horizontal => HitRect::new(
            track_bounds.x + thumb_position,
            track_bounds.y,
            thumb_length,
            track_bounds.height,
        ),
    };
    Some(PreparedScrollIndicator {
        region_id: region.id.clone(),
        axis: region.axis,
        track_bounds,
        thumb_bounds,
        opacity,
    })
}

fn with_opacity(mut color: [f32; 4], opacity: f32) -> [f32; 4] {
    color[3] *= opacity.clamp(0.0, 1.0);
    color
}
