use super::*;
use arcweft_bundle::{
    BundleImageObjectBounds,
    container::{BundleView, ReadBudget},
    patch::{BundlePatchArtifact, encode_patch_bundle},
};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_core::effect::RuntimeEffectExpr;
use arcweft_core::plan::{FlowRuntimeId, RuntimeFlow};
use arcweft_core::task::{
    AwaitTarget, HostTaskArgTemplate, HostTaskRequestTemplate, NeedId, TaskId,
};
use arcweft_layout::stage_placement::{StagePlacement, StageRect};
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use std::path::Path;

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
            id: FlowRuntimeId::from_runtime_target_value("flow.test").expect("flow runtime id"),
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
fn view_dsl_lowers_to_view_sidecars() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
style primary_button {
  Button:hover {
    background-color = rgba(54, 190, 170, 255)
  }
}

view FeedbackForm() {
  TextField("Tokyo")
    .style(@style:.primary_button)
    .style(.Css) {
      color: white;
    }
}

flow test {
  view(@view:.FeedbackForm)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");

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
fn nested_view_calls_retain_definition_spans_typed_parameters_and_reachability() {
    use arcweft_bundle::resource_codec::view::{
        ViewProgramResource, ViewValueInputNamespace, ViewValueInputSource,
    };
    use arcweft_presentation::fx::FxRuntimeType;
    use arcweft_view::ViewValueProgramInventory;

    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view Child(value: i32 = 2) {
  if value > 0 {
    Text("child")
  }
}

view Toggle(value: bool = false) {
  if value {
    Text("enabled")
  }
}

view Parent() {
  Column {
    Child(value = 3)
    Toggle(value = true)
  }
}

flow test {
  view(@view:.Parent)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");
    let program = sidecars.program.expect("program sidecar");

    assert_eq!(program.definitions.len(), 3);
    let child = program
        .definitions
        .iter()
        .find(|definition| definition.public_id == "view.Child")
        .expect("transitively reachable child definition");
    let parent = program
        .definitions
        .iter()
        .find(|definition| definition.public_id == "view.Parent")
        .expect("mounted parent definition");
    let toggle = program
        .definitions
        .iter()
        .find(|definition| definition.public_id == "view.Toggle")
        .expect("second transitively reachable child definition");
    assert_eq!(child.body.start_instruction, 0);
    assert_eq!(child.body.end_instruction, toggle.body.start_instruction);
    assert_eq!(toggle.body.end_instruction, parent.body.start_instruction);
    assert_eq!(
        parent.body.end_instruction as usize,
        program.instructions.len()
    );
    assert_eq!(child.parameters.len(), 1);
    assert_eq!(child.parameters[0].ordinal, 0);
    assert_eq!(child.parameters[0].name, "value");
    assert_eq!(child.parameters[0].value_type, Some(FxRuntimeType::I32));
    assert_eq!(child.parameters[0].value_slot, Some(0));
    assert!(child.parameters[0].default_program.is_some());
    assert_eq!(toggle.parameters[0].name, "value");
    assert_eq!(toggle.parameters[0].value_type, Some(FxRuntimeType::Bool));
    assert_eq!(toggle.parameters[0].value_slot, Some(1));
    assert!(matches!(
        &program.value_inputs[0].source,
        ViewValueInputSource::DefinitionParameter { view, name }
            if view == "view.Child" && name == "value"
    ));
    assert_eq!(
        program.value_inputs[0].namespace,
        ViewValueInputNamespace::Parameter
    );
    assert!(matches!(
        &program.value_inputs[1].source,
        ViewValueInputSource::DefinitionParameter { view, name }
            if view == "view.Toggle" && name == "value"
    ));
    let inventory = ViewValueProgramInventory::from_programs(program.value_programs.clone())
        .expect("common typed View value inventory");
    assert_eq!(
        inventory.parameter_types(),
        &[FxRuntimeType::I32, FxRuntimeType::Bool]
    );
    assert_nested_view_call_bindings(&program);

    let bytes = program
        .encode_canonical_section()
        .expect("nested View program encodes");
    let decoded =
        ViewProgramResource::decode_canonical_section(&bytes).expect("nested View program decodes");
    assert_eq!(decoded.encode_canonical_section().unwrap(), bytes);
}

fn assert_nested_view_call_bindings(
    program: &arcweft_bundle::resource_codec::view::ViewProgramResource,
) {
    use arcweft_bundle::resource_codec::view::ViewProgramInstruction;

    assert!(program.instructions.iter().any(|instruction| matches!(
        instruction,
        ViewProgramInstruction::CallView { view, arguments, .. }
            if view == "view.Child"
                && arguments.len() == 1
                && arguments[0].ordinal == 0
                && arguments[0].name.as_deref() == Some("value")
    )));
    assert!(program.instructions.iter().any(|instruction| matches!(
        instruction,
        ViewProgramInstruction::CallView { view, arguments, .. }
            if view == "view.Toggle"
                && arguments.len() == 1
                && arguments[0].ordinal == 0
    )));
}

#[test]
fn view_fx_modifier_lowers_typed_bindings_key_and_ordinal() {
    use arcweft_bundle::resource_codec::view::ViewProgramInstruction;

    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
#[fx]
fn notice(accent: Color) -> Fx {
  Fx.text(color = accent)
}

#[fx]
fn pulse(speed: f32) -> Fx {
  Fx.text(opacity = speed)
}

view Warning(state: WarningState) {
  Text("WARNING")
    .fx(notice(accent = state.warning_color), key = state.warning_id)
    .fx(pulse(speed = 1.5))
}

flow test {
  view(@view:.Warning)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources_for_package(&hir, &[], "test-package")
        .expect("sidecars lower");
    let program = sidecars.program.expect("program sidecar");
    let applications = program
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            ViewProgramInstruction::ApplyFx {
                fx,
                arguments,
                key_program,
                application_ordinal,
                ..
            } => Some((fx, arguments, key_program, application_ordinal)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(applications.len(), 2);
    assert_eq!(applications[0].0.package(), "test-package");
    assert_eq!(applications[0].0.function(), "notice");
    assert_eq!(applications[0].1.len(), 1);
    assert_eq!(applications[0].1[0].parameter, "accent");
    assert!(applications[0].2.is_some());
    assert_eq!(*applications[0].3, 0);
    assert_eq!(applications[1].0.function(), "pulse");
    assert_eq!(applications[1].1[0].parameter, "speed");
    assert!(applications[1].2.is_none());
    assert_eq!(*applications[1].3, 1);
}

#[test]
fn profile_bundle_package_identity_does_not_require_default_source_root() {
    let unique = format!(
        "arcweft-bundle-package-identity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock follows epoch")
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    fs::create_dir_all(&root).expect("fixture root creates");
    let manifest_path = root.join("arcw.toml");
    fs::write(
        &manifest_path,
        r#"
default = "main"

[package]
name = "launch-only"
version = "0.1.0"

[profiles.main]
kind = "game"
source = "demo.arcw"
"#,
    )
    .expect("fixture manifest writes");
    fs::write(root.join("demo.arcw"), "flow main { return () }").expect("profile source writes");

    let selection = resolve_source_selection(
        None,
        &ProfileOptions {
            profile: Some("main".to_owned()),
            manifest: manifest_path,
        },
    )
    .expect("profile resolves");
    assert_eq!(
        selection_package_identity(&selection).expect("package identity resolves"),
        "launch-only"
    );

    fs::remove_dir_all(root).expect("fixture root removes");
}

#[test]
fn view_local_let_input_handle_lowers_to_program_binding() {
    use arcweft_bundle::resource_codec::view::{ViewProgramInstruction, ViewTextSourceKind};

    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view FeedbackForm() {
  let visitor_name = input.text(@input:.visitor_name, initial = "")
  Column {
    TextField(visitor_name)
      .placeholder("Your name")
  }
}

flow test {
  view(@view:.FeedbackForm)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");

    let program = sidecars.program.expect("program sidecar");
    assert!(program.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            ViewProgramInstruction::BindLocal {
                binding,
                value_program: _,
                source: None
            } if binding == "visitor_name"
        )
    }));

    let input = sidecars.input.expect("input sidecar");
    assert_eq!(input.options.len(), 1);
    assert_eq!(input.options[0].public_id, "input.visitor_name");

    let text = sidecars.text.expect("text sidecar");
    let value_source = text
        .sources
        .iter()
        .find(|source| source.public_id == "text.value.input.visitor_name")
        .expect("value text source");
    assert_eq!(
        value_source.kind,
        ViewTextSourceKind::Literal {
            value: String::new()
        }
    );
}

#[test]
fn view_box_and_scroll_lower_to_typed_view_resources() {
    use arcweft_bundle::resource_codec::view::{
        ViewElementKind, ViewProgramInstruction, ViewStyleSelectorPart,
    };

    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
style glass_shell {
  Box {
    background-color = rgba(20, 24, 32, 180)
  }

  Scroll {
    width = milli(512000)
    height = milli(96000)
    axis = text("vertical")
    overflow = text("scroll")
    opacity = milli(920)
  }
}

view FeedbackForm() {
  Box {
    Scroll(id = @scroll:.feedback_body, axis = .vertical, width = 360px, height = 120px, overflow = .hidden) {
      Text("Message")
      TextField(@input:.feedback)
      Button(@button:.send, label = "Send")
    }
  }
}

flow test {
  view(@view:.FeedbackForm)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");

    let program = sidecars.program.expect("program sidecar");
    assert!(program.instructions.iter().any(|instruction| matches!(
        instruction,
        ViewProgramInstruction::OpenElement {
            element: ViewElementKind::Box,
            ..
        }
    )));
    assert!(program.instructions.iter().any(|instruction| matches!(
        instruction,
        ViewProgramInstruction::OpenElement {
            element: ViewElementKind::Scroll,
            ..
        }
    )));
    assert_eq!(program.scroll_regions.len(), 1);
    assert_eq!(
        program.scroll_regions[0].view.as_deref(),
        Some("view.FeedbackForm")
    );
    assert_eq!(program.scroll_regions[0].public_id, "scroll.feedback_body");
    assert_eq!(program.scroll_regions[0].bounds.width_milli, 360_000);
    assert_eq!(program.scroll_regions[0].bounds.height_milli, 120_000);
    assert_eq!(program.scroll_regions[0].content_height_milli, 148_000);
    assert_eq!(
        program.scroll_regions[0].axis,
        arcweft_bundle::resource_codec::ViewScrollAxis::Vertical
    );
    assert_eq!(
        program.scroll_regions[0].overflow,
        arcweft_bundle::resource_codec::ViewScrollOverflowPolicy::Hidden
    );
    assert_eq!(program.action_buttons.len(), 1);
    assert_eq!(
        program.action_buttons[0]
            .containing_scroll_region
            .as_deref(),
        Some(program.scroll_regions[0].public_id.as_str())
    );
    let input = sidecars.input.as_ref().expect("input sidecar");
    assert_eq!(input.options.len(), 1);
    assert_eq!(
        input.options[0].containing_scroll_region.as_deref(),
        Some(program.scroll_regions[0].public_id.as_str())
    );

    let style = sidecars.style.expect("style sidecar");
    assert!(style.rules.iter().any(|rule| {
        rule.selector
            .parts
            .contains(&ViewStyleSelectorPart::Element(ViewElementKind::Box))
    }));
    assert!(style.rules.iter().any(|rule| {
        rule.selector
            .parts
            .contains(&ViewStyleSelectorPart::Element(ViewElementKind::Scroll))
    }));
}

#[test]
fn view_scroll_uses_style_rules_for_viewport_and_overflow_defaults() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
style scroll_defaults {
  token layout.scroll_width = milli(512000)

  Scroll {
    width = token(layout.scroll_width)
    height = milli(96000)
    axis = text("vertical")
    overflow = text("hidden")
  }
}

view StyledScroll() {
  Scroll {
    Text("One")
    Text("Two")
  }
}

flow test {
  view(@view:.StyledScroll)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");

    let program = sidecars.program.expect("program sidecar");
    assert_eq!(program.scroll_regions.len(), 1);
    assert_eq!(
        program.scroll_regions[0].public_id,
        "scroll.view.StyledScroll.0"
    );
    assert_eq!(program.scroll_regions[0].bounds.width_milli, 512_000);
    assert_eq!(program.scroll_regions[0].bounds.height_milli, 96_000);
    assert_eq!(
        program.scroll_regions[0].overflow,
        arcweft_bundle::resource_codec::ViewScrollOverflowPolicy::Hidden
    );
    assert_eq!(
        program.scroll_regions[0].axis,
        arcweft_bundle::resource_codec::ViewScrollAxis::Vertical
    );
}

#[test]
fn view_scroll_uses_overflow_x_style_as_horizontal_scroll() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
style horizontal_scroll {
  Scroll {
    width = milli(128000)
    height = milli(72000)
    overflow-x = text("scroll")
  }
}

view Gallery() {
  Scroll {
    Row {
      Button(@button:.one, label = "One")
      Button(@button:.two, label = "Two")
    }
  }
}

flow test {
  view(@view:.Gallery)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");

    let program = sidecars.program.expect("program sidecar");
    assert_eq!(program.scroll_regions.len(), 1);
    assert_eq!(program.scroll_regions[0].bounds.width_milli, 128_000);
    assert_eq!(program.scroll_regions[0].bounds.height_milli, 72_000);
    assert_eq!(
        program.scroll_regions[0].axis,
        arcweft_bundle::resource_codec::ViewScrollAxis::Horizontal
    );
    assert_eq!(
        program.scroll_regions[0].overflow,
        arcweft_bundle::resource_codec::ViewScrollOverflowPolicy::Scroll
    );
}

#[test]
fn view_scroll_lowers_policy_options_from_authoring() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view PolicyScroll() {
  Scroll(id = @scroll:.policy, width = 180px, height = 96px, indicators = .visible, overscroll = .elastic, auto_scroll_focus = .end) {
    Text("One")
  }
}

flow test {
  view(@view:.PolicyScroll)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");

    let program = sidecars.program.expect("program sidecar");
    assert_eq!(program.scroll_regions.len(), 1);
    let region = &program.scroll_regions[0];
    assert_eq!(region.public_id, "scroll.policy");
    assert_eq!(
        region.indicators,
        arcweft_bundle::resource_codec::ViewScrollIndicatorsPolicy::Visible
    );
    assert_eq!(
        region.overscroll,
        arcweft_bundle::resource_codec::ViewScrollOverscrollPolicy::Elastic
    );
    assert_eq!(
        region.auto_scroll_focus,
        arcweft_bundle::resource_codec::ViewFocusAutoScrollPolicy::End
    );
}

#[test]
fn view_scroll_rejects_both_axis_authoring() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view BothAxisScroll() {
  Scroll(axis = .both, width = 120px, height = 72px) {
    Text("One")
  }
}

flow test {
  view(@view:.BothAxisScroll)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");

    assert!(collect_bundle_dsl_view_resources(&hir, &[]).is_err());
}

#[test]
fn view_lazy_row_and_column_require_a_future_typed_runtime_contract() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view LazyList() {
  Scroll(width = 240px, height = 120px) {
    LazyRow {
      Button(@button:.one, label = "One")
      Button(@button:.two, label = "Two")
    }
    LazyColumn {
      Text("A")
      Text("B")
    }
  }
}

flow test {
  view(@view:.LazyList)
}
"#,
    );
    assert!(parsed.errors().iter().any(|error| {
        error
            .message()
            .contains("unsupported View element `LazyRow`")
    }));
    assert!(parsed.errors().iter().any(|error| {
        error
            .message()
            .contains("unsupported View element `LazyColumn`")
    }));
}

#[test]
fn view_style_rule_rejects_interactive_overflow_on_non_scroll_element() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
style invalid_button_scroll {
  Button {
    overflow-x = text("auto")
  }
}

view Actions() {
  Button(@button:.send, label = "Send")
}

flow test {
  view(@view:.Actions)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");

    assert!(collect_bundle_dsl_view_resources(&hir, &[]).is_err());
}

#[test]
fn view_inline_style_rejects_interactive_overflow_on_non_scroll_element() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view Notes() {
  Text("No implicit scroll")
    .style {
      overflow-y: scroll
    }
}

flow test {
  view(@view:.Notes)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");

    assert!(collect_bundle_dsl_view_resources(&hir, &[]).is_err());
}

#[test]
fn view_scroll_without_axis_defaults_to_vertical_in_authoring() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view DefaultScrollAxis() {
  Scroll(width = 120px, height = 72px) {
    Text("One")
  }
}

flow test {
  view(@view:.DefaultScrollAxis)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");
    let program = sidecars.program.expect("program sidecar");
    assert_eq!(program.scroll_regions.len(), 1);
    assert_eq!(
        program.scroll_regions[0].axis,
        arcweft_bundle::resource_codec::ViewScrollAxis::Vertical
    );
}

#[test]
fn view_scroll_axis_horizontal_lowers_to_typed_scroll_region() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view Gallery() {
  Scroll(id = @scroll:.gallery, axis = .horizontal, width = 120px, height = 72px) {
    Row {
      Button(@button:.one, label = "One")
      Button(@button:.two, label = "Two")
    }
  }
}

flow test {
  view(@view:.Gallery)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");
    let program = sidecars.program.expect("program sidecar");
    assert_eq!(program.scroll_regions.len(), 1);
    let region = &program.scroll_regions[0];
    assert_eq!(region.public_id, "scroll.gallery");
    assert_eq!(region.bounds.width_milli, 120_000);
    assert_eq!(region.bounds.height_milli, 72_000);
    assert_eq!(
        region.axis,
        arcweft_bundle::resource_codec::ViewScrollAxis::Horizontal
    );
    assert!(region.content_width_milli > region.bounds.width_milli);
    assert_eq!(region.content_height_milli, region.bounds.height_milli);
}

#[test]
fn view_scroll_contains_nested_image_element() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r"
view Gallery() {
  Scroll(id = @scroll:.gallery, width = 120px, height = 72px) {
    Image(@image:.sample.pulse)
      .width(56px)
      .height(78px)
  }
}

flow test {
  view(@view:.Gallery)
}
",
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let source_images = vec![BundleImageObject {
        id: "image.sample.pulse".to_owned(),
        asset: "asset.bg.pulse".to_owned(),
        target: Some("target.sample.pulse".to_owned()),
        layer: Some("layer.foreground".to_owned()),
        view: None,
        containing_scroll_region: None,
        bounds: BundleImageObjectBounds::from_px(12, 34, 320, 180),
        placement: Some(StagePlacement::absolute(StageRect::new(
            12_000, 34_000, 320_000, 180_000,
        ))),
        fit: BundleImageObjectFit::Cover,
        alignment: BundleImageObjectAlignment::default(),
        playback: BundleImageObjectPlayback::default(),
        transform: BundleImageObjectTransform::default(),
        depth_milli: 0,
        opacity_milli: 1_000,
        actions: Vec::new(),
        params: BTreeMap::new(),
        proxies: Vec::new(),
        visible: true,
    }];
    let sidecars = collect_bundle_dsl_view_resources(&hir, &source_images).expect("sidecars lower");

    assert_eq!(sidecars.image_objects.len(), 1);
    let image = &sidecars.image_objects[0];
    assert_eq!(image.id, "image.view.Gallery.0");
    assert_eq!(image.asset, "asset.bg.pulse");
    assert_eq!(image.view.as_deref(), Some("view.Gallery"));
    assert_eq!(
        image.containing_scroll_region.as_deref(),
        Some("scroll.gallery")
    );
    assert_eq!(
        image.bounds,
        BundleImageObjectBounds::from_px(48, 48, 56, 78)
    );
    assert_eq!(image.placement, None);
    assert_eq!(image.fit, BundleImageObjectFit::Cover);
}

