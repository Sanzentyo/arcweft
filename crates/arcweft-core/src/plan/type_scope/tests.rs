use super::*;
use crate::effect_row::{DecisionControl, DecisionWork};

struct Meter;

impl DecisionControl for Meter {
    type Error = std::convert::Infallible;

    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn lexical_scopes_separate_namespaces_and_skip_empty_binders() {
    let outer = RuntimeTypeScope::root()
        .enter(RuntimeTypeBinder::new(1, 2, 3))
        .unwrap();
    let nested = outer.enter(RuntimeTypeBinder::new(0, 1, 0)).unwrap();
    assert_eq!(nested.enter(RuntimeTypeBinder::EMPTY).unwrap(), nested);
    assert_eq!(nested.bound_type(1, 0).unwrap().depth(), 1);
    assert!(matches!(
        nested.bound_type(0, 0),
        Err(RuntimeTypeScopeError::TypeSlot { arity: 0, .. })
    ));
    assert_eq!(nested.bound_const(0, 0).unwrap().slot(), 0);
    assert_eq!(nested.bound_effect(1, 2).unwrap().slot(), 2);
    assert!(matches!(
        nested.bound_effect(0, 0),
        Err(RuntimeTypeScopeError::EffectSlot { arity: 0, .. })
    ));
    assert!(matches!(
        nested.bound_type(2, 0),
        Err(RuntimeTypeScopeError::UnknownDepth { depth: 2 })
    ));
    assert!(matches!(
        nested.require_root(),
        Err(RuntimeTypeScopeError::ScopedRoot)
    ));
    assert!(RuntimeTypeScope::root().require_root().is_ok());
    assert_eq!(
        serde_json::from_slice::<RuntimeTypeScope>(&serde_json::to_vec(&nested).unwrap()).unwrap(),
        nested
    );
    assert!(
        serde_json::from_value::<RuntimeTypeScope>(
            serde_json::json!([{"types": 0, "const_lengths": 0, "effects": 0}])
        )
        .is_err()
    );
}

#[test]
fn function_effect_rows_and_predicates_use_the_exact_function_binder_scope() {
    let binder = RuntimeTypeBinder::new(0, 0, 2);
    let source = RuntimeTypeScope::root().enter(binder).unwrap();
    let first = EffectFormula::variable(source.bound_effect(0, 0).unwrap(), &mut Meter).unwrap();
    let second = EffectFormula::variable(source.bound_effect(0, 1).unwrap(), &mut Meter).unwrap();
    let contract = RuntimeFunctionTypeContract::new(
        binder,
        first.subset(&second, &mut Meter).unwrap(),
        first.union(&second, &mut Meter).unwrap(),
    );
    assert_eq!(
        contract.child_scope(&RuntimeTypeScope::root()).unwrap(),
        source
    );
    let malformed = RuntimeFunctionTypeContract::new(
        RuntimeTypeBinder::new(0, 0, 1),
        contract.predicate().clone(),
        contract.invocation().clone(),
    );
    assert!(matches!(
        malformed.child_scope(&RuntimeTypeScope::root()),
        Err(RuntimeTypeScopeError::EffectSlot { slot: 1, arity: 1 })
    ));
    let encoded = serde_json::to_vec(&contract).unwrap();
    assert_eq!(
        serde_json::from_slice::<RuntimeFunctionTypeContract>(&encoded).unwrap(),
        contract
    );
}

#[test]
fn decoded_coordinates_remain_inert_until_the_declared_scope_admits_them() {
    let reference: RuntimeBoundTypeReference =
        serde_json::from_value(serde_json::json!({"depth": 0, "slot": 0})).unwrap();
    assert!(RuntimeTypeScope::root().validate_type(reference).is_err());
    let scope = RuntimeTypeScope::root()
        .enter(RuntimeTypeBinder::new(1, 0, 0))
        .unwrap();
    assert!(scope.validate_type(reference).is_ok());
    assert!(
        scope
            .validate_length(RuntimeArrayLength::Constant(42))
            .is_ok()
    );
    let length = serde_json::from_value(
        serde_json::json!({"kind": "bound", "value": {"depth": 0, "slot": 0}}),
    )
    .unwrap();
    assert!(matches!(
        scope.validate_length(length),
        Err(RuntimeTypeScopeError::ConstSlot { arity: 0, .. })
    ));
}

#[test]
fn scope_admission_bounds_depth_and_rejects_uninhabited_function_contracts() {
    let binder = RuntimeTypeBinder::new(0, 0, 1);
    let scope = RuntimeTypeScope::try_from_binders(
        vec![binder; super::super::MAX_RUNTIME_PLAN_TYPE_DEPTH].into_boxed_slice(),
    )
    .unwrap();
    assert_eq!(scope.enter(binder), Err(RuntimeTypeScopeError::DepthLimit));
    assert_eq!(scope.enter(RuntimeTypeBinder::EMPTY).unwrap(), scope);
    assert!(matches!(
        RuntimeTypeScope::try_from_binders(
            vec![binder; super::super::MAX_RUNTIME_PLAN_TYPE_DEPTH + 1].into_boxed_slice()
        ),
        Err(RuntimeTypeScopeError::DepthLimit)
    ));

    let impossible = RuntimeFunctionTypeContract::new(
        RuntimeTypeBinder::EMPTY,
        EffectPredicate::impossible(),
        EffectFormula::literal(EffectSet::new(), None),
    );
    let decoded: RuntimeFunctionTypeContract =
        serde_json::from_slice(&serde_json::to_vec(&impossible).unwrap()).unwrap();
    assert_eq!(
        decoded.child_scope(&RuntimeTypeScope::root()),
        Err(RuntimeTypeScopeError::ImpossiblePredicate)
    );

    let invocation =
        EffectFormula::variable(scope.bound_effect(0, 0).unwrap(), &mut Meter).unwrap();
    let inherited = RuntimeFunctionTypeContract::new(
        RuntimeTypeBinder::EMPTY,
        EffectPredicate::unconstrained(),
        invocation,
    );
    assert_eq!(inherited.child_scope(&scope).unwrap(), scope);
    assert_eq!(
        inherited.child_scope(&RuntimeTypeScope::root()),
        Err(RuntimeTypeScopeError::UnknownDepth { depth: 0 })
    );
}
