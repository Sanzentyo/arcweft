use super::*;
use crate::{
    effect_row::{EffectPredicate, EffectRow},
    effects::EffectSet,
    types::{
        DetachedGenericOwnerId, GenericBinder, GenericConstParameterId, GenericDeclarationBinder,
        GenericEffectParameterId, GenericEffectReference, GenericParameterOwnerId, GenericScope,
        GenericScopeError, GenericTypeParameterId,
        constraints::{
            NoConstraintClient, OriginalProducerSource, OriginalRigidScope,
            context::{LocalConstraintAccounting, TypeConstraintLimits},
        },
    },
};
use std::{any::Any, borrow::Cow, sync::atomic::AtomicBool};

#[derive(Debug)]
struct Source {
    identity: Arc<()>,
    types: [GenericTypeReference; 1],
    consts: [GenericConstReference; 1],
    effects: [GenericEffectReference; 1],
}

impl Source {
    fn new(ordinal: u64) -> Arc<Self> {
        let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(ordinal));
        Arc::new(Self {
            identity: Arc::new(()),
            types: [GenericTypeParameterId::new(owner.clone(), 0).into()],
            consts: [GenericConstParameterId::new(owner.clone(), 0).into()],
            effects: [GenericEffectParameterId::new(owner, 0).into()],
        })
    }
}

impl OriginalProducerSource for Source {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn same_source(&self, other: &dyn OriginalProducerSource) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| Arc::ptr_eq(&self.identity, &other.identity))
    }
    fn rigid_scope(&self) -> OriginalRigidScope<'_> {
        OriginalRigidScope {
            types: &self.types,
            consts: &self.consts,
            effects: &self.effects,
            imported: None,
        }
    }
    fn declaration_formals(&self) -> Result<GenericDeclarationBinder, GenericScopeError> {
        GenericDeclarationBinder::new(
            GenericScope::default(),
            Box::new([]),
            Box::new([]),
            Box::new([]),
        )
    }
    fn domain(&self) -> Cow<'_, EffectPredicate> {
        Cow::Owned(EffectPredicate::unconstrained())
    }
}

fn context(
    cancellation: &AtomicBool,
) -> TypeConstraintContext<'_, LocalConstraintAccounting<'_>, NoConstraintClient> {
    TypeConstraintContext::with_accounting(LocalConstraintAccounting::new(
        TypeConstraintLimits::new(4096, 4096, 128, 128),
        cancellation,
    ))
}

#[test]
fn original_values_admit_only_their_actual_sources_free_type_const_and_effect() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let source = Source::new(9001);
    let foreign = Source::new(9002);
    let original = context
        .fixture_original_context_with_rigid(source.rigid_scope())
        .unwrap();
    for owner in [&source, &foreign] {
        let values = [
            TypeKind::Tuple(vec![TypeKind::GenericParam(owner.types[0].clone())]),
            TypeKind::Array {
                item: Box::new(TypeKind::I64),
                len: ArrayLength::Generic(owner.consts[0].clone()),
            },
            TypeKind::function_with_effects(
                [],
                TypeKind::I64,
                EffectRow::open(EffectSet::new(), owner.effects[0].clone()),
            ),
        ];
        for (index, value) in values.into_iter().enumerate() {
            let admitted = context.fixture_operand_in(&original, &value);
            if Arc::ptr_eq(owner, &source) {
                assert_eq!(admitted.unwrap().value(), &value);
            } else {
                let expected = match index {
                    0 => references::type_out_of_scope(&foreign.types[0]),
                    1 => references::const_out_of_scope(&foreign.consts[0]),
                    2 => references::effect_out_of_scope(&foreign.effects[0]),
                    _ => unreachable!(),
                };
                assert_eq!(admitted.err(), Some(expected));
            }
        }
        let length = ArrayLength::Generic(owner.consts[0].clone());
        let admitted = context.fixture_operand_in(&original, &length);
        if Arc::ptr_eq(owner, &source) {
            assert_eq!(admitted.unwrap().value(), &length);
        } else {
            assert_eq!(
                admitted.err(),
                Some(references::const_out_of_scope(&foreign.consts[0]))
            );
        }
    }
}

