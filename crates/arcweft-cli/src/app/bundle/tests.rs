use super::*;
use arcweft_bundle::{
    BundleImageObjectBounds,
    container::{BundleView, ReadBudget},
    patch::{BundlePatchArtifact, encode_patch_bundle},
};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_core::plan::{FlowRuntimeId, RuntimeFlow};
use arcweft_core::task::{
    AwaitTarget, HostTaskArgTemplate, HostTaskRequestTemplate, NeedId, TaskId,
};
use arcweft_layout::stage_placement::{StagePlacement, StageRect};
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

fn image_await(id: &str) -> FlowOp {
    FlowOp::Await {
        binding: None,
        target: AwaitTarget::new(
            NeedId(format!("need.{id}")),
            TaskId(format!("task.{id}")),
            HostTaskRequestTemplate::new(
                "asset",
                "image",
                [HostTaskArgTemplate::positional(RuntimeExpr::EntityRef(
                    id.to_owned(),
                ))],
            ),
        ),
        pending: Vec::new(),
    }
}

fn image_effect_call(callee: &str, arg: &str) -> FlowOp {
    FlowOp::Effect(LineEffectRequest::Call(RuntimeCall {
        callee: callee.to_owned(),
        args: vec![arg.to_owned()],
    }))
}

fn plan_with_ops(ops: Vec<FlowOp>) -> RuntimePlan {
    RuntimePlan {
        flows: vec![RuntimeFlow {
            id: FlowRuntimeId("flow.test".to_owned()),
            ops,
        }],
        ..RuntimePlan::default()
    }
}

fn plan_with_line_task(effect: LineEffectRequest) -> RuntimePlan {
    RuntimePlan {
        line_task_groups: vec![LineTaskGroup {
            root: LineTaskScope {
                node: LineTaskNode::Effect(effect),
                ..LineTaskScope::default()
            },
            ..LineTaskGroup::default()
        }],
        ..RuntimePlan::default()
    }
}

#[test]
fn component_view_dsl_lowers_to_ui_sidecars() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
style primary_button {
  Button:hover {
    background-color = rgba(54, 190, 170, 255)
  }
}

component FeedbackForm() -> View {
  TextField("Tokyo")
    .style(@style:.primary_button)
    .style(.Css) {
      color: white;
    }
}

flow test {
  component(@component:.FeedbackForm)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_ui_resources(&hir).expect("sidecars lower");

    let program = sidecars.program.expect("program sidecar");
    assert!(!program.instructions.is_empty());
    assert!(!program.semantic_targets.is_empty());
    assert!(!program.layout_bounds.is_empty());

    let input = sidecars.input.expect("input sidecar");
    assert_eq!(input.options.len(), 1);

    let style = sidecars.style.expect("style sidecar");
    assert_eq!(style.style_program_id, "style.primary_button");
    assert!(!style.rules.is_empty());
    assert!(!style.arcweft_sources.is_empty());
    assert!(!style.css_sources.is_empty());
}

#[test]
fn component_view_declaration_is_not_mounted_implicitly() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
component FeedbackForm() -> View {
  TextField(@input:.feedback)
    .label("Message")
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_ui_resources(&hir).expect("sidecars lower");

    assert!(sidecars.program.is_none());
    assert!(sidecars.text.is_none());
    assert!(sidecars.input.is_none());
}

#[test]
fn component_view_button_lowers_to_action_button_sidecar() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
component FeedbackForm() -> View {
  VStack {
    TextField(@input:.feedback, value: "", purpose: text, enter_key: send)
      .label("Message")
      .placeholder("Type text")
    Button(@button:.feedback_send)
      .label("Send")
      .on_click(|| text_submit(@input:.feedback, ime: .reject))
  }
}

