use std::sync::Arc;

use crate::final_analysis::tests::{analyze, fixture};

#[test]
fn lease_requires_the_registered_allocation_and_preserves_shared_clones() {
    let world = fixture("fn identity(value: i64) -> i64 { value }", None);
    let analysis = analyze(&world).unwrap();
    let catalog = analysis.checked_callables();
    let lease = catalog.authority_lease();
    let shared = catalog.as_ref().clone();
    assert!(lease.admits(&shared));
    let mut independently_registered = shared.clone();
    let registered = independently_registered.registered.as_ref().unwrap();
    independently_registered.registered = Some(Arc::new(registered.as_ref().clone()));
    assert_eq!(catalog.generation(), independently_registered.generation());
    assert_eq!(
        catalog.registered_catalog().unwrap().digest(),
        independently_registered
            .registered_catalog()
            .unwrap()
            .digest()
    );
    assert!(!lease.admits(&independently_registered));
}
