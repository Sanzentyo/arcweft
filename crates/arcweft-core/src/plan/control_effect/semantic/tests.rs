use super::super::{
    RuntimeControlEffectCancellation, RuntimeControlEffectCardinality,
    RuntimeControlEffectContractDefinition, RuntimeControlEffectKind, RuntimeControlEffectOrdering,
    RuntimeControlEffectRow, RuntimeControlEffectTerminalBehavior, RuntimeTaskControlMode,
};
use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimePlan, RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticEncodingError};
use std::sync::Arc;

fn id(index: usize) -> RuntimeControlEffectContractId {
    RuntimeControlEffectContractId::for_index(index).unwrap()
}
fn empty(children: &[usize]) -> RuntimeControlEffectContract {
    RuntimeControlEffectContract::new(RuntimeControlEffectContractDefinition {
        mode: RuntimeTaskControlMode::StraightLine,
        effects: Box::new([]),
        children: children.iter().copied().map(id).collect(),
    })
}
fn fixture() -> (RuntimePlan, [RuntimeControlEffectContractId; 3]) {
    let mut builder = RuntimePlanBuilder::new();
    let ty = RuntimeSemanticTypeId::from_bytes([1; 32]);
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                ty,
                RuntimePlanTypeProjection::Bool,
            )],
            [],
        )
        .unwrap();
    let child = builder
        .push_control_effect_contract(RuntimeControlEffectContractDefinition {
            mode: RuntimeTaskControlMode::StraightLine,
            effects: Box::new([]),
            children: Box::new([]),
        })
        .unwrap();
    let effect = RuntimeControlEffectRow {
        kind: RuntimeControlEffectKind::HostOperation,
        identity: None,
        inputs: Box::new([ty]),
        output: Some(ty),
        cardinality: RuntimeControlEffectCardinality::ExactlyOnce,
        ordering: RuntimeControlEffectOrdering::CompletionOrder,
        cancellation: RuntimeControlEffectCancellation::PropagatesToChildren,
        terminal: RuntimeControlEffectTerminalBehavior::ResultValue,
    };
    let parent = builder
        .push_control_effect_contract(RuntimeControlEffectContractDefinition {
            mode: RuntimeTaskControlMode::TimeoutRace,
            effects: Box::new([effect.clone()]),
            children: Box::new([child.clone(), child.clone()]),
        })
        .unwrap();
    let unused = builder
        .push_control_effect_contract(RuntimeControlEffectContractDefinition {
            mode: RuntimeTaskControlMode::StraightLine,
            effects: vec![effect; 8].into_boxed_slice(),
            children: Box::new([]),
        })
        .unwrap();
    (
        builder.finish().unwrap(),
        [child.id(), parent.id(), unused.id()],
    )
}

