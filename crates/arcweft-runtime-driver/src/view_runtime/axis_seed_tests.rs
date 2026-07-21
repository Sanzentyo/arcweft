//! Direct host-axis seed lifecycle, restore, and corruption tests.

use super::*;
use crate::presentation_handles::PresentationResourceState;
use crate::view_runtime::{BundleViewDiagnosticCode, BundleViewRuntime};
use arcweft_bundle::resource_codec::view::ViewProgramInstruction;
use arcweft_bundle::resource_codec::{
    ValidatedViewProduct, ViewDefinitionResource, ViewInstructionSpan, ViewProductValidationLimits,
    ViewProgramResource,
};
use arcweft_view::ViewProgramId;

fn validated(program: Option<ViewProgramResource>) -> ValidatedViewProduct {
    ValidatedViewProduct::try_new(None, program, None, ViewProductValidationLimits::default())
        .unwrap()
}

fn handle_id(value: &str) -> PresentationHandleId {
    PresentationHandleId::try_new(value).unwrap()
}

fn handle_record(
    value: &str,
    kind: PresentationHandleKind,
    state: PresentationResourceState,
) -> PresentationHandleRecord {
    PresentationHandleRecord::new(
        handle_id(value),
        kind,
        "resource.test".to_owned(),
        None,
        state,
        None,
        0,
    )
}

#[test]
fn pending_seed_prepare_is_transactional_and_later_commit_consumes_it_once() {
    let handle = handle_id("handle.pending");
    let mount = ViewMountId::from_raw(7);
    let seed = ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::VerticalRl);
    let mut registry = BundleViewAxisSeedRegistry::default();
    registry.configure_next(handle.clone(), seed, &[]).unwrap();

    let abandoned = registry.prepare_root_mount(&handle, mount).unwrap();
    assert_eq!(registry.snapshot().pending.len(), 1);
    assert!(registry.snapshot().mounted.is_empty());
    drop(abandoned);

    let retry = registry.prepare_root_mount(&handle, mount).unwrap();
    registry.commit_root_mount(retry).unwrap();
    let snapshot = registry.snapshot();
    assert!(snapshot.pending.is_empty());
    assert_eq!(snapshot.mounted.len(), 1);
    assert_eq!(snapshot.mounted[0].seed, seed);
    assert_eq!(
        snapshot.mounted[0].generation,
        ViewBoxAxisSeedGeneration::INITIAL
    );
    assert_eq!(
        registry.cancel_next(&handle),
        None,
        "the reservation is single-use"
    );
}

#[test]
fn reservation_last_write_cancel_and_mount_identity_are_deterministic() {
    let first = handle_id("handle.first");
    let second = handle_id("handle.second");
    let mut registry = BundleViewAxisSeedRegistry::default();
    registry
        .configure_next(first.clone(), ViewBoxAxisHostSeed::Default, &[])
        .unwrap();
    let explicit = ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::HorizontalLtr);
    registry
        .configure_next(first.clone(), explicit, &[])
        .unwrap();
    registry
        .configure_next(second.clone(), explicit, &[])
        .unwrap();
    assert_eq!(registry.cancel_next(&second), Some(explicit));

    let first_plan = registry
        .prepare_root_mount(&first, ViewMountId::from_raw(1))
        .unwrap();
    registry.commit_root_mount(first_plan).unwrap();
    let second_plan = registry
        .prepare_root_mount(&second, ViewMountId::from_raw(2))
        .unwrap();
    registry.commit_root_mount(second_plan).unwrap();
    let first_seed = registry.mounted_seed(ViewMountId::from_raw(1)).unwrap();
    let second_seed = registry.mounted_seed(ViewMountId::from_raw(2)).unwrap();
    assert_eq!(first_seed.source(), ViewBoxAxisSeedSource::HostExplicit);
    assert_eq!(second_seed.source(), ViewBoxAxisSeedSource::HostDefault);
    assert_ne!(first_seed.revision(), second_seed.revision());
}

