use super::view_style::{ResolvedViewStyleFrame, box_style};
use super::view_text::PreparedMountedViewText;
use super::{milli_i32_to_f32, milli_u32_to_f32, scroll_adjusted_bounds};
use arcweft_bundle::resource_codec::view::{
    ViewRuntimeControlCornerRadius, ViewRuntimeControlFilter, ViewRuntimeControlFilterList,
    ViewRuntimeControlRadii, ViewRuntimeControlVisualStyle, ViewRuntimeNodeStyle,
    ViewRuntimeShadow, ViewRuntimeShadowKind, ViewRuntimeSurface,
};
use arcweft_layout::{ContentRect, LayoutRect as FitLayoutRect};
use arcweft_presentation::appearance::PresentationColor;
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::geometry::{
    PreparedFrame, PreparedViewImageResource, PreparedViewScene, PreparedViewSceneResources,
    RenderImage, RenderScene,
};
use arcweft_render_wgpu::view_scene::{
    ViewAffine2D, ViewBoxShadow, ViewBoxShadowCornerRadius, ViewBoxShadowList, ViewBoxShadowRadii,
    ViewClip, ViewColorRgba8, ViewCompositingEffects, ViewCompositingGroup, ViewCornerRadii,
    ViewCornerRadius, ViewFilter, ViewFilterList, ViewImagePrimitive, ViewImageUvRect,
    ViewPaintNode, ViewPrimitive, ViewPrimitiveRange, ViewScene, ViewSceneContext,
    ViewSurfaceBackground, ViewSurfaceBorder, ViewSurfacePaint, ViewTextPrimitive,
};
use arcweft_runtime_driver::view_runtime::{
    BundleViewFrame, BundleViewMountOutput, BundleViewPaintItem,
};
use arcweft_view::style::ViewOverflow;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
struct PreparedMountedViewImage {
    resource_index: u32,
    bounds: HitRect,
    uv: ViewImageUvRect,
    transform: ViewAffine2D,
    opacity: f32,
}

pub(super) fn push_runtime_view_scene(
    frame: &mut PreparedFrame,
    scene: &RenderScene,
    surfaces: &[ViewRuntimeSurface],
    view: &BundleViewFrame,
    text: &[PreparedMountedViewText],
    styles: &ResolvedViewStyleFrame,
    content: Option<ContentRect>,
) {
    let mut consumed_surfaces = BTreeSet::new();
    let mut available_images = core::mem::take(&mut frame.images);
    for mount in view
        .mounts
        .iter()
        .filter(|mount| mount.path.segments().is_empty())
    {
        let mut output =
            ViewScene::new(frame.viewport.logical_width, frame.viewport.logical_height);
        let mut resources = PreparedViewSceneResources::default();
        let mut prepared_images = BTreeMap::new();
        let mut active_mounts = BTreeSet::new();
        push_mount_paint(
            &mut output,
            &mut resources,
            &mut available_images,
            &mut prepared_images,
            &mut consumed_surfaces,
            &mut active_mounts,
            scene,
            surfaces,
            view,
            text,
            styles,
            mount,
            content,
        );
        if !output.paint_nodes().is_empty() {
            frame.push_view_scene(PreparedViewScene::new(output).with_resources(resources));
        }
    }
    frame.images = available_images;
    if let Some(surface_scene) = runtime_surface_scene(
        frame.viewport.logical_width,
        frame.viewport.logical_height,
        scene,
        surfaces
            .iter()
            .filter(|surface| !consumed_surfaces.contains(&surface.public_id)),
        styles,
        content,
    ) {
        frame.push_view_scene(surface_scene);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "recursive painter expansion needs the shared mount, resource, and scene inventories"
)]
fn push_mount_paint(
    output: &mut ViewScene,
    resources: &mut PreparedViewSceneResources,
    available_images: &mut Vec<RenderImage>,
    prepared_images: &mut BTreeMap<String, PreparedMountedViewImage>,
    consumed_surfaces: &mut BTreeSet<String>,
    active_mounts: &mut BTreeSet<u64>,
    scene: &RenderScene,
    surfaces: &[ViewRuntimeSurface],
    view: &BundleViewFrame,
    text: &[PreparedMountedViewText],
    styles: &ResolvedViewStyleFrame,
    mount: &BundleViewMountOutput,
    content: Option<ContentRect>,
) {
    if !active_mounts.insert(mount.mount.get()) {
        return;
    }
    for item in &mount.paint {
        match item {
            BundleViewPaintItem::Element { target } => {
                let scoped = mount.scoped_id(target);
                if let Some(surface) = surfaces.iter().find(|surface| surface.target == scoped)
                    && push_surface(
                        output,
                        scene,
                        surface,
                        styles.control(&scoped).or_else(|| styles.part(&scoped)),
                        content,
                    )
                    .is_some()
                {
                    consumed_surfaces.insert(surface.public_id.clone());
                }
            }
            BundleViewPaintItem::Text { source_id, target } => {
                push_mount_text(output, mount, text, styles, source_id, target);
            }
            BundleViewPaintItem::Image { target } => {
                let scoped = mount.scoped_id(target);
                if let Some(image) = prepare_mounted_view_image(
                    &scoped,
                    resources,
                    available_images,
                    prepared_images,
                ) {
                    push_image(output, image);
                }
            }
            BundleViewPaintItem::Mount { mount: child } => {
                if let Some(child) = view
                    .mounts
                    .iter()
                    .find(|candidate| candidate.mount == *child)
                {
                    push_mount_paint(
                        output,
                        resources,
                        available_images,
                        prepared_images,
                        consumed_surfaces,
                        active_mounts,
                        scene,
                        surfaces,
                        view,
                        text,
                        styles,
                        child,
                        content,
                    );
                }
            }
        }
    }
    active_mounts.remove(&mount.mount.get());
}

