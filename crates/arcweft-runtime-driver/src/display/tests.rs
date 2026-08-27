use super::*;
use crate::presentation_handles::{PresentationHandleDiagnosticCode, PresentationResourceState};
use arcweft_bundle::resource_codec::view::{
    CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization,
    ViewFocusDirection, ViewFocusGroupPolicy, ViewFocusInitialPolicy, ViewFocusSkipPolicy,
    ViewFocusTargetResolution, ViewFocusWrapPolicy, ViewInputKind, ViewInputPurpose,
    ViewRuntimeFocusGroup, ViewRuntimeFocusNavigation, ViewRuntimeFocusNavigationEdge,
    ViewSecureInputPolicy, ViewTextSelectionPolicy, ViewTextShortcutPolicy, ViewTextTabPolicy,
    ViewTextVerticalNavigationPolicy,
};
use arcweft_bundle::resource_codec::{
    ViewRuntimeActionButtonAction, ViewRuntimeButtonBounds, ViewRuntimeControlVisualStyle,
    ViewRuntimeScrollRegionBounds, ViewRuntimeTextControlBounds, ViewRuntimeTextControlHandlers,
    ViewRuntimeTextControlOptions, ViewRuntimeTextSelection,
};
use arcweft_core::engine::ChoiceState;
use arcweft_core::plan::ChoiceRuntimeOption;
use arcweft_layout::stage_placement::StagePlacementContext;
use arcweft_layout::{LayoutCoordinateSpace, LayoutRect, LayoutSize};

#[test]
fn inline_image_call_accepts_runtime_length_labels() {
    let call = RuntimeCall {
        callee: "image".to_owned(),
        args: vec![
            "asset = @asset:.zundamon.normal".to_owned(),
            "id = \"image.zundamon.stand\"".to_owned(),
            "target = @target.zundamon.stand".to_owned(),
            "layer = @layer.character".to_owned(),
            "x = 760".to_owned(),
            "y = 24".to_owned(),
            "width = 360".to_owned(),
            "height = 600".to_owned(),
            "fit = \"contain\"".to_owned(),
            "alignment.x = \"right\"".to_owned(),
            "alignment.y = \"bottom\"".to_owned(),
            "playback.start = 250ms".to_owned(),
            "playback.paused_at = 500ms".to_owned(),
            "playback.local_time = 750ms".to_owned(),
        ],
    };

    assert_eq!(
        named_arg(&call.args, "asset").and_then(public_id_arg),
        Some("asset.zundamon.normal".to_owned())
    );
    assert_eq!(
        named_arg(&call.args, "id").and_then(public_id_arg),
        Some("image.zundamon.stand".to_owned())
    );
    assert_eq!(
        named_arg(&call.args, "x").and_then(parse_px_milli),
        Some(760_000)
    );
    let object = inline_image_object(&call).expect("inline image object");

    assert_eq!(object.id, "image.zundamon.stand");
    assert_eq!(object.asset, "asset.zundamon.normal");
    assert_eq!(object.target.as_deref(), Some("target.zundamon.stand"));
    assert_eq!(object.layer.as_deref(), Some("layer.character"));
    assert_eq!(object.bounds.x_milli, 760_000);
    assert_eq!(object.bounds.height_milli, 600_000);
    assert_eq!(object.alignment.x_milli, 1_000);
    assert_eq!(object.alignment.y_milli, 1_000);
    assert_eq!(object.playback.start_time_millis, 250);
    assert_eq!(object.playback.paused_at_millis, Some(500));
    assert_eq!(object.playback.pinned_local_time_millis, Some(750));
}

#[test]
fn choices_preserve_a_missing_id_without_synthesizing_from_the_label() {
    let status = FlowFiberStatus::Choice(ChoiceState {
        id: None,
        options: vec![ChoiceRuntimeOption {
            id: None,
            label: "Listen".to_owned(),
            target: None,
            out: None,
            effects: Vec::new(),
        }],
        resume: None,
    });

    assert_eq!(
        choices_from_status(&status),
        vec![BundleChoice {
            id: None,
            label: "Listen".to_owned(),
        }]
    );
}

#[test]
fn canonical_image_command_mutates_direct_runtime_state() {
    let image = presentation_image_object("image.glass_bg");
    let resources = image_runtime_resources(&image);
    let mut snapshot = BundlePresentationSnapshot::default();
    let diagnostics = update_snapshot_with_effects(
        &mut snapshot,
        &[LineEffectRequest::Call(RuntimeCall {
            callee: "image".to_owned(),
            args: vec!["@image.glass_bg".to_owned()],
        })],
        resources,
    );
    assert!(diagnostics.is_empty());
    assert_eq!(snapshot.images, vec![image]);
    assert_eq!(snapshot.revision, 1);
}

