use super::*;
use arcweft_bundle::{
    BundleImageObject, BundleImageObjectBounds, BundleImageObjectFit,
    container::{BundleView, ReadBudget},
    patch::{BundlePatchArtifact, encode_patch_bundle},
    resource_codec::{ViewInputResource, ViewProgramResource, ViewStyleResource},
};
use arcweft_compiler::project::{ProjectCompilationContext, compile_project};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_core::effect::RuntimeEffectExpr;
use arcweft_core::plan::{FlowRuntimeId, RuntimeFlow, RuntimeLineId};
use arcweft_core::task::{
    AwaitTarget, HostTaskArgTemplate, HostTaskRequestTemplate, NeedId, TaskId,
};
use arcweft_dialogue::{DialoguePresentationProfile, DialogueProfileRevision, InlineFailurePolicy};
use arcweft_lang_hir::{
    model::HirModule,
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_sema::{env::TypeCheckEnv, registration::ProjectRegistrationFacts};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_render_text::{LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_runtime_driver::view_runtime::BundleViewRuntime;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

mod scroll_style;
mod view_part_recovery;

#[derive(Clone, Debug)]
struct TestCompiledViewResources {
    compiled: CompiledViewProduct,
    program: Option<ViewProgramResource>,
    style: Option<ViewStyleResource>,
    text: Option<ViewTextResource>,
    input: Option<ViewInputResource>,
    image_objects: Vec<BundleImageObject>,
}

fn test_dialogue_revision() -> DialogueProfileRevision {
    let manifest = SourceDocument::try_new(
        SourceDocumentId::try_new("cli-bundle-test").expect("document ID"),
        SourceName::Memory,
        "test manifest",
    )
    .expect("test document");
    let sources =
        SourceSetRevision::try_for_identities([manifest.identity()]).expect("test source revision");
    DialogueProfileRevision::from_admitted_parts(
        manifest.identity().clone(),
        sources,
        sources,
        ViewProgramId::try_new("view_program.cli-bundle-test").expect("View program ID"),
        AcceptedViewProgramRevision::try_from_bytes([0x5a; 32]).expect("View program revision"),
        ResourceTypeRegistry::empty().digest(),
    )
}

impl TestCompiledViewResources {
    fn from_compiled(compiled: CompiledViewProduct) -> Self {
        let program = compiled
            .product()
            .program()
            .map(|program| program.resource().clone());
        let style = compiled
            .product()
            .style()
            .map(|style| style.resource().clone());
        Self {
            program,
            style,
            text: compiled.text().cloned(),
            input: compiled.input().cloned(),
            image_objects: compiled.image_objects().to_vec(),
            compiled,
        }
    }
}

fn collect_bundle_dsl_view_resources(
    module: &HirModule,
) -> Result<TestCompiledViewResources, ExitCode> {
    collect_bundle_dsl_view_resources_for_package(module, "local.test-package")
}

fn collect_bundle_dsl_view_resources_for_package(
    module: &HirModule,
    package: &str,
) -> Result<TestCompiledViewResources, ExitCode> {
    let source = module.source_document().ok_or_else(|| {
        eprintln!("error: test View compilation requires source-bound HIR");
        ExitCode::FAILURE
    })?;
    let document = Arc::new(source.clone());
    let package_spec = PackageSpec {
        id: PackageId::new(package).map_err(|error| {
            eprintln!("error: invalid test package ID: {error}");
            ExitCode::FAILURE
        })?,
        version: PackageVersion::new("0.0.0").map_err(|error| {
            eprintln!("error: invalid test package version: {error}");
            ExitCode::FAILURE
        })?,
    };
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        package_spec,
        BuildSpec::default(),
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("arcweft-test://{package}/manifest")).map_err(
                    |error| {
                        eprintln!("error: invalid test manifest source ID: {error}");
                        ExitCode::FAILURE
                    },
                )?,
                SourceName::path("arcw.toml"),
                "",
            )
            .map_err(|error| {
                eprintln!("error: invalid test manifest source: {error}");
                ExitCode::FAILURE
            })?,
        ),
        [ProjectSourceFile::new(
            arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root(),
            PathBuf::from("main.arcw"),
            Arc::clone(&document),
            [],
        )],
    )
    .map_err(|error| {
        eprintln!("error: failed to build test project sources: {error}");
        ExitCode::FAILURE
    })?;
    let package = CallablePackageId::try_new(project.package().id.as_str()).map_err(|error| {
        eprintln!("error: invalid callable package ID: {error}");
        ExitCode::FAILURE
    })?;
    let world = ProjectSymbolWorldId::try_new(
        package,
        document.identity().id().clone(),
        "cli-view-product-test",
    )
    .map_err(|error| {
        eprintln!("error: invalid test semantic world: {error}");
        ExitCode::FAILURE
    })?;
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        eprintln!("error: failed to build test registration facts: {error:?}");
        ExitCode::FAILURE
    })?;
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        None,
        None,
        Vec::new(),
    );
    let compiled = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .map_err(|error| {
            eprintln!("error: failed to compile the test View project: {error:?}");
            ExitCode::FAILURE
        })?;
    Ok(TestCompiledViewResources::from_compiled(
        compiled.view_product().clone(),
    ))
}

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

