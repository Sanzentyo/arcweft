use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
};

use super::*;
use crate::{
    effect_row::{EffectConstraintEligibility, EffectConstraintVariable, EffectRow},
    effects::EffectSet,
    types::{
        ArrayLength, DetachedGenericOwnerId, GenericBinder, GenericConstParameterId,
        GenericEffectParameterId, GenericParameterOwnerId, GenericTypeParameterId, TypeKind,
        constraints::{
            ConstraintAcceptance, ConstraintPath, TypeConstraintAbort,
            TypeConstraintConstEligibility, TypeConstraintError, TypeConstraintInvariant,
            TypeConstraintParameterEligibility, TypeConstraintSolution,
            context::{
                LocalConstraintAccounting, TypeConstraintConstParameterScopeRow,
                TypeConstraintContext, TypeConstraintLimits, TypeConstraintTypeParameterScopeRow,
            },
            fixture_relate_selected_call,
            normalization::seal_path,
        },
    },
};

struct Domain;

impl ConstraintDomain for Domain {
    type Application = u32;
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

    fn evidence_accepts((): &(), (): &()) -> bool {
        true
    }
    fn project_checked_evidence((): &(), _: &TypeKind) -> Option<()> {
        Some(())
    }
    fn alternative_ordinal((): &()) -> u32 {
        0
    }
    fn client_invariant_source((): &()) {}
    fn empty_sealed_branch() {}
}

type Context<'c> = TypeConstraintContext<'c, LocalConstraintAccounting<'c>, Domain>;

fn parameters(application: u32) -> TypeConstraintParameterScope {
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(
        90_000 + u64::from(application),
    ));
    let effect = GenericEffectParameterId::new(owner.clone(), 0).into();
    let effects = TypeConstraintEffectScope::seal_call_scope(
        [EffectConstraintVariable::new(
            effect,
            EffectConstraintEligibility::Bindable,
        )],
        [],
    )
    .expect("application effect inventory");
    let parameters = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        [TypeConstraintTypeParameterScopeRow::new(
            GenericTypeParameterId::new(owner.clone(), 0),
            TypeConstraintParameterEligibility::Bindable,
        )],
        [TypeConstraintConstParameterScopeRow::new(
            GenericConstParameterId::new(owner, 0),
            TypeConstraintConstEligibility::Bindable,
        )],
        effects,
        [],
        [],
    )
    .expect("application parameter inventory");
    parameters
}

fn scope(
    application: u32,
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    context: &mut Context<'_>,
) -> ConstraintApplicationScope<Domain> {
    ConstraintApplicationScope::test_root(application, parameters(application), graph, context)
}

fn effect(scope: &ConstraintApplicationScope<Domain>) -> GenericEffectReference {
    scope
        .parameters()
        .effect_reference(
            scope
                .effects()
                .variables()
                .next()
                .expect("effect parameter")
                .variable(),
        )
        .expect("effect slot shares the application's opening")
}

fn context(cancellation: &AtomicBool, nodes: u64) -> Context<'_> {
    Context::with_accounting(LocalConstraintAccounting::new(
        TypeConstraintLimits::new(4096, nodes, 128, 128),
        cancellation,
    ))
}

#[test]
fn binding_owners_keep_their_original_source_across_imports_and_equal_frame_shapes() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 4096);
    let source = |id| {
        crate::callable::EnclosingGenericParameterScope::sealed(
            [GenericTypeParameterId::new(
                GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(id)),
                0,
            )],
            [],
            [],
        )
        .unwrap()
    };
    let original = source(92_101).original_context(&mut context).unwrap();
    let foreign = source(92_102).original_context(&mut context).unwrap();
    assert_eq!(original.scope(), foreign.scope());
    assert_ne!(original, foreign);
    let parameters = parameters(0);
    let owner = context
        .application_scope(0, parameters.clone(), original.clone(), &mut effect_graph)
        .unwrap();
    let same = ConstraintApplicationScope::<Domain>::new(
        0,
        parameters.clone(),
        owner.introduction.clone(),
        original.clone(),
        owner.effect_functions.clone(),
        owner.opened_effect_predicate.clone(),
    );
    let substituted = ConstraintApplicationScope::<Domain>::new(
        0,
        parameters,
        owner.introduction.clone(),
        foreign,
        owner.effect_functions.clone(),
        owner.opened_effect_predicate.clone(),
    );
    assert!(owner.same_owner(&same));
    assert!(!owner.same_owner(&substituted));
    let root = context
        .start_path_with_imported(owner, None, original.clone(), effect_graph.read())
        .unwrap();
    let parameter = root
        .applications
        .require_application(root.root_id())
        .unwrap()
        .parameters()
        .iter()
        .next()
        .unwrap()
        .0
        .clone();
    let parameter = root
        .applications
        .require_application(root.root_id())
        .unwrap()
        .parameters()
        .type_reference(&parameter)
        .unwrap();
    let key = super::super::ConstraintGenericParameterId::Type(parameter);
    let binding = root.applications.binding_owner(&key).unwrap();
    assert_eq!(binding.original, &original);
    let lease = root.applications.import_parameters(&[key.clone()]).unwrap();
    let child_original = context.fixture_original_context(Some(&lease)).unwrap();
    assert_ne!(child_original, original);
    let child_owner = context
        .application_scope(
            1,
            TypeConstraintParameterScope::empty(),
            child_original.clone(),
            &mut effect_graph,
        )
        .unwrap();
    let child = context
        .start_path_with_imported(
            child_owner,
            Some(lease),
            child_original,
            effect_graph.read(),
        )
        .unwrap();
    let imported = child.applications.binding_owner(&key).unwrap();
    assert_eq!(imported.original, &original);
    assert_eq!(imported.introduction, binding.introduction);
}

#[test]
fn a_root_cannot_replace_its_application_owners_original_context() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 4096);
    let original = context.fixture_original_context(None).unwrap();
    let owner = context
        .application_scope(
            0,
            TypeConstraintParameterScope::empty(),
            original,
            &mut effect_graph,
        )
        .unwrap();
    let foreign = crate::callable::EnclosingGenericParameterScope::sealed([], [], [])
        .unwrap()
        .original_context(&mut context)
        .unwrap();
    assert!(matches!(
        context.start_path_with_imported(owner, None, foreign, effect_graph.read()),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::OriginalProducer
        ))
    ));
}

