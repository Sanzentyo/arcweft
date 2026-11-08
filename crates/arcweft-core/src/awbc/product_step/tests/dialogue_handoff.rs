use super::*;
use crate::awbc::fiber::{FiberStateError, FiberSuspensionReason};
use crate::awbc::product_step::{AwbcProductDialoguePhase, dialogue::ProductDialogueReceivers};
use crate::runtime_id::{
    DialogueActivationId, RuntimeFiberInstanceId, RuntimeIdNamespace, RuntimePersistentFiberId,
};

#[test]
fn pending_dialogue_packet_roundtrips_and_failed_handoff_keeps_the_complete_session() {
    let mut executor = AwbcProductStepExecutor::for_entry(
        dialogue_parent_program(
            init_scope_defer_host_call_program(),
            AwbcContentUnitId(0),
            AwbcTypeId(2),
        ),
        AwbcEntryId(0),
        64,
    )
    .unwrap();
    suspend_fixture_dialogue(
        &mut executor,
        vec![RuntimeValue::String("pending capture".into())],
    );
    // Exhaust the actual producer cursor after the VM has transferred its inputs.
    // Handoff must return the entire owned packet even though preparation takes it.
    executor.next_generation = u64::MAX;
    let before = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
    let wire = serde_json::to_vec(&save).unwrap();
    executor = executor
        .restore_inert_snapshot_owned(serde_json::from_slice(&wire).unwrap())
        .unwrap();
    for _ in 0..2 {
        assert!(matches!(
            executor.start_dialogue(),
            Err(ProductStepError::ChildGenerationOverflow)
        ));
        assert_eq!(executor.snapshot().unwrap(), before);
        let FiberSuspensionReason::Dialogue {
            line_task_captures, ..
        } = &executor.fiber.suspension.as_ref().unwrap().reason
        else {
            panic!("the original packet remains pending")
        };
        assert_eq!(
            line_task_captures.as_ref(),
            [RuntimeValue::String("pending capture".into())]
        );
        assert!(executor.dialogues.active_activation().is_none());
    }
}

#[test]
fn active_dialogue_save_rejects_missing_or_replaced_parent_receiver_without_changing_owners() {
    let (mut executor, activation) = initial_actor_dialogue_executor();
    let before = executor.snapshot().unwrap();
    for case in 0..4 {
        let mut saved =
            AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
        match case {
            0 => {
                saved.fiber.suspension = None;
                saved.fiber.status = FiberStatus::Running;
            }
            1 => {
                let replaced = crate::runtime_id::DialogueActivationId::new(
                    activation.artifact(),
                    activation.owner_fiber(),
                    activation.content(),
                    activation.occurrence() + 1,
                );
                saved.fiber.suspension.as_mut().unwrap().reason =
                    FiberSuspensionReason::DialogueActive {
                        activation: replaced,
                    };
            }
            2 => {
                let mut wire = serde_json::to_value(&saved.dialogues).unwrap();
                wire["entries"] = serde_json::json!([]);
                saved.dialogues = serde_json::from_value(wire).unwrap();
            }
            _ => {
                let mut alias = saved.fiber.clone();
                alias.instance = crate::runtime_id::RuntimeFiberInstanceId::from_allocated(
                    saved
                        .next_fiber_instance
                        .take_next(crate::runtime_id::RuntimeIdNamespace::FiberInstance)
                        .unwrap(),
                );
                alias.root = crate::awbc::fiber::AwbcFiberRoot::Function(alias.cursor.function);
                // Its own frame and opcode are valid; only the actual parent /
                // consumer relationship distinguishes this forged waiter.
                alias
                    .admit_retained(
                        &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                            crate::task::RuntimeProgramOwner::Awbc(executor.program_arc()),
                            crate::entry::RuntimeSchemaLimits::engine_default(),
                        ),
                    )
                    .unwrap();
                saved.child_fibers.push(crate::awbc::product_step::AwbcProductChildFiberSaveSnapshot {
                    owner: crate::awbc::product_step::AwbcProductChildFiberOwnerSnapshot::Independent,
                    fiber: alias, runtime_generation: executor.runtime_generation, pending_host_call: None,
                });
            }
        }
        let saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        let (original, error) = executor.restore_inert_snapshot_owned(saved).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&FiberStateError::InvalidDialogueReceiver { activation }.to_string()),
            "case={case}: {error}"
        );
        executor = original;
        assert_eq!(executor.snapshot().unwrap(), before, "case={case}");
    }
}

#[test]
fn dialogue_receiver_topology_checks_nested_edges_in_any_registry_order() {
    let (executor, outer_id) = initial_actor_dialogue_executor();
    let mut image = executor.snapshot().unwrap();
    let mut outer = None;
    image
        .dialogues
        .try_visit_active(|_, frame, _| {
            assert!(outer.replace(frame.clone()).is_none());
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
    let mut outer = outer.unwrap();
    let mut inner = outer.clone();
    let AwbcProductDialoguePhase::Activating { fiber, .. } = &mut outer.phase else {
        panic!("the outer fixture still owns its execution fiber");
    };
    let instance = fiber.instance;
    // Use the fixture's original Dialogue site in the intermediate frame so
    // the edge is checked against a real resume point and result pattern.
    *fiber = image.fiber.clone();
    fiber.instance = instance;
    let inner_id = DialogueActivationId::new(
        outer_id.artifact(),
        RuntimePersistentFiberId::from_allocated(instance.get().get()),
        outer_id.content(),
        0,
    );
    fiber.suspension.as_mut().unwrap().reason = FiberSuspensionReason::DialogueActive {
        activation: inner_id,
    };
    inner.activation = inner_id;
    let AwbcProductDialoguePhase::Activating { fiber, .. } = &mut inner.phase else {
        panic!("the inner fixture retains its allocated activation fiber");
    };
    fiber.instance = RuntimeFiberInstanceId::from_allocated(
        image
            .next_fiber_instance
            .take_next(RuntimeIdNamespace::FiberInstance)
            .unwrap(),
    );

    let mut receivers = ProductDialogueReceivers::default();
    receivers.register_dialogue(inner_id, &inner).unwrap();
    receivers.register_dialogue(outer_id, &outer).unwrap();
    receivers
        .register_fiber(
            &image.fiber,
            super::super::dialogue::ProductDialogueReceiver::Fiber(image.fiber.instance),
        )
        .unwrap();
    assert_eq!(
        receivers.validate(&executor.program),
        Err(FiberStateError::InvalidDialogueReceiver {
            activation: inner_id
        }),
    );
    receivers
        .register_fiber(
            outer.activation_fiber().unwrap(),
            super::super::dialogue::ProductDialogueReceiver::Activation(outer_id),
        )
        .unwrap();
    receivers
        .register_fiber(
            inner.activation_fiber().unwrap(),
            super::super::dialogue::ProductDialogueReceiver::Activation(inner_id),
        )
        .unwrap();
    receivers.validate(&executor.program).unwrap();

    let AwbcProductDialoguePhase::Activating { fiber, .. } = &mut inner.phase else {
        unreachable!("inner phase was established above");
    };
    fiber.instance = instance;
    assert_eq!(
        ProductDialogueReceivers::default().register_dialogue(inner_id, &inner),
        Err(FiberStateError::InvalidDialogueReceiver {
            activation: inner_id
        }),
    );
}
