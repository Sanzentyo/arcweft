use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeControlEffectContractSeed, RuntimeControlEffectContractSeedId, RuntimePlan,
    RuntimePlanBuildError, RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};

fn ty(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn builder(reverse_types: bool) -> RuntimePlanBuilder {
    let mut builder = RuntimePlanBuilder::new();
    let mut types = vec![
        RuntimePlanTypeSeed::new(ty(1), RuntimePlanTypeProjection::Bool),
        RuntimePlanTypeSeed::new(ty(2), RuntimePlanTypeProjection::String),
    ];
    if reverse_types {
        types.reverse();
    }
    builder.admit_type_batch(types, []).unwrap();
    builder
}

fn empty(mode: RuntimeTaskControlMode) -> RuntimeControlEffectContractSeed {
    RuntimeControlEffectContractDefinition {
        mode,
        effects: Box::new([]),
        children: Box::new([]),
    }
}

fn host_row() -> RuntimeControlEffectRow<RuntimeSemanticTypeId> {
    RuntimeControlEffectRow {
        kind: RuntimeControlEffectKind::HostOperation,
        identity: Some(RuntimeControlEffectIdentity::from_checked_digest([9; 32])),
        inputs: Box::new([ty(1), ty(2)]),
        output: Some(ty(1)),
        cardinality: RuntimeControlEffectCardinality::ExactlyOnce,
        ordering: RuntimeControlEffectOrdering::CompletionOrder,
        cancellation: RuntimeControlEffectCancellation::PropagatesToChildren,
        terminal: RuntimeControlEffectTerminalBehavior::ResultValue,
    }
}

fn seed(child: RuntimeControlEffectContractSeedId) -> RuntimeControlEffectContractSeed {
    RuntimeControlEffectContractDefinition {
        mode: RuntimeTaskControlMode::TimeoutRace,
        effects: Box::new([host_row()]),
        children: Box::new([child]),
    }
}

fn digest(
    plan: &RuntimePlan,
    handle: &RuntimeControlEffectContractSeedId,
) -> ControlEffectContractDigest {
    plan.control_effect_contracts().digest(handle.id()).unwrap()
}

#[test]
fn exact_control_effect_transcript_commits_abi_lifecycle_and_child_order() {
    let mut builder = builder(false);
    let child = builder
        .push_control_effect_contract(empty(RuntimeTaskControlMode::StraightLine))
        .unwrap();
    let mut definition = seed(child.clone());
    definition.children = Box::new([child.clone(), child.clone()]);
    let parent = builder.push_control_effect_contract(definition).unwrap();
    let plan = builder.finish().unwrap();
    let mut child_bytes = b"arcweft.task.control-effect-contract.v1\0".to_vec();
    child_bytes.push(0);
    child_bytes.extend_from_slice(&0_u32.to_le_bytes());
    child_bytes.extend_from_slice(&0_u32.to_le_bytes());
    let child_digest = blake3::hash(&child_bytes);
    assert_eq!(digest(&plan, &child).as_bytes(), child_digest.as_bytes());
    let mut expected = b"arcweft.task.control-effect-contract.v1\0".to_vec();
    expected.push(4); // TimeoutRace
    expected.extend_from_slice(&1_u32.to_le_bytes());
    expected.extend_from_slice(&0_u32.to_le_bytes()); // effect ordinal
    expected.extend_from_slice(&[0, 1]); // HostOperation, Some identity
    expected.extend_from_slice(&[9; 32]);
    expected.extend_from_slice(&2_u32.to_le_bytes());
    expected.extend_from_slice(&[1; 32]);
    expected.extend_from_slice(&[2; 32]);
    expected.push(1); // Some output
    expected.extend_from_slice(&[1; 32]);
    expected.extend_from_slice(&[0, 1, 2, 1]); // cardinality/order/cancel/terminal
    expected.extend_from_slice(&2_u32.to_le_bytes());
    for ordinal in [0_u32, 1] {
        expected.extend_from_slice(&ordinal.to_le_bytes());
        expected.extend_from_slice(child_digest.as_bytes());
    }
    assert_eq!(
        digest(&plan, &parent).as_bytes(),
        blake3::hash(&expected).as_bytes()
    );
}

#[test]
fn scoped_contract_types_retain_the_accepted_binder_semantics() {
    use crate::effect_row::{EffectFormula, EffectPredicate};
    use crate::plan::{RuntimeFunctionTypeContract, RuntimeTypeBinder, RuntimeTypeScope};
    let binder = RuntimeTypeBinder::new(1, 0, 0);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    ty(3),
                    RuntimePlanTypeProjection::BoundType(scope.bound_type(0, 0).unwrap()),
                )
                .with_scope(scope),
                RuntimePlanTypeSeed::new(
                    ty(4),
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            binder,
                            EffectPredicate::unconstrained(),
                            EffectFormula::empty(),
                        ),
                        parameters: Box::new([ty(3)]),
                        result: ty(3),
                    },
                ),
            ],
            [],
        )
        .unwrap();
    let mut definition = empty(RuntimeTaskControlMode::StraightLine);
    let mut row = host_row();
    row.inputs = Box::new([ty(3)]);
    row.output = Some(ty(3));
    definition.effects = Box::new([row]);
    let contract = builder.push_control_effect_contract(definition).unwrap();
    let plan = builder.finish().unwrap();
    let admitted = plan.control_effect_contracts().get(contract.id()).unwrap();
    let input = plan
        .type_table()
        .get(admitted.effects()[0].inputs[0])
        .unwrap();
    assert!(!input.scope().is_root());
    assert_eq!(input.semantic_identity(), ty(3));
    assert!(
        plan.control_effect_contracts()
            .digest(contract.id())
            .is_some()
    );
}

