//! Checked owner-local View-part catalog and diagnostics.

mod catalog;
mod check;
mod diagnostic;

pub use catalog::{
    CheckedViewId, CheckedViewLocalPart, CheckedViewPartCatalog, CheckedViewPartExport,
    CheckedViewPartExportSource, CheckedViewPartId, CheckedViewPartOccurrenceShape,
    CheckedViewPartOwner, CheckedViewPartRef, CheckedViewPartTargetKind,
};
pub use check::check_view_parts;
pub use diagnostic::{ViewPartDiagnostic, ViewPartDiagnosticCode};
