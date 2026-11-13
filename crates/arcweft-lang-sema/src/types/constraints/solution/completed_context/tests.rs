use super::*;
use crate::{
    effect_row::{EffectConstraintVariable, EffectFormula, EffectPredicate},
    effects::EffectSet,
    types::{
        DetachedGenericOwnerId, GenericBinder, GenericConstParameterId, GenericEffectParameterId,
        GenericParameterOwnerId, GenericTypeParameterId,
        constraints::{
            NoConstraintClient, TypeConstraintConstEligibility, TypeConstraintEffectScope,
            TypeConstraintParameterScope,
            context::{
                LocalConstraintAccounting, TypeConstraintConstParameterScopeRow as ConstRow,
                TypeConstraintLimits, TypeConstraintTypeParameterScopeRow as TypeRow,
            },
            references,
        },
    },
};
use std::sync::atomic::AtomicBool;

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

struct References {
    ty: GenericTypeReference,
    length: GenericConstReference,
    effect: GenericEffectReference,
}

fn fixture(
    cancellation: &AtomicBool,
) -> (
    Context<'_>,
    ConstraintPath<NoConstraintClient>,
    [References; 2],
) {
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(90_104));
    let references = [0, 1].map(|slot| References {
        ty: GenericTypeParameterId::new(owner.clone(), slot).into(),
        length: GenericConstParameterId::new(owner.clone(), slot).into(),
        effect: GenericEffectParameterId::new(owner.clone(), u32::from(slot)).into(),
    });
    let parameters = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        references
            .iter()
            .map(|r| TypeRow::new(r.ty.clone(), TypeConstraintParameterEligibility::Rigid)),
        references
            .iter()
            .map(|r| ConstRow::new(r.length.clone(), TypeConstraintConstEligibility::Rigid)),
        TypeConstraintEffectScope::seal_call_scope(
            references.iter().map(|r| {
                EffectConstraintVariable::new(r.effect.clone(), EffectConstraintEligibility::Rigid)
            }),
            [],
        )
        .unwrap(),
        [],
        [],
    )
    .unwrap();
    let (context, path) = crate::types::constraints::test_support::ConstraintTestSetup::with_scope(
        TypeConstraintLimits::new(65_536, 32_768, 1024, 1024),
        cancellation,
        parameters,
    )
    .into_path();
    (context, path, references)
}

#[test]
fn completed_type_and_const_values_cannot_reopen_with_a_siblings_capture() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path, references) = fixture(&cancellation);
    let application = path.root_scope();
    let generic =
        CompletedGenericContext::for_application(&path, application, &mut context).unwrap();
    let types = references.each_ref().map(|r| {
        generic
            .reify_type(
                &context.fixture_operand(TypeKind::GenericParam(r.ty.clone())),
                &path,
                application,
                None,
                &mut context,
            )
            .unwrap()
    });
    for value in &types {
        assert_eq!(
            generic
                .reopen_type(value, &path, application, &mut context)
                .unwrap(),
            *value.value()
        );
    }
    // Fault injection is confined to this private owner's tests. Both
    // declarations remain visible in the receiving application throughout.
    let wrong_type = CompletedValue {
        value: TypeKind::GenericParam(references[1].ty.clone()),
        captures: types[0].captures.clone(),
    };
    assert_eq!(
        generic.reopen_type(&wrong_type, &path, application, &mut context),
        Err(references::type_out_of_scope(&references[1].ty))
    );

    let lengths = references.each_ref().map(|r| {
        generic
            .reify_length(
                &ArrayLength::Generic(r.length.clone()),
                &path,
                application,
                &mut context,
            )
            .unwrap()
    });
    for value in &lengths {
        assert_eq!(
            generic
                .reopen_length(value, &path, application, &mut context)
                .unwrap(),
            *value.value()
        );
    }
    let wrong_length = CompletedValue {
        value: ArrayLength::Generic(references[1].length.clone()),
        captures: lengths[0].captures.clone(),
    };
    assert_eq!(
        generic.reopen_length(&wrong_length, &path, application, &mut context),
        Err(references::const_out_of_scope(&references[1].length))
    );
}

