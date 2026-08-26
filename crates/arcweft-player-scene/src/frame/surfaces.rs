use super::view_style::ResolvedViewStyleFrame;
use super::view_text::PreparedMountedViewText;
use super::{
    PlayerFrameError, ViewCommittedGeometryFrame, ViewGeometryConversionError,
    ViewGeometryConversionField, ViewGeometryPlatform, ViewGeometryProductKind,
    ViewGeometryRuntimeError, ViewGeometryTargetKey,
};
use arcweft_bundle::resource_codec::view::{
    ViewRuntimeControlCornerRadius, ViewRuntimeControlFilter, ViewRuntimeControlFilterList,
    ViewRuntimeControlRadii, ViewRuntimeControlVisualStyle, ViewRuntimeNodeStyle,
    ViewRuntimeShadow, ViewRuntimeShadowKind, ViewRuntimeSurface,
};
use arcweft_layout::{ContentRect, LayoutRect as FitLayoutRect};
use arcweft_presentation::appearance::PresentationColor;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::image::ImageObjectTransform;
use arcweft_render_wgpu::geometry::{
    PreparedFrame, PreparedViewImageResource, PreparedViewScene, PreparedViewSceneResources,
    RenderImage,
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
use arcweft_view::geometry::ViewGeometryConsumer;
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
    surfaces: &[ViewRuntimeSurface],
    view: &BundleViewFrame,
    text: &[PreparedMountedViewText],
    styles: &ResolvedViewStyleFrame,
    geometry: &ViewCommittedGeometryFrame,
    content: Option<ContentRect>,
) -> Result<(), PlayerFrameError> {
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
            surfaces,
            view,
            text,
            styles,
            geometry,
            mount,
            content,
        )?;
        if !output.paint_nodes().is_empty() {
            frame.push_view_scene(PreparedViewScene::new(output).with_resources(resources));
        }
    }
    frame.images = available_images;
    if let Some(surface_scene) = runtime_surface_scene(
        frame.viewport.logical_width,
        frame.viewport.logical_height,
        surfaces
            .iter()
            .filter(|surface| !consumed_surfaces.contains(&surface.public_id)),
        styles,
        geometry,
        content,
    )? {
        frame.push_view_scene(surface_scene);
    }
    Ok(())
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
    surfaces: &[ViewRuntimeSurface],
    view: &BundleViewFrame,
    text: &[PreparedMountedViewText],
    styles: &ResolvedViewStyleFrame,
    geometry: &ViewCommittedGeometryFrame,
    mount: &BundleViewMountOutput,
    content: Option<ContentRect>,
) -> Result<(), PlayerFrameError> {
    if !active_mounts.insert(mount.mount.get()) {
        return Ok(());
    }
    for item in &mount.paint {
        match item {
            BundleViewPaintItem::Element { target } => {
                let scoped = mount.scoped_id(target);
                if let Some(surface) = surfaces.iter().find(|surface| surface.target == scoped)
                    && push_surface(
                        output,
                        surface,
                        styles.control(&scoped).or_else(|| styles.part(&scoped)),
                        geometry,
                        content,
                    )?
                    .is_some()
                {
                    consumed_surfaces.insert(surface.public_id.clone());
                }
            }
            BundleViewPaintItem::Text { source_id, target } => {
                push_mount_text(output, mount, text, styles, source_id, target)?;
            }
            BundleViewPaintItem::Image { target } => {
                let scoped = mount.scoped_id(target);
                if let Some(image) = prepare_mounted_view_image(
                    &scoped,
                    resources,
                    available_images,
                    prepared_images,
                    geometry,
                    content,
                )? {
                    push_image(output, image)?;
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
                        surfaces,
                        view,
                        text,
                        styles,
                        geometry,
                        child,
                        content,
                    )?;
                }
            }
        }
    }
    active_mounts.remove(&mount.mount.get());
    Ok(())
}

