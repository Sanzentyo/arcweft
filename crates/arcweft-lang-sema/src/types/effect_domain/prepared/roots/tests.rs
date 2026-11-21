use super::*;
use crate::{
    effect_row::{
        DecisionControl, DecisionWork,
        prepared::{PreparedEffectFailure, PreparedEffectGraph, PreparedEffectInvariant},
    },
    effects::EffectSet,
    types::GenericBinder,
};

#[derive(Debug, thiserror::Error)]
#[error("original work exhausted")]
struct Exhausted;
struct Control(usize);
impl Control {
    fn take(&mut self) -> Result<(), Exhausted> {
        self.0 = self.0.checked_sub(1).ok_or(Exhausted)?;
        Ok(())
    }
}
impl TypeProjectionControl for Control {
    type Error = Exhausted;
    fn check(&mut self) -> Result<(), Exhausted> {
        self.take()
    }
    fn visit_node(&mut self, _: TypeProjectionNodeKind, _: u64) -> Result<(), Exhausted> {
        self.take()
    }
    fn visit_binding(&mut self) -> Result<(), Exhausted> {
        self.take()
    }
}
impl DecisionControl for Control {
    type Error = Exhausted;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Exhausted> {
        self.take()
    }
}
fn truth() -> PreparedEffectPredicate {
    PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
}
fn function(
    row: PreparedEffectRow,
    result: TypeKind<PreparedEffects>,
) -> TypeKind<PreparedEffects> {
    TypeKind::function_with_contract(GenericBinder::EMPTY, truth(), [], result, row)
}

#[test]
fn nested_headers_retain_the_actual_unprepared_named_input() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = Control(1_000_000);
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let call = graph.application(&named, vec![], &mut control).unwrap();
    let empty = PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new()));
    let ty = TypeKind::Tuple(vec![TypeKind::Array {
        item: Box::new(function(
            call.clone(),
            TypeKind::Option(Box::new(function(empty.clone(), TypeKind::I64))),
        )),
        len: ArrayLength::Const(2),
    }]);
    let extra = graph
        .relate(
            crate::effect_row::prepared::PreparedEffectRelation::Equal,
            call.clone(),
            empty.clone(),
            &mut control,
        )
        .unwrap();
    let roots = PreparedTypeEffectRoots::collect([&ty], [&extra], &mut control).unwrap();
    assert_eq!(roots.rows(), &[&call]);
    assert_eq!(roots.predicates(), &[&extra]);
    let frozen = graph
        .read()
        .freeze_scoped(roots.rows(), roots.predicates(), &[], &mut control)
        .unwrap();
    assert!(matches!(
        frozen
            .scheme(&named)
            .unwrap()
            .residual()
            .function
            .role()
            .unwrap(),
        crate::effect_row::prepared::ResidualFunctionRole::SchemeInput { .. }
    ));
    // Collection and freeze must leave the live declaration unprepared.
    graph
        .prepare_body(&named, empty, vec![], &mut control)
        .unwrap();
}

