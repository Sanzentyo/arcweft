use crate::input::InteractionTarget;
use crate::layer::LayerId;

/// Axis-aligned hit bounds in layer-local logical coordinates.
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
/// The records are pure data. Backends may derive them from native View layout,
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

    pub fn contains(self, x: f64, y: f64) -> bool {
        let left = f64::from(self.x);
        let top = f64::from(self.y);
        x >= left
            && y >= top
            && x < left + f64::from(self.width)
            && y < top + f64::from(self.height)
    }

    #[must_use]
    pub fn outset(self, amount: f32) -> Self {
        let doubled = amount * 2.0;
        Self {
            x: self.x - amount,
            y: self.y - amount,
            width: (self.width + doubled).max(0.0),
            height: (self.height + doubled).max(0.0),
        }
    }

    #[must_use]
    pub fn translated(self, x: f32, y: f32) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
            ..self
        }
    }

    #[must_use]
    pub fn scaled_about_center(self, scale: f32) -> Self {
        let scale = scale.max(0.0);
        let width = self.width * scale;
        let height = self.height * scale;
        Self {
            x: self.x + (self.width - width) * 0.5,
            y: self.y + (self.height - height) * 0.5,
            width,
            height,
        }
    }

    #[must_use]
    pub fn transformed(
        self,
        translate_x: f32,
        translate_y: f32,
        scale_x: f32,
        scale_y: f32,
    ) -> Self {
        Self {
            x: translate_x + self.x * scale_x,
            y: translate_y + self.y * scale_y,
            width: self.width * scale_x,
            height: self.height * scale_y,
        }
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
    pub fn with_hover_path(mut self, mut hover_path: Vec<InteractionTarget>) -> Self {
        if hover_path.last() != Some(&self.target) {
            hover_path.push(self.target.clone());
        }
        self.hover_path = hover_path;
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

    pub fn accepts_hit(&self, x: f64, y: f64) -> bool {
        self.enabled && self.visible && self.bounds.contains(x, y)
    }
}

impl HitTree {
    pub fn push(&mut self, record: HitRecord) {
        self.records.push(record);
    }

    #[must_use]
    pub fn with_transformed_bounds(
        mut self,
        translate_x: f32,
        translate_y: f32,
        scale_x: f32,
        scale_y: f32,
    ) -> Self {
        for record in &mut self.records {
            record.bounds = record
                .bounds
                .transformed(translate_x, translate_y, scale_x, scale_y);
        }
        self
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

    pub fn hit_in_layer(&self, layer: &LayerId, x: f64, y: f64) -> Option<&HitRecord> {
        self.records_for_layer(layer)
            .find(|record| record.accepts_hit(x, y))
    }
}
