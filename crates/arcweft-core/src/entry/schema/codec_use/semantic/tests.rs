use super::*;
use crate::entry::schema::{
    CanonicalBytesSink, CanonicalWriter, RuntimeEnumTagStyle, RuntimeFieldCodecUse,
    RuntimeVariantCodecUse,
};
use crate::task::semantic::TaskSemanticMeter;

fn fixture() -> RuntimeCodecUse {
    RuntimeCodecUse::Enum {
        name: "Diag".into(),
        tag: RuntimeEnumTagStyle::External,
        repr: None,
        cases: Box::new([
            RuntimeVariantCodecUse {
                wire_name: "None".into(),
                discriminant: None,
                payload: None,
            },
            RuntimeVariantCodecUse {
                wire_name: "Some".into(),
                discriminant: Some(-2),
                payload: Some(RuntimeCodecUse::Record {
                    name: "Inner".into(),
                    deny_unknown_fields: true,
                    fields: Box::new([RuntimeFieldCodecUse {
                        wire_name: "wire".into(),
                        has_default: false,
                        default_program: None,
                        skip: true,
                        bytes_format: None,
                        value: RuntimeCodecUse::Newtype {
                            inner: Box::new(RuntimeCodecUse::Tuple {
                                items: Box::new([
                                    RuntimeCodecUse::Plain,
                                    RuntimeCodecUse::NominalRef,
                                ]),
                            }),
                        },
                    }]),
                }),
            },
        ]),
    }
}
fn semantic(
    codec: &RuntimeCodecUse,
    work: u64,
    bytes: u64,
) -> (Result<blake3::Hash, RuntimeBodySemanticError>, (u64, u64)) {
    let plan = crate::plan::RuntimePlanBuilder::new().finish().unwrap();
    let context = RuntimeBodySemanticContext::new(&plan.inventory);
    let mut meter = TaskSemanticMeter::new(work, bytes);
    let mut encoder = TaskSemanticEncoder::new(b"codec-test.v1\0", &mut meter);
    let result = codec.encode_executable_policy(&context, &mut encoder);
    let completed = encoder.finish().map_err(Into::into);
    assert_eq!(result.is_ok(), completed.is_ok());
    (completed, meter.totals())
}
#[test]
fn shared_codec_cursor_preserves_distinct_wire_and_semantic_golden_grammars() {
    let codec = fixture();
    let mut sink = CanonicalBytesSink::default();
    let mut writer = CanonicalWriter {
        sink: &mut sink,
        max_encoded_bytes: 1000,
        max_string_bytes: None,
    };
    crate::entry::schema::codec_use::encode(&codec, &mut writer).unwrap();
    let mut wire = vec![7, 4];
    wire.extend(b"Diag");
    wire.extend([0, 0, 2, 4]);
    wire.extend(b"None");
    wire.extend([0, 0, 4]);
    wire.extend(b"Some");
    wire.push(1);
    wire.extend((-2_i128).to_le_bytes());
    wire.extend([1, 6, 5]);
    wire.extend(b"Inner");
    wire.extend([1, 1, 4]);
    wire.extend(b"wire");
    wire.extend([0, 0, 1, 0, 12, 3, 2, 0, 11]);
    assert_eq!(sink.finish(), wire);
    let mut expected = b"codec-test.v1\0".to_vec();
    expected.extend([8, 0, 0]);
    expected.extend(2_u32.to_le_bytes());
    for (ordinal, name, payload) in [(0_u32, "None", false), (1, "Some", true)] {
        expected.extend(ordinal.to_le_bytes());
        expected.extend(4_u32.to_le_bytes());
        expected.extend(name.as_bytes());
        expected.push(u8::from(payload));
        if payload {
            expected.extend((-2_i128).to_le_bytes());
        }
        expected.push(u8::from(payload));
    }
    expected.extend([7, 1]);
    expected.extend(1_u32.to_le_bytes());
    expected.extend(0_u32.to_le_bytes());
    expected.extend(4_u32.to_le_bytes());
    expected.extend(b"wire");
    expected.extend([0, 0, 1, 0, 3, 4]);
    expected.extend(2_u32.to_le_bytes());
    expected.extend(0_u32.to_le_bytes());
    expected.push(0);
    expected.extend(1_u32.to_le_bytes());
    expected.push(12);
    let (actual, (work, bytes)) = semantic(&codec, 1000, 1000);
    assert_eq!(actual.unwrap(), blake3::hash(&expected));
    assert_eq!(bytes, expected.len() as u64);
    assert_eq!(
        semantic(&codec, work, bytes).0.unwrap(),
        blake3::hash(&expected)
    );
    assert!(matches!(
        semantic(&codec, work - 1, bytes).0,
        Err(RuntimeBodySemanticError::Encoding(
            crate::task::semantic::TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert!(matches!(
        semantic(&codec, work, bytes - 1).0,
        Err(RuntimeBodySemanticError::Encoding(
            crate::task::semantic::TaskSemanticEncodingError::TranscriptBytes
        ))
    ));
    let mut widths = Vec::new();
    codec
        .try_visit_semantic_child_counts(&mut |count| {
            widths.push(count);
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(widths, [2, 0, 1, 1, 1, 1, 2, 0, 0]);
    let mut depths = Vec::new();
    codec
        .walk(|_, depth| {
            depths.push(depth);
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(depths, [0, 1, 2, 3, 4, 4]);
    assert_eq!(codec.children().count(), 1);
}
#[test]
fn nominal_codec_body_precedes_arguments_and_nested_width_precedes_default_resolution() {
    let unbound = arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([99; 32]);
    let codec = RuntimeNominalCodecUses {
        body: RuntimeCodecUse::Record {
            name: "Body".into(),
            deny_unknown_fields: false,
            fields: Box::new([RuntimeFieldCodecUse {
                wire_name: "value".into(),
                has_default: true,
                default_program: Some(unbound),
                skip: false,
                bytes_format: None,
                value: RuntimeCodecUse::Tuple {
                    items: Box::new([
                        RuntimeCodecUse::Plain,
                        RuntimeCodecUse::Plain,
                        RuntimeCodecUse::Plain,
                    ]),
                },
            }]),
        },
        arguments: Box::new([
            RuntimeCodecUse::Plain,
            RuntimeCodecUse::Plain,
            RuntimeCodecUse::Plain,
            RuntimeCodecUse::Plain,
        ]),
    };
    let mut visited = Vec::new();
    assert_eq!(
        codec.try_visit_semantic_child_counts(&mut |count| {
            visited.push(count);
            if count > 2 { Err(count) } else { Ok(()) }
        }),
        Err(3)
    );
    assert_eq!(visited, [1, 1, 3]);
    let mut counts = Vec::new();
    codec
        .try_visit_semantic_child_counts(&mut |count| {
            counts.push(count);
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(counts, [1, 1, 3, 0, 0, 0, 4, 0, 0, 0, 0]);
}
#[test]
fn deep_codec_walk_count_and_both_encoders_are_iterative() {
    let mut codec = RuntimeCodecUse::Plain;
    for _ in 0..20_000 {
        codec = RuntimeCodecUse::Unary {
            item: Box::new(codec),
        };
    }
    let mut depth = 0;
    codec
        .walk(|_, actual| {
            depth = actual;
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(depth, 20_000);
    let mut counts = 0;
    codec
        .try_visit_semantic_child_counts(&mut |count| {
            assert!(count <= 1);
            counts += 1;
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(counts, 20_001);
    let mut sink = CanonicalBytesSink::default();
    let mut writer = CanonicalWriter {
        sink: &mut sink,
        max_encoded_bytes: 20_001,
        max_string_bytes: None,
    };
    crate::entry::schema::codec_use::encode(&codec, &mut writer).unwrap();
    let bytes = sink.finish();
    assert_eq!(bytes.len(), 20_001);
    assert!(bytes[..20_000].iter().all(|byte| *byte == 2));
    assert_eq!(bytes[20_000], 0);
    assert!(semantic(&codec, 100_000, 100_000).0.is_ok());
}
