use super::*;
use crate::{
    effect_row::{DecisionControl, DecisionWork, EffectFormula},
    effects::EffectSet,
    types::{
        AcceptedNominalType, ArrayLength, EntityKind, EntityType, FixedVectorType, GenericBinder,
        GenericEffectReference, TypeInstantiationError, TypePoisonId, VariantPayloadOwnerFamily,
        VariantPayloadType, VariantPayloadTypeShape,
    },
};
use std::convert::Infallible;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct BoxedEffects;

impl sealed::Sealed for BoxedEffects {}

impl TypeEffectDomain for BoxedEffects {
    type Row = Box<EffectRow>;
    type Predicate = Box<EffectPredicate>;
}

#[derive(Debug, thiserror::Error)]
#[error("projection owner stopped")]
struct Stopped;

struct Projection {
    remaining: u64,
    cancelled: bool,
    rows: Vec<usize>,
    predicates: Vec<usize>,
}

impl Projection {
    fn new() -> Self {
        Self {
            remaining: 4096,
            cancelled: false,
            rows: Vec::new(),
            predicates: Vec::new(),
        }
    }

    fn charge(&mut self) -> Result<(), Stopped> {
        self.remaining = self.remaining.checked_sub(1).ok_or(Stopped)?;
        Ok(())
    }
}

impl TypeProjectionControl for Projection {
    type Error = Stopped;

    fn check(&mut self) -> Result<(), Stopped> {
        if self.cancelled { Err(Stopped) } else { Ok(()) }
    }

    fn visit_node(&mut self, _: TypeProjectionNodeKind, _: u64) -> Result<(), Stopped> {
        self.charge()
    }

    fn visit_binding(&mut self) -> Result<(), Stopped> {
        self.charge()
    }
}

impl TypeEffectProjection<CanonicalEffects, BoxedEffects> for Projection {
    fn project_row(
        &mut self,
        row: &EffectRow,
        scope: &GenericScope,
    ) -> Result<Box<EffectRow>, TypeProjectionError<Stopped>> {
        self.rows.push(scope.binders().len());
        Ok(Box::new(row.clone()))
    }

    fn project_predicate(
        &mut self,
        predicate: &EffectPredicate,
        scope: &GenericScope,
    ) -> Result<Box<EffectPredicate>, TypeProjectionError<Stopped>> {
        self.predicates.push(scope.binders().len());
        Ok(Box::new(predicate.clone()))
    }
}

impl TypeEffectProjection<BoxedEffects, CanonicalEffects> for Projection {
    fn project_row(
        &mut self,
        row: &Box<EffectRow>,
        scope: &GenericScope,
    ) -> Result<EffectRow, TypeProjectionError<Stopped>> {
        self.rows.push(scope.binders().len());
        Ok(row.as_ref().clone())
    }

    fn project_predicate(
        &mut self,
        predicate: &Box<EffectPredicate>,
        scope: &GenericScope,
    ) -> Result<EffectPredicate, TypeProjectionError<Stopped>> {
        self.predicates.push(scope.binders().len());
        Ok(predicate.as_ref().clone())
    }
}

struct Decisions;

impl DecisionControl for Decisions {
    type Error = Infallible;

    fn charge(&mut self, _: DecisionWork) -> Result<(), Infallible> {
        Ok(())
    }
}

fn nested_type() -> TypeKind {
    let outer = GenericBinder::new(1, 0, 1);
    let outer_scope = GenericScope::default().with_binder(outer);
    let inner = GenericBinder::new(0, 0, 1);
    let inner_scope = outer_scope.with_binder(inner);
    let restricted = EffectFormula::literal(
        EffectSet::new(),
        Some(inner_scope.bound_effect(0, 0).unwrap()),
    )
    .subset(
        &EffectFormula::literal(EffectSet::from_labels(["fs.read"]).unwrap(), None),
        &mut Decisions,
    )
    .unwrap();
    let function = TypeKind::function_with_contract(
        inner,
        restricted,
        [TypeKind::GenericParam(
            inner_scope.bound_type(1, 0).unwrap(),
        )],
        TypeKind::I64,
        EffectRow::open(
            EffectSet::from_labels(["log.write"]).unwrap(),
            inner_scope.bound_effect(1, 0).unwrap(),
        ),
    );
    let owner = TypeKind::Result {
        ok: Box::new(function.clone()),
        error: Box::new(TypeKind::String),
    };
    let payload = VariantPayloadType::from_prepared_case(
        VariantPayloadOwnerFamily::Result,
        owner,
        0,
        VariantPayloadTypeShape::Tuple(vec![function.clone()].into_boxed_slice()),
    );
    TypeKind::function_with_contract(
        outer,
        EffectPredicate::unconstrained(),
        [
            TypeKind::AcceptedNominal(AcceptedNominalType::new(
                crate::env::nominal::standard_nominal_id("Envelope"),
                vec![TypeKind::Array {
                    item: Box::new(function),
                    len: ArrayLength::Const(2),
                }],
            )),
            TypeKind::Ref(EntityType::new(EntityKind::Input, Some(TypeKind::String))),
            TypeKind::FixedVector(FixedVectorType::new(
                crate::callable::VectorDimensions::Two,
                TypeKind::F32,
            )),
        ],
        TypeKind::VariantPayload(Box::new(payload)),
        EffectRow::open(EffectSet::new(), outer_scope.bound_effect(0, 0).unwrap()),
    )
}

