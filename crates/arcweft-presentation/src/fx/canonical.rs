//! Shared canonical v1 primitives for Fx-owned codecs and digests.

use thiserror::Error;

/// Failure to measure or materialize a canonical representation exactly.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(super) enum CanonicalEncodeError {
    #[error("canonical length overflow")]
    LengthOverflow,
    #[error("canonical allocation failed")]
    AllocationFailed,
    #[error("canonical encoding wrote {actual} bytes, expected {expected} bytes")]
    LengthMismatch { actual: usize, expected: usize },
}

/// Rejection from the paired canonical reader.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum FxCanonicalDecodeError {
    #[error("canonical input is truncated")]
    Truncated,
    #[error("canonical boolean tag {0} is invalid")]
    InvalidBool(u8),
    #[error("canonical unsigned varint overflows u64")]
    VarintOverflow,
    #[error("canonical unsigned varint is not shortest")]
    NonCanonicalVarint,
    #[error("canonical length does not fit the host width")]
    LengthOverflow,
    #[error("canonical string is not UTF-8")]
    InvalidUtf8,
    #[error("canonical domain prefix does not match the expected owner")]
    DomainMismatch,
    #[error("canonical domain terminator byte {0} is invalid")]
    InvalidDomainTerminator(u8),
    #[error("canonical version {0} is unsupported")]
    UnsupportedVersion(u8),
    #[error("canonical input has {0} trailing bytes")]
    TrailingBytes(usize),
}

/// Byte sink shared by canonical measurement, materialization, and hashing.
pub(super) trait CanonicalSink {
    type Error;

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;
}

/// Typed canonical primitive writer over one sink.
pub(super) struct CanonicalEncoder<S> {
    sink: S,
}

/// Checked no-allocation canonical byte counter.
#[derive(Default)]
pub(super) struct CanonicalLengthSink {
    length: usize,
}

/// Preflighted canonical writer into a caller-owned byte vector.
pub(super) struct CanonicalVecSink<'a> {
    output: &'a mut Vec<u8>,
    start: usize,
    expected: usize,
}

/// Streaming canonical writer into a BLAKE3 transcript.
pub(super) struct CanonicalHashSink<'a> {
    hasher: &'a mut blake3::Hasher,
}

/// Borrowed reader for the exact primitives emitted by [`CanonicalEncoder`].
pub(super) struct CanonicalReader<'a> {
    input: &'a [u8],
    cursor: usize,
}

impl<S> CanonicalEncoder<S> {
    pub(super) const fn new(sink: S) -> Self {
        Self { sink }
    }

    pub(super) fn into_inner(self) -> S {
        self.sink
    }
}

impl<S: CanonicalSink> CanonicalEncoder<S> {
    /// Writes one exact ASCII owner domain, its zero terminator, and version 1.
    pub(super) fn domain_v1(&mut self, domain: &[u8]) -> Result<(), S::Error> {
        self.raw_bytes(domain)?;
        self.tag(0)?;
        self.tag(1)
    }

    pub(super) fn raw_bytes(&mut self, bytes: &[u8]) -> Result<(), S::Error> {
        self.sink.write(bytes)
    }

    pub(super) fn tag(&mut self, tag: u8) -> Result<(), S::Error> {
        self.raw_bytes(&[tag])
    }

    pub(super) fn boolean(&mut self, value: bool) -> Result<(), S::Error> {
        self.tag(u8::from(value))
    }

