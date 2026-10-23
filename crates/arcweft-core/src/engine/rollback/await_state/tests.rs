use super::*;
use crate::engine::{AwaitSource, AwaitStateRollbackImage, NativeAwaitSource, NativeCodeCursor};
use crate::entry::RuntimeSchemaLimits;
use crate::plan::{RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed};
use crate::task::{
    LogicalEpoch, RuntimeNeedOutcome, RuntimeProgramOwner, TaskOutcomeContract,
    TaskPublicationCursor, TaskSequence,
};
use crate::value::{AwbcRuntimeValueSnapshot, RuntimePayload, RuntimeValue};
use arcweft_need::Need;

fn waiting() -> (
    RuntimeProgramOwner,
    AwaitState,
    crate::runtime_id::RuntimePlanTypeId,
) {
    let boolean = crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let need = crate::pattern::RuntimeSemanticTypeId::from_bytes([0xb7; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(need, RuntimePlanTypeProjection::Need(boolean)),
            ],
            [],
        )
        .unwrap();
    let item_type = admitted.type_ids()[0];
    let wrong_type = admitted.type_ids()[1];
    let site = crate::tests::await_function(&mut builder, need).id();
    let plan = std::sync::Arc::new(builder.finish().unwrap());
    let operation =
        NativeCodeCursor::root(&plan, crate::plan::code::RuntimeCodeOwner::Function(site))
            .unwrap()
            .source_operation(&plan)
            .unwrap();
    let owner = RuntimeProgramOwner::Plan(plan);
    let handle = crate::tests::reusable_need_with_outcome(
        "native-wait",
        TaskOutcomeContract::program(boolean),
    );
    let cursor = TaskPublicationCursor {
        logical_epoch: LogicalEpoch(1),
        sequence: TaskSequence(1),
    };
    let queued = std::collections::VecDeque::from([RuntimeNeedPublication::State {
        correlation: handle.correlation(),
        state: Need::Ready(RuntimeNeedOutcome::Value(RuntimePayload(
            RuntimeValue::Bool(true),
        ))),
        cursor,
    }]);
    (
        owner,
        AwaitState {
            source_context: crate::value::RuntimeScopeTypeContext::Function {
                site,
                instantiation: None,
            },
            source: AwaitSource::Native(NativeAwaitSource {
                operation,
                item_type,
            }),
            handle,
            resume: None,
            observed_through: Some(cursor),
            queued,
        },
        wrong_type,
    )
}

#[test]
fn native_waits_share_live_inert_and_parent_node_limits() {
    let (owner, live, _) = waiting();
    let inert = live.inert_rollback_image(&owner).unwrap();
    let limits = RuntimeSchemaLimits {
        // Need + its complete descriptor/request nodes and String argument,
        // followed by the Ready Bool. Descriptor metadata also owns nodes.
        max_nodes: 32,
        ..RuntimeSchemaLimits::engine_default()
    };
    for state in [&live as &dyn SuspendedValues, &inert] {
        let mut exact = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        state.admit_suspended_values(&mut exact, None).unwrap();
        let mut shared = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        shared.validate(&RuntimeValue::Bool(false)).unwrap();
        let error = state.admit_suspended_values(&mut shared, None).unwrap_err();
        assert!(matches!(
            error,
            AwbcRuntimeValueSnapshotError::Resource(
                crate::entry::RuntimeSchemaError::BudgetExceeded { budget: "nodes" }
            )
        ));
        assert_eq!(
            state.admit_suspended_values(&mut shared, None).unwrap_err(),
            error
        );
    }
    let restored = AwaitState::from_image_with_admitted_values(inert.clone(), &owner).unwrap();
    assert_eq!(restored.inert_rollback_image(&owner).unwrap(), inert);
}

#[test]
fn native_waits_reject_wrong_types_correlations_and_cursor_frontiers_before_projection() {
    let (owner, live, wrong_type) = waiting();
    let inert = live.inert_rollback_image(&owner).unwrap();
    for case in 0..6 {
        let mut corrupt: AwaitStateRollbackImage = inert.clone();
        match case {
            0 => {
                let AwaitSource::Native(source) = &mut corrupt.source else {
                    unreachable!()
                };
                source.item_type = wrong_type;
            }
            1 => {
                let RuntimeNeedPublication::State { correlation, .. } =
                    corrupt.queued.front_mut().unwrap()
                else {
                    unreachable!()
                };
                correlation.generation = crate::task::GenerationId::new(2);
            }
            2 => {
                let RuntimeNeedPublication::State { state, .. } =
                    corrupt.queued.front_mut().unwrap()
                else {
                    unreachable!()
                };
                *state = Need::Ready(RuntimeNeedOutcome::Value(AwbcRuntimeValueSnapshot::Unit));
            }
            3 => corrupt
                .queued
                .push_back(corrupt.queued.front().unwrap().clone()),
            4 => corrupt.observed_through.as_mut().unwrap().sequence = TaskSequence(0),
            5 => {
                corrupt.source_context = crate::value::RuntimeScopeTypeContext::Function {
                    site: crate::runtime_id::RuntimeFunctionSiteId::from_accepted_ordinal(
                        std::num::NonZeroU32::MAX,
                    ),
                    instantiation: None,
                };
            }
            _ => unreachable!(),
        }
        let before = corrupt.clone();
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            RuntimeSchemaLimits::engine_default(),
        );
        assert!(
            corrupt
                .admit_suspended_values(&mut admission, None)
                .is_err(),
            "case {case}"
        );
        assert_eq!(corrupt, before);
    }
    assert_eq!(live.inert_rollback_image(&owner).unwrap(), inert);
}
