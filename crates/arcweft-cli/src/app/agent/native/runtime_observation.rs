use super::*;
use arcweft_agent_protocol::view::{
    AgentFocusAutoScrollPolicy, AgentObservedScrollRegion, AgentObservedVirtualItem,
    AgentObservedVirtualList, AgentScrollAxis, AgentScrollContentPart, AgentScrollIndicatorsPolicy,
    AgentScrollOverflow, AgentScrollOverscrollPolicy, AgentScrollRegionParts,
    AgentScrollRegionRole, AgentScrollViewportPart,
};
use arcweft_render_wgpu::geometry::{
    PreparedFrame, RenderFocusAutoScrollPolicy, RenderScrollAxis, RenderScrollIndicatorsPolicy,
    RenderScrollOverflow, RenderScrollOverscrollPolicy,
};
use std::collections::BTreeSet;

pub(super) fn agent_observed_virtual_lists(
    tables: &[arcweft_view::virtualization::ViewVirtualRangeTable],
) -> Vec<AgentObservedVirtualList> {
    tables
        .iter()
        .map(|table| {
            let target = format!("view.mount.{}", table.mount.get());
            AgentObservedVirtualList {
                target: target.clone(),
                scroll_target: table.scroll_target.as_str().to_owned(),
                axis: match table.axis {
                    arcweft_view::program::ViewVirtualAxis::Horizontal => {
                        AgentScrollAxis::Horizontal
                    }
                    arcweft_view::program::ViewVirtualAxis::Vertical => AgentScrollAxis::Vertical,
                },
                viewport_extent_milli: table.viewport_extent_milli,
                offset_milli: table.offset_milli,
                total_extent_milli: table.total_extent_milli,
                materialized_start: table.materialized.start,
                materialized_end: table.materialized.end,
                items: table
                    .items
                    .iter()
                    .map(|item| AgentObservedVirtualItem {
                        target: format!("{target}.item.{}", item.key.0),
                        index: item.index,
                        key: item.key.0,
                        start_milli: item.start_milli,
                        extent_milli: item.extent_milli,
                        materialized: item.materialized,
                    })
                    .collect(),
            }
        })
        .collect()
}

pub(super) fn agent_observe_layout_scene_graph(viewport: &AgentViewport) -> serde_json::Value {
    let content_rect = agent_observe_content_rect(viewport);
    let metadata = content_rect.fit_transform_metadata(
        arcweft_layout::LayoutCoordinateSpace::Output,
        arcweft_layout::LayoutCoordinateSpace::Output,
    );
    serde_json::json!({
        "kind": "layout.viewport_scale",
        "renderer_kind": "shared_wgpu_prepared_frame",
        "output_viewport": {
            "width": viewport.width,
            "height": viewport.height,
            "device_pixel_ratio": viewport.scale
        },
        "design_viewport": {
            "width": AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH,
            "height": AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT
        },
        "coordinate_spaces": {
            "design": metadata.design_space,
            "content": metadata.content_space,
            "output": metadata.output_space,
            "serialized_geometry": metadata.serialized_geometry_space,
            "hit_test_input": metadata.hit_test_input_space
        },
        "scale_policy": metadata.policy.as_str(),
        "content_rect": {
            "x": metadata.content_rect.origin.x,
            "y": metadata.content_rect.origin.y,
            "width": metadata.content_rect.size.width,
            "height": metadata.content_rect.size.height
        },
        "visible_output_rect": {
            "x": metadata.visible_output_rect.origin.x,
            "y": metadata.visible_output_rect.origin.y,
            "width": metadata.visible_output_rect.size.width,
            "height": metadata.visible_output_rect.size.height
        },
        "visible_design_rect": {
            "x": metadata.visible_design_rect.origin.x,
            "y": metadata.visible_design_rect.origin.y,
            "width": metadata.visible_design_rect.size.width,
            "height": metadata.visible_design_rect.size.height
        },
        "bars": {
            "top": metadata.bars.top,
            "right": metadata.bars.right,
            "bottom": metadata.bars.bottom,
            "left": metadata.bars.left
        },
        "crop": {
            "top": metadata.crop.top,
            "right": metadata.crop.right,
            "bottom": metadata.crop.bottom,
            "left": metadata.crop.left
        },
        "scale": {
            "x": metadata.scale_x,
            "y": metadata.scale_y
        },
        "raw_pixel_mode": metadata.raw_pixel_mode
    })
}

