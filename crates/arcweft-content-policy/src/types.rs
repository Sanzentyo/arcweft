use crate::raster::PixelMask;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Stable identity for one policy input.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ContentId(String);

impl ContentId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable policy profile identity.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct PolicyProfileId(String);

impl PolicyProfileId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable digest represented as lowercase BLAKE3 hexadecimal text.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ContentDigest(String);

impl ContentDigest {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(blake3::hash(bytes).to_hex().to_string())
    }

    pub fn from_hasher(hasher: &blake3::Hasher) -> Self {
        Self(hasher.finalize().to_hex().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable receipt identity.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct PolicyReceiptId(String);

impl PolicyReceiptId {
    pub fn from_digest(digest: &ContentDigest) -> Self {
        Self(format!("policy:{}", digest.as_str()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Content modality evaluated by the policy engine.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyModality {
    Text,
    Image,
    RenderedScene,
}

impl PolicyModality {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::RenderedScene => "rendered_scene",
        }
    }
}

/// Final policy action, ordered from least to most restrictive.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDisposition {
    Allow,
    Label,
    Sanitize,
    Review,
    Block,
}

impl PolicyDisposition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Label => "label",
            Self::Sanitize => "sanitize",
            Self::Review => "review",
            Self::Block => "block",
        }
    }

    pub const fn can_publish_original(self) -> bool {
        matches!(self, Self::Allow | Self::Label)
    }

    pub const fn can_publish_sanitized(self) -> bool {
        matches!(self, Self::Sanitize)
    }

    pub const fn is_withheld(self) -> bool {
        matches!(self, Self::Review | Self::Block)
    }

    #[must_use]
    pub fn stricter(self, other: Self) -> Self {
        if self >= other { self } else { other }
    }
}

/// Completeness reported by a classifier execution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    Complete,
    NotApplicable,
    Partial,
    Unsupported,
    Failed,
}

impl Completeness {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::NotApplicable => "not_applicable",
            Self::Partial => "partial",
            Self::Unsupported => "unsupported",
            Self::Failed => "failed",
        }
    }

    /// Merges independent classifier coverage without treating an optional,
    /// non-applicable classifier as a degradation when another classifier
    /// completed the modality.
    #[must_use]
    pub const fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Failed, _) | (_, Self::Failed) => Self::Failed,
            (Self::Unsupported, _) | (_, Self::Unsupported) => Self::Unsupported,
            (Self::Partial, _) | (_, Self::Partial) => Self::Partial,
            (Self::Complete, _) | (_, Self::Complete) => Self::Complete,
            (Self::NotApplicable, Self::NotApplicable) => Self::NotApplicable,
        }
    }
}

/// Namespaced policy category such as `safety.sexual` or `security.secret`.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct PolicyCategory(String);

impl PolicyCategory {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn matches_prefix(&self, prefix: &str) -> bool {
        self.0 == prefix
            || self
                .0
                .strip_prefix(prefix)
                .is_some_and(|suffix| suffix.starts_with('.'))
    }

    pub fn security_secret() -> Self {
        Self::new("security.secret")
    }

    pub fn security_confidential() -> Self {
        Self::new("security.confidential")
    }

    pub fn personal_data() -> Self {
        Self::new("privacy.personal_data")
    }

    pub fn untrusted_instruction() -> Self {
        Self::new("source.untrusted_instruction")
    }
}

/// UTF-8 byte range. Constructors validate bounds at the artifact boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

/// Pixel-space rectangle using an exclusive right/bottom edge.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub fn clamped(self, width: u32, height: u32) -> Self {
        let x = self.x.min(width);
        let y = self.y.min(height);
        Self {
            x,
            y,
            width: self.width.min(width.saturating_sub(x)),
            height: self.height.min(height.saturating_sub(y)),
        }
    }

    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// Stable renderer object identifier used with an object-id attachment.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ObjectId(pub u32);

/// Localizable target produced by a classifier.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FindingTarget {
    Whole,
    Text { range: TextRange },
    ImageRect { rect: PixelRect },
    ImageMask { mask: PixelMask },
    SceneViewRect { view_id: String, rect: PixelRect },
    SceneViewMask { view_id: String, mask: PixelMask },
    ObjectIds { ids: BTreeSet<ObjectId> },
}