#[test]
fn borrowed_control_pass_counts_only_reachable_rows_and_recomputes_cached_proofs() {
    let (mut plan, ids) = fixture();
    let expected = plan.control_effect_contracts().digest(ids[1]).unwrap();
    // Integrity/cache data is not an input to the recomputing owner.
    Arc::make_mut(&mut plan.inventory.control_effect_contracts.rows)[1].digest =
        ControlEffectContractDigest::from_hasher_output(blake3::hash(b"untrusted cache"));
    let limits = RuntimeTaskPlanSealLimits {
        max_control_effect_rows: 1,
        ..RuntimeTaskPlanSealLimits::default()
    };
    let mut meter = TaskSemanticMeter::new(10_000, 100_000);
    let mut pass = RuntimeControlEffectSemanticPass::from_table(
        plan.control_effect_contracts(),
        plan.type_table(),
        &[ids[1]],
        limits,
        &mut meter,
    )
    .unwrap();
    assert_eq!(pass.admitted.iter().copied().collect::<Vec<_>>(), [0, 1]);
    assert_eq!(pass.complete(ids[1], &mut meter).unwrap(), expected);
    let before = meter.totals();
    assert_eq!(pass.complete(ids[1], &mut meter).unwrap(), expected);
    assert_eq!(meter.totals(), before);
    assert!(!pass.state.contains_key(&ids[2].index()));
    assert!(matches!(
        pass.complete(ids[2], &mut meter),
        Err(RuntimeControlEffectContractError::UnknownChild { index: 2 })
    ));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn control_graph_first_child_failure_poison_and_cycle_are_source_ordered() {
    let types = RuntimePlanBuilder::new().finish().unwrap();
    let missing = |index| {
        RuntimeControlEffectContract::new(RuntimeControlEffectContractDefinition {
            mode: RuntimeTaskControlMode::StraightLine,
            effects: Box::new([RuntimeControlEffectRow {
                kind: RuntimeControlEffectKind::HostOperation,
                identity: None,
                inputs: Box::new([RuntimePlanTypeId::from_accepted_ordinal(
                    std::num::NonZeroU32::new(index).unwrap(),
                )]),
                output: None,
                cardinality: RuntimeControlEffectCardinality::ExactlyOnce,
                ordering: RuntimeControlEffectOrdering::CompletionOrder,
                cancellation: RuntimeControlEffectCancellation::PropagatesToChildren,
                terminal: RuntimeControlEffectTerminalBehavior::ResultValue,
            }]),
            children: Box::new([]),
        })
    };
    let rows = [empty(&[1, 2]), missing(9), missing(10)];
    let mut meter = TaskSemanticMeter::new(10_000, 100_000);
    let mut pass = RuntimeControlEffectSemanticPass::from_rows(
        &rows,
        types.type_table(),
        RuntimeTaskPlanSealLimits::default(),
        &mut meter,
    )
    .unwrap();
    assert!(
        matches!(pass.complete(id(0), &mut meter), Err(RuntimeControlEffectContractError::UnknownType { ty }) if ty.get().get() == 9)
    );
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
    assert!(matches!(
        pass.complete(id(2), &mut meter),
        Err(RuntimeControlEffectContractError::OwnerRejected)
    ));
    let rows = [empty(&[1]), empty(&[0])];
    let mut meter = TaskSemanticMeter::new(10_000, 100_000);
    let mut pass = RuntimeControlEffectSemanticPass::from_rows(
        &rows,
        types.type_table(),
        RuntimeTaskPlanSealLimits::default(),
        &mut meter,
    )
    .unwrap();
    assert!(matches!(
        pass.complete(id(0), &mut meter),
        Err(RuntimeControlEffectContractError::Cycle { index: 0 })
    ));
    assert!(
        pass.state
            .values()
            .all(|state| matches!(state, State::Visiting))
    );
}

#[test]
fn borrowed_control_roots_share_exact_parent_meter_limits() {
    let (plan, ids) = fixture();
    let run = |work, bytes| {
        let mut meter = TaskSemanticMeter::new(work, bytes);
        // Simulate an earlier F/Q writer's work and byte consumption.
        let mut earlier = TaskSemanticEncoder::new(b"earlier-child.v1\0", &mut meter);
        earlier.tag(7);
        let earlier = earlier.finish();
        let result = earlier
            .map_err(RuntimeControlEffectContractError::from)
            .and_then(|_| {
                RuntimeControlEffectSemanticPass::from_table(
                    plan.control_effect_contracts(),
                    plan.type_table(),
                    &[ids[1]],
                    RuntimeTaskPlanSealLimits::default(),
                    &mut meter,
                )
                .and_then(|mut pass| pass.complete(ids[1], &mut meter))
            });
        (result, meter.totals())
    };
    let (expected, (work, bytes)) = run(10_000, 100_000);
    assert_eq!(expected.unwrap(), run(work, bytes).0.unwrap());
    assert!(matches!(
        run(work - 1, bytes).0,
        Err(RuntimeControlEffectContractError::WorkLimit)
    ));
    assert!(matches!(
        run(work, bytes - 1).0,
        Err(RuntimeControlEffectContractError::TranscriptByteLimit)
    ));
}

#[test]
fn deep_control_graph_uses_an_iterative_count_and_completion_stack() {
    let plan = RuntimePlanBuilder::new().finish().unwrap();
    let rows = (0..32_768)
        .map(|index| {
            if index == 32_767 {
                empty(&[])
            } else {
                empty(&[index + 1])
            }
        })
        .collect::<Vec<_>>();
    let mut meter = TaskSemanticMeter::new(1_000_000, 10_000_000);
    let mut pass = RuntimeControlEffectSemanticPass::from_rows(
        &rows,
        plan.type_table(),
        RuntimeTaskPlanSealLimits::default(),
        &mut meter,
    )
    .unwrap();
    pass.complete(id(0), &mut meter).unwrap();
    assert_eq!(pass.state.len(), rows.len());
    assert!(
        pass.state
            .values()
            .all(|state| matches!(state, State::Done(_)))
    );
}

#[test]
fn count_only_control_preflight_preserves_global_order_before_proof_completion() {
    let (plan, ids) = fixture();
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let counted = RuntimeControlEffectPreflight::new(
        plan.control_effect_contracts(),
        plan.type_table(),
        [ids[1]],
        &mut meter,
    )
    .unwrap();
    assert_eq!(meter.totals(), (0, 0));
    counted
        .check_children(RuntimeTaskPlanSealLimits::default(), &mut meter)
        .unwrap();
    counted
        .check_effects(RuntimeTaskPlanSealLimits::default(), &mut meter)
        .unwrap();
    let known = counted.known_transcript_bytes(&mut meter).unwrap();
    assert!(known > 0);
    assert_eq!(meter.totals(), (0, 0));
    let mut pass = counted
        .finish(RuntimeTaskPlanSealLimits::default(), &mut meter)
        .unwrap();
    assert!(pass.state.is_empty());
    assert_eq!(
        pass.complete(ids[1], &mut meter).unwrap(),
        plan.control_effect_contracts().digest(ids[1]).unwrap()
    );
    assert!(!pass.state.contains_key(&ids[2].index()));
    let mut meter = TaskSemanticMeter::new(0, 0);
    let counted = RuntimeControlEffectPreflight::new(
        plan.control_effect_contracts(),
        plan.type_table(),
        [ids[1]],
        &mut meter,
    )
    .unwrap();
    assert!(matches!(
        counted.check_effects(
            RuntimeTaskPlanSealLimits {
                max_control_effect_rows: 0,
                ..RuntimeTaskPlanSealLimits::default()
            },
            &mut meter
        ),
        Err(RuntimeControlEffectContractError::EffectRowsLimit {
            actual: 1,
            maximum: 0
        })
    ));
    assert!(meter.status().is_err());
}
