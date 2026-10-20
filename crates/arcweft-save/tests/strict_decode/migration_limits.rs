use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use arcweft_data::{
    Codec, CodecRegistry, DataErrorKind, DecodeOptions, EncodeOptions, FormatId, Result,
    ShapeAccess, ShapeRef, TypeShape, Value,
};
use arcweft_save::{SaveDecodeOptions, SaveEnvelope, SaveMigration, SaveSchemaId, decode_save};

struct UncheckedCodec;

impl Codec for UncheckedCodec {
    fn id(&self) -> FormatId {
        FormatId::new("unchecked")
    }
    fn media_types(&self) -> &'static [&'static str] {
        &["application/x-unchecked-test"]
    }
    fn encode_to(
        &self,
        _: &Value,
        _: ShapeRef<'_>,
        _: &dyn ShapeAccess,
        _: &EncodeOptions,
        output: &mut arcweft_data::EncodeBuffer,
    ) -> Result<()> {
        output.append(&[])
    }
    fn decode_value(
        &self,
        input: &[u8],
        _: ShapeRef<'_>,
        _: &dyn ShapeAccess,
        _: &DecodeOptions,
    ) -> Result<Value> {
        Ok(if input.is_empty() {
            Value::Unit
        } else {
            Value::Seq(vec![
                Value::Seq(vec![Value::Unit]),
                Value::Seq(vec![Value::Unit]),
            ])
        })
    }
}

struct GrowingMigration {
    schema: SaveSchemaId,
    visits: Arc<AtomicUsize>,
}

impl SaveMigration for GrowingMigration {
    fn schema_id(&self) -> &SaveSchemaId {
        &self.schema
    }
    fn current_version(&self) -> u32 {
        1
    }
    fn migrate(&self, _: u32, _: Value) -> Result<Value> {
        self.visits.fetch_add(1, Ordering::SeqCst);
        Ok(Value::Seq(vec![Value::Unit, Value::Unit]))
    }
}

#[test]
fn codec_results_are_admitted_before_migration_and_shape_conversion() {
    let schema = SaveSchemaId::new("test:external-codec-admission");
    let registry = CodecRegistry::new()
        .with(UncheckedCodec)
        .expect("register test codec");
    let visits = Arc::new(AtomicUsize::new(0));
    let migration = GrowingMigration {
        schema: schema.clone(),
        visits: Arc::clone(&visits),
    };
    let mut options = SaveDecodeOptions::default();
    options.codec.limits.max_nodes = 4;
    // The equal-version case also has an incompatible final shape. A limit
    // refusal must happen before shape conversion, rather than allocating a RawValue.
    for schema_version in [0, 1] {
        let bytes = SaveEnvelope::new(schema.clone(), schema_version, "unchecked", vec![1])
            .encode_bytes()
            .expect("test envelope");
        let error = decode_save(
            &bytes,
            &TypeShape::Unit,
            &schema,
            1,
            &registry,
            &options,
            Some(&migration),
        )
        .expect_err("five logical nodes exceed four");
        assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
        assert_eq!(error.path().to_string(), "$[1][0]");
    }
    assert_eq!(
        visits.load(Ordering::SeqCst),
        0,
        "migration must not see unadmitted codec values"
    );
}

#[test]
fn migration_results_are_admitted_before_conversion_or_publication() {
    let schema = SaveSchemaId::new("test:external-migration-admission");
    let registry = CodecRegistry::new()
        .with(UncheckedCodec)
        .expect("register test codec");
    let visits = Arc::new(AtomicUsize::new(0));
    let migration = GrowingMigration {
        schema: schema.clone(),
        visits: Arc::clone(&visits),
    };
    let bytes = SaveEnvelope::new(schema.clone(), 0, "unchecked", Vec::new())
        .encode_bytes()
        .expect("test envelope");
    let mut options = SaveDecodeOptions::default();
    options.codec.limits.max_nodes = 2;
    let shape = TypeShape::seq(TypeShape::Unit);
    let error = decode_save(
        &bytes,
        &shape,
        &schema,
        1,
        &registry,
        &options,
        Some(&migration),
    )
    .expect_err("migration output contains three nodes");
    assert_eq!(error.kind(), &DataErrorKind::LimitExceeded);
    assert_eq!(error.path().to_string(), "$[1]");
    assert_eq!(visits.load(Ordering::SeqCst), 1);
    options.codec.limits.max_nodes = 3;
    assert_eq!(
        decode_save(
            &bytes,
            &shape,
            &schema,
            1,
            &registry,
            &options,
            Some(&migration)
        )
        .expect("exact migrated value limit"),
        Value::Seq(vec![Value::Unit, Value::Unit])
    );
    assert_eq!(visits.load(Ordering::SeqCst), 2);
}
