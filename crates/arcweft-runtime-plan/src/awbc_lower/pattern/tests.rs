use super::*;
use crate::awbc_lower::AwbcLowerOptions;

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
