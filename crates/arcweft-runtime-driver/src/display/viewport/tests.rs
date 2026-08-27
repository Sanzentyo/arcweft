use super::*;
use crate::display::{BundlePresentationResources, BundlePresentationSnapshot, DisplayResolution};
use arcweft_core::engine::FlowFiberStatus;

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

#[test]
fn viewport_effect_sets_and_clears_runtime_fit() {
    let contain = RuntimeCall {
        callee: "player_viewport".to_owned(),
        args: vec![
            "width = 1920".to_owned(),
            "height = 1080px".to_owned(),
            "fit = \"cover\"".to_owned(),
        ],
    };
    let reset = RuntimeCall {
        callee: "player_viewport".to_owned(),
        args: vec!["fit = \"default\"".to_owned()],
    };

    let fit = viewport_fit_from_effects(None, &[LineEffectRequest::Call(contain)])
        .expect("viewport command is valid")
        .expect("viewport fit is set");
    assert_eq!(fit.design_width, 1920);
    assert_eq!(fit.design_height, 1080);
    assert_eq!(fit.scale_policy, ScalePolicy::Cover);

    assert_eq!(
        viewport_fit_from_effects(Some(fit), &[LineEffectRequest::Call(reset)])
            .expect("viewport clear is valid"),
        None
    );
}

#[test]
fn canonical_viewport_command_mutates_direct_runtime_state() {
    let mut snapshot = BundlePresentationSnapshot::default();
    let diagnostics = snapshot
        .update(
            &DisplayResolution::default(),
            &FlowFiberStatus::Running,
            &[LineEffectRequest::Call(RuntimeCall {
                callee: "player_viewport".to_owned(),
                args: vec![
                    "width = 1920".to_owned(),
                    "height = 1080".to_owned(),
                    "fit = \"stretch\"".to_owned(),
                ],
            })],
            empty_presentation_resources(),
        )
        .expect("viewport update is valid");

    assert!(diagnostics.is_empty());
    assert_eq!(
        snapshot.viewport_fit,
        Some(BundleViewportFit::design(1920, 1080, ScalePolicy::Stretch))
    );
    assert_eq!(snapshot.revision, 1);
}

#[test]
fn malformed_viewport_arguments_are_rejected_atomically() {
    let malformed = [
        (vec!["width = 0".to_owned()], "width"),
        (vec!["width = 1e30".to_owned()], "width"),
        (vec!["fit = \"bogus\"".to_owned()], "fit"),
        (
            vec!["fit = \"raw\"".to_owned(), "width = 1280".to_owned()],
            "width",
        ),
    ];
    for (args, argument) in malformed {
        let mut snapshot = BundlePresentationSnapshot {
            revision: 43,
            viewport_fit: Some(BundleViewportFit::raw()),
            ..BundlePresentationSnapshot::default()
        };
        let before = snapshot.clone();
        let error = snapshot
            .update(
                &DisplayResolution::default(),
                &FlowFiberStatus::Running,
                &[LineEffectRequest::Call(RuntimeCall {
                    callee: "player_viewport".to_owned(),
                    args,
                })],
                empty_presentation_resources(),
            )
            .expect_err("malformed viewport command is rejected");

        assert!(matches!(
            error,
            BundlePresentationUpdateError::InvalidCommandArgument {
                callee: "player_viewport",
                argument: actual,
                ..
            } if actual == argument
        ));
        assert_eq!(snapshot, before);
    }
}

#[test]
fn viewport_command_without_a_known_shape_is_rejected_atomically() {
    let mut snapshot = BundlePresentationSnapshot {
        revision: 43,
        viewport_fit: Some(BundleViewportFit::raw()),
        ..BundlePresentationSnapshot::default()
    };
    let before = snapshot.clone();
    let error = snapshot
        .update(
            &DisplayResolution::default(),
            &FlowFiberStatus::Running,
            &[LineEffectRequest::Call(RuntimeCall {
                callee: "player_viewport".to_owned(),
                args: vec!["mystery = true".to_owned()],
            })],
            empty_presentation_resources(),
        )
        .expect_err("viewport command without fit or dimensions is rejected");

    assert_eq!(
        error,
        BundlePresentationUpdateError::MissingCommandArgument {
            callee: "player_viewport",
            argument: "fit or width/height",
        }
    );
    assert_eq!(snapshot, before);
}
