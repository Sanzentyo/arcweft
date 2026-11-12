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

#[test]
fn closing_an_observed_value_does_not_replace_its_association_with_an_alias_result() {
    use crate::types::constraints::transaction::{
        ConstraintResultProjection, TypeConstraintTransaction,
    };
    use crate::types::constraints::{
        ConstraintAcceptance, PreparedConstraintSourceProjection, PreparedSourceConstraint,
        TypeConstraintProjectionClosure,
    };
    let cancellation = AtomicBool::new(false);
    let (mut context, path, parameters, _) = super::bindings::setup(&cancellation, 1, 0);
    let application = path.root_id();
    let mut transaction =
        TypeConstraintTransaction::from_path(&mut context, application, path, None).unwrap();
    transaction.request_projection(
        &mut context,
        (),
        &TypeKind::I64,
        TypeConstraintProjectionClosure::Closed,
    );
    transaction
        .begin_prepared_probe(
            &mut context,
            PreparedSourceConstraint::unchecked((), PreparedConstraintSourceProjection::Scalar),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .unwrap();
    let mut ticket = transaction.next_probe(&mut context).unwrap().unwrap();
    let receipt = ticket.receipt();
    let mut path = ticket.fork_for_child(&mut context).unwrap().path;
    let result = ConstraintResultProjection {
        application,
        key: Arc::new(()),
    };
    let (value, result) = ResolvedConstraintResult::resolve(result, &path, &mut context).unwrap();
    let scope = Arc::new(context.capture_lexical_scope().unwrap());
    path.bindings.insert(
        parameters[0].clone(),
        ConstraintOperand::result(value, Arc::clone(&scope), result),
    );
    let actual = ConstraintOperand::source(
        TypeKind::GenericParam(parameters[0].clone()),
        scope,
        path.lineage.original.clone(),
        receipt,
        ResolvedConstraintSource::Fixture,
    );
    let effects = ConstraintClosedEffects::for_path(&path, &mut context).unwrap();
    let closed = close_type_operand(actual.view(), &path, &effects, &mut context).unwrap();
    assert_eq!(closed.value(), &TypeKind::I64);
    let ConstraintOperandOrigin::TypeAlias { terminal, .. } = closed.origin.as_ref() else {
        panic!("the result was reached through an alias")
    };
    assert!(terminal.origin.associated_result().is_some());
    assert!(
        closed.associated_result().is_none(),
        "an observed value remains a value even when its inferred type follows another result"
    );
}