#[test]
fn imported_bindable_parameters_solve_locally_without_mutating_or_exporting_the_parent() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 4096);
    let parent = scope(0, &mut effect_graph, &mut context);
    let parent_array = array(&parent);
    let parent_effect = effect(&parent);
    let expected = TypeKind::function_with_effects(
        [],
        parent_array.clone(),
        EffectRow::open(EffectSet::new(), parent_effect.clone()),
    );
    let parent = context.start_path(parent, &mut effect_graph).unwrap();
    let parent_effects = parent.effects.clone();
    let TypeKind::Array { item, len } = &parent_array else {
        unreachable!()
    };
    let TypeKind::GenericParam(ty) = item.as_ref() else {
        unreachable!()
    };
    let ArrayLength::Generic(length) = len else {
        unreachable!()
    };
    let parameters = [
        super::super::ConstraintGenericParameterId::Type(ty.clone()),
        super::super::ConstraintGenericParameterId::Const(length.clone()),
        super::super::ConstraintGenericParameterId::Effect(parent_effect.clone()),
    ];
    let imported = parent.applications.import_parameters(&parameters).unwrap();
    assert!(
        parameters
            .iter()
            .all(|parameter| imported.permits_local_inference(parameter))
    );
    let child = ConstraintApplicationScope::test_root(
        1,
        TypeConstraintParameterScope::empty(),
        &mut effect_graph,
        &mut context,
    );
    let child_id = child.id();
    let child = context
        .start_fixture_path_with_imported(child, Some(imported), &mut effect_graph)
        .unwrap();
    let actual = TypeKind::function_with_effects(
        [],
        TypeKind::Array {
            item: Box::new(TypeKind::String),
            len: ArrayLength::Const(3),
        },
        EffectRow::closed(EffectSet::from_labels(&["fs.read"]).unwrap()),
    );
    let child = relation(&mut effect_graph, child, &expected, &actual, &mut context);
    let child = seal_path(child, &mut effect_graph, &mut context).unwrap();
    let projected = crate::types::constraints::seal_type(
        &expected,
        child.projection_view(),
        &mut BTreeSet::new(),
        &mut effect_graph,
        &mut context,
    )
    .unwrap();
    let (
        TypeKind::Function {
            return_type: projected_result,
            ..
        },
        TypeKind::Function {
            return_type: actual_result,
            effects: actual_effects,
            ..
        },
    ) = (&projected, &actual)
    else {
        unreachable!()
    };
    assert_eq!(projected_result, actual_result);
    assert_eq!(
        child.effects.bindings(&mut context).unwrap(),
        vec![(parent_effect, actual_effects.clone())]
    );
    let solution = TypeConstraintSolution::complete_fixture_application(
        &child,
        child_id,
        &mut effect_graph,
        &mut context,
    )
    .expect("local inference is not a child declaration argument");
    assert_eq!(solution.bindings().count(), 0);
    assert_eq!(solution.const_bindings().count(), 0);
    assert_eq!(solution.effect_bindings().count(), 0);
    assert!(parent.bindings.is_empty());
    assert!(parent.const_bindings.is_empty());
    assert_eq!(parent.effects, parent_effects);
    assert_eq!(
        crate::types::constraints::seal_type(
            &parent_array,
            parent.projection_view(),
            &mut BTreeSet::new(),
            &mut effect_graph,
            &mut context,
        )
        .unwrap(),
        parent_array,
    );
}

#[test]
fn imported_rigid_and_future_parameters_cannot_be_specialized_by_a_child() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    for future in [false, true] {
        let cancellation = AtomicBool::new(false);
        let mut context = context(&cancellation, 4096);
        let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(90_200));
        let ty: GenericTypeReference = GenericTypeParameterId::new(owner.clone(), 0).into();
        let length: GenericConstReference = GenericConstParameterId::new(owner.clone(), 0).into();
        let effect: GenericEffectReference = GenericEffectParameterId::new(owner, 0).into();
        let scope = TypeConstraintParameterScope::seal_call_scope(
            GenericBinder::EMPTY,
            [TypeConstraintTypeParameterScopeRow::new(
                ty.clone(),
                if future {
                    TypeConstraintParameterEligibility::FutureEligible
                } else {
                    TypeConstraintParameterEligibility::Rigid
                },
            )],
            [TypeConstraintConstParameterScopeRow::new(
                length.clone(),
                if future {
                    TypeConstraintConstEligibility::FutureEligible
                } else {
                    TypeConstraintConstEligibility::Rigid
                },
            )],
            TypeConstraintEffectScope::seal_call_scope(
                [EffectConstraintVariable::new(
                    effect.clone(),
                    if future {
                        EffectConstraintEligibility::FutureEligible
                    } else {
                        EffectConstraintEligibility::Rigid
                    },
                )],
                [],
            )
            .unwrap(),
            [],
            [],
        )
        .unwrap();
        let ty = scope.type_reference(&ty).unwrap_or(ty);
        let length = scope.const_reference(&length).unwrap_or(length);
        let effect = scope.effect_reference(&effect).unwrap_or(effect);
        let parent = context
            .start_path(
                ConstraintApplicationScope::test_root(0, scope, &mut effect_graph, &mut context),
                &mut effect_graph,
            )
            .unwrap();
        let parameters = [
            super::super::ConstraintGenericParameterId::Type(ty.clone()),
            super::super::ConstraintGenericParameterId::Const(length.clone()),
            super::super::ConstraintGenericParameterId::Effect(effect.clone()),
        ];
        let imported = parent.applications.import_parameters(&parameters).unwrap();
        assert!(
            parameters
                .iter()
                .all(|parameter| !imported.permits_local_inference(parameter))
        );
        let child = context
            .start_fixture_path_with_imported(
                ConstraintApplicationScope::test_root(
                    1,
                    TypeConstraintParameterScope::empty(),
                    &mut effect_graph,
                    &mut context,
                ),
                Some(imported),
            )
            .unwrap();
        assert_eq!(
            context.parameter_eligibility(&ty, child.projection_view()),
            Some(TypeConstraintParameterEligibility::Rigid)
        );
        assert_eq!(
            context.const_parameter_eligibility(&length, child.projection_view()),
            Some(TypeConstraintConstEligibility::Rigid)
        );
        assert_eq!(
            context.effect_eligibility(&effect, child.projection_view()),
            Some(EffectConstraintEligibility::Rigid)
        );
        for (expected, actual) in [
            (TypeKind::GenericParam(ty), TypeKind::String),
            (
                TypeKind::Array {
                    item: Box::new(TypeKind::String),
                    len: ArrayLength::Generic(length),
                },
                TypeKind::Array {
                    item: Box::new(TypeKind::String),
                    len: ArrayLength::Const(3),
                },
            ),
            (
                TypeKind::function_with_effects(
                    [],
                    TypeKind::Unit,
                    EffectRow::open(EffectSet::new(), effect),
                ),
                TypeKind::function_with_effects(
                    [],
                    TypeKind::Unit,
                    EffectRow::closed(EffectSet::from_labels(&["fs.read"]).unwrap()),
                ),
            ),
        ] {
            let original = child.lineage.original.clone();
            let expected = context.fixture_operand_in(&original, &expected).unwrap();
            let actual = context.fixture_operand_in(&original, &actual).unwrap();
            assert!(
                crate::types::constraints::relate_selected_call(
                    &expected.view(),
                    &actual.view(),
                    context.fork_path(&child).unwrap(),
                    &mut context,
                    ConstraintAcceptance::ActualAcceptsPattern
                )
                .unwrap()
                .is_empty()
            );
        }
    }
}

