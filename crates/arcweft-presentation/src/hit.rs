use crate::input::InteractionTarget;
use crate::layer::LayerId;

/// Axis-aligned hit bounds in logical viewport coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// One stable hit target derived from layout, text geometry, or Activity metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct HitRecord {
    layer: LayerId,
    target: InteractionTarget,
    hover_path: Vec<InteractionTarget>,
    bounds: HitRect,
    enabled: bool,
    visible: bool,
}

/// Frame-local hit records keyed by stable `InteractionTarget` and `LayerId`.
///
/// The records are pure data. Backends may derive them from native UI layout,
/// text glyph geometry, object-id passes, or Activity semantic regions, but
/// input routing consumes only this normalized form.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HitTree {
    records: Vec<HitRecord>,
}

impl HitRect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}

impl HitRecord {
    pub fn new(layer: LayerId, target: InteractionTarget, bounds: HitRect) -> Self {
        Self {
            layer,
            hover_path: vec![target.clone()],
            target,
            bounds,
            enabled: true,
            visible: true,
        }
    }

    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub const fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    #[must_use]
    pub fn with_hover_path(mut self, hover_path: Vec<InteractionTarget>) -> Self {
        self.hover_path = if hover_path.is_empty() {
            vec![self.target.clone()]
        } else {
            hover_path
        };
        self
    }

    pub const fn layer(&self) -> &LayerId {
        &self.layer
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub fn hover_path(&self) -> &[InteractionTarget] {
        &self.hover_path
    }

    pub const fn bounds(&self) -> HitRect {
        self.bounds
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn visible(&self) -> bool {
        self.visible
    }

    pub fn accepts_hit(&self, x: f32, y: f32) -> bool {
        self.enabled && self.visible && self.bounds.contains(x, y)
    }
}

impl HitTree {
    pub fn push(&mut self, record: HitRecord) {
        self.records.push(record);
    }

    pub fn as_slice(&self) -> &[HitRecord] {
        &self.records
    }

    pub fn records_for_layer<'a>(
        &'a self,
        layer: &LayerId,
    ) -> impl Iterator<Item = &'a HitRecord> + 'a {
        let layer = layer.clone();
        self.records
            .iter()
            .filter(move |record| record.layer() == &layer)
    }

    pub fn find_target(&self, target: &InteractionTarget) -> Option<&HitRecord> {
        self.records.iter().find(|record| record.target() == target)
    }

    pub fn hit_in_layer(&self, layer: &LayerId, x: f32, y: f32) -> Option<&HitRecord> {
        self.records_for_layer(layer)
            .find(|record| record.accepts_hit(x, y))
    }
}