impl FindingTarget {
    pub const fn is_localized_for(&self, modality: PolicyModality) -> bool {
        matches!(
            (self, modality),
            (Self::Text { .. }, PolicyModality::Text)
                | (
                    Self::ImageRect { .. } | Self::ImageMask { .. } | Self::ObjectIds { .. },
                    PolicyModality::Image
                )
                | (
                    Self::SceneViewRect { .. }
                        | Self::SceneViewMask { .. }
                        | Self::ObjectIds { .. },
                    PolicyModality::RenderedScene
                )
        )
    }
}

/// One policy finding with a normalized confidence score in thousandths.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PolicyFinding {
    pub category: PolicyCategory,
    pub score_milli: u16,
    pub target: FindingTarget,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

impl PolicyFinding {
    pub fn new(category: PolicyCategory, score_milli: u16, target: FindingTarget) -> Self {
        Self {
            category,
            score_milli: score_milli.min(1000),
            target,
            attributes: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

/// Stable classifier identity included in every receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClassifierIdentity {
    pub id: String,
    pub revision: String,
    pub model_digest: Option<ContentDigest>,
}

impl ClassifierIdentity {
    pub fn new(id: impl Into<String>, revision: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            revision: revision.into(),
            model_digest: None,
        }
    }

    #[must_use]
    pub fn with_model_digest(mut self, digest: ContentDigest) -> Self {
        self.model_digest = Some(digest);
        self
    }
}

/// Result of one classifier component.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClassifierRun {
    pub identity: ClassifierIdentity,
    pub completeness: Completeness,
    pub failure_code: Option<String>,
}

impl ClassifierRun {
    pub fn complete(identity: ClassifierIdentity) -> Self {
        Self {
            identity,
            completeness: Completeness::Complete,
            failure_code: None,
        }
    }

    pub fn not_applicable(identity: ClassifierIdentity) -> Self {
        Self {
            identity,
            completeness: Completeness::NotApplicable,
            failure_code: None,
        }
    }

    pub fn incomplete(
        identity: ClassifierIdentity,
        completeness: Completeness,
        failure_code: impl Into<String>,
    ) -> Self {
        Self {
            identity,
            completeness,
            failure_code: Some(failure_code.into()),
        }
    }
}

/// Aggregated classifier report before deployment policy is applied.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClassificationReport {
    pub findings: Vec<PolicyFinding>,
    pub runs: Vec<ClassifierRun>,
}

impl ClassificationReport {
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.findings.extend(other.findings);
        self.runs.extend(other.runs);
        self
    }

    pub fn completeness(&self) -> Completeness {
        if self.runs.is_empty() {
            return Completeness::Failed;
        }
        self.runs
            .iter()
            .map(|run| run.completeness)
            .fold(Completeness::Complete, Completeness::merge)
    }
}

/// Deployment-policy result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PolicyDecision {
    pub disposition: PolicyDisposition,
    pub public_labels: BTreeSet<PolicyCategory>,
    pub reason_codes: BTreeSet<String>,
}

impl PolicyDecision {
    pub fn allow() -> Self {
        Self {
            disposition: PolicyDisposition::Allow,
            public_labels: BTreeSet::new(),
            reason_codes: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.disposition = self.disposition.stricter(other.disposition);
        self.public_labels.extend(other.public_labels);
        self.reason_codes.extend(other.reason_codes);
        self
    }

    #[must_use]
    pub fn force(mut self, disposition: PolicyDisposition, reason: impl Into<String>) -> Self {
        self.disposition = self.disposition.stricter(disposition);
        self.reason_codes.insert(reason.into());
        self
    }
}

/// Safe placeholder returned instead of withheld original content.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PolicyPlaceholder {
    pub code: String,
}

impl PolicyPlaceholder {
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }
}

/// Input used to construct one policy receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PolicyReceiptParts {
    pub profile_id: PolicyProfileId,
    pub profile_version: String,
    pub modality: PolicyModality,
    pub input_digest: ContentDigest,
    pub output_digest: Option<ContentDigest>,
    pub decision: PolicyDecision,
    pub classifier_runs: Vec<ClassifierRun>,
    pub sanitized: bool,
}

/// Receipt binding input, output, policy, classifier runs, and decision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PolicyReceipt {
    pub id: PolicyReceiptId,
    pub profile_id: PolicyProfileId,
    pub profile_version: String,
    pub modality: PolicyModality,
    pub input_digest: ContentDigest,
    pub output_digest: Option<ContentDigest>,
    pub decision: PolicyDecision,
    pub classifier_runs: Vec<ClassifierRun>,
    pub sanitized: bool,
}

