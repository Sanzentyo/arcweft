//! Compact product resource codecs for Arcweft UI, style, text, input, and theme data.
//!
//! This seq-02.4.1 cut keeps `arcweft-bundle` Sans I/O: it serializes already
//! lowered, typed UI resources into the shared seq02.1 compact envelope and does
//! not parse CSS, open external CSS files, allocate platform IME adapters, or
//! render UI. Product AWFB decode accepts only compact envelope magic for
//! migrated UI families; JSON exists only as deterministic transcript bytes and
//! human inspection/export output.

mod codec;
mod compat;
mod model;
mod runtime_control_style;

pub use codec::{UiResourceBudget, UiResourceExport};
pub use compat::{UiResourceCompatibility, migrated_ui_section_compatibility};
pub use model::*;
pub use runtime_control_style::*;
