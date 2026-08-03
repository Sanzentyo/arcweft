use core::num::{NonZeroU32, NonZeroU64};

use crate::identity::{ExprId, HirDatabaseId, HirTypedId, RawHirId, ScopeId, TypeId};
use crate::leaf::{HirIdRefIssue, HirIdRefRecovery, HirIdRefShape, HirIdRefValue, HirName};

use super::*;

fn module(database: u64) -> HirModuleId {
    HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::new(database).unwrap()),
        NonZeroU32::new(1).unwrap(),
    )
}

fn typed_id<I: HirTypedId>(module: HirModuleId, slot: u32) -> I {
    I::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).unwrap(),
        I::KIND,
    ))
}

fn name(value: &str) -> HirName {
    HirName::try_new(value.into()).unwrap()
}

fn clean_source(
    module: HirModuleId,
    backpressure: HirSourceBackpressureValue,
) -> Result<HirSourceItem, HirItemInvariantError> {
    HirSourceItem::try_new(
        module,
        None,
        Some(HirRequiredName::Resolved(name("events"))),
        typed_id::<TypeId>(module, 1),
        clean_headers(module, backpressure),
        Box::new([]),
        HirSourceBody::Braced { closed: true },
    )
}

fn clean_headers(
    module: HirModuleId,
    backpressure: HirSourceBackpressureValue,
) -> HirSourceHeaders {
    HirSourceHeaders::new(
        HirSourceRequiredSlot::authored(
            HirSourceExpressionValue::Expression(typed_id::<ExprId>(module, 2)),
            false,
        ),
        HirSourceRequiredSlot::authored(
            HirSourcePolicyBinding::new(HirSourcePunctuationState::Present, backpressure),
            false,
        ),
        HirSourceRequiredSlot::authored(
            HirSourcePolicyBinding::new(
                HirSourcePunctuationState::Present,
                HirSourceReplayValue::Resolved(HirSourceReplayPolicy::Full),
            ),
            false,
        ),
        HirSourceRequiredSlot::authored(
            HirSourcePolicyBinding::new(
                HirSourcePunctuationState::Present,
                HirSourcePrivacyValue::Resolved(HirSourcePrivacyPolicy::Private),
            ),
            false,
        ),
    )
}

#[test]
fn required_source_slots_do_not_fabricate_defaults() {
    let local = module(1);
    let source = clean_source(
        local,
        HirSourceBackpressureValue::Resolved(HirSourceBackpressurePolicy::Latest),
    )
    .unwrap();
    assert!(!source.has_structural_recovery());

    let missing = HirSourceItem::try_new(
        local,
        None,
        Some(HirRequiredName::Missing),
        typed_id::<TypeId>(local, 1),
        HirSourceHeaders::new(
            HirSourceRequiredSlot::Missing,
            HirSourceRequiredSlot::Missing,
            HirSourceRequiredSlot::Missing,
            HirSourceRequiredSlot::Missing,
        ),
        Box::new([]),
        HirSourceBody::Missing,
    )
    .unwrap();
    assert!(missing.has_structural_recovery());
    assert!(missing.from().value().is_none());
    assert!(missing.backpressure().value().is_none());
    assert!(missing.replay().value().is_none());
    assert!(missing.privacy().value().is_none());
}

#[test]
fn bounded_missing_capacity_is_typed_recovery_without_an_expr_id() {
    let local = module(1);
    let source = clean_source(
        local,
        HirSourceBackpressureValue::Resolved(HirSourceBackpressurePolicy::Bounded {
            capacity: HirSourceBoundedArgument::new(HirSourceExpressionValue::Missing, false),
            overflow: HirSourceBoundedArgument::new(
                HirSourceOverflowValue::Resolved(HirSourceOverflowPolicy::DropOldest),
                false,
            ),
            unexpected_arguments: false,
            recovered_call: false,
        }),
    )
    .unwrap();
    let HirSourceBackpressureValue::Resolved(HirSourceBackpressurePolicy::Bounded {
        capacity, ..
    }) = source.backpressure().value().unwrap().value()
    else {
        panic!("expected bounded backpressure");
    };
    assert_eq!(capacity.value().expression(), None);
    assert!(source.has_structural_recovery());
}

#[test]
fn source_id_requires_name_without_reinferring_its_family() {
    let local = module(1);
    let id = HirSourceId::new(
        HirIdRefValue::Recovered(HirIdRefRecovery::new(
            HirIdRefShape::Relative {
                parent_depth: 0,
                suffix_segment_count: 0,
            },
            HirIdRefIssue::Missing,
        )),
        true,
        true,
    );
    assert!(
        !id.has_recovery(),
        "an accepted relative marker delegates its suffix to the required Source name"
    );
    let missing_name = HirSourceItem::try_new(
        local,
        Some(id.clone()),
        None,
        typed_id::<TypeId>(local, 1),
        clean_headers(
            local,
            HirSourceBackpressureValue::Resolved(HirSourceBackpressurePolicy::Latest),
        ),
        Box::new([]),
        HirSourceBody::Braced { closed: true },
    );
    assert_eq!(
        missing_name,
        Err(HirItemInvariantError::InvalidSourceRecovery)
    );

    let recovered = HirSourceItem::try_new(
        local,
        Some(id),
        Some(HirRequiredName::Missing),
        typed_id::<TypeId>(local, 1),
        clean_headers(
            local,
            HirSourceBackpressureValue::Resolved(HirSourceBackpressurePolicy::Latest),
        ),
        Box::new([]),
        HirSourceBody::Braced { closed: true },
    )
    .unwrap();
    assert!(recovered.has_structural_recovery());
}

#[test]
fn policy_recovery_requires_a_typed_name_only_for_unsupported_values() {
    let local = module(1);
    let invalid = HirSourceItem::try_new(
        local,
        None,
        Some(HirRequiredName::Resolved(name("events"))),
        typed_id::<TypeId>(local, 1),
        HirSourceHeaders::new(
            HirSourceRequiredSlot::authored(
                HirSourceExpressionValue::Expression(typed_id::<ExprId>(local, 2)),
                false,
            ),
            HirSourceRequiredSlot::authored(
                HirSourcePolicyBinding::new(
                    HirSourcePunctuationState::Present,
                    HirSourceBackpressureValue::Resolved(HirSourceBackpressurePolicy::Latest),
                ),
                false,
            ),
            HirSourceRequiredSlot::authored(
                HirSourcePolicyBinding::new(
                    HirSourcePunctuationState::Present,
                    HirSourceReplayValue::Recovered {
                        authored: Some(name("custom")),
                        issue: HirSourcePolicyIssue::Invalid,
                    },
                ),
                false,
            ),
            HirSourceRequiredSlot::authored(
                HirSourcePolicyBinding::new(
                    HirSourcePunctuationState::Present,
                    HirSourcePrivacyValue::Resolved(HirSourcePrivacyPolicy::Private),
                ),
                false,
            ),
        ),
        Box::new([]),
        HirSourceBody::Braced { closed: true },
    );
    assert_eq!(invalid, Err(HirItemInvariantError::InvalidSourceRecovery));
}

#[test]
fn handler_children_must_share_the_source_module() {
    let local = module(1);
    let foreign = module(2);
    let handler = HirSourceHandler::new(
        HirSourceEventPattern::Disconnected(HirSourceChildState::Authored),
        HirSourcePunctuationState::Present,
        typed_id::<ScopeId>(foreign, 1),
        HirSourceHandlerBody::Missing,
    );
    assert!(matches!(
        handler.validate_module(local),
        Err(HirItemInvariantError::ForeignChild { .. })
    ));
}
