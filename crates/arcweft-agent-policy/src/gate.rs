use crate::AgentPublicationPolicy;
use crate::decode::decode_agent_image;
use crate::published::{AgentPolicySummary, PublishedAgentResource};
use arcweft_agent_protocol::resource::{
    AgentBinaryEncoding, AgentBinaryResourceBody, AgentResource, AgentResourceBody,
};
use arcweft_content_policy::{
    ContentDigest, ContentId, ContentPolicyEngine, PolicyDecision, PolicyDisposition, PolicyError,
    PolicyPublication, PolicyReceipt, TextArtifact,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;
use thiserror::Error;

/// Error at the Agent publication boundary.
#[derive(Debug, Error)]
pub enum AgentPolicyError {
    #[error(transparent)]
    Policy(#[from] PolicyError),
    #[error(transparent)]
    Base64(#[from] base64::DecodeError),
    #[error(transparent)]
    Image(#[from] arcweft_image::ImageError),
    #[error(transparent)]
    Png(#[from] png::EncodingError),
    #[error("image resource has no image metadata")]
    MissingImageMetadata,
    #[error("image resource has no binary body")]
    MissingImageBytes,
    #[error("decoded image has no frame")]
    MissingImageFrame,
    #[error("unsupported Agent image encoding `{0}`")]
    UnsupportedImageEncoding(String),
    #[error("failed to serialize a JSON policy projection: {0}")]
    Json(#[from] serde_json::Error),
}

/// Mandatory content-policy gate for Agent resources.
#[derive(Clone, Debug)]
pub struct AgentContentPolicyGate<C> {
    engine: ContentPolicyEngine<C>,
    publication: AgentPublicationPolicy,
}

impl AgentContentPolicyGate<arcweft_content_policy::RuleClassifier> {
    /// Strict built-in profile. Text rules are active; image and scene content
    /// fail closed until an embedded visual model is supplied.
    pub fn strict_builtin() -> Self {
        Self::new(ContentPolicyEngine::new(
            arcweft_content_policy::RuleClassifier::strict_builtin(),
            arcweft_content_policy::PolicyProfile::strict_default(),
        ))
    }
}

impl<C> AgentContentPolicyGate<C>
where
    C: arcweft_content_policy::ContentClassifier,
{
    pub fn new(engine: ContentPolicyEngine<C>) -> Self {
        Self {
            engine,
            publication: AgentPublicationPolicy::strict_default(),
        }
    }

    pub const fn with_publication_policy(
        engine: ContentPolicyEngine<C>,
        publication: AgentPublicationPolicy,
    ) -> Self {
        Self {
            engine,
            publication,
        }
    }

    pub const fn engine(&self) -> &ContentPolicyEngine<C> {
        &self.engine
    }

    /// Consumes a raw resource and returns only a policy-safe publication.
    pub fn publish(
        &self,
        mut resource: AgentResource,
    ) -> Result<PublishedAgentResource, AgentPolicyError> {
        if resource.kind == arcweft_agent_protocol::resource::AgentResourceKind::Image {
            return self.publish_image(resource);
        }
        let body = std::mem::replace(&mut resource.body, AgentResourceBody::Text(String::new()));
        match body {
            AgentResourceBody::Text(text) => self.publish_text(resource, text),
            AgentResourceBody::Json(value) => self.publish_json(resource, value),
            AgentResourceBody::BytesBase64(body) => Ok(self.withheld_resource(
                AgentResource {
                    body: AgentResourceBody::BytesBase64(body),
                    ..resource
                },
                PolicyDisposition::Review,
                "unsupported_non_image_binary",
            )),
        }
    }

    pub fn publish_rendered_scene(
        &self,
        scene: &arcweft_content_policy::RenderedScene,
        view_resources: &[(String, AgentResource)],
    ) -> Result<crate::PublishedAgentScene, AgentPolicyError> {
        let outcome = self.engine.process_rendered_scene(scene)?;
        crate::PublishedAgentScene::from_outcome(outcome, view_resources)
    }

    fn publish_text(
        &self,
        mut resource: AgentResource,
        text: String,
    ) -> Result<PublishedAgentResource, AgentPolicyError> {
        if resource.mime_type == "image/svg+xml" {
            return Ok(self.withheld_resource(
                AgentResource {
                    body: AgentResourceBody::Text(text),
                    ..resource
                },
                PolicyDisposition::Review,
                "svg_requires_trusted_rasterization",
            ));
        }
        let artifact = TextArtifact::new(ContentId::new(resource.uri.as_str()), text);
        let outcome = self.engine.process_text(&artifact)?;
        let summary = AgentPolicySummary::from_receipt(&outcome.receipt);
        resource.body = match outcome.publication {
            PolicyPublication::Allowed { value } | PolicyPublication::Sanitized { value } => {
                AgentResourceBody::Text(value.text)
            }
            PolicyPublication::Withheld { placeholder } => {
                "application/json".clone_into(&mut resource.mime_type);
                resource.image = None;
                AgentResourceBody::Json(policy_placeholder(
                    outcome.decision.disposition,
                    &placeholder.code,
                ))
            }
        };
        resource_body_digest(&resource.body)
            .as_str()
            .clone_into(&mut resource.hash);
        let extension = if resource.mime_type == "application/json" {
            "json"
        } else {
            "txt"
        };
        resource.uri = publication_uri(&resource, &summary, extension);
        Ok(PublishedAgentResource::new(resource, summary))
    }

    fn publish_json(
        &self,
        mut resource: AgentResource,
        mut value: Value,
    ) -> Result<PublishedAgentResource, AgentPolicyError> {
        let input_bytes = serde_json::to_vec(&value)?;
        let input_digest = ContentDigest::from_bytes(&input_bytes);
        let mut receipts = Vec::new();
        let mut key_decision = PolicyDecision::allow();
        moderate_json_value(
            &self.engine,
            resource.uri.as_str(),
            "$",
            &mut value,
            &mut receipts,
            &mut key_decision,
        )?;
        let decision = receipts
            .iter()
            .map(|receipt| receipt.decision.clone())
            .fold(key_decision, PolicyDecision::merge);
        if decision.disposition.is_withheld() {
            value = policy_placeholder(decision.disposition, "json_resource_withheld");
        }
        let output_bytes = serde_json::to_vec(&value)?;
        let output_digest = ContentDigest::from_bytes(&output_bytes);
        let summary = AgentPolicySummary::aggregate(
            self.engine.profile().id.as_str(),
            self.engine.profile().version.as_str(),
            input_digest,
            Some(output_digest.clone()),
            &receipts,
            decision,
        );
        resource.body = AgentResourceBody::Json(value);
        output_digest.as_str().clone_into(&mut resource.hash);
        resource.uri = publication_uri(&resource, &summary, "json");
        Ok(PublishedAgentResource::new(resource, summary))
    }

    fn publish_image(
        &self,
        mut resource: AgentResource,
    ) -> Result<PublishedAgentResource, AgentPolicyError> {
        let Some(metadata) = resource.image.as_ref() else {
            return Ok(self.withheld_resource(
                resource,
                PolicyDisposition::Review,
                "missing_image_metadata",
            ));
        };
        if metadata.kind.is_policy_auxiliary() && !self.publication.publish_auxiliary_images {
            return Ok(self.withheld_resource(
                resource,
                PolicyDisposition::Review,
                "auxiliary_capture_not_publishable",
            ));
        }
        let decoded = match decode_agent_image(&resource) {
            Ok(decoded) => decoded,
            Err(AgentPolicyError::MissingImageMetadata) => {
                return Ok(self.withheld_resource(
                    resource,
                    PolicyDisposition::Review,
                    "missing_image_metadata",
                ));
            }
            Err(AgentPolicyError::MissingImageBytes) => {
                return Ok(self.withheld_image_metadata_resource(
                    resource,
                    PolicyDisposition::Review,
                    "missing_image_bytes",
                ));
            }
            Err(AgentPolicyError::UnsupportedImageEncoding(_)) => {
                return Ok(self.withheld_resource(
                    resource,
                    PolicyDisposition::Review,
                    "unsupported_image_encoding",
                ));
            }
            Err(
                AgentPolicyError::Base64(_)
                | AgentPolicyError::Image(_)
                | AgentPolicyError::Png(_)
                | AgentPolicyError::MissingImageFrame,
            ) => {
                return Ok(self.withheld_resource(
                    resource,
                    PolicyDisposition::Review,
                    "image_decode_failed",
                ));
            }
            Err(error) => return Err(error),
        };
        let outcome = self.engine.process_image(&decoded.image, None)?;
        let summary = AgentPolicySummary::from_receipt(&outcome.receipt);
        match outcome.publication {
            PolicyPublication::Allowed { .. } => {
                resource_body_digest(&resource.body)
                    .as_str()
                    .clone_into(&mut resource.hash);
                resource.uri = moderated_resource_uri(&summary, decoded.input_encoding.extension());
            }
            PolicyPublication::Sanitized { value } => {
                let (output_encoding, bytes) = decoded.input_encoding.encode_sanitized(&value)?;
                resource.body = AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
                    encoding: AgentBinaryEncoding::Base64,
                    data: STANDARD.encode(&bytes),
                });
                output_encoding
                    .mime_type()
                    .clone_into(&mut resource.mime_type);
                resource.hash = blake3::hash(&bytes).to_hex().to_string();
                resource.uri = moderated_resource_uri(&summary, output_encoding.extension());
                if let Some(metadata) = resource.image.as_mut() {
                    metadata.width = value.width();
                    metadata.height = value.height();
                    metadata.pixel_format = output_encoding.pixel_format().map(str::to_owned);
                    metadata.row_stride_bytes = output_encoding.row_stride_bytes(value.width());
                    metadata.composition = metadata.composition.after_policy_mask();
                    metadata.content_bbox = None;
                    metadata.content_viewport_bbox = None;
                    metadata.content_pixels = None;
                }
            }
            PolicyPublication::Withheld { placeholder } => {
                "application/json".clone_into(&mut resource.mime_type);
                resource.image = None;
                resource.body = AgentResourceBody::Json(policy_placeholder(
                    outcome.decision.disposition,
                    &placeholder.code,
                ));
                resource_body_digest(&resource.body)
                    .as_str()
                    .clone_into(&mut resource.hash);
                resource.uri = moderated_resource_uri(&summary, "json");
            }
        }
        if let Some(metadata) = resource.image.as_mut() {
            metadata.scrub_for_external_publication(&summary.opaque_token());
        }
        Ok(PublishedAgentResource::new(resource, summary))
    }

    fn withheld_resource(
        &self,
        mut resource: AgentResource,
        disposition: PolicyDisposition,
        reason: &str,
    ) -> PublishedAgentResource {
        let input_digest = resource_body_digest(&resource.body);
        resource.image = None;
        "application/json".clone_into(&mut resource.mime_type);
        resource.body = AgentResourceBody::Json(policy_placeholder(disposition, reason));
        let output_digest = resource_body_digest(&resource.body);
        let summary = AgentPolicySummary::synthetic(
            self.engine.profile().id.as_str(),
            self.engine.profile().version.as_str(),
            disposition,
            reason,
            input_digest,
            Some(output_digest.clone()),
            false,
        );
        output_digest.as_str().clone_into(&mut resource.hash);
        resource.uri = moderated_resource_uri(&summary, "json");
        PublishedAgentResource::new(resource, summary)
    }

    fn withheld_image_metadata_resource(
        &self,
        mut resource: AgentResource,
        disposition: PolicyDisposition,
        reason: &str,
    ) -> PublishedAgentResource {
        let input_digest = resource_body_digest(&resource.body);
        "application/json".clone_into(&mut resource.mime_type);
        resource.body = AgentResourceBody::Json(policy_placeholder(disposition, reason));
        let output_digest = resource_body_digest(&resource.body);
        let summary = AgentPolicySummary::synthetic(
            self.engine.profile().id.as_str(),
            self.engine.profile().version.as_str(),
            disposition,
            reason,
            input_digest,
            Some(output_digest.clone()),
            false,
        );
        if let Some(metadata) = resource.image.as_mut() {
            metadata.scrub_for_external_publication(&summary.opaque_token());
        }
        output_digest.as_str().clone_into(&mut resource.hash);
        resource.uri = moderated_resource_uri(&summary, "json");
        PublishedAgentResource::new(resource, summary)
    }
}

fn publication_uri(
    resource: &AgentResource,
    summary: &AgentPolicySummary,
    moderated_extension: &str,
) -> arcweft_agent_protocol::ids::AgentResourceUri {
    if resource.has_canonical_public_uri()
        && summary.disposition.can_publish_original()
        && !summary.sanitized
    {
        resource.uri.clone()
    } else {
        moderated_resource_uri(summary, moderated_extension)
    }
}

fn moderated_resource_uri(
    summary: &AgentPolicySummary,
    extension: &str,
) -> arcweft_agent_protocol::ids::AgentResourceUri {
    arcweft_agent_protocol::ids::AgentResourceUri::new(summary.moderated_uri(extension))
        .expect("generated moderated URI is nonempty")
}

fn moderate_json_value<C>(
    engine: &ContentPolicyEngine<C>,
    resource_uri: &str,
    path: &str,
    value: &mut Value,
    receipts: &mut Vec<PolicyReceipt>,
    key_decision: &mut PolicyDecision,
) -> Result<(), AgentPolicyError>
where
    C: arcweft_content_policy::ContentClassifier,
{
    match value {
        Value::String(text) => {
            let artifact = TextArtifact::new(
                ContentId::new(format!("{resource_uri}#{path}")),
                text.clone(),
            );
            let outcome = engine.process_text(&artifact)?;
            *text = match outcome.publication {
                PolicyPublication::Allowed { value } | PolicyPublication::Sanitized { value } => {
                    value.text
                }
                PolicyPublication::Withheld { .. } => "[CONTENT WITHHELD]".to_owned(),
            };
            receipts.push(outcome.receipt);
        }
        Value::Array(values) => {
            for (index, value) in values.iter_mut().enumerate() {
                moderate_json_value(
                    engine,
                    resource_uri,
                    &format!("{path}[{index}]"),
                    value,
                    receipts,
                    key_decision,
                )?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values.iter_mut() {
                let key_artifact = TextArtifact::new(
                    ContentId::new(format!("{resource_uri}#{path}.<key>")),
                    key.clone(),
                );
                let key_outcome = engine.process_text(&key_artifact)?;
                let key_result = if key_outcome.decision.disposition >= PolicyDisposition::Sanitize
                {
                    key_outcome
                        .decision
                        .clone()
                        .force(PolicyDisposition::Review, "json_key_requires_rewrite")
                } else {
                    key_outcome.decision.clone()
                };
                *key_decision = key_decision.clone().merge(key_result);
                receipts.push(key_outcome.receipt);
                moderate_json_value(
                    engine,
                    resource_uri,
                    &format!("{path}.{key}"),
                    value,
                    receipts,
                    key_decision,
                )?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn policy_placeholder(disposition: PolicyDisposition, code: &str) -> Value {
    serde_json::json!({
        "content_policy": {
            "disposition": disposition.as_str(),
            "code": code,
        }
    })
}

fn resource_body_digest(body: &AgentResourceBody) -> ContentDigest {
    match body {
        AgentResourceBody::Json(value) => ContentDigest::from_bytes(value.to_string().as_bytes()),
        AgentResourceBody::Text(text) => ContentDigest::from_bytes(text.as_bytes()),
        AgentResourceBody::BytesBase64(body) => body.decode().map_or_else(
            |_| ContentDigest::from_bytes(body.data.as_bytes()),
            |bytes| ContentDigest::from_bytes(&bytes),
        ),
    }
}
