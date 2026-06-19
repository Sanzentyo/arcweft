use thiserror::Error;

/// Invalid little-endian f32 blob.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("f32 blob length must be divisible by four")]
pub struct VectorBlobError;

/// Encodes finite or non-finite f32 values without unsafe memory casts.
pub fn encode_f32_le(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// Decodes a little-endian contiguous f32 blob without alignment assumptions.
pub fn decode_f32_le(bytes: &[u8]) -> Result<Vec<f32>, VectorBlobError> {
    if !bytes.len().is_multiple_of(4) {
        return Err(VectorBlobError);
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_blob_round_trips() {
        let values = [1.0_f32, -2.5, 0.125];
        assert_eq!(decode_f32_le(&encode_f32_le(&values)), Ok(values.to_vec()));
    }
}
