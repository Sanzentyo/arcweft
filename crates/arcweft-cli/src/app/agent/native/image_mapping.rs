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
    let name = if let Some(view_id) = &options.view {
        agent_scoped_capture_name("view", view_id, default_name)
    } else if let Some(object_id) = &options.object {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AgentImageGeometry {
    pub(super) bbox: AgentBBox,
    pub(super) polygon: Vec<AgentPoint>,
}

pub(super) fn agent_image_geometry_from_render_quad(
    quad: arcweft_render_wgpu::geometry::RenderImageQuad,
    transform: arcweft_render_wgpu::geometry::RenderImageTransformMatrix,
    viewport: &AgentViewport,
) -> AgentImageGeometry {
    let corners = [
        agent_transform_image_point(transform, quad.rect.x, quad.rect.y),
        agent_transform_image_point(transform, quad.rect.x + quad.rect.width, quad.rect.y),
        agent_transform_image_point(
            transform,
            quad.rect.x + quad.rect.width,
            quad.rect.y + quad.rect.height,
        ),
        agent_transform_image_point(transform, quad.rect.x, quad.rect.y + quad.rect.height),
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
    transform: arcweft_render_wgpu::geometry::RenderImageTransformMatrix,
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
            capture_refs: agent_layer_capture_refs(
                session_id,
                tick,
                &id,
                &layer.bbox,
                layer.object_count,
            ),
            id,
            visible: layer.visible,
            bbox: layer.bbox,
            object_count: layer.object_count,
        })
        .collect()
}

pub(super) fn agent_observed_views(
    session_id: &str,
    tick: usize,
    objects: &[AgentObservedObject],
) -> Vec<AgentObservedView> {
    let mut grouped = BTreeMap::<String, Vec<&AgentObservedObject>>::new();
    for object in objects.iter().filter(|object| object.visible) {
        let view_id = agent_view_id_for_object(object);
        grouped.entry(view_id).or_default().push(object);
    }
    grouped
        .into_iter()
        .filter_map(|(view_id, objects)| {
            let bbox = objects
                .iter()
                .map(|object| object.bbox.clone())
                .reduce(|left, right| agent_union_bbox(&left, &right))?;
            let object_refs = objects
                .iter()
                .map(|object| object.id.clone())
                .collect::<Vec<_>>();
            Some(AgentObservedView {
                id: view_id.clone(),
                parent_id: None,
                visible: objects.iter().any(|object| object.visible),
                bbox: bbox.clone(),
                object_count: objects.len(),
                object_refs: object_refs.clone(),
                capture_refs: agent_view_capture_refs(
                    session_id,
                    tick,
                    &view_id,
                    &bbox,
                    objects.len(),
                    object_refs,
                ),
            })
        })
        .collect()
}

pub(super) fn agent_view_id_for_object(object: &AgentObservedObject) -> String {
    object
        .parent_id
        .clone()
        .or_else(|| object.entity.clone())
        .unwrap_or_else(|| object.id.clone())
}

pub(super) fn agent_view_scope_for_id<'a>(
    report: &'a AgentObservationReport,
    view_id: &str,
) -> Option<&'a AgentObservedView> {
    report.views.iter().find(|view| view.id == view_id)
}

