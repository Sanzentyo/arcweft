use crate::{
    capture::{TakumiCaptureFrame, TakumiCaptureRecord},
    diagnostic::TakumiAdapterError,
    metadata::{TakumiMetadataMap, TakumiPath},
    text::ArcweftTextLayoutBridge,
};
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_scene::{
    UiAffine2, UiBlendMode, UiBorder, UiClip, UiClipPath, UiColorRgba8, UiCompositingEffects,
    UiCompositingGroup, UiFillRule, UiFilter, UiFilterList, UiGradientStop, UiImagePrimitive,
    UiIsolation, UiLength, UiLinearGradient, UiMask, UiMaskImage, UiPaintNode, UiPoint,
    UiPrimitive, UiPrimitiveRange, UiRoundedRect, UiScene, UiSceneContext, UiShapeRadius,
    UiSolidRect,
};
use num_traits::ToPrimitive;
use std::{collections::HashMap, rc::Rc, sync::Arc};
use taffy::Size;
use takumi::prelude::{Fonts, ImageSource, Node, StyleSheet, Viewport};
use takumi::unstable::base::{
    context::RenderContext,
    layout::{
        style::{
            Affine, BackgroundImage, BasicShape, BlendMode as TakumiBlendMode,
            Color as TakumiColor, ComputedStyle, FillRule as TakumiFillRule,
            Filter as TakumiFilter, Isolation as TakumiIsolation, Length,
            ShapeRadius as TakumiShapeRadius, SizingContext,
        },
        tree::{LayoutResults, LayoutTree, RenderNode},
    },
    scene::{NodePaint, PaintItemKind, StackingContextNode, build_stacking_contexts},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DirectPaintCatalog {
    entries: Vec<(TakumiPath, DirectBoxPaint)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TakumiCompositingStyleCatalog {
    entries: Vec<(TakumiPath, TakumiCompositingStyle)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TakumiCompositingStyle {
    pub isolation: UiIsolation,
    pub effects: UiCompositingEffects,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirectBoxPaint {
    pub background: Option<DirectBackground>,
    pub border: Option<DirectBorder>,
    pub clip: Option<DirectClip>,
    pub opacity: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DirectBackground {
    Solid {
        color: UiColorRgba8,
        radius: f32,
    },
    LinearGradient {
        angle_degrees: f32,
        stops: Vec<UiGradientStop>,
    },
    Image {
        resource_index: u32,
        opacity: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectBorder {
    pub width: f32,
    pub radius: f32,
    pub color: UiColorRgba8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DirectClip {
    Rect,
    RoundedRect { radius: f32 },
}

pub struct TakumiSceneInput<'a> {
    pub node: Node,
    pub stylesheet: StyleSheet,
    pub metadata: TakumiMetadataMap,
    pub direct_paint: &'a DirectPaintCatalog,
    pub text: &'a ArcweftTextLayoutBridge,
    pub fonts: &'a Fonts,
    pub images: HashMap<Arc<str>, ImageSource>,
    pub viewport: Viewport,
    pub time_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TakumiSceneOutput {
    pub scene: UiScene,
    pub capture: TakumiCaptureFrame,
}

#[derive(Clone, Debug, Default)]
pub struct TakumiSceneLowerer;

#[derive(Default)]
struct UiSceneBuild {
    viewport_width: f32,
    viewport_height: f32,
    primitives: Vec<UiPrimitive>,
    contexts: Vec<UiSceneContext>,
    paint_nodes: Vec<UiPaintNode>,
    capture: TakumiCaptureFrame,
}

struct TakumiLoweringRefs<'a> {
    contexts: &'a [StackingContextNode],
    layout_results: &'a LayoutResults,
    metadata: &'a TakumiMetadataMap,
    direct_paint: &'a DirectPaintCatalog,
    text: &'a ArcweftTextLayoutBridge,
    compositing_styles: &'a TakumiCompositingStyleCatalog,
}

impl DirectPaintCatalog {
    pub fn insert(&mut self, path: TakumiPath, paint: DirectBoxPaint) {
        self.entries.push((path, paint));
    }

    pub fn get(&self, path: &TakumiPath) -> Option<&DirectBoxPaint> {
        self.entries
            .iter()
            .find(|(candidate, _)| candidate == path)
            .map(|(_, paint)| paint)
    }

    pub fn entries(&self) -> &[(TakumiPath, DirectBoxPaint)] {
        &self.entries
    }
}

impl TakumiCompositingStyleCatalog {
    pub fn insert(&mut self, path: TakumiPath, style: TakumiCompositingStyle) {
        self.entries.push((path, style));
    }

    pub fn get(&self, path: &TakumiPath) -> Option<&TakumiCompositingStyle> {
        self.entries
            .iter()
            .find(|(candidate, _)| candidate == path)
            .map(|(_, style)| style)
    }

    pub fn entries(&self) -> &[(TakumiPath, TakumiCompositingStyle)] {
        &self.entries
    }

    pub fn from_render_tree(root: &RenderNode) -> Self {
        let mut catalog = Self::default();
        collect_render_node_styles(&TakumiPath::root(), root, &mut catalog);
        catalog
    }
}

impl TakumiCompositingStyle {
    pub fn from_computed_style(
        style: &ComputedStyle,
        sizing: &SizingContext,
        current_color: TakumiColor,
    ) -> Self {
        Self {
            isolation: ui_isolation_from_takumi(style.isolation),
            effects: compositing_effects_from_takumi(style, sizing, current_color),
        }
    }

    pub fn is_identity(&self) -> bool {
        self.isolation == UiIsolation::Auto && self.effects.is_identity()
    }
}

impl DirectBoxPaint {
    pub fn new() -> Self {
        Self {
            background: None,
            border: None,
            clip: None,
            opacity: 1.0,
        }
    }

    #[must_use]
    pub fn with_background(mut self, background: DirectBackground) -> Self {
        self.background = Some(background);
        self
    }

    #[must_use]
    pub fn with_border(mut self, border: DirectBorder) -> Self {
        self.border = Some(border);
        self
    }

    #[must_use]
    pub fn with_clip(mut self, clip: DirectClip) -> Self {
        self.clip = Some(clip);
        self
    }

    #[must_use]
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
}

impl Default for DirectBoxPaint {
    fn default() -> Self {
        Self::new()
    }
}

impl TakumiSceneLowerer {
    pub fn lower(input: TakumiSceneInput<'_>) -> Result<TakumiSceneOutput, TakumiAdapterError> {
        let render_context = RenderContext::builder()
            .fonts(input.fonts.snapshot_with_fallbacks(None))
            .sizing(SizingContext::builder().viewport(input.viewport).build())
            .images(Rc::new(input.images))
            .stylesheet(Rc::new(input.stylesheet))
            .time_ms(input.time_ms)
            .draw_debug_border(false)
            .style(Box::<ComputedStyle>::default())
            .build();

        let root = RenderNode::from_node(&render_context, input.node);
        let compositing_styles = TakumiCompositingStyleCatalog::from_render_tree(&root);

        let mut tree = LayoutTree::from_render_node(&root);
        tree.compute_layout(input.viewport.into());
        let layout_results = tree.into_results();
        let root_node_id = layout_results.root_node_id();
        let contexts = build_stacking_contexts(
            &root,
            &layout_results,
            root_node_id,
            Affine::IDENTITY,
            Size {
                width: input.viewport.size.width.map(viewport_dimension_to_f32),
                height: input.viewport.size.height.map(viewport_dimension_to_f32),
            },
        )
        .map_err(|error| TakumiAdapterError::scene_extraction(error.to_string()))?;

        let mut build = UiSceneBuild::new(
            viewport_dimension_to_f32(input.viewport.size.width.unwrap_or_default()),
            viewport_dimension_to_f32(input.viewport.size.height.unwrap_or_default()),
        );
        let lowering_refs = TakumiLoweringRefs {
            contexts: &contexts,
            layout_results: &layout_results,
            metadata: &input.metadata,
            direct_paint: input.direct_paint,
            text: input.text,
            compositing_styles: &compositing_styles,
        };

        if let Some(root_node) = lower_context(0, &lowering_refs, &mut build)? {
            build.push_paint_node(root_node);
        }
        Ok(build.finish())
    }
}

fn viewport_dimension_to_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX)
}

impl UiSceneBuild {
    fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            viewport_width,
            viewport_height,
            primitives: Vec::new(),
            contexts: Vec::new(),
            paint_nodes: Vec::new(),
            capture: TakumiCaptureFrame::default(),
        }
    }

    fn primitive_start(&self) -> Result<u32, TakumiAdapterError> {
        u32::try_from(self.primitives.len()).map_err(|_| TakumiAdapterError::CapacityExceeded)
    }

    fn push_primitive(&mut self, primitive: UiPrimitive) {
        self.primitives.push(primitive);
    }

    fn push_context(&mut self, context: UiSceneContext) {
        self.contexts.push(context);
    }

    fn push_paint_node(&mut self, node: UiPaintNode) {
        self.paint_nodes.push(node);
    }

    fn finish(self) -> TakumiSceneOutput {
        let mut scene = UiScene::new(self.viewport_width, self.viewport_height);
        for primitive in self.primitives {
            scene.push_primitive(primitive);
        }
        for context in self.contexts {
            scene.push_context(context);
        }
        scene.replace_paint_nodes(self.paint_nodes);
        TakumiSceneOutput {
            scene,
            capture: self.capture,
        }
    }
}

fn lower_context(
    context_id: usize,
    refs: &TakumiLoweringRefs<'_>,
    build: &mut UiSceneBuild,
) -> Result<Option<UiPaintNode>, TakumiAdapterError> {
    let Some(context) = refs.contexts.get(context_id) else {
        return Ok(None);
    };

    let mut children = Vec::new();
    let mut root_path = None;
    let mut bounds = None;

    if let Some(root) = context.root() {
        root_path = Some(TakumiPath::from(root.path.clone()));
        bounds = Some(bounds_for_node(root, refs.layout_results)?);
        if let Some(node) = lower_node(root, refs, build)? {
            children.push(node);
        }
    }
    for bucket in context.in_paint_order() {
        for item in bucket {
            match &item.kind {
                PaintItemKind::Node(node) => {
                    if let Some(node) = lower_node(node, refs, build)? {
                        children.push(node);
                    }
                }
                PaintItemKind::Context(child_context) => {
                    if let Some(node) = lower_context(*child_context, refs, build)? {
                        children.push(node);
                    }
                }
            }
        }
    }

    if children.is_empty() {
        return Ok(None);
    }

    let compositing = root_path
        .as_ref()
        .and_then(|path| refs.compositing_styles.get(path))
        .cloned()
        .unwrap_or_default();
    let group = UiCompositingGroup {
        bounds: bounds.unwrap_or_else(|| HitRect::new(0.0, 0.0, 0.0, 0.0)),
        isolation: compositing.isolation,
        effects: compositing.effects,
        children,
    };

    Ok(Some(UiPaintNode::Group(group)))
}

fn lower_node(
    node: &NodePaint,
    refs: &TakumiLoweringRefs<'_>,
    build: &mut UiSceneBuild,
) -> Result<Option<UiPaintNode>, TakumiAdapterError> {
    let bounds = bounds_for_node(node, refs.layout_results)?;
    let path = TakumiPath::from(node.path.clone());
    let transform = affine_to_ui(node.transform.to_cols_array());
    let paint = refs.direct_paint.get(&path);
    let start = build.primitive_start()?;

    if let Some(paint) = paint {
        if let Some(background) = &paint.background {
            lower_background(background, bounds, build);
        }
        if let Some(border) = paint.border {
            build.push_primitive(UiPrimitive::Border(UiBorder {
                bounds,
                radius: border.radius,
                width: border.width,
                color: border.color,
            }));
        }
    }

    if let Some(metadata) = refs.metadata.get_by_path(&path)
        && let Some(participant) = refs.text.get(metadata.node())
    {
        for glyph_run in participant.glyph_runs() {
            build.push_primitive(glyph_run.clone().into_primitive());
        }
    }

    let end = build.primitive_start()?;
    if start == end {
        return Ok(None);
    }

    let clip = paint.and_then(|paint| paint.clip.map(|clip| clip.to_ui_clip(bounds)));
    let opacity = paint.map_or(1.0, |paint| paint.opacity);
    let primitive_range = UiPrimitiveRange { start, end };
    let scene_context = UiSceneContext {
        transform,
        opacity,
        clip: clip.clone(),
        primitive_range,
    };
    build.push_context(scene_context.clone());
    if let Some(metadata) = refs.metadata.get_by_path(&path) {
        build.capture.push(TakumiCaptureRecord::new(
            metadata.clone(),
            primitive_range,
            bounds,
            transform,
            clip,
        ));
    }
    Ok(Some(UiPaintNode::Direct(scene_context)))
}

fn bounds_for_node(
    node: &NodePaint,
    layout_results: &LayoutResults,
) -> Result<HitRect, TakumiAdapterError> {
    let layout = layout_results
        .layout(node.node_id)
        .map_err(|error| TakumiAdapterError::scene_extraction(error.to_string()))?;
    Ok(HitRect::new(
        0.0,
        0.0,
        layout.size.width,
        layout.size.height,
    ))
}

fn lower_background(background: &DirectBackground, bounds: HitRect, build: &mut UiSceneBuild) {
    match background {
        DirectBackground::Solid { color, radius } if *radius > 0.0 => {
            build.push_primitive(UiPrimitive::RoundedRect(UiRoundedRect {
                bounds,
                radius: *radius,
                color: *color,
            }));
        }
        DirectBackground::Solid { color, .. } => {
            build.push_primitive(UiPrimitive::SolidRect(UiSolidRect {
                bounds,
                color: *color,
            }));
        }
        DirectBackground::LinearGradient {
            angle_degrees,
            stops,
        } => {
            build.push_primitive(UiPrimitive::LinearGradient(UiLinearGradient {
                bounds,
                angle_degrees: *angle_degrees,
                stops: stops.clone(),
            }));
        }
        DirectBackground::Image {
            resource_index,
            opacity,
        } => {
            build.push_primitive(UiPrimitive::Image(UiImagePrimitive {
                resource_index: *resource_index,
                bounds,
                opacity: *opacity,
            }));
        }
    }
}

impl DirectClip {
    fn to_ui_clip(self, bounds: HitRect) -> UiClip {
        match self {
            Self::Rect => UiClip::Rect(bounds),
            Self::RoundedRect { radius } => UiClip::RoundedRect { bounds, radius },
        }
    }
}

fn collect_render_node_styles(
    path: &TakumiPath,
    render_node: &RenderNode,
    catalog: &mut TakumiCompositingStyleCatalog,
) {
    let style = render_node.context.style.as_ref();
    let compositing = TakumiCompositingStyle::from_computed_style(
        style,
        &render_node.context.sizing,
        render_node.context.current_color,
    );
    catalog.insert(path.clone(), compositing);

    if let Some(children) = render_node.children.as_deref() {
        for (index, child) in children.iter().enumerate() {
            collect_render_node_styles(&path.child(index), child, catalog);
        }
    }
}

fn compositing_effects_from_takumi(
    style: &ComputedStyle,
    sizing: &SizingContext,
    current_color: TakumiColor,
) -> UiCompositingEffects {
    UiCompositingEffects {
        opacity: style.opacity.0.clamp(0.0, 1.0),
        filters: filter_list_from_takumi(&style.filter, sizing, current_color),
        backdrop_filters: filter_list_from_takumi(&style.backdrop_filter, sizing, current_color),
        masks: masks_from_takumi(style),
        clip_path: style
            .clip_path
            .as_ref()
            .map(|clip_path| Box::new(clip_path_from_takumi(clip_path, sizing))),
        blend_mode: ui_blend_mode_from_takumi(style.mix_blend_mode),
    }
}

fn filter_list_from_takumi(
    filters: &[TakumiFilter],
    sizing: &SizingContext,
    current_color: TakumiColor,
) -> UiFilterList {
    UiFilterList::new(
        filters
            .iter()
            .map(|filter| ui_filter_from_takumi(filter, sizing, current_color)),
    )
}

fn ui_filter_from_takumi(
    filter: &TakumiFilter,
    sizing: &SizingContext,
    current_color: TakumiColor,
) -> UiFilter {
    match filter {
        TakumiFilter::Brightness(value) => UiFilter::Brightness(value.0),
        TakumiFilter::Contrast(value) => UiFilter::Contrast(value.0),
        TakumiFilter::Grayscale(value) => UiFilter::Grayscale(value.0),
        TakumiFilter::Saturate(value) => UiFilter::Saturate(value.0),
        TakumiFilter::HueRotate(angle) => UiFilter::HueRotateDegrees(**angle),
        TakumiFilter::Invert(value) => UiFilter::Invert(value.0),
        TakumiFilter::Sepia(value) => UiFilter::Sepia(value.0),
        TakumiFilter::Opacity(value) => UiFilter::Opacity(value.0),
        TakumiFilter::Blur(radius) => UiFilter::Blur {
            radius_px: length_px(*radius, sizing),
        },
        TakumiFilter::DropShadow(shadow) => UiFilter::DropShadow {
            offset_x_px: length_px(shadow.offset_x, sizing),
            offset_y_px: length_px(shadow.offset_y, sizing),
            blur_radius_px: length_px(shadow.blur_radius, sizing),
            color: ui_color_from_takumi(shadow.color.resolve(current_color)),
        },
    }
}

fn masks_from_takumi(style: &ComputedStyle) -> Vec<UiMask> {
    style
        .mask_image
        .as_ref()
        .map(|images| {
            images
                .iter()
                .filter_map(|image| {
                    let image = mask_image_from_takumi(image);
                    (!matches!(image, UiMaskImage::None)).then_some(UiMask {
                        image,
                        ..UiMask::default()
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn mask_image_from_takumi(image: &BackgroundImage) -> UiMaskImage {
    match image {
        BackgroundImage::None => UiMaskImage::None,
        BackgroundImage::Url(url) => UiMaskImage::Url(url.to_string().into_boxed_str()),
        BackgroundImage::Linear(_) => UiMaskImage::Unsupported("linear-gradient mask".into()),
        BackgroundImage::Radial(_) => UiMaskImage::Unsupported("radial-gradient mask".into()),
        BackgroundImage::Conic(_) => UiMaskImage::Unsupported("conic-gradient mask".into()),
    }
}

fn clip_path_from_takumi(shape: &BasicShape, sizing: &SizingContext) -> UiClipPath {
    match shape {
        BasicShape::Inset(shape) => UiClipPath::Inset {
            inset: lengths_from_sides(&shape.inset.0, sizing),
            radius: shape
                .border_radius
                .as_ref()
                .map_or_else(zero_lengths, |radius| lengths_from_sides(&radius.0, sizing)),
        },
        BasicShape::Ellipse(shape) => UiClipPath::Ellipse {
            radius_x: shape_radius_from_takumi(shape.radius_x, sizing),
            radius_y: shape_radius_from_takumi(shape.radius_y, sizing),
            center: point_from_space_pair(shape.position.0, sizing),
        },
        BasicShape::Polygon(shape) => UiClipPath::Polygon {
            fill_rule: shape
                .fill_rule
                .map_or(UiFillRule::NonZero, ui_fill_rule_from_takumi),
            points: shape
                .coordinates
                .iter()
                .copied()
                .map(|point| point_from_space_pair(point, sizing))
                .collect(),
        },
        BasicShape::Path(shape) => UiClipPath::Path {
            fill_rule: shape
                .fill_rule
                .map_or(UiFillRule::NonZero, ui_fill_rule_from_takumi),
            data: shape.path.clone(),
        },
    }
}

fn shape_radius_from_takumi(radius: TakumiShapeRadius, sizing: &SizingContext) -> UiShapeRadius {
    match radius {
        TakumiShapeRadius::ClosestSide => UiShapeRadius::ClosestSide,
        TakumiShapeRadius::FarthestSide => UiShapeRadius::FarthestSide,
        TakumiShapeRadius::Length(length) => {
            UiShapeRadius::Length(ui_length_from_takumi(length, sizing))
        }
    }
}

fn point_from_space_pair<T>(point: T, sizing: &SizingContext) -> UiPoint
where
    T: Into<taffy::Point<Length>>,
{
    let point = point.into();
    UiPoint {
        x: ui_length_from_takumi(point.x, sizing),
        y: ui_length_from_takumi(point.y, sizing),
    }
}

fn lengths_from_sides(sides: &[Length; 4], sizing: &SizingContext) -> [UiLength; 4] {
    std::array::from_fn(|index| ui_length_from_takumi(sides[index], sizing))
}

fn zero_lengths() -> [UiLength; 4] {
    std::array::from_fn(|_| UiLength::Px(0.0))
}

fn ui_length_from_takumi(length: Length, sizing: &SizingContext) -> UiLength {
    match length {
        Length::Auto => UiLength::Auto,
        Length::Percentage(value) => UiLength::Percent(value / 100.0),
        other => UiLength::Px(other.to_px(sizing, 0.0)),
    }
}

fn length_px(length: Length, sizing: &SizingContext) -> f32 {
    length.to_px(sizing, 0.0).max(0.0)
}

fn ui_color_from_takumi(color: TakumiColor) -> UiColorRgba8 {
    let [red, green, blue, alpha] = color.0;
    UiColorRgba8 {
        red,
        green,
        blue,
        alpha,
    }
}

fn ui_fill_rule_from_takumi(rule: TakumiFillRule) -> UiFillRule {
    match rule {
        TakumiFillRule::EvenOdd => UiFillRule::EvenOdd,
        TakumiFillRule::NonZero => UiFillRule::NonZero,
    }
}

fn ui_isolation_from_takumi(isolation: TakumiIsolation) -> UiIsolation {
    if matches!(isolation, TakumiIsolation::Isolate) {
        UiIsolation::Isolate
    } else {
        UiIsolation::Auto
    }
}

fn ui_blend_mode_from_takumi(mode: TakumiBlendMode) -> UiBlendMode {
    match mode {
        TakumiBlendMode::Normal => UiBlendMode::Normal,
        TakumiBlendMode::Multiply => UiBlendMode::Multiply,
        TakumiBlendMode::Screen => UiBlendMode::Screen,
        TakumiBlendMode::Overlay => UiBlendMode::Overlay,
        TakumiBlendMode::Darken => UiBlendMode::Darken,
        TakumiBlendMode::Lighten => UiBlendMode::Lighten,
        TakumiBlendMode::ColorDodge => UiBlendMode::ColorDodge,
        TakumiBlendMode::ColorBurn => UiBlendMode::ColorBurn,
        TakumiBlendMode::HardLight => UiBlendMode::HardLight,
        TakumiBlendMode::SoftLight => UiBlendMode::SoftLight,
        TakumiBlendMode::Difference => UiBlendMode::Difference,
        TakumiBlendMode::Exclusion => UiBlendMode::Exclusion,
        TakumiBlendMode::Hue => UiBlendMode::Hue,
        TakumiBlendMode::Saturation => UiBlendMode::Saturation,
        TakumiBlendMode::Color => UiBlendMode::Color,
        TakumiBlendMode::Luminosity => UiBlendMode::Luminosity,
        TakumiBlendMode::PlusLighter => UiBlendMode::PlusLighter,
        TakumiBlendMode::PlusDarker => UiBlendMode::PlusDarker,
    }
}

fn affine_to_ui(values: [f32; 6]) -> UiAffine2 {
    UiAffine2 {
        m11: values[0],
        m12: values[1],
        m21: values[2],
        m22: values[3],
        tx: values[4],
        ty: values[5],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(alpha: u8) -> UiColorRgba8 {
        UiColorRgba8 {
            red: 10,
            green: 20,
            blue: 30,
            alpha,
        }
    }

    #[test]
    fn direct_paint_catalog_returns_path_specific_paint() {
        let path = TakumiPath::root().child(1);
        let mut catalog = DirectPaintCatalog::default();
        catalog.insert(
            path.clone(),
            DirectBoxPaint::new().with_background(DirectBackground::Solid {
                color: color(255),
                radius: 4.0,
            }),
        );

        assert!(catalog.get(&path).is_some());
        assert!(catalog.get(&TakumiPath::root()).is_none());
    }

    #[test]
    fn compositing_style_catalog_returns_path_specific_style() {
        let path = TakumiPath::root().child(2);
        let mut catalog = TakumiCompositingStyleCatalog::default();
        catalog.insert(
            path.clone(),
            TakumiCompositingStyle {
                isolation: UiIsolation::Isolate,
                effects: UiCompositingEffects {
                    blend_mode: UiBlendMode::Multiply,
                    ..UiCompositingEffects::default()
                },
            },
        );

        let style = catalog.get(&path).expect("style for inserted path");
        assert_eq!(style.isolation, UiIsolation::Isolate);
        assert_eq!(style.effects.blend_mode, UiBlendMode::Multiply);
        assert!(catalog.get(&TakumiPath::root()).is_none());
    }

    #[test]
    fn lowering_build_preserves_child_order_inside_compositing_group() {
        let mut build = UiSceneBuild::new(320.0, 180.0);
        let first = UiSceneContext {
            transform: UiAffine2::IDENTITY,
            opacity: 1.0,
            clip: None,
            primitive_range: UiPrimitiveRange { start: 0, end: 1 },
        };
        let second = UiSceneContext {
            transform: UiAffine2::IDENTITY,
            opacity: 1.0,
            clip: None,
            primitive_range: UiPrimitiveRange { start: 1, end: 2 },
        };

        build.push_context(first.clone());
        build.push_context(second.clone());
        build.push_paint_node(UiPaintNode::Group(
            UiCompositingGroup::new(
                HitRect::new(0.0, 0.0, 10.0, 10.0),
                UiCompositingEffects::default(),
            )
            .with_children(vec![
                UiPaintNode::Direct(first),
                UiPaintNode::Direct(second),
            ]),
        ));

        let output = build.finish();
        let UiPaintNode::Group(group) = &output.scene.paint_nodes()[0] else {
            panic!("root paint node should be a compositing group");
        };
        let UiPaintNode::Direct(first) = &group.children[0] else {
            panic!("first child should be direct");
        };
        let UiPaintNode::Direct(second) = &group.children[1] else {
            panic!("second child should be direct");
        };

        assert_eq!(first.primitive_range, UiPrimitiveRange { start: 0, end: 1 });
        assert_eq!(
            second.primitive_range,
            UiPrimitiveRange { start: 1, end: 2 }
        );
    }
}