#[test]
fn a_nested_function_keeps_local_quantifiers_and_checks_free_predicate_variables() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let source = Source::new(9003);
    let foreign = Source::new(9004);
    let original = context
        .fixture_original_context_with_rigid(source.rigid_scope())
        .unwrap();
    let binder = GenericBinder::new(1, 1, 1);
    let nested = original.scope().with_binder(binder);
    for owner in [&source, &foreign] {
        let predicate =
            crate::effect_row::EffectFormula::variable(owner.effects[0].clone(), &mut context)
                .unwrap()
                .subset(&crate::effect_row::EffectFormula::empty(), &mut context)
                .unwrap();
        let value = TypeKind::function_with_contract(
            binder,
            predicate,
            [TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(nested.bound_type(0, 0).unwrap())),
                len: ArrayLength::Generic(nested.bound_const(0, 0).unwrap()),
            }],
            TypeKind::GenericParam(source.types[0].clone()),
            EffectRow::open(EffectSet::new(), nested.bound_effect(0, 0).unwrap()),
        );
        let admitted = context.fixture_operand_in(&original, &value);
        if Arc::ptr_eq(owner, &source) {
            assert_eq!(admitted.unwrap().value(), &value);
        } else {
            assert_eq!(
                admitted.err(),
                Some(references::effect_out_of_scope(&foreign.effects[0]))
            );
        }
    }
}

#[test]
fn a_candidates_rigid_inventory_cannot_admit_a_foreign_original_value() {
    use crate::types::constraints::{
        TypeConstraintEffectScope, TypeConstraintParameterEligibility,
        TypeConstraintParameterScope, context::TypeConstraintTypeParameterScopeRow,
        transaction::TypeConstraintTransaction,
    };
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let source = Source::new(9005);
    let candidate = Source::new(9006);
    let original = context.original_producer_context(&source).unwrap();
    let scope = Arc::new(context.capture_lexical_scope().unwrap());
    let parameters = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        [TypeConstraintTypeParameterScopeRow::new(
            candidate.types[0].clone(),
            TypeConstraintParameterEligibility::Rigid,
        )],
        [],
        TypeConstraintEffectScope::seal_call_scope([], []).unwrap(),
        [],
        [],
    )
    .unwrap();
    let transaction =
        TypeConstraintTransaction::initialize(&mut context, original.clone(), (), parameters, None)
            .unwrap();
    assert_eq!(
        context.parameter_eligibility(
            &candidate.types[0],
            transaction.frontier[0].projection_view()
        ),
        Some(TypeConstraintParameterEligibility::Rigid)
    );
    let value = TypeKind::GenericParam(candidate.types[0].clone());
    assert_eq!(
        ConstraintOperand::<NoConstraintClient>::from_original_in(
            &value,
            scope,
            original,
            &mut context
        )
        .err(),
        Some(references::type_out_of_scope(&candidate.types[0]))
    );
}

#[test]
fn original_value_admission_stops_before_copying_when_its_budget_is_exhausted() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let original = context
        .original_producer_context(&Source::new(9007))
        .unwrap();
    let scope = Arc::new(context.capture_lexical_scope().unwrap());
    let mut exhausted = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
        LocalConstraintAccounting::new(TypeConstraintLimits::new(0, 0, 0, 0), &cancellation),
    );
    assert!(matches!(
        ConstraintOperand::<NoConstraintClient>::from_original_in(
            &TypeKind::Tuple(vec![TypeKind::I64]),
            scope,
            original,
            &mut exhausted
        ),
        Err(TypeConstraintError::Abort(_))
    ));
}

#[test]
fn fixture_gamma_rejection_is_distinct_from_lower_effect_environment_rejection() {
    use crate::effect_row::{
        EffectConstraintEligibility, EffectConstraintEnvironment, EffectConstraintEnvironmentError,
        EffectConstraintVariable,
    };
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let declarations = Source::new(9010);
    let row = EffectRow::open(EffectSet::new(), declarations.effects[0].clone());
    let value = TypeKind::function_with_effects([], TypeKind::I64, row.clone());
    let ground = context.fixture_original_context(None).unwrap();
    let candidate = EffectConstraintEnvironment::new(&[EffectConstraintVariable::new(
        declarations.effects[0].clone(),
        EffectConstraintEligibility::Rigid,
    )])
    .unwrap();
    candidate.validate_row(&row).unwrap();
    assert_eq!(
        context.fixture_operand_in(&ground, &value).err(),
        Some(references::effect_out_of_scope(&declarations.effects[0]))
    );

    let original = context
        .fixture_original_context_with_rigid(declarations.rigid_scope())
        .unwrap();
    let admitted = context.fixture_operand_in(&original, &value).unwrap();
    let TypeKind::Function { effects, .. } = admitted.value() else {
        panic!("function input");
    };
    let lower = EffectConstraintEnvironment::new(&[]).unwrap();
    assert_eq!(
        lower.validate_row(effects),
        Err(EffectConstraintEnvironmentError::ForeignVariable {
            variable: declarations.effects[0].clone(),
        })
    );
}

