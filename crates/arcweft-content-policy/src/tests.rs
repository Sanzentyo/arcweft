use crate::{
    ClassificationReport, ClassifierIdentity, ClassifierRun, Completeness, ContentClassifier,
    ContentId, ContentPolicyEngine, FindingTarget, ObjectId, ObjectIdBuffer, PixelRect,
    PolicyCategory, PolicyFinding, PolicyInputRef, PolicyProfile, PolicyPublication,
    RenderCoverage, RenderSampleKind, RenderedScene, RenderedView, RgbaImage, RuleClassifier,
    TextArtifact,
};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
struct RedClassifier;

impl ContentClassifier for RedClassifier {
    fn identity(&self) -> ClassifierIdentity {
        ClassifierIdentity::new("test.red", "1")
    }

    fn classify(
        &self,
        input: PolicyInputRef<'_>,
    ) -> Result<ClassificationReport, crate::PolicyError> {
        let findings = match input {
            PolicyInputRef::Text(_) => Vec::new(),
            PolicyInputRef::Image(image) => red_finding(
                image,
                FindingTarget::ImageRect {
                    rect: PixelRect::new(0, 0, 1, 1),
                },
            ),
            PolicyInputRef::RenderedScene(scene) => scene
                .views
                .iter()
                .flat_map(|view| {
                    red_finding(
                        &view.color,
                        FindingTarget::ObjectIds {
                            ids: BTreeSet::from([ObjectId(7)]),
                        },
                    )
                })
                .collect(),
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

fn red_finding(image: &RgbaImage, target: FindingTarget) -> Vec<PolicyFinding> {
    if image
        .pixels()
        .chunks_exact(4)
        .any(|pixel| pixel[0] > 200 && pixel[1] < 80 && pixel[2] < 80)
    {
        vec![PolicyFinding::new(
            PolicyCategory::new("safety.visual"),
            900,
            target,
        )]
    } else {
        Vec::new()
    }
}

#[test]
fn text_is_redacted_and_rechecked() {
    let engine = ContentPolicyEngine::new(
        RuleClassifier::strict_builtin(),
        PolicyProfile::strict_default(),
    );
    let input = TextArtifact::new(ContentId::new("text.1"), "公開 / 社外秘 / 公開");

    let outcome = engine.process_text(&input).expect("text policy succeeds");

    let PolicyPublication::Sanitized { value } = outcome.publication else {
        panic!("confidential marker should be sanitized");
    };
    assert_eq!(value.text, "公開 / [REDACTED] / 公開");
    assert!(outcome.receipt.sanitized);
    assert_eq!(outcome.receipt.classifier_runs.len(), 2);
}

#[test]
fn image_region_is_solid_masked_and_rechecked() {
    let engine = ContentPolicyEngine::new(RedClassifier, PolicyProfile::strict_default());
    let input = RgbaImage::new(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 255]).expect("valid RGBA");

    let outcome = engine
        .process_image(&input, None)
        .expect("image policy succeeds");

    let PolicyPublication::Sanitized { value } = outcome.publication else {
        panic!("red pixel should be sanitized");
    };
    assert_eq!(&value.pixels()[0..4], &[32, 32, 32, 255]);
    assert_eq!(&value.pixels()[4..8], &[0, 255, 0, 255]);
}

#[test]
fn missing_image_classifier_fails_closed() {
    let engine = ContentPolicyEngine::new(
        RuleClassifier::strict_builtin(),
        PolicyProfile::strict_default(),
    );
    let input = RgbaImage::new(1, 1, vec![0, 0, 0, 255]).expect("valid RGBA");

    let outcome = engine
        .process_image(&input, None)
        .expect("unsupported classifier is a policy result");

    assert!(outcome.publication.is_withheld());
    assert_eq!(
        outcome.decision.disposition,
        crate::PolicyDisposition::Review
    );
}

#[test]
fn rendered_scene_masks_object_across_views() {
    let red = RgbaImage::new(1, 1, vec![255, 0, 0, 255]).expect("valid RGBA");
    let object_ids = ObjectIdBuffer::new(1, 1, vec![ObjectId(7)]).expect("valid ids");
    let scene = RenderedScene::new(
        ContentId::new("model.1"),
        vec![
            RenderedView {
                id: "front".to_owned(),
                sample_kind: RenderSampleKind::Canonical,
                sample_index: 0,
                color: red.clone(),
                object_ids: Some(object_ids.clone()),
            },
            RenderedView {
                id: "back".to_owned(),
                sample_kind: RenderSampleKind::Canonical,
                sample_index: 1,
                color: red,
                object_ids: Some(object_ids),
            },
        ],
        RenderCoverage {
            required_canonical_views: 2,
            observed_canonical_views: 2,
            required_animation_samples: 0,
            observed_animation_samples: 0,
            required_lod_samples: 0,
            observed_lod_samples: 0,
        },
    )
    .expect("valid scene");
    let engine = ContentPolicyEngine::new(RedClassifier, PolicyProfile::strict_default());

    let outcome = engine
        .process_rendered_scene(&scene)
        .expect("scene policy succeeds");

    let PolicyPublication::Sanitized { value } = outcome.publication else {
        panic!("scene should be sanitized");
    };
    assert!(
        value
            .views
            .iter()
            .all(|view| view.color.pixels() == [32, 32, 32, 255])
    );
}

#[derive(Clone, Debug)]
struct EmptyReportClassifier;

impl ContentClassifier for EmptyReportClassifier {
    fn identity(&self) -> ClassifierIdentity {
        ClassifierIdentity::new("test.empty", "1")
    }

    fn classify(
        &self,
        _input: PolicyInputRef<'_>,
    ) -> Result<ClassificationReport, crate::PolicyError> {
        Ok(ClassificationReport::default())
    }
}

#[test]
fn classifier_report_without_runs_fails_closed() {
    let engine = ContentPolicyEngine::new(EmptyReportClassifier, PolicyProfile::strict_default());
    let input = TextArtifact::new(ContentId::new("text.empty"), "public");

    let outcome = engine
        .process_text(&input)
        .expect("policy evaluation succeeds");

    assert!(outcome.publication.is_withheld());
    assert_eq!(
        outcome.decision.disposition,
        crate::PolicyDisposition::Block
    );
    assert!(
        outcome
            .decision
            .reason_codes
            .contains("classifier_missing_run")
    );
}

#[test]
fn deterministic_secret_rule_localizes_the_entire_line() {
    let classifier = RuleClassifier::strict_builtin();
    let text = "before\napi_key=top-secret\nafter";

    let report = classifier
        .classify(PolicyInputRef::Text(text))
        .expect("rule classifier succeeds");

    assert!(report.findings.iter().any(|finding| {
        matches!(
            &finding.target,
            FindingTarget::Text { range }
                if &text[range.start..range.end] == "api_key=top-secret"
        )
    }));
}

#[test]
fn scene_view_mask_leaves_unrelated_views_unchanged() {
    let red = RgbaImage::new(1, 1, vec![255, 0, 0, 255]).expect("valid RGBA");
    let green = RgbaImage::new(1, 1, vec![0, 255, 0, 255]).expect("valid RGBA");
    let scene = RenderedScene::new(
        ContentId::new("model.targeted"),
        vec![
            RenderedView {
                id: "front".to_owned(),
                sample_kind: RenderSampleKind::Canonical,
                sample_index: 0,
                color: red,
                object_ids: None,
            },
            RenderedView {
                id: "back".to_owned(),
                sample_kind: RenderSampleKind::Canonical,
                sample_index: 1,
                color: green.clone(),
                object_ids: None,
            },
        ],
        RenderCoverage {
            required_canonical_views: 2,
            observed_canonical_views: 2,
            required_animation_samples: 0,
            observed_animation_samples: 0,
            required_lod_samples: 0,
            observed_lod_samples: 0,
        },
    )
    .expect("valid scene");
    let finding = PolicyFinding::new(
        PolicyCategory::new("safety.visual"),
        900,
        FindingTarget::SceneViewRect {
            view_id: "front".to_owned(),
            rect: PixelRect::new(0, 0, 1, 1),
        },
    );

    let sanitized = scene
        .sanitized(&[finding], crate::MaskStyle::default(), true)
        .expect("scene sanitization succeeds");

    assert_eq!(sanitized.views[0].color.pixels(), &[32, 32, 32, 255]);
    assert_eq!(sanitized.views[1].color, green);
}

#[test]
fn non_applicable_rule_component_does_not_downgrade_complete_visual_model() {
    let classifier =
        crate::CompositeClassifier::new(RuleClassifier::strict_builtin(), RedClassifier);
    let engine = ContentPolicyEngine::new(classifier, PolicyProfile::strict_default());
    let input = RgbaImage::new(1, 1, vec![255, 0, 0, 255]).expect("valid RGBA");

    let outcome = engine
        .process_image(&input, None)
        .expect("composite image policy succeeds");

    assert!(matches!(
        outcome.publication,
        PolicyPublication::Sanitized { .. }
    ));
    assert!(
        !outcome
            .decision
            .reason_codes
            .contains("classifier_no_applicable_run")
    );
}