pub(super) fn agent_view_capture_refs(
    session_id: &str,
    tick: usize,
    view_id: &str,
    bbox: &AgentBBox,
    object_count: usize,
    object_refs: Vec<String>,
) -> AgentViewCaptureRefs {
    let name = agent_scoped_capture_name("view", view_id, "color");
    let object_id_name = agent_scoped_capture_name("view", view_id, "object-id");
    let mask_name = agent_scoped_capture_name("view", view_id, "mask");
    let source = AgentCaptureSourceIdentity::View {
        id: view_id.to_owned(),
        parent_id: None,
        object_count,
        object_refs,
    };
    AgentViewCaptureRefs {
        captures: vec![
            agent_view_capture_ref(AgentViewCaptureRefSpec {
                session_id,
                tick,
                name: &name,
                extension: "png",
                kind: AgentImageKind::Color,
                bbox,
                source: source.clone(),
            }),
            agent_view_capture_ref(AgentViewCaptureRefSpec {
                session_id,
                tick,
                name: &name,
                extension: "rgba",
                kind: AgentImageKind::Color,
                bbox,
                source: source.clone(),
            }),
            agent_view_capture_ref(AgentViewCaptureRefSpec {
                session_id,
                tick,
                name: &object_id_name,
                extension: "png",
                kind: AgentImageKind::ObjectId,
                bbox,
                source: source.clone(),
            }),
            agent_view_capture_ref(AgentViewCaptureRefSpec {
                session_id,
                tick,
                name: &object_id_name,
                extension: "rgba",
                kind: AgentImageKind::ObjectId,
                bbox,
                source: source.clone(),
            }),
            agent_view_capture_ref(AgentViewCaptureRefSpec {
                session_id,
                tick,
                name: &mask_name,
                extension: "png",
                kind: AgentImageKind::Mask,
                bbox,
                source: source.clone(),
            }),
            agent_view_capture_ref(AgentViewCaptureRefSpec {
                session_id,
                tick,
                name: &mask_name,
                extension: "rgba",
                kind: AgentImageKind::Mask,
                bbox,
                source,
            }),
        ],
    }
}

struct AgentViewCaptureRefSpec<'a> {
    session_id: &'a str,
    tick: usize,
    name: &'a str,
    extension: &'a str,
    kind: AgentImageKind,
    bbox: &'a AgentBBox,
    source: AgentCaptureSourceIdentity,
}

fn agent_view_capture_ref(spec: AgentViewCaptureRefSpec<'_>) -> AgentViewCaptureRef {
    let scope = match &spec.source {
        AgentCaptureSourceIdentity::View { id, .. } => AgentCaptureScope::View(id.clone()),
        AgentCaptureSourceIdentity::Viewport { .. }
        | AgentCaptureSourceIdentity::Layer { .. }
        | AgentCaptureSourceIdentity::Object { .. } => {
            AgentCaptureScope::View(spec.name.to_owned())
        }
    };
    AgentViewCaptureRef {
        kind: spec.kind,
        uri: agent_frame_capture_uri(spec.session_id, spec.tick, spec.name, spec.extension),
        mime_type: agent_capture_mime_type(spec.extension).to_owned(),
        page: 0,
        width: spec.bbox.width.max(1),
        height: spec.bbox.height.max(1),
        selected_capture: Some(agent_selected_capture_metadata_for_ref(
            AgentSelectedCaptureMetadataSpec {
                scope: &scope,
                kind: spec.kind,
                composition: spec.kind.default_capture_composition(),
                unclipped: spec.bbox,
                clipped: spec.bbox,
                source: spec.source,
                mask: None,
                viewport: None,
            },
        )),
    }
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
    object_count: usize,
) -> AgentLayerCaptureRefs {
    let name = agent_scoped_capture_name("layer", layer_id, "color");
    let object_id_name = agent_scoped_capture_name("layer", layer_id, "object-id");
    let mask_name = agent_scoped_capture_name("layer", layer_id, "mask");
    let source = AgentCaptureSourceIdentity::Layer {
        id: layer_id.to_owned(),
        object_count,
    };
    AgentLayerCaptureRefs {
        captures: vec![
            agent_layer_capture_ref(AgentLayerCaptureRefSpec {
                session_id,
                tick,
                name: &name,
                extension: "png",
                kind: AgentImageKind::Color,
                bbox,
                source: source.clone(),
            }),
            agent_layer_capture_ref(AgentLayerCaptureRefSpec {
                session_id,
                tick,
                name: &name,
                extension: "rgba",
                kind: AgentImageKind::Color,
                bbox,
                source: source.clone(),
            }),
            agent_layer_capture_ref(AgentLayerCaptureRefSpec {
                session_id,
                tick,
                name: &object_id_name,
                extension: "png",
                kind: AgentImageKind::ObjectId,
                bbox,
                source: source.clone(),
            }),
            agent_layer_capture_ref(AgentLayerCaptureRefSpec {
                session_id,
                tick,
                name: &object_id_name,
                extension: "rgba",
                kind: AgentImageKind::ObjectId,
                bbox,
                source: source.clone(),
            }),
            agent_layer_capture_ref(AgentLayerCaptureRefSpec {
                session_id,
                tick,
                name: &mask_name,
                extension: "png",
                kind: AgentImageKind::Mask,
                bbox,
                source: source.clone(),
            }),
            agent_layer_capture_ref(AgentLayerCaptureRefSpec {
                session_id,
                tick,
                name: &mask_name,
                extension: "rgba",
                kind: AgentImageKind::Mask,
                bbox,
                source,
            }),
        ],
    }
}