#[test]
fn fixture_operands_retain_the_declared_original_frames_instead_of_current_same_shape_frames() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let binder = GenericBinder::new(1, 0, 0);
    let previous = context.enter_binder_scope(binder);
    let declarations = Source::new(9011);
    let original = context
        .fixture_original_context_with_rigid(declarations.rigid_scope())
        .unwrap();
    let reference = original.scope().bound_type(0, 0).unwrap();
    context.restore_binder_scope(previous);
    let other = context.enter_binder_scope(binder);
    let current = context.capture_lexical_scope().unwrap();
    assert_eq!(original.scope(), current.shape());
    assert_ne!(original.gamma(), &current);
    let value = context
        .fixture_operand_in(&original, &TypeKind::GenericParam(reference))
        .unwrap();
    assert_eq!(value.scope(), original.gamma());
    assert_ne!(value.scope(), &current);
    let mut exhausted = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
        LocalConstraintAccounting::new(TypeConstraintLimits::new(0, 0, 0, 0), &cancellation),
    );
    assert!(matches!(
        exhausted.fixture_operand_in(&original, &TypeKind::I64),
        Err(TypeConstraintError::Abort(_))
    ));
    context.restore_binder_scope(other);
}

#[test]
fn original_admission_rejects_equally_shaped_foreign_ambient_frames() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let binder = GenericBinder::new(1, 1, 1);
    let previous = context.enter_binder_scope(binder);
    let original = context
        .original_producer_context(&Source::new(9013))
        .unwrap();
    context.restore_binder_scope(previous);
    let foreign = context.enter_binder_scope(binder);
    let target = Arc::new(context.capture_lexical_scope().unwrap());
    let reference = original.scope().bound_type(0, 0).unwrap();
    let length = original.scope().bound_const(0, 0).unwrap();
    let effect = original.scope().bound_effect(0, 0).unwrap();
    let cases = [
        (TypeKind::GenericParam(reference.clone()), reference.into()),
        (
            TypeKind::Array {
                item: Box::new(TypeKind::I64),
                len: ArrayLength::Generic(length.clone()),
            },
            length.clone().into(),
        ),
        (
            TypeKind::function_with_effects(
                [],
                TypeKind::I64,
                EffectRow::open(EffectSet::new(), effect.clone()),
            ),
            effect.into(),
        ),
    ];
    for (value, parameter) in cases {
        let admitted = ConstraintOperand::<NoConstraintClient>::from_original_in(
            &value,
            Arc::clone(&target),
            original.clone(),
            &mut context,
        );
        assert_eq!(
            admitted.err(),
            Some(TypeConstraintRejection::EscapingInstantiation { parameter }.into())
        );
    }
    let admitted = ConstraintOperand::<NoConstraintClient, ArrayLength>::from_original_in(
        &ArrayLength::Generic(length.clone()),
        target,
        original,
        &mut context,
    );
    assert_eq!(
        admitted.err(),
        Some(
            TypeConstraintRejection::EscapingInstantiation {
                parameter: length.into()
            }
            .into()
        )
    );
    context.restore_binder_scope(foreign);
}

#[test]
fn original_admission_lifts_ambient_type_const_effect_and_predicate_but_keeps_local_binders() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let previous = context.enter_binder_scope(GenericBinder::new(1, 1, 1));
    let original = context
        .original_producer_context(&Source::new(9014))
        .unwrap();
    let target = Arc::new(original.gamma().with_binder(GenericBinder::new(1, 1, 1)));
    let local = GenericBinder::new(1, 1, 1);
    let make = |outer: &GenericScope,
                context: &mut TypeConstraintContext<
        '_,
        LocalConstraintAccounting<'_>,
        NoConstraintClient,
    >| {
        let contents = outer.with_binder(local);
        let depth = u32::try_from(outer.binders().len()).unwrap();
        let predicate = crate::effect_row::EffectFormula::variable(
            contents.bound_effect(depth, 0).unwrap(),
            context,
        )
        .unwrap()
        .subset(
            &crate::effect_row::EffectFormula::variable(
                contents.bound_effect(0, 0).unwrap(),
                context,
            )
            .unwrap(),
            context,
        )
        .unwrap();
        TypeKind::function_with_contract(
            local,
            predicate,
            [TypeKind::Array {
                item: Box::new(TypeKind::Tuple(vec![
                    TypeKind::GenericParam(contents.bound_type(depth, 0).unwrap()),
                    TypeKind::GenericParam(contents.bound_type(0, 0).unwrap()),
                ])),
                len: ArrayLength::Generic(contents.bound_const(depth, 0).unwrap()),
            }],
            TypeKind::Array {
                item: Box::new(TypeKind::I64),
                len: ArrayLength::Generic(contents.bound_const(0, 0).unwrap()),
            },
            EffectRow::open(EffectSet::new(), contents.bound_effect(depth, 0).unwrap()),
        )
    };
    let value = make(original.scope(), &mut context);
    let expected = make(target.shape(), &mut context);
    let admitted = ConstraintOperand::<NoConstraintClient>::from_original_in(
        &value,
        Arc::clone(&target),
        original.clone(),
        &mut context,
    )
    .unwrap();
    assert_eq!(admitted.value(), &expected);
    assert_eq!(admitted.scope(), target.as_ref());
    assert_eq!(admitted.original_context(), &original);
    context.restore_binder_scope(previous);
}

