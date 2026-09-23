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
            normalization::seal_path,
            relate_selected_call,
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

fn scope(application: u32) -> ConstraintApplicationScope<Domain> {
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
    ConstraintApplicationScope::new(application, parameters)
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
    path: ConstraintPath<Domain>,
    left: &TypeKind,
    right: &TypeKind,
    context: &mut Context<'_>,
) -> ConstraintPath<Domain> {
    let mut paths = relate_selected_call(
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
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 2048);
    let parent = scope(0);
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
    let child = scope(1);
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
    let path = context.start_path(parent).unwrap();
    let path = context.admit_application(path, child).unwrap();
    let path = relation(path, &child_array, &parent_array, &mut context);
    let mut path = relation(
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
        .constrain_subset(&expected_row, &parent_row, &mut context)
        .unwrap();
    path.effects
        .constrain_subset(&parent_row, &expected_row, &mut context)
        .unwrap();
    path.effects
        .constrain_subset(&parent_row, &child_row, &mut context)
        .unwrap();
    let path = seal_path(path, &mut context).unwrap();
    for (application, effect) in [(parent_id, parent_effect_key), (child_id, child_effect_key)] {
        let solution =
            TypeConstraintSolution::complete_application(&path, application, &mut context)
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
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 4096);
    let parent = scope(0);
    let parent_id = parent.id();
    let child = scope(1);
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
    let path = context.start_path(parent).unwrap();
    let path = relation(path, &parent_array, &parent_value, &mut context);
    let mut parent_transaction =
        crate::types::constraints::transaction::TypeConstraintTransaction::from_path(
            &mut context,
            parent_id,
            path,
            None,
        )
        .unwrap();
    parent_transaction.request_projection(
        &mut context,
        (),
        &parent_array,
        crate::types::constraints::TypeConstraintProjectionClosure::Closed,
    );
    let path = context
        .fork_path(parent_transaction.test_single_path())
        .unwrap();
    let path = context.admit_application(path, child).unwrap();
    let mut transaction =
        crate::types::constraints::transaction::TypeConstraintTransaction::from_path(
            &mut context,
            child_id,
            path,
            None,
        )
        .unwrap();
    transaction.constrain(
        &mut context,
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
    let mut ticket = transaction.next_probe(&mut context).unwrap().unwrap();
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
            ticket.input(),
            crate::types::constraints::transaction::ProbeSubmission::Accepted(
                ticket
                    .observe(crate::types::constraints::SourceProbeResult::checked(
                        child_value.clone(),
                        (),
                        (),
                        (),
                    ))
                    .expect("observed child source"),
            ),
        )
        .unwrap();
    assert!(transaction.next_probe(&mut context).unwrap().is_none());
    transaction.request_projection(
        &mut context,
        (),
        &child_template,
        crate::types::constraints::TypeConstraintProjectionClosure::Closed,
    );
    let mut materialization = transaction
        .next_materialization_ticket(&mut context)
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
            .next_materialization_ticket(&mut context)
            .unwrap()
            .is_none()
    );
    let solved = transaction.finish(&mut context).unwrap();
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
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 8192);
    let prior = scope(1);
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
    let prior_path = context.start_path(prior).unwrap();
    let mut prior_path = relation(prior_path, &prior_array, &value, &mut context);
    prior_path
        .effects
        .constrain_subset(
            &row,
            &EffectRow::open(EffectSet::new(), effect.clone()),
            &mut context,
        )
        .unwrap();
    prior_path
        .effects
        .constrain_subset(
            &EffectRow::open(EffectSet::new(), effect.clone()),
            &row,
            &mut context,
        )
        .unwrap();
    let prior_path = seal_path(prior_path, &mut context).unwrap();
    let inherited =
        TypeConstraintSolution::complete_application(&prior_path, prior_id, &mut context).unwrap();

    let child = ConstraintApplicationScope::new(
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
    );
    let child_id = child.id();
    let child_array = array(&child);
    let parent = scope(0);
    let parent_array = array(&parent);
    let parent_value = TypeKind::Array {
        item: Box::new(TypeKind::I64),
        len: ArrayLength::Const(1),
    };
    let path = context.start_path(parent).unwrap();
    let path = relation(path, &parent_array, &parent_value, &mut context);
    let path = context.admit_application(path, child).unwrap();
    let without_inherited = context.fork_path(&path).unwrap();
    assert!(matches!(
        crate::types::constraints::transaction::TypeConstraintTransaction::from_path(
            &mut context,
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
            child_id,
            path,
            Some(&inherited),
        )
        .unwrap();
    assert_eq!(
        crate::types::constraints::seal_type(
            &parent_array,
            transaction.test_single_path().projection_view(),
            &mut BTreeSet::new(),
            &mut context,
        )
        .unwrap(),
        parent_value
    );
    transaction.request_projection(
        &mut context,
        (),
        &child_array,
        crate::types::constraints::TypeConstraintProjectionClosure::Closed,
    );
    let solved = transaction.finish(&mut context).unwrap();
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
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 4096);
    let path = context.start_path(scope(0)).unwrap();
    let foreign = scope(1);
    assert!(matches!(
        crate::types::constraints::transaction::TypeConstraintTransaction::from_path(
            &mut context,
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
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 2048);
    let root = context.start_path(scope(0)).unwrap();
    let sibling = context.fork_path(&root).unwrap();
    let child = scope(1);
    let child_type = child
        .parameters()
        .type_reference(child.parameters().iter().next().unwrap().0)
        .unwrap();
    let child_const = child
        .parameters()
        .const_reference(child.parameters().const_iter().next().unwrap().0)
        .unwrap();
    let child_effect = effect(&child);
    let chosen = context.admit_application(root, child).unwrap();
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
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(90_100));
    let type_reference = GenericTypeReference::Free(GenericTypeParameterId::new(owner.clone(), 0));
    let const_reference = GenericConstReference::Free(GenericConstParameterId::new(owner, 0));
    let captured_scope = |application| {
        ConstraintApplicationScope::new(
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
        )
    };
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 2048);
    let root = context.start_path(scope(0)).unwrap();
    let sibling = context.fork_path(&root).unwrap();
    let mut chosen = root;

    // Several applications may capture the same declaration-owned rigid
    // reference. Admission never turns that reference into an inference slot.
    for application in [1, 2] {
        chosen = context
            .admit_application(chosen, captured_scope(application))
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
fn application_identity_and_live_opening_cannot_be_admitted_twice() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 2048);
    let parent = scope(0);
    let reused_opening = ConstraintApplicationScope::new(1, parent.parameters().clone());
    let root = context.start_path(parent).unwrap();
    for duplicate in [scope(0), reused_opening] {
        let sibling = context.fork_path(&root).unwrap();
        assert!(matches!(
            context.admit_application(sibling, duplicate),
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
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 2048);
    let parent = scope(0);
    let child = ConstraintApplicationScope::new(1, scope(0).parameters);
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
    let root = context.start_path(parent).unwrap();
    let sibling = context.fork_path(&root).unwrap();
    let combined = context.admit_application(sibling, child).unwrap();
    assert_eq!(combined.effects.bindings(&mut context).unwrap().len(), 2);
    assert_eq!(root.applications.len(), 1);
    assert_eq!(root.effects.bindings(&mut context).unwrap().len(), 1);
}

#[test]
fn admission_charges_the_shared_context_and_observes_cancellation() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 2);
    let root = context.start_path(scope(0)).unwrap();
    let sibling = context.fork_path(&root).unwrap();
    assert!(matches!(
        context.admit_application(sibling, scope(1)),
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
        context.admit_application(sibling, scope(1)),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
    ));
    assert_eq!(root.applications.len(), 1);
}

#[test]
fn equal_binding_values_do_not_erase_different_application_membership() {
    fn empty(application: u32) -> ConstraintApplicationScope<Domain> {
        ConstraintApplicationScope::new(application, TypeConstraintParameterScope::empty())
    }

    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation, 2048);
    let root = context.start_path(empty(0)).unwrap();
    let first_seed = context.fork_path(&root).unwrap();
    let first = context.admit_application(first_seed, empty(1)).unwrap();
    let second_seed = context.fork_path(&root).unwrap();
    let second = context.admit_application(second_seed, empty(2)).unwrap();
    let same = context.fork_path(&first).unwrap();
    let application = first
        .applications
        .applications()
        .find(|scope| scope.application() == 1)
        .unwrap();
    let reused = ConstraintApplicationScope::new(1, application.parameters().clone());
    let equivalent_seed = context.fork_path(&root).unwrap();
    let equivalent = context.admit_application(equivalent_seed, reused).unwrap();
    let complete = |path: ConstraintPath<Domain>, context: &mut Context<'_>| {
        crate::types::constraints::transaction::TypeConstraintTransaction::from_path(
            context,
            path.applications.root_id(),
            path,
            None,
        )
        .unwrap()
        .finish(context)
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