#[test]
fn canonical_default_background_uses_stage_geometry_and_typed_options() {
    let image = presentation_image_object("image.glass_bg");
    let resources = image_runtime_resources(&image);
    let mut snapshot = BundlePresentationSnapshot::default();
    let diagnostics = update_snapshot_with_effects(
        &mut snapshot,
        &[LineEffectRequest::Call(RuntimeCall {
            callee: "bg".to_owned(),
            args: vec![
                "@asset.glass_bg".to_owned(),
                "opacity = 0.5".to_owned(),
                "alignment.x = 0.25".to_owned(),
                "alignment.y = 0.75".to_owned(),
                "playback.local_time = 150ms".to_owned(),
            ],
        })],
        resources,
    );
    assert!(diagnostics.is_empty());
    assert_eq!(snapshot.images.len(), 1);
    let background = &snapshot.images[0];
    assert_eq!(background.id, "image.background.default");
    assert_eq!(background.asset, "asset.glass_bg");
    assert_eq!(background.target.as_deref(), Some("target.scene"));
    assert_eq!(background.layer.as_deref(), Some("layer.background"));
    assert_eq!(
        background.bounds,
        BundleImageObjectBounds::from_px(0, 0, 1280, 720)
    );
    assert_eq!(
        background.placement,
        Some(StagePlacement::anchor(
            StageAnchor::TopLeft,
            StageAnchor::TopLeft,
            StageSize::new(1_280_000, 720_000),
        ))
    );
    let resolved = background
        .placement
        .expect("background has authored design placement")
        .resolve(StagePlacementContext::new(
            LayoutSize::new(1280.0, 720.0),
            LayoutSize::new(960.0, 540.0),
        ))
        .expect("background placement resolves");
    assert_eq!(resolved.authored_space, LayoutCoordinateSpace::Design);
    assert_eq!(
        resolved.output_bbox,
        LayoutRect::from_xywh(0.0, 0.0, 960.0, 540.0)
    );
    assert_eq!(background.fit, BundleImageObjectFit::Cover);
    assert_eq!(background.alignment.x_milli, 250);
    assert_eq!(background.alignment.y_milli, 750);
    assert_eq!(background.opacity_milli, 500);
    assert_eq!(background.playback.pinned_local_time_millis, Some(150));
    assert_ne!(background.id, image.id);
    assert_eq!(snapshot.revision, 1);
}

#[test]
fn canonical_background_pairs_replace_and_clear_exact_slots() {
    let image = presentation_image_object("image.glass_bg");
    let resources = image_runtime_resources(&image);
    let mut snapshot = BundlePresentationSnapshot::default();
    let diagnostics = update_snapshot_with_effects(
        &mut snapshot,
        &[
            LineEffectRequest::Call(RuntimeCall {
                callee: "bg".to_owned(),
                args: vec![
                    "@asset.city_far".to_owned(),
                    "target = @target.scene".to_owned(),
                    "slot = @slot.background.far".to_owned(),
                ],
            }),
            LineEffectRequest::Call(RuntimeCall {
                callee: "bg".to_owned(),
                args: vec![
                    "@asset.city_near".to_owned(),
                    "slot = @slot.background.near".to_owned(),
                ],
            }),
            LineEffectRequest::Call(RuntimeCall {
                callee: "bg".to_owned(),
                args: vec![
                    "@asset.reflection".to_owned(),
                    "target = @target.scene.reflection".to_owned(),
                    "slot = @slot.background.default".to_owned(),
                ],
            }),
            LineEffectRequest::Call(RuntimeCall {
                callee: "bg".to_owned(),
                args: vec![
                    "@asset.city_far.updated".to_owned(),
                    "target = @target.scene".to_owned(),
                    "slot = @slot.background.far".to_owned(),
                ],
            }),
            LineEffectRequest::Call(RuntimeCall {
                callee: "bg".to_owned(),
                args: vec![
                    "@asset.collision_probe".to_owned(),
                    "target = @target.scene".to_owned(),
                    "slot = @slot.background.default.target.scene.reflection".to_owned(),
                ],
            }),
        ],
        resources,
    );
    assert!(diagnostics.is_empty());
    assert_eq!(snapshot.images.len(), 4);
    let far = snapshot
        .images
        .iter()
        .find(|object| object.id == "image.background.pair.s14.background.far.t5.scene")
        .expect("far background slot");
    assert_eq!(far.asset, "asset.city_far.updated");
    assert_eq!(far.target.as_deref(), Some("target.scene"));
    let near = snapshot
        .images
        .iter()
        .find(|object| object.id == "image.background.pair.s15.background.near.t5.scene")
        .expect("near background slot");
    assert_eq!(near.asset, "asset.city_near");
    let reflection = snapshot
        .images
        .iter()
        .find(|object| {
            object.id == "image.background.pair.s18.background.default.t16.scene.reflection"
        })
        .expect("background slot on an explicit target");
    assert_eq!(reflection.asset, "asset.reflection");
    assert_eq!(
        reflection.target.as_deref(),
        Some("target.scene.reflection")
    );
    let collision_probe = snapshot
        .images
        .iter()
        .find(|object| object.asset == "asset.collision_probe")
        .expect("distinct target/slot pair remains distinct");
    assert_ne!(collision_probe.id, reflection.id);

    let diagnostics = update_snapshot_with_effects(
        &mut snapshot,
        &[LineEffectRequest::Call(RuntimeCall {
            callee: "bg.clear".to_owned(),
            args: vec![
                "target = @target.scene".to_owned(),
                "slot = @slot.background.far".to_owned(),
            ],
        })],
        resources,
    );
    assert!(diagnostics.is_empty());
    assert_eq!(snapshot.images.len(), 3);
    assert!(
        snapshot
            .images
            .iter()
            .all(|object| object.asset != "asset.city_far.updated")
    );
}

