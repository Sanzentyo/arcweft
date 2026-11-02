use super::*;
use arcweft_agent_protocol::image::AgentImageObjectRef;
use arcweft_content_policy::{PolicyDisposition, TextRange};

#[derive(Clone, Debug)]
struct MetadataClassifier;

impl ContentClassifier for MetadataClassifier {
    fn identity(&self) -> ClassifierIdentity {
        ClassifierIdentity::new("test.image-metadata", "1")
    }

    fn classify(
        &self,
        input: PolicyInputRef<'_>,
    ) -> Result<ClassificationReport, arcweft_content_policy::PolicyError> {
        let mut findings = Vec::new();
        if let PolicyInputRef::Text(text) = &input {
            if text.contains("do-not-publish") {
                findings.push(PolicyFinding::new(
                    PolicyCategory::security_secret(),
                    1000,
                    FindingTarget::Whole,
                ));
            }
            if let Some(start) = text.find("rewrite-metadata") {
                findings.push(PolicyFinding::new(
                    PolicyCategory::personal_data(),
                    1000,
                    FindingTarget::Text {
                        range: TextRange::new(start, start + "rewrite-metadata".len()),
                    },
                ));
            }
        }
        let mut report = RedClassifier.classify(input)?;
        report.findings.extend(findings);
        report.runs[0].identity = self.identity();
        Ok(report)
    }
}

fn gate() -> AgentContentPolicyGate<MetadataClassifier> {
    AgentContentPolicyGate::new(ContentPolicyEngine::new(
        MetadataClassifier,
        PolicyProfile::strict_default(),
    ))
}

fn image_resource() -> AgentResource {
    let mut resource = raw_rgba_resource(AgentImageKind::Color, vec![1, 2, 3, 255]);
    let object: AgentImageObjectRef = serde_json::from_value(serde_json::json!({
        "id": "object.image.handle.resource.fixture",
        "entity": "image.fixture",
        "layer": "layer.foreground",
        "role": "image",
        "bbox": { "space": "viewport", "x": 0, "y": 0, "width": 1, "height": 1 },
        "polygon": [],
        "capture_refs": {
            "object_id_color": { "red": 64, "green": 65, "blue": 66, "alpha": 255 },
            "captures": [{
                "kind": "color", "uri": resource.uri.as_str(),
                "mime_type": "application/octet-stream", "pixel_format": "rgba8_unorm",
                "width": 1, "height": 1
            }]
        },
        "content": {
            "kind": "image", "source": "image.fixture", "object": "image.fixture",
            "occurrence": "handle.resource.fixture", "asset": "asset.fixture",
            "frame_index": 1, "local_time_millis": 150,
            "params": { "role": { "kind": "text", "value": "preview" } }
        }
    }))
    .unwrap();
    let metadata = resource.image.as_mut().unwrap();
    metadata.scope = AgentImageScope::Object {
        id: object.id.clone(),
    };
    metadata.object = Some(object);
    metadata
        .diagnostics
        .push(arcweft_agent_protocol::diagnostic::AgentDiagnostic {
            step: 0,
            severity: arcweft_agent_protocol::diagnostic::AgentDiagnosticSeverity::Info,
            source: Some("image".into()),
            code: Some("FRAME_SELECTED".into()),
            effect_id: None,
            message: "selected frame 1".into(),
        });
    resource
}

