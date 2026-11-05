use super::*;
use crate::{
    CharacterDialogueRichTextProperty as Property, CharacterDialogueRichTextPropertyValue as Value,
};
use arcweft_presentation::rich_text::{
    PresentationReusableStylePropertyId as Id, RichTextFontFamily,
};
use arcweft_rich_text_schema::{RichTextEnumDomain, RichTextUnit};

#[test]
fn call_property_construction_keeps_original_result_type_and_resource_limits() {
    use arcweft_core::pure::{RuntimeExternalCallContext, RuntimeProgramExecutionContext};
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    let [RuntimeProgramOwner::Plan(plan), _] = types.program_owners() else {
        panic!("native fixture")
    };
    let owner = RuntimeProgramOwner::Plan(Arc::clone(&plan));
    let schema = types.schema(&characters, &views, &custom, owner);
    let execution = RuntimeProgramExecutionContext::native(plan, Default::default());
    let context = |result, limits| {
        RuntimeExternalCallContext::for_program(execution.clone(), [], result, limits).unwrap()
    };
    let property = Property::try_new(
        Id::Font,
        Value::FontFamily(RichTextFontFamily::Named {
            name: "Noto Sans JP",
        }),
    )
    .unwrap();
    let admitted = schema
        .construct_rich_text_properties_in_call(
            &context(
                role_semantic(Role::RichText),
                RuntimeSchemaLimits::engine_default(),
            ),
            &[property.clone()],
        )
        .unwrap();
    assert!(matches!(admitted, RuntimeValue::Opaque(_)));
    let mut limits = RuntimeSchemaLimits::engine_default();
    limits.max_string_bytes = 1;
    assert!(
        schema
            .construct_rich_text_properties_in_call(
                &context(role_semantic(Role::RichText), limits),
                &[property]
            )
            .is_err()
    );
    assert!(matches!(
        schema
            .construct_rich_text_properties_in_call(
                &context(
                    role_semantic(Role::Stage),
                    RuntimeSchemaLimits::engine_default()
                ),
                &[],
            )
            .map_err(|e| *e),
        Err(CharacterDialogueValueError::ExternalCallResultType { .. })
    ));
    assert!(matches!(
        schema
            .construct_rich_text_properties_in_call(&RuntimeExternalCallContext::unbound(), &[])
            .map_err(|e| *e),
        Err(CharacterDialogueValueError::ExternalCallContextRequired)
    ));
    let [RuntimeProgramOwner::Plan(foreign), _] = Types::new().program_owners() else {
        panic!("native fixture")
    };
    let foreign = RuntimeExternalCallContext::for_program(
        RuntimeProgramExecutionContext::native(foreign, Default::default()),
        [],
        role_semantic(Role::RichText),
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    assert!(matches!(
        schema
            .construct_rich_text_properties_in_call(&foreign, &[])
            .map_err(|e| *e),
        Err(CharacterDialogueValueError::ForeignProgramOwner)
    ));
}

#[test]
fn borrowed_properties_construct_and_restore_under_both_exact_programs() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    for program in types.program_owners() {
        let schema = types.schema(&characters, &views, &custom, program.clone());
        let value = {
            let font = String::from("Noto Sans JP");
            let properties = [
                Property::try_new(
                    Id::RubySize,
                    Value::Length {
                        milli: 10_000,
                        unit: RichTextUnit::Px,
                    },
                )
                .unwrap(),
                Property::try_new(
                    Id::Font,
                    Value::FontFamily(RichTextFontFamily::Named {
                        name: font.as_str(),
                    }),
                )
                .unwrap(),
                Property::try_new(
                    Id::RubySize,
                    Value::Length {
                        milli: 11_000,
                        unit: RichTextUnit::Px,
                    },
                )
                .unwrap(),
            ];
            schema
                .construct_rich_text_properties(&program, &properties)
                .unwrap()
        };
        let opaque = arcweft_core::value::RuntimeBorrowedValue::from(&value)
            .opaque()
            .unwrap();
        let codec = CharacterDialogueRolePayloadCodec::RichTextProperties;
        let borrowed = codec.borrow_properties(opaque.payload()).unwrap();
        assert_eq!(borrowed.len(), 2);
        assert_eq!(
            borrowed
                .iter()
                .find(|property| property.id() == Id::Font)
                .unwrap()
                .value(),
            &Value::FontFamily(RichTextFontFamily::Named {
                name: "Noto Sans JP"
            })
        );
        assert_eq!(
            borrowed
                .iter()
                .find(|property| property.id() == Id::RubySize)
                .unwrap()
                .value(),
            &Value::Length {
                milli: 11_000,
                unit: RichTextUnit::Px
            }
        );
        let image = arcweft_core::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
            &value, &program,
        )
        .unwrap();
        let bytes = serde_json::to_vec(&image).unwrap();
        let decoded: arcweft_core::value::AwbcRuntimeValueSnapshot =
            serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            decoded.into_runtime_value_for_program(&program).unwrap(),
            value
        );
        let empty = schema
            .construct_rich_text_properties(&program, &[])
            .unwrap();
        let RuntimeValue::Opaque(empty_opaque) = &empty else {
            panic!("RichText construction returns its exact opaque role");
        };
        assert_eq!(
            empty_opaque.payload(),
            &codec.no_overrides_payload().unwrap()
        );
        assert!(
            codec
                .borrow_properties(
                    arcweft_core::value::RuntimeBorrowedValue::from(&empty)
                        .opaque()
                        .unwrap()
                        .payload()
                )
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn borrowed_property_admission_rejects_erased_domains_units_and_limits() {
    assert!(
        Property::try_new(
            Id::WritingMode,
            Value::ClosedEnum {
                domain: RichTextEnumDomain::LayoutDirection.domain_id(),
                variant: 0,
            }
        )
        .is_err()
    );
    assert!(
        Property::try_new(
            Id::RubySize,
            Value::Length {
                milli: 11_000,
                unit: RichTextUnit::Deg
            }
        )
        .is_err()
    );
    for unit in [RichTextUnit::Px, RichTextUnit::Pt] {
        let property = Property::try_new(
            Id::RubySize,
            Value::Length {
                milli: 11_000,
                unit,
            },
        )
        .unwrap();
        assert_eq!(
            property.value(),
            &Value::Length {
                milli: 11_000,
                unit
            }
        );
    }
    assert!(Property::try_new(Id::Opacity, Value::Ratio(1001)).is_err());
    assert!(Property::try_new(Id::Font, Value::Text("serif")).is_err());
    let maximum = "x".repeat(256);
    assert!(
        Property::try_new(
            Id::Font,
            Value::FontFamily(RichTextFontFamily::Named { name: &maximum })
        )
        .is_ok()
    );
    for invalid in [
        String::new(),
        "x".repeat(257),
        "夢".repeat(86),
        "x".repeat(4097),
    ] {
        assert!(
            Property::try_new(
                Id::Font,
                Value::FontFamily(RichTextFontFamily::Named { name: &invalid })
            )
            .is_err()
        );
    }
}

#[test]
fn rich_text_producer_rejects_foreign_executables_and_excess_inputs() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    let foreign = Types::new().program_owners();
    for (program, foreign) in types.program_owners().into_iter().zip(foreign) {
        let schema = types.schema(&characters, &views, &custom, program.clone());
        assert!(matches!(
            schema
                .construct_rich_text_properties(&foreign, &[])
                .map_err(|error| *error),
            Err(CharacterDialogueValueError::ForeignProgramOwner)
        ));
        let property = Property::try_new(
            Id::Font,
            Value::FontFamily(RichTextFontFamily::Named {
                name: "Noto Sans JP",
            }),
        )
        .unwrap();
        let maximum = usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_structured_leaves);
        let excessive = vec![property; maximum + 1];
        assert!(
            matches!(schema.construct_rich_text_properties(&program,&excessive).map_err(|error| *error),
            Err(CharacterDialogueValueError::Limit{limit:"rich_text_property_inputs",maximum:observed}) if observed==maximum)
        );
        assert!(schema.construct_rich_text_properties(&program, &[]).is_ok());
    }
}