#[test]
fn reification_uses_the_operands_actual_frames_and_restores_the_receiver() {
    let cancellation = AtomicBool::new(false);
    let (mut context, parameters) = crate::types::constraints::test_support::ConstraintTestSetup::<
        LocalConstraintAccounting<'_>,
        NoConstraintClient,
    >::new(
        TypeConstraintLimits::new(65_536, 32_768, 1024, 1024),
        &cancellation,
    )
    .into_parts();
    let outer = context.enter_binder_scope(GenericBinder::new(1, 0, 1));
    let application = context.fixture_application_scope((), parameters).unwrap();
    let path = context.start_path(application).unwrap();
    let application = path.root_scope();
    let generic =
        CompletedGenericContext::for_application(&path, application, &mut context).unwrap();
    let ambient = context.lexical_scope().bound_type(0, 0).unwrap();
    let own = GenericBinder::new(1, 0, 1);
    let nested = context.lexical_scope().with_binder(own);
    let original = TypeKind::Tuple(vec![
        TypeKind::GenericParam(ambient),
        TypeKind::function_with_binder(
            own,
            [TypeKind::GenericParam(nested.bound_type(0, 0).unwrap())],
            TypeKind::GenericParam(nested.bound_type(1, 0).unwrap()),
            EffectRow::open(EffectSet::new(), nested.bound_effect(0, 0).unwrap())
                .union(
                    &EffectRow::open(EffectSet::new(), nested.bound_effect(1, 0).unwrap()),
                    &mut context,
                )
                .unwrap(),
        ),
    ]);
    let operand = context.fixture_operand(original.clone());
    let enclosing = context.enter_binder_scope(GenericBinder::new(1, 0, 1));
    let receiving = context.capture_lexical_scope().unwrap();
    let completed = generic
        .reify_type(&operand, &path, application, None, &mut context)
        .unwrap();
    assert_eq!(completed.value(), &original);
    assert!(completed.captures().parameters().is_empty());
    assert_eq!(context.capture_lexical_scope().unwrap(), receiving);
    cancellation.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(
        generic
            .reify_type(&operand, &path, application, None, &mut context)
            .is_err()
    );
    cancellation.store(false, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(context.capture_lexical_scope().unwrap(), receiving);
    context.restore_binder_scope(enclosing);
    assert_eq!(
        generic
            .reopen_type(&completed, &path, application, &mut context)
            .unwrap(),
        original
    );
    context.restore_binder_scope(outer);
}

fn predicate(reference: &GenericEffectReference, context: &mut Context<'_>) -> EffectPredicate {
    EffectFormula::literal(EffectSet::new(), Some(reference.clone()))
        .subset(&EffectFormula::literal(EffectSet::new(), None), context)
        .unwrap()
}

#[test]
fn completed_effect_rows_and_predicates_cannot_reopen_with_a_siblings_capture() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path, references) = fixture(&cancellation);
    let application = path.root_scope();
    let generic =
        CompletedGenericContext::for_application(&path, application, &mut context).unwrap();
    let rows = references.each_ref().map(|r| {
        generic
            .reify_effect_row(
                &EffectRow::open(EffectSet::new(), r.effect.clone()),
                &path,
                application,
                &mut context,
            )
            .unwrap()
    });
    for value in &rows {
        assert_eq!(
            generic
                .reopen_effect_row(value, &path, application, &mut context)
                .unwrap(),
            *value.value()
        );
    }
    let wrong_row = CompletedValue {
        value: EffectRow::open(EffectSet::new(), references[1].effect.clone()),
        captures: rows[0].captures.clone(),
    };
    assert_eq!(
        generic.reopen_effect_row(&wrong_row, &path, application, &mut context),
        Err(references::effect_out_of_scope(&references[1].effect))
    );
    let predicates = references.each_ref().map(|r| {
        let predicate = predicate(&r.effect, &mut context);
        generic
            .reify_effect_predicate(&predicate, &path, application, &mut context)
            .unwrap()
    });
    for value in &predicates {
        assert_eq!(
            generic
                .reopen_effect_predicate(value, &path, application, &mut context)
                .unwrap(),
            *value.value()
        );
    }
    let wrong_predicate = CompletedValue {
        value: predicate(&references[1].effect, &mut context),
        captures: predicates[0].captures.clone(),
    };
    assert_eq!(
        generic.reopen_effect_predicate(&wrong_predicate, &path, application, &mut context),
        Err(references::effect_out_of_scope(&references[1].effect))
    );
}

#[test]
fn inserted_result_predicate_seals_its_own_free_references() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path, references) = fixture(&cancellation);
    let application = path.root_scope();
    let generic =
        CompletedGenericContext::for_application(&path, application, &mut context).unwrap();
    let predicate = predicate(&references[0].effect, &mut context);
    let incoming = generic
        .reify_effect_predicate(&predicate, &path, application, &mut context)
        .unwrap();
    let value = generic
        .reify_type(
            &context.fixture_operand(TypeKind::function_with_effects(
                [],
                TypeKind::Unit,
                EffectRow::closed(EffectSet::new()),
            )),
            &path,
            application,
            Some(&incoming),
            &mut context,
        )
        .unwrap();
    assert_eq!(
        value.captures().parameters(),
        &[references[0].effect.clone().into()]
    );
    let reopened = generic
        .reopen_type(&value, &path, application, &mut context)
        .unwrap();
    let TypeKind::Function {
        predicate: actual, ..
    } = reopened
    else {
        panic!("function")
    };
    assert_eq!(actual, predicate);
}

#[test]
fn canonical_result_predicate_prunes_only_the_root_header_captures() {
    let cancellation = AtomicBool::new(false);
    let (mut context, path, references) = fixture(&cancellation);
    let application = path.root_scope();
    let generic =
        CompletedGenericContext::for_application(&path, application, &mut context).unwrap();
    let incoming = generic
        .reify_effect_predicate(
            &EffectPredicate::impossible(),
            &path,
            application,
            &mut context,
        )
        .unwrap();
    for child_capture in [false, true] {
        let predicate = predicate(&references[0].effect, &mut context);
        let children = child_capture.then(|| {
            TypeKind::function_with_effects(
                [],
                TypeKind::Unit,
                EffectRow::open(EffectSet::new(), references[0].effect.clone()),
            )
        });
        let original = TypeKind::function_with_contract(
            GenericBinder::EMPTY,
            predicate,
            children,
            TypeKind::Unit,
            EffectRow::closed(EffectSet::new()),
        );
        let value = generic
            .reify_type(
                &context.fixture_operand(original),
                &path,
                application,
                Some(&incoming),
                &mut context,
            )
            .unwrap();
        assert_eq!(
            value
                .captures()
                .contains(&references[0].effect.clone().into()),
            child_capture
        );
        let reopened = generic
            .reopen_type(&value, &path, application, &mut context)
            .unwrap();
        let TypeKind::Function { predicate, .. } = reopened else {
            panic!("function")
        };
        assert_eq!(predicate, EffectPredicate::impossible());
    }
}