#[test]
fn effect_domain_projection_preserves_nested_headers_binders_rows_and_predicates() {
    let original = nested_type();
    let before = original.semantic_identity_digest().unwrap();
    let mut lift = Projection::new();
    let prepared: TypeKind<BoxedEffects> = original
        .project_effect_domain(&GenericScope::default(), &mut lift)
        .unwrap();
    assert_eq!(lift.rows, [2, 2, 2, 1]);
    assert_eq!(lift.predicates, lift.rows);
    let mut close = Projection::new();
    let restored: TypeKind = prepared
        .project_effect_domain(&GenericScope::default(), &mut close)
        .unwrap();
    assert_eq!(restored, original);
    assert_eq!(restored.semantic_identity_digest().unwrap(), before);
    assert_eq!(close.rows, lift.rows);
    assert_eq!(close.predicates, lift.predicates);
}

#[test]
fn effect_domain_projection_preserves_recovery_subjects_and_omitted_annotations() {
    let original = TypeKind::Tuple(vec![
        TypeKind::Error(TypePoisonId::from_index(9)),
        TypeKind::Named("_".into()),
        TypeKind::Projection {
            subject: Box::new(TypeKind::Function {
                binder: GenericBinder::EMPTY,
                predicate: EffectPredicate::unconstrained(),
                params: vec![TypeKind::I64],
                return_type: Box::new(TypeKind::I64),
                effects: EffectRow::unknown(),
            }),
            trait_name: Some("Trait".into()),
            assoc: "Output".into(),
        },
    ]);
    let prepared: TypeKind<BoxedEffects> = original
        .project_effect_domain(&GenericScope::default(), &mut Projection::new())
        .unwrap();
    let restored: TypeKind = prepared
        .project_effect_domain(&GenericScope::default(), &mut Projection::new())
        .unwrap();
    assert_eq!(restored, original);
}

#[test]
fn effect_domain_projection_keeps_abort_ownership_before_partial_type_publication() {
    let original = nested_type();
    let before = original.semantic_identity_digest().unwrap();
    for cancelled in [false, true] {
        let mut owner = Projection::new();
        owner.cancelled = cancelled;
        owner.remaining = 2;
        assert!(matches!(
            original.project_effect_domain::<BoxedEffects, _>(&GenericScope::default(), &mut owner),
            Err(TypeProjectionError::Control(Stopped))
        ));
        assert_eq!(owner.rows, Vec::<usize>::new());
        assert_eq!(owner.predicates, Vec::<usize>::new());
        assert_eq!(original.semantic_identity_digest().unwrap(), before);
    }
}

