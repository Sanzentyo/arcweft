use arcweft_data::{
    Codec, CodecRegistry, DataErrorKind, DecodeOptions, EncodeOptions, FormatId, Result, TypeShape,
    Value,
};
use arcweft_save::{
    SaveDecodeOptions, SaveEnvelope, SaveMigration, SaveMigrationChain, SaveMigrationStep,
    SaveSchemaId, decode_save,
};

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

#[derive(Clone, Copy)]
struct TextCodec;

impl Codec for TextCodec {
    fn id(&self) -> FormatId {
        FormatId::new("text")
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["text/plain"]
    }

    fn encode_value(
        &self,
        value: &Value,
        _shape: &TypeShape,
        _options: &EncodeOptions,
    ) -> Result<Vec<u8>> {
        match value {
            Value::String(value) => Ok(value.as_bytes().to_vec()),
            other => Err(arcweft_data::DataError::invalid_type(
                "string",
                other.type_name(),
            )),
        }
    }

    fn decode_value(
        &self,
        input: &[u8],
        _shape: &TypeShape,
        _options: &DecodeOptions,
    ) -> Result<Value> {
        String::from_utf8(input.to_vec())
            .map(Value::String)
            .map_err(|error| {
                arcweft_data::DataError::new(DataErrorKind::InvalidEncoding, error.to_string())
            })
    }
}

struct TextStep {
    schema_id: SaveSchemaId,
    source_version: u32,
    target_version: u32,
    suffix: &'static str,
}

impl SaveMigrationStep for TextStep {
    fn schema_id(&self) -> &SaveSchemaId {
        &self.schema_id
    }

    fn source_version(&self) -> u32 {
        self.source_version
    }

    fn target_version(&self) -> u32 {
        self.target_version
    }

    fn migrate(&self, value: Value) -> Result<Value> {
        match value {
            Value::String(value) => Ok(Value::String(format!("{value}{}", self.suffix))),
            other => Err(arcweft_data::DataError::invalid_type(
                "string",
                other.type_name(),
            )),
        }
    }
}

fn registry() -> CodecRegistry {
    CodecRegistry::new()
        .with(UnitCodec)
        .expect("unit codec registers")
}

fn text_registry() -> CodecRegistry {
    CodecRegistry::new()
        .with(TextCodec)
        .expect("text codec registers")
}

fn envelope(schema: &str, version: u32, payload: Vec<u8>) -> SaveEnvelope {
    SaveEnvelope::new(SaveSchemaId::new(schema), version, "unit", payload)
}

fn text_envelope(schema: &str, version: u32, payload: &str) -> SaveEnvelope {
    SaveEnvelope::new(
        SaveSchemaId::new(schema),
        version,
        "text",
        payload.as_bytes().to_vec(),
    )
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

#[test]
fn save_decode_runs_multi_step_migration_chain() {
    let schema = SaveSchemaId::new("game.save");
    let step_1_to_2 = TextStep {
        schema_id: schema.clone(),
        source_version: 1,
        target_version: 2,
        suffix: "-v2",
    };
    let step_2_to_3 = TextStep {
        schema_id: schema.clone(),
        source_version: 2,
        target_version: 3,
        suffix: "-v3",
    };
    let chain = SaveMigrationChain::new(
        schema.clone(),
        3,
        [
            &step_1_to_2 as &dyn SaveMigrationStep,
            &step_2_to_3 as &dyn SaveMigrationStep,
        ],
    )
    .expect("chain");
    let bytes = text_envelope("game.save", 1, "legacy")
        .encode_bytes()
        .expect("encode");

    let value = decode_save(
        &bytes,
        &TypeShape::String,
        &schema,
        3,
        &text_registry(),
        &SaveDecodeOptions::default(),
        Some(&chain),
    )
    .expect("migrated");

    assert_eq!(value, Value::String("legacy-v2-v3".to_owned()));
}

#[test]
fn save_migration_chain_rejects_schema_mismatches_and_duplicate_steps() {
    let schema = SaveSchemaId::new("game.save");
    let wrong_schema_step = TextStep {
        schema_id: SaveSchemaId::new("other.save"),
        source_version: 1,
        target_version: 2,
        suffix: "-v2",
    };
    let error = SaveMigrationChain::new(
        schema.clone(),
        2,
        [&wrong_schema_step as &dyn SaveMigrationStep],
    )
    .err()
    .expect("schema mismatch");
    assert_eq!(error.kind(), &DataErrorKind::InvalidEncoding);

    let first = TextStep {
        schema_id: schema.clone(),
        source_version: 1,
        target_version: 2,
        suffix: "-a",
    };
    let duplicate = TextStep {
        schema_id: schema.clone(),
        source_version: 1,
        target_version: 3,
        suffix: "-b",
    };
    let error = SaveMigrationChain::new(
        schema,
        3,
        [
            &first as &dyn SaveMigrationStep,
            &duplicate as &dyn SaveMigrationStep,
        ],
    )
    .err()
    .expect("duplicate from version");
    assert_eq!(error.kind(), &DataErrorKind::DuplicateField);
}