pub(super) fn agent_observe_content_rect(viewport: &AgentViewport) -> arcweft_layout::ContentRect {
    arcweft_layout::ContentRect::calculate(
        arcweft_layout::LayoutSize::new(
            agent_u32_to_f32(AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH),
            agent_u32_to_f32(AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT),
        ),
        arcweft_layout::LayoutSize::new(
            agent_u32_to_f32(viewport.width),
            agent_u32_to_f32(viewport.height),
        ),
        arcweft_layout::ScalePolicy::Raw,
    )
    .expect("validated Agent viewport dimensions produce a content rect")
}

fn agent_u32_to_f32(value: u32) -> f32 {
    value.to_string().parse().unwrap_or(f32::MAX)
}

pub(super) fn agent_action_targets_for_runtime_status(
    status: &FlowFiberStatus,
) -> Vec<AgentActionTarget> {
    let FlowFiberStatus::Choice(state) = status else {
        return Vec::new();
    };
    state
        .options
        .iter()
        .map(|option| {
            let target = option.id.as_deref().unwrap_or(option.label.as_str());
            AgentActionTarget {
                id: format!("action.select_choice.{target}"),
                target: target.to_owned(),
                action: AgentActionKind::SelectChoice,
                kind: AgentActionDispatch::Semantic,
                enabled: true,
            }
        })
        .collect()
}

pub(super) fn agent_action_targets(objects: &[AgentObservedObject]) -> Vec<AgentActionTarget> {
    objects
        .iter()
        .flat_map(agent_action_targets_for_object)
        .collect()
}

pub(super) fn agent_action_targets_for_semantics(
    semantics: &arcweft_presentation::semantic::SemanticTree,
) -> Vec<AgentActionTarget> {
    semantics
        .as_slice()
        .iter()
        .flat_map(|node| {
            node.actions().iter().map(move |action| AgentActionTarget {
                id: action.as_str().to_owned(),
                target: node.target().id().as_str().to_owned(),
                action: AgentActionKind::Invoke,
                kind: AgentActionDispatch::Semantic,
                enabled: node.visible() && node.enabled(),
            })
        })
        .collect()
}

pub(super) fn agent_action_targets_for_scroll_regions(
    frame: &PreparedFrame,
) -> Vec<AgentActionTarget> {
    frame
        .scroll_regions
        .iter()
        .map(|region| AgentActionTarget {
            id: format!("action.scroll.{}", region.id),
            target: region.id.clone(),
            action: AgentActionKind::Scroll,
            kind: AgentActionDispatch::Semantic,
            enabled: region.overflow.scroll_enabled()
                && match region.axis {
                    RenderScrollAxis::Vertical => region.max_offset_y() > f32::EPSILON,
                    RenderScrollAxis::Horizontal => region.max_offset_x() > f32::EPSILON,
                },
        })
        .collect()
}

