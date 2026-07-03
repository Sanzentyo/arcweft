use super::core::{UiColorRgba8, UiGradientStop, UiSceneContext};
use arcweft_presentation::hit::HitRect;

const FILTER_BLUR_OUTSET_MULTIPLIER: f32 = 3.0;
const SHADOW_BLUR_OUTSET_MULTIPLIER: f32 = 1.5;
const EPSILON: f32 = 0.0001;

#[derive(Clone, Debug, PartialEq)]
pub enum UiPaintNode {
    Direct(UiSceneContext),
    Group(UiCompositingGroup),
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiCompositingGroup {
    pub bounds: HitRect,
    pub isolation: UiIsolation,
    pub effects: UiCompositingEffects,
    pub children: Vec<UiPaintNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiCompositingEffects {
    pub opacity: f32,
    pub filters: UiFilterList,
    pub backdrop_filters: UiFilterList,
    pub box_shadows: UiBoxShadowList,
    pub masks: Vec<UiMask>,
    pub clip_path: Option<Box<UiClipPath>>,
    pub blend_mode: UiBlendMode,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiCompositingRequirements {
    bits: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiCompositingEffectClass {
    Compositing,
    BackdropCompositing,
    BoxShadow,
    MaskCompositing,
    ClipGeometry,
    Resource,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiFilterList {
    filters: Vec<UiFilter>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiBoxShadowList {
    shadows: Vec<UiBoxShadow>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiBoxShadow {
    pub offset_x_px: f32,
    pub offset_y_px: f32,
    pub blur_radius_px: f32,
    pub spread_radius_px: f32,
    pub border_radius_px: f32,
    pub color: UiColorRgba8,
    pub kind: UiBoxShadowKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UiBoxShadowKind {
    #[default]
    Outer,
    Inset,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiFilter {
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
        color: UiColorRgba8,
    },
    Unsupported {
        name: Box<str>,
        reason: Box<str>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiMask {
    pub image: UiMaskImage,
    pub size: UiMaskSize,
    pub position: UiMaskPosition,
    pub repeat: UiMaskRepeat,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiMaskImage {
    None,
    Url(Box<str>),
    Gradient(UiMaskGradient),
    Element(UiElementMaskSource),
    Unsupported(Box<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiMaskGradient {
    Linear {
        angle_degrees: f32,
        stops: Vec<UiGradientStop>,
    },
    Radial {
        center: UiPoint,
        radius_x: UiLength,
        radius_y: UiLength,
        stops: Vec<UiGradientStop>,
    },
    Conic {
        center: UiPoint,
        from_degrees: f32,
        stops: Vec<UiGradientStop>,
    },
    Unsupported(Box<str>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiElementMaskSource {
    pub element_id: Box<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiMaskSize {
    Unspecified,
    Auto,
    Cover,
    Contain,
    Explicit { width: UiLength, height: UiLength },
    Unsupported(Box<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiMaskPosition {
    pub anchor: UiPoint,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiMaskRepeat {
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
pub enum UiClipPath {
    Inset {
        inset: [UiLength; 4],
        radius: [UiLength; 4],
    },
    Circle {
        radius: UiShapeRadius,
        center: UiPoint,
    },
    Ellipse {
        radius_x: UiShapeRadius,
        radius_y: UiShapeRadius,
        center: UiPoint,
    },
    Polygon {
        fill_rule: UiFillRule,
        points: Vec<UiPoint>,
    },
    Path {
        fill_rule: UiFillRule,
        data: Box<str>,
    },
    Url(Box<str>),
    Unsupported(Box<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiLength {
    Auto,
    Px(f32),
    Percent(f32),
    Unsupported(Box<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiShapeRadius {
    ClosestSide,
    FarthestSide,
    Length(UiLength),
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiPoint {
    pub x: UiLength,
    pub y: UiLength,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UiFillRule {
    #[default]
    NonZero,
    EvenOdd,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UiIsolation {
    #[default]
    Auto,
    Isolate,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UiBlendMode {
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

impl Default for UiCompositingEffects {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            filters: UiFilterList::default(),
            backdrop_filters: UiFilterList::default(),
            box_shadows: UiBoxShadowList::default(),
            masks: Vec::new(),
            clip_path: None,
            blend_mode: UiBlendMode::Normal,
        }
    }
}

impl Default for UiMask {
    fn default() -> Self {
        Self {
            image: UiMaskImage::None,
            size: UiMaskSize::Unspecified,
            position: UiMaskPosition::default(),
            repeat: UiMaskRepeat::Unspecified,
        }
    }
}

impl Default for UiMaskPosition {
    fn default() -> Self {
        Self {
            anchor: UiPoint::percent(0.0, 0.0),
        }
    }
}

impl UiPaintNode {
    pub fn visual_outset_px(&self) -> f32 {
        match self {
            Self::Direct(_) => 0.0,
            Self::Group(group) => group.visual_outset_px(),
        }
    }

    pub fn requirements(&self) -> UiCompositingRequirements {
        match self {
            Self::Direct(_) => UiCompositingRequirements::default(),
            Self::Group(group) => group.requirements(),
        }
    }
}

impl UiCompositingGroup {
    pub fn new(bounds: HitRect, effects: UiCompositingEffects) -> Self {
        Self {
            bounds,
            isolation: UiIsolation::Auto,
            effects,
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_isolation(mut self, isolation: UiIsolation) -> Self {
        self.isolation = isolation;
        self
    }

    #[must_use]
    pub fn with_children(mut self, children: Vec<UiPaintNode>) -> Self {
        self.children = children;
        self
    }

    pub fn visual_outset_px(&self) -> f32 {
        self.children
            .iter()
            .map(UiPaintNode::visual_outset_px)
            .fold(self.effects.visual_outset_px(), f32::max)
    }

    pub fn visual_bounds(&self) -> HitRect {
        self.bounds.outset(self.visual_outset_px())
    }

    pub fn requirements(&self) -> UiCompositingRequirements {
        let mut requirements = self.effects.requirements();
        if self.isolation == UiIsolation::Isolate {
            requirements.insert(UiCompositingEffectClass::Compositing);
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

impl UiCompositingEffects {
    pub fn is_identity(&self) -> bool {
        self.opacity_is_identity()
            && self.filters.is_empty()
            && self.backdrop_filters.is_empty()
            && self.box_shadows.is_empty()
            && self.masks.is_empty()
            && self.clip_path.is_none()
            && self.blend_mode == UiBlendMode::Normal
    }

    pub fn visual_outset_px(&self) -> f32 {
        self.filters
            .visual_outset_px()
            .max(self.backdrop_filters.visual_outset_px())
            .max(self.box_shadows.visual_outset_px())
            .max(
                self.masks
                    .iter()
                    .map(UiMask::visual_outset_px)
                    .fold(0.0, f32::max),
            )
    }

    pub fn requirements(&self) -> UiCompositingRequirements {
        let mut requirements = UiCompositingRequirements::default();

        if !self.opacity_is_identity()
            || !self.filters.is_empty()
            || self.blend_mode != UiBlendMode::Normal
        {
            requirements.insert(UiCompositingEffectClass::Compositing);
        }
        if !self.backdrop_filters.is_empty() {
            requirements.insert(UiCompositingEffectClass::BackdropCompositing);
        }
        if !self.box_shadows.is_empty() {
            requirements.insert(UiCompositingEffectClass::BoxShadow);
            requirements.insert(UiCompositingEffectClass::Compositing);
        }
        if !self.masks.is_empty() {
            requirements.insert(UiCompositingEffectClass::MaskCompositing);
        }
        if self.masks.iter().any(UiMask::requires_resource_revision) {
            requirements.insert(UiCompositingEffectClass::Resource);
        }
        if self.clip_path.is_some() {
            requirements.insert(UiCompositingEffectClass::ClipGeometry);
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

impl UiCompositingRequirements {
    const COMPOSITING: u16 = 1 << 0;
    const BACKDROP_COMPOSITING: u16 = 1 << 1;
    const BOX_SHADOW: u16 = 1 << 2;
    const MASK_COMPOSITING: u16 = 1 << 3;
    const CLIP_GEOMETRY: u16 = 1 << 4;
    const RESOURCE: u16 = 1 << 5;

    pub fn insert(&mut self, class: UiCompositingEffectClass) {
        self.bits |= Self::bit(class);
    }

    pub fn contains(self, class: UiCompositingEffectClass) -> bool {
        self.bits & Self::bit(class) != 0
    }

    pub fn merge(&mut self, other: Self) {
        self.bits |= other.bits;
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub fn requires_offscreen_surface(self) -> bool {
        self.contains(UiCompositingEffectClass::Compositing)
            || self.contains(UiCompositingEffectClass::BackdropCompositing)
            || self.contains(UiCompositingEffectClass::BoxShadow)
            || self.contains(UiCompositingEffectClass::MaskCompositing)
            || self.contains(UiCompositingEffectClass::ClipGeometry)
    }

    const fn bit(class: UiCompositingEffectClass) -> u16 {
        match class {
            UiCompositingEffectClass::Compositing => Self::COMPOSITING,
            UiCompositingEffectClass::BackdropCompositing => Self::BACKDROP_COMPOSITING,
            UiCompositingEffectClass::BoxShadow => Self::BOX_SHADOW,
            UiCompositingEffectClass::MaskCompositing => Self::MASK_COMPOSITING,
            UiCompositingEffectClass::ClipGeometry => Self::CLIP_GEOMETRY,
            UiCompositingEffectClass::Resource => Self::RESOURCE,
        }
    }
}

impl UiFilterList {
    pub fn new(filters: impl IntoIterator<Item = UiFilter>) -> Self {
        Self {
            filters: filters.into_iter().collect(),
        }
        .canonicalized()
    }

    pub fn from_filters(filters: Vec<UiFilter>) -> Self {
        Self::new(filters)
    }

    pub fn filters(&self) -> &[UiFilter] {
        &self.filters
    }

    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }

    pub fn visual_outset_px(&self) -> f32 {
        self.filters
            .iter()
            .map(UiFilter::visual_outset_px)
            .fold(0.0, f32::max)
    }

    #[must_use]
    pub fn canonicalized(mut self) -> Self {
        self.filters.retain(|filter| !filter.is_identity());
        self
    }
}

impl From<Vec<UiFilter>> for UiFilterList {
    fn from(value: Vec<UiFilter>) -> Self {
        Self::from_filters(value)
    }
}

impl UiBoxShadowList {
    pub fn new(shadows: impl IntoIterator<Item = UiBoxShadow>) -> Self {
        Self {
            shadows: shadows
                .into_iter()
                .filter(|shadow| !shadow.is_identity())
                .collect(),
        }
    }

    pub fn from_shadows(shadows: Vec<UiBoxShadow>) -> Self {
        Self::new(shadows)
    }

    pub fn shadows(&self) -> &[UiBoxShadow] {
        &self.shadows
    }

    pub fn is_empty(&self) -> bool {
        self.shadows.is_empty()
    }

    pub fn visual_outset_px(&self) -> f32 {
        self.shadows
            .iter()
            .copied()
            .map(UiBoxShadow::visual_outset_px)
            .fold(0.0, f32::max)
    }

    pub fn visual_inset_px(&self) -> f32 {
        self.shadows
            .iter()
            .copied()
            .map(UiBoxShadow::visual_inset_px)
            .fold(0.0, f32::max)
    }
}

impl From<Vec<UiBoxShadow>> for UiBoxShadowList {
    fn from(value: Vec<UiBoxShadow>) -> Self {
        Self::from_shadows(value)
    }
}

impl UiBoxShadowKind {
    pub const fn is_outer(self) -> bool {
        matches!(self, Self::Outer)
    }

    pub const fn is_inset(self) -> bool {
        matches!(self, Self::Inset)
    }
}

impl UiBoxShadow {
    pub const fn outer(
        horizontal_offset_px: f32,
        vertical_offset_px: f32,
        blur_radius_px: f32,
        spread_radius_px: f32,
        border_radius_px: f32,
        color: UiColorRgba8,
    ) -> Self {
        Self {
            offset_x_px: horizontal_offset_px,
            offset_y_px: vertical_offset_px,
            blur_radius_px,
            spread_radius_px,
            border_radius_px,
            color,
            kind: UiBoxShadowKind::Outer,
        }
    }

    pub const fn inset(
        horizontal_offset_px: f32,
        vertical_offset_px: f32,
        blur_radius_px: f32,
        spread_radius_px: f32,
        border_radius_px: f32,
        color: UiColorRgba8,
    ) -> Self {
        Self {
            offset_x_px: horizontal_offset_px,
            offset_y_px: vertical_offset_px,
            blur_radius_px,
            spread_radius_px,
            border_radius_px,
            color,
            kind: UiBoxShadowKind::Inset,
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
            || !self.border_radius_px.is_finite()
        {
            return false;
        }
        match self.kind {
            UiBoxShadowKind::Outer => {
                self.offset_x_px.abs() <= EPSILON
                    && self.offset_y_px.abs() <= EPSILON
                    && positive(self.blur_radius_px) <= EPSILON
                    && self.spread_radius_px <= EPSILON
            }
            UiBoxShadowKind::Inset => {
                self.offset_x_px.abs() <= EPSILON
                    && self.offset_y_px.abs() <= EPSILON
                    && positive(self.blur_radius_px) <= EPSILON
                    && self.spread_radius_px.abs() <= EPSILON
            }
        }
    }
}

impl UiFilter {
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

impl UiMask {
    pub fn visual_outset_px(&self) -> f32 {
        0.0
    }

    pub fn requires_resource_revision(&self) -> bool {
        self.image.requires_resource_revision()
    }
}

impl UiMaskImage {
    pub fn requires_resource_revision(&self) -> bool {
        matches!(self, Self::Url(_) | Self::Element(_))
    }
}

impl UiMaskGradient {
    pub fn stops(&self) -> Option<&[UiGradientStop]> {
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

impl UiLength {
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

impl Default for UiLength {
    fn default() -> Self {
        Self::Px(0.0)
    }
}

impl UiPoint {
    pub fn px(x: f32, y: f32) -> Self {
        Self {
            x: UiLength::Px(x),
            y: UiLength::Px(y),
        }
    }

    pub fn percent(x: f32, y: f32) -> Self {
        Self {
            x: UiLength::Percent(x),
            y: UiLength::Percent(y),
        }
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
        EPSILON, UiBlendMode, UiColorRgba8, UiCompositingEffectClass, UiCompositingEffects,
        UiFilter, UiFilterList, UiMask, UiMaskImage,
    };

    #[test]
    fn filter_list_canonicalization_drops_identity_filters_but_keeps_order() {
        let filters = UiFilterList::new([
            UiFilter::Brightness(1.0),
            UiFilter::Blur { radius_px: 4.0 },
            UiFilter::Opacity(1.0),
            UiFilter::Contrast(0.5),
        ]);

        assert_eq!(
            filters.filters(),
            &[UiFilter::Blur { radius_px: 4.0 }, UiFilter::Contrast(0.5)]
        );
    }

    #[test]
    fn filter_visual_outset_uses_css_blur_and_shadow_extents() {
        let filters = UiFilterList::new([
            UiFilter::Blur { radius_px: 6.0 },
            UiFilter::DropShadow {
                offset_x_px: 4.0,
                offset_y_px: -20.0,
                blur_radius_px: 8.0,
                color: UiColorRgba8 {
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
        let effects = UiCompositingEffects {
            blend_mode: UiBlendMode::Multiply,
            masks: vec![UiMask {
                image: UiMaskImage::Url("arcweft://mask/1".into()),
                ..UiMask::default()
            }],
            ..UiCompositingEffects::default()
        };

        let requirements = effects.requirements();
        assert!(requirements.contains(UiCompositingEffectClass::Compositing));
        assert!(requirements.contains(UiCompositingEffectClass::MaskCompositing));
        assert!(requirements.contains(UiCompositingEffectClass::Resource));
        assert!(!requirements.contains(UiCompositingEffectClass::BackdropCompositing));
    }
}
