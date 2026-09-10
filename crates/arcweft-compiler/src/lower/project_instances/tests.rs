use arcweft_core::entry::RuntimeCallableId;
use arcweft_lang_hir::item::HirItemKind;
use arcweft_lang_hir::source_index::HirCallableSourceOwner;
use arcweft_lang_sema::callable::select_project_function_runtime;
use arcweft_lang_sema::types::{ArrayLength, TypeKind, TypeProjectionError};

use super::*;
use std::sync::atomic::Ordering;

fn instance() -> (RuntimeProjectFunctionInstanceKey, ProjectInstanceNode) {
    let compiled = crate::source::compile_source(
        "fn identity<T>(value: T) -> T { value }\n\
         flow main() -> i64 { return identity(42i64) }\n",
    )
    .expect("checked generic instance fixture");
    let analysis = &compiled.semantic_analysis;
    let (owner, selection) = analysis
        .calls()
        .find_map(|(owner, call)| {
            let application = call.selected_application()?;
            let join = analysis.checked_callable_join(owner).expect("checked join");
            select_project_function_runtime(application, join, analysis.checked_callables())
                .expect("checked runtime selection")
                .map(|selection| (owner, selection))
        })
        .expect("fixture has one selected ordinary function");
    let executable = compiled
        .hir_project
        .analysis_view()
        .expect("executable HIR");
    let function = executable
        .items()
        .find(|item| matches!(item.item().kind(), HirItemKind::Function(_)))
        .expect("fixture has one function");
    let checked = analysis
        .checked_callables()
        .project_callable(selection.declaration())
        .expect("checked callable");
    let callable = RuntimeProjectCallable::try_new(
        selection.declaration().clone(),
        function.id(),
        HirCallableSourceOwner::Item,
        RuntimeCallableId::from_checked_digest(checked.id().semantic_digest().into_bytes()),
        None,
    )
    .expect("closed callable descriptor");
    let solution = selection.close_instance(None).expect("closed substitution");
    let key = RuntimeProjectFunctionInstanceKey::new(
        callable.runtime().clone(),
        solution.instantiation(),
        selection.group(),
    );
    (
        key,
        ProjectInstanceNode {
            origin: ProjectInstantiationOrigin::Call(owner),
            callable,
            selection: ProjectInstanceSelection::from_call(&selection),
            solution,
        },
    )
}

#[test]
fn instance_key_encoding_spends_the_same_projection_budget() {
    let compiled = crate::source::compile_source(
        "fn identity<T>(value: T) -> T { value }\nflow main() -> i64 { return identity(42i64) }\n",
    )
    .expect("generic invocation fixture");
    let analysis = &compiled.semantic_analysis;
    let (owner, selection) = analysis
        .calls()
        .find_map(|(owner, call)| {
            let application = call.selected_application()?;
            let join = analysis.checked_callable_join(owner).expect("checked join");
            select_project_function_runtime(application, join, analysis.checked_callables())
                .expect("checked selection")
                .map(|selection| (owner, selection))
        })
        .expect("one generic invocation");
    let origin = ProjectInstantiationOrigin::Call(owner);
    assert_eq!(
        selection.solution().effect_bindings().len(),
        1,
        "the checked pure invocation still owns an effect binding"
    );
    // Closing the type/effect bindings and function type consumes six visits.
    // Encoding the type key/value and the effect row identity needs five more.
    for (limit, attempted) in [(6, 7), (8, 9), (10, 11)] {
        let mut session = ProjectInstantiationSession::new(
            ProjectInstantiationControl::default()
                .with_limits(ProjectInstantiationLimits::new(1, 0, limit, 128, 100)),
        );
        assert!(matches!(
            ProjectInstanceProjection::Discover(&mut session).close_instance(origin, &selection, None),
            Err(RuntimeSemanticProjectionError::ProjectInstantiation(ProjectInstantiationError::LimitExceeded {
                kind: ProjectInstantiationLimitKind::StructuralNodes, limit: actual_limit, attempted: actual_attempted, origin: actual_origin,
            })) if actual_limit == limit && actual_attempted == attempted && actual_origin == origin
        ));
        assert!(
            session.seal().is_err(),
            "failed key encoding cannot seal a graph"
        );
    }
    let mut exact = ProjectInstantiationSession::new(
        ProjectInstantiationControl::default()
            .with_limits(ProjectInstantiationLimits::new(1, 0, 11, 128, 100)),
    );
    let closed = ProjectInstanceProjection::Discover(&mut exact)
        .close_instance(origin, &selection, None)
        .expect("exact bound admits the canonical key");
    assert_eq!(
        closed,
        selection
            .close_instance(None)
            .expect("same closed instance")
    );
    assert_eq!(exact.work.counters().structural_nodes, 11);
    assert_eq!(exact.work.counters().work, 15);
}