#[test]
fn digest_is_independent_of_type_and_child_table_allocation() {
    let make = |reverse_types| {
        let mut builder = builder(reverse_types);
        if reverse_types {
            builder
                .push_control_effect_contract(empty(RuntimeTaskControlMode::LineTimeline))
                .unwrap();
        }
        let child = builder
            .push_control_effect_contract(empty(RuntimeTaskControlMode::StraightLine))
            .unwrap();
        let parent = builder.push_control_effect_contract(seed(child)).unwrap();
        let plan = builder.finish().unwrap();
        (digest(&plan, &parent), parent.id())
    };
    let (first, first_id) = make(false);
    let (second, second_id) = make(true);
    assert_ne!(first_id, second_id);
    assert_eq!(first, second);
}

#[test]
fn every_included_control_effect_role_changes_the_digest() {
    let mut builder = builder(false);
    let child = builder
        .push_control_effect_contract(empty(RuntimeTaskControlMode::StraightLine))
        .unwrap();
    let base = seed(child);
    let original = builder.push_control_effect_contract(base.clone()).unwrap();
    let mut variants = Vec::new();
    let mut changed = base.clone();
    changed.mode = RuntimeTaskControlMode::MaySuspend;
    variants.push(changed);
    let mut changed = base.clone();
    changed.effects[0].kind = RuntimeControlEffectKind::RuntimeOperation;
    variants.push(changed);
    let mut changed = base.clone();
    changed.effects[0].identity = None;
    variants.push(changed);
    let mut changed = base.clone();
    changed.effects[0].identity = Some(RuntimeControlEffectIdentity::from_checked_digest([8; 32]));
    variants.push(changed);
    let mut changed = base.clone();
    changed.effects[0].inputs.reverse();
    variants.push(changed);
    let mut changed = base.clone();
    changed.effects[0].output = None;
    variants.push(changed);
    let mut changed = base.clone();
    changed.effects[0].output = Some(ty(2));
    variants.push(changed);
    let mut changed = base.clone();
    changed.effects[0].cardinality = RuntimeControlEffectCardinality::ZeroOrMore;
    variants.push(changed);
    let mut changed = base.clone();
    changed.effects[0].ordering = RuntimeControlEffectOrdering::SourceOrder;
    variants.push(changed);
    let mut changed = base.clone();
    changed.effects[0].cancellation = RuntimeControlEffectCancellation::ObservedNoPayload;
    variants.push(changed);
    let mut changed = base.clone();
    changed.effects[0].terminal =
        RuntimeControlEffectTerminalBehavior::InfrastructureFailureControl;
    variants.push(changed);
    let mut changed = base;
    changed.children = Box::new([]);
    variants.push(changed);
    let variants = variants
        .into_iter()
        .map(|seed| builder.push_control_effect_contract(seed).unwrap())
        .collect::<Vec<_>>();
    let plan = builder.finish().unwrap();
    for variant in variants {
        assert_ne!(digest(&plan, &original), digest(&plan, &variant));
    }
}

#[test]
fn effect_and_child_source_order_are_preserved() {
    let mut builder = builder(false);
    let first = builder
        .push_control_effect_contract(empty(RuntimeTaskControlMode::StraightLine))
        .unwrap();
    let second = builder
        .push_control_effect_contract(empty(RuntimeTaskControlMode::MustSuspend))
        .unwrap();
    let mut base = seed(first.clone());
    let mut second_row = host_row();
    second_row.kind = RuntimeControlEffectKind::TimeoutClock;
    base.effects = Box::new([host_row(), second_row]);
    base.children = Box::new([first, second]);
    let original = builder.push_control_effect_contract(base.clone()).unwrap();
    let mut reordered = base.clone();
    reordered.effects.reverse();
    let effects = builder.push_control_effect_contract(reordered).unwrap();
    let mut reordered = base;
    reordered.children.reverse();
    let children = builder.push_control_effect_contract(reordered).unwrap();
    let plan = builder.finish().unwrap();
    assert_ne!(digest(&plan, &original), digest(&plan, &effects));
    assert_ne!(digest(&plan, &original), digest(&plan, &children));
}