struct AgentLayerCaptureRefSpec<'a> {
    session_id: &'a str,
    tick: usize,
    name: &'a str,
    extension: &'a str,
    kind: AgentImageKind,
    bbox: &'a AgentBBox,
    source: AgentCaptureSourceIdentity,
}

fn agent_layer_capture_ref(spec: AgentLayerCaptureRefSpec<'_>) -> AgentLayerCaptureRef {
    let source = spec.source;
    let layer_id = match &source {
        AgentCaptureSourceIdentity::Layer { id, .. } => id.clone(),
        AgentCaptureSourceIdentity::Viewport { .. }
        | AgentCaptureSourceIdentity::View { .. }
        | AgentCaptureSourceIdentity::Object { .. } => spec.name.to_owned(),
    };
    let scope = AgentCaptureScope::Layer(layer_id.clone());
    AgentLayerCaptureRef {
        kind: spec.kind,
        uri: agent_frame_capture_uri(spec.session_id, spec.tick, spec.name, spec.extension),
        mime_type: agent_capture_mime_type(spec.extension).to_owned(),
        page: 0,
        width: spec.bbox.width.max(1),
        height: spec.bbox.height.max(1),
        selected_capture: Some(agent_selected_capture_metadata_for_ref(
            AgentSelectedCaptureMetadataSpec {
                scope: &scope,
                kind: spec.kind,
                composition: spec.kind.default_capture_composition(),
                unclipped: spec.bbox,
                clipped: spec.bbox,
                source,
                mask: None,
                viewport: None,
            },
        )),
    }
}

pub(super) fn agent_object_capture_refs_for_page(
    session_id: &str,
    tick: usize,
    object_id: &str,
    bbox: &AgentBBox,
    page: usize,
) -> AgentObjectCaptureRefs {
    agent_object_capture_refs_with_source(
        session_id,
        tick,
        object_id,
        bbox,
        page,
        AgentCaptureSourceIdentity::Object {
            id: object_id.to_owned(),
            parent_id: None,
            entity: None,
            layer: String::new(),
            role: String::new(),
            object_layer: None,
            object_depth: None,
            rich_text: None,
        },
    )
}

pub(super) fn agent_object_capture_refs_with_source(
    session_id: &str,
    tick: usize,
    object_id: &str,
    bbox: &AgentBBox,
    page: usize,
    source: AgentCaptureSourceIdentity,
) -> AgentObjectCaptureRefs {
    let name = agent_scoped_capture_name("object", object_id, "color");
    let object_id_name = agent_scoped_capture_name("object", object_id, "object-id");
    let mask_name = agent_scoped_capture_name("object", object_id, "mask");
    AgentObjectCaptureRefs {
        object_id_color: agent_object_id_rgba_color(object_id),
        captures: vec![
            agent_object_capture_ref(AgentObjectCaptureRefSpec {
                session_id,
                tick,
                name: &name,
                extension: "png",
                kind: AgentImageKind::Color,
                bbox,
                page,
                source: source.clone(),
            }),
            agent_object_capture_ref(AgentObjectCaptureRefSpec {
                session_id,
                tick,
                name: &name,
                extension: "rgba",
                kind: AgentImageKind::Color,
                bbox,
                page,
                source: source.clone(),
            }),
            agent_object_capture_ref(AgentObjectCaptureRefSpec {
                session_id,
                tick,
                name: &object_id_name,
                extension: "png",
                kind: AgentImageKind::ObjectId,
                bbox,
                page,
                source: source.clone(),
            }),
            agent_object_capture_ref(AgentObjectCaptureRefSpec {
                session_id,
                tick,
                name: &object_id_name,
                extension: "rgba",
                kind: AgentImageKind::ObjectId,
                bbox,
                page,
                source: source.clone(),
            }),
            agent_object_capture_ref(AgentObjectCaptureRefSpec {
                session_id,
                tick,
                name: &mask_name,
                extension: "png",
                kind: AgentImageKind::Mask,
                bbox,
                page,
                source: source.clone(),
            }),
            agent_object_capture_ref(AgentObjectCaptureRefSpec {
                session_id,
                tick,
                name: &mask_name,
                extension: "rgba",
                kind: AgentImageKind::Mask,
                bbox,
                page,
                source,
            }),
        ],
    }
}