fn view_definition<'a>(
    program: &'a ViewProgramResource,
    id: &str,
) -> &'a arcweft_bundle::resource_codec::view::ViewDefinitionResource {
    program
        .definitions
        .iter()
        .find(|definition| definition.public_id.as_str() == id)
        .unwrap_or_else(|| panic!("missing View definition `{id}`"))
}

fn view_action_buttons<'a>(
    program: &'a ViewProgramResource,
    view: &str,
) -> Vec<&'a arcweft_bundle::resource_codec::view::ViewActionButtonResource> {
    program
        .action_buttons
        .iter()
        .filter(|button| button.view.as_deref() == Some(view))
        .collect()
}

fn view_text_blocks<'a>(
    program: &'a ViewProgramResource,
    view: &str,
) -> Vec<&'a arcweft_bundle::resource_codec::view::ViewTextBlockResource> {
    program
        .text_blocks
        .iter()
        .filter(|block| block.view.as_deref() == Some(view))
        .collect()
}

fn view_surfaces<'a>(
    program: &'a ViewProgramResource,
    view: &str,
) -> Vec<&'a arcweft_bundle::resource_codec::view::ViewSurfaceResource> {
    program
        .surfaces
        .iter()
        .filter(|surface| surface.view.as_deref() == Some(view))
        .collect()
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
    use arcweft_bundle::resource_codec::view::{ViewElementKind, ViewProgramInstruction};
    use arcweft_view::style::{ViewStyleApplicationTarget, ViewStylePatchId, ViewStyleSheetId};

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
    .style {
      color = rgba(255, 255, 255, 255)
    }
}

flow test {
  view(@view:.FeedbackForm)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");

    let program = sidecars.program.expect("program sidecar");
    assert!(!program.instructions.is_empty());
    assert!(!program.semantic_targets.is_empty());
    assert!(!program.layout_bounds.is_empty());

    let input = sidecars.input.expect("input sidecar");
    assert_eq!(input.options.len(), 1);

    let style = sidecars.style.expect("style sidecar");
    assert!(
        style
            .program
            .sheets()
            .iter()
            .any(|sheet| sheet.id().public_id().as_str() == "style.primary_button"),
        "the authored sheet shares the compiler-owned product with the standard sheet"
    );
    assert_eq!(style.program.patches().len(), 1);

    let applications = program
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::TextField,
                styles,
                ..
            } => Some(styles.as_slice()),
            _ => None,
        })
        .expect("TextField producer owns its ordered Style applications");
    assert_eq!(
        applications,
        &[
            ViewStyleApplicationTarget::named(
                ViewStyleSheetId::try_new("style.primary_button").expect("valid sheet ID")
            ),
            ViewStyleApplicationTarget::inline(ViewStylePatchId::new(0)),
        ]
    );
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
    let program = sidecars.program.expect("program sidecar");

    assert_eq!(
        program
            .definitions
            .iter()
            .filter(|definition| {
                definition.public_id.as_str() != arcweft_bundle::standard_view::DIALOGUE_VIEW_ID
            })
            .count(),
        3
    );
    let child = view_definition(&program, "view.Child");
    let parent = view_definition(&program, "view.Parent");
    let toggle = view_definition(&program, "view.Toggle");
    assert_eq!(child.body.start_instruction, 0);
    assert_eq!(child.body.end_instruction, toggle.body.start_instruction);
    assert_eq!(toggle.body.end_instruction, parent.body.start_instruction);
    let standard_dialogue =
        view_definition(&program, arcweft_bundle::standard_view::DIALOGUE_VIEW_ID);
    assert!(parent.body.end_instruction as usize <= program.instructions.len());
    assert!(standard_dialogue.body.end_instruction as usize <= program.instructions.len());
    assert!(
        parent.body.end_instruction <= standard_dialogue.body.start_instruction
            || standard_dialogue.body.end_instruction <= child.body.start_instruction,
        "authored and standard definition bodies must not overlap"
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
            if view.as_str() == "view.Child"
                && arguments.len() == 1
                && arguments[0].ordinal == 0
                && arguments[0].name.as_deref() == Some("value")
    )));
    assert!(program.instructions.iter().any(|instruction| matches!(
        instruction,
        ViewProgramInstruction::CallView { view, arguments, .. }
            if view.as_str() == "view.Toggle"
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources_for_package(&hir, "local.test-package")
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
    assert_eq!(applications[0].0.package(), "local.test-package");
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
fn launch_profile_compiles_without_enumerating_default_source_root() {
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
schema = 1
default-profile = "main"

[package]
id = "org.arcweft.test.launch-only"
version = "0.1.0"

[profiles.main]
kind = "cli"
entry = "@entry.main"
source = "demo.arcw"
"#,
    )
    .expect("fixture manifest writes");
    fs::write(
        root.join("demo.arcw"),
        "entry cli @entry.main { goto @flow.main }\nflow @flow.main main { return () }",
    )
    .expect("profile source writes");

    let selection = resolve_source_selection(
        None,
        &ProfileOptions {
            profile: Some("main".to_owned()),
            manifest: manifest_path,
        },
    )
    .expect("profile resolves");
    assert_eq!(
        selection
            .package_identity()
            .expect("package identity resolves"),
        "org.arcweft.test.launch-only"
    );
    super::super::project::load_and_check_selection(&selection, None)
        .expect("launch profile compiles its selected source directly");

    fs::remove_dir_all(root).expect("fixture root removes");
}

#[test]
fn view_local_let_input_handle_lowers_without_a_fabricated_scalar_program() {
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");

    let program = sidecars.program.expect("program sidecar");
    assert!(
        !program.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                ViewProgramInstruction::BindLocal {
                    binding,
                    value_program: _,
                    source: None
                } if binding == "visitor_name"
            )
        }),
        "input handles are compiler bindings, not scalar runtime values"
    );

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
    use arcweft_bundle::resource_codec::view::{ViewElementKind, ViewProgramInstruction};

    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