#[test]
fn malformed_background_arguments_fail_atomically() {
    let mut snapshot = BundlePresentationSnapshot::default();
    update_snapshot_with_effects(
        &mut snapshot,
        &[LineEffectRequest::Call(RuntimeCall {
            callee: "bg".to_owned(),
            args: vec!["@asset.initial".to_owned()],
        })],
        empty_presentation_resources(),
    );
    let before = snapshot.clone();

    let missing_asset = snapshot
        .update(
            &DisplayResolution::default(),
            &FlowFiberStatus::Running,
            &[LineEffectRequest::Call(RuntimeCall {
                callee: "bg".to_owned(),
                args: vec!["fit = \"contain\"".to_owned()],
            })],
            empty_presentation_resources(),
        )
        .expect_err("background asset is required");
    assert_eq!(
        missing_asset,
        BundlePresentationUpdateError::MissingCommandArgument {
            callee: "bg",
            argument: "asset",
        }
    );
    assert_eq!(snapshot, before);

    let malformed = [
        (
            vec![
                "@asset.replacement".to_owned(),
                "fit = \"bogus\"".to_owned(),
            ],
            "fit",
        ),
        (
            vec![
                "@asset.replacement".to_owned(),
                "alignment.x = 1.5".to_owned(),
            ],
            "alignment.x",
        ),
        (
            vec![
                "@asset.replacement".to_owned(),
                "target = @layer.not_a_target".to_owned(),
            ],
            "target",
        ),
        (
            vec![
                "@asset.replacement".to_owned(),
                "opacity = -0.25".to_owned(),
            ],
            "opacity",
        ),
        (
            vec![
                "@asset.replacement".to_owned(),
                "playback.start = -1ms".to_owned(),
            ],
            "playback.start",
        ),
    ];

    for (args, argument) in malformed {
        let error = snapshot
            .update(
                &DisplayResolution::default(),
                &FlowFiberStatus::Running,
                &[LineEffectRequest::Call(RuntimeCall {
                    callee: "bg".to_owned(),
                    args,
                })],
                empty_presentation_resources(),
            )
            .expect_err("malformed background is rejected");
        assert!(matches!(
            error,
            BundlePresentationUpdateError::InvalidCommandArgument {
                callee: "bg",
                argument: actual,
                ..
            } if actual == argument
        ));
        assert_eq!(snapshot, before);
    }
}

#[test]
fn malformed_background_clear_preserves_the_existing_slot() {
    let mut snapshot = BundlePresentationSnapshot::default();
    update_snapshot_with_effects(
        &mut snapshot,
        &[LineEffectRequest::Call(RuntimeCall {
            callee: "bg".to_owned(),
            args: vec!["@asset.initial".to_owned()],
        })],
        empty_presentation_resources(),
    );
    let before = snapshot.clone();

    let error = snapshot
        .update(
            &DisplayResolution::default(),
            &FlowFiberStatus::Running,
            &[LineEffectRequest::Call(RuntimeCall {
                callee: "bg.clear".to_owned(),
                args: vec!["slot = @slot.character.alice".to_owned()],
            })],
            empty_presentation_resources(),
        )
        .expect_err("malformed background clear is rejected");

    assert!(matches!(
        error,
        BundlePresentationUpdateError::InvalidCommandArgument {
            callee: "bg.clear",
            argument: "slot",
            ..
        }
    ));
    assert_eq!(snapshot, before);
}