#[test]
fn view_scroll_contains_nested_text_element() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view NotesPanel() {
  Scroll(id = @scroll:.notes, width = 280px, height = 64px) {
    Text("Arcweft Concierge")
  }
}

flow test {
  view(@view:.NotesPanel)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");
    let program = sidecars.program.expect("program lowers");
    let text = sidecars.text.expect("text resource lowers");

    assert_eq!(program.scroll_regions.len(), 1);
    assert_eq!(program.text_blocks.len(), 1);
    let block = &program.text_blocks[0];
    assert_eq!(block.public_id, "text.block.view.NotesPanel.0");
    assert_eq!(block.view.as_deref(), Some("view.NotesPanel"));
    assert_eq!(
        block.containing_scroll_region.as_deref(),
        Some("scroll.notes")
    );
    assert_eq!(
        text.literal_text(&block.text_source),
        Some("Arcweft Concierge")
    );
}

#[test]
fn view_static_text_bounds_account_for_wrapped_lines() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view StatusPanel() {
  Column {
    Text("aaaaaaaaaaa")
      .style {
        width = 100px
        font-size = 20000milli
      }
    Text("After")
  }
}

flow test {
  view(@view:.StatusPanel)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");
    let program = sidecars.program.expect("program lowers");

    assert_eq!(program.text_blocks.len(), 2);
    let wrapped = &program.text_blocks[0].bounds;
    assert_eq!(wrapped.width_milli, 100_000);
    assert_eq!(wrapped.height_milli, 48_000);
    let after = &program.text_blocks[1].bounds;
    assert_eq!(after.y_milli, 112_000);
}

