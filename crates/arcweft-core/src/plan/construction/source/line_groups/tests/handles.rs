use super::*;
use crate::line_task::{RuntimeLineHandleScope, RuntimeLineHandleSiteKind};
use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeSemanticTypeId};
use crate::runtime_id::RuntimeLineHandleSiteId;
use crate::value::{
    RuntimeEntityReference, RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass,
    RuntimeValue,
};

fn ty(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn site(index: u32) -> RuntimeLineHandleSiteId {
    RuntimeLineHandleSiteId::from_zero_based(index)
}
fn opaque(kind: RuntimeHandleKind, ty: RuntimeSemanticTypeId) -> RuntimePlanTypeSeed {
    RuntimePlanTypeSeed::new(
        ty,
        RuntimePlanTypeProjection::Opaque {
            producer: kind.try_producer().unwrap(),
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::AffineHandle(kind),
            persistence: RuntimeOpaquePersistence::SnapshotOnly,
            arguments: Box::new([]),
        },
    )
}
fn duration(nanos: u64) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        ty(0xf6),
        RuntimeExprSeedKind::Value(RuntimeValue::Duration(
            crate::time::LogicalDuration::from_nanos(nanos),
        )),
    )
}
pub(in crate::plan::construction::source) fn fixture() -> RuntimePlan {
    let (mut builder, content) = crate::plan::body_semantic::content_root_builder_fixture();
    let admitted = builder
        .admit_type_batch(
            [
                opaque(RuntimeHandleKind::StageActor, ty(0xf3)),
                opaque(RuntimeHandleKind::Cue, ty(0xf4)),
                opaque(RuntimeHandleKind::Voice, ty(0xf5)),
                RuntimePlanTypeSeed::new(ty(0xf6), RuntimePlanTypeProjection::Duration),
                RuntimePlanTypeSeed::new(ty(0xf7), RuntimePlanTypeProjection::EntityReference),
            ],
            [(0xf8, ty(0xf3)), (0xf9, ty(2))]
                .into_iter()
                .map(|(identity, ty)| {
                    RuntimeLocalDeclarationSeed::new(
                        RuntimeLocalDeclarationSource::Binding {
                            identity: [identity; 32],
                            declaration: RuntimeLocalBindingDeclaration::new(
                                RuntimeLocalBindingKind::PatternBinding,
                                false,
                                RuntimeLocalBindingStorage::Derived,
                            ),
                        },
                        ty,
                    )
                }),
        )
        .unwrap();
    let actor = admitted.local_ids()[0].clone();
    let packet = admitted.local_ids()[1].clone();
    let character = arcweft_character::id::CharacterId::try_new("character.akane").unwrap();
    let activation_ops = vec![
        RuntimeFlowOpSeed::LineOperation {
            binding: Some(RuntimePatternSeed::new(
                ty(0xf3),
                RuntimePatternSeedKind::Bind {
                    local: actor.clone(),
                    mutable: false,
                },
            )),
            operation: RuntimeLineOperationSeed::AcquireActor {
                site: site(0),
                character: character.clone(),
                scope: RuntimeLineHandleScope::Line,
            },
        },
        RuntimeFlowOpSeed::LineOperation {
            binding: None,
            operation: RuntimeLineOperationSeed::Schedule {
                site: site(1),
                delay: duration(1),
                child: RuntimeLineTaskNodeSeedId::from_zero_based(2).unwrap(),
                captures: Box::new([RuntimeScheduledCaptureSeed {
                    local: packet.clone(),
                    value: RuntimeExprSeed::new(
                        ty(2),
                        RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                    ),
                }]),
            },
        },
    ];
    let root = RuntimeLineTaskNodeSeed::Sequence(vec![
        RuntimeLineTaskNodeSeed::Action(vec![
            RuntimeFlowOpSeed::LineOperation {
                binding: None,
                operation: RuntimeLineOperationSeed::ActorLook {
                    site: site(2),
                    character: character.clone(),
                    actor: RuntimeBorrowedLocalSeed::new(actor),
                    look: RuntimeExprSeed::new(
                        ty(0xf7),
                        RuntimeExprSeedKind::EntityRef(RuntimeEntityReference::CharacterLook {
                            character: character.clone(),
                            look: arcweft_character::id::CharacterLookId::try_new("happy").unwrap(),
                        }),
                    ),
                    crossfade: duration(2),
                },
            },
            RuntimeFlowOpSeed::LineOperation {
                binding: None,
                operation: RuntimeLineOperationSeed::VoiceHandle { site: site(3) },
            },
        ]),
        RuntimeLineTaskNodeSeed::Child {
            node: RuntimeLineTaskNodeSeedId::from_zero_based(2).unwrap(),
            trigger: RuntimeLineTaskTriggerSeed::Scheduled(site(1)),
            join_policy: ChildJoinPolicy::Join,
            cancel_policy: ChildCancelPolicy::CancelAndJoin,
            scope: Box::new(RuntimeLineTaskNodeSeed::Action(vec![
                RuntimeFlowOpSeed::Let {
                    pattern: RuntimePatternSeed::new(ty(2), RuntimePatternSeedKind::Discard),
                    expr: RuntimeExprSeed::new(
                        ty(2),
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            packet,
                            crate::value::RuntimeLocalReadMode::Copy,
                        )),
                    ),
                },
            ])),
        },
        RuntimeLineTaskNodeSeed::Child {
            node: RuntimeLineTaskNodeSeedId::from_zero_based(4).unwrap(),
            trigger: RuntimeLineTaskTriggerSeed::Immediate,
            join_policy: ChildJoinPolicy::Join,
            cancel_policy: ChildCancelPolicy::Finish,
            scope: Box::new(RuntimeLineTaskNodeSeed::Action(vec![
                RuntimeFlowOpSeed::Noop,
            ])),
        },
    ]);
    let mut handle_sites = Vec::new();
    for (index, kind, result, character, child) in [
        (
            0,
            RuntimeLineHandleSiteKind::StageActor,
            ty(0xf3),
            Some(character.clone()),
            None,
        ),
        (
            1,
            RuntimeLineHandleSiteKind::ScheduledCue,
            ty(0xf4),
            None,
            Some(RuntimeLineTaskNodeSeedId::from_zero_based(2).unwrap()),
        ),
        (
            2,
            RuntimeLineHandleSiteKind::StageLookCue,
            ty(0xf4),
            Some(character),
            None,
        ),
        (3, RuntimeLineHandleSiteKind::Voice, ty(0xf5), None, None),
    ] {
        handle_sites.push(RuntimeLineHandleSiteSeed {
            id: site(index),
            source_ordinal: index * 11 + 1,
            kind,
            result_type: result,
            character,
            scheduled_child: child,
        });
    }
    let group = builder
        .push_line_task_group_seed(RuntimeLineTaskGroupSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xfb; 32]),
            activation_ops,
            result_type: ty(1),
            handle_sites: handle_sites.into_boxed_slice(),
            root,
            cancel_rules: Box::new([]),
            cleanup_completed: vec![],
            cleanup_cancelled: vec![],
            cleanup_failed: vec![],
            cleanup_policy: LineCleanupPolicy::default(),
        })
        .unwrap();
    builder
        .attach_line_task_group_seed(&content, &group)
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn default_source_artifact_accepts_the_full_line_handle_site_limit() {
    let (mut builder, content) = crate::plan::body_semantic::content_root_builder_fixture();
    let voice = ty(0xee);
    builder
        .admit_type_batch([opaque(RuntimeHandleKind::Voice, voice)], [])
        .unwrap();
    let count = u32::try_from(crate::line_task::MAX_LINE_HANDLE_SITES).unwrap();
    let group = builder
        .push_line_task_group_seed(RuntimeLineTaskGroupSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xed; 32]),
            activation_ops: vec![RuntimeFlowOpSeed::Noop],
            result_type: ty(1),
            handle_sites: (0..count)
                .map(|index| RuntimeLineHandleSiteSeed {
                    id: site(index),
                    source_ordinal: index + 1,
                    kind: RuntimeLineHandleSiteKind::Voice,
                    result_type: voice,
                    character: None,
                    scheduled_child: None,
                })
                .collect(),
            root: RuntimeLineTaskNodeSeed::Action(
                (0..count)
                    .map(|index| RuntimeFlowOpSeed::LineOperation {
                        binding: None,
                        operation: RuntimeLineOperationSeed::VoiceHandle { site: site(index) },
                    })
                    .collect(),
            ),
            cancel_rules: Box::new([]),
            cleanup_completed: vec![],
            cleanup_cancelled: vec![],
            cleanup_failed: vec![],
            cleanup_policy: LineCleanupPolicy::default(),
        })
        .unwrap();
    builder
        .attach_line_task_group_seed(&content, &group)
        .unwrap();
    let original = builder.finish().unwrap();
    let limits = crate::plan::RuntimeTaskSourceArtifactLimits::default();
    for producer_limit in [
        limits.codec.program.opaque_producer_bytes,
        1 << 20,
        usize::MAX,
    ] {
        let mut limits = limits;
        limits.codec.program.opaque_producer_bytes = producer_limit;
        let source = original.task_source_artifact(limits).unwrap();
        let restored = source.admit(limits, None).unwrap();
        assert_eq!(restored.line_task_groups(), original.line_task_groups());
        assert_eq!(restored.task_source_artifact(limits).unwrap(), source);
    }
}

