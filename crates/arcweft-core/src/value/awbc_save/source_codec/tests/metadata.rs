use super::*;
use crate::entry::{
    RuntimeCommandConstructorId, RuntimeCommandTargetId, RuntimeNominalTypeId, TypeLayoutHash,
};
use crate::pattern::{
    RuntimeBuiltinVariantIdentity, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
    RuntimeSemanticTypeId, RuntimeVariantIdentity,
};
use crate::value::awbc_save::{
    AwbcRuntimeCommandSnapshot, AwbcRuntimeNominalFieldSnapshot, AwbcRuntimeNominalRecordSnapshot,
    AwbcRuntimeOpaqueSnapshot, AwbcRuntimeReductionSnapshot,
};
use crate::value::{
    RuntimeImportedProjectEntityReference, RuntimeOpaquePersistence, RuntimeOpaqueValueClass,
};
use arcweft_id::{DeclarationIdentityFamily, ProjectEntityReferenceFamily, PublicId};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}

#[test]
fn literal_source_retains_exact_entity_segments_public_labels_and_imported_targets() {
    use crate::plan::{FlowRuntimeId, RuntimeLineId};
    let checked_flow =
        FlowRuntimeId::from_checked_declaration_digest([17; 32], "flow.opening").unwrap();
    let mut entities = vec![
        RuntimeEntityReference::StructuralFlow(checked_flow.clone()),
        RuntimeEntityReference::StructuralFlow(
            FlowRuntimeId::from_runtime_contract("main.branch", "public.selector").unwrap(),
        ),
        RuntimeEntityReference::StructuralFlow(
            FlowRuntimeId::from_source_entity_body("flow.flow.main").unwrap(),
        ),
        RuntimeEntityReference::DialogueLine(
            RuntimeLineId::from_source_entity_body("say.say.line").unwrap(),
        ),
        RuntimeEntityReference::try_project(
            DeclarationIdentityFamily::Signal,
            PublicId::try_new("signal.level").unwrap(),
        )
        .unwrap(),
        RuntimeEntityReference::CharacterLook {
            character: arcweft_character::id::CharacterId::try_new("character.hero").unwrap(),
            look: arcweft_character::id::CharacterLookId::try_new("smile").unwrap(),
        },
    ];
    for (family, id, flow) in [
        (ProjectEntityReferenceFamily::Signal, "signal.level", None),
        (
            ProjectEntityReferenceFamily::Flow,
            "flow.opening",
            Some(checked_flow),
        ),
        (
            ProjectEntityReferenceFamily::DialogueLine,
            "say.say.line",
            None,
        ),
    ] {
        entities.push(RuntimeEntityReference::ImportedProject(
            RuntimeImportedProjectEntityReference::try_new(
                family,
                PublicId::try_new(id).unwrap(),
                [1; 32],
                [2; 32],
                [3; 32],
                flow,
            )
            .unwrap(),
        ));
    }
    for entity in entities {
        roundtrip(Value::EntityRef(entity));
    }
    // A public label is deliberately independent of the actual runtime path.
    let family = crate::runtime_id::RuntimeIdFamily::Flow;
    let segment = |text| crate::runtime_id::RuntimeIdSegment::new(family, text).unwrap();
    let path =
        crate::runtime_id::RuntimeIdPath::from_segments(family, [segment("one.two")]).unwrap();
    let other =
        crate::runtime_id::RuntimeIdPath::from_segments(family, [segment("one"), segment("two")])
            .unwrap();
    assert_ne!(path, other);
    let first = Value::EntityRef(RuntimeEntityReference::StructuralFlow(
        FlowRuntimeId::from_source_path(path, "label".into()),
    ));
    let second = Value::EntityRef(RuntimeEntityReference::StructuralFlow(
        FlowRuntimeId::from_source_path(other, "label".into()),
    ));
    assert_ne!(encode(&first), encode(&second));
    roundtrip(first);
    roundtrip(second);
}