style glass_shell {
  Box {
    background-color = rgba(20, 24, 32, 180)
  }

  Scroll {
    width = 512px
    height = 96px
    overflow = .Scroll
    opacity = 920milli
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");

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
    let feedback_buttons = view_action_buttons(&program, "view.FeedbackForm");
    assert_eq!(feedback_buttons.len(), 1);
    assert_eq!(
        feedback_buttons[0].containing_scroll_region.as_deref(),
        Some(program.scroll_regions[0].public_id.as_str())
    );
    let input = sidecars.input.as_ref().expect("input sidecar");
    assert_eq!(input.options.len(), 1);
    assert_eq!(
        input.options[0].containing_scroll_region.as_deref(),
        Some(program.scroll_regions[0].public_id.as_str())
    );

    let style = sidecars.style.expect("style sidecar");
    assert!(style.program.sheets().iter().any(|sheet| {
        sheet.rules().iter().any(|rule| {
            rule.selector()
                .sequences()
                .iter()
                .any(|sequence| sequence.element() == Some(ViewElementKind::Box))
        })
    }));
    assert!(style.program.sheets().iter().any(|sheet| {
        sheet.rules().iter().any(|rule| {
            rule.selector()
                .sequences()
                .iter()
                .any(|sequence| sequence.element() == Some(ViewElementKind::Scroll))
        })
    }));
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");

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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");

    assert!(collect_bundle_dsl_view_resources(&hir).is_err());
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
    overflow-x = .Auto
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");

    assert!(collect_bundle_dsl_view_resources(&hir).is_err());
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
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
        r#"
pub image @image.sample.pulse {
  asset = @asset.bg.pulse
  x = 12px
  y = 34px
  width = 320px
  height = 180px
  fit = "cover"
}

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
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");

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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
    let program = sidecars.program.expect("program lowers");
    let text = sidecars.text.expect("text resource lowers");

    assert_eq!(program.scroll_regions.len(), 1);
    let notes_blocks = view_text_blocks(&program, "view.NotesPanel");
    assert_eq!(notes_blocks.len(), 1);
    let block = notes_blocks[0];
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
      .width(100px)
    Text("After")
  }
}

flow test {
  view(@view:.StatusPanel)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
    let program = sidecars.program.expect("program lowers");

    let status_blocks = view_text_blocks(&program, "view.StatusPanel");
    assert_eq!(status_blocks.len(), 2);
    let wrapped = &status_blocks[0].bounds;
    assert_eq!(wrapped.width_milli, 100_000);
    assert_eq!(wrapped.height_milli, 48_000);
    let after = &status_blocks[1].bounds;
    assert_eq!(after.y_milli, 112_000);
}

#[test]
fn modern_feedback_view_subtitle_text_block_reserves_wrapped_height() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("samples/modern-feedback-view/arcw.toml");
    let selection = resolve_source_selection(
        None,
        &ProfileOptions {
            profile: Some("main".to_owned()),
            manifest,
        },
    )
    .expect("modern feedback view profile resolves");
    let mut phases = Vec::new();
    let semantic = semantic_context_for_selection(&selection, None)
        .expect("modern feedback project semantic context loads");
    let compiled = compile_profile_runtime_plan(&selection, &semantic, &mut phases)
        .expect("modern feedback project compiles through the canonical profile path");
    let sidecars = TestCompiledViewResources::from_compiled(compiled.view_product.clone());
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
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
        ViewProgramId::try_new("view-program.authored-test").unwrap(),
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
    let text = sidecars.text.expect("text sidecar");

    assert_eq!(
        text.sources[0].kind,
        ViewTextSourceKind::Projection {
            path: vec!["state".to_owned(), "message".to_owned()],
        }
    );
}

