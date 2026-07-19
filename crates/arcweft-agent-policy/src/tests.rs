use crate::AgentContentPolicyGate;
use arcweft_agent_protocol::ids::{AgentResourceUri, AgentRunId, SessionId, StableHash};
use arcweft_agent_protocol::image::{
    AgentImageComposition, AgentImageKind, AgentImageMetadata, AgentImageRenderer, AgentImageScope,
};
use arcweft_agent_protocol::resource::{
    AgentBinaryEncoding, AgentBinaryResourceBody, AgentResource, AgentResourceBody,
    AgentResourceKind, trace_resource,
};
use arcweft_agent_protocol::trace::{AgentTraceKind, AgentTraceRecord};
use arcweft_content_policy::{
    ClassificationReport, ClassifierIdentity, ClassifierRun, Completeness, ContentClassifier,
    ContentPolicyEngine, FindingTarget, PixelRect, PolicyCategory, PolicyFinding, PolicyInputRef,
    PolicyProfile, RuleClassifier,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};

fn resource_uri(value: &str) -> AgentResourceUri {
    AgentResourceUri::new(value).expect("test resource URI is nonempty")
}

fn sealed_trace_resource(payload: serde_json::Value) -> AgentResource {
    trace_resource(&[AgentTraceRecord {
        schema_version: 1,
        run_id: AgentRunId::new("run.policy").expect("test run ID is canonical"),
        session_id: Some(SessionId::new("session.policy").expect("test session ID is nonempty")),
        sequence: 0,
        tick: None,
        kind: AgentTraceKind::DiagnosticEmitted,
        payload_hash: StableHash::new("blake3:test-payload").expect("test hash is nonempty"),
        payload,
        blob_refs: Vec::new(),
    }])
    .expect("same-run typed trace resource serializes")
}

#[derive(Clone, Debug)]
struct RedClassifier;

impl ContentClassifier for RedClassifier {
    fn identity(&self) -> ClassifierIdentity {
        ClassifierIdentity::new("test.red", "1")
    }

    fn classify(
        &self,
        input: PolicyInputRef<'_>,
    ) -> Result<ClassificationReport, arcweft_content_policy::PolicyError> {
        let findings = match input {
            PolicyInputRef::Image(image)
                if image.pixels().chunks_exact(4).any(|pixel| pixel[0] > 200) =>
            {
                vec![PolicyFinding::new(
                    PolicyCategory::new("safety.visual"),
                    900,
                    FindingTarget::ImageRect {
                        rect: PixelRect::new(0, 0, 1, 1),
                    },
                )]
            }
            PolicyInputRef::Text(_)
            | PolicyInputRef::Image(_)
            | PolicyInputRef::RenderedScene(_) => Vec::new(),
        };
        Ok(ClassificationReport {
            findings,
            runs: vec![ClassifierRun {
                identity: self.identity(),
                completeness: Completeness::Complete,
                failure_code: None,
            }],
        })
    }
}

fn raw_rgba_resource(kind: AgentImageKind, pixels: Vec<u8>) -> AgentResource {
    AgentResource {
        uri: resource_uri("arcweft://session/test/frame/0/color.rgba"),
        kind: AgentResourceKind::Image,
        mime_type: "application/octet-stream".to_owned(),
        hash: "raw".to_owned(),
        image: Some(AgentImageMetadata {
            kind,
            renderer: AgentImageRenderer::Native,
            scope: AgentImageScope::Object {
                id: "internal.customer.object".to_owned(),
            },
            composition: AgentImageComposition::Framebuffer,
            page: 0,
            capture_step: 0,
            capture_time_millis: 0,
            width: 1,
            height: 1,
            crop_origin: None,
            pixel_format: Some("rgba8_unorm".to_owned()),
            row_stride_bytes: Some(4),
            content_bbox: None,
            content_viewport_bbox: None,
            content_pixels: None,
            object: None,
            view: None,
            selected_capture: None,
            diagnostics: Vec::new(),
        }),
        body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
            encoding: AgentBinaryEncoding::Base64,
            data: STANDARD.encode(pixels),
        }),
    }
}

#[test]
fn gate_masks_image_before_publication() {
    let gate = AgentContentPolicyGate::new(ContentPolicyEngine::new(
        RedClassifier,
        PolicyProfile::strict_default(),
    ));

    let published = gate
        .publish(raw_rgba_resource(
            AgentImageKind::Color,
            vec![255, 0, 0, 255],
        ))
        .expect("publication succeeds");

    assert!(published.policy().sanitized);
    let bytes = published
        .resource()
        .body
        .decoded_bytes()
        .expect("base64 is valid")
        .expect("binary body");
    assert_eq!(bytes, vec![32, 32, 32, 255]);
    assert_eq!(
        published
            .resource()
            .image
            .as_ref()
            .map(|image| image.composition),
        Some(AgentImageComposition::MaskedFramebufferCrop)
    );
    assert!(published.resource().uri.starts_with("arcweft://moderated/"));
    let metadata = published
        .resource()
        .image
        .as_ref()
        .expect("image metadata remains");
    assert!(matches!(
        &metadata.scope,
        AgentImageScope::Object { id } if id.starts_with("object.")
            && !id.contains("customer")
    ));
    assert!(metadata.object.is_none());
    assert!(metadata.diagnostics.is_empty());
}

