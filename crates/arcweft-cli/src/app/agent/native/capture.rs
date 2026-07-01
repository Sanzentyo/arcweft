use super::*;

#[derive(Clone, Debug)]
pub(super) struct AgentCaptureReadRequest {
    pub(super) uri: String,
    pub(super) image_kind: AgentObserveImageKind,
    pub(super) capture_kind: AgentObserveCaptureKind,
    pub(super) scope: AgentCaptureScope,
    pub(super) page: usize,
    pub(super) capture_step: usize,
    pub(super) capture_time_seconds: f32,
}

#[derive(Clone, Debug)]
pub(super) enum AgentCaptureScope {
    Viewport,
    Layer(String),
    Object(String),
}

pub(super) fn agent_capture_request_from_uri(
    report: &AgentObservationReport,
    uri: &str,
) -> Option<AgentCaptureReadRequest> {
    let (uri_without_query, page) = agent_capture_uri_query(uri)?;
    let prefix = format!(
        "arcweft://session/{}/frame/{}/",
        report.session_id, report.tick
    );
    let name = uri_without_query.strip_prefix(&prefix)?;
    let (stem, extension) = name.rsplit_once('.')?;
    let image_kind = match extension {
        "png" => AgentObserveImageKind::Png,
        "rgba" => AgentObserveImageKind::RawRgba,
        _ => return None,
    };
    let (capture_stem, capture_kind) = if let Some(base) = stem.strip_suffix(".object-id") {
        (base, AgentObserveCaptureKind::ObjectId)
    } else if let Some(base) = stem.strip_suffix(".mask") {
        (base, AgentObserveCaptureKind::Mask)
    } else if stem == "object-id" {
        ("", AgentObserveCaptureKind::ObjectId)
    } else if stem == "mask" {
        ("", AgentObserveCaptureKind::Mask)
    } else {
        (stem, AgentObserveCaptureKind::Color)
    };
    let scope = if capture_stem.is_empty() || capture_stem == "color" {
        AgentCaptureScope::Viewport
    } else if let Some(layer) = capture_stem.strip_prefix("layer.") {
        AgentCaptureScope::Layer(layer.to_owned())
    } else if let Some(object) = capture_stem.strip_prefix("object.") {
        AgentCaptureScope::Object(object.to_owned())
    } else {
        return None;
    };
    Some(AgentCaptureReadRequest {
        uri: uri.to_owned(),
        image_kind,
        capture_kind,
        scope,
        page,
        capture_step: report.steps,
        capture_time_seconds: agent_report_capture_time_seconds(report),
    })
}

pub(super) fn agent_capture_uri_query(uri: &str) -> Option<(&str, usize)> {
    let Some((base, query)) = uri.split_once('?') else {
        return Some((uri, 0));
    };
    let mut page = 0;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=')?;
        match key {
            "page" => {
                page = value.parse::<usize>().ok()?;
            }
            _ => return None,
        }
    }
    Some((base, page))
}

pub(super) fn agent_observe_capture_resource(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
) -> Result<AgentResource, ExitCode> {
    agent_native_capture_resource(report, request)
}

pub(super) fn agent_native_capture_resource(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
) -> Result<AgentResource, ExitCode> {
    let result = agent_native_capture_image(report, request)?;
    Ok(report.image_resource(&result.image, &result.bytes))
}

pub(super) fn agent_native_capture_resource_with_session_and_frame_store(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
    image_frames: &AgentImageFrameStore,
) -> Result<AgentResource, ExitCode> {
    let result =
        agent_native_capture_image_with_frame_store(report, request, native_session, image_frames)?;
    Ok(report.image_resource(&result.image, &result.bytes))
}