#[test]
fn structural_projection_charges_constants_and_rejects_the_next_occurrence() {
    let (_, node) = instance();
    let session = ProjectInstantiationSession::new(
        ProjectInstantiationControl::default()
            .with_limits(ProjectInstantiationLimits::new(1, 0, 3, 2, 100)),
    );
    let ty = TypeKind::Array {
        item: Box::new(TypeKind::Bool),
        len: ArrayLength::Const(3),
    };
    let mut control = session.work.type_control(node.origin);
    assert_eq!(
        node.solution
            .instantiate_type_with_control(&ty, &mut control)
            .expect("exact structural bound"),
        ty,
    );
    assert_eq!(session.work.counters().structural_nodes, 3);
    assert_eq!(session.work.counters().type_depth, 2);
    assert!(matches!(
        node.solution.instantiate_type_with_control(&TypeKind::Unit, &mut control),
        Err(TypeProjectionError::Control(ProjectInstantiationError::LimitExceeded {
            kind: ProjectInstantiationLimitKind::StructuralNodes, limit: 3, attempted: 4, origin,
        })) if origin == node.origin
    ));
    assert_eq!(session.work.counters().structural_nodes, 3);
    assert!(matches!(
        session.seal(),
        Err(ProjectInstantiationError::LimitExceeded {
            kind: ProjectInstantiationLimitKind::StructuralNodes,
            limit: 3,
            attempted: 4,
            ..
        })
    ));
}

#[test]
fn projection_depth_failure_retains_the_precise_origin_and_first_abort() {
    let (_, node) = instance();
    let session = ProjectInstantiationSession::new(
        ProjectInstantiationControl::default()
            .with_limits(ProjectInstantiationLimits::new(1, 0, 100, 1, 100)),
    );
    let ty = TypeKind::Vec(Box::new(TypeKind::Bool));
    let mut control = session.work.type_control(node.origin);
    assert!(matches!(
        node.solution.instantiate_type_with_control(&ty, &mut control),
        Err(TypeProjectionError::Control(ProjectInstantiationError::LimitExceeded {
            kind: ProjectInstantiationLimitKind::TypeDepth, limit: 1, attempted: 2, origin,
        })) if origin == node.origin
    ));
    assert_eq!(session.work.counters().structural_nodes, 1);
    assert_eq!(session.work.counters().type_depth, 1);
    assert!(matches!(
        node.solution
            .instantiate_type_with_control(&TypeKind::Unit, &mut control),
        Err(TypeProjectionError::Control(
            ProjectInstantiationError::LimitExceeded {
                kind: ProjectInstantiationLimitKind::TypeDepth,
                limit: 1,
                attempted: 2,
                ..
            }
        ))
    ));
}

