use super::*;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(untagged)]
pub(super) enum AgentObserveResourceOutput {
    One(Box<AgentResource>),
    Many(Vec<AgentResource>),
}

impl AgentObserveResourceOutput {
    fn into_resources(self) -> Vec<AgentResource> {
        match self {
            Self::One(resource) => vec![*resource],
            Self::Many(resources) => resources,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(untagged)]
pub(super) enum AgentObserveMcpResourceOutput {
    OneRead(McpReadResourceResult),
    ManyRead(Vec<McpReadResourceResult>),
    List(McpListResourcesResult),
    ToolResult(McpCallToolResult),
}

pub(super) fn agent_observe_mcp_resource_output(
    resource: AgentObserveResourceOutput,
    format: AgentObserveMcpFormat,
    content_policy_mode: AgentContentPolicyMode,
) -> Result<AgentObserveMcpResourceOutput, ExitCode> {
    let resources = resource
        .into_resources()
        .into_iter()
        .map(|resource| {
            agent_publish_resource_with_mode(content_policy_mode, resource).map_err(|error| {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    match format {
        AgentObserveMcpFormat::Read => {
            let mut read_results = resources
                .into_iter()
                .map(|resource| {
                    read_resource_result(&resource).map_err(|error| agent_json_error(&error))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if read_results.len() == 1 {
                Ok(AgentObserveMcpResourceOutput::OneRead(
                    read_results.remove(0),
                ))
            } else {
                Ok(AgentObserveMcpResourceOutput::ManyRead(read_results))
            }
        }
        AgentObserveMcpFormat::List => Ok(AgentObserveMcpResourceOutput::List(
            list_resources_result(&resources),
        )),
        AgentObserveMcpFormat::ToolResult => {
            if resources.len() == 1 {
                let resource = resources.first().expect("length checked");
                Ok(AgentObserveMcpResourceOutput::ToolResult(
                    tool_result_for_resource(resource).map_err(|error| agent_json_error(&error))?,
                ))
            } else {
                Ok(AgentObserveMcpResourceOutput::ToolResult(
                    tool_result_for_resources(&resources),
                ))
            }
        }
    }
}

pub(super) fn agent_observe_resource(
    report: &AgentObservationReport,
    image_output: Option<&AgentImageOutput>,
    resource: AgentObserveResourceKind,
) -> Result<AgentObserveResourceOutput, ExitCode> {
    let resource = match resource {
        AgentObserveResourceKind::Observation => AgentObserveResourceOutput::One(Box::new(
            report
                .observation_resource()
                .map_err(|error| agent_json_error(&error))?,
        )),
        AgentObserveResourceKind::Objects => AgentObserveResourceOutput::One(Box::new(
            report
                .objects_resource()
                .map_err(|error| agent_json_error(&error))?,
        )),
        AgentObserveResourceKind::Components => AgentObserveResourceOutput::One(Box::new(
            report
                .components_resource()
                .map_err(|error| agent_json_error(&error))?,
        )),
        AgentObserveResourceKind::PresentationTree => AgentObserveResourceOutput::One(Box::new(
            report
                .presentation_tree_resource()
                .map_err(|error| agent_json_error(&error))?,
        )),
        AgentObserveResourceKind::Overlay => {
            let Some(resource) = report.overlay_svg_resource() else {
                eprintln!("error: overlay resource was not generated");
                return Err(ExitCode::from(2));
            };
            AgentObserveResourceOutput::One(Box::new(resource))
        }
        AgentObserveResourceKind::Image => {
            let Some(resource) = agent_observe_image_resource(report, image_output) else {
                eprintln!("error: --resource image requires --image");
                return Err(ExitCode::from(2));
            };
            AgentObserveResourceOutput::One(Box::new(resource))
        }
        AgentObserveResourceKind::Logs => AgentObserveResourceOutput::One(Box::new(
            report
                .logs_resource()
                .map_err(|error| agent_json_error(&error))?,
        )),
        AgentObserveResourceKind::Signals => AgentObserveResourceOutput::One(Box::new(
            report
                .signals_resource()
                .map_err(|error| agent_json_error(&error))?,
        )),
        AgentObserveResourceKind::Audio => AgentObserveResourceOutput::One(Box::new(
            report
                .audio_resource()
                .map_err(|error| agent_json_error(&error))?,
        )),
        AgentObserveResourceKind::All => {
            AgentObserveResourceOutput::Many(agent_observe_all_resources(report, image_output)?)
        }
    };
    Ok(resource)
}

pub(super) fn agent_observe_all_resources(
    report: &AgentObservationReport,
    image_output: Option<&AgentImageOutput>,
) -> Result<Vec<AgentResource>, ExitCode> {
    let mut resources = agent_observe_base_resources(report, image_output)?;
    let mut known = resources
        .iter()
        .map(|resource| resource.uri.clone())
        .collect::<BTreeSet<_>>();
    for uri in report.layers.iter().flat_map(|layer| {
        layer
            .capture_refs
            .captures
            .iter()
            .map(|capture| capture.uri.as_str())
    }) {
        if known.insert(uri.to_owned()) {
            resources.push(agent_observe_resource_by_uri(report, uri)?);
        }
    }
    for uri in report.components.iter().flat_map(|component| {
        component
            .capture_refs
            .captures
            .iter()
            .map(|capture| capture.uri.as_str())
    }) {
        if known.insert(uri.to_owned()) {
            resources.push(agent_observe_resource_by_uri(report, uri)?);
        }
    }
    for uri in report.objects.iter().flat_map(|object| {
        object
            .capture_refs
            .captures
            .iter()
            .map(|capture| capture.uri.as_str())
    }) {
        if known.insert(uri.to_owned()) {
            resources.push(agent_observe_resource_by_uri(report, uri)?);
        }
    }
    Ok(resources)
}

pub(super) fn agent_observe_list_resources(
    report: &AgentObservationReport,
    image_output: Option<&AgentImageOutput>,
) -> Result<Vec<AgentResource>, ExitCode> {
    let mut resources = agent_observe_base_resources(report, image_output)?;
    let mut known = resources
        .iter()
        .map(|resource| resource.uri.clone())
        .collect::<BTreeSet<_>>();
    for layer in &report.layers {
        for capture in &layer.capture_refs.captures {
            if known.insert(capture.uri.clone()) {
                resources.push(agent_layer_capture_ref_resource(report, layer, capture));
            }
        }
    }
    for component in &report.components {
        for capture in &component.capture_refs.captures {
            if known.insert(capture.uri.clone()) {
                resources.push(agent_component_capture_ref_resource(
                    report, component, capture,
                ));
            }
        }
    }
    for object in &report.objects {
        for capture in &object.capture_refs.captures {
            if known.insert(capture.uri.clone()) {
                resources.push(agent_object_capture_ref_resource(report, object, capture));
            }
        }
    }
    Ok(resources)
}

pub(super) fn agent_observe_base_resources(
    report: &AgentObservationReport,
    image_output: Option<&AgentImageOutput>,
) -> Result<Vec<AgentResource>, ExitCode> {
    let mut resources = vec![
        report
            .observation_resource()
            .map_err(|error| agent_json_error(&error))?,
        report
            .objects_resource()
            .map_err(|error| agent_json_error(&error))?,
        report
            .components_resource()
            .map_err(|error| agent_json_error(&error))?,
        report
            .presentation_tree_resource()
            .map_err(|error| agent_json_error(&error))?,
        report
            .logs_resource()
            .map_err(|error| agent_json_error(&error))?,
        report
            .signals_resource()
            .map_err(|error| agent_json_error(&error))?,
        report
            .audio_resource()
            .map_err(|error| agent_json_error(&error))?,
    ];
    if let Some(overlay) = report.overlay_svg_resource() {
        resources.push(overlay);
    }
    if let Some(image) = agent_observe_image_resource(report, image_output) {
        resources.push(image);
    }
    Ok(resources)
}

pub(super) fn agent_layer_capture_ref_resource(
    report: &AgentObservationReport,
    layer: &AgentObservedLayer,
    capture: &AgentLayerCaptureRef,
) -> AgentResource {
    agent_capture_ref_resource(
        report,
        AgentCaptureRefResourceSpec {
            uri: &capture.uri,
            mime_type: &capture.mime_type,
            kind: capture.kind,
            scope: AgentImageScope::Layer {
                id: layer.id.clone(),
            },
            page: capture.page,
            width: capture.width,
            height: capture.height,
            object: None,
            component: None,
            selected_capture: capture.selected_capture.clone(),
        },
    )
}

pub(super) fn agent_component_capture_ref_resource(
    report: &AgentObservationReport,
    component: &AgentObservedComponent,
    capture: &AgentComponentCaptureRef,
) -> AgentResource {
    agent_capture_ref_resource(
        report,
        AgentCaptureRefResourceSpec {
            uri: &capture.uri,
            mime_type: &capture.mime_type,
            kind: capture.kind,
            scope: AgentImageScope::Component {
                id: component.id.clone(),
            },
            page: capture.page,
            width: capture.width,
            height: capture.height,
            object: None,
            component: Some(AgentImageComponentRef::from_observed(component)),
            selected_capture: capture.selected_capture.clone(),
        },
    )
}

pub(super) fn agent_object_capture_ref_resource(
    report: &AgentObservationReport,
    object: &AgentObservedObject,
    capture: &AgentObjectCaptureRef,
) -> AgentResource {
    agent_capture_ref_resource(
        report,
        AgentCaptureRefResourceSpec {
            uri: &capture.uri,
            mime_type: &capture.mime_type,
            kind: capture.kind,
            scope: AgentImageScope::Object {
                id: object.id.clone(),
            },
            page: capture.page,
            width: capture.width,
            height: capture.height,
            object: Some(AgentImageObjectRef::from_observed(object)),
            component: None,
            selected_capture: capture.selected_capture.clone(),
        },
    )
}

pub(super) struct AgentCaptureRefResourceSpec<'a> {
    pub(super) uri: &'a str,
    pub(super) mime_type: &'a str,
    pub(super) kind: AgentImageKind,
    pub(super) scope: AgentImageScope,
    pub(super) page: usize,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) object: Option<AgentImageObjectRef>,
    pub(super) component: Option<AgentImageComponentRef>,
    pub(super) selected_capture: Option<AgentSelectedCaptureMetadata>,
}

pub(super) fn agent_capture_ref_resource(
    report: &AgentObservationReport,
    spec: AgentCaptureRefResourceSpec<'_>,
) -> AgentResource {
    AgentResource {
        uri: spec.uri.to_owned(),
        kind: AgentResourceKind::Image,
        mime_type: spec.mime_type.to_owned(),
        hash: report.render_hash.clone(),
        image: Some(AgentImageMetadata {
            kind: spec.kind,
            renderer: AgentImageRenderer::Native,
            scope: spec.scope,
            composition: spec.kind.default_capture_composition(),
            page: spec.page,
            capture_step: 0,
            capture_time_millis: report.capture_time_millis.unwrap_or_default(),
            width: spec.width,
            height: spec.height,
            crop_origin: None,
            pixel_format: (spec.mime_type == "application/octet-stream")
                .then(|| "rgba8_unorm".to_owned()),
            row_stride_bytes: (spec.mime_type == "application/octet-stream")
                .then(|| spec.width.saturating_mul(4)),
            content_bbox: None,
            content_viewport_bbox: None,
            content_pixels: None,
            object: spec.object,
            component: spec.component,
            selected_capture: spec.selected_capture,
            diagnostics: Vec::new(),
        }),
        body: AgentResourceBody::Text(String::new()),
    }
}

pub(super) fn agent_observe_image_resource(
    report: &AgentObservationReport,
    image_output: Option<&AgentImageOutput>,
) -> Option<AgentResource> {
    let image = report.images.first()?;
    let output = image_output?;
    if image.uri != output.uri {
        return None;
    }
    Some(report.image_resource(image, &output.bytes))
}

pub(super) fn agent_observe_cached_image_resource(
    report: &AgentObservationReport,
    image_output: Option<&AgentImageOutput>,
    uri: &str,
) -> Option<AgentResource> {
    let output = image_output?;
    if output.uri != uri {
        return None;
    }
    let image = report.images.iter().find(|image| image.uri == uri)?;
    Some(report.image_resource(image, &output.bytes))
}

pub(super) fn agent_json_error(error: &serde_json::Error) -> ExitCode {
    eprintln!("error: failed to build agent resource JSON: {error}");
    ExitCode::FAILURE
}
