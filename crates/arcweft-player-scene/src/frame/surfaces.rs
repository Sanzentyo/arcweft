use super::view_style::ResolvedViewStyleFrame;
use super::view_text::{PreparedMountedViewText, PreparedViewText};
use super::{
    PlayerFrameError, ViewCommittedGeometryFrame, ViewGeometryConversionError,
    ViewGeometryConversionField, ViewGeometryPlatform, ViewGeometryProductKind,
    ViewGeometryRuntimeError, ViewGeometryTargetKey,
};
use arcweft_bundle::resource_codec::view::{
    ViewRuntimeControlCornerRadius, ViewRuntimeControlFilter, ViewRuntimeControlFilterList,
    ViewRuntimeControlRadii, ViewRuntimeControlVisualStyle, ViewRuntimeGeometryOwner,
    ViewRuntimeNodeStyle, ViewRuntimeShadow, ViewRuntimeShadowKind,
};
use arcweft_interaction_model::presentation::ImageObjectTransform;
use arcweft_layout::{ContentRect, LayoutRect as FitLayoutRect};
use arcweft_presentation::appearance::PresentationColor;
use arcweft_presentation::hit::HitRect;

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
    BundleViewFrame, BundleViewMountOutput, BundleViewPaintItem, BundleViewTextOrigin,
    BundleViewTextOutputIndex,
};
use arcweft_view::geometry::{ViewFinalGeometry, ViewGeometryConsumer};
use arcweft_view::style::ViewStyleNodeKey;
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
    view: &BundleViewFrame,
    text: &PreparedViewText,
    styles: &ResolvedViewStyleFrame,
    geometry: &ViewCommittedGeometryFrame,
    content: Option<ContentRect>,
) -> Result<(), PlayerFrameError> {
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
            &mut active_mounts,
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
    active_mounts: &mut BTreeSet<u64>,
    view: &BundleViewFrame,
    text: &PreparedViewText,
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
            BundleViewPaintItem::Surface { node } => {
                let key = node.style_node_key(mount.mount);
                let style = styles
                    .node(&key)
                    .filter(|style| {
                        matches!(
                            style.physical().owner(),
                            ViewRuntimeGeometryOwner::Element(element) if element.paints_surface()
                        )
                    })
                    .ok_or_else(|| PlayerFrameError::InvalidSurfacePaintNode {
                        node: key.clone(),
                    })?;
                if let Some(committed) = geometry.final_geometry(&key) {
                    push_surface(output, &key, committed, style.visual(), content)?;
                }
            }
            BundleViewPaintItem::Text {
                output: text_output,
            } => {
                push_mount_text(output, mount, text, styles, *text_output)?;
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
                        active_mounts,
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
    text: &PreparedViewText,
    styles: &ResolvedViewStyleFrame,
    output_index: BundleViewTextOutputIndex,
) -> Result<(), PlayerFrameError> {
    let Some(prepared) = text.get(&(mount.mount, output_index)) else {
        return Ok(());
    };
    let missing = || PlayerFrameError::MissingTextPaintOutput {
        mount: mount.mount.get(),
        output: output_index,
    };
    let text = mount.text_output(output_index).ok_or_else(missing)?;
    let BundleViewTextOrigin::Block { target, .. } = &text.origin else {
        return Err(missing());
    };
    let scoped = mount.scoped_id(&target.public_id);
    let resolved_visual = styles.text(&scoped).map(ViewRuntimeNodeStyle::visual);
    let effects = compositing_effects_from_style(resolved_visual.unwrap_or(&target.style))?;
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

fn push_surface(
    scene: &mut ViewScene,
    node: &ViewStyleNodeKey,
    geometry: &ViewFinalGeometry,
    visual: &ViewRuntimeControlVisualStyle,
    content: Option<ContentRect>,
) -> Result<(), PlayerFrameError> {
    let Some(bounds) = ViewCommittedGeometryFrame::node_consumer_hit_rect(
        node,
        geometry,
        ViewGeometryConsumer::Layout,
    )?
    else {
        return Ok(());
    };
    let Some(visible_bounds) = ViewCommittedGeometryFrame::node_consumer_hit_rect(
        node,
        geometry,
        ViewGeometryConsumer::Capture,
    )?
    else {
        return Ok(());
    };
    let bounds = map_rect(bounds, content);
    let visible_bounds = map_rect(visible_bounds, content);
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return Ok(());
    }
    let clip = (visible_bounds != bounds).then_some(visible_bounds);
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
        (true, None) => {}
    }
    Ok(())
}

