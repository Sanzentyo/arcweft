use arcweft_core::pattern::{
    RuntimeBuiltinVariantIdentity, RuntimeSemanticTypeId, RuntimeVariantIdentity,
};
use arcweft_core::plan::{
    RuntimePlan, RuntimePlanBuildError, RuntimePlanBuilder, RuntimePlanTypeProjection as Type,
    RuntimePlanTypeSeed, RuntimePlanTypeTableError,
};
use arcweft_core::value::RuntimeValue;

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}

fn seed(tag: u8, projection: Type<RuntimeSemanticTypeId>) -> RuntimePlanTypeSeed {
    RuntimePlanTypeSeed::new(semantic(tag), projection)
}

fn types(root: Type<RuntimeSemanticTypeId>) -> [RuntimePlanTypeSeed; 7] {
    [
        seed(1, root),
        seed(2, Type::Bool),
        seed(3, Type::Tuple(Box::new([semantic(2)]))),
        seed(4, Type::Tuple(Box::new([]))),
        seed(5, Type::Tuple(Box::new([semantic(2), semantic(2)]))),
        seed(6, Type::Bool),
        seed(7, Type::Tuple(Box::new([semantic(6)]))),
    ]
}

fn plan(root: Type<RuntimeSemanticTypeId>) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(types(root), [], [], [])
        .unwrap();
    builder.finish().unwrap()
}

fn rejects_atomically(root: Type<RuntimeSemanticTypeId>) {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch([seed(9, Type::Unit)], [], [], [])
        .unwrap();
    assert!(
        matches!(builder.admit_semantic_batch(types(root), [], [], []),
        Err(RuntimePlanBuildError::TypeGraph(RuntimePlanTypeTableError::InvalidBuiltinVariantSchema {
            semantic_identity,
        })) if semantic_identity == semantic(1))
    );
    let plan = builder.finish().unwrap();
    assert_eq!(plan.type_table().declarations().len(), 1);
    assert!(plan.type_table().id_for_semantic(semantic(9)).is_some());
    assert!(plan.type_table().id_for_semantic(semantic(1)).is_none());
}

#[test]
fn every_builtin_payload_case_requires_one_tuple_item_before_publication() {
    for owner in (0..=u8::MAX).filter_map(RuntimeBuiltinVariantIdentity::from_wire_tag) {
        let projection = |payload| Type::BuiltinVariant {
            owner,
            cases: owner
                .cases()
                .iter()
                .map(|case| case.has_payload().then_some(semantic(payload)))
                .collect(),
        };
        let plan = plan(projection(3));
        let ty = plan.type_table().id_for_semantic(semantic(1)).unwrap();
        let checked = plan.checked_type(ty).unwrap().unwrap();
        for (ordinal, case) in owner.cases().iter().enumerate() {
            let value = RuntimeValue::Variant {
                owner: RuntimeVariantIdentity::Builtin(owner),
                ordinal: u32::try_from(ordinal).unwrap(),
                name: case.name().to_owned(),
                payload: case
                    .has_payload()
                    .then(|| Box::new(RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)]))),
            };
            assert!(checked.accepts_value(&value), "{owner:?}::{case:?}");
        }
        if owner.cases().iter().any(|case| case.has_payload()) {
            for payload in [2, 4, 5] {
                rejects_atomically(projection(payload));
            }
        }
    }
}

#[test]
fn option_and_result_payloads_must_reference_their_exact_declared_arguments() {
    for payload in [2, 4, 5, 7] {
        rejects_atomically(Type::Option {
            item: semantic(2),
            some_payload: semantic(payload),
        });
        rejects_atomically(Type::Result {
            value: semantic(2),
            error: semantic(2),
            value_payload: semantic(payload),
            error_payload: semantic(3),
        });
        rejects_atomically(Type::Result {
            value: semantic(2),
            error: semantic(2),
            value_payload: semantic(3),
            error_payload: semantic(payload),
        });
    }
    // Type 6 also projects to Bool: matching only a materialized predicate
    // would erase its different declaration identity and accept payload 7.
    let option = plan(Type::Option {
        item: semantic(2),
        some_payload: semantic(3),
    });
    let ty = option.type_table().id_for_semantic(semantic(1)).unwrap();
    assert!(
        option
            .checked_type(ty)
            .unwrap()
            .unwrap()
            .accepts_value(&RuntimeValue::option_some(RuntimeValue::Bool(true)))
    );
    let result = plan(Type::Result {
        value: semantic(2),
        error: semantic(6),
        value_payload: semantic(3),
        error_payload: semantic(7),
    });
    let ty = result.type_table().id_for_semantic(semantic(1)).unwrap();
    let checked = result.checked_type(ty).unwrap().unwrap();
    assert!(checked.accepts_value(&RuntimeValue::result_ok(RuntimeValue::Bool(true))));
    assert!(checked.accepts_value(&RuntimeValue::result_err(RuntimeValue::Bool(false))));
}
