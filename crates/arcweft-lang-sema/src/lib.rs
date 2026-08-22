//! Semantic analysis for Arcweft HIR.
//!
//! This crate owns name resolution, symbol collection, and the current minimal
//! type-checking pass. It depends on parsed syntax and HIR, but parser/runtime
//! crates do not depend on it.

pub mod assertion;
pub mod callable;
pub mod character_definition;
pub mod character_dialogue;
pub mod checked_rich_text;
pub mod dialogue_view;
pub mod effect_analysis;
pub mod effect_catalog;
pub mod effect_diagnostics;
pub mod effect_model;
pub mod effect_row;
pub mod effects;
pub mod entry;
pub mod env;
pub mod final_analysis;
pub mod nominal;
pub mod ownership;
pub use ownership::{
    CheckedOwnershipCertificate, CheckedOwnershipError, CheckedOwnershipLimits,
    OwnershipEvidenceDigest, RetainedValueDisposition,
};
mod producer_admission;
pub use producer_admission::{
    CheckedNeedProducerAdmission, CheckedNeedProducerAdmissionDigest,
    CheckedNeedProducerAdmissionError, CheckedProducerArgumentAdmission,
};
pub mod project_index;
pub mod proof_return;
pub mod registration;
pub mod signature;
pub mod types;