#[test]
fn sealed_graph_materialization_uses_the_discovery_work_ledger() {
    let (key, node) = instance();
    let mut session = ProjectInstantiationSession::new(
        ProjectInstantiationControl::default()
            .with_limits(ProjectInstantiationLimits::new(1, 0, 100, 128, 7)),
    );
    session.request(key, node.clone()).expect("admit one root");
    let work = session
        .next()
        .expect("queue")
        .expect("one pending instance");
    session.complete(work).expect("complete discovery");
    let graph = session.seal().expect("seal graph after four work units");
    let ty = TypeKind::Array {
        item: Box::new(TypeKind::Bool),
        len: ArrayLength::Const(3),
    };
    let mut control = graph.work.type_control(node.origin);
    node.solution
        .instantiate_type_with_control(&ty, &mut control)
        .expect("three more visits fit");
    assert_eq!(graph.work.counters().work, 7);
    assert!(matches!(
        node.solution
            .instantiate_type_with_control(&TypeKind::Unit, &mut control),
        Err(TypeProjectionError::Control(
            ProjectInstantiationError::LimitExceeded {
                kind: ProjectInstantiationLimitKind::Work,
                limit: 7,
                attempted: 8,
                ..
            }
        ))
    ));
    assert!(matches!(
        graph.check_cancelled(node.origin),
        Err(ProjectInstantiationError::LimitExceeded {
            kind: ProjectInstantiationLimitKind::Work,
            limit: 7,
            attempted: 8,
            ..
        })
    ));
}

#[test]
fn pending_work_prevents_graph_publication() {
    let (key, node) = instance();
    let mut session = ProjectInstantiationSession::new(ProjectInstantiationControl::default());
    session.request(key, node).expect("admit instance");
    assert!(matches!(
        session.seal(),
        Err(ProjectInstantiationError::IncompleteDiscovery)
    ));
}

#[test]
fn dropping_in_flight_work_cannot_publish_or_skip_it() {
    let (key, node) = instance();
    let mut session = ProjectInstantiationSession::new(ProjectInstantiationControl::default());
    session.request(key, node).expect("admit instance");
    drop(
        session
            .next()
            .expect("next work")
            .expect("one pending instance"),
    );
    assert!(matches!(
        session.next(),
        Err(ProjectInstantiationError::ActiveDiscovery)
    ));
    assert!(matches!(
        session.seal(),
        Err(ProjectInstantiationError::ActiveDiscovery)
    ));
}

#[test]
fn recursive_and_repeated_requests_share_the_admitted_instance() {
    let (key, node) = instance();
    let mut session = ProjectInstantiationSession::new(ProjectInstantiationControl::default());
    session
        .request(key.clone(), node.clone())
        .expect("admit instance");
    session
        .request(key.clone(), node.clone())
        .expect("repeat root");
    let work = session
        .next()
        .expect("next work")
        .expect("one pending instance");
    session.request(key, node).expect("recursive dependency");
    session.complete(work).expect("complete the only work item");
    assert!(session.next().expect("drained queue").is_none());
    assert_eq!(session.seal().expect("complete graph").nodes().count(), 1);
}

#[test]
fn work_from_another_session_is_rejected_even_for_the_same_key() {
    let (key, node) = instance();
    let mut first = ProjectInstantiationSession::new(ProjectInstantiationControl::default());
    let mut second = ProjectInstantiationSession::new(ProjectInstantiationControl::default());
    first
        .request(key.clone(), node.clone())
        .expect("first admission");
    second.request(key, node).expect("second admission");
    let first_work = first.next().expect("first queue").expect("first work");
    let second_work = second.next().expect("second queue").expect("second work");
    assert!(matches!(
        first.complete(second_work),
        Err(ProjectInstantiationError::ForeignWorkItem)
    ));
    first
        .complete(first_work)
        .expect("own completion permission");
    assert!(first.seal().is_ok());
    assert!(matches!(
        second.seal(),
        Err(ProjectInstantiationError::ActiveDiscovery)
    ));
}

