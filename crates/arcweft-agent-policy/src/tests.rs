use crate::AgentContentPolicyGate;
use arcweft_agent_protocol::image::{
    AgentImageComposition, AgentImageKind, AgentImageMetadata, AgentImageRenderer, AgentImageScope,
};
use arcweft_agent_protocol::resource::{
    AgentBinaryEncoding, AgentBinaryResourceBody, AgentResource, AgentResourceBody,
    AgentResourceKind,
};
use arcweft_content_policy::{
    ClassificationReport, ClassifierIdentity, ClassifierRun, Completeness, ContentClassifier,
    ContentPolicyEngine, FindingTarget, PixelRect, PolicyCategory, PolicyFinding, PolicyInputRef,
    PolicyProfile, RuleClassifier,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};

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
        uri: "arcweft://session/test/frame/0/color.rgba".to_owned(),
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
fn json_string_values_are_redacted() {
    let gate = AgentContentPolicyGate::new(ContentPolicyEngine::new(
        RuleClassifier::strict_builtin(),
        PolicyProfile::strict_default(),
    ));
    let resource = AgentResource {
        uri: "arcweft://session/test/observation/latest.json".to_owned(),
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