#[test]
fn host_seed_mode_matrix_and_known_handle_rejections_are_exact_and_atomic() {
    let seeds = [
        ViewBoxAxisHostSeed::Default,
        ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::HorizontalLtr),
        ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::HorizontalRtl),
        ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::VerticalRl),
        ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::VerticalLr),
        ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::VerticalRl),
    ];
    let mut registry = BundleViewAxisSeedRegistry::default();
    let mut derived = Vec::new();
    for (ordinal, seed) in seeds.into_iter().enumerate() {
        let handle = handle_id(&format!("handle.mode.{ordinal}"));
        let mount = ViewMountId::from_raw(u64::try_from(ordinal).unwrap() + 1);
        registry.configure_next(handle.clone(), seed, &[]).unwrap();
        let plan = registry.prepare_root_mount(&handle, mount).unwrap();
        registry.commit_root_mount(plan).unwrap();
        let current = registry.mounted_seed(mount).unwrap();
        assert_eq!(current.mode(), seed.mode());
        assert_eq!(current.source(), seed.source());
        derived.push(current);
    }
    assert_eq!(derived[0].mode(), derived[1].mode());
    assert_ne!(derived[0].source(), derived[1].source());
    assert_ne!(derived[0].revision(), derived[1].revision());
    assert_eq!(derived[3].mode(), derived[5].mode());
    assert_eq!(derived[3].source(), derived[5].source());
    assert_ne!(derived[3].revision(), derived[5].revision());
    assert_eq!(
        derived
            .iter()
            .map(|seed| seed.revision())
            .collect::<BTreeSet<_>>()
            .len(),
        derived.len()
    );

    let terminal = handle_record(
        "handle.terminal",
        PresentationHandleKind::View,
        PresentationResourceState::Released,
    );
    let non_view = handle_record(
        "handle.non-view",
        PresentationHandleKind::Image,
        PresentationResourceState::Mounted,
    );
    let before = registry.snapshot();
    assert_eq!(
        registry.configure_next(
            terminal.id.clone(),
            ViewBoxAxisHostSeed::Default,
            std::slice::from_ref(&terminal),
        ),
        Err(BundleViewAxisSeedError::TerminalHandle {
            handle: terminal.id.clone(),
        })
    );
    assert_eq!(registry.snapshot(), before);
    assert_eq!(
        registry.configure_next(
            non_view.id.clone(),
            ViewBoxAxisHostSeed::Default,
            std::slice::from_ref(&non_view),
        ),
        Err(BundleViewAxisSeedError::NonViewHandle {
            handle: non_view.id.clone(),
        })
    );
    assert_eq!(registry.snapshot(), before);
    let mounted_handle = handle_id("handle.mode.0");
    assert!(matches!(
        registry.configure_next(
            mounted_handle.clone(),
            ViewBoxAxisHostSeed::Default,
            &[],
        ),
        Err(BundleViewAxisSeedError::HandleAlreadyMounted { handle, .. })
            if handle == mounted_handle
    ));
    assert_eq!(registry.snapshot(), before);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one mounted seed lifecycle keeps no-op, tag, mode, stale, mismatch, and exhaustion CAS evidence on the same exact state"
)]
fn live_update_is_checked_noop_or_generation_advancing_without_wrap() {
    let handle = handle_id("handle.live");
    let mount = ViewMountId::from_raw(3);
    let mut registry = BundleViewAxisSeedRegistry::default();
    let plan = registry.prepare_root_mount(&handle, mount).unwrap();
    registry.commit_root_mount(plan).unwrap();
    let initial = registry.mounted_seed(mount).unwrap();
    assert_eq!(
        registry
            .update(BundleViewAxisSeedUpdate {
                mount,
                expected_revision: initial.revision(),
                seed: ViewBoxAxisHostSeed::Default,
            })
            .unwrap(),
        BundleViewAxisSeedUpdateOutcome::Unchanged { seed: initial }
    );

    let explicit = ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::HorizontalLtr);
    let updated = registry
        .update(BundleViewAxisSeedUpdate {
            mount,
            expected_revision: initial.revision(),
            seed: explicit,
        })
        .unwrap();
    let BundleViewAxisSeedUpdateOutcome::Updated { previous, current } = updated else {
        panic!("identity-changing seed must update");
    };
    assert_eq!(previous, initial);
    assert_eq!(current.mode(), initial.mode());
    assert_ne!(current.source(), initial.source());
    assert_ne!(current.revision(), initial.revision());
    assert_eq!(
        registry
            .update(BundleViewAxisSeedUpdate {
                mount,
                expected_revision: current.revision(),
                seed: explicit,
            })
            .unwrap(),
        BundleViewAxisSeedUpdateOutcome::Unchanged { seed: current }
    );
    let mut direct_back = registry.clone();
    let direct_back = direct_back
        .update(BundleViewAxisSeedUpdate {
            mount,
            expected_revision: current.revision(),
            seed: ViewBoxAxisHostSeed::Default,
        })
        .unwrap();
    let BundleViewAxisSeedUpdateOutcome::Updated {
        current: direct_default,
        ..
    } = direct_back
    else {
        panic!("explicit horizontal-ltr to default must update identity");
    };
    assert_eq!(direct_default.mode(), current.mode());
    assert_eq!(direct_default.source(), ViewBoxAxisSeedSource::HostDefault);
    assert_ne!(direct_default.revision(), current.revision());

    let before_rejections = registry.snapshot();
    assert_eq!(
        registry.update(BundleViewAxisSeedUpdate {
            mount,
            expected_revision: initial.revision(),
            seed: ViewBoxAxisHostSeed::Default,
        }),
        Err(BundleViewAxisSeedError::RevisionMismatch {
            mount,
            expected: initial.revision(),
            actual: current.revision(),
        })
    );
    assert_eq!(registry.snapshot(), before_rejections);
    assert_eq!(
        registry.update(BundleViewAxisSeedUpdate {
            mount: ViewMountId::from_raw(99),
            expected_revision: current.revision(),
            seed: explicit,
        }),
        Err(BundleViewAxisSeedError::StaleMount {
            mount: ViewMountId::from_raw(99),
        })
    );
    assert_eq!(registry.snapshot(), before_rejections);

    let changed_mode = registry
        .update(BundleViewAxisSeedUpdate {
            mount,
            expected_revision: current.revision(),
            seed: ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::HorizontalRtl),
        })
        .unwrap();
    let BundleViewAxisSeedUpdateOutcome::Updated {
        current: horizontal_rtl,
        ..
    } = changed_mode
    else {
        panic!("mode change must advance the seed generation");
    };
    assert_eq!(
        registry.snapshot().mounted[0].generation.value(),
        2,
        "default-to-explicit identity change and the later mode change each advance once"
    );
    let back_to_default = registry
        .update(BundleViewAxisSeedUpdate {
            mount,
            expected_revision: horizontal_rtl.revision(),
            seed: ViewBoxAxisHostSeed::Default,
        })
        .unwrap();
    let BundleViewAxisSeedUpdateOutcome::Updated {
        current: restored_default,
        ..
    } = back_to_default
    else {
        panic!("explicit-to-default must remain an identity change");
    };
    assert_eq!(
        restored_default.source(),
        ViewBoxAxisSeedSource::HostDefault
    );
    assert_ne!(restored_default.revision(), initial.revision());

    let max_generation: ViewBoxAxisSeedGeneration =
        serde_json::from_value(serde_json::json!(u64::MAX)).unwrap();
    let derived = ViewInheritedBoxAxes::for_host_seed(mount, max_generation, explicit);
    let snapshot = BundleViewAxisSeedRegistrySnapshot {
        pending: Vec::new(),
        mounted: vec![BundleViewMountedAxisSeedSnapshot {
            handle: handle.clone(),
            mount,
            seed: explicit,
            generation: max_generation,
            derived,
        }],
    };
    let roots = BTreeMap::from([(mount, handle.clone())]);
    let handles = [handle_record(
        "handle.live",
        PresentationHandleKind::View,
        PresentationResourceState::Mounted,
    )];
    let mut exhausted = BundleViewAxisSeedRegistry::restore(&snapshot, &roots, &handles).unwrap();
    assert!(matches!(
        exhausted.update(BundleViewAxisSeedUpdate {
            mount,
            expected_revision: derived.revision(),
            seed: ViewBoxAxisHostSeed::Default,
        }),
        Err(BundleViewAxisSeedError::RevisionExhausted { .. })
    ));
    assert_eq!(exhausted.snapshot(), snapshot);
}

