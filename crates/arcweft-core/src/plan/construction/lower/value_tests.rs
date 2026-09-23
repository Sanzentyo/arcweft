use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{RuntimePlan, RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed};
use crate::value::{RuntimeIterator, RuntimeRange};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn seed(tag: u8, projection: Type<RuntimeSemanticTypeId>) -> RuntimePlanTypeSeed {
    RuntimePlanTypeSeed::new(semantic(tag), projection)
}
fn plan(types: impl IntoIterator<Item = RuntimePlanTypeSeed>) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    builder.admit_type_batch(types, []).unwrap();
    builder.finish().unwrap()
}
fn admitted(
    plan: &RuntimePlan,
    tag: u8,
    value: &RuntimeValue,
) -> Result<crate::entry::RuntimeValueDigest, crate::plan::RuntimePlanValueAdmissionError> {
    plan.accepts_value(
        plan.type_table().id_for_semantic(semantic(tag)).unwrap(),
        value,
        crate::entry::RuntimeSchemaLimits::engine_default(),
    )
}
#[test]
fn literal_builder_uses_the_same_choice_rule_and_preserves_runtime_only_values() {
    let types = || {
        [
            seed(1, Type::Choice(Box::new([semantic(2), semantic(3)]))),
            seed(2, Type::Bool),
            seed(3, Type::AgentValue),
            seed(4, Type::Iterator(semantic(5))),
            seed(5, Type::Signed(RuntimeSignedIntWidth::I16)),
            seed(6, Type::Range(semantic(5))),
        ]
    };
    let mut builder = RuntimePlanBuilder::new();
    builder.admit_type_batch(types(), []).unwrap();
    assert!(matches!(
        builder.lower_expression(RuntimeExprSeed::new(
            semantic(1),
            RuntimeExprSeedKind::Value(RuntimeValue::Bool(true))
        )),
        Err(RuntimePlanBuildError::InvalidValueType { .. })
    ));
    let mut builder = RuntimePlanBuilder::new();
    builder.admit_type_batch(types(), []).unwrap();
    let iterator = RuntimeValue::Iterator(RuntimeIterator::Values {
        items: vec![RuntimeValue::i16(1)],
        index: 0,
    });
    let range = RuntimeValue::Range(RuntimeRange::Int {
        start: Some(crate::value::RuntimeInt::I16(1)),
        end: Some(crate::value::RuntimeInt::I16(3)),
        inclusive: false,
    });
    for (tag, value) in [(4, iterator), (6, range)] {
        builder
            .lower_expression(RuntimeExprSeed::new(
                semantic(tag),
                RuntimeExprSeedKind::Value(value.clone()),
            ))
            .unwrap();
        let sealed = plan(types());
        assert!(admitted(&sealed, tag, &value).is_err());
    }
}