fn push_mount_text(
    output: &mut ViewScene,
    mount: &BundleViewMountOutput,
    text: &[PreparedMountedViewText],
    styles: &ResolvedViewStyleFrame,
    source_id: &str,
    target: &str,
) -> Result<(), PlayerFrameError> {
    let Some(prepared) = text.iter().find(|prepared| {
        prepared.mount == mount.mount.get()
            && prepared.source_id == source_id
            && prepared.target == target
    }) else {
        return Ok(());
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
    let effects = match resolved_visual.or(authored_visual) {
        Some(visual) => compositing_effects_from_style(visual)?,
        None => ViewCompositingEffects::default(),
    };
    push_text(output, prepared, effects)?;
    Ok(())
}

fn prepare_mounted_view_image(
    scoped_id: &str,
    resources: &mut PreparedViewSceneResources,
    available: &mut Vec<RenderImage>,
    prepared: &mut BTreeMap<String, PreparedMountedViewImage>,
    geometry: &ViewCommittedGeometryFrame,
    content: Option<ContentRect>,
) -> Result<Option<PreparedMountedViewImage>, PlayerFrameError> {
    if let Some(image) = prepared.get(scoped_id) {
        return Ok(Some(*image));
    }
    let target = ViewGeometryTargetKey::new(ViewGeometryProductKind::Image, scoped_id.to_owned());
    let Some(bounds) = geometry.target_consumer_hit_rect(&target, ViewGeometryConsumer::Layout)?
    else {
        return Ok(None);
    };
    let Some(visible_bounds) =
        geometry.target_consumer_hit_rect(&target, ViewGeometryConsumer::Capture)?
    else {
        return Ok(None);
    };
    let Some(image_index) = available.iter().position(|image| image.id == scoped_id) else {
        return Ok(None);
    };
    let mut image = available.remove(image_index);
    image.bounds = map_rect(bounds, content);
    image.viewport_clip = Some(map_rect(visible_bounds, content));
    image.containing_scroll_region = None;
    image.placement = None;
    image.transform = ImageObjectTransform::identity();
    let Some(quad) = image.visible_quad() else {
        return Ok(None);
    };
    let resource_index = paint_index(resources.images().len())?;
    let prepared_image = PreparedMountedViewImage {
        resource_index,
        bounds: quad.rect,
        uv: ViewImageUvRect {
            left: quad.uv_left,
            top: quad.uv_top,
            right: quad.uv_right,
            bottom: quad.uv_bottom,
        },
        transform: ViewAffine2D::IDENTITY,
        opacity: f32::from(image.opacity_milli) / 1_000.0,
    };
    resources.push_image(PreparedViewImageResource {
        resource_index,
        frame: image.frame,
    });
    prepared.insert(scoped_id.to_owned(), prepared_image);
    Ok(Some(prepared_image))
}

fn push_image(
    scene: &mut ViewScene,
    image: PreparedMountedViewImage,
) -> Result<(), PlayerFrameError> {
    let start = paint_index(scene.primitives().len())?;
    scene.push_primitive(ViewPrimitive::Image(ViewImagePrimitive {
        resource_index: image.resource_index,
        bounds: image.bounds,
        uv: image.uv,
        opacity: image.opacity,
    }));
    let end = paint_index(scene.primitives().len())?;
    scene.push_paint_node(ViewPaintNode::Direct(ViewSceneContext {
        transform: image.transform,
        opacity: 1.0,
        clip: None,
        primitive_range: ViewPrimitiveRange { start, end },
    }));
    Ok(())
}

fn runtime_surface_scene<'a>(
    viewport_width: f32,
    viewport_height: f32,
    surfaces: impl Iterator<Item = &'a ViewRuntimeSurface>,
    styles: &ResolvedViewStyleFrame,
    geometry: &ViewCommittedGeometryFrame,
    content: Option<ContentRect>,
) -> Result<Option<PreparedViewScene>, PlayerFrameError> {
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
            surface,
            resolved_surface_style(styles, surface),
            geometry,
            content,
        )?;
    }
    Ok((!scene.paint_nodes().is_empty()).then(|| PreparedViewScene::new(scene)))
}