#[test]
fn hidden_unmounted_terminal_and_remount_lifecycle_retains_or_replaces_exact_state() {
    let handle = handle_id("handle.lifecycle");
    let pending = handle_id("handle.pending.lifecycle");
    let mount = ViewMountId::from_raw(40);
    let mut registry = BundleViewAxisSeedRegistry::default();
    registry
        .configure_next(
            handle.clone(),
            ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::VerticalRl),
            &[],
        )
        .unwrap();
    registry
        .configure_next(
            pending.clone(),
            ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::HorizontalRtl),
            &[],
        )
        .unwrap();
    let plan = registry.prepare_root_mount(&handle, mount).unwrap();
    registry.commit_root_mount(plan).unwrap();
    let retained = registry.snapshot();

    for state in [
        PresentationResourceState::Hidden,
        PresentationResourceState::Unmounted,
    ] {
        let record = handle_record("handle.lifecycle", PresentationHandleKind::View, state);
        assert!(registry.cleanup_known_handles(&[record]).is_empty());
        registry.retain_mounts(&BTreeSet::from([mount]));
        assert_eq!(registry.snapshot(), retained);
    }

    let terminal = handle_record(
        "handle.lifecycle",
        PresentationHandleKind::View,
        PresentationResourceState::Destroyed,
    );
    let terminal_pending = handle_record(
        "handle.pending.lifecycle",
        PresentationHandleKind::View,
        PresentationResourceState::Released,
    );
    assert!(
        registry
            .cleanup_known_handles(&[terminal, terminal_pending])
            .is_empty()
    );
    assert!(registry.snapshot().mounted.is_empty());
    assert!(registry.snapshot().pending.is_empty());

    let remount = ViewMountId::from_raw(41);
    let plan = registry.prepare_root_mount(&handle, remount).unwrap();
    registry.commit_root_mount(plan).unwrap();
    let remounted = registry.mounted_seed(remount).unwrap();
    assert_eq!(remounted.source(), ViewBoxAxisSeedSource::HostDefault);
    assert_ne!(remounted.revision(), retained.mounted[0].derived.revision());
}

