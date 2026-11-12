use super::*;
use crate::effect_row::{
    EffectConstraintEligibility, EffectConstraintVariable, EffectFormula, EffectRow,
};
use crate::effects::EffectSet;
use crate::types::constraints::{
    TypeConstraintEffectScope, TypeConstraintParameterScope,
    normalization::{ConstraintClosedEffects, close_type_operand},
    transaction::ConstraintPath,
};
use crate::types::{
    DetachedGenericOwnerId, GenericEffectParameterId, GenericEffectReference,
    GenericParameterOwnerId,
};

fn parameters() -> (TypeConstraintParameterScope, [GenericEffectReference; 2]) {
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(793));
    let declared: [GenericEffectReference; 2] =
        [0, 1].map(|slot| GenericEffectParameterId::new(owner.clone(), slot).into());
    let effects = TypeConstraintEffectScope::seal_call_scope(
        declared
            .iter()
            .cloned()
            .map(|key| EffectConstraintVariable::new(key, EffectConstraintEligibility::Bindable)),
        [],
    )
    .unwrap();
    let parameters = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        [],
        [],
        effects,
        [],
        [],
    )
    .unwrap();
    let keys = declared.map(|key| parameters.effect_reference(&key).unwrap());
    (parameters, keys)
}

fn path(
    context: &mut Context<'_>,
) -> (
    ConstraintPath<NoConstraintClient>,
    [GenericEffectReference; 2],
) {
    let (parameters, keys) = parameters();
    let scope = context.fixture_application_scope((), parameters).unwrap();
    (context.start_path(scope).unwrap(), keys)
}

#[test]
fn closed_headers_lift_the_used_completion_row_through_the_actual_local_binder() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let ambient = context.enter_binder_scope(GenericBinder::new(0, 0, 1));
    let ambient_effect = context.lexical_scope().bound_effect(0, 0).unwrap();
    let (mut path, keys) = path(&mut context);
    path.effects
        .constrain_subset(
            &EffectRow::open(EffectSet::new(), ambient_effect),
            &EffectRow::open(EffectSet::new(), keys[0].clone()),
            &mut context,
        )
        .unwrap();
    let local = GenericBinder::new(0, 0, 1);
    let contents = context.lexical_scope().with_binder(local);
    let predicate = EffectFormula::variable(keys[0].clone(), &mut context)
        .unwrap()
        .subset(
            &EffectFormula::variable(contents.bound_effect(0, 0).unwrap(), &mut context).unwrap(),
            &mut context,
        )
        .unwrap();
    let input = context.fixture_operand(TypeKind::function_with_contract(
        local,
        predicate,
        [],
        TypeKind::I64,
        EffectRow::open(EffectSet::new(), keys[0].clone()),
    ));
    let retained_input = Arc::downgrade(&input.value);
    let effects = ConstraintClosedEffects::for_path(&path, &mut context).unwrap();
    let retained_effects = Arc::downgrade(&effects);
    let closed = close_type_operand(input.view(), &path, &effects, &mut context).unwrap();
    let TypeKind::Function {
        effects: row,
        predicate,
        ..
    } = closed.value()
    else {
        panic!("closed function")
    };
    assert_eq!(
        row,
        &EffectRow::open(EffectSet::new(), contents.bound_effect(1, 0).unwrap())
    );
    let expected_predicate =
        EffectFormula::variable(contents.bound_effect(1, 0).unwrap(), &mut context)
            .unwrap()
            .subset(
                &EffectFormula::variable(contents.bound_effect(0, 0).unwrap(), &mut context)
                    .unwrap(),
                &mut context,
            )
            .unwrap();
    assert_eq!(predicate, &expected_predicate);
    let ConstraintOperandOrigin::Rebuilt {
        header: Some(proof),
        contents_scope,
        ..
    } = closed.origin.as_ref()
    else {
        panic!("the actual header retains its completion derivation")
    };
    assert!(Arc::ptr_eq(&proof.owner, &effects));
    assert_eq!(proof.row_keys.as_ref(), &[keys[0].clone()]);
    assert_eq!(proof.predicate_keys.as_ref(), &[keys[0].clone()]);
    assert_eq!(contents_scope.shape(), &contents);
    assert!(effects.completion().substitution.get(&keys[1]).is_some());
    drop(input);
    drop(path);
    drop(effects);
    assert!(retained_input.upgrade().is_some());
    assert!(retained_effects.upgrade().is_some());
    drop(closed);
    assert!(retained_input.upgrade().is_none());
    assert!(retained_effects.upgrade().is_none());
    context.restore_binder_scope(ambient);
}

#[test]
fn a_closed_effect_owner_rejects_changed_premises_and_another_application_inventory() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let (path, keys) = path(&mut context);
    let input = context.fixture_operand(TypeKind::I64);
    let effects = ConstraintClosedEffects::for_path(&path, &mut context).unwrap();
    let mut changed = context.fork_path(&path).unwrap();
    changed
        .effects
        .constrain_subset(
            &EffectRow::open(EffectSet::new(), keys[0].clone()),
            &EffectRow::open(EffectSet::new(), keys[1].clone()),
            &mut context,
        )
        .unwrap();
    assert!(matches!(
        close_type_operand(input.view(), &changed, &effects, &mut context),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::OriginalProducer
        ))
    ));
    let (parameters, _) = parameters();
    let scope = context.fixture_application_scope((), parameters).unwrap();
    let foreign = context.start_path(scope).unwrap();
    assert!(matches!(
        close_type_operand(input.view(), &foreign, &effects, &mut context),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::OriginalProducer
        ))
    ));
    assert_eq!(
        close_type_operand(input.view(), &path, &effects, &mut context)
            .unwrap()
            .value(),
        &TypeKind::I64
    );
}
