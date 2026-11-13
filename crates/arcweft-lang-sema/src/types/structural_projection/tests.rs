use super::*;
use crate::{
    effect_row::{
        DecisionControl, DecisionWork, EffectFormula, EffectPredicate, EffectRow,
        prepared::{PreparedEffectPredicate, PreparedEffectRow},
    },
    effects::EffectSet,
    types::{CanonicalEffects, effect_domain::PreparedEffects},
};
use std::convert::Infallible;

struct Decisions;

impl DecisionControl for Decisions {
    type Error = Infallible;

    fn charge(&mut self, _: DecisionWork) -> Result<(), Infallible> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("projection stopped")]
struct Stopped;

struct Projection {
    source: GenericScope,
    target: GenericScope,
    remaining: u64,
    cancelled: bool,
    ports: [usize; 4],
}

impl Projection {
    fn charge(&mut self) -> Result<(), Stopped> {
        self.remaining = self.remaining.checked_sub(1).ok_or(Stopped)?;
        Ok(())
    }

    fn scopes(&self, source: &GenericScope, target: &GenericScope) {
        assert_eq!(source, &self.source);
        assert_eq!(target, &self.target);
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

fn restriction(scope: &GenericScope, slot: u32) -> EffectPredicate {
    EffectFormula::literal(EffectSet::new(), Some(scope.bound_effect(1, slot).unwrap()))
        .subset(
            &EffectFormula::literal(EffectSet::from_labels(["fs.read"]).unwrap(), None),
            &mut Decisions,
        )
        .unwrap()
}

impl ScopedTypeProjection<CanonicalEffects, PreparedEffects> for Projection {
    fn project_type_reference(
        &mut self,
        reference: &GenericTypeReference,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<TypeKind<PreparedEffects>, TypeProjectionError<Stopped>> {
        self.scopes(source, target);
        self.ports[0] += 1;
        if reference == &source.bound_type(1, 0)? {
            // This target reference is not a valid source key. Revisiting the
            // replacement would reinterpret caller data or reject it.
            Ok(TypeKind::Tuple(vec![TypeKind::GenericParam(
                target.bound_type(1, 1)?,
            )]))
        } else {
            assert_eq!(reference, &source.bound_type(0, 0)?);
            Ok(TypeKind::GenericParam(target.bound_type(0, 0)?))
        }
    }

    fn project_length(
        &mut self,
        length: &ArrayLength,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<ArrayLength, TypeProjectionError<Stopped>> {
        self.scopes(source, target);
        self.ports[1] += 1;
        assert_eq!(length, &ArrayLength::Generic(source.bound_const(1, 0)?));
        Ok(ArrayLength::Generic(target.bound_const(1, 1)?))
    }

    fn project_row(
        &mut self,
        row: &EffectRow,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<PreparedEffectRow, TypeProjectionError<Stopped>> {
        self.scopes(source, target);
        self.ports[2] += 1;
        assert_eq!(
            row,
            &EffectRow::open(EffectSet::new(), source.bound_effect(1, 0)?)
        );
        Ok(PreparedEffectRow::Canonical(EffectRow::open(
            EffectSet::new(),
            target.bound_effect(1, 1)?,
        )))
    }

    fn project_predicate(
        &mut self,
        predicate: &EffectPredicate,
        source: &GenericScope,
        target: &GenericScope,
        _: u64,
    ) -> Result<PreparedEffectPredicate, TypeProjectionError<Stopped>> {
        self.scopes(source, target);
        self.ports[3] += 1;
        assert_eq!(predicate, &restriction(source, 0));
        Ok(PreparedEffectPredicate::Canonical(restriction(target, 1)))
    }
}

fn fixture() -> (TypeKind, GenericScope, GenericScope, Projection) {
    let source = GenericScope::default().with_binder(GenericBinder::new(1, 1, 1));
    let target = GenericScope::default()
        .with_binder(GenericBinder::new(0, 0, 1))
        .with_binder(GenericBinder::new(2, 2, 2));
    let binder = GenericBinder::new(1, 1, 1);
    let nested_source = source.with_binder(binder);
    let ty = TypeKind::function_with_contract(
        binder,
        restriction(&nested_source, 0),
        [TypeKind::Array {
            item: Box::new(TypeKind::GenericParam(
                nested_source.bound_type(1, 0).unwrap(),
            )),
            len: ArrayLength::Generic(nested_source.bound_const(1, 0).unwrap()),
        }],
        TypeKind::GenericParam(nested_source.bound_type(0, 0).unwrap()),
        EffectRow::open(EffectSet::new(), nested_source.bound_effect(1, 0).unwrap()),
    );
    let projection = Projection {
        source: nested_source,
        target: target.with_binder(binder),
        remaining: 1024,
        cancelled: false,
        ports: [0; 4],
    };
    (ty, source, target, projection)
}

#[test]
fn shared_fold_transports_all_ports_and_keeps_replacements_in_their_target_scope() {
    let (ty, source, target, mut projection) = fixture();
    let value = ty
        .project_structure(&source, &target, 1, &mut projection)
        .unwrap();
    let nested = &projection.target;
    assert_eq!(
        value,
        TypeKind::<PreparedEffects>::Function {
            binder: GenericBinder::new(1, 1, 1),
            predicate: PreparedEffectPredicate::Canonical(restriction(nested, 1)),
            params: vec![TypeKind::Array {
                item: Box::new(TypeKind::Tuple(vec![TypeKind::GenericParam(
                    nested.bound_type(1, 1).unwrap()
                )])),
                len: ArrayLength::Generic(nested.bound_const(1, 1).unwrap()),
            }],
            return_type: Box::new(TypeKind::GenericParam(nested.bound_type(0, 0).unwrap())),
            effects: PreparedEffectRow::Canonical(EffectRow::open(
                EffectSet::new(),
                nested.bound_effect(1, 1).unwrap()
            )),
        }
    );
    assert_eq!(projection.ports, [2, 1, 1, 1]);
}

#[test]
fn shared_fold_admits_both_scope_extensions_before_scalar_projection() {
    let (ty, source, target, _) = fixture();
    let before = ty.clone();
    // One root node, then two source and three target scope bindings. The
    // first child cannot enter until all five scope copies have been admitted.
    for remaining in 0..=6 {
        let (_, _, _, mut projection) = fixture();
        projection.remaining = remaining;
        assert!(matches!(
            ty.project_structure::<PreparedEffects, _>(&source, &target, 1, &mut projection),
            Err(TypeProjectionError::Control(Stopped))
        ));
        assert_eq!(projection.ports, [0; 4]);
        assert_eq!(ty, before);
    }
    let (_, _, _, mut projection) = fixture();
    projection.cancelled = true;
    assert!(matches!(
        ty.project_structure::<PreparedEffects, _>(&source, &target, 1, &mut projection),
        Err(TypeProjectionError::Control(Stopped))
    ));
    assert_eq!(projection.remaining, 1024);
    assert_eq!(projection.ports, [0; 4]);
    assert_eq!(ty, before);
}
