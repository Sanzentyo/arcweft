use super::*;
use crate::engine::NativeCodeCursor;
use crate::entry::RuntimeSchemaLimits;
use crate::plan::{
    FlowRuntimeId, RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeFlowBodyRole,
    RuntimeFlowOpSeed, RuntimeFlowSchema, RuntimeFlowSeed, RuntimeFunctionDefinitionIdentity,
    RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteDeclarationSeed,
};
use crate::tests::function_application::returning_function_builder;

#[test]
fn native_loop_transfer_image_requires_the_original_loop_and_value() {
    let mut builder = returning_function_builder(RuntimeFunctionSiteBodyKind::Executable);
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let flow = FlowRuntimeId::canonical("test.lexical_transfer").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: vec![],
        })
        .unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([0x79; 32]),
                None,
                Box::new([]),
                unit,
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::Loop {
                    result: None,
                    body: vec![RuntimeFlowOpSeed::Loop {
                        result: None,
                        body: vec![RuntimeFlowOpSeed::Break {
                            depth: 1,
                            value: None,
                        }],
                    }],
                }]),
            },
        ))
        .unwrap();
    let plan = std::sync::Arc::new(builder.finish().unwrap());
    let site = plan.validate_flow_invocation(&flow, &[]).unwrap().site();
    let owner = RuntimeProgramOwner::Plan(plan.clone());
    let admission = || {
        RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            RuntimeSchemaLimits::engine_default(),
        )
    };
    let outer = NativeCodeCursor::root(&plan, crate::plan::code::RuntimeCodeOwner::Function(site))
        .unwrap()
        .source_operation(&plan)
        .unwrap();
    let inner = outer
        .child_body(RuntimeFlowBodyRole::Body, &mut admission())
        .unwrap()
        .source_operation(&plan)
        .unwrap();
    let source = inner
        .child_body(RuntimeFlowBodyRole::Body, &mut admission())
        .unwrap()
        .source_operation(&plan)
        .unwrap();
    let exit = NativeLexicalExit {
        target: 0,
        action: NativeLexicalExitAction::Transfer(NativeLoopTransfer {
            source,
            target: outer,
            kind: NativeLoopTransferKind::Break(RuntimeValue::Unit),
        }),
    };
    exit.admit(&mut admission(), None).unwrap();
    let image = exit.snapshot(&owner).unwrap();
    image.admit(&mut admission(), None).unwrap();
    assert_eq!(image.clone().into_live(&owner).unwrap(), exit);

    let mut wrong_target = image.clone();
    let NativeLexicalExitAction::Transfer(transfer) = &mut wrong_target.action else {
        panic!("image contains its suspended loop transfer");
    };
    transfer.target = inner;
    assert!(wrong_target.admit(&mut admission(), None).is_err());

    let mut wrong_value = image.clone();
    let NativeLexicalExitAction::Transfer(transfer) = &mut wrong_value.action else {
        panic!("image contains its suspended loop transfer");
    };
    transfer.kind = NativeLoopTransferKind::Break(
        AwbcRuntimeValueSnapshot::from_admitted_runtime_value(&RuntimeValue::Bool(true), &owner)
            .unwrap(),
    );
    assert!(wrong_value.admit(&mut admission(), None).is_err());
    assert_eq!(image.into_live(&owner).unwrap(), exit);
}
