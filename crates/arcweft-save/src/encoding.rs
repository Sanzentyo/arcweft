//! Bounded in-memory save output using the existing envelope grammar.

use std::io::{self, Write};

use serde::Serialize;

use super::{
    DataError, DataErrorKind, HEADER_LEN, MAGIC, Result, SaveEnvelope, SaveSchemaId,
    TYPED_JSON_CODEC_ID,
};

#[cfg(test)]
mod tests;

/// Limits for the same save envelope on either side of the codec boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SaveEnvelopeLimits {
    /// Complete header, identifiers, and encoded payload.
    pub max_envelope_bytes: u64,
    /// UTF-8 bytes in the schema identifier.
    pub max_schema_id_bytes: u32,
    /// UTF-8 bytes in the codec identifier.
    pub max_codec_id_bytes: u32,
    /// Encoded payload bytes, excluding the envelope header and identifiers.
    pub max_payload_bytes: u64,
}

impl Default for SaveEnvelopeLimits {
    fn default() -> Self {
        Self {
            max_envelope_bytes: 512 * 1024 * 1024,
            max_schema_id_bytes: 1024,
            max_codec_id_bytes: 256,
            max_payload_bytes: 256 * 1024 * 1024,
        }
    }
}

impl SaveEnvelopeLimits {
    /// Encodes directly into one bounded envelope buffer. Header limits are
    /// checked before invoking Serialize; payload writes fail before growth or
    /// copying would exceed either the payload or complete-envelope allowance.
    ///
    /// # Errors
    /// Returns an encoding error for serializer refusal, a limit error before
    /// output exceeds its allowance, or a range error for wire/address limits.
    pub fn encode_typed_json<T: Serialize + ?Sized>(
        &self,
        value: &T,
        schema_id: &SaveSchemaId,
        schema_version: u32,
    ) -> Result<Vec<u8>> {
        let mut output = SaveEnvelopeEncoder::new(
            schema_id.as_str(),
            schema_version,
            TYPED_JSON_CODEC_ID,
            *self,
            None,
        )?;
        if let Err(error) = serde_json::to_writer(&mut output, value) {
            return Err(output.failure.unwrap_or_else(|| {
                DataError::new(
                    DataErrorKind::InvalidEncoding,
                    format!("failed to encode typed JSON save payload: {error}"),
                )
            }));
        }
        output.finish(None)
    }
}

impl SaveEnvelope {
    /// Encodes this envelope under the default output limits.
    ///
    /// # Errors
    /// Returns a limit/range error before an oversized output is allocated.
    pub fn encode_bytes(&self) -> Result<Vec<u8>> {
        self.encode_bytes_with_limits(&SaveEnvelopeLimits::default())
    }

    /// Checks the complete known payload before allocating or copying output.
    ///
    /// # Errors
    /// Returns a limit/range error before an oversized output is allocated.
    pub fn encode_bytes_with_limits(&self, limits: &SaveEnvelopeLimits) -> Result<Vec<u8>> {
        let mut output = SaveEnvelopeEncoder::new(
            self.schema_id.as_str(),
            self.schema_version,
            &self.codec_id,
            *limits,
            Some(self.payload.len()),
        )?;
        output.append(&self.payload)?;
        output.finish(Some(self.payload_checksum))
    }
}

const PAYLOAD_LEN_OFFSET: usize = MAGIC.len() + 4 + 4;
const CHECKSUM_OFFSET: usize = PAYLOAD_LEN_OFFSET + 4;

/// No partially encoded buffer escapes this owner. Its first hard failure
/// remains final even if a custom serializer catches a Write error.
struct SaveEnvelopeEncoder {
    bytes: Vec<u8>,
    payload_start: usize,
    maximum_buffer_len: usize,
    limits: SaveEnvelopeLimits,
    failure: Option<DataError>,
}

