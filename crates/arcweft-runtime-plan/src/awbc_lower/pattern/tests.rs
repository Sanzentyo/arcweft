use super::*;
use crate::awbc_lower::AwbcLowerOptions;

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