#[test]
fn modern_feedback_view_subtitle_text_block_reserves_wrapped_height() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("samples/modern-feedback-view/src/main.arcw"),
    )
    .expect("modern feedback view sample source");
    let parsed = arcweft_lang_syntax::parser::parse_source(&source);
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");
    let program = sidecars.program.expect("program lowers");
    let text = sidecars.text.expect("text resource lowers");

    let subtitle = program
        .text_blocks
        .iter()
        .find(|block| {
            text.literal_text(&block.text_source)
                .is_some_and(|value| value.contains("one-line player-rendered view"))
        })
        .expect("subtitle text block");
    assert!(
        subtitle.bounds.height_milli >= 48_000,
        "subtitle must reserve multiple visual lines: {subtitle:?}"
    );
    let name_field = program
        .text_control_bounds_for("input.visitor_name")
        .expect("name field bounds");
    assert!(
        name_field.y_milli
            >= subtitle
                .bounds
                .y_milli
                .saturating_add(i32::try_from(subtitle.bounds.height_milli).unwrap_or(i32::MAX))
                .saturating_add(16_000),
        "name field must be placed after the wrapped subtitle: subtitle={subtitle:?}, name={name_field:?}"
    );
}

#[test]
fn view_await_lowers_to_view_program_branch_spans() {
    use arcweft_bundle::resource_codec::view::ViewProgramInstruction;

    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view AvatarPanel() {
  Column {
    AwaitView(load_avatar(user)) {
      pending _ => Text("Loading")
      ready img => Image(img)
      error _ => Button(@button:.fallback, label = "Fallback")
    }
  }
}

flow test {
  view(@view:.AvatarPanel)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");

    let program = sidecars.program.expect("program sidecar");
    let await_instruction = program
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            ViewProgramInstruction::Await {
                pending_branch,
                ready_branch,
                error_branch,
                denied_branch,
                ..
            } => Some((
                pending_branch.as_ref(),
                ready_branch.as_ref(),
                error_branch.as_ref(),
                denied_branch.as_ref(),
            )),
            _ => None,
        })
        .expect("await instruction");
    assert!(
        await_instruction
            .0
            .is_some_and(|branch| branch.body_span > 0)
    );
    assert!(
        await_instruction
            .1
            .is_some_and(|branch| branch.body_span > 0)
    );
    assert!(
        await_instruction
            .2
            .is_some_and(|branch| branch.body_span > 0)
    );
    assert!(await_instruction.3.is_none());
}