#[test]
fn materialization_cannot_add_a_target_to_the_sealed_graph() {
    let (key, node) = instance();
    let empty = ProjectInstantiationSession::new(ProjectInstantiationControl::default())
        .seal()
        .expect("empty graph");
    assert!(matches!(
        ProjectInstanceProjection::Materialize {
            graph: &empty,
            caller: Some(&key)
        }
        .request(key.clone(), node.clone()),
        Err(ProjectInstantiationError::UndiscoveredInstance { .. }),
    ));
    let mut session = ProjectInstantiationSession::new(ProjectInstantiationControl::default());
    session
        .request(key.clone(), node.clone())
        .expect("admit instance");
    let work = session
        .next()
        .expect("next work")
        .expect("one pending instance");
    session
        .request(key.clone(), node.clone())
        .expect("self dependency");
    session.complete(work).expect("completed discovery");
    let graph = session.seal().expect("complete graph");
    ProjectInstanceProjection::Materialize {
        graph: &graph,
        caller: Some(&key),
    }
    .request(key.clone(), node)
    .expect("materialization retains a discovered target");
    assert_eq!(graph.nodes().count(), 1);
}

#[test]
fn admitted_target_does_not_authorize_an_undiscovered_dependency() {
    let (key, node) = instance();
    let mut session = ProjectInstantiationSession::new(ProjectInstantiationControl::default());
    session
        .request(key.clone(), node.clone())
        .expect("root instance");
    let work = session.next().expect("queue").expect("one instance");
    session.complete(work).expect("no dependencies");
    let graph = session.seal().expect("complete graph");
    assert!(matches!(
        ProjectInstanceProjection::Materialize { graph: &graph, caller: Some(&key) }.request(key.clone(), node),
        Err(ProjectInstantiationError::UndiscoveredDependency { caller, callee, .. }) if *caller == key && *callee == key
    ));
}

#[test]
fn same_key_requires_equal_closed_evidence_in_both_phases() {
    let (key, node) = instance();
    let mut conflicting = node.clone();
    conflicting.selection.effects = EffectSet::from_labels(["fs.read"]).expect("effect identity");
    let mut session = ProjectInstantiationSession::new(ProjectInstantiationControl::default());
    session
        .request(key.clone(), node.clone())
        .expect("first evidence");
    assert!(matches!(
        session.request(key.clone(), conflicting.clone()),
        Err(ProjectInstantiationError::ConflictingInstance { .. })
    ));
    let work = session.next().expect("queue").expect("one instance");
    session.request(key.clone(), node).expect("self dependency");
    session.complete(work).expect("complete");
    let graph = session.seal().expect("complete graph");
    assert!(matches!(
        ProjectInstanceProjection::Materialize {
            graph: &graph,
            caller: Some(&key)
        }
        .request(key.clone(), conflicting),
        Err(ProjectInstantiationError::ConflictingInstance { .. })
    ));
}

#[test]
fn graph_limits_are_inclusive_and_failure_cannot_publish_partial_work() {
    let (key, node) = instance();
    let zero = ProjectInstantiationControl::default()
        .with_limits(ProjectInstantiationLimits::new(0, 0, 0, 0, 0));
    assert!(ProjectInstantiationSession::new(zero).seal().is_ok());
    // A single node costs one visit and three state transitions.
    let mut exact = ProjectInstantiationSession::new(
        ProjectInstantiationControl::default().with_limits(ProjectInstantiationLimits::new(
            1,
            0,
            u64::MAX,
            128,
            4,
        )),
    );
    exact
        .request(key.clone(), node.clone())
        .expect("at instance bound");
    let work = exact.next().expect("queue").expect("one instance");
    exact.complete(work).expect("at work bound");
    assert!(exact.next().expect("empty queue costs no work").is_none());
    assert!(exact.seal().is_ok());

    for (limits, kind, limit, attempted) in [
        (
            ProjectInstantiationLimits::new(0, 1, u64::MAX, 128, 10),
            ProjectInstantiationLimitKind::Instances,
            0,
            1,
        ),
        (
            ProjectInstantiationLimits::new(1, 1, u64::MAX, 128, 1),
            ProjectInstantiationLimitKind::Work,
            1,
            2,
        ),
    ] {
        let mut session = ProjectInstantiationSession::new(
            ProjectInstantiationControl::default().with_limits(limits),
        );
        assert!(
            matches!(session.request(key.clone(), node.clone()), Err(ProjectInstantiationError::LimitExceeded { kind: actual_kind, limit: actual_limit, attempted: actual_attempted, origin }) if actual_kind == kind && actual_limit == limit && actual_attempted == attempted && origin == node.origin)
        );
        assert!(session.nodes.is_empty());
        assert!(session.pending.is_empty());
        assert_eq!(
            session.work.counters().work,
            1,
            "the seed visit was performed before admission failed"
        );
        assert!(matches!(
            session.seal(),
            Err(ProjectInstantiationError::LimitExceeded { .. })
        ));
    }
}

