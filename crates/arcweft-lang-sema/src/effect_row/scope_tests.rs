use super::*;

#[test]
fn scoped_reference_mapping_preserves_inner_binders_and_reorders_outer_slots() {
    use crate::types::{GenericBinder, GenericEffectReference, GenericScope, GenericScopeError};

    struct ScopeControl;
    impl DecisionControl for ScopeControl {
        type Error = GenericScopeError;
        fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    let incoming = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let nested = incoming.with_binder(GenericBinder::new(0, 0, 1));
    let outer_first = nested.bound_effect(1, 0).unwrap();
    let outer_second = nested.bound_effect(1, 1).unwrap();
    let inner = nested.bound_effect(0, 0).unwrap();
    let mut control = ScopeControl;
    let first = EffectFormula::variable(outer_first, &mut control).unwrap();
    let second = EffectFormula::variable(outer_second, &mut control).unwrap();
    let inner_formula = EffectFormula::variable(inner.clone(), &mut control).unwrap();
    let effects = EffectSet::from_labels(["fs.read"]).unwrap();
    let read = EffectFormula::from_set(&effects, &mut control).unwrap();
    let source = first
        .union(&inner_formula, &mut control)
        .unwrap()
        .difference(&second, &mut control)
        .unwrap()
        .union(&read, &mut control)
        .unwrap();
    let mut mapping = |reference: &GenericEffectReference, _: &mut ScopeControl| match reference
        .template_key(&incoming, &nested)?
    {
        Some(GenericEffectReference::Bound(parameter)) => {
            nested.bound_effect(1, 1 - parameter.slot())
        }
        Some(_) | None => Ok(reference.clone()),
    };
    let mapped = source.map_references(&mut control, &mut mapping).unwrap();
    let expected = second
        .union(&inner_formula, &mut control)
        .unwrap()
        .difference(&first, &mut control)
        .unwrap()
        .union(&read, &mut control)
        .unwrap();
    assert_eq!(mapped, expected);
    let predicate = source.subset(&read, &mut control).unwrap();
    assert_eq!(
        predicate
            .map_references(&mut control, &mut mapping)
            .unwrap(),
        expected.subset(&read, &mut control).unwrap()
    );

    let difference = first.difference(&second, &mut control).unwrap();
    let collapsed = difference
        .map_references(&mut control, &mut |_, _| Ok(inner.clone()))
        .unwrap();
    assert_eq!(collapsed, EffectFormula::empty());
    let before = source.clone();
    let root = GenericScope::default();
    assert!(matches!(
        source.map_references(&mut control, &mut |reference, _| {
            reference
                .template_key(&root, &root)
                .map(|key| key.expect("root template key"))
        }),
        Err(GenericScopeError::UnknownDepth { .. })
    ));
    assert_eq!(source, before);
}