#[test]
fn malformed_inline_image_arguments_are_rejected_atomically() {
    let image = presentation_image_object("image.glass_bg.inline");
    let resources = image_runtime_resources(&image);
    let malformed = [
        ("width", "width = 0"),
        ("fit", "fit = \"bogus\""),
        ("alignment.x", "alignment.x = 1.5"),
        ("playback.start", "playback.start = -1ms"),
        ("transform.m11", "transform.m11 = nan"),
        ("opacity", "opacity = -0.25"),
        ("visible", "visible = maybe"),
        ("depth", "depth = nan"),
    ];
    for (argument, replacement) in malformed {
        let mut call = inline_image_runtime_call();
        if let Some(existing) = call
            .args
            .iter_mut()
            .find(|candidate| candidate.starts_with(&format!("{argument} =")))
        {
            *existing = replacement.to_owned();
        } else {
            call.args.push(replacement.to_owned());
        }
        let mut snapshot = BundlePresentationSnapshot {
            revision: 47,
            images: vec![image.clone()],
            ..BundlePresentationSnapshot::default()
        };
        let before = snapshot.clone();
        let error = snapshot
            .update(
                &DisplayResolution::default(),
                &FlowFiberStatus::Running,
                &[LineEffectRequest::Call(call)],
                resources,
            )
            .expect_err("malformed inline image is rejected");

        assert!(matches!(
            error,
            BundlePresentationUpdateError::InvalidCommandArgument {
                callee: "image",
                argument: actual,
                ..
            } if actual == argument
        ));
        assert_eq!(snapshot, before);
    }

    let mut missing_id = inline_image_runtime_call();
    missing_id
        .args
        .retain(|argument| !argument.starts_with("id ="));
    let mut snapshot = BundlePresentationSnapshot::default();
    let error = snapshot
        .update(
            &DisplayResolution::default(),
            &FlowFiberStatus::Running,
            &[LineEffectRequest::Call(missing_id)],
            resources,
        )
        .expect_err("runtime driver requires a source-site-stable inline image id");
    assert_eq!(
        error,
        BundlePresentationUpdateError::MissingCommandArgument {
            callee: "image",
            argument: "id",
        }
    );
}

#[test]
fn unknown_presentation_commands_do_not_mutate_direct_runtime_state() {
    let image = presentation_image_object("image.glass_bg");
    let resources = image_runtime_resources(&image);
    let call = RuntimeCall {
        callee: "mystery.presentation".to_owned(),
        args: vec!["@image.glass_bg".to_owned()],
    };
    let mut snapshot = BundlePresentationSnapshot {
        revision: 41,
        ..BundlePresentationSnapshot::default()
    };
    let before = snapshot.clone();
    let diagnostics =
        update_snapshot_with_effects(&mut snapshot, &[LineEffectRequest::Call(call)], resources);

    assert!(diagnostics.is_empty());
    assert_eq!(snapshot, before);
}

#[test]
fn unknown_image_argument_does_not_mutate_direct_runtime_state() {
    let canonical_call = inline_image_runtime_call();
    let image = inline_image_object(&canonical_call).expect("canonical inline image");
    let resources = image_runtime_resources(&image);
    let mut call = canonical_call.clone();
    call.args.push("mystery = true".to_owned());
    let mut snapshot = BundlePresentationSnapshot {
        revision: 47,
        images: vec![image.clone()],
        ..BundlePresentationSnapshot::default()
    };
    let before = snapshot.clone();
    let diagnostics =
        update_snapshot_with_effects(&mut snapshot, &[LineEffectRequest::Call(call)], resources);

    assert!(diagnostics.is_empty());
    assert_eq!(snapshot, before);
}