#[test]
fn auxiliary_capture_is_not_exposed_by_default() {
    let gate = AgentContentPolicyGate::new(ContentPolicyEngine::new(
        RedClassifier,
        PolicyProfile::strict_default(),
    ));

    let published = gate
        .publish(raw_rgba_resource(
            AgentImageKind::ObjectId,
            vec![1, 2, 3, 255],
        ))
        .expect("publication succeeds");

    assert_eq!(
        published.policy().disposition,
        arcweft_content_policy::PolicyDisposition::Review
    );
    assert!(published.resource().image.is_none());
    assert_eq!(published.resource().mime_type, "application/json");
}

#[test]
fn missing_image_metadata_is_withheld() {
    let gate = AgentContentPolicyGate::new(ContentPolicyEngine::new(
        RedClassifier,
        PolicyProfile::strict_default(),
    ));
    let mut resource = raw_rgba_resource(AgentImageKind::Color, vec![1, 2, 3, 255]);
    resource.image = None;

    let published = gate.publish(resource).expect("publication succeeds");

    assert_review_placeholder(&published, "missing_image_metadata");
}

#[test]
fn missing_image_bytes_is_withheld() {
    let gate = AgentContentPolicyGate::new(ContentPolicyEngine::new(
        RedClassifier,
        PolicyProfile::strict_default(),
    ));
    let mut resource = raw_rgba_resource(AgentImageKind::Color, vec![1, 2, 3, 255]);
    resource.body = AgentResourceBody::Text("not image bytes".to_owned());

    let published = gate.publish(resource).expect("publication succeeds");

    assert_review_placeholder_with_image_metadata(&published, "missing_image_bytes");
    let metadata = published
        .resource()
        .image
        .as_ref()
        .expect("metadata-only image resource keeps scrubbed metadata");
    assert!(matches!(
        &metadata.scope,
        AgentImageScope::Object { id } if id.starts_with("object.")
            && !id.contains("customer")
    ));
    assert!(metadata.object.is_none());
    assert!(metadata.diagnostics.is_empty());
}

#[test]
fn failed_image_decode_is_withheld() {
    let gate = AgentContentPolicyGate::new(ContentPolicyEngine::new(
        RedClassifier,
        PolicyProfile::strict_default(),
    ));
    let mut resource = raw_rgba_resource(AgentImageKind::Color, vec![1, 2, 3, 255]);
    resource.mime_type = "image/png".to_owned();
    resource.body = AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
        encoding: AgentBinaryEncoding::Base64,
        data: STANDARD.encode(b"not a png"),
    });

    let published = gate.publish(resource).expect("publication succeeds");

    assert_review_placeholder(&published, "image_decode_failed");
}

#[test]
fn json_string_values_are_redacted() {
    let gate = AgentContentPolicyGate::new(ContentPolicyEngine::new(
        RuleClassifier::strict_builtin(),
        PolicyProfile::strict_default(),
    ));
    let resource = AgentResource {
        uri: resource_uri("arcweft://session/test/observation/latest.json"),
        kind: AgentResourceKind::ObservationLatest,
        mime_type: "application/json".to_owned(),
        hash: "raw".to_owned(),
        image: None,
        body: AgentResourceBody::Json(serde_json::json!({
            "message": "公開 / 社外秘 / 公開"
        })),
    };

    let published = gate.publish(resource).expect("publication succeeds");

    let AgentResourceBody::Json(value) = &published.resource().body else {
        panic!("JSON remains JSON");
    };
    assert_eq!(value["message"], "公開 / [REDACTED] / 公開");
}

#[test]
fn allowed_trace_retains_its_canonical_public_uri() {
    let gate = AgentContentPolicyGate::new(ContentPolicyEngine::new(
        RuleClassifier::strict_builtin(),
        PolicyProfile::strict_default(),
    ));
    let resource = sealed_trace_resource(serde_json::json!({ "status": "ok" }));
    let canonical_uri = resource.uri.clone();

    let published = gate.publish(resource).expect("trace publication succeeds");

    assert_eq!(
        published.policy().disposition,
        arcweft_content_policy::PolicyDisposition::Allow
    );
    assert!(!published.policy().sanitized);
    assert!(published.policy().reason_codes.is_empty());
    assert_eq!(published.resource().uri, canonical_uri);
}

