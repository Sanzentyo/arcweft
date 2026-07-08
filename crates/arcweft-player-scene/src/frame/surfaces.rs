use super::{milli_i32_to_f32, milli_u32_to_f32, scroll_adjusted_bounds};
use arcweft_bundle::resource_codec::view::{
    RgbaColor, ViewRuntimeControlCornerRadius, ViewRuntimeControlFilter,
    ViewRuntimeControlFilterList, ViewRuntimeControlRadii, ViewRuntimeControlState,
    ViewRuntimeControlVisualStyle, ViewRuntimeShadow, ViewRuntimeShadowKind, ViewRuntimeSurface,
};
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::geometry::{PreparedFrame, PreparedViewScene, RenderScene};
use arcweft_render_wgpu::view_scene::{
    ViewAffine2D, ViewBoxShadow, ViewBoxShadowCornerRadius, ViewBoxShadowList, ViewBoxShadowRadii,
    ViewClip, ViewColorRgba8, ViewCompositingEffects, ViewCompositingGroup, ViewCornerRadii,
    ViewCornerRadius, ViewFilter, ViewFilterList, ViewPaintNode, ViewPrimitiveRange, ViewScene,
    ViewSceneContext, ViewSurfaceBackground, ViewSurfacePaint,
};

pub(super) fn push_runtime_surfaces(
    frame: &mut PreparedFrame,
    scene: &RenderScene,
    surfaces: &[ViewRuntimeSurface],
) {
    if let Some(surface_scene) = runtime_surface_scene(scene, surfaces) {
        frame.push_view_scene(surface_scene);
    }
}

fn runtime_surface_scene(
    render_scene: &RenderScene,
    surfaces: &[ViewRuntimeSurface],
) -> Option<PreparedViewScene> {
    let mut scene = ViewScene::new(
        render_scene.viewport.logical_width,
        render_scene.viewport.logical_height,
    );
    let mut ordered = surfaces.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        surface_depth_milli(left)
            .cmp(&surface_depth_milli(right))
            .then_with(|| left.public_id.cmp(&right.public_id))
    });
    for surface in ordered {
        push_surface(&mut scene, render_scene, surface);
    }
    (!scene.paint_nodes().is_empty()).then(|| PreparedViewScene::new(scene))
}

fn push_surface(
    scene: &mut ViewScene,
    render_scene: &RenderScene,
    surface: &ViewRuntimeSurface,
) -> Option<()> {
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
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return None;
    }
    let visual = surface
        .style
        .visual_for_state(ViewRuntimeControlState::Normal);
    let effects = compositing_effects_from_style(&visual);
    let direct = surface_fill_range(scene, bounds, &visual).map(|range| direct(range, clip));
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

fn surface_fill_range(
    scene: &mut ViewScene,
    bounds: HitRect,
    visual: &ViewRuntimeControlVisualStyle,
) -> Option<ViewPrimitiveRange> {
    let fill = visual.fill.filter(|color| color.alpha > 0)?;
    let radii = surface_fill_radii(visual);
    let paint = ViewSurfacePaint::new().with_background(ViewSurfaceBackground::Solid {
        color: view_rgba(fill),
        radii,
    });
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

fn surface_depth_milli(surface: &ViewRuntimeSurface) -> i32 {
    surface
        .style
        .visual_for_state(ViewRuntimeControlState::Normal)
        .depth_milli
        .unwrap_or_default()
}

fn direct(range: ViewPrimitiveRange, clip: Option<HitRect>) -> ViewPaintNode {
    ViewPaintNode::Direct(ViewSceneContext {
        transform: ViewAffine2D::IDENTITY,
        opacity: 1.0,
        clip: clip.map(ViewClip::Rect),
        primitive_range: range,
    })
}

fn view_rgba(color: RgbaColor) -> ViewColorRgba8 {
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
