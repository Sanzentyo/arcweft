use crate::action::AgentActionTarget;
use crate::diagnostic::AgentDiagnostic;
use crate::geometry::AgentViewport;
use crate::ids::AgentResourceUri;
use crate::image::AgentImageResource;
use crate::object::{AgentObservedLayer, AgentObservedObject, AgentObservedView};
use crate::presentation::{AgentPresentationTree, AgentPresentationTreeQuery};
use crate::resource::{
    AgentBinaryResourceBody, AgentResource, AgentResourceBody, AgentResourceKind,
};
use crate::session::{AgentAssignment, AgentAudioState};
use crate::view::{AgentObservedScrollRegion, AgentObservedVirtualList, AgentViewTree};
use arcweft_core::effect::{RuntimeEvent, RuntimeLog};
use serde::{Deserialize, Serialize};

/// One Agent Debug Bus observation frame.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentObservationReport {
    pub status: String,
    pub session_id: String,
    pub tick: usize,
    pub frame_id: String,
    pub state_hash: String,
    pub render_hash: String,
    pub source: String,
    pub viewport: AgentViewport,
    pub images: Vec<AgentImageResource>,
    pub layers: Vec<AgentObservedLayer>,
    pub views: Vec<AgentObservedView>,
    pub objects: Vec<AgentObservedObject>,
    pub text_contexts: crate::text_context::AgentTextContextCatalog,
    pub presentation_tree: AgentPresentationTree,
    pub actions: Vec<AgentActionTarget>,
    /// Authored Scroll targets with non-actionable viewport/content parts.
    pub scroll_regions: Vec<AgentObservedScrollRegion>,
    /// Stable range tables for mounted lazy lists, including off-window items.
    pub virtual_lists: Vec<AgentObservedVirtualList>,
    pub view_tree: AgentViewTree,
    pub scene_graph: Vec<serde_json::Value>,
    pub audio_state: AgentAudioState,
    pub logs: Vec<RuntimeLog>,
    pub signals: Vec<AgentAssignment>,
    pub metrics: Vec<AgentAssignment>,
    pub events: Vec<RuntimeEvent>,
    pub diagnostics: Vec<AgentDiagnostic>,
    pub steps: usize,
    pub capture_time_millis: Option<u32>,
    pub task_requests: usize,
    pub final_status: String,
    pub overlay_svg: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(remote = "AgentObservationReport")]
struct AgentObservationReportWire {
    status: String,
    session_id: String,
    tick: usize,
    frame_id: String,
    state_hash: String,
    render_hash: String,
    source: String,
    viewport: AgentViewport,
    images: Vec<AgentImageResource>,
    layers: Vec<AgentObservedLayer>,
    views: Vec<AgentObservedView>,
    objects: Vec<AgentObservedObject>,
    text_contexts: crate::text_context::AgentTextContextCatalog,
    presentation_tree: AgentPresentationTree,
    actions: Vec<AgentActionTarget>,
    /// Authored Scroll targets with non-actionable viewport/content parts.
    scroll_regions: Vec<AgentObservedScrollRegion>,
    /// Stable range tables for mounted lazy lists, including off-window items.
    virtual_lists: Vec<AgentObservedVirtualList>,
    view_tree: AgentViewTree,
    scene_graph: Vec<serde_json::Value>,
    audio_state: AgentAudioState,
    logs: Vec<RuntimeLog>,
    signals: Vec<AgentAssignment>,
    metrics: Vec<AgentAssignment>,
    events: Vec<RuntimeEvent>,
    diagnostics: Vec<AgentDiagnostic>,
    steps: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    capture_time_millis: Option<u32>,
    task_requests: usize,
    final_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    overlay_svg: Option<String>,
}

impl Serialize for AgentObservationReport {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.validate_text_contexts()
            .map_err(serde::ser::Error::custom)?;
        AgentObservationReportWire::serialize(self, serializer)
    }
}
impl<'de> Deserialize<'de> for AgentObservationReport {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let report = AgentObservationReportWire::deserialize(deserializer)?;
        report
            .validate_text_contexts()
            .map_err(serde::de::Error::custom)?;
        Ok(report)
    }
}

impl AgentObservationReport {
    pub fn decode_json(bytes: &[u8]) -> Result<Self, crate::text_context::AgentTextContextError> {
        crate::text_context::decode_text_context_envelope(bytes)
    }