#[test]
fn dialogue_view_text_style_and_primary_action_lower_to_typed_resources() {
    use arcweft_bundle::resource_codec::view::{
        DialogueTextProjection, ViewActionButtonActionResource, ViewProgramInstruction,
        ViewTextSourceKind,
    };
    use arcweft_view::style::{ViewStyleApplicationTarget, ViewStyleSheetId};

    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
pub style dialogue_text {}

pub view DialoguePanel(dialogue: DialogueView) {
  Panel(x = 57.6px, y = 460.8px, width = 1164.8px, height = 201.6px, part = dialogue_panel) {
    Text(dialogue.speaker)
      .x(85.6px)
      .y(480.8px)
      .width(1108.8px)
      .height(28px)
    RichText(dialogue.content)
      .x(85.6px)
      .y(518.8px)
      .width(1108.8px)
      .height(125.6px)
      .style(@style.dialogue_text)
    Button("", x = 57.6px, y = 460.8px, width = 1164.8px, height = 201.6px)
      .on_click { dialogue.primary_action }
  }
}

"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
    let text = sidecars.text.expect("dialogue text sidecar");
    assert!(text.sources.iter().any(|source| {
        source.kind
            == ViewTextSourceKind::Dialogue {
                parameter: "dialogue".to_owned(),
                projection: DialogueTextProjection::Speaker,
            }
    }));
    assert!(text.sources.iter().any(|source| {
        source.kind
            == ViewTextSourceKind::Dialogue {
                parameter: "dialogue".to_owned(),
                projection: DialogueTextProjection::Content,
            }
    }));

    let program = sidecars.program.expect("dialogue View program");
    let dialogue_style = ViewStyleApplicationTarget::named(
        ViewStyleSheetId::try_new("style.dialogue_text").expect("valid dialogue Style ID"),
    );
    assert!(
        program
            .instructions
            .iter()
            .any(|instruction| match instruction {
                ViewProgramInstruction::EmitText { styles, .. } => {
                    styles.as_slice() == std::slice::from_ref(&dialogue_style)
                }
                _ => false,
            })
    );
    let primary_action = program
        .action_buttons
        .iter()
        .find(|button| {
            button.view.as_deref() == Some("view.DialoguePanel")
                && matches!(
                    &button.action,
                    ViewActionButtonActionResource::DialoguePrimaryAction { parameter }
                        if parameter == "dialogue"
                )
        })
        .expect("dialogue primary action button");
    assert!(text.sources.iter().any(|source| {
        source.public_id == primary_action.label_text_source
            && source.kind
                == ViewTextSourceKind::Literal {
                    value: String::new(),
                }
    }));
    let dialogue_surfaces = view_surfaces(&program, "view.DialoguePanel");
    assert_eq!(dialogue_surfaces.len(), 1);
    assert_eq!(dialogue_surfaces[0].bounds.x_milli, 57_600);
    assert_eq!(dialogue_surfaces[0].bounds.y_milli, 460_800);
    assert_eq!(dialogue_surfaces[0].bounds.width_milli, 1_164_800);
    assert_eq!(dialogue_surfaces[0].bounds.height_milli, 201_600);
    let dialogue_text_blocks = view_text_blocks(&program, "view.DialoguePanel");
    assert_eq!(dialogue_text_blocks.len(), 2);
    assert_eq!(dialogue_text_blocks[0].bounds.x_milli, 85_600);
    assert_eq!(dialogue_text_blocks[0].bounds.y_milli, 480_800);
    assert_eq!(dialogue_text_blocks[1].bounds.x_milli, 85_600);
    assert_eq!(dialogue_text_blocks[1].bounds.y_milli, 518_800);
    assert_eq!(dialogue_text_blocks[1].bounds.height_milli, 125_600);
}

#[test]
fn authored_export_part_lowers_to_typed_product_inventory() {
    use arcweft_bundle::resource_codec::view::ViewProgramInstruction;
    use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use std::sync::Arc;

    let source = r#"
view Card() {
  export part title as heading

  Text("Title").part(title)
}

flow test {
  view(@view.Card)
}
"#;
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("test.arcw").expect("source identity"),
            SourceName::path("test.arcw"),
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_document_with_source(document.clone(), ParseOptions::default());
    assert_eq!(parsed.errors(), &[]);
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
    let program = sidecars.program.expect("program sidecar");
    let [export] = program.exported_parts.as_slice() else {
        panic!("authored export must produce exactly one product record");
    };
    assert_eq!(export.target.view.view_id().as_str(), "view.Card");
    assert_eq!(export.target.part.as_public_id().as_str(), "title");
    assert_eq!(export.public_name.as_public_id().as_str(), "heading");
    let expected_source = arcweft_bundle::resource_codec::ProductSourceId::try_for_document_id(
        document.identity().id(),
    )
    .expect("product source identity");
    assert_eq!(program.source_refs.len(), 1);
    assert_eq!(program.source_refs[0].id(), &expected_source);
    assert_eq!(
        program.source_refs[0].revision(),
        document.identity().revision()
    );
    assert_eq!(
        program.source_refs[0].source_len(),
        document.identity().source_len()
    );
    assert!(export.source.declaration.start_byte() < export.source.local_name.start_byte());
    assert!(export.source.local_name.end_byte() < export.source.public_name.start_byte());
    let source = export.source.declaration.source();
    assert!(
        export
            .source
            .ranges()
            .iter()
            .all(|range| range.source() == source)
    );
    assert!(program.instructions.iter().any(|instruction| matches!(
        instruction,
        ViewProgramInstruction::EmitText { part: Some(part), .. }
            if part.as_public_id().as_str() == "title"
    )));
}

