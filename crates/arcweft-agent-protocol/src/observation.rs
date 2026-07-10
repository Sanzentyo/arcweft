use crate::action::AgentActionTarget;
use crate::diagnostic::AgentDiagnostic;
use crate::geometry::AgentViewport;
use crate::image::AgentImageResource;
use crate::object::{AgentObservedLayer, AgentObservedObject, AgentObservedView};
use crate::presentation::{AgentPresentationTree, AgentPresentationTreeQuery};
use crate::resource::{
    AgentBinaryEncoding, AgentBinaryResourceBody, AgentResource, AgentResourceBody,
    AgentResourceKind,
};
use crate::session::{AgentAssignment, AgentAudioState};
use crate::view::{AgentObservedScrollRegion, AgentViewTree};
use arcweft_core::effect::{RuntimeEvent, RuntimeLog};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

/// One Agent Debug Bus observation frame.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
    pub presentation_tree: AgentPresentationTree,
    pub actions: Vec<AgentActionTarget>,
    /// Authored Scroll targets with non-actionable viewport/content parts.
    pub scroll_regions: Vec<AgentObservedScrollRegion>,
    pub view_tree: AgentViewTree,
    pub scene_graph: Vec<serde_json::Value>,
    pub audio_state: AgentAudioState,
    pub logs: Vec<RuntimeLog>,
    pub signals: Vec<AgentAssignment>,
    pub metrics: Vec<AgentAssignment>,
    pub events: Vec<RuntimeEvent>,
    pub diagnostics: Vec<AgentDiagnostic>,
    pub steps: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_time_millis: Option<u32>,
    pub task_requests: usize,
    pub final_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay_svg: Option<String>,
}

impl AgentObservationReport {
    /// Builds the MCP-style latest observation JSON resource.
    pub fn observation_resource(&self) -> Result<AgentResource, serde_json::Error> {
        Ok(AgentResource {
            uri: format!(
                "arcweft://session/{}/observation/latest.json",
                self.session_id
            ),
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
            uri: format!(
                "arcweft://session/{}/frame/{}/objects.json",
                self.session_id, self.tick
            ),
            kind: AgentResourceKind::Objects,
            mime_type: "application/json".to_owned(),
            hash: self.render_hash.clone(),
            image: None,
            body: AgentResourceBody::Json(serde_json::to_value(&self.objects)?),
        })
    }

    /// Builds the MCP-style observed views JSON resource.
    pub fn views_resource(&self) -> Result<AgentResource, serde_json::Error> {
        Ok(AgentResource {
            uri: format!(
                "arcweft://session/{}/frame/{}/views.json",
                self.session_id, self.tick
            ),
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
            format!(
                "arcweft://session/{}/frame/{}/presentation-tree.json",
                self.session_id, self.tick
            ),
            &self.presentation_tree,
        )
    }

    /// Builds the MCP-style presentation object tree JSON resource with a typed filter.
    pub fn filtered_presentation_tree_resource(
        &self,
        uri: String,
        query: &AgentPresentationTreeQuery,
    ) -> Result<AgentResource, serde_json::Error> {
        let tree = self.presentation_tree.filtered(query);
        self.presentation_tree_resource_with_tree(uri, &tree)
    }

    fn presentation_tree_resource_with_tree(
        &self,
        uri: String,
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
            uri: format!(
                "arcweft://session/{}/frame/{}/overlay.svg",
                self.session_id, self.tick
            ),
            kind: AgentResourceKind::OverlaySvg,
            mime_type: "image/svg+xml".to_owned(),
            hash: self.render_hash.clone(),
            image: None,
            body: AgentResourceBody::Text(overlay.clone()),
        })
    }

    /// Builds an MCP-style image resource body for an image listed in this observation.
    pub fn image_resource(&self, image: &AgentImageResource, bytes: &[u8]) -> AgentResource {
        AgentResource {
            uri: image.uri.clone(),
            kind: AgentResourceKind::Image,
            mime_type: image.mime_type.clone(),
            hash: image.hash.clone(),
            image: Some(crate::image::AgentImageMetadata::from_image_resource(image)),
            body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
                encoding: AgentBinaryEncoding::Base64,
                data: STANDARD.encode(bytes),
            }),
        }
    }

    /// Builds the MCP-style signals JSON resource.
    pub fn signals_resource(&self) -> Result<AgentResource, serde_json::Error> {
        Ok(AgentResource {
            uri: format!("arcweft://session/{}/signals.json", self.session_id),
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
            uri: format!("arcweft://session/{}/audio.json", self.session_id),
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
            uri: format!("arcweft://session/{}/logs.ndjson", self.session_id),
            kind: AgentResourceKind::Logs,
            mime_type: "application/x-ndjson".to_owned(),
            hash: self.state_hash.clone(),
            image: None,
            body: AgentResourceBody::Text(lines),
        })
    }
}
