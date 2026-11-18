use super::*;
use crate::types::constraints::transaction::materialization::MaterializationInput;
use crate::{
    effect_row::{
        EffectPredicate, EffectRow,
        prepared::{
            BoundScopedEffectCompletion, PreparedEffectGraph, PreparedEffectPredicate,
            PreparedEffectRow,
        },
    },
    effects::EffectSet,
    types::{
        GenericBinder, MapKind, PreparedEffects, ScopedTypeView,
        constraints::{
            ConstraintAcceptance, ConstraintSourceContainerPolicy, NoConstraintClient,
            TypeConstraintInvariant,
            context::{LocalConstraintAccounting, TypeConstraintLimits},
            test_support::ConstraintTestSetup,
            transaction::{
                ConstraintEquationId, ConstraintEquationTiming, ConstraintPath, PendingEquation,
            },
        },
        effect_domain::PreparedTypeEffectRoots,
    },
};
use std::sync::atomic::AtomicBool;

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;
type Path = ConstraintPath<NoConstraintClient>;

fn setup<'a>(
    cancellation: &'a AtomicBool,
    binder: GenericBinder,
    graph: &mut PreparedEffectGraph,
) -> (Context<'a>, Path) {
    let (mut context, parameters) = ConstraintTestSetup::new(
        TypeConstraintLimits::new(131_072, 65_536, 1024, 1024),
        cancellation,
    )
    .into_parts();
    context.enter_binder_scope(binder);
    let application = context
        .fixture_application_scope((), parameters, graph)
        .unwrap();
    let path = context.start_path(application, graph).unwrap();
    (context, path)
}

fn original(
    path: &Path,
    values: &[&TypeKind<PreparedEffects>],
    graph: &mut PreparedEffectGraph,
    context: &mut Context<'_>,
) -> BoundScopedEffectCompletion {
    let receipt = path.prepare_effect_constraint(graph, context).unwrap();
    let roots =
        PreparedTypeEffectRoots::collect(values.iter().copied(), std::iter::empty(), context)
            .unwrap();
    let frozen = graph
        .read()
        .freeze_scoped(roots.rows(), roots.predicates(), &[&receipt], context)
        .unwrap();
    graph
        .bind_original_scoped(&frozen, context)
        .unwrap()
        .freeze(graph.read(), context)
        .unwrap()
}

fn function(row: PreparedEffectRow) -> TypeKind<PreparedEffects> {
    let binder = GenericBinder::new(1, 1, 0);
    let scope = crate::types::GenericScope::default().with_binder(binder);
    TypeKind::function_with_contract(
        binder,
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        [TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(scope.bound_type(0, 0).unwrap())),
            len: crate::types::ArrayLength::Generic(scope.bound_const(0, 0).unwrap()),
        }],
        TypeKind::Unit,
        row,
    )
}