#[test]
fn view_reactive_if_match_for_lower_to_view_program_instructions() {
    use arcweft_bundle::resource_codec::view::ViewProgramInstruction;

    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view ReactivePanel() {
  Column {
    if true {
      Text("Empty")
    } else {
      Text("Available")
    }

    for choice in [1, 2] key = choice {
      Text("Choice")
    }

    match .Debug {
      .Normal => Text("Normal")
      .Debug => Text("Debug")
    }
  }
}

flow test {
  view(@view:.ReactivePanel)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");
    let program = sidecars.program.expect("program sidecar");

    let branch_count = program
        .instructions
        .iter()
        .filter(|instruction| matches!(instruction, ViewProgramInstruction::Branch { .. }))
        .count();
    assert!(branch_count >= 3, "expected if plus match branches");
    assert!(program.instructions.iter().any(|instruction| matches!(
        instruction,
        ViewProgramInstruction::RepeatKeyed {
            body_span,
            ..
        } if *body_span > 0
    )));
    assert!(program.instructions.iter().any(|instruction| matches!(
        instruction,
        ViewProgramInstruction::Branch {
            then_span,
            else_span: Some(else_span),
            ..
        } if *then_span > 0 && *else_span > 0
    )));

    assert_reactive_view_value_programs(&program);
}

fn assert_reactive_view_value_programs(
    program: &arcweft_bundle::resource_codec::ViewProgramResource,
) {
    use arcweft_bundle::resource_codec::view::ViewProgramInstruction;
    use arcweft_presentation::fx::{FxEvaluationBudget, FxRuntimeValue, FxSampleContext, Seconds};
    use arcweft_view::{
        ViewMountAllocator, ViewMountState, ViewProgramId, ViewValueProgramInventory,
    };

    let condition_program = program
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            ViewProgramInstruction::Branch {
                condition_program, ..
            } => Some(*condition_program),
            _ => None,
        })
        .expect("if condition program");
    let (source_program, key_program) = program
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            ViewProgramInstruction::RepeatKeyed {
                source_program,
                key_program,
                ..
            } => Some((*source_program, *key_program)),
            _ => None,
        })
        .expect("repeat value programs");
    let inventory =
        ViewValueProgramInventory::from_programs(program.value_programs.clone()).unwrap();
    assert_eq!(
        inventory.state_types(),
        &[arcweft_presentation::fx::FxRuntimeType::I32]
    );
    let mut allocator = ViewMountAllocator::default();
    let mut mount = ViewMountState::new(
        allocator.allocate().unwrap(),
        ViewProgramId(0),
        0,
        vec![],
        vec![FxRuntimeValue::I32(0)],
        &inventory,
    )
    .unwrap();
    let context = FxSampleContext::from_elapsed(Seconds::ZERO, 0, 7, false);
    let mut budget = FxEvaluationBudget::default();
    assert_eq!(
        mount
            .evaluate(condition_program, &inventory, context, &mut budget)
            .unwrap()
            .value(),
        FxRuntimeValue::Bool(true)
    );
    assert_eq!(
        mount
            .evaluate(source_program, &inventory, context, &mut budget)
            .unwrap()
            .value(),
        FxRuntimeValue::I32(2)
    );
    assert_eq!(
        mount
            .evaluate(key_program, &inventory, context, &mut budget)
            .unwrap()
            .value(),
        FxRuntimeValue::I32(0)
    );
    mount
        .set_state(0, FxRuntimeValue::I32(9), &inventory)
        .unwrap();
    assert_eq!(
        mount
            .evaluate(key_program, &inventory, context, &mut budget)
            .unwrap()
            .value(),
        FxRuntimeValue::I32(9)
    );
}

