use super::{ParsedReducerResult, parse_reducer_result};
use crate::{
    pattern::{RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId},
    value::{RuntimeReductionValue, RuntimeValue},
};

#[test]
fn canonical_result_ok_delivers_its_inner_reduction() {
    let owner = RuntimeOpaqueTypeOwner::exact(
        RuntimeOpaqueTypeProducerId::try_new("std.reduction").unwrap(),
        RuntimeSemanticTypeId::from_bytes([20; 32]),
    );
    let reduction = RuntimeReductionValue::try_unchanged(owner, RuntimeValue::Bool(true)).unwrap();
    let value = RuntimeValue::Reduction(reduction.clone());
    let parsed = parse_reducer_result(RuntimeValue::result_ok(value.clone())).unwrap();
    let ParsedReducerResult::Committed(actual) = parsed else {
        panic!("Result::Ok must deliver the admitted Reduction");
    };
    assert_eq!(actual, reduction);

    assert!(parse_reducer_result(value.clone()).is_err());
    assert!(parse_reducer_result(RuntimeValue::option_some(value.clone())).is_err());
    assert!(
        parse_reducer_result(RuntimeValue::result_ok(RuntimeValue::Tuple(vec![value]))).is_err()
    );
}