fn push_surface(
    scene: &mut ViewScene,
    surface: &ViewRuntimeSurface,
    style: Option<&ViewRuntimeNodeStyle>,
    geometry: &ViewCommittedGeometryFrame,
    content: Option<ContentRect>,
) -> Result<Option<()>, PlayerFrameError> {
    let target =
        ViewGeometryTargetKey::new(ViewGeometryProductKind::Surface, surface.target.clone());
    let Some(bounds) = geometry.target_consumer_hit_rect(&target, ViewGeometryConsumer::Layout)?
    else {
        return Ok(None);
    };
    let Some(visible_bounds) =
        geometry.target_consumer_hit_rect(&target, ViewGeometryConsumer::Capture)?
    else {
        return Ok(None);
    };
    let bounds = map_rect(bounds, content);
    let visible_bounds = map_rect(visible_bounds, content);
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return Ok(None);
    }
    let clip = (visible_bounds != bounds).then_some(visible_bounds);
    let visual = style.map_or(&surface.style, |style| style.visual());
    let effects = compositing_effects_from_style(visual)?;
    let direct = surface_paint_range(scene, bounds, visual)?.map(|range| direct(range, clip));
    match (effects.is_identity(), direct) {
        (true, Some(node)) => scene.push_paint_node(node),
        (false, Some(node)) => scene.push_paint_node(ViewPaintNode::Group(
            ViewCompositingGroup::new(bounds, effects).with_children(vec![node]),
        )),
        (false, None) => scene.push_paint_node(ViewPaintNode::Group(ViewCompositingGroup::new(
            bounds, effects,
        ))),
        (true, None) => return Ok(None),
    }
    Ok(Some(()))
}

fn push_text(
    scene: &mut ViewScene,
    prepared: &PreparedMountedViewText,
    effects: ViewCompositingEffects,
) -> Result<(), PlayerFrameError> {
    let start = paint_index(scene.primitives().len())?;
    scene.push_primitive(ViewPrimitive::Text(ViewTextPrimitive {
        text: prepared.text,
    }));
    let end = paint_index(scene.primitives().len())?;
    if start == end {
        return Ok(());
    }
    let direct = direct(ViewPrimitiveRange { start, end }, prepared.clip);
    if effects.is_identity() {
        scene.push_paint_node(direct);
    } else {
        scene.push_paint_node(ViewPaintNode::Group(
            ViewCompositingGroup::new(prepared.bounds, effects).with_children(vec![direct]),
        ));
    }
    Ok(())
}

fn surface_paint_range(
    scene: &mut ViewScene,
    bounds: HitRect,
    visual: &ViewRuntimeControlVisualStyle,
) -> Result<Option<ViewPrimitiveRange>, PlayerFrameError> {
    let radii = surface_fill_radii(visual)?;
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
            width: paint_milli(
                i64::from(border.width_milli),
                ViewGeometryConversionField::Width,
            )?,
            radius: radii.top_left.x_px.max(radii.top_left.y_px),
            color: view_rgba(border.color),
        });
    }
    let start = paint_index(scene.primitives().len())?;
    paint.append_primitives(bounds, |primitive| scene.push_primitive(primitive));
    let end = paint_index(scene.primitives().len())?;
    Ok((start != end).then_some(ViewPrimitiveRange { start, end }))
}

fn compositing_effects_from_style(
    visual: &ViewRuntimeControlVisualStyle,
) -> Result<ViewCompositingEffects, PlayerFrameError> {
    let box_shadows = visual
        .shadows
        .iter()
        .copied()
        .map(|shadow| view_box_shadow_from_runtime(shadow, visual))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ViewCompositingEffects {
        opacity: visual.opacity_milli.map_or(1.0, ratio_milli_u16),
        filters: view_filter_list(visual.filters.as_ref())?,
        backdrop_filters: view_filter_list(visual.backdrop_filters.as_ref())?,
        box_shadows: ViewBoxShadowList::new(box_shadows),
        ..ViewCompositingEffects::default()
    })
}

fn view_filter_list(
    filters: Option<&ViewRuntimeControlFilterList>,
) -> Result<ViewFilterList, PlayerFrameError> {
    let filters = filters
        .into_iter()
        .flat_map(|filters| filters.filters.iter().copied())
        .map(view_filter_from_runtime)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ViewFilterList::new(filters))
}