#[test]
fn view_text_state_projection_is_retained_as_typed_source() {
    use arcweft_bundle::resource_codec::view::ViewTextSourceKind;

    let parsed = arcweft_lang_syntax::parser::parse_source(
        r"
view StatusPanel(state: StatusState) {
  Text(state.message)
}

flow test {
  view(@view:.StatusPanel)
}
",
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");
    let text = sidecars.text.expect("text sidecar");

    assert_eq!(
        text.sources[0].kind,
        ViewTextSourceKind::Projection {
            path: vec!["state".to_owned(), "message".to_owned()],
        }
    );
}

#[test]
fn view_declaration_is_not_mounted_implicitly() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view FeedbackForm() {
  TextField(@input:.feedback)
    .label("Message")
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");

    assert!(sidecars.program.is_none());
    assert!(sidecars.text.is_none());
    assert!(sidecars.input.is_none());
}

#[test]
fn view_button_lowers_to_action_button_sidecar() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
pub action feedback.submit(value: String)

view FeedbackForm() {
  let feedback = input.text(@input:.feedback, initial = "")

  Column {
    TextField(feedback)
      .label("Message")
      .placeholder("Type text")
      .purpose(.text)
      .enter_key(.send)
      .on_submit {
        action.invoke(@action:.feedback.submit, value = feedback.text)
      }
    Button(@button:.feedback_send, label = "Send")
      .on_click {
        action.invoke(@action:.feedback.submit, value = feedback.text)
      }
  }
}

flow test {
  view(@view:.FeedbackForm)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");
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
        .expect("view text field input option");
    assert_eq!(
        option.placeholder_text_source.as_deref(),
        Some("text.placeholder.input.feedback")
    );
    assert_eq!(
        option.submit_handler.as_deref(),
        Some("action.feedback.submit")
    );
    assert_eq!(option.change_handler.as_deref(), Some("input.feedback"));
    let runtime_controls = input.runtime_text_controls(Some(&text), Some(&program));
    let feedback_control = runtime_controls
        .iter()
        .find(|control| control.public_id == "input.feedback")
        .expect("view text field runtime control");
    assert_eq!(
        feedback_control.bounds,
        arcweft_bundle::resource_codec::ViewRuntimeTextControlBounds::new(
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
        arcweft_bundle::resource_codec::view::ViewActionButtonActionResource::ActionInvoke {
            action,
            payload,
        } if action == "action.feedback.submit" && payload.is_some()
    ));
    assert_eq!(
        button.bounds,
        arcweft_bundle::resource_codec::ViewRuntimeButtonBounds::new(
            48_000, 112_000, 180_000, 44_000,
        )
    );
    assert!(program.semantic_targets.iter().any(|target| {
        target.public_id == "button.feedback_send"
            && target.label_text_source.as_deref() == Some(&button.label_text_source)
    }));
}

#[test]
fn view_text_area_and_secure_field_emit_layout_bounds() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view Credentials() {
  Column {
    TextArea(@input:.bio, value: "")
      .label("Bio")
      .placeholder("Bio")
    SecureField(@input:.password, value: "")
      .label("Password")
      .placeholder("Password")
  }
}

flow test {
  view(@view:.Credentials)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");
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
        arcweft_bundle::resource_codec::ViewRuntimeTextControlBounds::new(
            48_000, 48_000, 420_000, 136_000,
        )
    );
    assert_eq!(
        password.bounds,
        arcweft_bundle::resource_codec::ViewRuntimeTextControlBounds::new(
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
fn view_submit_buttons_follow_target_text_control_slots() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
pub action feedback.submit_name(value: String)
pub action feedback.submit_brief(value: String)

view FeedbackForm() {
  let name = input.text(@input:.name, initial = "")
  let brief = input.text(@input:.brief, initial = "")

  Column {
    Row {
      TextField(name)
        .label("Name")
        .placeholder("Name")
        .purpose(.name)
        .enter_key(.next)
        .on_submit {
          action.invoke(@action:.feedback.submit_name, value = name.text)
        }
      Button(@button:.continue, label = "Continue")
        .on_click {
          action.invoke(@action:.feedback.submit_name, value = name.text)
        }
    }
    TextArea(brief)
      .label("Brief")
      .placeholder("Idea")
      .purpose(.text)
      .enter_key(.send)
      .on_submit {
        action.invoke(@action:.feedback.submit_brief, value = brief.text)
      }
    Row {
      Button(@button:.send, label = "Send")
        .on_click {
          action.invoke(@action:.feedback.submit_brief, value = brief.text)
        }
    }
  }
}

flow test {
  view(@view:.FeedbackForm)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");
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
        arcweft_bundle::resource_codec::ViewRuntimeButtonBounds::new(
            484_000, 48_000, 180_000, 44_000,
        )
    );
    assert_eq!(
        send_button.bounds,
        arcweft_bundle::resource_codec::ViewRuntimeButtonBounds::new(
            48_000, 264_000, 180_000, 44_000,
        )
    );
}

#[test]
fn view_action_invoke_button_lowers_to_action_resource() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
pub action feedback.submit(value: String)

view FeedbackForm() {
  Button(@button:.continue, label = "Continue")
    .on_click {
      action.invoke(@action:.feedback.submit, value = visitor_name.text)
    }
}

flow test {
  view(@view:.FeedbackForm)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");
    let program = sidecars.program.expect("program sidecar");

    let button = program
        .action_buttons
        .iter()
        .find(|button| button.public_id == "button.continue")
        .expect("continue action button emitted");

    assert!(matches!(
        &button.action,
        arcweft_bundle::resource_codec::view::ViewActionButtonActionResource::ActionInvoke {
            action,
            payload,
        } if action == "action.feedback.submit"
            && payload == &Some(
                arcweft_bundle::resource_codec::view::ViewActionPayloadResource::TextControlProjection {
                    input: "input.visitor_name".to_owned(),
                    field: arcweft_bundle::resource_codec::view::ViewActionTextControlPayloadField::Text,
                }
            )
    ));
}

#[test]
fn view_generic_callback_block_lowers_to_handler_binding() {
    use arcweft_bundle::resource_codec::view::ViewProgramInstruction;

    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
pub action feedback.focus(value: String)

view FeedbackForm() {
  Button(@button:.continue, label = "Continue")
    .on_focus {
      action.invoke(@action:.feedback.focus, value = "focused")
    }
}

flow test {
  view(@view:.FeedbackForm)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree()).expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir, &[]).expect("sidecars lower");
    let program = sidecars.program.expect("program sidecar");

    assert!(program.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            ViewProgramInstruction::BindHandler { event, handler, .. }
                if event == "focus" && handler.contains(".handler.focus.")
        )
    }));
}