fn array(scope: &ConstraintApplicationScope<Domain>) -> TypeKind {
    TypeKind::Array {
        item: Box::new(TypeKind::GenericParam(
            scope
                .parameters()
                .type_reference(scope.parameters().iter().next().unwrap().0)
                .unwrap(),
        )),
        len: ArrayLength::Generic(
            scope
                .parameters()
                .const_reference(scope.parameters().const_iter().next().unwrap().0)
                .unwrap(),
        ),
    }
}

fn relation(
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    path: ConstraintPath<Domain>,
    left: &TypeKind,
    right: &TypeKind,
    context: &mut Context<'_>,
) -> ConstraintPath<Domain> {
    let mut paths = fixture_relate_selected_call(
        graph,
        left,
        right,
        path,
        context,
        ConstraintAcceptance::PatternAcceptsActual,
    )
    .expect("application relation");
    assert_eq!(paths.len(), 1);
    paths.pop().unwrap()
}

#[test]
fn parent_and_child_close_type_constant_and_effect_relations_together() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 2048);
    let parent = scope(0, &mut effect_graph, &mut context);
    let parent_id = parent.id();
    let parent_array = array(&parent);
    let parent_effect = effect(&parent);
    let parent_effect_key = parent
        .effects()
        .variables()
        .next()
        .expect("parent effect parameter")
        .variable()
        .clone();
    let child = scope(1, &mut effect_graph, &mut context);
    let child_id = child.id();
    let child_array = array(&child);
    let child_effect = effect(&child);
    let child_effect_key = child
        .effects()
        .variables()
        .next()
        .expect("child effect parameter")
        .variable()
        .clone();
    let path = context.start_path(parent, &mut effect_graph).unwrap();
    let path = context
        .admit_application(path, child, &mut effect_graph)
        .unwrap();
    let path = relation(
        &mut effect_graph,
        path,
        &child_array,
        &parent_array,
        &mut context,
    );
    let mut path = relation(
        &mut effect_graph,
        path,
        &parent_array,
        &TypeKind::Array {
            item: Box::new(TypeKind::I64),
            len: ArrayLength::Const(9),
        },
        &mut context,
    );
    let parent_row = EffectRow::open(EffectSet::new(), parent_effect.clone());
    let child_row = EffectRow::open(EffectSet::new(), child_effect.clone());
    let expected_row = EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap());
    path.effects
        .constrain_fixture_subset(&mut effect_graph, &expected_row, &parent_row, &mut context)
        .unwrap();
    path.effects
        .constrain_fixture_subset(&mut effect_graph, &parent_row, &expected_row, &mut context)
        .unwrap();
    path.effects
        .constrain_fixture_subset(&mut effect_graph, &parent_row, &child_row, &mut context)
        .unwrap();
    let path = seal_path(path, &mut effect_graph, &mut context).unwrap();
    for (application, effect) in [(parent_id, parent_effect_key), (child_id, child_effect_key)] {
        let solution = TypeConstraintSolution::complete_fixture_application(
            &path,
            application,
            &mut effect_graph,
            &mut context,
        )
        .expect("both application contracts close from the same path");
        assert_eq!(solution.bindings().count(), 1);
        assert_eq!(
            solution.bindings().next().unwrap().1.value(),
            &TypeKind::I64
        );
        assert_eq!(solution.const_bindings().count(), 1);
        assert_eq!(
            solution.const_bindings().next().unwrap().1.value(),
            &ArrayLength::Const(9)
        );
        assert_eq!(
            solution
                .effect_bindings()
                .map(|(key, value)| (key.value(), value.value()))
                .collect::<Vec<_>>(),
            vec![(&effect, &expected_row)]
        );
    }
}