fn view_filter_from_runtime(
    filter: ViewRuntimeControlFilter,
) -> Result<ViewFilter, PlayerFrameError> {
    Ok(match filter {
        ViewRuntimeControlFilter::Brightness { factor_milli } => {
            ViewFilter::Brightness(ratio_milli_u32(factor_milli)?)
        }
        ViewRuntimeControlFilter::Contrast { factor_milli } => {
            ViewFilter::Contrast(ratio_milli_u32(factor_milli)?)
        }
        ViewRuntimeControlFilter::Grayscale { amount_milli } => {
            ViewFilter::Grayscale(ratio_milli_u16(amount_milli))
        }
        ViewRuntimeControlFilter::Saturate { factor_milli } => {
            ViewFilter::Saturate(ratio_milli_u32(factor_milli)?)
        }
        ViewRuntimeControlFilter::HueRotate { degrees_milli } => ViewFilter::HueRotateDegrees(
            paint_milli(i64::from(degrees_milli), ViewGeometryConversionField::Scale)?,
        ),
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
            radius_px: paint_milli(i64::from(radius_milli), ViewGeometryConversionField::Width)?,
        },
    })
}

fn view_box_shadow_from_runtime(
    shadow: ViewRuntimeShadow,
    visual: &ViewRuntimeControlVisualStyle,
) -> Result<ViewBoxShadow, PlayerFrameError> {
    let offset_x = paint_milli(
        i64::from(shadow.offset_x_milli),
        ViewGeometryConversionField::Left,
    )?;
    let offset_y = paint_milli(
        i64::from(shadow.offset_y_milli),
        ViewGeometryConversionField::Top,
    )?;
    let blur = paint_milli(
        i64::from(shadow.blur_milli),
        ViewGeometryConversionField::Width,
    )?;
    let spread = paint_milli(
        i64::from(shadow.spread_milli),
        ViewGeometryConversionField::Width,
    )?;
    let radii = match visual.radii_milli {
        Some(radii) => view_box_shadow_radii_from_runtime(radii)?,
        None => ViewBoxShadowRadii::uniform(paint_milli(
            i64::from(shadow.radius_milli),
            ViewGeometryConversionField::Width,
        )?),
    };
    let color = view_rgba(shadow.color);
    Ok(match shadow.kind {
        ViewRuntimeShadowKind::Outer => {
            ViewBoxShadow::outer_with_radii(offset_x, offset_y, blur, spread, radii, color)
        }
        ViewRuntimeShadowKind::Inset => {
            ViewBoxShadow::inset_with_radii(offset_x, offset_y, blur, spread, radii, color)
        }
    })
}

fn view_box_shadow_radii_from_runtime(
    radii: ViewRuntimeControlRadii,
) -> Result<ViewBoxShadowRadii, PlayerFrameError> {
    Ok(ViewBoxShadowRadii::from_corners(
        view_box_shadow_corner_radius(radii.top_left)?,
        view_box_shadow_corner_radius(radii.top_right)?,
        view_box_shadow_corner_radius(radii.bottom_right)?,
        view_box_shadow_corner_radius(radii.bottom_left)?,
    ))
}

fn view_box_shadow_corner_radius(
    radius: ViewRuntimeControlCornerRadius,
) -> Result<ViewBoxShadowCornerRadius, PlayerFrameError> {
    Ok(ViewBoxShadowCornerRadius::new(
        paint_milli(
            i64::from(radius.x_milli),
            ViewGeometryConversionField::Width,
        )?,
        paint_milli(
            i64::from(radius.y_milli),
            ViewGeometryConversionField::Height,
        )?,
    ))
}

fn surface_fill_radii(
    visual: &ViewRuntimeControlVisualStyle,
) -> Result<ViewCornerRadii, PlayerFrameError> {
    match visual.radii_milli {
        Some(radii) => view_corner_radii_from_runtime(radii),
        None => match visual.radius_milli {
            Some(radius_milli) => Ok(ViewCornerRadii::uniform(paint_milli(
                i64::from(radius_milli),
                ViewGeometryConversionField::Width,
            )?)),
            None => Ok(ViewCornerRadii::ZERO),
        },
    }
}

fn view_corner_radii_from_runtime(
    radii: ViewRuntimeControlRadii,
) -> Result<ViewCornerRadii, PlayerFrameError> {
    Ok(ViewCornerRadii::from_corners(
        view_corner_radius(radii.top_left)?,
        view_corner_radius(radii.top_right)?,
        view_corner_radius(radii.bottom_right)?,
        view_corner_radius(radii.bottom_left)?,
    ))
}

