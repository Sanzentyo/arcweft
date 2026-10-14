use super::*;
use crate::awbc::schema::*;
use crate::entry::{RuntimeSchemaLimits, schema::value_encoding::ValueAdmissionOrigin};
use crate::pattern::RuntimeCheckedType;

fn program() -> AwbcProgram {
    AwbcProgram {
        runtime_types: vec![AwbcRuntimeType::new(
            RuntimeCheckedType::String.semantic_identity_digest(),
            AwbcRuntimeTypeShape::String,
        )],
        signatures: vec![AwbcSignature {
            params: vec![],
            result: Some(AwbcTypeId(0)),
            effects: AwbcEffectSetId(0),
        }],
        functions: vec![AwbcFunction {
            definition: crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                *blake3::hash(b"arcweft.terminal-input-test.definition.v1\0").as_bytes(),
            ),
            semantic_role: crate::plan::RuntimeFunctionSemanticRole::Ordinary,
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(0),
            type_context: None,
            input_ownership: vec![],
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 0),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags::default(),
        }],
        frame_layouts: vec![AwbcFrameLayout {
            slots: vec![],
            scopes: vec![],
            max_scope_depth: 0,
        }],
        ..AwbcProgram::default()
    }
}

fn context<V>(program: &AwbcProgram) -> FrameValueContext<'_, V> {
    FrameValueContext {
        program,
        function: AwbcFunctionId(0),
        layout: &program.frame_layouts[0],
        binding: None,
        registers: &[],
        scope_count: 0,
    }
}

#[test]
fn returned_live_and_inert_share_exact_resources_and_original_result_row() {
    let mut program = program();
    let live = FiberTerminalValue::Returned(Some(RuntimeValue::String("cause".into())));
    let inert =
        FiberTerminalValue::Returned(Some(AwbcRuntimeValueSnapshot::String("cause".into())));
    live.validate_summary(None, Some("cause"), RuntimeValue::view)
        .unwrap();
    inert
        .validate_summary(None, Some("cause"), AwbcRuntimeValueSnapshot::view)
        .unwrap();
    assert!(
        live.validate_summary(None, Some("forged"), RuntimeValue::view)
            .is_err()
    );
    assert!(
        inert
            .validate_summary(None, Some("forged"), AwbcRuntimeValueSnapshot::view)
            .is_err()
    );
    let exact = RuntimeSchemaLimits {
        max_nodes: 1,
        max_encoded_bytes: 7,
        max_string_bytes: 5,
        max_validation_work: 5,
        ..RuntimeSchemaLimits::engine_default()
    };
    for origin in [
        ValueAdmissionOrigin::Live,
        ValueAdmissionOrigin::InertSnapshot,
    ] {
        let validate = |limits| {
            let mut resources = ValueResourceAdmission::snapshot(limits, origin);
            match origin {
                ValueAdmissionOrigin::Live => live.validate_inputs(
                    &program,
                    AwbcFiberRoot::Function(AwbcFunctionId(0)),
                    Some(&context(&program)),
                    1,
                    &mut resources,
                    RuntimeValue::view,
                ),
                ValueAdmissionOrigin::InertSnapshot => inert.validate_inputs(
                    &program,
                    AwbcFiberRoot::Function(AwbcFunctionId(0)),
                    Some(&context(&program)),
                    1,
                    &mut resources,
                    AwbcRuntimeValueSnapshot::view,
                ),
            }
        };
        validate(exact).unwrap();
        for short in [
            RuntimeSchemaLimits {
                max_nodes: 0,
                ..exact
            },
            RuntimeSchemaLimits {
                max_encoded_bytes: 6,
                ..exact
            },
            RuntimeSchemaLimits {
                max_string_bytes: 4,
                ..exact
            },
            RuntimeSchemaLimits {
                max_validation_work: 4,
                ..exact
            },
        ] {
            assert!(validate(short).is_err(), "one-over {short:?}");
        }
    }
    program.runtime_types.push(AwbcRuntimeType::new(
        program.runtime_types[0].semantic_identity(),
        AwbcRuntimeTypeShape::Bool,
    ));
    program.signatures[0].result = Some(AwbcTypeId(1));
    assert!(
        inert
            .validate_inputs(
                &program,
                AwbcFiberRoot::Function(AwbcFunctionId(0)),
                Some(&context(&program)),
                1,
                &mut ValueResourceAdmission::snapshot(
                    RuntimeSchemaLimits::engine_default(),
                    ValueAdmissionOrigin::InertSnapshot
                ),
                AwbcRuntimeValueSnapshot::view
            )
            .is_err()
    );
}