#[test]
fn a_child_transaction_opens_projects_and_seals_its_own_parameters() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 4096);
    let parent = scope(0, &mut effect_graph, &mut context);
    let parent_id = parent.id();
    let child = scope(1, &mut effect_graph, &mut context);
    let child_id = child.id();
    let child_type_key = child.parameters().iter().next().unwrap().0.clone();
    let child_const_key = child.parameters().const_iter().next().unwrap().0.clone();
    let parent_array = array(&parent);
    let child_template = TypeKind::Array {
        item: Box::new(TypeKind::GenericParam(child_type_key.clone())),
        len: ArrayLength::Generic(child_const_key),
    };
    let parent_value = TypeKind::Array {
        item: Box::new(TypeKind::I64),
        len: ArrayLength::Const(1),
    };
    let child_value = TypeKind::Array {
        item: Box::new(TypeKind::String),
        len: ArrayLength::Const(2),
    };
    let path = context.start_path(parent, &mut effect_graph).unwrap();
    let path = relation(
        &mut effect_graph,
        path,
        &parent_array,
        &parent_value,
        &mut context,
    );
    let mut parent_transaction =
        crate::types::constraints::transaction::TypeConstraintTransaction::from_path(
            &mut context,
            &mut effect_graph,
            parent_id,
            path,
            None,
        )
        .unwrap();
    parent_transaction.request_projection(
        &mut context,
        &mut effect_graph,
        (),
        &parent_array,
        crate::types::constraints::TypeConstraintProjectionClosure::Closed,
    );
    let path = context
        .fork_path(parent_transaction.test_single_path())
        .unwrap();
    let path = context
        .admit_application(path, child, &mut effect_graph)
        .unwrap();
    let mut transaction =
        crate::types::constraints::transaction::TypeConstraintTransaction::from_path(
            &mut context,
            &mut effect_graph,
            child_id,
            path,
            None,
        )
        .unwrap();
    transaction.constrain(
        &mut context,
        &mut effect_graph,
        &child_template,
        &child_value,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    let prepared = crate::types::constraints::PreparedSourceConstraint::checked(
        (),
        crate::types::constraints::PreparedConstraintSourceProjection::Scalar,
        [],
        crate::types::constraints::PreparedSourceAlternative::new((), (), child_template.clone()),
    )
    .unwrap();
    transaction
        .begin_prepared_probe(
            &mut context,
            prepared,
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .unwrap();
    let mut ticket = transaction
        .next_probe(&mut context, &mut effect_graph)
        .unwrap()
        .unwrap();
    ticket.input().with_hint(|hint| {
        let crate::types::constraints::ExpectedHint::Alternatives([alternative]) = hint else {
            panic!("one child source alternative");
        };
        assert!(matches!(
            alternative.value_expected(),
            crate::types::constraints::ProjectedExpectedHint::Complete(expected)
                if *expected == &child_value
        ));
    });
    transaction
        .submit_probe(
            &mut context,
            &mut effect_graph,
            ticket.input(),
            crate::types::constraints::transaction::ProbeSubmission::Accepted(
                (ticket
                    .observe(crate::types::constraints::SourceProbeResult::checked(
                        child_value.clone(),
                        (),
                        (),
                        (),
                    ))
                    .expect("observed child source"))
                .close_preparation(()),
            ),
        )
        .unwrap();
    assert!(
        transaction
            .next_probe(&mut context, &mut effect_graph)
            .unwrap()
            .is_none()
    );
    transaction.request_projection(
        &mut context,
        &mut effect_graph,
        (),
        &child_template,
        crate::types::constraints::TypeConstraintProjectionClosure::Closed,
    );
    let mut materialization = transaction
        .next_materialization_ticket(&mut context, &mut effect_graph)
        .unwrap()
        .unwrap();
    assert_eq!(materialization.requests().len(), 1);
    assert_eq!(
        materialization.requests().next().unwrap().actual(),
        &child_value
    );
    let request = materialization.requests().next().unwrap();
    assert_eq!(request.component().applications().len(), 2);
    for (application, expected) in [(0, &parent_value), (1, &child_value)] {
        let completed = request.component().application(application).unwrap();
        assert_eq!(completed.projections().len(), 1);
        assert_eq!(completed.projections()[0].key(), &());
        assert_eq!(completed.projections()[0].value().value(), expected);
        assert_eq!(completed.solution().bindings().count(), 1);
        assert_eq!(completed.solution().const_bindings().count(), 1);
    }
    materialization.bind_callback().unwrap();
    let closed = materialization
        .bind_closed_submission(
            crate::types::constraints::ClosedMaterializationSubmission::Sealed(()),
        )
        .unwrap();
    transaction
        .submit_closed_materialization(materialization, closed)
        .unwrap();
    assert!(
        transaction
            .next_materialization_ticket(&mut context, &mut effect_graph)
            .unwrap()
            .is_none()
    );
    let solved = transaction
        .finish(&mut context, &mut effect_graph, |left, right, _| {
            Ok(left == right)
        })
        .unwrap();
    assert_eq!(solved.component.selected().projections().len(), 1);
    assert_eq!(
        solved.component.selected().projections()[0].value().value(),
        &child_value
    );
    assert_eq!(
        solved
            .component
            .selected()
            .solution()
            .bindings()
            .map(|(key, value)| (key.value().clone(), value.value().clone()))
            .collect::<Vec<_>>(),
        vec![(child_type_key, TypeKind::String)]
    );
}

#[test]
fn inherited_child_bindings_restore_without_replacing_the_parent() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 8192);
    let prior = scope(1, &mut effect_graph, &mut context);
    let prior_id = prior.id();
    let type_key = prior.parameters().iter().next().unwrap().0.clone();
    let const_key = prior.parameters().const_iter().next().unwrap().0.clone();
    let effect_key = prior
        .effects()
        .variables()
        .next()
        .expect("effect parameter")
        .variable()
        .clone();
    let effect = effect(&prior);
    let prior_array = array(&prior);
    let value = TypeKind::Array {
        item: Box::new(TypeKind::String),
        len: ArrayLength::Const(7),
    };
    let row = EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap());
    let prior_path = context.start_path(prior, &mut effect_graph).unwrap();
    let mut prior_path = relation(
        &mut effect_graph,
        prior_path,
        &prior_array,
        &value,
        &mut context,
    );
    prior_path
        .effects
        .constrain_fixture_subset(
            &mut effect_graph,
            &row,
            &EffectRow::open(EffectSet::new(), effect.clone()),
            &mut context,
        )
        .unwrap();
    prior_path
        .effects
        .constrain_fixture_subset(
            &mut effect_graph,
            &EffectRow::open(EffectSet::new(), effect.clone()),
            &row,
            &mut context,
        )
        .unwrap();
    let prior_path = seal_path(prior_path, &mut effect_graph, &mut context).unwrap();
    let inherited = TypeConstraintSolution::complete_fixture_application(
        &prior_path,
        prior_id,
        &mut effect_graph,
        &mut context,
    )
    .unwrap();

    let child = ConstraintApplicationScope::test_root(
        2,
        TypeConstraintParameterScope::seal_call_scope(
            GenericBinder::EMPTY,
            [TypeConstraintTypeParameterScopeRow::new(
                type_key.clone(),
                TypeConstraintParameterEligibility::Bindable,
            )],
            [TypeConstraintConstParameterScopeRow::new(
                const_key.clone(),
                TypeConstraintConstEligibility::Bindable,
            )],
            TypeConstraintEffectScope::seal_call_scope(
                [EffectConstraintVariable::new(
                    effect_key.clone(),
                    EffectConstraintEligibility::Bindable,
                )],
                [effect_key.clone()],
            )
            .unwrap(),
            [type_key.clone()],
            [const_key.clone()],
        )
        .unwrap(),
        &mut effect_graph,
        &mut context,
    );
    let child_id = child.id();
    let child_array = array(&child);
    let parent = scope(0, &mut effect_graph, &mut context);
    let parent_array = array(&parent);
    let parent_value = TypeKind::Array {
        item: Box::new(TypeKind::I64),
        len: ArrayLength::Const(1),
    };
    let path = context.start_path(parent, &mut effect_graph).unwrap();
    let path = relation(
        &mut effect_graph,
        path,
        &parent_array,
        &parent_value,
        &mut context,
    );
    let path = context
        .admit_application(path, child, &mut effect_graph)
        .unwrap();
    let without_inherited = context.fork_path(&path).unwrap();
    assert!(matches!(
        crate::types::constraints::transaction::TypeConstraintTransaction::from_path(
            &mut context,
            &mut effect_graph,
            child_id,
            without_inherited,
            None,
        ),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::InheritedSolution(_)
        ))
    ));
    let mut transaction =
        crate::types::constraints::transaction::TypeConstraintTransaction::from_path(
            &mut context,
            &mut effect_graph,
            child_id,
            path,
            Some(&Arc::new(inherited)),
        )
        .unwrap();
    assert_eq!(
        crate::types::constraints::seal_type(
            &parent_array,
            transaction.test_single_path().projection_view(),
            &mut BTreeSet::new(),
            &mut effect_graph,
            &mut context,
        )
        .unwrap(),
        parent_value
    );
    transaction.request_projection(
        &mut context,
        &mut effect_graph,
        (),
        &child_array,
        crate::types::constraints::TypeConstraintProjectionClosure::Closed,
    );
    let solved = transaction
        .finish(&mut context, &mut effect_graph, |left, right, _| {
            Ok(left == right)
        })
        .unwrap();
    assert_eq!(
        solved.component.selected().projections()[0].value().value(),
        &value
    );
    assert_eq!(
        solved
            .component
            .selected()
            .solution()
            .effect_bindings()
            .map(|(key, value)| (key.value(), value.value()))
            .collect::<Vec<_>>(),
        vec![(&effect_key, &row)]
    );
}

