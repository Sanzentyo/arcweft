//! Canonical integer primitives shared by Arcweft identity and wire owners.

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CanonicalVarintDecodeError {
    #[error("canonical unsigned varint is truncated")]
    Truncated,
    #[error("canonical unsigned varint overflows u64")]
    Overflow,
    #[error("canonical unsigned varint is not shortest")]
    NonCanonical,
}

/// Appends one shortest unsigned LEB128 value to a canonical transcript.
pub fn append_canonical_varint(output: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// Returns the encoded length of one shortest unsigned LEB128 value.
pub const fn canonical_varint_len(mut value: u64) -> usize {
    let mut length = 1;
    while value >= 0x80 {
        value >>= 7;
        length += 1;
    }
    length
}

/// Decodes one shortest unsigned LEB128 value without consuming trailing input.
pub fn decode_canonical_varint(input: &[u8]) -> Result<(u64, usize), CanonicalVarintDecodeError> {
    let mut value = 0_u64;
    for index in 0..10 {
        let byte = *input
            .get(index)
            .ok_or(CanonicalVarintDecodeError::Truncated)?;
        let payload = byte & 0x7f;
        if index == 9 && (payload > 1 || byte & 0x80 != 0) {
            return Err(CanonicalVarintDecodeError::Overflow);
        }
        value |= u64::from(payload) << (index * 7);
        if byte & 0x80 == 0 {
            if index != 0 && payload == 0 {
                return Err(CanonicalVarintDecodeError::NonCanonical);
            }
            return Ok((value, index + 1));
        }
    }
    Err(CanonicalVarintDecodeError::Overflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_varint_round_trips_boundaries_and_rejects_invalid_forms() {
        assert_eq!(canonical_varint_len(0), 1);
        assert_eq!(canonical_varint_len(127), 1);
        assert_eq!(canonical_varint_len(128), 2);
        assert_eq!(canonical_varint_len(u64::MAX), 10);

        let mut encoded = Vec::new();
        append_canonical_varint(&mut encoded, u64::MAX);
        assert_eq!(decode_canonical_varint(&encoded), Ok((u64::MAX, 10)));
        assert_eq!(decode_canonical_varint(&[0]), Ok((0, 1)));
        assert_eq!(decode_canonical_varint(&[0x80, 1, 9]), Ok((128, 2)));
        assert_eq!(
            decode_canonical_varint(&[0x80, 0]),
            Err(CanonicalVarintDecodeError::NonCanonical)
        );
        assert_eq!(
            decode_canonical_varint(&[0x80]),
            Err(CanonicalVarintDecodeError::Truncated)
        );
        assert_eq!(
            decode_canonical_varint(&[0xff; 10]),
            Err(CanonicalVarintDecodeError::Overflow)
        );
    }
}