    /// Writes the shortest unsigned LEB128 representation of `value`.
    pub(super) fn unsigned(&mut self, mut value: u64) -> Result<(), S::Error> {
        let mut bytes = [0_u8; 10];
        let mut length = 0;
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            bytes[length] = byte;
            length += 1;
            if value == 0 {
                break;
            }
        }
        self.raw_bytes(&bytes[..length])
    }

    /// Writes an `i32` as zigzag-projected shortest unsigned LEB128.
    pub(super) fn signed_i32(&mut self, value: i32) -> Result<(), S::Error> {
        let magnitude = u64::from(value.unsigned_abs());
        let zigzag = if value < 0 {
            magnitude * 2 - 1
        } else {
            magnitude * 2
        };
        self.unsigned(zigzag)
    }

    pub(super) fn digest32(&mut self, digest: &[u8; 32]) -> Result<(), S::Error> {
        self.raw_bytes(digest)
    }

    /// Writes one canonical IEEE-754 single-precision word in little endian.
    pub(super) fn f32_bits(&mut self, bits: u32) -> Result<(), S::Error> {
        self.raw_bytes(&bits.to_le_bytes())
    }
}

impl CanonicalSink for CanonicalLengthSink {
    type Error = CanonicalEncodeError;

    fn write(&mut self, bytes: &[u8]) -> Result<(), CanonicalEncodeError> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .ok_or(CanonicalEncodeError::LengthOverflow)?;
        Ok(())
    }
}

impl CanonicalLengthSink {
    pub(super) const fn finish(self) -> usize {
        self.length
    }
}

impl<'a> CanonicalVecSink<'a> {
    pub(super) fn with_preflight(
        output: &'a mut Vec<u8>,
        expected: usize,
    ) -> Result<Self, CanonicalEncodeError> {
        output
            .len()
            .checked_add(expected)
            .ok_or(CanonicalEncodeError::LengthOverflow)?;
        output
            .try_reserve_exact(expected)
            .map_err(|_| CanonicalEncodeError::AllocationFailed)?;
        let start = output.len();
        Ok(Self {
            output,
            start,
            expected,
        })
    }

    pub(super) fn finish(self) -> Result<(), CanonicalEncodeError> {
        let actual = self
            .output
            .len()
            .checked_sub(self.start)
            .ok_or(CanonicalEncodeError::LengthOverflow)?;
        if actual != self.expected {
            return Err(CanonicalEncodeError::LengthMismatch {
                actual,
                expected: self.expected,
            });
        }
        Ok(())
    }
}

impl CanonicalSink for CanonicalVecSink<'_> {
    type Error = CanonicalEncodeError;

    fn write(&mut self, bytes: &[u8]) -> Result<(), CanonicalEncodeError> {
        let actual = self
            .output
            .len()
            .checked_sub(self.start)
            .and_then(|written| written.checked_add(bytes.len()))
            .ok_or(CanonicalEncodeError::LengthOverflow)?;
        if actual > self.expected {
            return Err(CanonicalEncodeError::LengthMismatch {
                actual,
                expected: self.expected,
            });
        }
        self.output.extend_from_slice(bytes);
        Ok(())
    }
}

impl<'a> CanonicalHashSink<'a> {
    pub(super) const fn new(hasher: &'a mut blake3::Hasher) -> Self {
        Self { hasher }
    }
}

impl CanonicalSink for CanonicalHashSink<'_> {
    type Error = std::convert::Infallible;

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.hasher.update(bytes);
        Ok(())
    }
}

impl<'a> CanonicalReader<'a> {
    pub(super) const fn new(input: &'a [u8]) -> Self {
        Self { input, cursor: 0 }
    }

    pub(super) fn finish(self) -> Result<(), FxCanonicalDecodeError> {
        let trailing = self.input.len() - self.cursor;
        if trailing == 0 {
            Ok(())
        } else {
            Err(FxCanonicalDecodeError::TrailingBytes(trailing))
        }
    }

    pub(super) fn domain_v1(&mut self, expected: &[u8]) -> Result<(), FxCanonicalDecodeError> {
        if self.raw_bytes(expected.len())? != expected {
            return Err(FxCanonicalDecodeError::DomainMismatch);
        }
        let terminator = self.tag()?;
        if terminator != 0 {
            return Err(FxCanonicalDecodeError::InvalidDomainTerminator(terminator));
        }
        let version = self.tag()?;
        if version != 1 {
            return Err(FxCanonicalDecodeError::UnsupportedVersion(version));
        }
        Ok(())
    }

