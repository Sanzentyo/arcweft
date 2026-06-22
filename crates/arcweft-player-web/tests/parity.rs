use arcweft_bundle::ArcweftBundle;
use arcweft_player_web::parity::{WebGpuParityFrameOptions, prepare_bundle_parity_frame};
use arcweft_player_web::report::{WebFrameBounds, WebFrameObservationReport, WebFrameViewport};

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
    let prepared = prepare_bundle_parity_frame(&bundle, WebGpuParityFrameOptions::default())
        .expect("parity frame");
    WebFrameObservationReport::from_prepared_frame(&prepared)
}