fn return_bundle(source_label: &str, return_value: &str) -> ArcweftBundle {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId::from_runtime_target_value("flow.test").expect("flow runtime id")),
        vec![RuntimeFlow {
            id: FlowRuntimeId::from_runtime_target_value("flow.test").expect("flow runtime id"),
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
            .join("assets")
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
entry game { goto @flow.main }

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
fn project_bundle_uses_manifest_resource_roots_and_project_local_state() {
    let unique = format!(
        "arcweft-project-resource-roots-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after UNIX epoch")
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    let source_root = root.join("src");
    let custom_asset_root = root.join("game-assets").join("bg");
    let state_root = root.join(".arcweft").join("save");
    fs::create_dir_all(&source_root).expect("temporary project source directory");
    fs::create_dir_all(&custom_asset_root).expect("custom asset directory");
    fs::create_dir_all(&state_root).expect("project state directory");
    fs::create_dir_all(source_root.join(".arcweft/save")).expect("source-local legacy state");
    let manifest_path = root.join("arcw.toml");
    let source_path = source_root.join("main.arcw");
    fs::write(
        &manifest_path,
        r#"
[package]
name = "resource_root_builder"

[resources]
asset-dir = "game-assets"
content-dir = "game-content"
"#,
    )
    .expect("temporary manifest writes");
    fs::write(&source_path, "flow main { return \"done\" }")
        .expect("temporary project source writes");
    fs::write(
        custom_asset_root.join("room.png"),
        sample_image_virtual_file("bg/room.png").bytes,
    )
    .expect("custom asset writes");
    fs::write(state_root.join("slot.txt"), "project-state").expect("project state writes");
    fs::write(source_root.join(".arcweft/save/legacy.txt"), "legacy-state")
        .expect("legacy state writes");
    let selection = SourceSelection::Project {
        manifest: manifest_path,
        path: source_path,
    };
    let mut phases = Vec::new();

    let artifact = compile_bundle_for_selection(
        &selection,
        vec![BundleVirtualFileSpace::Asset, BundleVirtualFileSpace::Save],
        &mut phases,
    )
    .expect("project bundle uses explicit roots");

    assert!(
        artifact
            .bundle
            .virtual_file(&BundleVirtualFileRef {
                space: BundleVirtualFileSpace::Asset,
                path: "bg/room.png".to_owned(),
            })
            .is_some()
    );
    assert!(
        artifact
            .bundle
            .virtual_file(&BundleVirtualFileRef {
                space: BundleVirtualFileSpace::Save,
                path: "slot.txt".to_owned(),
            })
            .is_some()
    );
    assert!(
        artifact
            .bundle
            .virtual_file(&BundleVirtualFileRef {
                space: BundleVirtualFileSpace::Save,
                path: "legacy.txt".to_owned(),
            })
            .is_none()
    );

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
        else_ops: vec![image_effect_call("image", "asset = @asset:.view.logo")],
    }]);

    assert_eq!(
        static_image_asset_refs(&plan, &BTreeMap::new()),
        vec!["asset.bg.room".to_owned(), "asset.view.logo".to_owned()]
    );
}