#[test]
fn line_source_storage_counts_actual_producers_independently_of_allowance() {
    let plan = fixture();
    let bounds = limits(&plan);
    let encoded = plan
        .encode_line_group_sources(bounds, &resources(), &mut meter())
        .unwrap();
    for producer_limit in [bounds.opaque_producer_bytes, 1 << 20, usize::MAX] {
        let bounds = RuntimeLineGroupSourceLimits {
            opaque_producer_bytes: producer_limit,
            decoded_bytes: encoded.storage_bytes(),
            ..bounds
        };
        let prepared = PreparedRuntimeLineGroupSources::preflight(
            encoded.bytes(),
            bounds,
            &resources(),
            &mut meter(),
        )
        .unwrap();
        assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
        let restored = restore(&plan, encoded.bytes(), bounds);
        assert_eq!(restored.line_task_groups(), plan.line_task_groups());
        assert_eq!(
            restored.executable_semantic_digest(),
            plan.executable_semantic_digest()
        );
        assert_eq!(
            restored
                .encode_line_group_sources(bounds, &resources(), &mut meter())
                .unwrap()
                .bytes(),
            encoded.bytes()
        );
    }
}

#[test]
fn line_sources_restore_all_handle_site_kinds_scheduled_packets_and_borrowed_actor_operations() {
    let plan = fixture();
    let bounds = limits(&plan);
    let encoded = plan
        .encode_line_group_sources(bounds, &resources(), &mut meter())
        .unwrap();
    let restored = restore(&plan, encoded.bytes(), bounds);
    assert_eq!(plan.line_task_groups(), restored.line_task_groups());
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
    let group = &restored.line_task_groups()[0];
    assert_eq!(group.activation_exports().len(), 1);
    assert!(group.captures().is_empty());
    assert_eq!(
        group
            .handle_sites()
            .iter()
            .map(|site| site.site_kind())
            .collect::<Vec<_>>(),
        [
            RuntimeLineHandleSiteKind::StageActor,
            RuntimeLineHandleSiteKind::ScheduledCue,
            RuntimeLineHandleSiteKind::StageLookCue,
            RuntimeLineHandleSiteKind::Voice,
        ]
    );
}