#[test]
fn a_transaction_cannot_enter_an_unadmitted_application() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 4096);
    let opening = scope(0, &mut effect_graph, &mut context);
    let path = context.start_path(opening, &mut effect_graph).unwrap();
    let foreign = scope(1, &mut effect_graph, &mut context);
    assert!(matches!(
        crate::types::constraints::transaction::TypeConstraintTransaction::from_path(
            &mut context,
            &mut effect_graph,
            foreign.id(),
            path,
            None,
        ),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::ParameterScope(
                super::super::TypeConstraintParameterScopeInvariant::ApplicationOutOfScope
            )
        ))
    ));
}

#[test]
fn a_child_scope_is_ineligible_on_an_unchosen_sibling_path() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 2048);
    let opening = scope(0, &mut effect_graph, &mut context);
    let root = context.start_path(opening, &mut effect_graph).unwrap();
    let root_id = root.root_id();
    let sibling = context.fork_path(&root).unwrap();
    let child = scope(1, &mut effect_graph, &mut context);
    let child_type = child
        .parameters()
        .type_reference(child.parameters().iter().next().unwrap().0)
        .unwrap();
    let child_const = child
        .parameters()
        .const_reference(child.parameters().const_iter().next().unwrap().0)
        .unwrap();
    let child_effect = effect(&child);
    let chosen = context
        .admit_application(root, child, &mut effect_graph)
        .unwrap();
    let chosen = seal_path(chosen, &mut effect_graph, &mut context).unwrap();
    assert_eq!(chosen.root_id(), root_id);
    assert_eq!(sibling.root_id(), root_id);
    assert_eq!(chosen.root_scope().application(), 0);
    assert_eq!(sibling.root_scope().application(), 0);
    assert_eq!(
        context.parameter_eligibility(&child_type, chosen.projection_view()),
        Some(TypeConstraintParameterEligibility::Bindable)
    );
    assert_eq!(
        context.const_parameter_eligibility(&child_const, chosen.projection_view()),
        Some(TypeConstraintConstEligibility::Bindable)
    );
    assert_eq!(
        context.effect_eligibility(&child_effect, chosen.projection_view()),
        Some(EffectConstraintEligibility::Bindable)
    );
    assert_eq!(
        context.parameter_eligibility(&child_type, sibling.projection_view()),
        None
    );
    assert_eq!(
        context.const_parameter_eligibility(&child_const, sibling.projection_view()),
        None
    );
    assert_eq!(
        context.effect_eligibility(&child_effect, sibling.projection_view()),
        None
    );
    assert!(
        sibling
            .effects
            .validate_row(&EffectRow::open(EffectSet::new(), child_effect.clone()))
            .is_err()
    );
}