#[test]
fn preparation_domain_preserves_every_nested_arrow_contract() {
    use crate::effect_row::prepared::{PreparedEffectPredicate, PreparedEffectRow};

    let source = nested_type();
    let digest = source.semantic_identity_digest().unwrap();
    let prepared = source
        .prepare_effects(&GenericScope::default(), &mut Projection::new())
        .unwrap();
    let mut pending = vec![(&source, &prepared)];
    let mut arrows = 0;
    while let Some((source, prepared)) = pending.pop() {
        if let TypeKind::Function {
            binder,
            predicate,
            params,
            effects,
            ..
        } = source
        {
            let TypeKind::Function {
                binder: prepared_binder,
                predicate: prepared_predicate,
                params: prepared_params,
                effects: prepared_effects,
                ..
            } = prepared
            else {
                panic!("arrow grammar changed")
            };
            assert_eq!(prepared_binder, binder);
            assert_eq!(prepared_params.len(), params.len());
            assert_eq!(
                prepared_predicate,
                &PreparedEffectPredicate::Canonical(predicate.clone())
            );
            assert_eq!(
                prepared_effects,
                &PreparedEffectRow::Canonical(effects.clone())
            );
            arrows += 1;
        }
        let source_shape = source.constraint_shape();
        let prepared_shape = prepared.constraint_shape();
        let source_children: Vec<_> = source_shape.children().collect();
        let prepared_children: Vec<_> = prepared_shape.children().collect();
        assert_eq!(source_children.len(), prepared_children.len());
        pending.extend(source_children.into_iter().zip(prepared_children));
    }
    assert_eq!(arrows, 4);
    assert_eq!(source.semantic_identity_digest().unwrap(), digest);
}

#[test]
fn preparation_domain_distinguishes_omitted_annotations_from_live_scheme_terms() {
    use crate::effect_row::prepared::{
        PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow,
    };

    let source = TypeKind::Function {
        binder: GenericBinder::EMPTY,
        predicate: EffectPredicate::unconstrained(),
        params: vec![TypeKind::I64],
        return_type: Box::new(TypeKind::I64),
        effects: EffectRow::unknown(),
    };
    let mut prepared = source
        .prepare_effects(&GenericScope::default(), &mut Projection::new())
        .unwrap();
    let TypeKind::Function { effects, .. } = &mut prepared else {
        panic!("expected prepared arrow")
    };
    assert_eq!(*effects, PreparedEffectRow::Canonical(EffectRow::unknown()));
    let mut graph = PreparedEffectGraph::default();
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut Decisions)
        .unwrap();
    let term = graph.application(&scheme, vec![], &mut Decisions).unwrap();
    *effects = term.clone();
    assert_ne!(*effects, PreparedEffectRow::Canonical(EffectRow::unknown()));
    assert!(matches!(effects, PreparedEffectRow::Term(_)));
    graph
        .prepare_body(
            &scheme,
            PreparedEffectRow::Canonical(EffectRow::closed(
                EffectSet::from_labels(["log.write"]).unwrap(),
            )),
            vec![],
            &mut Decisions,
        )
        .unwrap();
    let closed = graph.solve_principal(&mut Decisions).unwrap();
    let canonical = closed.resolve_row(&term, &mut Decisions).unwrap();
    assert_eq!(
        canonical.closed_value().unwrap(),
        EffectSet::from_labels(["log.write"]).unwrap()
    );
    let TypeKind::Function { predicate, .. } = prepared else {
        panic!("expected prepared arrow")
    };
    assert_eq!(
        predicate,
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
    );
}

#[test]
fn preparation_domain_charges_effect_graph_copying_before_publication() {
    let source = nested_type();
    let digest = source.semantic_identity_digest().unwrap();
    let mut control = Projection::new();
    let before = control.remaining;
    source
        .prepare_effects(&GenericScope::default(), &mut control)
        .unwrap();
    let consumed = before - control.remaining;
    assert!(consumed > 1);
    let mut exact = Projection::new();
    exact.remaining = consumed;
    source
        .prepare_effects(&GenericScope::default(), &mut exact)
        .unwrap();
    let mut short = Projection::new();
    short.remaining = consumed - 1;
    assert!(matches!(
        source.prepare_effects(&GenericScope::default(), &mut short),
        Err(TypeProjectionError::Control(Stopped))
    ));
    let mut cancelled = Projection::new();
    cancelled.cancelled = true;
    assert!(matches!(
        source.prepare_effects(&GenericScope::default(), &mut cancelled),
        Err(TypeProjectionError::Control(Stopped))
    ));
    assert_eq!(source.semantic_identity_digest().unwrap(), digest);
}