    pub fn validate_text_contexts(&self) -> Result<(), crate::text_context::AgentTextContextError> {
        let scope = self.text_contexts.scope();
        if scope.session_id != self.session_id
            || scope.frame_id != self.frame_id
            || scope.tick != self.tick
            || scope.capture_time_millis != self.capture_time_millis
        {
            return Err(crate::text_context::AgentTextContextError::ScopeMismatch);
        }
        self.text_contexts.validate_objects(&self.objects)?;
        for image in &self.images {
            crate::image::validate_agent_image_format(
                image.pixel_format,
                &image.mime_type,
                image.width,
                image.height,
            )?;
            if let Some(object) = &image.object {
                object.validate_text_context(&self.text_contexts)?;
            }
        }
        crate::text_context::validate_text_context_envelope(|writer| {
            AgentObservationReportWire::serialize(self, &mut serde_json::Serializer::new(writer))
        })
    }
    /// Builds the MCP-style latest observation JSON resource.
    pub fn observation_resource(&self) -> Result<AgentResource, serde_json::Error> {
        Ok(AgentResource {
            uri: resource_uri(format!(
                "arcweft://session/{}/observation/latest.json",
                self.session_id
            )),
            kind: AgentResourceKind::ObservationLatest,
            mime_type: "application/json".to_owned(),
            hash: self.state_hash.clone(),
            image: None,
            body: AgentResourceBody::Json(serde_json::to_value(self)?),
        })
    }

    /// Builds the MCP-style observed objects JSON resource.
    pub fn objects_resource(&self) -> Result<AgentResource, serde_json::Error> {
        Ok(AgentResource {
            uri: resource_uri(format!(
                "arcweft://session/{}/frame/{}/objects.json",
                self.session_id, self.tick
            )),
            kind: AgentResourceKind::Objects,
            mime_type: "application/json".to_owned(),
            hash: self.render_hash.clone(),
            image: None,
            body: AgentResourceBody::Json(serde_json::to_value(AgentObjectsReport {
                text_contexts: self.text_contexts.clone(),
                objects: self.objects.clone(),
            })?),
        })
    }

    /// Builds the MCP-style observed views JSON resource.
    pub fn views_resource(&self) -> Result<AgentResource, serde_json::Error> {
        Ok(AgentResource {
            uri: resource_uri(format!(
                "arcweft://session/{}/frame/{}/views.json",
                self.session_id, self.tick
            )),
            kind: AgentResourceKind::Views,
            mime_type: "application/json".to_owned(),
            hash: self.render_hash.clone(),
            image: None,
            body: AgentResourceBody::Json(serde_json::to_value(&self.views)?),
        })
    }

    /// Builds the MCP-style presentation object tree JSON resource.
    pub fn presentation_tree_resource(&self) -> Result<AgentResource, serde_json::Error> {
        self.presentation_tree_resource_with_tree(
            resource_uri(format!(
                "arcweft://session/{}/frame/{}/presentation-tree.json",
                self.session_id, self.tick
            )),
            &self.presentation_tree,
        )
    }

    /// Builds the MCP-style presentation object tree JSON resource with a typed filter.
    pub fn filtered_presentation_tree_resource(
        &self,
        uri: AgentResourceUri,
        query: &AgentPresentationTreeQuery,
    ) -> Result<AgentResource, serde_json::Error> {
        let tree = self.presentation_tree.filtered(query);
        self.presentation_tree_resource_with_tree(uri, &tree)
    }

    fn presentation_tree_resource_with_tree(
        &self,
        uri: AgentResourceUri,
        tree: &AgentPresentationTree,
    ) -> Result<AgentResource, serde_json::Error> {
        Ok(AgentResource {
            uri,
            kind: AgentResourceKind::PresentationTree,
            mime_type: "application/json".to_owned(),
            hash: self.render_hash.clone(),
            image: None,
            body: AgentResourceBody::Json(serde_json::to_value(tree)?),
        })
    }

    /// Builds the MCP-style overlay SVG resource when the observation embeds one.
    pub fn overlay_svg_resource(&self) -> Option<AgentResource> {
        self.overlay_svg.as_ref().map(|overlay| AgentResource {
            uri: resource_uri(format!(
                "arcweft://session/{}/frame/{}/overlay.svg",
                self.session_id, self.tick
            )),
            kind: AgentResourceKind::OverlaySvg,
            mime_type: "image/svg+xml".to_owned(),
            hash: self.render_hash.clone(),
            image: None,
            body: AgentResourceBody::Text(overlay.clone()),
        })
    }