#[test]
fn presentation_snapshot_serializes_handle_epoch_and_tombstones() {
    let mut snapshot = BundlePresentationSnapshot::default();
    let resources = BundlePresentationResources {
        image_objects: &[],
        text_inputs: &[],
        action_buttons: &[],
        scroll_regions: &[],
        surfaces: &[],
        focus_groups: &[],
        focus_navigation: &[],
    };
    let create = LineEffectRequest::Call(RuntimeCall {
        callee: "presentation.handle.create".to_owned(),
        args: vec![
            "handle = @handle.flow.save.panel".to_owned(),
            "kind = \"view\"".to_owned(),
            "resource = @view.SavePanel".to_owned(),
            "owner = @flow.save".to_owned(),
        ],
    });
    let dispose = LineEffectRequest::Call(RuntimeCall {
        callee: "presentation.handle.dispose".to_owned(),
        args: vec!["handle = @handle.flow.save.panel".to_owned()],
    });
    let diagnostics = snapshot
        .update(
            &DisplayResolution::default(),
            &FlowFiberStatus::Running,
            &[create, dispose],
            resources,
        )
        .expect("dialogue presentation store updates");

    assert!(diagnostics.is_empty());
    assert_eq!(snapshot.presentation_handle_epoch, 2);
    assert_eq!(snapshot.presentation_handles.len(), 1);
    assert_eq!(
        snapshot.presentation_handles[0].state,
        PresentationResourceState::Released
    );
    assert_eq!(snapshot.presentation_handles[0].created_epoch, 1);
    assert_eq!(snapshot.presentation_handles[0].updated_epoch, 2);

    let encoded = serde_json::to_string(&snapshot).expect("snapshot serializes");
    let mut restored: BundlePresentationSnapshot =
        serde_json::from_str(&encoded).expect("snapshot deserializes");
    assert_eq!(restored, snapshot);

    let stale_show = LineEffectRequest::Call(RuntimeCall {
        callee: "presentation.handle.show".to_owned(),
        args: vec!["handle = @handle.flow.save.panel".to_owned()],
    });
    let diagnostics = restored
        .update(
            &DisplayResolution::default(),
            &FlowFiberStatus::Running,
            &[stale_show],
            resources,
        )
        .expect("dialogue presentation store updates");

    assert_eq!(
        diagnostics[0].code,
        PresentationHandleDiagnosticCode::TerminalHandle
    );
    assert_eq!(restored.presentation_handle_epoch, 3);
    assert_eq!(
        restored.presentation_handles[0].state,
        PresentationResourceState::Released
    );
    assert_eq!(restored.presentation_handles[0].updated_epoch, 2);
}

#[test]
fn view_handle_lifecycle_filters_runtime_controls() {
    let mut snapshot = BundlePresentationSnapshot::default();
    let text_input = view_text_input();
    let action_button = view_action_button();
    let resources = view_runtime_resources(&text_input, &action_button);
    let create_visible = view_handle_create("@handle.flow.feedback.panel");

    let diagnostics = update_snapshot_with_effects(&mut snapshot, &[create_visible], resources);

    assert!(diagnostics.is_empty());
    assert_view_controls_visible(&snapshot, &text_input, &action_button);

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.hide",
            "@handle.flow.feedback.panel",
        )],
        resources,
    );
    assert!(snapshot.text_inputs.is_empty());
    assert!(snapshot.action_buttons.is_empty());

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.show",
            "@handle.flow.feedback.panel",
        )],
        resources,
    );
    assert_view_controls_visible(&snapshot, &text_input, &action_button);

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.unmount",
            "@handle.flow.feedback.panel",
        )],
        resources,
    );
    assert!(snapshot.text_inputs.is_empty());
    assert!(snapshot.action_buttons.is_empty());

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.show",
            "@handle.flow.feedback.panel",
        )],
        resources,
    );
    assert_view_controls_visible(&snapshot, &text_input, &action_button);

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.release",
            "@handle.flow.feedback.panel",
        )],
        resources,
    );
    assert!(snapshot.text_inputs.is_empty());
    assert!(snapshot.action_buttons.is_empty());

    let mut destroy_snapshot = BundlePresentationSnapshot::default();
    update_snapshot_with_effects(
        &mut destroy_snapshot,
        &[
            view_handle_create("@handle.flow.feedback.panel.destroy"),
            presentation_handle_call(
                "presentation.handle.destroy",
                "@handle.flow.feedback.panel.destroy",
            ),
        ],
        resources,
    );
    assert!(destroy_snapshot.text_inputs.is_empty());
    assert!(destroy_snapshot.action_buttons.is_empty());
}

#[test]
fn view_handle_lifecycle_filters_scroll_regions() {
    let mut snapshot = BundlePresentationSnapshot::default();
    let scroll_region = view_scroll_region();
    let resources = BundlePresentationResources {
        image_objects: &[],
        text_inputs: &[],
        action_buttons: &[],
        scroll_regions: std::slice::from_ref(&scroll_region),
        surfaces: &[],
        focus_groups: &[],
        focus_navigation: &[],
    };
    let create_visible = view_handle_create("@handle.flow.feedback.panel");

    let diagnostics = update_snapshot_with_effects(&mut snapshot, &[create_visible], resources);

    assert!(diagnostics.is_empty());
    assert_eq!(snapshot.scroll_regions, vec![scroll_region.clone()]);

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.hide",
            "@handle.flow.feedback.panel",
        )],
        resources,
    );
    assert!(snapshot.scroll_regions.is_empty());

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.show",
            "@handle.flow.feedback.panel",
        )],
        resources,
    );
    assert_eq!(snapshot.scroll_regions, vec![scroll_region]);
}