#[test]
fn font_families_keep_generic_and_named_identity_in_borrowed_and_saved_properties() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    for program in types.program_owners() {
        let schema = types.schema(&characters, &views, &custom, program.clone());
        for family in RichTextFontFamily::<&str>::GENERIC.into_iter().chain([
            RichTextFontFamily::Named { name: "serif" },
            RichTextFontFamily::Named {
                name: "Noto Sans JP",
            },
        ]) {
            let property = Property::try_new(Id::Font, Value::FontFamily(family.clone())).unwrap();
            let value = schema
                .construct_rich_text_properties(&program, &[property])
                .unwrap();
            let snapshot =
                arcweft_core::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
                    &value, &program,
                )
                .unwrap();
            let bytes = serde_json::to_vec(&snapshot).unwrap();
            let restored: arcweft_core::value::AwbcRuntimeValueSnapshot =
                serde_json::from_slice(&bytes).unwrap();
            let restored = restored.into_runtime_value_for_program(&program).unwrap();
            assert_eq!(restored, value);
            let payload = arcweft_core::value::RuntimeBorrowedValue::from(&restored)
                .opaque()
                .unwrap()
                .payload();
            let properties = CharacterDialogueRolePayloadCodec::RichTextProperties
                .borrow_properties(payload)
                .unwrap();
            assert_eq!(properties.len(), 1);
            assert_eq!(
                properties.iter().next().unwrap().value(),
                &Value::FontFamily(family)
            );
        }
    }
}

#[test]
fn stored_font_properties_reject_erased_unknown_and_malformed_family_payloads() {
    use arcweft_core::value::RuntimeUInt;
    use arcweft_presentation::rich_text::PRESENTATION_CONTENT_CALLABLE_CATALOG;

    let codec = CharacterDialogueRolePayloadCodec::RichTextProperties;
    let schema = PRESENTATION_CONTENT_CALLABLE_CATALOG
        .reusable_style_schema()
        .unwrap();
    let font = schema
        .properties()
        .iter()
        .position(|field| field.id == Id::Font)
        .unwrap();
    let tuple = |ordinal, name: &str| {
        RuntimeValue::Tuple(vec![
            RuntimeValue::UInt(RuntimeUInt::U32(ordinal)),
            RuntimeValue::String(name.into()),
        ])
    };
    for malformed in [
        RuntimeValue::String("serif".into()),
        tuple(0, "serif"),
        tuple(RichTextFontFamily::<&str>::NAMED_ORDINAL, ""),
        tuple(6, ""),
        tuple(u32::MAX, "serif"),
        tuple(RichTextFontFamily::<&str>::NAMED_ORDINAL, &"x".repeat(257)),
        RuntimeValue::Tuple(vec![RuntimeValue::UInt(RuntimeUInt::U32(0))]),
        RuntimeValue::Tuple(vec![
            RuntimeValue::UInt(RuntimeUInt::U16(0)),
            RuntimeValue::String(String::new()),
        ]),
    ] {
        let mut payload = codec.no_overrides_payload().unwrap();
        let RuntimeValue::Tuple(slots) = &mut payload else {
            panic!("property tuple")
        };
        slots[font] = RuntimeValue::option_some(malformed);
        assert!(codec.decode_properties(&payload).is_err());
        assert!(codec.borrow_properties(&payload).is_err());
    }
}
