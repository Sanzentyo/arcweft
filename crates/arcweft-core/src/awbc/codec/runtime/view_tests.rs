use super::*;
use crate::{
    entry::RuntimeSchemaLimits,
    pattern::RuntimeSemanticTypeId,
    presentation::{RuntimePresentationViewCall, RuntimePresentationViewIdentity},
};
use arcweft_id::view::ViewId;
use std::sync::Arc;

fn call() -> RuntimePresentationViewCall {
    RuntimePresentationViewCall::try_new(
        ViewId::try_new("view.card").unwrap(),
        RuntimePresentationViewIdentity::from_checked_declaration([3; 32]).unwrap(),
        4,
        Box::new([2, 0]),
        RuntimeSemanticTypeId::from_bytes([1; 32]),
        RuntimeSemanticTypeId::from_bytes([2; 32]),
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap()
}

#[test]
fn view_wire_retains_omission_and_supplied_parameter_order() {
    let target = RuntimeCallTarget::PresentationView(Arc::new(call()));
    let mut writer = Writer::default();
    target.write_wire(&mut writer).unwrap();
    let bytes = writer.into_bytes();
    let budget = super::super::AwbcDecodeBudget::default();
    let mut reader = Reader::new(&bytes, &budget);
    assert_eq!(RuntimeCallTarget::read_wire(&mut reader).unwrap(), target);
    reader.finish().unwrap();
}

#[test]
fn view_wire_uses_the_owned_metadata_boundary_before_copying_inputs() {
    let call = Arc::new(call());
    let target = RuntimeCallTarget::PresentationView(Arc::clone(&call));
    let mut writer = Writer::default();
    target.write_wire(&mut writer).unwrap();
    let bytes = writer.into_bytes();
    for resource in ["depth", "collection", "string", "metadata", "work"] {
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
        let admit = |limits| {
            RuntimePresentationViewCall::try_new(
                call.view().clone(),
                call.definition(),
                call.parameters(),
                call.supplied().into(),
                call.resource_type(),
                call.body_type(),
                limits,
            )
        };
        let mut low = 0;
        let mut high = 4096;
        assert!(admit(policy(high)).is_ok());
        while low < high {
            let middle = (low + high) / 2;
            if admit(policy(middle)).is_ok() {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        assert!(low > 0);
        for (limit, accepted) in [(low, true), (low - 1, false)] {
            let mut budget = super::super::AwbcDecodeBudget::default();
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
fn view_wire_rejects_repeated_and_out_of_declaration_destinations() {
    for supplied in [[0, 0], [0, 4]] {
        let original = call();
        let mut writer = Writer::default();
        5_u8.write_wire(&mut writer).unwrap();
        writer.write_str(original.view().as_str()).unwrap();
        original
            .definition()
            .as_bytes()
            .write_wire(&mut writer)
            .unwrap();
        original.parameters().write_wire(&mut writer).unwrap();
        original
            .resource_type()
            .as_bytes()
            .write_wire(&mut writer)
            .unwrap();
        original
            .body_type()
            .as_bytes()
            .write_wire(&mut writer)
            .unwrap();
        writer.write_table(&supplied).unwrap();
        let bytes = writer.into_bytes();
        let budget = super::super::AwbcDecodeBudget::default();
        let mut reader = Reader::new(&bytes, &budget);
        assert!(matches!(
            RuntimeCallTarget::read_wire(&mut reader),
            Err(AwbcCodecError::InvalidMetadata { .. })
        ));
    }
}
