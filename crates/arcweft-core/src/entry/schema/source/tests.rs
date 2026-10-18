use super::*;
use crate::entry::schema::*;
use crate::entry::{RootExecutionLimits, RuntimeNominalTypeId};
use crate::pattern::{
    RuntimeBuiltinVariantIdentity, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner,
    RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
};
use crate::value::{
    RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeRecordFieldId,
};

fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(100_000_000, 100_000_000)
}
fn limits() -> RuntimeSchemaSourceLimits {
    RuntimeSchemaSourceLimits {
        nodes: 100_000,
        depth: 10_000,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 27,
    }
}
fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn encode(schema: &RuntimeTypeSchema) -> Vec<u8> {
    let prepared = schema
        .prepare_source_encoding(limits(), &mut meter())
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    bytes
}
fn record() -> RuntimeTypeSchema {
    RuntimeTypeSchema::Record {
        name: "操作".into(),
        deny_unknown_fields: true,
        fields: vec![
            RuntimeSchemaField {
                rust_name: "payload".into(),
                wire_name: "値".into(),
                schema: RuntimeTypeSchema::Seq(Box::new(RuntimeTypeSchema::Bool)),
                has_default: false,
                skip: false,
                bytes_format: None,
            },
            RuntimeSchemaField {
                rust_name: "bytes".into(),
                wire_name: "raw".into(),
                schema: RuntimeTypeSchema::Bytes {
                    format: RuntimeBytesFormat::Base64,
                },
                has_default: true,
                skip: true,
                bytes_format: Some(RuntimeBytesFormat::Hex),
            },
        ],
    }
}
fn fixture() -> RuntimeTypeSchema {
    use RuntimeTypeSchema as S;
    let mut schemas = vec![
        S::Unit,
        S::Bool,
        S::I8,
        S::I16,
        S::I32,
        S::I64,
        S::I128,
        S::ISize,
        S::U8,
        S::U16,
        S::U32,
        S::U64,
        S::U128,
        S::USize,
        S::F32,
        S::F64,
        S::String,
        S::Char,
        S::Never,
        S::Duration,
        S::Progress,
        S::EntityReference,
        S::AgentValue,
        S::Color,
    ];
    schemas.extend(
        RuntimeBytesFormat::ALL
            .iter()
            .map(|format| S::Bytes { format: *format }),
    );
    for tag in 0..=u8::MAX {
        if let Some(owner) = RuntimeBuiltinVariantIdentity::from_wire_tag(tag) {
            schemas.push(S::Builtin(
                RuntimeBuiltinSchema::try_new(
                    owner,
                    (0..owner.payload_count())
                        .map(|_| S::Bool)
                        .collect::<Box<[_]>>(),
                )
                .unwrap(),
            ));
        }
    }
    for kind in [
        RuntimeMapKind::Ordered,
        RuntimeMapKind::Sorted,
        RuntimeMapKind::BTree,
    ] {
        schemas.push(S::Map {
            kind,
            key: Box::new(S::String),
            value: Box::new(S::I64),
        });
    }
    schemas.extend([
        S::Seq(Box::new(S::Unit)),
        S::Array {
            length: u64::MAX,
            item: Box::new(S::Bool),
        },
        record(),
        S::Named("source::未解決".into()),
        S::Tuple(Box::new([])),
        S::Choice(Box::new([])),
        S::Choice(Box::new([S::Bool, S::I64])),
        S::RecordValue {
            fields: Box::new([RuntimeSchemaValueField::new(
                RuntimeRecordFieldId::try_from_zero_based_ordinal(7).unwrap(),
                "八".into(),
                S::Bool,
            )]),
        },
        S::NominalRef(RuntimeNominalSchemaIdentity::new(
            RuntimeNominalTypeId::try_new("nominal.Source").unwrap(),
            semantic(1),
        )),
        S::ExecutableRef(semantic(2)),
    ]);
    for tag in [
        RuntimeEnumTagStyle::External,
        RuntimeEnumTagStyle::Internal { tag: "kind".into() },
        RuntimeEnumTagStyle::Adjacent {
            tag: "kind".into(),
            content: "body".into(),
        },
    ] {
        for repr in std::iter::once(None).chain(RuntimeEnumRepr::ALL.iter().map(|repr| Some(*repr)))
        {
            schemas.push(S::Enum {
                name: "Variant".into(),
                tag: tag.clone(),
                repr,
                variants: vec![
                    RuntimeSchemaVariant {
                        rust_name: "Unit".into(),
                        wire_name: "empty".into(),
                        payload: None,
                        discriminant: Some(i128::MIN),
                    },
                    RuntimeSchemaVariant {
                        rust_name: "Some".into(),
                        wire_name: "item".into(),
                        payload: Some(S::String),
                        discriminant: Some(i128::MAX),
                    },
                    RuntimeSchemaVariant {
                        rust_name: "Last".into(),
                        wire_name: "終".into(),
                        payload: Some(S::Bool),
                        discriminant: None,
                    },
                ],
            });
        }
    }
    for admission in [
        RuntimeOpaqueTypeAdmission::ExactIdentity,
        RuntimeOpaqueTypeAdmission::ProducerWide,
    ] {
        for class in [
            RuntimeOpaqueValueClass::Plain,
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::StageActor),
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Voice),
        ] {
            for persistence in [
                RuntimeOpaquePersistence::ConstantAndSnapshot,
                RuntimeOpaquePersistence::SnapshotOnly,
            ] {
                schemas.push(S::ExactOpaque {
                    owner: RuntimeOpaqueTypeOwner::with_admission(
                        RuntimeOpaqueTypeProducerId::try_new("source.opaque").unwrap(),
                        semantic(3),
                        admission,
                        class,
                        persistence,
                    ),
                    arguments: Box::new([S::Bool, S::String]),
                });
            }
        }
    }
    for first in 0..=u8::MAX {
        for second in 0..=u8::MAX {
            if let Some(dense) =
                crate::math::RuntimeDenseNumericType::from_semantic_parts([first, second])
            {
                schemas.push(S::DenseNumeric(dense));
            }
        }
    }
    S::Tuple(schemas.into_boxed_slice())
}