impl SaveEnvelopeEncoder {
    fn new(
        schema: &str,
        schema_version: u32,
        codec: &str,
        limits: SaveEnvelopeLimits,
        known_payload_len: Option<usize>,
    ) -> Result<Self> {
        if schema.len() as u64 > u64::from(limits.max_schema_id_bytes) {
            return Err(DataError::limit("save schema id exceeds its byte limit"));
        }
        if codec.len() as u64 > u64::from(limits.max_codec_id_bytes) {
            return Err(DataError::limit("save codec id exceeds its byte limit"));
        }
        let schema_len = u32::try_from(schema.len())
            .map_err(|_| DataError::new(DataErrorKind::NumberOutOfRange, "schema id too long"))?;
        let codec_len = u32::try_from(codec.len())
            .map_err(|_| DataError::new(DataErrorKind::NumberOutOfRange, "codec id too long"))?;
        let payload_start = HEADER_LEN
            .checked_add(schema.len())
            .and_then(|length| length.checked_add(codec.len()))
            .ok_or_else(|| {
                DataError::new(
                    DataErrorKind::NumberOutOfRange,
                    "save header length overflow",
                )
            })?;
        if payload_start as u64 > limits.max_envelope_bytes {
            return Err(DataError::limit(
                "save header exceeds the envelope byte limit",
            ));
        }
        let maximum_payload_len = limits
            .max_payload_bytes
            .min(u64::from(u32::MAX))
            .min(limits.max_envelope_bytes - payload_start as u64);
        let maximum_buffer_len =
            usize::try_from(payload_start as u64 + maximum_payload_len).unwrap_or(usize::MAX);
        let mut output = Self {
            bytes: Vec::new(),
            payload_start,
            maximum_buffer_len,
            limits,
            failure: None,
        };
        let initial_capacity = match known_payload_len {
            Some(length) => output.check_payload_len(length)?,
            None => payload_start,
        };
        output.reserve_to(initial_capacity)?;
        output.bytes.extend_from_slice(MAGIC);
        output.bytes.extend_from_slice(&schema_len.to_le_bytes());
        output.bytes.extend_from_slice(&codec_len.to_le_bytes());
        output.bytes.extend_from_slice(&0_u32.to_le_bytes());
        output.bytes.extend_from_slice(&[0; 32]);
        output
            .bytes
            .extend_from_slice(&schema_version.to_le_bytes());
        output.bytes.extend_from_slice(schema.as_bytes());
        output.bytes.extend_from_slice(codec.as_bytes());
        Ok(output)
    }

    fn check_payload_len(&self, length: usize) -> Result<usize> {
        if length as u64 > self.limits.max_payload_bytes {
            return Err(DataError::limit("save payload exceeds its byte limit"));
        }
        u32::try_from(length)
            .map_err(|_| DataError::new(DataErrorKind::NumberOutOfRange, "payload too large"))?;
        let total = self.payload_start.checked_add(length).ok_or_else(|| {
            DataError::new(
                DataErrorKind::NumberOutOfRange,
                "save output length overflow",
            )
        })?;
        if total as u64 > self.limits.max_envelope_bytes {
            return Err(DataError::limit(
                "save output exceeds the envelope byte limit",
            ));
        }
        Ok(total)
    }

    fn reserve_to(&mut self, capacity: usize) -> Result<()> {
        if capacity > self.bytes.capacity() {
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(|error| {
                    DataError::limit(format!("save output allocation failed: {error}"))
                })?;
        }
        Ok(())
    }

    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result: Result<()> = (|| {
            let length = (self.bytes.len() - self.payload_start)
                .checked_add(bytes.len())
                .ok_or_else(|| {
                    DataError::new(
                        DataErrorKind::NumberOutOfRange,
                        "save payload length overflow",
                    )
                })?;
            let total = self.check_payload_len(length)?;
            // Amortized growth stays inside the admitted complete-output limit.
            // No default Vec growth may silently request a larger capacity.
            if total > self.bytes.capacity() {
                let capacity = total
                    .max(self.bytes.capacity().saturating_mul(2))
                    .min(self.maximum_buffer_len);
                self.reserve_to(capacity)?;
            }
            self.bytes.extend_from_slice(bytes);
            Ok(())
        })();
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }

    fn finish(mut self, checksum: Option<[u8; 32]>) -> Result<Vec<u8>> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let payload = &self.bytes[self.payload_start..];
        let length = u32::try_from(payload.len())
            .map_err(|_| DataError::new(DataErrorKind::NumberOutOfRange, "payload too large"))?;
        let checksum = checksum.unwrap_or_else(|| *blake3::hash(payload).as_bytes());
        self.bytes[PAYLOAD_LEN_OFFSET..CHECKSUM_OFFSET].copy_from_slice(&length.to_le_bytes());
        self.bytes[CHECKSUM_OFFSET..CHECKSUM_OFFSET + 32].copy_from_slice(&checksum);
        Ok(self.bytes)
    }
}

impl Write for SaveEnvelopeEncoder {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.append(bytes).map_err(io::Error::other)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match &self.failure {
            Some(error) => Err(io::Error::other(error.clone())),
            None => Ok(()),
        }
    }
}
