use super::*;
use crate::awbc::codec::AwbcDecodeBudget;
use crate::task::TaskHandleType;

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
                source: crate::task::ExpectedTaskPlanKey::new([0x35; 32]),
                kernel: AwbcFunctionId(0),
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