#[test]
fn repeated_recursive_edges_charge_work_without_consuming_another_edge_slot() {
    let (key, node) = instance();
    let mut session = ProjectInstantiationSession::new(
        ProjectInstantiationControl::default().with_limits(ProjectInstantiationLimits::new(
            1,
            1,
            u64::MAX,
            128,
            6,
        )),
    );
    session.request(key.clone(), node.clone()).expect("root");
    let work = session.next().expect("queue").expect("one instance");
    session
        .request(key.clone(), node.clone())
        .expect("first recursive edge");
    session
        .request(key.clone(), node)
        .expect("repeat recursive edge");
    assert_eq!(session.edges.len(), 1);
    assert_eq!(session.work.counters().work, 5);
    session.complete(work).expect("sixth work unit");
    assert!(session.seal().is_ok());
}

#[test]
fn a_disallowed_edge_does_not_mutate_the_graph() {
    let (key, node) = instance();
    let mut session = ProjectInstantiationSession::new(
        ProjectInstantiationControl::default().with_limits(ProjectInstantiationLimits::new(
            1,
            0,
            u64::MAX,
            128,
            10,
        )),
    );
    session.request(key.clone(), node.clone()).expect("root");
    let work = session.next().expect("queue").expect("one instance");
    assert!(matches!(
        session.request(key, node),
        Err(ProjectInstantiationError::LimitExceeded {
            kind: ProjectInstantiationLimitKind::Edges,
            limit: 0,
            attempted: 1,
            ..
        })
    ));
    assert_eq!(session.nodes.len(), 1);
    assert!(session.edges.is_empty());
    assert_eq!(
        session.work.counters().work,
        4,
        "the rejected dependency still consumed one visit"
    );
    assert!(matches!(
        session.complete(work),
        Err(ProjectInstantiationError::LimitExceeded { .. })
    ));
    assert!(matches!(
        session.seal(),
        Err(ProjectInstantiationError::LimitExceeded { .. })
    ));
}

#[test]
fn cancellation_is_checked_at_admission_transition_and_publication() {
    let (key, node) = instance();
    for phase in 0..4 {
        let cancellation = Arc::new(AtomicBool::new(false));
        let mut session = ProjectInstantiationSession::new(ProjectInstantiationControl::new(
            Arc::clone(&cancellation),
        ));
        if phase == 0 {
            cancellation.store(true, Ordering::Relaxed);
            assert!(matches!(
                session.request(key.clone(), node.clone()),
                Err(ProjectInstantiationError::Cancelled { .. })
            ));
        } else {
            session.request(key.clone(), node.clone()).expect("root");
            if phase == 1 {
                cancellation.store(true, Ordering::Relaxed);
                assert!(matches!(
                    session.next(),
                    Err(ProjectInstantiationError::Cancelled { .. })
                ));
            } else {
                let work = session.next().expect("queue").expect("one instance");
                if phase == 2 {
                    cancellation.store(true, Ordering::Relaxed);
                    assert!(matches!(
                        session.complete(work),
                        Err(ProjectInstantiationError::Cancelled { .. })
                    ));
                } else {
                    session.complete(work).expect("complete");
                    cancellation.store(true, Ordering::Relaxed);
                }
            }
        }
        assert!(matches!(
            session.seal(),
            Err(ProjectInstantiationError::Cancelled { .. })
        ));
    }
}

