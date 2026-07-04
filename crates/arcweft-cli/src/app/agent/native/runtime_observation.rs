use super::*;

pub(super) fn agent_observe_layout_scene_graph(viewport: &AgentViewport) -> serde_json::Value {
    let content_rect = agent_observe_content_rect(viewport);
    let metadata = content_rect.fit_transform_metadata(
        arcweft_layout::LayoutCoordinateSpace::Output,
        arcweft_layout::LayoutCoordinateSpace::Output,
    );
    serde_json::json!({
        "kind": "layout.viewport_scale",
        "renderer_kind": "native_rich_text_observer",
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
    pub(super) background: [u8; 4],
    pub(super) rgba: Vec<u8>,
    pub(super) diagnostics: Vec<arcweft_render_native::NativeVisualDiagnostic>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct AgentRasterContentStats {
    pub(super) bbox: Option<AgentImageContentBBox>,
    pub(super) content_pixels: u64,
}

impl AgentRasterCapture {
    pub(super) fn new(
        width: u32,
        height: u32,
        color: [u8; 4],
        composition: AgentImageComposition,
    ) -> Self {
        let pixel_count = usize::try_from(width)
            .unwrap_or(0)
            .saturating_mul(usize::try_from(height).unwrap_or(0));
        let mut rgba = Vec::with_capacity(pixel_count.saturating_mul(4));
        for _ in 0..pixel_count {
            rgba.extend_from_slice(&color);
        }
        Self {
            width,
            height,
            crop_origin: None,
            composition,
            background: color,
            rgba,
            diagnostics: Vec::new(),
        }
    }

    pub(super) fn content_stats(&self) -> AgentRasterContentStats {
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0;
        let mut max_y = 0;
        let mut count = 0_u64;
        for y in 0..self.height {
            for x in 0..self.width {
                let index = usize::try_from(y)
                    .unwrap_or(0)
                    .saturating_mul(usize::try_from(self.width).unwrap_or(0))
                    .saturating_add(usize::try_from(x).unwrap_or(0))
                    .saturating_mul(4)
                    .saturating_add(3);
                let Some(pixel) = self
                    .rgba
                    .get(index.saturating_sub(3)..index.saturating_add(1))
                else {
                    continue;
                };
                if pixel == self.background {
                    continue;
                }
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                count = count.saturating_add(1);
            }
        }
        AgentRasterContentStats {
            bbox: (count > 0).then_some(AgentImageContentBBox {
                x: min_x,
                y: min_y,
                width: max_x.saturating_sub(min_x).saturating_add(1),
                height: max_y.saturating_sub(min_y).saturating_add(1),
            }),
            content_pixels: count,
        }
    }
}

pub(super) fn agent_observe_image_output(
    report: &mut AgentObservationReport,
    options: &AgentObserveOptions,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
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
                component: agent_image_component_for_capture_scope(report, &scope),
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
            let capture_result = match native_session {
                Some(native_session) => agent_native_capture_image_with_frame_store(
                    report,
                    &request,
                    native_session,
                    image_frames,
                )?,
                None => agent_native_capture_image(report, &request)?,
            };
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

pub(super) fn agent_native_visual_diagnostics(
    step: usize,
    diagnostics: &[arcweft_render_native::NativeVisualDiagnostic],
) -> Vec<AgentDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| AgentDiagnostic {
            step,
            severity: match diagnostic.severity {
                arcweft_render_native::NativeVisualDiagnosticSeverity::Error => {
                    AgentDiagnosticSeverity::Error
                }
                arcweft_render_native::NativeVisualDiagnosticSeverity::Warning => {
                    AgentDiagnosticSeverity::Warning
                }
                arcweft_render_native::NativeVisualDiagnosticSeverity::Info => {
                    AgentDiagnosticSeverity::Info
                }
            },
            source: Some("native_rich_text".to_owned()),
            code: Some(diagnostic.code.clone()),
            effect_id: diagnostic.effect_id.clone(),
            message: format!(
                "native rich-text {}: {}",
                diagnostic.code, diagnostic.message
            ),
        })
        .collect()
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
    if let Some(component_id) = &options.component {
        AgentCaptureScope::Component(component_id.clone())
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
        AgentCaptureScope::Component(id) => AgentImageScope::Component { id: id.clone() },
        AgentCaptureScope::Layer(id) => AgentImageScope::Layer { id: id.clone() },
        AgentCaptureScope::Object(id) => AgentImageScope::Object { id: id.clone() },
    }
}

pub(super) fn select_agent_capture_objects<'a>(
    objects: &'a [AgentObservedObject],
    options: &AgentObserveOptions,
) -> Result<Vec<&'a AgentObservedObject>, ExitCode> {
    if let Some(component_id) = &options.component {
        let selected = objects
            .iter()
            .filter(|object| agent_component_id_for_object(object) == *component_id)
            .collect::<Vec<_>>();
        if selected.is_empty() {
            eprintln!("error: no observed object matches --component {component_id}");
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
