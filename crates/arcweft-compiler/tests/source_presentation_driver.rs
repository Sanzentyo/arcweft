use arcweft_bundle::{ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary};
use arcweft_compiler::source::compile_source;
use arcweft_core::{
    effect::RuntimeArtifactFingerprint,
    engine::{FlowExit, FlowFiberStatus},
    presentation::{
        RuntimeLineHostCommand, RuntimeLineHostOutcome, RuntimeStageCommand,
        RuntimeStageCommandOutcome,
    },
    step::RuntimeStepMode,
    value::RuntimeScalarView,
};
use arcweft_interaction_model::presentation::{ImageObjectFit, PresentationImageParam};
use arcweft_runtime_driver::{
    clock::RuntimeClockStep,
    session::{BundleSession, BundleSessionOptions, BundleStepInput},
    session_save::BundleSessionSaveError,
    view_runtime::BundleViewTextValue,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

fn session(source: &str) -> BundleSession {
    session_from_bytes(&source_bundle(source))
}

fn source_bundle(source: &str) -> Vec<u8> {
    let compiled = compile_source(source).expect("Source presentation compiles");
    let generation = compiled
        .character_dialogue_generation
        .as_ref()
        .unwrap()
        .as_ref()
        .clone();
    let program = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "source-presentation.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let views = compiled.view_product();
    let product = views.product();
    let bundle = ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            locale: Default::default(),
            entry: Some("entry.main".to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                artifact_fingerprint: RuntimeArtifactFingerprint::try_from_bytes([0x42; 32])
                    .unwrap(),
                entry_flow: Some("flow.main".to_owned()),
                flows: compiled.plan.flows().len(),
                bytecode_instructions: program.instructions.len(),
                line_task_groups: program.line_task_groups.len(),
                stream_plans: 0,
            },
        },
        product.source_map().clone(),
        program,
        compiled.dialogue_content.clone(),
    )
    .unwrap()
    .try_with_validated_view_product(product.as_ref())
    .unwrap()
    .with_character_dialogue_generation(generation);
    let bundle = match compiled.character_presentation_catalog() {
        Some(catalog) => bundle.with_character_presentation_catalog(catalog.clone()),
        None => bundle,
    };
    let bundle = match views.text() {
        Some(text) => bundle.with_view_text(text.clone()),
        None => bundle,
    };
    bundle.to_format_bytes(BundleFormat::Awfb).unwrap()
}

