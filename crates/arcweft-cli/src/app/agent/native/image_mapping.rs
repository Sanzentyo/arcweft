use super::*;

impl AgentObserveCaptureKind {
    pub(super) fn resource_name(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::ObjectId => "object-id",
            Self::Mask => "mask",
        }
    }
}

pub(super) fn agent_object_id_color(id: &str) -> [u8; 4] {
    let color = agent_object_id_rgba_color(id);
    [color.red, color.green, color.blue, color.alpha]
}

pub(super) fn agent_object_id_rgba_color(id: &str) -> AgentRgbaColor {
    let hash = blake3::hash(id.as_bytes());
    let bytes = hash.as_bytes();
    AgentRgbaColor {
        red: bytes[0].saturating_div(2).saturating_add(64),
        green: bytes[1].saturating_div(2).saturating_add(64),
        blue: bytes[2].saturating_div(2).saturating_add(64),
        alpha: 255,
    }
}

pub(super) fn agent_encode_png(capture: &AgentRasterCapture) -> Result<Vec<u8>, ExitCode> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, capture.width, capture.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| agent_png_error(&error))?;
        writer
            .write_image_data(&capture.rgba)
            .map_err(|error| agent_png_error(&error))?;
        writer.finish().map_err(|error| agent_png_error(&error))?;
    }
    Ok(bytes)
}

pub(super) fn agent_png_error(error: &png::EncodingError) -> ExitCode {
    eprintln!("error: failed to encode PNG capture: {error}");
    ExitCode::FAILURE
}

pub(super) fn agent_capture_uri(
    report: &AgentObservationReport,
    default_name: &str,
    extension: &str,
    options: &AgentObserveOptions,
) -> String {
    let name = if let Some(object_id) = &options.object {
        agent_scoped_capture_name("object", object_id, default_name)
    } else if let Some(layer) = &options.layer {
        agent_scoped_capture_name("layer", layer, default_name)
    } else {
        default_name.to_owned()
    };
    agent_frame_capture_uri_for_page(
        &report.session_id,
        report.tick,
        &name,
        extension,
        options.page.unwrap_or(0),
    )
}

pub(super) fn agent_frame_capture_uri(
    session_id: &str,
    tick: usize,
    name: &str,
    extension: &str,
) -> String {
    agent_frame_capture_uri_for_page(session_id, tick, name, extension, 0)
}

pub(super) fn agent_frame_capture_uri_for_page(
    session_id: &str,
    tick: usize,
    name: &str,
    extension: &str,
    page: usize,
) -> String {
    let base = agent_frame_capture_uri_base(session_id, tick, name, extension);
    if page == 0 {
        return base;
    }
    format!("{base}?page={page}")
}

pub(super) fn agent_frame_capture_uri_base(
    session_id: &str,
    tick: usize,
    name: &str,
    extension: &str,
) -> String {
    format!("arcweft://session/{session_id}/frame/{tick}/{name}.{extension}")
}

pub(super) fn agent_scoped_capture_name(prefix: &str, scope: &str, default_name: &str) -> String {
    let scope = agent_uri_component(scope);
    if default_name == "color" {
        format!("{prefix}.{scope}")
    } else {
        format!("{prefix}.{scope}.{default_name}")
    }
}

pub(super) fn agent_uri_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub(super) fn agent_textbox_object(
    step: usize,
    index: usize,
    frame: LineDisplayFrame,
    viewport: &AgentViewport,
    options: &AgentObserveOptions,
) -> AgentObservedObject {
    let width = viewport.width.saturating_sub(192);
    let lines = u32::try_from(frame.text.lines().count().max(1)).unwrap_or(u32::MAX);
    let object_slot = u32::try_from(index % 4).unwrap_or(0);
    let bottom_margin = 48 + object_slot * 10;
    let default_height = (96 + lines * 28).min(220);
    let height = options
        .textbox_height
        .unwrap_or(default_height)
        .min(viewport.height.saturating_sub(bottom_margin))
        .max(1);
    let y = viewport
        .height
        .saturating_sub(height)
        .saturating_sub(bottom_margin);
    let bbox = AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x: 96,
        y,
        width,
        height,
    };
    let object_id = format!("object.dialogue.{step}.{index}");
    let capture_refs = agent_object_capture_refs("cli", step, &object_id, &bbox);
    AgentObservedObject {
        id: object_id,
        parent_id: None,
        entity: Some(frame.callee.clone()),
        layer: "dialogue".to_owned(),
        role: AGENT_ROLE_DIALOGUE_TEXTBOX.to_owned(),
        visible: true,
        enabled: true,
        bbox: bbox.clone(),
        polygon: bbox.polygon(),
        capture_refs,
        object_layer: None,
        object_depth: None,
        text: Some(frame.text.clone()),
        rich_text_ref: None,
        content: AgentObservedObjectContent::RichText {
            frame: Box::new(frame),
        },
    }
}

pub(super) fn agent_observed_rich_text(object: &AgentObservedObject) -> &LineDisplayFrame {
    object
        .rich_text_frame()
        .expect("observed rich-text object carries rich-text content")
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "wired into live Agent observe once runtime UI commits are exposed to the CLI adapter"
    )
)]
pub(crate) fn agent_image_objects_from_ui_frame(
    session_id: &str,
    step: usize,
    viewport: &AgentViewport,
    frame: &UiFrameCommit,
    images: &UiImageSourceTable,
    visual_time_millis: u64,
) -> Vec<AgentObservedObject> {
    agent_image_observation_from_ui_frame(
        session_id,
        step,
        viewport,
        frame,
        images,
        visual_time_millis,
    )
    .objects
}

pub(super) fn agent_image_observation_from_ui_frame(
    session_id: &str,
    step: usize,
    viewport: &AgentViewport,
    frame: &UiFrameCommit,
    images: &UiImageSourceTable,
    visual_time_millis: u64,
) -> AgentUiImageObservation {
    let mut observation = AgentUiImageObservation::default();
    frame
        .image_items()
        .into_iter()
        .filter_map(|item| {
            agent_image_observation_from_ui_item(
                session_id,
                step,
                viewport,
                &item,
                images,
                visual_time_millis,
            )
        })
        .for_each(|(object, frame)| {
            observation
                .image_frames
                .frames_by_object
                .insert(object.id.clone(), frame);
            observation.objects.push(object);
        });
    observation
}

pub(super) fn agent_image_observation_from_ui_item(
    session_id: &str,
    step: usize,
    viewport: &AgentViewport,
    item: &UiFrameImageItem,
    images: &UiImageSourceTable,
    visual_time_millis: u64,
) -> Option<(AgentObservedObject, AgentStoredImageFrame)> {
    let source = images.get(item.image())?;
    let local_time_millis = source.playback().local_time_millis(visual_time_millis);
    let resolved = images
        .resolve_frame(item.image(), item.layout(), visual_time_millis)
        .ok()?;
    let frame = resolved.frame();
    let native_quad =
        arcweft_render_native::native_image_quad_from_resolved_frame(resolved).ok()?;
    let geometry = agent_image_geometry_from_native_quad(native_quad, viewport);
    let bbox = geometry.bbox;
    let polygon = geometry.polygon;
    let presentation = source.presentation();
    let semantic = item.semantic();
    let object_id = format!(
        "object.image.{}.{}.{}",
        agent_uri_component(item.layer().public_id().as_str()),
        item.node().0,
        item.image().0
    );
    let source_id = format!("ui.image.{}", item.image().0);
    let metadata = agent_image_observation_metadata(item, &source_id, presentation, semantic);
    let opacity_milli = source.opacity_milli();
    let fit = source.fit();
    let alignment = source.alignment();
    let transform = source.transform();
    let dimensions = source.image().dimensions();
    let frame_dimensions = frame.dimensions();
    Some((
        AgentObservedObject {
            id: object_id.clone(),
            parent_id: None,
            entity: Some(metadata.entity),
            layer: item.layer().public_id().as_str().to_owned(),
            role: "image".to_owned(),
            visible: semantic.is_none_or(arcweft_ui::UiSemanticNode::visible),
            enabled: semantic.is_none_or(arcweft_ui::UiSemanticNode::enabled),
            bbox: bbox.clone(),
            polygon,
            capture_refs: agent_object_capture_refs(session_id, step, &object_id, &bbox),
            object_layer: Some(metadata.object_layer),
            object_depth: metadata.object_depth,
            text: None,
            rich_text_ref: None,
            content: AgentObservedObjectContent::Image(Box::new(AgentObservedImageContent {
                source: source_id,
                object: presentation.map(|presentation| presentation.object().as_str().to_owned()),
                target: metadata.target,
                asset: presentation.map(|presentation| presentation.asset().as_str().to_owned()),
                frame_index: usize::try_from(frame.index()).ok(),
                local_time_millis: Some(local_time_millis),
                opacity_milli: Some(opacity_milli),
                fit: Some(agent_image_fit(fit)),
                alignment: Some(agent_image_alignment(alignment)),
                transform: Some(agent_image_transform(transform)),
                intrinsic_width: Some(dimensions.width()),
                intrinsic_height: Some(dimensions.height()),
                actions: metadata.actions,
                params: metadata.params,
                proxies: metadata.proxies,
            })),
        },
        AgentStoredImageFrame {
            width: frame_dimensions.width(),
            height: frame_dimensions.height(),
            rgba: frame.rgba().to_vec(),
            placement: Some(AgentStoredImagePlacement {
                dst: native_quad.dst,
                transform: native_quad.transform,
                opacity_milli: native_quad.opacity_milli,
            }),
        },
    ))
}