pub(super) fn mount_surface_avoidance(
    mount: &BundleViewMountOutput,
    geometry: &ViewCommittedGeometryFrame,
) -> Result<Option<HitRect>, PlayerFrameError> {
    let mut bounds: Option<HitRect> = None;
    for node in mount.surface_nodes() {
        let key = node.style_node_key(mount.mount);
        let Some(geometry) = geometry.final_geometry(&key) else {
            continue;
        };
        if let Some(rect) = ViewCommittedGeometryFrame::node_consumer_hit_rect(
            &key,
            geometry,
            ViewGeometryConsumer::Avoidance,
        )? {
            bounds = Some(bounds.map_or(rect, |current| super::union_hit_rects(current, rect)));
        }
    }
    Ok(bounds)
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
    use arcweft_id::presentation::PresentationHandleId;
    use arcweft_id::view::ViewId;
    use arcweft_presentation::appearance::PresentationColor;
    use arcweft_presentation::hit::HitRect;
    use arcweft_render_wgpu::geometry::PreparedViewSceneResources;
    use arcweft_render_wgpu::view_scene::{PreparedTextId, ViewPrimitive, ViewScene};
    use arcweft_runtime_driver::view_runtime::{
        BundleViewFrame, BundleViewInstancePath, BundleViewMountOutput, BundleViewPaintItem,
        BundleViewStyleNodeId, BundleViewTextOrigin, BundleViewTextOutput,
        BundleViewTextOutputIndex, BundleViewTextTarget, BundleViewTextValue,
    };
    use arcweft_view::{
        ViewMountId,
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
            action_buttons: Vec::new(),
            text_inputs: Vec::new(),
            paint: vec![
                BundleViewPaintItem::Text {
                    output: BundleViewTextOutputIndex::new(0),
                },
                BundleViewPaintItem::Mount { mount: child_mount },
                BundleViewPaintItem::Text {
                    output: BundleViewTextOutputIndex::new(1),
                },
            ],
            text: vec![text_output("before", 0), text_output("after", 2)],
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
            action_buttons: Vec::new(),
            text_inputs: Vec::new(),
            paint: vec![BundleViewPaintItem::Text {
                output: BundleViewTextOutputIndex::new(0),
            }],
            text: vec![text_output("child", 0)],
            fx: Vec::new(),
            events: Vec::new(),
            style_nodes: Vec::new(),
        };
        let view = BundleViewFrame {
            mounts: vec![root.clone(), child],
            diagnostics: Vec::new(),
        };
        let text = [
            (
                (root_mount, BundleViewTextOutputIndex::new(0)),
                prepared_text(0),
            ),
            (
                (child_mount, BundleViewTextOutputIndex::new(0)),
                prepared_text(1),
            ),
            (
                (root_mount, BundleViewTextOutputIndex::new(1)),
                prepared_text(2),
            ),
        ]
        .into_iter()
        .collect();
        let mut output = ViewScene::new(320.0, 180.0);
        let mut resources = PreparedViewSceneResources::default();
        let mut available_images = Vec::new();
        let mut prepared_images = BTreeMap::new();
        let mut active_mounts = BTreeSet::new();
        let styles = ResolvedViewStyleFrame::default();
        let geometry = ViewCommittedGeometryFrame::empty_for_test();

        push_mount_paint(
            &mut output,
            &mut resources,
            &mut available_images,
            &mut prepared_images,
            &mut active_mounts,
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

    fn prepared_text(text: u32) -> PreparedMountedViewText {
        PreparedMountedViewText {
            text: PreparedTextId::from_index(text),
            bounds: HitRect::new(0.0, 0.0, 10.0, 10.0),
            clip: None,
        }
    }

    fn text_output(source: &str, instruction: u32) -> BundleViewTextOutput {
        BundleViewTextOutput {
            source_id: source.to_owned(),
            origin: BundleViewTextOrigin::Block {
                node: BundleViewStyleNodeId {
                    path: BundleViewInstancePath::default(),
                    instruction,
                },
                target: BundleViewTextTarget {
                    public_id: format!("{source}.target"),
                    containing_scroll_region: None,
                    bounds: arcweft_bundle::resource_codec::ViewTextBlockBounds::from_px(
                        0, 0, 10, 10,
                    ),
                    selection_policy: Default::default(),
                    style: Default::default(),
                },
            },
            value: BundleViewTextValue::Plain {
                value: source.to_owned(),
            },
            classification: Default::default(),
            replacement: None,
        }
    }
}
