use crate::callable::SignatureAccountingError;

use super::{
    CallableCatalogBuildError, CallableCatalogError, CallableDocumentation, CallableLookupKey,
    CallableOverloadIndex, CallableQueryDepth, CallableQueryLimitError, EnvironmentCallableKind,
    EnvironmentCallablePublicationRecord, EnvironmentDeclarationOrdinal,
    PRODUCTION_CALLABLE_LIMITS, RegisteredCallableCatalogBuilder, ResolverWork,
    SignatureQueryWorkMeter, SignatureWorkKind, accepted_nominal_world, external_binding_project,
    multi_group_schema, path, projected_publication,
};

#[test]
fn production_catalog_overload_limit_accepts_exact_and_rejects_one_over() {
    let (_, symbols) = external_binding_project([]);
    let world = accepted_nominal_world(&symbols);

    let exact = projected_publication(
        &world,
        "adapter.production-overload-exact",
        overload_records(32),
    );
    let mut exact_builder =
        RegisteredCallableCatalogBuilder::for_nominal_world(&world, PRODUCTION_CALLABLE_LIMITS);
    exact_builder
        .add_environment(exact)
        .expect("exact publication belongs to the accepted world");
    let catalog = exact_builder.finish().expect("exact overload boundary");
    assert_eq!(
        catalog
            .free(&path(&["production_overload"]))
            .expect("accepted overload set")
            .len()
            .get(),
        32
    );

    let one_over = projected_publication(
        &world,
        "adapter.production-overload-one-over",
        overload_records(33),
    );
    let mut one_over_builder =
        RegisteredCallableCatalogBuilder::for_nominal_world(&world, PRODUCTION_CALLABLE_LIMITS);
    one_over_builder
        .add_environment(one_over)
        .expect("one-over publication is validated while freezing the catalog");
    assert!(matches!(
        one_over_builder.finish(),
        Err(CallableCatalogBuildError::InvalidRecord(
            CallableCatalogError::OverloadLimit {
                actual: 33,
                limit: 32,
            }
        ))
    ));
}

#[test]
fn production_callable_depth_accepts_exact_and_rejects_one_over_without_mutation() {
    let mut depth = CallableQueryDepth::new(PRODUCTION_CALLABLE_LIMITS);
    for _ in 0..PRODUCTION_CALLABLE_LIMITS.max_nested_calls() {
        depth.try_enter().expect("exact callable nesting boundary");
    }
    assert_eq!(depth.current(), 32);
    assert_eq!(
        depth.try_enter(),
        Err(CallableQueryLimitError::NestedCalls {
            actual: 33,
            limit: 32,
        })
    );
    assert_eq!(depth.current(), 32);
    for _ in 0..PRODUCTION_CALLABLE_LIMITS.max_nested_calls() {
        depth.leave();
    }
    assert_eq!(depth.current(), 0);
}

#[test]
fn callable_work_counters_classify_overflow_and_preserve_exact_state() {
    let mut build = super::CatalogBuildWork::new(u64::MAX);
    build
        .charge(u64::MAX)
        .expect("exact catalog build counter range");
    assert_eq!(
        build.charge(1),
        Err(CallableCatalogBuildError::WorkOverflow)
    );
    assert_eq!(build.consumed(), u64::MAX);

    let mut query = ResolverWork::new(u64::MAX);
    query
        .charge(u64::MAX)
        .expect("exact callable query counter range");
    assert_eq!(
        query.charge(1),
        Err(CallableQueryLimitError::ArithmeticOverflow)
    );
    assert_eq!(query.consumed(), u64::MAX);

    let mut signature = SignatureQueryWorkMeter::new(super::PRODUCTION_SIGNATURE_LIMITS);
    let mut parameters = u64::MAX;
    assert_eq!(
        signature.charge_parameter(&mut parameters),
        Err(SignatureAccountingError::Arithmetic {
            counter: SignatureWorkKind::Parameters,
        })
    );
    assert_eq!(parameters, u64::MAX);
    assert_eq!(signature.report().total_work(), 0);
}

fn overload_records(count: usize) -> Vec<EnvironmentCallablePublicationRecord> {
    let key = CallableLookupKey::Free(path(&["production_overload"]));
    (0..count)
        .map(|overload| {
            EnvironmentCallablePublicationRecord::try_new(
                EnvironmentCallableKind::Function,
                key.clone(),
                CallableOverloadIndex::try_from_usize(overload).expect("overload index"),
                (*multi_group_schema(1)).clone(),
                CallableDocumentation::missing(),
                None,
                None,
                EnvironmentDeclarationOrdinal::try_from_usize(overload)
                    .expect("declaration ordinal"),
            )
            .expect("production overload record")
        })
        .collect()
}