#[test]
fn allowed_image_preserves_the_complete_admitted_metadata_and_pixels() {
    let original = image_resource();
    let published = gate().publish(original.clone()).unwrap();
    assert_eq!(published.resource().image, original.image);
    assert_eq!(published.resource().body, original.body);
    assert_eq!(published.policy().disposition, PolicyDisposition::Allow);
    assert!(!published.policy().sanitized);
    assert!(published.policy().receipt_ids.len() > 1);
    assert_eq!(
        published.policy().input_digest,
        *published.policy().output_digest.as_ref().unwrap()
    );
    let bytes = serde_json::to_vec(published.resource()).unwrap();
    let decoded: AgentResource = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded.image, original.image);

    let mut changed = original;
    changed
        .image
        .as_mut()
        .unwrap()
        .object
        .as_mut()
        .unwrap()
        .entity = Some("image.other".into());
    let changed = gate().publish(changed).unwrap();
    assert_ne!(
        published.policy().input_digest,
        changed.policy().input_digest
    );
    assert_ne!(published.resource().uri, changed.resource().uri);
    assert_eq!(published.resource().body, changed.resource().body);
}

#[test]
fn allowed_pixels_do_not_authorize_blocked_metadata_values_or_keys() {
    for secret_in_key in [false, true] {
        let mut resource = image_resource();
        let object = resource.image.as_mut().unwrap().object.as_mut().unwrap();
        if secret_in_key {
            let arcweft_agent_protocol::object::AgentObservedObjectContent::Image(content) =
                &mut object.content
            else {
                panic!()
            };
            content.params.insert(
                "do-not-publish".into(),
                arcweft_agent_protocol::image::AgentImageObjectParam::Bool { value: true },
            );
        } else {
            object.text = Some("do-not-publish".into());
        }
        let published = gate().publish(resource).unwrap();
        assert_eq!(published.policy().disposition, PolicyDisposition::Block);
        assert!(published.resource().image.is_none());
        assert!(matches!(
            published.resource().body,
            AgentResourceBody::Json(_)
        ));
        assert!(published.policy().receipt_ids.len() > 1);
        assert!(
            !serde_json::to_string(&published)
                .unwrap()
                .contains("do-not-publish")
        );
    }
}

#[test]
fn metadata_requiring_rewrite_withholds_the_whole_resource() {
    let mut resource = image_resource();
    resource
        .image
        .as_mut()
        .unwrap()
        .object
        .as_mut()
        .unwrap()
        .text = Some("rewrite-metadata".into());
    let published = gate().publish(resource).unwrap();
    assert_eq!(published.policy().disposition, PolicyDisposition::Review);
    assert!(published.policy().sanitized);
    assert!(
        published
            .policy()
            .reason_codes
            .contains("image_metadata_requires_rewrite")
    );
    assert!(published.resource().image.is_none());
    assert!(
        !serde_json::to_string(&published)
            .unwrap()
            .contains("rewrite-metadata")
    );
}

#[test]
fn mismatched_object_scope_cannot_enter_the_admitted_publication() {
    let mut resource = image_resource();
    resource.image.as_mut().unwrap().scope = AgentImageScope::Object {
        id: "object.other".into(),
    };
    assert!(matches!(
        gate().publish(resource),
        Err(crate::AgentPolicyError::Json(_))
    ));
}

#[test]
fn sanitized_pixels_keep_the_scrubbed_descriptor() {
    let mut resource = image_resource();
    resource.body = AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
        encoding: AgentBinaryEncoding::Base64,
        data: STANDARD.encode([255, 0, 0, 255]),
    });
    let mut other = resource.clone();
    other.image.as_mut().unwrap().capture_time_millis = 150;
    let published = gate().publish(resource).unwrap();
    let other = gate().publish(other).unwrap();
    assert_ne!(published.resource().uri, other.resource().uri);
    assert_eq!(published.resource().body, other.resource().body);
    assert!(published.policy().sanitized);
    let metadata = published.resource().image.as_ref().unwrap();
    assert!(metadata.object.is_none());
    assert!(metadata.diagnostics.is_empty());
    assert!(
        !serde_json::to_string(&published)
            .unwrap()
            .contains("handle.resource.fixture")
    );
    let AgentResourceBody::BytesBase64(body) = &published.resource().body else {
        panic!()
    };
    assert_eq!(
        crate::decode::decode_agent_binary_body(body).unwrap(),
        [32, 32, 32, 255]
    );
}