#[test]
fn canonical_source_ports_share_one_completion_without_rewriting_original_c() {
    let cancellation = AtomicBool::new(false);
    let mut graph = PreparedEffectGraph::default();
    let (mut context, mut path) = setup(&cancellation, GenericBinder::EMPTY, &mut graph);
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let actual_row = EffectRow::closed(EffectSet::from_labels(["body.actual"]).unwrap());
    graph
        .prepare_body(
            &named,
            PreparedEffectRow::Canonical(actual_row.clone()),
            vec![],
            &mut context,
        )
        .unwrap();
    let actual_function = function(graph.application(&named, vec![], &mut context).unwrap());
    let expected_function = function(PreparedEffectRow::Canonical(EffectRow::closed(
        EffectSet::from_labels(["body.actual", "allowed.extra"]).unwrap(),
    )));
    let actual = TypeKind::Map {
        kind: MapKind::Ordered,
        key: Box::new(TypeKind::String),
        value: Box::new(actual_function),
    };
    let expected = TypeKind::Map {
        kind: MapKind::Ordered,
        key: Box::new(TypeKind::String),
        value: Box::new(expected_function.clone()),
    };
    let original = original(&path, &[&actual, &expected], &mut graph, &mut context);
    let effects =
        ConstraintClosedEffects::for_path(&path, &original, &[], &mut graph, &mut context).unwrap();
    let application = path.root_id();
    let ordinal = PreparedSourceOrdinal {
        application,
        ordinal: 0,
    };
    let actual_operand = context.fixture_operand(actual);
    let pattern = context.fixture_operand(expected);
    let before_actual = actual_operand.clone();
    let before_pattern = pattern.clone();
    path.equations.push(PendingEquation {
        ordinal: ConstraintEquationId {
            application,
            ordinal: 0,
        },
        direction: ConstraintAcceptance::PatternAcceptsActual,
        pattern,
        actual: actual_operand.clone(),
        source_ordinal: Some(ordinal),
        timing: ConstraintEquationTiming::Immediate,
    });
    path.lineage.probe_trace.push(ActiveConstraintProbe {
        source: ConstraintSourceId::new(application, ()),
        source_ordinal: ordinal,
        branch: SourceProbeObservation::closed_for_test((), ()),
        selection: StoredSourceSelection::Checked {
            alternative: (),
            evidence: Arc::new(()),
        },
        prepared_source_projection: PreparedConstraintSourceProjection::InferSpreadContainer {
            policy: ConstraintSourceContainerPolicy::Named,
        },
        value_expected: Some(context.fixture_operand(expected_function)),
        actual: actual_operand,
    });
    let (sources, _) = path
        .lineage
        .close_trace(&path, &mut effects.projection(), graph.read(), &mut context)
        .unwrap();
    let closed = &sources[0];
    closed.require_completion(&effects).unwrap();
    assert!(std::ptr::eq(
        path.lineage.probe_trace[0].actual.value(),
        before_actual.value()
    ));
    assert!(std::ptr::eq(
        path.equations[0].actual.value(),
        before_actual.value()
    ));
    assert!(std::ptr::eq(
        path.equations[0].pattern.value(),
        before_pattern.value()
    ));
    let TypeKind::Map { value, .. } = closed.actual() else {
        panic!("canonical map")
    };
    let TypeKind::Function {
        effects: row,
        binder,
        ..
    } = value.as_ref()
    else {
        panic!("canonical function")
    };
    assert_eq!(row, &actual_row);
    assert_eq!(*binder, GenericBinder::new(1, 1, 0));
    assert!(closed.source_projection().matches_actual(closed.actual()));
    path.equations[0]
        .validate_completed_source(
            closed,
            &path,
            &mut effects.projection(),
            graph.read(),
            &mut context,
        )
        .unwrap();
    let other =
        ConstraintClosedEffects::for_path(&path, &original, &[], &mut graph, &mut context).unwrap();
    assert!(matches!(
        path.equations[0].validate_completed_source(
            closed,
            &path,
            &mut other.projection(),
            graph.read(),
            &mut context
        ),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::OriginalProducer
        ))
    ));
    closed.require_completion(&effects).unwrap();
    let component = MaterializationInput::from_closed_original(
        &path,
        application,
        &path,
        &mut effects.projection(),
        graph.read(),
        &mut context,
    )
    .unwrap()
    .component;
    assert_eq!(component.sources().all()[0].actual(), closed.actual());
    assert_eq!(
        component.sources().all()[0].final_expected(),
        closed.final_expected()
    );
}

#[test]
fn a_canonical_source_transport_keeps_real_frames_and_its_source_authority() {
    let cancellation = AtomicBool::new(false);
    let mut graph = PreparedEffectGraph::default();
    let (mut context, path) = setup(&cancellation, GenericBinder::new(1, 0, 0), &mut graph);
    let ambient = context.lexical_scope().bound_type(0, 0).unwrap();
    let value = TypeKind::Tuple(vec![
        TypeKind::GenericParam(ambient),
        function(PreparedEffectRow::Canonical(EffectRow::closed(
            EffectSet::new(),
        ))),
    ]);
    let original = original(&path, &[&value], &mut graph, &mut context);
    let effects =
        ConstraintClosedEffects::for_path(&path, &original, &[], &mut graph, &mut context).unwrap();
    let application = path.root_id();
    let source = ActiveConstraintProbe {
        source: ConstraintSourceId::new(application, ()),
        source_ordinal: PreparedSourceOrdinal {
            application,
            ordinal: 0,
        },
        branch: SourceProbeObservation::closed_for_test((), ()),
        selection: StoredSourceSelection::Unchecked,
        prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
        value_expected: None,
        actual: context.fixture_operand(value),
    }
    .close(&path, &mut effects.projection(), graph.read(), &mut context)
    .unwrap();
    let source_context = source.actual_operand().original_context().clone();
    let old_scope = source.actual_operand().scope().clone();
    let previous = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
    let target = Arc::new(context.capture_lexical_scope().unwrap());
    let transported = source
        .actual_operand()
        .transport_canonical(&target, &mut context)
        .unwrap();
    assert_ne!(&old_scope, transported.scope());
    assert_eq!(transported.scope(), target.as_ref());
    assert_eq!(transported.original_context(), &source_context);
    let TypeKind::Tuple(parts) = transported.value() else {
        panic!("tuple")
    };
    assert_eq!(
        parts[0],
        TypeKind::GenericParam(target.shape().bound_type(1, 0).unwrap())
    );
    let TypeKind::Function { binder, params, .. } = &parts[1] else {
        panic!("function")
    };
    let nested = target.shape().with_binder(*binder);
    assert_eq!(
        params[0],
        TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(nested.bound_type(0, 0).unwrap())),
            len: crate::types::ArrayLength::Generic(nested.bound_const(0, 0).unwrap())
        }
    );
    ScopedTypeView::sealed(transported.value(), target.shape())
        .semantic_identity_digest_with_control(&mut context)
        .unwrap();
    context.restore_binder_scope(previous);
    assert_eq!(source.actual_operand().scope(), &old_scope);
}