#[test]
fn subtree_sheet_styles_survive_standard_resource_linking() {
    use arcweft_bundle::resource_codec::view::ViewProgramInstruction;
    use arcweft_presentation::appearance::PresentationColor;
    use arcweft_view::style::{
        ViewColorValue, ViewLengthMilli, ViewPropertyKind, ViewSpecifiedValue,
        ViewStyleApplicationTarget, ViewStyleSheetId,
    };

    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
style showcase {
  .speaker {
    color = rgba(139, 211, 255, 255)
    font-size = 18px
  }
}

view Showcase() {
  Panel {
    Text("Hello").part(speaker)
  }
    .style(@style.showcase)
}

flow test {
  view(@view.Showcase)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
    let product = sidecars.compiled.product();
    let program = product.program().expect("linked View program").resource();
    let style = product.style().expect("linked Style resource").resource();
    let showcase_id =
        ViewStyleSheetId::try_new("style.showcase").expect("valid authored Style sheet ID");
    let standard_id = ViewStyleSheetId::try_new_engine_owned("std.style.dialogue")
        .expect("valid standard Style sheet ID");
    let showcase_target = ViewStyleApplicationTarget::named(showcase_id.clone());

    let showcase_definition = view_definition(program, "view.Showcase");
    let showcase_body = &program.instructions[showcase_definition.body.start_instruction as usize
        ..showcase_definition.body.end_instruction as usize];
    assert!(
        showcase_body.iter().any(|instruction| matches!(
            instruction,
            ViewProgramInstruction::OpenElement { styles, .. }
                if styles.as_slice() == std::slice::from_ref(&showcase_target)
        )),
        "the authored View must retain its named Style application"
    );

    let showcase = style
        .program
        .sheet(&showcase_id)
        .expect("authored Style sheet survives standard-resource linking");
    assert!(
        style.program.sheet(&standard_id).is_some(),
        "the standard dialogue Style sheet coexists with the authored sheet"
    );
    let [showcase_rule] = showcase.rules() else {
        panic!("showcase sheet must retain exactly one rule");
    };
    let [speaker_selector] = showcase_rule.selector().sequences() else {
        panic!("showcase rule must retain its single selector sequence");
    };
    assert_eq!(
        speaker_selector
            .part()
            .map(|part| part.as_public_id().as_str()),
        Some("speaker")
    );
    let [color, font_size] = showcase_rule.declarations() else {
        panic!("showcase rule must retain both typed declarations");
    };
    assert_eq!(color.property(), ViewPropertyKind::Color);
    assert_eq!(
        color.value(),
        &ViewSpecifiedValue::Color {
            value: ViewColorValue::Literal {
                color: PresentationColor::rgba(139, 211, 255, 255),
            },
        }
    );
    assert_eq!(font_size.property(), ViewPropertyKind::FontSize);
    assert_eq!(
        font_size.value(),
        &ViewSpecifiedValue::Length {
            value: ViewLengthMilli::new(18_000),
        }
    );

    assert_linked_style_sources_are_valid(style);
    program
        .validate_style_contract(Some(style))
        .expect("linked View program keeps valid typed Style references");
}

fn assert_linked_style_sources_are_valid(
    style: &arcweft_bundle::resource_codec::view::ViewStyleResource,
) {
    let source_count = style.source_map_refs.len();
    for sheet in style.program.sheets() {
        for token in sheet.tokens() {
            assert!((token.source().value() as usize) < source_count);
        }
        for rule in sheet.rules() {
            assert!((rule.source().value() as usize) < source_count);
            for declaration in rule.declarations() {
                assert!((declaration.source().value() as usize) < source_count);
            }
        }
    }
    for patch in style.program.patches() {
        for declaration in patch.declarations() {
            assert!((declaration.source().value() as usize) < source_count);
        }
    }
    for range in &style.source_map_refs {
        style
            .source_refs
            .get(range.source().value() as usize)
            .expect("every linked Style source range has a valid product source");
    }
    style
        .encode_canonical_section()
        .expect("linked Style source references remain canonically encodable");
}

const CUSTOM_DIALOGUE_VIEW_SOURCE: &str = r#"
#[dialogue_view]
pub struct StoryDialogue {
  speaker: String
  content: DialogueContent
  occurrence: DialogueOccurrenceId
  stage: DialogueStage
  reveal: DialogueReveal
  primary_action: DialogueAction
}

pub view StoryPanel(line: StoryDialogue) {
  Panel(x = 32px, y = 400px, width = 900px, height = 240px) {
    Text(line.speaker).x(48px).y(416px).width(860px).height(32px)
    RichText(line.content).x(48px).y(456px).width(860px).height(140px)
    Button("", x = 32px, y = 400px, width = 900px, height = 240px)
      .on_click { line.primary_action }
  }
}

"#;

#[test]
fn custom_dialogue_view_role_lowers_and_evaluates_through_the_bundle_runtime() {
    use arcweft_bundle::resource_codec::view::{ViewActionButtonActionResource, ViewParameterRole};
    use arcweft_runtime_driver::dialogue::{
        DialoguePresentationOperation, DialoguePresentationStore,
    };
    use arcweft_runtime_driver::view_runtime::{BundleViewRuntime, BundleViewTextValue};

    let parsed = arcweft_lang_syntax::parser::parse_source(CUSTOM_DIALOGUE_VIEW_SOURCE);
    assert_eq!(parsed.errors(), &[]);
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
    let program = sidecars.program.expect("custom dialogue View program");
    let text = sidecars.text.expect("custom dialogue text resource");
    let definition = program
        .definitions
        .iter()
        .find(|definition| definition.public_id.as_str() == "view.StoryPanel")
        .expect("custom View definition");
    assert!(definition.parameters.iter().any(|parameter| {
        parameter.name == "line" && parameter.role == ViewParameterRole::Dialogue
    }));
    assert!(program.action_buttons.iter().any(|button| matches!(
        &button.action,
        ViewActionButtonActionResource::DialoguePrimaryAction { parameter }
            if parameter == "line"
    )));
    program
        .validate_dialogue_contract(Some(&text))
        .expect("custom role produces a valid bundle contract");

    let line_id =
        RuntimeLineId::from_runtime_line_value("say.custom.dialogue").expect("runtime line id");
    let display_spec = LineDisplaySpec {
        line: line_id.clone(),
        callee: "character.hero".to_owned(),
        speaker_label: Some("Hero".to_owned()),
        text_key: None,
        view: arcweft_view::ViewId::try_new("view.StoryPanel").unwrap(),
        profile_style: None,
        dialogue_revision: test_dialogue_revision(),
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        inline_failure: InlineFailurePolicy::FailLine,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![RichTextNode::Text {
            text: "Custom runtime content".to_owned(),
        }]),
    };
    let display_frame = display_spec
        .clone()
        .resolve_frame(&arcweft_render_text::RuntimeLineContext::default())
        .expect("display frame resolves");
    let mut dialogue = DialoguePresentationStore::default();
    dialogue
        .apply_operations(&[DialoguePresentationOperation::append(
            arcweft_runtime_driver::dialogue::DialogueViewDefinition::new(
                arcweft_view::ViewId::try_new("view.StoryPanel").unwrap(),
            ),
            display_frame.clone(),
        )])
        .expect("dialogue appends");
    dialogue
        .synchronize_waiting_line(Some(&line_id))
        .expect("primary action synchronizes");
    let product = arcweft_bundle::resource_codec::ValidatedViewProduct::try_new(
        None,
        Some(program),
        None,
        arcweft_bundle::resource_codec::ViewProductValidationLimits::default(),
    )
    .expect("custom dialogue View product validates");
    let mut runtime = BundleViewRuntime::try_new_with_dialogue_display(
        product,
        Some(text),
        &LineDisplayCatalog::try_from_lines(test_dialogue_revision(), vec![display_spec])
            .expect("test display catalog is revision-consistent"),
    )
    .expect("custom dialogue View runtime builds");
    let frame = runtime.evaluate_with_dialogue(&[], &dialogue.view_inputs(), &[], false);

    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    assert_eq!(frame.mounts.len(), 1);
    let mount = &frame.mounts[0];
    assert_eq!(mount.view.as_str(), "view.StoryPanel");
    assert!(
        mount
            .dialogue
            .is_some_and(|state| state.primary_action.target.is_some())
    );
    assert!(mount.text.iter().any(|output| matches!(
        &output.value,
        BundleViewTextValue::DialogueSpeaker { label, frame }
            if label == "Hero" && frame.as_ref() == &display_frame
    )));
    assert!(mount.text.iter().any(|output| matches!(
        &output.value,
        BundleViewTextValue::DisplayFrame { frame, stage_index: 0 }
            if frame.as_ref() == &display_frame
    )));
}