pub(super) fn agent_observed_scroll_regions(
    frame: &PreparedFrame,
) -> Vec<AgentObservedScrollRegion> {
    frame
        .scroll_regions
        .iter()
        .map(|region| AgentObservedScrollRegion {
            target: region.id.clone(),
            role: AgentScrollRegionRole::ScrollRegion,
            parts: AgentScrollRegionParts {
                viewport: AgentScrollViewportPart {
                    internal: true,
                    bounds: [
                        finite_logical_pixel(region.bounds.x),
                        finite_logical_pixel(region.bounds.y),
                        finite_logical_pixel(region.bounds.width),
                        finite_logical_pixel(region.bounds.height),
                    ],
                },
                content: AgentScrollContentPart {
                    internal: true,
                    size: [
                        finite_logical_pixel(region.content_width),
                        finite_logical_pixel(region.content_height),
                    ],
                    offset: [
                        finite_logical_pixel(region.clamped_offset_x(region.offset_x)),
                        finite_logical_pixel(region.clamped_offset_y(region.offset_y)),
                    ],
                    max_offset: [
                        finite_logical_pixel(region.max_offset_x()),
                        finite_logical_pixel(region.max_offset_y()),
                    ],
                },
            },
            axis: match region.axis {
                RenderScrollAxis::Vertical => AgentScrollAxis::Vertical,
                RenderScrollAxis::Horizontal => AgentScrollAxis::Horizontal,
            },
            overflow: match region.overflow {
                RenderScrollOverflow::Auto => AgentScrollOverflow::Auto,
                RenderScrollOverflow::Scroll => AgentScrollOverflow::Scroll,
                RenderScrollOverflow::Hidden => AgentScrollOverflow::Hidden,
            },
            indicators: match region.indicators {
                RenderScrollIndicatorsPolicy::Auto => AgentScrollIndicatorsPolicy::Auto,
                RenderScrollIndicatorsPolicy::Visible => AgentScrollIndicatorsPolicy::Visible,
                RenderScrollIndicatorsPolicy::Hidden => AgentScrollIndicatorsPolicy::Hidden,
            },
            overscroll: match region.overscroll {
                RenderScrollOverscrollPolicy::Clamp => AgentScrollOverscrollPolicy::Clamp,
                RenderScrollOverscrollPolicy::Contain => AgentScrollOverscrollPolicy::Contain,
                RenderScrollOverscrollPolicy::Elastic => AgentScrollOverscrollPolicy::Elastic,
            },
            auto_scroll_focus: match region.auto_scroll_focus {
                RenderFocusAutoScrollPolicy::Nearest => AgentFocusAutoScrollPolicy::Nearest,
                RenderFocusAutoScrollPolicy::Start => AgentFocusAutoScrollPolicy::Start,
                RenderFocusAutoScrollPolicy::End => AgentFocusAutoScrollPolicy::End,
                RenderFocusAutoScrollPolicy::Disabled => AgentFocusAutoScrollPolicy::Disabled,
            },
        })
        .collect()
}

fn finite_logical_pixel(value: f32) -> f64 {
    if value.is_finite() {
        f64::from(value)
    } else {
        0.0
    }
}

pub(super) fn dedupe_agent_action_targets(actions: &mut Vec<AgentActionTarget>) {
    let mut seen = BTreeSet::new();
    actions.retain(|action| {
        seen.insert((
            action.id.clone(),
            action.target.clone(),
            action.action,
            action.kind,
        ))
    });
}