    pub(super) fn tag(&mut self) -> Result<u8, FxCanonicalDecodeError> {
        let [value] = self.raw_fixed::<1>()?;
        Ok(value)
    }

    pub(super) fn boolean(&mut self) -> Result<bool, FxCanonicalDecodeError> {
        match self.tag()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(FxCanonicalDecodeError::InvalidBool(value)),
        }
    }

    /// Reads a shortest unsigned LEB128 value and rejects every alternative
    /// representation of the same integer.
    pub(super) fn unsigned(&mut self) -> Result<u64, FxCanonicalDecodeError> {
        let input = self
            .input
            .get(self.cursor..)
            .ok_or(FxCanonicalDecodeError::Truncated)?;
        let (value, consumed) =
            arcweft_id::canonical::decode_canonical_varint(input).map_err(|error| match error {
                arcweft_id::canonical::CanonicalVarintDecodeError::Truncated => {
                    FxCanonicalDecodeError::Truncated
                }
                arcweft_id::canonical::CanonicalVarintDecodeError::Overflow => {
                    FxCanonicalDecodeError::VarintOverflow
                }
                arcweft_id::canonical::CanonicalVarintDecodeError::NonCanonical => {
                    FxCanonicalDecodeError::NonCanonicalVarint
                }
            })?;
        self.cursor = self
            .cursor
            .checked_add(consumed)
            .ok_or(FxCanonicalDecodeError::LengthOverflow)?;
        Ok(value)
    }

    pub(super) fn signed_i32(&mut self) -> Result<i32, FxCanonicalDecodeError> {
        let encoded = self.unsigned()?;
        let encoded = u32::try_from(encoded).map_err(|_| FxCanonicalDecodeError::VarintOverflow)?;
        let magnitude = (encoded >> 1).cast_signed();
        Ok(magnitude ^ -(encoded & 1).cast_signed())
    }

    pub(super) fn length(&mut self) -> Result<usize, FxCanonicalDecodeError> {
        usize::try_from(self.unsigned()?).map_err(|_| FxCanonicalDecodeError::LengthOverflow)
    }

    pub(super) fn digest32(&mut self) -> Result<[u8; 32], FxCanonicalDecodeError> {
        self.raw_fixed()
    }

    pub(super) fn f32_bits(&mut self) -> Result<u32, FxCanonicalDecodeError> {
        self.raw_fixed().map(u32::from_le_bytes)
    }

    pub(super) fn raw_fixed<const N: usize>(&mut self) -> Result<[u8; N], FxCanonicalDecodeError> {
        let bytes = self.raw_bytes(N)?;
        let mut output = [0; N];
        output.copy_from_slice(bytes);
        Ok(output)
    }

    pub(super) fn raw_bytes(&mut self, length: usize) -> Result<&'a [u8], FxCanonicalDecodeError> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(FxCanonicalDecodeError::LengthOverflow)?;
        let bytes = self
            .input
            .get(self.cursor..end)
            .ok_or(FxCanonicalDecodeError::Truncated)?;
        self.cursor = end;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_leb128_boundaries_are_shortest() {
        let cases = [
            (0, &[0][..]),
            (127, &[0x7f][..]),
            (128, &[0x80, 0x01][..]),
            (16_383, &[0xff, 0x7f][..]),
            (16_384, &[0x80, 0x80, 0x01][..]),
            (
                u64::MAX,
                &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01][..],
            ),
        ];
        for (value, expected) in cases {
            let mut bytes = Vec::new();
            let sink = CanonicalVecSink::with_preflight(&mut bytes, expected.len()).unwrap();
            let mut encoder = CanonicalEncoder::new(sink);
            encoder.unsigned(value).unwrap();
            encoder.into_inner().finish().unwrap();
            assert_eq!(bytes, expected);
        }
    }

    #[test]
    fn signed_i32_uses_zigzag_varints() {
        let cases = [
            (0, vec![0]),
            (-1, vec![1]),
            (1, vec![2]),
            (i32::MIN, vec![0xff, 0xff, 0xff, 0xff, 0x0f]),
            (i32::MAX, vec![0xfe, 0xff, 0xff, 0xff, 0x0f]),
        ];
        for (value, expected) in cases {
            let mut bytes = Vec::new();
            let sink = CanonicalVecSink::with_preflight(&mut bytes, expected.len()).unwrap();
            let mut encoder = CanonicalEncoder::new(sink);
            encoder.signed_i32(value).unwrap();
            encoder.into_inner().finish().unwrap();
            assert_eq!(bytes, expected);
        }
    }

    #[test]
    fn reader_round_trips_boundary_varints_and_i32() {
        let unsigned = [0, 127, 128, 16_383, 16_384, u64::MAX];
        let signed = [i32::MIN, -1, 0, 1, i32::MAX];
        let mut counter = CanonicalEncoder::new(CanonicalLengthSink::default());
        for value in unsigned {
            counter.unsigned(value).unwrap();
        }
        for value in signed {
            counter.signed_i32(value).unwrap();
        }
        let length = counter.into_inner().finish();
        let mut bytes = Vec::new();
        let sink = CanonicalVecSink::with_preflight(&mut bytes, length).unwrap();
        let mut encoder = CanonicalEncoder::new(sink);
        for value in unsigned {
            encoder.unsigned(value).unwrap();
        }
        for value in signed {
            encoder.signed_i32(value).unwrap();
        }
        encoder.into_inner().finish().unwrap();

        let mut reader = CanonicalReader::new(&bytes);
        for expected in unsigned {
            assert_eq!(reader.unsigned().unwrap(), expected);
        }
        for expected in signed {
            assert_eq!(reader.signed_i32().unwrap(), expected);
        }
        reader.finish().unwrap();
    }

    #[test]
    fn reader_rejects_overlong_overflow_truncation_and_trailing_bytes() {
        assert_eq!(
            CanonicalReader::new(&[0x80, 0x00]).unsigned(),
            Err(FxCanonicalDecodeError::NonCanonicalVarint)
        );
        assert_eq!(
            CanonicalReader::new(&[0xff; 10]).unsigned(),
            Err(FxCanonicalDecodeError::VarintOverflow)
        );
        assert_eq!(
            CanonicalReader::new(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x02])
                .unsigned(),
            Err(FxCanonicalDecodeError::VarintOverflow)
        );
        assert_eq!(
            CanonicalReader::new(&[0x80]).unsigned(),
            Err(FxCanonicalDecodeError::Truncated)
        );
        assert_eq!(
            CanonicalReader::new(&[2]).boolean(),
            Err(FxCanonicalDecodeError::InvalidBool(2))
        );
        assert_eq!(
            CanonicalReader::new(&[0, 1]).finish(),
            Err(FxCanonicalDecodeError::TrailingBytes(2))
        );
        assert_eq!(
            CanonicalReader::new(b"wrong\0\x01").domain_v1(b"owner"),
            Err(FxCanonicalDecodeError::DomainMismatch)
        );
        assert_eq!(
            CanonicalReader::new(b"owner\x01\x01").domain_v1(b"owner"),
            Err(FxCanonicalDecodeError::InvalidDomainTerminator(1))
        );
        assert_eq!(
            CanonicalReader::new(b"owner\0\x02").domain_v1(b"owner"),
            Err(FxCanonicalDecodeError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn reader_mirrors_fixed_and_length_prefixed_primitives() {
        let digest = [0x5a; 32];
        let mut counter = CanonicalEncoder::new(CanonicalLengthSink::default());
        counter.domain_v1(b"arcweft.test").unwrap();
        counter.boolean(false).unwrap();
        counter.unsigned(3).unwrap();
        counter.raw_bytes(&[1, 2, 3]).unwrap();
        counter.unsigned(2).unwrap();
        counter.raw_bytes(b"fx").unwrap();
        counter.digest32(&digest).unwrap();
        counter.f32_bits((-0.5_f32).to_bits()).unwrap();
        let length = counter.into_inner().finish();

        let mut bytes = Vec::new();
        let sink = CanonicalVecSink::with_preflight(&mut bytes, length).unwrap();
        let mut encoder = CanonicalEncoder::new(sink);
        encoder.domain_v1(b"arcweft.test").unwrap();
        encoder.boolean(false).unwrap();
        encoder.unsigned(3).unwrap();
        encoder.raw_bytes(&[1, 2, 3]).unwrap();
        encoder.unsigned(2).unwrap();
        encoder.raw_bytes(b"fx").unwrap();
        encoder.digest32(&digest).unwrap();
        encoder.f32_bits((-0.5_f32).to_bits()).unwrap();
        encoder.into_inner().finish().unwrap();

        let mut reader = CanonicalReader::new(&bytes);
        reader.domain_v1(b"arcweft.test").unwrap();
        assert!(!reader.boolean().unwrap());
        let byte_length = reader.length().unwrap();
        assert_eq!(reader.raw_bytes(byte_length).unwrap(), [1, 2, 3]);
        let string_length = reader.length().unwrap();
        assert_eq!(reader.raw_bytes(string_length).unwrap(), b"fx");
        assert_eq!(reader.digest32().unwrap(), digest);
        assert_eq!(reader.f32_bits().unwrap(), (-0.5_f32).to_bits());
        reader.finish().unwrap();
    }

    #[test]
    fn measurement_materialization_and_hashing_share_one_transcript() {
        fn write<S: CanonicalSink>(encoder: &mut CanonicalEncoder<S>)
        where
            S::Error: std::fmt::Debug,
        {
            encoder.tag(7).unwrap();
            encoder.boolean(true).unwrap();
            encoder.unsigned(4).unwrap();
            encoder.raw_bytes(b"wave").unwrap();
            encoder.f32_bits(1.0_f32.to_bits()).unwrap();
            encoder.digest32(&[0xa5; 32]).unwrap();
        }

        let mut counter = CanonicalEncoder::new(CanonicalLengthSink::default());
        write(&mut counter);
        let length = counter.into_inner().finish();

        let mut bytes = Vec::new();
        let sink = CanonicalVecSink::with_preflight(&mut bytes, length).unwrap();
        let mut materializer = CanonicalEncoder::new(sink);
        write(&mut materializer);
        materializer.into_inner().finish().unwrap();

        let mut hasher = blake3::Hasher::new();
        let mut transcript = CanonicalEncoder::new(CanonicalHashSink::new(&mut hasher));
        write(&mut transcript);
        assert_eq!(hasher.finalize(), blake3::hash(&bytes));
    }

    #[test]
    fn vector_sink_rejects_preflight_length_mismatch() {
        let mut bytes = Vec::new();
        let sink = CanonicalVecSink::with_preflight(&mut bytes, 1).unwrap();
        let mut encoder = CanonicalEncoder::new(sink);
        assert_eq!(
            encoder.raw_bytes(&[1, 2]),
            Err(CanonicalEncodeError::LengthMismatch {
                actual: 2,
                expected: 1
            })
        );
        assert_eq!(
            encoder.into_inner().finish(),
            Err(CanonicalEncodeError::LengthMismatch {
                actual: 0,
                expected: 1
            })
        );

        let mut nonempty = vec![0];
        let Err(error) = CanonicalVecSink::with_preflight(&mut nonempty, usize::MAX) else {
            panic!("overflowing preflight must fail before allocation");
        };
        assert_eq!(error, CanonicalEncodeError::LengthOverflow);
    }

    #[test]
    fn length_sink_rejects_overflow() {
        let mut sink = CanonicalLengthSink { length: usize::MAX };
        assert_eq!(sink.write(&[0]), Err(CanonicalEncodeError::LengthOverflow));
    }
}