fn inference_parameters(
    source: &Source,
) -> crate::types::constraints::TypeConstraintParameterScope {
    use crate::{
        effect_row::{EffectConstraintEligibility, EffectConstraintVariable},
        types::constraints::{
            TypeConstraintConstEligibility, TypeConstraintEffectScope,
            TypeConstraintParameterEligibility, TypeConstraintParameterScope,
            context::{TypeConstraintConstParameterScopeRow, TypeConstraintTypeParameterScopeRow},
        },
    };
    TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        [TypeConstraintTypeParameterScopeRow::new(
            source.types[0].clone(),
            TypeConstraintParameterEligibility::Bindable,
        )],
        [TypeConstraintConstParameterScopeRow::new(
            source.consts[0].clone(),
            TypeConstraintConstEligibility::Bindable,
        )],
        TypeConstraintEffectScope::seal_call_scope(
            [EffectConstraintVariable::new(
                source.effects[0].clone(),
                EffectConstraintEligibility::Bindable,
            )],
            [],
        )
        .unwrap(),
        [],
        [],
    )
    .unwrap()
}

#[test]
fn original_inference_references_require_the_sources_actual_import_lease() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let source = Source::new(9015);
    let original = context.original_producer_context(&source).unwrap();
    let parameters = inference_parameters(&source);
    let ty = parameters.type_reference(&source.types[0]).unwrap();
    let length = parameters.const_reference(&source.consts[0]).unwrap();
    let effect = parameters.effect_reference(&source.effects[0]).unwrap();
    let application = context
        .application_scope((), parameters, original.clone())
        .unwrap();
    let path = context
        .start_path_with_imported(application, None, original.clone())
        .unwrap();
    let predicate = crate::effect_row::EffectFormula::variable(effect.clone(), &mut context)
        .unwrap()
        .subset(&crate::effect_row::EffectFormula::empty(), &mut context)
        .unwrap();
    let cases = [
        (
            TypeKind::GenericParam(ty.clone()),
            references::type_out_of_scope(&ty),
        ),
        (
            TypeKind::Array {
                item: Box::new(TypeKind::I64),
                len: ArrayLength::Generic(length.clone()),
            },
            references::const_out_of_scope(&length),
        ),
        (
            TypeKind::function_with_effects(
                [],
                TypeKind::I64,
                EffectRow::open(EffectSet::new(), effect.clone()),
            ),
            references::effect_out_of_scope(&effect),
        ),
        (
            TypeKind::function_with_contract(
                GenericBinder::EMPTY,
                predicate,
                [],
                TypeKind::I64,
                EffectRow::closed(EffectSet::new()),
            ),
            references::effect_out_of_scope(&effect),
        ),
    ];
    let mut keys = vec![ty.into(), length.clone().into(), effect.into()];
    keys.sort();
    let lease = path.applications.import_parameters(&keys).unwrap();
    let imported = context.fixture_original_context(Some(&lease)).unwrap();
    for (value, rejected) in cases {
        assert_eq!(
            context.fixture_operand_in(&original, &value).err(),
            Some(rejected)
        );
        let admitted = context.fixture_operand_in(&imported, &value).unwrap();
        assert_eq!(admitted.value(), &value);
    }
    let value = ArrayLength::Generic(length.clone());
    assert_eq!(
        context.fixture_operand_in(&original, &value).err(),
        Some(references::const_out_of_scope(&length))
    );
    assert_eq!(
        context
            .fixture_operand_in(&imported, &value)
            .unwrap()
            .value(),
        &value
    );
}

