use super::*;
use arcweft_render_wgpu::offscreen::{CaptureAttachment, SharedFrameCapture};

impl AgentObserveImageKind {
    pub(super) const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::RawRgba => "rgba",
            Self::Overlay => "svg",
        }
    }
    pub(super) const fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::RawRgba => "application/octet-stream",
            Self::Overlay => "image/svg+xml",
        }
    }
    pub(super) const fn pixel_format(
        self,
    ) -> Option<arcweft_agent_protocol::image::AgentImagePixelFormat> {
        match self {
            Self::RawRgba => Some(arcweft_agent_protocol::image::AgentImagePixelFormat::Rgba8Unorm),
            Self::Png | Self::Overlay => None,
        }
    }
}

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
    View(String),
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
    } else if let Some(view) = capture_stem.strip_prefix("view.") {
        AgentCaptureScope::View(view.to_owned())
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
            "page" => page = value.parse::<usize>().ok()?,
            _ => return None,
        }
    }
    Some((base, page))
}

pub(super) struct AgentCaptureImageResult {
    pub(super) image: AgentImageResource,
    pub(super) bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct AgentImageFrameStore {
    full_frame: Option<AgentStoredImageFrame>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct AgentStoredImageFrame {
    scope: arcweft_agent_protocol::text_context::AgentTextContextScope,
    pub(super) width: u32,
    pub(super) height: u32,
    color_rgba: Vec<u8>,
    object_id_rgba: Vec<u8>,
    mask_rgba: Vec<u8>,
    diagnostics: Vec<AgentDiagnostic>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum AgentImageFrameStoreError {
    #[error("shared capture must cover the full frame from origin (0, 0)")]
    CroppedCapture,
    #[error("shared capture is missing its {attachment:?} attachment")]
    MissingAttachment { attachment: CaptureAttachment },
    #[error("shared color/debug capture dimensions do not match")]
    DimensionMismatch,
    #[error("capture extent {width}x{height} overflows an RGBA8 buffer")]
    ExtentOverflow { width: u32, height: u32 },
    #[error("{attachment:?} attachment has {actual} bytes; expected {expected}")]
    AttachmentSizeMismatch {
        attachment: CaptureAttachment,
        expected: usize,
        actual: usize,
    },
}

impl AgentImageFrameStore {
    pub(super) fn from_shared_captures(
        scope: arcweft_agent_protocol::text_context::AgentTextContextScope,
        color: &SharedFrameCapture,
        debug: Option<&SharedFrameCapture>,
        diagnostics: Vec<AgentDiagnostic>,
    ) -> Result<Self, AgentImageFrameStoreError> {
        if color.origin_x != 0 || color.origin_y != 0 {
            return Err(AgentImageFrameStoreError::CroppedCapture);
        }
        let color_rgba = color
            .attachment_rgba(CaptureAttachment::Color)
            .ok_or(AgentImageFrameStoreError::MissingAttachment {
                attachment: CaptureAttachment::Color,
            })?
            .to_vec();
        let pixel_bytes = agent_capture_rgba_len(color.width, color.height)?;
        if color_rgba.len() != pixel_bytes {
            return Err(AgentImageFrameStoreError::AttachmentSizeMismatch {
                attachment: CaptureAttachment::Color,
                expected: pixel_bytes,
                actual: color_rgba.len(),
            });
        }
        let (object_id_rgba, mask_rgba) = if let Some(debug) = debug {
            if debug.origin_x != 0 || debug.origin_y != 0 {
                return Err(AgentImageFrameStoreError::CroppedCapture);
            }
            if debug.width != color.width || debug.height != color.height {
                return Err(AgentImageFrameStoreError::DimensionMismatch);
            }
            let object_id = debug
                .attachment_rgba(CaptureAttachment::ObjectId)
                .ok_or(AgentImageFrameStoreError::MissingAttachment {
                    attachment: CaptureAttachment::ObjectId,
                })?
                .to_vec();
            let mask = debug
                .attachment_rgba(CaptureAttachment::Mask)
                .ok_or(AgentImageFrameStoreError::MissingAttachment {
                    attachment: CaptureAttachment::Mask,
                })?
                .to_vec();
            agent_validate_stored_attachment(CaptureAttachment::ObjectId, pixel_bytes, &object_id)?;
            agent_validate_stored_attachment(CaptureAttachment::Mask, pixel_bytes, &mask)?;
            (object_id, mask)
        } else {
            (vec![0; pixel_bytes], vec![0; pixel_bytes])
        };
        Ok(Self {
            full_frame: Some(AgentStoredImageFrame {
                scope,
                width: color.width,
                height: color.height,
                color_rgba,
                object_id_rgba,
                mask_rgba,
                diagnostics,
            }),
        })
    }

    #[cfg(test)]
    pub(super) fn from_full_attachments(
        scope: arcweft_agent_protocol::text_context::AgentTextContextScope,
        width: u32,
        height: u32,
        color_rgba: Vec<u8>,
        object_id_rgba: Vec<u8>,
        mask_rgba: Vec<u8>,
    ) -> Result<Self, AgentImageFrameStoreError> {
        let expected = agent_capture_rgba_len(width, height)?;
        agent_validate_stored_attachment(CaptureAttachment::Color, expected, &color_rgba)?;
        agent_validate_stored_attachment(CaptureAttachment::ObjectId, expected, &object_id_rgba)?;
        agent_validate_stored_attachment(CaptureAttachment::Mask, expected, &mask_rgba)?;
        Ok(Self {
            full_frame: Some(AgentStoredImageFrame {
                scope,
                width,
                height,
                color_rgba,
                object_id_rgba,
                mask_rgba,
                diagnostics: Vec::new(),
            }),
        })
    }

    pub(super) const fn full_frame(&self) -> Option<&AgentStoredImageFrame> {
        self.full_frame.as_ref()
    }
}

impl AgentStoredImageFrame {
    fn attachment(&self, kind: AgentObserveCaptureKind) -> &[u8] {
        match kind {
            AgentObserveCaptureKind::Color => &self.color_rgba,
            AgentObserveCaptureKind::ObjectId => &self.object_id_rgba,
            AgentObserveCaptureKind::Mask => &self.mask_rgba,
        }
    }
}

fn agent_capture_rgba_len(width: u32, height: u32) -> Result<usize, AgentImageFrameStoreError> {
    usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(AgentImageFrameStoreError::ExtentOverflow { width, height })
}

fn agent_validate_stored_attachment(
    attachment: CaptureAttachment,
    expected: usize,
    rgba: &[u8],
) -> Result<(), AgentImageFrameStoreError> {
    if rgba.len() == expected {
        return Ok(());
    }
    Err(AgentImageFrameStoreError::AttachmentSizeMismatch {
        attachment,
        expected,
        actual: rgba.len(),
    })
}

pub(super) fn agent_capture_resource(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    frames: &AgentImageFrameStore,
) -> Result<AgentResource, ExitCode> {
    let result = agent_capture_image(report, request, frames)?;
    report
        .image_resource(&result.image, agent_binary_resource_body(&result.bytes))
        .map_err(|error| agent_json_error(&error))
}

pub(super) fn agent_capture_image(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    frames: &AgentImageFrameStore,
) -> Result<AgentCaptureImageResult, ExitCode> {
    report.validate_text_contexts().map_err(|error| {
        eprintln!("error: capture report context: {error}");
        ExitCode::FAILURE
    })?;
    let frame = frames.full_frame().ok_or_else(|| {
        eprintln!("error: capture requires the retained shared PreparedFrame attachments");
        ExitCode::from(2)
    })?;
    if &frame.scope != report.text_contexts.scope()
        || request.capture_step != report.steps
        || agent_capture_time_millis(request.capture_time_seconds)
            != agent_capture_time_millis(agent_report_capture_time_seconds(report))
    {
        eprintln!("error: retained capture belongs to another observed session/frame/sample");
        return Err(ExitCode::from(2));
    }
    if frame.width != report.viewport.width || frame.height != report.viewport.height {
        eprintln!(
            "error: retained capture extent {}x{} does not match observed viewport {}x{}",
            frame.width, frame.height, report.viewport.width, report.viewport.height
        );
        return Err(ExitCode::FAILURE);
    }
    let page = agent_current_capture_page(report, &request.scope, request.page)?;
    let request = AgentCaptureReadRequest {
        page,
        ..request.clone()
    };
    let raster = agent_raster_for_request(report, &request, frame)?;
    agent_capture_result_from_raster(report, &request, &raster)
}

fn agent_current_capture_page(
    report: &AgentObservationReport,
    scope: &AgentCaptureScope,
    requested: usize,
) -> Result<usize, ExitCode> {
    let selection = agent_capture_object_selection(report, scope)?;
    let mut pages = BTreeSet::new();
    for object in selection.coverage {
        if let Some(reference) = &object.text_context {
            let entry = report.text_contexts.resolve(reference).map_err(|error| {
                eprintln!("error: capture page context: {error}");
                ExitCode::FAILURE
            })?;
            if let Some(stage) = entry.content.display_stage() {
                pages.insert(stage.frame().page_index);
            }
        }
    }
    let current = if pages.len() == 1 {
        pages.first().copied()
    } else {
        None
    };
    if requested != 0 && current != Some(requested) {
        eprintln!(
            "error: requested capture page {requested} is not the exact current scoped prepared page"
        );
        return Err(ExitCode::from(2));
    }
    Ok(current.unwrap_or(0))
}

fn agent_raster_for_request(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    frame: &AgentStoredImageFrame,
) -> Result<AgentRasterCapture, ExitCode> {
    let selection = agent_capture_object_selection(report, &request.scope)?;
    // Leaf text descriptors own their exact sampled geometry. Their backing
    // cluster attachment may be broader than one physical glyph or proxy;
    // aggregate object/view/layer scopes still union visible descendants.
    let leaf_text_scope = matches!(request.scope, AgentCaptureScope::Object(_))
        && selection.roots.first().is_some_and(|object| {
            object.rich_text_ref.as_ref().is_some_and(|element| {
                matches!(
                element.kind,
                arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextGlyph
                    | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::GlyphCluster
                    | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::Ruby
                    | arcweft_agent_protocol::rich_text::AgentRichTextElementKind::TextObjectProxy
            )
            })
        });
    let geometry_objects = if leaf_text_scope {
        &selection.roots
    } else {
        &selection.coverage
    };
    let geometry_bbox = agent_capture_objects_bbox(geometry_objects, frame.width, frame.height)
        .or_else(|| agent_capture_scope_bbox(report, &request.scope));
    let composition = match request.capture_kind {
        AgentObserveCaptureKind::Color => match request.scope {
            AgentCaptureScope::Viewport => AgentImageComposition::Framebuffer,
            AgentCaptureScope::Object(_)
            | AgentCaptureScope::View(_)
            | AgentCaptureScope::Layer(_) => AgentImageComposition::MaskedFramebufferCrop,
        },
        AgentObserveCaptureKind::ObjectId => AgentImageComposition::ObjectIdAttachment,
        AgentObserveCaptureKind::Mask => AgentImageComposition::MaskAttachment,
    };
    if matches!(request.scope, AgentCaptureScope::Viewport) {
        return Ok(agent_viewport_raster(
            request,
            frame,
            geometry_bbox.as_ref(),
            composition,
        ));
    }
    let bbox = geometry_bbox.ok_or_else(|| {
        agent_report_missing_capture_scope(&request.scope);
        ExitCode::from(2)
    })?;
    agent_scoped_raster(report, request, frame, &selection, &bbox, composition)
}

fn agent_viewport_raster(
    request: &AgentCaptureReadRequest,
    frame: &AgentStoredImageFrame,
    geometry_bbox: Option<&AgentBBox>,
    composition: AgentImageComposition,
) -> AgentRasterCapture {
    let content_pixels = frame
        .mask_rgba
        .chunks_exact(4)
        .filter(|pixel| pixel[3] != 0)
        .count()
        .try_into()
        .unwrap_or(u64::MAX);
    AgentRasterCapture {
        width: frame.width,
        height: frame.height,
        crop_origin: None,
        composition,
        rgba: frame.attachment(request.capture_kind).to_vec(),
        content_bbox: geometry_bbox.map(agent_content_bbox_from_agent_bbox),
        content_pixels,
        selected_object_ids: Vec::new(),
        diagnostics: frame.diagnostics.clone(),
    }
}

struct AgentCaptureObjectSelection<'a> {
    roots: Vec<&'a AgentObservedObject>,
    coverage: Vec<&'a AgentObservedObject>,
}

fn agent_scoped_raster(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    frame: &AgentStoredImageFrame,
    selection: &AgentCaptureObjectSelection<'_>,
    bbox: &AgentBBox,
    composition: AgentImageComposition,
) -> Result<AgentRasterCapture, ExitCode> {
    let (x, y, width, height) = agent_clamped_bbox_rect(
        frame.width,
        frame.height,
        bbox.x,
        bbox.y,
        bbox.width,
        bbox.height,
    );
    let selected_colors = agent_capture_object_id_remap(report, &request.scope, selection);
    let mut rgba = vec![
        0;
        agent_capture_rgba_len(width, height).map_err(|error| {
            eprintln!("error: capture allocation failed: {error}");
            ExitCode::FAILURE
        })?
    ];
    let frame_width = usize::try_from(frame.width).unwrap_or(0);
    let output_width = usize::try_from(width).unwrap_or(0);
    let mut content_pixels = 0_u64;
    for output_y in 0..height {
        for output_x in 0..width {
            let source_x = x.saturating_add(output_x);
            let source_y = y.saturating_add(output_y);
            let source_index = usize::try_from(source_y)
                .unwrap_or(0)
                .saturating_mul(frame_width)
                .saturating_add(usize::try_from(source_x).unwrap_or(0))
                .saturating_mul(4);
            let output_index = usize::try_from(output_y)
                .unwrap_or(0)
                .saturating_mul(output_width)
                .saturating_add(usize::try_from(output_x).unwrap_or(0))
                .saturating_mul(4);
            let Some(object_id) = frame
                .object_id_rgba
                .get(source_index..source_index.saturating_add(4))
            else {
                continue;
            };
            let Ok(object_id) = <&[u8; 4]>::try_from(object_id) else {
                continue;
            };
            let Some(selected_object_id) = selected_colors.get(object_id) else {
                continue;
            };
            let Some(output) = rgba.get_mut(output_index..output_index.saturating_add(4)) else {
                continue;
            };
            match request.capture_kind {
                AgentObserveCaptureKind::Color => {
                    if let Some(source) = frame
                        .color_rgba
                        .get(source_index..source_index.saturating_add(4))
                    {
                        output.copy_from_slice(source);
                    }
                }
                AgentObserveCaptureKind::ObjectId => {
                    output.copy_from_slice(selected_object_id);
                }
                AgentObserveCaptureKind::Mask => output.copy_from_slice(&[255, 255, 255, 255]),
            }
            content_pixels = content_pixels.saturating_add(1);
        }
    }
    Ok(AgentRasterCapture {
        width,
        height,
        crop_origin: Some(AgentImageCropOrigin {
            space: AgentCoordinateSpace::Viewport,
            x,
            y,
        }),
        composition,
        rgba,
        content_bbox: (content_pixels > 0).then_some(AgentImageContentBBox {
            x: 0,
            y: 0,
            width,
            height,
        }),
        content_pixels,
        selected_object_ids: selection
            .roots
            .iter()
            .map(|object| object.id.clone())
            .collect(),
        diagnostics: frame.diagnostics.clone(),
    })
}

fn agent_capture_object_id_remap(
    report: &AgentObservationReport,
    scope: &AgentCaptureScope,
    selection: &AgentCaptureObjectSelection<'_>,
) -> BTreeMap<[u8; 4], [u8; 4]> {
    let root_ids = selection
        .roots
        .iter()
        .map(|object| object.id.clone())
        .collect::<BTreeSet<_>>();
    selection
        .coverage
        .iter()
        .map(|object| {
            let selected_id = match scope {
                AgentCaptureScope::Object(id) => id.as_str(),
                AgentCaptureScope::Viewport
                | AgentCaptureScope::View(_)
                | AgentCaptureScope::Layer(_) => {
                    agent_capture_root_id(report, &root_ids, object).unwrap_or(&object.id)
                }
            };
            (
                agent_object_id_color(&object.id),
                agent_object_id_color(selected_id),
            )
        })
        .collect()
}

fn agent_capture_root_id<'a>(
    report: &'a AgentObservationReport,
    root_ids: &'a BTreeSet<String>,
    object: &'a AgentObservedObject,
) -> Option<&'a str> {
    let mut current = object;
    for _ in 0..=report.objects.len() {
        if let Some(root) = root_ids.get(&current.id) {
            return Some(root.as_str());
        }
        let parent_id = current.parent_id.as_deref()?;
        current = report
            .objects
            .iter()
            .find(|candidate| candidate.id == parent_id)?;
    }
    None
}