#[test]
fn view_declaration_is_catalogued_without_creating_a_runtime_mount() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
view FeedbackForm() {
  TextField(@input:.feedback)
    .label("Message")
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");

    let product = sidecars.compiled.product().as_ref().clone();
    let runtime = BundleViewRuntime::try_new(product, sidecars.text.clone())
        .expect("accepted catalog creates a View runtime");
    assert_eq!(runtime.live_mount_count(), 0);

    let program = sidecars.program.expect("complete View program");
    assert!(
        program
            .definitions
            .iter()
            .any(|definition| definition.public_id.as_str() == "view.FeedbackForm")
    );
    assert!(sidecars.input.as_ref().is_some_and(|input| {
        input
            .options
            .iter()
            .any(|option| option.view.as_deref() == Some("view.FeedbackForm"))
    }));
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
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
fn view_text_control_submit_uses_only_the_input_writeback_route() {
    use arcweft_bundle::resource_codec::view::ViewProgramInstruction;

    let parsed = arcweft_lang_syntax::parser::parse_source(
        r#"
pub action feedback.submit(value: String)

view FeedbackForm() {
  let feedback = input.text(@input:.feedback, initial = "")
  TextField(feedback)
    .on_submit {
      action.invoke(@action:.feedback.submit, value = feedback.text)
    }
}

flow test {
  view(@view:.FeedbackForm)
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
    let program = sidecars.program.expect("program sidecar");
    let input = sidecars.input.expect("input sidecar");

    assert_eq!(
        input.options[0].submit_handler.as_deref(),
        Some("action.feedback.submit")
    );
    assert!(!program.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            ViewProgramInstruction::BindHandler { event, .. } if event == "submit"
        )
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
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
    let hir =
        arcweft_lang_hir::lower::lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("HIR lowers");
    let sidecars = collect_bundle_dsl_view_resources(&hir).expect("sidecars lower");
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
fn bundle_hydrates_default_view_localization_from_matching_display_text_key() {
    let document = RichTextDocument::new(vec![RichTextNode::Ruby {
        base: "夢".to_owned(),
        ruby: "ゆめ".to_owned(),
    }]);
    let display = LineDisplayCatalog::try_from_lines(
        test_dialogue_revision(),
        vec![LineDisplaySpec {
            line: RuntimeLineId::from_runtime_line_value("say.localization.display").unwrap(),
            callee: "narrator".to_owned(),
            speaker_label: None,
            text_key: Some("text.opening.dream".to_owned()),
            view: arcweft_bundle::standard_view::dialogue_view_id(),
            profile_style: None,
            dialogue_revision: test_dialogue_revision(),
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            inline_failure: InlineFailurePolicy::FailLine,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: document.clone(),
        }],
    )
    .expect("test display catalog is revision-consistent");
    let mut text = ViewTextResource {
        sources: vec![arcweft_bundle::resource_codec::view::ViewTextSourceRecord {
            public_id: "text.view.dream".to_owned(),
            kind: arcweft_bundle::resource_codec::view::ViewTextSourceKind::Localized {
                key: "text.opening.dream".to_owned(),
                locale: None,
            },
            source: None,
        }],
        ..ViewTextResource::default()
    };

    hydrate_default_view_localization(&mut text, &display);

    assert_eq!(
        text.localized_document("text.opening.dream", None),
        Some(&document)
    );
}

fn return_bundle(source_label: &str, return_value: &str) -> ArcweftBundle {
    let source = format!(
        "entry cli @entry.test {{ goto @flow.test }}\nflow test {{ return \"{return_value}\" }}"
    );
    let parsed = arcweft_lang_syntax::parser::parse_source(&source);
    assert_eq!(parsed.errors(), &[]);
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree())
        .expect("test source lowers to HIR");
    let runtime_options = RuntimePlanLowerOptions::default().with_dialogue_profile(
        DialoguePresentationProfile::engine_default(),
        test_dialogue_revision(),
    );
    let plan = arcweft_runtime_plan::flow::lower_runtime_plan(&hir, &runtime_options)
        .expect("test source lowers to a runtime plan");
    let display = LineDisplayCatalog::new(test_dialogue_revision());
    let product_awbc = AwbcLowerer::new(&plan, &display, source_label)
        .lower()
        .expect("test product AWBC lowers")
        .program;
    let program = BytecodeProgram::from_runtime_plan(plan);
    let stats = program.stats();
    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some("entry.test".to_owned()),
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
        arcweft_bundle::resource_codec::SourceMapSection::try_from_documents(&[
            &SourceDocument::try_new(
                SourceDocumentId::try_new(source_label).expect("source ID"),
                SourceName::path(source_label),
                source,
            )
            .expect("source document"),
        ])
        .expect("source map"),
        program,
        display,
    )
    .expect("standard dialogue source joins source map")
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
    fs::write(
        &source_path,
        r#"
struct GameState {
    started: bool
}

enum GameEvent {
    Start
}

fn initial_game_state() -> GameState
effects {}
{
    GameState { started = false }
}

fn reduce_game(state: &GameState, event: GameEvent)
    -> Result<Reduction<GameState>, ReducerError>
effects {}
{
    Ok(Reduction.unchanged(state))
}

entry game @entry.main {
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.main
}

flow main(state: GameState) { return "done" }
"#,
    )
    .expect("temporary source writes");
    let selection = SourceSelection::Direct {
        path: source_path.clone(),
    };
    let mut phases = Vec::new();

    let artifact = compile_bundle_for_selection(&selection, Vec::new(), &mut phases)
        .expect("ordinary source bundle compiles");
    assert!(
        artifact.bundle.view_program.is_some(),
        "the bundle consumes the compiler-owned accepted View product"
    );
    assert!(
        artifact.bundle.view_theme.is_none(),
        "an absent authored theme remains the canonical runtime default"
    );
    assert!(matches!(
        artifact.bundle.bytecode.program.entries[0].roles,
        arcweft_core::entry::RuntimeEntryRoles::Stateful(_)
    ));
    let product_awbc = artifact
        .bundle
        .product_awbc()
        .expect("ordinary source bundle has product AWBC");
    assert!(!product_awbc.program().source_map.is_empty());
    assert_eq!(
        product_awbc.program().display_map.is_empty(),
        artifact.bundle.display.lines().is_empty()
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
fn direct_bundle_source_defined_fx_application_resolves_its_definition() {
    let root =
        std::env::temp_dir().join(format!("arcweft-source-fx-bundle-{}", std::process::id()));
    fs::create_dir_all(&root).expect("temporary source directory");
    let source_path = root.join("opening.arcw");
    fs::write(
        &source_path,
        r#"
pub character narrator {
    display = "Narrator"
}

#[fx]
fn wave(amplitude: Length = 2px) -> Fx {
    Fx.transform(
        target = .glyph,
        sample = |ctx| Transform2D { translate_y: amplitude },
    )
}

entry cli @entry.main {
    goto @flow.main
}

flow main {
    narrator: [fx wave()]A[/fx][p]
    return "done"
}
"#,
    )
    .expect("temporary source writes");
    let selection = SourceSelection::Direct {
        path: source_path.clone(),
    };
    let mut phases = Vec::new();

    let artifact = compile_bundle_for_selection(&selection, Vec::new(), &mut phases)
        .expect("source-defined Fx bundle compiles");
    let frame = artifact.bundle.display.lines()[0]
        .resolve_frame(&arcweft_render_text::RuntimeLineContext::default())
        .expect("line display resolves");
    let application = frame
        .fx_applications()
        .next()
        .expect("typed Fx application remains in RichText");
    let expected_definition = format!(
        "{}::wave",
        selection
            .package_identity()
            .expect("direct source has one compiler package identity")
    );

    assert_eq!(application.definition().to_string(), expected_definition);
    assert!(
        artifact
            .bundle
            .fx_definitions
            .get(application.definition())
            .is_some(),
        "the application ID must resolve in the bundle FxDefinitions inventory"
    );

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
schema = 1

[package]
id = "org.arcweft.test.product-awbc-builder"
version = "0.1.0"

[build]
source-dir = "src"
"#,
    )
    .expect("temporary manifest writes");
    fs::write(
        &source_path,
        r#"
entry cli @entry.main { goto @flow.main }

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
fn project_bundle_uses_schema_one_asset_root_and_project_local_state() {
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
    let asset_root = root.join("assets").join("bg");
    let state_root = root.join(".arcweft").join("save");
    fs::create_dir_all(&source_root).expect("temporary project source directory");
    fs::create_dir_all(&asset_root).expect("project asset directory");
    fs::create_dir_all(&state_root).expect("project state directory");
    fs::create_dir_all(source_root.join(".arcweft/save")).expect("source-local legacy state");
    let manifest_path = root.join("arcw.toml");
    let source_path = source_root.join("main.arcw");
    fs::write(
        &manifest_path,
        r#"
schema = 1

[package]
id = "org.arcweft.test.resource-root-builder"
version = "0.1.0"

[build]
source-dir = "src"
"#,
    )
    .expect("temporary manifest writes");
    fs::write(
        &source_path,
        r#"
entry cli @entry.main { goto @flow.main }

flow main { return "done" }
"#,
    )
    .expect("temporary project source writes");
    fs::write(
        asset_root.join("room.png"),
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
    .expect("project bundle uses schema-one roots");

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
        static_image_asset_refs(&plan, std::iter::empty::<&str>()),
        vec!["asset.bg.room".to_owned(), "asset.view.logo".to_owned()]
    );
}

#[test]
fn static_image_asset_refs_collects_runtime_presentation_image_calls() {
    let plan = plan_with_ops(vec![
        image_effect_call("bg", "@asset:.bg.room"),
        image_effect_call("image", "asset = \"asset.view.logo\""),
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
        static_image_asset_refs(&plan, std::iter::empty::<&str>()),
        vec![
            "asset.bg.pulse".to_owned(),
            "asset.bg.room".to_owned(),
            "asset.view.logo".to_owned()
        ]
    );
}

#[test]
fn static_image_asset_refs_ignore_unknown_calls() {
    let plan = plan_with_ops(vec![image_effect_call(
        "mystery_present",
        "asset = @asset:.view.logo",
    )]);

    assert!(static_image_asset_refs(&plan, std::iter::empty::<&str>()).is_empty());
}

#[test]
fn static_image_asset_refs_rejects_non_asset_public_ids() {
    let plan = plan_with_ops(vec![
        image_effect_call("image", "@view:.logo"),
        image_effect_call("bg", "not a public id"),
    ]);

    assert!(static_image_asset_refs(&plan, std::iter::empty::<&str>()).is_empty());
}

#[test]
fn evaluated_builtin_effects_are_not_host_tasks_or_static_image_calls() {
    let plan = plan_with_ops(vec![FlowOp::EvaluatedEffect(RuntimeEffectExpr::Panic(
        RuntimeExpr::EntityRef("asset.bg.room".to_owned()),
    ))]);

    assert!(bundle_required_host_calls(&plan).is_empty());
    assert!(static_image_asset_refs(&plan, std::iter::empty::<&str>()).is_empty());
}

#[test]
fn static_image_asset_refs_collects_line_task_image_calls() {
    let plan = plan_with_line_task(LineEffectRequest::Call(RuntimeCall {
        callee: "bg".to_owned(),
        args: vec!["@asset:.bg.room".to_owned()],
    }));

    assert_eq!(
        static_image_asset_refs(&plan, std::iter::empty::<&str>()),
        vec!["asset.bg.room"]
    );
}

#[test]
fn static_image_asset_refs_collects_compiled_catalog_assets() {
    assert_eq!(
        static_image_asset_refs(&plan_with_ops(Vec::new()), ["asset.bg.pulse"]),
        vec!["asset.bg.pulse"]
    );
}

#[test]
fn validate_referenced_bundle_image_assets_rejects_missing_static_refs() {
    let plan = plan_with_ops(vec![
        image_await("asset.bg.room"),
        image_effect_call("image", "asset = @asset:.view.logo"),
    ]);

    assert!(
        validate_referenced_bundle_image_assets(&plan, std::iter::empty::<&str>(), &[]).is_err()
    );
    assert!(
        validate_referenced_bundle_image_assets(
            &plan,
            std::iter::empty::<&str>(),
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