fn push_mount_text(
    output: &mut ViewScene,
    mount: &BundleViewMountOutput,
    text: &[PreparedMountedViewText],
    styles: &ResolvedViewStyleFrame,
    source_id: &str,
    target: &str,
) {
    let Some(prepared) = text.iter().find(|prepared| {
        prepared.mount == mount.mount.get()
            && prepared.source_id == source_id
            && prepared.target == target
    }) else {
        return;
    };
    let scoped = mount.scoped_id(target);
    let resolved_visual = styles
        .text(&scoped)
        .or_else(|| styles.part(&scoped))
        .map(ViewRuntimeNodeStyle::visual);
    let authored_visual = mount
        .text
        .iter()
        .find(|output| output.source_id == source_id)
        .and_then(|output| {
            output
                .targets
                .iter()
                .find(|candidate| candidate.public_id == target)
        })
        .map(|target| &target.style);
    let effects = resolved_visual.or(authored_visual).map_or_else(
        ViewCompositingEffects::default,
        compositing_effects_from_style,
    );
    push_text(output, prepared, effects);
}

fn prepare_mounted_view_image(
    scoped_id: &str,
    resources: &mut PreparedViewSceneResources,
    available: &mut Vec<RenderImage>,
    prepared: &mut BTreeMap<String, PreparedMountedViewImage>,
) -> Option<PreparedMountedViewImage> {
    if let Some(image) = prepared.get(scoped_id) {
        return Some(*image);
    }
    let image_index = available.iter().position(|image| image.id == scoped_id)?;
    let image = available.remove(image_index);
    let quad = image.visible_quad()?;
    let resource_index = u32::try_from(resources.images().len()).ok()?;
    let transform = image.transform_matrix();
    let prepared_image = PreparedMountedViewImage {
        resource_index,
        bounds: quad.rect,
        uv: ViewImageUvRect {
            left: quad.uv_left,
            top: quad.uv_top,
            right: quad.uv_right,
            bottom: quad.uv_bottom,
        },
        transform: ViewAffine2D {
            m11: transform.m11,
            m12: transform.m12,
            m21: transform.m21,
            m22: transform.m22,
            tx: transform.tx,
            ty: transform.ty,
        },
        opacity: f32::from(image.opacity_milli) / 1_000.0,
    };
    resources.push_image(PreparedViewImageResource {
        resource_index,
        frame: image.frame,
    });
    prepared.insert(scoped_id.to_owned(), prepared_image);
    Some(prepared_image)
}