pub(super) struct AgentNativeCaptureImageResult {
    pub(super) image: AgentImageResource,
    pub(super) bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct AgentImageFrameStore {
    pub(super) full_frame: Option<AgentStoredImageFrame>,
    pub(super) frames_by_object: BTreeMap<String, AgentStoredImageFrame>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct AgentUiImageObservation {
    pub(super) objects: Vec<AgentObservedObject>,
    pub(super) image_frames: AgentImageFrameStore,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct AgentStoredImageFrame {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) rgba: Vec<u8>,
    pub(super) placement: Option<AgentStoredImagePlacement>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AgentStoredImagePlacement {
    pub(super) dst: arcweft_render_native::NativeImageRect,
    pub(super) transform: arcweft_render_native::NativeImageTransform,
    pub(super) opacity_milli: u16,
}

impl AgentImageFrameStore {
    pub(super) fn set_full_frame(&mut self, width: u32, height: u32, rgba: Vec<u8>) {
        self.full_frame = Some(AgentStoredImageFrame {
            width,
            height,
            rgba,
            placement: None,
        });
    }

    pub(super) const fn full_frame(&self) -> Option<&AgentStoredImageFrame> {
        self.full_frame.as_ref()
    }

    #[cfg(test)]
    pub(super) fn insert(
        &mut self,
        object_id: impl Into<String>,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) {
        self.insert_with_placement(object_id, width, height, rgba, None);
    }

    #[cfg(test)]
    pub(super) fn insert_with_placement(
        &mut self,
        object_id: impl Into<String>,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        placement: Option<AgentStoredImagePlacement>,
    ) {
        self.frames_by_object.insert(
            object_id.into(),
            AgentStoredImageFrame {
                width,
                height,
                rgba,
                placement,
            },
        );
    }

    pub(super) fn get(&self, object_id: &str) -> Option<&AgentStoredImageFrame> {
        self.frames_by_object.get(object_id)
    }
}

pub(super) fn agent_native_capture_image(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
) -> Result<AgentNativeCaptureImageResult, ExitCode> {
    let mut native_session =
        arcweft_render_native::NativeOffscreenCaptureSession::new().map_err(|error| {
            eprintln!("error: native capture failed: {error}");
            ExitCode::FAILURE
        })?;
    agent_native_capture_image_with_session(report, request, &mut native_session)
}

pub(super) fn agent_native_capture_image_with_session(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
) -> Result<AgentNativeCaptureImageResult, ExitCode> {
    agent_native_capture_image_with_frame_store(
        report,
        request,
        native_session,
        &AgentImageFrameStore::default(),
    )
}

pub(super) fn agent_native_capture_image_with_frame_store(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
    image_frames: &AgentImageFrameStore,
) -> Result<AgentNativeCaptureImageResult, ExitCode> {
    if let Some(result) = agent_native_shared_frame_capture(report, request, image_frames)? {
        return Ok(result);
    }
    if let Some(result) =
        agent_native_image_layer_frame_capture(report, request, native_session, image_frames)?
    {
        return Ok(result);
    }
    if let Some(result) =
        agent_native_image_object_frame_capture(report, request, native_session, image_frames)?
    {
        return Ok(result);
    }
    if let Some(result) = agent_native_image_object_geometry_capture(report, request)? {
        return Ok(result);
    }
    let Some(textbox) = agent_native_textbox_for_capture(report, &request.scope) else {
        if let Some(result) = agent_native_image_viewport_frame_capture(
            report,
            request,
            native_session,
            image_frames,
        )? {
            return Ok(result);
        }
        eprintln!("error: native renderer requires an observed textbox or image frame");
        return Err(ExitCode::from(2));
    };
    let (left, top) = agent_native_text_origin(textbox);
    let capture = native_session
        .capture_frame_rgba_in(
            agent_observed_rich_text(textbox),
            arcweft_render_native::NativeCaptureViewport::new(
                report.viewport.width,
                report.viewport.height,
                left,
                top,
                request.page,
            )
            .with_time_seconds(request.capture_time_seconds),
        )
        .map_err(|error| {
            eprintln!("error: native capture failed: {error}");
            ExitCode::FAILURE
        })?;
    let capture = agent_native_scoped_capture(
        &capture,
        AgentNativeCaptureContext {
            frame: agent_observed_rich_text(textbox),
            left,
            top,
            objects: &report.objects,
            page_index: request.page,
            capture_time_seconds: request.capture_time_seconds,
        },
        &request.scope,
        request.capture_kind,
        Some(native_session),
    )?;
    agent_native_capture_result_from_raster(report, request, &capture)
}

fn agent_native_shared_frame_capture(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    image_frames: &AgentImageFrameStore,
) -> Result<Option<AgentNativeCaptureImageResult>, ExitCode> {
    let Some(frame) = image_frames.full_frame() else {
        return Ok(None);
    };
    let capture = match request.capture_kind {
        AgentObserveCaptureKind::Color => agent_shared_color_frame_capture(report, request, frame)?,
        AgentObserveCaptureKind::ObjectId | AgentObserveCaptureKind::Mask => {
            agent_shared_debug_frame_capture(report, request)?
        }
    };
    agent_native_capture_result_from_raster(report, request, &capture).map(Some)
}

fn agent_shared_color_frame_capture(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    frame: &AgentStoredImageFrame,
) -> Result<AgentRasterCapture, ExitCode> {
    let full = AgentRasterCapture {
        width: frame.width,
        height: frame.height,
        crop_origin: None,
        composition: AgentImageComposition::Framebuffer,
        background: [0, 0, 0, 0],
        rgba: frame.rgba.clone(),
        diagnostics: Vec::new(),
    };
    if matches!(request.scope, AgentCaptureScope::Viewport) {
        return Ok(full);
    }
    let bbox = agent_capture_scope_bbox(report, &request.scope).ok_or_else(|| {
        agent_report_missing_capture_scope(&request.scope);
        ExitCode::from(2)
    })?;
    let (x, y, width, height) = agent_clamped_bbox_rect(
        report.viewport.width.max(1),
        report.viewport.height.max(1),
        bbox.x,
        bbox.y,
        bbox.width,
        bbox.height,
    );
    Ok(agent_crop_raster_capture(&full, x, y, width, height))
}

fn agent_shared_debug_frame_capture(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
) -> Result<AgentRasterCapture, ExitCode> {
    let composition = match request.capture_kind {
        AgentObserveCaptureKind::ObjectId => AgentImageComposition::ObjectIdAttachment,
        AgentObserveCaptureKind::Mask => AgentImageComposition::MaskAttachment,
        AgentObserveCaptureKind::Color => unreachable!("color handled by shared color capture"),
    };
    let mut full = AgentRasterCapture::new(
        report.viewport.width.max(1),
        report.viewport.height.max(1),
        [0, 0, 0, 0],
        composition,
    );
    let selected = agent_shared_capture_objects(report, &request.scope)?;
    for object in selected {
        let color = match request.capture_kind {
            AgentObserveCaptureKind::ObjectId => agent_object_id_color(&object.id),
            AgentObserveCaptureKind::Mask => [255, 255, 255, 255],
            AgentObserveCaptureKind::Color => unreachable!("color handled by shared color capture"),
        };
        let (x, y, width, height) = agent_clamped_bbox_rect(
            report.viewport.width.max(1),
            report.viewport.height.max(1),
            object.bbox.x,
            object.bbox.y,
            object.bbox.width,
            object.bbox.height,
        );
        agent_fill_raster_rect(&mut full, x, y, width, height, color);
    }
    if matches!(request.scope, AgentCaptureScope::Viewport) {
        return Ok(full);
    }
    let bbox = agent_capture_scope_bbox(report, &request.scope).ok_or_else(|| {
        agent_report_missing_capture_scope(&request.scope);
        ExitCode::from(2)
    })?;
    let (x, y, width, height) = agent_clamped_bbox_rect(
        report.viewport.width.max(1),
        report.viewport.height.max(1),
        bbox.x,
        bbox.y,
        bbox.width,
        bbox.height,
    );
    Ok(agent_crop_raster_capture(&full, x, y, width, height))
}

fn agent_shared_capture_objects<'a>(
    report: &'a AgentObservationReport,
    scope: &AgentCaptureScope,
) -> Result<Vec<&'a AgentObservedObject>, ExitCode> {
    match scope {
        AgentCaptureScope::Viewport => Ok(report
            .objects
            .iter()
            .filter(|object| object.visible)
            .collect()),
        AgentCaptureScope::Layer(layer) => {
            let selected = report
                .objects
                .iter()
                .filter(|object| object.visible && agent_object_matches_layer(object, layer))
                .collect::<Vec<_>>();
            if selected.is_empty() {
                agent_report_missing_capture_scope(scope);
                return Err(ExitCode::from(2));
            }
            Ok(selected)
        }
        AgentCaptureScope::Object(object_id) => report
            .objects
            .iter()
            .find(|object| object.visible && object.id == *object_id)
            .map(|object| vec![object])
            .ok_or_else(|| {
                agent_report_missing_capture_scope(scope);
                ExitCode::from(2)
            }),
    }
}

fn agent_report_missing_capture_scope(scope: &AgentCaptureScope) {
    match scope {
        AgentCaptureScope::Viewport => {
            eprintln!("error: no observed viewport is available for capture");
        }
        AgentCaptureScope::Layer(layer) => {
            eprintln!("error: no observed object matches resource layer {layer}");
        }
        AgentCaptureScope::Object(object_id) => {
            eprintln!("error: no observed object matches resource object {object_id}");
        }
    }
}

pub(super) fn agent_native_capture_result_from_raster(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    capture: &AgentRasterCapture,
) -> Result<AgentNativeCaptureImageResult, ExitCode> {
    let (mime_type, bytes) = match request.image_kind {
        AgentObserveImageKind::Png => ("image/png", agent_encode_png(capture)?),
        AgentObserveImageKind::RawRgba => ("application/octet-stream", capture.rgba.clone()),
        AgentObserveImageKind::Overlay => unreachable!("overlay is not a raster capture"),
    };
    let stats = capture.content_stats();
    let content_viewport_bbox = agent_content_viewport_bbox(capture.crop_origin, stats.bbox);
    let selected_capture =
        agent_selected_capture_metadata_from_raster(report, request, capture, stats.bbox);
    let image = AgentImageResource {
        kind: agent_image_kind(request.capture_kind),
        renderer: AgentImageRenderer::Native,
        scope: agent_image_scope_for_capture_scope(&request.scope),
        composition: capture.composition,
        page: request.page,
        capture_step: request.capture_step,
        capture_time_millis: agent_capture_time_millis(request.capture_time_seconds),
        uri: request.uri.clone(),
        mime_type: mime_type.to_owned(),
        width: capture.width,
        height: capture.height,
        hash: hash_hex(&bytes),
        crop_origin: capture.crop_origin,
        content_bbox: stats.bbox,
        content_viewport_bbox,
        content_pixels: Some(stats.content_pixels),
        object: agent_image_object_for_capture_scope(report, &request.scope),
        selected_capture,
        diagnostics: agent_native_visual_diagnostics(request.capture_step, &capture.diagnostics),
        written: None,
    };
    Ok(AgentNativeCaptureImageResult { image, bytes })
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn agent_native_image_layer_frame_capture(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
    image_frames: &AgentImageFrameStore,
) -> Result<Option<AgentNativeCaptureImageResult>, ExitCode> {
    let AgentCaptureScope::Layer(layer) = &request.scope else {
        return Ok(None);
    };
    let image_items = report
        .objects
        .iter()
        .filter(|object| {
            object.visible
                && agent_object_matches_layer(object, layer)
                && matches!(object.content, AgentObservedObjectContent::Image(_))
        })
        .filter_map(|object| image_frames.get(&object.id).map(|frame| (object, frame)))
        .collect::<Vec<_>>();
    if image_items.is_empty() {
        return Ok(None);
    }

    let viewport_width = report.viewport.width.max(1);
    let viewport_height = report.viewport.height.max(1);
    let crop = image_items
        .iter()
        .map(|(object, _)| {
            agent_clamped_bbox_rect(
                viewport_width,
                viewport_height,
                object.bbox.x,
                object.bbox.y,
                object.bbox.width,
                object.bbox.height,
            )
        })
        .reduce(|left, right| agent_union_rect(left, right, viewport_width, viewport_height))
        .expect("non-empty image layer capture has crop rect");
    let capture = match request.capture_kind {
        AgentObserveCaptureKind::Color => {
            let quads = image_items
                .iter()
                .map(|(object, frame)| agent_native_image_quad(object, frame))
                .collect::<Vec<_>>();
            native_session.capture_image_quads_rgba(&quads, viewport_width, viewport_height)
        }
        AgentObserveCaptureKind::ObjectId | AgentObserveCaptureKind::Mask => {
            let quads = image_items
                .iter()
                .map(|(object, frame)| {
                    let color = match request.capture_kind {
                        AgentObserveCaptureKind::ObjectId => agent_object_id_color(&object.id),
                        AgentObserveCaptureKind::Mask => [255, 255, 255, 255],
                        AgentObserveCaptureKind::Color => unreachable!("handled above"),
                    };
                    arcweft_render_native::NativeImageDebugQuad {
                        quad: agent_native_image_quad(object, frame),
                        color,
                    }
                })
                .collect::<Vec<_>>();
            native_session.capture_image_debug_quads_rgba(&quads, viewport_width, viewport_height)
        }
    }
    .map_err(|error| {
        eprintln!("error: native image layer capture failed: {error}");
        ExitCode::FAILURE
    })?;
    let raster = AgentRasterCapture {
        width: capture.width,
        height: capture.height,
        crop_origin: None,
        composition: match request.capture_kind {
            AgentObserveCaptureKind::Color => AgentImageComposition::Framebuffer,
            AgentObserveCaptureKind::ObjectId => AgentImageComposition::ObjectIdAttachment,
            AgentObserveCaptureKind::Mask => AgentImageComposition::MaskAttachment,
        },
        background: [0, 0, 0, 0],
        rgba: capture.rgba,
        diagnostics: capture.diagnostics,
    };
    let capture = agent_crop_raster_capture(&raster, crop.0, crop.1, crop.2, crop.3);
    agent_native_capture_result_from_raster(report, request, &capture).map(Some)
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn agent_native_image_viewport_frame_capture(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
    image_frames: &AgentImageFrameStore,
) -> Result<Option<AgentNativeCaptureImageResult>, ExitCode> {
    let AgentCaptureScope::Viewport = &request.scope else {
        return Ok(None);
    };
    let image_items = report
        .objects
        .iter()
        .filter(|object| {
            object.visible && matches!(object.content, AgentObservedObjectContent::Image(_))
        })
        .filter_map(|object| image_frames.get(&object.id).map(|frame| (object, frame)))
        .collect::<Vec<_>>();
    if image_items.is_empty() {
        return Ok(None);
    }

    let viewport_width = report.viewport.width.max(1);
    let viewport_height = report.viewport.height.max(1);
    let capture = match request.capture_kind {
        AgentObserveCaptureKind::Color => {
            let quads = image_items
                .iter()
                .map(|(object, frame)| agent_native_image_quad(object, frame))
                .collect::<Vec<_>>();
            native_session.capture_image_quads_rgba(&quads, viewport_width, viewport_height)
        }
        AgentObserveCaptureKind::ObjectId | AgentObserveCaptureKind::Mask => {
            let quads = image_items
                .iter()
                .map(|(object, frame)| {
                    let color = match request.capture_kind {
                        AgentObserveCaptureKind::ObjectId => agent_object_id_color(&object.id),
                        AgentObserveCaptureKind::Mask => [255, 255, 255, 255],
                        AgentObserveCaptureKind::Color => unreachable!("handled above"),
                    };
                    arcweft_render_native::NativeImageDebugQuad {
                        quad: agent_native_image_quad(object, frame),
                        color,
                    }
                })
                .collect::<Vec<_>>();
            native_session.capture_image_debug_quads_rgba(&quads, viewport_width, viewport_height)
        }
    }
    .map_err(|error| {
        eprintln!("error: native image viewport capture failed: {error}");
        ExitCode::FAILURE
    })?;
    let raster = AgentRasterCapture {
        width: capture.width,
        height: capture.height,
        crop_origin: None,
        composition: match request.capture_kind {
            AgentObserveCaptureKind::Color => AgentImageComposition::Framebuffer,
            AgentObserveCaptureKind::ObjectId => AgentImageComposition::ObjectIdAttachment,
            AgentObserveCaptureKind::Mask => AgentImageComposition::MaskAttachment,
        },
        background: [0, 0, 0, 0],
        rgba: capture.rgba,
        diagnostics: capture.diagnostics,
    };
    agent_native_capture_result_from_raster(report, request, &raster).map(Some)
}

pub(super) fn agent_native_image_object_frame_capture(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
    image_frames: &AgentImageFrameStore,
) -> Result<Option<AgentNativeCaptureImageResult>, ExitCode> {
    let AgentCaptureScope::Object(object_id) = &request.scope else {
        return Ok(None);
    };
    let Some(object) = report.objects.iter().find(|object| object.id == *object_id) else {
        return Ok(None);
    };
    if !matches!(object.content, AgentObservedObjectContent::Image(_)) {
        return Ok(None);
    }
    let Some(frame) = image_frames.get(&object.id) else {
        return Ok(None);
    };
    let capture = agent_native_image_frame_capture(
        report.viewport.width,
        report.viewport.height,
        object,
        frame,
        request.capture_kind,
        native_session,
    )?;
    agent_native_capture_result_from_raster(report, request, &capture).map(Some)
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn agent_native_image_frame_capture(
    viewport_width: u32,
    viewport_height: u32,
    object: &AgentObservedObject,
    frame: &AgentStoredImageFrame,
    capture_kind: AgentObserveCaptureKind,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
) -> Result<AgentRasterCapture, ExitCode> {
    let (x, y, width, height) = agent_clamped_bbox_rect(
        viewport_width,
        viewport_height,
        object.bbox.x,
        object.bbox.y,
        object.bbox.width,
        object.bbox.height,
    );
    let quad = agent_native_image_quad(object, frame);
    let capture = match capture_kind {
        AgentObserveCaptureKind::Color => native_session.capture_image_quads_rgba(
            &[quad],
            viewport_width.max(1),
            viewport_height.max(1),
        ),
        AgentObserveCaptureKind::ObjectId | AgentObserveCaptureKind::Mask => {
            let color = match capture_kind {
                AgentObserveCaptureKind::ObjectId => agent_object_id_color(&object.id),
                AgentObserveCaptureKind::Mask => [255, 255, 255, 255],
                AgentObserveCaptureKind::Color => unreachable!("handled above"),
            };
            native_session.capture_image_debug_quads_rgba(
                &[arcweft_render_native::NativeImageDebugQuad { quad, color }],
                viewport_width.max(1),
                viewport_height.max(1),
            )
        }
    }
    .map_err(|error| {
        eprintln!("error: native image object capture failed: {error}");
        ExitCode::FAILURE
    })?;
    let raster = AgentRasterCapture {
        width: capture.width,
        height: capture.height,
        crop_origin: None,
        composition: match capture_kind {
            AgentObserveCaptureKind::Color => AgentImageComposition::Framebuffer,
            AgentObserveCaptureKind::ObjectId => AgentImageComposition::ObjectIdAttachment,
            AgentObserveCaptureKind::Mask => AgentImageComposition::MaskAttachment,
        },
        background: [0, 0, 0, 0],
        rgba: capture.rgba,
        diagnostics: capture.diagnostics,
    };
    Ok(agent_crop_raster_capture(&raster, x, y, width, height))
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn agent_native_image_quad<'a>(
    object: &AgentObservedObject,
    frame: &'a AgentStoredImageFrame,
) -> arcweft_render_native::NativeImageQuad<'a> {
    if let Some(placement) = frame.placement {
        return arcweft_render_native::NativeImageQuad {
            width: frame.width,
            height: frame.height,
            rgba: &frame.rgba,
            opacity_milli: placement.opacity_milli,
            dst: placement.dst,
            transform: placement.transform,
        };
    }
    agent_native_image_quad_for_rect(
        frame,
        object.bbox.x,
        object.bbox.y,
        object.bbox.width,
        object.bbox.height,
        agent_image_object_opacity_milli(object),
    )
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn agent_native_image_quad_for_rect(
    frame: &AgentStoredImageFrame,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    opacity_milli: u16,
) -> arcweft_render_native::NativeImageQuad<'_> {
    arcweft_render_native::NativeImageQuad {
        width: frame.width,
        height: frame.height,
        rgba: &frame.rgba,
        opacity_milli,
        dst: arcweft_render_native::NativeImageRect {
            x: x as f32,
            y: y as f32,
            width: width as f32,
            height: height as f32,
        },
        transform: arcweft_render_native::NativeImageTransform::identity(),
    }
}

pub(super) fn agent_image_object_opacity_milli(object: &AgentObservedObject) -> u16 {
    match &object.content {
        AgentObservedObjectContent::Image(content) => content.opacity_milli.unwrap_or(1_000),
        AgentObservedObjectContent::RichText { .. } | AgentObservedObjectContent::Custom { .. } => {
            1_000
        }
    }
}

pub(super) fn agent_native_image_object_geometry_capture(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
) -> Result<Option<AgentNativeCaptureImageResult>, ExitCode> {
    let AgentCaptureScope::Object(object_id) = &request.scope else {
        return Ok(None);
    };
    let Some(object) = report.objects.iter().find(|object| object.id == *object_id) else {
        return Ok(None);
    };
    if !matches!(object.content, AgentObservedObjectContent::Image(_)) {
        return Ok(None);
    }
    if request.capture_kind == AgentObserveCaptureKind::Color {
        eprintln!(
            "error: native image object color capture requires decoded image pixels in the observation frame"
        );
        return Err(ExitCode::from(2));
    }
    let capture = agent_observed_object_geometry_capture(
        report.viewport.width,
        report.viewport.height,
        object,
        request.capture_kind,
    );
    agent_native_capture_result_from_raster(report, request, &capture).map(Some)
}

pub(super) fn agent_observed_object_geometry_capture(
    viewport_width: u32,
    viewport_height: u32,
    object: &AgentObservedObject,
    capture_kind: AgentObserveCaptureKind,
) -> AgentRasterCapture {
    let (x, y, width, height) = agent_clamped_bbox_rect(
        viewport_width,
        viewport_height,
        object.bbox.x,
        object.bbox.y,
        object.bbox.width,
        object.bbox.height,
    );
    let color = match capture_kind {
        AgentObserveCaptureKind::Color => [0, 0, 0, 0],
        AgentObserveCaptureKind::ObjectId => agent_object_id_color(&object.id),
        AgentObserveCaptureKind::Mask => [255, 255, 255, 255],
    };
    let composition = match capture_kind {
        AgentObserveCaptureKind::Color => AgentImageComposition::FramebufferCrop,
        AgentObserveCaptureKind::ObjectId => AgentImageComposition::ObjectIdAttachment,
        AgentObserveCaptureKind::Mask => AgentImageComposition::MaskAttachment,
    };
    let mut full = AgentRasterCapture::new(
        viewport_width.max(1),
        viewport_height.max(1),
        [0, 0, 0, 0],
        composition,
    );
    agent_fill_raster_rect(&mut full, x, y, width, height, color);
    agent_crop_raster_capture(&full, x, y, width, height)
}

pub(super) fn agent_image_object_for_capture_scope(
    report: &AgentObservationReport,
    scope: &AgentCaptureScope,
) -> Option<AgentImageObjectRef> {
    let AgentCaptureScope::Object(object_id) = scope else {
        return None;
    };
    report
        .objects
        .iter()
        .find(|object| object.id == *object_id)
        .map(AgentImageObjectRef::from_observed)
}

pub(super) fn agent_native_textbox_for_capture<'a>(
    report: &'a AgentObservationReport,
    scope: &AgentCaptureScope,
) -> Option<&'a AgentObservedObject> {
    if let AgentCaptureScope::Object(object_id) = scope {
        if let Some(object) = report.objects.iter().find(|object| object.id == *object_id) {
            if agent_is_dialogue_textbox(object) {
                return Some(object);
            }
            if let Some(parent_id) = agent_rich_text_child_parent_object_id(&object.id) {
                return report.objects.iter().find(|candidate| {
                    agent_is_dialogue_textbox(candidate) && candidate.id == parent_id
                });
            }
        }
        if let Some(parent_id) = agent_rich_text_child_parent_object_id(object_id) {
            return report.objects.iter().find(|candidate| {
                agent_is_dialogue_textbox(candidate) && candidate.id == parent_id
            });
        }
    }
    report
        .objects
        .iter()
        .find(|object| agent_is_dialogue_textbox(object))
}

pub(super) fn agent_rich_text_child_parent_object_id(object_id: &str) -> Option<&str> {
    object_id
        .split_once(".page.")
        .or_else(|| object_id.split_once(".line."))
        .or_else(|| object_id.split_once(".run."))
        .or_else(|| object_id.split_once(".ruby."))
        .or_else(|| object_id.split_once(".cluster."))
        .or_else(|| object_id.split_once(".proxy."))
        .map(|(parent, _)| parent)
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn agent_native_text_origin(textbox: &AgentObservedObject) -> (f32, f32) {
    (
        textbox.bbox.x.saturating_add(24) as f32,
        textbox.bbox.y.saturating_add(24) as f32,
    )
}

#[derive(Clone, Copy)]
pub(super) struct AgentNativeCaptureContext<'a> {
    pub(super) frame: &'a LineDisplayFrame,
    pub(super) left: f32,
    pub(super) top: f32,
    pub(super) objects: &'a [AgentObservedObject],
    pub(super) page_index: usize,
    pub(super) capture_time_seconds: f32,
}

pub(super) fn agent_native_scoped_capture(
    capture: &arcweft_render_native::NativeFrameCapture,
    context: AgentNativeCaptureContext<'_>,
    scope: &AgentCaptureScope,
    capture_kind: AgentObserveCaptureKind,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<AgentRasterCapture, ExitCode> {
    let mut native_session = native_session;
    let full = AgentRasterCapture {
        width: capture.width,
        height: capture.height,
        crop_origin: None,
        composition: AgentImageComposition::Framebuffer,
        background: [0, 0, 0, 255],
        rgba: capture.rgba.clone(),
        diagnostics: capture.diagnostics.clone(),
    };
    let selected = agent_native_capture_targets_for_scope(context, scope)?;
    let selected = agent_native_capture_targets_for_page(
        capture.width,
        capture.height,
        context,
        scope,
        selected,
        native_session.as_deref_mut(),
    )?;
    if capture_kind == AgentObserveCaptureKind::Color {
        let AgentCaptureScope::Viewport = scope else {
            if matches!(scope, AgentCaptureScope::Layer(_))
                && selected
                    .iter()
                    .any(|target| !target.role().starts_with("rich_text_"))
            {
                let (x, y, width, height) = agent_native_scope_rect(
                    capture.width,
                    capture.height,
                    context,
                    &selected,
                    native_session.as_deref_mut(),
                )?;
                return Ok(agent_crop_raster_capture(&full, x, y, width, height));
            }
            if let Some(isolated) = agent_native_color_capture(
                capture,
                context,
                &selected,
                native_session.as_deref_mut(),
            )? {
                let mut rgba = isolated.rgba;
                make_nontransparent_pixels_opaque(&mut rgba);
                let full = AgentRasterCapture {
                    width: isolated.width,
                    height: isolated.height,
                    crop_origin: None,
                    composition: AgentImageComposition::IsolatedRegions,
                    background: [0, 0, 0, 0],
                    rgba,
                    diagnostics: isolated.diagnostics,
                };
                let (x, y, width, height) = agent_native_scope_rect(
                    capture.width,
                    capture.height,
                    context,
                    &selected,
                    native_session.as_deref_mut(),
                )?;
                return Ok(agent_crop_raster_capture(&full, x, y, width, height));
            }
            return agent_native_masked_framebuffer_capture(
                capture,
                context,
                &selected,
                native_session.as_deref_mut(),
            );
        };
        return Ok(full);
    }

    let debug = agent_native_debug_capture(
        capture,
        context,
        &selected,
        capture_kind,
        native_session.as_deref_mut(),
    )?;
    let full = AgentRasterCapture {
        width: debug.capture.width,
        height: debug.capture.height,
        crop_origin: None,
        composition: debug.composition,
        background: [0, 0, 0, 0],
        rgba: debug.capture.rgba,
        diagnostics: debug.capture.diagnostics,
    };
    if !matches!(scope, AgentCaptureScope::Viewport) {
        let (x, y, width, height) = agent_native_scope_rect(
            capture.width,
            capture.height,
            context,
            &selected,
            native_session,
        )?;
        return Ok(agent_crop_raster_capture(&full, x, y, width, height));
    }
    Ok(full)
}

pub(super) fn make_nontransparent_pixels_opaque(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[3] > 0 {
            pixel[3] = 255;
        }
    }
}

#[derive(Clone)]
pub(super) enum AgentNativeCaptureTarget<'a> {
    Observed(&'a AgentObservedObject),
    RichTextElement {
        id: String,
        role: &'static str,
        parent: &'a AgentObservedObject,
        element: arcweft_render_native::NativeFrameElement,
    },
}

impl AgentNativeCaptureTarget<'_> {
    pub(super) fn id(&self) -> &str {
        match self {
            AgentNativeCaptureTarget::Observed(object) => &object.id,
            AgentNativeCaptureTarget::RichTextElement { id, .. } => id,
        }
    }

