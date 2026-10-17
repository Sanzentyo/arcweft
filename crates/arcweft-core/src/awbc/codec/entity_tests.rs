use super::wire::{Reader, Wire, Writer};
use super::{AwbcCodecError, AwbcDecodeBudget};
use crate::plan::{EntryRuntimeId, FlowRuntimeId, RuntimeLineId};
use crate::runtime_id::{RuntimeIdFamily, RuntimeIdPath, RuntimeIdSegment};
use crate::value::{RuntimeEntityReference, RuntimeImportedProjectEntityReference};
use arcweft_id::{ProjectEntityReferenceFamily, PublicId};

fn path(family: RuntimeIdFamily, parts: &[&str]) -> RuntimeIdPath {
    RuntimeIdPath::from_segments(
        family,
        parts
            .iter()
            .map(|part| RuntimeIdSegment::new(family, part).unwrap()),
    )
    .unwrap()
}

fn bytes<T: Wire>(value: &T) -> Vec<u8> {
    let mut writer = Writer::default();
    value.write_wire(&mut writer).unwrap();
    writer.into_bytes()
}

#[test]
fn awbc_entity_wire_retains_original_segment_boundaries_and_targets() {
    let flow = FlowRuntimeId::from_source_path(
        path(RuntimeIdFamily::Flow, &["one.two"]),
        "flow.opening".into(),
    );
    let imported = RuntimeImportedProjectEntityReference::try_new(
        ProjectEntityReferenceFamily::Flow,
        PublicId::try_new("flow.opening").unwrap(),
        [1; 32],
        [2; 32],
        [3; 32],
        Some(flow.clone()),
    )
    .unwrap();
    for original in [
        RuntimeEntityReference::StructuralFlow(flow),
        RuntimeEntityReference::DialogueLine(RuntimeLineId::from_source_path(path(
            RuntimeIdFamily::Line,
            &["one.two"],
        ))),
        RuntimeEntityReference::ImportedProject(imported),
        RuntimeEntityReference::StructuralFlow(
            FlowRuntimeId::from_checked_declaration_digest([17; 32], "flow.opening").unwrap(),
        ),
        RuntimeEntityReference::StructuralFlow(
            FlowRuntimeId::from_source_entity_body("flow.flow.main").unwrap(),
        ),
        RuntimeEntityReference::DialogueLine(
            RuntimeLineId::from_source_entity_body("say.say.line").unwrap(),
        ),
    ] {
        let encoded = bytes(&original);
        let mut reader = Reader::new(&encoded, &AwbcDecodeBudget::default());
        let decoded = RuntimeEntityReference::read_wire(&mut reader).unwrap();
        reader.finish().unwrap();
        assert_eq!(decoded, original);
        assert_eq!(bytes(&decoded), encoded);
    }
}

#[test]
fn awbc_entry_wire_retains_original_segment_boundaries() {
    let original: EntryRuntimeId =
        serde_json::from_value(serde_json::json!({"path":{"segments":["one.two"]}})).unwrap();
    let encoded = bytes(&original);
    let mut reader = Reader::new(&encoded, &AwbcDecodeBudget::default());
    let decoded = EntryRuntimeId::read_wire(&mut reader).unwrap();
    reader.finish().unwrap();
    assert_eq!(decoded, original);
    assert_eq!(bytes(&decoded), encoded);
}

#[test]
fn awbc_imported_line_wire_carries_its_original_explicit_target() {
    let reference = RuntimeEntityReference::ImportedProject(
        RuntimeImportedProjectEntityReference::try_new(
            ProjectEntityReferenceFamily::DialogueLine,
            PublicId::try_new("say.say.line").unwrap(),
            [1; 32],
            [2; 32],
            [3; 32],
            None,
        )
        .unwrap(),
    );
    let encoded = bytes(&reference);
    // The owner retains a closed Public/Flow/Line target; it must be carried
    // explicitly rather than rebuilt from the unrelated public-label field.
    let mut reader = Reader::new(&encoded, &AwbcDecodeBudget::default());
    assert_eq!(reader.read_u8().unwrap(), 3);
    reader.read_u8().unwrap();
    reader.read_str().unwrap();
    reader.read_exact(96).unwrap();
    assert_eq!(reader.read_u8().unwrap(), 2);
    let mut reader = Reader::new(&encoded, &AwbcDecodeBudget::default());
    assert_eq!(
        RuntimeEntityReference::read_wire(&mut reader).unwrap(),
        reference
    );
    reader.finish().unwrap();
}

fn flow_reference() -> RuntimeEntityReference {
    RuntimeEntityReference::StructuralFlow(FlowRuntimeId::from_source_path(
        path(RuntimeIdFamily::Flow, &["root", "child"]),
        "flow.label".into(),
    ))
}

