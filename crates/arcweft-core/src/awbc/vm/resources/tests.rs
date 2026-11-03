use super::*;
use crate::{
    awbc::{schema::*, vm::*},
    entry::RuntimeSchemaLimits,
    pattern::{RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId},
    presentation::RuntimePresentationFactoryCall,
    value::{RuntimeCallTarget, RuntimeValue},
};
use arcweft_interaction_model::presentation::{
    PresentationArgumentKey, PresentationArgumentRole, PresentationCallableId,
    PresentationResourceKind,
};
use std::sync::Arc;

fn program() -> AwbcProgram {
    let mut program = crate::awbc::tests::minimal_program();
    let string = RuntimeSemanticTypeId::from_bytes([71; 32]);
    let resource = RuntimeSemanticTypeId::from_bytes([72; 32]);
    let result = RuntimeSemanticTypeId::from_bytes([73; 32]);
    let owner =
        RuntimeOpaqueTypeOwner::presentation_handle(PresentationResourceKind::Image, result);
    let inputs: Box<[PresentationArgumentKey]> = Box::new([PresentationArgumentKey::try_new(
        PresentationArgumentRole::Parameter,
        Some("channel".into()),
    )
    .unwrap()]);
    let body = RuntimePresentationFactoryCall::retained_body_identity(
        PresentationCallableId::Image,
        &inputs,
        resource,
        &crate::plan::RuntimeTypeScope::root(),
        1,
        |_| Some(string),
    )
    .unwrap();
    program.strings.push(owner.producer().as_str().into());
    program.runtime_types = vec![
        AwbcRuntimeType::new(string, AwbcRuntimeTypeShape::String),
        AwbcRuntimeType::new(
            resource,
            AwbcRuntimeTypeShape::PresentationResource(PresentationResourceKind::Image),
        ),
        AwbcRuntimeType::new(body, AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0)])),
        AwbcRuntimeType::new(
            result,
            AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId(1),
                admission: owner.admission(),
                value_class: owner.value_class(),
                persistence: owner.persistence(),
                arguments: vec![],
            },
        ),
        AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([74; 32]),
            AwbcRuntimeTypeShape::String,
        ),
    ];
    program.signatures.push(AwbcSignature {
        params: vec![AwbcTypeId(0)],
        result: Some(AwbcTypeId(3)),
        effects: AwbcEffectSetId(0),
    });
    program.intrinsics.push(AwbcIntrinsic {
        identity: RuntimeCallTarget::PresentationFactory(Arc::new(
            RuntimePresentationFactoryCall::try_new(
                PresentationCallableId::Image,
                inputs,
                resource,
                body,
                RuntimeSchemaLimits::engine_default(),
            )
            .unwrap(),
        )),
        signature: AwbcSignatureId(1),
        revision: 1,
    });
    program.frame_layouts[0].slots = [AwbcTypeId(0), AwbcTypeId(3)]
        .into_iter()
        .map(|ty| AwbcFrameSlot {
            name: None,
            ty,
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        })
        .collect();
    program.canonicalize_string_table();
    program
}

