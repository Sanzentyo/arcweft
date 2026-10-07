use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeLocalDeclarationSeed, RuntimeLocalReadSeed,
    RuntimeMutablePlaceSeed, RuntimePlan, RuntimePlanBuilder, RuntimePlanSequenceKind,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimePureInputType, RuntimePureOutputType,
    RuntimeTraitMethodIdentity, RuntimeTraitMethodSeed,
};
use crate::pure::VmRuntimePureCallBackend;
use crate::task::NeedId;
use crate::value::{AwbcRuntimeValueSnapshot, RuntimeLocalReadMode, RuntimeSeq};

fn semantic(index: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([index; 32])
}

fn iterator_plan(copy_receiver: bool) -> RuntimePlan {
    let inner = semantic(1);
    let item = semantic(2);
    let payload = semantic(3);
    let option = semantic(4);
    let state = semantic(5);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    inner,
                    RuntimePlanTypeProjection::Signed(crate::value::RuntimeSignedIntWidth::I64),
                ),
                RuntimePlanTypeSeed::new(item, RuntimePlanTypeProjection::Need(inner)),
                RuntimePlanTypeSeed::new(
                    payload,
                    RuntimePlanTypeProjection::Tuple(Box::new([item])),
                ),
                RuntimePlanTypeSeed::new(
                    option,
                    RuntimePlanTypeProjection::Option {
                        item,
                        some_payload: payload,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    state,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item,
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(
                    manual_local_origin(
                        "arcweft-core.fixture.engine.flow.iterator_tests.iterator_plan.binding_a",
                    ),
                    state,
                ),
                RuntimeLocalDeclarationSeed::new(
                    manual_local_origin(
                        "arcweft-core.fixture.engine.flow.iterator_tests.iterator_plan.binding_b",
                    ),
                    state,
                ),
            ],
        )
        .expect("iterator state and Option item type admit");
    let receiver = admission.local_ids()[0].clone();
    let body = RuntimeExprSeed::new(
        option,
        RuntimeExprSeedKind::SequencePopFront {
            place: RuntimeMutablePlaceSeed::Local(receiver.clone()),
        },
    );
    let body = if copy_receiver {
        RuntimeExprSeed::new(
            option,
            RuntimeExprSeedKind::Let {
                binding: admission.local_ids()[1].clone(),
                expr: Box::new(RuntimeExprSeed::new(
                    state,
                    RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                        receiver.clone(),
                        RuntimeLocalReadMode::Copy,
                    )),
                )),
                body: Box::new(body),
            },
        )
    } else {
        body
    };
    builder
        .push_trait_method_seed(RuntimeTraitMethodSeed {
            definition: crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                [51; 32],
            ),
            identity: RuntimeTraitMethodIdentity {
                impl_id: 0,
                trait_id: Some(0),
                witness: Some(0),
                trait_name: Some("Iterator".to_owned()),
                self_type: "Vec<Need<i64>>".to_owned(),
                method_name: "next".to_owned(),
                monomorph_label: "Vec<Need<i64>>::next".to_owned(),
            },
            receiver: RuntimeReceiverMode::MutRef,
            inputs: Box::new([crate::plan::RuntimeCallableParameterSeed {
                identity: crate::plan::RuntimeFunctionParameterIdentity::from_accepted_identity(
                    [91; 32],
                ),
                local: receiver,
                passing: crate::plan::RuntimeFunctionParameterPassing::Affine,
                abi: RuntimePureInputType::Value,
            }]),
            output_abi: RuntimePureOutputType::Value,
            body,
        })
        .expect("selected Iterator::next method admits");
    builder.finish().expect("iterator plan seals")
}

fn witness_with_need(plan: &RuntimePlan) -> RuntimeIterator {
    RuntimeIterator::witness(
        RuntimeValue::Seq(RuntimeSeq::values(vec![
            RuntimeValue::NeedHandle(crate::tests::reusable_need("need.iterator.first")),
            RuntimeValue::NeedHandle(crate::tests::reusable_need("need.iterator.second")),
        ])),
        plan.trait_methods()[0].id,
    )
}

fn remaining_need_identity_ptr(iterator: &RuntimeIterator) -> *const u8 {
    let RuntimeIterator::Witness { state, .. } = iterator else {
        panic!("selected witness state must remain a witness iterator");
    };
    let RuntimeValue::Seq(RuntimeSeq::Values(values)) = state.as_ref() else {
        panic!("the affine Vec must remain the live witness state");
    };
    let Some(RuntimeValue::NeedHandle(handle)) = values.last() else {
        panic!("the remaining affine Need handle must stay in the witness state");
    };
    handle.spec() as *const crate::task::TaskSpec as *const u8
}

#[test]
fn witness_next_moves_affine_receiver_and_snapshot_restores_the_selected_state() {
    let plan = iterator_plan(false);
    let mut engine = Engine::new(plan);
    let mut iterator = witness_with_need(engine.program_plan().as_ref());
    let original = remaining_need_identity_ptr(&iterator);
    let mut backend = VmRuntimePureCallBackend::default();

    assert_eq!(
        engine.next_runtime_iterator_item(&mut iterator, &mut backend),
        Ok(Some(RuntimeValue::NeedHandle(crate::tests::reusable_need(
            "need.iterator.first"
        ))))
    );
    assert_eq!(
        remaining_need_identity_ptr(&iterator),
        original,
        "Iterator::next must retain the original affine remainder allocation"
    );
    let RuntimeIterator::Witness { state, .. } = &iterator else {
        panic!("the selected witness remains active after its first item");
    };
    assert!(!state.ownership().permits_copy());

    let owner = crate::task::RuntimeProgramOwner::Plan(engine.program_plan());
    let snapshot = AwbcRuntimeValueSnapshot::from_runtime_iterator_for_program(&iterator, &owner)
        .expect("the live witness state has a typed rollback image");
    drop(iterator);
    let bytes = serde_json::to_vec(&snapshot).expect("rollback image serializes");
    let decoded =
        serde_json::from_slice::<AwbcRuntimeValueSnapshot>(&bytes).expect("rollback image decodes");
    let mut iterator = decoded
        .into_runtime_iterator_for_program(&owner)
        .expect("the selected witness state restores under its plan lease");
    assert_eq!(
        engine.next_runtime_iterator_item(&mut iterator, &mut backend),
        Ok(Some(RuntimeValue::NeedHandle(crate::tests::reusable_need(
            "need.iterator.second"
        ))))
    );
    assert_eq!(
        engine.next_runtime_iterator_item(&mut iterator, &mut backend),
        Ok(None)
    );
}

#[test]
fn witness_next_rejects_a_copy_of_its_affine_receiver() {
    let plan = iterator_plan(true);
    let mut engine = Engine::new(plan);
    let mut iterator = witness_with_need(engine.program_plan().as_ref());
    assert!(matches!(
        engine.next_runtime_iterator_item(&mut iterator, &mut VmRuntimePureCallBackend::default()),
        Err(RuntimeEvalError::AffineLocalCopy(_))
    ));
}

#[cfg(test)]
fn manual_local_origin(declaration: &str) -> crate::plan::RuntimeLocalOrigin {
    // This fixture declares a semantic binding name independent of its value,
    // type, source offset, and builder-issued local ordinal.
    let mut identity = blake3::Hasher::new();
    identity.update(b"arcweft.manual-fixture-binding.v1\0");
    identity.update(declaration.as_bytes());
    crate::plan::RuntimeLocalOrigin::Binding(*identity.finalize().as_bytes())
}
