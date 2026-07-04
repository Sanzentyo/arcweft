use crate::geometry::{AgentBBox, AgentPoint};
use crate::image::{
    AgentComponentCaptureRefs, AgentImageAlignment, AgentImageFit, AgentImageObjectContentRef,
    AgentImageObjectParam, AgentImageTransform, AgentLayerCaptureRefs, AgentObjectCaptureRefs,
};
use crate::proxy::AgentPresentationObjectProxyRef;
use crate::rich_text::AgentRichTextElementRef;
use crate::serde_helpers::default_true;
use arcweft_layout::stage_placement::{ResolvedStagePlacement, StagePlacement};
use arcweft_render_text::LineDisplayFrame;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Visible rendering layer known to the Agent observer.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentObservedLayer {
    pub id: String,
    pub visible: bool,
    pub bbox: AgentBBox,
    pub object_count: usize,
    pub capture_refs: AgentLayerCaptureRefs,
}

/// Stable component boundary known to the Agent observer.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentObservedComponent {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub visible: bool,
    pub bbox: AgentBBox,
    pub object_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_refs: Vec<String>,
    pub capture_refs: AgentComponentCaptureRefs,
}

/// Visible object or UI element known to the Agent observer.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentObservedObject {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    pub layer: String,
    pub role: String,
    pub visible: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub bbox: AgentBBox,
    pub polygon: Vec<AgentPoint>,
    pub capture_refs: AgentObjectCaptureRefs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_layer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_depth: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rich_text_ref: Option<AgentRichTextElementRef>,
    pub content: AgentObservedObjectContent,
}

/// Renderable payload carried by an observed presentation object.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentObservedObjectContent {
    RichText { frame: Box<LineDisplayFrame> },
    Image(Box<AgentObservedImageContent>),
    Custom { object_type: String },
}

impl AgentObservedObjectContent {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::RichText { .. } => "rich_text",
            Self::Image(_) => "image",
            Self::Custom { .. } => "custom",
        }
    }

    /// Text that must be evaluated before this object is published to an Agent.
    pub fn policy_text(&self) -> Option<&str> {
        match self {
            Self::RichText { frame } => Some(frame.text.as_str()),
            Self::Image(_) | Self::Custom { .. } => None,
        }
    }

    pub const fn requires_visual_policy(&self) -> bool {
        matches!(self, Self::Image(_) | Self::Custom { .. })
    }
}

/// Image-specific payload for an observed image presentation object.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentObservedImageContent {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_time_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity_milli: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<AgentImageFit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<AgentImageAlignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<AgentImageTransform>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authored_placement: Option<StagePlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_placement: Option<ResolvedStagePlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intrinsic_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intrinsic_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, AgentImageObjectParam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proxies: Vec<AgentPresentationObjectProxyRef>,
}

impl AgentObservedObject {
    pub fn rich_text_frame(&self) -> Option<&LineDisplayFrame> {
        match &self.content {
            AgentObservedObjectContent::RichText { frame } => Some(frame.as_ref()),
            AgentObservedObjectContent::Image(_) | AgentObservedObjectContent::Custom { .. } => {
                None
            }
        }
    }

    pub fn image_content_ref(&self) -> Option<AgentImageObjectContentRef> {
        match &self.content {
            AgentObservedObjectContent::Image(content) => {
                Some(AgentImageObjectContentRef::from(content.as_ref()))
            }
            AgentObservedObjectContent::RichText { .. }
            | AgentObservedObjectContent::Custom { .. } => None,
        }
    }

    pub fn resolved_object_layer(&self) -> Option<String> {
        self.object_layer.clone().or_else(|| {
            self.rich_text_ref
                .as_ref()
                .and_then(|rich_text_ref| rich_text_ref.object_layer.clone())
        })
    }

    pub fn resolved_object_depth(&self) -> Option<i32> {
        self.object_depth.or_else(|| {
            self.rich_text_ref
                .as_ref()
                .and_then(|rich_text_ref| rich_text_ref.object_depth)
        })
    }
}