#[test]
fn literal_source_preserves_nominal_effect_scope_opaque_and_reduction_metadata() {
    let binding = crate::program_types::RuntimeFunctionEffectInstantiation::from_source_rows(
        semantic(8),
        vec![
            [
                arcweft_id::EffectId::parse("io.read").unwrap(),
                arcweft_id::EffectId::parse("io.write").unwrap(),
            ]
            .into_iter()
            .collect(),
            crate::effect_row::EffectSet::new(),
        ]
        .into_boxed_slice(),
    );
    let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap();
    let nominal = RuntimeNominalTypeId::try_new("fixture.Source").unwrap();
    roundtrip(Value::NominalRecord(AwbcRuntimeNominalRecordSnapshot {
        type_id: nominal.clone(),
        semantic_identity: semantic(1),
        layout: TypeLayoutHash::from_bytes([2; 32]),
        type_instantiation: Some(binding.clone()),
        fields: vec![AwbcRuntimeNominalFieldSnapshot {
            field,
            value: Value::Bool(true),
        }],
    }));
    for owner in [
        RuntimeVariantIdentity::Nominal {
            nominal,
            semantic_identity: semantic(3),
            layout: TypeLayoutHash::from_bytes([4; 32]),
        },
        RuntimeVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
    ] {
        roundtrip(Value::Variant {
            owner,
            ordinal: 1,
            name: "Some".into(),
            type_instantiation: Some(binding.clone()),
            payload: Some(Box::new(Value::Tuple(vec![Value::Bool(true)]))),
        });
    }
    roundtrip(Value::Opaque(AwbcRuntimeOpaqueSnapshot {
        producer: RuntimeOpaqueTypeProducerId::try_new("fixture.Box").unwrap(),
        semantic_identity: semantic(5),
        value_class: RuntimeOpaqueValueClass::Plain,
        persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
        payload: Box::new(Value::Bool(true)),
    }));
    roundtrip(Value::Reduction(AwbcRuntimeReductionSnapshot {
        owner: RuntimeOpaqueTypeOwner::exact(
            RuntimeOpaqueTypeProducerId::try_new("std.reduction").unwrap(),
            semantic(6),
        ),
        state: Box::new(Value::String("state".into())),
        commands: vec![AwbcRuntimeCommandSnapshot {
            constructor: RuntimeCommandConstructorId::try_new("presentation.show").unwrap(),
            target: RuntimeCommandTargetId::try_new("scene.object").unwrap(),
            payload: Box::new(Value::Tuple(vec![Value::Bool(false)])),
        }],
    }));
}

#[test]
fn literal_source_rejects_runtime_carriers_empty_predicates_and_malformed_dense_fields() {
    let frame = |body: &[u8]| {
        let mut bytes = DOMAIN.to_vec();
        bytes.push(1);
        bytes.extend_from_slice(body);
        bytes
    };
    for body in [
        vec![23],
        vec![24],
        vec![13, 2],
        vec![21, 9],
        vec![15, 1, 0, 0, 0, 24],
    ] {
        assert!(matches!(
            rejected(&frame(&body)),
            RuntimeLiteralSourceError::RuntimeCarrier { .. }
        ));
    }
    for collection in [4, 5] {
        assert!(matches!(
            rejected(&frame(&[21, 6, collection, 0, 0, 0, 0])),
            RuntimeLiteralSourceError::PredicateOperands { .. }
        ));
    }
    assert!(matches!(
        rejected(&frame(&[16, 1, 255])),
        RuntimeLiteralSourceError::Tag {
            kind: "dense storage",
            ..
        }
    ));
    assert!(matches!(
        rejected(&frame(&[16, 1, 15, 1, 0, 0, 0, 2])),
        RuntimeLiteralSourceError::Tag { .. }
    ));
    assert!(matches!(
        rejected(&frame(&[17, 1, 0, 0, 0, 0, 0, 0, 0])),
        RuntimeLiteralSourceError::Field { actual: 0, .. }
    ));
    let carrier = Value::Tuple(vec![Value::Agent(Agent::DataShape {
        shape_type: semantic(1),
        value_type: semantic(2),
    })]);
    assert!(matches!(
        carrier.prepare_literal_source_encoding(limits(), &resources(), &mut meter()),
        Err(RuntimeLiteralSourceError::RuntimeCarrier { .. })
    ));
}