#[test]
fn view_handle_lifecycle_filters_surfaces() {
    let mut snapshot = BundlePresentationSnapshot::default();
    let surface = view_surface();
    let resources = BundlePresentationResources {
        image_objects: &[],
        text_inputs: &[],
        action_buttons: &[],
        scroll_regions: &[],
        surfaces: std::slice::from_ref(&surface),
        focus_groups: &[],
        focus_navigation: &[],
    };
    let create_visible = view_handle_create("@handle.flow.feedback.panel");

    let diagnostics = update_snapshot_with_effects(&mut snapshot, &[create_visible], resources);

    assert!(diagnostics.is_empty());
    assert_eq!(snapshot.surfaces, vec![surface.clone()]);

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.hide",
            "@handle.flow.feedback.panel",
        )],
        resources,
    );
    assert!(snapshot.surfaces.is_empty());

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.show",
            "@handle.flow.feedback.panel",
        )],
        resources,
    );
    assert_eq!(snapshot.surfaces, vec![surface]);
}

#[test]
fn view_handle_lifecycle_filters_focus_resources() {
    let mut snapshot = BundlePresentationSnapshot::default();
    let focus_group = view_focus_group();
    let focus_navigation = view_focus_navigation();
    let resources = BundlePresentationResources {
        image_objects: &[],
        text_inputs: &[],
        action_buttons: &[],
        scroll_regions: &[],
        surfaces: &[],
        focus_groups: std::slice::from_ref(&focus_group),
        focus_navigation: std::slice::from_ref(&focus_navigation),
    };
    let create_visible = view_handle_create("@handle.flow.feedback.panel");

    let diagnostics = update_snapshot_with_effects(&mut snapshot, &[create_visible], resources);

    assert!(diagnostics.is_empty());
    assert_eq!(snapshot.focus_groups, vec![focus_group.clone()]);
    assert_eq!(snapshot.focus_navigation, vec![focus_navigation.clone()]);

    let diagnostics = update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.hide",
            "@handle.flow.feedback.panel",
        )],
        resources,
    );
    assert!(snapshot.focus_groups.is_empty());
    assert!(snapshot.focus_navigation.is_empty());
    assert_eq!(
        diagnostics[0].code,
        PresentationHandleDiagnosticCode::HiddenButFocusable
    );

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.show",
            "@handle.flow.feedback.panel",
        )],
        resources,
    );
    assert_eq!(snapshot.focus_groups, vec![focus_group.clone()]);
    assert_eq!(snapshot.focus_navigation, vec![focus_navigation.clone()]);

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.unmount",
            "@handle.flow.feedback.panel",
        )],
        resources,
    );
    assert!(snapshot.focus_groups.is_empty());
    assert!(snapshot.focus_navigation.is_empty());

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.release",
            "@handle.flow.feedback.panel",
        )],
        resources,
    );
    assert!(snapshot.focus_groups.is_empty());
    assert!(snapshot.focus_navigation.is_empty());

    let mut destroy_snapshot = BundlePresentationSnapshot::default();
    update_snapshot_with_effects(
        &mut destroy_snapshot,
        &[
            view_handle_create("@handle.flow.feedback.panel.destroy"),
            presentation_handle_call(
                "presentation.handle.destroy",
                "@handle.flow.feedback.panel.destroy",
            ),
        ],
        resources,
    );
    assert!(destroy_snapshot.focus_groups.is_empty());
    assert!(destroy_snapshot.focus_navigation.is_empty());
}

#[test]
fn image_handle_lifecycle_filters_presentation_images() {
    let mut snapshot = BundlePresentationSnapshot::default();
    let image = presentation_image_object("image.glass_bg");
    let resources = image_runtime_resources(&image);
    let create_visible = image_handle_create("@handle.flow.feedback.bg", "image.glass_bg");

    let diagnostics = update_snapshot_with_effects(&mut snapshot, &[create_visible], resources);

    assert!(diagnostics.is_empty());
    assert_eq!(snapshot.images, vec![image.clone()]);

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.hide",
            "@handle.flow.feedback.bg",
        )],
        resources,
    );
    assert!(snapshot.images.is_empty());

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.show",
            "@handle.flow.feedback.bg",
        )],
        resources,
    );
    assert_eq!(snapshot.images, vec![image.clone()]);

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.unmount",
            "@handle.flow.feedback.bg",
        )],
        resources,
    );
    assert!(snapshot.images.is_empty());

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.show",
            "@handle.flow.feedback.bg",
        )],
        resources,
    );
    assert_eq!(snapshot.images, vec![image.clone()]);

    update_snapshot_with_effects(
        &mut snapshot,
        &[presentation_handle_call(
            "presentation.handle.release",
            "@handle.flow.feedback.bg",
        )],
        resources,
    );
    assert!(snapshot.images.is_empty());

    let mut destroy_snapshot = BundlePresentationSnapshot::default();
    update_snapshot_with_effects(
        &mut destroy_snapshot,
        &[
            image_handle_create("@handle.flow.feedback.bg.destroy", "image.glass_bg"),
            presentation_handle_call(
                "presentation.handle.destroy",
                "@handle.flow.feedback.bg.destroy",
            ),
        ],
        resources,
    );
    assert!(destroy_snapshot.images.is_empty());
}

