use crate::geometry::AgentViewport;
use crate::image::{AgentImageObjectRef, AgentObjectCaptureRefs};
use crate::rich_text::{AgentHitRegion, AgentRichTextElementRef};
use serde::{Deserialize, Serialize};

/// Result of hit-testing observed Agent objects at a viewport coordinate.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentHitTestReport {
    pub status: String,
    pub session_id: String,
    pub frame_id: String,
    pub source: String,
    pub viewport: AgentViewport,
    pub x: u32,
    pub y: u32,
    pub top_object_id: Option<String>,
    pub hits: Vec<AgentHitTestHit>,
    pub text_contexts: crate::text_context::AgentTextContextCatalog,
}
#[derive(Deserialize, Serialize)]
#[serde(remote = "AgentHitTestReport")]
struct AgentHitTestReportWire {
    status: String,
    session_id: String,
    frame_id: String,
    source: String,
    viewport: AgentViewport,
    x: u32,
    y: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    top_object_id: Option<String>,
    hits: Vec<AgentHitTestHit>,
    text_contexts: crate::text_context::AgentTextContextCatalog,
}

/// One observed object/region hit by an Agent hit-test query.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentHitTestHit {
    pub rank: usize,
    pub object_id: String,
    pub object: AgentImageObjectRef,
    pub layer: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub bbox: crate::geometry::AgentBBox,
    pub polygon: Vec<crate::geometry::AgentPoint>,
    pub capture_refs: AgentObjectCaptureRefs,
    pub region: AgentHitRegion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rich_text_ref: Option<AgentRichTextElementRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i32>,
}

impl AgentHitTestReport {
    pub fn decode_json(bytes: &[u8]) -> Result<Self, crate::text_context::AgentTextContextError> {
        crate::text_context::decode_text_context_envelope(bytes)
    }

    pub fn validate_text_contexts(&self) -> Result<(), crate::text_context::AgentTextContextError> {
        let scope = self.text_contexts.scope();
        if scope.session_id != self.session_id || scope.frame_id != self.frame_id {
            return Err(crate::text_context::AgentTextContextError::ScopeMismatch);
        }
        for hit in &self.hits {
            hit.object.validate_text_context(&self.text_contexts)?;
            let layer = hit
                .region
                .proxy_layer
                .as_ref()
                .or(hit.object.object_layer.as_ref())
                .or_else(|| {
                    hit.rich_text_ref
                        .as_ref()
                        .and_then(|element| element.object_layer.as_ref())
                })
                .unwrap_or(&hit.object.layer);
            let region_matches = match &hit.rich_text_ref {
                Some(element) => element.hit_test && element.hit_regions.contains(&hit.region),
                None => {
                    hit.region
                        == AgentHitRegion::for_object(
                            &hit.object.bbox,
                            hit.object.object_layer.as_deref(),
                            hit.object.object_depth,
                        )
                }
            };
            let depth = hit.region.depth.or(hit.object.object_depth).or_else(|| {
                hit.rich_text_ref
                    .as_ref()
                    .and_then(|element| element.object_depth)
            });
            if hit.object.text_context.is_some()
                && (hit.object_id != hit.object.id
                    || &hit.layer != layer
                    || hit.role != hit.object.role
                    || hit.text != hit.object.text
                    || hit.bbox != hit.object.bbox
                    || hit.polygon != hit.object.polygon
                    || hit.capture_refs != hit.object.capture_refs
                    || hit.depth != depth
                    || hit.rich_text_ref != hit.object.rich_text_ref
                    || !region_matches)
            {
                return Err(crate::text_context::AgentTextContextError::ObjectOwner {
                    object_id: hit.object_id.clone(),
                });
            }
        }
        crate::text_context::validate_text_context_envelope(|writer| {
            AgentHitTestReportWire::serialize(self, &mut serde_json::Serializer::new(writer))
        })
    }
}
impl Serialize for AgentHitTestReport {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.validate_text_contexts()
            .map_err(serde::ser::Error::custom)?;
        AgentHitTestReportWire::serialize(self, serializer)
    }
}
impl<'de> Deserialize<'de> for AgentHitTestReport {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let report = AgentHitTestReportWire::deserialize(deserializer)?;
        report
            .validate_text_contexts()
            .map_err(serde::de::Error::custom)?;
        Ok(report)
    }
}
