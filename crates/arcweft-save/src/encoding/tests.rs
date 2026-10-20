use std::cell::Cell;
use std::io::Write;

use serde::{Serialize, ser::SerializeSeq};

use arcweft_data::DataErrorKind;

use super::SaveEnvelopeEncoder;
use crate::{
    HEADER_LEN, MAGIC, SaveDecodeOptions, SaveEnvelope, SaveEnvelopeLimits, SaveSchemaId,
    TYPED_JSON_CODEC_ID, decode_strict_typed_json_save,
};

fn schema() -> SaveSchemaId {
    SaveSchemaId::new("arcweft.test.bounded_output")
}

/// Independent reference for the unchanged version-one envelope transcript.
fn reference_wire(envelope: &SaveEnvelope) -> Vec<u8> {
    let mut bytes = MAGIC.to_vec();
    for count in [
        envelope.schema_id.as_str().len(),
        envelope.codec_id.len(),
        envelope.payload.len(),
    ] {
        bytes.extend_from_slice(&u32::try_from(count).unwrap().to_le_bytes());
    }
    bytes.extend_from_slice(&envelope.payload_checksum);
    bytes.extend_from_slice(&envelope.schema_version.to_le_bytes());
    bytes.extend_from_slice(envelope.schema_id.as_str().as_bytes());
    bytes.extend_from_slice(envelope.codec_id.as_bytes());
    bytes.extend_from_slice(&envelope.payload);
    bytes
}

#[test]
fn typed_and_raw_output_keep_the_exact_version_one_transcript_at_the_limit() {
    let value = serde_json::json!({
        "text": "\0\"\\\n🦀",
        "rows": [true, false, null, 314],
    });
    let payload = serde_json::to_vec(&value).unwrap();
    let envelope = SaveEnvelope::new(schema(), 1, TYPED_JSON_CODEC_ID, payload);
    let reference = reference_wire(&envelope);
    let limits = SaveEnvelopeLimits {
        max_envelope_bytes: reference.len() as u64,
        max_schema_id_bytes: envelope.schema_id.as_str().len().try_into().unwrap(),
        max_codec_id_bytes: TYPED_JSON_CODEC_ID.len().try_into().unwrap(),
        max_payload_bytes: envelope.payload.len() as u64,
    };
    let bytes = limits.encode_typed_json(&value, &schema(), 1).unwrap();
    assert_eq!(bytes, reference);
    assert_eq!(
        envelope.encode_bytes_with_limits(&limits).unwrap(),
        reference
    );
    assert_eq!(
        crate::encode_typed_json_save(&value, schema(), 1).unwrap(),
        reference
    );
    let decoded = decode_strict_typed_json_save::<serde_json::Value>(
        &bytes,
        &schema(),
        1,
        &SaveDecodeOptions {
            envelope: limits,
            ..SaveDecodeOptions::default()
        },
    )
    .unwrap();
    assert_eq!(decoded, value);

    for limited in [
        SaveEnvelopeLimits {
            max_payload_bytes: limits.max_payload_bytes - 1,
            ..limits
        },
        SaveEnvelopeLimits {
            max_envelope_bytes: limits.max_envelope_bytes - 1,
            ..limits
        },
    ] {
        assert_eq!(
            limited
                .encode_typed_json(&value, &schema(), 1)
                .unwrap_err()
                .kind(),
            &DataErrorKind::LimitExceeded,
        );
        assert_eq!(
            envelope
                .encode_bytes_with_limits(&limited)
                .unwrap_err()
                .kind(),
            &DataErrorKind::LimitExceeded,
        );
    }
}

struct SerializeProbe<'a>(&'a Cell<usize>);

impl Serialize for SerializeProbe<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        self.0.set(self.0.get() + 1);
        serializer.serialize_unit()
    }
}

