use crate::entry::RuntimeSchemaError;
use crate::entry::schema::canonical_runtime_value_bytes;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{FlowRuntimeId, RuntimeLineId};
use crate::runtime_id::{RuntimeIdFamily, RuntimeIdPath, RuntimeIdSegment};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticEncodingError, TaskSemanticMeter};
use crate::value::{
    RuntimeEntityReference, RuntimeEntityReferenceField, RuntimeImportedProjectEntityReference,
    RuntimeValue,
};

fn path(family: RuntimeIdFamily, segments: &[&str]) -> RuntimeIdPath {
    RuntimeIdPath::from_segments(
        family,
        segments
            .iter()
            .map(|segment| RuntimeIdSegment::new(family, segment).unwrap()),
    )
    .unwrap()
}

fn flow(segments: &[&str], label: &str) -> FlowRuntimeId {
    FlowRuntimeId::from_source_path(path(RuntimeIdFamily::Flow, segments), label.to_owned())
}

fn imported(segments: &[&str]) -> RuntimeEntityReference {
    RuntimeEntityReference::ImportedProject(
        RuntimeImportedProjectEntityReference::try_new(
            arcweft_id::ProjectEntityReferenceFamily::Flow,
            arcweft_id::PublicId::try_new("flow.opening").unwrap(),
            [1; 32],
            [2; 32],
            [3; 32],
            Some(flow(segments, "flow.opening")),
        )
        .unwrap(),
    )
}

fn segment_pairs() -> [(RuntimeEntityReference, RuntimeEntityReference); 3] {
    [
        (
            RuntimeEntityReference::StructuralFlow(flow(&["one.two"], "flow.opening")),
            RuntimeEntityReference::StructuralFlow(flow(&["one", "two"], "flow.opening")),
        ),
        (
            RuntimeEntityReference::DialogueLine(RuntimeLineId::from_source_path(path(
                RuntimeIdFamily::Line,
                &["one.two"],
            ))),
            RuntimeEntityReference::DialogueLine(RuntimeLineId::from_source_path(path(
                RuntimeIdFamily::Line,
                &["one", "two"],
            ))),
        ),
        (imported(&["one.two"]), imported(&["one", "two"])),
    ]
}

fn literal_digest(value: &RuntimeValue) -> blake3::Hash {
    let mut meter = TaskSemanticMeter::new(1 << 20, 1 << 20);
    let mut encoder = TaskSemanticEncoder::new(b"entity-identity-test.v1\0", &mut meter);
    value.encode_static_literal(&mut encoder).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn canonical_entity_identity_distinguishes_original_segment_boundaries() {
    for (left, right) in segment_pairs() {
        assert_ne!(left, right);
        assert_ne!(
            canonical_runtime_value_bytes(&RuntimeValue::EntityRef(left), 1024).unwrap(),
            canonical_runtime_value_bytes(&RuntimeValue::EntityRef(right), 1024).unwrap()
        );
    }
}

#[test]
fn executable_entity_identity_distinguishes_original_segment_boundaries() {
    for (left, right) in segment_pairs() {
        assert_ne!(left, right);
        assert_ne!(
            literal_digest(&RuntimeValue::EntityRef(left)),
            literal_digest(&RuntimeValue::EntityRef(right))
        );
    }
}

#[test]
fn flow_public_labels_change_canonical_and_executable_value_identity() {
    let left = RuntimeEntityReference::StructuralFlow(flow(&["main"], "flow.left"));
    let right = RuntimeEntityReference::StructuralFlow(flow(&["main"], "flow.right"));
    // The stored runtime paths are equal, while the complete references retain
    // distinct observable public selectors. Both fields belong in identity.
    let (
        RuntimeEntityReference::StructuralFlow(left_flow),
        RuntimeEntityReference::StructuralFlow(right_flow),
    ) = (&left, &right)
    else {
        unreachable!()
    };
    assert_eq!(left_flow.path(), right_flow.path());
    assert_ne!(left, right);
    assert_ne!(
        left.field_value(RuntimeEntityReferenceField::Id),
        right.field_value(RuntimeEntityReferenceField::Id)
    );
    let left = RuntimeValue::EntityRef(left);
    let right = RuntimeValue::EntityRef(right);
    assert_ne!(
        canonical_runtime_value_bytes(&left, 1024).unwrap(),
        canonical_runtime_value_bytes(&right, 1024).unwrap()
    );
    assert_ne!(literal_digest(&left), literal_digest(&right));
}

#[test]
fn common_executable_seal_distinguishes_observable_entity_reference_labels() {
    use crate::plan::{
        RuntimeExprMatchArmSeed, RuntimeExprSeed, RuntimeExprSeedKind,
        RuntimeFunctionDefinitionIdentity, RuntimeFunctionSemanticRole, RuntimePatternSeed,
        RuntimePatternSeedKind, RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
    };
    #[derive(Clone, Copy)]
    enum Embedding {
        Literal,
        Expression,
        Pattern,
    }
    let plan = |label, embedding| {
        let mut builder = RuntimePlanBuilder::new();
        let ty = RuntimeSemanticTypeId::from_bytes([91; 32]);
        let boolean = RuntimeSemanticTypeId::from_bytes([93; 32]);
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(ty, RuntimePlanTypeProjection::EntityReference),
                    RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                ],
                [],
            )
            .unwrap();
        let reference = RuntimeEntityReference::StructuralFlow(flow(&["main"], label));
        let (result, body) = match embedding {
            Embedding::Literal => (
                ty,
                RuntimeExprSeedKind::Value(RuntimeValue::EntityRef(reference)),
            ),
            Embedding::Expression => (ty, RuntimeExprSeedKind::EntityRef(reference)),
            Embedding::Pattern => (
                boolean,
                RuntimeExprSeedKind::Match {
                    scrutinee: Box::new(RuntimeExprSeed::new(
                        ty,
                        RuntimeExprSeedKind::EntityRef(RuntimeEntityReference::StructuralFlow(
                            flow(&["main"], "flow.left"),
                        )),
                    )),
                    arms: Box::new([
                        RuntimeExprMatchArmSeed::new(
                            RuntimePatternSeed::new(ty, RuntimePatternSeedKind::Entity(reference)),
                            None,
                            RuntimeExprSeed::new(
                                boolean,
                                RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                            ),
                        ),
                        RuntimeExprMatchArmSeed::new(
                            RuntimePatternSeed::new(ty, RuntimePatternSeedKind::Discard),
                            None,
                            RuntimeExprSeed::new(
                                boolean,
                                RuntimeExprSeedKind::Value(RuntimeValue::Bool(false)),
                            ),
                        ),
                    ]),
                },
            ),
        };
        builder
            .push_function_site_seed(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([92; 32]),
                RuntimeFunctionSemanticRole::Ordinary,
                [],
                RuntimeExprSeed::new(result, body),
            )
            .unwrap();
        builder.finish().unwrap()
    };
    for embedding in [
        Embedding::Literal,
        Embedding::Expression,
        Embedding::Pattern,
    ] {
        assert_ne!(
            plan("flow.left", embedding).executable_semantic_digest(),
            plan("flow.right", embedding).executable_semantic_digest()
        );
    }
}

