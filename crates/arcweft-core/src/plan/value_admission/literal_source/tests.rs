use super::*;
use crate::entry::{RuntimeSchemaError, RuntimeSchemaLimits};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{RuntimePlanInventory, RuntimeTaskPlanSealLimits};
use crate::program_types::RuntimeProgramTypes;
use crate::value::{AwbcRuntimeCallableSnapshot, RuntimeValue};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}

fn inventory() -> RuntimePlanInventory {
    crate::plan::type_table::source_codec::domains::tests::builder(
        crate::entry::RuntimeNominalRecordShape::Record,
        2,
        true,
    )
    .prepare_inventory(RuntimeTaskPlanSealLimits::default(), 0)
    .unwrap()
}

fn record_source(
    inventory: &RuntimePlanInventory,
) -> (RuntimePlanTypeId, AwbcRuntimeValueSnapshot) {
    let value = RuntimeProgramTypes::Plan(inventory)
        .try_record_value(
            semantic(1),
            vec![RuntimeValue::Bool(true), RuntimeValue::Bool(false)],
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
    let source = AwbcRuntimeValueSnapshot::from_admitted_storage(&value, None).unwrap();
    (
        inventory.type_table().id_for_semantic(semantic(1)).unwrap(),
        source,
    )
}

fn resources(limits: RuntimeSchemaLimits) -> ValueResourceAdmission {
    ValueResourceAdmission::snapshot(limits, ValueAdmissionOrigin::InertSnapshot)
}

#[test]
fn literal_sources_use_the_original_nominal_domains_before_program_publication() {
    let inventory = inventory();
    let (record_ty, record) = record_source(&inventory);
    let types = RuntimeProgramTypes::Plan(&inventory);
    let mut sources = vec![(record_ty, record)];
    for (ordinal, payload) in [(0, None), (1, Some(RuntimeValue::Bool(true)))] {
        let value = types
            .try_variant_value(
                semantic(2),
                ordinal,
                payload,
                RuntimeSchemaLimits::engine_default(),
            )
            .unwrap();
        sources.push((
            inventory.type_table().id_for_semantic(semantic(2)).unwrap(),
            AwbcRuntimeValueSnapshot::from_admitted_storage(&value, None).unwrap(),
        ));
    }
    let mut bank = resources(RuntimeSchemaLimits::engine_default());
    for (ty, source) in sources {
        let original = source.clone();
        inventory
            .admit_literal_source_value(ty, &source, &mut bank)
            .unwrap();
        assert_eq!(source, original);
    }
}

#[test]
fn literal_sources_reject_changed_nominal_headers_field_ids_and_payload_types() {
    let inventory = inventory();
    let (ty, original) = record_source(&inventory);
    for case in 0..6 {
        let mut source = original.clone();
        let AwbcRuntimeValueSnapshot::NominalRecord(record) = &mut source else {
            unreachable!()
        };
        match case {
            0 => record.semantic_identity = semantic(77),
            1 => record.layout = crate::entry::TypeLayoutHash::from_bytes([77; 32]),
            2 => {
                record.type_id =
                    crate::entry::RuntimeNominalTypeId::try_new("fixture.Foreign").unwrap()
            }
            3 => record.fields[1].field = record.fields[0].field,
            4 => record.fields[0].value = AwbcRuntimeValueSnapshot::String("wrong".into()),
            5 => {
                record.fields.pop();
            }
            _ => unreachable!(),
        }
        assert!(
            inventory
                .admit_literal_source_value(
                    ty,
                    &source,
                    &mut resources(RuntimeSchemaLimits::engine_default())
                )
                .is_err(),
            "mutation {case}"
        );
    }
}

#[test]
fn literal_source_resource_and_type_passes_share_precharged_sticky_work() {
    let inventory = inventory();
    let (ty, source) = record_source(&inventory);
    let mut resource_only = resources(RuntimeSchemaLimits::engine_default());
    resource_only.validate_view(source.view(), 0, None).unwrap();
    let resource_work = resource_only.work().consumed();
    let mut measured = resources(RuntimeSchemaLimits::engine_default());
    inventory
        .admit_literal_source_value(ty, &source, &mut measured)
        .unwrap();
    let work = measured.work().consumed();
    assert!(work > resource_work);
    for (precharge, allowance, accepted) in [
        (0, work, true),
        (0, work - 1, false),
        (1, work + 1, true),
        (1, work, false),
    ] {
        let mut bank = resources(RuntimeSchemaLimits {
            max_validation_work: allowance,
            ..RuntimeSchemaLimits::engine_default()
        });
        let mut shared = bank.work();
        for _ in 0..precharge {
            shared.charge(0).unwrap();
        }
        let result = inventory.admit_literal_source_value(ty, &source, &mut bank);
        if accepted {
            result.unwrap();
            assert_eq!(bank.work().consumed(), work + precharge);
        } else {
            assert!(matches!(
                result,
                Err(RuntimePlanValueAdmissionError::Value {
                    source: RuntimeSchemaError::ValidationWork { .. },
                    ..
                })
            ));
            let failure = bank.work().failure().unwrap();
            assert!(
                inventory
                    .admit_literal_source_value(ty, &source, &mut bank)
                    .is_err()
            );
            assert_eq!(bank.work().failure(), Some(failure));
        }
    }
}

#[test]
fn literal_source_rows_share_the_parent_node_limit() {
    let inventory = inventory();
    let (ty, source) = record_source(&inventory);
    let mut bank = resources(RuntimeSchemaLimits {
        max_nodes: 3,
        ..RuntimeSchemaLimits::engine_default()
    });
    inventory
        .admit_literal_source_value(ty, &source, &mut bank)
        .unwrap();
    let boolean = inventory.type_table().id_for_semantic(semantic(3)).unwrap();
    assert!(matches!(
        inventory.admit_literal_source_value(
            boolean,
            &AwbcRuntimeValueSnapshot::Bool(true),
            &mut bank
        ),
        Err(RuntimePlanValueAdmissionError::Value {
            source: RuntimeSchemaError::BudgetExceeded { budget: "nodes" },
            ..
        })
    ));
}

#[test]
fn literal_views_reject_runtime_callable_and_witness_coordinates_inside_containers() {
    let state = crate::runtime_id::RuntimeCallableStateId::from_accepted_ordinal(
        std::num::NonZeroU32::new(1).unwrap(),
    );
    // These are deliberately untrusted inert coordinates, not a program owner
    // or a constructed runtime callable/witness.
    for value in [
        AwbcRuntimeValueSnapshot::Callable(AwbcRuntimeCallableSnapshot {
            state,
            retained: vec![],
        }),
        serde_json::from_value(serde_json::json!({
            "Iterator": {"Witness": {"state": "Unit", "next": 0}}
        }))
        .unwrap(),
    ] {
        let source = AwbcRuntimeValueSnapshot::Tuple(vec![value]);
        let mut validation = value_encoding::NoSchemaValidation::default();
        assert!(matches!(
            value_encoding::validate_literal_view(
                source.view(),
                RuntimeSchemaLimits::engine_default(),
                &mut validation,
                ()
            ),
            Err(RuntimeSchemaError::Encoding { .. })
        ));
    }
}

#[test]
fn literal_sources_check_physical_storage_before_expected_type_admission() {
    let inventory = inventory();
    let (ty, _) = record_source(&inventory);
    let source = serde_json::from_value(serde_json::json!({
        "Seq": {"TupleColumns": {"len": 2, "columns": [{"Values": [{"Bool": true}]}]}}
    }))
    .unwrap();
    assert!(matches!(
        inventory.admit_literal_source_value(
            ty,
            &source,
            &mut resources(RuntimeSchemaLimits::engine_default())
        ),
        Err(RuntimePlanValueAdmissionError::Value {
            source: RuntimeSchemaError::Encoding { .. },
            ..
        })
    ));
}
