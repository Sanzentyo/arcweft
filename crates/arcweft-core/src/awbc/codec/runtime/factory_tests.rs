use super::*;
use crate::{
    entry::RuntimeSchemaLimits, pattern::RuntimeSemanticTypeId,
    presentation::RuntimePresentationFactoryCall,
};
use arcweft_interaction_model::presentation::{
    PresentationArgumentKey, PresentationArgumentRole, PresentationCallableId,
};
use std::sync::Arc;

fn inputs() -> Box<[PresentationArgumentKey]> {
    ["channel", "user.role"]
        .map(|key| {
            PresentationArgumentKey::try_new(PresentationArgumentRole::Parameter, Some(key.into()))
                .unwrap()
        })
        .into()
}

fn call(
    limits: RuntimeSchemaLimits,
) -> Result<RuntimePresentationFactoryCall, crate::presentation::RuntimePresentationFactoryCallError>
{
    RuntimePresentationFactoryCall::try_new(
        PresentationCallableId::Image,
        inputs(),
        RuntimeSemanticTypeId::from_bytes([1; 32]),
        RuntimeSemanticTypeId::from_bytes([2; 32]),
        limits,
    )
}

#[test]
fn factory_wire_uses_the_owned_metadata_policy_for_borrowed_keys() {
    let call = Arc::new(call(RuntimeSchemaLimits::engine_default()).unwrap());
    let target = RuntimeCallTarget::PresentationFactory(Arc::clone(&call));
    let mut writer = Writer::default();
    target.write_wire(&mut writer).unwrap();
    let bytes = writer.into_bytes();
    let budget = super::super::AwbcDecodeBudget::default();
    let mut reader = Reader::new(&bytes, &budget);
    assert_eq!(RuntimeCallTarget::read_wire(&mut reader).unwrap(), target);
    reader.finish().unwrap();
    for resource in ["depth", "collection", "string", "metadata", "work"] {
        // Find the exact boundary from the owning grammar. The borrowed wire
        // path must accept the same boundary and reject one less for each bank.
        let policy = |limit| {
            let mut limits = RuntimeSchemaLimits::engine_default();
            match resource {
                "depth" => limits.max_depth = limit as u32,
                "collection" => limits.max_sequence_items = limit as u32,
                "string" => limits.max_string_bytes = limit,
                "metadata" => limits.max_metadata_bytes = limit,
                "work" => limits.max_validation_work = limit,
                _ => unreachable!(),
            }
            limits
        };
        let mut low = 0;
        let mut high = 1024;
        let admit_owned = |limits| {
            crate::value::RuntimeRetainedValueAdmission::standalone(limits)
                .validate_metadata(call.as_ref())
        };
        assert!(admit_owned(policy(high)).is_ok(), "{resource}");
        while low < high {
            let middle = (low + high) / 2;
            if admit_owned(policy(middle)).is_ok() {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        assert!(low > 0, "{resource}");
        for (limit, accepted) in [(low, true), (low - 1, false)] {
            let mut budget = budget;
            budget.original_source.values = policy(limit);
            let mut reader = Reader::new(&bytes, &budget);
            let result = RuntimeCallTarget::read_wire(&mut reader);
            assert_eq!(
                result.is_ok(),
                accepted,
                "{resource} at {limit}: {result:?}"
            );
            if accepted {
                reader.finish().unwrap();
            }
        }
    }
}

#[test]
fn factory_wire_rejects_duplicate_borrowed_parameter_destinations() {
    let mut writer = Writer::default();
    4_u8.write_wire(&mut writer).unwrap();
    PresentationCallableId::Image
        .semantic_tag()
        .write_wire(&mut writer)
        .unwrap();
    [1_u8; 32].write_wire(&mut writer).unwrap();
    [2_u8; 32].write_wire(&mut writer).unwrap();
    writer.write_len(2).unwrap();
    for _ in 0..2 {
        PresentationArgumentRole::Parameter
            .semantic_tag()
            .write_wire(&mut writer)
            .unwrap();
        writer.write_str("channel").unwrap();
    }
    let bytes = writer.into_bytes();
    let budget = super::super::AwbcDecodeBudget::default();
    let mut reader = Reader::new(&bytes, &budget);
    assert!(
        matches!(RuntimeCallTarget::read_wire(&mut reader), Err(AwbcCodecError::InvalidMetadata { message, .. }) if message.contains("duplicate presentation argument"))
    );
}