pub(super) fn agent_action_targets_for_object(
    object: &AgentObservedObject,
) -> Vec<AgentActionTarget> {
    match &object.content {
        AgentObservedObjectContent::RichText { .. } if agent_is_dialogue_textbox(object) => {
            vec![AgentActionTarget {
                id: format!("action.advance_text.{}", object.id),
                target: object.id.clone(),
                action: AgentActionKind::AdvanceText,
                kind: AgentActionDispatch::Semantic,
                enabled: object.visible && object.enabled,
            }]
        }
        AgentObservedObjectContent::Image(content) => content
            .actions
            .iter()
            .map(|action| AgentActionTarget {
                id: action.clone(),
                target: content
                    .target
                    .clone()
                    .or_else(|| content.object.clone())
                    .unwrap_or_else(|| object.id.clone()),
                action: AgentActionKind::Invoke,
                kind: AgentActionDispatch::Semantic,
                enabled: object.visible && object.enabled,
            })
            .collect(),
        AgentObservedObjectContent::RichText { .. } | AgentObservedObjectContent::Custom { .. } => {
            Vec::new()
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct AgentImageOutput {
    pub(super) uri: String,
    pub(super) bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(super) struct AgentRasterCapture {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) crop_origin: Option<AgentImageCropOrigin>,
    pub(super) composition: AgentImageComposition,
    pub(super) rgba: Vec<u8>,
    pub(super) content_bbox: Option<AgentImageContentBBox>,
    pub(super) content_pixels: u64,
    pub(super) diagnostics: Vec<AgentDiagnostic>,
}

pub(super) fn agent_observe_image_output(
    report: &mut AgentObservationReport,
    options: &AgentObserveOptions,
    image_frames: &AgentImageFrameStore,
) -> Result<Option<AgentImageOutput>, ExitCode> {
    let Some(image) = options.image else {
        return Ok(None);
    };
    match image {
        AgentObserveImageKind::Overlay => {
            let overlay_svg = {
                let selected = select_agent_capture_objects(&report.objects, options)?;
                agent_overlay_svg(&report.viewport, &selected)
            };
            let hash = hash_hex(overlay_svg.as_bytes());
            report.render_hash.clone_from(&hash);
            let uri = agent_capture_uri(report, "overlay", "svg", options);
            let scope = agent_capture_scope_for_options(options);
            report.images = vec![AgentImageResource {
                kind: AgentImageKind::OverlaySvg,
                renderer: AgentImageRenderer::Native,
                scope: agent_image_scope_for_capture_scope(&scope),
                composition: AgentImageComposition::OverlayVector,
                page: 0,
                capture_step: report.steps,
                capture_time_millis: agent_capture_time_millis(agent_observe_capture_time_seconds(
                    options,
                )),
                uri: uri.clone(),
                mime_type: "image/svg+xml".to_owned(),
                width: report.viewport.width,
                height: report.viewport.height,
                hash,
                crop_origin: None,
                content_bbox: None,
                content_viewport_bbox: None,
                content_pixels: None,
                object: agent_image_object_for_capture_scope(report, &scope),
                view: agent_image_view_for_capture_scope(report, &scope),
                selected_capture: None,
                diagnostics: Vec::new(),
                written: options.out.as_deref().map(report_path),
            }];
            report.overlay_svg = Some(overlay_svg.clone());
            Ok(Some(AgentImageOutput {
                uri,
                bytes: overlay_svg.into_bytes(),
            }))
        }
        AgentObserveImageKind::RawRgba | AgentObserveImageKind::Png => {
            let request = agent_capture_request_for_options(report, image, options);
            let capture_result = agent_capture_image(report, &request, image_frames)?;
            report
                .diagnostics
                .extend(capture_result.image.diagnostics.clone());
            let (mut image, bytes) = (capture_result.image, capture_result.bytes);
            image.written = options.out.as_deref().map(report_path);
            report.render_hash.clone_from(&image.hash);
            let uri = image.uri.clone();
            report.images = vec![image];
            Ok(Some(AgentImageOutput { uri, bytes }))
        }
    }
}

pub(super) fn agent_capture_request_for_options(
    report: &AgentObservationReport,
    image_kind: AgentObserveImageKind,
    options: &AgentObserveOptions,
) -> AgentCaptureReadRequest {
    let capture_kind = agent_capture_kind(options);
    let extension = match image_kind {
        AgentObserveImageKind::Png => "png",
        AgentObserveImageKind::RawRgba => "rgba",
        AgentObserveImageKind::Overlay => "svg",
    };
    AgentCaptureReadRequest {
        uri: agent_capture_uri(report, capture_kind.resource_name(), extension, options),
        image_kind,
        capture_kind,
        scope: agent_capture_scope_for_options(options),
        page: options.page.unwrap_or(0),
        capture_step: report.steps,
        capture_time_seconds: agent_observe_capture_time_seconds(options),
    }
}

pub(super) fn agent_capture_scope_for_options(options: &AgentObserveOptions) -> AgentCaptureScope {
    if let Some(view_id) = &options.view {
        AgentCaptureScope::View(view_id.clone())
    } else if let Some(object_id) = &options.object {
        AgentCaptureScope::Object(object_id.clone())
    } else if let Some(layer) = &options.layer {
        AgentCaptureScope::Layer(layer.clone())
    } else {
        AgentCaptureScope::Viewport
    }
}

pub(super) fn agent_image_scope_for_capture_scope(scope: &AgentCaptureScope) -> AgentImageScope {
    match scope {
        AgentCaptureScope::Viewport => AgentImageScope::Viewport,
        AgentCaptureScope::View(id) => AgentImageScope::View { id: id.clone() },
        AgentCaptureScope::Layer(id) => AgentImageScope::Layer { id: id.clone() },
        AgentCaptureScope::Object(id) => AgentImageScope::Object { id: id.clone() },
    }
}

pub(super) fn select_agent_capture_objects<'a>(
    objects: &'a [AgentObservedObject],
    options: &AgentObserveOptions,
) -> Result<Vec<&'a AgentObservedObject>, ExitCode> {
    if let Some(view_id) = &options.view {
        let selected = objects
            .iter()
            .filter(|object| agent_view_id_for_object(object) == *view_id)
            .collect::<Vec<_>>();
        if selected.is_empty() {
            eprintln!("error: no observed object matches --view {view_id}");
            return Err(ExitCode::from(2));
        }
        return Ok(selected);
    }
    if let Some(object_id) = &options.object {
        let Some(object) = objects.iter().find(|object| object.id == *object_id) else {
            eprintln!("error: no observed object matches --object {object_id}");
            return Err(ExitCode::from(2));
        };
        return Ok(vec![object]);
    }
    if let Some(layer) = &options.layer {
        let selected = objects
            .iter()
            .filter(|object| agent_object_matches_layer(object, layer))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            eprintln!("error: no observed object matches --layer {layer}");
            return Err(ExitCode::from(2));
        }
        return Ok(selected);
    }
    Ok(objects.iter().collect())
}

