use crate::{
    capture::{
        TakumiCaptureFrame, TakumiCaptureRecord, TakumiCompositingCaptureRecord,
        TakumiCompositingGroupId, TakumiEffectOutsets, TakumiPaintNodeId,
    },
    diagnostic::TakumiAdapterError,
    metadata::{TakumiMetadataMap, TakumiPath},
    text::ArcweftTextLayoutBridge,
};
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::view_scene::{
    ViewAffine2D, ViewBlendMode, ViewBoxShadow, ViewBoxShadowCornerRadius, ViewBoxShadowList,
    ViewBoxShadowRadii, ViewClipPath, ViewColorRgba8, ViewCompositingEffects, ViewCompositingGroup,
    ViewFillRule, ViewFilter, ViewFilterList, ViewGradientStop, ViewIsolation, ViewLength,
    ViewMask, ViewMaskGradient, ViewMaskImage, ViewPaintNode, ViewPoint, ViewPrimitive,
    ViewPrimitiveRange, ViewScene, ViewSceneContext, ViewShapeRadius, ViewSurfaceBackground,
    ViewSurfaceBorder, ViewSurfaceClip, ViewSurfacePaint,
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
            BoxShadow as TakumiBoxShadow, Color as TakumiColor, ComputedStyle,
            FillRule as TakumiFillRule, Filter as TakumiFilter, GradientStop,
            Isolation as TakumiIsolation, Length, LinearGradient, ShapeRadius as TakumiShapeRadius,
            SizingContext, SpacePair,
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
    pub isolation: ViewIsolation,
    pub effects: ViewCompositingEffects,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirectBoxPaint {
    pub surface: ViewSurfacePaint,
    pub clip: Option<ViewSurfaceClip>,
    pub opacity: f32,
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
    pub scene: ViewScene,
    pub capture: TakumiCaptureFrame,
}

#[derive(Clone, Debug, Default)]
pub struct TakumiSceneLowerer;

#[derive(Default)]
struct ViewSceneBuild {
    viewport_width: f32,
    viewport_height: f32,
    primitives: Vec<ViewPrimitive>,
    contexts: Vec<ViewSceneContext>,
    paint_nodes: Vec<ViewPaintNode>,
    capture: TakumiCaptureFrame,
    next_paint_node_id: u32,
    next_compositing_group_id: u32,
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
            isolation: view_isolation_from_takumi(style.isolation),
            effects: compositing_effects_from_takumi(style, sizing, current_color),
        }
    }

    pub fn is_identity(&self) -> bool {
        self.isolation == ViewIsolation::Auto && self.effects.is_identity()
    }
}

impl DirectBoxPaint {
    pub fn new() -> Self {
        Self {
            surface: ViewSurfacePaint::new(),
            clip: None,
            opacity: 1.0,
        }
    }

    #[must_use]
    pub fn with_background(mut self, background: ViewSurfaceBackground) -> Self {
        self.surface.backgrounds.push(background);
        self
    }

    #[must_use]
    pub fn with_backgrounds(
        mut self,
        backgrounds: impl IntoIterator<Item = ViewSurfaceBackground>,
    ) -> Self {
        self.surface.backgrounds.extend(backgrounds);
        self
    }

    #[must_use]
    pub fn with_border(mut self, border: ViewSurfaceBorder) -> Self {
        self.surface.border = Some(border);
        self
    }

    #[must_use]
    pub fn with_clip(mut self, clip: ViewSurfaceClip) -> Self {
        self.clip = Some(clip);
        self
    }

