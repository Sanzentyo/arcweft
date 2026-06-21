#![forbid(unsafe_code)]
//! Optional serde bridge. Arcweft core does not use serde to implement its own
//! encode/decode traits; this crate is for third-party Rust interop.

use arcweft_data::{BytesFormat, DataError, DataErrorKind, Result, Value};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Serialize a serde-compatible external Rust value into Arcweft dynamic data.
pub fn to_arcweft_value<T: Serialize>(value: &T) -> Result<Value> {
    let json = serde_json::to_value(value)
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))?;
    arcweft_codec_json::from_json_value(&json)
}

/// Deserialize a serde-compatible external Rust value from Arcweft dynamic data.
pub fn from_arcweft_value<T: DeserializeOwned>(value: &Value) -> Result<T> {
    let json = arcweft_codec_json::to_json_value(value, BytesFormat::Base64)?;
    serde_json::from_value(json)
        .map_err(|error| DataError::new(DataErrorKind::InvalidEncoding, error.to_string()))
}

/// Re-export optimized serde byte wrappers for adapter authors.
pub mod bytes {
    pub use serde_bytes::{ByteArray, ByteBuf, Bytes};
}

/// Re-export serde repr derives for adapter-only Rust DTOs.
pub mod repr {
    pub use serde_repr::{Deserialize_repr, Serialize_repr};
}