struct AgentObjectCaptureRefSpec<'a> {
    session_id: &'a str,
    tick: usize,
    name: &'a str,
    extension: &'a str,
    kind: AgentImageKind,
    bbox: &'a AgentBBox,
    page: usize,
    source: AgentCaptureSourceIdentity,
}

fn agent_object_capture_ref(spec: AgentObjectCaptureRefSpec<'_>) -> AgentObjectCaptureRef {
    let scope = match &spec.source {
        AgentCaptureSourceIdentity::Object { id, .. } => AgentCaptureScope::Object(id.clone()),
        AgentCaptureSourceIdentity::Viewport { .. }
        | AgentCaptureSourceIdentity::View { .. }
        | AgentCaptureSourceIdentity::Layer { .. } => {
            AgentCaptureScope::Object(spec.name.to_owned())
        }
    };
    AgentObjectCaptureRef {
        kind: spec.kind,
        uri: agent_frame_capture_uri_for_page(
            spec.session_id,
            spec.tick,
            spec.name,
            spec.extension,
            spec.page,
        ),
        mime_type: agent_capture_mime_type(spec.extension).to_owned(),
        page: spec.page,
        width: spec.bbox.width.max(1),
        height: spec.bbox.height.max(1),
        selected_capture: Some(agent_selected_capture_metadata_for_ref(
            AgentSelectedCaptureMetadataSpec {
                scope: &scope,
                kind: spec.kind,
                composition: spec.kind.default_capture_composition(),
                unclipped: spec.bbox,
                clipped: spec.bbox,
                source: spec.source,
                mask: None,
                viewport: None,
            },
        )),
    }
}

pub(super) struct AgentSelectedCaptureMetadataSpec<'a> {
    pub(super) scope: &'a AgentCaptureScope,
    pub(super) kind: AgentImageKind,
    pub(super) composition: AgentImageComposition,
    pub(super) unclipped: &'a AgentBBox,
    pub(super) clipped: &'a AgentBBox,
    pub(super) source: AgentCaptureSourceIdentity,
    pub(super) mask: Option<AgentSelectedCaptureMask>,
    pub(super) viewport: Option<&'a AgentViewport>,
}