#[test]
fn factory_call_rejects_invalid_metadata_and_missing_authority_before_moving_captures() {
    for rejection in [
        "context",
        "signature",
        "count",
        "input identity",
        "register identity",
        "destination",
        "destination bounds",
        "destination type",
        "result owner",
        "explicit scope",
    ] {
        let mut program = program();
        let mut args = vec![AwbcRegisterId(0)];
        let mut dst = Some(AwbcRegisterId(1));
        let expected = match rejection {
            "context" => "execution context",
            "signature" => {
                program.signatures[1].result = None;
                "no result type"
            }
            "count" => {
                args.clear();
                "invalid call signature"
            }
            "input identity" => {
                program.signatures[1].params[0] = AwbcTypeId(4);
                "requires type"
            }
            "register identity" => {
                program.frame_layouts[0].slots[0].ty = AwbcTypeId(4);
                "requires type"
            }
            "destination" => {
                dst = None;
                "invalid call signature"
            }
            "destination bounds" => {
                dst = Some(AwbcRegisterId(99));
                "register 99"
            }
            "destination type" => {
                program.frame_layouts[0].slots[1].ty = AwbcTypeId(0);
                "different type"
            }
            "result owner" => {
                program.signatures[1].result = Some(AwbcTypeId(0));
                program.frame_layouts[0].slots[1].ty = AwbcTypeId(0);
                "invalid Handle result"
            }
            "explicit scope" => {
                let RuntimeCallTarget::PresentationFactory(call) = &program.intrinsics[0].identity
                else {
                    unreachable!()
                };
                // Explicit scope belongs to the slotted Background grammar;
                // Image deliberately has no such input role.
                let mut inputs = vec![
                    PresentationArgumentKey::try_new(PresentationArgumentRole::Slot, None).unwrap(),
                ];
                inputs.push(
                    PresentationArgumentKey::try_new(PresentationArgumentRole::Scope, None)
                        .unwrap(),
                );
                let body = RuntimePresentationFactoryCall::retained_body_identity(
                    PresentationCallableId::Background,
                    &inputs,
                    call.resource_type(),
                    &crate::plan::RuntimeTypeScope::root(),
                    1,
                    |_| Some(program.runtime_types[0].semantic_identity()),
                )
                .unwrap();
                let replacement = RuntimePresentationFactoryCall::try_new(
                    PresentationCallableId::Background,
                    inputs.into_boxed_slice(),
                    call.resource_type(),
                    body,
                    RuntimeSchemaLimits::engine_default(),
                )
                .unwrap();
                program.runtime_types[2] =
                    AwbcRuntimeType::new(body, AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0)]));
                let owner = RuntimeOpaqueTypeOwner::presentation_handle(
                    PresentationResourceKind::Background,
                    program.runtime_types[3].semantic_identity(),
                );
                let producer = AwbcStringId(program.strings.len() as u32);
                program.strings.push(owner.producer().as_str().into());
                program.runtime_types[1] = AwbcRuntimeType::new(
                    call.resource_type(),
                    AwbcRuntimeTypeShape::PresentationResource(
                        PresentationResourceKind::Background,
                    ),
                );
                program.runtime_types[3] = AwbcRuntimeType::new(
                    owner.semantic_identity(),
                    AwbcRuntimeTypeShape::Opaque {
                        producer,
                        admission: owner.admission(),
                        value_class: owner.value_class(),
                        persistence: owner.persistence(),
                        arguments: vec![],
                    },
                );
                program.intrinsics[0].identity =
                    RuntimeCallTarget::PresentationFactory(Arc::new(replacement));
                program.signatures[1].params.push(AwbcTypeId(0));
                args.push(AwbcRegisterId(0));
                program.canonicalize_string_table();
                "actual lexical scope capability"
            }
            _ => unreachable!(),
        };
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 64).unwrap();
        let input = String::from("retained capture allocation");
        let pointer = input.as_ptr();
        fiber
            .active_frame_mut()
            .unwrap()
            .set_register(AwbcRegisterId(0), RuntimeValue::String(input))
            .unwrap();
        let before_cursor = fiber.cursor;
        let mut storage = None;
        let mut execution = VmExecutionResources {
            context: None,
            presentation: Some(&mut storage),
            actions: None,
        };
        let mut observations = Vec::new();
        let error = execute_instruction(
            &program,
            &mut fiber,
            &mut RejectingVmHost,
            &mut execution,
            &AwbcInstruction::CallIntrinsic {
                dst,
                intrinsic: AwbcIntrinsicId(0),
                args,
            },
            None,
            &mut observations,
        )
        .unwrap_err();
        assert!(error.to_string().contains(expected), "{rejection}: {error}");
        let RuntimeValue::String(input) = fiber
            .active_frame()
            .unwrap()
            .register(AwbcRegisterId(0))
            .unwrap()
        else {
            panic!("{rejection}: original capture remains owned by its register");
        };
        assert_eq!(input, "retained capture allocation", "{rejection}");
        assert_eq!(input.as_ptr(), pointer, "{rejection}");
        assert!(
            fiber
                .active_frame()
                .unwrap()
                .register(AwbcRegisterId(1))
                .is_err(),
            "{rejection}"
        );
        assert_eq!(fiber.cursor, before_cursor, "{rejection}");
        assert!(observations.is_empty(), "{rejection}");
        assert!(storage.is_none(), "{rejection}");
    }
}