#[test]
fn line_sources_admit_derived_producer_storage_and_reject_site_metadata_before_values() {
    let plan = fixture();
    let bounds = limits(&plan);
    let encoded = plan
        .encode_line_group_sources(bounds, &resources(), &mut meter())
        .unwrap();
    let count_offset = DOMAIN.len() + 5 + 32 + 4 + 3 + 4;
    let mut over = encoded.bytes().to_vec();
    over[count_offset..count_offset + 4].copy_from_slice(&129u32.to_le_bytes());
    assert!(
        matches!(PreparedRuntimeLineGroupSources::preflight(&over,bounds,&resources(),&mut meter()),Err(Error::Construction(error)) if matches!(*error,RuntimePlanBuildError::LineHandleSiteLimit { actual:129,limit:128 }))
    );

    let mut wrong_type = encoded.bytes().to_vec();
    let type_offset = count_offset + 4 + 4 + 1;
    wrong_type[type_offset..type_offset + 4].copy_from_slice(&1u32.to_le_bytes());
    for (bytes, bounds, producer_limit) in [
        (&wrong_type[..], bounds, false),
        (
            encoded.bytes(),
            RuntimeLineGroupSourceLimits {
                opaque_producer_bytes: bounds.opaque_producer_bytes - 1,
                ..bounds
            },
            true,
        ),
    ] {
        let (mut source, _, states, contents) = super::source(&plan);
        let expressions = super::handles(&states);
        let handles = RuntimeFlowSourceHandles {
            expression: &expressions,
            contents: &contents,
            defers: &[],
            tasks: &[],
        };
        let prepared = PreparedRuntimeLineGroupSources::preflight(
            bytes,
            bounds,
            &resources(),
            &mut source.meter,
        );
        let error = if producer_limit {
            prepared
                .err()
                .expect("the derived producer bound is checked in preflight")
        } else {
            prepared
                .unwrap()
                .materialize(&mut source, &handles, &mut resources())
                .err()
                .unwrap()
        };
        if producer_limit {
            assert!(matches!(
                error,
                Error::Limit {
                    kind: "opaque producer bytes",
                    actual,
                    maximum,
                } if actual == RuntimeHandleKind::StageActor.try_producer().unwrap().as_str().len()
                    && maximum == bounds.opaque_producer_bytes
            ));
        } else {
            assert!(
                matches!(error,Error::Construction(error) if matches!(*error,RuntimePlanBuildError::InvalidLineHandleType {..}))
            );
        }
        assert!(source.builder.line_task_groups.is_empty());
        assert!(source.meter.status().is_err());
    }
}