#[test]
fn header_limits_refuse_before_entering_the_typed_serializer() {
    let prefix = HEADER_LEN + schema().as_str().len() + TYPED_JSON_CODEC_ID.len();
    for limits in [
        SaveEnvelopeLimits {
            max_schema_id_bytes: (schema().as_str().len() - 1).try_into().unwrap(),
            ..SaveEnvelopeLimits::default()
        },
        SaveEnvelopeLimits {
            max_codec_id_bytes: (TYPED_JSON_CODEC_ID.len() - 1).try_into().unwrap(),
            ..SaveEnvelopeLimits::default()
        },
        SaveEnvelopeLimits {
            max_envelope_bytes: (prefix - 1) as u64,
            ..SaveEnvelopeLimits::default()
        },
    ] {
        let calls = Cell::new(0);
        let error = limits
            .encode_typed_json(&SerializeProbe(&calls), &schema(), 1)
            .unwrap_err();
        assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
        assert_eq!(calls.get(), 0);
    }
}

struct CountedSequence<'a>(&'a Cell<usize>);

impl Serialize for CountedSequence<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(128))?;
        for _ in 0..128 {
            self.0.set(self.0.get() + 1);
            sequence.serialize_element("payload that cannot fit in the remaining output")?;
        }
        sequence.end()
    }
}

#[test]
fn payload_output_limit_stops_serialization_before_the_remaining_members() {
    let calls = Cell::new(0);
    let error = SaveEnvelopeLimits {
        max_payload_bytes: 16,
        ..SaveEnvelopeLimits::default()
    }
    .encode_typed_json(&CountedSequence(&calls), &schema(), 1)
    .unwrap_err();
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
    assert_eq!(calls.get(), 1);
}

struct SwallowedWriteFailure;

impl Serialize for SwallowedWriteFailure {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(1))?;
        let _ignored_error =
            sequence.serialize_element("this payload exceeds the output allowance");
        sequence.end()
    }
}

#[test]
fn a_swallowed_write_failure_cannot_publish_a_partial_envelope() {
    let error = SaveEnvelopeLimits {
        max_payload_bytes: 5,
        ..SaveEnvelopeLimits::default()
    }
    .encode_typed_json(&SwallowedWriteFailure, &schema(), 1)
    .unwrap_err();
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn output_growth_and_failure_preserve_the_same_bank() {
    let limits = SaveEnvelopeLimits {
        max_payload_bytes: 8,
        ..SaveEnvelopeLimits::default()
    };
    let mut output =
        SaveEnvelopeEncoder::new(schema().as_str(), 1, TYPED_JSON_CODEC_ID, limits, None).unwrap();
    for _ in 0..8 {
        output.write_all(b"x").unwrap();
        assert!(output.bytes.capacity() <= output.maximum_buffer_len);
    }
    let before = output.bytes.clone();
    let error = output.append(b"x").unwrap_err();
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
    assert_eq!(output.bytes, before);
    assert_eq!(output.append(b""), Err(error.clone()));
    assert_eq!(output.bytes, before);
    assert_eq!(output.finish(None), Err(error));
}

#[test]
fn raw_output_keeps_its_explicit_checksum_for_defensive_decoder_tests() {
    let mut envelope = SaveEnvelope::new(schema(), 1, TYPED_JSON_CODEC_ID, b"null".to_vec());
    envelope.payload_checksum[0] ^= 1;
    let bytes = envelope.encode_bytes().unwrap();
    assert_eq!(bytes, reference_wire(&envelope));
    let error = SaveEnvelope::decode_bytes(&bytes, &SaveDecodeOptions::default()).unwrap_err();
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);
}

#[test]
fn unrepresentable_known_payload_length_is_rejected_without_allocating_it() {
    let limits = SaveEnvelopeLimits {
        max_envelope_bytes: u64::MAX,
        max_payload_bytes: u64::MAX,
        ..SaveEnvelopeLimits::default()
    };
    let error = SaveEnvelopeEncoder::new("schema", 1, "codec", limits, Some(usize::MAX))
        .err()
        .expect("wire or address-space length fails before any payload allocation");
    assert_eq!(error.kind(), &DataErrorKind::NumberOutOfRange);
}

struct SerializerRefusal;

impl Serialize for SerializerRefusal {
    fn serialize<S: serde::Serializer>(&self, _: S) -> std::result::Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom(
            "deliberate typed serialization refusal",
        ))
    }
}

#[test]
fn serializer_failures_preserve_their_encoding_error_family() {
    let error = SaveEnvelopeLimits::default()
        .encode_typed_json(&SerializerRefusal, &schema(), 1)
        .unwrap_err();
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);
    assert!(
        error
            .to_string()
            .contains("deliberate typed serialization refusal")
    );
}
