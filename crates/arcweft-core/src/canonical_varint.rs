//! One canonical unsigned LEB128 grammar for supported integer widths.

use thiserror::Error;

pub(crate) const MAX_U32_VARINT_BYTES: usize = 5;
pub(crate) const MAX_U64_VARINT_BYTES: usize = 10;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum CanonicalVarintError {
    #[error("canonical unsigned varint is truncated after {consumed} bytes")]
    Truncated { consumed: usize },
    #[error("canonical unsigned varint is noncanonical or overflows its width")]
    NonCanonical,
}

#[derive(Clone, Copy)]
pub(crate) enum CanonicalVarintWidth {
    U32,
    U64,
}

impl CanonicalVarintWidth {
    pub(crate) fn decode(self, bytes: &[u8]) -> Result<(u64, usize), CanonicalVarintError> {
        let (maximum, final_payload) = match self {
            Self::U32 => (MAX_U32_VARINT_BYTES, 0x0f),
            Self::U64 => (MAX_U64_VARINT_BYTES, 0x01),
        };
        let available = bytes.len().min(maximum);
        let mut value = 0_u64;
        for (index, &byte) in bytes.iter().take(available).enumerate() {
            let payload = byte & 0x7f;
            if index == maximum - 1 && payload > final_payload {
                return Err(CanonicalVarintError::NonCanonical);
            }
            value |= u64::from(payload) << (index * 7);
            if byte & 0x80 == 0 {
                if index != 0 && payload == 0 {
                    return Err(CanonicalVarintError::NonCanonical);
                }
                return Ok((value, index + 1));
            }
        }
        if available < maximum {
            Err(CanonicalVarintError::Truncated {
                consumed: available,
            })
        } else {
            Err(CanonicalVarintError::NonCanonical)
        }
    }
}

pub(crate) fn encode_u64(mut value: u64) -> ([u8; MAX_U64_VARINT_BYTES], usize) {
    let mut encoded = [0; MAX_U64_VARINT_BYTES];
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

pub(crate) fn encode_u32(value: u32) -> ([u8; MAX_U32_VARINT_BYTES], usize) {
    let (wide, length) = encode_u64(u64::from(value));
    let mut encoded = [0; MAX_U32_VARINT_BYTES];
    encoded.copy_from_slice(&wide[..MAX_U32_VARINT_BYTES]);
    (encoded, length)
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
            assert_eq!(
                CanonicalVarintWidth::U32.decode(&encoded[..length]),
                Ok((u64::from(value), length))
            );
        }
    }

    #[test]
    fn decodes_only_the_value_prefix_without_copying() {
        assert_eq!(
            CanonicalVarintWidth::U32.decode(&[0x80, 0x01, 0xaa]),
            Ok((128, 2))
        );
    }

    #[test]
    fn reports_truncation_with_the_consumed_prefix_length() {
        assert_eq!(
            CanonicalVarintWidth::U32.decode(&[]),
            Err(CanonicalVarintError::Truncated { consumed: 0 })
        );
        assert_eq!(
            CanonicalVarintWidth::U32.decode(&[0x80]),
            Err(CanonicalVarintError::Truncated { consumed: 1 })
        );
        assert_eq!(
            CanonicalVarintWidth::U32.decode(&[0x80, 0x80, 0x80, 0x80]),
            Err(CanonicalVarintError::Truncated { consumed: 4 })
        );
    }

    #[test]
    fn rejects_overlong_and_alternate_encodings() {
        for bytes in [[0x80, 0x00], [0x81, 0x00], [0xff, 0x00]] {
            assert_eq!(
                CanonicalVarintWidth::U32.decode(&bytes),
                Err(CanonicalVarintError::NonCanonical)
            );
        }
        assert_eq!(
            CanonicalVarintWidth::U32.decode(&[0x80, 0x80, 0x80, 0x80, 0x00]),
            Err(CanonicalVarintError::NonCanonical)
        );
    }

    #[test]
    fn rejects_u32_overflow_and_sixth_byte_continuations() {
        assert_eq!(
            CanonicalVarintWidth::U32.decode(&[0xff, 0xff, 0xff, 0xff, 0x10]),
            Err(CanonicalVarintError::NonCanonical)
        );
        assert_eq!(
            CanonicalVarintWidth::U32.decode(&[0xff, 0xff, 0xff, 0xff, 0x80]),
            Err(CanonicalVarintError::NonCanonical)
        );
        assert_eq!(
            CanonicalVarintWidth::U32.decode(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x00]),
            Err(CanonicalVarintError::NonCanonical)
        );
    }
}

#[cfg(test)]
mod wide_tests {
    use super::*;
    #[test]
    fn canonical_u64_boundaries_and_invalid_encodings() {
        for value in [
            0,
            127,
            128,
            u64::from(u32::MAX),
            u64::from(u32::MAX) + 1,
            u64::MAX,
        ] {
            let (bytes, length) = encode_u64(value);
            assert_eq!(
                CanonicalVarintWidth::U64.decode(&bytes[..length]),
                Ok((value, length))
            );
        }
        assert_eq!(
            CanonicalVarintWidth::U64.decode(&[0x80; 9]),
            Err(CanonicalVarintError::Truncated { consumed: 9 })
        );
        for bytes in [
            vec![0x81, 0],
            vec![0x80; 10],
            vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 2],
        ] {
            assert_eq!(
                CanonicalVarintWidth::U64.decode(&bytes),
                Err(CanonicalVarintError::NonCanonical)
            );
        }
    }
}
