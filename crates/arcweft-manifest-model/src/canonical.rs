use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;
use thiserror::Error;

/// Failure while producing Arcweft's deterministic semantic JSON encoding.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CanonicalJsonError {
    #[error("failed to project a semantic value to JSON: {0}")]
    Projection(String),
    #[error("canonical semantic JSON does not admit null values")]
    Null,
    #[error("canonical semantic JSON does not admit floating-point values")]
    Float,
}

/// Encodes a serializable semantic value using Arcweft's canonical JSON rules.
pub fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, CanonicalJsonError> {
    let value = serde_json::to_value(value)
        .map_err(|error| CanonicalJsonError::Projection(error.to_string()))?;
    let mut output = Vec::new();
    write_value(&value, &mut output)?;
    Ok(output)
}

fn write_value(value: &Value, output: &mut Vec<u8>) -> Result<(), CanonicalJsonError> {
    match value {
        Value::Null => Err(CanonicalJsonError::Null),
        Value::Bool(value) => {
            output.extend_from_slice(if *value { b"true" } else { b"false" });
            Ok(())
        }
        Value::Number(value) => {
            if value.is_f64() {
                return Err(CanonicalJsonError::Float);
            }
            output.extend_from_slice(value.to_string().as_bytes());
            Ok(())
        }
        Value::String(value) => {
            let encoded = serde_json::to_string(value)
                .map_err(|error| CanonicalJsonError::Projection(error.to_string()))?;
            output.extend_from_slice(encoded.as_bytes());
            Ok(())
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_value(value, output)?;
            }
            output.push(b']');
            Ok(())
        }
        Value::Object(values) => {
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| compare_utf8(left, right));
            output.push(b'{');
            for (index, (key, value)) in entries.into_iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                let encoded = serde_json::to_string(key)
                    .map_err(|error| CanonicalJsonError::Projection(error.to_string()))?;
                output.extend_from_slice(encoded.as_bytes());
                output.push(b':');
                write_value(value, output)?;
            }
            output.push(b'}');
            Ok(())
        }
    }
}

fn compare_utf8(left: &str, right: &str) -> Ordering {
    left.as_bytes().cmp(right.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::{CanonicalJsonError, canonical_json_bytes};
    use serde::Serialize;
    use serde_json::json;

    #[derive(Serialize)]
    struct Projection<'a> {
        z: u64,
        a: &'a str,
    }

    #[test]
    fn sorts_object_keys_and_uses_compact_integer_encoding() {
        let bytes = canonical_json_bytes(&Projection { z: 12, a: "é" }).unwrap();
        assert_eq!(bytes, r#"{"a":"é","z":12}"#.as_bytes());
    }

    #[test]
    fn rejects_null_and_floating_point_values() {
        assert_eq!(
            canonical_json_bytes(&json!({ "value": null })),
            Err(CanonicalJsonError::Null)
        );
        assert_eq!(
            canonical_json_bytes(&json!({ "value": 1.5 })),
            Err(CanonicalJsonError::Float)
        );
    }
}
