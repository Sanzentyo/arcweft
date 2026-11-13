//! A checked value's Γ is independent of the candidate's visible references.

use super::*;
use crate::{
    callable::EnclosingGenericParameterScope,
    effect_row::EffectFormula,
    types::{
        GenericBinder, GenericConstParameterId, GenericConstReference, GenericEffectParameterId,
        GenericEffectReference, GenericScope, GenericTypeReference,
        constraints::{
            ConstraintGenericParameterId, ConstraintSourceValueOwner,
            TypeConstraintConstEligibility,
            context::{TypeConstraintConstParameterScopeRow, TypeConstraintTypeParameterScopeRow},
            references,
        },
    },
};

type Context<'a> =
    TypeConstraintContext<'a, LocalConstraintAccounting<'a>, ApplicationSourceDomain>;

struct SourceParameters {
    ty: GenericTypeReference,
    length: GenericConstReference,
    effect: GenericEffectReference,
}

impl SourceParameters {
    fn new() -> Self {
        let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(9401));
        Self {
            ty: GenericTypeParameterId::new(owner.clone(), 0).into(),
            length: GenericConstParameterId::new(owner.clone(), 0).into(),
            effect: GenericEffectParameterId::new(owner, 0).into(),
        }
    }

    fn visible(&self) -> TypeConstraintParameterScope {
        TypeConstraintParameterScope::seal_call_scope(
            GenericBinder::EMPTY,
            [TypeConstraintTypeParameterScopeRow::new(
                self.ty.clone(),
                TypeConstraintParameterEligibility::Rigid,
            )],
            [TypeConstraintConstParameterScopeRow::new(
                self.length.clone(),
                TypeConstraintConstEligibility::Rigid,
            )],
            TypeConstraintEffectScope::seal_call_scope(
                [EffectConstraintVariable::new(
                    self.effect.clone(),
                    EffectConstraintEligibility::Rigid,
                )],
                [],
            )
            .unwrap(),
            [],
            [],
        )
        .unwrap()
    }

    fn original(&self, context: &mut Context<'_>) -> OriginalProducerContext {
        EnclosingGenericParameterScope::sealed(
            [self.ty.free_parameter().unwrap().clone()],
            [self.length.free_parameter().unwrap().clone()],
            [self.effect.free_parameter().unwrap().clone()],
        )
        .unwrap()
        .original_context(context)
        .unwrap()
    }

    fn values(&self, context: &mut Context<'_>) -> Vec<TypeKind> {
        let binder = GenericBinder::new(1, 1, 1);
        let scope = GenericScope::default().with_binder(binder);
        let predicate = EffectFormula::variable(self.effect.clone(), context)
            .unwrap()
            .subset(&EffectFormula::empty(), context)
            .unwrap();
        vec![
            TypeKind::GenericParam(self.ty.clone()),
            TypeKind::Array {
                item: Box::new(TypeKind::I64),
                len: ArrayLength::Generic(self.length.clone()),
            },
            TypeKind::function_with_effects(
                [],
                TypeKind::I64,
                EffectRow::open(EffectSet::new(), self.effect.clone()),
            ),
            TypeKind::function_with_contract(
                binder,
                predicate,
                [TypeKind::Array {
                    item: Box::new(TypeKind::GenericParam(scope.bound_type(0, 0).unwrap())),
                    len: ArrayLength::Generic(scope.bound_const(0, 0).unwrap()),
                }],
                TypeKind::I64,
                EffectRow::open(EffectSet::new(), scope.bound_effect(0, 0).unwrap()),
            ),
        ]
    }
}

fn context(cancellation: &AtomicBool) -> Context<'_> {
    TypeConstraintContext::with_accounting(LocalConstraintAccounting::new(
        TypeConstraintLimits::new(8192, 8192, 128, 128),
        cancellation,
    ))
}

#[test]
fn source_values_cannot_borrow_free_references_from_the_receiving_inventory() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let parameters = SourceParameters::new();
    let application = context
        .fixture_application_scope(0, parameters.visible())
        .unwrap();
    let path = context.start_path(application).unwrap();
    let original = context.fixture_original_context(None).unwrap();
    assert_eq!(
        path.applications.parameter_eligibility(&parameters.ty),
        Some(TypeConstraintParameterEligibility::Rigid)
    );
    for (index, value) in parameters.values(&mut context).into_iter().enumerate() {
        let owner = Arc::new(OwnedSourceValue {
            source: ConstraintSourceId::new(path.root_id(), 7),
            original: original.clone(),
            receipt: None,
            value,
        });
        let expected = match index {
            0 => references::type_out_of_scope(&parameters.ty),
            1 => references::const_out_of_scope(&parameters.length),
            _ => references::effect_out_of_scope(&parameters.effect),
        };
        assert_eq!(
            resolve_probe_term(SourceProbeTerm::Value(owner), &path, &mut context).err(),
            Some(expected)
        );
    }
}

#[test]
fn source_values_keep_their_own_declaration_and_explicit_import_authority() {
    for imported in [false, true] {
        let cancellation = AtomicBool::new(false);
        let mut context = context(&cancellation);
        let parameters = SourceParameters::new();
        let application = context
            .fixture_application_scope(0, parameters.visible())
            .unwrap();
        let path = context.start_path(application).unwrap();
        let original = if imported {
            let mut keys: Vec<ConstraintGenericParameterId> = vec![
                parameters.ty.clone().into(),
                parameters.length.clone().into(),
                parameters.effect.clone().into(),
            ];
            keys.sort();
            let lease = path.applications.import_parameters(&keys).unwrap();
            context.fixture_original_context(Some(&lease)).unwrap()
        } else {
            parameters.original(&mut context)
        };
        assert_ne!(original, path.lineage.original);
        for value in parameters.values(&mut context) {
            let expected = value.clone();
            let owner: Arc<dyn ConstraintSourceValueOwner<ApplicationSourceDomain>> =
                Arc::new(OwnedSourceValue {
                    source: ConstraintSourceId::new(path.root_id(), 7),
                    original: original.clone(),
                    receipt: None,
                    value,
                });
            let retained = Arc::downgrade(&owner);
            let (actual, resolved) =
                resolve_probe_term(SourceProbeTerm::Value(owner), &path, &mut context).unwrap();
            assert_eq!(actual, expected);
            let ResolvedConstraintSource::Value(owner) = resolved else {
                panic!("the checked source value keeps its actual owner");
            };
            assert_eq!(owner.original_context(), &original);
            assert!(Arc::ptr_eq(&owner, &retained.upgrade().unwrap()));
        }
    }
}

#[test]
fn source_value_admission_checks_budget_before_copying_the_owned_type() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let application = context
        .fixture_application_scope(0, TypeConstraintParameterScope::empty())
        .unwrap();
    let path = context.start_path(application).unwrap();
    let owner = Arc::new(OwnedSourceValue {
        source: ConstraintSourceId::new(path.root_id(), 7),
        original: context.fixture_original_context(None).unwrap(),
        receipt: None,
        value: TypeKind::Tuple(vec![TypeKind::I64]),
    });
    let mut exhausted = TypeConstraintContext::<_, ApplicationSourceDomain>::with_accounting(
        LocalConstraintAccounting::new(TypeConstraintLimits::new(0, 0, 0, 0), &cancellation),
    );
    assert!(matches!(
        resolve_probe_term(SourceProbeTerm::Value(owner), &path, &mut exhausted),
        Err(TypeConstraintError::Abort(_))
    ));
}
