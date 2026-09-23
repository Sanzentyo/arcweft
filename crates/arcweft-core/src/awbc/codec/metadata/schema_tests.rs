use super::*;
use crate::awbc::codec::AwbcDecodeBudget;
use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeProducerId};
use crate::value::{RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass};

fn round_trip(schema: RuntimeTypeSchema, expected: &[u8]) {
    let mut writer = Writer::default();
    schema.write_wire(&mut writer).unwrap();
    assert_eq!(writer.into_bytes(), expected);
    let budget = AwbcDecodeBudget::default();
    let mut reader = Reader::new(expected, &budget);
    assert_eq!(RuntimeTypeSchema::read_wire(&mut reader).unwrap(), schema);
    assert_eq!(reader.offset(), expected.len());
}

#[test]
fn map_schema_wire_retains_ordering_kind_in_version_one() {
    for (kind, tag) in [
        (RuntimeMapKind::Ordered, 0),
        (RuntimeMapKind::Sorted, 1),
        (RuntimeMapKind::BTree, 2),
    ] {
        round_trip(
            RuntimeTypeSchema::Map {
                kind,
                key: Box::new(RuntimeTypeSchema::String),
                value: Box::new(RuntimeTypeSchema::Bool),
            },
            &[21, tag, 16, 1],
        );
    }
    let budget = AwbcDecodeBudget::default();
    let mut reader = Reader::new(&[21, 3, 16, 1], &budget);
    assert!(matches!(
        RuntimeTypeSchema::read_wire(&mut reader),
        Err(AwbcCodecError::UnknownTag {
            kind: "runtime map kind",
            tag: 3,
            ..
        })
    ));
}

#[test]
fn array_schema_wire_keeps_length_and_checks_nesting_before_its_item() {
    for length in [0, 2, u64::MAX] {
        let mut expected = vec![36];
        expected.extend_from_slice(&length.to_le_bytes());
        expected.push(1);
        round_trip(
            RuntimeTypeSchema::Array {
                item: Box::new(RuntimeTypeSchema::Bool),
                length,
            },
            &expected,
        );
        for end in 1..expected.len() {
            let budget = AwbcDecodeBudget::default();
            let mut reader = Reader::new(&expected[..end], &budget);
            assert!(matches!(
                RuntimeTypeSchema::read_wire(&mut reader),
                Err(AwbcCodecError::Truncated { .. })
            ));
        }
    }
    let mut bytes = Vec::new();
    for _ in 0..20_000 {
        bytes.push(36);
        bytes.extend_from_slice(&1_u64.to_le_bytes());
    }
    bytes.push(1);
    let budget = AwbcDecodeBudget {
        nesting_depth: 8,
        ..AwbcDecodeBudget::default()
    };
    let mut reader = Reader::new(&bytes, &budget);
    assert!(matches!(
        RuntimeTypeSchema::read_wire(&mut reader),
        Err(AwbcCodecError::NestingDepthExceeded { limit: 8 })
    ));
    assert_eq!(reader.offset(), 72);
}

#[test]
fn builtin_schema_wire_uses_one_registry_owner_and_complete_payload_rows() {
    use RuntimeBuiltinVariantIdentity as Owner;
    for (owner, payloads, expected) in [
        (
            Owner::Option,
            vec![RuntimeTypeSchema::Bool],
            vec![19, 0, 1, 1],
        ),
        (
            Owner::Result,
            vec![RuntimeTypeSchema::Bool, RuntimeTypeSchema::String],
            vec![19, 1, 2, 1, 16],
        ),
        (
            Owner::AgentResourceBody,
            vec![RuntimeTypeSchema::Bool; 3],
            vec![19, 2, 3, 1, 1, 1],
        ),
        (Owner::AgentBinaryEncoding, vec![], vec![19, 3, 0]),
        (Owner::CaptureFormat, vec![], vec![19, 4, 0]),
        (Owner::CaptureKind, vec![], vec![19, 5, 0]),
        (Owner::PointerButton, vec![], vec![19, 6, 0]),
    ] {
        round_trip(
            RuntimeTypeSchema::builtin(owner, payloads).unwrap(),
            &expected,
        );
    }
}

#[test]
fn builtin_schema_wire_rejects_invalid_counts_before_reading_payload_items() {
    let budget = AwbcDecodeBudget::default();
    for bytes in [[19, 0, 0], [19, 0, 2], [19, 1, 1], [19, 2, 4], [19, 3, 1]] {
        let mut reader = Reader::new(&bytes, &budget);
        assert!(matches!(
            RuntimeTypeSchema::read_wire(&mut reader),
            Err(AwbcCodecError::InvalidBuiltinSchema { .. })
        ));
        assert_eq!(reader.offset(), 3);
    }
    let mut unknown = Reader::new(&[19, 7, 0], &budget);
    assert!(matches!(
        RuntimeTypeSchema::read_wire(&mut unknown),
        Err(AwbcCodecError::UnknownTag {
            kind: "builtin variant identity",
            tag: 7,
            ..
        })
    ));
    let mut truncated = Reader::new(&[19, 1, 2, 1], &budget);
    assert!(matches!(
        RuntimeTypeSchema::read_wire(&mut truncated),
        Err(AwbcCodecError::Truncated { .. })
    ));
    let mut removed = Reader::new(&[26, 1, 16], &budget);
    assert!(matches!(
        RuntimeTypeSchema::read_wire(&mut removed),
        Err(AwbcCodecError::UnknownTag { .. })
    ));
}