fn view_text_input() -> ViewRuntimeTextControl {
    ViewRuntimeTextControl {
        public_id: "input.visitor_name".to_owned(),
        target: "input.visitor_name".to_owned(),
        view: Some("view.ModernFeedbackPanel".to_owned()),
        containing_scroll_region: None,
        session: 1,
        value: String::new(),
        selection: ViewRuntimeTextSelection::new(0, 0),
        options: ViewRuntimeTextControlOptions {
            purpose: ViewInputPurpose::Text,
            autocorrect: TextAssistPolicy::PlatformDefault,
            spellcheck: TextAssistPolicy::PlatformDefault,
            capitalization: TextCapitalization::None,
            enter_key: EnterKeyHint::Default,
            multiline: false,
            selection_policy: ViewTextSelectionPolicy::Enabled,
            shortcut_policy: ViewTextShortcutPolicy::Enabled,
            tab_policy: ViewTextTabPolicy::FocusNavigation,
            vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
            secure_policy: ViewSecureInputPolicy::Plain,
            composition_on_blur: CompositionOnBlurPolicy::Commit,
        },
        kind: ViewInputKind::TextField,
        bounds: ViewRuntimeTextControlBounds::from_px(48, 48, 420, 48),
        label: None,
        handlers: ViewRuntimeTextControlHandlers::default(),
        style: ViewRuntimeControlVisualStyle::default(),
    }
}

fn view_action_button() -> ViewRuntimeActionButton {
    ViewRuntimeActionButton {
        public_id: "button.continue".to_owned(),
        target: "button.continue".to_owned(),
        view: Some("view.ModernFeedbackPanel".to_owned()),
        containing_scroll_region: None,
        label: "Continue".to_owned(),
        enabled: true,
        bounds: ViewRuntimeButtonBounds::new(484_000, 48_000, 180_000, 48_000),
        action: ViewRuntimeActionButtonAction::Noop,
        style: ViewRuntimeControlVisualStyle::default(),
    }
}

fn view_scroll_region() -> ViewRuntimeScrollRegion {
    ViewRuntimeScrollRegion {
        public_id: "scroll.ModernFeedbackPanel.0".to_owned(),
        target: "scroll.ModernFeedbackPanel.0".to_owned(),
        view: Some("view.ModernFeedbackPanel".to_owned()),
        bounds: ViewRuntimeScrollRegionBounds::new(48_000, 48_000, 420_000, 180_000),
        content_width_milli: 420_000,
        content_height_milli: 360_000,
        axis: arcweft_bundle::resource_codec::ViewScrollAxis::Vertical,
        overflow: arcweft_bundle::resource_codec::ViewScrollOverflowPolicy::Auto,
        indicators: arcweft_bundle::resource_codec::ViewScrollIndicatorsPolicy::Auto,
        overscroll: arcweft_bundle::resource_codec::ViewScrollOverscrollPolicy::Clamp,
        auto_scroll_focus: arcweft_bundle::resource_codec::ViewFocusAutoScrollPolicy::Nearest,
    }
}

fn view_surface() -> ViewRuntimeSurface {
    ViewRuntimeSurface {
        public_id: "surface.ModernFeedbackPanel.card".to_owned(),
        target: "surface.ModernFeedbackPanel.card".to_owned(),
        view: Some("view.ModernFeedbackPanel".to_owned()),
        containing_scroll_region: None,
        element: arcweft_bundle::resource_codec::view::ViewElementKind::Panel,
        bounds: arcweft_bundle::resource_codec::ViewRuntimeSurfaceBounds::from_px(8, 12, 96, 48),
        style: ViewRuntimeControlVisualStyle::default(),
    }
}

fn view_focus_group() -> ViewRuntimeFocusGroup {
    ViewRuntimeFocusGroup {
        public_id: "group.ModernFeedbackPanel.0".to_owned(),
        view: Some("view.ModernFeedbackPanel".to_owned()),
        parent: None,
        policy: ViewFocusGroupPolicy::Normal,
        initial: ViewFocusInitialPolicy::Explicit {
            target: "button.continue".to_owned(),
        },
        wrap: ViewFocusWrapPolicy::Wrap,
        disabled_skip: ViewFocusSkipPolicy::Skip,
        hidden_skip: ViewFocusSkipPolicy::Skip,
    }
}