#[test]
fn same_evaluation_non_view_resolution_discards_pending_seed_and_emits_one_diagnostic() {
    let handle = handle_id("handle.prospective");
    let mut runtime = BundleViewRuntime::try_new(validated(None), None).unwrap();
    runtime
        .configure_next_axis_seed(
            handle.clone(),
            ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::VerticalLr),
            &[],
        )
        .unwrap();
    let record = handle_record(
        "handle.prospective",
        PresentationHandleKind::Image,
        PresentationResourceState::Mounted,
    );

    let frame = runtime.evaluate(&[record], &[], false);
    assert_eq!(frame.diagnostics.len(), 1);
    assert_eq!(
        frame.diagnostics[0].code,
        BundleViewDiagnosticCode::InvalidControlFlow
    );
    assert_eq!(frame.diagnostics[0].handle.as_ref(), Some(&handle));
    assert_eq!(runtime.cancel_next_axis_seed(&handle), None);
    assert!(runtime.snapshot().unwrap().axis_seeds.pending.is_empty());
}

#[test]
fn nested_mount_host_mutation_is_rejected_without_changing_runtime_state() {
    let program = ViewProgramResource {
        program_id: ViewProgramId::try_new("view.program.nested-axis-update").unwrap(),
        definitions: vec![
            ViewDefinitionResource {
                public_id: arcweft_bundle::resource_codec::view::ViewDefinitionRef::new(
                    arcweft_view::ViewId::try_new("view.Parent").unwrap(),
                ),
                body: ViewInstructionSpan::new(0, 1),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 1,
            },
            ViewDefinitionResource {
                public_id: arcweft_bundle::resource_codec::view::ViewDefinitionRef::new(
                    arcweft_view::ViewId::try_new("view.Child").unwrap(),
                ),
                body: ViewInstructionSpan::new(1, 1),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 2,
            },
        ],
        instructions: vec![ViewProgramInstruction::CallView {
            view: arcweft_bundle::resource_codec::view::ViewDefinitionRef::new(
                arcweft_view::ViewId::try_new("view.Child").unwrap(),
            ),
            arguments: Vec::new(),
            styles: Vec::new(),
            part: None,
            key: None,
            source: None,
        }],
        ..ViewProgramResource::default()
    };
    let mut runtime = BundleViewRuntime::try_new(validated(Some(program)), None).unwrap();
    let parent = PresentationHandleRecord::new(
        handle_id("handle.nested.parent"),
        PresentationHandleKind::View,
        "view.Parent".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let frame = runtime.evaluate(std::slice::from_ref(&parent), &[], false);
    assert!(frame.diagnostics.is_empty());
    let root = frame
        .mounts
        .iter()
        .find(|output| output.path.segments().is_empty())
        .unwrap();
    let nested = frame
        .mounts
        .iter()
        .find(|output| !output.path.segments().is_empty())
        .unwrap();
    let before = runtime.snapshot().unwrap();

    assert_eq!(
        runtime.update_axis_seed(BundleViewAxisSeedUpdate {
            mount: nested.mount,
            expected_revision: root.host_axis_seed.unwrap().revision(),
            seed: ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::VerticalRl,),
        }),
        Err(BundleViewAxisSeedError::NestedMount {
            mount: nested.mount,
        })
    );
    assert_eq!(runtime.snapshot().unwrap(), before);

    let mut tampered = before.clone();
    let nested_generation = ViewBoxAxisSeedGeneration::INITIAL;
    let nested_seed = ViewBoxAxisHostSeed::Default;
    tampered
        .axis_seeds
        .mounted
        .push(BundleViewMountedAxisSeedSnapshot {
            handle: parent.id.clone(),
            mount: nested.mount,
            seed: nested_seed,
            generation: nested_generation,
            derived: ViewInheritedBoxAxes::for_host_seed(
                nested.mount,
                nested_generation,
                nested_seed,
            ),
        });
    assert!(matches!(
        runtime.restore(&tampered, std::slice::from_ref(&parent)),
        Err(BundleViewRuntimeError::AxisSeed(
            BundleViewAxisSeedError::UnknownSnapshotMount { mount }
        )) if mount == nested.mount
    ));
    assert_eq!(runtime.snapshot().unwrap(), before);
}