#[test]
fn introduced_parameter_operands_keep_the_exact_lease_across_child_imports() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let source = Source::new(9016);
    let original = context.original_producer_context(&source).unwrap();
    let parameters = inference_parameters(&source);
    let ty = parameters.type_reference(&source.types[0]).unwrap();
    let length = parameters.const_reference(&source.consts[0]).unwrap();
    let application = context
        .application_scope((), parameters, original.clone())
        .unwrap();
    let path = context
        .start_path_with_imported(application, None, original.clone())
        .unwrap();
    let imported = path
        .applications
        .import_parameters(&[ty.clone().into()])
        .unwrap();
    let child_original = context.fixture_original_context(Some(&imported)).unwrap();
    let child_owner = context
        .application_scope(
            (),
            crate::types::constraints::TypeConstraintParameterScope::empty(),
            child_original.clone(),
        )
        .unwrap();
    let child = context
        .start_path_with_imported(child_owner, Some(imported), child_original)
        .unwrap();
    let target = Arc::new(context.capture_lexical_scope().unwrap());
    let owner = child
        .applications
        .owner_for_parameter(&ty.clone().into())
        .unwrap();
    assert!(
        child
            .applications
            .owner_for_parameter(&length.clone().into())
            .is_none()
    );
    assert_eq!(
        ConstraintOperand::<NoConstraintClient, ArrayLength>::introduced_const(
            &length,
            Arc::clone(&target),
            owner.clone(),
            &mut context
        )
        .err(),
        Some(references::const_out_of_scope(&length))
    );
    let operand = ConstraintOperand::<NoConstraintClient>::introduced_type(
        &ty,
        Arc::clone(&target),
        owner,
        &mut context,
    )
    .unwrap();
    drop(child);
    drop(path);
    assert_eq!(operand.value(), &TypeKind::GenericParam(ty));
    assert_eq!(operand.scope(), target.as_ref());
    assert_eq!(operand.original_context(), &original);
    assert!(matches!(
        operand.origin.as_ref(),
        ConstraintOperandOrigin::Parameter(_)
    ));
}

#[test]
fn introduced_parameter_admission_observes_budget_and_cancellation() {
    use crate::types::constraints::TypeConstraintAbort;
    use std::sync::atomic::Ordering;

    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let source = Source::new(9017);
    let parameters = inference_parameters(&source);
    let ty = parameters.type_reference(&source.types[0]).unwrap();
    let length = parameters.const_reference(&source.consts[0]).unwrap();
    let application = context.fixture_application_scope((), parameters).unwrap();
    let path = context.start_path(application).unwrap();
    let target = Arc::new(context.capture_lexical_scope().unwrap());
    for cancelled in [false, true] {
        let mut stopped = TypeConstraintContext::with_accounting(LocalConstraintAccounting::new(
            TypeConstraintLimits::new(0, 0, 0, 0),
            &cancellation,
        ));
        cancellation.store(cancelled, Ordering::Release);
        let ty_result = ConstraintOperand::<NoConstraintClient>::introduced_type(
            &ty,
            Arc::clone(&target),
            path.applications
                .owner_for_parameter(&ty.clone().into())
                .unwrap(),
            &mut stopped,
        );
        let const_result = ConstraintOperand::<NoConstraintClient, ArrayLength>::introduced_const(
            &length,
            Arc::clone(&target),
            path.applications
                .owner_for_parameter(&length.clone().into())
                .unwrap(),
            &mut stopped,
        );
        for result in [ty_result.err(), const_result.err()] {
            assert!(matches!(result, Some(TypeConstraintError::Abort(_))));
            if cancelled {
                assert_eq!(
                    result,
                    Some(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
                );
            }
        }
    }
}

#[test]
fn result_equation_hints_read_the_checked_operands_receiving_frames() {
    use crate::types::constraints::{ConstraintAcceptance, transaction::TypeConstraintTransaction};

    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let outer = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
    let source = Source::new(9018);
    let original = context.original_producer_context(&source).unwrap();
    let parameters = inference_parameters(&source);
    let parameter = parameters.type_reference(&source.types[0]).unwrap();
    let actual = TypeKind::GenericParam(original.scope().bound_type(0, 0).unwrap());
    let mut transaction =
        TypeConstraintTransaction::initialize(&mut context, original, (), parameters, None)
            .unwrap();
    let nested = context.enter_binder_scope(GenericBinder::new(0, 1, 0));
    let expected = TypeKind::GenericParam(context.lexical_scope().bound_type(1, 0).unwrap());
    transaction.constrain_result(
        &mut context,
        &TypeKind::GenericParam(parameter),
        &actual,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    assert!(
        transaction.first_failure.is_none(),
        "{:?}",
        transaction.first_failure
    );
    let equation = transaction.frontier[0].equations.last().unwrap();
    assert_eq!(equation.actual.value(), &expected);
    assert_eq!(equation.actual.scope().shape(), context.lexical_scope());
    context.restore_binder_scope(nested);
    context.restore_binder_scope(outer);
}