pub(super) fn agent_capture_kind(options: &AgentObserveOptions) -> AgentObserveCaptureKind {
    options.capture.unwrap_or(AgentObserveCaptureKind::Color)
}

pub(super) fn agent_image_kind(capture: AgentObserveCaptureKind) -> AgentImageKind {
    match capture {
        AgentObserveCaptureKind::Color => AgentImageKind::Color,
        AgentObserveCaptureKind::ObjectId => AgentImageKind::ObjectId,
        AgentObserveCaptureKind::Mask => AgentImageKind::Mask,
    }
}

#[cfg(test)]
mod scroll_observation_tests {
    use super::{
        agent_action_targets_for_scroll_regions, agent_observed_scroll_regions,
        agent_observed_virtual_lists,
    };
    use crate::app::agent::native::observe::dispatch_native_agent_scroll;
    use arcweft_agent_protocol::{
        action::AgentActionKind,
        protocol::AgentScrollAction,
        view::{AgentScrollAxis, AgentScrollOverscrollPolicy},
    };
    use arcweft_id::PublicId;
    use arcweft_player_scene::input::InputController;
    use arcweft_presentation::hit::HitRect;
    use arcweft_render_wgpu::geometry::{
        ChoiceScroll, InteractionVisualState, RenderFocusAutoScrollPolicy, RenderPreferences,
        RenderScene, RenderScrollAxis, RenderScrollIndicatorsPolicy, RenderScrollOverflow,
        RenderScrollOverscrollPolicy, RenderScrollRegion, RenderViewport, SharedFramePlanContext,
    };
    use arcweft_view::program::{ViewStableKey, ViewVirtualAxis};
    use arcweft_view::virtualization::{
        ViewVirtualItem, ViewVirtualScrollTarget, ViewVirtualizationRuntime,
    };

    fn assert_exact_geometry<const N: usize>(actual: [f64; N], expected: [f64; N]) {
        assert_eq!(actual.map(f64::to_bits), expected.map(f64::to_bits));
    }

