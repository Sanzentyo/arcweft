use arcweft_core::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use arcweft_core::plan::{
    RuntimePlanBuildError, RuntimePlanBuilder, RuntimePlanRecordField, RuntimePlanTypeProjection,
    RuntimePlanTypeSeed, RuntimePlanTypeTableError,
};
use arcweft_core::value::{RuntimeRecordFieldId, RuntimeValue};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}

fn record_type(names: [&str; 2]) -> RuntimeCheckedType {
    RuntimeCheckedType::try_record(names.into_iter().enumerate().map(|(ordinal, name)| {
        (
            RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
            name.to_owned(),
            RuntimeCheckedType::Bool,
        )
    }))
    .unwrap()
}

#[test]
fn record_name_order_changes_the_contract_even_when_child_types_are_identical() {
    let expected = record_type(["z", "a"]);
    let reordered = record_type(["a", "z"]);
    assert_ne!(expected, reordered);
    assert_ne!(
        expected.semantic_identity_digest(),
        reordered.semantic_identity_digest()
    );
    for names in [["z", "a"], ["a", "z"], ["z", "renamed"]] {
        let value = RuntimeValue::try_record(
            names
                .into_iter()
                .map(|name| (name.to_owned(), RuntimeValue::Bool(true)))
                .collect(),
        )
        .unwrap();
        assert_eq!(expected.accepts_value(&value), names == ["z", "a"]);
    }
}

#[test]
fn conflicting_record_names_reject_the_entire_plan_type_batch() {
    let record = |name| {
        RuntimePlanTypeSeed::new(
            semantic(1),
            RuntimePlanTypeProjection::Record(Box::new([RuntimePlanRecordField::new(
                name,
                semantic(2),
            )])),
        )
    };
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                record("accepted"),
                RuntimePlanTypeSeed::new(semantic(2), RuntimePlanTypeProjection::Bool),
            ],
            [],
            [],
            [],
        )
        .unwrap();
    assert!(matches!(
        builder.admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(semantic(3), RuntimePlanTypeProjection::String),
                record("conflicting"),
            ],
            [],
            [],
            [],
        ),
        Err(RuntimePlanBuildError::TypeGraph(
            RuntimePlanTypeTableError::ConflictingProjection { .. }
        ))
    ));
    let plan = builder.finish().unwrap();
    assert_eq!(plan.type_table().declarations().len(), 2);
    assert!(plan.type_table().id_for_semantic(semantic(3)).is_none());
    let ty = plan.type_table().id_for_semantic(semantic(1)).unwrap();
    let checked = plan.checked_type(ty).unwrap().unwrap();
    for name in ["accepted", "conflicting"] {
        let value =
            RuntimeValue::try_record(vec![(name.to_owned(), RuntimeValue::Bool(true))]).unwrap();
        assert_eq!(checked.accepts_value(&value), name == "accepted");
    }
}