#[test]
fn entity_identity_preserves_canonical_literal_and_source_frame_goldens() {
    use crate::entry::RuntimeSchemaLimits;
    use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};
    use crate::value::{AwbcRuntimeValueSnapshot, RuntimeLiteralSourceLimits};
    let reference = RuntimeEntityReference::StructuralFlow(flow(&["root", "child"], "flow.label"));
    let value = RuntimeValue::EntityRef(reference.clone());
    assert_eq!(
        canonical_runtime_value_bytes(&value, 1024).unwrap(),
        b"\x0a\x04\x02\x04root\x05child\x0aflow.label"
    );
    let literal = b"entity-identity-test.v1\0\x0b\x04\x02\0\0\0\x04\0\0\0root\x05\0\0\0child\x0a\0\0\0flow.label";
    assert_eq!(literal_digest(&value), blake3::hash(literal));
    // The shared field owner leaves the existing private v1 source wire intact.
    let expected = b"arcweft.runtime.literal-source.v1\0\x01\x0e\x04\x02\0\0\0\x04\0\0\0root\x05\0\0\0child\x0a\0\0\0flow.label";
    let source = AwbcRuntimeValueSnapshot::EntityRef(reference);
    let limits = RuntimeLiteralSourceLimits {
        encoded_bytes: 1024,
        decoded_bytes: 4096,
        elements: 100,
    };
    let resources = ValueResourceAdmission::snapshot(
        RuntimeSchemaLimits::engine_default(),
        ValueAdmissionOrigin::InertSnapshot,
    );
    let mut meter = TaskSemanticMeter::new(10_000, 10_000);
    let prepared = source
        .prepare_literal_source_encoding(limits, &resources, &mut meter)
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    assert_eq!(bytes, expected);
    let decoded = AwbcRuntimeValueSnapshot::preflight_literal_source(
        expected, limits, &resources, &mut meter,
    )
    .unwrap()
    .materialize(&mut meter)
    .unwrap();
    assert_eq!(decoded, source);
}

#[test]
fn entity_identity_encoding_keeps_exact_byte_and_precharged_work_boundaries() {
    let value = RuntimeValue::EntityRef(RuntimeEntityReference::StructuralFlow(flow(
        &["root", "child"],
        "flow.label",
    )));
    let bytes = canonical_runtime_value_bytes(&value, 1024).unwrap();
    assert_eq!(
        canonical_runtime_value_bytes(&value, bytes.len()).unwrap(),
        bytes
    );
    assert_eq!(
        canonical_runtime_value_bytes(&value, bytes.len() - 1),
        Err(RuntimeSchemaError::BudgetExceeded {
            budget: "encoded_bytes"
        })
    );
    let mut measured = TaskSemanticMeter::new(u64::MAX, u64::MAX);
    let mut encoder = TaskSemanticEncoder::new(b"identity-boundary.v1\0", &mut measured);
    value.encode_static_literal(&mut encoder).unwrap();
    let expected = encoder.finish().unwrap();
    let (work, bytes) = measured.totals();
    let mut exact = TaskSemanticMeter::new(work, bytes);
    let mut encoder = TaskSemanticEncoder::new(b"identity-boundary.v1\0", &mut exact);
    value.encode_static_literal(&mut encoder).unwrap();
    assert_eq!(encoder.finish().unwrap(), expected);
    let mut short = TaskSemanticMeter::new(work, bytes);
    short.charge_work(1).unwrap();
    let mut encoder = TaskSemanticEncoder::new(b"identity-boundary.v1\0", &mut short);
    assert!(value.encode_static_literal(&mut encoder).is_err());
    assert_eq!(
        encoder.finish(),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    assert_eq!(short.status(), Err(TaskSemanticEncodingError::SemanticWork));
}
