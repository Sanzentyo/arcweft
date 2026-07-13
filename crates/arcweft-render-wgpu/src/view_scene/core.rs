use super::compositing::ViewPaintNode;
use arcweft_glyphon::PreparedTextId;
use arcweft_presentation::hit::HitRect;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewScene {
    viewport_width: f32,
    viewport_height: f32,
    contexts: Vec<ViewSceneContext>,
    primitives: Vec<ViewPrimitive>,
    paint_nodes: Vec<ViewPaintNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewSceneContext {
    pub transform: ViewAffine2D,
    pub opacity: f32,
    pub clip: Option<ViewClip>,
    pub primitive_range: ViewPrimitiveRange,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewPrimitiveRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewPrimitive {
    SolidRect(ViewSolidRect),
    RoundedRect(ViewRoundedRect),
    Border(ViewBorder),
    LinearGradient(ViewLinearGradient),
    Image(ViewImagePrimitive),
    Text(ViewTextPrimitive),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewAffine2D {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub tx: f32,
    pub ty: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewClip {
    Rect(HitRect),
    RoundedRect { bounds: HitRect, radius: f32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewColorRgba8 {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl From<arcweft_render_text::TextColor> for ViewColorRgba8 {
    fn from(color: arcweft_render_text::TextColor) -> Self {
        let [red, green, blue, alpha] = color.channels();
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewSolidRect {
    pub bounds: HitRect,
    pub color: ViewColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewRoundedRect {
    pub bounds: HitRect,
    pub radii: ViewCornerRadii,
    pub color: ViewColorRgba8,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewSurfacePaint {
    pub backgrounds: Vec<ViewSurfaceBackground>,
    pub border: Option<ViewSurfaceBorder>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewSurfaceBackground {
    Solid {
        color: ViewColorRgba8,
        radii: ViewCornerRadii,
    },
    LinearGradient {
        angle_degrees: f32,
        stops: Vec<ViewGradientStop>,
    },
    Image {
        resource_index: u32,
        opacity: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewSurfaceBorder {
    pub width: f32,
    pub radius: f32,
    pub color: ViewColorRgba8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewSurfaceClip {
    Rect,
    RoundedRect { radius: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewCornerRadii {
    pub top_left: ViewCornerRadius,
    pub top_right: ViewCornerRadius,
    pub bottom_right: ViewCornerRadius,
    pub bottom_left: ViewCornerRadius,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewCornerRadius {
    pub x_px: f32,
    pub y_px: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewBorder {
    pub bounds: HitRect,
    pub radius: f32,
    pub width: f32,
    pub color: ViewColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewLinearGradient {
    pub bounds: HitRect,
    pub angle_degrees: f32,
    pub stops: Vec<ViewGradientStop>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewGradientStop {
    pub offset: f32,
    pub color: ViewColorRgba8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewImagePrimitive {
    pub resource_index: u32,
    pub bounds: HitRect,
    pub uv: ViewImageUvRect,
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewImageUvRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl ViewImageUvRect {
    pub const FULL: Self = Self {
        left: 0.0,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
    };
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewTextPrimitive {
    pub text: PreparedTextId,
}

impl Default for ViewAffine2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl ViewAffine2D {
    pub const IDENTITY: Self = Self {
        m11: 1.0,
        m12: 0.0,
        m21: 0.0,
        m22: 1.0,
        tx: 0.0,
        ty: 0.0,
    };
}

impl ViewCornerRadius {
    pub const ZERO: Self = Self {
        x_px: 0.0,
        y_px: 0.0,
    };

    pub const fn new(x_px: f32, y_px: f32) -> Self {
        Self { x_px, y_px }
    }

    pub const fn circular(radius_px: f32) -> Self {
        Self {
            x_px: radius_px,
            y_px: radius_px,
        }
    }

    #[must_use]
    pub fn non_negative(self) -> Self {
        Self {
            x_px: self.x_px.max(0.0),
            y_px: self.y_px.max(0.0),
        }
    }

    fn scaled(self, scale: f32) -> Self {
        Self {
            x_px: self.x_px * scale,
            y_px: self.y_px * scale,
        }
    }
}

impl ViewCornerRadii {
    pub const ZERO: Self = Self {
        top_left: ViewCornerRadius::ZERO,
        top_right: ViewCornerRadius::ZERO,
        bottom_right: ViewCornerRadius::ZERO,
        bottom_left: ViewCornerRadius::ZERO,
    };

    pub const fn uniform(radius_px: f32) -> Self {
        Self {
            top_left: ViewCornerRadius::circular(radius_px),
            top_right: ViewCornerRadius::circular(radius_px),
            bottom_right: ViewCornerRadius::circular(radius_px),
            bottom_left: ViewCornerRadius::circular(radius_px),
        }
    }

    pub const fn from_corners(
        top_left: ViewCornerRadius,
        top_right: ViewCornerRadius,
        bottom_right: ViewCornerRadius,
        bottom_left: ViewCornerRadius,
    ) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    #[must_use]
    pub fn non_negative(self) -> Self {
        Self {
            top_left: self.top_left.non_negative(),
            top_right: self.top_right.non_negative(),
            bottom_right: self.bottom_right.non_negative(),
            bottom_left: self.bottom_left.non_negative(),
        }
    }

    #[must_use]
    pub fn clamped_to_rect(self, rect: HitRect) -> Self {
        let radii = self.non_negative();
        let width = rect.width.max(0.0);
        let height = rect.height.max(0.0);
        let scale = radius_scale_limit(
            radius_scale_limit(
                radius_scale_limit(
                    radius_scale_limit(1.0, width, radii.top_left.x_px + radii.top_right.x_px),
                    width,
                    radii.bottom_left.x_px + radii.bottom_right.x_px,
                ),
                height,
                radii.top_left.y_px + radii.bottom_left.y_px,
            ),
            height,
            radii.top_right.y_px + radii.bottom_right.y_px,
        );
        if scale < 1.0 {
            radii.scaled(scale)
        } else {
            radii
        }
    }

    pub fn uniform_circular_radius(self) -> Option<f32> {
        let radius = self.top_left.x_px;
        (same_f32(radius, self.top_left.y_px)
            && same_f32(radius, self.top_right.x_px)
            && same_f32(radius, self.top_right.y_px)
            && same_f32(radius, self.bottom_right.x_px)
            && same_f32(radius, self.bottom_right.y_px)
            && same_f32(radius, self.bottom_left.x_px)
            && same_f32(radius, self.bottom_left.y_px))
        .then_some(radius)
    }

    #[must_use]
    pub fn has_rounded_corner(self) -> bool {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
        .into_iter()
        .any(|radius| radius.x_px > f32::EPSILON && radius.y_px > f32::EPSILON)
    }

    fn scaled(self, scale: f32) -> Self {
        Self {
            top_left: self.top_left.scaled(scale),
            top_right: self.top_right.scaled(scale),
            bottom_right: self.bottom_right.scaled(scale),
            bottom_left: self.bottom_left.scaled(scale),
        }
    }
}

impl ViewSurfacePaint {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_background(mut self, background: ViewSurfaceBackground) -> Self {
        self.backgrounds.push(background);
        self
    }

    #[must_use]
    pub fn with_backgrounds(
        mut self,
        backgrounds: impl IntoIterator<Item = ViewSurfaceBackground>,
    ) -> Self {
        self.backgrounds.extend(backgrounds);
        self
    }

    #[must_use]
    pub fn with_border(mut self, border: ViewSurfaceBorder) -> Self {
        self.border = Some(border);
        self
    }

    #[must_use]
    pub fn has_visible_primitives(&self) -> bool {
        !self.backgrounds.is_empty() || self.border.is_some()
    }

    pub fn append_primitives(&self, bounds: HitRect, mut push: impl FnMut(ViewPrimitive)) -> bool {
        let mut pushed = false;
        for background in &self.backgrounds {
            push(background.to_primitive(bounds));
            pushed = true;
        }
        if let Some(border) = self.border {
            push(ViewPrimitive::Border(ViewBorder {
                bounds,
                radius: border.radius,
                width: border.width,
                color: border.color,
            }));
            pushed = true;
        }
        pushed
    }
}

impl ViewSurfaceBackground {
    #[must_use]
    pub fn to_primitive(&self, bounds: HitRect) -> ViewPrimitive {
        match self {
            Self::Solid { color, radii } if radii.has_rounded_corner() => {
                ViewPrimitive::RoundedRect(ViewRoundedRect {
                    bounds,
                    radii: *radii,
                    color: *color,
                })
            }
            Self::Solid { color, .. } => ViewPrimitive::SolidRect(ViewSolidRect {
                bounds,
                color: *color,
            }),
            Self::LinearGradient {
                angle_degrees,
                stops,
            } => ViewPrimitive::LinearGradient(ViewLinearGradient {
                bounds,
                angle_degrees: *angle_degrees,
                stops: stops.clone(),
            }),
            Self::Image {
                resource_index,
                opacity,
            } => ViewPrimitive::Image(ViewImagePrimitive {
                resource_index: *resource_index,
                bounds,
                uv: ViewImageUvRect::FULL,
                opacity: *opacity,
            }),
        }
    }
}

impl ViewSurfaceClip {
    #[must_use]
    pub const fn to_view_clip(self, bounds: HitRect) -> ViewClip {
        match self {
            Self::Rect => ViewClip::Rect(bounds),
            Self::RoundedRect { radius } => ViewClip::RoundedRect { bounds, radius },
        }
    }
}

fn radius_scale_limit(current: f32, side: f32, radii_sum: f32) -> f32 {
    if side <= 0.0 || radii_sum <= side || radii_sum <= f32::EPSILON {
        current
    } else {
        current.min(side / radii_sum)
    }
}

fn same_f32(left: f32, right: f32) -> bool {
    (left - right).abs() <= f32::EPSILON
}

impl ViewScene {
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            viewport_width,
            viewport_height,
            contexts: Vec::new(),
            primitives: Vec::new(),
            paint_nodes: Vec::new(),
        }
    }

    pub fn push_context(&mut self, context: ViewSceneContext) {
        self.paint_nodes
            .push(ViewPaintNode::Direct(context.clone()));
        self.contexts.push(context);
    }

    pub fn push_primitive(&mut self, primitive: ViewPrimitive) {
        self.primitives.push(primitive);
    }

    pub fn push_surface_primitives(
        &mut self,
        bounds: HitRect,
        paint: &ViewSurfacePaint,
    ) -> Option<ViewPrimitiveRange> {
        let start = u32::try_from(self.primitives.len()).unwrap_or(u32::MAX);
        paint.append_primitives(bounds, |primitive| self.push_primitive(primitive));
        let end = u32::try_from(self.primitives.len()).unwrap_or(u32::MAX);
        (start != end).then_some(ViewPrimitiveRange { start, end })
    }

    pub fn push_paint_node(&mut self, node: ViewPaintNode) {
        self.paint_nodes.push(node);
    }

    pub fn replace_paint_nodes(&mut self, paint_nodes: Vec<ViewPaintNode>) {
        self.paint_nodes = paint_nodes;
    }

    pub const fn viewport_width(&self) -> f32 {
        self.viewport_width
    }

    pub const fn viewport_height(&self) -> f32 {
        self.viewport_height
    }

    pub fn contexts(&self) -> &[ViewSceneContext] {
        &self.contexts
    }

    pub fn primitives(&self) -> &[ViewPrimitive] {
        &self.primitives
    }

    /// Returns the canonical prepared-text items consumed in this scene's
    /// painter order. Repeated identifiers remain repeated because one item
    /// may intentionally be painted under more than one View context.
    pub fn prepared_text_ids(&self) -> impl Iterator<Item = PreparedTextId> + '_ {
        self.primitives
            .iter()
            .filter_map(|primitive| match primitive {
                ViewPrimitive::Text(text) => Some(text.text),
                ViewPrimitive::SolidRect(_)
                | ViewPrimitive::RoundedRect(_)
                | ViewPrimitive::Border(_)
                | ViewPrimitive::LinearGradient(_)
                | ViewPrimitive::Image(_) => None,
            })
    }

    pub fn paint_nodes(&self) -> &[ViewPaintNode] {
        &self.paint_nodes
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ViewAffine2D, ViewColorRgba8, ViewCornerRadii, ViewCornerRadius, ViewPrimitive,
        ViewPrimitiveRange, ViewScene, ViewSceneContext, ViewSolidRect, ViewSurfaceBackground,
        ViewSurfaceBorder, ViewSurfacePaint, ViewTextPrimitive,
    };
    use crate::view_scene::ViewPaintNode;
    use arcweft_glyphon::PreparedTextId;
    use arcweft_presentation::hit::HitRect;

    #[test]
    fn view_scene_preserves_context_primitive_and_paint_node_order() {
        let mut scene = ViewScene::new(320.0, 180.0);
        scene.push_primitive(ViewPrimitive::SolidRect(ViewSolidRect {
            bounds: HitRect::new(0.0, 0.0, 10.0, 10.0),
            color: ViewColorRgba8 {
                red: 1,
                green: 2,
                blue: 3,
                alpha: 255,
            },
        }));
        scene.push_context(ViewSceneContext {
            transform: ViewAffine2D::IDENTITY,
            opacity: 1.0,
            clip: None,
            primitive_range: ViewPrimitiveRange { start: 0, end: 1 },
        });

        assert!((scene.viewport_width() - 320.0).abs() < f32::EPSILON);
        assert!((scene.viewport_height() - 180.0).abs() < f32::EPSILON);
        assert_eq!(scene.primitives().len(), 1);
        assert_eq!(
            scene.contexts()[0].primitive_range,
            ViewPrimitiveRange { start: 0, end: 1 }
        );
        assert!(matches!(scene.paint_nodes()[0], ViewPaintNode::Direct(_)));
    }

    #[test]
    fn corner_radii_clamp_to_rect_bounds() {
        let radii = ViewCornerRadii::from_corners(
            ViewCornerRadius::new(80.0, 40.0),
            ViewCornerRadius::new(80.0, 20.0),
            ViewCornerRadius::new(20.0, 20.0),
            ViewCornerRadius::new(20.0, 40.0),
        )
        .clamped_to_rect(HitRect::new(0.0, 0.0, 100.0, 50.0));

        assert!((radii.top_left.x_px - 50.0).abs() < f32::EPSILON);
        assert!((radii.top_left.y_px - 25.0).abs() < f32::EPSILON);
        assert!((radii.top_right.x_px - 50.0).abs() < f32::EPSILON);
        assert!((radii.bottom_left.y_px - 25.0).abs() < f32::EPSILON);
    }

    #[test]
    fn surface_paint_pushes_shared_rounded_fill_and_border_primitives() {
        let mut scene = ViewScene::new(320.0, 180.0);
        let radii = ViewCornerRadii::from_corners(
            ViewCornerRadius::new(18.0, 12.0),
            ViewCornerRadius::new(10.0, 6.0),
            ViewCornerRadius::new(14.0, 8.0),
            ViewCornerRadius::new(6.0, 4.0),
        );
        let paint = ViewSurfacePaint::new()
            .with_background(ViewSurfaceBackground::Solid {
                color: ViewColorRgba8 {
                    red: 1,
                    green: 2,
                    blue: 3,
                    alpha: 255,
                },
                radii,
            })
            .with_border(ViewSurfaceBorder {
                width: 2.0,
                radius: 9.0,
                color: ViewColorRgba8 {
                    red: 4,
                    green: 5,
                    blue: 6,
                    alpha: 255,
                },
            });

        let range = scene
            .push_surface_primitives(HitRect::new(0.0, 0.0, 80.0, 40.0), &paint)
            .expect("surface paint emits primitives");

        assert_eq!(range, ViewPrimitiveRange { start: 0, end: 2 });
        let ViewPrimitive::RoundedRect(rect) = &scene.primitives()[0] else {
            panic!("solid surface background lowers to rounded fill");
        };
        assert_eq!(rect.radii, radii);
        assert!(matches!(scene.primitives()[1], ViewPrimitive::Border(_)));
    }

    #[test]
    fn prepared_text_ids_preserve_direct_painter_reuse_order() {
        let first = PreparedTextId::from_index(3);
        let second = PreparedTextId::from_index(7);
        let mut scene = ViewScene::new(320.0, 180.0);
        for text in [first, second, first] {
            scene.push_primitive(ViewPrimitive::Text(ViewTextPrimitive { text }));
        }

        assert_eq!(
            scene.prepared_text_ids().collect::<Vec<_>>(),
            [first, second, first]
        );
    }
}
