use super::*;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};

fn limits() -> RuntimeLiteralSourceLimits {
    RuntimeLiteralSourceLimits {
        encoded_bytes: 1 << 20,
        decoded_bytes: 1 << 20,
        elements: 10_000,
    }
}

fn encode(source: &AwbcRuntimeValueSnapshot) -> Vec<u8> {
    let prepared = source
        .prepare_literal_source_encoding(
            limits(),
            &resources(RuntimeSchemaLimits::engine_default()),
            &mut TaskSemanticMeter::new(u64::MAX, u64::MAX),
        )
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn binary_literal_inventory_materialization_preserves_original_nominal_storage_and_work() {
    let inventory = inventory();
    let (ty, source) = record_source(&inventory);
    let bytes = encode(&source);
    let mut expected_bank = resources(RuntimeSchemaLimits::engine_default());
    inventory
        .admit_literal_source_value(ty, &source, &mut expected_bank)
        .unwrap();
    let work = expected_bank.work().consumed();
    let mut bank = resources(RuntimeSchemaLimits {
        max_validation_work: work,
        ..RuntimeSchemaLimits::engine_default()
    });
    let mut meter = TaskSemanticMeter::new(u64::MAX, u64::MAX);
    let restored = inventory
        .decode_literal_source_value(ty, &bytes, limits(), &mut bank, &mut meter)
        .unwrap();
    assert_eq!(bank.work().consumed(), work);
    assert_eq!(
        AwbcRuntimeValueSnapshot::from_admitted_storage(&restored, None).unwrap(),
        source
    );
    assert!(
        inventory
            .decode_literal_source_value(ty, &bytes, limits(), &mut bank, &mut meter)
            .is_err()
    );
    assert!(bank.work().failure().is_some());
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn binary_literal_type_failure_poisoning_does_not_publish_a_live_value() {
    let inventory = inventory();
    let (ty, original) = record_source(&inventory);
    for mutation in 0..3 {
        let mut source = original.clone();
        let AwbcRuntimeValueSnapshot::NominalRecord(record) = &mut source else {
            unreachable!()
        };
        match mutation {
            0 => record.semantic_identity = semantic(77),
            1 => record.fields[0].value = AwbcRuntimeValueSnapshot::String("wrong type".into()),
            2 => record.fields[1].field = record.fields[0].field,
            _ => unreachable!(),
        }
        let bytes = encode(&source);
        let mut bank = resources(RuntimeSchemaLimits::engine_default());
        let mut meter = TaskSemanticMeter::new(u64::MAX, u64::MAX);
        assert!(matches!(
            inventory.decode_literal_source_value(ty, &bytes, limits(), &mut bank, &mut meter),
            Err(RuntimeLiteralSourceError::Value(_))
        ));
        assert_eq!(
            meter.status(),
            Err(TaskSemanticEncodingError::OwnerRejected)
        );
        assert!(matches!(
            inventory.decode_literal_source_value(
                ty,
                &encode(&original),
                limits(),
                &mut bank,
                &mut meter
            ),
            Err(RuntimeLiteralSourceError::Resource(
                TaskSemanticEncodingError::OwnerRejected
            ))
        ));
    }
}

#[test]
fn binary_builtin_payload_wrapper_does_not_consume_an_extra_logical_value_node() {
    use crate::plan::{RuntimePlanTypeProjection as P, RuntimePlanTypeSeed};
    let mut builder = crate::plan::RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(semantic(1), P::Bool),
                RuntimePlanTypeSeed::new(semantic(2), P::Tuple(Box::new([semantic(1)]))),
                RuntimePlanTypeSeed::new(
                    semantic(3),
                    P::Option {
                        item: semantic(1),
                        some_payload: semantic(2),
                    },
                ),
            ],
            [],
        )
        .unwrap();
    let inventory = builder
        .prepare_inventory(RuntimeTaskPlanSealLimits::default(), 0)
        .unwrap();
    let value = RuntimeValue::option_some(RuntimeValue::Bool(true));
    let source = AwbcRuntimeValueSnapshot::from_admitted_storage(&value, None).unwrap();
    let bytes = encode(&source);
    let ty = inventory.type_table().id_for_semantic(semantic(3)).unwrap();
    let bound = RuntimeSchemaLimits {
        max_nodes: 2,
        ..RuntimeSchemaLimits::engine_default()
    };
    inventory
        .admit_literal_source_value(ty, &source, &mut resources(bound))
        .unwrap();
    let restored = inventory
        .decode_literal_source_value(
            ty,
            &bytes,
            limits(),
            &mut resources(bound),
            &mut TaskSemanticMeter::new(u64::MAX, u64::MAX),
        )
        .unwrap();
    assert_eq!(restored, value);
}