fn agent_capture_object_selection<'a>(
    report: &'a AgentObservationReport,
    scope: &AgentCaptureScope,
) -> Result<AgentCaptureObjectSelection<'a>, ExitCode> {
    let roots = match scope {
        AgentCaptureScope::Viewport => report
            .objects
            .iter()
            .filter(|object| object.visible)
            .collect::<Vec<_>>(),
        AgentCaptureScope::Layer(layer) => report
            .objects
            .iter()
            .filter(|object| object.visible && agent_object_matches_layer(object, layer))
            .collect(),
        AgentCaptureScope::View(view_id) => {
            let Some(view) = agent_view_scope_for_id(report, view_id) else {
                agent_report_missing_capture_scope(scope);
                return Err(ExitCode::from(2));
            };
            view.object_refs
                .iter()
                .filter_map(|id| {
                    report
                        .objects
                        .iter()
                        .find(|object| object.visible && object.id == *id)
                })
                .collect()
        }
        AgentCaptureScope::Object(object_id) => report
            .objects
            .iter()
            .find(|object| object.visible && object.id == *object_id)
            .into_iter()
            .collect::<Vec<_>>(),
    };
    if roots.is_empty() && !matches!(scope, AgentCaptureScope::Viewport) {
        agent_report_missing_capture_scope(scope);
        return Err(ExitCode::from(2));
    }
    let mut coverage_roots = roots.clone();
    if let AgentCaptureScope::Object(_) = scope
        && let Some(root) = roots.first().copied()
        && root.rich_text_ref.is_some()
    {
        let capture = report
            .text_contexts
            .element_capture(root)
            .map_err(|error| {
                eprintln!("error: capture root context: {error}");
                ExitCode::FAILURE
            })?;
        for candidate in report
            .objects
            .iter()
            .filter(|candidate| candidate.visible && candidate.id != root.id)
        {
            let included = capture.contains(candidate).map_err(|error| {
                eprintln!("error: capture contributor context: {error}");
                ExitCode::FAILURE
            })?;
            if included {
                coverage_roots.push(candidate);
            }
        }
    }
    let coverage = agent_expand_capture_descendants(report, &coverage_roots);
    Ok(AgentCaptureObjectSelection { roots, coverage })
}

