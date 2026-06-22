use crate::classifier::{ContentClassifier, PolicyInputRef};
use crate::profile::PolicyProfile;
use crate::raster::{ObjectIdBuffer, RgbaImage};
use crate::scene::RenderedScene;
use crate::text::TextArtifact;
use crate::types::{
    ClassificationReport, ClassifierRun, Completeness, ContentDigest, FindingTarget,
    PolicyDecision, PolicyDisposition, PolicyError, PolicyModality, PolicyPlaceholder,
    PolicyReceipt,
};
use serde::{Deserialize, Serialize};

/// Publication result. Withheld content never carries the original value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PolicyPublication<T> {
    Allowed { value: T },
    Sanitized { value: T },
    Withheld { placeholder: PolicyPlaceholder },
}

impl<T> PolicyPublication<T> {
    pub const fn is_sanitized(&self) -> bool {
        matches!(self, Self::Sanitized { .. })
    }

    pub const fn is_withheld(&self) -> bool {
        matches!(self, Self::Withheld { .. })
    }

    pub fn into_publishable(self) -> Option<T> {
        match self {
            Self::Allowed { value } | Self::Sanitized { value } => Some(value),
            Self::Withheld { .. } => None,
        }
    }
}

/// Decision, safe publication, and cryptographic receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PolicyOutcome<T> {
    pub publication: PolicyPublication<T>,
    pub decision: PolicyDecision,
    pub receipt: PolicyReceipt,
}

/// Deterministic Sans I/O policy pipeline.
#[derive(Clone, Debug)]
pub struct ContentPolicyEngine<C> {
    classifier: C,
    profile: PolicyProfile,
}

type Processed<T> = (
    PolicyPublication<T>,
    PolicyDecision,
    Option<ContentDigest>,
    bool,
    Vec<ClassifierRun>,
);

