use super::*;

#[test]
fn registry_clears_values_when_scope_exits() {
    let mut registry = PresentationRegistry::default();
    let line_scope = PresentationScope::line();
    let flow_scope = PresentationScope::flow();

    let line_bg = bg(asset("bg.room"), line_scope.clone());
    let flow_bg = PresentationHandle::new(
        BackgroundSurface::new(asset("bg.evening")),
        PresentationTarget::scene(),
        PresentationSlot::new(PublicId::try_new("slot.background.flow").unwrap()),
        flow_scope.clone(),
    );
    let line_ref = line_bg.slot_ref();
    let flow_ref = flow_bg.slot_ref();

    assert_eq!(registry.set(line_bg), None);
    assert_eq!(registry.set(flow_bg), None);
    assert!(registry.get(&line_ref).is_some());
    assert!(registry.get(&flow_ref).is_some());

    let removed = registry.exit_scope(&line_scope);
    assert_eq!(removed.len(), 1);
    assert!(registry.get(&line_ref).is_none());
    assert!(registry.get(&flow_ref).is_some());
}

#[test]
fn clear_returns_registered_value() {
    let mut registry = PresentationRegistry::default();
    let scope = PresentationScope::line();
    let handle = bg(asset("bg.room"), scope.clone());
    let clear = clear_bg(scope);

    registry.set(handle);
    let removed = registry.clear(&clear).expect("registered background");
    assert_eq!(removed.asset().as_str(), "asset.bg.room");
    assert!(registry.clear(&clear).is_none());
}
