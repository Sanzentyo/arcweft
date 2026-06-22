use crate::{AgentPolicySummary, PublishedAgentResource};
use arcweft_agent_protocol::resource::{
    AgentBinaryEncoding, AgentBinaryResourceBody, AgentResource, AgentResourceBody,
};
use arcweft_content_policy::{
    PolicyOutcome, PolicyPublication, PolicyReceipt, RenderSampleKind, RenderedScene,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;

/// One safe rendered-scene view and its Agent image resource.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PublishedAgentSceneView {
    pub view_index: usize,
    pub sample_kind: RenderSampleKind,
    pub sample_index: u32,
    pub resource: PublishedAgentResource,
}

/// Multi-view policy result for a 3D model or composed scene.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PublishedAgentScene {
    pub views: Vec<PublishedAgentSceneView>,
    pub receipt: PolicyReceipt,
}

impl PublishedAgentScene {
    pub(crate) fn from_outcome(
        outcome: PolicyOutcome<RenderedScene>,
        templates: &[(String, AgentResource)],
    ) -> Result<Self, crate::AgentPolicyError> {
        let receipt = outcome.receipt;
        let summary = AgentPolicySummary::from_receipt(&receipt);
        let sanitized = receipt.sanitized;
        let views = match outcome.publication {
            PolicyPublication::Allowed { value } | PolicyPublication::Sanitized { value } => value
                .views
                .into_iter()
                .enumerate()
                .map(|(view_index, view)| {
                    let (_, mut resource) = templates
                        .iter()
                        .find(|(view_id, _)| view_id == &view.id)
                        .cloned()
                        .ok_or_else(|| {
                            crate::AgentPolicyError::UnsupportedImageEncoding(format!(
                                "missing resource template for scene view {}",
                                view.id
                            ))
                        })?;
                    let mut bytes = Vec::new();
                    {
                        let mut encoder =
                            png::Encoder::new(&mut bytes, view.color.width(), view.color.height());
                        encoder.set_color(png::ColorType::Rgba);
                        encoder.set_depth(png::BitDepth::Eight);
                        let mut writer = encoder.write_header()?;
                        writer.write_image_data(view.color.pixels())?;
                    }
                    let view_token = summary.opaque_child_token("scene-view", view_index);
                    "image/png".clone_into(&mut resource.mime_type);
                    resource.hash = blake3::hash(&bytes).to_hex().to_string();
                    resource.uri = summary.moderated_child_uri("scene-view", view_index, "png");
                    resource.body = AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
                        encoding: AgentBinaryEncoding::Base64,
                        data: STANDARD.encode(bytes),
                    });
                    if let Some(metadata) = resource.image.as_mut() {
                        metadata.width = view.color.width();
                        metadata.height = view.color.height();
                        metadata.composition = if sanitized {
                            metadata.composition.after_policy_mask()
                        } else {
                            metadata.composition
                        };
                        metadata.pixel_format = None;
                        metadata.row_stride_bytes = None;
                        metadata.scrub_for_external_publication(&view_token);
                    }
                    Ok(PublishedAgentSceneView {
                        view_index,
                        sample_kind: view.sample_kind,
                        sample_index: view.sample_index,
                        resource: PublishedAgentResource::new(resource, summary.clone()),
                    })
                })
                .collect::<Result<Vec<_>, crate::AgentPolicyError>>()?,
            PolicyPublication::Withheld { .. } => Vec::new(),
        };
        Ok(Self { views, receipt })
    }
}