impl<C> ContentPolicyEngine<C>
where
    C: ContentClassifier,
{
    pub const fn new(classifier: C, profile: PolicyProfile) -> Self {
        Self {
            classifier,
            profile,
        }
    }

    pub const fn profile(&self) -> &PolicyProfile {
        &self.profile
    }

    pub fn process_text(
        &self,
        input: &TextArtifact,
    ) -> Result<PolicyOutcome<TextArtifact>, PolicyError> {
        let input_digest = input.content_digest();
        let report = self.classify(PolicyInputRef::Text(&input.text));
        let decision = self.profile.evaluate(&report, PolicyModality::Text);
        let processed = match decision.disposition {
            PolicyDisposition::Allow | PolicyDisposition::Label => Ok((
                PolicyPublication::Allowed {
                    value: input.clone(),
                },
                decision,
                Some(input_digest.clone()),
                false,
                report.runs,
            )),
            PolicyDisposition::Sanitize => {
                let ranges = report
                    .findings
                    .iter()
                    .filter_map(|finding| match &finding.target {
                        FindingTarget::Whole => {
                            Some(crate::types::TextRange::new(0, input.text.len()))
                        }
                        FindingTarget::Text { range } => Some(*range),
                        _ => None,
                    });
                let sanitized_text = input.redacted(
                    ranges,
                    &self.profile.text_replacement,
                    self.profile.whole_resource_if_unlocalized,
                )?;
                Ok(self.finish_sanitized(
                    PolicyModality::Text,
                    decision,
                    report.runs,
                    sanitized_text.artifact,
                    TextArtifact::content_digest,
                    |value| PolicyInputRef::Text(&value.text),
                ))
            }
            PolicyDisposition::Review | PolicyDisposition::Block => Ok((
                PolicyPublication::Withheld {
                    placeholder: PolicyPlaceholder::new(decision.disposition.as_str()),
                },
                decision,
                None,
                false,
                report.runs,
            )),
        }?;
        Ok(self.outcome(PolicyModality::Text, input_digest, processed))
    }

    pub fn process_image(
        &self,
        input: &RgbaImage,
        object_ids: Option<&ObjectIdBuffer>,
    ) -> Result<PolicyOutcome<RgbaImage>, PolicyError> {
        let input_digest = input.content_digest();
        let report = self.classify(PolicyInputRef::Image(input));
        let decision = self.profile.evaluate(&report, PolicyModality::Image);
        let processed = match decision.disposition {
            PolicyDisposition::Allow | PolicyDisposition::Label => Ok((
                PolicyPublication::Allowed {
                    value: input.clone(),
                },
                decision,
                Some(input_digest.clone()),
                false,
                report.runs,
            )),
            PolicyDisposition::Sanitize => {
                let mask = input.mask_for_findings(
                    &report.findings,
                    object_ids,
                    self.profile.whole_resource_if_unlocalized,
                )?;
                let sanitized_image = input.masked(&mask, self.profile.image_mask_style)?;
                Ok(self.finish_sanitized(
                    PolicyModality::Image,
                    decision,
                    report.runs,
                    sanitized_image,
                    RgbaImage::content_digest,
                    |value| PolicyInputRef::Image(value),
                ))
            }
            PolicyDisposition::Review | PolicyDisposition::Block => Ok((
                PolicyPublication::Withheld {
                    placeholder: PolicyPlaceholder::new(decision.disposition.as_str()),
                },
                decision,
                None,
                false,
                report.runs,
            )),
        }?;
        Ok(self.outcome(PolicyModality::Image, input_digest, processed))
    }

    pub fn process_rendered_scene(
        &self,
        input: &RenderedScene,
    ) -> Result<PolicyOutcome<RenderedScene>, PolicyError> {
        let input_digest = input.content_digest();
        let report = self.classify(PolicyInputRef::RenderedScene(input));
        let mut decision = self
            .profile
            .evaluate(&report, PolicyModality::RenderedScene);
        if !input.coverage.is_sufficient() {
            decision = decision.force(
                self.profile.insufficient_scene_coverage_disposition,
                "insufficient_render_coverage",
            );
        }
        let processed = match decision.disposition {
            PolicyDisposition::Allow | PolicyDisposition::Label => Ok((
                PolicyPublication::Allowed {
                    value: input.clone(),
                },
                decision,
                Some(input_digest.clone()),
                false,
                report.runs,
            )),
            PolicyDisposition::Sanitize => {
                let sanitized_scene = input.sanitized(
                    &report.findings,
                    self.profile.image_mask_style,
                    self.profile.whole_resource_if_unlocalized,
                )?;
                Ok(self.finish_sanitized(
                    PolicyModality::RenderedScene,
                    decision,
                    report.runs,
                    sanitized_scene,
                    RenderedScene::content_digest,
                    |value| PolicyInputRef::RenderedScene(value),
                ))
            }
            PolicyDisposition::Review | PolicyDisposition::Block => Ok((
                PolicyPublication::Withheld {
                    placeholder: PolicyPlaceholder::new(decision.disposition.as_str()),
                },
                decision,
                None,
                false,
                report.runs,
            )),
        }?;
        Ok(self.outcome(PolicyModality::RenderedScene, input_digest, processed))
    }

    fn classify(&self, input: PolicyInputRef<'_>) -> ClassificationReport {
        self.classifier
            .classify(input)
            .unwrap_or_else(|error| ClassificationReport {
                findings: Vec::new(),
                runs: vec![ClassifierRun::incomplete(
                    self.classifier.identity(),
                    Completeness::Failed,
                    error.receipt_code(),
                )],
            })
    }

    fn finish_sanitized<T, D, I>(
        &self,
        modality: PolicyModality,
        decision: PolicyDecision,
        mut runs: Vec<ClassifierRun>,
        value: T,
        digest: D,
        input: I,
    ) -> Processed<T>
    where
        D: FnOnce(&T) -> ContentDigest,
        I: for<'a> FnOnce(&'a T) -> PolicyInputRef<'a>,
    {
        let second_report = self.classify(input(&value));
        let second_decision = self.profile.evaluate(&second_report, modality);
        runs.extend(second_report.runs);
        let merged = decision.merge(second_decision.clone());
        if second_decision.disposition >= PolicyDisposition::Sanitize {
            let blocked = merged.force(
                self.profile.residual_after_sanitize_disposition,
                "residual_after_sanitize",
            );
            return (
                PolicyPublication::Withheld {
                    placeholder: PolicyPlaceholder::new("residual_after_sanitize"),
                },
                blocked,
                None,
                true,
                runs,
            );
        }
        let output_digest = digest(&value);
        (
            PolicyPublication::Sanitized { value },
            merged,
            Some(output_digest),
            true,
            runs,
        )
    }

    fn outcome<T>(
        &self,
        modality: PolicyModality,
        input_digest: ContentDigest,
        processed: Processed<T>,
    ) -> PolicyOutcome<T> {
        let (publication, decision, output_digest, sanitized, classifier_runs) = processed;
        let receipt = PolicyReceipt::build(crate::types::PolicyReceiptParts {
            profile_id: self.profile.id.clone(),
            profile_version: self.profile.version.clone(),
            modality,
            input_digest,
            output_digest,
            decision: decision.clone(),
            classifier_runs,
            sanitized,
        });
        PolicyOutcome {
            publication,
            decision,
            receipt,
        }
    }
}