#[test]
fn restore_rejects_duplicates_tampering_and_non_view_lifecycle_atomically() {
    let handle = handle_id("handle.restore");
    let mount = ViewMountId::from_raw(5);
    let seed = ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::VerticalLr);
    let generation = ViewBoxAxisSeedGeneration::INITIAL;
    let derived = ViewInheritedBoxAxes::for_host_seed(mount, generation, seed);
    let mounted = BundleViewMountedAxisSeedSnapshot {
        handle: handle.clone(),
        mount,
        seed,
        generation,
        derived,
    };
    let roots = BTreeMap::from([(mount, handle.clone())]);
    let handles = [handle_record(
        "handle.restore",
        PresentationHandleKind::View,
        PresentationResourceState::Mounted,
    )];
    let valid = BundleViewAxisSeedRegistrySnapshot {
        pending: Vec::new(),
        mounted: vec![mounted.clone()],
    };
    let restored = BundleViewAxisSeedRegistry::restore(&valid, &roots, &handles).unwrap();
    assert_eq!(restored.snapshot(), valid);
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(&valid, &roots, &[]),
        Err(BundleViewAxisSeedError::UnknownSnapshotMount { mount: unknown })
            if unknown == mount
    ));

    let duplicate = BundleViewAxisSeedRegistrySnapshot {
        pending: Vec::new(),
        mounted: vec![mounted.clone(), mounted.clone()],
    };
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(&duplicate, &roots, &handles),
        Err(BundleViewAxisSeedError::DuplicateMount { .. })
    ));

    let mut tampered = mounted.clone();
    tampered.derived =
        ViewInheritedBoxAxes::for_host_seed(mount, generation, ViewBoxAxisHostSeed::Default);
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(
            &BundleViewAxisSeedRegistrySnapshot {
                pending: Vec::new(),
                mounted: vec![tampered],
            },
            &roots,
            &handles,
        ),
        Err(BundleViewAxisSeedError::SnapshotSeedSource { .. }
            | BundleViewAxisSeedError::SnapshotSeedMismatch { .. })
    ));

    let non_view = [handle_record(
        "handle.restore",
        PresentationHandleKind::Menu,
        PresentationResourceState::Mounted,
    )];
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(&valid, &roots, &non_view),
        Err(BundleViewAxisSeedError::SnapshotNonViewHandle { .. })
    ));
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(
            &BundleViewAxisSeedRegistrySnapshot::default(),
            &roots,
            &handles,
        ),
        Err(BundleViewAxisSeedError::MissingSnapshotSeed { .. })
    ));
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the restore cross-table matrix keeps every duplicate, lifecycle, identity, and prospective reservation invariant explicit"
)]
fn restore_cross_table_matrix_is_strict_and_replay_is_exact() {
    let handle = handle_id("handle.matrix");
    let other = handle_id("handle.matrix.other");
    let prospective = handle_id("handle.matrix.future");
    let mount = ViewMountId::from_raw(50);
    let other_mount = ViewMountId::from_raw(51);
    let seed = ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::VerticalLr);
    let generation = ViewBoxAxisSeedGeneration::INITIAL.checked_next().unwrap();
    let mounted = BundleViewMountedAxisSeedSnapshot {
        handle: handle.clone(),
        mount,
        seed,
        generation,
        derived: ViewInheritedBoxAxes::for_host_seed(mount, generation, seed),
    };
    let handles = [handle_record(
        "handle.matrix",
        PresentationHandleKind::View,
        PresentationResourceState::Mounted,
    )];
    let roots = BTreeMap::from([(mount, handle.clone())]);
    let valid = BundleViewAxisSeedRegistrySnapshot {
        pending: vec![BundleViewPendingAxisSeedSnapshot {
            handle: prospective.clone(),
            seed: ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::HorizontalRtl),
        }],
        mounted: vec![mounted.clone()],
    };
    let restored = BundleViewAxisSeedRegistry::restore(&valid, &roots, &handles).unwrap();
    assert_eq!(restored.snapshot(), valid);
    let mut cloned = restored.clone();
    let mut replay = restored.clone();
    let next_seed = ViewBoxAxisHostSeed::Explicit(arcweft_view::ViewBoxAxisMode::VerticalRl);
    let update = BundleViewAxisSeedUpdate {
        mount,
        expected_revision: mounted.derived.revision(),
        seed: next_seed,
    };
    assert_eq!(
        cloned.update(update).unwrap(),
        replay.update(update).unwrap()
    );
    assert_eq!(cloned.snapshot(), replay.snapshot());
    let future_mount = ViewMountId::from_raw(52);
    let future_plan = cloned
        .prepare_root_mount(&prospective, future_mount)
        .unwrap();
    cloned.commit_root_mount(future_plan).unwrap();
    assert_eq!(
        cloned.mounted_seed(future_mount).unwrap(),
        ViewInheritedBoxAxes::for_host_seed(
            future_mount,
            ViewBoxAxisSeedGeneration::INITIAL,
            valid.pending[0].seed,
        )
    );
    assert!(
        !cloned
            .snapshot()
            .pending
            .iter()
            .any(|entry| entry.handle == prospective)
    );

    let duplicate_pending = BundleViewAxisSeedRegistrySnapshot {
        pending: vec![valid.pending[0].clone(), valid.pending[0].clone()],
        mounted: vec![mounted.clone()],
    };
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(&duplicate_pending, &roots, &handles),
        Err(BundleViewAxisSeedError::DuplicatePendingHandle { .. })
    ));

    let duplicate_handle = BundleViewAxisSeedRegistrySnapshot {
        pending: Vec::new(),
        mounted: vec![
            mounted.clone(),
            BundleViewMountedAxisSeedSnapshot {
                mount: other_mount,
                derived: ViewInheritedBoxAxes::for_host_seed(other_mount, generation, seed),
                ..mounted.clone()
            },
        ],
    };
    let duplicate_roots = BTreeMap::from([(mount, handle.clone()), (other_mount, handle.clone())]);
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(&duplicate_handle, &duplicate_roots, &handles,),
        Err(BundleViewAxisSeedError::DuplicateMountedHandle { .. })
    ));

    let pending_for_mounted = BundleViewAxisSeedRegistrySnapshot {
        pending: vec![BundleViewPendingAxisSeedSnapshot {
            handle: handle.clone(),
            seed: ViewBoxAxisHostSeed::Default,
        }],
        mounted: vec![mounted.clone()],
    };
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(&pending_for_mounted, &roots, &handles),
        Err(BundleViewAxisSeedError::PendingForMountedHandle { .. })
    ));

    let mismatched_roots = BTreeMap::from([(mount, other.clone())]);
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(&valid, &mismatched_roots, &handles),
        Err(BundleViewAxisSeedError::SnapshotHandleMismatch { .. })
    ));

    let terminal = [handle_record(
        "handle.matrix.future",
        PresentationHandleKind::View,
        PresentationResourceState::Released,
    )];
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(
            &BundleViewAxisSeedRegistrySnapshot {
                pending: valid.pending.clone(),
                mounted: Vec::new(),
            },
            &BTreeMap::new(),
            &terminal,
        ),
        Err(BundleViewAxisSeedError::SnapshotTerminalHandle { .. })
    ));

    let non_view = [handle_record(
        "handle.matrix.future",
        PresentationHandleKind::Overlay,
        PresentationResourceState::Mounted,
    )];
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(
            &BundleViewAxisSeedRegistrySnapshot {
                pending: valid.pending.clone(),
                mounted: Vec::new(),
            },
            &BTreeMap::new(),
            &non_view,
        ),
        Err(BundleViewAxisSeedError::SnapshotNonViewHandle { .. })
    ));

    let mut mismatched_derived = serde_json::to_value(&mounted).unwrap();
    mismatched_derived["derived"]["revision"] = serde_json::json!(1);
    let mismatched_derived: BundleViewMountedAxisSeedSnapshot =
        serde_json::from_value(mismatched_derived).unwrap();
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(
            &BundleViewAxisSeedRegistrySnapshot {
                pending: Vec::new(),
                mounted: vec![mismatched_derived],
            },
            &roots,
            &handles,
        ),
        Err(BundleViewAxisSeedError::SnapshotSeedMismatch { .. })
    ));

    let unknown_mount = ViewMountId::from_raw(99);
    let unknown = BundleViewMountedAxisSeedSnapshot {
        mount: unknown_mount,
        derived: ViewInheritedBoxAxes::for_host_seed(unknown_mount, generation, seed),
        ..mounted
    };
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(
            &BundleViewAxisSeedRegistrySnapshot {
                pending: Vec::new(),
                mounted: vec![unknown],
            },
            &roots,
            &handles,
        ),
        Err(BundleViewAxisSeedError::UnknownSnapshotMount { mount })
            if mount == unknown_mount
    ));
}