    pub(super) fn role(&self) -> &str {
        match self {
            AgentNativeCaptureTarget::Observed(object) => &object.role,
            AgentNativeCaptureTarget::RichTextElement { role, .. } => role,
        }
    }

    pub(super) fn observed(&self) -> Option<&AgentObservedObject> {
        match self {
            AgentNativeCaptureTarget::Observed(object) => Some(object),
            AgentNativeCaptureTarget::RichTextElement { .. } => None,
        }
    }
}

pub(super) fn agent_native_capture_targets_for_page<'a>(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'a>,
    scope: &AgentCaptureScope,
    selected: Vec<AgentNativeCaptureTarget<'a>>,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<Vec<AgentNativeCaptureTarget<'a>>, ExitCode> {
    if !matches!(scope, AgentCaptureScope::Layer(_)) {
        return Ok(selected);
    }
    let mut native_session = native_session;
    selected
        .into_iter()
        .filter_map(|target| {
            let Some(object) = target.observed() else {
                return Some(Ok(target));
            };
            match agent_native_object_is_visible_on_page(
                capture_width,
                capture_height,
                context,
                object,
                native_session.as_deref_mut(),
            ) {
                Ok(true) => Some(Ok(target)),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            }
        })
        .collect()
}

pub(super) fn agent_native_object_is_visible_on_page(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    object: &AgentObservedObject,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<bool, ExitCode> {
    if !object.role.starts_with("rich_text_") {
        return Ok(true);
    }
    agent_native_rich_text_child_rect(
        capture_width,
        capture_height,
        context,
        object,
        native_session,
    )
    .map(|rect| rect.is_some())
}

pub(super) struct AgentNativeDebugCapture {
    pub(super) capture: arcweft_render_native::NativeFrameCapture,
    pub(super) composition: AgentImageComposition,
}

pub(super) fn agent_native_color_capture(
    capture: &arcweft_render_native::NativeFrameCapture,
    context: AgentNativeCaptureContext<'_>,
    selected: &[AgentNativeCaptureTarget<'_>],
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<Option<arcweft_render_native::NativeFrameCapture>, ExitCode> {
    let mut native_session = native_session;
    let mut regions = Vec::new();
    for target in selected {
        let object_regions = agent_native_regions_for_target(
            capture.width,
            capture.height,
            context,
            target,
            [0, 0, 0, 0],
            native_session.as_deref_mut(),
        )?;
        if object_regions.iter().any(|region| region.element.is_none()) {
            return Ok(None);
        }
        regions.extend(object_regions);
    }
    let capture_result = if let Some(native_session) = native_session {
        native_session.capture_frame_color_regions_in(
            context.frame,
            arcweft_render_native::NativeCaptureViewport::new(
                capture.width,
                capture.height,
                context.left,
                context.top,
                context.page_index,
            )
            .with_time_seconds(context.capture_time_seconds),
            &regions,
        )
    } else {
        arcweft_render_native::capture_frame_color_regions_at_page(
            context.frame,
            capture.width,
            capture.height,
            context.left,
            context.top,
            context.page_index,
            &regions,
        )
    };
    capture_result.map(Some).map_err(|error| {
        eprintln!("error: native color region capture failed: {error}");
        ExitCode::FAILURE
    })
}

pub(super) fn agent_native_debug_capture(
    capture: &arcweft_render_native::NativeFrameCapture,
    context: AgentNativeCaptureContext<'_>,
    selected: &[AgentNativeCaptureTarget<'_>],
    capture_kind: AgentObserveCaptureKind,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<AgentNativeDebugCapture, ExitCode> {
    let mut native_session = native_session;
    let mut regions = Vec::new();
    for target in selected {
        let color = match capture_kind {
            AgentObserveCaptureKind::Color => {
                unreachable!("native geometry capture is debug-only")
            }
            AgentObserveCaptureKind::ObjectId => agent_object_id_color(target.id()),
            AgentObserveCaptureKind::Mask => [255, 255, 255, 255],
        };
        regions.extend(agent_native_regions_for_target(
            capture.width,
            capture.height,
            context,
            target,
            color,
            native_session.as_deref_mut(),
        )?);
    }
    let composition = match capture_kind {
        AgentObserveCaptureKind::Color => {
            unreachable!("native geometry capture is debug-only")
        }
        AgentObserveCaptureKind::ObjectId => AgentImageComposition::ObjectIdAttachment,
        AgentObserveCaptureKind::Mask => AgentImageComposition::MaskAttachment,
    };
    let capture_result = if let Some(native_session) = native_session {
        native_session.capture_frame_debug_regions_in(
            context.frame,
            arcweft_render_native::NativeCaptureViewport::new(
                capture.width,
                capture.height,
                context.left,
                context.top,
                context.page_index,
            )
            .with_time_seconds(context.capture_time_seconds),
            &regions,
        )
    } else {
        arcweft_render_native::capture_frame_debug_regions_at_page(
            context.frame,
            capture.width,
            capture.height,
            context.left,
            context.top,
            context.page_index,
            &regions,
        )
    };
    capture_result
        .map(|capture| AgentNativeDebugCapture {
            capture,
            composition,
        })
        .map_err(|error| {
            eprintln!("error: native debug capture failed: {error}");
            ExitCode::FAILURE
        })
}

pub(super) fn agent_native_masked_framebuffer_capture(
    capture: &arcweft_render_native::NativeFrameCapture,
    context: AgentNativeCaptureContext<'_>,
    selected: &[AgentNativeCaptureTarget<'_>],
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<AgentRasterCapture, ExitCode> {
    let mut native_session = native_session;
    let mut masked = AgentRasterCapture::new(
        capture.width,
        capture.height,
        [0, 0, 0, 0],
        AgentImageComposition::MaskedFramebufferCrop,
    );
    masked.diagnostics.clone_from(&capture.diagnostics);
    for target in selected {
        let (x, y, width, height) = agent_native_target_rect(
            capture.width,
            capture.height,
            context,
            target,
            native_session.as_deref_mut(),
        )?;
        agent_copy_native_framebuffer_rect(&mut masked, capture, x, y, width, height);
    }
    let (x, y, width, height) = agent_native_scope_rect(
        capture.width,
        capture.height,
        context,
        selected,
        native_session,
    )?;
    Ok(agent_crop_raster_capture(&masked, x, y, width, height))
}

pub(super) fn agent_copy_native_framebuffer_rect(
    target: &mut AgentRasterCapture,
    source: &arcweft_render_native::NativeFrameCapture,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) {
    let source_width = usize::try_from(source.width).unwrap_or(0);
    let target_width = usize::try_from(target.width).unwrap_or(0);
    let copy_width = usize::try_from(width).unwrap_or(0);
    let row_bytes = copy_width.saturating_mul(4);
    for row in 0..height {
        let source_y = y.saturating_add(row);
        let source_start = usize::try_from(source_y)
            .unwrap_or(0)
            .saturating_mul(source_width)
            .saturating_add(usize::try_from(x).unwrap_or(0))
            .saturating_mul(4);
        let target_start = usize::try_from(source_y)
            .unwrap_or(0)
            .saturating_mul(target_width)
            .saturating_add(usize::try_from(x).unwrap_or(0))
            .saturating_mul(4);
        let Some(source_row) = source
            .rgba
            .get(source_start..source_start.saturating_add(row_bytes))
        else {
            continue;
        };
        let Some(target_row) = target
            .rgba
            .get_mut(target_start..target_start.saturating_add(row_bytes))
        else {
            continue;
        };
        target_row.copy_from_slice(source_row);
    }
}

pub(super) fn agent_fill_raster_rect(
    target: &mut AgentRasterCapture,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
) {
    let target_width = usize::try_from(target.width).unwrap_or(0);
    for row in y..y.saturating_add(height).min(target.height) {
        for col in x..x.saturating_add(width).min(target.width) {
            let start = usize::try_from(row)
                .unwrap_or(0)
                .saturating_mul(target_width)
                .saturating_add(usize::try_from(col).unwrap_or(0))
                .saturating_mul(4);
            let Some(pixel) = target.rgba.get_mut(start..start.saturating_add(4)) else {
                continue;
            };
            pixel.copy_from_slice(&color);
        }
    }
}

pub(super) fn agent_native_regions_for_target(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    target: &AgentNativeCaptureTarget<'_>,
    color: [u8; 4],
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<Vec<arcweft_render_native::NativeFrameDebugRegion>, ExitCode> {
    let (x, y, width, height) = agent_native_target_rect(
        capture_width,
        capture_height,
        context,
        target,
        native_session,
    )?;
    let fallback_bbox = arcweft_render_native::NativeFrameContentBBox {
        x,
        y,
        width,
        height,
    };
    let elements = agent_native_elements_for_target(context, target);
    if elements.is_empty() {
        return Ok(vec![arcweft_render_native::NativeFrameDebugRegion {
            element: None,
            fallback_bbox,
            color,
        }]);
    }
    Ok(elements
        .into_iter()
        .map(|element| arcweft_render_native::NativeFrameDebugRegion {
            element: Some(element),
            fallback_bbox,
            color,
        })
        .collect())
}

pub(super) fn agent_native_elements_for_target(
    context: AgentNativeCaptureContext<'_>,
    target: &AgentNativeCaptureTarget<'_>,
) -> Vec<arcweft_render_native::NativeFrameElement> {
    match target {
        AgentNativeCaptureTarget::Observed(object) => {
            agent_native_elements_for_object(context, object)
        }
        AgentNativeCaptureTarget::RichTextElement { element, .. } => vec![*element],
    }
}

pub(super) fn agent_native_elements_for_object(
    context: AgentNativeCaptureContext<'_>,
    object: &AgentObservedObject,
) -> Vec<arcweft_render_native::NativeFrameElement> {
    if agent_is_dialogue_textbox(object) {
        let frame = agent_observed_rich_text(object);
        return frame
            .display_map
            .text_runs
            .iter()
            .enumerate()
            .map(|(index, _)| arcweft_render_native::NativeFrameElement::TextRun { index })
            .chain(
                frame
                    .display_map
                    .ruby_annotations
                    .iter()
                    .enumerate()
                    .map(|(index, _)| arcweft_render_native::NativeFrameElement::Ruby { index }),
            )
            .collect();
    }
    if object.rich_text_ref.as_ref().is_some_and(|rich_text_ref| {
        matches!(
            rich_text_ref.kind,
            AgentRichTextElementKind::TextPage | AgentRichTextElementKind::TextLine
        )
    }) {
        return agent_native_text_range_elements(context, object);
    }
    agent_native_element_for_object(object)
        .into_iter()
        .collect()
}

pub(super) fn agent_native_text_range_elements(
    context: AgentNativeCaptureContext<'_>,
    object: &AgentObservedObject,
) -> Vec<arcweft_render_native::NativeFrameElement> {
    let Some(rich_text_ref) = &object.rich_text_ref else {
        return Vec::new();
    };
    let Some(textbox) = agent_native_textbox_for_rich_text_child(context.objects, object) else {
        return Vec::new();
    };
    let range = rich_text_ref.range;
    let frame = agent_observed_rich_text(textbox);
    frame
        .display_map
        .text_runs
        .iter()
        .enumerate()
        .filter(move |(_, run)| agent_rich_text_ranges_overlap(run.range, range))
        .map(|(index, _)| arcweft_render_native::NativeFrameElement::TextRun { index })
        .chain(
            frame
                .display_map
                .ruby_annotations
                .iter()
                .enumerate()
                .filter(move |(_, ruby)| agent_rich_text_ranges_overlap(ruby.base_range, range))
                .map(|(index, _)| arcweft_render_native::NativeFrameElement::Ruby { index }),
        )
        .collect()
}

pub(super) fn agent_native_scope_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    selected: &[AgentNativeCaptureTarget<'_>],
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<(u32, u32, u32, u32), ExitCode> {
    let mut native_session = native_session;
    let mut min_x = capture_width;
    let mut min_y = capture_height;
    let mut max_x = 0_u32;
    let mut max_y = 0_u32;
    for target in selected {
        let (x, y, width, height) = agent_native_target_rect(
            capture_width,
            capture_height,
            context,
            target,
            native_session.as_deref_mut(),
        )?;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x.saturating_add(width));
        max_y = max_y.max(y.saturating_add(height));
    }
    let x = min_x.min(capture_width.saturating_sub(1));
    let y = min_y.min(capture_height.saturating_sub(1));
    let width = max_x
        .saturating_sub(x)
        .min(capture_width.saturating_sub(x))
        .max(1);
    let height = max_y
        .saturating_sub(y)
        .min(capture_height.saturating_sub(y))
        .max(1);
    Ok((x, y, width, height))
}

pub(super) fn agent_native_target_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    target: &AgentNativeCaptureTarget<'_>,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<(u32, u32, u32, u32), ExitCode> {
    match target {
        AgentNativeCaptureTarget::Observed(object) => agent_native_object_rect(
            capture_width,
            capture_height,
            context,
            object,
            native_session,
        ),
        AgentNativeCaptureTarget::RichTextElement {
            parent, element, ..
        } => agent_native_rich_text_element_rect(
            capture_width,
            capture_height,
            context,
            parent,
            *element,
            native_session,
        )?
        .ok_or_else(|| {
            eprintln!(
                "error: no native layout bounds match resource object {}",
                target.id()
            );
            ExitCode::from(2)
        }),
    }
}

pub(super) fn agent_native_object_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    object: &AgentObservedObject,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<(u32, u32, u32, u32), ExitCode> {
    if agent_is_dialogue_textbox(object) {
        return agent_native_textbox_rect(
            capture_width,
            capture_height,
            context,
            object,
            native_session,
        );
    }
    if object.role.starts_with("rich_text_")
        && let Some(rect) = agent_native_rich_text_child_rect(
            capture_width,
            capture_height,
            context,
            object,
            native_session,
        )?
    {
        return Ok(rect);
    }
    Ok(agent_clamped_bbox_rect(
        capture_width,
        capture_height,
        object.bbox.x,
        object.bbox.y,
        object.bbox.width,
        object.bbox.height,
    ))
}

pub(super) fn agent_native_textbox_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    textbox: &AgentObservedObject,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<(u32, u32, u32, u32), ExitCode> {
    let mut rect = agent_clamped_bbox_rect(
        capture_width,
        capture_height,
        textbox.bbox.x,
        textbox.bbox.y,
        textbox.bbox.width,
        textbox.bbox.height,
    );
    let (left, top) = agent_native_text_origin(textbox);
    let bounds = match agent_measure_frame_elements_with_session(
        agent_observed_rich_text(textbox),
        arcweft_render_native::NativeCaptureViewport::new(
            capture_width,
            capture_height,
            left,
            top,
            context.page_index,
        )
        .with_time_seconds(context.capture_time_seconds),
        native_session,
    ) {
        Ok(bounds) => bounds,
        Err(arcweft_render_native::NativeWindowError::EmptyPages) => return Ok(rect),
        Err(error) => {
            eprintln!("error: native text layout measurement failed: {error}");
            return Err(ExitCode::FAILURE);
        }
    };
    for bounds in bounds {
        let child_rect = agent_clamped_bbox_rect(
            capture_width,
            capture_height,
            bounds.bbox.x,
            bounds.bbox.y,
            bounds.bbox.width,
            bounds.bbox.height,
        );
        rect = agent_union_rect(rect, child_rect, capture_width, capture_height);
    }
    Ok(rect)
}

pub(super) fn agent_native_rich_text_child_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    object: &AgentObservedObject,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<Option<(u32, u32, u32, u32)>, ExitCode> {
    if object.rich_text_ref.as_ref().is_some_and(|rich_text_ref| {
        matches!(
            rich_text_ref.kind,
            AgentRichTextElementKind::TextPage | AgentRichTextElementKind::TextLine
        ) && rich_text_ref.page == context.page_index
    }) {
        return Ok(Some(agent_clamped_bbox_rect(
            capture_width,
            capture_height,
            object.bbox.x,
            object.bbox.y,
            object.bbox.width,
            object.bbox.height,
        )));
    }
    let Some(element) = agent_native_element_for_object(object) else {
        return Ok(None);
    };
    let Some(textbox) = agent_native_textbox_for_rich_text_child(context.objects, object) else {
        return Ok(None);
    };
    agent_native_rich_text_element_rect(
        capture_width,
        capture_height,
        context,
        textbox,
        element,
        native_session,
    )
}

pub(super) fn agent_native_rich_text_element_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    textbox: &AgentObservedObject,
    element: arcweft_render_native::NativeFrameElement,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<Option<(u32, u32, u32, u32)>, ExitCode> {
    let (left, top) = agent_native_text_origin(textbox);
    let bounds = agent_measure_frame_elements_with_session(
        agent_observed_rich_text(textbox),
        arcweft_render_native::NativeCaptureViewport::new(
            capture_width,
            capture_height,
            left,
            top,
            context.page_index,
        )
        .with_time_seconds(context.capture_time_seconds),
        native_session,
    )
    .map_err(|error| {
        eprintln!("error: native text layout measurement failed: {error}");
        ExitCode::FAILURE
    })?;
    Ok(bounds
        .into_iter()
        .find(|bounds| bounds.element == element)
        .map(|bounds| {
            agent_clamped_bbox_rect(
                capture_width,
                capture_height,
                bounds.bbox.x,
                bounds.bbox.y,
                bounds.bbox.width,
                bounds.bbox.height,
            )
        }))
}

pub(super) fn agent_native_textbox_for_rich_text_child<'a>(
    objects: &'a [AgentObservedObject],
    object: &AgentObservedObject,
) -> Option<&'a AgentObservedObject> {
    let parent_id = agent_rich_text_child_parent_object_id(&object.id)?;
    objects
        .iter()
        .find(|candidate| agent_is_dialogue_textbox(candidate) && candidate.id == parent_id)
}

pub(super) fn agent_native_element_for_object(
    object: &AgentObservedObject,
) -> Option<arcweft_render_native::NativeFrameElement> {
    let Some(rich_text_ref) = &object.rich_text_ref else {
        return agent_native_element_for_object_id(&object.id);
    };
    match rich_text_ref.kind {
        AgentRichTextElementKind::TextPage | AgentRichTextElementKind::TextLine => None,
        AgentRichTextElementKind::TextRun
        | AgentRichTextElementKind::Ruby
        | AgentRichTextElementKind::TextObjectProxy => {
            agent_native_element_for_object_id(&object.id)
        }
        AgentRichTextElementKind::TextGlyph | AgentRichTextElementKind::GlyphCluster => {
            Some(arcweft_render_native::NativeFrameElement::GlyphCluster {
                index: rich_text_ref.index,
                range_start: rich_text_ref.range.start,
                range_end: rich_text_ref.range.end,
            })
        }
    }
}

pub(super) fn agent_native_element_for_object_id(
    object_id: &str,
) -> Option<arcweft_render_native::NativeFrameElement> {
    agent_native_element_and_role_for_object_id(object_id).map(|(element, _)| element)
}

pub(super) fn agent_native_element_and_role_for_object_id(
    object_id: &str,
) -> Option<(arcweft_render_native::NativeFrameElement, &'static str)> {
    if let Some((_, index)) = object_id.rsplit_once(".run.") {
        return index.parse().ok().map(|index| {
            (
                arcweft_render_native::NativeFrameElement::TextRun { index },
                "rich_text_run",
            )
        });
    }
    if let Some((_, index)) = object_id.rsplit_once(".ruby.") {
        return index.parse().ok().map(|index| {
            (
                arcweft_render_native::NativeFrameElement::Ruby { index },
                "rich_text_ruby",
            )
        });
    }
    if let Some((_, suffix)) = object_id.split_once(".proxy.") {
        let mut parts = suffix.split('.');
        let run_index = parts.next()?.parse().ok()?;
        let proxy_index = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        return Some((
            arcweft_render_native::NativeFrameElement::TextObjectProxy {
                run_index,
                proxy_index,
            },
            "rich_text_proxy",
        ));
    }
    if let Some((_, suffix)) = object_id.split_once(".cluster.") {
        let mut parts = suffix.split('.');
        let index = parts.next()?.parse().ok()?;
        let range_start = parts.next()?.parse().ok()?;
        let range_end = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        return Some((
            arcweft_render_native::NativeFrameElement::GlyphCluster {
                index,
                range_start,
                range_end,
            },
            "rich_text_cluster",
        ));
    }
    if let Some((_, suffix)) = object_id.split_once(".glyph.") {
        let mut parts = suffix.split('.');
        let index = parts.next()?.parse().ok()?;
        let range_start = parts.next()?.parse().ok()?;
        let range_end = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        return Some((
            arcweft_render_native::NativeFrameElement::GlyphCluster {
                index,
                range_start,
                range_end,
            },
            "rich_text_glyph",
        ));
    }
    None
}

pub(super) fn agent_clamped_bbox_rect(
    capture_width: u32,
    capture_height: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> (u32, u32, u32, u32) {
    let x = x.min(capture_width.saturating_sub(1));
    let y = y.min(capture_height.saturating_sub(1));
    let width = width.min(capture_width.saturating_sub(x)).max(1);
    let height = height.min(capture_height.saturating_sub(y)).max(1);
    (x, y, width, height)
}

pub(super) fn agent_union_rect(
    left: (u32, u32, u32, u32),
    right: (u32, u32, u32, u32),
    capture_width: u32,
    capture_height: u32,
) -> (u32, u32, u32, u32) {
    let min_x = left.0.min(right.0);
    let min_y = left.1.min(right.1);
    let max_x = left
        .0
        .saturating_add(left.2)
        .max(right.0.saturating_add(right.2));
    let max_y = left
        .1
        .saturating_add(left.3)
        .max(right.1.saturating_add(right.3));
    let width = max_x
        .saturating_sub(min_x)
        .min(capture_width.saturating_sub(min_x))
        .max(1);
    let height = max_y
        .saturating_sub(min_y)
        .min(capture_height.saturating_sub(min_y))
        .max(1);
    (min_x, min_y, width, height)
}

pub(super) fn agent_crop_raster_capture(
    source: &AgentRasterCapture,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> AgentRasterCapture {
    let mut crop = AgentRasterCapture::new(
        width,
        height,
        source.background,
        agent_cropped_composition(source.composition),
    );
    crop.crop_origin = Some(agent_crop_origin(source.crop_origin, x, y));
    crop.diagnostics.clone_from(&source.diagnostics);
    let source_width = usize::try_from(source.width).unwrap_or(0);
    let crop_width = usize::try_from(width).unwrap_or(0);
    let row_bytes = crop_width.saturating_mul(4);
    for row in 0..height {
        let source_y = y.saturating_add(row);
        let source_start = usize::try_from(source_y)
            .unwrap_or(0)
            .saturating_mul(source_width)
            .saturating_add(usize::try_from(x).unwrap_or(0))
            .saturating_mul(4);
        let crop_start = usize::try_from(row)
            .unwrap_or(0)
            .saturating_mul(crop_width)
            .saturating_mul(4);
        let Some(source_row) = source
            .rgba
            .get(source_start..source_start.saturating_add(row_bytes))
        else {
            continue;
        };
        let Some(crop_row) = crop
            .rgba
            .get_mut(crop_start..crop_start.saturating_add(row_bytes))
        else {
            continue;
        };
        crop_row.copy_from_slice(source_row);
    }
    crop
}

pub(super) fn agent_cropped_composition(
    composition: AgentImageComposition,
) -> AgentImageComposition {
    match composition {
        AgentImageComposition::Framebuffer => AgentImageComposition::FramebufferCrop,
        composition => composition,
    }
}

pub(super) fn agent_crop_origin(
    source_origin: Option<AgentImageCropOrigin>,
    x: u32,
    y: u32,
) -> AgentImageCropOrigin {
    let source_origin = source_origin.unwrap_or(AgentImageCropOrigin {
        space: AgentCoordinateSpace::Viewport,
        x: 0,
        y: 0,
    });
    AgentImageCropOrigin {
        space: source_origin.space,
        x: source_origin.x.saturating_add(x),
        y: source_origin.y.saturating_add(y),
    }
}

pub(super) fn agent_content_viewport_bbox(
    crop_origin: Option<AgentImageCropOrigin>,
    content_bbox: Option<AgentImageContentBBox>,
) -> Option<AgentImageContentBBox> {
    let content_bbox = content_bbox?;
    let origin = crop_origin.unwrap_or(AgentImageCropOrigin {
        space: AgentCoordinateSpace::Viewport,
        x: 0,
        y: 0,
    });
    (origin.space == AgentCoordinateSpace::Viewport).then_some(AgentImageContentBBox {
        x: origin.x.saturating_add(content_bbox.x),
        y: origin.y.saturating_add(content_bbox.y),
        width: content_bbox.width,
        height: content_bbox.height,
    })
}

pub(super) fn agent_native_capture_targets_for_scope<'a>(
    context: AgentNativeCaptureContext<'a>,
    scope: &AgentCaptureScope,
) -> Result<Vec<AgentNativeCaptureTarget<'a>>, ExitCode> {
    match scope {
        AgentCaptureScope::Viewport => Ok(context
            .objects
            .iter()
            .map(AgentNativeCaptureTarget::Observed)
            .collect()),
        AgentCaptureScope::Layer(layer) => {
            let selected = context
                .objects
                .iter()
                .filter(|object| agent_object_matches_layer(object, layer))
                .map(AgentNativeCaptureTarget::Observed)
                .collect::<Vec<_>>();
            if selected.is_empty() {
                eprintln!("error: no observed object matches resource layer {layer}");
                return Err(ExitCode::from(2));
            }
            Ok(selected)
        }
        AgentCaptureScope::Object(object_id) => {
            if let Some(object) = context
                .objects
                .iter()
                .find(|object| object.id == *object_id)
            {
                return Ok(vec![AgentNativeCaptureTarget::Observed(object)]);
            }
            if let Some(target) = agent_native_rich_text_target_for_object_id(context, object_id) {
                return Ok(vec![target]);
            }
            eprintln!("error: no observed object matches resource object {object_id}");
            Err(ExitCode::from(2))
        }
    }
}

pub(super) fn agent_native_rich_text_target_for_object_id<'a>(
    context: AgentNativeCaptureContext<'a>,
    object_id: &str,
) -> Option<AgentNativeCaptureTarget<'a>> {
    let parent_id = agent_rich_text_child_parent_object_id(object_id)?;
    let parent = context
        .objects
        .iter()
        .find(|candidate| agent_is_dialogue_textbox(candidate) && candidate.id == parent_id)?;
    let (element, role) = agent_native_element_and_role_for_object_id(object_id)?;
    Some(AgentNativeCaptureTarget::RichTextElement {
        id: object_id.to_owned(),
        role,
        parent,
        element,
    })
}

pub(super) fn agent_selected_capture_metadata_from_raster(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    capture: &AgentRasterCapture,
    content_bbox: Option<AgentImageContentBBox>,
) -> Option<AgentSelectedCaptureMetadata> {
    if matches!(request.scope, AgentCaptureScope::Viewport) {
        return None;
    }
    let unclipped = agent_capture_scope_bbox(report, &request.scope)
        .unwrap_or_else(|| agent_capture_bbox_from_raster(report, capture));
    let clipped = agent_capture_bbox_from_raster(report, capture);
    let mask = agent_capture_mask_for_scope(report, request, capture, content_bbox, &clipped);
    Some(agent_selected_capture_metadata_for_ref(
        AgentSelectedCaptureMetadataSpec {
            scope: &request.scope,
            kind: agent_image_kind(request.capture_kind),
            composition: capture.composition,
            unclipped: &unclipped,
            clipped: &clipped,
            source: agent_capture_source_identity(report, &request.scope),
            mask,
            viewport: Some(&report.viewport),
        },
    ))
}

pub(super) fn agent_capture_bbox_from_raster(
    report: &AgentObservationReport,
    capture: &AgentRasterCapture,
) -> AgentBBox {
    let origin = capture.crop_origin.unwrap_or(AgentImageCropOrigin {
        space: AgentCoordinateSpace::Viewport,
        x: 0,
        y: 0,
    });
    AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x: origin.x,
        y: origin.y,
        width: capture
            .width
            .min(report.viewport.width.saturating_sub(origin.x))
            .max(1),
        height: capture
            .height
            .min(report.viewport.height.saturating_sub(origin.y))
            .max(1),
    }
}

pub(super) fn agent_capture_scope_bbox(
    report: &AgentObservationReport,
    scope: &AgentCaptureScope,
) -> Option<AgentBBox> {
    match scope {
        AgentCaptureScope::Viewport => Some(AgentBBox {
            space: AgentCoordinateSpace::Viewport,
            x: 0,
            y: 0,
            width: report.viewport.width.max(1),
            height: report.viewport.height.max(1),
        }),
        AgentCaptureScope::Layer(layer_id) => report
            .layers
            .iter()
            .find(|layer| layer.id == *layer_id)
            .map(|layer| layer.bbox.clone()),
        AgentCaptureScope::Object(object_id) => report
            .objects
            .iter()
            .find(|object| object.id == *object_id)
            .map(|object| object.bbox.clone()),
    }
}

pub(super) fn agent_capture_source_identity(
    report: &AgentObservationReport,
    scope: &AgentCaptureScope,
) -> AgentCaptureSourceIdentity {
    match scope {
        AgentCaptureScope::Viewport => {
            AgentCaptureSourceIdentity::viewport(report.viewport.width, report.viewport.height)
        }
        AgentCaptureScope::Layer(layer_id) => report
            .layers
            .iter()
            .find(|layer| layer.id == *layer_id)
            .map_or_else(
                || AgentCaptureSourceIdentity::Layer {
                    id: layer_id.clone(),
                    object_count: 0,
                },
                AgentCaptureSourceIdentity::from_layer,
            ),
        AgentCaptureScope::Object(object_id) => report
            .objects
            .iter()
            .find(|object| object.id == *object_id)
            .map_or_else(
                || AgentCaptureSourceIdentity::Object {
                    id: object_id.clone(),
                    parent_id: None,
                    entity: None,
                    layer: String::new(),
                    role: String::new(),
                    object_layer: None,
                    object_depth: None,
                    rich_text: None,
                },
                AgentCaptureSourceIdentity::from_object,
            ),
    }
}

pub(super) fn agent_capture_mask_for_scope(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    capture: &AgentRasterCapture,
    content_bbox: Option<AgentImageContentBBox>,
    clipped: &AgentBBox,
) -> Option<AgentSelectedCaptureMask> {
    if matches!(request.scope, AgentCaptureScope::Viewport) {
        return None;
    }
    let bounds = agent_layout_rect_from_bbox(clipped);
    let (object_ids, layer_ids) = match &request.scope {
        AgentCaptureScope::Viewport => (Vec::new(), Vec::new()),
        AgentCaptureScope::Layer(layer_id) => (
            report
                .objects
                .iter()
                .filter(|object| object.visible && agent_object_matches_layer(object, layer_id))
                .map(|object| object.id.clone())
                .collect(),
            vec![layer_id.clone()],
        ),
        AgentCaptureScope::Object(object_id) => (
            vec![object_id.clone()],
            report
                .objects
                .iter()
                .find(|object| object.id == *object_id)
                .map(agent_object_layers)
                .unwrap_or_default(),
        ),
    };
    Some(AgentSelectedCaptureMask {
        availability: AgentCaptureMaskAvailability::default(),
        basis: LayoutCoordinateSpace::Output,
        bounds,
        object_ids,
        layer_ids,
        has_object_id_attachment: true,
        has_alpha_mask: request.capture_kind == AgentObserveCaptureKind::Mask
            || matches!(
                capture.composition,
                AgentImageComposition::MaskAttachment
                    | AgentImageComposition::MaskedFramebufferCrop
                    | AgentImageComposition::IsolatedRegions
            )
            || content_bbox.is_some(),
    })
}
