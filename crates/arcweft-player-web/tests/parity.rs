use arcweft_bundle::resource_codec::view::{
    CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization,
    ViewActionButtonActionResource, ViewActionButtonResource, ViewActionPayloadResource,
    ViewInputKind, ViewInputOptions, ViewInputPurpose, ViewInputResource, ViewLayoutBoundsResource,
    ViewLogicalRect, ViewProgramResource, ViewScrollAxis, ViewScrollRegionResource,
    ViewSecureInputPolicy, ViewTextResource, ViewTextSelectionPolicy, ViewTextShortcutPolicy,
    ViewTextSourceKind, ViewTextSourceRecord, ViewTextTabPolicy, ViewTextVerticalNavigationPolicy,
};
use arcweft_bundle::{
    ArcweftBundle, BundleFormat, BundleImageAnimation, BundleImageAsset, BundleImageDimensions,
    BundleImageFormat, BundleImageObject, BundleImageObjectAlignment, BundleImageObjectBounds,
    BundleImageObjectFit, BundleImageObjectPlayback, BundleImageObjectTransform, BundleManifest,
    BundleRuntimeSummary, BundleSource, BundleVirtualFile, BundleVirtualFileSpace,
};
use arcweft_core::{bytecode::BytecodeProgram, plan::RuntimePlan};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_syntax::parser::parse_source;
use arcweft_player_scene::{
    frame::{PlayerFrameFit, PlayerFramePlanner, PlayerFrameRequest},
    images::BundleImageCatalog,
    input::{InputController, InputPointerModifiers},
};
use arcweft_player_web::parity::{WebGpuParityFrameOptions, prepare_bundle_parity_frame};
use arcweft_player_web::report::{WebFrameBounds, WebFrameObservationReport, WebFrameViewport};
use arcweft_presentation::{
    hit::HitRect,
    image::{ImageObjectAlignment, ImageObjectFit, ImageObjectTransform},
    input::{InteractionTarget, PointerId, ViewportPoint},
    text_input::{TextInput, TextInputSerial},
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderFocusAutoScrollPolicy, RenderImage,
    RenderImageFrame, RenderPreferences, RenderScene, RenderScrollAxis, RenderScrollOverflow,
    RenderScrollRegion, RenderViewport, SharedFramePlanner,
};
use arcweft_render_wgpu::renderer::StyledParagraphEvidenceFontContext;
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{BundleSession, BundleSessionOptions, BundleStepInput};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_runtime_plan::flow::lower_runtime_plan;
use std::collections::BTreeMap;

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
    // The generated character is 180x300. Contain-fit preserves that 3:5
    // aspect ratio inside the authored 208x332 box and centers the 199.2px
    // visible width horizontally.
    assert_eq!(
        report.images[1].bounds,
        WebFrameBounds {
            x_milli: 76_400,
            y_milli: 52_000,
            width_milli: 199_200,
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
    assert!(report.text.iter().any(|text| text.text == "ずんだガイド"));
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

#[test]
fn web_frame_report_drops_released_image_handle_resources() {
    let live = image_frame_report(vec![test_render_image("image.glass_bg")]);

    assert_eq!(live.image_count, 1);
    assert_eq!(
        live.images
            .iter()
            .map(|image| image.id.as_str())
            .collect::<Vec<_>>(),
        vec!["image.glass_bg"]
    );
    assert_eq!(
        live.images[0].bounds,
        WebFrameBounds {
            x_milli: 24_000,
            y_milli: 32_000,
            width_milli: 128_000,
            height_milli: 96_000,
        }
    );

    let released = image_frame_report(Vec::new());
    assert_eq!(released.image_count, 0);
    assert!(released.images.is_empty());
}

#[test]
fn web_frame_report_uses_visible_bounds_for_scroll_clipped_images() {
    let mut image = test_render_image("image.scroll.card");
    image.bounds = HitRect::new(100.0, 170.0, 200.0, 80.0);
    image.containing_scroll_region = Some("scroll.gallery".to_owned());
    let scene = RenderScene {
        dialogue: None,
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: vec![image],
        viewport: RenderViewport {
            logical_width: 800.0,
            logical_height: 450.0,
            physical_width: 800,
            physical_height: 450,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: vec![RenderScrollRegion {
            id: "scroll.gallery".to_owned(),
            bounds: HitRect::new(100.0, 100.0, 160.0, 80.0),
            content_width: 240.0,
            content_height: 260.0,
            offset_x: 0.0,
            offset_y: 60.0,
            axis: RenderScrollAxis::Vertical,
            overflow: RenderScrollOverflow::Auto,
            auto_scroll_focus: RenderFocusAutoScrollPolicy::Nearest,
        }],
    };
    let prepared = SharedFramePlanner::prepare(&scene).expect("frame prepares");
    let report = WebFrameObservationReport::from_prepared_frame(&prepared, &[])
        .expect("image frame reports");

    assert_eq!(report.image_count, 1);
    assert_eq!(
        report.images[0].bounds,
        WebFrameBounds {
            x_milli: 100_000,
            y_milli: 110_000,
            width_milli: 160_000,
            height_milli: 70_000,
        }
    );
}

#[test]
fn web_runner_drops_authored_released_and_scoped_disposed_image_handles() {
    let bundle = authored_image_handle_bundle();

    let live = authored_image_flow_report(&bundle, "manual_live");
    assert_eq!(live.image_count, 1);
    assert_eq!(
        live.images
            .iter()
            .map(|image| image.id.as_str())
            .collect::<Vec<_>>(),
        vec!["image.card"]
    );
    assert_eq!(
        live.images[0].bounds,
        WebFrameBounds {
            x_milli: 24_000,
            y_milli: 32_000,
            width_milli: 128_000,
            height_milli: 96_000,
        }
    );

    let released = authored_image_flow_report(&bundle, "manual_released");
    assert_eq!(released.image_count, 0);
    assert!(released.images.is_empty());

    let destroyed = authored_image_flow_report(&bundle, "manual_destroyed");
    assert_eq!(destroyed.image_count, 0);
    assert!(destroyed.images.is_empty());

    let scoped_disposed = authored_image_flow_report(&bundle, "scoped_disposed");
    assert_eq!(scoped_disposed.image_count, 0);
    assert!(scoped_disposed.images.is_empty());
}

#[test]
fn web_runner_filters_authored_view_owned_controls_and_scroll_regions() {
    let bundle = authored_view_control_bundle();

    let mut live_input = InputController::default();
    let live = authored_view_flow_player_frame(&bundle, "view_manual_live", &mut live_input);
    assert_eq!(live.scene.text_inputs.len(), 1);
    assert_eq!(live.scene.action_buttons.len(), 1);
    assert_eq!(live.frame.scroll_regions.len(), 1);
    assert_eq!(live.frame.scroll_regions[0].id, "scroll.panel");

    let text_target = live.scene.text_inputs[0].target.clone();
    let button_target = live.scene.action_buttons[0].target.clone();
    assert!(live.frame.hits.find_target(&text_target).is_some());
    assert!(live.frame.hits.find_target(&button_target).is_some());
    assert!(live.frame.keyboard_focus_targets().contains(&text_target));
    assert!(live.frame.keyboard_focus_targets().contains(&button_target));

    let button_bounds = live
        .frame
        .hits
        .find_target(&button_target)
        .expect("live button is hittable")
        .bounds();
    let button_point = ViewportPoint::new(
        button_bounds.x + button_bounds.width * 0.5,
        button_bounds.y + button_bounds.height * 0.5,
    );

    live_input
        .activate_text_control(&live.scene.text_inputs[0])
        .expect("live text control activates");
    live_input.pointer_down(
        &live.frame,
        PointerId(0),
        button_point,
        InputPointerModifiers::NONE,
    );

    let hidden = authored_view_flow_player_frame(&bundle, "view_manual_released", &mut live_input);
    assert_view_controls_absent(&hidden, &text_target, &button_target);

    let stale_text = live_input
        .text_input(
            &hidden.frame,
            TextInput::committed(
                live.scene.text_inputs[0].session,
                TextInputSerial(9),
                " ignored",
            ),
        )
        .expect("stale text input is ignored");
    assert!(stale_text.text_control_write_backs().is_empty());

    let stale_click = live_input.pointer_up(
        &hidden.frame,
        PointerId(0),
        button_point,
        InputPointerModifiers::NONE,
    );
    assert!(stale_click.actions().is_empty());
    assert!(stale_click.text_control_write_backs().is_empty());

    let mut unmounted_input = InputController::default();
    let unmounted =
        authored_view_flow_player_frame(&bundle, "view_manual_unmounted", &mut unmounted_input);
    assert_view_controls_absent(&unmounted, &text_target, &button_target);

    let mut destroyed_input = InputController::default();
    let destroyed =
        authored_view_flow_player_frame(&bundle, "view_manual_destroyed", &mut destroyed_input);
    assert_view_controls_absent(&destroyed, &text_target, &button_target);

    let mut scoped_input = InputController::default();
    let scoped_disposed =
        authored_view_flow_player_frame(&bundle, "view_scoped_disposed", &mut scoped_input);
    assert!(scoped_disposed.scene.text_inputs.is_empty());
    assert!(scoped_disposed.scene.action_buttons.is_empty());
    assert!(scoped_disposed.frame.scroll_regions.is_empty());
}

fn assert_view_controls_absent(
    prepared: &arcweft_player_scene::frame::PlayerPreparedFrame,
    text_target: &InteractionTarget,
    button_target: &InteractionTarget,
) {
    assert!(prepared.scene.text_inputs.is_empty());
    assert!(prepared.scene.action_buttons.is_empty());
    assert!(prepared.frame.scroll_regions.is_empty());
    assert!(prepared.frame.hits.find_target(text_target).is_none());
    assert!(prepared.frame.hits.find_target(button_target).is_none());
    assert!(
        !prepared
            .frame
            .keyboard_focus_targets()
            .iter()
            .any(|target| target == text_target)
    );
    assert!(
        !prepared
            .frame
            .keyboard_focus_targets()
            .iter()
            .any(|target| target == button_target)
    );
    assert!(prepared.frame.focused_text_input_target().is_none());
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
                "ずんだガイド" | "このまま進む" | "別ルートを見る"
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

fn image_frame_report(images: Vec<RenderImage>) -> WebFrameObservationReport {
    let prepared = SharedFramePlanner::prepare(&RenderScene {
        dialogue: None,
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images,
        viewport: RenderViewport {
            logical_width: 800.0,
            logical_height: 450.0,
            physical_width: 800,
            physical_height: 450,
            scale_factor: 1.0,
        },
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    })
    .expect("image frame prepares");
    WebFrameObservationReport::from_prepared_frame(&prepared, &[]).expect("image frame reports")
}

fn authored_image_flow_report(bundle: &ArcweftBundle, flow: &str) -> WebFrameObservationReport {
    let prepared = authored_image_flow_frame(bundle, flow);
    WebFrameObservationReport::from_prepared_frame(&prepared, &[])
        .expect("authored image flow reports")
}

fn authored_image_flow_frame(
    bundle: &ArcweftBundle,
    flow: &str,
) -> arcweft_render_wgpu::geometry::PreparedFrame {
    let presentation = authored_flow_presentation(bundle, flow);
    let images = BundleImageCatalog::from_bundle(bundle).expect("image catalog decodes");
    let viewport = parity_test_viewport();
    let scene = RenderScene {
        dialogue: None,
        choices: Vec::new(),
        text_inputs: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        images: images
            .render_images(&presentation.images, 0, viewport)
            .expect("presentation images render"),
        viewport,
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
        scroll_regions: Vec::new(),
    };
    SharedFramePlanner::prepare(&scene).expect("authored image flow prepares")
}

fn authored_view_flow_player_frame(
    bundle: &ArcweftBundle,
    flow: &str,
    input: &mut InputController,
) -> arcweft_player_scene::frame::PlayerPreparedFrame {
    let presentation = authored_flow_presentation(bundle, flow);
    let images = BundleImageCatalog::empty();
    PlayerFramePlanner::prepare(
        input,
        PlayerFrameRequest {
            presentation: &presentation,
            images: &images,
            viewport: parity_test_viewport(),
            fit: PlayerFrameFit::raw(),
            image_time_millis: 0,
            visual_time_millis: 0,
            preferences: RenderPreferences::default(),
        },
    )
    .expect("authored view flow prepares")
}

fn authored_flow_presentation(
    bundle: &ArcweftBundle,
    flow: &str,
) -> arcweft_runtime_driver::display::BundlePresentationSnapshot {
    let mut session = BundleSession::new(
        bundle,
        BundleSessionOptions {
            flow: Some(flow.to_owned()),
            ..BundleSessionOptions::default()
        },
    )
    .expect("web runner session starts selected flow");
    let mut presentation = None;
    for tick in 1..=4 {
        let step = session.step_with_clock(
            RuntimeClockStep::from_millis(tick, 16).expect("clock step"),
            BundleStepInput::default(),
        );
        assert!(
            step.diagnostics.is_empty(),
            "unexpected runner diagnostics for flow {flow}: {:?}",
            step.diagnostics
        );
        presentation = Some(step.presentation);
        if step.finished {
            break;
        }
    }
    presentation.expect("runner produced a presentation snapshot")
}

const fn parity_test_viewport() -> RenderViewport {
    RenderViewport {
        logical_width: 800.0,
        logical_height: 450.0,
        physical_width: 800,
        physical_height: 450,
        scale_factor: 1.0,
    }
}

fn authored_image_handle_bundle() -> ArcweftBundle {
    const SOURCE: &str = r#"
pub asset card_file {
  kind = image
  file = "bg/room.png"
}

pub image card {
  asset = @asset:.card_file
  target = @target.card
  x = 24px
  y = 32px
  width = 128px
  height = 96px
  fit = stretch
  visible = true
}

flow manual_live {
  let sprite = image(@image.card, lifetime = .manual)
  return "mounted"
}

flow manual_released {
  let sprite = image(@image.card, lifetime = .manual)
  sprite.release()
  return "released"
}

flow manual_destroyed {
  let sprite = image(@image.card, lifetime = .manual)
  sprite.destroy()
  return "destroyed"
}

flow scoped_disposed {
  let sprite = image(@image.card)
  return "disposed"
}
"#;
    let parsed = parse_source(SOURCE);
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_to_hir(parsed.typed_tree()).expect("authored fixture lowers to HIR");
    let plan = lower_runtime_plan(&hir).expect("authored fixture lowers to runtime plan");
    bundle_from_runtime_plan(&plan, SOURCE, "web-authored-image-handle.arcw")
        .with_virtual_files([authored_image_virtual_file()])
        .with_image_assets([authored_image_asset()])
        .with_image_objects([authored_image_object()])
}

fn authored_view_control_bundle() -> ArcweftBundle {
    const SOURCE: &str = r#"
view WebPanel() {
  Panel {
    Text("Web")
  }
}

flow view_manual_live {
  let panel = view(@view.WebPanel, lifetime = .manual)
  return "mounted"
}

flow view_manual_released {
  let panel = view(@view.WebPanel, lifetime = .manual)
  panel.release()
  return "released"
}

flow view_manual_unmounted {
  let panel = view(@view.WebPanel, lifetime = .manual)
  panel.unmount()
  return "unmounted"
}

flow view_manual_destroyed {
  let panel = view(@view.WebPanel, lifetime = .manual)
  panel.destroy()
  return "destroyed"
}

flow view_scoped_disposed {
  let panel = view(@view.WebPanel)
  return "disposed"
}
"#;
    let parsed = parse_source(SOURCE);
    assert_eq!(parsed.errors(), &[]);
    let hir = lower_to_hir(parsed.typed_tree()).expect("authored view fixture lowers to HIR");
    let plan = lower_runtime_plan(&hir).expect("authored view fixture lowers to runtime plan");
    bundle_from_runtime_plan(&plan, SOURCE, "web-authored-view-controls.arcw")
        .with_view_text(authored_view_text_resource())
        .with_view_program(authored_view_program_resource())
        .with_view_input(authored_view_input_resource())
}

fn bundle_from_runtime_plan(plan: &RuntimePlan, source: &str, source_label: &str) -> ArcweftBundle {
    let bytecode = BytecodeProgram::from_runtime_plan(plan.clone());
    let stats = bytecode.stats();
    let display = arcweft_render_text::LineDisplayCatalog::default();
    let product_awbc = AwbcLowerer::new(plan, &display, source_label)
        .lower()
        .expect("authored fixture lowers to product AWBC")
        .program;
    ArcweftBundle::new(
        BundleManifest {
            source_label: source_label.to_owned(),
            profile_id: None,
            profile_kind: None,
            entry: None,
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                entry_flow: None,
                flows: stats.flows,
                bytecode_instructions: stats.instructions,
                line_task_groups: stats.line_task_groups,
                stream_plans: stats.stream_plans,
                source_plans: stats.source_plans,
            },
        },
        BundleSource {
            label: source_label.to_owned(),
            text: source.to_owned(),
        },
        bytecode,
        display,
    )
    .with_product_awbc(product_awbc)
}

fn authored_view_text_resource() -> ViewTextResource {
    ViewTextResource {
        sources: vec![
            literal_text_source("text.input.name.value", "Ada"),
            literal_text_source("text.input.name.label", "Name"),
            literal_text_source("text.button.send", "Send"),
        ],
        display_frame_refs: Vec::new(),
        source_ranges: Vec::new(),
        reveal_policies: Vec::new(),
        cursor_policies: Vec::new(),
        redactions: Vec::new(),
    }
}

fn literal_text_source(public_id: &str, value: &str) -> ViewTextSourceRecord {
    ViewTextSourceRecord {
        public_id: public_id.to_owned(),
        kind: ViewTextSourceKind::Literal {
            value: value.to_owned(),
        },
        source: None,
    }
}

fn authored_view_program_resource() -> ViewProgramResource {
    ViewProgramResource {
        program_id: "view.web_panel".to_owned(),
        root_view: "view.WebPanel".to_owned(),
        instructions: Vec::new(),
        child_spans: Vec::new(),
        handlers: Vec::new(),
        state_schema_hashes: Vec::new(),
        exported_parts: Vec::new(),
        semantic_targets: Vec::new(),
        layout_bounds: vec![ViewLayoutBoundsResource::text_control(
            "input.name",
            ViewLogicalRect::from_px(64, 64, 240, 44),
        )],
        scroll_regions: vec![ViewScrollRegionResource::new(
            "scroll.panel",
            Some("view.WebPanel".to_owned()),
            ViewLogicalRect::from_px(48, 48, 360, 140),
            360_000,
            420_000,
            ViewScrollAxis::Vertical,
        )],
        surfaces: Vec::new(),
        text_blocks: Vec::new(),
        action_buttons: vec![ViewActionButtonResource {
            public_id: "button.send".to_owned(),
            view: Some("view.WebPanel".to_owned()),
            containing_scroll_region: Some("scroll.panel".to_owned()),
            label_text_source: "text.button.send".to_owned(),
            enabled: true,
            action: ViewActionButtonActionResource::ActionInvoke {
                action: "action.feedback.submit".to_owned(),
                payload: Some(ViewActionPayloadResource::LiteralString {
                    value: "ready".to_owned(),
                }),
            },
            bounds: ViewLogicalRect::from_px(64, 124, 160, 44).runtime_button_bounds(),
            style: None,
            source: None,
        }],
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        adapter_requirements: Vec::new(),
    }
}

fn authored_view_input_resource() -> ViewInputResource {
    ViewInputResource {
        options: vec![ViewInputOptions {
            public_id: "input.name".to_owned(),
            view: Some("view.WebPanel".to_owned()),
            containing_scroll_region: Some("scroll.panel".to_owned()),
            kind: ViewInputKind::TextField,
            value_text_source: "text.input.name.value".to_owned(),
            placeholder_text_source: None,
            purpose: ViewInputPurpose::Name,
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
            submit_handler: None,
            change_handler: None,
            adapter_requirements: Vec::new(),
        }],
        adapter_requirements: Vec::new(),
    }
}

fn authored_image_virtual_file() -> BundleVirtualFile {
    BundleVirtualFile {
        space: BundleVirtualFileSpace::Asset,
        path: "bg/room.png".to_owned(),
        bytes: include_bytes!("../../../samples/assets/bg/room.png").to_vec(),
    }
}

fn authored_image_asset() -> BundleImageAsset {
    BundleImageAsset {
        id: "asset.card_file".to_owned(),
        file: authored_image_virtual_file().file_ref(),
        format: BundleImageFormat::Png,
        animation: BundleImageAnimation::Static,
        dimensions: Some(BundleImageDimensions::new(2, 2)),
    }
}

fn authored_image_object() -> BundleImageObject {
    BundleImageObject {
        id: "image.card".to_owned(),
        asset: "asset.card_file".to_owned(),
        target: Some("target.card".to_owned()),
        layer: None,
        view: None,
        containing_scroll_region: None,
        bounds: BundleImageObjectBounds::from_px(24, 32, 128, 96),
        placement: None,
        fit: BundleImageObjectFit::Stretch,
        alignment: BundleImageObjectAlignment::default(),
        playback: BundleImageObjectPlayback::default(),
        transform: BundleImageObjectTransform::default(),
        depth_milli: 0,
        opacity_milli: 1_000,
        actions: Vec::new(),
        params: BTreeMap::new(),
        proxies: Vec::new(),
        visible: true,
    }
}

fn test_render_image(id: &str) -> RenderImage {
    RenderImage {
        id: id.to_owned(),
        frame: RenderImageFrame {
            index: None,
            width: 2,
            height: 2,
            rgba: vec![
                0x20, 0x30, 0x40, 0xff, 0x50, 0x60, 0x70, 0xff, 0x80, 0x90, 0xa0, 0xff, 0xb0, 0xc0,
                0xd0, 0xff,
            ],
        },
        bounds: HitRect::new(24.0, 32.0, 128.0, 96.0),
        containing_scroll_region: None,
        viewport_clip: None,
        placement: None,
        fit: ImageObjectFit::Stretch,
        alignment: ImageObjectAlignment::default(),
        transform: ImageObjectTransform::identity(),
        opacity_milli: 1_000,
    }
}