#[test]
fn retained_application_keeps_its_arguments_and_independently_observed_use() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = Control(1_000_000);
    let argument = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let operand = graph.application(&argument, vec![], &mut work).unwrap();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let scheme = graph
        .declare(
            vec![scope.bound_effect(0, 0).unwrap()],
            EffectPredicate::unconstrained(),
            &mut work,
        )
        .unwrap();
    let declaration = graph.declaration(&scheme, &mut work).unwrap();
    let effect = graph
        .application(&scheme, vec![operand.clone()], &mut work)
        .unwrap();
    let result = function(effect.clone(), TypeKind::I64);
    let port = PreparedNamedEffectPort::for_type(&declaration, 0, &result, graph.read(), &mut work)
        .unwrap()
        .unwrap();
    let mut roots = PreparedTypeEffectRoots::default();
    roots
        .extend_type(
            &result,
            port.application(&result, graph.read(), &mut work).unwrap(),
            &mut work,
        )
        .unwrap();
    assert_eq!(roots.rows(), &[&operand]);
    assert!(roots.has_effect_inputs());
    assert!(roots.predicates().is_empty());
    assert!(matches!(
        graph.read().require_query_bodies(roots.rows(), roots.predicates(), &mut work),
        Err(PreparedEffectFailure::ObservationNeedsBody(reference)) if reference == argument
    ));
    graph
        .prepare_body(
            &argument,
            PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new())),
            vec![],
            &mut work,
        )
        .unwrap();

    let with_callback = TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        truth(),
        [result],
        TypeKind::I64,
        effect.clone(),
    );
    let mut roots = PreparedTypeEffectRoots::default();
    roots
        .extend_type(
            &with_callback,
            port.application(&with_callback, graph.read(), &mut work)
                .unwrap(),
            &mut work,
        )
        .unwrap();
    assert_eq!(roots.rows(), &[&operand, &effect]);
    assert!(roots.predicates().is_empty());
    assert!(matches!(
        graph.read().require_query_bodies(roots.rows(), roots.predicates(), &mut work),
        Err(PreparedEffectFailure::ObservationNeedsBody(reference)) if reference == scheme
    ));
}

#[test]
fn canonical_inputs_still_signal_effect_observation_and_omitted_rows() {
    let mut control = Control(4096);
    let known = function(
        PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new())),
        TypeKind::I64,
    );
    let omitted = function(
        PreparedEffectRow::Canonical(EffectRow::unknown()),
        known.clone(),
    );
    let scalar = TypeKind::I64;
    let empty = PreparedTypeEffectRoots::collect([&scalar], [], &mut control).unwrap();
    assert!(!empty.has_effect_inputs());
    let known_roots = PreparedTypeEffectRoots::collect([&known], [], &mut control).unwrap();
    assert!(known_roots.has_effect_inputs());
    assert!(!known_roots.has_terms());
    assert!(!known_roots.has_omitted_rows());
    assert!(
        omitted
            .has_omitted_function_effect_rows_with_control(&mut control)
            .unwrap()
    );
    let omitted_roots = PreparedTypeEffectRoots::collect([&omitted], [], &mut control).unwrap();
    assert!(omitted_roots.has_effect_inputs());
    assert!(!omitted_roots.has_terms());
}

#[test]
fn root_collection_cannot_lend_a_foreign_term_to_the_receiving_graph() {
    let graph = PreparedEffectGraph::default();
    let mut foreign = PreparedEffectGraph::default();
    let mut control = Control(1_000_000);
    let row = foreign
        .union(
            vec![PreparedEffectRow::Canonical(EffectRow::closed(
                EffectSet::new(),
            ))],
            &mut control,
        )
        .unwrap();
    let ty = function(row, TypeKind::I64);
    let roots = PreparedTypeEffectRoots::collect([&ty], [], &mut control).unwrap();
    assert!(matches!(
        graph
            .read()
            .freeze_scoped(roots.rows(), roots.predicates(), &[], &mut control),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
}

#[test]
fn exhausted_collection_never_returns_a_partial_inventory_or_edits_the_graph() {
    let graph = PreparedEffectGraph::default();
    let ty = function(
        PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new())),
        TypeKind::Tuple(vec![TypeKind::Bool, TypeKind::I64]),
    );
    let mut measured = Control(4096);
    PreparedTypeEffectRoots::collect([&ty], [], &mut measured).unwrap();
    let required = 4096 - measured.0;
    for available in 0..required {
        let mut control = Control(available);
        assert!(matches!(
            PreparedTypeEffectRoots::collect([&ty], [], &mut control),
            Err(TypeProjectionError::Control(Exhausted))
        ));
        assert_eq!(control.0, 0);
        assert_eq!(graph.retained_terms(), (0, 0));
    }
}
