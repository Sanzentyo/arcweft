use core::num::{NonZeroU32, NonZeroU64};
use std::collections::BTreeSet;

use super::*;
use crate::identity::{HirDatabaseId, HirIdKind, HirModuleId, HirTypedId, RawHirId};

fn type_id(slot: u32) -> TypeId {
    let module = HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::MIN),
        NonZeroU32::MIN,
    );
    TypeId::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).expect("test type slot"),
        HirIdKind::Type,
    ))
}

#[test]
fn dual_type_root_reachability_is_runtime_bearing() {
    let root = type_id(1);
    let projection = HirExpressionTypeRootProjection {
        runtime_bearing: BTreeSet::from([root]),
        semantic: BTreeSet::from([root]),
        resolution_inputs: BTreeSet::from([root]),
    };

    assert!(projection.contains_runtime_bearing(root));
    assert!(projection.contains_semantic(root));
    assert!(!projection.is_semantic_only(root));
    assert_eq!(
        projection.disposition(root),
        Some(HirTypeRootDisposition::RuntimeBearing)
    );
}

#[test]
fn semantic_only_disposition_requires_no_runtime_reachability() {
    let root = type_id(2);
    let projection = HirExpressionTypeRootProjection {
        runtime_bearing: BTreeSet::new(),
        semantic: BTreeSet::from([root]),
        resolution_inputs: BTreeSet::new(),
    };

    assert!(!projection.contains_runtime_bearing(root));
    assert!(projection.contains_semantic(root));
    assert!(projection.is_semantic_only(root));
    assert_eq!(
        projection.disposition(root),
        Some(HirTypeRootDisposition::SemanticOnly)
    );
}

#[test]
fn callee_resolution_input_requires_no_standalone_value_type() {
    let root = type_id(3);
    let projection = HirExpressionTypeRootProjection {
        runtime_bearing: BTreeSet::new(),
        semantic: BTreeSet::new(),
        resolution_inputs: BTreeSet::from([root]),
    };
    assert!(projection.is_non_runtime(root));
    assert!(!projection.is_semantic_only(root));
    assert_eq!(
        projection.disposition(root),
        Some(HirTypeRootDisposition::ResolutionInput)
    );
}
