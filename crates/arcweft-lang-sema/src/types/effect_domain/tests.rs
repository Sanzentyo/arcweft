use super::*;
use crate::{
    effect_row::{DecisionControl, DecisionWork, EffectFormula},
    effects::EffectSet,
    types::{
        AcceptedNominalType, ArrayLength, EntityKind, EntityType, FixedVectorType, GenericBinder,
        TypePoisonId, VariantPayloadOwnerFamily, VariantPayloadType, VariantPayloadTypeShape,
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
    let closed = graph.close(&mut Decisions).unwrap();
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
