use super::*;
use crate::effect_row::prepared::{
    PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow,
};
use crate::effect_row::{EffectPredicate, EffectRow};
use crate::types::constraints::NoConstraintClient;
use crate::types::constraints::context::{
    LocalConstraintAccounting, TypeConstraintContext, TypeConstraintLimits,
};
use crate::types::{
    GenericBinder, GenericScope, VariantPayloadOwnerFamily, VariantPayloadTypeShape,
};
use std::sync::atomic::AtomicBool;

#[test]
fn record_pattern_retains_a_live_field_before_original_body_preparation() {
    let cancellation = AtomicBool::new(false);
    let mut work = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
        LocalConstraintAccounting::new(
            TypeConstraintLimits::new(100_000, 100_000, 128, 128),
            &cancellation,
        ),
    );
    let mut graph = PreparedEffectGraph::default();
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let row = graph.application(&scheme, vec![], &mut work).unwrap();
    let callback = TypeKind::<PreparedEffects>::Function {
        binder: GenericBinder::EMPTY,
        predicate: PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        params: vec![],
        return_type: Box::new(TypeKind::I64),
        effects: row.clone(),
    };
    // This is an explicit provisional schema fixture. It has no checked
    // environment, field identity, definition seal, or source Γ authority.
    let payload = crate::types::VariantPayloadType::from_prepared_case(
        VariantPayloadOwnerFamily::BuiltinClosed,
        TypeKind::Named("fixture::DeferredRecord".into()),
        0,
        VariantPayloadTypeShape::try_record([("callback".into(), callback)]).unwrap(),
    );
    let ty = TypeKind::VariantPayload(Box::new(payload.clone()));
    let owner = PreparedRecordPatternOwner::variant_payload(payload).unwrap();
    // Omitting the one field requires a rest pattern even before completion.
    assert!(
        PreparedRecordPattern::try_new(
            ty.clone(),
            owner.clone(),
            Box::new([]),
            PreparedRecordPatternRest::Absent,
        )
        .is_none()
    );
    let prepared =
        PreparedRecordPattern::try_new(ty, owner, Box::new([]), PreparedRecordPatternRest::Ignore)
            .unwrap();
    let retained = prepared
        .ty()
        .retain_effects(&GenericScope::default(), graph.read(), &mut work)
        .unwrap();
    let TypeKind::VariantPayload(payload) = &retained else {
        panic!("the record pattern retains its payload root")
    };
    let TypeKind::Function { effects, .. } = payload.shape().record_fields().unwrap()[0].ty()
    else {
        panic!("the payload retains its callback field")
    };
    // Operational handle identity is retained; no semantic equality is decided.
    assert_eq!(effects, &row);

    let completed = EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap());
    graph
        .prepare_body(
            &scheme,
            PreparedEffectRow::Canonical(completed.clone()),
            vec![],
            &mut work,
        )
        .unwrap();
    let principal = graph.solve_principal(&mut work).unwrap();
    let closed = retained
        .project_canonical_effects(&GenericScope::default(), &principal, &mut work)
        .unwrap();
    let TypeKind::VariantPayload(payload) = closed else {
        panic!("closure preserves the record payload shape")
    };
    let TypeKind::Function { effects, .. } = payload.shape().record_fields().unwrap()[0].ty()
    else {
        panic!("closure preserves the callback field")
    };
    assert_eq!(effects, &completed);
    assert!(payload.try_seal().is_ok());
}