#[test]
fn prepared_parameter_projection_keeps_inner_binders_and_projects_pending_predicates() {
    use crate::effect_row::prepared::{
        PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRelation, PreparedEffectRow,
    };

    let incoming = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let inner_binder = GenericBinder::new(0, 0, 1);
    let occurrence = incoming.with_binder(inner_binder);
    let a = incoming.bound_effect(0, 0).unwrap();
    let b = incoming.bound_effect(0, 1).unwrap();
    let outer_a = occurrence.bound_effect(1, 0).unwrap();
    let outer_b = occurrence.bound_effect(1, 1).unwrap();
    let inner = occurrence.bound_effect(0, 0).unwrap();
    let row = |reference: &GenericEffectReference| {
        PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), reference.clone()))
    };
    let mut graph = PreparedEffectGraph::default();
    let f = graph
        .declare(
            vec![a.clone(), b.clone()],
            EffectPredicate::unconstrained(),
            &mut Decisions,
        )
        .unwrap();
    let body = graph.union(vec![row(&a), row(&b)], &mut Decisions).unwrap();
    graph
        .prepare_body(&f, body, vec![], &mut Decisions)
        .unwrap();
    let source_row = graph
        .application(&f, vec![row(&outer_a), row(&inner)], &mut Decisions)
        .unwrap();
    let permitted = graph
        .union(
            vec![
                PreparedEffectRow::Canonical(EffectRow::closed(
                    EffectSet::from_labels(["fs.read"]).unwrap(),
                )),
                row(&inner),
            ],
            &mut Decisions,
        )
        .unwrap();
    let source_predicate = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            source_row.clone(),
            permitted,
            &mut Decisions,
        )
        .unwrap();
    let source = TypeKind::<PreparedEffects>::Function {
        binder: inner_binder,
        predicate: source_predicate,
        params: vec![TypeKind::I64],
        return_type: Box::new(TypeKind::I64),
        effects: source_row.clone(),
    };
    let before = source.clone();
    let mut occurrences = Vec::new();
    let projected = source
        .project_effect_parameters(
            &incoming,
            &mut graph,
            &mut Projection::new(),
            &mut |reference, scope, control| {
                occurrences.push((reference.clone(), scope.binders().len()));
                let Some(GenericEffectReference::Bound(parameter)) =
                    reference.template_key(&incoming, scope)?
                else {
                    return Ok(None);
                };
                control
                    .visit_node(TypeProjectionNodeKind::Effect, 1)
                    .map_err(TypeProjectionError::Control)?;
                Ok(Some(row(&scope.bound_effect(1, 1 - parameter.slot())?)))
            },
        )
        .unwrap();
    assert_eq!(source, before);
    assert!(occurrences.iter().all(|(_, depth)| *depth == 2));
    assert!(occurrences.iter().any(|(reference, _)| reference == &inner));
    let TypeKind::Function {
        binder,
        predicate,
        effects,
        ..
    } = projected
    else {
        panic!("expected prepared arrow")
    };
    assert_eq!(binder, inner_binder);
    let expected_row = EffectFormula::variable(outer_b.clone(), &mut Decisions)
        .unwrap()
        .union(
            &EffectFormula::variable(inner.clone(), &mut Decisions).unwrap(),
            &mut Decisions,
        )
        .unwrap();
    let expected_permitted = EffectFormula::from_set(
        &EffectSet::from_labels(["fs.read"]).unwrap(),
        &mut Decisions,
    )
    .unwrap()
    .union(
        &EffectFormula::variable(inner.clone(), &mut Decisions).unwrap(),
        &mut Decisions,
    )
    .unwrap();
    let expected_predicate = PreparedEffectPredicate::Canonical(
        expected_row
            .subset(&expected_permitted, &mut Decisions)
            .unwrap(),
    );
    let yes = graph
        .and(
            vec![predicate.clone(), expected_predicate.clone()],
            &mut Decisions,
        )
        .unwrap();
    let not_actual = graph.not(predicate, &mut Decisions).unwrap();
    let not_expected = graph.not(expected_predicate, &mut Decisions).unwrap();
    let no = graph
        .and(vec![not_actual, not_expected], &mut Decisions)
        .unwrap();
    let equal = graph.or(vec![yes, no], &mut Decisions).unwrap();
    let proof = graph
        .declare(
            vec![outer_a, outer_b, inner],
            EffectPredicate::unconstrained(),
            &mut Decisions,
        )
        .unwrap();
    graph
        .prepare_body(
            &proof,
            PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new())),
            vec![equal],
            &mut Decisions,
        )
        .unwrap();
    let closed = graph.solve_principal(&mut Decisions).unwrap();
    assert_eq!(
        closed
            .resolve_row(&effects, &mut Decisions)
            .unwrap()
            .formula()
            .unwrap(),
        &expected_row
    );
    let original = EffectFormula::variable(a, &mut Decisions)
        .unwrap()
        .union(
            &EffectFormula::variable(b, &mut Decisions).unwrap(),
            &mut Decisions,
        )
        .unwrap();
    assert_eq!(closed.scheme(&f).unwrap().formula().unwrap(), &original);
}