#[test]
fn admitted_children_retain_their_rigid_type_and_constant_references() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(90_100));
    let type_reference = GenericTypeReference::Free(GenericTypeParameterId::new(owner.clone(), 0));
    let const_reference = GenericConstReference::Free(GenericConstParameterId::new(owner, 0));
    let captured_scope = |application| {
        ConstraintApplicationScope::test_root(
            application,
            TypeConstraintParameterScope::seal_call_scope(
                GenericBinder::EMPTY,
                [TypeConstraintTypeParameterScopeRow::new(
                    type_reference.clone(),
                    TypeConstraintParameterEligibility::Rigid,
                )],
                [TypeConstraintConstParameterScopeRow::new(
                    const_reference.clone(),
                    TypeConstraintConstEligibility::Rigid,
                )],
                crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], [])
                    .expect("empty effect scope"),
                [],
                [],
            )
            .unwrap(),
            &mut effect_graph,
            &mut context,
        )
    };
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 2048);
    let opening = scope(0, &mut effect_graph, &mut context);
    let root = context.start_path(opening, &mut effect_graph).unwrap();
    let sibling = context.fork_path(&root).unwrap();
    let mut chosen = root;

    // Several applications may capture the same declaration-owned rigid
    // reference. Admission never turns that reference into an inference slot.
    for application in [1, 2] {
        chosen = context
            .admit_application(chosen, captured_scope(application), &mut effect_graph)
            .unwrap();
        assert_eq!(
            context.parameter_eligibility(&type_reference, chosen.projection_view()),
            Some(TypeConstraintParameterEligibility::Rigid)
        );
        assert_eq!(
            context.const_parameter_eligibility(&const_reference, chosen.projection_view()),
            Some(TypeConstraintConstEligibility::Rigid)
        );
        let captured = TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(type_reference.clone())),
            len: ArrayLength::Generic(const_reference.clone()),
        };
        assert_eq!(
            crate::types::constraints::seal_type(
                &captured,
                chosen.projection_view(),
                &mut BTreeSet::new(),
                &mut effect_graph,
                &mut context,
            )
            .unwrap(),
            captured
        );
    }
    assert_eq!(
        context.parameter_eligibility(&type_reference, sibling.projection_view()),
        None
    );
    assert_eq!(
        context.const_parameter_eligibility(&const_reference, sibling.projection_view()),
        None
    );
}

#[test]
fn completion_checks_an_alias_in_its_original_owner_before_merging_ambient_frames() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 8192);
    let parameters = |application: u64| {
        TypeConstraintParameterScope::seal_call_scope(
            GenericBinder::EMPTY,
            [TypeConstraintTypeParameterScopeRow::new(
                GenericTypeParameterId::new(
                    GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(
                        90_102 + application,
                    )),
                    0,
                ),
                TypeConstraintParameterEligibility::Bindable,
            )],
            [],
            TypeConstraintEffectScope::seal_call_scope([], []).unwrap(),
            [],
            [],
        )
        .unwrap()
    };
    let outer = context
        .fixture_application_scope(0, parameters(0), &mut effect_graph)
        .unwrap();
    let outer_id = outer.id();
    let outer_key = outer
        .parameters()
        .type_reference(outer.parameters().iter().next().unwrap().0)
        .unwrap();
    let mut path = context.start_path(outer, &mut effect_graph).unwrap();
    let enclosing = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
    let inner = context
        .fixture_application_scope(1, parameters(1), &mut effect_graph)
        .unwrap();
    let inner_key = inner
        .parameters()
        .type_reference(inner.parameters().iter().next().unwrap().0)
        .unwrap();
    path = context
        .admit_application(path, inner, &mut effect_graph)
        .unwrap();
    let alias = TypeKind::GenericParam(inner_key.clone());
    path = context
        .fixture_add_binding(path, outer_key, &alias, &mut effect_graph)
        .unwrap()
        .unwrap();
    let local = TypeKind::GenericParam(context.lexical_scope().bound_type(0, 0).unwrap());
    path = context
        .fixture_add_binding(path, inner_key, &local, &mut effect_graph)
        .unwrap()
        .unwrap();
    assert!(matches!(
        TypeConstraintSolution::complete_fixture_application(
            &path,
            outer_id,
            &mut effect_graph,
            &mut context
        ),
        Err(TypeConstraintError::Rejected(
            crate::types::constraints::TypeConstraintRejection::EscapingInstantiation { .. }
        ))
    ));
    assert_eq!(context.lexical_scope().binders().len(), 1);
    context.restore_binder_scope(enclosing);
}