pub(super) fn agent_selected_capture_metadata_for_ref(
    spec: AgentSelectedCaptureMetadataSpec<'_>,
) -> AgentSelectedCaptureMetadata {
    let clipped_rect = agent_layout_rect_from_bbox(spec.clipped);
    let mask = spec.mask.or_else(|| {
        (!matches!(spec.scope, AgentCaptureScope::Viewport)).then(|| AgentSelectedCaptureMask {
            availability: AgentCaptureMaskAvailability::default(),
            basis: LayoutCoordinateSpace::Output,
            bounds: clipped_rect,
            object_ids: match spec.scope {
                AgentCaptureScope::Object(object_id) => vec![object_id.clone()],
                AgentCaptureScope::Viewport
                | AgentCaptureScope::View(_)
                | AgentCaptureScope::Layer(_) => Vec::new(),
            },
            layer_ids: match spec.scope {
                AgentCaptureScope::Layer(layer_id) => vec![layer_id.clone()],
                AgentCaptureScope::Viewport
                | AgentCaptureScope::View(_)
                | AgentCaptureScope::Object(_) => Vec::new(),
            },
            has_object_id_attachment: true,
            has_alpha_mask: spec.kind == AgentImageKind::Mask,
        })
    });
    let metadata = LayoutCaptureMetadata {
        renderer: CaptureRendererKind::SharedWgpuPreparedFrame,
        scope: agent_layout_capture_scope(spec.scope),
        composition: agent_layout_capture_composition(spec.composition),
        coordinate_basis: LayoutCoordinateSpace::Output,
        crop: CaptureCropBounds {
            basis: LayoutCoordinateSpace::Output,
            unclipped: agent_layout_rect_from_bbox(spec.unclipped),
            clipped: clipped_rect,
        },
        mask: mask.as_ref().map(|mask| LayoutCaptureMaskMetadata {
            basis: mask.basis,
            bounds: mask.bounds,
            object_ids: mask.object_ids.clone(),
            layer_ids: mask.layer_ids.clone(),
            has_object_id_attachment: mask.has_object_id_attachment,
            has_alpha_mask: mask.has_alpha_mask,
        }),
        fit_transform: agent_fit_transform_for_selected_capture(spec.viewport),
    };
    AgentSelectedCaptureMetadata::from_layout(metadata, spec.source).with_mask(mask)
}

pub(super) fn agent_fit_transform_for_selected_capture(
    viewport: Option<&AgentViewport>,
) -> arcweft_layout::FitTransformMetadata {
    let width = viewport.map_or(AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH, |viewport| {
        viewport.width
    });
    let height = viewport.map_or(AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT, |viewport| {
        viewport.height
    });
    arcweft_layout::ContentRect::calculate(
        LayoutSize::new(
            agent_u32_to_f32(AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH),
            agent_u32_to_f32(AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT),
        ),
        LayoutSize::new(agent_u32_to_f32(width), agent_u32_to_f32(height)),
        ScalePolicy::Raw,
    )
    .expect("validated Agent viewport dimensions produce a content rect")
    .fit_transform_metadata(LayoutCoordinateSpace::Output, LayoutCoordinateSpace::Output)
}

pub(super) fn agent_layout_capture_scope(scope: &AgentCaptureScope) -> LayoutCaptureScope {
    match scope {
        AgentCaptureScope::Viewport => LayoutCaptureScope::Viewport,
        AgentCaptureScope::View(id) => LayoutCaptureScope::View { id: id.clone() },
        AgentCaptureScope::Layer(id) => LayoutCaptureScope::Layer { id: id.clone() },
        AgentCaptureScope::Object(id) => LayoutCaptureScope::Object { id: id.clone() },
    }
}

pub(super) const fn agent_layout_capture_composition(
    composition: AgentImageComposition,
) -> LayoutCaptureComposition {
    match composition {
        AgentImageComposition::Framebuffer => LayoutCaptureComposition::Framebuffer,
        AgentImageComposition::OverlayVector => LayoutCaptureComposition::OverlayVector,
        AgentImageComposition::FramebufferCrop => LayoutCaptureComposition::FramebufferCrop,
        AgentImageComposition::ObjectIdAttachment => LayoutCaptureComposition::ObjectIdAttachment,
        AgentImageComposition::MaskAttachment => LayoutCaptureComposition::MaskAttachment,
        AgentImageComposition::MaskedFramebufferCrop => {
            LayoutCaptureComposition::MaskedFramebufferCrop
        }
        AgentImageComposition::IsolatedRegions => LayoutCaptureComposition::IsolatedRegions,
        AgentImageComposition::DebugGeometry => LayoutCaptureComposition::DebugGeometry,
    }
}

pub(super) fn agent_layout_rect_from_bbox(bbox: &AgentBBox) -> LayoutRect {
    LayoutRect::new(
        LayoutPoint::new(agent_u32_to_f32(bbox.x), agent_u32_to_f32(bbox.y)),
        LayoutSize::new(
            agent_u32_to_f32(bbox.width.max(1)),
            agent_u32_to_f32(bbox.height.max(1)),
        ),
    )
}

pub(super) fn agent_u32_to_f32(value: u32) -> f32 {
    value.to_string().parse().unwrap_or(f32::MAX)
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
