use crate::geometry::{AgentBBox, AgentPoint};
use crate::image::{
    AgentImageAlignment, AgentImageFit, AgentImageObjectParam, AgentImageTransform,
    AgentLayerCaptureRefs, AgentObjectCaptureRefs, AgentViewCaptureRefs,
};
use crate::proxy::AgentPresentationObjectProxyRef;
use crate::rich_text::AgentRichTextElementRef;
use crate::serde_helpers::default_true;
use crate::text_context::{
    AgentTextContextCatalog, AgentTextContextError, AgentTextContextReference,
};
use arcweft_layout::stage_placement::{ResolvedStagePlacement, StagePlacement};
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

/// Stable view boundary known to the Agent observer.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentObservedView {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub visible: bool,
    pub bbox: AgentBBox,
    pub object_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_refs: Vec<String>,
    pub capture_refs: AgentViewCaptureRefs,
}

/// Visible object or View element known to the Agent observer.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_context: Option<AgentTextContextReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rich_text_ref: Option<AgentRichTextElementRef>,
    pub content: AgentObservedObjectContent,
}

/// Renderable payload carried by an observed presentation object.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentObservedObjectContent {
    RichText(AgentTextObjectContent),
    ViewText(AgentTextObjectContent),
    Image(Box<AgentObservedImageContent>),
    Custom { object_type: String },
}

/// Strict text-family marker; the issued catalog owns the complete context.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentTextObjectContent {}

impl AgentObservedObjectContent {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::RichText(_) => "rich_text",
            Self::ViewText(_) => "view_text",
            Self::Image(_) => "image",
            Self::Custom { .. } => "custom",
        }
    }

    pub const fn requires_visual_policy(&self) -> bool {
        matches!(self, Self::Image(_) | Self::Custom { .. })
    }

    pub const fn requires_text_context(&self) -> bool {
        matches!(self, Self::RichText(_) | Self::ViewText(_))
    }

    pub(crate) fn validate_text_context_family(
        &self,
        object_id: &str,
        reference: Option<&AgentTextContextReference>,
        catalog: &AgentTextContextCatalog,
    ) -> Result<(), AgentTextContextError> {
        let Some(reference) = reference else {
            return if self.requires_text_context() {
                Err(AgentTextContextError::MissingObjectContext {
                    object_id: object_id.to_owned(),
                })
            } else {
                Ok(())
            };
        };
        match self {
            Self::RichText(_) | Self::ViewText(_) => {
                let is_stage = catalog
                    .resolve(reference)?
                    .content
                    .display_stage()
                    .is_some();
                if matches!(self, Self::RichText(_)) == is_stage {
                    Ok(())
                } else {
                    Err(AgentTextContextError::ContentFamily {
                        owner: reference.owner(),
                    })
                }
            }
            Self::Image(_) | Self::Custom { .. } => Err(AgentTextContextError::ObjectOwner {
                object_id: object_id.to_owned(),
            }),
        }
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
    pub fn validate_text_context(
        &self,
        catalog: &AgentTextContextCatalog,
    ) -> Result<(), AgentTextContextError> {
        if self.rich_text_ref.is_some() && self.text_context.is_none() {
            return Err(AgentTextContextError::MissingObjectContext {
                object_id: self.id.clone(),
            });
        }
        self.content
            .validate_text_context_family(&self.id, self.text_context.as_ref(), catalog)?;
        if let Some(reference) = &self.text_context {
            let entry = catalog.resolve(reference)?;
            if self.parent_id != entry.element_parent_id(self.rich_text_ref.as_ref())? {
                return Err(AgentTextContextError::ObjectOwner {
                    object_id: self.id.clone(),
                });
            }
            catalog.validate_element(
                &self.id,
                reference,
                self.rich_text_ref.as_ref(),
                self.text.as_deref(),
            )?;
            for capture in &self.capture_refs.captures {
                if capture.page
                    != entry
                        .content
                        .display_stage()
                        .map_or(0, |stage| stage.frame().page_index)
                    || capture.selected_capture.as_ref().is_some_and(|selected| {
                        selected.source
                            != crate::image::AgentCaptureSourceIdentity::from_object(self)
                    })
                {
                    return Err(AgentTextContextError::ObjectOwner {
                        object_id: self.id.clone(),
                    });
                }
            }
        }
        Ok(())
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
