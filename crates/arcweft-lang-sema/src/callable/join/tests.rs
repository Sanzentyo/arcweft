use super::{CallableInstantiation, IntrinsicCallableCandidateTag, callable_instantiation_digest};
use crate::{
    callable::{
        CallableArgumentPolicy, CallableCandidateId, CallableEffectSchema, CallableGroupKind,
        CallableLookupKey, CallableName, CallableParameter, CallableParameterPassing,
        CallableParameterPresence, CallableParameterType, CallableSignatureSchema,
        CallableValidator, CheckedCallableId, CheckedMethodLookup, CollectionMethodId,
        EnvironmentCallableId, EnvironmentCallableKind, EnvironmentCallableOwner,
        LanguageCallableFamily, PRODUCTION_CALLABLE_LIMITS, ReceiverMethodKey,
        RegisteredCallableCatalogDigest, ResolvedCallable, SignatureOrigin, StandardEnvironmentId,
    },
    effect_row::EffectRow,
    effects::EffectSet,
    types::TypeKind,
};

#[test]
fn intrinsic_tags_are_closed_and_stable() {
    assert_eq!(IntrinsicCallableCandidateTag::Fx.semantic_tag(), 0);
    assert_eq!(IntrinsicCallableCandidateTag::Promotion.semantic_tag(), 21);
    assert_ne!(
        callable_instantiation_digest(&CallableInstantiation::None).bytes(),
        callable_instantiation_digest(&CallableInstantiation::ExpectedEnum {
            expected: TypeKind::Unit,
        })
        .bytes()
    );
}

#[test]
fn callable_owner_group_and_partial_result_projection_is_exact() {
    let callable = two_group_callable(CallableInstantiation::None);
    let current = super::super::CallableGroupIndex::try_from_usize(0).unwrap();
    assert_eq!(
        callable
            .next_group_for(current)
            .map(super::super::CallableGroupIndex::get),
        Some(1)
    );
    assert_eq!(
        callable.result_type_for_group(current),
        Some(TypeKind::function_with_effects(
            [TypeKind::Unit],
            TypeKind::Unit,
            EffectRow::closed(EffectSet::new()),
        ))
    );
}

#[test]
fn selected_group_and_result_negative_evidence_is_typed() {
    let callable = two_group_callable(CallableInstantiation::None);
    let current = super::super::CallableGroupIndex::try_from_usize(0).unwrap();
    let next = super::super::CallableGroupIndex::try_from_usize(1).unwrap();
    let absent = super::super::CallableGroupIndex::try_from_usize(2).unwrap();

    assert_eq!(
        super::validate_selected_groups(&callable, next, Some(next), false),
        Err(super::CheckedCallableJoinError::SelectedGroupMismatch)
    );
    assert_eq!(
        super::validate_selected_groups(&callable, absent, None, true),
        Err(super::CheckedCallableJoinError::CurrentGroupMissing)
    );
    assert_eq!(
        super::validate_selected_groups(&callable, current, None, false),
        Err(super::CheckedCallableJoinError::NextGroupMismatch)
    );
    assert_eq!(
        super::validate_result_type(Some(&TypeKind::Bool), &TypeKind::Unit),
        Err(super::CheckedCallableJoinError::ResultMismatch)
    );
    assert_eq!(
        super::validate_result_type(None, &TypeKind::Unit),
        Err(super::CheckedCallableJoinError::MissingResult)
    );
}

#[test]
fn method_lookup_absent_and_mismatched_ids_fail_closed() {
    let selected = test_checked_id("selected");
    let other = test_checked_id("other");
    assert_eq!(
        super::validate_method_lookup_result(&selected, CheckedMethodLookup::Absent),
        Err(super::CheckedCallableJoinError::MethodLookupMissing)
    );
    assert_eq!(
        super::validate_method_lookup_result(
            &selected,
            CheckedMethodLookup::Unique(Box::new(other)),
        ),
        Err(super::CheckedCallableJoinError::MethodLookupMismatch)
    );
}

#[test]
fn receiver_key_mismatch_is_rejected_without_name_fallback() {
    let callable = receiver_callable();
    let wrong_key = ReceiverMethodKey::new(TypeKind::Bool, CallableName::try_new("len").unwrap());
    assert_eq!(
        super::receiver_mode(&callable, None, Some(&wrong_key)),
        Err(super::CheckedCallableJoinError::ReceiverTypeMismatch)
    );
}

#[test]
fn direct_extension_row_key_does_not_require_method_lookup() {
    let callable = two_group_callable(CallableInstantiation::None);
    let extension_key =
        ReceiverMethodKey::new(TypeKind::String, CallableName::try_new("len").unwrap());
    assert_eq!(
        super::receiver_mode(&callable, Some(&extension_key), None),
        Ok(super::CallableReceiverMode::None)
    );
    assert_eq!(
        super::receiver_mode(&callable, Some(&extension_key), Some(&extension_key)),
        Err(super::CheckedCallableJoinError::UnexpectedReceiverKey)
    );
}

fn two_group_callable(instantiation: CallableInstantiation) -> ResolvedCallable {
    let parameter = |group: usize| {
        let index = super::super::CallableParameterIndex::try_from_usize(0).unwrap();
        super::super::CallableParameterGroup::try_new(
            super::super::CallableGroupIndex::try_from_usize(group).unwrap(),
            if group == 0 {
                CallableGroupKind::Initial
            } else {
                CallableGroupKind::Curried
            },
            vec![
                CallableParameter::try_new(
                    index,
                    Some(CallableName::try_new(format!("arg{group}")).unwrap()),
                    CallableParameterType::Exact(TypeKind::Unit),
                    CallableParameterPassing::PositionalOnly,
                    CallableParameterPresence::Required,
                    None,
                    None,
                )
                .unwrap(),
            ],
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .unwrap()
    };
    let schema = CallableSignatureSchema::try_new(
        vec![parameter(0), parameter(1)],
        TypeKind::Unit,
        CallableEffectSchema::fixed(EffectRow::closed(EffectSet::new())),
        CallableArgumentPolicy::new(
            super::super::UnknownNamedArgumentPolicy::Reject,
            super::super::SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .unwrap();
    let (candidate, family) = match instantiation {
        CallableInstantiation::Receiver { .. } => (
            CallableCandidateId::CollectionMethod(CollectionMethodId::Len),
            LanguageCallableFamily::CollectionMethod,
        ),
        _ => (
            CallableCandidateId::Fx(super::super::FxCallableSignatureId::Style),
            LanguageCallableFamily::Fx,
        ),
    };
    ResolvedCallable::try_from_intrinsic(
        candidate,
        SignatureOrigin::Language { family },
        std::sync::Arc::new(schema),
        instantiation,
        Vec::new(),
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .unwrap()
}

fn receiver_callable() -> ResolvedCallable {
    two_group_callable(CallableInstantiation::Receiver {
        receiver: TypeKind::String,
    })
}

fn test_checked_id(name: &str) -> CheckedCallableId {
    let method = CallableName::try_new(name).unwrap();
    let key = ReceiverMethodKey::new(TypeKind::String, method);
    let id = EnvironmentCallableId::new(
        EnvironmentCallableOwner::Standard(StandardEnvironmentId::Core),
        EnvironmentCallableKind::Method,
        CallableLookupKey::Method(key),
        super::super::CallableOverloadIndex::try_from_usize(0).unwrap(),
    );
    CheckedCallableId::for_environment(RegisteredCallableCatalogDigest::from_bytes([0; 32]), id)
}
