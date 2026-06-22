//! Sans I/O content classification, policy evaluation, sanitization, and receipts.
//!
//! The crate owns content-policy concepts that do not already exist elsewhere in
//! Arcweft. Integration-specific behavior for Arcweft-owned enums remains on
//! those enums in their defining crates.

pub mod classifier;
pub mod engine;
pub mod model;
pub mod profile;
pub mod raster;
pub mod scene;
pub mod text;
pub mod types;

pub use classifier::{
    CompositeClassifier, ContentClassifier, PolicyInputRef, RuleClassifier, TextRule, TextRuleScope,
};
pub use engine::{ContentPolicyEngine, PolicyOutcome, PolicyPublication};
pub use model::{EmbeddedPolicyModel, ModelClassifier, ModelDetection, ModelInput, ModelOutput};
pub use profile::{CategoryRule, PolicyProfile};
pub use raster::{MaskStyle, ObjectIdBuffer, PixelMask, RgbaImage};
pub use scene::{RenderCoverage, RenderSampleKind, RenderedScene, RenderedView};
pub use text::{TextArtifact, TextRedaction, TextSanitization};
pub use types::{
    ClassificationReport, ClassifierIdentity, ClassifierRun, Completeness, ContentDigest,
    ContentId, FindingTarget, ObjectId, PixelRect, PolicyCategory, PolicyDecision,
    PolicyDisposition, PolicyError, PolicyFinding, PolicyModality, PolicyPlaceholder,
    PolicyProfileId, PolicyReceipt, PolicyReceiptId, PolicyReceiptParts, TextRange,
};

#[cfg(test)]
mod tests;