#[test]
fn schema_sources_restore_every_schema_and_policy_without_issuing_a_program_or_layout_authority() {
    let original = fixture();
    let bytes = encode(&original);
    let mut bank = meter();
    let prepared = PreparedRuntimeSchemaSource::preflight(&bytes, limits(), &mut bank).unwrap();
    let encoded = original
        .prepare_source_encoding(limits(), &mut meter())
        .unwrap();
    assert_eq!(prepared.elements(), encoded.elements());
    assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
    let restored = prepared.materialize(&mut bank).unwrap();
    assert_eq!(restored, original);
    assert_eq!(encode(&restored), bytes);
    // A retained nominal reference still requires its real nominal graph.
    assert!(restored.try_layout_hash().is_err());
    restored.drop_iteratively();
    original.drop_iteratively();
}

#[test]
fn schema_sources_enter_the_actual_stateful_entry_owner_and_common_executable_seal() {
    let original = record();
    let bytes = encode(&original);
    let mut bank = meter();
    let restored = PreparedRuntimeSchemaSource::preflight(&bytes, limits(), &mut bank)
        .unwrap()
        .materialize(&mut bank)
        .unwrap();
    assert_eq!(
        original.try_layout_hash().unwrap(),
        restored.try_layout_hash().unwrap()
    );
    let plan = crate::plan::body_semantic::stateful_root_builder_fixture(
        original,
        RootExecutionLimits::engine_default(),
    )
    .finish()
    .unwrap();
    let rebuilt = crate::plan::body_semantic::stateful_root_builder_fixture(
        restored,
        RootExecutionLimits::engine_default(),
    )
    .finish()
    .unwrap();
    assert_eq!(plan.entries(), rebuilt.entries());
    assert_eq!(
        plan.executable_semantic_digest(),
        rebuilt.executable_semantic_digest()
    );
}

