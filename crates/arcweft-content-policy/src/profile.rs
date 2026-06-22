use crate::raster::MaskStyle;
use crate::types::{
    ClassificationReport, Completeness, PolicyDecision, PolicyDisposition, PolicyModality,
    PolicyProfileId,
};
use serde::{Deserialize, Serialize};

/// Maps one category namespace and threshold to a deployment action.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CategoryRule {
    pub category_prefix: String,
    pub minimum_score_milli: u16,
    pub disposition: PolicyDisposition,
    #[serde(default = "default_public_label")]
    pub publish_label: bool,
}

const fn default_public_label() -> bool {
    true
}

impl CategoryRule {
    pub fn matches(&self, finding: &crate::types::PolicyFinding) -> bool {
        finding.category.matches_prefix(&self.category_prefix)
            && finding.score_milli >= self.minimum_score_milli.min(1000)
    }
}

/// Product policy independent from any one classifier implementation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PolicyProfile {
    pub id: PolicyProfileId,
    pub version: String,
    pub default_finding_disposition: PolicyDisposition,
    pub partial_classifier_disposition: PolicyDisposition,
    pub unsupported_classifier_disposition: PolicyDisposition,
    pub failed_classifier_disposition: PolicyDisposition,
    pub insufficient_scene_coverage_disposition: PolicyDisposition,
    pub residual_after_sanitize_disposition: PolicyDisposition,
    pub text_replacement: String,
    pub image_mask_style: MaskStyle,
    pub whole_resource_if_unlocalized: bool,
    pub rules: Vec<CategoryRule>,
}

impl PolicyProfile {
    pub fn strict_default() -> Self {
        Self {
            id: PolicyProfileId::new("arcweft.agent.strict"),
            version: "2026-06-22".to_owned(),
            default_finding_disposition: PolicyDisposition::Review,
            partial_classifier_disposition: PolicyDisposition::Review,
            unsupported_classifier_disposition: PolicyDisposition::Review,
            failed_classifier_disposition: PolicyDisposition::Block,
            insufficient_scene_coverage_disposition: PolicyDisposition::Review,
            residual_after_sanitize_disposition: PolicyDisposition::Block,
            text_replacement: "[REDACTED]".to_owned(),
            image_mask_style: MaskStyle::default(),
            whole_resource_if_unlocalized: true,
            rules: vec![
                CategoryRule {
                    category_prefix: "security.secret".to_owned(),
                    minimum_score_milli: 1,
                    disposition: PolicyDisposition::Block,
                    publish_label: true,
                },
                CategoryRule {
                    category_prefix: "security.confidential".to_owned(),
                    minimum_score_milli: 1,
                    disposition: PolicyDisposition::Sanitize,
                    publish_label: true,
                },
                CategoryRule {
                    category_prefix: "privacy.personal_data".to_owned(),
                    minimum_score_milli: 500,
                    disposition: PolicyDisposition::Sanitize,
                    publish_label: true,
                },
                CategoryRule {
                    category_prefix: "source.untrusted_instruction".to_owned(),
                    minimum_score_milli: 500,
                    disposition: PolicyDisposition::Sanitize,
                    publish_label: true,
                },
                CategoryRule {
                    category_prefix: "safety".to_owned(),
                    minimum_score_milli: 700,
                    disposition: PolicyDisposition::Sanitize,
                    publish_label: true,
                },
            ],
        }
    }

    pub fn evaluate(
        &self,
        report: &ClassificationReport,
        modality: PolicyModality,
    ) -> PolicyDecision {
        let mut decision = PolicyDecision::allow();
        if report.runs.is_empty() {
            decision = decision.force(self.failed_classifier_disposition, "classifier_missing_run");
        } else if report
            .runs
            .iter()
            .all(|run| run.completeness == Completeness::NotApplicable)
        {
            decision = decision.force(
                self.unsupported_classifier_disposition,
                "classifier_no_applicable_run",
            );
        }
        for run in &report.runs {
            let disposition = match run.completeness {
                Completeness::Complete | Completeness::NotApplicable => PolicyDisposition::Allow,
                Completeness::Partial => self.partial_classifier_disposition,
                Completeness::Unsupported => self.unsupported_classifier_disposition,
                Completeness::Failed => self.failed_classifier_disposition,
            };
            if disposition != PolicyDisposition::Allow {
                decision = decision.force(
                    disposition,
                    format!("classifier_{}", run.completeness.as_str()),
                );
            }
        }
        for finding in &report.findings {
            let rule = self.rules.iter().find(|rule| rule.matches(finding));
            let disposition =
                rule.map_or(self.default_finding_disposition, |rule| rule.disposition);
            decision.disposition = decision.disposition.stricter(disposition);
            decision
                .reason_codes
                .insert(format!("category:{}", finding.category.as_str()));
            if rule.is_none_or(|rule| rule.publish_label) {
                decision.public_labels.insert(finding.category.clone());
            }
            if disposition == PolicyDisposition::Sanitize
                && !finding.target.is_localized_for(modality)
                && !matches!(&finding.target, crate::types::FindingTarget::Whole)
            {
                decision = decision.force(
                    self.residual_after_sanitize_disposition,
                    "finding_target_not_localizable",
                );
            }
        }
        decision
    }
}
