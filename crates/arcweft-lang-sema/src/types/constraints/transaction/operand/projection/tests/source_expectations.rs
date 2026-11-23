use super::*;
use crate::types::MapKind;
use crate::types::constraints::{
    CheckedConstraintContainerConstructor, ConstraintSourceContainerPolicy,
    PreparedConstraintSourceProjection,
};

#[test]
fn a_fixed_source_expectation_keeps_the_callers_rigid_reference_in_the_equation() {
    use crate::types::{
        DetachedGenericOwnerId, GenericConstParameterId, GenericConstReference,
        GenericParameterOwnerId, GenericTypeParameterId,
        constraints::{
            OriginalRigidScope, PreparedSourceAlternative, PreparedSourceConstraint,
            SourceProbeResult, TypeConstraintParameterScope,
            transaction::{
                ProbeSubmission, TypeConstraintTransaction, tests::ApplicationSourceDomain,
            },
        },
    };

    let cancellation = AtomicBool::new(false);
    let (mut context, _) = ConstraintTestSetup::<_, ApplicationSourceDomain>::new(
        TypeConstraintLimits::new(4096, 2048, 128, 128),
        &cancellation,
    )
    .into_parts();
    let mut graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let caller = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(91_087));
    let item: GenericTypeReference = GenericTypeParameterId::new(caller.clone(), 0).into();
    let length: GenericConstReference = GenericConstParameterId::new(caller, 0).into();
    let original = context
        .fixture_original_context_with_rigid(OriginalRigidScope {
            types: std::slice::from_ref(&item),
            consts: std::slice::from_ref(&length),
            effects: &[],
            imported: None,
        })
        .unwrap();
    let application = context
        .application_scope(
            0,
            TypeConstraintParameterScope::empty(),
            original.clone(),
            &mut graph,
        )
        .unwrap();
    let path = context
        .start_path_with_imported(application, None, original.clone(), graph.read())
        .unwrap();
    let application = path.root_id();
    let mut transaction =
        TypeConstraintTransaction::from_path(&mut context, &mut graph, application, path, None)
            .unwrap();
    let actual = Arc::new(TypeKind::Array {
        item: Box::new(TypeKind::GenericParam(item)),
        len: ArrayLength::Generic(length),
    });
    let source = PreparedSourceConstraint::checked(
        0,
        PreparedConstraintSourceProjection::Scalar,
        [],
        PreparedSourceAlternative::from_original(0, (), Arc::clone(&actual)),
    )
    .unwrap();
    transaction
        .begin_prepared_probe(
            &mut context,
            source,
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .unwrap();
    let mut ticket = transaction
        .next_probe(&mut context, &mut graph)
        .unwrap()
        .unwrap();
    let observation = ticket
        .observe(SourceProbeResult::checked(
            actual.as_ref().clone(),
            0,
            0,
            (),
        ))
        .unwrap()
        .close_preparation(());
    transaction
        .submit_probe(
            &mut context,
            &mut graph,
            ticket.input(),
            ProbeSubmission::Accepted(observation),
        )
        .unwrap();
    assert!(
        transaction
            .next_probe(&mut context, &mut graph)
            .unwrap()
            .is_none()
    );
    let path = transaction.test_single_path();
    let expected = path.lineage.probe_trace[0].value_expected.as_ref().unwrap();
    assert!(
        matches!(expected.origin.as_ref(), ConstraintOperandOrigin::Original(owner) if owner == &original)
    );
    assert_eq!(expected.value(), actual.as_ref());
    assert!(path.bindings.is_empty());
    assert!(path.const_bindings.is_empty());
}