    #[must_use]
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn has_visible_direct_paint(&self) -> bool {
        self.surface.has_visible_primitives() || self.clip.is_some() || self.opacity < 1.0
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

        let mut build = ViewSceneBuild::new(
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

impl ViewSceneBuild {
    fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            viewport_width,
            viewport_height,
            primitives: Vec::new(),
            contexts: Vec::new(),
            paint_nodes: Vec::new(),
            capture: TakumiCaptureFrame::default(),
            next_paint_node_id: 0,
            next_compositing_group_id: 0,
        }
    }

    fn primitive_start(&self) -> Result<u32, TakumiAdapterError> {
        u32::try_from(self.primitives.len()).map_err(|_| TakumiAdapterError::CapacityExceeded)
    }

    fn push_primitive(&mut self, primitive: ViewPrimitive) {
        self.primitives.push(primitive);
    }

    fn push_surface_primitives(&mut self, bounds: HitRect, paint: &ViewSurfacePaint) {
        paint.append_primitives(bounds, |primitive| self.push_primitive(primitive));
    }

    fn push_context(&mut self, context: ViewSceneContext) {
        self.contexts.push(context);
    }

    fn push_paint_node(&mut self, node: ViewPaintNode) {
        self.paint_nodes.push(node);
    }

    fn next_paint_node_id(&mut self) -> TakumiPaintNodeId {
        self.next_paint_node_id = self.next_paint_node_id.saturating_add(1);
        TakumiPaintNodeId::new(self.next_paint_node_id)
    }

    fn next_compositing_group_id(&mut self) -> TakumiCompositingGroupId {
        self.next_compositing_group_id = self.next_compositing_group_id.saturating_add(1);
        TakumiCompositingGroupId::new(self.next_compositing_group_id)
    }

    fn finish(self) -> TakumiSceneOutput {
        let mut scene = ViewScene::new(self.viewport_width, self.viewport_height);
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
    build: &mut ViewSceneBuild,
) -> Result<Option<ViewPaintNode>, TakumiAdapterError> {
    let Some(context) = refs.contexts.get(context_id) else {
        return Ok(None);
    };

    let mut children = Vec::new();
    let mut root_path = None;
    let mut bounds = None;
    let group_id = build.next_compositing_group_id();
    let group_paint_node_id = build.next_paint_node_id();

    if let Some(root) = context.root() {
        root_path = Some(TakumiPath::from(root.path.clone()));
        bounds = Some(bounds_for_node(root, refs.layout_results)?);
        if let Some(node) = lower_node(root, refs, build, group_id)? {
            children.push(node);
        }
    }
    for bucket in context.in_paint_order() {
        for item in bucket {
            match &item.kind {
                PaintItemKind::Node(node) => {
                    if let Some(node) = lower_node(node, refs, build, group_id)? {
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
    let group = ViewCompositingGroup {
        bounds: bounds.unwrap_or_else(|| HitRect::new(0.0, 0.0, 0.0, 0.0)),
        isolation: compositing.isolation,
        effects: compositing.effects,
        children,
    };

    if let Some(path) = root_path.as_ref()
        && let Some(metadata) = refs.metadata.get_by_path(path)
    {
        let primitive_range = primitive_range_for_group(&group);
        let effect_outsets = effect_outsets_for_effects(&group.effects);
        build.capture.push_compositing_group(
            TakumiCompositingCaptureRecord::new(
                metadata.clone(),
                group_id,
                group_paint_node_id,
                group.bounds,
                group.visual_bounds(),
            )
            .with_primitive_range(primitive_range)
            .with_hit_bounds(group.bounds)
            .with_clip_bounds(clip_bounds_for_group(&group))
            .with_mask_bounds(mask_bounds_for_group(&group))
            .with_effect_outsets(effect_outsets)
            .with_isolation(group.isolation)
            .with_blend_mode(group.effects.blend_mode),
        );
    }

    Ok(Some(ViewPaintNode::Group(group)))
}

fn lower_node(
    node: &NodePaint,
    refs: &TakumiLoweringRefs<'_>,
    build: &mut ViewSceneBuild,
    group_id: TakumiCompositingGroupId,
) -> Result<Option<ViewPaintNode>, TakumiAdapterError> {
    let bounds = bounds_for_node(node, refs.layout_results)?;
    let path = TakumiPath::from(node.path.clone());
    let transform = affine_to_view(node.transform.to_cols_array());
    let paint_node_id = build.next_paint_node_id();
    let paint = refs.direct_paint.get(&path);
    let start = build.primitive_start()?;

    if let Some(paint) = paint {
        build.push_surface_primitives(bounds, &paint.surface);
    }

    if let Some(metadata) = refs.metadata.get_by_path(&path)
        && let Some(participant) = refs.text.get(metadata.node())
    {
        for text in participant.prepared_text() {
            build.push_primitive(text.clone().into_primitive());
        }
    }

    let end = build.primitive_start()?;
    if start == end {
        return Ok(None);
    }

    let clip = paint.and_then(|paint| paint.clip.map(|clip| clip.to_view_clip(bounds)));
    let opacity = paint.map_or(1.0, |paint| paint.opacity);
    let primitive_range = ViewPrimitiveRange { start, end };
    let scene_context = ViewSceneContext {
        transform,
        opacity,
        clip: clip.clone(),
        primitive_range,
    };
    build.push_context(scene_context.clone());
    if let Some(metadata) = refs.metadata.get_by_path(&path) {
        build.capture.push(
            TakumiCaptureRecord::new(
                metadata.clone(),
                primitive_range,
                bounds,
                transform,
                clip.clone(),
            )
            .with_paint_node_id(paint_node_id)
            .with_compositing_group_id(group_id)
            .with_layout_bounds(bounds)
            .with_visual_bounds(bounds)
            .with_hit_bounds(bounds)
            .with_clip_bounds(clip.as_ref().map(crate::capture::view_clip_bounds)),
        );
    }
    Ok(Some(ViewPaintNode::Direct(scene_context)))
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

fn primitive_range_for_group(group: &ViewCompositingGroup) -> Option<ViewPrimitiveRange> {
    group
        .children
        .iter()
        .filter_map(primitive_range_for_paint_node)
        .fold(None, |acc, range| {
            Some(match acc {
                Some(existing) => ViewPrimitiveRange {
                    start: existing.start.min(range.start),
                    end: existing.end.max(range.end),
                },
                None => range,
            })
        })
}

fn primitive_range_for_paint_node(node: &ViewPaintNode) -> Option<ViewPrimitiveRange> {
    match node {
        ViewPaintNode::Direct(context) => Some(context.primitive_range),
        ViewPaintNode::Group(group) => primitive_range_for_group(group),
    }
}

fn effect_outsets_for_effects(effects: &ViewCompositingEffects) -> TakumiEffectOutsets {
    TakumiEffectOutsets::new(
        effects.filters.visual_outset_px(),
        effects.backdrop_filters.visual_outset_px(),
        effects
            .masks
            .iter()
            .map(ViewMask::visual_outset_px)
            .fold(0.0, f32::max),
    )
}

fn clip_bounds_for_group(group: &ViewCompositingGroup) -> Option<HitRect> {
    let clip_path = group.effects.clip_path.as_deref()?;
    match clip_path {
        ViewClipPath::Inset { inset, .. } => {
            let top = inset[0].resolve_px(group.bounds.height)?;
            let right = inset[1].resolve_px(group.bounds.width)?;
            let bottom = inset[2].resolve_px(group.bounds.height)?;
            let left = inset[3].resolve_px(group.bounds.width)?;
            Some(HitRect::new(
                group.bounds.x + left,
                group.bounds.y + top,
                (group.bounds.width - left - right).max(0.0),
                (group.bounds.height - top - bottom).max(0.0),
            ))
        }
        ViewClipPath::Circle { .. }
        | ViewClipPath::Ellipse { .. }
        | ViewClipPath::Polygon { .. }
        | ViewClipPath::Path { .. }
        | ViewClipPath::Url(_)
        | ViewClipPath::Unsupported(_) => Some(group.bounds),
    }
}

fn mask_bounds_for_group(group: &ViewCompositingGroup) -> Vec<HitRect> {
    group.effects.masks.iter().map(|_| group.bounds).collect()
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
) -> ViewCompositingEffects {
    ViewCompositingEffects {
        opacity: style.opacity.0.clamp(0.0, 1.0),
        filters: filter_list_from_takumi(&style.filter, sizing, current_color),
        backdrop_filters: filter_list_from_takumi(&style.backdrop_filter, sizing, current_color),
        box_shadows: box_shadow_list_from_takumi(
            style.box_shadow.as_deref(),
            style,
            sizing,
            current_color,
        ),
        masks: masks_from_takumi(style, sizing, current_color),
        clip_path: style
            .clip_path
            .as_ref()
            .map(|clip_path| Box::new(clip_path_from_takumi(clip_path, sizing))),
        blend_mode: view_blend_mode_from_takumi(style.mix_blend_mode),
    }
}

fn filter_list_from_takumi(
    filters: &[TakumiFilter],
    sizing: &SizingContext,
    current_color: TakumiColor,
) -> ViewFilterList {
    ViewFilterList::new(
        filters
            .iter()
            .map(|filter| view_filter_from_takumi(filter, sizing, current_color)),
    )
}

fn view_filter_from_takumi(
    filter: &TakumiFilter,
    sizing: &SizingContext,
    current_color: TakumiColor,
) -> ViewFilter {
    match filter {
        TakumiFilter::Brightness(value) => ViewFilter::Brightness(value.0),
        TakumiFilter::Contrast(value) => ViewFilter::Contrast(value.0),
        TakumiFilter::Grayscale(value) => ViewFilter::Grayscale(value.0),
        TakumiFilter::Saturate(value) => ViewFilter::Saturate(value.0),
        TakumiFilter::HueRotate(angle) => ViewFilter::HueRotateDegrees(**angle),
        TakumiFilter::Invert(value) => ViewFilter::Invert(value.0),
        TakumiFilter::Sepia(value) => ViewFilter::Sepia(value.0),
        TakumiFilter::Opacity(value) => ViewFilter::Opacity(value.0),
        TakumiFilter::Blur(radius) => ViewFilter::Blur {
            radius_px: length_px(*radius, sizing),
        },
        TakumiFilter::DropShadow(shadow) => ViewFilter::DropShadow {
            offset_x_px: length_px(shadow.offset_x, sizing),
            offset_y_px: length_px(shadow.offset_y, sizing),
            blur_radius_px: length_px(shadow.blur_radius, sizing),
            color: view_color_from_takumi(shadow.color.resolve(current_color)),
        },
    }
}

fn box_shadow_list_from_takumi(
    shadows: Option<&[TakumiBoxShadow]>,
    style: &ComputedStyle,
    sizing: &SizingContext,
    current_color: TakumiColor,
) -> ViewBoxShadowList {
    let Some(shadows) = shadows else {
        return ViewBoxShadowList::default();
    };
    let border_radii = box_shadow_border_radii(style, sizing);
    ViewBoxShadowList::new(
        shadows
            .iter()
            .map(|shadow| box_shadow_from_takumi(shadow, border_radii, sizing, current_color)),
    )
}

fn box_shadow_from_takumi(
    shadow: &TakumiBoxShadow,
    border_radii: ViewBoxShadowRadii,
    sizing: &SizingContext,
    current_color: TakumiColor,
) -> ViewBoxShadow {
    let horizontal_shift_px = length_value_px(shadow.offset_x, sizing);
    let vertical_shift_px = length_value_px(shadow.offset_y, sizing);
    let blur_radius_px = length_value_px(shadow.blur_radius, sizing).max(0.0);
    let spread_radius_px = length_value_px(shadow.spread_radius, sizing);
    let color = view_color_from_takumi(shadow.color.resolve(current_color));
    if shadow.inset {
        ViewBoxShadow::inset_with_radii(
            horizontal_shift_px,
            vertical_shift_px,
            blur_radius_px,
            spread_radius_px,
            border_radii,
            color,
        )
    } else {
        ViewBoxShadow::outer_with_radii(
            horizontal_shift_px,
            vertical_shift_px,
            blur_radius_px,
            spread_radius_px,
            border_radii,
            color,
        )
    }
}

fn box_shadow_border_radii(style: &ComputedStyle, sizing: &SizingContext) -> ViewBoxShadowRadii {
    ViewBoxShadowRadii::from_corners(
        box_shadow_corner_radius_from_takumi(style.border_top_left_radius, sizing),
        box_shadow_corner_radius_from_takumi(style.border_top_right_radius, sizing),
        box_shadow_corner_radius_from_takumi(style.border_bottom_right_radius, sizing),
        box_shadow_corner_radius_from_takumi(style.border_bottom_left_radius, sizing),
    )
}

fn box_shadow_corner_radius_from_takumi(
    radius: SpacePair<Length>,
    sizing: &SizingContext,
) -> ViewBoxShadowCornerRadius {
    ViewBoxShadowCornerRadius::new(
        length_value_px(radius.x, sizing).max(0.0),
        length_value_px(radius.y, sizing).max(0.0),
    )
}

fn masks_from_takumi(
    style: &ComputedStyle,
    sizing: &SizingContext,
    current_color: TakumiColor,
) -> Vec<ViewMask> {
    style
        .mask_image
        .as_ref()
        .map(|images| {
            images
                .iter()
                .filter_map(|image| {
                    let image = mask_image_from_takumi(image, sizing, current_color);
                    (!matches!(image, ViewMaskImage::None)).then_some(ViewMask {
                        image,
                        ..ViewMask::default()
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn mask_image_from_takumi(
    image: &BackgroundImage,
    sizing: &SizingContext,
    current_color: TakumiColor,
) -> ViewMaskImage {
    match image {
        BackgroundImage::None => ViewMaskImage::None,
        BackgroundImage::Url(url) => ViewMaskImage::Url(url.to_string().into_boxed_str()),
        BackgroundImage::Linear(gradient) => {
            linear_mask_gradient_from_takumi(gradient, sizing, current_color)
        }
        BackgroundImage::Radial(_) => ViewMaskImage::Unsupported("radial-gradient mask".into()),
        BackgroundImage::Conic(_) => ViewMaskImage::Unsupported("conic-gradient mask".into()),
    }
}

fn linear_mask_gradient_from_takumi(
    gradient: &LinearGradient,
    sizing: &SizingContext,
    current_color: TakumiColor,
) -> ViewMaskImage {
    if gradient.repeating {
        return ViewMaskImage::Unsupported("repeating-linear-gradient mask".into());
    }
    let Some(stops) = gradient_stops_from_takumi(&gradient.stops, sizing, current_color) else {
        return ViewMaskImage::Unsupported("linear-gradient mask stops".into());
    };
    ViewMaskImage::Gradient(ViewMaskGradient::Linear {
        angle_degrees: gradient_angle_degrees(gradient.direction),
        stops,
    })
}

fn gradient_stops_from_takumi(
    stops: &[GradientStop],
    sizing: &SizingContext,
    current_color: TakumiColor,
) -> Option<Vec<ViewGradientStop>> {
    let color_stop_count = stops
        .iter()
        .filter(|stop| matches!(stop, GradientStop::ColorHint { .. }))
        .count();
    if color_stop_count < 2 {
        return None;
    }
    let mut color_index = 0usize;
    let mut result = Vec::with_capacity(color_stop_count);
    for stop in stops {
        match stop {
            GradientStop::ColorHint { color, hint } => {
                let fallback_offset = fallback_gradient_offset(color_index, color_stop_count);
                let offset = match hint {
                    Some(hint) => stop_position_to_offset(hint.0, sizing)?,
                    None => fallback_offset,
                };
                result.push(ViewGradientStop {
                    offset: offset.clamp(0.0, 1.0),
                    color: view_color_from_takumi(color.resolve(current_color)),
                });
                color_index += 1;
            }
            _ => return None,
        }
    }
    Some(result)
}

fn fallback_gradient_offset(index: usize, len: usize) -> f32 {
    if len <= 1 {
        return 0.0;
    }
    index.to_f32().unwrap_or(0.0) / (len - 1).to_f32().unwrap_or(1.0)
}

fn stop_position_to_offset(length: Length, sizing: &SizingContext) -> Option<f32> {
    match length {
        Length::Percentage(value) => Some(value / 100.0),
        Length::Px(value) if (0.0..=1.0).contains(&value) => Some(value),
        Length::Calc(_) => Some(length.to_px(sizing, 1.0)),
        _ => None,
    }
}

fn gradient_angle_degrees(
    direction: takumi::unstable::base::layout::style::LinearGradientDirection,
) -> f32 {
    match direction {
        takumi::unstable::base::layout::style::LinearGradientDirection::Angle(angle) => *angle,
        takumi::unstable::base::layout::style::LinearGradientDirection::Keyword(keyword) => {
            *keyword.to_angle()
        }
    }
}

fn clip_path_from_takumi(shape: &BasicShape, sizing: &SizingContext) -> ViewClipPath {
    match shape {
        BasicShape::Inset(shape) => ViewClipPath::Inset {
            inset: lengths_from_sides(&shape.inset.0, sizing),
            radius: shape
                .border_radius
                .as_ref()
                .map_or_else(zero_lengths, |radius| lengths_from_sides(&radius.0, sizing)),
        },
        BasicShape::Ellipse(shape) => ViewClipPath::Ellipse {
            radius_x: shape_radius_from_takumi(shape.radius_x, sizing),
            radius_y: shape_radius_from_takumi(shape.radius_y, sizing),
            center: point_from_space_pair(shape.position.0, sizing),
        },
        BasicShape::Polygon(shape) => ViewClipPath::Polygon {
            fill_rule: shape
                .fill_rule
                .map_or(ViewFillRule::NonZero, view_fill_rule_from_takumi),
            points: shape
                .coordinates
                .iter()
                .copied()
                .map(|point| point_from_space_pair(point, sizing))
                .collect(),
        },
        BasicShape::Path(shape) => ViewClipPath::Path {
            fill_rule: shape
                .fill_rule
                .map_or(ViewFillRule::NonZero, view_fill_rule_from_takumi),
            data: shape.path.clone(),
        },
    }
}

fn shape_radius_from_takumi(radius: TakumiShapeRadius, sizing: &SizingContext) -> ViewShapeRadius {
    match radius {
        TakumiShapeRadius::ClosestSide => ViewShapeRadius::ClosestSide,
        TakumiShapeRadius::FarthestSide => ViewShapeRadius::FarthestSide,
        TakumiShapeRadius::Length(length) => {
            ViewShapeRadius::Length(view_length_from_takumi(length, sizing))
        }
    }
}

fn point_from_space_pair<T>(point: T, sizing: &SizingContext) -> ViewPoint
where
    T: Into<taffy::Point<Length>>,
{
    let point = point.into();
    ViewPoint {
        x: view_length_from_takumi(point.x, sizing),
        y: view_length_from_takumi(point.y, sizing),
    }
}

fn lengths_from_sides(sides: &[Length; 4], sizing: &SizingContext) -> [ViewLength; 4] {
    std::array::from_fn(|index| view_length_from_takumi(sides[index], sizing))
}

fn zero_lengths() -> [ViewLength; 4] {
    std::array::from_fn(|_| ViewLength::Px(0.0))
}

fn view_length_from_takumi(length: Length, sizing: &SizingContext) -> ViewLength {
    match length {
        Length::Auto => ViewLength::Auto,
        Length::Percentage(value) => ViewLength::Percent(value / 100.0),
        other => ViewLength::Px(other.to_px(sizing, 0.0)),
    }
}

fn length_value_px(length: Length, sizing: &SizingContext) -> f32 {
    length.to_px(sizing, 0.0)
}

fn length_px(length: Length, sizing: &SizingContext) -> f32 {
    length.to_px(sizing, 0.0).max(0.0)
}

fn view_color_from_takumi(color: TakumiColor) -> ViewColorRgba8 {
    let [red, green, blue, alpha] = color.0;
    ViewColorRgba8 {
        red,
        green,
        blue,
        alpha,
    }
}

fn view_fill_rule_from_takumi(rule: TakumiFillRule) -> ViewFillRule {
    match rule {
        TakumiFillRule::EvenOdd => ViewFillRule::EvenOdd,
        TakumiFillRule::NonZero => ViewFillRule::NonZero,
    }
}

fn view_isolation_from_takumi(isolation: TakumiIsolation) -> ViewIsolation {
    if matches!(isolation, TakumiIsolation::Isolate) {
        ViewIsolation::Isolate
    } else {
        ViewIsolation::Auto
    }
}

fn view_blend_mode_from_takumi(mode: TakumiBlendMode) -> ViewBlendMode {
    match mode {
        TakumiBlendMode::Normal => ViewBlendMode::Normal,
        TakumiBlendMode::Multiply => ViewBlendMode::Multiply,
        TakumiBlendMode::Screen => ViewBlendMode::Screen,
        TakumiBlendMode::Overlay => ViewBlendMode::Overlay,
        TakumiBlendMode::Darken => ViewBlendMode::Darken,
        TakumiBlendMode::Lighten => ViewBlendMode::Lighten,
        TakumiBlendMode::ColorDodge => ViewBlendMode::ColorDodge,
        TakumiBlendMode::ColorBurn => ViewBlendMode::ColorBurn,
        TakumiBlendMode::HardLight => ViewBlendMode::HardLight,
        TakumiBlendMode::SoftLight => ViewBlendMode::SoftLight,
        TakumiBlendMode::Difference => ViewBlendMode::Difference,
        TakumiBlendMode::Exclusion => ViewBlendMode::Exclusion,
        TakumiBlendMode::Hue => ViewBlendMode::Hue,
        TakumiBlendMode::Saturation => ViewBlendMode::Saturation,
        TakumiBlendMode::Color => ViewBlendMode::Color,
        TakumiBlendMode::Luminosity => ViewBlendMode::Luminosity,
        TakumiBlendMode::PlusLighter => ViewBlendMode::PlusLighter,
        TakumiBlendMode::PlusDarker => ViewBlendMode::PlusDarker,
    }
}

fn affine_to_view(values: [f32; 6]) -> ViewAffine2D {
    ViewAffine2D {
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
    use arcweft_render_wgpu::view_scene::{ViewCornerRadii, ViewCornerRadius};

    fn color(alpha: u8) -> ViewColorRgba8 {
        ViewColorRgba8 {
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
            DirectBoxPaint::new().with_background(ViewSurfaceBackground::Solid {
                color: color(255),
                radii: ViewCornerRadii::uniform(4.0),
            }),
        );

        assert!(catalog.get(&path).is_some());
        assert!(catalog.get(&TakumiPath::root()).is_none());
    }

    #[test]
    fn shared_surface_builder_preserves_direct_solid_corner_radii() {
        let radii = ViewCornerRadii::from_corners(
            ViewCornerRadius::new(18.0, 12.0),
            ViewCornerRadius::new(10.0, 6.0),
            ViewCornerRadius::new(14.0, 8.0),
            ViewCornerRadius::new(6.0, 4.0),
        );
        let mut build = ViewSceneBuild::new(100.0, 50.0);

        ViewSurfacePaint::new()
            .with_background(ViewSurfaceBackground::Solid {
                color: color(255),
                radii,
            })
            .append_primitives(HitRect::new(0.0, 0.0, 80.0, 40.0), |primitive| {
                build.push_primitive(primitive);
            });

        let ViewPrimitive::RoundedRect(rect) = &build.primitives[0] else {
            panic!("rounded direct background lowers to ViewRoundedRect");
        };
        assert_eq!(rect.radii, radii);
    }

    #[test]
    fn compositing_style_catalog_returns_path_specific_style() {
        let path = TakumiPath::root().child(2);
        let mut catalog = TakumiCompositingStyleCatalog::default();
        catalog.insert(
            path.clone(),
            TakumiCompositingStyle {
                isolation: ViewIsolation::Isolate,
                effects: ViewCompositingEffects {
                    blend_mode: ViewBlendMode::Multiply,
                    ..ViewCompositingEffects::default()
                },
            },
        );

        let style = catalog.get(&path).expect("style for inserted path");
        assert_eq!(style.isolation, ViewIsolation::Isolate);
        assert_eq!(style.effects.blend_mode, ViewBlendMode::Multiply);
        assert!(catalog.get(&TakumiPath::root()).is_none());
    }

    #[test]
    fn lowering_build_preserves_child_order_inside_compositing_group() {
        let mut build = ViewSceneBuild::new(320.0, 180.0);
        let first = ViewSceneContext {
            transform: ViewAffine2D::IDENTITY,
            opacity: 1.0,
            clip: None,
            primitive_range: ViewPrimitiveRange { start: 0, end: 1 },
        };
        let second = ViewSceneContext {
            transform: ViewAffine2D::IDENTITY,
            opacity: 1.0,
            clip: None,
            primitive_range: ViewPrimitiveRange { start: 1, end: 2 },
        };

        build.push_context(first.clone());
        build.push_context(second.clone());
        build.push_paint_node(ViewPaintNode::Group(
            ViewCompositingGroup::new(
                HitRect::new(0.0, 0.0, 10.0, 10.0),
                ViewCompositingEffects::default(),
            )
            .with_children(vec![
                ViewPaintNode::Direct(first),
                ViewPaintNode::Direct(second),
            ]),
        ));

        let output = build.finish();
        let ViewPaintNode::Group(group) = &output.scene.paint_nodes()[0] else {
            panic!("root paint node should be a compositing group");
        };
        let ViewPaintNode::Direct(first) = &group.children[0] else {
            panic!("first child should be direct");
        };
        let ViewPaintNode::Direct(second) = &group.children[1] else {
            panic!("second child should be direct");
        };

        assert_eq!(
            first.primitive_range,
            ViewPrimitiveRange { start: 0, end: 1 }
        );
        assert_eq!(
            second.primitive_range,
            ViewPrimitiveRange { start: 1, end: 2 }
        );
    }

    #[test]
    fn capture_records_include_compositing_group_and_paint_node_bounds() {
        let effects = ViewCompositingEffects {
            filters: ViewFilterList::new([ViewFilter::Blur { radius_px: 4.0 }]),
            blend_mode: ViewBlendMode::Multiply,
            ..ViewCompositingEffects::default()
        };
        let group = ViewCompositingGroup::new(HitRect::new(10.0, 20.0, 80.0, 40.0), effects)
            .with_children(vec![ViewPaintNode::Direct(ViewSceneContext {
                transform: ViewAffine2D::IDENTITY,
                opacity: 1.0,
                clip: None,
                primitive_range: ViewPrimitiveRange { start: 2, end: 6 },
            })]);

        let outsets = effect_outsets_for_effects(&group.effects);
        assert_eq!(
            primitive_range_for_group(&group),
            Some(ViewPrimitiveRange { start: 2, end: 6 })
        );
        assert!((outsets.filter_px - 12.0).abs() <= f32::EPSILON);
        assert_eq!(clip_bounds_for_group(&group), None);
        assert!(mask_bounds_for_group(&group).is_empty());
    }
}
