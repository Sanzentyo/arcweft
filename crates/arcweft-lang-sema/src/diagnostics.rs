//! Structured semantic diagnostics for type checking, traits, and warnings.

mod effect_trace;
mod error;
mod trait_diagnostic;
mod warning;

pub use error::{TypeCheckError, TypeCheckErrorKind, TypeCheckReadinessError};
pub use trait_diagnostic::{TraitDiagnostic, TraitDiagnosticKind};
pub use warning::{TypeCheckWarning, TypeCheckWarningKind};