pub(super) struct AgentImageObservationMetadata {
    pub(super) entity: String,
    pub(super) object_layer: String,
    pub(super) object_depth: Option<i32>,
    pub(super) target: Option<String>,
    pub(super) actions: Vec<String>,
    pub(super) params: BTreeMap<String, AgentImageObjectParam>,
    pub(super) proxies: Vec<AgentPresentationObjectProxyRef>,
}

pub(super) fn agent_image_observation_metadata(
    item: &UiFrameImageItem,
    source_id: &str,
    presentation: Option<&arcweft_ui::UiImagePresentationMetadata>,
    semantic: Option<&arcweft_ui::UiSemanticNode>,
) -> AgentImageObservationMetadata {
    AgentImageObservationMetadata {
        entity: presentation.map_or_else(
            || source_id.to_owned(),
            |presentation| presentation.object().as_str().to_owned(),
        ),
        object_layer: presentation.map_or_else(
            || item.layer().public_id().as_str().to_owned(),
            |presentation| presentation.layer().as_str().to_owned(),
        ),
        object_depth: presentation.map(arcweft_ui::UiImagePresentationMetadata::depth_milli),
        target: presentation
            .map(|presentation| presentation.target().as_str().to_owned())
            .or_else(|| semantic.map(|semantic| semantic.target().id().as_str().to_owned())),
        actions: agent_image_observation_actions(presentation, semantic),
        params: presentation
            .map(|presentation| {
                presentation
                    .params()
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.as_str().to_owned(),
                            agent_image_object_param(value.clone()),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        proxies: presentation
            .map(|presentation| {
                presentation
                    .proxies()
                    .iter()
                    .map(agent_image_object_proxy_ref)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

pub(super) fn agent_image_observation_actions(
    presentation: Option<&arcweft_ui::UiImagePresentationMetadata>,
    semantic: Option<&arcweft_ui::UiSemanticNode>,
) -> Vec<String> {
    presentation.map_or_else(
        || {
            semantic
                .map(|semantic| {
                    semantic
                        .actions()
                        .iter()
                        .map(|action| action.as_str().to_owned())
                        .collect()
                })
                .unwrap_or_default()
        },
        |presentation| {
            presentation
                .actions()
                .iter()
                .map(|action| action.as_str().to_owned())
                .collect()
        },
    )
}

pub(super) fn agent_image_object_param(value: ImageObjectParam) -> AgentImageObjectParam {
    match value {
        ImageObjectParam::Bool(value) => AgentImageObjectParam::Bool { value },
        ImageObjectParam::Integer(value) => AgentImageObjectParam::Integer { value },
        ImageObjectParam::Milli(value) => AgentImageObjectParam::Milli { value },
        ImageObjectParam::Text(value) => AgentImageObjectParam::Text { value },
        ImageObjectParam::Id(value) => AgentImageObjectParam::Id {
            value: value.as_str().to_owned(),
        },
    }
}

pub(super) fn agent_image_object_proxy_ref(
    proxy: &ImageObjectProxy,
) -> AgentPresentationObjectProxyRef {
    AgentPresentationObjectProxyRef {
        id: proxy.id().as_str().to_owned(),
        type_name: proxy.type_name().map(str::to_owned),
        role: proxy.role().map(str::to_owned),
        layer: proxy.layer().map(|layer| layer.as_str().to_owned()),
        depth: proxy.depth_milli(),
        declaration: None,
        hit_test: proxy.hit_test(),
        params: proxy
            .params()
            .iter()
            .map(|(key, value)| {
                (
                    key.as_str().to_owned(),
                    agent_image_object_proxy_param(value.clone()),
                )
            })
            .collect(),
    }
}

pub(super) fn agent_image_object_proxy_param(value: ImageObjectParam) -> RichTextParam {
    match value {
        ImageObjectParam::Bool(value) => RichTextParam::Bool { value },
        ImageObjectParam::Integer(value) => RichTextParam::Int { value },
        ImageObjectParam::Milli(value) => RichTextParam::Milli {
            value: Milli(value),
        },
        ImageObjectParam::Text(value) => RichTextParam::Text { value },
        ImageObjectParam::Id(value) => RichTextParam::Selector {
            value: value.as_str().to_owned(),
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AgentImageGeometry {
    pub(super) bbox: AgentBBox,
    pub(super) polygon: Vec<AgentPoint>,
}

pub(super) fn agent_image_geometry_from_native_quad(
    quad: arcweft_render_native::NativeImageQuad<'_>,
    viewport: &AgentViewport,
) -> AgentImageGeometry {
    let corners = [
        agent_transform_image_point(quad.transform, quad.dst.x, quad.dst.y),
        agent_transform_image_point(quad.transform, quad.dst.x + quad.dst.width, quad.dst.y),
        agent_transform_image_point(
            quad.transform,
            quad.dst.x + quad.dst.width,
            quad.dst.y + quad.dst.height,
        ),
        agent_transform_image_point(quad.transform, quad.dst.x, quad.dst.y + quad.dst.height),
    ];
    let polygon = corners
        .into_iter()
        .map(|(x, y)| agent_point_from_viewport_f32(x, y, viewport))
        .collect::<Vec<_>>();
    let min_x = corners
        .iter()
        .map(|(x, _)| *x)
        .fold(f32::INFINITY, f32::min);
    let min_y = corners
        .iter()
        .map(|(_, y)| *y)
        .fold(f32::INFINITY, f32::min);
    let max_x = corners
        .iter()
        .map(|(x, _)| *x)
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = corners
        .iter()
        .map(|(_, y)| *y)
        .fold(f32::NEG_INFINITY, f32::max);
    let x = agent_floor_viewport_f32(min_x, viewport.width);
    let y = agent_floor_viewport_f32(min_y, viewport.height);
    let right = agent_ceil_viewport_f32(max_x, viewport.width);
    let bottom = agent_ceil_viewport_f32(max_y, viewport.height);
    AgentImageGeometry {
        bbox: AgentBBox {
            space: AgentCoordinateSpace::Viewport,
            x,
            y,
            width: right.saturating_sub(x).max(1),
            height: bottom.saturating_sub(y).max(1),
        },
        polygon,
    }
}

pub(super) fn agent_transform_image_point(
    transform: arcweft_render_native::NativeImageTransform,
    x: f32,
    y: f32,
) -> (f32, f32) {
    (
        transform
            .m11
            .mul_add(x, transform.m12.mul_add(y, transform.tx)),
        transform
            .m21
            .mul_add(x, transform.m22.mul_add(y, transform.ty)),
    )
}

pub(super) fn agent_point_from_viewport_f32(
    x: f32,
    y: f32,
    viewport: &AgentViewport,
) -> AgentPoint {
    AgentPoint {
        x: agent_round_viewport_f32(x, viewport.width),
        y: agent_round_viewport_f32(y, viewport.height),
    }
}

pub(super) fn agent_round_viewport_f32(value: f32, viewport_extent: u32) -> u32 {
    agent_clamp_viewport_f32(value.round(), viewport_extent)
}

pub(super) fn agent_floor_viewport_f32(value: f32, viewport_extent: u32) -> u32 {
    agent_clamp_viewport_f32(value.floor(), viewport_extent)
}

pub(super) fn agent_ceil_viewport_f32(value: f32, viewport_extent: u32) -> u32 {
    agent_clamp_viewport_f32(value.ceil(), viewport_extent)
}

pub(super) fn agent_clamp_viewport_f32(value: f32, viewport_extent: u32) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    let max = viewport_extent
        .max(1)
        .to_string()
        .parse::<f32>()
        .unwrap_or(f32::MAX);
    value.clamp(0.0, max).to_string().parse().unwrap_or(0)
}

pub(super) fn agent_image_fit(fit: arcweft_ui::ImageFit) -> AgentImageFit {
    match fit {
        arcweft_ui::ImageFit::Contain => AgentImageFit::Contain,
        arcweft_ui::ImageFit::Cover => AgentImageFit::Cover,
        arcweft_ui::ImageFit::Stretch => AgentImageFit::Stretch,
        arcweft_ui::ImageFit::Intrinsic => AgentImageFit::Intrinsic,
    }
}

pub(super) fn agent_image_alignment(alignment: arcweft_ui::ImageAlignment) -> AgentImageAlignment {
    AgentImageAlignment {
        x_milli: alignment.x_milli(),
        y_milli: alignment.y_milli(),
    }
}

pub(super) fn agent_image_transform(
    transform: arcweft_presentation::image::ImageObjectTransform,
) -> AgentImageTransform {
    AgentImageTransform {
        m11_milli: transform.m11_milli,
        m12_milli: transform.m12_milli,
        m21_milli: transform.m21_milli,
        m22_milli: transform.m22_milli,
        tx_milli: transform.tx_milli,
        ty_milli: transform.ty_milli,
    }
}

pub(super) fn agent_rich_text_child_objects(
    step: usize,
    index: usize,
    textbox: &AgentObservedObject,
    viewport: &AgentViewport,
    time_seconds: f32,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Vec<AgentObservedObject> {
    let mut children = Vec::new();
    let mut native_session = native_session;
    children.extend(agent_rich_text_page_objects(
        step,
        index,
        textbox,
        viewport,
        time_seconds,
        native_bounds,
        native_session.as_deref_mut(),
    ));
    children.extend(agent_rich_text_line_objects(
        step,
        index,
        textbox,
        viewport,
        time_seconds,
        native_bounds,
        native_session,
    ));
    let frame = agent_observed_rich_text(textbox);
    for (run_index, run) in frame.display_map.text_runs.iter().enumerate() {
        if matches!(
            run.source,
            RichTextTextSource::ControlHardBreak | RichTextTextSource::ControlRaw
        ) {
            continue;
        }
        if let Some(object) =
            agent_rich_text_run_object(step, index, run_index, textbox, run, native_bounds)
        {
            children.push(object);
        }
        children.extend(agent_rich_text_proxy_objects(
            step,
            index,
            run_index,
            textbox,
            run,
            native_bounds,
        ));
    }
    for (ruby_index, ruby) in frame.display_map.ruby_annotations.iter().enumerate() {
        if let Some(object) =
            agent_rich_text_ruby_object(step, index, ruby_index, textbox, ruby, native_bounds)
        {
            children.push(object);
        }
    }
    children.extend(agent_rich_text_glyph_objects(
        step,
        index,
        textbox,
        native_bounds,
    ));
    children.extend(agent_rich_text_cluster_objects(
        step,
        index,
        textbox,
        native_bounds,
    ));
    agent_repair_rich_text_child_parent_ids(textbox, &mut children);
    children
}

pub(super) fn agent_repair_rich_text_child_parent_ids(
    textbox: &AgentObservedObject,
    children: &mut [AgentObservedObject],
) {
    let mut valid_ids = children
        .iter()
        .map(|child| child.id.clone())
        .collect::<BTreeSet<_>>();
    valid_ids.insert(textbox.id.clone());
    for child in children {
        let is_valid = child
            .parent_id
            .as_ref()
            .is_some_and(|parent_id| valid_ids.contains(parent_id));
        if !is_valid {
            child.parent_id = Some(textbox.id.clone());
        }
    }
}

pub(super) fn agent_rich_text_page_objects(
    step: usize,
    index: usize,
    textbox: &AgentObservedObject,
    viewport: &AgentViewport,
    time_seconds: f32,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Vec<AgentObservedObject> {
    let mut native_session = native_session;
    let frame = agent_observed_rich_text(textbox);
    agent_rich_text_page_ranges(frame)
        .into_iter()
        .enumerate()
        .filter_map(|(page_index, page_range)| {
            if page_range.is_empty() {
                return None;
            }
            let page_text = frame.text.get(page_range.clone())?;
            if page_text.trim().is_empty() {
                return None;
            }
            let bbox = agent_native_textbox_capture_bbox_for_page(
                textbox,
                viewport,
                page_index,
                time_seconds,
                native_session.as_deref_mut(),
            )?;
            let range = RichTextRange::new(page_range.start, page_range.end);
            let presentation = agent_rich_text_range_presentation(frame, range);
            let mut hit_regions =
                vec![agent_hit_region(AgentHitRegionKind::TextPage, &bbox, range)];
            hit_regions.extend(agent_rich_text_range_proxy_hit_regions(
                frame,
                range,
                native_bounds,
            ));
            let object_id = agent_rich_text_page_object_id(step, index, page_index);
            Some(agent_rich_text_child_object(
                step,
                textbox,
                AgentRichTextChildObjectSpec {
                    object_id: &object_id,
                    parent_id: Some(textbox.id.clone()),
                    role: "rich_text_page",
                    text: page_text.to_owned(),
                    bbox: &bbox,
                    rich_text_ref: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextPage,
                        index: page_index,
                        page: page_index,
                        range,
                        node_index: agent_rich_text_page_node_index(frame, range),
                        source: None,
                        ruby: None,
                        presentation,
                        orientation: None,
                        vertical_form: None,
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: agent_rich_text_range_object_layer(frame, range),
                        object_depth: agent_rich_text_page_object_depth(frame, range),
                        hit_test: true,
                        hit_regions,
                    },
                    page: page_index,
                },
            ))
        })
        .collect()
}

pub(super) fn agent_rich_text_line_objects(
    step: usize,
    index: usize,
    textbox: &AgentObservedObject,
    viewport: &AgentViewport,
    time_seconds: f32,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Vec<AgentObservedObject> {
    let mut native_session = native_session;
    agent_rich_text_line_ranges(agent_observed_rich_text(textbox))
        .into_iter()
        .enumerate()
        .filter_map(|(line_index, line_range)| {
            if line_range.is_empty() {
                return None;
            }
            let line_text = agent_observed_rich_text(textbox)
                .text
                .get(line_range.clone())?;
            if line_text.trim().is_empty() {
                return None;
            }
            let range = RichTextRange::new(line_range.start, line_range.end);
            let page = agent_rich_text_page_for_range(agent_observed_rich_text(textbox), range);
            let bbox = agent_native_text_range_capture_bbox_for_page(
                textbox,
                viewport,
                page,
                range,
                time_seconds,
                native_session.as_deref_mut(),
            )?;
            let presentation =
                agent_rich_text_range_presentation(agent_observed_rich_text(textbox), range);
            let mut hit_regions =
                vec![agent_hit_region(AgentHitRegionKind::TextLine, &bbox, range)];
            hit_regions.extend(agent_rich_text_range_proxy_hit_regions(
                agent_observed_rich_text(textbox),
                range,
                native_bounds,
            ));
            let object_id = agent_rich_text_line_object_id(step, index, line_index);
            let parent_id = agent_rich_text_page_object_id(step, index, page);
            Some(agent_rich_text_child_object(
                step,
                textbox,
                AgentRichTextChildObjectSpec {
                    object_id: &object_id,
                    parent_id: Some(parent_id),
                    role: "rich_text_line",
                    text: line_text.to_owned(),
                    bbox: &bbox,
                    rich_text_ref: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextLine,
                        index: line_index,
                        page,
                        range,
                        node_index: agent_rich_text_page_node_index(
                            agent_observed_rich_text(textbox),
                            range,
                        ),
                        source: None,
                        ruby: None,
                        presentation,
                        orientation: None,
                        vertical_form: None,
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: agent_rich_text_range_object_layer(
                            agent_observed_rich_text(textbox),
                            range,
                        ),
                        object_depth: agent_rich_text_page_object_depth(
                            agent_observed_rich_text(textbox),
                            range,
                        ),
                        hit_test: true,
                        hit_regions,
                    },
                    page,
                },
            ))
        })
        .collect()
}

pub(super) fn agent_measure_frame_elements_with_session(
    frame: &LineDisplayFrame,
    viewport: arcweft_render_native::NativeCaptureViewport,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<
    Vec<arcweft_render_native::NativeFrameElementBounds>,
    arcweft_render_native::NativeWindowError,
> {
    if let Some(native_session) = native_session {
        return native_session.measure_frame_elements_in(frame, viewport);
    }
    arcweft_render_native::measure_frame_elements_at_page_with_time(
        frame,
        viewport.width,
        viewport.height,
        viewport.left,
        viewport.top,
        viewport.page_index,
        viewport.time_seconds,
    )
}

pub(super) fn agent_native_rich_text_element_bboxes(
    textbox: &AgentObservedObject,
    viewport: &AgentViewport,
    time_seconds: f32,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> BTreeMap<arcweft_render_native::NativeFrameElement, AgentNativeRichTextElementBounds> {
    let (left, top) = agent_native_text_origin(textbox);
    let mut bboxes = BTreeMap::new();
    let mut native_session = native_session;
    for page_index in 0.. {
        let bounds = match agent_measure_frame_elements_with_session(
            agent_observed_rich_text(textbox),
            arcweft_render_native::NativeCaptureViewport::new(
                viewport.width,
                viewport.height,
                left,
                top,
                page_index,
            )
            .with_time_seconds(time_seconds),
            native_session.as_deref_mut(),
        ) {
            Ok(bounds) => bounds,
            Err(arcweft_render_native::NativeWindowError::EmptyPages) => break,
            Err(_) => return BTreeMap::new(),
        };
        for bounds in bounds {
            bboxes
                .entry(bounds.element)
                .or_insert(AgentNativeRichTextElementBounds {
                    bbox: agent_bbox_from_native(bounds.bbox),
                    glyph: bounds.glyph,
                    ruby: bounds.ruby.map(agent_ruby_geometry_from_native),
                });
        }
    }
    bboxes
}

pub(super) fn agent_native_textbox_capture_bbox_for_page(
    textbox: &AgentObservedObject,
    viewport: &AgentViewport,
    page_index: usize,
    time_seconds: f32,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Option<AgentBBox> {
    let (left, top) = agent_native_text_origin(textbox);
    let Ok(bounds) = agent_measure_frame_elements_with_session(
        agent_observed_rich_text(textbox),
        arcweft_render_native::NativeCaptureViewport::new(
            viewport.width,
            viewport.height,
            left,
            top,
            page_index,
        )
        .with_time_seconds(time_seconds),
        native_session,
    ) else {
        return None;
    };
    Some(
        bounds
            .into_iter()
            .fold(textbox.bbox.clone(), |bbox, bounds| {
                agent_union_bbox(&bbox, &agent_bbox_from_native(bounds.bbox))
            }),
    )
}

pub(super) fn agent_native_text_range_capture_bbox_for_page(
    textbox: &AgentObservedObject,
    viewport: &AgentViewport,
    page_index: usize,
    range: RichTextRange,
    time_seconds: f32,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Option<AgentBBox> {
    let (left, top) = agent_native_text_origin(textbox);
    let bounds = agent_measure_frame_elements_with_session(
        agent_observed_rich_text(textbox),
        arcweft_render_native::NativeCaptureViewport::new(
            viewport.width,
            viewport.height,
            left,
            top,
            page_index,
        )
        .with_time_seconds(time_seconds),
        native_session,
    )
    .ok()?;
    bounds
        .into_iter()
        .filter(|bounds| {
            agent_native_element_overlaps_range(
                agent_observed_rich_text(textbox),
                bounds.element,
                range,
            )
        })
        .map(|bounds| agent_bbox_from_native(bounds.bbox))
        .reduce(|bbox, child| agent_union_bbox(&bbox, &child))
}

#[derive(Clone, Debug)]
pub(super) struct AgentNativeRichTextElementBounds {
    pub(super) bbox: AgentBBox,
    pub(super) glyph: Option<arcweft_render_native::NativeGlyphClusterMetadata>,
    pub(super) ruby: Option<AgentRubyElementGeometry>,
}

#[derive(Clone, Debug)]
pub(super) struct AgentRubyElementGeometry {
    pub(super) base_bbox: AgentBBox,
    pub(super) annotation_bbox: AgentBBox,
}

pub(super) fn agent_rich_text_run_object(
    step: usize,
    index: usize,
    run_index: usize,
    textbox: &AgentObservedObject,
    run: &RichTextTextRun,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Option<AgentObservedObject> {
    let frame = agent_observed_rich_text(textbox);
    let text = frame
        .text
        .get(valid_rich_text_range(run.range, &frame.text)?)?;
    if text.trim().is_empty() {
        return None;
    }
    let bbox = native_bounds
        .get(&arcweft_render_native::NativeFrameElement::TextRun { index: run_index })
        .map(|bounds| bounds.bbox.clone())?;
    let object_id = agent_rich_text_run_object_id(step, index, run_index);
    let page = agent_rich_text_page_for_range(frame, run.range);
    let parent_id = agent_rich_text_line_for_range(frame, run.range).map_or_else(
        || agent_rich_text_page_object_id(step, index, page),
        |line| agent_rich_text_line_object_id(step, index, line),
    );
    Some(agent_rich_text_child_object(
        step,
        textbox,
        AgentRichTextChildObjectSpec {
            object_id: &object_id,
            parent_id: Some(parent_id),
            role: "rich_text_run",
            text: text.to_owned(),
            bbox: &bbox,
            rich_text_ref: AgentRichTextElementRef {
                kind: AgentRichTextElementKind::TextRun,
                index: run_index,
                page,
                range: run.range,
                node_index: run.node_index,
                source: Some(run.source),
                ruby: None,
                presentation: Some(run.presentation.clone()),
                orientation: None,
                vertical_form: None,
                ruby_base_bbox: None,
                ruby_annotation_bbox: None,
                object_layer: agent_object_layer(&run.presentation),
                object_depth: agent_object_depth(&run.presentation),
                hit_test: agent_presentation_has_hit_test_proxy(&run.presentation),
                hit_regions: agent_text_hit_regions(
                    AgentHitRegionKind::TextRun,
                    &bbox,
                    run.range,
                    &run.presentation,
                ),
            },
            page,
        },
    ))
}

pub(super) fn agent_rich_text_proxy_objects(
    step: usize,
    index: usize,
    run_index: usize,
    textbox: &AgentObservedObject,
    run: &RichTextTextRun,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Vec<AgentObservedObject> {
    let frame = agent_observed_rich_text(textbox);
    let Some(range) = valid_rich_text_range(run.range, &frame.text) else {
        return Vec::new();
    };
    let Some(text) = frame.text.get(range) else {
        return Vec::new();
    };
    if text.trim().is_empty() {
        return Vec::new();
    }
    let page = agent_rich_text_page_for_range(frame, run.range);
    run.presentation
        .object_proxies
        .iter()
        .enumerate()
        .filter_map(|(proxy_index, proxy)| {
            let object_id =
                format!("object.dialogue.{step}.{index}.proxy.{run_index}.{proxy_index}");
            let presentation = agent_proxy_presentation(&run.presentation, proxy);
            let bbox = native_bounds
                .get(
                    &arcweft_render_native::NativeFrameElement::TextObjectProxy {
                        run_index,
                        proxy_index,
                    },
                )
                .map(|bounds| bounds.bbox.clone())?;
            Some(agent_rich_text_child_object(
                step,
                textbox,
                AgentRichTextChildObjectSpec {
                    object_id: &object_id,
                    parent_id: Some(agent_rich_text_run_object_id(step, index, run_index)),
                    role: "rich_text_proxy",
                    text: text.to_owned(),
                    bbox: &bbox,
                    rich_text_ref: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextObjectProxy,
                        index: proxy_index,
                        page,
                        range: run.range,
                        node_index: run.node_index,
                        source: Some(run.source),
                        ruby: None,
                        presentation: Some(presentation),
                        orientation: None,
                        vertical_form: None,
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: proxy
                            .layer
                            .clone()
                            .or_else(|| run.presentation.layer.clone()),
                        object_depth: proxy.depth.map(|depth| depth.0).or_else(|| {
                            (run.presentation.z_index != 0)
                                .then_some(i32::from(run.presentation.z_index) * 1000)
                        }),
                        hit_test: proxy.hit_test,
                        hit_regions: agent_proxy_hit_regions(
                            &bbox,
                            run.range,
                            &run.presentation,
                            proxy,
                        ),
                    },
                    page,
                },
            ))
        })
        .collect()
}

pub(super) fn agent_rich_text_ruby_object(
    step: usize,
    index: usize,
    ruby_index: usize,
    textbox: &AgentObservedObject,
    ruby: &RichTextRubyAnnotation,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Option<AgentObservedObject> {
    let frame = agent_observed_rich_text(textbox);
    let base_range = valid_rich_text_range(ruby.base_range, &frame.text)?;
    let base_text = frame.text.get(base_range)?;
    let bbox = native_bounds
        .get(&arcweft_render_native::NativeFrameElement::Ruby { index: ruby_index })
        .cloned()?;
    let object_id = format!("object.dialogue.{step}.{index}.ruby.{ruby_index}");
    let page = agent_rich_text_page_for_range(frame, ruby.base_range);
    let parent_id = agent_rich_text_line_for_range(frame, ruby.base_range).map_or_else(
        || agent_rich_text_page_object_id(step, index, page),
        |line| agent_rich_text_line_object_id(step, index, line),
    );
    let hit_regions = agent_ruby_hit_regions(&bbox, ruby.base_range);
    Some(agent_rich_text_child_object(
        step,
        textbox,
        AgentRichTextChildObjectSpec {
            object_id: &object_id,
            parent_id: Some(parent_id),
            role: "rich_text_ruby",
            text: format!("{base_text} ({})", ruby.ruby),
            bbox: &bbox.bbox,
            rich_text_ref: AgentRichTextElementRef {
                kind: AgentRichTextElementKind::Ruby,
                index: ruby_index,
                page,
                range: ruby.base_range,
                node_index: ruby.node_index,
                source: None,
                ruby: Some(ruby.ruby.clone()),
                presentation: Some(ruby.presentation.clone()),
                orientation: None,
                vertical_form: None,
                ruby_base_bbox: bbox.ruby.as_ref().map(|ruby| ruby.base_bbox.clone()),
                ruby_annotation_bbox: bbox.ruby.as_ref().map(|ruby| ruby.annotation_bbox.clone()),
                object_layer: agent_object_layer(&ruby.presentation),
                object_depth: agent_object_depth(&ruby.presentation),
                hit_test: agent_presentation_has_hit_test_proxy(&ruby.presentation),
                hit_regions,
            },
            page,
        },
    ))
}

pub(super) fn agent_rich_text_glyph_objects(
    step: usize,
    index: usize,
    textbox: &AgentObservedObject,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Vec<AgentObservedObject> {
    let frame = agent_observed_rich_text(textbox);
    native_bounds
        .iter()
        .filter_map(|(element, bounds)| {
            let arcweft_render_native::NativeFrameElement::GlyphCluster {
                index: glyph_index,
                range_start,
                range_end,
            } = *element
            else {
                return None;
            };
            let range = RichTextRange::new(range_start, range_end);
            let text = frame.text.get(valid_rich_text_range(range, &frame.text)?)?;
            if text.trim().is_empty() {
                return None;
            }
            let (run_index, run) = frame
                .display_map
                .text_runs
                .iter()
                .enumerate()
                .find(|(_, run)| range.start >= run.range.start && range.end <= run.range.end)?;
            let object_id = format!(
                "object.dialogue.{step}.{index}.glyph.{glyph_index}.{range_start}.{range_end}"
            );
            let page = agent_rich_text_page_for_range(frame, range);
            let parent_id = agent_rich_text_run_object_id(step, index, run_index);
            Some(agent_rich_text_child_object(
                step,
                textbox,
                AgentRichTextChildObjectSpec {
                    object_id: &object_id,
                    parent_id: Some(parent_id),
                    role: "rich_text_glyph",
                    text: text.to_owned(),
                    bbox: &bounds.bbox,
                    rich_text_ref: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextGlyph,
                        index: glyph_index,
                        page,
                        range,
                        node_index: run.node_index,
                        source: Some(run.source),
                        ruby: None,
                        presentation: Some(run.presentation.clone()),
                        orientation: bounds
                            .glyph
                            .map(|glyph| agent_glyph_orientation_from_native(glyph.orientation)),
                        vertical_form: bounds.glyph.map(|glyph| {
                            agent_glyph_vertical_form_from_native(glyph.vertical_form)
                        }),
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: agent_object_layer(&run.presentation),
                        object_depth: agent_object_depth(&run.presentation),
                        hit_test: agent_presentation_has_hit_test_proxy(&run.presentation),
                        hit_regions: agent_text_hit_regions(
                            AgentHitRegionKind::TextGlyph,
                            &bounds.bbox,
                            range,
                            &run.presentation,
                        ),
                    },
                    page,
                },
            ))
        })
        .collect()
}

pub(super) fn agent_rich_text_cluster_objects(
    step: usize,
    index: usize,
    textbox: &AgentObservedObject,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Vec<AgentObservedObject> {
    let frame = agent_observed_rich_text(textbox);
    native_bounds
        .iter()
        .filter_map(|(element, bounds)| {
            let arcweft_render_native::NativeFrameElement::GlyphCluster {
                index: cluster_index,
                range_start,
                range_end,
            } = *element
            else {
                return None;
            };
            let range = RichTextRange::new(range_start, range_end);
            let text = frame.text.get(valid_rich_text_range(range, &frame.text)?)?;
            if text.trim().is_empty() {
                return None;
            }
            let (run_index, run) = frame
                .display_map
                .text_runs
                .iter()
                .enumerate()
                .find(|(_, run)| range.start >= run.range.start && range.end <= run.range.end)?;
            let object_id = format!(
                "object.dialogue.{step}.{index}.cluster.{cluster_index}.{range_start}.{range_end}"
            );
            let page = agent_rich_text_page_for_range(frame, range);
            let parent_id = agent_rich_text_run_object_id(step, index, run_index);
            Some(agent_rich_text_child_object(
                step,
                textbox,
                AgentRichTextChildObjectSpec {
                    object_id: &object_id,
                    parent_id: Some(parent_id),
                    role: "rich_text_cluster",
                    text: text.to_owned(),
                    bbox: &bounds.bbox,
                    rich_text_ref: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::GlyphCluster,
                        index: cluster_index,
                        page,
                        range,
                        node_index: run.node_index,
                        source: Some(run.source),
                        ruby: None,
                        presentation: Some(run.presentation.clone()),
                        orientation: bounds
                            .glyph
                            .map(|glyph| agent_glyph_orientation_from_native(glyph.orientation)),
                        vertical_form: bounds.glyph.map(|glyph| {
                            agent_glyph_vertical_form_from_native(glyph.vertical_form)
                        }),
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: agent_object_layer(&run.presentation),
                        object_depth: agent_object_depth(&run.presentation),
                        hit_test: agent_presentation_has_hit_test_proxy(&run.presentation),
                        hit_regions: agent_text_hit_regions(
                            AgentHitRegionKind::GlyphCluster,
                            &bounds.bbox,
                            range,
                            &run.presentation,
                        ),
                    },
                    page,
                },
            ))
        })
        .collect()
}

pub(super) fn agent_bbox_from_native(
    bbox: arcweft_render_native::NativeFrameContentBBox,
) -> AgentBBox {
    AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x: bbox.x,
        y: bbox.y,
        width: bbox.width,
        height: bbox.height,
    }
}

pub(super) fn agent_hit_region(
    kind: AgentHitRegionKind,
    bbox: &AgentBBox,
    range: RichTextRange,
) -> AgentHitRegion {
    AgentHitRegion {
        kind,
        bbox: bbox.clone(),
        range,
        proxy_id: None,
        proxy_type: None,
        proxy_declaration: None,
        proxy_role: None,
        proxy_layer: None,
        depth: None,
        proxy_params: BTreeMap::new(),
    }
}

pub(super) fn agent_text_hit_regions(
    base_kind: AgentHitRegionKind,
    bbox: &AgentBBox,
    range: RichTextRange,
    presentation: &RichTextPresentation,
) -> Vec<AgentHitRegion> {
    let mut regions = vec![agent_hit_region(base_kind, bbox, range)];
    regions.extend(
        presentation
            .object_proxies
            .iter()
            .filter(|proxy| proxy.hit_test)
            .map(|proxy| agent_proxy_hit_region(bbox, range, presentation, proxy)),
    );
    regions
}

pub(super) fn agent_proxy_presentation(
    presentation: &RichTextPresentation,
    proxy: &RichTextObjectProxy,
) -> RichTextPresentation {
    let mut proxy_presentation = presentation.clone();
    proxy_presentation.object_proxies = vec![proxy.clone()];
    proxy_presentation
}

pub(super) fn agent_proxy_hit_regions(
    bbox: &AgentBBox,
    range: RichTextRange,
    presentation: &RichTextPresentation,
    proxy: &RichTextObjectProxy,
) -> Vec<AgentHitRegion> {
    proxy
        .hit_test
        .then(|| agent_proxy_hit_region(bbox, range, presentation, proxy))
        .into_iter()
        .collect()
}

pub(super) fn agent_proxy_hit_region(
    bbox: &AgentBBox,
    range: RichTextRange,
    presentation: &RichTextPresentation,
    proxy: &RichTextObjectProxy,
) -> AgentHitRegion {
    AgentHitRegion {
        kind: AgentHitRegionKind::TextObjectProxy,
        bbox: bbox.clone(),
        range,
        proxy_id: Some(proxy.id.clone()),
        proxy_type: proxy.type_name.clone(),
        proxy_declaration: proxy.declaration.clone(),
        proxy_role: proxy.role.clone(),
        proxy_layer: proxy.layer.clone().or_else(|| presentation.layer.clone()),
        depth: proxy.depth.map(|depth| depth.0),
        proxy_params: proxy.params.clone(),
    }
}

pub(super) fn agent_object_layer(presentation: &RichTextPresentation) -> Option<String> {
    presentation
        .object_proxies
        .iter()
        .filter_map(|proxy| {
            proxy
                .layer
                .as_ref()
                .map(|layer| (proxy.depth.map_or(0, |depth| depth.0), layer))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, layer)| layer.clone())
        .or_else(|| presentation.layer.clone())
}

pub(super) fn agent_object_depth(presentation: &RichTextPresentation) -> Option<i32> {
    presentation
        .object_proxies
        .iter()
        .filter_map(|proxy| proxy.depth.map(|depth| depth.0))
        .max()
        .or_else(|| (presentation.z_index != 0).then_some(i32::from(presentation.z_index) * 1000))
}

pub(super) fn agent_presentation_has_hit_test_proxy(presentation: &RichTextPresentation) -> bool {
    presentation
        .object_proxies
        .iter()
        .any(|proxy| proxy.hit_test)
}

pub(super) fn agent_ruby_hit_regions(
    bounds: &AgentNativeRichTextElementBounds,
    range: RichTextRange,
) -> Vec<AgentHitRegion> {
    let mut regions = vec![agent_hit_region(
        AgentHitRegionKind::RubyObject,
        &bounds.bbox,
        range,
    )];
    if let Some(ruby) = &bounds.ruby {
        regions.push(agent_hit_region(
            AgentHitRegionKind::RubyBase,
            &ruby.base_bbox,
            range,
        ));
        regions.push(agent_hit_region(
            AgentHitRegionKind::RubyAnnotation,
            &ruby.annotation_bbox,
            range,
        ));
    }
    regions
}

pub(super) fn agent_ruby_geometry_from_native(
    value: arcweft_render_native::NativeRubyElementGeometry,
) -> AgentRubyElementGeometry {
    AgentRubyElementGeometry {
        base_bbox: agent_bbox_from_native(value.base_bbox),
        annotation_bbox: agent_bbox_from_native(value.annotation_bbox),
    }
}

pub(super) const fn agent_glyph_orientation_from_native(
    value: arcweft_render_native::NativeGlyphOrientation,
) -> AgentGlyphOrientation {
    match value {
        arcweft_render_native::NativeGlyphOrientation::Upright => AgentGlyphOrientation::Upright,
        arcweft_render_native::NativeGlyphOrientation::SidewaysCw => {
            AgentGlyphOrientation::SidewaysCw
        }
        arcweft_render_native::NativeGlyphOrientation::TextCombineUpright => {
            AgentGlyphOrientation::TextCombineUpright
        }
    }
}

pub(super) const fn agent_glyph_vertical_form_from_native(
    value: arcweft_render_native::NativeGlyphVerticalForm,
) -> AgentGlyphVerticalForm {
    match value {
        arcweft_render_native::NativeGlyphVerticalForm::None => AgentGlyphVerticalForm::None,
        arcweft_render_native::NativeGlyphVerticalForm::UprightAlternate => {
            AgentGlyphVerticalForm::UprightAlternate
        }
        arcweft_render_native::NativeGlyphVerticalForm::RotatedAlternate => {
            AgentGlyphVerticalForm::RotatedAlternate
        }
    }
}

pub(super) struct AgentRichTextChildObjectSpec<'a> {
    pub(super) object_id: &'a str,
    pub(super) parent_id: Option<String>,
    pub(super) role: &'a str,
    pub(super) text: String,
    pub(super) bbox: &'a AgentBBox,
    pub(super) rich_text_ref: AgentRichTextElementRef,
    pub(super) page: usize,
}

pub(super) fn agent_rich_text_child_object(
    step: usize,
    textbox: &AgentObservedObject,
    spec: AgentRichTextChildObjectSpec<'_>,
) -> AgentObservedObject {
    AgentObservedObject {
        id: spec.object_id.to_owned(),
        parent_id: spec.parent_id.or_else(|| Some(textbox.id.clone())),
        entity: textbox.entity.clone(),
        layer: "dialogue.rich_text".to_owned(),
        role: spec.role.to_owned(),
        visible: textbox.visible,
        enabled: textbox.enabled,
        bbox: spec.bbox.clone(),
        polygon: spec.bbox.polygon(),
        capture_refs: agent_object_capture_refs_for_page(
            "cli",
            step,
            spec.object_id,
            spec.bbox,
            spec.page,
        ),
        object_layer: spec.rich_text_ref.object_layer.clone(),
        object_depth: spec.rich_text_ref.object_depth,
        text: Some(spec.text.clone()),
        rich_text_ref: Some(spec.rich_text_ref),
        content: AgentObservedObjectContent::RichText {
            frame: Box::new(agent_child_line_display_frame(
                agent_observed_rich_text(textbox),
                spec.text,
            )),
        },
    }
}

pub(super) fn agent_rich_text_page_for_range(
    frame: &LineDisplayFrame,
    range: RichTextRange,
) -> usize {
    let Some(valid_range) = valid_rich_text_range(range, &frame.text) else {
        return 0;
    };
    agent_rich_text_page_ranges(frame)
        .into_iter()
        .filter(|page_range| !page_range.is_empty())
        .position(|page_range| {
            valid_range.start >= page_range.start && valid_range.end <= page_range.end
        })
        .unwrap_or(0)
}

pub(super) fn agent_rich_text_line_for_range(
    frame: &LineDisplayFrame,
    range: RichTextRange,
) -> Option<usize> {
    let valid_range = valid_rich_text_range(range, &frame.text)?;
    agent_rich_text_line_ranges(frame)
        .into_iter()
        .filter(|line_range| !line_range.is_empty())
        .position(|line_range| {
            valid_range.start >= line_range.start && valid_range.end <= line_range.end
        })
}

pub(super) fn agent_rich_text_page_object_id(step: usize, index: usize, page: usize) -> String {
    format!("object.dialogue.{step}.{index}.page.{page}")
}

pub(super) fn agent_rich_text_line_object_id(step: usize, index: usize, line: usize) -> String {
    format!("object.dialogue.{step}.{index}.line.{line}")
}

pub(super) fn agent_rich_text_run_object_id(step: usize, index: usize, run: usize) -> String {
    format!("object.dialogue.{step}.{index}.run.{run}")
}

pub(super) fn agent_rich_text_page_node_index(
    frame: &LineDisplayFrame,
    range: RichTextRange,
) -> usize {
    frame
        .display_map
        .text_runs
        .iter()
        .find(|run| agent_rich_text_ranges_overlap(run.range, range))
        .map_or(0, |run| run.node_index)
}

pub(super) fn agent_rich_text_range_presentation(
    frame: &LineDisplayFrame,
    range: RichTextRange,
) -> Option<RichTextPresentation> {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| agent_rich_text_ranges_overlap(run.range, range))
        .map(|run| run.presentation.clone())
        .reduce(|mut accumulated, presentation| {
            accumulated.merge(presentation);
            accumulated
        })
}

pub(super) fn agent_rich_text_page_object_depth(
    frame: &LineDisplayFrame,
    range: RichTextRange,
) -> Option<i32> {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| agent_rich_text_ranges_overlap(run.range, range))
        .filter_map(|run| agent_object_depth(&run.presentation))
        .max()
}

pub(super) fn agent_rich_text_range_object_layer(
    frame: &LineDisplayFrame,
    range: RichTextRange,
) -> Option<String> {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| agent_rich_text_ranges_overlap(run.range, range))
        .filter_map(|run| {
            agent_object_layer(&run.presentation)
                .map(|layer| (agent_object_depth(&run.presentation).unwrap_or(0), layer))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, layer)| layer)
}

pub(super) fn agent_rich_text_range_proxy_hit_regions(
    frame: &LineDisplayFrame,
    range: RichTextRange,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Vec<AgentHitRegion> {
    frame
        .display_map
        .text_runs
        .iter()
        .enumerate()
        .filter(|(_, run)| agent_rich_text_ranges_overlap(run.range, range))
        .flat_map(|(run_index, run)| {
            let hit_range = RichTextRange::new(
                run.range.start.max(range.start),
                run.range.end.min(range.end),
            );
            run.presentation
                .object_proxies
                .iter()
                .enumerate()
                .filter(|(_, proxy)| proxy.hit_test)
                .filter_map(move |(proxy_index, proxy)| {
                    native_bounds
                        .get(
                            &arcweft_render_native::NativeFrameElement::TextObjectProxy {
                                run_index,
                                proxy_index,
                            },
                        )
                        .map(|bounds| {
                            agent_proxy_hit_region(
                                &bounds.bbox,
                                hit_range,
                                &run.presentation,
                                proxy,
                            )
                        })
                })
        })
        .collect()
}

pub(super) fn agent_rich_text_ranges_overlap(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

pub(super) fn agent_native_element_overlaps_range(
    frame: &LineDisplayFrame,
    element: arcweft_render_native::NativeFrameElement,
    range: RichTextRange,
) -> bool {
    match element {
        arcweft_render_native::NativeFrameElement::TextRun { index } => frame
            .display_map
            .text_runs
            .get(index)
            .is_some_and(|run| agent_rich_text_ranges_overlap(run.range, range)),
        arcweft_render_native::NativeFrameElement::Ruby { index } => frame
            .display_map
            .ruby_annotations
            .get(index)
            .is_some_and(|ruby| agent_rich_text_ranges_overlap(ruby.base_range, range)),
        arcweft_render_native::NativeFrameElement::TextObjectProxy { run_index, .. } => frame
            .display_map
            .text_runs
            .get(run_index)
            .is_some_and(|run| agent_rich_text_ranges_overlap(run.range, range)),
        arcweft_render_native::NativeFrameElement::GlyphCluster {
            range_start,
            range_end,
            ..
        } => agent_rich_text_ranges_overlap(RichTextRange::new(range_start, range_end), range),
    }
}

pub(super) fn agent_rich_text_page_ranges(frame: &LineDisplayFrame) -> Vec<std::ops::Range<usize>> {
    let mut break_offsets = frame
        .display_map
        .controls
        .iter()
        .filter(|marker| {
            matches!(
                marker.control,
                RichTextControl::Page | RichTextControl::LineWait | RichTextControl::Clear
            )
        })
        .map(|marker| agent_display_map_offset_before_node(frame, marker.node_index))
        .map(|offset| agent_display_map_offset_after_atomic_ruby_base(frame, offset))
        .filter(|offset| *offset <= frame.text.len() && frame.text.is_char_boundary(*offset))
        .collect::<Vec<_>>();
    break_offsets.sort_unstable();
    break_offsets.dedup();

    let mut start = 0;
    let mut ranges = Vec::with_capacity(break_offsets.len() + 1);
    for end in break_offsets {
        if start <= end {
            ranges.push(start..end);
            start = end;
        }
    }
    ranges.push(start..frame.text.len());
    ranges
}

pub(super) fn agent_rich_text_line_ranges(frame: &LineDisplayFrame) -> Vec<std::ops::Range<usize>> {
    let mut break_offsets = frame
        .display_map
        .controls
        .iter()
        .filter(|marker| {
            matches!(
                marker.control,
                RichTextControl::HardBreak
                    | RichTextControl::Page
                    | RichTextControl::LineWait
                    | RichTextControl::Clear
            )
        })
        .map(|marker| agent_display_map_line_break_offset(frame, marker))
        .map(|offset| agent_display_map_offset_after_atomic_ruby_base(frame, offset))
        .filter(|offset| *offset <= frame.text.len() && frame.text.is_char_boundary(*offset))
        .collect::<Vec<_>>();
    break_offsets.sort_unstable();
    break_offsets.dedup();

    let mut start = 0;
    let mut ranges = Vec::with_capacity(break_offsets.len() + 1);
    for end in break_offsets {
        if start <= end {
            ranges.push(start..end);
            start = end;
        }
    }
    ranges.push(start..frame.text.len());
    ranges
}

pub(super) fn agent_display_map_line_break_offset(
    frame: &LineDisplayFrame,
    marker: &arcweft_render_text::RichTextControlMarker,
) -> usize {
    match marker.control {
        RichTextControl::HardBreak => marker.range.map_or_else(
            || agent_display_map_offset_before_node(frame, marker.node_index),
            |range| range.end,
        ),
        _ => agent_display_map_offset_before_node(frame, marker.node_index),
    }
}

pub(super) fn agent_display_map_offset_after_atomic_ruby_base(
    frame: &LineDisplayFrame,
    offset: usize,
) -> usize {
    let mut adjusted = offset;
    loop {
        let Some(range) = frame
            .display_map
            .ruby_annotations
            .iter()
            .filter_map(|annotation| valid_rich_text_range(annotation.base_range, &frame.text))
            .find(|range| range.start < adjusted && adjusted < range.end)
        else {
            return adjusted;
        };
        adjusted = range.end;
    }
}

pub(super) fn agent_display_map_offset_before_node(
    frame: &LineDisplayFrame,
    node_index: usize,
) -> usize {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| run.node_index < node_index)
        .map(|run| run.range.end)
        .max()
        .unwrap_or(0)
}

pub(super) fn agent_child_line_display_frame(
    parent: &LineDisplayFrame,
    text: String,
) -> LineDisplayFrame {
    LineDisplayFrame {
        line: parent.line.clone(),
        callee: parent.callee.clone(),
        text: text.clone(),
        base_styles: parent.base_styles.clone(),
        default_inline_failure_policy: parent.default_inline_failure_policy.clone(),
        style_contributions: parent.style_contributions.clone(),
        nodes: vec![RichTextNode::Text { text }],
        display_map: arcweft_render_text::RichTextDisplayMap::default(),
        host_events: Vec::new(),
        inline_failures: Vec::new(),
        unresolved: Vec::new(),
    }
}

pub(super) fn agent_object_layers(object: &AgentObservedObject) -> Vec<String> {
    let mut layers = vec![object.layer.clone()];
    if let Some(object_layer) = object
        .resolved_object_layer()
        .as_ref()
        .filter(|object_layer| *object_layer != &object.layer)
    {
        layers.push(object_layer.clone());
    }
    layers
}

pub(super) fn agent_object_matches_layer(object: &AgentObservedObject, layer: &str) -> bool {
    object.layer == layer
        || object
            .resolved_object_layer()
            .as_ref()
            .is_some_and(|object_layer| object_layer == layer)
}

pub(super) fn valid_rich_text_range(
    range: RichTextRange,
    text: &str,
) -> Option<std::ops::Range<usize>> {
    if range.start <= range.end
        && range.end <= text.len()
        && text.is_char_boundary(range.start)
        && text.is_char_boundary(range.end)
    {
        Some(range.start..range.end)
    } else {
        None
    }
}

#[derive(Clone, Debug)]
pub(super) struct AgentLayerAccumulator {
    pub(super) visible: bool,
    pub(super) bbox: AgentBBox,
    pub(super) object_count: usize,
}

pub(super) fn agent_observed_layers(
    session_id: &str,
    tick: usize,
    objects: &[AgentObservedObject],
) -> Vec<AgentObservedLayer> {
    let mut layers = BTreeMap::<String, AgentLayerAccumulator>::new();
    for object in objects {
        for object_layer in agent_object_layers(object) {
            layers
                .entry(object_layer)
                .and_modify(|layer| {
                    layer.visible |= object.visible;
                    layer.object_count = layer.object_count.saturating_add(1);
                    layer.bbox = agent_union_bbox(&layer.bbox, &object.bbox);
                })
                .or_insert_with(|| AgentLayerAccumulator {
                    visible: object.visible,
                    bbox: object.bbox.clone(),
                    object_count: 1,
                });
        }
    }
    layers
        .into_iter()
        .map(|(id, layer)| AgentObservedLayer {
            capture_refs: agent_layer_capture_refs(session_id, tick, &id, &layer.bbox),
            id,
            visible: layer.visible,
            bbox: layer.bbox,
            object_count: layer.object_count,
        })
        .collect()
}

pub(super) fn agent_union_bbox(left: &AgentBBox, right: &AgentBBox) -> AgentBBox {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let max_x = left
        .x
        .saturating_add(left.width)
        .max(right.x.saturating_add(right.width));
    let max_y = left
        .y
        .saturating_add(left.height)
        .max(right.y.saturating_add(right.height));
    AgentBBox {
        space: left.space,
        x,
        y,
        width: max_x.saturating_sub(x).max(1),
        height: max_y.saturating_sub(y).max(1),
    }
}

pub(super) fn agent_layer_capture_refs(
    session_id: &str,
    tick: usize,
    layer_id: &str,
    bbox: &AgentBBox,
) -> AgentLayerCaptureRefs {
    let name = agent_scoped_capture_name("layer", layer_id, "color");
    let object_id_name = agent_scoped_capture_name("layer", layer_id, "object-id");
    let mask_name = agent_scoped_capture_name("layer", layer_id, "mask");
    AgentLayerCaptureRefs {
        captures: vec![
            agent_layer_capture_ref(session_id, tick, &name, "png", AgentImageKind::Color, bbox),
            agent_layer_capture_ref(session_id, tick, &name, "rgba", AgentImageKind::Color, bbox),
            agent_layer_capture_ref(
                session_id,
                tick,
                &object_id_name,
                "png",
                AgentImageKind::ObjectId,
                bbox,
            ),
            agent_layer_capture_ref(
                session_id,
                tick,
                &object_id_name,
                "rgba",
                AgentImageKind::ObjectId,
                bbox,
            ),
            agent_layer_capture_ref(
                session_id,
                tick,
                &mask_name,
                "png",
                AgentImageKind::Mask,
                bbox,
            ),
            agent_layer_capture_ref(
                session_id,
                tick,
                &mask_name,
                "rgba",
                AgentImageKind::Mask,
                bbox,
            ),
        ],
    }
}

pub(super) fn agent_layer_capture_ref(
    session_id: &str,
    tick: usize,
    name: &str,
    extension: &str,
    kind: AgentImageKind,
    bbox: &AgentBBox,
) -> AgentLayerCaptureRef {
    AgentLayerCaptureRef {
        kind,
        uri: agent_frame_capture_uri(session_id, tick, name, extension),
        mime_type: agent_capture_mime_type(extension).to_owned(),
        page: 0,
        width: bbox.width.max(1),
        height: bbox.height.max(1),
    }
}

pub(super) fn agent_object_capture_refs(
    session_id: &str,
    tick: usize,
    object_id: &str,
    bbox: &AgentBBox,
) -> AgentObjectCaptureRefs {
    agent_object_capture_refs_for_page(session_id, tick, object_id, bbox, 0)
}

pub(super) fn agent_object_capture_refs_for_page(
    session_id: &str,
    tick: usize,
    object_id: &str,
    bbox: &AgentBBox,
    page: usize,
) -> AgentObjectCaptureRefs {
    let name = agent_scoped_capture_name("object", object_id, "color");
    let object_id_name = agent_scoped_capture_name("object", object_id, "object-id");
    let mask_name = agent_scoped_capture_name("object", object_id, "mask");
    AgentObjectCaptureRefs {
        object_id_color: agent_object_id_rgba_color(object_id),
        captures: vec![
            agent_object_capture_ref(
                session_id,
                tick,
                &name,
                "png",
                AgentImageKind::Color,
                bbox,
                page,
            ),
            agent_object_capture_ref(
                session_id,
                tick,
                &name,
                "rgba",
                AgentImageKind::Color,
                bbox,
                page,
            ),
            agent_object_capture_ref(
                session_id,
                tick,
                &object_id_name,
                "png",
                AgentImageKind::ObjectId,
                bbox,
                page,
            ),
            agent_object_capture_ref(
                session_id,
                tick,
                &object_id_name,
                "rgba",
                AgentImageKind::ObjectId,
                bbox,
                page,
            ),
            agent_object_capture_ref(
                session_id,
                tick,
                &mask_name,
                "png",
                AgentImageKind::Mask,
                bbox,
                page,
            ),
            agent_object_capture_ref(
                session_id,
                tick,
                &mask_name,
                "rgba",
                AgentImageKind::Mask,
                bbox,
                page,
            ),
        ],
    }
}

pub(super) fn agent_object_capture_ref(
    session_id: &str,
    tick: usize,
    name: &str,
    extension: &str,
    kind: AgentImageKind,
    bbox: &AgentBBox,
    page: usize,
) -> AgentObjectCaptureRef {
    AgentObjectCaptureRef {
        kind,
        uri: agent_frame_capture_uri_for_page(session_id, tick, name, extension, page),
        mime_type: agent_capture_mime_type(extension).to_owned(),
        page,
        width: bbox.width.max(1),
        height: bbox.height.max(1),
    }
}

pub(super) fn agent_capture_mime_type(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        _ => "application/octet-stream",
    }
}

pub(super) fn agent_overlay_svg(
    viewport: &AgentViewport,
    objects: &[&AgentObservedObject],
) -> String {
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}"><rect width="100%" height="100%" fill="#101418"/>"##,
        viewport.width, viewport.height, viewport.width, viewport.height
    );
    for object in objects {
        let _ = write!(
            svg,
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="#1f2630" stroke="#76d7c4" stroke-width="2"/>"##,
            object.bbox.x, object.bbox.y, object.bbox.width, object.bbox.height
        );
        if let Some(text) = &object.text {
            let escaped = escape_xml(text);
            let _ = write!(
                svg,
                r##"<text x="{}" y="{}" fill="#f4f7fb" font-family="sans-serif" font-size="24">{}</text>"##,
                object.bbox.x + 24,
                object.bbox.y + 48,
                escaped
            );
        }
    }
    svg.push_str("</svg>");
    svg
}

pub(super) fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(super) fn hash_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