#[test]
fn completed_parent_retains_chosen_child_captures_read_only_in_its_continuation() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 8192);
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(90_101));
    let ty = GenericTypeReference::Free(GenericTypeParameterId::new(owner.clone(), 0));
    let length = GenericConstReference::Free(GenericConstParameterId::new(owner.clone(), 0));
    let row = GenericEffectReference::Free(GenericEffectParameterId::new(owner, 0));
    let child = ConstraintApplicationScope::test_root(
        1,
        TypeConstraintParameterScope::seal_call_scope(
            GenericBinder::EMPTY,
            [TypeConstraintTypeParameterScopeRow::new(
                ty.clone(),
                TypeConstraintParameterEligibility::Rigid,
            )],
            [TypeConstraintConstParameterScopeRow::new(
                length.clone(),
                TypeConstraintConstEligibility::Rigid,
            )],
            TypeConstraintEffectScope::seal_call_scope(
                [EffectConstraintVariable::new(
                    row.clone(),
                    EffectConstraintEligibility::Rigid,
                )],
                [],
            )
            .unwrap(),
            [],
            [],
        )
        .unwrap(),
        &mut effect_graph,
        &mut context,
    );
    let parent = scope(0, &mut effect_graph, &mut context);
    let parent_array = array(&parent);
    let parent_effect = EffectRow::open(EffectSet::new(), effect(&parent));
    let type_keys = parent
        .parameters()
        .iter()
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    let const_keys = parent
        .parameters()
        .const_iter()
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    let effect_keys = parent
        .effects()
        .variables()
        .map(|row| row.variable().clone())
        .collect::<Vec<_>>();
    let captured = TypeKind::Array {
        item: Box::new(TypeKind::GenericParam(ty.clone())),
        len: ArrayLength::Generic(length.clone()),
    };
    let captured_effect = EffectRow::open(EffectSet::new(), row.clone());
    let parent = context.start_path(parent, &mut effect_graph).unwrap();
    let parent = context
        .admit_application(parent, child, &mut effect_graph)
        .unwrap();
    let original = context
        .fixture_original_context_with_rigid(crate::types::constraints::OriginalRigidScope {
            types: &[ty.clone()],
            consts: &[length.clone()],
            effects: &[row.clone()],
            imported: None,
        })
        .unwrap();
    let captured_operand = context.fixture_operand_in(&original, &captured).unwrap();
    let pattern = context.fixture_operand(parent_array);
    let mut paths = crate::types::constraints::relate_selected_call(
        &pattern.view(),
        &captured_operand.view(),
        parent,
        &mut context,
        ConstraintAcceptance::PatternAcceptsActual,
    )
    .unwrap();
    assert_eq!(paths.len(), 1);
    let mut parent = paths.pop().unwrap();
    parent
        .effects
        .constrain_fixture_subset(
            &mut effect_graph,
            &parent_effect,
            &captured_effect,
            &mut context,
        )
        .unwrap();
    parent
        .effects
        .constrain_fixture_subset(
            &mut effect_graph,
            &captured_effect,
            &parent_effect,
            &mut context,
        )
        .unwrap();
    let solution = TypeConstraintSolution::complete_fixture_application(
        &parent,
        parent.root_id(),
        &mut effect_graph,
        &mut context,
    )
    .unwrap();
    let continuation = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        type_keys.iter().cloned().map(|key| {
            TypeConstraintTypeParameterScopeRow::new(
                key,
                TypeConstraintParameterEligibility::Bindable,
            )
        }),
        const_keys.iter().cloned().map(|key| {
            TypeConstraintConstParameterScopeRow::new(key, TypeConstraintConstEligibility::Bindable)
        }),
        TypeConstraintEffectScope::seal_call_scope(
            effect_keys.iter().cloned().map(|key| {
                EffectConstraintVariable::new(key, EffectConstraintEligibility::Bindable)
            }),
            effect_keys.iter().cloned(),
        )
        .unwrap(),
        type_keys.iter().cloned(),
        const_keys.iter().cloned(),
    )
    .unwrap();
    let next = context
        .start_path(
            ConstraintApplicationScope::test_root(2, continuation, &mut effect_graph, &mut context),
            &mut effect_graph,
        )
        .unwrap();
    assert_eq!(
        context.parameter_eligibility(&ty, next.projection_view()),
        None
    );
    let next = Arc::new(solution)
        .restore_inherited_path(next.root_id(), next, &mut effect_graph, &mut context)
        .unwrap();
    assert_eq!(
        context.parameter_eligibility(&ty, next.projection_view()),
        Some(TypeConstraintParameterEligibility::Rigid)
    );
    assert_eq!(
        context.const_parameter_eligibility(&length, next.projection_view()),
        Some(TypeConstraintConstEligibility::Rigid)
    );
    assert_eq!(
        context.effect_eligibility(&row, next.projection_view()),
        Some(EffectConstraintEligibility::Rigid)
    );
    let parameters = [ty.clone().into(), length.clone().into(), row.into()];
    let lease = next.applications.import_parameters(&parameters).unwrap();
    assert_eq!(lease.parameters(), &parameters);
    assert!(
        parameters
            .iter()
            .all(|parameter| !lease.permits_local_inference(parameter))
    );
    let empty = ConstraintApplicationScope::test_root(
        3,
        TypeConstraintParameterScope::empty(),
        &mut effect_graph,
        &mut context,
    );
    let child = context
        .start_fixture_path_with_imported(empty, Some(lease), &mut effect_graph)
        .unwrap();
    assert_eq!(
        crate::types::constraints::seal_type(
            &captured,
            child.projection_view(),
            &mut BTreeSet::new(),
            &mut effect_graph,
            &mut context
        )
        .unwrap(),
        captured
    );
    let completed = TypeConstraintSolution::complete_fixture_application(
        &next,
        next.root_id(),
        &mut effect_graph,
        &mut context,
    )
    .unwrap();
    assert_eq!(
        completed.bindings().next().unwrap().1.value(),
        &TypeKind::GenericParam(ty)
    );
    assert_eq!(
        completed.const_bindings().next().unwrap().1.value(),
        &ArrayLength::Generic(length)
    );
    assert_eq!(
        completed.effect_bindings().next().unwrap().1.value(),
        &captured_effect
    );
}

#[test]
fn completed_parent_does_not_export_unused_child_declarations() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 8192);
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(90_103));
    let ty = GenericTypeReference::Free(GenericTypeParameterId::new(owner.clone(), 0));
    let length = GenericConstReference::Free(GenericConstParameterId::new(owner.clone(), 0));
    let row = GenericEffectReference::Free(GenericEffectParameterId::new(owner, 0));
    let child = ConstraintApplicationScope::test_root(
        1,
        TypeConstraintParameterScope::seal_call_scope(
            GenericBinder::EMPTY,
            [TypeConstraintTypeParameterScopeRow::new(
                ty.clone(),
                TypeConstraintParameterEligibility::Rigid,
            )],
            [TypeConstraintConstParameterScopeRow::new(
                length.clone(),
                TypeConstraintConstEligibility::Rigid,
            )],
            TypeConstraintEffectScope::seal_call_scope(
                [EffectConstraintVariable::new(
                    row.clone(),
                    EffectConstraintEligibility::Rigid,
                )],
                [],
            )
            .unwrap(),
            [],
            [],
        )
        .unwrap(),
        &mut effect_graph,
        &mut context,
    );
    let parent = context
        .start_path(ConstraintApplicationScope::test_root(
            0,
            TypeConstraintParameterScope::empty(),
            &mut effect_graph,
            &mut context,
        ))
        .unwrap();
    let parent = context
        .admit_application(parent, child, &mut effect_graph)
        .unwrap();
    let parameters = [ty.clone().into(), length.clone().into(), row.clone().into()];
    assert!(parent.applications.import_parameters(&parameters).is_ok());
    let solution = Arc::new(
        TypeConstraintSolution::complete_fixture_application(
            &parent,
            parent.root_id(),
            &mut effect_graph,
            &mut context,
        )
        .unwrap(),
    );
    let next = context
        .start_path(ConstraintApplicationScope::test_root(
            2,
            TypeConstraintParameterScope::empty(),
            &mut effect_graph,
            &mut context,
        ))
        .unwrap();
    let next = solution
        .restore_inherited_path(next.root_id(), next, &mut effect_graph, &mut context)
        .unwrap();
    for parameter in parameters {
        assert!(
            next.applications.import_parameters(&[parameter]).is_err(),
            "a completed value cannot issue a lease for an unused child declaration"
        );
    }
    assert_eq!(
        context.parameter_eligibility(&ty, next.projection_view()),
        None
    );
    assert_eq!(
        context.const_parameter_eligibility(&length, next.projection_view()),
        None
    );
    assert_eq!(
        context.effect_eligibility(&row, next.projection_view()),
        None
    );
    assert!(
        next.effects
            .validate_row(&EffectRow::open(EffectSet::new(), row))
            .is_err()
    );
}