impl PolicyReceipt {
    pub fn build(parts: PolicyReceiptParts) -> Self {
        let mut hasher = blake3::Hasher::new();
        hash_field(&mut hasher, b"arcweft.content-policy.receipt.v1");
        hash_field(&mut hasher, parts.profile_id.as_str().as_bytes());
        hash_field(&mut hasher, parts.profile_version.as_bytes());
        hash_field(&mut hasher, parts.modality.as_str().as_bytes());
        hash_field(&mut hasher, parts.input_digest.as_str().as_bytes());
        match &parts.output_digest {
            Some(output_digest) => {
                hash_field(&mut hasher, b"some");
                hash_field(&mut hasher, output_digest.as_str().as_bytes());
            }
            None => hash_field(&mut hasher, b"none"),
        }
        hash_field(&mut hasher, parts.decision.disposition.as_str().as_bytes());
        hash_field(&mut hasher, b"public-labels");
        hash_count(&mut hasher, parts.decision.public_labels.len());
        for label in &parts.decision.public_labels {
            hash_field(&mut hasher, label.as_str().as_bytes());
        }
        hash_field(&mut hasher, b"reason-codes");
        hash_count(&mut hasher, parts.decision.reason_codes.len());
        for reason in &parts.decision.reason_codes {
            hash_field(&mut hasher, reason.as_bytes());
        }
        hash_field(&mut hasher, b"classifier-runs");
        hash_count(&mut hasher, parts.classifier_runs.len());
        for run in &parts.classifier_runs {
            hash_field(&mut hasher, run.identity.id.as_bytes());
            hash_field(&mut hasher, run.identity.revision.as_bytes());
            match &run.identity.model_digest {
                Some(digest) => {
                    hash_field(&mut hasher, b"model");
                    hash_field(&mut hasher, digest.as_str().as_bytes());
                }
                None => hash_field(&mut hasher, b"no-model"),
            }
            hash_field(&mut hasher, run.completeness.as_str().as_bytes());
            match &run.failure_code {
                Some(code) => hash_field(&mut hasher, code.as_bytes()),
                None => hash_field(&mut hasher, b"ok"),
            }
        }
        hash_field(&mut hasher, b"sanitized");
        hash_field(&mut hasher, &[u8::from(parts.sanitized)]);
        let receipt_digest = ContentDigest::from_hasher(&hasher);
        Self {
            id: PolicyReceiptId::from_digest(&receipt_digest),
            profile_id: parts.profile_id,
            profile_version: parts.profile_version,
            modality: parts.modality,
            input_digest: parts.input_digest,
            output_digest: parts.output_digest,
            decision: parts.decision,
            classifier_runs: parts.classifier_runs,
            sanitized: parts.sanitized,
        }
    }
}

fn hash_count(hasher: &mut blake3::Hasher, value: usize) {
    let value = u64::try_from(value).unwrap_or(u64::MAX);
    hasher.update(&value.to_le_bytes());
}

fn hash_field(hasher: &mut blake3::Hasher, value: &[u8]) {
    hash_count(hasher, value.len());
    hasher.update(value);
}

/// Validation or sanitization failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PolicyError {
    #[error("RGBA dimensions must be non-zero")]
    EmptyImage,
    #[error("RGBA byte length {actual} does not match expected length {expected}")]
    InvalidRgbaLength { expected: usize, actual: usize },
    #[error("mask dimensions do not match the image")]
    MaskDimensionMismatch,
    #[error("object-id dimensions do not match the image")]
    ObjectIdDimensionMismatch,
    #[error("invalid UTF-8 byte range")]
    InvalidTextRange,
    #[error("classifier failed: {code}")]
    Classifier { code: String },
    #[error("rendered scene has no views")]
    EmptyRenderedScene,
    #[error("rendered scene view `{view_id}` has mismatched object-id dimensions")]
    InvalidSceneObjectIds { view_id: String },
}

impl PolicyError {
    /// Stable non-sensitive code suitable for receipts and external diagnostics.
    pub const fn receipt_code(&self) -> &'static str {
        match self {
            Self::EmptyImage => "empty_image",
            Self::InvalidRgbaLength { .. } => "invalid_rgba_length",
            Self::MaskDimensionMismatch => "mask_dimension_mismatch",
            Self::ObjectIdDimensionMismatch => "object_id_dimension_mismatch",
            Self::InvalidTextRange => "invalid_text_range",
            Self::Classifier { .. } => "classifier_error",
            Self::EmptyRenderedScene => "empty_rendered_scene",
            Self::InvalidSceneObjectIds { .. } => "invalid_scene_object_ids",
        }
    }
}