#[test]
fn prepared_type_projection_rolls_back_earlier_rows_and_keeps_scope_errors_typed() {
    use crate::effect_row::prepared::{
        PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow,
    };

    let incoming = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let a = incoming.bound_effect(0, 0).unwrap();
    let mut graph = PreparedEffectGraph::default();
    let f = graph
        .declare(
            vec![a.clone()],
            EffectPredicate::unconstrained(),
            &mut Decisions,
        )
        .unwrap();
    graph
        .prepare_body(
            &f,
            PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), a.clone())),
            vec![],
            &mut Decisions,
        )
        .unwrap();
    let source_row = graph
        .application(
            &f,
            vec![PreparedEffectRow::Canonical(EffectRow::open(
                EffectSet::new(),
                a.clone(),
            ))],
            &mut Decisions,
        )
        .unwrap();
    let arrow = TypeKind::<PreparedEffects>::Function {
        binder: GenericBinder::EMPTY,
        predicate: PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        params: vec![],
        return_type: Box::new(TypeKind::I64),
        effects: source_row.clone(),
    };
    let source = TypeKind::Tuple(vec![arrow.clone(), arrow]);
    let before = graph.retained_terms();
    let mut calls = 0;
    let failed = source.project_effect_parameters(
        &incoming,
        &mut graph,
        &mut Projection::new(),
        &mut |reference, scope, control| {
            reference.template_key(&incoming, scope)?;
            calls += 1;
            if calls == 2 {
                return Err(crate::types::GenericScopeError::UnknownDepth { depth: 9 }.into());
            }
            control
                .visit_node(TypeProjectionNodeKind::Effect, 1)
                .map_err(TypeProjectionError::Control)?;
            Ok(Some(PreparedEffectRow::Canonical(EffectRow::closed(
                EffectSet::from_labels(["fs.read"]).unwrap(),
            ))))
        },
    );
    assert!(matches!(
        failed,
        Err(TypeProjectionError::Instantiation(
            TypeInstantiationError::Scope(crate::types::GenericScopeError::UnknownDepth {
                depth: 9
            })
        ))
    ));
    assert_eq!(calls, 2);
    assert_eq!(graph.retained_terms(), before);
    assert_eq!(
        graph
            .solve_principal(&mut Decisions)
            .unwrap()
            .resolve_row(&source_row, &mut Decisions)
            .unwrap(),
        EffectRow::open(EffectSet::new(), a)
    );
}

#[test]
fn prepared_type_projection_preserves_unknown_rows_and_the_control_failure() {
    use crate::effect_row::prepared::{
        PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow,
    };

    let source = TypeKind::<PreparedEffects>::Function {
        binder: GenericBinder::EMPTY,
        predicate: PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        params: vec![],
        return_type: Box::new(TypeKind::I64),
        effects: PreparedEffectRow::Canonical(EffectRow::unknown()),
    };
    let mut graph = PreparedEffectGraph::default();
    let mut calls = 0;
    let mut projection = Projection::new();
    let before = projection.remaining;
    let projected = source
        .project_effect_parameters(
            &GenericScope::default(),
            &mut graph,
            &mut projection,
            &mut |_, _, _| {
                calls += 1;
                Ok(None)
            },
        )
        .unwrap();
    assert_eq!(projected, source);
    assert_eq!(calls, 0);
    let consumed = before - projection.remaining;
    let mut short = Projection::new();
    short.remaining = consumed - 1;
    assert!(matches!(
        source.project_effect_parameters(
            &GenericScope::default(),
            &mut graph,
            &mut short,
            &mut |_, _, _| Ok(None)
        ),
        Err(TypeProjectionError::Control(
            PreparedTypeEffectProjectionFailure::Control(Stopped)
        ))
    ));
    let mut cancelled = Projection::new();
    cancelled.cancelled = true;
    assert!(matches!(
        source.project_effect_parameters(
            &GenericScope::default(),
            &mut graph,
            &mut cancelled,
            &mut |_, _, _| Ok(None)
        ),
        Err(TypeProjectionError::Control(
            PreparedTypeEffectProjectionFailure::Control(Stopped)
        ))
    ));
}
