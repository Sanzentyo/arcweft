use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeControlEffectContractError, RuntimePlan, RuntimePlanBuilder, RuntimePlanTypeProjection,
    RuntimePlanTypeSeed, RuntimeTaskPlanSealLimits,
};

fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(u64::MAX, u64::MAX)
}
fn limits(type_count: usize) -> RuntimeControlSourceLimits {
    RuntimeControlSourceLimits {
        rows: 128,
        elements: 4096,
        encoded_bytes: 65536,
        decoded_bytes: 65536,
        type_count,
    }
}
fn ty(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}
fn builder() -> RuntimePlanBuilder {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(ty(1), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(ty(2), RuntimePlanTypeProjection::String),
            ],
            [],
        )
        .unwrap();
    builder
}
fn empty(
    mode: RuntimeTaskControlMode,
) -> RuntimeControlEffectContractDefinition<
    RuntimeSemanticTypeId,
    crate::plan::RuntimeControlEffectContractSeedId,
> {
    RuntimeControlEffectContractDefinition {
        mode,
        effects: Box::new([]),
        children: Box::new([]),
    }
}
fn effects() -> Box<[RuntimeControlEffectRow<RuntimeSemanticTypeId>]> {
    let kinds = [
        RuntimeControlEffectKind::HostOperation,
        RuntimeControlEffectKind::RuntimeOperation,
        RuntimeControlEffectKind::ViewSubscription,
        RuntimeControlEffectKind::AwaitObservation,
        RuntimeControlEffectKind::TimeoutClock,
        RuntimeControlEffectKind::LineEmission,
        RuntimeControlEffectKind::CancellationObservation,
    ];
    let cardinalities = [
        RuntimeControlEffectCardinality::ExactlyOnce,
        RuntimeControlEffectCardinality::ZeroOrOne,
        RuntimeControlEffectCardinality::ZeroOrMore,
        RuntimeControlEffectCardinality::OneOrMore,
    ];
    let orderings = [
        RuntimeControlEffectOrdering::SourceOrder,
        RuntimeControlEffectOrdering::CompletionOrder,
        RuntimeControlEffectOrdering::SingleTerminal,
    ];
    let cancellations = [
        RuntimeControlEffectCancellation::NotObserved,
        RuntimeControlEffectCancellation::ObservedNoPayload,
        RuntimeControlEffectCancellation::PropagatesToChildren,
    ];
    let terminals = [
        RuntimeControlEffectTerminalBehavior::Value,
        RuntimeControlEffectTerminalBehavior::ResultValue,
        RuntimeControlEffectTerminalBehavior::OptionValue,
        RuntimeControlEffectTerminalBehavior::NonreturningCancellation,
        RuntimeControlEffectTerminalBehavior::InfrastructureFailureControl,
    ];
    kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| RuntimeControlEffectRow {
            kind,
            identity: (index != 0).then_some(RuntimeControlEffectIdentity::from_checked_digest(
                [u8::try_from(index).unwrap(); 32],
            )),
            inputs: if index == 0 {
                Box::new([])
            } else {
                Box::new([ty(1), ty(2), ty(1)])
            },
            output: (index != 0).then_some(ty(2)),
            cardinality: cardinalities[index % cardinalities.len()],
            ordering: orderings[index % orderings.len()],
            cancellation: cancellations[index % cancellations.len()],
            terminal: terminals[index % terminals.len()],
        })
        .collect()
}
fn plan() -> RuntimePlan {
    let mut builder = builder();
    let parent = builder.reserve_control_effect_contract().unwrap();
    let leaf = builder
        .push_control_effect_contract(empty(RuntimeTaskControlMode::StraightLine))
        .unwrap();
    builder
        .define_control_effect_contract(
            &parent,
            RuntimeControlEffectContractDefinition {
                mode: RuntimeTaskControlMode::TimeoutRace,
                effects: effects(),
                children: Box::new([leaf.clone(), leaf.clone()]),
            },
        )
        .unwrap();
    for mode in [
        RuntimeTaskControlMode::StraightLine,
        RuntimeTaskControlMode::MaySuspend,
        RuntimeTaskControlMode::MustSuspend,
        RuntimeTaskControlMode::RuntimeAggregate,
        RuntimeTaskControlMode::TimeoutRace,
        RuntimeTaskControlMode::LineTimeline,
    ] {
        builder
            .push_control_effect_contract(RuntimeControlEffectContractDefinition {
                mode,
                effects: effects(),
                children: Box::new([parent.clone(), leaf.clone(), parent.clone()]),
            })
            .unwrap();
    }
    builder.finish().unwrap()
}
fn complete(
    table: RuntimeControlEffectContractTable,
    types: &RuntimePlanTypeTable,
) -> Result<RuntimeControlEffectContractTable, RuntimeControlEffectContractError> {
    let proofs = {
        let mut bank = meter();
        let mut pass = super::super::semantic::RuntimeControlEffectSemanticPass::from_rows(
            &table.rows,
            types,
            RuntimeTaskPlanSealLimits::default(),
            &mut bank,
        )?;
        (0..table.len())
            .map(|index| {
                pass.complete(
                    RuntimeControlEffectContractId::for_index(index).unwrap(),
                    &mut bank,
                )
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    table.complete(proofs.into_boxed_slice())
}

#[test]
fn all_original_control_roles_round_trip_without_issuing_any_wire_proof() {
    let plan = plan();
    let original = plan.control_effect_contracts();
    let bounds = limits(plan.type_table().len());
    let mut bank = meter();
    let bytes = original.encode_sources(bounds, &mut bank).unwrap();
    let source =
        RuntimeControlEffectContractTable::preflight_sources(&bytes, bounds, &mut bank).unwrap();
    assert_eq!(source.row_count(), 8);
    let decoded = source.materialize(plan.type_table(), &mut bank).unwrap();
    assert_eq!(decoded.rows, original.rows);
    assert!(decoded.proofs.is_none());
    assert_eq!(decoded.iter().count(), 0);
    for index in 0..decoded.len() {
        assert!(
            decoded
                .digest(RuntimeControlEffectContractId::for_index(index).unwrap())
                .is_none()
        );
    }
    assert_eq!(decoded.encode_sources(bounds, &mut bank).unwrap(), bytes);
    let sealed = complete(decoded, plan.type_table()).unwrap();
    assert_eq!(&sealed, original);
    // Cached proofs are deliberately absent from the source grammar.
    let source_only = RuntimeControlEffectContractTable::from_definitions(original.rows.to_vec());
    assert_eq!(
        source_only.encode_sources(bounds, &mut bank).unwrap(),
        bytes
    );
}

fn golden() -> (RuntimePlan, Vec<u8>) {
    let mut builder = builder();
    builder
        .push_control_effect_contract(RuntimeControlEffectContractDefinition {
            mode: RuntimeTaskControlMode::TimeoutRace,
            effects: Box::new([RuntimeControlEffectRow {
                kind: RuntimeControlEffectKind::HostOperation,
                identity: Some(RuntimeControlEffectIdentity::from_checked_digest([9; 32])),
                inputs: Box::new([ty(1), ty(2)]),
                output: Some(ty(1)),
                cardinality: RuntimeControlEffectCardinality::ExactlyOnce,
                ordering: RuntimeControlEffectOrdering::CompletionOrder,
                cancellation: RuntimeControlEffectCancellation::PropagatesToChildren,
                terminal: RuntimeControlEffectTerminalBehavior::ResultValue,
            }]),
            children: Box::new([]),
        })
        .unwrap();
    let plan = builder.finish().unwrap();
    let mut bytes = b"arcweft.runtime.control-effect-sources.v1\0".to_vec();
    bytes.push(1);
    bytes.extend_from_slice(&1_u32.to_le_bytes()); // table count
    bytes.extend_from_slice(&0_u32.to_le_bytes()); // original row ordinal
    bytes.push(4); // TimeoutRace
    bytes.extend_from_slice(&1_u32.to_le_bytes()); // effect count
    bytes.extend_from_slice(&0_u32.to_le_bytes()); // effect ordinal
    bytes.extend_from_slice(&[0, 1]); // host, identity present
    bytes.extend_from_slice(&[9; 32]);
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.push(1); // output present
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&[0, 1, 2, 1]); // cardinality, ordering, cancellation, terminal
    bytes.extend_from_slice(&0_u32.to_le_bytes()); // no child contracts
    (plan, bytes)
}

#[test]
fn canonical_version_one_source_has_exact_original_abi_and_lifecycle_fields() {
    let (plan, expected) = golden();
    let bounds = limits(plan.type_table().len());
    assert_eq!(
        plan.control_effect_contracts()
            .encode_sources(bounds, &mut meter())
            .unwrap(),
        expected
    );
    let mut bank = meter();
    let decoded =
        RuntimeControlEffectContractTable::preflight_sources(&expected, bounds, &mut bank)
            .unwrap()
            .materialize(plan.type_table(), &mut bank)
            .unwrap();
    assert_eq!(
        complete(decoded, plan.type_table()).unwrap(),
        *plan.control_effect_contracts()
    );
}

#[test]
fn whole_source_rejects_every_truncation_unknown_tag_ordinal_and_type() {
    let (plan, bytes) = golden();
    let bounds = limits(plan.type_table().len());
    for cut in 0..bytes.len() {
        assert!(
            RuntimeControlEffectContractTable::preflight_sources(
                &bytes[..cut],
                bounds,
                &mut meter()
            )
            .is_err(),
            "cut {cut}"
        );
    }
    let row = DOMAIN.len() + 5;
    let effect = row + 9;
    // Every closed tag/option, source row and effect ordinal.
    for offset in [
        0,
        DOMAIN.len(),
        row,
        row + 4,
        effect,
        effect + 4,
        effect + 5,
        effect + 50,
        effect + 55,
        effect + 56,
        effect + 57,
        effect + 58,
    ] {
        let mut corrupt = bytes.clone();
        corrupt[offset] = 255;
        let mut bank = meter();
        assert!(
            RuntimeControlEffectContractTable::preflight_sources(&corrupt, bounds, &mut bank)
                .is_err(),
            "offset {offset}"
        );
        assert_eq!(bank.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
    for offset in [effect + 42, effect + 46, effect + 51] {
        for id in [0_u32, 3, u32::MAX] {
            let mut corrupt = bytes.clone();
            corrupt[offset..offset + 4].copy_from_slice(&id.to_le_bytes());
            assert!(
                matches!(RuntimeControlEffectContractTable::preflight_sources(&corrupt, bounds, &mut meter()), Err(RuntimeControlSourceError::Reference { kind: "type", actual, .. }) if actual == id)
            );
        }
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(matches!(
        RuntimeControlEffectContractTable::preflight_sources(&trailing, bounds, &mut meter()),
        Err(RuntimeControlSourceError::Trailing)
    ));
}

#[test]
fn original_child_order_forward_edges_and_duplicate_edges_are_preserved() {
    let plan = plan();
    let bounds = limits(plan.type_table().len());
    let bytes = plan
        .control_effect_contracts()
        .encode_sources(bounds, &mut meter())
        .unwrap();
    let mut bank = meter();
    let decoded = RuntimeControlEffectContractTable::preflight_sources(&bytes, bounds, &mut bank)
        .unwrap()
        .materialize(plan.type_table(), &mut bank)
        .unwrap();
    assert_eq!(
        decoded.rows[0]
            .children()
            .iter()
            .map(|child| child.index())
            .collect::<Vec<_>>(),
        [1, 1]
    );
    assert_eq!(
        decoded.rows[2]
            .children()
            .iter()
            .map(|child| child.index())
            .collect::<Vec<_>>(),
        [0, 1, 0]
    );
    let mut reversed = bytes.clone();
    let tail = reversed.len();
    // The final row has three source-ordered references, each ordinal + ID.
    reversed[tail - 20..tail - 16].copy_from_slice(&2_u32.to_le_bytes());
    reversed[tail - 12..tail - 8].copy_from_slice(&1_u32.to_le_bytes());
    let changed =
        RuntimeControlEffectContractTable::preflight_sources(&reversed, bounds, &mut bank)
            .unwrap()
            .materialize(plan.type_table(), &mut bank)
            .unwrap();
    let changed = complete(changed, plan.type_table()).unwrap();
    let last = RuntimeControlEffectContractId::for_index(7).unwrap();
    assert_ne!(
        changed.digest(last),
        plan.control_effect_contracts().digest(last)
    );
    for offset in [tail - 24, tail - 16, tail - 8] {
        let mut corrupt = bytes.clone();
        corrupt[offset] = 255;
        assert!(matches!(
            RuntimeControlEffectContractTable::preflight_sources(&corrupt, bounds, &mut meter()),
            Err(RuntimeControlSourceError::Ordinal { .. })
        ));
    }
    for id in [0_u32, 9, u32::MAX] {
        let mut corrupt = bytes.clone();
        corrupt[tail - 4..].copy_from_slice(&id.to_le_bytes());
        assert!(
            matches!(RuntimeControlEffectContractTable::preflight_sources(&corrupt, bounds, &mut meter()), Err(RuntimeControlSourceError::Reference { kind: "child", actual, .. }) if actual == id)
        );
    }
}

#[test]
fn a_valid_source_cycle_stays_unsealed_and_the_sole_graph_authority_rejects_it() {
    let plan = builder().finish().unwrap();
    let table = RuntimeControlEffectContractTable::from_definitions(vec![
        RuntimeControlEffectContract::new(RuntimeControlEffectContractDefinition {
            mode: RuntimeTaskControlMode::RuntimeAggregate,
            effects: Box::new([]),
            children: Box::new([RuntimeControlEffectContractId::for_index(0).unwrap()]),
        }),
    ]);
    let bounds = limits(plan.type_table().len());
    let mut bank = meter();
    let bytes = table.encode_sources(bounds, &mut bank).unwrap();
    let decoded = RuntimeControlEffectContractTable::preflight_sources(&bytes, bounds, &mut bank)
        .unwrap()
        .materialize(plan.type_table(), &mut bank)
        .unwrap();
    assert!(decoded.proofs.is_none());
    assert_eq!(
        complete(decoded, plan.type_table()),
        Err(RuntimeControlEffectContractError::Cycle { index: 0 })
    );
}

#[test]
fn exact_storage_elements_and_original_work_are_preflighted_before_retention() {
    let plan = plan();
    let table = plan.control_effect_contracts();
    let bounds = limits(plan.type_table().len());
    let mut bank = meter();
    let prepared = table.prepare_sources_encoding(bounds, &mut bank).unwrap();
    let size = prepared.encoded_bytes();
    let storage = prepared.storage_bytes();
    let elements = prepared.elements();
    let exact = RuntimeControlSourceLimits {
        rows: 8,
        elements,
        encoded_bytes: size,
        decoded_bytes: storage,
        ..bounds
    };
    let bytes = table.encode_sources(exact, &mut bank).unwrap();
    let mut bank = TaskSemanticMeter::new(3 * size as u64, u64::MAX);
    let source =
        RuntimeControlEffectContractTable::preflight_sources(&bytes, exact, &mut bank).unwrap();
    assert_eq!(source.storage_bytes(), storage);
    assert_eq!(source.elements(), elements);
    assert_eq!(
        source
            .materialize(plan.type_table(), &mut bank)
            .unwrap()
            .rows,
        table.rows
    );
    assert_eq!(
        bank.charge_work(1),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    for reduced in [
        RuntimeControlSourceLimits { rows: 7, ..exact },
        RuntimeControlSourceLimits {
            elements: elements - 1,
            ..exact
        },
        RuntimeControlSourceLimits {
            encoded_bytes: size - 1,
            ..exact
        },
        RuntimeControlSourceLimits {
            decoded_bytes: storage - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            RuntimeControlEffectContractTable::preflight_sources(&bytes, reduced, &mut meter()),
            Err(RuntimeControlSourceError::Limit { .. })
        ));
        assert!(matches!(
            table.encode_sources(reduced, &mut meter()),
            Err(RuntimeControlSourceError::Limit { .. })
        ));
    }
    let mut bank = TaskSemanticMeter::new(3 * size as u64 - 1, u64::MAX);
    assert!(matches!(
        RuntimeControlEffectContractTable::preflight_sources(&bytes, exact, &mut bank),
        Err(RuntimeControlSourceError::Resource(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    let mut bank = meter();
    let earlier =
        RuntimeControlEffectContractTable::preflight_sources(&bytes, exact, &mut bank).unwrap();
    let mut corrupt = bytes.clone();
    corrupt[0] ^= 1;
    assert!(
        RuntimeControlEffectContractTable::preflight_sources(&corrupt, exact, &mut bank).is_err()
    );
    assert_eq!(
        earlier.materialize(plan.type_table(), &mut bank),
        Err(RuntimeControlSourceError::Resource(
            TaskSemanticEncodingError::OwnerRejected
        ))
    );
    let mut bank = meter();
    let wrong_count = RuntimeControlSourceLimits {
        type_count: 3,
        ..bounds
    };
    let source =
        RuntimeControlEffectContractTable::preflight_sources(&bytes, wrong_count, &mut bank)
            .unwrap();
    assert_eq!(
        source.materialize(plan.type_table(), &mut bank),
        Err(RuntimeControlSourceError::TypeTableCount {
            expected: 3,
            actual: 2
        })
    );
    assert_eq!(bank.status(), Err(TaskSemanticEncodingError::OwnerRejected));
}

#[test]
fn parent_append_is_reserved_atomic_and_empty_definitions_issue_no_proof() {
    let (plan, bytes) = golden();
    let table = plan.control_effect_contracts();
    let bounds = limits(plan.type_table().len());
    let prepared = table
        .prepare_sources_encoding(bounds, &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(bytes.len() + 3);
    output.extend_from_slice(&[7, 8, 9]);
    let address = output.as_ptr();
    prepared.append_to(&mut output).unwrap();
    assert_eq!(output.as_ptr(), address);
    assert_eq!(&output[..3], &[7, 8, 9]);
    assert_eq!(&output[3..], bytes);
    let mut short = Vec::from([1, 2, 3]);
    assert!(matches!(
        table
            .prepare_sources_encoding(bounds, &mut meter())
            .unwrap()
            .append_to(&mut short),
        Err(RuntimeControlSourceError::OutputCapacity { .. })
    ));
    assert_eq!(short, [1, 2, 3]);
    let empty = RuntimeControlEffectContractTable::from_definitions(Vec::new());
    let bounds = RuntimeControlSourceLimits {
        rows: 0,
        elements: 0,
        encoded_bytes: DOMAIN.len() + 5,
        decoded_bytes: 0,
        type_count: 0,
    };
    let bytes = empty.encode_sources(bounds, &mut meter()).unwrap();
    let mut bank = meter();
    let types = RuntimePlanBuilder::new().finish().unwrap();
    let decoded = RuntimeControlEffectContractTable::preflight_sources(&bytes, bounds, &mut bank)
        .unwrap()
        .materialize(types.type_table(), &mut bank)
        .unwrap();
    assert!(decoded.is_empty());
    assert!(decoded.proofs.is_none());
}
