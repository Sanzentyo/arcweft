//! Semantic checking and checked catalog for typed View styles.

pub mod catalog;
pub mod check;
pub mod diagnostic;
pub mod token_graph;
pub mod value;

pub use catalog::{
    CheckedViewStyleCatalog, CheckedViewStyleDeclaration, CheckedViewStylePatch,
    CheckedViewStyleRule, CheckedViewStyleSheet, CheckedViewStyleToken,
};
pub use check::check_view_styles;
pub use diagnostic::{StyleDiagnostic, StyleDiagnosticCode};
