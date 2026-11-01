use super::*;
use crate::awbc::codec::AwbcDecodeBudget;
use crate::presentation::{RuntimePresentationSlotRefCall, RuntimePresentationSlotRefInput};
use arcweft_interaction_model::presentation::PresentationResourceKind;

#[test]
fn slot_reference_wire_preserves_the_selected_role_abi_for_every_resource() {
    for resource in PresentationResourceKind::ALL {
        let call = RuntimeCallTarget::PresentationSlotRef(
            RuntimePresentationSlotRefCall::try_new(
                resource,
                [
                    RuntimePresentationSlotRefInput::Slot,
                    RuntimePresentationSlotRefInput::Target,
                ],
            )
            .unwrap(),
        );
        let mut writer = Writer::default();
        call.write_wire(&mut writer).unwrap();
        let bytes = writer.into_bytes();
        let budget = AwbcDecodeBudget::default();
        let mut reader = Reader::new(&bytes, &budget);
        assert_eq!(RuntimeCallTarget::read_wire(&mut reader).unwrap(), call);
        reader.finish().unwrap();
    }
}

#[test]
fn slot_reference_wire_rejects_overlong_metadata_before_reading_its_members() {
    let resource = PresentationResourceKind::Background.semantic_tag();
    let bytes = [3, resource, 5];
    let budget = AwbcDecodeBudget::default();
    let mut reader = Reader::new(&bytes, &budget);
    assert!(matches!(
        RuntimeCallTarget::read_wire(&mut reader),
        Err(AwbcCodecError::BudgetExceeded {
            budget: "slot reference inputs",
            ..
        })
    ));
    for bytes in [
        vec![3, 255, 0],
        vec![3, resource, 1, 255],
        vec![3, resource, 2, 0, 0],
        vec![
            3,
            resource,
            1,
            RuntimePresentationSlotRefInput::Character.semantic_tag(),
        ],
    ] {
        let mut reader = Reader::new(&bytes, &budget);
        assert!(RuntimeCallTarget::read_wire(&mut reader).is_err());
    }
}
