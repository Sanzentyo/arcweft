use super::*;
use arcweft_core::pattern::{RuntimeBuiltinVariantIdentity, RuntimeVariantIdentity};
use arcweft_core::value::{RuntimeAgentCaptureTarget, RuntimeAgentValue};

fn unit(case: RuntimeBuiltinVariantCaseIdentity) -> RuntimeValue {
    RuntimeValue::try_builtin_variant(case, None).unwrap()
}

#[test]
fn capture_requests_retain_typed_format_and_kind_cases() {
    let args = [RuntimePayload::new(RuntimeValue::Agent(
        RuntimeAgentValue::CaptureTarget(RuntimeAgentCaptureTarget::Viewport),
    ))];
    for (format, expected_format) in [
        (
            RuntimeBuiltinVariantCaseIdentity::CaptureFormatPng,
            CaptureFormat::Png,
        ),
        (
            RuntimeBuiltinVariantCaseIdentity::CaptureFormatRawRgba,
            CaptureFormat::RawRgba,
        ),
    ] {
        for (kind, expected_kind) in [
            (RuntimeBuiltinVariantCaseIdentity::CaptureKindColor, "color"),
            (RuntimeBuiltinVariantCaseIdentity::CaptureKindMask, "mask"),
        ] {
            let named = [
                NamedHostArg {
                    name: "format".to_owned(),
                    value: RuntimePayload::new(unit(format)),
                },
                NamedHostArg {
                    name: "kind".to_owned(),
                    value: RuntimePayload::new(unit(kind)),
                },
            ];
            let request = RuntimeAgentArgs::new(&args, &named)
                .capture_request()
                .unwrap();
            assert_eq!(request.format, expected_format);
            assert_eq!(request.capture_kind, expected_kind);
        }
    }
    let default = RuntimeAgentArgs::new(&args, &[]).capture_request().unwrap();
    assert_eq!(default.format, CaptureFormat::Png);
    assert_eq!(default.capture_kind, "color");
}

#[test]
fn pointer_requests_retain_typed_button_cases() {
    let args = [RuntimePayload::new(RuntimeValue::Agent(
        RuntimeAgentValue::ViewportPoint { x: 4, y: 8 },
    ))];
    for (case, button) in [
        (
            RuntimeBuiltinVariantCaseIdentity::PointerButtonPrimary,
            PointerButton::Primary,
        ),
        (
            RuntimeBuiltinVariantCaseIdentity::PointerButtonSecondary,
            PointerButton::Secondary,
        ),
        (
            RuntimeBuiltinVariantCaseIdentity::PointerButtonMiddle,
            PointerButton::Middle,
        ),
    ] {
        let named = [NamedHostArg {
            name: "button".to_owned(),
            value: RuntimePayload::new(unit(case)),
        }];
        assert_eq!(
            RuntimeAgentArgs::new(&args, &named)
                .pointer_click_action()
                .unwrap(),
            AgentAction::PointerClick { x: 4, y: 8, button },
        );
    }
}

#[test]
fn runtime_enum_arguments_reject_labels_wrong_owners_and_malformed_cases() {
    for owner in [
        RuntimeBuiltinVariantIdentity::CaptureFormat,
        RuntimeBuiltinVariantIdentity::CaptureKind,
        RuntimeBuiltinVariantIdentity::PointerButton,
    ] {
        for schema in owner.cases() {
            let value = unit(schema.identity());
            assert_eq!(
                runtime_capture_format(&value).is_ok(),
                owner == RuntimeBuiltinVariantIdentity::CaptureFormat
            );
            assert_eq!(
                runtime_capture_kind(&value).is_ok(),
                owner == RuntimeBuiltinVariantIdentity::CaptureKind
            );
            assert_eq!(
                runtime_pointer_button(&value).is_ok(),
                owner == RuntimeBuiltinVariantIdentity::PointerButton
            );
            let (_, case) = owner.resolve_case(schema.identity()).unwrap();
            let mut bad_ordinal = value.clone();
            let RuntimeValue::Variant { ordinal, .. } = &mut bad_ordinal else {
                unreachable!()
            };
            *ordinal = u32::MAX;
            let mut bad_name = value.clone();
            let RuntimeValue::Variant { name, .. } = &mut bad_name else {
                unreachable!()
            };
            *name = "forged".to_owned();
            let mut bad_payload = value.clone();
            let RuntimeValue::Variant { payload, .. } = &mut bad_payload else {
                unreachable!()
            };
            *payload = Some(Box::new(RuntimeValue::Unit));
            for invalid in [
                RuntimeValue::String(case.name().to_owned()),
                bad_ordinal,
                bad_name,
                bad_payload,
                RuntimeValue::Variant {
                    owner: RuntimeVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
                    ordinal: 0,
                    name: case.name().to_owned(),
                    payload: None,
                },
            ] {
                assert!(runtime_capture_format(&invalid).is_err());
                assert!(runtime_capture_kind(&invalid).is_err());
                assert!(runtime_pointer_button(&invalid).is_err());
            }
            assert!(runtime_string(&value).is_err());
        }
    }
}
