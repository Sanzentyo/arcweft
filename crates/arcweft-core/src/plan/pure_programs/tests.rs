use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::runtime_id::RuntimeFunctionSiteId;
use std::num::NonZeroU32;

fn binding(program: u8, site: u32) -> RuntimePureProgramBinding {
    RuntimePureProgramBinding::new(
        RuntimePureProgramId::from_checked_digest([program; 32]),
        RuntimeFunctionSiteId::from_accepted_ordinal(NonZeroU32::new(site).unwrap()),
        None,
        [],
        RuntimeSemanticTypeId::from_bytes([1; 32]),
    )
}

#[test]
fn indexed_lookup_keeps_source_order_and_exact_binding_identity() {
    let rows = vec![binding(2, 1), binding(1, 2)];
    let table = RuntimePureProgramTable::from_rows(rows.clone());
    assert_eq!(table.as_slice(), rows);
    assert!(std::ptr::eq(
        table.resolve(rows[1].program()).unwrap(),
        &table.as_slice()[1]
    ));
    assert_eq!(
        table.resolve(RuntimePureProgramId::from_checked_digest([9; 32])),
        Err(RuntimePureProgramLookupError::Missing)
    );
}

#[test]
fn duplicate_bindings_remain_ambiguous_until_structural_rejection() {
    let first = binding(1, 1);
    let table =
        RuntimePureProgramTable::from_rows(vec![first.clone(), binding(2, 2), binding(1, 3)]);
    assert_eq!(table.as_slice()[0], first);
    assert_eq!(
        table.resolve(first.program()),
        Err(RuntimePureProgramLookupError::Ambiguous)
    );
    assert!(
        table
            .resolve(RuntimePureProgramId::from_checked_digest([2; 32]))
            .is_ok()
    );
}
