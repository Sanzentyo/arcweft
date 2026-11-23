use std::sync::atomic::AtomicBool;

use crate::{
    effect_row::{
        EffectConstraintEligibility, EffectConstraintVariable, EffectFormula, EffectRow,
        prepared::{
            PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow, SourceEffectParameter,
        },
    },
    effects::EffectSet,
    types::{
        ArrayLength, DetachedGenericOwnerId, GenericBinder, GenericConstParameterId,
        GenericConstReference, GenericEffectParameterId, GenericEffectReference,
        GenericParameterOwnerId, GenericScope, GenericTypeParameterId, GenericTypeReference,
        PreparedEffects, TypeKind,
        constraints::{
            ConstraintDomain, ConstraintGenericParameterId, TypeConstraintConstEligibility,
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

struct TemplateDomain;

impl ConstraintDomain for TemplateDomain {
    type Application = u8;
    type Source = ();
    type AlternativeIndex = ();
    type EvidenceRule = ();
    type ObservedEvidence = ();
    type CheckedEvidence = ();
    type ProbeSemanticBranch = ();
    type SourceValue = std::convert::Infallible;
    type ProbePreparation = ();
    type SealedBranchValue = ();
    type Projection = ();
    type SourceErrorCause = ();
    type ClientInvariant = ();

    fn evidence_accepts(_: &(), _: &()) -> bool {
        false
    }

    fn project_checked_evidence(_: &(), _: &TypeKind) -> Option<()> {
        Some(())
    }

    fn alternative_ordinal(_: &()) -> u32 {
        0
    }

    fn client_invariant_source(_: &()) {}

    fn empty_sealed_branch() {}
}

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, TemplateDomain>;

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

fn assert_opened_type(
    mut actual: TypeKind<PreparedEffects>,
    expected: &TypeKind,
    graph: &PreparedEffectGraph,
    context: &mut Context<'_>,
) {
    let canonical = context.prepare_fixture_type(expected).unwrap();
    if let (
        TypeKind::Function {
            effects, predicate, ..
        },
        TypeKind::Function {
            effects: PreparedEffectRow::Canonical(expected_row),
            predicate: PreparedEffectPredicate::Canonical(expected_predicate),
            ..
        },
    ) = (&mut actual, &canonical)
    {
        let mut row_references = expected_row.variables().unwrap();
        if let Some(reference @ GenericEffectReference::Inference(_)) = row_references.next() {
            assert!(row_references.next().is_none());
            let mut introductions = Vec::new();
            graph
                .read()
                .visit_row_source_parameters(effects, context, &mut |parameter, _| {
                    let SourceEffectParameter::Introduction(owner) = parameter else {
                        panic!("the opened effect is an application of its introducing owner");
                    };
                    introductions.push(owner.clone());
                    Ok(())
                })
                .unwrap();
            assert_eq!(introductions, std::slice::from_ref(reference));
            *effects = PreparedEffectRow::Canonical(expected_row.clone());
        }
        for reference in expected_predicate.variables() {
            if !matches!(reference, GenericEffectReference::Inference(_)) {
                continue;
            }
            let mut introductions = Vec::new();
            graph
                .read()
                .visit_predicate_source_parameters(predicate, context, &mut |parameter, _| {
                    let SourceEffectParameter::Introduction(owner) = parameter else {
                        panic!("the opened predicate uses its introducing owner");
                    };
                    introductions.push(owner.clone());
                    Ok(())
                })
                .unwrap();
            assert_eq!(introductions, std::slice::from_ref(reference));
            *predicate = PreparedEffectPredicate::Canonical(expected_predicate.clone());
        }
    }
    assert_eq!(actual, canonical);
}

#[test]
fn a_template_cannot_borrow_another_applications_rigid_or_inference_inventory() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    for eligibility in [
        TypeConstraintParameterEligibility::Rigid,
        TypeConstraintParameterEligibility::Bindable,
        TypeConstraintParameterEligibility::FutureEligible,
    ] {
        let cancellation = AtomicBool::new(false);
        let mut context = context(&cancellation);
        let root = context
            .fixture_application_scope(0, TypeConstraintParameterScope::empty(), &mut effect_graph)
            .unwrap();
        let root_id = root.id();
        let mut path = context.start_path(root, &mut effect_graph).unwrap();
        let (parameters, keys) = scope(eligibility);
        let expected = opened(&parameters, &keys, eligibility);
        let child = context
            .fixture_application_scope(1, parameters, &mut effect_graph)
            .unwrap();
        let child_id = child.id();
        path = context
            .admit_application(path, child, &mut effect_graph)
            .unwrap();
        for (index, value) in values(&expected, &mut context).into_iter().enumerate() {
            let result = context
                .open_fixture_template_type(&value, &path, child_id, &mut effect_graph)
                .unwrap();
            assert_opened_type(result, &value, &effect_graph, &mut context);
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
                context.open_fixture_template_type(&value, &path, root_id, &mut effect_graph),
                Err(error)
            );
        }
    }
}

#[test]
fn a_template_can_read_its_original_sources_exact_imported_scope() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    for eligibility in [
        TypeConstraintParameterEligibility::Rigid,
        TypeConstraintParameterEligibility::Bindable,
        TypeConstraintParameterEligibility::FutureEligible,
    ] {
        let cancellation = AtomicBool::new(false);
        let mut context = context(&cancellation);
        let (parameters, keys) = scope(eligibility);
        let expected = opened(&parameters, &keys, eligibility);
        let root = context
            .fixture_application_scope(0, parameters, &mut effect_graph)
            .unwrap();
        let mut path = context.start_path(root, &mut effect_graph).unwrap();
        let lease = path
            .applications
            .import_parameters(&expected, &mut context)
            .unwrap();
        let original = context.fixture_original_context(Some(&lease)).unwrap();
        let child = ConstraintApplicationScope::test_root(
            1,
            TypeConstraintParameterScope::empty(),
            &mut effect_graph,
            &mut context,
        )
        .with_fixture_original(original, &mut effect_graph, &mut context)
        .unwrap();
        let child_id = child.id();
        path = context
            .admit_application(path, child, &mut effect_graph)
            .unwrap();
        for value in values(&expected, &mut context) {
            let result = context
                .open_fixture_template_type(&value, &path, child_id, &mut effect_graph)
                .unwrap();
            assert_opened_type(result, &value, &effect_graph, &mut context);
        }
    }
}

#[test]
fn template_opening_retains_function_local_binders_and_restores_actual_ambient_frames() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let binder = GenericBinder::new(1, 1, 1);
    let outer = context.enter_binder_scope(binder);
    let ambient = context.lexical_frames().clone();
    let root = context
        .fixture_application_scope(0, TypeConstraintParameterScope::empty(), &mut effect_graph)
        .unwrap();
    let path = context.start_path(root, &mut effect_graph).unwrap();
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
    let value = context.prepare_fixture_type(&value).unwrap();
    assert_eq!(
        context
            .open_template_type(&value, &path, path.root_id(), &mut effect_graph)
            .unwrap(),
        value
    );
    assert_eq!(context.lexical_frames(), &ambient);
    let (_, keys) = scope(TypeConstraintParameterEligibility::Rigid);
    for value in values(&keys, &mut context) {
        assert!(
            context
                .open_fixture_template_type(&value, &path, path.root_id(), &mut effect_graph)
                .is_err()
        );
        assert_eq!(context.lexical_frames(), &ambient);
    }
    context.restore_binder_scope(outer);
}