#[test]
fn rejected_foreign_or_unknown_inputs_do_not_publish_rows() {
    let mut first = builder(false);
    let foreign = first.reserve_control_effect_contract().unwrap();
    let mut second = builder(false);
    assert!(matches!(
        second.push_control_effect_contract(seed(foreign)),
        Err(RuntimePlanBuildError::ControlEffectContract(
            RuntimeControlEffectContractError::ForeignSeed
        ))
    ));
    let mut invalid = empty(RuntimeTaskControlMode::StraightLine);
    let mut row = host_row();
    row.inputs = Box::new([ty(99)]);
    invalid.effects = Box::new([row]);
    assert!(matches!(
        second.push_control_effect_contract(invalid),
        Err(RuntimePlanBuildError::UnknownSemanticType { .. })
    ));
    let accepted = second
        .push_control_effect_contract(empty(RuntimeTaskControlMode::StraightLine))
        .unwrap();
    assert_eq!(accepted.id().index(), 0);
    assert_eq!(second.finish().unwrap().control_effect_contracts().len(), 1);
}

#[test]
fn forward_child_and_failed_definition_retain_the_reserved_owner() {
    let mut builder = builder(false);
    let parent = builder.reserve_control_effect_contract().unwrap();
    let child = builder.reserve_control_effect_contract().unwrap();
    let mut invalid = seed(child.clone());
    invalid.effects[0].output = Some(ty(99));
    assert!(matches!(
        builder.define_control_effect_contract(&parent, invalid),
        Err(RuntimePlanBuildError::UnknownSemanticType { .. })
    ));
    builder
        .define_control_effect_contract(&parent, seed(child.clone()))
        .unwrap();
    builder
        .define_control_effect_contract(&child, empty(RuntimeTaskControlMode::StraightLine))
        .unwrap();
    assert!(matches!(
        builder
            .define_control_effect_contract(&parent, empty(RuntimeTaskControlMode::LineTimeline)),
        Err(RuntimePlanBuildError::ControlEffectContract(
            RuntimeControlEffectContractError::AlreadyDefined { index: 0 }
        ))
    ));
    let plan = builder.finish().unwrap();
    assert_eq!(parent.id().index(), 0);
    assert_eq!(
        plan.control_effect_contracts()
            .get(parent.id())
            .unwrap()
            .children(),
        &[child.id()]
    );
    assert!(
        plan.control_effect_contracts()
            .digest(parent.id())
            .is_some()
    );
}

#[test]
fn incomplete_and_cyclic_contracts_cannot_publish_a_plan() {
    let mut incomplete = builder(false);
    incomplete.reserve_control_effect_contract().unwrap();
    assert!(matches!(
        incomplete.finish(),
        Err(RuntimePlanBuildError::ControlEffectContract(
            RuntimeControlEffectContractError::Incomplete { index: 0 }
        ))
    ));
    let mut cyclic = builder(false);
    let first = cyclic.reserve_control_effect_contract().unwrap();
    let second = cyclic.reserve_control_effect_contract().unwrap();
    cyclic
        .define_control_effect_contract(&first, seed(second.clone()))
        .unwrap();
    cyclic
        .define_control_effect_contract(&second, seed(first))
        .unwrap();
    assert!(matches!(
        cyclic.finish(),
        Err(RuntimePlanBuildError::ControlEffectContract(
            RuntimeControlEffectContractError::Cycle { index: 0 }
        ))
    ));
}

#[test]
fn deep_contract_graph_seals_without_recursive_rust_calls() {
    let mut builder = builder(false);
    let handles = (0..10_000)
        .map(|_| builder.reserve_control_effect_contract().unwrap())
        .collect::<Vec<_>>();
    for (index, handle) in handles.iter().enumerate() {
        let mut definition = empty(RuntimeTaskControlMode::StraightLine);
        if let Some(child) = handles.get(index + 1) {
            definition.children = Box::new([child.clone()]);
        }
        builder
            .define_control_effect_contract(handle, definition)
            .unwrap();
    }
    let plan = builder.finish().unwrap();
    assert_eq!(plan.control_effect_contracts().len(), 10_000);
    assert!(
        plan.control_effect_contracts()
            .digest(handles[0].id())
            .is_some()
    );
}

#[test]
fn semantic_work_budget_rejects_before_publishing_a_digest() {
    let types = builder(false).finish().unwrap();
    let leaf = RuntimeControlEffectContract::new(RuntimeControlEffectContractDefinition {
        mode: RuntimeTaskControlMode::StraightLine,
        effects: Box::new([]),
        children: Box::new([]),
    });
    assert_eq!(
        RuntimeControlEffectContractTable::seal(vec![leaf.clone()], types.type_table(), 2),
        Err(RuntimeControlEffectContractError::WorkLimit)
    );
    let child = RuntimeControlEffectContractId::for_index(1).unwrap();
    let parent = RuntimeControlEffectContract::new(RuntimeControlEffectContractDefinition {
        mode: RuntimeTaskControlMode::RuntimeAggregate,
        effects: Box::new([]),
        children: vec![child; 1000].into_boxed_slice(),
    });
    assert_eq!(
        RuntimeControlEffectContractTable::seal(vec![parent, leaf], types.type_table(), 20),
        Err(RuntimeControlEffectContractError::WorkLimit)
    );
}