flow test {
  component(@component:.FeedbackForm)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_ui_resources(&hir).expect("sidecars lower");
    let program = sidecars.program.expect("program sidecar");
    let text = sidecars.text.expect("text sidecar");
    let input = sidecars.input.expect("input sidecar");

    let button = program
        .action_buttons
        .iter()
        .find(|button| button.public_id == "button.feedback_send")
        .expect("action button emitted");
    assert!(
        input
            .options
            .iter()
            .any(|option| option.public_id == "input.feedback")
    );
    let option = input
        .options
        .iter()
        .find(|option| option.public_id == "input.feedback")
        .expect("component text field input option");
    assert_eq!(
        option.placeholder_text_source.as_deref(),
        Some("text.placeholder.input.feedback")
    );
    assert_eq!(option.submit_handler.as_deref(), Some("input.feedback"));
    assert_eq!(option.change_handler.as_deref(), Some("input.feedback"));
    let runtime_controls = input.runtime_text_controls(Some(&text), Some(&program));
    let feedback_control = runtime_controls
        .iter()
        .find(|control| control.public_id == "input.feedback")
        .expect("component text field runtime control");
    assert_eq!(
        feedback_control.bounds,
        arcweft_bundle::resource_codec::UiRuntimeTextControlBounds::new(
            48_000, 48_000, 420_000, 48_000,
        )
    );
    assert_eq!(
        program.text_control_bounds_for("input.feedback"),
        Some(feedback_control.bounds),
    );
    assert_eq!(
        program
            .semantic_targets
            .iter()
            .filter(|target| target.public_id == "input.feedback")
            .count(),
        1
    );
    assert_eq!(text.literal_text(&button.label_text_source), Some("Send"));
    assert!(matches!(
        &button.action,
        arcweft_bundle::resource_codec::ui::UiActionButtonActionResource::TextInputSubmit {
            input,
            ime_policy,
        } if input == "input.feedback"
            && *ime_policy
                == arcweft_bundle::resource_codec::ui::UiTextSubmitImePolicy::Reject
    ));
    assert_eq!(
        button.bounds,
        arcweft_bundle::resource_codec::UiRuntimeButtonBounds::new(
            484_000, 50_000, 180_000, 44_000,
        )
    );
    assert!(program.semantic_targets.iter().any(|target| {
        target.public_id == "button.feedback_send"
            && target.label_text_source.as_deref() == Some(&button.label_text_source)
    }));
}

#[test]
fn component_view_text_area_and_secure_field_emit_layout_bounds() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
component Credentials() -> View {
  VStack {
    TextArea(@input:.bio, value: "")
      .label("Bio")
      .placeholder("Bio")
    SecureField(@input:.password, value: "")
      .label("Password")
      .placeholder("Password")
  }
}

flow test {
  component(@component:.Credentials)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_ui_resources(&hir).expect("sidecars lower");
    let program = sidecars.program.expect("program sidecar");
    let input = sidecars.input.expect("input sidecar");
    let text = sidecars.text.expect("text sidecar");

    let controls = input.runtime_text_controls(Some(&text), Some(&program));
    let bio = controls
        .iter()
        .find(|control| control.public_id == "input.bio")
        .expect("text area runtime control");
    let password = controls
        .iter()
        .find(|control| control.public_id == "input.password")
        .expect("secure field runtime control");

    assert_eq!(
        bio.bounds,
        arcweft_bundle::resource_codec::UiRuntimeTextControlBounds::new(
            48_000, 48_000, 420_000, 136_000,
        )
    );
    assert_eq!(
        password.bounds,
        arcweft_bundle::resource_codec::UiRuntimeTextControlBounds::new(
            48_000, 200_000, 420_000, 48_000,
        )
    );
    assert_eq!(
        program.semantic_target_bounds_for("input.bio"),
        Some(bio.bounds),
    );
    assert_eq!(
        program.semantic_target_bounds_for("input.password"),
        Some(password.bounds),
    );
}

#[test]
fn component_view_submit_buttons_follow_target_text_control_slots() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
component FeedbackForm() -> View {
  VStack {
    HStack {
      TextField(@input:.name, value: "", purpose: name, enter_key: next)
        .label("Name")
        .placeholder("Name")
      Button(@button:.continue)
        .label("Continue")
        .on_click(|| text_submit @input:.name)
    }
    TextArea(@input:.brief, value: "", purpose: text, enter_key: send)
      .label("Brief")
      .placeholder("Idea")
    HStack {
      Button(@button:.send)
        .label("Send")
        .on_click(|| text_submit @input:.brief)
    }
  }
}

flow test {
  component(@component:.FeedbackForm)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_ui_resources(&hir).expect("sidecars lower");
    let program = sidecars.program.expect("program sidecar");

    let continue_button = program
        .action_buttons
        .iter()
        .find(|button| button.public_id == "button.continue")
        .expect("continue action button emitted");
    let send_button = program
        .action_buttons
        .iter()
        .find(|button| button.public_id == "button.send")
        .expect("send action button emitted");

    assert_eq!(
        continue_button.bounds,
        arcweft_bundle::resource_codec::UiRuntimeButtonBounds::new(
            484_000, 50_000, 180_000, 44_000,
        )
    );
    assert_eq!(
        send_button.bounds,
        arcweft_bundle::resource_codec::UiRuntimeButtonBounds::new(
            48_000, 264_000, 180_000, 44_000,
        )
    );
}

