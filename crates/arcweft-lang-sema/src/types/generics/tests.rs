use super::*;

#[test]
fn effect_template_keys_preserve_nested_and_empty_binder_ownership() {
    let incoming = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let nested = incoming.with_binder(GenericBinder::new(1, 0, 1));
    let outer = nested.bound_effect(1, 1).unwrap();
    assert_eq!(
        outer.template_key(&incoming, &nested).unwrap(),
        Some(incoming.bound_effect(0, 1).unwrap())
    );
    let inner = nested.bound_effect(0, 0).unwrap();
    assert_eq!(inner.template_key(&incoming, &nested).unwrap(), None);
    let empty = incoming.with_binder(GenericBinder::EMPTY);
    assert_eq!(empty, incoming);
    assert_eq!(
        incoming
            .bound_effect(0, 1)
            .unwrap()
            .template_key(&incoming, &empty)
            .unwrap(),
        Some(incoming.bound_effect(0, 1).unwrap())
    );
}

#[test]
fn effect_slots_validate_kind_width_and_lexical_depth() {
    let scope = GenericScope::default().with_binder(GenericBinder::new(2, 3, 1));
    assert_eq!(
        scope.bound_effect(0, 1),
        Err(GenericScopeError::UnknownSlot {
            kind: GenericParameterKind::Effect,
            slot: 1,
            arity: 1
        })
    );
    let reference = scope.bound_effect(0, 0).unwrap();
    assert_eq!(
        reference.template_key(&GenericScope::default(), &GenericScope::default()),
        Err(GenericScopeError::UnknownDepth { depth: 0 })
    );
    let wide = GenericScope::default().with_binder(GenericBinder::new(0, 0, 70_000));
    assert!(wide.bound_effect(0, 69_999).is_ok());
    assert!(scope.bound_type(0, 1).is_ok());
    assert!(scope.bound_const(0, 2).is_ok());
}

#[test]
fn effect_declaration_identity_does_not_alias_a_fresh_application_slot() {
    let owner = super::super::GenericParameterOwnerId::Detached(
        super::super::DetachedGenericOwnerId::new(7),
    );
    let declared = GenericEffectParameterId::new(owner.clone(), 0);
    let free = GenericEffectReference::from(declared.clone());
    assert_eq!(free.free_parameter(), Some(&declared));
    assert_eq!(declared.owner(), &owner);
    assert_eq!(declared.ordinal(), 0);
    let first = OpenedGenericScope::new(GenericBinder::new(0, 0, 1)).unwrap();
    let second = OpenedGenericScope::new(GenericBinder::new(0, 0, 1)).unwrap();
    let opened = first.effect_reference(0).unwrap();
    assert_ne!(opened, second.effect_reference(0).unwrap());
    assert_ne!(opened, free);
    assert!(opened.free_parameter().is_none());
    assert_eq!(
        opened.template_key(&GenericScope::default(), &GenericScope::default()),
        Err(GenericScopeError::EscapedInference {
            kind: GenericParameterKind::Effect
        })
    );
    assert_eq!(
        free.template_key(&GenericScope::default(), &GenericScope::default())
            .unwrap(),
        Some(free.clone())
    );
    assert!(first.effect_reference(1).is_err());
}
