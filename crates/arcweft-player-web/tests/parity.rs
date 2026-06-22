use arcweft_bundle::ArcweftBundle;
use arcweft_player_web::images::BrowserImageCatalog;
use arcweft_player_web::report::{WebFrameBounds, WebFrameObservationReport, WebFrameViewport};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderChoiceItem, RenderDialogue, RenderPreferences,
    RenderScene, RenderViewport, SharedFramePlanner,
};
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{BundleSession, BundleSessionOptions, BundleStepInput};

#[test]
fn native_headless_demo_frame_matches_browser_frame_observation_contract() {
    let report = demo_frame_report();

    assert_eq!(report.schema_version, "arcweft.web_frame_observation.v1");
    assert_eq!(
        report.viewport,
        WebFrameViewport {
            logical_width_milli: 1_280_000,
            logical_height_milli: 720_000,
            physical_width: 1280,
            physical_height: 720,
            scale_factor_milli: 1_000,
        }
    );
    assert_eq!(report.image_count, 4);
    assert_eq!(report.text_count, 4);
    assert_eq!(report.choice_count, 2);
    assert_eq!(
        report
            .images
            .iter()
            .map(|image| image.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "image.generated.background",
            "image.generated.character_stand",
            "image.generated.gif_pulse",
            "image.generated.webp_pulse",
        ]
    );
    assert_eq!(
        report.images[0].bounds,
        WebFrameBounds {
            x_milli: 0,
            y_milli: 0,
            width_milli: 1_280_000,
            height_milli: 720_000,
        }
    );
    assert_eq!(
        report.images[1].bounds,
        WebFrameBounds {
            x_milli: 64_000,
            y_milli: 52_000,
            width_milli: 180_000,
            height_milli: 300_000,
        }
    );
    assert_eq!(
        report
            .choices
            .iter()
            .map(|choice| (
                choice.option_id.as_str(),
                choice.label.as_str(),
                choice.bounds
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "choice.web_demo.continue",
                "Continue",
                WebFrameBounds {
                    x_milli: 230_400,
                    y_milli: 248_640,
                    width_milli: 819_200,
                    height_milli: 58_000,
                },
            ),
            (
                "choice.web_demo.alternate",
                "Alternate route",
                WebFrameBounds {
                    x_milli: 230_400,
                    y_milli: 318_640,
                    width_milli: 819_200,
                    height_milli: 58_000,
                },
            ),
        ]
    );
    assert!(
        report.text.iter().any(|text| text.text
            == "Arcweft is running on a shared wgpu renderer in this WebGPU canvas.")
    );
}

fn demo_frame_report() -> WebFrameObservationReport {
    let bundle =
        ArcweftBundle::from_json_slice(include_bytes!("../../../web/demo.awfb")).expect("bundle");
    let mut session =
        BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session");
    let images = BrowserImageCatalog::from_bundle(&bundle).expect("images");

    let mut last_step = None;
    for tick in 1..=16 {
        let dt_millis = u32::try_from(tick * 16).expect("test clock fits u32");
        let clock = RuntimeClockStep::from_millis(tick, dt_millis).expect("clock");
        let step = session.step_with_clock(clock, BundleStepInput::default());
        let ready = step.presentation.choices.len() == 2 && step.presentation.images.len() == 4;
        last_step = Some(step);
        if ready {
            break;
        }
    }
    let presentation = &last_step.expect("step").presentation;
    let viewport = RenderViewport {
        logical_width: 1280.0,
        logical_height: 720.0,
        physical_width: 1280,
        physical_height: 720,
        scale_factor: 1.0,
    };
    let scene = RenderScene {
        dialogue: presentation
            .dialogue
            .as_ref()
            .map(|dialogue| RenderDialogue {
                speaker: dialogue.callee.clone(),
                text: dialogue.text.clone(),
            }),
        choices: presentation
            .choices
            .iter()
            .map(|choice| RenderChoiceItem {
                id: choice.id.clone(),
                label: choice.label.clone(),
            })
            .collect(),
        images: images
            .render_images(&presentation.images, 32)
            .expect("render images"),
        viewport,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
    };
    let mut prepared = SharedFramePlanner::prepare(&scene).expect("frame");
    let focused = prepared.first_choice_target();
    prepared = SharedFramePlanner::prepare(&RenderScene {
        interaction: InteractionVisualState {
            focused,
            hovered: None,
            pressed: None,
        },
        ..scene
    })
    .expect("focused frame");
    WebFrameObservationReport::from_prepared_frame(&prepared)
}