fn agent_expand_capture_descendants<'a>(
    report: &'a AgentObservationReport,
    selected: &[&'a AgentObservedObject],
) -> Vec<&'a AgentObservedObject> {
    let mut ids = selected
        .iter()
        .map(|object| object.id.clone())
        .collect::<BTreeSet<_>>();
    loop {
        let mut changed = false;
        for object in report.objects.iter().filter(|object| object.visible) {
            if object
                .parent_id
                .as_ref()
                .is_some_and(|parent| ids.contains(parent))
                && ids.insert(object.id.clone())
            {
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    report
        .objects
        .iter()
        .filter(|object| object.visible && ids.contains(&object.id))
        .collect()
}

fn agent_report_missing_capture_scope(scope: &AgentCaptureScope) {
    match scope {
        AgentCaptureScope::Viewport => {
            eprintln!("error: no observed viewport is available for capture");
        }
        AgentCaptureScope::Layer(layer) => {
            eprintln!("error: no observed object matches resource layer {layer}");
        }
        AgentCaptureScope::View(view) => {
            eprintln!("error: no observed view matches resource view {view}");
        }
        AgentCaptureScope::Object(object) => {
            eprintln!("error: no observed object matches resource object {object}");
        }
    }
}

fn agent_capture_objects_bbox(
    objects: &[&AgentObservedObject],
    viewport_width: u32,
    viewport_height: u32,
) -> Option<AgentBBox> {
    let mut bounds = objects.iter().map(|object| {
        agent_clamped_bbox_rect(
            viewport_width,
            viewport_height,
            object.bbox.x,
            object.bbox.y,
            object.bbox.width,
            object.bbox.height,
        )
    });
    let first = bounds.next()?;
    let union = bounds.fold(first, |left, right| {
        agent_union_rect(left, right, viewport_width, viewport_height)
    });
    Some(AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x: union.0,
        y: union.1,
        width: union.2,
        height: union.3,
    })
}

fn agent_content_bbox_from_agent_bbox(bbox: &AgentBBox) -> AgentImageContentBBox {
    AgentImageContentBBox {
        x: bbox.x,
        y: bbox.y,
        width: bbox.width,
        height: bbox.height,
    }
}

pub(super) fn agent_capture_result_from_raster(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    capture: &AgentRasterCapture,
) -> Result<AgentCaptureImageResult, ExitCode> {
    let bytes = match request.image_kind {
        AgentObserveImageKind::Png => agent_encode_png(capture)?,
        AgentObserveImageKind::RawRgba => capture.rgba.clone(),
        AgentObserveImageKind::Overlay => unreachable!("overlay is not a raster capture"),
    };
    let content_viewport_bbox =
        agent_content_viewport_bbox(capture.crop_origin, capture.content_bbox);
    let selected_capture =
        agent_selected_capture_metadata_from_raster(report, request, capture, capture.content_bbox);
    let image = AgentImageResource {
        kind: agent_image_kind(request.capture_kind),
        renderer: AgentImageRenderer::Native,
        scope: agent_image_scope_for_capture_scope(&request.scope),
        composition: capture.composition,
        page: request.page,
        capture_step: request.capture_step,
        capture_time_millis: agent_capture_time_millis(request.capture_time_seconds),
        uri: request.uri.clone(),
        mime_type: request.image_kind.mime_type().to_owned(),
        pixel_format: request.image_kind.pixel_format(),
        width: capture.width,
        height: capture.height,
        hash: hash_hex(&bytes),
        crop_origin: capture.crop_origin,
        content_bbox: capture.content_bbox,
        content_viewport_bbox,
        content_pixels: Some(capture.content_pixels),
        object: agent_image_object_for_capture_scope(report, &request.scope),
        view: agent_image_view_for_capture_scope(report, &request.scope),
        selected_capture,
        diagnostics: capture.diagnostics.clone(),
        written: None,
    };
    Ok(AgentCaptureImageResult { image, bytes })
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

pub(super) fn agent_image_view_for_capture_scope(
    report: &AgentObservationReport,
    scope: &AgentCaptureScope,
) -> Option<AgentImageViewRef> {
    let AgentCaptureScope::View(view_id) = scope else {
        return None;
    };
    report
        .views
        .iter()
        .find(|view| view.id == *view_id)
        .map(AgentImageViewRef::from_observed)
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
    (
        min_x,
        min_y,
        max_x
            .saturating_sub(min_x)
            .min(capture_width.saturating_sub(min_x))
            .max(1),
        max_y
            .saturating_sub(min_y)
            .min(capture_height.saturating_sub(min_y))
            .max(1),
    )
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
        AgentCaptureScope::View(view_id) => report
            .views
            .iter()
            .find(|view| view.id == *view_id)
            .map(|view| view.bbox.clone()),
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
        AgentCaptureScope::View(view_id) => report
            .views
            .iter()
            .find(|view| view.id == *view_id)
            .map_or_else(
                || AgentCaptureSourceIdentity::View {
                    id: view_id.clone(),
                    parent_id: None,
                    object_count: 0,
                    object_refs: Vec::new(),
                },
                AgentCaptureSourceIdentity::from_view,
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
    let object_ids = capture.selected_object_ids.clone();
    let layer_ids = match &request.scope {
        AgentCaptureScope::Viewport => Vec::new(),
        AgentCaptureScope::Layer(layer_id) => vec![layer_id.clone()],
        AgentCaptureScope::View(_) | AgentCaptureScope::Object(_) => object_ids
            .iter()
            .filter_map(|id| report.objects.iter().find(|object| object.id == *id))
            .flat_map(agent_object_layers)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
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
