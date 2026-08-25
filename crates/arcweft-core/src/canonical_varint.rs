//! Canonical unsigned LEB128 primitives for `u32` values.

use thiserror::Error;

/// Maximum number of bytes in a canonical unsigned `u32` varint.
pub(crate) const MAX_U32_VARINT_BYTES: usize = 5;

/// Failure to decode a canonical unsigned `u32` varint.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum CanonicalU32VarintError {
    /// The input ended before a terminating byte was available.
    #[error("canonical u32 varint is truncated after {consumed} bytes")]
    Truncated { consumed: usize },

    /// The input is overlong, uses an alternate encoding, or overflows `u32`.
    #[error("canonical u32 varint is noncanonical or overflows u32")]
    NonCanonical,
}

/// Encodes `value` as its shortest unsigned LEB128 representation.
pub(crate) fn encode_u32(value: u32) -> ([u8; MAX_U32_VARINT_BYTES], usize) {
    let mut value = value;
    let mut encoded = [0; MAX_U32_VARINT_BYTES];
    let mut length = 0;

    loop {
        let payload = (value & 0x7f) as u8;
        value >>= 7;

        if value == 0 {
            encoded[length] = payload;
            return (encoded, length + 1);
        }

        encoded[length] = payload | 0x80;
        length += 1;
    }
}

/// Decodes one canonical unsigned LEB128 `u32` value from the input prefix.
///
/// The returned length identifies the consumed prefix, so bytes after the
/// terminating byte remain available to the caller. No input allocation or
/// copying is performed.
pub(crate) fn decode_u32(bytes: &[u8]) -> Result<(u32, usize), CanonicalU32VarintError> {
    let available = bytes.len().min(MAX_U32_VARINT_BYTES);
    let mut value = 0_u32;

    for (index, &byte) in bytes.iter().take(available).enumerate() {
        let payload = byte & 0x7f;

        // Only four payload bits are available in the fifth byte of a u32.
        if index == MAX_U32_VARINT_BYTES - 1 && payload > 0x0f {
            return Err(CanonicalU32VarintError::NonCanonical);
        }

        value |= u32::from(payload) << (index * 7);

        if byte & 0x80 == 0 {
            // A zero terminal payload after a continuation byte is an
            // overlong representation of the value accumulated so far.
            if index != 0 && payload == 0 {
                return Err(CanonicalU32VarintError::NonCanonical);
            }
            return Ok((value, index + 1));
        }
    }

    if available < MAX_U32_VARINT_BYTES {
        Err(CanonicalU32VarintError::Truncated {
            consumed: available,
        })
    } else {
        // A sixth byte would be required, which is outside the u32 varint
        // width and therefore cannot be canonical.
        Err(CanonicalU32VarintError::NonCanonical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_exact_boundary_values() {
        let cases = [
            (0, [0, 0, 0, 0, 0], 1),
            (127, [0x7f, 0, 0, 0, 0], 1),
            (128, [0x80, 0x01, 0, 0, 0], 2),
            (u32::MAX, [0xff, 0xff, 0xff, 0xff, 0x0f], 5),
        ];

        for (value, expected, expected_length) in cases {
            let (encoded, length) = encode_u32(value);
            assert_eq!(encoded, expected);
            assert_eq!(length, expected_length);
            assert_eq!(decode_u32(&encoded[..length]), Ok((value, length)));
        }
    }

    #[test]
    fn decodes_only_the_value_prefix_without_copying() {
        assert_eq!(decode_u32(&[0x80, 0x01, 0xaa]), Ok((128, 2)));
    }

    #[test]
    fn reports_truncation_with_the_consumed_prefix_length() {
        assert_eq!(
            decode_u32(&[]),
            Err(CanonicalU32VarintError::Truncated { consumed: 0 })
        );
        assert_eq!(
            decode_u32(&[0x80]),
            Err(CanonicalU32VarintError::Truncated { consumed: 1 })
        );
        assert_eq!(
            decode_u32(&[0x80, 0x80, 0x80, 0x80]),
            Err(CanonicalU32VarintError::Truncated { consumed: 4 })
        );
    }

    #[test]
    fn rejects_overlong_and_alternate_encodings() {
        for bytes in [[0x80, 0x00], [0x81, 0x00], [0xff, 0x00]] {
            assert_eq!(
                decode_u32(&bytes),
                Err(CanonicalU32VarintError::NonCanonical)
            );
        }
        assert_eq!(
            decode_u32(&[0x80, 0x80, 0x80, 0x80, 0x00]),
            Err(CanonicalU32VarintError::NonCanonical)
        );
    }

    #[test]
    fn rejects_u32_overflow_and_sixth_byte_continuations() {
        assert_eq!(
            decode_u32(&[0xff, 0xff, 0xff, 0xff, 0x10]),
            Err(CanonicalU32VarintError::NonCanonical)
        );
        assert_eq!(
            decode_u32(&[0xff, 0xff, 0xff, 0xff, 0x80]),
            Err(CanonicalU32VarintError::NonCanonical)
        );
        assert_eq!(
            decode_u32(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x00]),
            Err(CanonicalU32VarintError::NonCanonical)
        );
    }
}