    /// Builds an MCP-style image resource for an image listed in this observation.
    pub fn image_resource(
        &self,
        image: &AgentImageResource,
        body: AgentBinaryResourceBody,
    ) -> Result<AgentResource, serde_json::Error> {
        if let Some(object) = &image.object {
            object
                .validate_text_context(&self.text_contexts)
                .map_err(text_context_json_error)?;
        }
        let contexts = image
            .object
            .as_ref()
            .and_then(|object| object.text_context.as_ref())
            .map(|reference| self.text_contexts.selected(std::iter::once(reference)))
            .transpose()
            .map_err(text_context_json_error)?;
        Ok(AgentResource {
            uri: resource_uri(image.uri.clone()),
            kind: AgentResourceKind::Image,
            mime_type: image.mime_type.clone(),
            hash: image.hash.clone(),
            image: Some(
                crate::image::AgentImageMetadata::from_image_resource(image, contexts)
                    .map_err(|error| <serde_json::Error as serde::ser::Error>::custom(error))?,
            ),
            body: AgentResourceBody::BytesBase64(body),
        })
    }

    /// Builds the MCP-style signals JSON resource.
    pub fn signals_resource(&self) -> Result<AgentResource, serde_json::Error> {
        Ok(AgentResource {
            uri: resource_uri(format!(
                "arcweft://session/{}/signals.json",
                self.session_id
            )),
            kind: AgentResourceKind::Signals,
            mime_type: "application/json".to_owned(),
            hash: self.state_hash.clone(),
            image: None,
            body: AgentResourceBody::Json(serde_json::to_value(&self.signals)?),
        })
    }

    /// Builds the MCP-style audio JSON resource.
    pub fn audio_resource(&self) -> Result<AgentResource, serde_json::Error> {
        Ok(AgentResource {
            uri: resource_uri(format!("arcweft://session/{}/audio.json", self.session_id)),
            kind: AgentResourceKind::Audio,
            mime_type: "application/json".to_owned(),
            hash: self.state_hash.clone(),
            image: None,
            body: AgentResourceBody::Json(serde_json::to_value(&self.audio_state)?),
        })
    }

    /// Builds the MCP-style log stream resource as newline-delimited JSON.
    pub fn logs_resource(&self) -> Result<AgentResource, serde_json::Error> {
        let mut lines = self
            .logs
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()?
            .join("\n");
        if !lines.is_empty() {
            lines.push('\n');
        }
        Ok(AgentResource {
            uri: resource_uri(format!("arcweft://session/{}/logs.ndjson", self.session_id)),
            kind: AgentResourceKind::Logs,
            mime_type: "application/x-ndjson".to_owned(),
            hash: self.state_hash.clone(),
            image: None,
            body: AgentResourceBody::Text(lines),
        })
    }
}

fn resource_uri(value: String) -> AgentResourceUri {
    AgentResourceUri::new(value).expect("generated observation resource URI is nonempty")
}

fn text_context_json_error(error: crate::text_context::AgentTextContextError) -> serde_json::Error {
    <serde_json::Error as serde::ser::Error>::custom(error)
}

/// Self-contained observed Objects resource body.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentObjectsReport {
    pub text_contexts: crate::text_context::AgentTextContextCatalog,
    pub objects: Vec<AgentObservedObject>,
}
#[derive(Deserialize, Serialize)]
#[serde(remote = "AgentObjectsReport", deny_unknown_fields)]
struct AgentObjectsReportWire {
    text_contexts: crate::text_context::AgentTextContextCatalog,
    objects: Vec<AgentObservedObject>,
}
impl Serialize for AgentObjectsReport {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.validate_text_contexts()
            .map_err(serde::ser::Error::custom)?;
        AgentObjectsReportWire::serialize(self, serializer)
    }
}
impl AgentObjectsReport {
    /// Validate exact object contexts and the complete compact JSON envelope.
    pub fn validate_text_contexts(&self) -> Result<(), crate::text_context::AgentTextContextError> {
        self.text_contexts.validate_objects(&self.objects)?;
        crate::text_context::validate_text_context_envelope(|writer| {
            AgentObjectsReportWire::serialize(self, &mut serde_json::Serializer::new(writer))
        })
    }

    pub fn decode_json(bytes: &[u8]) -> Result<Self, crate::text_context::AgentTextContextError> {
        crate::text_context::decode_text_context_envelope(bytes)
    }
}
impl<'de> Deserialize<'de> for AgentObjectsReport {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let report = AgentObjectsReportWire::deserialize(deserializer)?;
        report
            .validate_text_contexts()
            .map_err(serde::de::Error::custom)?;
        Ok(report)
    }
}