#[test]
fn literal_source_bounds_aggregate_path_label_strings_and_framing_depth_before_retention() {
    let value = Value::EntityRef(RuntimeEntityReference::StructuralFlow(
        crate::plan::FlowRuntimeId::from_runtime_contract("a.b.c", "label").unwrap(),
    ));
    let bytes = encode(&value);
    let bank = |maximum| {
        ValueResourceAdmission::snapshot(
            RuntimeSchemaLimits {
                max_string_bytes: maximum,
                ..RuntimeSchemaLimits::engine_default()
            },
            ValueAdmissionOrigin::InertSnapshot,
        )
    };
    Value::preflight_literal_source(&bytes, limits(), &bank(5), &mut meter()).unwrap();
    assert!(matches!(
        Value::preflight_literal_source(&bytes, limits(), &bank(4), &mut meter()),
        Err(RuntimeLiteralSourceError::Limit {
            kind: "string bytes",
            ..
        })
    ));
    assert!(matches!(
        value.prepare_literal_source_encoding(limits(), &bank(4), &mut meter()),
        Err(RuntimeLiteralSourceError::Limit {
            kind: "string bytes",
            ..
        })
    ));
    let nested = (0..9).fold(Value::Unit, |value, _| Value::Tuple(vec![value]));
    let bytes = encode(&nested);
    let strict = ValueResourceAdmission::snapshot(
        RuntimeSchemaLimits {
            max_depth: 0,
            ..RuntimeSchemaLimits::engine_default()
        },
        ValueAdmissionOrigin::InertSnapshot,
    );
    assert!(matches!(
        Value::preflight_literal_source(&bytes, limits(), &strict, &mut meter()),
        Err(RuntimeLiteralSourceError::Limit {
            kind: "inert source depth",
            ..
        })
    ));
    assert!(matches!(
        nested.prepare_literal_source_encoding(limits(), &strict, &mut meter()),
        Err(RuntimeLiteralSourceError::Limit {
            kind: "inert source depth",
            ..
        })
    ));
}

#[test]
fn literal_source_rejects_invalid_numeric_shape_and_noncanonical_effect_rows_in_preflight() {
    use crate::math::{
        RuntimeDenseNumericElements, RuntimeDenseNumericPayload, RuntimeDenseNumericType,
    };
    let ty = RuntimeDenseNumericType::ALL
        .into_iter()
        .find(|ty| !ty.accepts_rank(3) && ty.scalar_width() == crate::math::RuntimeFloatWidth::F32)
        .unwrap();
    let value = Value::DenseNumeric(
        RuntimeDenseNumericPayload::try_new(
            ty,
            vec![1, 2],
            RuntimeDenseNumericElements::F32(vec![0, 0]),
        )
        .unwrap(),
    );
    let mut bytes = encode(&value);
    let first_dimension = DOMAIN.len() + 1 + 1 + 2 + 4;
    bytes[first_dimension..first_dimension + 8].copy_from_slice(&3_u64.to_le_bytes());
    assert!(matches!(
        rejected(&bytes),
        RuntimeLiteralSourceError::Dense { .. }
    ));
    let binding = crate::program_types::RuntimeFunctionEffectInstantiation::from_source_rows(
        semantic(8),
        vec![
            [
                arcweft_id::EffectId::parse("io.read").unwrap(),
                arcweft_id::EffectId::parse("io.work").unwrap(),
            ]
            .into_iter()
            .collect(),
        ]
        .into_boxed_slice(),
    );
    let value = Value::Variant {
        owner: RuntimeVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
        ordinal: 0,
        name: "None".into(),
        type_instantiation: Some(binding),
        payload: None,
    };
    let bytes = encode(&value);
    let first = bytes
        .windows(7)
        .position(|bytes| bytes == b"io.read")
        .unwrap();
    let second = bytes
        .windows(7)
        .position(|bytes| bytes == b"io.work")
        .unwrap();
    for duplicate in [false, true] {
        let mut invalid = bytes.clone();
        invalid[first..first + 7].copy_from_slice(b"io.work");
        if !duplicate {
            invalid[second..second + 7].copy_from_slice(b"io.read");
        }
        assert!(matches!(
            rejected(&invalid),
            RuntimeLiteralSourceError::Invalid {
                kind: "effect set canonical order",
                ..
            }
        ));
    }
}
