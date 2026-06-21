use crate::geometry::AgentViewport;
use crate::image::{AgentImageObjectRef, AgentObjectCaptureRefs};
use crate::rich_text::{AgentHitRegion, AgentRichTextElementRef};
use serde::{Deserialize, Serialize};

/// Result of hit-testing observed Agent objects at a viewport coordinate.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentHitTestReport {
    pub status: String,
    pub session_id: String,
    pub frame_id: String,
    pub source: String,
    pub viewport: AgentViewport,
    pub x: u32,
    pub y: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_object_id: Option<String>,
    pub hits: Vec<AgentHitTestHit>,
}

/// One observed object/region hit by an Agent hit-test query.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