#[test]
fn application_identity_and_live_opening_cannot_be_admitted_twice() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 2048);
    let parent = scope(0, &mut effect_graph, &mut context);
    let reused_opening = ConstraintApplicationScope::test_root(
        1,
        parent.parameters().clone(),
        &mut effect_graph,
        &mut context,
    );
    let root = context.start_path(parent, &mut effect_graph).unwrap();
    for duplicate in [scope(0, &mut effect_graph, &mut context), reused_opening] {
        let sibling = context.fork_path(&root).unwrap();
        assert!(matches!(
            context.admit_application(sibling, duplicate, &mut effect_graph),
            Err(TypeConstraintError::Invariant(
                TypeConstraintInvariant::ParameterScope(
                    TypeConstraintParameterScopeInvariant::ApplicationAlreadyAdmitted
                )
            ))
        ));
    }
    assert_eq!(root.applications.len(), 1);
}

#[test]
fn repeated_effect_declarations_open_independently_in_distinct_applications() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 2048);
    let parent = scope(0, &mut effect_graph, &mut context);
    let child =
        ConstraintApplicationScope::test_root(1, parameters(0), &mut effect_graph, &mut context);
    assert_eq!(
        parent
            .effects()
            .variables()
            .next()
            .expect("parent effect parameter")
            .variable(),
        child
            .effects()
            .variables()
            .next()
            .expect("child effect parameter")
            .variable()
    );
    assert_ne!(effect(&parent), effect(&child));
    let root = context.start_path(parent, &mut effect_graph).unwrap();
    let sibling = context.fork_path(&root).unwrap();
    let combined = context
        .admit_application(sibling, child, &mut effect_graph)
        .unwrap();
    assert_eq!(combined.effects.bindings(&mut context).unwrap().len(), 2);
    assert_eq!(root.applications.len(), 1);
    assert_eq!(root.effects.bindings(&mut context).unwrap().len(), 1);
}

#[test]
fn admission_charges_the_shared_context_and_observes_cancellation() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut setup = context(&cancellation, 2048);
    let parent = scope(0, &mut effect_graph, &mut setup);
    let limited_child = scope(1, &mut effect_graph, &mut setup);
    let cancelled_child = scope(1, &mut effect_graph, &mut setup);
    let mut context = context(&cancellation, 2);
    // This test isolates application admission from source-context admission.
    let original = parent.original_context().clone();
    let root = context
        .start_path_with_imported(parent, None, original, effect_graph.read())
        .unwrap();
    let sibling = context.fork_path(&root).unwrap();
    assert!(matches!(
        context.admit_application(sibling, limited_child, &mut effect_graph),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit {
            actual: 3,
            limit: 2
        }))
    ));
    assert!(context.enter_node().is_ok());
    assert!(context.enter_node().is_ok());
    let sibling = context.fork_path(&root).unwrap();
    cancellation.store(true, Ordering::Release);
    assert!(matches!(
        context.admit_application(sibling, cancelled_child, &mut effect_graph),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
    ));
    assert_eq!(root.applications.len(), 1);
}

#[test]
fn equal_binding_values_do_not_erase_different_application_membership() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    fn empty(application: u32) -> ConstraintApplicationScope<Domain> {
        ConstraintApplicationScope::test_root(application, TypeConstraintParameterScope::empty())
    }

    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 2048);
    let root = context.start_path(empty(0), &mut effect_graph).unwrap();
    let first_seed = context.fork_path(&root).unwrap();
    let first = context
        .admit_application(first_seed, empty(1), &mut effect_graph)
        .unwrap();
    let second_seed = context.fork_path(&root).unwrap();
    let second = context
        .admit_application(second_seed, empty(2), &mut effect_graph)
        .unwrap();
    let same = context.fork_path(&first).unwrap();
    let application = first
        .applications
        .applications()
        .find(|scope| scope.application() == 1)
        .unwrap();
    let reused = ConstraintApplicationScope::test_root(
        1,
        application.parameters().clone(),
        &mut effect_graph,
        &mut context,
    );
    let equivalent_seed = context.fork_path(&root).unwrap();
    let equivalent = context
        .admit_application(equivalent_seed, reused, &mut effect_graph)
        .unwrap();
    let mut complete = |path: ConstraintPath<Domain>, context: &mut Context<'_>| {
        crate::types::constraints::transaction::TypeConstraintTransaction::from_path(
            context,
            &mut effect_graph,
            path.root_id(),
            path,
            None,
        )
        .unwrap()
        .finish(context, &mut effect_graph, |left, right, _| {
            Ok(left == right)
        })
        .unwrap()
        .component
    };
    let root = complete(root, &mut context);
    let first = complete(first, &mut context);
    let second = complete(second, &mut context);
    let same = complete(same, &mut context);
    let equivalent = complete(equivalent, &mut context);
    assert!(!root.equal_with(&first, &mut context).unwrap());
    assert!(!first.equal_with(&second, &mut context).unwrap());
    assert!(first.equal_with(&same, &mut context).unwrap());
    assert!(first.equal_with(&equivalent, &mut context).unwrap());
}
