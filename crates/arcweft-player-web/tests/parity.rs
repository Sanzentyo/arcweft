use arcweft_bundle::{ArcweftBundle, BundleFormat};
use arcweft_player_web::parity::{WebGpuParityFrameOptions, prepare_bundle_parity_frame};
use arcweft_player_web::report::{WebFrameBounds, WebFrameObservationReport, WebFrameViewport};
use arcweft_render_wgpu::renderer::StyledParagraphEvidenceFontContext;

#[test]
fn native_headless_demo_frame_matches_browser_frame_observation_contract() {
    let report = demo_frame_report();
    let complete_report = demo_frame_report_at(2_500);

    assert_eq!(report.schema_version, "arcweft.web_frame_observation.v3");
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
    assert_eq!(dialogue_text(&report), "こちらは");
    assert_eq!(
        dialogue_text(&complete_report),
        "こちらはキャラクターsurfaceの色とフォントを使う行なのだ。波打つ文字と、右上のアニメーション画像も同じフレーム計画で動いているのだ。"
    );
    assert_eq!(complete_report.text_count, 4);
    assert_eq!(complete_report.styled_paragraph_count, 1);
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
            x_milli: 72_000,
            y_milli: 52_000,
            width_milli: 208_000,
            height_milli: 332_000,
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
                "このまま進む",
                WebFrameBounds {
                    x_milli: 307_200,
                    y_milli: 306_800,
                    width_milli: 665_600,
                    height_milli: 60_000,
                },
            ),
            (
                "choice.web_demo.alternate",
                "別ルートを見る",
                WebFrameBounds {
                    x_milli: 307_200,
                    y_milli: 378_800,
                    width_milli: 665_600,
                    height_milli: 60_000,
                },
            ),
        ]
    );
    assert!(report.text.iter().any(|text| text.text == "zunda_guide"));
}

#[test]
fn web_frame_report_serializes_styled_paragraph_evidence() {
    let report = demo_frame_report_at(2_500);
    let paragraph = report
        .styled_paragraphs
        .iter()
        .find(|paragraph| paragraph.text.contains("波打つ文字"))
        .expect("styled paragraph");

    assert!(!paragraph.line_boxes.is_empty());
    assert!(!paragraph.glyph_bounds.is_empty());
    assert!(
        paragraph
            .glyph_bounds
            .iter()
            .all(|glyph| glyph.style.rgba.len() == 4 && glyph.source_start < glyph.source_end)
    );
    serde_json::to_string(&report).expect("serialize v3 report");
}

fn demo_frame_report() -> WebFrameObservationReport {
    demo_frame_report_at(WebGpuParityFrameOptions::default().visual_time_millis)
}

fn demo_frame_report_at(visual_time_millis: u64) -> WebFrameObservationReport {
    let bundle = ArcweftBundle::from_format_slice(
        BundleFormat::Awfb,
        include_bytes!("../../../web/demo.awfb"),
    )
    .expect("bundle");
    let prepared = prepare_bundle_parity_frame(
        &bundle,
        WebGpuParityFrameOptions {
            visual_time_millis,
            ..WebGpuParityFrameOptions::default()
        },
    )
    .expect("parity frame");
    let mut evidence = StyledParagraphEvidenceFontContext::new();
    evidence
        .register_font_bytes(include_bytes!("../../../web/assets/arcweft-demo.ttf").to_vec())
        .expect("font");
    let paragraph_evidence = evidence.frame_styled_paragraph_layout_evidence(&prepared);
    WebFrameObservationReport::from_prepared_frame(&prepared, &paragraph_evidence)
        .expect("frame report")
}

fn dialogue_text(report: &WebFrameObservationReport) -> String {
    report
        .text
        .iter()
        .filter(|text| {
            !matches!(
                text.text.as_str(),
                "zunda_guide" | "このまま進む" | "別ルートを見る"
            )
        })
        .map(|text| text.text.as_str())
        .chain(report.styled_paragraphs.iter().map(|paragraph| {
            paragraph
                .text
                .get(..paragraph.visible_end.min(paragraph.text.len()))
                .unwrap_or("")
        }))
        .collect()
}