#[test]
fn static_image_asset_refs_collects_runtime_presentation_image_calls() {
    let plan = plan_with_ops(vec![
        image_effect_call("bg", "@asset:.bg.room"),
        image_effect_call("image.show", "asset = \"asset.view.logo\""),
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
            "asset.view.logo".to_owned()
        ]
    );
}

#[test]
fn evaluated_builtin_effects_are_not_host_tasks_or_static_image_calls() {
    let plan = plan_with_ops(vec![FlowOp::EvaluatedEffect(RuntimeEffectExpr::Panic(
        RuntimeExpr::EntityRef("asset.bg.room".to_owned()),
    ))]);

    assert!(bundle_required_host_calls(&plan).is_empty());
    assert!(static_image_asset_refs(&plan, &BTreeMap::new()).is_empty());
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
    action = action.inspect.pulse
    param.role = animated-hotspot
    proxy.id = proxy.pulse.hotspot
    proxy.type = PulseHotspot
    proxy.role = inspect
    proxy.layer = layer.hit
    proxy.depth = 2600
    proxy.hit_test = true
    proxy.param.channel = preview
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
            view: None,
            containing_scroll_region: None,
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
            actions: vec!["action.inspect.pulse".to_owned()],
            params: [(
                "param.role".to_owned(),
                BundleImageObjectParam::Text {
                    value: "animated-hotspot".to_owned(),
                },
            )]
            .into(),
            proxies: vec![BundleImageObjectProxy {
                id: "proxy.pulse.hotspot".to_owned(),
                type_name: Some("PulseHotspot".to_owned()),
                role: Some("inspect".to_owned()),
                layer: Some("layer.hit".to_owned()),
                depth_milli: Some(2600),
                hit_test: true,
                params: [(
                    "channel".to_owned(),
                    BundleImageObjectParam::Text {
                        value: "preview".to_owned(),
                    },
                )]
                .into(),
            }],
            visible: true,
        }]
    );
}

#[test]
fn validate_referenced_bundle_image_assets_rejects_missing_static_refs() {
    let plan = plan_with_ops(vec![
        image_await("asset.bg.room"),
        image_effect_call("image", "asset = @asset:.view.logo"),
    ]);

    assert!(validate_referenced_bundle_image_assets(&plan, &BTreeMap::new(), &[]).is_err());
    assert!(
        validate_referenced_bundle_image_assets(
            &plan,
            &BTreeMap::new(),
            &[image_asset("asset.bg.room"), image_asset("asset.view.logo")]
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
