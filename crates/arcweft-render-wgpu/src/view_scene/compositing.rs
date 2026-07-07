use super::core::{ViewColorRgba8, ViewGradientStop, ViewSceneContext};
use arcweft_presentation::hit::HitRect;

const FILTER_BLUR_OUTSET_MULTIPLIER: f32 = 3.0;
const SHADOW_BLUR_OUTSET_MULTIPLIER: f32 = 1.5;
const EPSILON: f32 = 0.0001;

#[derive(Clone, Debug, PartialEq)]
pub enum ViewPaintNode {
    Direct(ViewSceneContext),
    Group(ViewCompositingGroup),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewCompositingGroup {
    pub bounds: HitRect,
    pub isolation: ViewIsolation,
    pub effects: ViewCompositingEffects,
    pub children: Vec<ViewPaintNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewCompositingEffects {
    pub opacity: f32,
    pub filters: ViewFilterList,
    pub backdrop_filters: ViewFilterList,
    pub box_shadows: ViewBoxShadowList,
    pub masks: Vec<ViewMask>,
    pub clip_path: Option<Box<ViewClipPath>>,
    pub blend_mode: ViewBlendMode,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewCompositingRequirements {
    bits: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewCompositingEffectClass {
    Compositing,
    BackdropCompositing,
    BoxShadow,
    MaskCompositing,
    ClipGeometry,
    Resource,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewFilterList {
    filters: Vec<ViewFilter>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewBoxShadowList {
    shadows: Vec<ViewBoxShadow>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewBoxShadow {
    pub offset_x_px: f32,
    pub offset_y_px: f32,
    pub blur_radius_px: f32,
    pub spread_radius_px: f32,
    pub border_radii: ViewBoxShadowRadii,
    pub color: ViewColorRgba8,
    pub kind: ViewBoxShadowKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewBoxShadowRadii {
    pub top_left: ViewBoxShadowCornerRadius,
    pub top_right: ViewBoxShadowCornerRadius,
    pub bottom_right: ViewBoxShadowCornerRadius,
    pub bottom_left: ViewBoxShadowCornerRadius,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewBoxShadowCornerRadius {
    pub x_px: f32,
    pub y_px: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewBoxShadowCorner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewBoxShadowRadiusAxis {
    X,
    Y,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewBoxShadowKind {
    #[default]
    Outer,
    Inset,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewFilter {
    Brightness(f32),
    Contrast(f32),
    Grayscale(f32),
    Saturate(f32),
    HueRotateDegrees(f32),
    Invert(f32),
    Sepia(f32),
    Opacity(f32),
    Blur {
        radius_px: f32,
    },
    DropShadow {
        offset_x_px: f32,
        offset_y_px: f32,
        blur_radius_px: f32,
        color: ViewColorRgba8,
    },
    Unsupported {
        name: Box<str>,
        reason: Box<str>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewMask {
    pub image: ViewMaskImage,
    pub size: ViewMaskSize,
    pub position: ViewMaskPosition,
    pub repeat: ViewMaskRepeat,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewMaskImage {
    None,
    Url(Box<str>),
    Gradient(ViewMaskGradient),
    Element(ViewElementMaskSource),
    Unsupported(Box<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewMaskGradient {
    Linear {
        angle_degrees: f32,
        stops: Vec<ViewGradientStop>,
    },
    Radial {
        center: ViewPoint,
        radius_x: ViewLength,
        radius_y: ViewLength,
        stops: Vec<ViewGradientStop>,
    },
    Conic {
        center: ViewPoint,
        from_degrees: f32,
        stops: Vec<ViewGradientStop>,
    },
    Unsupported(Box<str>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewElementMaskSource {
    pub element_id: Box<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewMaskSize {
    Unspecified,
    Auto,
    Cover,
    Contain,
    Explicit {
        width: ViewLength,
        height: ViewLength,
    },
    Unsupported(Box<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewMaskPosition {
    pub anchor: ViewPoint,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewMaskRepeat {
    Unspecified,
    Repeat,
    NoRepeat,
    RepeatX,
    RepeatY,
    Space,
    Round,
    Unsupported(Box<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewClipPath {
    Inset {
        inset: [ViewLength; 4],
        radius: [ViewLength; 4],
    },
    Circle {
        radius: ViewShapeRadius,
        center: ViewPoint,
    },
    Ellipse {
        radius_x: ViewShapeRadius,
        radius_y: ViewShapeRadius,
        center: ViewPoint,
    },
    Polygon {
        fill_rule: ViewFillRule,
        points: Vec<ViewPoint>,
    },
    Path {
        fill_rule: ViewFillRule,
        data: Box<str>,
    },
    Url(Box<str>),
    Unsupported(Box<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewLength {
    Auto,
    Px(f32),
    Percent(f32),
    Unsupported(Box<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewShapeRadius {
    ClosestSide,
    FarthestSide,
    Length(ViewLength),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewPoint {
    pub x: ViewLength,
    pub y: ViewLength,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewFillRule {
    #[default]
    NonZero,
    EvenOdd,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewIsolation {
    #[default]
    Auto,
    Isolate,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewBlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
    PlusLighter,
    PlusDarker,
}

impl Default for ViewCompositingEffects {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            filters: ViewFilterList::default(),
            backdrop_filters: ViewFilterList::default(),
            box_shadows: ViewBoxShadowList::default(),
            masks: Vec::new(),
            clip_path: None,
            blend_mode: ViewBlendMode::Normal,
        }
    }
}

impl Default for ViewMask {
    fn default() -> Self {
        Self {
            image: ViewMaskImage::None,
            size: ViewMaskSize::Unspecified,
            position: ViewMaskPosition::default(),
            repeat: ViewMaskRepeat::Unspecified,
        }
    }
}

impl Default for ViewMaskPosition {
    fn default() -> Self {
        Self {
            anchor: ViewPoint::percent(0.0, 0.0),
        }
    }
}

impl ViewPaintNode {
    pub fn visual_outset_px(&self) -> f32 {
        match self {
            Self::Direct(_) => 0.0,
            Self::Group(group) => group.visual_outset_px(),
        }
    }

    pub fn requirements(&self) -> ViewCompositingRequirements {
        match self {
            Self::Direct(_) => ViewCompositingRequirements::default(),
            Self::Group(group) => group.requirements(),
        }
    }
}

impl ViewCompositingGroup {
    pub fn new(bounds: HitRect, effects: ViewCompositingEffects) -> Self {
        Self {
            bounds,
            isolation: ViewIsolation::Auto,
            effects,
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_isolation(mut self, isolation: ViewIsolation) -> Self {
        self.isolation = isolation;
        self
    }

    #[must_use]
    pub fn with_children(mut self, children: Vec<ViewPaintNode>) -> Self {
        self.children = children;
        self
    }

    pub fn visual_outset_px(&self) -> f32 {
        self.children
            .iter()
            .map(ViewPaintNode::visual_outset_px)
            .fold(self.effects.visual_outset_px(), f32::max)
    }

    pub fn visual_bounds(&self) -> HitRect {
        self.bounds.outset(self.visual_outset_px())
    }

    pub fn requirements(&self) -> ViewCompositingRequirements {
        let mut requirements = self.effects.requirements();
        if self.isolation == ViewIsolation::Isolate {
            requirements.insert(ViewCompositingEffectClass::Compositing);
        }
        for child in &self.children {
            requirements.merge(child.requirements());
        }
        requirements
    }

    pub fn requires_offscreen_surface(&self) -> bool {
        self.requirements().requires_offscreen_surface()
    }
}

impl ViewCompositingEffects {
    pub fn is_identity(&self) -> bool {
        self.opacity_is_identity()
            && self.filters.is_empty()
            && self.backdrop_filters.is_empty()
            && self.box_shadows.is_empty()
            && self.masks.is_empty()
            && self.clip_path.is_none()
            && self.blend_mode == ViewBlendMode::Normal
    }

    pub fn visual_outset_px(&self) -> f32 {
        self.filters
            .visual_outset_px()
            .max(self.backdrop_filters.visual_outset_px())
            .max(self.box_shadows.visual_outset_px())
            .max(
                self.masks
                    .iter()
                    .map(ViewMask::visual_outset_px)
                    .fold(0.0, f32::max),
            )
    }

    pub fn requirements(&self) -> ViewCompositingRequirements {
        let mut requirements = ViewCompositingRequirements::default();

        if !self.opacity_is_identity()
            || !self.filters.is_empty()
            || self.blend_mode != ViewBlendMode::Normal
        {
            requirements.insert(ViewCompositingEffectClass::Compositing);
        }
        if !self.backdrop_filters.is_empty() {
            requirements.insert(ViewCompositingEffectClass::BackdropCompositing);
        }
        if !self.box_shadows.is_empty() {
            requirements.insert(ViewCompositingEffectClass::BoxShadow);
            requirements.insert(ViewCompositingEffectClass::Compositing);
        }
        if !self.masks.is_empty() {
            requirements.insert(ViewCompositingEffectClass::MaskCompositing);
        }
        if self.masks.iter().any(ViewMask::requires_resource_revision) {
            requirements.insert(ViewCompositingEffectClass::Resource);
        }
        if self.clip_path.is_some() {
            requirements.insert(ViewCompositingEffectClass::ClipGeometry);
        }

        requirements
    }

    pub fn requires_offscreen_surface(&self) -> bool {
        self.requirements().requires_offscreen_surface()
    }

    fn opacity_is_identity(&self) -> bool {
        (self.opacity - 1.0).abs() <= EPSILON
    }
}

impl ViewCompositingRequirements {
    const COMPOSITING: u16 = 1 << 0;
    const BACKDROP_COMPOSITING: u16 = 1 << 1;
    const BOX_SHADOW: u16 = 1 << 2;
    const MASK_COMPOSITING: u16 = 1 << 3;
    const CLIP_GEOMETRY: u16 = 1 << 4;
    const RESOURCE: u16 = 1 << 5;

    pub fn insert(&mut self, class: ViewCompositingEffectClass) {
        self.bits |= Self::bit(class);
    }

    pub fn contains(self, class: ViewCompositingEffectClass) -> bool {
        self.bits & Self::bit(class) != 0
    }

    pub fn merge(&mut self, other: Self) {
        self.bits |= other.bits;
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub fn requires_offscreen_surface(self) -> bool {
        self.contains(ViewCompositingEffectClass::Compositing)
            || self.contains(ViewCompositingEffectClass::BackdropCompositing)
            || self.contains(ViewCompositingEffectClass::BoxShadow)
            || self.contains(ViewCompositingEffectClass::MaskCompositing)
            || self.contains(ViewCompositingEffectClass::ClipGeometry)
    }

    const fn bit(class: ViewCompositingEffectClass) -> u16 {
        match class {
            ViewCompositingEffectClass::Compositing => Self::COMPOSITING,
            ViewCompositingEffectClass::BackdropCompositing => Self::BACKDROP_COMPOSITING,
            ViewCompositingEffectClass::BoxShadow => Self::BOX_SHADOW,
            ViewCompositingEffectClass::MaskCompositing => Self::MASK_COMPOSITING,
            ViewCompositingEffectClass::ClipGeometry => Self::CLIP_GEOMETRY,
            ViewCompositingEffectClass::Resource => Self::RESOURCE,
        }
    }
}

impl ViewFilterList {
    pub fn new(filters: impl IntoIterator<Item = ViewFilter>) -> Self {
        Self {
            filters: filters.into_iter().collect(),
        }
        .canonicalized()
    }

    pub fn from_filters(filters: Vec<ViewFilter>) -> Self {
        Self::new(filters)
    }

    pub fn filters(&self) -> &[ViewFilter] {
        &self.filters
    }

    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }

    pub fn visual_outset_px(&self) -> f32 {
        self.filters
            .iter()
            .map(ViewFilter::visual_outset_px)
            .fold(0.0, f32::max)
    }

    #[must_use]
    pub fn canonicalized(mut self) -> Self {
        self.filters.retain(|filter| !filter.is_identity());
        self
    }
}

impl From<Vec<ViewFilter>> for ViewFilterList {
    fn from(value: Vec<ViewFilter>) -> Self {
        Self::from_filters(value)
    }
}

impl ViewBoxShadowList {
    pub fn new(shadows: impl IntoIterator<Item = ViewBoxShadow>) -> Self {
        Self {
            shadows: shadows
                .into_iter()
                .filter(|shadow| !shadow.is_identity())
                .collect(),
        }
    }

    pub fn from_shadows(shadows: Vec<ViewBoxShadow>) -> Self {
        Self::new(shadows)
    }

    pub fn shadows(&self) -> &[ViewBoxShadow] {
        &self.shadows
    }

    pub fn is_empty(&self) -> bool {
        self.shadows.is_empty()
    }

    pub fn visual_outset_px(&self) -> f32 {
        self.shadows
            .iter()
            .copied()
            .map(ViewBoxShadow::visual_outset_px)
            .fold(0.0, f32::max)
    }

    pub fn visual_inset_px(&self) -> f32 {
        self.shadows
            .iter()
            .copied()
            .map(ViewBoxShadow::visual_inset_px)
            .fold(0.0, f32::max)
    }
}

impl From<Vec<ViewBoxShadow>> for ViewBoxShadowList {
    fn from(value: Vec<ViewBoxShadow>) -> Self {
        Self::from_shadows(value)
    }
}

impl ViewBoxShadowKind {
    pub const fn is_outer(self) -> bool {
        matches!(self, Self::Outer)
    }

    pub const fn is_inset(self) -> bool {
        matches!(self, Self::Inset)
    }
}

impl ViewBoxShadowCornerRadius {
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

    pub fn is_finite(self) -> bool {
        self.x_px.is_finite() && self.y_px.is_finite()
    }

    fn has_negative(self) -> bool {
        self.x_px < -EPSILON || self.y_px < -EPSILON
    }

    fn non_negative(self) -> Self {
        Self {
            x_px: self.x_px.max(0.0),
            y_px: self.y_px.max(0.0),
        }
    }

    fn with_spread(self, spread_px: f32) -> Self {
        Self {
            x_px: (self.x_px + spread_px).max(0.0),
            y_px: (self.y_px + spread_px).max(0.0),
        }
    }

    fn scaled(self, scale: f32) -> Self {
        Self {
            x_px: self.x_px * scale,
            y_px: self.y_px * scale,
        }
    }
}

impl ViewBoxShadowRadii {
    pub const ZERO: Self = Self {
        top_left: ViewBoxShadowCornerRadius::ZERO,
        top_right: ViewBoxShadowCornerRadius::ZERO,
        bottom_right: ViewBoxShadowCornerRadius::ZERO,
        bottom_left: ViewBoxShadowCornerRadius::ZERO,
    };

    pub const fn uniform(radius_px: f32) -> Self {
        Self {
            top_left: ViewBoxShadowCornerRadius::circular(radius_px),
            top_right: ViewBoxShadowCornerRadius::circular(radius_px),
            bottom_right: ViewBoxShadowCornerRadius::circular(radius_px),
            bottom_left: ViewBoxShadowCornerRadius::circular(radius_px),
        }
    }

    pub const fn from_corners(
        top_left: ViewBoxShadowCornerRadius,
        top_right: ViewBoxShadowCornerRadius,
        bottom_right: ViewBoxShadowCornerRadius,
        bottom_left: ViewBoxShadowCornerRadius,
    ) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub fn corners(self) -> [(ViewBoxShadowCorner, ViewBoxShadowCornerRadius); 4] {
        [
            (ViewBoxShadowCorner::TopLeft, self.top_left),
            (ViewBoxShadowCorner::TopRight, self.top_right),
            (ViewBoxShadowCorner::BottomRight, self.bottom_right),
            (ViewBoxShadowCorner::BottomLeft, self.bottom_left),
        ]
    }

    pub fn is_finite(self) -> bool {
        self.corners().iter().all(|(_, radius)| radius.is_finite())
    }

    #[must_use]
    pub fn clamped_to_rect(self, rect: HitRect) -> Self {
        let radii = self.non_negative();
        let width = rect.width.max(0.0);
        let height = rect.height.max(0.0);
        let scale = corner_scale_limit(
            corner_scale_limit(
                corner_scale_limit(
                    corner_scale_limit(1.0, width, radii.top_left.x_px + radii.top_right.x_px),
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

    #[must_use]
    pub fn with_spread(self, spread_px: f32) -> Self {
        Self {
            top_left: self.top_left.with_spread(spread_px),
            top_right: self.top_right.with_spread(spread_px),
            bottom_right: self.bottom_right.with_spread(spread_px),
            bottom_left: self.bottom_left.with_spread(spread_px),
        }
    }

    fn has_negative(self) -> bool {
        self.corners()
            .iter()
            .any(|(_, radius)| radius.has_negative())
    }

    fn non_negative(self) -> Self {
        Self {
            top_left: self.top_left.non_negative(),
            top_right: self.top_right.non_negative(),
            bottom_right: self.bottom_right.non_negative(),
            bottom_left: self.bottom_left.non_negative(),
        }
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

impl ViewBoxShadow {
    pub const fn outer(
        horizontal_offset_px: f32,
        vertical_offset_px: f32,
        blur_radius_px: f32,
        spread_radius_px: f32,
        border_radius_px: f32,
        color: ViewColorRgba8,
    ) -> Self {
        Self::outer_with_radii(
            horizontal_offset_px,
            vertical_offset_px,
            blur_radius_px,
            spread_radius_px,
            ViewBoxShadowRadii::uniform(border_radius_px),
            color,
        )
    }

    pub const fn outer_with_radii(
        horizontal_offset_px: f32,
        vertical_offset_px: f32,
        blur_radius_px: f32,
        spread_radius_px: f32,
        border_radii: ViewBoxShadowRadii,
        color: ViewColorRgba8,
    ) -> Self {
        Self {
            offset_x_px: horizontal_offset_px,
            offset_y_px: vertical_offset_px,
            blur_radius_px,
            spread_radius_px,
            border_radii,
            color,
            kind: ViewBoxShadowKind::Outer,
        }
    }

    pub const fn inset(
        horizontal_offset_px: f32,
        vertical_offset_px: f32,
        blur_radius_px: f32,
        spread_radius_px: f32,
        border_radius_px: f32,
        color: ViewColorRgba8,
    ) -> Self {
        Self::inset_with_radii(
            horizontal_offset_px,
            vertical_offset_px,
            blur_radius_px,
            spread_radius_px,
            ViewBoxShadowRadii::uniform(border_radius_px),
            color,
        )
    }

    pub const fn inset_with_radii(
        horizontal_offset_px: f32,
        vertical_offset_px: f32,
        blur_radius_px: f32,
        spread_radius_px: f32,
        border_radii: ViewBoxShadowRadii,
        color: ViewColorRgba8,
    ) -> Self {
        Self {
            offset_x_px: horizontal_offset_px,
            offset_y_px: vertical_offset_px,
            blur_radius_px,
            spread_radius_px,
            border_radii,
            color,
            kind: ViewBoxShadowKind::Inset,
        }
    }

    pub fn visual_outset_px(self) -> f32 {
        if self.kind.is_inset() || self.color.alpha == 0 {
            return 0.0;
        }
        self.offset_x_px.abs().max(self.offset_y_px.abs())
            + positive(self.spread_radius_px)
            + positive(self.blur_radius_px) * FILTER_BLUR_OUTSET_MULTIPLIER
    }

    pub fn visual_inset_px(self) -> f32 {
        if self.kind.is_outer() || self.color.alpha == 0 {
            return 0.0;
        }
        self.offset_x_px.abs().max(self.offset_y_px.abs())
            + positive(self.spread_radius_px)
            + positive(self.blur_radius_px) * FILTER_BLUR_OUTSET_MULTIPLIER
    }

    pub fn is_identity(self) -> bool {
        if self.color.alpha == 0 {
            return true;
        }
        if !self.offset_x_px.is_finite()
            || !self.offset_y_px.is_finite()
            || !self.blur_radius_px.is_finite()
            || !self.spread_radius_px.is_finite()
            || !self.border_radii.is_finite()
            || self.border_radii.has_negative()
        {
            return false;
        }
        match self.kind {
            ViewBoxShadowKind::Outer => {
                self.offset_x_px.abs() <= EPSILON
                    && self.offset_y_px.abs() <= EPSILON
                    && positive(self.blur_radius_px) <= EPSILON
                    && self.spread_radius_px <= EPSILON
            }
            ViewBoxShadowKind::Inset => {
                self.offset_x_px.abs() <= EPSILON
                    && self.offset_y_px.abs() <= EPSILON
                    && positive(self.blur_radius_px) <= EPSILON
                    && self.spread_radius_px.abs() <= EPSILON
            }
        }
    }
}

impl ViewFilter {
    pub fn visual_outset_px(&self) -> f32 {
        match self {
            Self::Blur { radius_px } => positive(*radius_px) * FILTER_BLUR_OUTSET_MULTIPLIER,
            Self::DropShadow {
                offset_x_px,
                offset_y_px,
                blur_radius_px,
                ..
            } => {
                let blur_outset = positive(*blur_radius_px) * SHADOW_BLUR_OUTSET_MULTIPLIER;
                (offset_x_px.abs() + blur_outset).max(offset_y_px.abs() + blur_outset)
            }
            Self::Brightness(_)
            | Self::Contrast(_)
            | Self::Grayscale(_)
            | Self::Saturate(_)
            | Self::HueRotateDegrees(_)
            | Self::Invert(_)
            | Self::Sepia(_)
            | Self::Opacity(_)
            | Self::Unsupported { .. } => 0.0,
        }
    }

    pub fn is_identity(&self) -> bool {
        match self {
            Self::Brightness(value) | Self::Contrast(value) | Self::Saturate(value) => {
                (*value - 1.0).abs() <= EPSILON
            }
            Self::Grayscale(value) | Self::Invert(value) | Self::Sepia(value) => {
                value.abs() <= EPSILON
            }
            Self::HueRotateDegrees(value) => value.abs() <= EPSILON,
            Self::Opacity(value) => (*value - 1.0).abs() <= EPSILON,
            Self::Blur { radius_px } => positive(*radius_px) <= EPSILON,
            Self::DropShadow {
                offset_x_px,
                offset_y_px,
                blur_radius_px,
                color,
            } => {
                offset_x_px.abs() <= EPSILON
                    && offset_y_px.abs() <= EPSILON
                    && positive(*blur_radius_px) <= EPSILON
                    && color.alpha == 0
            }
            Self::Unsupported { .. } => false,
        }
    }
}

impl ViewMask {
    pub fn visual_outset_px(&self) -> f32 {
        0.0
    }

    pub fn requires_resource_revision(&self) -> bool {
        self.image.requires_resource_revision()
    }
}

impl ViewMaskImage {
    pub fn requires_resource_revision(&self) -> bool {
        matches!(self, Self::Url(_) | Self::Element(_))
    }
}

impl ViewMaskGradient {
    pub fn stops(&self) -> Option<&[ViewGradientStop]> {
        match self {
            Self::Linear { stops, .. } | Self::Radial { stops, .. } | Self::Conic { stops, .. } => {
                Some(stops)
            }
            Self::Unsupported(_) => None,
        }
    }

    pub fn is_supported(&self) -> bool {
        !matches!(self, Self::Unsupported(_))
    }
}

impl ViewLength {
    pub fn px(value: f32) -> Self {
        Self::Px(value)
    }

    pub fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    pub fn resolve_px(&self, percentage_basis_px: f32) -> Option<f32> {
        match self {
            Self::Auto | Self::Unsupported(_) => None,
            Self::Px(value) => Some(*value),
            Self::Percent(value) => Some(*value * percentage_basis_px),
        }
    }

    pub fn is_zero(&self) -> bool {
        matches!(self, Self::Px(value) if value.abs() <= EPSILON)
            || matches!(self, Self::Percent(value) if value.abs() <= EPSILON)
    }
}

impl Default for ViewLength {
    fn default() -> Self {
        Self::Px(0.0)
    }
}

impl ViewPoint {
    pub fn px(x: f32, y: f32) -> Self {
        Self {
            x: ViewLength::Px(x),
            y: ViewLength::Px(y),
        }
    }

    pub fn percent(x: f32, y: f32) -> Self {
        Self {
            x: ViewLength::Percent(x),
            y: ViewLength::Percent(y),
        }
    }
}

fn corner_scale_limit(current: f32, basis_px: f32, sum_px: f32) -> f32 {
    if sum_px <= EPSILON {
        current
    } else {
        current.min((basis_px / sum_px).clamp(0.0, 1.0))
    }
}

fn positive(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EPSILON, ViewBlendMode, ViewColorRgba8, ViewCompositingEffectClass, ViewCompositingEffects,
        ViewFilter, ViewFilterList, ViewMask, ViewMaskImage,
    };

    #[test]
    fn filter_list_canonicalization_drops_identity_filters_but_keeps_order() {
        let filters = ViewFilterList::new([
            ViewFilter::Brightness(1.0),
            ViewFilter::Blur { radius_px: 4.0 },
            ViewFilter::Opacity(1.0),
            ViewFilter::Contrast(0.5),
        ]);

        assert_eq!(
            filters.filters(),
            &[
                ViewFilter::Blur { radius_px: 4.0 },
                ViewFilter::Contrast(0.5)
            ]
        );
    }

    #[test]
    fn filter_visual_outset_uses_css_blur_and_shadow_extents() {
        let filters = ViewFilterList::new([
            ViewFilter::Blur { radius_px: 6.0 },
            ViewFilter::DropShadow {
                offset_x_px: 4.0,
                offset_y_px: -20.0,
                blur_radius_px: 8.0,
                color: ViewColorRgba8 {
                    red: 1,
                    green: 2,
                    blue: 3,
                    alpha: 255,
                },
            },
        ]);

        assert!((filters.visual_outset_px() - 32.0).abs() <= EPSILON);
    }

    #[test]
    fn compositing_requirements_are_deterministic() {
        let effects = ViewCompositingEffects {
            blend_mode: ViewBlendMode::Multiply,
            masks: vec![ViewMask {
                image: ViewMaskImage::Url("arcweft://mask/1".into()),
                ..ViewMask::default()
            }],
            ..ViewCompositingEffects::default()
        };

        let requirements = effects.requirements();
        assert!(requirements.contains(ViewCompositingEffectClass::Compositing));
        assert!(requirements.contains(ViewCompositingEffectClass::MaskCompositing));
        assert!(requirements.contains(ViewCompositingEffectClass::Resource));
        assert!(!requirements.contains(ViewCompositingEffectClass::BackdropCompositing));
    }
}
