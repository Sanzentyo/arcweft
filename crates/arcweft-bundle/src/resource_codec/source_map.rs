//! Canonical multi-source product section and source-bound validation metadata.

mod codec;
mod error;
mod model;

pub use error::{SourceMapBuildError, SourceMapCodecError};
pub use model::{
    MAX_SOURCE_BYTES_PER_DOCUMENT, MAX_SOURCE_DISPLAY_NAME_BYTES, MAX_SOURCE_MAP_DOCUMENTS,
    MAX_SOURCE_MAP_TOTAL_UTF8_BYTES, SourceMapDocument, SourceMapSection,
};
