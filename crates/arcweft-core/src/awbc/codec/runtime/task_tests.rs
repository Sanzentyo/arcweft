use super::*;
use crate::awbc::codec::AwbcDecodeBudget;
use crate::task::TaskHandleType;

#[test]
fn program_capture_modes_are_required_and_unknown_wire_tags_are_rejected() {
    use crate::value::RuntimeLocalReadMode as Mode;
    let row = AwbcProgramTaskPlan {
        delivery: crate::task::ProgramTaskDelivery::HostMetadata,
        source: crate::task::ExpectedTaskPlanKey::new([0x36; 32]),
        kernel: AwbcFunctionId(0),
        capture_modes: vec![Mode::Move, Mode::Copy],
    };
    let mut writer = Writer::default();
    let execution = AwbcTaskPlanExecution::Program(row.clone());
    execution.write_wire(&mut writer).unwrap();
    let bytes = writer.into_bytes();
    let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
    assert_eq!(
        AwbcTaskPlanExecution::read_wire(&mut reader).unwrap(),
        execution
    );
    reader.finish().unwrap();
    let mut json = serde_json::to_value(&row).unwrap();
    assert_eq!(
        serde_json::from_value::<AwbcProgramTaskPlan>(json.clone()).unwrap(),
        row
    );
    json.as_object_mut().unwrap().remove("capture_modes");
    assert!(serde_json::from_value::<AwbcProgramTaskPlan>(json).is_err());
    for tag in 2..=u8::MAX {
        let bytes = [tag];
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert!(
            matches!(Mode::read_wire(&mut reader), Err(AwbcCodecError::UnknownTag { kind: "Program capture mode", tag: actual, offset: 0 }) if actual == tag)
        );
    }
}

#[test]
fn program_delivery_is_required_and_unknown_wire_tags_are_rejected_before_payload() {
    use crate::task::ProgramTaskDelivery as Delivery;
    for delivery in [Delivery::HostMetadata, Delivery::LocalOnly] {
        let row = AwbcProgramTaskPlan {
            source: crate::task::ExpectedTaskPlanKey::new([0x37; 32]),
            kernel: AwbcFunctionId(0),
            delivery,
            capture_modes: vec![],
        };
        let execution = AwbcTaskPlanExecution::Program(row.clone());
        let mut writer = Writer::default();
        execution.write_wire(&mut writer).unwrap();
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(
            AwbcTaskPlanExecution::read_wire(&mut reader).unwrap(),
            execution
        );
        reader.finish().unwrap();
        let mut json = serde_json::to_value(&row).unwrap();
        assert_eq!(
            serde_json::from_value::<AwbcProgramTaskPlan>(json.clone()).unwrap(),
            row
        );
        json.as_object_mut().unwrap().remove("delivery");
        assert!(serde_json::from_value::<AwbcProgramTaskPlan>(json).is_err());
    }
    for tag in 2..=u8::MAX {
        let bytes = [tag];
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert!(
            matches!(Delivery::read_wire(&mut reader), Err(AwbcCodecError::UnknownTag { kind: "Program task delivery", tag: actual, offset: 0 }) if actual == tag)
        );
    }
}

#[test]
fn task_handle_type_codec_preserves_carrier_and_payload() {
    for (handle_type, expected) in [
        (
            TaskHandleType::Need {
                payload: AwbcTypeId(41),
            },
            [0, 41],
        ),
        (
            TaskHandleType::ThreadHandle {
                payload: AwbcTypeId(41),
            },
            [1, 41],
        ),
    ] {
        let mut writer = Writer::default();
        handle_type.write_wire(&mut writer).unwrap();
        let bytes = writer.into_bytes();
        assert_eq!(bytes, expected);
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(TaskHandleType::read_wire(&mut reader).unwrap(), handle_type);
        reader.finish().unwrap();
    }
}

#[test]
fn task_handle_type_codec_rejects_unknown_tags_before_payload() {
    for tag in 2..=u8::MAX {
        let bytes = [tag];
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert!(matches!(
            TaskHandleType::<AwbcTypeId>::read_wire(&mut reader),
            Err(AwbcCodecError::UnknownTag {
                kind: "task handle type",
                tag: actual,
                offset: 0,
            }) if actual == tag
        ));
    }
    for tag in [0, 1] {
        let bytes = [tag];
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert!(TaskHandleType::<AwbcTypeId>::read_wire(&mut reader).is_err());
    }
}

#[test]
fn task_row_codecs_require_full_carrier_independently_of_program_execution() {
    let mut encodings = Vec::new();
    for handle_type in [
        TaskHandleType::Need {
            payload: AwbcTypeId(41),
        },
        TaskHandleType::ThreadHandle {
            payload: AwbcTypeId(41),
        },
    ] {
        let row = AwbcTaskPlan {
            class: AwbcTaskClass::Cpu,
            priority: 3,
            cancel_scope: AwbcStringId(0),
            policy: AwbcTaskPolicy::AlwaysStart,
            handle_type,
            execution: AwbcTaskPlanExecution::Program(AwbcProgramTaskPlan {
                delivery: crate::task::ProgramTaskDelivery::HostMetadata,
                source: crate::task::ExpectedTaskPlanKey::new([0x35; 32]),
                kernel: AwbcFunctionId(0),
                capture_modes: vec![],
            }),
        };
        let mut writer = Writer::default();
        row.write_wire(&mut writer).unwrap();
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(AwbcTaskPlan::read_wire(&mut reader).unwrap(), row);
        reader.finish().unwrap();
        encodings.push(bytes);

        let mut json = serde_json::to_value(&row).unwrap();
        assert_eq!(
            serde_json::from_value::<AwbcTaskPlan>(json.clone()).unwrap(),
            row
        );
        json["payload_type"] = serde_json::json!(41);
        assert!(serde_json::from_value::<AwbcTaskPlan>(json.clone()).is_err());
        json.as_object_mut().unwrap().remove("handle_type");
        assert!(serde_json::from_value::<AwbcTaskPlan>(json).is_err());
    }
    assert_ne!(encodings[0], encodings[1]);
}