fn push_image(scene: &mut ViewScene, image: PreparedMountedViewImage) {
    let start = u32::try_from(scene.primitives().len()).unwrap_or(u32::MAX);
    scene.push_primitive(ViewPrimitive::Image(ViewImagePrimitive {
        resource_index: image.resource_index,
        bounds: image.bounds,
        uv: image.uv,
        opacity: image.opacity,
    }));
    let end = u32::try_from(scene.primitives().len()).unwrap_or(u32::MAX);
    scene.push_paint_node(ViewPaintNode::Direct(ViewSceneContext {
        transform: image.transform,
        opacity: 1.0,
        clip: None,
        primitive_range: ViewPrimitiveRange { start, end },
    }));
}

fn runtime_surface_scene<'a>(
    viewport_width: f32,
    viewport_height: f32,
    render_scene: &RenderScene,
    surfaces: impl Iterator<Item = &'a ViewRuntimeSurface>,
    styles: &ResolvedViewStyleFrame,
    content: Option<ContentRect>,
) -> Option<PreparedViewScene> {
    let mut scene = ViewScene::new(viewport_width, viewport_height);
    let mut ordered = surfaces.collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        surface_depth_milli(left, resolved_surface_style(styles, left))
            .cmp(&surface_depth_milli(
                right,
                resolved_surface_style(styles, right),
            ))
            .then_with(|| left.public_id.cmp(&right.public_id))
    });
    for surface in ordered {
        push_surface(
            &mut scene,
            render_scene,
            surface,
            resolved_surface_style(styles, surface),
            content,
        );
    }
    (!scene.paint_nodes().is_empty()).then(|| PreparedViewScene::new(scene))
}

fn push_surface(
    scene: &mut ViewScene,
    render_scene: &RenderScene,
    surface: &ViewRuntimeSurface,
    style: Option<&ViewRuntimeNodeStyle>,
    content: Option<ContentRect>,
) -> Option<()> {
    let box_style = style.map(box_style);
    let bounds = HitRect::new(
        milli_i32_to_f32(surface.bounds.x_milli),
        milli_i32_to_f32(surface.bounds.y_milli),
        milli_u32_to_f32(surface.bounds.width_milli),
        milli_u32_to_f32(surface.bounds.height_milli),
    );
    let (bounds, clip) = scroll_adjusted_bounds(
        render_scene,
        surface.containing_scroll_region.as_deref(),
        bounds,
    )?;
    let bounds = map_rect(bounds, content);
    let scroll_clip = clip.map(|clip| map_rect(clip, content));
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return None;
    }
    let overflow_clip = box_style.and_then(|style| {
        overflow_clip(
            render_scene.viewport.logical_width,
            render_scene.viewport.logical_height,
            bounds,
            style.overflow_x,
            style.overflow_y,
        )
    });
    let clip = match (scroll_clip, overflow_clip) {
        (Some(left), Some(right)) => Some(intersection(left, right)?),
        (Some(clip), None) | (None, Some(clip)) => Some(clip),
        (None, None) => None,
    };
    let visual = style.map_or(&surface.style, |style| style.visual());
    let effects = compositing_effects_from_style(visual);
    let direct = surface_paint_range(scene, bounds, visual).map(|range| direct(range, clip));
    match (effects.is_identity(), direct) {
        (true, Some(node)) => scene.push_paint_node(node),
        (false, Some(node)) => scene.push_paint_node(ViewPaintNode::Group(
            ViewCompositingGroup::new(bounds, effects).with_children(vec![node]),
        )),
        (false, None) => scene.push_paint_node(ViewPaintNode::Group(ViewCompositingGroup::new(
            bounds, effects,
        ))),
        (true, None) => return None,
    }
    Some(())
}

fn push_text(
    scene: &mut ViewScene,
    prepared: &PreparedMountedViewText,
    effects: ViewCompositingEffects,
) {
    let start = u32::try_from(scene.primitives().len()).unwrap_or(u32::MAX);
    scene.push_primitive(ViewPrimitive::Text(ViewTextPrimitive {
        text: prepared.text,
    }));
    let end = u32::try_from(scene.primitives().len()).unwrap_or(u32::MAX);
    if start == end {
        return;
    }
    let direct = direct(ViewPrimitiveRange { start, end }, prepared.clip);
    if effects.is_identity() {
        scene.push_paint_node(direct);
    } else {
        scene.push_paint_node(ViewPaintNode::Group(
            ViewCompositingGroup::new(prepared.bounds, effects).with_children(vec![direct]),
        ));
    }
}