fn return_bundle(source_label: &str, return_value: &str) -> ArcweftBundle {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.test".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.test".to_owned()),
            ops: vec![FlowOp::Return(return_value.to_owned())],
        }],
        Vec::new(),
    )
    .expect("test runtime plan is valid");
    let display = LineDisplayCatalog::default();
    let product_awbc = AwbcLowerer::new(&plan, &display, source_label)
        .lower()
        .expect("test product AWBC lowers")
        .program;
    let program = BytecodeProgram::from_runtime_plan(plan);
    let stats = program.stats();
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
                entry_flow: Some("flow.test".to_owned()),
                flows: stats.flows,
                bytecode_instructions: stats.instructions,
                line_task_groups: stats.line_task_groups,
                stream_plans: stats.stream_plans,
                source_plans: stats.source_plans,
            },
        },
        BundleSource {
            label: source_label.to_owned(),
            text: format!("flow test {{ return \"{return_value}\" }}"),
        },
        program,
        display,
    )
    .with_product_awbc(product_awbc)
}

fn image_asset(id: &str) -> BundleImageAsset {
    BundleImageAsset {
        id: id.to_owned(),
        file: BundleVirtualFileRef {
            space: BundleVirtualFileSpace::Asset,
            path: "bg/room.png".to_owned(),
        },
        format: BundleImageFormat::Png,
        animation: BundleImageAnimation::Static,
        dimensions: None,
    }
}

fn sample_image_virtual_file(path: &str) -> BundleVirtualFile {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("samples")
            .join(".arcweft")
            .join("asset")
            .join(path),
    )
    .expect("sample image asset is readable");
    BundleVirtualFile {
        space: BundleVirtualFileSpace::Asset,
        path: path.to_owned(),
        bytes,
    }
}