#[test]
fn source_expectations_keep_the_actual_header_and_selected_template_value_in_distinct_roles() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let actuals = [
        TypeKind::I64,
        TypeKind::Vec(Box::new(TypeKind::I64)),
        TypeKind::Seq(Box::new(TypeKind::I64)),
        TypeKind::Slice(Box::new(TypeKind::I64)),
        TypeKind::Array {
            item: Box::new(TypeKind::I64),
            len: ArrayLength::Const(7),
        },
        TypeKind::Map {
            kind: MapKind::Sorted,
            key: Box::new(TypeKind::String),
            value: Box::new(TypeKind::I64),
        },
    ];
    for (case, actual) in actuals.into_iter().enumerate() {
        let expected = match case {
            0 => TypeKind::Bool,
            1 => TypeKind::Vec(Box::new(TypeKind::Bool)),
            2 => TypeKind::Seq(Box::new(TypeKind::Bool)),
            3 => TypeKind::Slice(Box::new(TypeKind::Bool)),
            4 => TypeKind::Array {
                item: Box::new(TypeKind::Bool),
                len: ArrayLength::Const(7),
            },
            5 => TypeKind::Map {
                kind: MapKind::Sorted,
                key: Box::new(TypeKind::String),
                value: Box::new(TypeKind::Bool),
            },
            _ => unreachable!(),
        };
        let prepared = match case {
            0 => PreparedConstraintSourceProjection::Scalar,
            5 => PreparedConstraintSourceProjection::InferSpreadContainer {
                policy: ConstraintSourceContainerPolicy::Named,
            },
            _ => PreparedConstraintSourceProjection::InferSpreadContainer {
                policy: ConstraintSourceContainerPolicy::Positional,
            },
        };
        let actual_root = context.fixture_operand(TypeKind::Tuple(vec![actual, TypeKind::U64]));
        let expected_root =
            context.fixture_operand(TypeKind::Tuple(vec![TypeKind::String, TypeKind::Bool]));
        let retained_actual = Arc::downgrade(&actual_root.value);
        let retained_expected = Arc::downgrade(&expected_root.value);
        let actual = actual_root
            .view()
            .children(&mut context)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        let value_expected = expected_root
            .view()
            .children(&mut context)
            .unwrap()
            .nth(1)
            .unwrap()
            .unwrap();
        let projection =
            CheckedConstraintSourceProjection::derive(prepared, actual.value()).unwrap();
        let composed = projection
            .compose_operand(&actual, &value_expected)
            .unwrap();
        assert_eq!(composed.value(), &expected);
        let ConstraintOperandOrigin::SourceExpectation {
            projection: proof,
            actual,
            value_expected,
        } = composed.origin.as_ref()
        else {
            panic!("source header and expected value have explicit, distinct roles")
        };
        assert_eq!(proof, &projection);
        assert!(proof.matches_actual(actual.value()));
        assert_eq!(
            steps(actual),
            [ConstraintOperandProjectionStep::TypeChild(0)]
        );
        assert_eq!(
            steps(value_expected),
            [ConstraintOperandProjectionStep::TypeChild(1)]
        );
        assert_eq!(value_expected.value(), &TypeKind::Bool);
        assert_eq!(
            composed.original_context(),
            value_expected.original_context()
        );
        drop(actual_root);
        drop(expected_root);
        assert!(retained_actual.upgrade().is_some());
        assert!(retained_expected.upgrade().is_some());
        drop(composed);
        assert!(retained_actual.upgrade().is_none());
        assert!(retained_expected.upgrade().is_none());
    }
}

#[test]
fn a_source_expectation_rejects_a_forged_container_header() {
    let cancellation = AtomicBool::new(false);
    let context = context(&cancellation);
    let expected = context.fixture_operand(TypeKind::Bool);
    let array = context.fixture_operand(TypeKind::Array {
        item: Box::new(TypeKind::I64),
        len: ArrayLength::Const(4),
    });
    let map = context.fixture_operand(TypeKind::Map {
        kind: MapKind::Sorted,
        key: Box::new(TypeKind::String),
        value: Box::new(TypeKind::I64),
    });
    for (constructor, actual) in [
        (
            CheckedConstraintContainerConstructor::Array {
                len: ArrayLength::Const(5),
            },
            &array,
        ),
        (
            CheckedConstraintContainerConstructor::MapValue {
                kind: MapKind::Sorted,
                key: Box::new(TypeKind::Bool),
            },
            &map,
        ),
        (
            CheckedConstraintContainerConstructor::MapValue {
                kind: MapKind::Ordered,
                key: Box::new(TypeKind::String),
            },
            &map,
        ),
        (CheckedConstraintContainerConstructor::Vec, &array),
    ] {
        assert!(matches!(
            CheckedConstraintSourceProjection::SpreadContainer(constructor)
                .compose_operand(&actual.view(), &expected.view()),
            Err(TypeConstraintError::Invariant(
                TypeConstraintInvariant::OriginalProducer
            )),
        ));
    }
}

#[test]
fn a_source_expectation_cannot_adopt_an_equal_shaped_foreign_template_frame() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let binder = GenericBinder::new(1, 0, 0);
    let outer = context.enter_binder_scope(binder);
    let actual = context.fixture_operand(TypeKind::I64);
    context.restore_binder_scope(outer);
    let outer = context.enter_binder_scope(binder);
    let expected = context.fixture_operand(TypeKind::Bool);
    context.restore_binder_scope(outer);
    assert_eq!(actual.scope().shape(), expected.scope().shape());
    assert_ne!(actual.scope(), expected.scope());
    assert!(matches!(
        CheckedConstraintSourceProjection::Scalar.compose_operand(&actual.view(), &expected.view()),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::OriginalProducer
        )),
    ));
}