#[test]
fn awbc_identity_wire_has_exact_golden_and_cumulative_original_budgets() {
    let encoded = bytes(&flow_reference());
    assert_eq!(encoded, b"\x04\x02\x04root\x05child\x0aflow.label");
    let exact = AwbcDecodeBudget {
        collection_items: 2,
        string_bytes: 19,
        nesting_depth: 1,
        ..AwbcDecodeBudget::default()
    };
    let mut reader = Reader::new(&encoded, &exact);
    assert_eq!(
        RuntimeEntityReference::read_wire(&mut reader).unwrap(),
        flow_reference()
    );
    reader.finish().unwrap();
    for (budget, expected) in [
        (
            AwbcDecodeBudget {
                collection_items: 1,
                ..exact
            },
            "collection_items",
        ),
        (
            AwbcDecodeBudget {
                string_bytes: 18,
                ..exact
            },
            "string_bytes",
        ),
    ] {
        let mut reader = Reader::new(&encoded, &budget);
        assert!(
            matches!(RuntimeEntityReference::read_wire(&mut reader),Err(AwbcCodecError::BudgetExceeded { budget, .. }) if budget == expected)
        );
    }
    let mut reader = Reader::new(
        &encoded,
        &AwbcDecodeBudget {
            nesting_depth: 0,
            ..exact
        },
    );
    assert_eq!(
        RuntimeEntityReference::read_wire(&mut reader),
        Err(AwbcCodecError::NestingDepthExceeded { limit: 0 })
    );
    let mut two = encoded.clone();
    two.extend_from_slice(&encoded);
    for (budget, expected) in [
        (
            AwbcDecodeBudget {
                collection_items: 3,
                string_bytes: 100,
                ..exact
            },
            "collection_items",
        ),
        (
            AwbcDecodeBudget {
                collection_items: 10,
                string_bytes: 30,
                ..exact
            },
            "string_bytes",
        ),
    ] {
        let mut reader = Reader::new(&two, &budget);
        RuntimeEntityReference::read_wire(&mut reader).unwrap();
        assert!(
            matches!(RuntimeEntityReference::read_wire(&mut reader),Err(AwbcCodecError::BudgetExceeded { budget, .. }) if budget == expected)
        );
    }
}

#[test]
fn awbc_identity_wire_preflights_malformed_paths_before_retaining_segments() {
    for encoded in [&[4, 0, 0][..], &[4, 1, 0, 0][..]] {
        let mut reader = Reader::new(encoded, &AwbcDecodeBudget::default());
        assert!(matches!(
            RuntimeEntityReference::read_wire(&mut reader),
            Err(AwbcCodecError::InvalidMetadata { .. })
        ));
    }
    let mut reader = Reader::new(&[4, 1, 1, 255, 0], &AwbcDecodeBudget::default());
    assert!(matches!(
        RuntimeEntityReference::read_wire(&mut reader),
        Err(AwbcCodecError::InvalidUtf8 { .. })
    ));
    // An admitted large count with no payload must fail in the borrowed pass,
    // before reserving the claimed slot buffer on either pointer width.
    let mut writer = Writer::default();
    writer.write_u8(4);
    writer.write_u32_var(u32::MAX);
    let encoded = writer.into_bytes();
    let mut reader = Reader::new(
        &encoded,
        &AwbcDecodeBudget {
            collection_items: usize::MAX,
            ..AwbcDecodeBudget::default()
        },
    );
    assert!(matches!(
        RuntimeEntityReference::read_wire(&mut reader),
        Err(AwbcCodecError::Truncated { .. })
    ));
}

#[test]
fn awbc_imported_target_is_admitted_without_rebuilding_it_from_a_label() {
    let reference = RuntimeEntityReference::ImportedProject(
        RuntimeImportedProjectEntityReference::try_new(
            ProjectEntityReferenceFamily::DialogueLine,
            PublicId::try_new("say.say.line").unwrap(),
            [1; 32],
            [2; 32],
            [3; 32],
            None,
        )
        .unwrap(),
    );
    let encoded = bytes(&reference);
    let mut reader = Reader::new(&encoded, &AwbcDecodeBudget::default());
    reader.read_u8().unwrap();
    reader.read_u8().unwrap();
    reader.read_str().unwrap();
    reader.read_exact(96).unwrap();
    let target = reader.offset();
    for kind in [0, 255] {
        let mut invalid = encoded.clone();
        invalid[target] = kind;
        let mut reader = Reader::new(&invalid, &AwbcDecodeBudget::default());
        let error = RuntimeEntityReference::read_wire(&mut reader).unwrap_err();
        if kind == 0 {
            assert!(matches!(
                error,
                AwbcCodecError::InvalidMetadata {
                    kind: "imported entity owner",
                    ..
                }
            ));
        } else {
            assert!(matches!(
                error,
                AwbcCodecError::UnknownTag {
                    kind: "imported entity target",
                    ..
                }
            ));
        }
    }
    let mut invalid = encoded;
    invalid[target + 3] = b'x';
    let mut reader = Reader::new(&invalid, &AwbcDecodeBudget::default());
    assert!(matches!(
        RuntimeEntityReference::read_wire(&mut reader),
        Err(AwbcCodecError::InvalidMetadata {
            kind: "imported entity owner",
            ..
        })
    ));
}

#[test]
fn awbc_entity_writer_rolls_back_an_invalid_original_path() {
    let path: RuntimeIdPath = serde_json::from_value(serde_json::json!({"segments":[""]})).unwrap();
    let reference = RuntimeEntityReference::DialogueLine(RuntimeLineId::from_source_path(path));
    let mut writer = Writer::default();
    writer.write_bytes(b"prefix");
    assert!(matches!(
        reference.write_wire(&mut writer),
        Err(AwbcCodecError::InvalidMetadata { .. })
    ));
    assert_eq!(writer.into_bytes(), b"prefix");
}