    #[test]
    fn authored_scroll_is_one_action_target_with_internal_parts_metadata() {
        let frame = SharedFramePlanContext::new()
            .prepare(&RenderScene {
                content_avoidance_regions: Vec::new(),
                choices: Vec::new(),
                text_inputs: Vec::new(),
                action_buttons: Vec::new(),
                focus_groups: Vec::new(),
                focus_navigation: Vec::new(),
                images: Vec::new(),
                viewport: RenderViewport {
                    logical_width: 1280.0,
                    logical_height: 720.0,
                    physical_width: 1280,
                    physical_height: 720,
                    scale_factor: 1.0,
                },
                visual_time_millis: 0,
                preferences: RenderPreferences::default(),
                interaction: InteractionVisualState::default(),
                choice_scroll: ChoiceScroll::default(),
                scroll_regions: vec![RenderScrollRegion {
                    id: "scroll.Inventory.0".to_owned(),
                    bounds: HitRect::new(48.0, 48.0, 420.0, 180.0),
                    content_width: 420.0,
                    content_height: 960.0,
                    offset_x: 0.0,
                    offset_y: 240.0,
                    overscroll_x: 0.0,
                    overscroll_y: 0.0,
                    axis: RenderScrollAxis::Vertical,
                    overflow: RenderScrollOverflow::Auto,
                    indicators: RenderScrollIndicatorsPolicy::Auto,
                    overscroll: RenderScrollOverscrollPolicy::Contain,
                    auto_scroll_focus: RenderFocusAutoScrollPolicy::Nearest,
                    indicator_activity_millis: None,
                }],
            })
            .expect("scroll frame plans");

        let actions = agent_action_targets_for_scroll_regions(&frame);
        let observed = agent_observed_scroll_regions(&frame);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "action.scroll.scroll.Inventory.0");
        assert_eq!(actions[0].target, "scroll.Inventory.0");
        assert_eq!(actions[0].action, AgentActionKind::Scroll);
        assert!(actions[0].enabled);
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].target, actions[0].target);
        assert_eq!(observed[0].axis, AgentScrollAxis::Vertical);
        assert_eq!(observed[0].overscroll, AgentScrollOverscrollPolicy::Contain);
        assert!(observed[0].parts.viewport.internal);
        assert!(observed[0].parts.content.internal);
        assert_exact_geometry(
            observed[0].parts.viewport.bounds,
            [48.0, 48.0, 420.0, 180.0],
        );
        assert_exact_geometry(observed[0].parts.content.size, [420.0, 960.0]);
        assert_exact_geometry(observed[0].parts.content.offset, [0.0, 240.0]);
        assert_exact_geometry(observed[0].parts.content.max_offset, [0.0, 780.0]);

        let mut input = InputController::default();
        assert!(
            dispatch_native_agent_scroll(
                &mut input,
                &frame,
                &actions,
                &AgentScrollAction {
                    region: "scroll.Inventory.0".to_owned(),
                    delta_x_milli: 0,
                    delta_y_milli: -90_000,
                },
            )
            .expect("observed enabled scroll action dispatches")
        );
        assert!((input.scroll_offset_y("scroll.Inventory.0") - 330.0).abs() < f32::EPSILON);
        assert!(
            dispatch_native_agent_scroll(
                &mut input,
                &frame,
                &[],
                &AgentScrollAction {
                    region: "scroll.Inventory.0".to_owned(),
                    delta_x_milli: 0,
                    delta_y_milli: -1_000,
                },
            )
            .is_err()
        );
    }

    #[test]
    fn virtual_list_observation_keeps_off_window_stable_targets() {
        let mut runtime = ViewVirtualizationRuntime::default();
        let mount = runtime
            .mount(
                ViewVirtualScrollTarget::from(PublicId::try_new("scroll.inventory").unwrap()),
                ViewVirtualAxis::Vertical,
                100,
                vec![
                    ViewVirtualItem::new(ViewStableKey(10), 60),
                    ViewVirtualItem::new(ViewStableKey(11), 60),
                    ViewVirtualItem::new(ViewStableKey(12), 60),
                    ViewVirtualItem::new(ViewStableKey(13), 60),
                ],
            )
            .expect("finite list validates");
        runtime.get_mut(mount).unwrap().scroll_to_milli(70);

        let observed = agent_observed_virtual_lists(&runtime.range_tables());
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].target, "view.mount.0");
        assert_eq!(observed[0].scroll_target, "scroll.inventory");
        assert_eq!(observed[0].materialized_start, 1);
        assert_eq!(observed[0].materialized_end, 3);
        assert_eq!(observed[0].items[0].target, "view.mount.0.item.10");
        assert!(!observed[0].items[0].materialized);
        assert!(observed[0].items[1].materialized);
        assert!(!observed[0].items[3].materialized);
    }
}