#[test]
fn payload_transfer_and_terminal_lifecycle_have_distinct_inert_grammar() {
    for terminal in [
        FiberTerminalValue::<AwbcRuntimeValueSnapshot>::Returned(None),
        FiberTerminalValue::ReturnedTransferred,
        FiberTerminalValue::DialogueResultTransferred,
        FiberTerminalValue::Cancelled,
    ] {
        let json = serde_json::to_value(&terminal).unwrap();
        assert_eq!(
            serde_json::from_value::<FiberTerminalValue<AwbcRuntimeValueSnapshot>>(json).unwrap(),
            terminal
        );
    }
    assert!(
        FiberStatus::Returned
            .validate_retained_shape(
                false,
                Some(&FiberTerminalValue::<AwbcRuntimeValueSnapshot>::Returned(
                    None
                )),
                Some("old transferred label")
            )
            .is_err()
    );
    FiberStatus::Returned
        .validate_retained_shape(
            false,
            Some(&FiberTerminalValue::<AwbcRuntimeValueSnapshot>::ReturnedTransferred),
            Some("label"),
        )
        .unwrap();
    assert!(
        FiberStatus::Running
            .validate_retained_shape(
                false,
                Some(&FiberTerminalValue::<AwbcRuntimeValueSnapshot>::ReturnedTransferred),
                Some("label")
            )
            .is_err()
    );
    assert!(
        FiberStatus::Returned
            .validate_retained_shape(
                true,
                Some(&FiberTerminalValue::<AwbcRuntimeValueSnapshot>::ReturnedTransferred),
                Some("label")
            )
            .is_err()
    );
    assert!(
        serde_json::from_value::<FiberTerminalValue<AwbcRuntimeValueSnapshot>>(
            serde_json::json!({"ReturnedTransferred": {"old_value": "hidden"}})
        )
        .is_err()
    );
}

#[test]
fn empty_payloads_never_masquerade_as_typed_return_values() {
    let program = program();
    let empty = FiberTerminalValue::<AwbcRuntimeValueSnapshot>::Returned(None);
    assert!(
        empty
            .validate_inputs(
                &program,
                AwbcFiberRoot::Function(AwbcFunctionId(0)),
                Some(&context(&program)),
                1,
                &mut ValueResourceAdmission::snapshot(
                    RuntimeSchemaLimits::engine_default(),
                    ValueAdmissionOrigin::InertSnapshot
                ),
                AwbcRuntimeValueSnapshot::view
            )
            .is_err()
    );
    let transferred = FiberTerminalValue::<AwbcRuntimeValueSnapshot>::ReturnedTransferred;
    transferred
        .validate_inputs(
            &program,
            AwbcFiberRoot::Function(AwbcFunctionId(0)),
            Some(&context(&program)),
            1,
            &mut ValueResourceAdmission::snapshot(
                RuntimeSchemaLimits {
                    max_nodes: 0,
                    max_encoded_bytes: 0,
                    ..RuntimeSchemaLimits::engine_default()
                },
                ValueAdmissionOrigin::InertSnapshot,
            ),
            AwbcRuntimeValueSnapshot::view,
        )
        .unwrap();
}