#[test]
fn compile_bundle_for_selection_attaches_product_awbc_before_awfb_encoding() {
    let root = std::env::temp_dir().join(format!(
        "arcweft-product-awbc-builder-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("temporary source directory");
    let source_path = root.join("main.arcw");
    fs::write(&source_path, "flow main { return \"done\" }").expect("temporary source writes");
    let selection = SourceSelection::Direct {
        path: source_path.clone(),
    };
    let mut phases = Vec::new();

    let artifact = compile_bundle_for_selection(&selection, Vec::new(), &mut phases)
        .expect("ordinary source bundle compiles");
    let product_awbc = artifact
        .bundle
        .product_awbc()
        .expect("ordinary source bundle has product AWBC");
    assert!(!product_awbc.program().source_map.is_empty());
    assert_eq!(
        product_awbc.program().display_map.is_empty(),
        artifact.bundle.display == LineDisplayCatalog::default()
    );

    let bytes = artifact
        .bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("ordinary product AWFB encodes");
    let decoded = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &bytes)
        .expect("ordinary product AWFB decodes");
    assert!(decoded.product_awbc().is_some());
    assert!(decoded.bytecode.program.flows.is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_bundle_for_selection_attaches_product_awbc_before_awfb_encoding() {
    let root = std::env::temp_dir().join(format!(
        "arcweft-project-product-awbc-builder-{}",
        std::process::id()
    ));
    let source_root = root.join("src");
    fs::create_dir_all(&source_root).expect("temporary project source directory");
    let manifest_path = root.join("arcw.toml");
    let source_path = source_root.join("main.arcw");
    fs::write(
        &manifest_path,
        r#"
[package]
name = "product_awbc_builder"
"#,
    )
    .expect("temporary manifest writes");
    fs::write(
        &source_path,
        r#"
entry game {
    start(@flow.main)
}

flow main {
    return "done"
}
"#,
    )
    .expect("temporary project source writes");
    let selection = SourceSelection::Project {
        manifest: manifest_path,
        path: source_path,
    };
    let mut phases = Vec::new();

    let artifact = compile_bundle_for_selection(&selection, Vec::new(), &mut phases)
        .expect("ordinary project bundle compiles");
    let bytes = artifact
        .bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("ordinary project product AWFB encodes");
    let decoded = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &bytes)
        .expect("ordinary project product AWFB decodes");
    assert!(decoded.product_awbc().is_some());
    assert!(decoded.bytecode.program.flows.is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn collect_bundle_image_assets_decodes_static_and_animated_webp_metadata() {
    let assets = collect_bundle_image_assets(&[
        sample_image_virtual_file("bg/poster.webp"),
        sample_image_virtual_file("bg/loop.webp"),
    ])
    .expect("sample image assets decode");

    let poster = assets
        .iter()
        .find(|asset| asset.id == "asset.bg.poster")
        .expect("static webp asset is collected");
    assert_eq!(poster.format, BundleImageFormat::WebP);
    assert_eq!(poster.animation, BundleImageAnimation::Static);
    assert!(poster.dimensions.is_some());

    let loop_asset = assets
        .iter()
        .find(|asset| asset.id == "asset.bg.loop")
        .expect("animated webp asset is collected");
    assert_eq!(loop_asset.format, BundleImageFormat::WebP);
    assert_eq!(loop_asset.animation, BundleImageAnimation::Animated);
    assert!(loop_asset.dimensions.is_some());
}

#[test]
fn static_image_asset_refs_collects_nested_asset_image_entity_refs() {
    let plan = plan_with_ops(vec![FlowOp::If {
        condition: RuntimeExpr::Value(RuntimeValue::Bool(true)),
        then_ops: vec![image_await("asset.bg.room")],
        else_ops: vec![image_effect_call("image", "asset = @asset:.ui.logo")],
    }]);

    assert_eq!(
        static_image_asset_refs(&plan, &BTreeMap::new()),
        vec!["asset.bg.room".to_owned(), "asset.ui.logo".to_owned()]
    );
}

#[test]
fn static_image_asset_refs_collects_runtime_presentation_image_calls() {
    let plan = plan_with_ops(vec![
        image_effect_call("bg", "@asset:.bg.room"),
        image_effect_call("image.show", "asset = \"asset.ui.logo\""),
        FlowOp::Await {
            binding: None,
            target: AwaitTarget::new(
                NeedId("need.unrelated".to_owned()),
                TaskId("task.unrelated".to_owned()),
                HostTaskRequestTemplate::new("system", "info", []),
            ),
            pending: vec![LineEffectRequest::Call(RuntimeCall {
                callee: "image".to_owned(),
                args: vec!["asset = @asset:.bg.pulse".to_owned()],
            })],
        },
    ]);

    assert_eq!(
        static_image_asset_refs(&plan, &BTreeMap::new()),
        vec![
            "asset.bg.pulse".to_owned(),
            "asset.bg.room".to_owned(),
            "asset.ui.logo".to_owned()
        ]
    );
}

#[test]
fn static_image_asset_refs_collects_line_task_image_calls() {
    let plan = plan_with_line_task(LineEffectRequest::Call(RuntimeCall {
        callee: "bg".to_owned(),
        args: vec!["@asset:.bg.room".to_owned()],
    }));

    assert_eq!(
        static_image_asset_refs(&plan, &BTreeMap::new()),
        vec!["asset.bg.room"]
    );
}

#[test]
fn static_image_asset_refs_collects_declared_image_object_assets() {
    let declarations = parse_declared_image_objects(
        r"
image @image.sample.pulse {
    asset = @asset:.bg.pulse
    target = @target.sample.pulse
    layer = @layer.foreground
    x = 12px
    y = 34px
    width = 56px
    height = 78px
}
",
    );

    assert_eq!(
        static_image_asset_refs(&plan_with_ops(Vec::new()), &declarations),
        vec!["asset.bg.pulse"]
    );
}

#[test]
fn bundle_image_objects_collect_declared_bounds_and_opacity() {
    let declarations = parse_declared_image_objects(
        r"
image @image.sample.pulse {
    asset = @asset:.bg.pulse
    target = @target.sample.pulse
    layer = @layer.foreground
    x = 12px
    y = 34px
    width = 56px
    height = 78px
    fit = intrinsic
    alignment.x = right
    alignment.y = bottom
    playback.local_time = 50ms
    transform.tx = 24px
    transform.ty = 12px
    depth = 2500
    opacity = 0.875
    visible = true
}
",
    );

    let objects = bundle_image_objects(&declarations).expect("image object metadata");

    assert_eq!(
        objects,
        vec![BundleImageObject {
            id: "image.sample.pulse".to_owned(),
            asset: "asset.bg.pulse".to_owned(),
            target: Some("target.sample.pulse".to_owned()),
            layer: Some("layer.foreground".to_owned()),
            bounds: BundleImageObjectBounds::from_px(12, 34, 56, 78),
            placement: Some(StagePlacement::absolute(StageRect::new(
                12_000, 34_000, 56_000, 78_000,
            ))),
            fit: BundleImageObjectFit::Intrinsic,
            alignment: BundleImageObjectAlignment {
                x_milli: 1_000,
                y_milli: 1_000,
            },
            playback: BundleImageObjectPlayback {
                start_time_millis: 0,
                rate_milli: 1_000,
                paused_at_millis: None,
                pinned_local_time_millis: Some(50),
            },
            transform: BundleImageObjectTransform {
                m11_milli: 1_000,
                m12_milli: 0,
                m21_milli: 0,
                m22_milli: 1_000,
                tx_milli: 24_000,
                ty_milli: 12_000,
            },
            depth_milli: 2500,
            opacity_milli: 875,
            visible: true,
        }]
    );
}

#[test]
fn validate_referenced_bundle_image_assets_rejects_missing_static_refs() {
    let plan = plan_with_ops(vec![
        image_await("asset.bg.room"),
        image_effect_call("image", "asset = @asset:.ui.logo"),
    ]);

    assert!(validate_referenced_bundle_image_assets(&plan, &BTreeMap::new(), &[]).is_err());
    assert!(
        validate_referenced_bundle_image_assets(
            &plan,
            &BTreeMap::new(),
            &[image_asset("asset.bg.room"), image_asset("asset.ui.logo")]
        )
        .is_ok()
    );
}

#[test]
fn patch_bundle_artifact_helper_diffs_base_and_next_awfb_bytes() {
    let base_bytes = return_bundle("base.arcw", "base-done")
        .to_format_bytes(BundleFormat::Awfb)
        .expect("base bundle encodes");
    let next_bytes = return_bundle("next.arcw", "next-done")
        .to_format_bytes(BundleFormat::Awfb)
        .expect("next bundle encodes");

    let artifact = build_patch_bundle_artifact_from_awfb_bytes(&base_bytes, &next_bytes)
        .expect("patch artifact builds");

    assert_eq!(
        artifact.manifest.base_content_root,
        artifact.plan.base_content_root
    );
    assert_eq!(
        artifact.manifest.target_content_root,
        artifact.plan.target_content_root
    );
    assert!(!artifact.plan.operations.is_empty());
}

#[test]
fn run_bundle_applies_awfb_patch_before_execution() {
    let base_bytes = return_bundle("base.arcw", "base-done")
        .to_format_bytes(BundleFormat::Awfb)
        .expect("base bundle encodes");
    let target_bytes = return_bundle("target.arcw", "target-done")
        .to_format_bytes(BundleFormat::Awfb)
        .expect("target bundle encodes");
    let base_view =
        BundleView::parse(&base_bytes, ReadBudget::default()).expect("base AWFB parses");
    let target_view =
        BundleView::parse(&target_bytes, ReadBudget::default()).expect("target AWFB parses");
    let artifact =
        BundlePatchArtifact::from_views(&base_view, &target_view).expect("patch artifact");
    let patch_bytes = encode_patch_bundle(&artifact).expect("patch bundle encodes");
    let unique = format!(
        "arcweft-run-bundle-patch-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after UNIX epoch")
            .as_nanos()
    );
    let base_path = std::env::temp_dir().join(format!("{unique}-base.awfb"));
    let patch_path = std::env::temp_dir().join(format!("{unique}-patch.awfb"));
    fs::write(&base_path, base_bytes).expect("base bundle writes");
    fs::write(&patch_path, patch_bytes).expect("patch bundle writes");

    let report = run_patched_bundle_with_native_adapters(
        &base_path,
        &patch_path,
        &BundleRunnerOptions {
            steps: 4,
            max_ops: 8,
            ..BundleRunnerOptions::default()
        },
        &[],
    )
    .expect("patched bundle runs");

    assert_eq!(report.source, "target.arcw");
    assert_eq!(report.final_status, "done return target-done");

    let _ = fs::remove_file(base_path);
    let _ = fs::remove_file(patch_path);
}
