use super::*;
use crate::awbc_lower::AwbcLowerOptions;
use arcweft_core::entry::RuntimeMapKind;
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{RuntimePlanBuilder, RuntimePlanTypeSeed};

#[test]
fn agent_probe_results_remain_distinct_through_awbc_interning() {
    let mut inventory = AwbcInventory::new("probe-projection", AwbcLowerOptions::default());
    let probes: Vec<_> = [RuntimeCheckedType::Bool, RuntimeCheckedType::String]
        .into_iter()
        .map(|result| {
            let checked =
                RuntimeCheckedType::Agent(RuntimeAgentTypeProjection::Probe(Box::new(result)));
            let ty = intern_runtime_type(&mut inventory, &checked);
            (ty, checked)
        })
        .collect();
    assert_ne!(probes[0].0, probes[1].0);
    let program = inventory.finish();
    for (ty, checked) in probes {
        assert_eq!(program.checked_type(ty).unwrap(), checked);
    }
}

#[test]
fn data_shape_children_remain_exact_through_awbc_interning() {
    let mut inventory = AwbcInventory::new("data-shape-projection", AwbcLowerOptions::default());
    let shapes: Vec<_> = [RuntimeCheckedType::Bool, RuntimeCheckedType::String]
        .into_iter()
        .map(|value| {
            let checked =
                RuntimeCheckedType::Agent(RuntimeAgentTypeProjection::DataShape(Box::new(value)));
            let ty = intern_runtime_type(&mut inventory, &checked);
            (ty, checked)
        })
        .collect();
    assert_ne!(shapes[0].0, shapes[1].0);
    let program = inventory.finish();
    for (ty, checked) in shapes {
        assert_eq!(program.checked_type(ty).unwrap(), checked);
        let row = &program.runtime_types[ty.index()];
        let AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::DataShape(child)) = row.shape() else {
            panic!("DataShape keeps a typed AWBC child reference")
        };
        let RuntimeCheckedType::Agent(RuntimeAgentTypeProjection::DataShape(expected_child)) =
            checked
        else {
            panic!("fixture has a checked DataShape projection")
        };
        assert_eq!(
            program.checked_type(*child).unwrap(),
            expected_child.as_ref().clone()
        );
    }
}

#[test]
fn checked_array_lengths_remain_distinct_through_awbc_interning_and_projection() {
    let mut inventory = AwbcInventory::new("array-projection", AwbcLowerOptions::default());
    let arrays: Vec<_> = [0, 1, 2, u64::MAX]
        .into_iter()
        .map(|length| {
            let checked = RuntimeCheckedType::Array {
                item: Box::new(RuntimeCheckedType::Bool),
                length,
            };
            let ty = intern_runtime_type(&mut inventory, &checked);
            (ty, checked)
        })
        .collect();
    let program = inventory.finish();
    let ids: std::collections::BTreeSet<_> = arrays.iter().map(|(ty, _)| *ty).collect();
    assert_eq!(ids.len(), arrays.len());
    for (ty, checked) in arrays {
        assert_eq!(program.checked_type(ty).unwrap(), checked);
    }
}

#[test]
fn plan_map_ordering_kind_survives_awbc_type_preflight() {
    for (marker, kind) in [
        (0x31, RuntimeMapKind::Ordered),
        (0x32, RuntimeMapKind::Sorted),
        (0x33, RuntimeMapKind::BTree),
    ] {
        let key = RuntimeSemanticTypeId::from_bytes([0x11; 32]);
        let value = RuntimeSemanticTypeId::from_bytes([0x12; 32]);
        let map = RuntimeSemanticTypeId::from_bytes([marker; 32]);
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(key, RuntimePlanTypeProjection::String),
                    RuntimePlanTypeSeed::new(value, RuntimePlanTypeProjection::Bool),
                    RuntimePlanTypeSeed::new(
                        map,
                        RuntimePlanTypeProjection::Map { kind, key, value },
                    ),
                ],
                [],
            )
            .expect("map type graph admits");
        let plan = builder.finish().expect("map-only plan seals");
        let mut inventory = AwbcInventory::new("map-kind", AwbcLowerOptions::default());
        preflight_plan_types(&mut inventory, &plan).expect("plan types preflight");
        let program = inventory.finish();
        let row = program
            .runtime_types
            .iter()
            .find(|row| row.semantic_identity() == map)
            .expect("map row reaches AWBC");

        assert!(matches!(
            row.shape(),
            AwbcRuntimeTypeShape::Map { kind: actual, .. } if *actual == kind
        ));
    }
}
