use super::super::content_plans::{PreparedRuntimeContentPlanSources, tests as content};
use super::super::expression::RuntimeExpressionSourceHandles;
use super::super::pattern::tests::{meter, resources};
use super::*;
use crate::line_task::{
    AudioCleanup, ChildCancelPolicy, ChildJoinPolicy, ChildTaskCleanup, LineCleanupPolicy,
    ParallelPolicy, PresentationCleanup,
};
use crate::plan::*;

pub(in crate::plan::construction::source) mod handles;

fn limits(plan: &RuntimePlan) -> RuntimeLineGroupSourceLimits {
    RuntimeLineGroupSourceLimits {
        groups: 100,
        nodes: 100_000,
        depth: 128,
        elements: 1_000_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        opaque_producer_bytes: plan
            .type_table()
            .declarations_with_ids()
            .filter_map(|(_, declaration)| match declaration.projection() {
                RuntimePlanTypeProjection::Opaque { producer, .. } => Some(producer.as_str().len()),
                _ => None,
            })
            .max()
            .unwrap_or(0),
        flow: super::super::flow::tests::limits(plan),
    }
}
pub(in crate::plan::construction::source) fn fixture(
    policy: LineCleanupPolicy,
    cancel: ChildCancelPolicy,
) -> RuntimePlan {
    scoped_fixture(policy, cancel, 0)
}
fn scoped_fixture(
    policy: LineCleanupPolicy,
    cancel: ChildCancelPolicy,
    depth: usize,
) -> RuntimePlan {
    let (mut builder, content) = crate::plan::body_semantic::content_root_builder_fixture();
    let mut activation_ops = vec![RuntimeFlowOpSeed::Noop];
    let mut action = vec![RuntimeFlowOpSeed::Noop];
    if depth != 0 {
        let boolean = crate::pattern::RuntimeSemanticTypeId::from_bytes([2; 32]);
        let admitted = builder
            .admit_type_batch(
                [],
                [RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Binding {
                        identity: [0xf2; 32],
                        declaration: RuntimeLocalBindingDeclaration::new(
                            RuntimeLocalBindingKind::PatternBinding,
                            false,
                            RuntimeLocalBindingStorage::Derived,
                        ),
                    },
                    boolean,
                )],
            )
            .unwrap();
        let local = admitted.local_ids()[0].clone();
        let read = || RuntimeFlowOpSeed::Let {
            pattern: RuntimePatternSeed::new(boolean, RuntimePatternSeedKind::Discard),
            expr: RuntimeExprSeed::new(
                boolean,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    local.clone(),
                    crate::value::RuntimeLocalReadMode::Copy,
                )),
            ),
        };
        activation_ops = vec![RuntimeFlowOpSeed::Let {
            pattern: RuntimePatternSeed::new(
                boolean,
                RuntimePatternSeedKind::Bind {
                    local: local.clone(),
                    mutable: false,
                },
            ),
            expr: RuntimeExprSeed::new(
                boolean,
                RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
            ),
        }];
        activation_ops.extend((0..depth).map(|_| RuntimeFlowOpSeed::EnterScope {
            identity: crate::scope::RuntimeScopeIdentity::Anonymous,
        }));
        let mut nested = vec![read()];
        for _ in 0..depth {
            nested = vec![RuntimeFlowOpSeed::Match {
                scrutinee: RuntimeExprSeed::new(
                    boolean,
                    RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
                ),
                arms: vec![RuntimeFlowMatchArmSeed {
                    pattern: RuntimePatternSeed::new(boolean, RuntimePatternSeedKind::Discard),
                    guard: None,
                    ops: nested,
                }],
            }];
        }
        activation_ops.extend(nested);
        activation_ops.extend((0..depth).map(|_| RuntimeFlowOpSeed::ExitScope));
        action.push(read());
    }
    let root = RuntimeLineTaskNodeSeed::Sequence(vec![RuntimeLineTaskNodeSeed::Start(vec![
        RuntimeLineTaskNodeSeed::Action(action),
        RuntimeLineTaskNodeSeed::Parallel {
            policy: ParallelPolicy::JoinAll,
            children: vec![
                RuntimeLineTaskNodeSeed::Action(vec![RuntimeFlowOpSeed::Noop]),
                RuntimeLineTaskNodeSeed::Child {
                    node: RuntimeLineTaskNodeSeedId::from_zero_based(5).unwrap(),
                    trigger: RuntimeLineTaskTriggerSeed::Mark(content.mark(0).unwrap()),
                    join_policy: ChildJoinPolicy::Join,
                    cancel_policy: cancel,
                    scope: Box::new(RuntimeLineTaskNodeSeed::Action(vec![
                        RuntimeFlowOpSeed::Noop,
                    ])),
                },
            ],
        },
    ])]);
    let group = builder
        .push_line_task_group_seed(RuntimeLineTaskGroupSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xf1; 32]),
            activation_ops,
            result_type: crate::pattern::RuntimeSemanticTypeId::from_bytes([1; 32]),
            handle_sites: Box::new([]),
            root,
            cancel_rules: Box::new([RuntimeLineTaskCancelRuleSeed {
                trigger: arcweft_interaction_model::input::InputActionId::new(" cancel.終了 ")
                    .unwrap(),
                action: vec![RuntimeFlowOpSeed::Noop],
            }]),
            cleanup_completed: vec![RuntimeFlowOpSeed::Noop],
            cleanup_cancelled: vec![RuntimeFlowOpSeed::Noop],
            cleanup_failed: vec![RuntimeFlowOpSeed::Noop],
            cleanup_policy: policy,
        })
        .unwrap();
    builder
        .attach_line_task_group_seed(&content, &group)
        .unwrap();
    builder.finish().unwrap()
}
fn handles(states: &[RuntimeCallableStateSeedId]) -> RuntimeExpressionSourceHandles<'_> {
    RuntimeExpressionSourceHandles {
        states,
        specializations: &[],
        helpers: &[],
        methods: &[],
        attempts: &[],
    }
}
fn source(
    plan: &RuntimePlan,
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
    Box<[RuntimeCallableStateSeedId]>,
    Box<[RuntimeDialogueContentPlanSeedId]>,
) {
    let (mut source, functions, states) = content::source(plan, meter());
    let bytes = content::encode(plan);
    let prepared = PreparedRuntimeContentPlanSources::preflight(
        &bytes,
        content::limits(plan),
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    let contents = prepared
        .materialize(&mut source, &functions, &handles(&states), &mut resources())
        .unwrap();
    content::bodies(plan, &mut source, &functions, &states, &contents);
    (source, functions, states, contents)
}
fn restore(plan: &RuntimePlan, bytes: &[u8], bounds: RuntimeLineGroupSourceLimits) -> RuntimePlan {
    let (mut source, _, states, contents) = source(plan);
    let expressions = handles(&states);
    let handles = RuntimeFlowSourceHandles {
        expression: &expressions,
        contents: &contents,
        defers: &[],
        tasks: &[],
    };
    let prepared =
        PreparedRuntimeLineGroupSources::preflight(bytes, bounds, &resources(), &mut source.meter)
            .unwrap();
    let groups = prepared
        .materialize(&mut source, &handles, &mut resources())
        .unwrap();
    let content_bytes = content::encode(plan);
    PreparedRuntimeContentPlanSources::preflight(
        &content_bytes,
        content::limits(plan),
        &resources(),
        &mut source.meter,
    )
    .unwrap()
    .bind_line_groups(&mut source, &contents, &groups)
    .unwrap();
    content::finish(source)
}

#[test]
fn line_sources_restore_original_preorder_mark_owner_cancellation_and_cleanup_through_actual_factories()
 {
    for child_tasks in [ChildTaskCleanup::CancelAndJoin, ChildTaskCleanup::Finish] {
        for presentation in [
            PresentationCleanup::DropRegistered,
            PresentationCleanup::KeepRegistered,
        ] {
            for audio in [
                AudioCleanup::StopRegistered,
                AudioCleanup::FadeRegistered,
                AudioCleanup::KeepRegistered,
            ] {
                for cancel in [ChildCancelPolicy::CancelAndJoin, ChildCancelPolicy::Finish] {
                    let plan = fixture(
                        LineCleanupPolicy {
                            child_tasks,
                            presentation,
                            audio,
                        },
                        cancel,
                    );
                    let bounds = limits(&plan);
                    let encoded = plan
                        .encode_line_group_sources(bounds, &resources(), &mut meter())
                        .unwrap();
                    let prepared = PreparedRuntimeLineGroupSources::preflight(
                        encoded.bytes(),
                        bounds,
                        &resources(),
                        &mut meter(),
                    )
                    .unwrap();
                    assert_eq!(prepared.node_count(), encoded.node_count());
                    assert_eq!(prepared.elements(), encoded.elements());
                    assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
                    let restored = restore(&plan, encoded.bytes(), bounds);
                    assert_eq!(plan.line_task_groups(), restored.line_task_groups());
                    assert_eq!(plan.dialogue_content(), restored.dialogue_content());
                    assert_eq!(
                        plan.executable_semantic_digest(),
                        restored.executable_semantic_digest()
                    );
                    assert_eq!(
                        encoded.bytes(),
                        restored
                            .encode_line_group_sources(bounds, &resources(), &mut meter())
                            .unwrap()
                            .bytes()
                    );
                }
            }
        }
    }
}

#[test]
fn line_sources_admit_cumulative_quota_edges_and_reject_every_truncated_frame() {
    let plan = fixture(
        LineCleanupPolicy::default(),
        ChildCancelPolicy::CancelAndJoin,
    );
    let bounds = limits(&plan);
    let encoded = plan
        .encode_line_group_sources(bounds, &resources(), &mut meter())
        .unwrap();
    let exact = RuntimeLineGroupSourceLimits {
        groups: 1,
        nodes: u32::try_from(encoded.node_count()).unwrap(),
        elements: u32::try_from(encoded.elements()).unwrap(),
        encoded_bytes: encoded.bytes().len(),
        decoded_bytes: encoded.storage_bytes(),
        ..bounds
    };
    let restored = restore(&plan, encoded.bytes(), exact);
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    for bounds in [
        RuntimeLineGroupSourceLimits { groups: 0, ..exact },
        RuntimeLineGroupSourceLimits {
            nodes: exact.nodes - 1,
            ..exact
        },
        RuntimeLineGroupSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeLineGroupSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeLineGroupSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
        RuntimeLineGroupSourceLimits { depth: 3, ..exact },
    ] {
        let mut bank = meter();
        assert!(
            PreparedRuntimeLineGroupSources::preflight(
                encoded.bytes(),
                bounds,
                &resources(),
                &mut bank
            )
            .is_err()
        );
        assert!(bank.status().is_err());
        assert!(
            PreparedRuntimeLineGroupSources::preflight(
                encoded.bytes(),
                exact,
                &resources(),
                &mut bank
            )
            .is_err()
        );
    }
    for end in 0..encoded.bytes().len() {
        assert!(
            PreparedRuntimeLineGroupSources::preflight(
                &encoded.bytes()[..end],
                bounds,
                &resources(),
                &mut meter()
            )
            .is_err(),
            "accepted truncation {end}"
        );
    }
    let policy = DOMAIN.len() + 5 + 32 + 4;
    for (offset, replacement) in [
        (0, 0xff),
        (DOMAIN.len(), 2),
        (policy, 0xff),
        (policy + 1, 0xff),
        (policy + 2, 0xff),
    ] {
        let mut bad = encoded.bytes().to_vec();
        bad[offset] = replacement;
        assert!(
            PreparedRuntimeLineGroupSources::preflight(&bad, bounds, &resources(), &mut meter())
                .is_err()
        );
    }
    let mut trailing = encoded.bytes().to_vec();
    trailing.push(0);
    assert!(matches!(
        PreparedRuntimeLineGroupSources::preflight(&trailing, bounds, &resources(), &mut meter()),
        Err(Error::Trailing { .. })
    ));
    let mut output = Vec::new();
    assert!(matches!(
        encoded.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert!(output.is_empty());
}

#[test]
fn line_sources_authenticate_original_content_and_actual_mark_extent_before_values() {
    let plan = fixture(
        LineCleanupPolicy::default(),
        ChildCancelPolicy::CancelAndJoin,
    );
    let bounds = limits(&plan);
    let encoded = plan
        .encode_line_group_sources(bounds, &resources(), &mut meter())
        .unwrap();
    let (mut source, _, states, _) = source(&plan);
    let (_, _, _, foreign) = self::source(&plan);
    let expressions = handles(&states);
    let handles = RuntimeFlowSourceHandles {
        expression: &expressions,
        contents: &foreign,
        defers: &[],
        tasks: &[],
    };
    let prepared = PreparedRuntimeLineGroupSources::preflight(
        encoded.bytes(),
        bounds,
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    assert!(
        prepared
            .materialize(&mut source, &handles, &mut resources())
            .is_err()
    );
    assert!(source.builder.line_task_groups.is_empty());
    assert!(source.meter.status().is_err());

    let pattern = [3, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0];
    let matches = encoded
        .bytes()
        .windows(pattern.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == pattern).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "exact original Mark field");
    let mut bytes = encoded.bytes().to_vec();
    bytes[matches[0] + 6] = 2;
    let (mut source, _, states, contents) = self::source(&plan);
    let expressions = self::handles(&states);
    let handles = RuntimeFlowSourceHandles {
        expression: &expressions,
        contents: &contents,
        defers: &[],
        tasks: &[],
    };
    let prepared =
        PreparedRuntimeLineGroupSources::preflight(&bytes, bounds, &resources(), &mut source.meter)
            .unwrap();
    assert!(matches!(
        prepared.materialize(&mut source, &handles, &mut resources()),
        Err(Error::Reference {
            kind: "mark",
            actual: 2,
            maximum: 1
        })
    ));
    assert!(source.builder.line_task_groups.is_empty());
    assert!(source.meter.status().is_err());
}

#[test]
fn line_sources_cover_scope_snapshot_storage_and_derive_actual_activation_exports() {
    let plan = scoped_fixture(
        LineCleanupPolicy::default(),
        ChildCancelPolicy::CancelAndJoin,
        32,
    );
    assert_eq!(plan.line_task_groups()[0].activation_exports().len(), 1);
    let bounds = limits(&plan);
    let encoded = plan
        .encode_line_group_sources(bounds, &resources(), &mut meter())
        .unwrap();
    let exact = RuntimeLineGroupSourceLimits {
        decoded_bytes: encoded.storage_bytes(),
        ..bounds
    };
    let restored = restore(&plan, encoded.bytes(), exact);
    assert_eq!(restored.line_task_groups(), plan.line_task_groups());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    assert!(matches!(
        PreparedRuntimeLineGroupSources::preflight(
            encoded.bytes(),
            RuntimeLineGroupSourceLimits {
                decoded_bytes: exact.decoded_bytes - 1,
                ..exact
            },
            &resources(),
            &mut meter()
        ),
        Err(Error::Limit {
            kind: "decoded bytes",
            ..
        })
    ));
}

#[test]
fn line_sources_share_actual_detach_policy_rejection_before_flow_values() {
    let plan = fixture(
        LineCleanupPolicy::default(),
        ChildCancelPolicy::CancelAndJoin,
    );
    let bounds = limits(&plan);
    let encoded = plan
        .encode_line_group_sources(bounds, &resources(), &mut meter())
        .unwrap();
    let policy = DOMAIN.len() + 5 + 32 + 4;
    let pattern = [3, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0];
    let offset = encoded
        .bytes()
        .windows(pattern.len())
        .position(|bytes| bytes == pattern)
        .unwrap();
    for (offset, tag) in [(policy, 1), (offset + 11, 2)] {
        let mut bytes = encoded.bytes().to_vec();
        bytes[offset] = tag;
        let mut bank = meter();
        assert!(
            matches!(PreparedRuntimeLineGroupSources::preflight(&bytes,bounds,&resources(),&mut bank),Err(Error::Construction(error)) if matches!(*error,RuntimePlanBuildError::UnsupportedLineTaskDetach))
        );
        assert!(bank.status().is_err());
    }
}