#[test]
fn snapshot_wire_rejects_unknown_fields_and_preserves_duplicate_evidence() {
    assert!(
        serde_json::from_value::<BundleViewAxisSeedRegistrySnapshot>(serde_json::json!({
            "pending": [],
            "mounted": [],
            "unknown": true
        }))
        .is_err()
    );
    let decoded: BundleViewAxisSeedRegistrySnapshot = serde_json::from_value(serde_json::json!({
        "pending": [
            {"handle": "handle.duplicate", "seed": {"kind": "default"}},
            {"handle": "handle.duplicate", "seed": {"kind": "default"}}
        ],
        "mounted": []
    }))
    .unwrap();
    assert_eq!(decoded.pending.len(), 2);
    assert!(matches!(
        BundleViewAxisSeedRegistry::restore(&decoded, &BTreeMap::new(), &[]),
        Err(BundleViewAxisSeedError::DuplicatePendingHandle { .. })
    ));
}

#[test]
fn ordinary_and_dialogue_restore_roots_cannot_share_a_handle_identity() {
    let handle = handle_id("handle.restore.collision");
    let ordinary = PresentationHandleRecord::new(
        handle.clone(),
        PresentationHandleKind::View,
        "view.Ordinary".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let frame = arcweft_render_text::LineDisplayFrame {
        line: arcweft_core::plan::RuntimeLineId::from_runtime_line_value("line.restore.collision")
            .unwrap(),
        callee: "narrator".to_owned(),
        speaker_label: None,
        text: String::new(),
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        nodes: Vec::new(),
        display_map: arcweft_render_text::RichTextDisplayMap::default(),
        host_events: Vec::new(),
        inline_failures: Vec::new(),
        unresolved: Vec::new(),
    };
    let dialogue_view = arcweft_view::ViewId::try_new("view.Dialogue").unwrap();
    let dialogue = crate::dialogue::DialogueViewInput {
        handle: handle.clone(),
        view: &dialogue_view,
        frame: &frame,
        state: crate::dialogue::DialogueViewState {
            occurrence: crate::dialogue::DialogueViewOccurrence {
                presentation: arcweft_view::DialoguePresentationId::new(1),
                entry: arcweft_view::DialogueEntryId::new(1),
                instance: arcweft_view::DialogueInstanceId::new(1),
            },
            stage: crate::dialogue::DialogueViewStage {
                index: arcweft_view::DialogueStageIndex::new(0),
                page: crate::dialogue::DialoguePageIndex::new(0),
                stage_count: 1,
                page_count: 1,
            },
            reveal: crate::dialogue::DialogueViewReveal::complete(),
            primary_action: crate::dialogue::DialogueViewPrimaryAction { target: None },
        },
    };

    assert_eq!(
        super::super::reconciled_root_handles_for_restore(&[ordinary], &[dialogue]),
        Err(BundleViewAxisSeedError::SnapshotRootHandleCollision { handle })
    );
}