fn view_focus_navigation() -> ViewRuntimeFocusNavigation {
    ViewRuntimeFocusNavigation {
        public_id: "button.continue".to_owned(),
        view: Some("view.ModernFeedbackPanel".to_owned()),
        group: Some("group.ModernFeedbackPanel.0".to_owned()),
        edges: vec![ViewRuntimeFocusNavigationEdge {
            direction: ViewFocusDirection::Left,
            target: ViewFocusTargetResolution::Explicit {
                target: "input.visitor_name".to_owned(),
            },
        }],
    }
}

fn view_runtime_resources<'a>(
    text_input: &'a ViewRuntimeTextControl,
    action_button: &'a ViewRuntimeActionButton,
) -> BundlePresentationResources<'a> {
    BundlePresentationResources {
        image_objects: &[],
        text_inputs: std::slice::from_ref(text_input),
        action_buttons: std::slice::from_ref(action_button),
        scroll_regions: &[],
        surfaces: &[],
        focus_groups: &[],
        focus_navigation: &[],
    }
}

fn view_handle_create(handle: &str) -> LineEffectRequest {
    LineEffectRequest::Call(RuntimeCall {
        callee: "presentation.handle.create".to_owned(),
        args: vec![
            format!("handle = {handle}"),
            "kind = \"view\"".to_owned(),
            "resource = @view:.ModernFeedbackPanel".to_owned(),
        ],
    })
}

fn presentation_handle_call(callee: &str, handle: &str) -> LineEffectRequest {
    LineEffectRequest::Call(RuntimeCall {
        callee: callee.to_owned(),
        args: vec![format!("handle = {handle}")],
    })
}

fn inline_image_runtime_call() -> RuntimeCall {
    RuntimeCall {
        callee: "image".to_owned(),
        args: vec![
            "asset = @asset:.glass_bg".to_owned(),
            "id = \"image.glass_bg.inline\"".to_owned(),
            "x = 0".to_owned(),
            "y = 0".to_owned(),
            "width = 1280".to_owned(),
            "height = 720".to_owned(),
        ],
    }
}

fn presentation_image_object(id: &str) -> BundleImageObject {
    BundleImageObject {
        id: id.to_owned(),
        asset: "asset.glass_bg".to_owned(),
        target: Some("target.glass_bg".to_owned()),
        layer: Some("layer.background".to_owned()),
        view: None,
        containing_scroll_region: None,
        bounds: BundleImageObjectBounds::from_px(0, 0, 1280, 720),
        placement: None,
        fit: BundleImageObjectFit::Cover,
        alignment: BundleImageObjectAlignment::default(),
        playback: BundleImageObjectPlayback::default(),
        transform: BundleImageObjectTransform::default(),
        depth_milli: -10_000,
        opacity_milli: 1_000,
        actions: Vec::new(),
        params: std::collections::BTreeMap::default(),
        proxies: Vec::new(),
        visible: true,
    }
}

fn image_runtime_resources(image: &BundleImageObject) -> BundlePresentationResources<'_> {
    BundlePresentationResources {
        image_objects: std::slice::from_ref(image),
        text_inputs: &[],
        action_buttons: &[],
        scroll_regions: &[],
        surfaces: &[],
        focus_groups: &[],
        focus_navigation: &[],
    }
}

fn empty_presentation_resources() -> BundlePresentationResources<'static> {
    BundlePresentationResources {
        image_objects: &[],
        text_inputs: &[],
        action_buttons: &[],
        scroll_regions: &[],
        surfaces: &[],
        focus_groups: &[],
        focus_navigation: &[],
    }
}

fn image_handle_create(handle: &str, resource: &str) -> LineEffectRequest {
    LineEffectRequest::Call(RuntimeCall {
        callee: "presentation.handle.create".to_owned(),
        args: vec![
            format!("handle = {handle}"),
            "kind = \"image\"".to_owned(),
            format!("resource = @{resource}"),
        ],
    })
}

fn update_snapshot_with_effects(
    snapshot: &mut BundlePresentationSnapshot,
    effects: &[LineEffectRequest],
    resources: BundlePresentationResources<'_>,
) -> Vec<PresentationHandleDiagnostic> {
    snapshot
        .update(
            &DisplayResolution::default(),
            &FlowFiberStatus::Running,
            effects,
            resources,
        )
        .expect("dialogue presentation store updates")
}

fn assert_view_controls_visible(
    snapshot: &BundlePresentationSnapshot,
    text_input: &ViewRuntimeTextControl,
    action_button: &ViewRuntimeActionButton,
) {
    assert_eq!(snapshot.text_inputs, vec![text_input.clone()]);
    assert_eq!(snapshot.action_buttons, vec![action_button.clone()]);
}