fn surface_paint_range(
    scene: &mut ViewScene,
    bounds: HitRect,
    visual: &ViewRuntimeControlVisualStyle,
) -> Option<ViewPrimitiveRange> {
    let radii = surface_fill_radii(visual);
    let mut paint = ViewSurfacePaint::new();
    if let Some(fill) = visual.fill.filter(|color| color.alpha > 0) {
        paint = paint.with_background(ViewSurfaceBackground::Solid {
            color: view_rgba(fill),
            radii,
        });
    }
    if let Some(border) = visual
        .border
        .filter(|border| border.width_milli > 0 && border.color.alpha > 0)
    {
        paint = paint.with_border(ViewSurfaceBorder {
            width: milli_u32_to_f32(border.width_milli),
            radius: radii.top_left.x_px.max(radii.top_left.y_px),
            color: view_rgba(border.color),
        });
    }
    scene.push_surface_primitives(bounds, &paint)
}

fn compositing_effects_from_style(
    visual: &ViewRuntimeControlVisualStyle,
) -> ViewCompositingEffects {
    ViewCompositingEffects {
        opacity: visual.opacity_milli.map_or(1.0, ratio_milli_u16),
        filters: view_filter_list(visual.filters.as_ref()),
        backdrop_filters: view_filter_list(visual.backdrop_filters.as_ref()),
        box_shadows: ViewBoxShadowList::new(
            visual
                .shadows
                .iter()
                .copied()
                .map(|shadow| view_box_shadow_from_runtime(shadow, visual)),
        ),
        ..ViewCompositingEffects::default()
    }
}

fn view_filter_list(filters: Option<&ViewRuntimeControlFilterList>) -> ViewFilterList {
    ViewFilterList::new(
        filters
            .into_iter()
            .flat_map(|filters| filters.filters.iter().copied())
            .map(view_filter_from_runtime),
    )
}

fn view_filter_from_runtime(filter: ViewRuntimeControlFilter) -> ViewFilter {
    match filter {
        ViewRuntimeControlFilter::Brightness { factor_milli } => {
            ViewFilter::Brightness(ratio_milli_u32(factor_milli))
        }
        ViewRuntimeControlFilter::Contrast { factor_milli } => {
            ViewFilter::Contrast(ratio_milli_u32(factor_milli))
        }
        ViewRuntimeControlFilter::Grayscale { amount_milli } => {
            ViewFilter::Grayscale(ratio_milli_u16(amount_milli))
        }
        ViewRuntimeControlFilter::Saturate { factor_milli } => {
            ViewFilter::Saturate(ratio_milli_u32(factor_milli))
        }
        ViewRuntimeControlFilter::HueRotate { degrees_milli } => {
            ViewFilter::HueRotateDegrees(milli_i32_to_f32(degrees_milli))
        }
        ViewRuntimeControlFilter::Invert { amount_milli } => {
            ViewFilter::Invert(ratio_milli_u16(amount_milli))
        }
        ViewRuntimeControlFilter::Sepia { amount_milli } => {
            ViewFilter::Sepia(ratio_milli_u16(amount_milli))
        }
        ViewRuntimeControlFilter::Opacity { amount_milli } => {
            ViewFilter::Opacity(ratio_milli_u16(amount_milli))
        }
        ViewRuntimeControlFilter::Blur { radius_milli } => ViewFilter::Blur {
            radius_px: milli_u32_to_f32(radius_milli),
        },
    }
}

fn view_box_shadow_from_runtime(
    shadow: ViewRuntimeShadow,
    visual: &ViewRuntimeControlVisualStyle,
) -> ViewBoxShadow {
    let offset_x = milli_i32_to_f32(shadow.offset_x_milli);
    let offset_y = milli_i32_to_f32(shadow.offset_y_milli);
    let blur = milli_u32_to_f32(shadow.blur_milli);
    let spread = milli_i32_to_f32(shadow.spread_milli);
    let radii = visual.radii_milli.map_or_else(
        || ViewBoxShadowRadii::uniform(milli_u32_to_f32(shadow.radius_milli)),
        view_box_shadow_radii_from_runtime,
    );
    let color = view_rgba(shadow.color);
    match shadow.kind {
        ViewRuntimeShadowKind::Outer => {
            ViewBoxShadow::outer_with_radii(offset_x, offset_y, blur, spread, radii, color)
        }
        ViewRuntimeShadowKind::Inset => {
            ViewBoxShadow::inset_with_radii(offset_x, offset_y, blur, spread, radii, color)
        }
    }
}

