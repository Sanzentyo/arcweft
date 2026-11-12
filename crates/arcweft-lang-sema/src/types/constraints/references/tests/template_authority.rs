use std::sync::atomic::AtomicBool;

use crate::{
    effect_row::{EffectConstraintEligibility, EffectConstraintVariable, EffectFormula, EffectRow},
    effects::EffectSet,
    types::{
        ArrayLength, DetachedGenericOwnerId, GenericBinder, GenericConstParameterId,
        GenericConstReference, GenericEffectParameterId, GenericEffectReference,
        GenericParameterOwnerId, GenericScope, GenericTypeParameterId, GenericTypeReference,
        TypeKind,
        constraints::{
            ConstraintGenericParameterId, NoConstraintClient, TypeConstraintConstEligibility,
            TypeConstraintEffectScope, TypeConstraintParameterEligibility,
            TypeConstraintParameterScope,
            application::ConstraintApplicationScope,
            context::{
                LocalConstraintAccounting, TypeConstraintConstParameterScopeRow,
                TypeConstraintContext, TypeConstraintLimits, TypeConstraintTypeParameterScopeRow,
            },
            references,
        },
    },
};

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

fn context(cancellation: &AtomicBool) -> Context<'_> {
    TypeConstraintContext::with_accounting(LocalConstraintAccounting::new(
        TypeConstraintLimits::new(8192, 8192, 128, 128),
        cancellation,
    ))
}

fn scope(
    eligibility: TypeConstraintParameterEligibility,
) -> (
    TypeConstraintParameterScope,
    [ConstraintGenericParameterId; 3],
) {
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(9301));
    let ty: GenericTypeReference = GenericTypeParameterId::new(owner.clone(), 0).into();
    let length: GenericConstReference = GenericConstParameterId::new(owner.clone(), 0).into();
    let effect: GenericEffectReference = GenericEffectParameterId::new(owner, 0).into();
    let (const_eligibility, effect_eligibility) = match eligibility {
        TypeConstraintParameterEligibility::Rigid => (
            TypeConstraintConstEligibility::Rigid,
            EffectConstraintEligibility::Rigid,
        ),
        TypeConstraintParameterEligibility::Bindable => (
            TypeConstraintConstEligibility::Bindable,
            EffectConstraintEligibility::Bindable,
        ),
        TypeConstraintParameterEligibility::FutureEligible => (
            TypeConstraintConstEligibility::FutureEligible,
            EffectConstraintEligibility::FutureEligible,
        ),
    };
    (
        TypeConstraintParameterScope::seal_call_scope(
            GenericBinder::EMPTY,
            [TypeConstraintTypeParameterScopeRow::new(
                ty.clone(),
                eligibility,
            )],
            [TypeConstraintConstParameterScopeRow::new(
                length.clone(),
                const_eligibility,
            )],
            TypeConstraintEffectScope::seal_call_scope(
                [EffectConstraintVariable::new(
                    effect.clone(),
                    effect_eligibility,
                )],
                [],
            )
            .unwrap(),
            [],
            [],
        )
        .unwrap(),
        [ty.into(), length.into(), effect.into()],
    )
}

fn opened(
    scope: &TypeConstraintParameterScope,
    keys: &[ConstraintGenericParameterId; 3],
    eligibility: TypeConstraintParameterEligibility,
) -> [ConstraintGenericParameterId; 3] {
    if eligibility == TypeConstraintParameterEligibility::Rigid {
        return keys.clone();
    }
    let [
        ConstraintGenericParameterId::Type(ty),
        ConstraintGenericParameterId::Const(length),
        ConstraintGenericParameterId::Effect(effect),
    ] = keys
    else {
        unreachable!()
    };
    [
        scope.type_reference(ty).unwrap().into(),
        scope.const_reference(length).unwrap().into(),
        scope.effect_reference(effect).unwrap().into(),
    ]
}

fn values(keys: &[ConstraintGenericParameterId; 3], context: &mut Context<'_>) -> Vec<TypeKind> {
    let [
        ConstraintGenericParameterId::Type(ty),
        ConstraintGenericParameterId::Const(length),
        ConstraintGenericParameterId::Effect(effect),
    ] = keys
    else {
        unreachable!()
    };
    let predicate = EffectFormula::variable(effect.clone(), context)
        .unwrap()
        .subset(&EffectFormula::empty(), context)
        .unwrap();
    vec![
        TypeKind::Tuple(vec![TypeKind::GenericParam(ty.clone())]),
        TypeKind::Array {
            item: Box::new(TypeKind::I64),
            len: ArrayLength::Generic(length.clone()),
        },
        TypeKind::function_with_effects(
            [],
            TypeKind::I64,
            EffectRow::open(EffectSet::new(), effect.clone()),
        ),
        TypeKind::function_with_contract(
            GenericBinder::EMPTY,
            predicate,
            [],
            TypeKind::I64,
            EffectRow::closed(EffectSet::new()),
        ),
    ]
}

