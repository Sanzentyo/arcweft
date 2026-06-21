use arcweft_data::{
    Codec, CodecRegistry, DataErrorKind, DecodeOptions, EncodeOptions, FormatId, Result, TypeShape,
    Value,
};
use arcweft_save::{SaveDecodeOptions, SaveEnvelope, SaveMigration, SaveSchemaId, decode_save};

#[derive(Clone, Copy)]
struct UnitCodec;

impl Codec for UnitCodec {
    fn id(&self) -> FormatId {
        FormatId::new("unit")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/x-test-unit"]
    }

    fn encode_value(
        &self,
        value: &Value,
        _shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        match value {
            Value::Unit => Ok(Vec::new()),
            other => Err(arcweft_data::DataError::invalid_type(
                "unit",
                other.type_name(),
            )),
        }
    }

    fn decode_value(
        &self,
        _input: &[u8],
        _shape: &TypeShape,
        _options: &DecodeOptions,
    ) -> Result<Value> {
        Ok(Value::Unit)
    }
}

struct UnitMigration {
    schema_id: SaveSchemaId,
    current_version: u32,
}

impl SaveMigration for UnitMigration {
    fn schema_id(&self) -> &SaveSchemaId {
        &self.schema_id
    }

    fn current_version(&self) -> u32 {
        self.current_version
    }

    fn migrate(&self, _from_version: u32, value: Value) -> Result<Value> {
        Ok(value)
    }
}

fn registry() -> CodecRegistry {
    CodecRegistry::new()
        .with(UnitCodec)
        .expect("unit codec registers")
}

fn envelope(schema: &str, version: u32, payload: Vec<u8>) -> SaveEnvelope {
    SaveEnvelope::new(SaveSchemaId::new(schema), version, "unit", payload)
}

#[test]
fn save_envelope_rejects_trailing_data_by_default() {
    let mut bytes = envelope("game.save", 1, Vec::new())
        .encode_bytes()
        .expect("encode");
    bytes.push(0);

    let error =
        SaveEnvelope::decode_bytes(&bytes, &SaveDecodeOptions::default()).expect_err("trailing");
    assert_eq!(error.kind(), &DataErrorKind::TrailingData);
}

#[test]
fn save_envelope_checks_lengths_before_copying_payloads() {
    let bytes = envelope("game.save", 1, vec![1, 2, 3, 4])
        .encode_bytes()
        .expect("encode");
    let options = SaveDecodeOptions {
        max_payload_bytes: 3,
        ..SaveDecodeOptions::default()
    };

    let error = SaveEnvelope::decode_bytes(&bytes, &options).expect_err("payload cap");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
}

#[test]
fn save_decode_rejects_schema_identity_and_future_versions() {
    let bytes = envelope("game.save", 2, Vec::new())
        .encode_bytes()
        .expect("encode");
    let schema = SaveSchemaId::new("other.save");
    let error = decode_save(
        &bytes,
        &TypeShape::Unit,
        &schema,
        2,
        &registry(),
        &SaveDecodeOptions::default(),
        None,
    )
    .expect_err("schema mismatch");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let schema = SaveSchemaId::new("game.save");
    let error = decode_save(
        &bytes,
        &TypeShape::Unit,
        &schema,
        1,
        &registry(),
        &SaveDecodeOptions::default(),
        None,
    )
    .expect_err("future version");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);
}

#[test]
fn save_decode_requires_matching_migration_for_old_versions() {
    let bytes = envelope("game.save", 1, Vec::new())
        .encode_bytes()
        .expect("encode");
    let schema = SaveSchemaId::new("game.save");
    let error = decode_save(
        &bytes,
        &TypeShape::Unit,
        &schema,
        2,
        &registry(),
        &SaveDecodeOptions::default(),
        None,
    )
    .expect_err("missing migration");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let migration = UnitMigration {
        schema_id: SaveSchemaId::new("game.save"),
        current_version: 2,
    };
    let value = decode_save(
        &bytes,
        &TypeShape::Unit,
        &schema,
        2,
        &registry(),
        &SaveDecodeOptions::default(),
        Some(&migration),
    )
    .expect("migrated");
    assert_eq!(value, Value::Unit);
}