fn view_box_shadow_radii_from_runtime(radii: ViewRuntimeControlRadii) -> ViewBoxShadowRadii {
    ViewBoxShadowRadii::from_corners(
        view_box_shadow_corner_radius(radii.top_left),
        view_box_shadow_corner_radius(radii.top_right),
        view_box_shadow_corner_radius(radii.bottom_right),
        view_box_shadow_corner_radius(radii.bottom_left),
    )
}

fn view_box_shadow_corner_radius(
    radius: ViewRuntimeControlCornerRadius,
) -> ViewBoxShadowCornerRadius {
    ViewBoxShadowCornerRadius::new(
        milli_u32_to_f32(radius.x_milli),
        milli_u32_to_f32(radius.y_milli),
    )
}

fn surface_fill_radii(visual: &ViewRuntimeControlVisualStyle) -> ViewCornerRadii {
    visual.radii_milli.map_or_else(
        || {
            visual
                .radius_milli
                .map_or(ViewCornerRadii::ZERO, |radius_milli| {
                    ViewCornerRadii::uniform(milli_u32_to_f32(radius_milli))
                })
        },
        view_corner_radii_from_runtime,
    )
}

fn view_corner_radii_from_runtime(radii: ViewRuntimeControlRadii) -> ViewCornerRadii {
    ViewCornerRadii::from_corners(
        view_corner_radius(radii.top_left),
        view_corner_radius(radii.top_right),
        view_corner_radius(radii.bottom_right),
        view_corner_radius(radii.bottom_left),
    )
}

fn view_corner_radius(radius: ViewRuntimeControlCornerRadius) -> ViewCornerRadius {
    ViewCornerRadius::new(
        milli_u32_to_f32(radius.x_milli),
        milli_u32_to_f32(radius.y_milli),
    )
}

fn surface_depth_milli(surface: &ViewRuntimeSurface, style: Option<&ViewRuntimeNodeStyle>) -> i32 {
    style
        .map_or(&surface.style, |style| style.visual())
        .depth_milli
        .unwrap_or_default()
}

fn resolved_surface_style<'a>(
    styles: &'a ResolvedViewStyleFrame,
    surface: &ViewRuntimeSurface,
) -> Option<&'a ViewRuntimeNodeStyle> {
    styles
        .control(&surface.target)
        .or_else(|| styles.part(&surface.target))
        .or_else(|| styles.part(&surface.public_id))
}

fn overflow_clip(
    viewport_width: f32,
    viewport_height: f32,
    bounds: HitRect,
    overflow_x: ViewOverflow,
    overflow_y: ViewOverflow,
) -> Option<HitRect> {
    let clip_x = overflow_x != ViewOverflow::Visible;
    let clip_y = overflow_y != ViewOverflow::Visible;
    (clip_x || clip_y).then(|| {
        HitRect::new(
            if clip_x { bounds.x } else { 0.0 },
            if clip_y { bounds.y } else { 0.0 },
            if clip_x { bounds.width } else { viewport_width },
            if clip_y {
                bounds.height
            } else {
                viewport_height
            },
        )
    })
}

fn intersection(left: HitRect, right: HitRect) -> Option<HitRect> {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    let right_edge = (left.x + left.width).min(right.x + right.width);
    let bottom_edge = (left.y + left.height).min(right.y + right.height);
    (right_edge > x && bottom_edge > y).then(|| HitRect::new(x, y, right_edge - x, bottom_edge - y))
}

fn direct(range: ViewPrimitiveRange, clip: Option<HitRect>) -> ViewPaintNode {
    ViewPaintNode::Direct(ViewSceneContext {
        transform: ViewAffine2D::IDENTITY,
        opacity: 1.0,
        clip: clip.map(ViewClip::Rect),
        primitive_range: range,
    })
}

fn view_rgba(color: PresentationColor) -> ViewColorRgba8 {
    ViewColorRgba8 {
        red: color.red,
        green: color.green,
        blue: color.blue,
        alpha: color.alpha,
    }
}

fn ratio_milli_u16(value: u16) -> f32 {
    f32::from(value) / 1_000.0
}

fn ratio_milli_u32(value: u32) -> f32 {
    milli_u32_to_f32(value)
}