fn view_corner_radius(
    radius: ViewRuntimeControlCornerRadius,
) -> Result<ViewCornerRadius, PlayerFrameError> {
    Ok(ViewCornerRadius::new(
        paint_milli(
            i64::from(radius.x_milli),
            ViewGeometryConversionField::Width,
        )?,
        paint_milli(
            i64::from(radius.y_milli),
            ViewGeometryConversionField::Height,
        )?,
    ))
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

fn ratio_milli_u32(value: u32) -> Result<f32, PlayerFrameError> {
    paint_milli(i64::from(value), ViewGeometryConversionField::Scale)
}

fn paint_milli(
    value_milli: i64,
    field: ViewGeometryConversionField,
) -> Result<f32, PlayerFrameError> {
    ViewGeometryConversionError::exact_f32(
        None,
        ViewGeometryPlatform::Wgpu,
        ViewGeometryConsumer::Paint,
        field,
        value_milli,
    )
    .map_err(|source| {
        ViewGeometryRuntimeError::Conversion {
            node: None,
            consumer: ViewGeometryConsumer::Paint,
            source,
        }
        .into()
    })
}

fn paint_index(value: usize) -> Result<u32, PlayerFrameError> {
    let value = u64::try_from(value).expect("all supported Rust pointer widths fit in u64");
    u32::try_from(value)
        .map_err(|_| ViewGeometryConversionError::IndexRange {
            node: None,
            platform: ViewGeometryPlatform::Wgpu,
            consumer: ViewGeometryConsumer::Paint,
            field: ViewGeometryConversionField::IndexRange,
            value,
            max: u64::from(u32::MAX),
        })
        .map_err(|source| {
            ViewGeometryRuntimeError::Conversion {
                node: None,
                consumer: ViewGeometryConsumer::Paint,
                source,
            }
            .into()
        })
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
        PreparedMountedViewText, ResolvedViewStyleFrame, push_mount_paint, surface_paint_range,
    };
    use crate::frame::ViewCommittedGeometryFrame;
    use arcweft_bundle::resource_codec::view::{
        ViewRuntimeControlBorderStyle, ViewRuntimeControlVisualStyle,
    };
    use arcweft_presentation::appearance::PresentationColor;
    use arcweft_presentation::hit::HitRect;
    use arcweft_render_wgpu::geometry::PreparedViewSceneResources;
    use arcweft_render_wgpu::view_scene::{PreparedTextId, ViewPrimitive, ViewScene};
    use arcweft_runtime_driver::presentation_handles::PresentationHandleId;
    use arcweft_runtime_driver::view_runtime::{
        BundleViewFrame, BundleViewInstancePath, BundleViewMountOutput, BundleViewPaintItem,
    };
    use arcweft_view::{
        ViewId, ViewMountId,
        style::{ViewBoxAxisHostSeed, ViewBoxAxisSeedGeneration, ViewInheritedBoxAxes},
    };
    use std::collections::{BTreeMap, BTreeSet};

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
            .expect("paint conversion succeeds")
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
            view: ViewId::try_new("view.Root").unwrap(),
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
            events: Vec::new(),
            style_nodes: Vec::new(),
        };
        let child = BundleViewMountOutput {
            dialogue: None,
            handle,
            mount: child_mount,
            host_axis_seed: None,
            view: ViewId::try_new("view.Child").unwrap(),
            path: BundleViewInstancePath::default(),
            active_targets: Vec::new(),
            active_images: Vec::new(),
            paint: vec![BundleViewPaintItem::Text {
                source_id: "child".to_owned(),
                target: "child.target".to_owned(),
            }],
            text: Vec::new(),
            fx: Vec::new(),
            events: Vec::new(),
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
        let mut output = ViewScene::new(320.0, 180.0);
        let mut resources = PreparedViewSceneResources::default();
        let mut available_images = Vec::new();
        let mut prepared_images = BTreeMap::new();
        let mut consumed_surfaces = BTreeSet::new();
        let mut active_mounts = BTreeSet::new();
        let styles = ResolvedViewStyleFrame::default();
        let geometry = ViewCommittedGeometryFrame::empty_for_test();

        push_mount_paint(
            &mut output,
            &mut resources,
            &mut available_images,
            &mut prepared_images,
            &mut consumed_surfaces,
            &mut active_mounts,
            &[],
            &view,
            &text,
            &styles,
            &geometry,
            &root,
            None,
        )
        .expect("nested paint order is valid");

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
}
