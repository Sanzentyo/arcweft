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
        ConstraintOperand::<NoConstraintClient>::original(&value, scope, original, &mut context)
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
        ConstraintOperand::<NoConstraintClient>::original(
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