fn map_rect(rect: HitRect, content: Option<ContentRect>) -> HitRect {
    let Some(content) = content else {
        return rect;
    };
    let mapped = content.map_rect(FitLayoutRect::from_xywh(
        rect.x,
        rect.y,
        rect.width,
        rect.height,
    ));
    HitRect::new(
        mapped.origin.x,
        mapped.origin.y,
        mapped.size.width,
        mapped.size.height,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        PreparedMountedViewText, ResolvedViewStyleFrame, overflow_clip, prepare_mounted_view_image,
        push_image, push_mount_paint, push_surface, surface_paint_range,
    };
    use arcweft_bundle::resource_codec::view::{
        ViewRuntimeControlBorderStyle, ViewRuntimeControlVisualStyle, ViewRuntimeNodeStyle,
        ViewRuntimeSurface, ViewRuntimeSurfaceBounds,
    };
    use arcweft_presentation::appearance::{
        PresentationColor, PresentationEnvironment, SystemPaletteSet,
    };
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::image::{ImageObjectAlignment, ImageObjectFit, ImageObjectTransform};
    use arcweft_render_wgpu::geometry::{
        ChoiceScroll, InteractionVisualState, PreparedViewSceneResources, RenderImage,
        RenderImageFrame, RenderPreferences, RenderScene, RenderViewport,
    };
    use arcweft_render_wgpu::view_scene::{
        PreparedTextId, ViewPaintNode, ViewPrimitive, ViewScene,
    };
    use arcweft_runtime_driver::presentation_handles::PresentationHandleId;
    use arcweft_runtime_driver::view_runtime::{
        BundleViewFrame, BundleViewInstancePath, BundleViewMountOutput, BundleViewPaintItem,
    };
    use arcweft_view::{
        ViewElementKind, ViewMountId,
        style::{
            ComputedViewStyleBuilder, ComputedViewStyleRevision, ViewBoxAxisHostSeed,
            ViewBoxAxisSeedGeneration, ViewColorValue, ViewInheritedBoxAxes, ViewLengthMilli,
            ViewOverflow, ViewPropertyKind, ViewScalarMilli, ViewSpecifiedValue, ViewStyleAssignOp,
            ViewStyleContribution, ViewStyleContributionSource, ViewStylePriority,
        },
    };
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn axis_overflow_clip_preserves_the_visible_axis() {
        let bounds = HitRect::new(24.0, 32.0, 120.0, 48.0);
        assert_eq!(
            overflow_clip(
                320.0,
                180.0,
                bounds,
                ViewOverflow::Hidden,
                ViewOverflow::Visible,
            ),
            Some(HitRect::new(24.0, 0.0, 120.0, 180.0))
        );
    }

    #[test]
    fn current_surface_border_reaches_view_border_primitive() {
        let mut scene = ViewScene::new(320.0, 180.0);
        let style = ViewRuntimeControlVisualStyle {
            radius_milli: Some(12_000),
            border: Some(ViewRuntimeControlBorderStyle {
                color: PresentationColor::rgba(94, 234, 212, 255),
                width_milli: 2_000,
            }),
            ..ViewRuntimeControlVisualStyle::default()
        };

        let range = surface_paint_range(&mut scene, HitRect::new(20.0, 30.0, 100.0, 60.0), &style)
            .expect("border creates a surface primitive");

        assert_eq!(range.start, 0);
        assert_eq!(range.end, 1);
        let ViewPrimitive::Border(border) = &scene.primitives()[0] else {
            panic!("surface border must lower to ViewPrimitive::Border");
        };
        assert!((border.width - 2.0).abs() < f32::EPSILON);
        assert!((border.radius - 12.0).abs() < f32::EPSILON);
        assert_eq!(border.color.red, 94);
        assert_eq!(border.color.green, 234);
        assert_eq!(border.color.blue, 212);
        assert_eq!(border.color.alpha, 255);
    }

    #[test]
    fn surface_paint_uses_precomputed_resource_bounds_without_reapplying_box_style() {
        let mut builder = ComputedViewStyleBuilder::default();
        for (order, (property, value)) in [
            (
                ViewPropertyKind::Width,
                ViewSpecifiedValue::Length {
                    value: ViewLengthMilli::new(200_000),
                },
            ),
            (
                ViewPropertyKind::TranslateX,
                ViewSpecifiedValue::Length {
                    value: ViewLengthMilli::new(50_000),
                },
            ),
            (
                ViewPropertyKind::Scale,
                ViewSpecifiedValue::Scalar {
                    value: ViewScalarMilli::new(1_500),
                },
            ),
            (
                ViewPropertyKind::BackgroundColor,
                ViewSpecifiedValue::Color {
                    value: ViewColorValue::Literal {
                        color: PresentationColor::rgb(20, 40, 60),
                    },
                },
            ),
        ]
        .into_iter()
        .enumerate()
        {
            assert!(builder.apply(ViewStyleContribution::new(
                property,
                value,
                ViewStyleAssignOp::Replace,
                ViewStylePriority::new(1, 1, 0, 0, 0, u32::try_from(order).unwrap_or(u32::MAX),),
                ViewStyleContributionSource::Inherited,
            )));
        }
        let style = ViewRuntimeNodeStyle::try_from_computed(
            &builder.finish(ComputedViewStyleRevision::new(1)),
            &PresentationEnvironment::ENGINE_DEFAULT,
            &SystemPaletteSet::ENGINE_DEFAULT,
        )
        .unwrap();
        let surface = ViewRuntimeSurface {
            public_id: "surface.precomputed".to_owned(),
            target: "surface.precomputed".to_owned(),
            view: None,
            containing_scroll_region: None,
            element: ViewElementKind::Panel,
            bounds: ViewRuntimeSurfaceBounds::from_px(70, 30, 300, 60),
            style: ViewRuntimeControlVisualStyle::default(),
        };
        let mut output = ViewScene::new(320.0, 180.0);

        push_surface(
            &mut output,
            &empty_render_scene(),
            &surface,
            Some(&style),
            None,
        )
        .expect("current fill paints the precomputed resource bounds");

        let ViewPrimitive::SolidRect(fill) = &output.primitives()[0] else {
            panic!("zero-radius current fill must lower to a solid rectangle");
        };
        assert_eq!(fill.bounds, HitRect::new(70.0, 30.0, 300.0, 60.0));
    }

    #[test]
    fn mounted_image_moves_into_view_resources_with_exact_crop_and_transform() {
        let mut available = vec![RenderImage {
            id: "view_mount_7.image.card".to_owned(),
            frame: RenderImageFrame {
                index: Some(3),
                width: 4,
                height: 2,
                rgba: vec![255; 32],
            },
            bounds: HitRect::new(10.0, 20.0, 100.0, 50.0),
            containing_scroll_region: None,
            viewport_clip: Some(HitRect::new(35.0, 30.0, 50.0, 20.0)),
            placement: None,
            fit: ImageObjectFit::Stretch,
            alignment: ImageObjectAlignment::default(),
            transform: ImageObjectTransform {
                tx_milli: 5_000,
                ty_milli: -3_000,
                ..ImageObjectTransform::identity()
            },
            opacity_milli: 625,
        }];
        let mut resources = PreparedViewSceneResources::default();
        let mut prepared = BTreeMap::new();

        let image = prepare_mounted_view_image(
            "view_mount_7.image.card",
            &mut resources,
            &mut available,
            &mut prepared,
        )
        .expect("mounted image is visible");

        assert!(available.is_empty());
        assert_eq!(resources.images().len(), 1);
        assert_eq!(resources.images()[0].frame.index, Some(3));
        assert_eq!(image.bounds, HitRect::new(35.0, 30.0, 50.0, 20.0));
        assert!((image.uv.left - 0.25).abs() < f32::EPSILON);
        assert!((image.uv.top - 0.2).abs() < f32::EPSILON);
        assert!((image.uv.right - 0.75).abs() < f32::EPSILON);
        assert!((image.uv.bottom - 0.6).abs() < f32::EPSILON);
        assert!((image.transform.tx - 5.0).abs() < f32::EPSILON);
        assert!((image.transform.ty + 3.0).abs() < f32::EPSILON);
        assert!((image.opacity - 0.625).abs() < f32::EPSILON);

        let mut scene = ViewScene::new(320.0, 180.0);
        push_image(&mut scene, image);
        let ViewPrimitive::Image(primitive) = &scene.primitives()[0] else {
            panic!("mounted image must remain an image primitive");
        };
        assert_eq!(primitive.uv, image.uv);
        let ViewPaintNode::Direct(context) = &scene.paint_nodes()[0] else {
            panic!("image must retain its direct painter slot");
        };
        assert_eq!(context.transform, image.transform);
    }

    #[test]
    fn nested_mount_expands_at_its_exact_parent_painter_slot() {
        let root_mount = ViewMountId::from_raw(10);
        let child_mount = ViewMountId::from_raw(11);
        let handle = PresentationHandleId::try_new("handle.view").unwrap();
        let root = BundleViewMountOutput {
            dialogue: None,
            handle: handle.clone(),
            mount: root_mount,
            host_axis_seed: Some(ViewInheritedBoxAxes::for_host_seed(
                root_mount,
                ViewBoxAxisSeedGeneration::INITIAL,
                ViewBoxAxisHostSeed::Default,
            )),
            view: "view.Root".to_owned(),
            path: BundleViewInstancePath::default(),
            active_targets: Vec::new(),
            active_images: Vec::new(),
            paint: vec![
                BundleViewPaintItem::Text {
                    source_id: "before".to_owned(),
                    target: "before.target".to_owned(),
                },
                BundleViewPaintItem::Mount { mount: child_mount },
                BundleViewPaintItem::Text {
                    source_id: "after".to_owned(),
                    target: "after.target".to_owned(),
                },
            ],
            text: Vec::new(),
            fx: Vec::new(),
            style_nodes: Vec::new(),
        };
        let child = BundleViewMountOutput {
            dialogue: None,
            handle,
            mount: child_mount,
            host_axis_seed: None,
            view: "view.Child".to_owned(),
            path: BundleViewInstancePath::default(),
            active_targets: Vec::new(),
            active_images: Vec::new(),
            paint: vec![BundleViewPaintItem::Text {
                source_id: "child".to_owned(),
                target: "child.target".to_owned(),
            }],
            text: Vec::new(),
            fx: Vec::new(),
            style_nodes: Vec::new(),
        };
        let view = BundleViewFrame {
            mounts: vec![root.clone(), child],
            diagnostics: Vec::new(),
        };
        let text = [
            prepared_text(root_mount, "before", "before.target", 0),
            prepared_text(child_mount, "child", "child.target", 1),
            prepared_text(root_mount, "after", "after.target", 2),
        ];
        let render_scene = empty_render_scene();
        let mut output = ViewScene::new(320.0, 180.0);
        let mut resources = PreparedViewSceneResources::default();
        let mut available_images = Vec::new();
        let mut prepared_images = BTreeMap::new();
        let mut consumed_surfaces = BTreeSet::new();
        let mut active_mounts = BTreeSet::new();
        let styles = ResolvedViewStyleFrame::default();

        push_mount_paint(
            &mut output,
            &mut resources,
            &mut available_images,
            &mut prepared_images,
            &mut consumed_surfaces,
            &mut active_mounts,
            &render_scene,
            &[],
            &view,
            &text,
            &styles,
            &root,
            None,
        );

        assert_eq!(
            output.prepared_text_ids().collect::<Vec<_>>(),
            [
                PreparedTextId::from_index(0),
                PreparedTextId::from_index(1),
                PreparedTextId::from_index(2),
            ]
        );
        assert_eq!(output.paint_nodes().len(), 3);
    }

    fn prepared_text(
        mount: ViewMountId,
        source_id: &str,
        target: &str,
        index: u32,
    ) -> PreparedMountedViewText {
        PreparedMountedViewText {
            mount: mount.get(),
            source_id: source_id.to_owned(),
            target: target.to_owned(),
            text: PreparedTextId::from_index(index),
            bounds: HitRect::new(0.0, 0.0, 10.0, 10.0),
            clip: None,
        }
    }

    fn empty_render_scene() -> RenderScene {
        RenderScene {
            content_avoidance_regions: Vec::new(),
            choices: Vec::new(),
            text_inputs: Vec::new(),
            action_buttons: Vec::new(),
            focus_groups: Vec::new(),
            focus_navigation: Vec::new(),
            images: Vec::new(),
            viewport: RenderViewport {
                logical_width: 320.0,
                logical_height: 180.0,
                physical_width: 320,
                physical_height: 180,
                scale_factor: 1.0,
            },
            visual_time_millis: 0,
            preferences: RenderPreferences::default(),
            interaction: InteractionVisualState::default(),
            choice_scroll: ChoiceScroll::default(),
            scroll_regions: Vec::new(),
        }
    }
}