fn session_from_bytes(bytes: &[u8]) -> BundleSession {
    BundleSession::from_awfb_bytes(
        bytes,
        BundleSessionOptions {
            mode: RuntimeStepMode::OneOp,
            max_ops: 1,
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn source_dialogue_line_commands_resume_through_the_bundle_session_after_restore() {
    let bytes = source_bundle(
        r#"
pub character alice { display = "Alice" }
flow main() -> i64 {
    alice()[Heard[p]] with {
        init {
            let actor = alice.stage.acquire(scope=line)
            let _ = actor
        }
    }
    return 42i64
}
entry cli @entry.main { goto @flow.main }
"#,
    );
    let mut session = session_from_bytes(&bytes);
    let mut outcomes = Vec::new();
    let mut acquired_actor = None;
    let mut released = false;
    for tick in 1..=256 {
        let step = session.step_with_clock(
            RuntimeClockStep::from_millis(tick, 1).unwrap(),
            BundleStepInput {
                line_outcomes: std::mem::take(&mut outcomes),
                ..BundleStepInput::default()
            },
        );
        assert!(step.stats.executed_ops <= 1);
        assert!(step.diagnostics.is_empty(), "{:?}", step.diagnostics);
        if !step.line_commands.is_empty() {
            let save = session.export_session_save_bytes().unwrap();
            let mut restored = session_from_bytes(&bytes);
            restored
                .import_session_save_bytes(&save, &Default::default())
                .unwrap();
            assert_eq!(restored.export_session_save_bytes().unwrap(), save);
            session = restored;
        }
        for request in step.line_commands {
            let outcome = match request {
                RuntimeLineHostCommand::Stage(RuntimeStageCommand::AcquireActor {
                    command,
                    actor,
                    character,
                    ..
                }) => {
                    assert_eq!(character.as_str(), "character.alice");
                    assert_eq!(actor.activation(), command.activation());
                    assert!(acquired_actor.replace(actor.clone()).is_none());
                    RuntimeStageCommandOutcome::Acquired { command, actor }
                }
                RuntimeLineHostCommand::Stage(RuntimeStageCommand::ReleaseActor {
                    command,
                    actor,
                }) => {
                    assert_eq!(actor.activation(), command.activation());
                    assert_eq!(acquired_actor.as_ref(), Some(&actor));
                    assert!(!released);
                    released = true;
                    RuntimeStageCommandOutcome::ReleasedActor { command, actor }
                }
                other => panic!("unexpected line command {other:?}"),
            };
            outcomes.push(RuntimeLineHostOutcome::Stage(outcome));
        }
        if let Some((dialogue, _)) = step.presentation.dialogue.waiting_entries().next() {
            session.queue_dialogue_advance(dialogue.advance_target().unwrap());
        }
        match step.fiber_status {
            FlowFiberStatus::Done(exit) => {
                assert_eq!(exit, FlowExit::Return("42".into()));
                assert!(released && outcomes.is_empty());
                return;
            }
            FlowFiberStatus::Failed(failure) => panic!("dialogue failed: {failure}"),
            _ => {}
        }
    }
    panic!("line command acknowledgements did not finish the flow");
}

fn run(session: &mut BundleSession, mut observe: impl FnMut(&mut BundleSession)) {
    for tick in 1..=256 {
        let result = session.step_with_clock(
            RuntimeClockStep::from_millis(tick, 1).unwrap(),
            BundleStepInput {
                view_bindings: vec![
                    arcweft_core::value::RuntimeBinding {
                        name: "caption".into(),
                        value: arcweft_core::value::RuntimeValue::String("host-wrong".into()),
                    },
                    arcweft_core::value::RuntimeBinding {
                        name: "suffix".into(),
                        value: arcweft_core::value::RuntimeValue::String("host-wrong".into()),
                    },
                ],
                ..Default::default()
            },
        );
        assert!(result.stats.executed_ops <= 1);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(result.requested_host_calls.is_empty());
        assert!(result.requested_tasks.is_empty());
        observe(session);
        let before = session
            .snapshot_session()
            .expect("Source frame is publicly saveable");
        let saved = session.snapshot_session().unwrap();
        session
            .restore_session_snapshot(saved)
            .expect("original Core and View frame restore together");
        assert_eq!(session.snapshot_session().unwrap(), before);
        if let FlowFiberStatus::Done(exit) = result.fiber_status {
            assert_eq!(exit, FlowExit::Return("42".into()));
            return;
        }
    }
    panic!("Source presentation lifecycle exceeded its deterministic step bound");
}

#[test]
fn source_images_keep_typed_metadata_distinct_occurrences_and_owned_restore() {
    let mut session = session(
        r#"
fn surface<T>(marker: T) -> Handle<ImageSurface> {
    image(asset = @asset.bg.pulse, id = "image.same",
        x = 96px, y = 72px, width = 360px, height = 180px,
        fit = "stretch", opacity = 0.5, depth = 2500,
        playback.local_time = 50ms, playback.rate = 2.0,
        transform.tx = 24px, transform.ty = 12px,
        param.user.role = "animated-hotspot", param.色 = "red",
        action = "action.inspect.sprite",
        proxy.id = "proxy.sprite", proxy.type = "SpriteHotspot",
        proxy.role = "inspect", proxy.depth = 2600, proxy.hit_test = true,
        proxy.param.channel = "preview")
}
flow main() -> i64 {
    let first = surface(7i64);
    let second = surface("marker");
    wait(10ms)
    first.hide();
    wait(10ms)
    first.show();
    wait(10ms)
    first.release();
    wait(10ms)
    second.release();
    return 42i64
}
entry cli @entry.main { goto @flow.main }
"#,
    );
    let (mut two, mut hidden, mut shown, mut one, mut empty, mut forged_rejected) =
        (false, false, false, false, false, false);
    let mut first_id = None;
    run(&mut session, |session| {
        let images = &session.presentation().images;
        two |= images.len() == 2;
        empty |= two && images.is_empty();
        if images.len() == 2 {
            assert_ne!(images[0].object_id(), images[1].object_id());
            assert!(images.iter().all(|image| image.id == "image.same"));
            first_id
                .get_or_insert_with(|| session.presentation().presentation_handles[0].id.clone());
        }
        for image in images {
            assert_eq!(image.asset, "asset.bg.pulse");
            assert_eq!(
                (image.bounds.x_milli, image.bounds.y_milli),
                (96_000, 72_000)
            );
            assert_eq!(
                (image.bounds.width_milli, image.bounds.height_milli),
                (360_000, 180_000)
            );
            assert_eq!(
                (image.transform.tx_milli, image.transform.ty_milli),
                (24_000, 12_000)
            );
            assert_eq!(image.opacity_milli, 500);
            assert_eq!(image.fit, ImageObjectFit::Stretch);
            assert_eq!(image.depth_milli, 2500);
            assert_eq!(image.playback.pinned_local_time_millis(), Some(50));
            assert_eq!(image.playback.rate_milli(), 2000);
            assert_eq!(
                image.params["user.role"],
                PresentationImageParam::Text {
                    value: "animated-hotspot".into()
                }
            );
            assert_eq!(
                image.params["色"],
                PresentationImageParam::Text {
                    value: "red".into()
                }
            );
            assert_eq!(
                image.proxies[0].params["channel"],
                PresentationImageParam::Text {
                    value: "preview".into()
                }
            );
            assert!(image.proxies[0].hit_test);
            assert_eq!(image.proxies[0].depth_milli, Some(2600));
            if image.occurrence == first_id {
                hidden |= !image.visible;
                shown |= hidden && image.visible;
            }
        }
        one |= two && images.len() == 1 && images[0].occurrence != first_id;
        if two && !forged_rejected {
            let before = session.snapshot_session().unwrap();
            let mut forged = session.snapshot_session().unwrap();
            forged.presentation.images.clear();
            assert!(matches!(
                session.restore_session_snapshot(forged),
                Err(BundleSessionSaveError::Presentation { .. })
            ));
            assert_eq!(session.snapshot_session().unwrap(), before);
            forged_rejected = true;
        }
    });
    assert!(two && hidden && shown && one && empty && forged_rejected);
}

#[test]
fn source_scroll_and_virtual_list_share_live_ownership_across_cold_restore() {
    use arcweft_bundle::resource_codec::ViewScrollAxis;
    use arcweft_runtime_driver::view_runtime::{
        BundleViewPaintItem, BundleViewStyleNodeKind, BundleViewTextOutputIndex,
    };
    use arcweft_view::{
        program::{ViewStableKey, ViewVirtualAxis},
        virtualization::{ViewVirtualItem, ViewVirtualScrollTarget},
    };

    let bytes = source_bundle(
        r#"
view Inventory() {
    Scroll(target = @scroll:.inventory, width = 200px, height = 100px) {
        Text("Inventory")
        Text("Details")
        Button("Confirm")
    }
}
flow main() -> i64 {
    let inventory = mount(Inventory());
    wait(10ms)
    inventory.destroy();
    return 42i64
}
entry cli @entry.main { goto @flow.main }
"#,
    );
    let mut session = session_from_bytes(&bytes);
    let mut virtual_mount = None;
    let mut retired = false;
    run(&mut session, |session| {
        let target = session
            .presentation()
            .view
            .scroll_occurrences()
            .next()
            .map(|scroll| scroll.target.clone());
        if let Some(mount) = virtual_mount {
            if let Some(target) = target {
                let list = session.view_virtualization().get(mount).unwrap();
                assert_eq!(list.scroll_target().target(), &target);
            } else {
                assert!(session.view_virtualization().get(mount).is_none());
                retired = true;
            }
            return;
        }
        let Some(target) = target else { return };
        let scroll_target = ViewVirtualScrollTarget::from(target);
        assert!(
            session
                .mount_virtual_list(
                    scroll_target.clone(),
                    ViewVirtualAxis::Horizontal,
                    100_000,
                    Vec::new()
                )
                .is_err()
        );
        virtual_mount = Some(
            session
                .mount_virtual_list(
                    scroll_target,
                    ViewVirtualAxis::Vertical,
                    100_000,
                    vec![ViewVirtualItem::new(ViewStableKey(1), 120_000)],
                )
                .unwrap(),
        );
        let save = session.export_session_save_bytes().unwrap();
        let mut cold = session_from_bytes(&bytes);
        cold.import_session_save_bytes(&save, &Default::default())
            .unwrap();
        assert_eq!(cold.export_session_save_bytes().unwrap(), save);
        let before = session.snapshot_session().unwrap();
        let mut forged = session.snapshot_session().unwrap();
        let node = forged
            .presentation
            .view
            .mounts
            .iter_mut()
            .flat_map(|mount| &mut mount.style_nodes)
            .find(|node| matches!(node.kind, BundleViewStyleNodeKind::Scroll { .. }))
            .unwrap();
        let BundleViewStyleNodeKind::Scroll { policy, .. } = &mut node.kind else {
            unreachable!()
        };
        policy.axis = ViewScrollAxis::Horizontal;
        assert!(matches!(
            session.restore_session_snapshot(forged),
            Err(BundleSessionSaveError::ViewRuntime { .. })
        ));
        assert_eq!(session.snapshot_session().unwrap(), before);

        let mut forged = session.snapshot_session().unwrap();
        let mount = &mut forged.presentation.view.mounts[0];
        let first_text = mount
            .style_nodes
            .iter_mut()
            .find_map(|node| match &mut node.kind {
                BundleViewStyleNodeKind::Text { output } => Some(output),
                _ => None,
            })
            .unwrap();
        *first_text = BundleViewTextOutputIndex::new(1);
        assert!(matches!(
            session.restore_session_snapshot(forged),
            Err(BundleSessionSaveError::ViewRuntime { .. })
        ));
        assert_eq!(session.snapshot_session().unwrap(), before);

        let mut forged = session.snapshot_session().unwrap();
        let mount = &mut forged.presentation.view.mounts[0];
        let text_slots = mount
            .paint
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                matches!(item, BundleViewPaintItem::Text { .. }).then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(text_slots.len(), 2);
        mount.paint.swap(text_slots[0], text_slots[1]);
        assert!(matches!(
            session.restore_session_snapshot(forged),
            Err(BundleSessionSaveError::ViewRuntime { .. })
        ));
        assert_eq!(session.snapshot_session().unwrap(), before);

        let mut forged = session.snapshot_session().unwrap();
        let mount = &mut forged.presentation.view.mounts[0];
        let first_text = mount
            .style_nodes
            .iter()
            .find(|node| matches!(node.kind, BundleViewStyleNodeKind::Text { .. }))
            .unwrap();
        mount.action_buttons[0].node.instruction = first_text.instruction;
        assert!(matches!(
            session.restore_session_snapshot(forged),
            Err(BundleSessionSaveError::ViewRuntime { .. })
        ));
        assert_eq!(session.snapshot_session().unwrap(), before);
    });
    assert!(virtual_mount.is_some() && retired);
}

#[test]
fn source_view_inputs_and_defaults_reach_the_retained_mount_after_each_restore() {
    let mut session = session(
        r#"
view Card(caption: String, callback: String -> String effects {}, suffix: String = "default") {
    Text(caption)
    Text(suffix)
}
flow main() -> i64 {
    let capture = "captured-only-in-callback";
    let handle = mount(Card(caption = "supplied", callback = |text: String| capture));
    wait(10ms)
    handle.unmount();
    return 42i64
}
entry cli @entry.main { goto @flow.main }
"#,
    );
    let mut text_seen = false;
    let mut capture_seen = false;
    let mut empty = false;
    run(&mut session, |session| {
        for mount in &session.presentation().view.mounts {
            let values = mount
                .text
                .iter()
                .map(|text| match &text.value {
                    BundleViewTextValue::Plain { value } => value.as_str(),
                    other => panic!("expected exact Source strings, got {other:?}"),
                })
                .collect::<Vec<_>>();
            assert_eq!(values, ["supplied", "default"]);
            text_seen = true;
        }
        let saved = session.snapshot_session().unwrap();
        for binding in saved
            .view_runtime
            .mounts
            .iter()
            .flat_map(|mount| &mount.runtime_parameters)
        {
            arcweft_core::value::RuntimeBorrowedValue::from(&binding.value)
                .try_visit(
                    arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
                    |value| {
                        if value.scalar()
                            == Some(RuntimeScalarView::String("captured-only-in-callback"))
                        {
                            capture_seen = true;
                        }
                        Ok::<(), String>(())
                    },
                )
                .unwrap();
        }
        empty |= text_seen && session.presentation().view.mounts.is_empty();
    });
    assert!(text_seen && capture_seen && empty);
}

#[test]
fn source_background_replacement_and_old_handle_release_keep_the_current_occupant() {
    let mut session = session(
        r#"
flow main() -> i64 {
    let first = bg(@asset.bg.room);
    wait(10ms)
    let second = bg(@asset.bg.pulse);
    wait(10ms)
    first.release();
    wait(10ms)
    second.release();
    return 42i64
}
entry cli @entry.main { goto @flow.main }
"#,
    );
    let (mut room, mut pulse, mut empty) = (false, false, false);
    let mut pulse_steps = 0;
    run(&mut session, |session| {
        let images = &session.presentation().images;
        assert!(images.len() <= 1);
        if let Some(image) = images.first() {
            assert_eq!(image.fit, ImageObjectFit::Cover);
            match image.asset.as_str() {
                "asset.bg.room" => {
                    assert!(!pulse);
                    room = true;
                }
                "asset.bg.pulse" => {
                    assert!(room);
                    pulse = true;
                    pulse_steps += 1;
                }
                other => panic!("unexpected slot occupant {other}"),
            }
        }
        empty |= pulse && images.is_empty();
    });
    assert!(room && pulse && pulse_steps >= 20 && empty);
}

#[test]
fn source_viewport_handles_project_latest_live_selection_and_release() {
    let mut session = session(
        r#"
flow main() -> i64 {
    let first = player_viewport(width = 1920px, height = 1080px, fit = "cover");
    wait(10ms)
    let second = player_viewport(fit = "raw");
    wait(10ms)
    first.release();
    wait(10ms)
    second.release();
    return 42i64
}
entry cli @entry.main { goto @flow.main }
"#,
    );
    let (mut design, mut raw, mut empty) = (false, false, false);
    run(&mut session, |session| {
        if let Some(fit) = session.presentation().viewport_fit {
            if fit.design_width == 1920 {
                assert_eq!(fit.design_height, 1080);
                assert_eq!(fit.scale_policy, arcweft_layout::ScalePolicy::Cover);
                design = true;
            } else {
                assert_eq!((fit.design_width, fit.design_height), (0, 0));
                assert_eq!(fit.scale_policy, arcweft_layout::ScalePolicy::Raw);
                raw = true;
            }
        } else {
            empty |= raw;
        }
    });
    assert!(design && raw && empty);
}