#[test]
fn choice_schema_wire_preserves_order_and_requires_complete_alternatives() {
    round_trip(
        RuntimeTypeSchema::Choice(
            vec![RuntimeTypeSchema::Bool, RuntimeTypeSchema::Unit].into_boxed_slice(),
        ),
        &[35, 2, 1, 0],
    );
    round_trip(
        RuntimeTypeSchema::Choice(vec![].into_boxed_slice()),
        &[35, 0],
    );
    for bytes in [&[35][..], &[35, 1][..], &[35, 2, 1][..]] {
        let budget = AwbcDecodeBudget::default();
        let mut reader = Reader::new(bytes, &budget);
        assert!(RuntimeTypeSchema::read_wire(&mut reader).is_err());
    }
}

#[test]
fn schema_wire_nesting_bounds_unary_and_choice_paths_and_unwinds_on_failure() {
    let mut unary = vec![20; 20_000];
    unary.push(0);
    let budget = AwbcDecodeBudget {
        nesting_depth: 8,
        ..AwbcDecodeBudget::default()
    };
    let mut reader = Reader::new(&unary, &budget);
    assert!(matches!(
        RuntimeTypeSchema::read_wire(&mut reader),
        Err(AwbcCodecError::NestingDepthExceeded { limit: 8 })
    ));

    let budget = AwbcDecodeBudget {
        nesting_depth: 2,
        ..AwbcDecodeBudget::default()
    };
    let mut reader = Reader::new(&[35, 2, 1, 0], &budget);
    assert!(matches!(
        RuntimeTypeSchema::read_wire(&mut reader),
        Err(AwbcCodecError::NestingDepthExceeded { limit: 2 })
    ));
    assert_eq!(
        RuntimeTypeSchema::read_wire(&mut reader).unwrap(),
        RuntimeTypeSchema::Bool
    );
    assert_eq!(
        RuntimeTypeSchema::read_wire(&mut reader).unwrap(),
        RuntimeTypeSchema::Unit
    );
    reader.finish().unwrap();
}

#[test]
fn runtime_value_atoms_round_trip_through_version_one_schema_rows() {
    for (schema, tag) in [
        (RuntimeTypeSchema::Never, 30),
        (RuntimeTypeSchema::Duration, 31),
        (RuntimeTypeSchema::Progress, 32),
        (RuntimeTypeSchema::EntityReference, 33),
        (RuntimeTypeSchema::AgentValue, 34),
    ] {
        round_trip(schema, &[tag]);
    }
}

#[test]
fn structural_schema_rows_use_their_exact_version_one_tags_and_field_ids() {
    round_trip(
        RuntimeTypeSchema::Tuple(vec![].into_boxed_slice()),
        &[25, 0],
    );
    round_trip(
        RuntimeTypeSchema::Tuple(vec![RuntimeTypeSchema::Bool].into_boxed_slice()),
        &[25, 1, 1],
    );
    round_trip(
        RuntimeTypeSchema::result(RuntimeTypeSchema::Bool, RuntimeTypeSchema::String),
        &[19, 1, 2, 1, 16],
    );
    round_trip(
        RuntimeTypeSchema::RecordValue {
            fields: vec![RuntimeSchemaValueField::new(
                RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                "a".to_owned(),
                RuntimeTypeSchema::Bool,
            )]
            .into_boxed_slice(),
        },
        &[27, 1, 1, 1, b'a', 1],
    );
    let mut expected = vec![29, 2, b'n', b'1'];
    expected.extend_from_slice(&[1; 32]);
    round_trip(
        RuntimeTypeSchema::NominalRef(RuntimeNominalSchemaIdentity::new(
            RuntimeNominalTypeId::try_new("n1").unwrap(),
            RuntimeSemanticTypeId::from_bytes([1; 32]),
        )),
        &expected,
    );
}

#[test]
fn exact_opaque_schema_wire_retains_admission_handle_kind_and_arguments() {
    let owner = RuntimeOpaqueTypeOwner::with_admission(
        RuntimeOpaqueTypeProducerId::try_new("p").unwrap(),
        RuntimeSemanticTypeId::from_bytes([2; 32]),
        RuntimeOpaqueTypeAdmission::ExactIdentity,
        RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
        RuntimeOpaquePersistence::SnapshotOnly,
    );
    let mut expected = vec![28, 1, b'p'];
    expected.extend_from_slice(&[2; 32]);
    expected.extend_from_slice(&[0, 1, 1, 1, 1, 1]);
    round_trip(
        RuntimeTypeSchema::ExactOpaque {
            owner,
            arguments: vec![RuntimeTypeSchema::Bool].into_boxed_slice(),
        },
        &expected,
    );
}

#[test]
fn structural_schema_wire_rejects_zero_and_noncanonical_field_ids() {
    let budget = AwbcDecodeBudget::default();
    let mut zero = Reader::new(&[27, 1, 0, 1, b'a', 1], &budget);
    assert!(matches!(
        RuntimeTypeSchema::read_wire(&mut zero),
        Err(AwbcCodecError::InvalidMetadata {
            kind: "record field identity",
            ..
        })
    ));
    let mut overlong = Reader::new(&[27, 1, 0x81, 0, 1, b'a', 1], &budget);
    assert!(matches!(
        RuntimeTypeSchema::read_wire(&mut overlong),
        Err(AwbcCodecError::NonCanonicalVarint { .. })
    ));
    let mut truncated = Reader::new(&[27, 1], &budget);
    assert!(matches!(
        RuntimeTypeSchema::read_wire(&mut truncated),
        Err(AwbcCodecError::Truncated { .. })
    ));
}
