use std::sync::atomic::{AtomicUsize, Ordering};

use arcweft_data::{
    Codec, DataErrorKind, DecodeOptions, EncodeBuffer, EncodeOptions, FormatId, Result,
    ShapeAccess, ShapeRef, TypeShape, Value,
};

use crate::{HEADER_LEN, SaveDecodeOptions, SaveEnvelope, SaveEnvelopeLimits, SaveSchemaId};

struct ChunkCodec<'a> {
    calls: &'a AtomicUsize,
    catch_error: bool,
}

impl Codec for ChunkCodec<'_> {
    fn id(&self) -> FormatId {
        FormatId::new("chunks")
    }
    fn media_types(&self) -> &'static [&'static str] {
        &["application/x-chunks"]
    }
    fn encode_to(
        &self,
        _: &Value,
        _: ShapeRef<'_>,
        _: &dyn ShapeAccess,
        _: &EncodeOptions,
        output: &mut EncodeBuffer,
    ) -> Result<()> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        for chunk in [b"ab", b"cd", b"ef"] {
            let result = output.append(chunk);
            if !self.catch_error {
                result?;
            }
        }
        Ok(())
    }
    fn decode_value(
        &self,
        _: &[u8],
        _: ShapeRef<'_>,
        _: &dyn ShapeAccess,
        _: &DecodeOptions,
    ) -> Result<Value> {
        Ok(Value::Unit)
    }
}

#[test]
fn generic_codec_output_obeys_payload_and_complete_envelope_limits() {
    let calls = AtomicUsize::new(0);
    let schema = SaveSchemaId::new("bounded");
    let header = HEADER_LEN + schema.as_str().len() + "chunks".len();
    for catch_error in [false, true] {
        let codec = ChunkCodec {
            calls: &calls,
            catch_error,
        };
        for limits in [
            SaveEnvelopeLimits {
                max_payload_bytes: 5,
                ..SaveEnvelopeLimits::default()
            },
            SaveEnvelopeLimits {
                max_envelope_bytes: (header + 5) as u64,
                ..SaveEnvelopeLimits::default()
            },
        ] {
            let error = limits
                .encode_value(&Value::Unit, &TypeShape::Unit, &schema, 1, &codec)
                .unwrap_err();
            assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
        }
        let limits = SaveEnvelopeLimits {
            max_payload_bytes: 6,
            max_envelope_bytes: (header + 6) as u64,
            ..SaveEnvelopeLimits::default()
        };
        let bytes = limits
            .encode_value(&Value::Unit, &TypeShape::Unit, &schema, 1, &codec)
            .unwrap();
        let envelope = SaveEnvelope::decode_bytes(&bytes, &SaveDecodeOptions::default()).unwrap();
        assert_eq!(envelope.payload, b"abcdef");
        assert_eq!(bytes.len(), header + 6);
    }
    assert_eq!(calls.load(Ordering::Relaxed), 6);
}

#[test]
fn generic_codec_header_is_admitted_before_the_codec_is_called() {
    let calls = AtomicUsize::new(0);
    let codec = ChunkCodec {
        calls: &calls,
        catch_error: false,
    };
    let schema = SaveSchemaId::new("bounded");
    for limits in [
        SaveEnvelopeLimits {
            max_schema_id_bytes: 6,
            ..SaveEnvelopeLimits::default()
        },
        SaveEnvelopeLimits {
            max_codec_id_bytes: 5,
            ..SaveEnvelopeLimits::default()
        },
        SaveEnvelopeLimits {
            max_envelope_bytes: HEADER_LEN as u64,
            ..SaveEnvelopeLimits::default()
        },
    ] {
        let error = limits
            .encode_value(&Value::Unit, &TypeShape::Unit, &schema, 1, &codec)
            .unwrap_err();
        assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }
}