#[test]
fn a_template_cannot_borrow_another_applications_rigid_or_inference_inventory() {
    for eligibility in [
        TypeConstraintParameterEligibility::Rigid,
        TypeConstraintParameterEligibility::Bindable,
        TypeConstraintParameterEligibility::FutureEligible,
    ] {
        let cancellation = AtomicBool::new(false);
        let mut context = context(&cancellation);
        let root = context
            .fixture_application_scope((), TypeConstraintParameterScope::empty())
            .unwrap();
        let root_id = root.id();
        let mut path = context.start_path(root).unwrap();
        let (parameters, keys) = scope(eligibility);
        let expected = opened(&parameters, &keys, eligibility);
        let child = context.fixture_application_scope((), parameters).unwrap();
        let child_id = child.id();
        path = context.admit_application(path, child).unwrap();
        for (index, value) in values(&expected, &mut context).into_iter().enumerate() {
            assert_eq!(
                context.open_template_type(&value, &path, child_id).unwrap(),
                value
            );
            let [
                ConstraintGenericParameterId::Type(ty),
                ConstraintGenericParameterId::Const(length),
                ConstraintGenericParameterId::Effect(effect),
            ] = &expected
            else {
                unreachable!()
            };
            let error = match index {
                0 => references::type_out_of_scope(ty),
                1 => references::const_out_of_scope(length),
                2 | 3 => references::effect_out_of_scope(effect),
                _ => unreachable!(),
            };
            assert_eq!(
                context.open_template_type(&value, &path, root_id),
                Err(error)
            );
        }
    }
}

#[test]
fn a_template_can_read_its_original_sources_exact_imported_scope() {
    for eligibility in [
        TypeConstraintParameterEligibility::Rigid,
        TypeConstraintParameterEligibility::Bindable,
        TypeConstraintParameterEligibility::FutureEligible,
    ] {
        let cancellation = AtomicBool::new(false);
        let mut context = context(&cancellation);
        let (parameters, keys) = scope(eligibility);
        let expected = opened(&parameters, &keys, eligibility);
        let root = context.fixture_application_scope((), parameters).unwrap();
        let mut path = context.start_path(root).unwrap();
        let lease = path.applications.import_parameters(&expected).unwrap();
        let original = context.fixture_original_context(Some(&lease)).unwrap();
        let child =
            ConstraintApplicationScope::test_root((), TypeConstraintParameterScope::empty())
                .with_fixture_original(original);
        let child_id = child.id();
        path = context.admit_application(path, child).unwrap();
        for value in values(&expected, &mut context) {
            assert_eq!(
                context.open_template_type(&value, &path, child_id).unwrap(),
                value
            );
        }
    }
}

#[test]
fn template_opening_retains_function_local_binders_and_restores_actual_ambient_frames() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let binder = GenericBinder::new(1, 1, 1);
    let outer = context.enter_binder_scope(binder);
    let ambient = context.lexical_frames().clone();
    let root = context
        .fixture_application_scope((), TypeConstraintParameterScope::empty())
        .unwrap();
    let path = context.start_path(root).unwrap();
    let local = GenericScope::default().with_binder(binder);
    let predicate = EffectFormula::variable(local.bound_effect(0, 0).unwrap(), &mut context)
        .unwrap()
        .subset(&EffectFormula::empty(), &mut context)
        .unwrap();
    let value = TypeKind::function_with_contract(
        binder,
        predicate,
        [TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(local.bound_type(0, 0).unwrap())),
            len: ArrayLength::Generic(local.bound_const(0, 0).unwrap()),
        }],
        TypeKind::I64,
        EffectRow::open(EffectSet::new(), local.bound_effect(0, 0).unwrap()),
    );
    assert_eq!(
        context
            .open_template_type(&value, &path, path.root_id())
            .unwrap(),
        value
    );
    assert_eq!(context.lexical_frames(), &ambient);
    let (_, keys) = scope(TypeConstraintParameterEligibility::Rigid);
    for value in values(&keys, &mut context) {
        assert!(
            context
                .open_template_type(&value, &path, path.root_id())
                .is_err()
        );
        assert_eq!(context.lexical_frames(), &ambient);
    }
    context.restore_binder_scope(outer);
}