#[test]
fn schema_sources_admit_exact_aggregate_resources_and_keep_failure_sticky() {
    let schema = fixture();
    let bytes = encode(&schema);
    let prepared = PreparedRuntimeSchemaSource::preflight(&bytes, limits(), &mut meter()).unwrap();
    let exact = RuntimeSchemaSourceLimits {
        nodes: prepared.nodes() as u32,
        depth: prepared.footprint.depth as u32,
        elements: prepared.elements() as u32,
        encoded_bytes: bytes.len(),
        decoded_bytes: prepared.storage_bytes(),
    };
    schema.prepare_source_encoding(exact, &mut meter()).unwrap();
    PreparedRuntimeSchemaSource::preflight(&bytes, exact, &mut meter())
        .unwrap()
        .materialize(&mut meter())
        .unwrap()
        .drop_iteratively();
    for short in [
        RuntimeSchemaSourceLimits {
            nodes: exact.nodes - 1,
            ..exact
        },
        RuntimeSchemaSourceLimits {
            depth: exact.depth - 1,
            ..exact
        },
        RuntimeSchemaSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeSchemaSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeSchemaSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        let mut bank = meter();
        assert!(PreparedRuntimeSchemaSource::preflight(&bytes, short, &mut bank).is_err());
        assert!(PreparedRuntimeSchemaSource::preflight(&bytes, exact, &mut bank).is_err());
        assert!(schema.prepare_source_encoding(short, &mut meter()).is_err());
    }
    let mut bank = TaskSemanticMeter::new(0, u64::MAX);
    assert!(matches!(
        PreparedRuntimeSchemaSource::preflight(&bytes, exact, &mut bank),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
    assert!(matches!(
        PreparedRuntimeSchemaSource::preflight(&bytes, exact, &mut bank),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
    let mut bank = meter();
    let prepared = PreparedRuntimeSchemaSource::preflight(&bytes, exact, &mut bank).unwrap();
    bank.reject_owner();
    assert!(prepared.materialize(&mut bank).is_err());
    schema.drop_iteratively();
}

#[test]
fn schema_sources_reject_all_truncations_tags_utf8_coordinates_and_output_without_partial_append() {
    let schema = fixture();
    let bytes = encode(&schema);
    for end in 0..bytes.len() {
        assert!(
            PreparedRuntimeSchemaSource::preflight(&bytes[..end], limits(), &mut meter()).is_err(),
            "prefix {end}"
        );
    }
    for (offset, value) in [(0, 0), (DOMAIN.len(), 2), (DOMAIN.len() + 1, 255)] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(PreparedRuntimeSchemaSource::preflight(&bad, limits(), &mut meter()).is_err());
    }
    let mut bad = bytes.clone();
    bad.push(0);
    assert!(matches!(
        PreparedRuntimeSchemaSource::preflight(&bad, limits(), &mut meter()),
        Err(Error::Trailing { .. })
    ));
    let mut named = encode(&RuntimeTypeSchema::Named("x".into()));
    *named.last_mut().unwrap() = 255;
    assert!(matches!(
        PreparedRuntimeSchemaSource::preflight(&named, limits(), &mut meter()),
        Err(Error::Utf8(_))
    ));
    let field = RuntimeTypeSchema::RecordValue {
        fields: Box::new([RuntimeSchemaValueField::new(
            RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
            "x".into(),
            RuntimeTypeSchema::Bool,
        )]),
    };
    let mut field = encode(&field);
    let start = DOMAIN.len() + 1 + 1 + 4;
    field[start..start + 4].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        PreparedRuntimeSchemaSource::preflight(&field, limits(), &mut meter()),
        Err(Error::Field { actual: 0 })
    ));
    let prepared = schema
        .prepare_source_encoding(limits(), &mut meter())
        .unwrap();
    let mut output = vec![42];
    assert!(matches!(
        prepared.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, [42]);
    schema.drop_iteratively();
}

#[test]
fn schema_sources_bound_and_restore_deep_schemas_without_recursive_traversal_or_construction() {
    let mut schema = RuntimeTypeSchema::Bool;
    for _ in 0..4096 {
        schema = RuntimeTypeSchema::Seq(Box::new(schema));
    }
    let bytes = encode(&schema);
    let mut bank = meter();
    let short = RuntimeSchemaSourceLimits {
        depth: 4096,
        ..limits()
    };
    assert!(matches!(
        PreparedRuntimeSchemaSource::preflight(&bytes, short, &mut bank),
        Err(Error::Limit { kind: "depth", .. })
    ));
    let mut bank = meter();
    let restored = PreparedRuntimeSchemaSource::preflight(&bytes, limits(), &mut bank)
        .unwrap()
        .materialize(&mut bank)
        .unwrap();
    assert_eq!(encode(&restored), bytes);
    restored.drop_iteratively();
    schema.drop_iteratively();
}
