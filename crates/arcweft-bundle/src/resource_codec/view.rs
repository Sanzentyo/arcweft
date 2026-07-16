//! Compact product resource codecs for Arcweft View, style, text, input, and theme data.
//!
//! This seq-02.4.1 cut keeps `arcweft-bundle` Sans I/O: it serializes already
//! lowered, typed View resources into the shared seq02.1 compact envelope and does
//! not parse authoring syntax, allocate platform IME adapters, or
//! render View. Product AWFB decode accepts only compact envelope magic for View
//! families; JSON exists only as deterministic transcript bytes and human
//! inspection/export output.

mod codec;
mod compat;
mod dialogue_contract;
mod merge;
mod model;
mod runtime_control_style;
mod style_contract;
mod validated;

pub use codec::{
    ViewExportValidationError, ViewResourceBudget, ViewResourceExport,
    ViewStyleEnvironmentSourceError,
};
pub use compat::{ViewResourceCompatibility, migrated_view_section_compatibility};
pub use dialogue_contract::DialogueViewContractError;
pub use merge::{ViewProgramStyleResources, ViewResourceMergeError};
pub use model::*;
pub use runtime_control_style::*;
pub use style_contract::ViewStyleContractError;
pub use validated::{
    ValidatedViewProduct, ValidatedViewProgramResource, ViewProductValidationError,
    ViewProductValidationLimits,
};