#[test]
fn sanitized_trace_receives_a_moderated_uri() {
    let gate = AgentContentPolicyGate::new(ContentPolicyEngine::new(
        RuleClassifier::strict_builtin(),
        PolicyProfile::strict_default(),
    ));
    let resource = sealed_trace_resource(serde_json::json!({ "message": "社外秘" }));
    let canonical_uri = resource.uri.clone();

    let published = gate.publish(resource).expect("trace publication succeeds");

    assert_eq!(
        published.policy().disposition,
        arcweft_content_policy::PolicyDisposition::Sanitize
    );
    assert!(published.policy().sanitized);
    assert!(published.resource().uri.starts_with("arcweft://moderated/"));
    assert_ne!(published.resource().uri, canonical_uri);
}

#[test]
fn canonical_trace_spelling_without_a_seal_receives_a_moderated_uri() {
    let gate = AgentContentPolicyGate::new(ContentPolicyEngine::new(
        RuleClassifier::strict_builtin(),
        PolicyProfile::strict_default(),
    ));
    let forged = AgentResource {
        uri: resource_uri("arcweft://run/run.policy/trace.arcwx"),
        kind: AgentResourceKind::Trace,
        mime_type: "application/vnd.arcweft.agent-trace+json".to_owned(),
        hash: "forged".to_owned(),
        image: None,
        body: AgentResourceBody::Json(serde_json::json!([])),
    };

    assert!(!forged.has_canonical_public_uri());
    let published = gate.publish(forged).expect("trace publication succeeds");
    assert!(published.resource().uri.starts_with("arcweft://moderated/"));
}

#[test]
fn trace_wire_round_trip_drops_canonical_publication_authority() {
    let gate = AgentContentPolicyGate::new(ContentPolicyEngine::new(
        RuleClassifier::strict_builtin(),
        PolicyProfile::strict_default(),
    ));
    let sealed = sealed_trace_resource(serde_json::json!({ "status": "ok" }));
    assert!(sealed.has_canonical_public_uri());
    let encoded = serde_json::to_vec(&sealed).expect("trace resource serializes");
    let decoded: AgentResource =
        serde_json::from_slice(&encoded).expect("trace resource deserializes");

    assert_eq!(decoded.uri, sealed.uri);
    assert!(!decoded.has_canonical_public_uri());
    let published = gate.publish(decoded).expect("trace publication succeeds");
    assert!(published.resource().uri.starts_with("arcweft://moderated/"));
}

#[test]
fn mutated_trace_body_cannot_reuse_a_canonical_public_uri() {
    let gate = AgentContentPolicyGate::new(ContentPolicyEngine::new(
        RuleClassifier::strict_builtin(),
        PolicyProfile::strict_default(),
    ));
    let mut forged = sealed_trace_resource(serde_json::json!({ "status": "ok" }));
    let canonical_uri = forged.uri.clone();
    forged.body = AgentResourceBody::Json(serde_json::json!([{
        "run_id": "run.other",
        "payload": { "status": "forged" }
    }]));

    assert!(!forged.has_canonical_public_uri());
    let published = gate.publish(forged).expect("trace publication succeeds");
    assert_ne!(published.resource().uri, canonical_uri);
    assert!(published.resource().uri.starts_with("arcweft://moderated/"));
}

#[test]
fn moderated_scene_children_receive_distinct_opaque_uris() {
    let summary = crate::AgentPolicySummary::synthetic(
        "profile",
        "1",
        arcweft_content_policy::PolicyDisposition::Allow,
        "test",
        arcweft_content_policy::ContentDigest::from_bytes(b"input"),
        Some(arcweft_content_policy::ContentDigest::from_bytes(b"output")),
        false,
    );

    let first = summary.moderated_child_uri("scene-view", 0, "png");
    let second = summary.moderated_child_uri("scene-view", 1, "png");

    assert_ne!(first, second);
    assert!(first.starts_with("arcweft://moderated/"));
    assert!(!first.contains("scene-view"));
}

fn assert_review_placeholder(published: &crate::PublishedAgentResource, expected_reason: &str) {
    assert_review_placeholder_policy(published, expected_reason);
    assert!(published.resource().image.is_none());
}

fn assert_review_placeholder_with_image_metadata(
    published: &crate::PublishedAgentResource,
    expected_reason: &str,
) {
    assert_review_placeholder_policy(published, expected_reason);
    assert!(published.resource().image.is_some());
}

fn assert_review_placeholder_policy(
    published: &crate::PublishedAgentResource,
    expected_reason: &str,
) {
    assert_eq!(
        published.policy().disposition,
        arcweft_content_policy::PolicyDisposition::Review
    );
    assert!(published.policy().reason_codes.contains(expected_reason));
    assert!(published.resource().uri.starts_with("arcweft://moderated/"));
    assert_eq!(published.resource().mime_type, "application/json");
    let AgentResourceBody::Json(value) = &published.resource().body else {
        panic!("withheld resource is JSON");
    };
    assert_eq!(value["content_policy"]["disposition"], "review");
    assert_eq!(value["content_policy"]["code"], expected_reason);
}