#[test]
fn a_zero_work_bound_rejects_the_first_visit() {
    let (key, node) = instance();
    let mut session = ProjectInstantiationSession::new(
        ProjectInstantiationControl::default().with_limits(ProjectInstantiationLimits::new(
            1,
            1,
            u64::MAX,
            128,
            0,
        )),
    );
    assert!(matches!(
        session.request(key, node),
        Err(ProjectInstantiationError::LimitExceeded {
            kind: ProjectInstantiationLimitKind::Work,
            limit: 0,
            attempted: 1,
            ..
        })
    ));
    assert_eq!(session.work.counters().work, 0);
    assert!(session.nodes.is_empty());
}

#[test]
fn mismatched_keys_are_rejected_before_graph_mutation() {
    let (key, node) = instance();
    let wrong_key = RuntimeProjectFunctionInstanceKey::new(
        key.callable().clone(),
        key.instantiation(),
        CallableGroupIndex::try_from_usize(1).expect("group coordinate"),
    );
    let mut session = ProjectInstantiationSession::new(ProjectInstantiationControl::default());
    assert!(matches!(
        session.request(wrong_key, node),
        Err(ProjectInstantiationError::InvalidInstanceKey { .. })
    ));
    assert!(session.nodes.is_empty());
    assert!(session.roots.is_empty());
}

#[test]
fn cancellation_after_discovery_prevents_materialization() {
    let (key, node) = instance();
    let cancellation = Arc::new(AtomicBool::new(false));
    let mut session = ProjectInstantiationSession::new(ProjectInstantiationControl::new(
        Arc::clone(&cancellation),
    ));
    session.request(key.clone(), node.clone()).expect("root");
    let work = session.next().expect("queue").expect("one instance");
    session.complete(work).expect("complete");
    let graph = session.seal().expect("complete graph");
    cancellation.store(true, Ordering::Relaxed);
    assert!(matches!(
        ProjectInstanceProjection::Materialize {
            graph: &graph,
            caller: None
        }
        .request(key, node),
        Err(ProjectInstantiationError::Cancelled { .. })
    ));
}

#[test]
fn an_admitted_instance_does_not_authorize_a_different_root() {
    let (key, node) = instance();
    let mut other_root = node.clone();
    other_root.origin = ProjectInstantiationOrigin::Root(node.callable.owner());
    let mut session = ProjectInstantiationSession::new(ProjectInstantiationControl::default());
    session.request(key.clone(), node).expect("call-site root");
    let work = session.next().expect("queue").expect("one instance");
    session.complete(work).expect("complete");
    let graph = session.seal().expect("complete graph");
    assert!(matches!(
        ProjectInstanceProjection::Materialize {
            graph: &graph,
            caller: None
        }
        .request(key, other_root),
        Err(ProjectInstantiationError::UndiscoveredRoot { .. })
    ));
}

#[test]
fn graph_work_overflow_is_an_abort_before_mutation() {
    let (key, node) = instance();
    let mut session =
        ProjectInstantiationSession::new(ProjectInstantiationControl::default().with_limits(
            ProjectInstantiationLimits::new(1, 1, u64::MAX, 128, u64::MAX),
        ));
    session
        .charge(node.origin, u64::MAX, false, false)
        .expect("prepaid work at the integer boundary");
    assert!(matches!(
        session.request(key, node),
        Err(ProjectInstantiationError::ArithmeticOverflow {
            kind: ProjectInstantiationLimitKind::Work,
            ..
        })
    ));
    assert_eq!(session.work.counters().work, u64::MAX);
    assert!(session.nodes.is_empty());
    assert!(session.roots.is_empty());
    assert!(matches!(
        session.seal(),
        Err(ProjectInstantiationError::ArithmeticOverflow { .. })
    ));
}
