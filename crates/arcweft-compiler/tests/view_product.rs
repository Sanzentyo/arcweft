use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use arcweft_bundle::{
    ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary,
    resource_codec::{
        SourceMapSection, ValidatedViewProduct, ViewProductValidationLimits,
        view::{
            EventKind, ViewFxArgumentSourceRef, ViewProgramInstruction, ViewProgramResource,
            ViewValueInputNamespace, ViewValueInputSource,
        },
    },
};
use arcweft_character::id::CharacterId;
use arcweft_compiler::project::{
    CompiledProject, ProjectCompilationContext, ProjectCompilationSession, ProjectCompileError,
    ProjectCompileStage, ProjectEntrySelection, ProjectEntrySelectionKind, compile_project,
};
use arcweft_core::{
    effect::RuntimeArtifactFingerprint, entry::RuntimeValueDigest, plan::RuntimeLineId,
};
use arcweft_dialogue::InlineFailurePolicy;
use arcweft_id::{PublicId, TextKey};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{env::TypeCheckEnv, registration::ProjectRegistrationFacts};
use arcweft_lang_syntax::{
    ast::module_path::{CanonicalModulePath, ModuleSegment},
    incremental::{ParsedSource, SyntaxDatabase},
    parser::ParseOptions,
};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_presentation::{
    fx::{FxRuntimeType, ValueInstruction},
    input::{InputEpoch, InputEvent},
};
use arcweft_project::graph::ModuleDependency;
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_runtime_driver::{
    dialogue::{
        DialoguePageIndex, DialogueViewInput, DialogueViewOccurrence, DialogueViewPrimaryAction,
        DialogueViewReveal, DialogueViewStage, DialogueViewState,
    },
    presentation_handles::PresentationHandleId,
    view_runtime::BundleViewRuntime,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_source::{
    DiagnosticLabelStyle, SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId,
};
use arcweft_text_model::{
    CharacterDialoguePresentationConfig, DialoguePresentationCharacter, LineDisplayFrame,
    RichTextDisplayMap,
};
use arcweft_view::{
    DialogueAdvanceTarget, DialogueEntryId, DialogueInstanceId, DialoguePresentationId,
    DialogueRevision, DialogueStageIndex, ViewHandlerInvocation, ViewHandlerResultRole, ViewId,
    style::ViewStyleSheetId,
};

fn compile_attached_project(
    project: &ProjectSources,
    context: &ProjectCompilationContext,
) -> Result<CompiledProject, ProjectCompileError> {
    let mut syntax = SyntaxDatabase::try_new().expect("View test syntax database");
    let parsed_sources: BTreeMap<CanonicalModulePath, ParsedSource> = project
        .modules()
        .map(|source| {
            let parsed = syntax
                .parse_initial(
                    SourceSnapshotId::initial(source.document().display_name().clone()),
                    Arc::clone(source.document()),
                    ParseOptions::default(),
                )
                .expect("View test attached source");
            (source.module().clone(), parsed)
        })
        .collect();
    let mut compiler = ProjectCompilationSession::try_new().expect("View test HIR database");
    compile_project(&mut compiler, project, &parsed_sources, context)
}

#[test]
fn compiler_lowers_every_typed_view_into_one_validated_product() {
    let source = r#"
view First() {
  Text("first")
}

view Second() {
  Text("second")
}

style Primary {
  Button { color = rgba(10, 20, 30, 255) }
}
"#;
    let fixture = project_view_fixture(source, "arcweft-test://compiler-view-product");
    let compiled = fixture.compile().expect("validated View product");
    let product = compiled.view_product();

    let program = product.product().program().expect("program");
    let first = ViewId::try_new("view.First").expect("first View ID");
    let second = ViewId::try_new("view.Second").expect("second View ID");
    assert!(program.definition(&first).is_some());
    assert!(program.definition(&second).is_some());
    assert!(
        program.definition(&ViewId::standard_dialogue()).is_some(),
        "the standard View is linked by the compiler"
    );
    assert_eq!(
        product.view_source(&first).expect("first source").source(),
        fixture.document.identity()
    );
    let style_id = ViewStyleSheetId::try_new("style.Primary").expect("Style ID");
    assert_eq!(
        product
            .style_source(&style_id)
            .expect("Style source")
            .source(),
        fixture.document.identity()
    );
    assert_eq!(
        product
            .view_source(&ViewId::standard_dialogue())
            .expect("standard View source")
            .source()
            .id()
            .as_str(),
        arcweft_bundle::standard_view::DIALOGUE_VIEW_SOURCE_ID
    );
    assert_ne!(
        product.authored_source_revision(),
        product.product_source_revision(),
        "engine-generated standard View/Style sources extend the complete product source set"
    );
    assert_eq!(
        product.resource_type_registry_digest(),
        ResourceTypeRegistry::empty().digest()
    );
}

#[test]
fn compiler_lowers_checked_on_click_to_typed_bundle_handler_without_fx_conflation() {
    let fixture = project_view_fixture_with_entry(
        "entry cli @entry.main { goto @flow.main }\n\
         flow main() -> String { return \"done\" }\n\
         view Main(dialogue: DialogueView, enabled: bool = true) {\n  Button(enabled = enabled).on_click { dialogue.primary_action }\n}\n",
        "arcweft-test://compiler-view-on-click",
    );
    let compiled = fixture.compile().expect("typed on_click View product");
    let program = compiled
        .view_product()
        .product()
        .program()
        .expect("View program")
        .resource();
    let definition = program
        .definitions
        .iter()
        .find(|definition| definition.public_id.as_str() == "view.Main")
        .expect("authored View definition");
    let body = &program.instructions
        [definition.body.start_instruction as usize..definition.body.end_instruction as usize];
    let handler = match body {
        [
            ViewProgramInstruction::OpenElement { .. },
            ViewProgramInstruction::CloseElement,
            ViewProgramInstruction::BindHandler {
                event: EventKind::Activate,
                handler,
                ..
            },
        ] => *handler,
        other => panic!("unexpected typed on_click body: {other:?}"),
    };
    let specification = program
        .handler_ref(handler)
        .expect("typed handler specification");
    assert!(matches!(
        specification.result.role(),
        ViewHandlerResultRole::DialogueAction
    ));
    assert_eq!(specification.captures.len(), 1);
    assert_eq!(specification.captures[0].parameter().unwrap().value(), 0);
    let binding = compiled
        .runtime_plan()
        .plan
        .pure_programs()
        .iter()
        .find(|binding| binding.program() == handler)
        .expect("mount-only runtime pure-program binding");
    let site = compiled
        .runtime_plan()
        .plan
        .function_sites()
        .get(binding.site())
        .unwrap();
    assert_eq!(site.inputs().len(), 1);
    assert!(compiled.runtime_plan().plan.pure_helpers().is_empty());
    assert!(
        !body
            .iter()
            .any(|instruction| matches!(instruction, ViewProgramInstruction::ApplyFx { .. }))
    );

    let encoded = program
        .encode_canonical_section()
        .expect("typed View codec");
    let decoded = ViewProgramResource::decode_canonical_section(&encoded)
        .expect("typed View codec round trip");
    assert!(decoded.instructions.iter().any(|instruction| matches!(
        instruction,
        ViewProgramInstruction::BindHandler {
            event: EventKind::Activate,
            ..
        }
    )));

    let awbc = AwbcLowerer::new(
        &compiled.runtime_plan().plan,
        &compiled.runtime_plan().dialogue_content_catalog,
        "main.arcw",
    )
    .lower()
    .expect("typed project lowers to verified Product AWBC")
    .program;
    let instruction_count = awbc.instructions.len();
    let bundle = ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            locale: compiled.locale().clone(),
            entry: Some("entry.main".to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                artifact_fingerprint: RuntimeArtifactFingerprint::try_from_bytes([0x7d; 32])
                    .expect("fixture artifact fingerprint is non-zero"),
                entry_flow: Some("flow.main".to_owned()),
                flows: compiled.runtime_plan().plan.flows().len(),
                bytecode_instructions: instruction_count,
                line_task_groups: 0,
                stream_plans: 0,
            },
        },
        SourceMapSection::try_from_documents(&[fixture.document.as_ref()])
            .expect("authored project source map"),
        awbc,
        compiled.runtime_plan().dialogue_content_catalog.clone(),
    )
    .expect("standard handler merges into compiled Product AWBC")
    .try_with_validated_view_product(compiled.view_product().product())
    .expect("compiler View product joins its exact Product AWBC");
    let bundle = if let Some(text) = compiled.view_product().text() {
        bundle.with_view_text(text.clone())
    } else {
        bundle
    };
    let encoded = bundle
        .with_character_dialogue_generation(
            compiled
                .runtime_plan()
                .character_dialogue_generation
                .as_ref()
                .expect("generation for typed DialogueView fixture")
                .as_ref()
                .clone(),
        )
        .to_format_bytes(BundleFormat::Awfb)
        .expect("compiled View product encodes as validated AWFB");
    let decoded = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &encoded)
        .expect("compiled View product decodes with exact handler cross-sections");
    let runtime_product = ValidatedViewProduct::try_new(
        Some(decoded.source_map.clone()),
        decoded.view_program.clone(),
        decoded.view_style.clone(),
        ViewProductValidationLimits::default(),
    )
    .expect("decoded View product validates");
    let mut runtime = BundleViewRuntime::try_new_with_awbc(
        runtime_product,
        decoded.view_text.clone(),
        Arc::new(decoded.product_awbc_program().clone()),
    )
    .expect("decoded handler catalog joins Product AWBC");
    let authored_view = ViewId::try_new("view.Main").expect("authored View ID");
    let display_frame = minimal_dialogue_frame(authored_view.clone());
    let advance_target = DialogueAdvanceTarget::new(
        DialoguePresentationId::new(11),
        DialogueEntryId::new(12),
        DialogueInstanceId::new(13),
        DialogueStageIndex::new(0),
        DialogueRevision::new(1),
    );
    let dialogue_input = DialogueViewInput {
        handle: PresentationHandleId::try_new("dialogue.compiler.e2e")
            .expect("dialogue presentation handle"),
        view: &authored_view,
        frame: &display_frame,
        state: DialogueViewState {
            occurrence: DialogueViewOccurrence {
                presentation: DialoguePresentationId::new(11),
                entry: DialogueEntryId::new(12),
                instance: DialogueInstanceId::new(13),
            },
            stage: DialogueViewStage {
                index: DialogueStageIndex::new(0),
                page: DialoguePageIndex::new(0),
                stage_count: 1,
                page_count: 1,
            },
            reveal: DialogueViewReveal::complete(),
            primary_action: DialogueViewPrimaryAction {
                target: Some(advance_target),
            },
        },
    };
    let mounted = runtime.evaluate_with_dialogue(&[], &[dialogue_input.clone()], &[], false);
    assert!(mounted.diagnostics.is_empty(), "{mounted:#?}");
    let [mount] = mounted.mounts.as_slice() else {
        panic!("compiled handler must publish exactly one View mount")
    };
    let [binding] = mount.events.as_slice() else {
        panic!("compiled handler must publish exactly one typed event binding")
    };
    let invocation = ViewHandlerInvocation::from_input(
        &InputEvent::activate(InputEpoch(1), binding.target().clone()),
        binding.event(),
        binding.route(),
    )
    .expect("presentation Activate forms the accepted typed invocation");
    assert_eq!(
        runtime
            .dispatch_invocation(&invocation)
            .expect("sealed handler token dispatches"),
        Some(
            arcweft_runtime_driver::dialogue::BundlePresentationInput::advance_dialogue(
                advance_target,
            ),
        )
    );
    let disabled = [arcweft_core::value::RuntimeBinding {
        name: "enabled".to_owned(),
        value: arcweft_core::value::RuntimeValue::Bool(false),
    }];
    let disabled_frame =
        runtime.evaluate_with_dialogue(&[], &[dialogue_input.clone()], &disabled, false);
    assert!(disabled_frame.diagnostics.is_empty(), "{disabled_frame:?}");
    assert!(!disabled_frame.mounts[0].action_buttons[0].enabled);
    assert!(disabled_frame.mounts[0].events.is_empty());
    assert!(
        runtime.dispatch_invocation(&invocation).is_err(),
        "disabled controls revoke their previous route"
    );
    let enabled = [arcweft_core::value::RuntimeBinding {
        name: "enabled".to_owned(),
        value: arcweft_core::value::RuntimeValue::Bool(true),
    }];
    let enabled_frame = runtime.evaluate_with_dialogue(&[], &[dialogue_input], &enabled, false);
    assert!(enabled_frame.diagnostics.is_empty(), "{enabled_frame:?}");
    assert!(enabled_frame.mounts[0].action_buttons[0].enabled);
    assert_eq!(enabled_frame.mounts[0].events.len(), 1);
}

#[test]
fn retained_state_handler_publishes_update_only_on_routed_event() {
    use arcweft_core::value::RuntimeValue;
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    use arcweft_runtime_driver::view_runtime::BundleViewTextValue;

    let compiled = project_view_fixture_with_entry(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main() {
    local state mut caption: String = "first"
    Text(caption)
    Button("change").on_click(|| { caption = "second"; () })
}
"#,
        "arcweft-test://retained-state-command",
    )
    .compile()
    .expect("typed state command handler");
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().cloned();
    let awbc = Arc::new(
        arcweft_bundle::standard_view::install_dialogue_handler_awbc(
            AwbcLowerer::new(
                &compiled.runtime_plan().plan,
                &compiled.runtime_plan().dialogue_content_catalog,
                "main.arcw",
            )
            .lower()
            .unwrap()
            .program,
        )
        .unwrap(),
    );
    let handle = PresentationHandleRecord::new(
        PresentationHandleId::try_new("view.command.first").unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let mut runtime = BundleViewRuntime::try_new_with_awbc(product, text, awbc).unwrap();
    let frame = runtime.evaluate(std::slice::from_ref(&handle), &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    assert!(
        matches!(&frame.mounts[0].text[0].value, BundleViewTextValue::Plain { value } if value == "first")
    );
    let binding = &frame.mounts[0].events[0];
    let invocation = ViewHandlerInvocation::from_input(
        &InputEvent::activate(InputEpoch(1), binding.target().clone()),
        binding.event(),
        binding.route(),
    )
    .unwrap();
    assert_eq!(runtime.dispatch_invocation(&invocation).unwrap(), None);
    let updated = runtime.evaluate(std::slice::from_ref(&handle), &[], false);
    assert!(updated.diagnostics.is_empty(), "{updated:#?}");
    assert!(
        matches!(&updated.mounts[0].text[0].value, BundleViewTextValue::Plain { value } if value == "second")
    );
    let snapshot = runtime.snapshot().unwrap();
    assert!(
        matches!(&snapshot.mounts[0].local_state[0].value, RuntimeValue::String(value) if value == "second")
    );
}

#[test]
fn compiler_lowers_view_fx_closed_and_reactive_bindings_from_checked_authority() {
    let fixture = project_view_fixture(
        r#"
view Main(speed: f32) {
  Text("closed").fx(wave(speed = 1.25))
  Text("direct").fx(wave(speed = speed))
  Text("sum").fx(wave(speed = speed + speed))
}
"#,
        "arcweft-test://compiler-view-fx",
    );
    let compiled = fixture.compile().expect("typed View Fx product");
    let program = compiled
        .view_product()
        .product()
        .program()
        .expect("View program")
        .resource();
    let definition = program
        .definitions
        .iter()
        .find(|definition| definition.public_id.as_str() == "view.Main")
        .expect("authored View definition");
    assert_eq!(definition.parameters.len(), 1);
    assert_eq!(
        definition.parameters[0].value_type,
        Some(FxRuntimeType::F32)
    );
    assert_eq!(definition.parameters[0].value_slot, Some(0));
    assert!(matches!(
        program.value_inputs.as_slice(),
        [input]
            if input.namespace == ViewValueInputNamespace::Parameter
                && input.slot == 0
                && input.value_type == FxRuntimeType::F32
                && matches!(&input.source,
                    ViewValueInputSource::DefinitionParameter { view, parameter }
                        if view.as_str() == "view.Main" && parameter.value() == 0)
    ));

    let body = &program.instructions
        [definition.body.start_instruction as usize..definition.body.end_instruction as usize];
    let applications = body
        .iter()
        .filter_map(|instruction| match instruction {
            ViewProgramInstruction::ApplyFx {
                arguments,
                application_ordinal,
                ..
            } => Some((arguments, *application_ordinal)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(applications.len(), 3);
    assert_eq!(
        applications
            .iter()
            .map(|(_, ordinal)| *ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(
        applications[0]
            .0
            .iter()
            .any(|argument| matches!(argument.source, ViewFxArgumentSourceRef::Closed(_)))
    );
    let reactive = applications
        .iter()
        .flat_map(|(arguments, _)| arguments.iter())
        .filter_map(|argument| match &argument.source {
            ViewFxArgumentSourceRef::Reactive(program) => Some(*program),
            ViewFxArgumentSourceRef::Closed(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(reactive.len(), 2);
    let direct = program
        .value_programs
        .iter()
        .find(|program| program.id() == reactive[0])
        .expect("direct reactive program");
    let repeated = program
        .value_programs
        .iter()
        .find(|program| program.id() == reactive[1])
        .expect("repeated reactive program");
    assert!(matches!(
        direct.program().instructions(),
        [
            ValueInstruction::LoadParameter { parameter },
            ValueInstruction::Return,
        ] if parameter.slot().get() == 0 && parameter.runtime_type() == FxRuntimeType::F32
    ));
    assert!(matches!(
        repeated.program().instructions(),
        [
            ValueInstruction::LoadParameter { parameter: left },
            ValueInstruction::LoadParameter { parameter: right },
            ValueInstruction::Add,
            ValueInstruction::Return,
        ] if left.slot().get() == 0 && right.slot().get() == 0
    ));

    let encoded = program
        .encode_canonical_section()
        .expect("typed View Fx codec");
    let decoded = ViewProgramResource::decode_canonical_section(&encoded)
        .expect("typed View Fx codec round trip");
    assert_eq!(decoded.value_inputs, program.value_inputs);
    assert_eq!(decoded.value_programs, program.value_programs);
}

#[test]
fn compiler_lowers_project_fx_view_binding_through_the_shared_catalog() {
    let fixture = project_view_fixture(
        r#"
#[fx]
fn tint(accent: Color) -> Fx {
  Fx.text(color = accent)
}

view Main(accent: Color) {
  Text("project").fx(tint(accent = accent))
}
"#,
        "arcweft-test://compiler-view-project-fx",
    );
    let compiled = fixture.compile().expect("typed project View Fx product");
    let program = compiled
        .view_product()
        .product()
        .program()
        .expect("View program")
        .resource();
    let definition = program
        .definitions
        .iter()
        .find(|definition| definition.public_id.as_str() == "view.Main")
        .expect("authored View definition");
    assert_eq!(
        definition.parameters[0].value_type,
        Some(FxRuntimeType::Color)
    );
    assert_eq!(definition.parameters[0].value_slot, Some(0));
    let body = &program.instructions
        [definition.body.start_instruction as usize..definition.body.end_instruction as usize];
    let application = body
        .iter()
        .find_map(|instruction| match instruction {
            ViewProgramInstruction::ApplyFx {
                fx,
                arguments,
                application_ordinal,
                ..
            } => Some((fx, arguments, application_ordinal)),
            _ => None,
        })
        .expect("project Fx application");
    assert_eq!(*application.2, 0);
    assert_eq!(application.1.len(), 1);
    assert_eq!(application.1[0].parameter.get(), 0);
    assert!(matches!(
        application.1[0].source,
        ViewFxArgumentSourceRef::Reactive(_)
    ));
    assert!(
        compiled
            .fx_definitions()
            .iter()
            .any(|definition| definition.id() == application.0)
    );
}

#[test]
fn compiler_qualifies_view_fx_parameter_slots_by_definition() {
    let fixture = project_view_fixture(
        r#"
view Alpha(speed: f32) {
  Text("alpha").fx(wave(speed = speed))
}

view Zeta(speed: f32) {
  Text("zeta").fx(wave(speed = speed))
}
"#,
        "arcweft-test://compiler-view-fx-qualified-inputs",
    );
    let compiled = fixture.compile().expect("definition-qualified View inputs");
    let program = compiled
        .view_product()
        .product()
        .program()
        .expect("View program")
        .resource();
    assert!(matches!(
        program.value_inputs.as_slice(),
        [alpha, zeta]
            if alpha.slot == 0
                && zeta.slot == 1
                && matches!(&alpha.source,
                    ViewValueInputSource::DefinitionParameter { view, parameter }
                        if view.as_str() == "view.Alpha" && parameter.value() == 0)
                && matches!(&zeta.source,
                    ViewValueInputSource::DefinitionParameter { view, parameter }
                        if view.as_str() == "view.Zeta" && parameter.value() == 0)
    ));
    for (view, expected_slot) in [("view.Alpha", 0), ("view.Zeta", 1)] {
        let definition = program
            .definitions
            .iter()
            .find(|definition| definition.public_id.as_str() == view)
            .expect("authored View definition");
        assert_eq!(definition.parameters[0].value_slot, Some(expected_slot));
        let reactive = program.instructions
            [definition.body.start_instruction as usize..definition.body.end_instruction as usize]
            .iter()
            .find_map(|instruction| match instruction {
                ViewProgramInstruction::ApplyFx { arguments, .. } => {
                    arguments.iter().find_map(|argument| match argument.source {
                        ViewFxArgumentSourceRef::Reactive(program) => Some(program),
                        ViewFxArgumentSourceRef::Closed(_) => None,
                    })
                }
                _ => None,
            })
            .expect("reactive View Fx program");
        let value_program = program
            .value_programs
            .iter()
            .find(|program| program.id() == reactive)
            .expect("definition-qualified value program");
        assert!(matches!(
            value_program.program().instructions(),
            [
                ValueInstruction::LoadParameter { parameter },
                ValueInstruction::Return,
            ] if parameter.slot().get() == expected_slot
        ));
    }
}

#[test]
fn handler_capture_abi_uses_canonical_inputs_instead_of_first_use_order() {
    let fixture = project_view_fixture_with_entry(
        "entry cli @entry.main { goto @flow.main }\nflow main() -> String { return \"done\" }\nview Main(dialogue: DialogueView, label: String) { Button().on_click { let observed = label; dialogue.primary_action } }\n",
        "arcweft-test://compiler-view-handler-capture-order",
    );
    let compiled = fixture
        .compile()
        .expect("mixed capture types follow the admitted ABI");
    let program = compiled
        .view_product()
        .product()
        .program()
        .unwrap()
        .resource();
    let handler = program
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            ViewProgramInstruction::BindHandler { handler, .. } => Some(*handler),
            _ => None,
        })
        .unwrap();
    let specification = program.handler_ref(handler).unwrap();
    let coordinates = specification
        .captures
        .iter()
        .map(|capture| capture.parameter().unwrap().value())
        .collect::<Vec<_>>();
    assert_eq!(
        coordinates,
        [0, 1],
        "first body use of label must not reorder the published parameter ABI"
    );
    let binding = compiled
        .runtime_plan()
        .plan
        .pure_programs()
        .iter()
        .find(|binding| binding.program() == handler)
        .unwrap();
    assert_eq!(binding.input_types().len(), 2);
    assert_ne!(binding.input_types()[0], binding.input_types()[1]);
    AwbcLowerer::new(
        &compiled.runtime_plan().plan,
        &compiled.runtime_plan().dialogue_content_catalog,
        "capture-order.arcw",
    )
    .lower()
    .expect("both capture types verify through the ordinary AWBC function ABI");
}

#[test]
fn compiler_rejects_affine_view_handler_capture_before_bundle_publication() {
    let fixture = project_view_fixture(
        "view Main(dialogue: DialogueView, pending: Need<i64>) {\n\
           Button().on_click { let observed = pending; dialogue.primary_action }\n\
         }\n",
        "arcweft-test://compiler-view-affine-handler-capture",
    );
    let error = fixture
        .compile()
        .expect_err("an affine Need capture cannot enter a retained View handler snapshot");
    let [diagnostic] = error.diagnostics() else {
        panic!("affine handler capture must have one owning diagnostic: {error:?}")
    };
    assert_eq!(diagnostic.stage(), ProjectCompileStage::ViewLower);
    assert_eq!(
        diagnostic
            .diagnostic()
            .code()
            .map(arcweft_source::DiagnosticCode::as_str),
        Some("compiler.view.lower")
    );
    assert!(
        diagnostic
            .diagnostic()
            .message()
            .contains("is not snapshot-retainable"),
        "the final compiler ownership boundary must own this rejection: {error:?}"
    );
}

#[test]
fn compiler_lowers_project_views_in_canonical_module_and_source_order() {
    let (project, context, root_document, a_document, z_document) =
        canonical_view_project_fixture();
    let compiled =
        compile_attached_project(&project, &context).expect("canonical multi-module View project");
    let program = compiled
        .view_product()
        .product()
        .program()
        .expect("project View program");
    let standard = ViewId::standard_dialogue();
    let retained_ids = compiled
        .analysis_lease()
        .project_symbols()
        .retained_symbols()
        .filter(|symbol| symbol.family() == arcweft_id::DeclarationIdentityFamily::View)
        .map(|symbol| {
            let module = compiled
                .analysis_lease()
                .hir_project()
                .view()
                .module(symbol.module())
                .unwrap();
            let arcweft_lang_hir::item::HirItemKind::View(view) =
                module.resolve_item(symbol.owner()).unwrap().kind()
            else {
                panic!("View owner");
            };
            assert_eq!(
                view.header().public_id().resolved(),
                Some(symbol.public_id())
            );
            symbol.public_id().as_str().to_owned()
        })
        .collect::<std::collections::BTreeSet<_>>();
    let authored = program
        .definitions()
        .filter(|definition| definition.public_id.view_id() != &standard)
        .collect::<Vec<_>>();
    let source_order = compiled
        .analysis_lease()
        .hir_project()
        .view()
        .items()
        .filter_map(|item| match item.item().kind() {
            arcweft_lang_hir::item::HirItemKind::View(view) => Some(
                view.header()
                    .public_id()
                    .resolved()
                    .unwrap()
                    .as_str()
                    .to_owned(),
            ),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        source_order,
        [
            "view.RootFirst",
            "view.RootSecond",
            "view.a.Card",
            "view.z.Card",
            "view.authored"
        ]
    );
    assert_eq!(
        authored
            .iter()
            .map(|definition| definition.public_id.view_id().as_str().to_owned())
            .collect::<std::collections::BTreeSet<_>>(),
        retained_ids
    );
    assert_eq!(program.program_id().as_str(), "view.program.view.RootFirst");
    let source_ordered_definitions = source_order
        .iter()
        .map(|id| {
            authored
                .iter()
                .find(|definition| definition.public_id.view_id().as_str() == id)
                .unwrap()
        })
        .collect::<Vec<_>>();
    for pair in source_ordered_definitions.windows(2) {
        assert!(
            pair[0].body.end_instruction <= pair[1].body.start_instruction,
            "View instruction spans must advance across module boundaries"
        );
    }

    for (view, document) in [
        ("view.RootFirst", &root_document),
        ("view.a.Card", &a_document),
        ("view.z.Card", &z_document),
        ("view.authored", &z_document),
    ] {
        let view = ViewId::try_new(view).expect("View ID");
        let span = compiled
            .view_product()
            .view_source(&view)
            .expect("module-bound View source");
        assert_eq!(span.source(), document.identity());
        assert!(source_text(document, span).starts_with("pub view"));
    }
}

#[test]
fn compiler_rejects_nested_view_recovery_before_product_acceptance() {
    for source in [
        "view Broken() {\n  Text(@@@)\n}\n",
        "view Broken() {\n  Panel(width = @@@)\n}\n",
        "view Broken() {\n  Scroll(axis = @@@) { Text(\"x\") }\n}\n",
        "view Broken() {\n  if @@@ { Text(\"x\") }\n}\n",
        "view Broken(value: i32) {\n  match value {\n    ??? => Text(\"x\")\n  }\n}\n",
        "view Broken(value: i32) {\n  match value {\n    .MissingArrow Text(\"x\")\n  }\n}\n",
        "view Broken() {\n  Button(\"x\").unknown_modifier(@@@)\n}\n",
        "view Broken() {\n  Button(\"x\").on_focus { wait(@@@) }\n}\n",
        "view Broken(items: Vec<Item>) {\n  for item in items key item.id {\n    Text(\"x\")\n  }\n}\n",
        "view Broken() {\n  Button(\"x\").nav(sideways: auto)\n}\n",
        "view Broken() {\n  Button(\"x\").nav(right: nowhere)\n}\n",
    ] {
        let fixture = project_view_fixture(source, "arcweft-test://compiler-view-recovery");
        let error = fixture
            .compile()
            .expect_err("malformed View must not enter an accepted product");
        let diagnostics = error.diagnostics();
        for diagnostic in diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.stage() == ProjectCompileStage::Parse)
        {
            assert!(diagnostic.syntax_diagnostic().is_some());
            assert!(diagnostic.diagnostic().labels().iter().any(|label| {
                label.style() == DiagnosticLabelStyle::Primary
                    && label.span().validate_for(&fixture.document).is_ok()
            }));
        }
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.stage() == ProjectCompileStage::Readiness),
            "a recovered HIR module must fail execution readiness: {source}\n{error:?}"
        );
    }
}

#[test]
fn compiler_rejects_invalid_builtin_view_arguments() {
    for head in [
        "Button(42)",
        "Button(\"x\", enabled = \"no\")",
        "Button(\"x\", label = \"duplicate\")",
        "Button(\"x\", unknown = true)",
        "Button(\"x\", \"extra\")",
    ] {
        let source = format!("view Good() {{ Text(\"ok\") }}\nview Broken() {{ {head} }}\n");
        let error = project_view_fixture(&source, "arcweft-test://view-argument-rejection")
            .compile()
            .expect_err("invalid builtin arguments must not publish a product");
        assert!(!error.diagnostics().is_empty(), "{head}: {error:?}");
    }
}

#[test]
fn compiler_defaults_execute_general_values_and_refresh_preceding_inputs() {
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };

    for (case, source) in [
        r#"view Main(value: String = "hello") { Text("static") }"#,
        "fn fallback() -> String { \"hello\" }\nview Main(value: String = fallback()) { Text(\"static\") }",
        r#"view Main(first: String = "hello", value: (String, String) = (first, "world")) { Text("static") }"#,
        "struct Label { value: String }\nview Main(value: Label = Label { value = \"hello\" }) { Text(\"static\") }",
        "enum Toggle { On, Off }\nview Main(value: Toggle = .On) { Text(\"static\") }",
        r#"view Main(first: i64 = 1, value: i64 -> i64 = |input: i64| input + first) { Text("static") }"#,
        r#"view Main(first: i64 = 1, value: i64 -> i64 = _ + first) { Text("static") }"#,
        r#"view Main(first: i64 -> i64 = |input: i64| input + 1, value: i64 -> i64 = first) { Text("static") }"#,
        r#"view Main(first: Vec<i64 -> i64> = [|input: i64| input + 1], value: Vec<i64 -> i64> = first) { Text("static") }"#,
        r#"view Main(first: Array<i64 -> i64, 1> = [|input: i64| input + 1], value: Array<i64 -> i64, 1> = first) { Text("static") }"#,
        r#"struct Handler { callback: i64 -> i64 effects {} }
view Main(first: Handler = Handler { callback = |input: i64| input + 1 }, value: Handler = first) { Text("static") }"#,
        r#"struct Holder<T> { callback: T }
view Main(first: Holder<i64 -> i64> = Holder { callback = |input: i64| input + 1 }, value: Holder<i64 -> i64> = first) { Text("static") }"#,
        r#"enum Slot<T> { Empty, Full T }
view Main(first: Slot<i64 -> i64> = .Full(|input: i64| input + 1), value: Slot<i64 -> i64> = first) { Text("static") }"#,
    ].into_iter().enumerate() {
        let source = format!(
            "entry cli @entry.main {{ goto @flow.main }}\nflow main() -> String {{ return \"done\" }}\n{source}"
        );
        let compiled =
            project_view_fixture_with_entry(&source, "arcweft-test://view-default-general")
                .compile()
                .unwrap_or_else(|error| panic!("{source}\n{error:?}"));
        let awbc = AwbcLowerer::new(
            &compiled.runtime_plan().plan,
            &compiled.runtime_plan().dialogue_content_catalog,
            "main.arcw",
        )
        .lower()
        .unwrap()
        .program;
        let awbc = arcweft_bundle::standard_view::install_dialogue_handler_awbc(awbc).unwrap();
        let resource = compiled.view_product().product().program().unwrap().resource();
        let encoded = resource.encode_canonical_section().unwrap();
        assert_eq!(ViewProgramResource::decode_canonical_section(&encoded).unwrap(), *resource);
        let mut forged = resource.clone();
        forged.definitions.iter_mut().find(|definition| definition.public_id.as_str() == "view.Main").unwrap().parameters.iter_mut().find_map(|parameter| parameter.default_program.as_mut()).unwrap().program = arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([0xa9;32]);
        assert!(forged.validate_awbc_programs(&awbc, None).is_err());
        let awbc = Arc::new(awbc);
        let mut runtime = BundleViewRuntime::try_new_with_awbc(
            compiled.view_product().product().as_ref().clone(),
            compiled.view_product().text().cloned(),
            Arc::clone(&awbc),
        )
        .unwrap_or_else(|error| {
            panic!("{source}\n{error:?}")
        });
        let handle = PresentationHandleRecord::new(
            PresentationHandleId::try_new("view.default.first").unwrap(),
            PresentationHandleKind::View,
            "view.Main".to_owned(),
            None,
            PresentationResourceState::Mounted,
            None,
            0,
        );
        let second = PresentationHandleRecord::new(PresentationHandleId::try_new("view.default.second").unwrap(), PresentationHandleKind::View, "view.Main".to_owned(), None, PresentationResourceState::Mounted, None, 0);
        let handles = [handle, second];
        let result = runtime.evaluate(&handles, &[], false);
        assert!(result.diagnostics.is_empty(), "{source}\n{result:#?}");
        let snapshot = runtime.snapshot().unwrap();
        if case >= 7 {
            use arcweft_core::awbc::{fiber::AwbcFiberStateSnapshot, product_step::AwbcProductStepExecutor};
            let definition = resource.definitions.iter().find(|definition| definition.public_id.as_str() == "view.Main").unwrap();
            let program = definition.parameters.iter().find(|parameter| parameter.name == "value").unwrap().default_program.as_ref().unwrap().program;
            let first_program = definition.parameters.iter().find(|parameter| parameter.name == "first").unwrap().default_program.as_ref().unwrap().program;
            let native_plan = Arc::new(compiled.runtime_plan().plan.clone());
            let finish_native = |mut engine: arcweft_core::engine::Engine| {
                for _ in 0..64 {
                    let output = engine.step(Default::default(), Default::default()).output;
                    assert!(output.diagnostics.is_empty(), "{output:?}");
                    if let Some((_, value)) = engine.take_program_result().unwrap() { return value; }
                }
                panic!("native default exceeded its deterministic step limit");
            };
            let native_first = finish_native(arcweft_core::engine::Engine::for_program_invocation(Arc::clone(&native_plan), first_program, vec![]).unwrap());
            let expected_native = native_first.clone();
            let native_value = finish_native(arcweft_core::engine::Engine::for_program_invocation(Arc::clone(&native_plan), program, vec![native_first]).unwrap());
            assert_eq!(native_value, expected_native, "native dependent default preserves all callbacks");
            if let RuntimeValue::Callable(native_value) = native_value {
                assert!(matches!(native_value.owner(), arcweft_core::task::RuntimeProgramOwner::Plan(owner) if Arc::ptr_eq(owner, &native_plan)));
            }
            if case <= 9 || case >= 11 {
            let first = snapshot.mounts[0].runtime_parameters.iter().find(|binding| binding.name == "first").unwrap().value.clone();
            let executor = AwbcProductStepExecutor::for_program_invocation(Arc::clone(&awbc), program, vec![first], arcweft_core::task::GenerationId::new(0), 64).unwrap();
            let owner = arcweft_core::task::RuntimeProgramOwner::Awbc(Arc::clone(&awbc));
            let image = AwbcFiberStateSnapshot::from_live(executor.compact_fiber()).unwrap();
            let encoded = serde_json::to_vec(&image).unwrap();
            let decoded: AwbcFiberStateSnapshot = serde_json::from_slice(&encoded).unwrap();
            let mut restored = decoded.into_live_for_program(&owner).unwrap();
            restored.validate_for_program(&awbc).unwrap();
            let checkpoint = restored.checkpoint().unwrap();
            restored.active_frame_mut().unwrap().clear_register(arcweft_core::awbc::schema::AwbcRegisterId(0)).unwrap();
            restored.restore(checkpoint, &owner).unwrap();
            let mut missing = image.clone();
            missing.frames[0].type_instantiation = None;
            assert!(missing.into_live_for_program(&owner).unwrap().validate_for_program(&awbc).is_err());
            let mut malformed = serde_json::to_value(&image).unwrap();
            malformed["frames"][0]["type_instantiation"]["effects"] = serde_json::json!([]);
            let malformed: AwbcFiberStateSnapshot = serde_json::from_value(malformed).unwrap();
            assert!(malformed.into_live_for_program(&owner).unwrap().validate_for_program(&awbc).is_err());
            let returned = restored.frames[0].registers.iter().find_map(|storage| storage.as_ref()).unwrap().clone();
            restored.mark_returned(Some(returned)).unwrap();
            restored.validate_for_program(&awbc).unwrap();
            let mut frameless = AwbcFiberStateSnapshot::from_live(&restored).unwrap();
            frameless.frames.clear();
            assert!(matches!(frameless.into_live_for_program(&owner).unwrap().validate_for_program(&awbc), Err(arcweft_core::awbc::fiber::FiberStateError::ReturnValueMismatch)));
            }
        }
        assert_eq!(snapshot.mounts.len(), 2);
        assert_ne!(snapshot.mounts[0].state.mount, snapshot.mounts[1].state.mount);
        let value = &snapshot.mounts[0]
            .runtime_parameters
            .iter()
            .find(|binding| binding.name == "value")
            .unwrap()
            .value;
        match case {
            0 | 1 => assert_eq!(*value, RuntimeValue::String("hello".to_owned())),
            2 => assert_eq!(*value, RuntimeValue::Tuple(vec![RuntimeValue::String("hello".to_owned()), RuntimeValue::String("world".to_owned())])),
            3 => { let RuntimeValue::NominalRecord(record) = value else { panic!("expected nominal record: {value:?}") }; assert_eq!(record.fields(), [RuntimeValue::String("hello".to_owned())]); }
            4 => assert!(matches!(value, RuntimeValue::Variant { name, ordinal: 0, .. } if name == "On")),
            5 | 6 | 7 => assert!(matches!(value, RuntimeValue::Callable(_))),
            8 | 9 => { let RuntimeValue::Seq(values) = value else { panic!("expected callback container: {value:?}") }; assert_eq!(values.len(), 1); assert!(matches!(values.value_at(0), RuntimeValue::Callable(_))); }
            10 | 11 => {
                let RuntimeValue::NominalRecord(record) = value else { panic!("expected callback record: {value:?}") };
                assert!(matches!(record.fields(), [RuntimeValue::Callable(_)]));
                let ordinal = awbc.runtime_types.iter().position(|row| row.semantic_identity() == record.semantic_identity()).unwrap();
                let ty = arcweft_core::awbc::schema::AwbcTypeId(u32::try_from(ordinal).unwrap());
                let wrong_field = RuntimeValue::NominalRecord(arcweft_core::value::RuntimeNominalRecordValue::new(record.type_id().clone(), record.semantic_identity(), record.layout(), vec![RuntimeValue::Unit]));
                assert!(awbc.validate_live_value(ty, &wrong_field, arcweft_core::entry::RuntimeSchemaLimits::engine_default()).is_err());
                for (nominal, semantic, layout, fields) in [
                    (arcweft_core::entry::RuntimeNominalTypeId::from_checked_digest([0xa1; 32]), record.semantic_identity(), record.layout(), record.fields().to_vec()),
                    (record.type_id().clone(), arcweft_core::pattern::RuntimeSemanticTypeId::from_bytes([0xa2; 32]), record.layout(), record.fields().to_vec()),
                    (record.type_id().clone(), record.semantic_identity(), arcweft_core::entry::TypeLayoutHash::from_bytes([0xa3; 32]), record.fields().to_vec()),
                    (record.type_id().clone(), record.semantic_identity(), record.layout(), vec![]),
                ] {
                    let forged = RuntimeValue::NominalRecord(arcweft_core::value::RuntimeNominalRecordValue::new(nominal, semantic, layout, fields));
                    assert!(awbc.validate_live_value(ty, &forged, arcweft_core::entry::RuntimeSchemaLimits::engine_default()).is_err());
                }
            }
            12 => assert!(matches!(value, RuntimeValue::Variant { name, ordinal: 1, payload: Some(payload), .. } if name == "Full" && matches!(payload.as_ref(), RuntimeValue::Tuple(fields) if matches!(fields.as_slice(), [RuntimeValue::Callable(_)]))), "{value:?}"),
            _ => unreachable!(),
        }
        runtime.restore(&snapshot, &handles).unwrap();
        assert!(
            runtime
                .evaluate(&handles, &[], false)
                .diagnostics
                .is_empty()
        );
        assert_eq!(
            runtime.snapshot().unwrap().mounts[0].runtime_parameters,
            snapshot.mounts[0].runtime_parameters
        );

        if source.contains("value: (String, String)") {
            let changed = [RuntimeBinding {
                name: "first".to_owned(),
                value: RuntimeValue::String("changed".to_owned()),
            }];
            assert!(
                runtime
                    .evaluate(&handles, &changed, false)
                    .diagnostics
                    .is_empty()
            );
            let changed_snapshot = runtime.snapshot().unwrap();
            assert_eq!(
                changed_snapshot.mounts[0]
                    .runtime_parameters
                    .iter()
                    .find(|binding| binding.name == "value")
                    .unwrap()
                    .value,
                RuntimeValue::Tuple(vec![
                    RuntimeValue::String("changed".to_owned()),
                    RuntimeValue::String("world".to_owned())
                ])
            );
            let supplied = [RuntimeBinding {
                name: "value".to_owned(),
                value: RuntimeValue::Tuple(vec![
                    RuntimeValue::String("supplied".to_owned()),
                    RuntimeValue::String("override".to_owned()),
                ]),
            }];
            assert!(
                runtime
                    .evaluate(&handles, &supplied, false)
                    .diagnostics
                    .is_empty()
            );
            assert_eq!(
                runtime.snapshot().unwrap().mounts[0]
                    .runtime_parameters
                    .iter()
                    .find(|binding| binding.name == "value")
                    .unwrap()
                    .value,
                supplied[0].value
            );
        }
        if case == 0 {
            let before = runtime.snapshot().unwrap();
            let supplied = [RuntimeBinding {
                name: "value".to_owned(),
                value: RuntimeValue::Tuple(vec![RuntimeValue::Need(arcweft_core::task::NeedId("need.borrowed-view".to_owned()))]),
            }];
            assert!(!runtime.evaluate(&handles, &supplied, false).diagnostics.is_empty());
            assert_eq!(runtime.snapshot().unwrap(), before);
            assert!(matches!(&supplied[0].value, RuntimeValue::Tuple(values) if matches!(values.first(), Some(RuntimeValue::Need(_)))));
        }
        {
            let before = runtime.snapshot().unwrap();
            let mut forged = before.clone();
            forged.mounts[0].runtime_parameters.iter_mut().find(|binding| binding.name == "value").unwrap().value = RuntimeValue::Bool(true);
            assert!(runtime.restore(&forged, &handles).is_err());
            assert_eq!(runtime.snapshot().unwrap(), before);
        }
        if case >= 5 {
            let supplied = [RuntimeBinding { name: "first".to_owned(), value: RuntimeValue::Int(arcweft_core::value::RuntimeInt::i64(2)) }, RuntimeBinding { name: "value".to_owned(), value: RuntimeValue::Bool(false) }];
            assert!(!runtime.evaluate(&handles, &supplied, false).diagnostics.is_empty());
        }
    }
}

#[test]
fn nominal_defaults_keep_distinct_scope_widths_and_share_equal_scopes() {
    use arcweft_core::awbc::schema::AwbcRuntimeTypeShape;
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
struct Holder<T> { callback: T }
view Narrow(first: Holder<i64 -> i64> = Holder { callback = |input: i64| input + 1 }) { Text("narrow") }
view Wide(first: Holder<i64 -> i64> = Holder { callback = |input: i64| input + 1 }, extra: i64 -> i64 = |input: i64| input + 2) { Text("wide") }
view Mirror(first: Holder<i64 -> i64> = Holder { callback = |input: i64| input + 1 }) { Text("mirror") }
"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://nominal-scope-widths")
        .compile()
        .unwrap();
    let awbc = AwbcLowerer::new(
        &compiled.runtime_plan().plan,
        &compiled.runtime_plan().dialogue_content_catalog,
        "main.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let awbc =
        Arc::new(arcweft_bundle::standard_view::install_dialogue_handler_awbc(awbc).unwrap());
    let product = compiled.view_product().product();
    let resource = product.program().unwrap().resource();
    let parameter = |name: &str| {
        let definition = resource
            .definitions
            .iter()
            .find(|definition| definition.public_id.as_str() == name)
            .unwrap();
        let contract = awbc
            .semantic_type_id(definition.parameter_contract.unwrap())
            .unwrap();
        let AwbcRuntimeTypeShape::Function {
            contract: header,
            parameters,
            ..
        } = awbc.runtime_types[contract.index()].shape()
        else {
            panic!("parameter contract")
        };
        (header.binder().effects(), parameters[0])
    };
    let narrow = parameter("view.Narrow");
    let wide = parameter("view.Wide");
    let mirror = parameter("view.Mirror");
    assert_eq!(narrow.0, 1);
    assert_eq!(wide.0, 2);
    assert_eq!(narrow, mirror);
    assert_ne!(narrow.1, wide.1);
    assert_ne!(
        awbc.runtime_types[narrow.1.index()].semantic_identity(),
        awbc.runtime_types[wide.1.index()].semantic_identity()
    );
    assert_eq!(
        awbc.runtime_types[narrow.1.index()].nominal_declaration(),
        awbc.runtime_types[wide.1.index()].nominal_declaration()
    );
    let handles = ["Narrow", "Wide", "Mirror"]
        .into_iter()
        .enumerate()
        .map(|(ordinal, name)| {
            PresentationHandleRecord::new(
                PresentationHandleId::try_new(format!("view.scope.{ordinal}")).unwrap(),
                PresentationHandleKind::View,
                format!("view.{name}"),
                None,
                PresentationResourceState::Mounted,
                None,
                0,
            )
        })
        .collect::<Vec<_>>();
    let mut runtime = BundleViewRuntime::try_new_with_awbc(
        product.as_ref().clone(),
        compiled.view_product().text().cloned(),
        awbc,
    )
    .unwrap();
    let frame = runtime.evaluate(&handles, &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    assert_eq!(runtime.snapshot().unwrap().mounts.len(), 3);
}

#[test]
fn nominal_default_constructed_from_scoped_input_keeps_its_origin() {
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
struct Holder<T> { callback: T }
view Main(first: i64 -> i64 = |input: i64| input + 1,
          value: Holder<i64 -> i64> = Holder { callback = first },
          copied: Holder<i64 -> i64> = value) { Text("static") }
"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://nominal-origin")
        .compile()
        .unwrap();
    let awbc = AwbcLowerer::new(
        &compiled.runtime_plan().plan,
        &compiled.runtime_plan().dialogue_content_catalog,
        "main.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let awbc =
        Arc::new(arcweft_bundle::standard_view::install_dialogue_handler_awbc(awbc).unwrap());
    let mut runtime = BundleViewRuntime::try_new_with_awbc(
        compiled.view_product().product().as_ref().clone(),
        compiled.view_product().text().cloned(),
        Arc::clone(&awbc),
    )
    .unwrap();
    let handle = PresentationHandleRecord::new(
        PresentationHandleId::try_new("view.origin").unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let frame = runtime.evaluate(&[handle], &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    let snapshot = runtime.snapshot().unwrap();
    let parameters = &snapshot.mounts[0].runtime_parameters;
    let first = &parameters
        .iter()
        .find(|value| value.name == "first")
        .unwrap()
        .value;
    let value = &parameters
        .iter()
        .find(|value| value.name == "value")
        .unwrap()
        .value;
    let arcweft_core::value::RuntimeValue::NominalRecord(record) = value else {
        panic!("nominal default: {value:?}")
    };
    assert_eq!(record.fields(), [first.clone()]);
    let ty = awbc.semantic_type_id(record.semantic_identity()).unwrap();
    assert!(!awbc.runtime_types[ty.index()].scope().is_root());
    assert!(record.type_instantiation().is_some());
    assert_eq!(
        value,
        &parameters
            .iter()
            .find(|value| value.name == "copied")
            .unwrap()
            .value
    );
    let owner = arcweft_core::task::RuntimeProgramOwner::Awbc(Arc::clone(&awbc));
    let image = arcweft_core::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
        value, &owner,
    )
    .unwrap();
    let bytes = serde_json::to_vec(&image).unwrap();
    let decoded: arcweft_core::value::AwbcRuntimeValueSnapshot =
        serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        decoded.into_runtime_value_for_program(&owner).unwrap(),
        *value
    );
    let arcweft_core::value::AwbcRuntimeValueSnapshot::NominalRecord(record_image) = image else {
        panic!("record snapshot")
    };
    let mut missing = record_image.clone();
    missing.type_instantiation = None;
    assert!(
        arcweft_core::value::AwbcRuntimeValueSnapshot::NominalRecord(missing)
            .into_runtime_value_for_program(&owner)
            .is_err()
    );
    for (field, replacement) in [
        ("effects", serde_json::json!([])),
        ("context", serde_json::json!(vec![0xa9; 32])),
    ] {
        let mut forged = record_image.clone();
        let mut binding =
            serde_json::to_value(forged.type_instantiation.as_ref().unwrap()).unwrap();
        binding[field] = replacement;
        forged.type_instantiation = Some(serde_json::from_value(binding).unwrap());
        assert!(
            arcweft_core::value::AwbcRuntimeValueSnapshot::NominalRecord(forged)
                .into_runtime_value_for_program(&owner)
                .is_err()
        );
    }
    let definition = compiled
        .view_product()
        .product()
        .program()
        .unwrap()
        .resource()
        .definitions
        .iter()
        .find(|definition| definition.public_id.as_str() == "view.Main")
        .unwrap();
    let default = |name: &str| {
        definition
            .parameters
            .iter()
            .find(|parameter| parameter.name == name)
            .unwrap()
            .default_program
            .as_ref()
            .unwrap()
            .program
    };
    let native_plan = Arc::new(compiled.runtime_plan().plan.clone());
    let finish = |program, inputs| {
        let mut engine = arcweft_core::engine::Engine::for_program_invocation(
            Arc::clone(&native_plan),
            program,
            inputs,
        )
        .unwrap();
        for _ in 0..64 {
            let output = engine.step(Default::default(), Default::default()).output;
            assert!(output.diagnostics.is_empty(), "{output:?}");
            if let Some((_, value)) = engine.take_program_result().unwrap() {
                return value;
            }
        }
        panic!("native nominal default exceeded its deterministic step limit")
    };
    let native_first = finish(default("first"), vec![]);
    let native_value = finish(default("value"), vec![native_first.clone()]);
    let arcweft_core::value::RuntimeValue::NominalRecord(record) = &native_value else {
        panic!("native nominal record")
    };
    assert_eq!(record.fields(), [native_first]);
    assert!(record.type_instantiation().is_some());
    assert_eq!(
        finish(default("copied"), vec![native_value.clone()]),
        native_value
    );
}

#[test]
fn nominal_enum_default_constructed_from_scoped_input_keeps_its_origin() {
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
enum Slot<T> { Empty, Full T }
view Main(first: i64 -> i64 = |input: i64| input + 1,
          value: Slot<i64 -> i64> = .Full(first),
          copied: Slot<i64 -> i64> = value) { Text("static") }
"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://nominal-enum-origin")
        .compile()
        .unwrap();
    let awbc = AwbcLowerer::new(
        &compiled.runtime_plan().plan,
        &compiled.runtime_plan().dialogue_content_catalog,
        "main.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let awbc =
        Arc::new(arcweft_bundle::standard_view::install_dialogue_handler_awbc(awbc).unwrap());
    let mut runtime = BundleViewRuntime::try_new_with_awbc(
        compiled.view_product().product().as_ref().clone(),
        compiled.view_product().text().cloned(),
        Arc::clone(&awbc),
    )
    .unwrap();
    let handle = PresentationHandleRecord::new(
        PresentationHandleId::try_new("view.enum-origin").unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let frame = runtime.evaluate(&[handle], &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    let snapshot = runtime.snapshot().unwrap();
    let parameters = &snapshot.mounts[0].runtime_parameters;
    let value = &parameters
        .iter()
        .find(|value| value.name == "value")
        .unwrap()
        .value;
    let copied = &parameters
        .iter()
        .find(|value| value.name == "copied")
        .unwrap()
        .value;
    assert_eq!(value, copied);
    let arcweft_core::value::RuntimeValue::Variant {
        owner:
            arcweft_core::pattern::RuntimeVariantIdentity::Nominal {
                semantic_identity, ..
            },
        type_instantiation: Some(origin),
        ..
    } = value
    else {
        panic!("scoped enum origin")
    };
    assert!(
        !awbc.runtime_types[awbc.semantic_type_id(*semantic_identity).unwrap().index()]
            .scope()
            .is_root()
    );
    let arcweft_core::value::RuntimeValue::Variant {
        type_instantiation: Some(copied_origin),
        ..
    } = copied
    else {
        panic!("transferred enum origin")
    };
    assert!(Arc::ptr_eq(origin, copied_origin));
    let owner = arcweft_core::task::RuntimeProgramOwner::Awbc(Arc::clone(&awbc));
    let image = arcweft_core::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(
        value, &owner,
    )
    .unwrap();
    let decoded: arcweft_core::value::AwbcRuntimeValueSnapshot =
        serde_json::from_slice(&serde_json::to_vec(&image).unwrap()).unwrap();
    assert_eq!(
        decoded.into_runtime_value_for_program(&owner).unwrap(),
        *value
    );
    let mut missing = image.clone();
    let arcweft_core::value::AwbcRuntimeValueSnapshot::Variant {
        type_instantiation, ..
    } = &mut missing
    else {
        panic!("enum snapshot")
    };
    *type_instantiation = None;
    assert!(missing.into_runtime_value_for_program(&owner).is_err());
    for (field, replacement) in [
        ("effects", serde_json::json!([])),
        ("context", serde_json::json!(vec![0xa9; 32])),
    ] {
        let mut forged = image.clone();
        let arcweft_core::value::AwbcRuntimeValueSnapshot::Variant {
            type_instantiation: Some(binding),
            ..
        } = &mut forged
        else {
            panic!("enum binding snapshot")
        };
        let mut altered = serde_json::to_value(&*binding).unwrap();
        altered[field] = replacement;
        *binding = serde_json::from_value(altered).unwrap();
        assert!(forged.into_runtime_value_for_program(&owner).is_err());
    }
    let definition = compiled
        .view_product()
        .product()
        .program()
        .unwrap()
        .resource()
        .definitions
        .iter()
        .find(|definition| definition.public_id.as_str() == "view.Main")
        .unwrap();
    let default = |name: &str| {
        definition
            .parameters
            .iter()
            .find(|parameter| parameter.name == name)
            .unwrap()
            .default_program
            .as_ref()
            .unwrap()
            .program
    };
    let native_plan = Arc::new(compiled.runtime_plan().plan.clone());
    let finish = |program, inputs| {
        let mut engine = arcweft_core::engine::Engine::for_program_invocation(
            Arc::clone(&native_plan),
            program,
            inputs,
        )
        .unwrap();
        for _ in 0..64 {
            let output = engine.step(Default::default(), Default::default()).output;
            assert!(output.diagnostics.is_empty(), "{output:?}");
            if let Some((_, value)) = engine.take_program_result().unwrap() {
                return value;
            }
        }
        panic!("native enum default exceeded its deterministic step limit")
    };
    let first = finish(default("first"), vec![]);
    let native = finish(default("value"), vec![first.clone()]);
    let arcweft_core::value::RuntimeValue::Variant {
        payload: Some(payload),
        type_instantiation: Some(_),
        ..
    } = &native
    else {
        panic!("native enum origin")
    };
    assert_eq!(
        payload.as_ref(),
        &arcweft_core::value::RuntimeValue::Tuple(vec![first])
    );
    assert_eq!(finish(default("copied"), vec![native.clone()]), native);
}

#[test]
fn compiler_rejects_general_view_calls_at_the_unimplemented_runtime_boundary() {
    let cases = ["view Child(value: i32) { Text(value) }\nview Main() { Child(1i32) }\n"];

    for source in cases {
        let fixture = project_view_fixture(
            source,
            "arcweft-test://compiler-view-call-and-default-rejection",
        );
        let error = fixture.compile().expect_err(
            "checked View semantics must fail closed before unsupported runtime lowering",
        );
        assert_eq!(
            error.diagnostics().len(),
            1,
            "unexpected diagnostics: {error:?}"
        );
        let diagnostic = &error.diagnostics()[0];
        assert_eq!(diagnostic.stage(), ProjectCompileStage::ViewLower);
        assert_eq!(
            diagnostic
                .diagnostic()
                .code()
                .map(arcweft_source::DiagnosticCode::as_str),
            Some("compiler.view.lower")
        );
        assert!(diagnostic.source().is_none());
        assert!(diagnostic.diagnostic().labels().is_empty());
    }
}

#[test]
fn compiler_rejects_every_unknown_text_control_and_scroll_policy_symbol() {
    for (_policy, value, authored) in unknown_policy_cases() {
        let source = format!("view Broken() {{\n  {authored}\n}}\n");
        let fixture = project_view_fixture(
            &source,
            &format!("arcweft-test://compiler-view-unknown-{value}"),
        );
        let error = fixture
            .compile()
            .expect_err("an explicitly authored typo must not lower as a default policy");
        assert!(matches!(
            error.diagnostics()[0].stage(),
            ProjectCompileStage::Parse | ProjectCompileStage::TypeCheck
        ));
        let diagnostic = error.diagnostics()[0].diagnostic();
        assert!(diagnostic.code().is_some());
    }
}

fn unknown_policy_cases() -> [(&'static str, &'static str, &'static str); 12] {
    [
        (
            "text input purpose",
            "serch",
            "TextField(\"value\")\n    .purpose(\"serch\")",
        ),
        (
            "enter-key hint",
            "snd",
            "TextField(\"value\")\n    .enter_key(\"snd\")",
        ),
        (
            "text selection",
            "enabeld",
            "TextField(\"value\", selection = enabeld)",
        ),
        (
            "text shortcut",
            "enabeld",
            "TextField(\"value\", shortcuts = enabeld)",
        ),
        (
            "Tab-key",
            "insert_tba",
            "TextField(\"value\", tab = insert_tba)",
        ),
        (
            "vertical navigation",
            "visaul",
            "TextField(\"value\", vertical_navigation = visaul)",
        ),
        (
            "secure input",
            "pasword",
            "SecureField(\"value\", secure_policy = pasword)",
        ),
        (
            "scroll overflow",
            "scrol",
            "Scroll(overflow = \"scrol\") {\n    Text(\"x\")\n  }",
        ),
        (
            "scroll axis",
            "vertcial",
            "Scroll(axis = \"vertcial\") {\n    Text(\"x\")\n  }",
        ),
        (
            "scroll indicators",
            "visble",
            "Scroll(indicators = \"visble\") {\n    Text(\"x\")\n  }",
        ),
        (
            "scroll overscroll",
            "elstic",
            "Scroll(overscroll = \"elstic\") {\n    Text(\"x\")\n  }",
        ),
        (
            "scroll focus",
            "nerest",
            "Scroll(auto_scroll_focus = \"nerest\") {\n    Text(\"x\")\n  }",
        ),
    ]
}

struct ProjectViewFixture {
    project: ProjectSources,
    document: Arc<SourceDocument>,
    context: ProjectCompilationContext,
}

impl ProjectViewFixture {
    fn compile(&self) -> Result<CompiledProject, ProjectCompileError> {
        compile_attached_project(&self.project, &self.context)
    }
}

fn project_view_fixture(source: &str, source_id: &str) -> ProjectViewFixture {
    project_view_fixture_with_selection(source, source_id, None)
}

fn project_view_fixture_with_entry(source: &str, source_id: &str) -> ProjectViewFixture {
    project_view_fixture_with_selection(
        source,
        source_id,
        Some(ProjectEntrySelection::new(
            PublicId::try_new("entry.main").expect("fixture Entry ID"),
            ProjectEntrySelectionKind::Cli,
        )),
    )
}

fn project_view_fixture_with_selection(
    source: &str,
    source_id: &str,
    entry_selection: Option<ProjectEntrySelection>,
) -> ProjectViewFixture {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(source_id).expect("source ID"),
            SourceName::path("main.arcw"),
            source,
        )
        .expect("source document"),
    );
    let module = CanonicalModulePath::crate_root();
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new("local.arcweft.view-diagnostic").expect("package ID"),
            version: PackageVersion::new("0.0.0").expect("package version"),
        },
        BuildSpec::default(),
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://compile-project-view-manifest")
                    .expect("manifest source ID"),
                SourceName::path("arcw.toml"),
                "",
            )
            .expect("manifest document"),
        ),
        vec![ProjectSourceFile::new(
            module,
            PathBuf::from("main.arcw"),
            Arc::clone(&document),
            [],
        )],
    )
    .expect("project sources");
    let package =
        CallablePackageId::try_new(project.package().id.as_str()).expect("callable package ID");
    let world = ProjectSymbolWorldId::try_new(
        package,
        document.identity().id().clone(),
        "compiler-view-product-test",
    )
    .expect("symbol world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        None,
        entry_selection,
    );
    ProjectViewFixture {
        project,
        document,
        context,
    }
}

fn minimal_dialogue_frame(view: ViewId) -> LineDisplayFrame {
    LineDisplayFrame {
        line: RuntimeLineId::from_runtime_line_value("say.compiler.view.handler")
            .expect("runtime line identity"),
        character: DialoguePresentationCharacter {
            id: CharacterId::try_new("character.compiler").expect("dialogue character identity"),
            display_name: "Compiler".to_owned(),
        },
        text_key: TextKey::try_new("text.compiler.view.handler").expect("dialogue text key"),
        effective: CharacterDialoguePresentationConfig {
            view,
            style_sheet: None,
            voice: None,
            look: None,
            stage: None,
            portrait: None,
            focus: None,
            cleanup: None,
            source_locale: None,
            hooks: Vec::new(),
            inline_failure: InlineFailurePolicy::FailLine,
            custom: BTreeMap::new(),
            config_digest: RuntimeValueDigest::ZERO,
        },
        text: String::new(),
        base_styles: Vec::new(),
        style_contributions: Vec::new(),
        nodes: Vec::new(),
        display_map: RichTextDisplayMap::default(),
        host_events: Vec::new(),
        inline_failures: Vec::new(),
        unresolved: Vec::new(),
        content: arcweft_core::value::RuntimeDialogueContentValue::try_new(
            arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x71; 32])
                .expect("fixture artifact"),
            arcweft_core::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
                .expect("fixture template"),
            arcweft_core::entry::RuntimeDialogueContentTemplateDigest::from_bytes([0x72; 32]),
            [],
        )
        .expect("fixture Content envelope"),
    }
}

fn canonical_view_project_fixture() -> (
    ProjectSources,
    ProjectCompilationContext,
    Arc<SourceDocument>,
    Arc<SourceDocument>,
    Arc<SourceDocument>,
) {
    let a_module =
        CanonicalModulePath::from_segments(
            [ModuleSegment::new("a").expect("valid module segment")],
        );
    let z_module =
        CanonicalModulePath::from_segments(
            [ModuleSegment::new("z").expect("valid module segment")],
        );
    let root_document = canonical_view_document(
        "arcweft-test://canonical-view/root",
        "src/main.arcw",
        "pub view RootFirst() { Text(\"root first\") }\n\
         pub view RootSecond() { Text(\"root second\") }\n",
    );
    let a_document = canonical_view_document(
        "arcweft-test://canonical-view/a",
        "src/a.arcw",
        "mod a\n\npub view Card() { Text(\"a\") }\n",
    );
    let z_document = canonical_view_document(
        "arcweft-test://canonical-view/z",
        "src/z.arcw",
        "mod z\n\npub view Card() { Text(\"z\") }\npub view @view.authored Named() { Text(\"authored\") }\n",
    );
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new("local.arcweft.canonical-view").expect("package ID"),
            version: PackageVersion::new("0.0.0").expect("package version"),
        },
        BuildSpec::default(),
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://canonical-view/manifest")
                    .expect("manifest source ID"),
                SourceName::path("arcw.toml"),
                "",
            )
            .expect("manifest document"),
        ),
        [
            ProjectSourceFile::new(
                z_module.clone(),
                PathBuf::from("src/z.arcw"),
                Arc::clone(&z_document),
                [],
            ),
            ProjectSourceFile::new(
                CanonicalModulePath::crate_root(),
                PathBuf::from("src/main.arcw"),
                Arc::clone(&root_document),
                [
                    ModuleDependency::new(z_module),
                    ModuleDependency::new(a_module.clone()),
                ],
            ),
            ProjectSourceFile::new(
                a_module,
                PathBuf::from("src/a.arcw"),
                Arc::clone(&a_document),
                [],
            ),
        ],
    )
    .expect("canonical View project sources");
    let package =
        CallablePackageId::try_new(project.package().id.as_str()).expect("callable package ID");
    let world = ProjectSymbolWorldId::try_new(
        package,
        root_document.identity().id().clone(),
        "canonical-view-project-test",
    )
    .expect("symbol world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        project
            .modules()
            .map(|source| Arc::clone(source.document()))
            .collect::<Vec<_>>(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        None,
        None,
    );
    (project, context, root_document, a_document, z_document)
}

fn canonical_view_document(id: &str, path: &str, source: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("source ID"),
            SourceName::path(path),
            source,
        )
        .expect("canonical View source document"),
    )
}

fn source_text<'a>(document: &'a SourceDocument, span: &arcweft_source::SourceSpan) -> &'a str {
    span.validate_for(document)
        .expect("span belongs to fixture");
    let range = span.range();
    &document.text()[range.start()..range.end()]
}

#[test]
fn authored_expression_programs_keep_distinct_operation_roots() {
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main(label: String = "seed") {
    Text(label)
    Text(label)
    Button(label, enabled = true)
}
"#;
    let compiled =
        project_view_fixture_with_entry(source, "arcweft-test://view-expression-sharing")
            .compile()
            .expect("equivalent expressions retain their accepted roots");
    use arcweft_bundle::resource_codec::view::ViewTextSourceKind;
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().unwrap().clone();
    let expressions = text
        .sources
        .iter()
        .filter_map(|source| match &source.kind {
            ViewTextSourceKind::Program { program } => Some(program),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(expressions.len(), 3);
    assert_eq!(
        expressions
            .iter()
            .map(|expression| expression.program)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    for expression in &expressions[1..] {
        assert_eq!(expression.inputs, expressions[0].inputs);
        assert_eq!(expression.result_type, expressions[0].result_type);
    }
    let awbc = AwbcLowerer::new(
        &compiled.runtime_plan().plan,
        &compiled.runtime_plan().dialogue_content_catalog,
        "main.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let awbc =
        Arc::new(arcweft_bundle::standard_view::install_dialogue_handler_awbc(awbc).unwrap());
    product
        .program()
        .unwrap()
        .resource()
        .validate_awbc_programs(&awbc, Some(&text))
        .unwrap();
    for expression in &expressions {
        assert!(awbc.pure_program_binding(expression.program).is_some());
    }
    let mut runtime = BundleViewRuntime::try_new_with_awbc(
        product.clone(),
        Some(text.clone()),
        Arc::clone(&awbc),
    )
    .unwrap();
    let handle = PresentationHandleRecord::new(
        PresentationHandleId::try_new("view.expression.roots").unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let frame = runtime.evaluate(std::slice::from_ref(&handle), &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:?}");
    assert_eq!(frame.mounts[0].text.len(), 2);
    for output in &frame.mounts[0].text {
        assert!(
            matches!(&output.value, arcweft_runtime_driver::view_runtime::BundleViewTextValue::Plain { value } if value == "seed")
        );
    }
    assert_eq!(frame.mounts[0].action_buttons[0].label, "seed");
    let snapshot = runtime.snapshot().unwrap();
    let mut restored = BundleViewRuntime::try_new_with_awbc(product, Some(text), awbc).unwrap();
    restored
        .restore(&snapshot, std::slice::from_ref(&handle))
        .unwrap();
    let cold = restored.evaluate(&[handle], &[], false);
    assert!(cold.diagnostics.is_empty(), "{cold:?}");
    assert_eq!(frame, cold);
}

#[test]
fn authored_element_geometry_inputs_reach_typed_layout() {
    use arcweft_presentation::appearance::PresentationEnvironment;
    use arcweft_view::geometry::*;
    use arcweft_view::{ViewMountId, style::*};
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main() { Panel(x = 12px, y = 34px, width = 200px, height = 90px) }
view Other() { Panel(width = 17px, height = 18px) }
style Primary { Panel { color = rgba(10, 20, 30, 255) } }
"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://view-element-geometry")
        .compile()
        .expect("checked element geometry compiles");
    let product = compiled.view_product().product();
    let program = product.program().unwrap().resource();
    let style = product.style().unwrap().resource();
    assert_eq!(
        arcweft_bundle::resource_codec::view::ViewStyleResource::decode_canonical_section(
            &style.encode_canonical_section().unwrap()
        )
        .unwrap(),
        *style
    );
    let main = program
        .definitions
        .iter()
        .find(|view| view.public_id.as_str() == "view.Main")
        .unwrap();
    let ViewProgramInstruction::OpenElement {
        element, styles, ..
    } = &program.instructions[main.body.start_instruction as usize]
    else {
        panic!("element instruction");
    };
    assert_eq!(styles.len(), 1);
    let ViewStyleApplicationTarget::Inline { patch } = styles[0] else {
        panic!("inline geometry");
    };
    let patch = style.inline_patch(patch).unwrap();
    assert_eq!(patch.declarations().len(), 5);
    for declaration in patch.declarations() {
        let range = &style.source_map_refs[declaration.source().value() as usize];
        let expected = match declaration.property() {
            ViewPropertyKind::Position | ViewPropertyKind::Left => "12px",
            ViewPropertyKind::Top => "34px",
            ViewPropertyKind::Width => "200px",
            ViewPropertyKind::Height => "90px",
            _ => panic!("geometry property"),
        };
        assert_eq!(
            &source[range.start_byte() as usize..range.end_byte() as usize],
            expected
        );
    }
    let mount = ViewMountId::from_raw(7);
    let key = ViewStyleNodeKey::new(mount, Vec::new(), main.body.start_instruction);
    let node = ViewStyleNodeFacts::new(Some(*element));
    let applications = [ViewStyleApplication::new(
        styles[0].clone(),
        ViewStyleScopeId::new(1),
        0,
        0,
        ViewStyleBoundaryFacts::SAME_VIEW,
    )];
    let environment = PresentationEnvironment::initial(
        arcweft_presentation::appearance::PresentationEnvironmentValues::ENGINE_DEFAULT,
    );
    let mut resolver = ViewStyleResolver::new(ViewStyleResolverLimits::default());
    let context = ViewStyleResolveContext {
        node_key: &key,
        node: &node,
        ancestors: &[],
        applications: &applications,
        parent: None,
        parent_node_key: None,
        inherited_axes: ViewInheritedBoxAxes::for_host_seed(
            mount,
            ViewBoxAxisSeedGeneration::INITIAL,
            ViewBoxAxisHostSeed::Default,
        ),
        axis_provider_participation: ViewAxisProviderParticipation::ProjectionOnly,
        environment: &environment,
        revisions: ViewStyleRevisionSet::default(),
        trace: ViewStyleTraceMode::Off,
    };
    let computed = resolver
        .resolve(&style.program, &context)
        .unwrap()
        .computed()
        .clone();
    let physical = computed.physical_box();
    assert_eq!(physical.position, ViewPosition::Absolute);
    let intrinsic = ViewIntrinsicMeasure {
        content_size: ViewGeometrySize::new(0, 0),
        revision: ViewIntrinsicMeasureRevision::new(1),
    };
    let measured = measure_box(&key, &physical, intrinsic).unwrap();
    let placed = place_box(
        &key,
        &physical,
        measured,
        ViewGeometryRect::new(10_000, 20_000, 600_000, 500_000).unwrap(),
        ViewGeometryPoint::new(100_000, 100_000),
    )
    .unwrap();
    assert_eq!(
        placed.border_box,
        ViewGeometryRect::new(22_000, 54_000, 222_000, 144_000).unwrap()
    );
    assert!(
        resolver
            .resolve(&style.program, &context)
            .unwrap()
            .cache_hit()
    );
    let other = program
        .definitions
        .iter()
        .find(|view| view.public_id.as_str() == "view.Other")
        .unwrap();
    let ViewProgramInstruction::OpenElement {
        styles: other_styles,
        ..
    } = &program.instructions[other.body.start_instruction as usize]
    else {
        panic!("other element");
    };
    assert_ne!(styles, other_styles, "each element owns its patch");
}

#[test]
fn authored_element_geometry_refuses_units_without_a_layout_conversion() {
    for value in ["2pt", "2em", "2e0em"] {
        let source = format!(
            "entry cli @entry.main {{ goto @flow.main }}\nflow main() -> String {{ return \"done\" }}\nview Main() {{ Panel(width = {value}) }}"
        );
        let error = project_view_fixture_with_entry(&source, "arcweft-test://view-geometry-unit")
            .compile()
            .unwrap_err();
        assert_eq!(error.stage(), "view-lower");
    }
}

#[test]
fn authored_button_inputs_use_typed_expression_execution() {
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    for head in [
        "Button(identity(label), enabled = enabled)",
        "Button(identity(label), enabled)",
        "Button(enabled = enabled, label = identity(label))",
        "Button(identity(label), width = 200px, y = 12px, enabled = enabled)",
    ] {
        let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
fn identity(value: String) -> String { value }
view Main(label: String = "hello", enabled: bool = true) {
    BUTTON_INPUT_HEAD
}
"#;
        let source = source.replace("BUTTON_INPUT_HEAD", head);
        let compiled =
            project_view_fixture_with_entry(&source, "arcweft-test://view-button-expression")
                .compile()
                .expect("typed Button arguments compile");
        let product = compiled.view_product().product().as_ref().clone();
        let text = compiled.view_product().text().cloned();
        let awbc = AwbcLowerer::new(
            &compiled.runtime_plan().plan,
            &compiled.runtime_plan().dialogue_content_catalog,
            "main.arcw",
        )
        .lower()
        .unwrap()
        .program;
        let awbc =
            Arc::new(arcweft_bundle::standard_view::install_dialogue_handler_awbc(awbc).unwrap());
        let resource = product.program().unwrap().resource();
        assert_eq!(
            ViewProgramResource::decode_canonical_section(
                &resource.encode_canonical_section().unwrap()
            )
            .unwrap(),
            *resource
        );
        let button = resource
            .action_buttons
            .iter()
            .find(|button| button.view.as_deref() == Some("view.Main"))
            .unwrap();
        assert_eq!(
            matches!(
                button.inputs[0],
                arcweft_bundle::resource_codec::view::ViewActionButtonInput::Enabled { .. }
            ),
            head.starts_with("Button(enabled")
        );
        for forged_kind in 0..5 {
            let mut forged = resource.clone();
            let button = forged
                .action_buttons
                .iter_mut()
                .find(|button| button.view.as_deref() == Some("view.Main"))
                .unwrap();
            match forged_kind {
                0 => {
                    let label = text
                        .as_ref()
                        .unwrap()
                        .sources
                        .iter()
                        .find(|source| Some(source.public_id.as_str()) == button.label_source())
                        .unwrap();
                    let arcweft_bundle::resource_codec::view::ViewTextSourceKind::Program {
                        program,
                    } = &label.kind
                    else {
                        panic!("authored label program")
                    };
                    let enabled = button.inputs.iter_mut().find(|input| matches!(input, arcweft_bundle::resource_codec::view::ViewActionButtonInput::Enabled { .. })).unwrap();
                    *enabled =
                        arcweft_bundle::resource_codec::view::ViewActionButtonInput::Enabled {
                            value: arcweft_view::ViewExpressionValue::Program {
                                program: program.clone(),
                            },
                        };
                }
                1 => button.view = Some("view.missing".to_owned()),
                2 => {
                    button.inputs =
                        vec![button.inputs[0].clone(), button.inputs[0].clone()].into_boxed_slice()
                }
                3 => button.inputs = vec![button.inputs[0].clone()].into_boxed_slice(),
                _ => {
                    let target = button.public_id.clone();
                    let instruction = forged.instructions.iter_mut().find(|instruction| matches!(instruction, ViewProgramInstruction::OpenElement { target: Some(candidate), .. } if candidate == &target)).unwrap();
                    let ViewProgramInstruction::OpenElement { element, .. } = instruction else {
                        unreachable!()
                    };
                    *element = arcweft_view::ViewElementKind::Box;
                }
            }
            assert!(
                forged.validate_awbc_programs(&awbc, text.as_ref()).is_err(),
                "control forgery {forged_kind}"
            );
        }
        let mut runtime =
            BundleViewRuntime::try_new_with_awbc(product.clone(), text.clone(), Arc::clone(&awbc))
                .unwrap();
        let handles = [PresentationHandleRecord::new(
            PresentationHandleId::try_new("view.button.expression").unwrap(),
            PresentationHandleKind::View,
            "view.Main".to_owned(),
            None,
            PresentationResourceState::Mounted,
            None,
            0,
        )];
        for (label, enabled) in [
            ("hello", true),
            ("changed", false),
            ("changed", false),
            ("again", true),
        ] {
            let bindings = [
                RuntimeBinding {
                    name: "label".to_owned(),
                    value: RuntimeValue::String(label.to_owned()),
                },
                RuntimeBinding {
                    name: "enabled".to_owned(),
                    value: RuntimeValue::Bool(enabled),
                },
            ];
            let output = runtime.evaluate(&handles, &bindings, false);
            assert!(output.diagnostics.is_empty(), "{output:?}");
            assert_eq!(output.mounts[0].action_buttons.len(), 1);
            let button = &output.mounts[0].action_buttons[0];
            assert_eq!(button.label, label);
            assert_eq!(button.enabled, enabled);
        }
        let saved = runtime.snapshot().unwrap();
        let mut restored = BundleViewRuntime::try_new_with_awbc(product, text, awbc).unwrap();
        restored.restore(&saved, &handles).unwrap();
        let output = restored.evaluate(&handles, &[], false);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        assert_eq!(output.mounts[0].action_buttons[0].label, "again");
        assert!(output.mounts[0].action_buttons[0].enabled);
    }
}

#[test]
fn authored_text_expression_executes_and_refreshes_typed_parameter_inputs() {
    use arcweft_bundle::resource_codec::view::{ViewTextResource, ViewTextSourceKind};
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    use arcweft_runtime_driver::view_runtime::BundleViewTextValue;
    for expression in [
        "label",
        "identity(label)",
        "{ let inner = identity(label); inner }",
        "match enabled { true => identity(label), false => label }",
        "name.value",
        "(|value: String| identity(value))(label)",
    ] {
        let source = format!(
            r#"
entry cli @entry.main {{ goto @flow.main }}
flow main() -> String {{ return "done" }}
fn identity(value: String) -> String {{ value }}
struct Label {{ value: String }}
view Main(label: String = "hello", enabled: bool = true,
          name: Label = Label {{ value = label }},
          callback: String -> String = |value: String| value) {{ Text({expression}) }}
"#
        );
        let compiled =
            project_view_fixture_with_entry(&source, "arcweft-test://view-text-expression")
                .compile()
                .unwrap_or_else(|error| panic!("{expression}: {error:?}"));
        let awbc = AwbcLowerer::new(
            &compiled.runtime_plan().plan,
            &compiled.runtime_plan().dialogue_content_catalog,
            "main.arcw",
        )
        .lower()
        .unwrap()
        .program;
        let awbc =
            Arc::new(arcweft_bundle::standard_view::install_dialogue_handler_awbc(awbc).unwrap());
        let product = compiled.view_product().product().as_ref().clone();
        let text = compiled.view_product().text().cloned().unwrap();
        let resource = product.program().unwrap().resource();
        resource.validate_awbc_programs(&awbc, Some(&text)).unwrap();
        assert_eq!(
            ViewTextResource::decode_canonical_section(&text.encode_canonical_section().unwrap())
                .unwrap(),
            text
        );
        assert!(BundleViewRuntime::try_new(product.clone(), Some(text.clone())).is_err());
        let definition = resource
            .definitions
            .iter()
            .find(|definition| definition.public_id.as_str() == "view.Main")
            .unwrap();
        let binding = text
            .sources
            .iter()
            .find_map(|source| match &source.kind {
                ViewTextSourceKind::Program { program } => Some(program),
                _ => None,
            })
            .unwrap();
        for forgery in 0..5 {
            let mut forged = text.clone();
            let expression = forged
                .sources
                .iter_mut()
                .find_map(|source| match &mut source.kind {
                    ViewTextSourceKind::Program { program } => Some(program),
                    _ => None,
                })
                .unwrap();
            match forgery {
                0 => {
                    expression.program =
                        arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest(
                            [0xaf; 32],
                        )
                }
                1 => expression.result_type = definition.parameters[1].semantic_type,
                2 => {
                    expression.inputs[0] = arcweft_view::ViewParameterInput::new(
                        arcweft_view::ViewParameterCoordinate::try_from_index(999).unwrap(),
                        expression.inputs[0].value_type(),
                    )
                    .into()
                }
                3 => {
                    expression.inputs[0] = arcweft_view::ViewParameterInput::new(
                        expression.inputs[0].parameter().unwrap(),
                        definition.parameters[1].semantic_type,
                    )
                    .into()
                }
                _ => expression.inputs = vec![expression.inputs[0]; 2].into_boxed_slice(),
            }
            assert!(
                resource
                    .validate_awbc_programs(&awbc, Some(&forged))
                    .is_err(),
                "forgery {forgery}"
            );
            assert!(
                BundleViewRuntime::try_new_with_awbc(
                    product.clone(),
                    Some(forged),
                    Arc::clone(&awbc)
                )
                .is_err()
            );
        }
        let mut orphan = text.clone();
        let mut source = orphan
            .sources
            .iter()
            .find(|source| matches!(source.kind, ViewTextSourceKind::Program { .. }))
            .unwrap()
            .clone();
        source.public_id.push_str(".orphan");
        orphan.sources.push(source);
        assert!(
            resource
                .validate_awbc_programs(&awbc, Some(&orphan))
                .is_err()
        );
        let empty =
            ValidatedViewProduct::try_new(None, None, None, ViewProductValidationLimits::default())
                .unwrap();
        assert!(
            BundleViewRuntime::try_new_with_awbc(empty, Some(text.clone()), Arc::clone(&awbc))
                .is_err()
        );
        for forgery in 0..3 {
            let mut forged = awbc.as_ref().clone();
            let row = forged
                .pure_programs
                .iter()
                .position(|row| row.program == binding.program)
                .unwrap();
            let function = forged.pure_programs[row].function;
            match forgery {
                0 => {
                    let other = definition.parameters[1]
                        .default_program
                        .as_ref()
                        .unwrap()
                        .program;
                    forged.pure_programs[row].function =
                        forged.pure_program_binding(other).unwrap().function;
                }
                1 => forged.functions[function.index()].type_context = None,
                _ => {
                    let effects = arcweft_core::awbc::schema::AwbcEffectSetId(
                        u32::try_from(forged.effect_sets.len()).unwrap(),
                    );
                    let name = arcweft_core::awbc::schema::AwbcStringId(
                        u32::try_from(forged.strings.len()).unwrap(),
                    );
                    forged.strings.push("effect.external".to_owned());
                    forged
                        .effect_sets
                        .push(arcweft_core::awbc::schema::AwbcEffectSet {
                            effects: vec![name],
                        });
                    let signature = forged.functions[function.index()].signature;
                    forged.signatures[signature.index()].effects = effects;
                }
            }
            assert!(
                resource
                    .validate_awbc_programs(&forged, Some(&text))
                    .is_err(),
                "AWBC forgery {forgery}"
            );
        }
        let mut runtime = BundleViewRuntime::try_new_with_awbc(
            product.clone(),
            Some(text.clone()),
            Arc::clone(&awbc),
        )
        .unwrap();
        let handle = PresentationHandleRecord::new(
            PresentationHandleId::try_new("view.text.expression").unwrap(),
            PresentationHandleKind::View,
            "view.Main".to_owned(),
            None,
            PresentationResourceState::Mounted,
            None,
            0,
        );
        let handles = [handle];
        for label in ["hello", "changed", "changed"] {
            let bindings = [RuntimeBinding {
                name: "label".to_owned(),
                value: RuntimeValue::String(label.to_owned()),
            }];
            let output = runtime.evaluate(&handles, &bindings, false);
            assert!(output.diagnostics.is_empty(), "{expression}: {output:?}");
            assert_eq!(output.mounts.len(), 1);
            assert!(
                matches!(&output.mounts[0].text[0].value, BundleViewTextValue::Plain { value } if value == label)
            );
        }
        let saved = runtime.snapshot().unwrap();
        runtime.restore(&saved, &handles).unwrap();
        let output = runtime.evaluate(&handles, &[], false);
        assert!(output.diagnostics.is_empty(), "{expression}: {output:?}");
        assert!(
            matches!(&output.mounts[0].text[0].value, BundleViewTextValue::Plain { value } if value == "changed")
        );
        let native = Arc::new(compiled.runtime_plan().plan.clone());
        let execute = |program, inputs| {
            let mut engine = arcweft_core::engine::Engine::for_program_invocation(
                Arc::clone(&native),
                program,
                inputs,
            )
            .unwrap();
            for _ in 0..64 {
                let output = engine.step(Default::default(), Default::default()).output;
                assert!(output.diagnostics.is_empty(), "{output:?}");
                if let Some((_, value)) = engine.take_program_result().unwrap() {
                    return value;
                }
            }
            panic!("native expression exceeded deterministic step limit");
        };
        let mut values: Vec<RuntimeValue> = Vec::new();
        for parameter in &definition.parameters {
            let default = parameter.default_program.as_ref().unwrap();
            let inputs = default
                .inputs
                .iter()
                .map(|input| {
                    values[input.parameter().expect("parameter-only fixture").index()].clone()
                })
                .collect();
            values.push(execute(default.program, inputs));
        }
        let inputs = binding
            .inputs
            .iter()
            .map(|input| values[input.parameter().expect("parameter-only fixture").index()].clone())
            .collect();
        assert_eq!(
            execute(binding.program, inputs),
            RuntimeValue::String("hello".to_owned())
        );
    }
}

#[test]
fn authored_text_expression_cache_and_restore_preserve_budget_failure() {
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    use arcweft_runtime_driver::view_runtime::BundleViewDiagnosticCode;
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
fn identity(value: String) -> String { value }
view Main(label: String) { Text(identity(identity(identity(identity(identity(label)))))) }
"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://view-text-budget")
        .compile()
        .unwrap();
    let awbc = AwbcLowerer::new(
        &compiled.runtime_plan().plan,
        &compiled.runtime_plan().dialogue_content_catalog,
        "main.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let awbc =
        Arc::new(arcweft_bundle::standard_view::install_dialogue_handler_awbc(awbc).unwrap());
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().cloned();
    let mut warm =
        BundleViewRuntime::try_new_with_awbc(product.clone(), text.clone(), Arc::clone(&awbc))
            .unwrap();
    let mut cold = BundleViewRuntime::try_new_with_awbc(product, text, awbc).unwrap();
    let handles = (0..3000)
        .map(|index| {
            PresentationHandleRecord::new(
                PresentationHandleId::try_new(format!("view.text.budget.{index}")).unwrap(),
                PresentationHandleKind::View,
                "view.Main".to_owned(),
                None,
                PresentationResourceState::Mounted,
                None,
                0,
            )
        })
        .collect::<Vec<_>>();
    let bindings = [RuntimeBinding {
        name: "label".to_owned(),
        value: RuntimeValue::String("same".to_owned()),
    }];
    let first = warm.evaluate(&handles[..1000], &bindings, false);
    assert!(first.diagnostics.is_empty(), "{first:?}");
    let saved = warm.snapshot().unwrap();
    cold.restore(&saved, &handles[..1000]).unwrap();
    let warm_output = warm.evaluate(&handles, &bindings, false);
    let cold_output = cold.evaluate(&handles, &bindings, false);
    assert!(warm_output.diagnostics.iter().any(|diagnostic| diagnostic.code == BundleViewDiagnosticCode::EvaluationBudgetExceeded));
    assert_eq!(warm_output, cold_output);
    assert_eq!(warm.snapshot().unwrap(), cold.snapshot().unwrap());
}

#[test]
fn authored_text_expression_rejects_unproven_dynamic_callback_suspension() {
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main(label: String, callback: String -> String effects {}) { Text(callback(label)) }
"#;
    let error =
        project_view_fixture_with_entry(source, "arcweft-test://view-text-unproven-callback")
            .compile()
            .expect_err("a dynamic callback has no non-suspending execution proof");
    assert_eq!(
        error.diagnostics()[0].stage(),
        ProjectCompileStage::ViewLower
    );
    assert!(
        error.diagnostics()[0]
            .diagnostic()
            .message()
            .contains("may suspend")
    );
}

#[test]
fn authored_button_input_failures_preserve_authored_evaluation_order() {
    use arcweft_bundle::resource_codec::view::{ViewActionButtonInput, ViewTextSourceKind};
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    for head in [
        "Button(label = fail_text(zero), enabled = fail_bool(zero))",
        "Button(enabled = fail_bool(zero), label = fail_text(zero))",
    ] {
        let source = format!(
            r#"
entry cli @entry.main {{ goto @flow.main }}
flow main() -> String {{ return "done" }}
fn fail_bool(value: i64) -> bool {{ (1i64 / value) == 0i64 }}
fn fail_text(value: i64) -> String {{ match (1i64 / value) == 0i64 {{ true => "a", false => "b" }} }}
view Main(zero: i64) {{ {head} }}
"#
        );
        let compiled = project_view_fixture_with_entry(&source, "arcweft-test://view-button-order")
            .compile()
            .unwrap();
        let product = compiled.view_product().product().as_ref().clone();
        let text = compiled.view_product().text().cloned().unwrap();
        let resource = product.program().unwrap().resource();
        let button = resource
            .action_buttons
            .iter()
            .find(|button| button.view.as_deref() == Some("view.Main"))
            .unwrap();
        let first = match &button.inputs[0] {
            ViewActionButtonInput::Label { text_source } => {
                let source = text
                    .sources
                    .iter()
                    .find(|source| source.public_id == *text_source)
                    .unwrap();
                let ViewTextSourceKind::Program { program } = &source.kind else {
                    panic!("text program")
                };
                program.program
            }
            ViewActionButtonInput::Enabled { value } => value.program().unwrap().program,
        };
        let awbc = AwbcLowerer::new(
            &compiled.runtime_plan().plan,
            &compiled.runtime_plan().dialogue_content_catalog,
            "main.arcw",
        )
        .lower()
        .unwrap()
        .program;
        let awbc =
            Arc::new(arcweft_bundle::standard_view::install_dialogue_handler_awbc(awbc).unwrap());
        let mut runtime = BundleViewRuntime::try_new_with_awbc(product, Some(text), awbc).unwrap();
        let handles = [PresentationHandleRecord::new(
            PresentationHandleId::try_new("view.button.order").unwrap(),
            PresentationHandleKind::View,
            "view.Main".to_owned(),
            None,
            PresentationResourceState::Mounted,
            None,
            0,
        )];
        let bindings = [RuntimeBinding {
            name: "zero".to_owned(),
            value: RuntimeValue::i64(0),
        }];
        let frame = runtime.evaluate(&handles, &bindings, false);
        assert!(frame.mounts.is_empty(), "failed control publishes no mount");
        assert!(
            frame.diagnostics[0]
                .message
                .contains("did not complete purely"),
            "{frame:?}"
        );
        assert_eq!(frame.diagnostics.len(), 1, "{frame:?}");
        assert!(
            frame.diagnostics[0].message.contains(&first.to_string()),
            "{head}: {frame:?}"
        );
    }
}

#[test]
fn authored_view_block_locals_keep_patterns_scopes_and_cold_restore() {
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    use arcweft_runtime_driver::view_runtime::BundleViewTextValue;
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
struct Row { label: String, count: i64 }
view Main(label: String = "seed") {
    {
        let pair = (label, 42i64);
        let (inner, count) = pair;
        Text(inner);
        { let inner = "shadow"; Text(inner) };
        let Row { label: inner, count } = Row { label: inner, count };
        Text(inner);
        Button(inner, enabled = count == 42i64)
    }
}
"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://view-block-local")
        .compile()
        .expect("View locals execute through Core");
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().unwrap().clone();
    let resource = product.program().unwrap().resource();
    assert_eq!(
        ViewProgramResource::decode_canonical_section(
            &resource.encode_canonical_section().unwrap()
        )
        .unwrap(),
        *resource
    );
    let awbc = Arc::new(
        arcweft_bundle::standard_view::install_dialogue_handler_awbc(
            AwbcLowerer::new(
                &compiled.runtime_plan().plan,
                &compiled.runtime_plan().dialogue_content_catalog,
                "main.arcw",
            )
            .lower()
            .unwrap()
            .program,
        )
        .unwrap(),
    );
    resource.validate_awbc_programs(&awbc, Some(&text)).unwrap();
    for forgery in 0..8 {
        let mut forged = resource.clone();
        let mut forged_text = text.clone();
        let binding_positions = forged
            .instructions
            .iter()
            .enumerate()
            .filter_map(|(index, instruction)| {
                matches!(instruction, ViewProgramInstruction::BindLocal { .. }).then_some(index)
            })
            .collect::<Vec<_>>();
        let nested_local = forged_text
            .sources
            .iter()
            .filter_map(|source| match &source.kind {
                arcweft_bundle::resource_codec::view::ViewTextSourceKind::Program { program } => {
                    Some(program.inputs[0])
                }
                _ => None,
            })
            .nth(1)
            .unwrap();
        match forgery {
            0 => {
                let ViewProgramInstruction::BindLocal { program, .. } =
                    &mut forged.instructions[binding_positions[1]]
                else {
                    panic!("binding")
                };
                program.outputs.swap(0, 1);
            }
            1 => {
                let ViewProgramInstruction::BindLocal { program, .. } =
                    &mut forged.instructions[binding_positions[1]]
                else {
                    panic!("binding")
                };
                let arcweft_view::ViewExecutionInputSource::Local(ref mut local) =
                    program.execution.inputs[0].source
                else {
                    panic!("local")
                };
                local.output = u16::MAX;
            }
            2 => {
                let ViewProgramInstruction::BindLocal { program, .. } =
                    &mut forged.instructions[binding_positions[1]]
                else {
                    panic!("binding")
                };
                program.outputs = Box::new([]);
            }
            3 => {
                let ViewProgramInstruction::BindLocal { program, .. } =
                    &mut forged.instructions[binding_positions[1]]
                else {
                    panic!("binding")
                };
                program.outputs[0].value_type = program.outputs[1].value_type;
            }
            4 => {
                let ViewProgramInstruction::BindLocal { program, .. } =
                    &mut forged.instructions[binding_positions[1]]
                else {
                    panic!("binding")
                };
                program.outputs[1].coordinate = program.outputs[0].coordinate;
            }
            5 => {
                let target = forged_text
                    .sources
                    .iter_mut()
                    .filter_map(|source| match &mut source.kind {
                        arcweft_bundle::resource_codec::view::ViewTextSourceKind::Program {
                            program,
                        } => Some(program),
                        _ => None,
                    })
                    .nth(2)
                    .unwrap();
                target.inputs[0] = nested_local;
            }
            6 => {
                let ViewProgramInstruction::BindLocal { program, .. } =
                    &forged.instructions[binding_positions[1]]
                else {
                    panic!("binding")
                };
                let output = program.outputs[0];
                let ViewProgramInstruction::BindLocal { program, .. } =
                    &mut forged.instructions[binding_positions[0]]
                else {
                    panic!("binding")
                };
                program.execution.inputs[0].source =
                    arcweft_view::ViewExecutionInputSource::Local(output.coordinate);
            }
            _ => {
                let close = forged
                    .instructions
                    .iter()
                    .position(|instruction| matches!(instruction, ViewProgramInstruction::EndScope))
                    .unwrap();
                forged.instructions[close] = ViewProgramInstruction::BeginScope;
            }
        }
        assert!(
            forged
                .validate_awbc_programs(&awbc, Some(&forged_text))
                .is_err(),
            "forgery {forgery}"
        );
    }
    assert!(BundleViewRuntime::try_new(product.clone(), Some(text.clone())).is_err());
    let handle = PresentationHandleRecord::new(
        PresentationHandleId::try_new("view.block.locals").unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let mut runtime = BundleViewRuntime::try_new_with_awbc(
        product.clone(),
        Some(text.clone()),
        Arc::clone(&awbc),
    )
    .unwrap();
    for label in ["first", "second"] {
        let inputs = [RuntimeBinding {
            name: "label".to_owned(),
            value: RuntimeValue::String(label.to_owned()),
        }];
        let frame = runtime.evaluate(std::slice::from_ref(&handle), &inputs, false);
        assert!(frame.diagnostics.is_empty(), "{frame:?}");
        let values = frame.mounts[0]
            .text
            .iter()
            .map(|output| match &output.value {
                BundleViewTextValue::Plain { value } => value.as_str(),
                _ => panic!("plain text"),
            })
            .collect::<Vec<_>>();
        assert_eq!(values, [label, "shadow", label]);
        assert_eq!(frame.mounts[0].action_buttons[0].label, label);
        assert!(frame.mounts[0].action_buttons[0].enabled);
        let saved = runtime.snapshot().unwrap();
        let mut cold = BundleViewRuntime::try_new_with_awbc(
            product.clone(),
            Some(text.clone()),
            Arc::clone(&awbc),
        )
        .unwrap();
        cold.restore(&saved, std::slice::from_ref(&handle)).unwrap();
        assert_eq!(
            frame,
            cold.evaluate(std::slice::from_ref(&handle), &inputs, false)
        );
    }
}

#[test]
fn authored_view_conditionals_use_core_and_isolate_arm_locals() {
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    use arcweft_runtime_driver::view_runtime::BundleViewTextValue;
    for body in [
        "if active { Text(label) } else { Text(\"no\") }",
        "{ let visible = choose(active); if visible { let label = label; Text(label); } else if !visible { let label = \"no\"; Text(label); }; Text(\"tail\") }",
        "{ if active { Text(label); }; Text(\"tail\") }",
    ] {
        let source = format!(
            "entry cli @entry.main {{ goto @flow.main }}\nflow main() -> String {{ return \"done\" }}\nfn choose(value: bool) -> bool {{ value }}\nview Main(active: bool, label: String = \"yes\") {{ {body} }}"
        );
        let compiled = project_view_fixture_with_entry(&source, "arcweft-test://view-conditional")
            .compile()
            .expect("authored View conditional compiles");
        let product = compiled.view_product().product().as_ref().clone();
        let text = compiled.view_product().text().unwrap().clone();
        let resource = product.program().unwrap().resource();
        assert_eq!(
            ViewProgramResource::decode_canonical_section(
                &resource.encode_canonical_section().unwrap()
            )
            .unwrap(),
            *resource
        );
        let awbc = Arc::new(
            arcweft_bundle::standard_view::install_dialogue_handler_awbc(
                AwbcLowerer::new(
                    &compiled.runtime_plan().plan,
                    &compiled.runtime_plan().dialogue_content_catalog,
                    "main.arcw",
                )
                .lower()
                .unwrap()
                .program,
            )
            .unwrap(),
        );
        resource.validate_awbc_programs(&awbc, Some(&text)).unwrap();
        for forgery in 0..3 {
            let mut forged = resource.clone();
            let ViewProgramInstruction::Branch {
                condition,
                then_span,
                ..
            } = forged
                .instructions
                .iter_mut()
                .find(|instruction| matches!(instruction, ViewProgramInstruction::Branch { .. }))
                .unwrap()
            else {
                panic!("branch")
            };
            match forgery {
                0 => {
                    condition.program =
                        arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest(
                            [0xfc; 32],
                        )
                }
                1 => {
                    let string = text
                        .sources
                        .iter()
                        .find_map(|source| match &source.kind {
                            arcweft_bundle::resource_codec::view::ViewTextSourceKind::Program {
                                program,
                            } => Some(program),
                            _ => None,
                        })
                        .unwrap();
                    *condition = string.clone();
                }
                _ => *then_span = u32::MAX,
            }
            assert!(
                forged.validate_awbc_programs(&awbc, Some(&text)).is_err(),
                "forgery {forgery}"
            );
        }
        if body.contains("visible") {
            let mut forged = resource.clone();
            let branch_index = forged
                .instructions
                .iter()
                .position(|instruction| {
                    matches!(instruction, ViewProgramInstruction::Branch { .. })
                })
                .unwrap();
            let ViewProgramInstruction::Branch {
                condition,
                then_span,
                else_span,
                ..
            } = &forged.instructions[branch_index]
            else {
                panic!("branch")
            };
            let condition = condition.clone();
            let ranges = arcweft_view::ViewBranchRanges::try_from_spans(
                u32::try_from(branch_index).unwrap(),
                *then_span,
                *else_span,
                u32::try_from(forged.instructions.len()).unwrap(),
            )
            .unwrap();
            for instruction in &mut forged.instructions
                [ranges.then_range().start as usize..ranges.then_range().end as usize]
            {
                if matches!(
                    instruction,
                    ViewProgramInstruction::BeginScope | ViewProgramInstruction::EndScope
                ) {
                    *instruction = ViewProgramInstruction::Branch {
                        condition: condition.clone(),
                        then_span: 0,
                        else_span: None,
                        source: None,
                    };
                }
            }
            forged
                .encode_canonical_section()
                .expect("implicit arm scopes are independently valid");
            let mut forged_text = text.clone();
            let arm_program = forged_text
                .sources
                .iter()
                .find_map(|source| match &source.kind {
                    arcweft_bundle::resource_codec::view::ViewTextSourceKind::Program {
                        program,
                    } if program.inputs.iter().any(|input| {
                        matches!(
                            input.source,
                            arcweft_view::ViewExecutionInputSource::Local(_)
                        )
                    }) =>
                    {
                        Some(program.clone())
                    }
                    _ => None,
                })
                .unwrap();
            let source = forged_text.sources.last_mut().unwrap();
            source.kind = arcweft_bundle::resource_codec::view::ViewTextSourceKind::Program {
                program: arm_program,
            };
            assert_eq!(
                forged.validate_awbc_programs(&awbc, Some(&forged_text)),
                Err(
                    arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
                        "view_expression_program_binding"
                    )
                )
            );
            let mut crossed = resource.clone();
            let nested = crossed
                .instructions
                .iter()
                .enumerate()
                .skip(branch_index + 1)
                .find_map(|(index, instruction)| {
                    matches!(instruction, ViewProgramInstruction::Branch { .. }).then_some(index)
                })
                .unwrap();
            let oversized = u32::try_from(crossed.instructions.len() - nested - 1).unwrap();
            let ViewProgramInstruction::Branch {
                then_span,
                else_span,
                ..
            } = &mut crossed.instructions[nested]
            else {
                panic!("nested branch")
            };
            *then_span = oversized;
            *else_span = None;
            assert_eq!(
                crossed.encode_canonical_section(),
                Err(
                    arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
                        "view_control_flow_spans"
                    )
                )
            );
        }
        let handle = PresentationHandleRecord::new(
            PresentationHandleId::try_new("view.conditionals").unwrap(),
            PresentationHandleKind::View,
            "view.Main".to_owned(),
            None,
            PresentationResourceState::Mounted,
            None,
            0,
        );
        let mut runtime = BundleViewRuntime::try_new_with_awbc(
            product.clone(),
            Some(text.clone()),
            Arc::clone(&awbc),
        )
        .unwrap();
        for active in [true, false, true] {
            let inputs = [RuntimeBinding {
                name: "active".to_owned(),
                value: RuntimeValue::Bool(active),
            }];
            let frame = runtime.evaluate(std::slice::from_ref(&handle), &inputs, false);
            assert!(frame.diagnostics.is_empty(), "{frame:?}");
            let values = frame.mounts[0]
                .text
                .iter()
                .map(|output| match &output.value {
                    BundleViewTextValue::Plain { value } => value.as_str(),
                    _ => panic!("plain text"),
                })
                .collect::<Vec<_>>();
            let expected = if body.contains("else") {
                if active { vec!["yes"] } else { vec!["no"] }
            } else if active {
                vec!["yes"]
            } else {
                Vec::new()
            };
            let mut expected = expected;
            if body.contains("tail") {
                expected.push("tail");
            }
            assert_eq!(values, expected);
            let saved = runtime.snapshot().unwrap();
            let mut cold = BundleViewRuntime::try_new_with_awbc(
                product.clone(),
                Some(text.clone()),
                Arc::clone(&awbc),
            )
            .unwrap();
            cold.restore(&saved, std::slice::from_ref(&handle)).unwrap();
            assert_eq!(
                frame,
                cold.evaluate(std::slice::from_ref(&handle), &inputs, false)
            );
        }
    }
}

#[test]
fn authored_view_conditional_does_not_execute_the_unselected_arm_and_recovers_from_failure() {
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
fn fail(divisor: i64) -> String { match (1i64 / divisor) == 0i64 { true => "zero", false => "one" } }
view Main(active: bool, label: String = "yes", divisor: i64 = 0i64) {
    { let label = label; if active { Text(label); } else { Text(fail(divisor)); }; Text(label) }
}
"#;
    let compiled =
        project_view_fixture_with_entry(source, "arcweft-test://view-conditional-failure")
            .compile()
            .unwrap();
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().unwrap().clone();
    let awbc = Arc::new(
        arcweft_bundle::standard_view::install_dialogue_handler_awbc(
            AwbcLowerer::new(
                &compiled.runtime_plan().plan,
                &compiled.runtime_plan().dialogue_content_catalog,
                "main.arcw",
            )
            .lower()
            .unwrap()
            .program,
        )
        .unwrap(),
    );
    let handle = PresentationHandleRecord::new(
        PresentationHandleId::try_new("view.conditional.failure").unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let mut runtime = BundleViewRuntime::try_new_with_awbc(
        product.clone(),
        Some(text.clone()),
        Arc::clone(&awbc),
    )
    .unwrap();
    let inputs = |active| {
        [RuntimeBinding {
            name: "active".to_owned(),
            value: RuntimeValue::Bool(active),
        }]
    };
    let success = runtime.evaluate(std::slice::from_ref(&handle), &inputs(true), false);
    assert!(success.diagnostics.is_empty(), "{success:?}");
    let saved = runtime.snapshot().unwrap();
    let mut cold = BundleViewRuntime::try_new_with_awbc(product, Some(text), awbc).unwrap();
    cold.restore(&saved, std::slice::from_ref(&handle)).unwrap();
    let failure = runtime.evaluate(std::slice::from_ref(&handle), &inputs(false), false);
    assert!(failure.mounts.is_empty());
    assert!(
        failure.diagnostics[0]
            .message
            .contains("did not complete purely"),
        "{failure:?}"
    );
    assert_eq!(
        failure,
        cold.evaluate(std::slice::from_ref(&handle), &inputs(false), false)
    );
    assert_eq!(
        success,
        runtime.evaluate(std::slice::from_ref(&handle), &inputs(true), false)
    );
    assert_eq!(
        success,
        cold.evaluate(std::slice::from_ref(&handle), &inputs(true), false)
    );
}

#[test]
fn authored_nested_views_pass_core_values_locals_aliases_and_defaults() {
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    use arcweft_runtime_driver::view_runtime::BundleViewTextValue;
    for body in [
        "Child(label, Row { label: label }, divisor)",
        "Child(payload = Row { label: label }, label = label, divisor = divisor)",
        "{ let inner = label; let saved = Child; saved(payload = Row { label: inner }, label = inner, divisor = divisor) }",
        "{ let saved = Child; { saved }(label, Row { label: label }, divisor) }",
    ] {
        let source = format!(
            "entry cli @entry.main {{ goto @flow.main }}\nflow main() -> String {{ return \"done\" }}\nstruct Row {{ label: String }}\nfn fallback(label: String, divisor: i64) -> String {{ let checked = 1i64 / divisor; label }}\nview Child(label: String, payload: Row, divisor: i64 = 1i64, suffix: String = fallback(label, divisor)) {{ Text(label); Text(payload.label); Text(suffix) }}\nview Main(label: String = \"seed\", divisor: i64 = 1i64) {{ {body} }}"
        );
        let compiled = project_view_fixture_with_entry(&source, "arcweft-test://view-nested-core")
            .compile()
            .expect("nested View uses the accepted call");
        let product = compiled.view_product().product().as_ref().clone();
        let text = compiled.view_product().text().unwrap().clone();
        let resource = product.program().unwrap().resource();
        assert_eq!(
            ViewProgramResource::decode_canonical_section(
                &resource.encode_canonical_section().unwrap()
            )
            .unwrap(),
            *resource
        );
        let awbc = Arc::new(
            arcweft_bundle::standard_view::install_dialogue_handler_awbc(
                AwbcLowerer::new(
                    &compiled.runtime_plan().plan,
                    &compiled.runtime_plan().dialogue_content_catalog,
                    "main.arcw",
                )
                .lower()
                .unwrap()
                .program,
            )
            .unwrap(),
        );
        resource.validate_awbc_programs(&awbc, Some(&text)).unwrap();
        let call = resource
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                ViewProgramInstruction::CallView { arguments, .. } => Some(arguments),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            call.iter()
                .map(|argument| argument.ordinal)
                .collect::<Vec<_>>(),
            if body.contains("payload =") {
                vec![1, 0, 2]
            } else {
                vec![0, 1, 2]
            }
        );
        for forgery in 0..6 {
            let mut forged = resource.clone();
            let ViewProgramInstruction::CallView {
                arguments, view, ..
            } = forged
                .instructions
                .iter_mut()
                .find(|instruction| matches!(instruction, ViewProgramInstruction::CallView { .. }))
                .unwrap()
            else {
                panic!("nested call")
            };
            if forgery == 5 {
                forged.definitions.push(forged.definitions[0].clone());
            } else {
                match forgery {
                    0 => {
                        arguments[0].value.program =
                            arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest(
                                [0xf9; 32],
                            )
                    }
                    1 => arguments[0].ordinal = u16::MAX,
                    2 => arguments[0].name = Some("unknown".to_owned()),
                    3 => {
                        arguments.remove(0);
                    }
                    _ => {
                        *view = arcweft_bundle::resource_codec::view::ViewDefinitionRef::new(
                            arcweft_view::ViewId::try_new("view.Missing").unwrap(),
                        )
                    }
                }
            }
            assert!(
                forged.validate_awbc_programs(&awbc, Some(&text)).is_err(),
                "forgery {forgery}"
            );
        }
        let handle = PresentationHandleRecord::new(
            PresentationHandleId::try_new("view.nested.core").unwrap(),
            PresentationHandleKind::View,
            "view.Main".to_owned(),
            None,
            PresentationResourceState::Mounted,
            None,
            0,
        );
        let mut runtime = BundleViewRuntime::try_new_with_awbc(
            product.clone(),
            Some(text.clone()),
            Arc::clone(&awbc),
        )
        .unwrap();
        for label in ["first", "changed", "first"] {
            let inputs = [
                RuntimeBinding {
                    name: "label".to_owned(),
                    value: RuntimeValue::String(label.to_owned()),
                },
                RuntimeBinding {
                    name: "divisor".to_owned(),
                    value: RuntimeValue::Int(arcweft_core::value::RuntimeInt::i64(1)),
                },
            ];
            let frame = runtime.evaluate(std::slice::from_ref(&handle), &inputs, false);
            assert!(frame.diagnostics.is_empty(), "{frame:?}");
            assert_eq!(frame.mounts.len(), 2);
            assert_eq!(frame.mounts[1].view.as_str(), "view.Child");
            assert_eq!(
                frame.mounts[1]
                    .text
                    .iter()
                    .map(|output| match &output.value {
                        BundleViewTextValue::Plain { value } => value.as_str(),
                        _ => panic!("plain text"),
                    })
                    .collect::<Vec<_>>(),
                vec![label, label, label]
            );
            let saved = runtime.snapshot().unwrap();
            let mut cold = BundleViewRuntime::try_new_with_awbc(
                product.clone(),
                Some(text.clone()),
                Arc::clone(&awbc),
            )
            .unwrap();
            cold.restore(&saved, std::slice::from_ref(&handle)).unwrap();
            assert_eq!(
                frame,
                cold.evaluate(std::slice::from_ref(&handle), &inputs, false)
            );
            let failed_inputs = [
                inputs[0].clone(),
                RuntimeBinding {
                    name: "divisor".to_owned(),
                    value: RuntimeValue::Int(arcweft_core::value::RuntimeInt::i64(0)),
                },
            ];
            let failure = runtime.evaluate(std::slice::from_ref(&handle), &failed_inputs, false);
            assert!(
                failure.mounts.is_empty(),
                "child default failure must retire the whole parent output: {failure:?}"
            );
            assert!(!failure.diagnostics.is_empty());
            assert_eq!(
                failure,
                cold.evaluate(std::slice::from_ref(&handle), &failed_inputs, false)
            );
            assert_eq!(
                frame,
                runtime.evaluate(std::slice::from_ref(&handle), &inputs, false)
            );
            assert_eq!(
                frame,
                cold.evaluate(std::slice::from_ref(&handle), &inputs, false)
            );
        }
    }
}

#[test]
fn handler_local_captures_use_core_inputs_and_revoke_changed_or_restored_routes() {
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
fn echo(value: String) -> String { value }
view Main(dialogue: DialogueView, label: String) {
    {
        let current = dialogue;
        let (first, second) = (label, "suffix");
        let transform = echo;
        match (first, true) { (label, true) => Button(label).on_click { let observed = transform(second); let observed = label; current.primary_action }, (_, false) => Button("unused") }
    }
}

"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://view-local-handler")
        .compile()
        .expect("locals can be retained as typed handler captures");
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().unwrap().clone();
    let resource = product.program().unwrap().resource();
    let handler = resource.handlers.first().unwrap();
    assert_eq!(handler.captures.len(), 4);
    assert!(handler.captures.iter().all(|input| matches!(
        input.source,
        arcweft_view::ViewExecutionInputSource::Local(_)
    )));
    assert_eq!(
        ViewProgramResource::decode_canonical_section(
            &resource.encode_canonical_section().unwrap()
        )
        .unwrap(),
        *resource
    );
    let awbc = Arc::new(
        arcweft_bundle::standard_view::install_dialogue_handler_awbc(
            AwbcLowerer::new(
                &compiled.runtime_plan().plan,
                &compiled.runtime_plan().dialogue_content_catalog,
                "main.arcw",
            )
            .lower()
            .unwrap()
            .program,
        )
        .unwrap(),
    );
    resource.validate_awbc_programs(&awbc, Some(&text)).unwrap();
    for forgery in 0..4 {
        let mut forged = resource.clone();
        match forgery {
            0 => forged.handlers[0].captures[0].source =
                arcweft_view::ViewExecutionInputSource::Local(arcweft_view::ViewLocalCoordinate {
                    program: arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest(
                        [0xf3; 32],
                    ),
                    output: 0,
                }),
            1 => {
                let duplicate = forged.handlers[0].captures[0];
                forged.handlers[0].captures.push(duplicate);
            }
            2 => {
                forged.handlers[0].captures[0].value_type =
                    arcweft_core::pattern::RuntimeCheckedType::Bool.semantic_identity_digest()
            }
            _ => {
                let index = forged
                    .instructions
                    .iter()
                    .position(|instruction| {
                        matches!(instruction, ViewProgramInstruction::BindHandler { .. })
                    })
                    .unwrap();
                let instruction = forged.instructions.remove(index);
                forged.instructions.push(instruction);
            }
        }
        assert!(
            forged.encode_canonical_section().is_err(),
            "forgery {forgery}"
        );
        assert!(
            forged.validate_awbc_programs(&awbc, Some(&text)).is_err(),
            "forgery {forgery}"
        );
    }
    let view = ViewId::try_new("view.Main").unwrap();
    let display = minimal_dialogue_frame(view.clone());
    let advance = DialogueAdvanceTarget::new(
        DialoguePresentationId::new(11),
        DialogueEntryId::new(12),
        DialogueInstanceId::new(13),
        DialogueStageIndex::new(0),
        DialogueRevision::new(1),
    );
    let dialogue = DialogueViewInput {
        handle: PresentationHandleId::try_new("dialogue.local.handler").unwrap(),
        view: &view,
        frame: &display,
        state: DialogueViewState {
            occurrence: DialogueViewOccurrence {
                presentation: DialoguePresentationId::new(11),
                entry: DialogueEntryId::new(12),
                instance: DialogueInstanceId::new(13),
            },
            stage: DialogueViewStage {
                index: DialogueStageIndex::new(0),
                page: DialoguePageIndex::new(0),
                stage_count: 1,
                page_count: 1,
            },
            reveal: DialogueViewReveal::complete(),
            primary_action: DialogueViewPrimaryAction {
                target: Some(advance),
            },
        },
    };
    let invocation = |binding: &arcweft_runtime_driver::view_runtime::BundleViewEventBinding| {
        ViewHandlerInvocation::from_input(
            &InputEvent::activate(InputEpoch(1), binding.target().clone()),
            binding.event(),
            binding.route(),
        )
        .unwrap()
    };
    let mut runtime = BundleViewRuntime::try_new_with_awbc(
        product.clone(),
        Some(text.clone()),
        Arc::clone(&awbc),
    )
    .unwrap();
    let first_input = [RuntimeBinding {
        name: "label".to_owned(),
        value: RuntimeValue::String("first".to_owned()),
    }];
    let first =
        runtime.evaluate_with_dialogue(&[], std::slice::from_ref(&dialogue), &first_input, false);
    assert!(first.diagnostics.is_empty(), "{first:?}");
    let old = invocation(&first.mounts[0].events[0]);
    let expected =
        Some(arcweft_runtime_driver::dialogue::BundlePresentationInput::advance_dialogue(advance));
    assert_eq!(runtime.dispatch_invocation(&old).unwrap(), expected);
    assert_eq!(
        first,
        runtime.evaluate_with_dialogue(&[], std::slice::from_ref(&dialogue), &first_input, false)
    );
    let second_input = [RuntimeBinding {
        name: "label".to_owned(),
        value: RuntimeValue::String("changed".to_owned()),
    }];
    let changed =
        runtime.evaluate_with_dialogue(&[], std::slice::from_ref(&dialogue), &second_input, false);
    assert!(changed.diagnostics.is_empty(), "{changed:?}");
    assert_ne!(
        first.mounts[0].events[0].route(),
        changed.mounts[0].events[0].route()
    );
    assert!(runtime.dispatch_invocation(&old).is_err());
    let current = invocation(&changed.mounts[0].events[0]);
    assert_eq!(runtime.dispatch_invocation(&current).unwrap(), expected);
    let saved = runtime.snapshot().unwrap();
    let mut cold = BundleViewRuntime::try_new_with_awbc(product, Some(text), awbc).unwrap();
    let live_owner = arcweft_runtime_driver::presentation_handles::PresentationHandleRecord::new(
        dialogue.handle.clone(),
        arcweft_runtime_driver::presentation_handles::PresentationHandleKind::View,
        view.as_str().to_owned(),
        None,
        arcweft_runtime_driver::presentation_handles::PresentationResourceState::Mounted,
        None,
        0,
    );
    cold.restore(&saved, std::slice::from_ref(&live_owner))
        .unwrap();
    let restored =
        cold.evaluate_with_dialogue(&[], std::slice::from_ref(&dialogue), &second_input, false);
    assert!(restored.diagnostics.is_empty(), "{restored:?}");
    assert_eq!(
        changed.mounts[0].action_buttons,
        restored.mounts[0].action_buttons
    );
    assert!(
        cold.dispatch_invocation(&current).is_err(),
        "restore must not revive a pre-restore route"
    );
    assert_eq!(
        cold.dispatch_invocation(&invocation(&restored.mounts[0].events[0]))
            .unwrap(),
        expected
    );
}

#[test]
fn authored_view_matches_select_core_patterns_and_export_arm_bindings() {
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    use arcweft_runtime_driver::view_runtime::BundleViewTextValue;
    for body in [
        "match choose_pair(value) { (true, label) => Text(label), (false, label) => Button(label) }",
        "match value { (true, label) => Text(label), (false, label) => Button(label) };",
        "{ match value { (true, label) => Text(label), (false, label) => Button(label) }; }",
        "{ match value { (true, label) when choose(enabled) => { let label = label; Text(label) }, (true, _) => Text(\"fallback\"), (false, label) => match enabled { true => Text(label), false => Button(label) } }; }",
        "match value { (true, label) when choose(enabled) => { let label = label; Text(label) }, (true, _) => Text(\"fallback\"), (false, label) => match enabled { true => Text(label), false => Button(label) } }",
    ] {
        let guarded = body.contains(" when ");
        let source = format!(
            "entry cli @entry.main {{ goto @flow.main }}\nflow main() -> String {{ return \"done\" }}\nfn choose(value: bool) -> bool {{ value }}\nfn choose_pair(value: (bool, String)) -> (bool, String) {{ value }}\nview Main(value: (bool, String), enabled: bool = true) {{ {body} }}"
        );
        let compiled =
            project_view_fixture_with_entry(&source, "arcweft-test://view-match-bindings")
                .compile()
                .expect("retained Match consumes shared Core selection and owned bindings");
        let product = compiled.view_product().product().as_ref().clone();
        let text = compiled.view_product().text().unwrap().clone();
        let resource = product.program().unwrap().resource();
        assert_eq!(
            ViewProgramResource::decode_canonical_section(
                &resource.encode_canonical_section().unwrap()
            )
            .unwrap(),
            *resource
        );
        let awbc = Arc::new(
            arcweft_bundle::standard_view::install_dialogue_handler_awbc(
                AwbcLowerer::new(
                    &compiled.runtime_plan().plan,
                    &compiled.runtime_plan().dialogue_content_catalog,
                    "main.arcw",
                )
                .lower()
                .unwrap()
                .program,
            )
            .unwrap(),
        );
        resource.validate_awbc_programs(&awbc, Some(&text)).unwrap();
        let native_plan = Arc::new(compiled.runtime_plan().plan.clone());
        let selection = resource
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                ViewProgramInstruction::Match { program, .. } => Some(program),
                _ => None,
            })
            .unwrap();
        for forgery in 0..9 {
            let mut forged = resource.clone();
            let ViewProgramInstruction::Match { program, .. } = forged
                .instructions
                .iter_mut()
                .find(|instruction| matches!(instruction, ViewProgramInstruction::Match { .. }))
                .unwrap()
            else {
                panic!("Match");
            };
            match forgery {
                0 => {
                    program.execution.program =
                        arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest(
                            [0xe3; 32],
                        )
                }
                1 => {
                    program.execution.result_type =
                        arcweft_core::pattern::RuntimeCheckedType::Bool.semantic_identity_digest()
                }
                2 => program.arms.swap(0, 1),
                3 => {
                    let output = program.arms[0].outputs[0];
                    program.arms[0].outputs = vec![output, output].into_boxed_slice();
                }
                4 => {
                    program.arms[0].outputs[0].value_type =
                        arcweft_core::pattern::RuntimeCheckedType::Bool.semantic_identity_digest()
                }
                5 => program.arms[0].body_span = u32::MAX,
                6 => program.arms[0].body_span = 0,
                7 => {
                    program.arms[0].outputs[0].coordinate.program =
                        arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest(
                            [0xe4; 32],
                        )
                }
                _ => program.arms = Box::new([]),
            }
            assert!(
                forged.validate_awbc_programs(&awbc, Some(&text)).is_err(),
                "forgery {forgery}"
            );
        }
        if !guarded {
            let first = text
                .sources
                .iter()
                .find_map(|source| match &source.kind {
                    arcweft_bundle::resource_codec::view::ViewTextSourceKind::Program {
                        program,
                    } => Some(program.clone()),
                    _ => None,
                })
                .unwrap();
            let mut escaped = text.clone();
            escaped.sources.last_mut().unwrap().kind =
                arcweft_bundle::resource_codec::view::ViewTextSourceKind::Program {
                    program: first,
                };
            assert!(
                resource
                    .validate_awbc_programs(&awbc, Some(&escaped))
                    .is_err(),
                "an arm cannot read another arm's binding"
            );
        }
        let handle = PresentationHandleRecord::new(
            PresentationHandleId::try_new("view.match.bindings").unwrap(),
            PresentationHandleKind::View,
            "view.Main".to_owned(),
            None,
            PresentationResourceState::Mounted,
            None,
            0,
        );
        let mut runtime = BundleViewRuntime::try_new_with_awbc(
            product.clone(),
            Some(text.clone()),
            Arc::clone(&awbc),
        )
        .unwrap();
        for (active, enabled, label) in [
            (true, true, "first"),
            (true, false, "second"),
            (false, true, "third"),
            (false, false, "fourth"),
            (true, true, "first"),
        ] {
            let value = RuntimeValue::Tuple(vec![
                RuntimeValue::Bool(active),
                RuntimeValue::String(label.to_owned()),
            ]);
            let inputs = [
                RuntimeBinding {
                    name: "value".to_owned(),
                    value: value.clone(),
                },
                RuntimeBinding {
                    name: "enabled".to_owned(),
                    value: RuntimeValue::Bool(enabled),
                },
            ];
            let core_inputs = selection
                .execution
                .inputs
                .iter()
                .map(|input| match input.parameter().unwrap().value() {
                    0 => value.clone(),
                    1 => RuntimeValue::Bool(enabled),
                    _ => panic!("unexpected formal"),
                })
                .collect::<Vec<_>>();
            let mut native = arcweft_core::pure::VmRuntimePureCallBackend::default();
            let selected = arcweft_core::pure::evaluate_pure_program_with_backend(
                &native_plan,
                selection.execution.program,
                &core_inputs,
                &mut native,
            )
            .unwrap();
            let mut product_backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
            assert_eq!(
                selected,
                arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
                    &awbc,
                    selection.execution.program,
                    &core_inputs,
                    &mut product_backend
                )
                .unwrap()
            );
            let expected_tag = if active {
                if guarded && !enabled { 1 } else { 0 }
            } else if guarded {
                2
            } else {
                1
            };
            let expected_payload = if guarded && active && !enabled {
                RuntimeValue::Unit
            } else {
                RuntimeValue::Tuple(vec![RuntimeValue::String(label.to_owned())])
            };
            assert_eq!(
                selected,
                RuntimeValue::Tuple(vec![
                    RuntimeValue::UInt(arcweft_core::value::RuntimeUInt::U32(expected_tag)),
                    expected_payload
                ])
            );
            let frame = runtime.evaluate(std::slice::from_ref(&handle), &inputs, false);
            assert!(frame.diagnostics.is_empty(), "{frame:?}");
            assert_eq!(frame.mounts.len(), 1);
            let rendered_text = active || guarded && enabled;
            let expected_label = if guarded && active && !enabled {
                "fallback"
            } else {
                label
            };
            if rendered_text {
                assert_eq!(frame.mounts[0].text.len(), 1);
                assert!(
                    matches!(&frame.mounts[0].text[0].value, BundleViewTextValue::Plain { value } if value == expected_label)
                );
                assert!(frame.mounts[0].action_buttons.is_empty());
            } else {
                assert_eq!(frame.mounts[0].action_buttons.len(), 1);
                assert_eq!(frame.mounts[0].action_buttons[0].label, expected_label);
                assert!(frame.mounts[0].text.is_empty());
            }
            let saved = runtime.snapshot().unwrap();
            let mut cold = BundleViewRuntime::try_new_with_awbc(
                product.clone(),
                Some(text.clone()),
                Arc::clone(&awbc),
            )
            .unwrap();
            cold.restore(&saved, std::slice::from_ref(&handle)).unwrap();
            assert_eq!(
                frame,
                cold.evaluate(std::slice::from_ref(&handle), &inputs, false)
            );
        }
    }
}

#[test]
fn authored_view_match_failures_publish_no_partial_frame_and_recover() {
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
fn choose(divisor: i64) -> bool { (1i64 / divisor) == 0i64 }
fn fail(divisor: i64) -> String { let observed = 1i64 / divisor; "one" }
view Main(active: bool, divisor: i64 = 1i64) {
    Text("prefix");
    match (active, divisor) {
        (true, denominator) when choose(denominator) => Text("guard"),
        (false, _) => Text(fail(divisor)),
        _ => Text("fallback"),
    }
}
"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://view-match-failure")
        .compile()
        .unwrap();
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().unwrap().clone();
    let awbc = Arc::new(
        arcweft_bundle::standard_view::install_dialogue_handler_awbc(
            AwbcLowerer::new(
                &compiled.runtime_plan().plan,
                &compiled.runtime_plan().dialogue_content_catalog,
                "main.arcw",
            )
            .lower()
            .unwrap()
            .program,
        )
        .unwrap(),
    );
    let handle = PresentationHandleRecord::new(
        PresentationHandleId::try_new("view.match.failure").unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let mut runtime = BundleViewRuntime::try_new_with_awbc(
        product.clone(),
        Some(text.clone()),
        Arc::clone(&awbc),
    )
    .unwrap();
    let inputs = |active, divisor| {
        [
            RuntimeBinding {
                name: "active".to_owned(),
                value: RuntimeValue::Bool(active),
            },
            RuntimeBinding {
                name: "divisor".to_owned(),
                value: RuntimeValue::i64(divisor),
            },
        ]
    };
    let success = runtime.evaluate(std::slice::from_ref(&handle), &inputs(false, 1), false);
    assert!(success.diagnostics.is_empty(), "{success:?}");
    let saved = runtime.snapshot().unwrap();
    let mut cold = BundleViewRuntime::try_new_with_awbc(product, Some(text), awbc).unwrap();
    cold.restore(&saved, std::slice::from_ref(&handle)).unwrap();
    for active in [true, false] {
        let failure = runtime.evaluate(std::slice::from_ref(&handle), &inputs(active, 0), false);
        assert!(
            failure.mounts.is_empty(),
            "neither a failed guard nor a failed arm may publish the prefix"
        );
        assert!(!failure.diagnostics.is_empty());
        assert_eq!(
            failure,
            cold.evaluate(std::slice::from_ref(&handle), &inputs(active, 0), false)
        );
        assert_eq!(
            success,
            runtime.evaluate(std::slice::from_ref(&handle), &inputs(false, 1), false)
        );
        assert_eq!(
            success,
            cold.evaluate(std::slice::from_ref(&handle), &inputs(false, 1), false)
        );
    }
}

#[test]
fn authored_view_match_pattern_families_share_the_core_selector() {
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    use arcweft_runtime_driver::view_runtime::BundleViewTextValue;
    let cases = [
        r#"view Main(value: (bool, String) = (false, "or")) {
            match value { (true, label) | (false, label) => Text(label) }
        }"#,
        r#"view Main(value: Array<String, 2> = ["array", "tail"]) {
            match value { [label, ..] => Text(label) }
        }"#,
        r#"struct Holder { label: String }
        view Main(value: Holder = Holder { label = "record" }) {
            match value { Holder { label } => Text(label) }
        }"#,
        r#"enum Envelope<T> { Value(T), Empty }
        view Main(value: Envelope<String> = .Value("variant")) {
            match value { .Value(label) => Text(label), .Empty => Text("empty") }
        }"#,
    ];
    for (case, label) in cases.into_iter().zip(["or", "array", "record", "variant"]) {
        let source = format!(
            "entry cli @entry.main {{ goto @flow.main }}\nflow main() -> String {{ return \"done\" }}\n{case}"
        );
        let compiled = project_view_fixture_with_entry(&source, "arcweft-test://view-patterns")
            .compile()
            .unwrap_or_else(|error| panic!("{label}: {error:?}"));
        let product = compiled.view_product().product().as_ref().clone();
        let text = compiled.view_product().text().unwrap().clone();
        let resource = product.program().unwrap().resource();
        assert_eq!(
            ViewProgramResource::decode_canonical_section(
                &resource.encode_canonical_section().unwrap()
            )
            .unwrap(),
            *resource
        );
        let awbc = Arc::new(
            arcweft_bundle::standard_view::install_dialogue_handler_awbc(
                AwbcLowerer::new(
                    &compiled.runtime_plan().plan,
                    &compiled.runtime_plan().dialogue_content_catalog,
                    "main.arcw",
                )
                .lower()
                .unwrap()
                .program,
            )
            .unwrap(),
        );
        let bytes = awbc.encode_canonical().unwrap();
        let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
            &bytes,
            arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
        )
        .unwrap();
        assert_eq!(decoded, *awbc);
        let awbc = Arc::new(decoded);
        resource.validate_awbc_programs(&awbc, Some(&text)).unwrap();
        let handle = PresentationHandleRecord::new(
            PresentationHandleId::try_new("view.match.patterns").unwrap(),
            PresentationHandleKind::View,
            "view.Main".to_owned(),
            None,
            PresentationResourceState::Mounted,
            None,
            0,
        );
        let mut runtime = BundleViewRuntime::try_new_with_awbc(
            product.clone(),
            Some(text.clone()),
            Arc::clone(&awbc),
        )
        .unwrap();
        let frame = runtime.evaluate(std::slice::from_ref(&handle), &[], false);
        assert!(frame.diagnostics.is_empty(), "{label}: {frame:?}");
        assert!(matches!(&frame.mounts[0].text[0].value,
            BundleViewTextValue::Plain { value } if value == label));
        let saved = runtime.snapshot().unwrap();
        let mut cold = BundleViewRuntime::try_new_with_awbc(product, Some(text), awbc).unwrap();
        cold.restore(&saved, std::slice::from_ref(&handle)).unwrap();
        assert_eq!(
            frame,
            cold.evaluate(std::slice::from_ref(&handle), &[], false)
        );
    }
}

#[test]
fn view_or_patterns_refuse_inconsistent_binding_positions() {
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main(value: (bool, String, String) = (false, "ignored", "other")) {
    match value { (true, left, right) | (false, right, left) => Text(left) }
}
"#;
    let error = project_view_fixture_with_entry(source, "arcweft-test://view-or-layout")
        .compile()
        .expect_err("Or alternatives retain the same binding positions");
    assert_eq!(error.stage(), "hir-lower");
    assert!(format!("{error:?}").contains("PositionMismatch"));
}

#[test]
fn uninhabited_view_match_keeps_an_empty_case_inventory() {
    let source = "entry cli @entry.main { goto @flow.main }\nflow main() -> String { return \"done\" }\nview Main(value: Never) { match value {} }";
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://view-empty-match")
        .compile()
        .expect("Never has an exhaustive empty Match");
    let resource = compiled
        .view_product()
        .product()
        .program()
        .unwrap()
        .resource();
    let selection = resource
        .instructions
        .iter()
        .enumerate()
        .find_map(|(index, row)| match row {
            ViewProgramInstruction::Match { program, .. } => Some((index, program)),
            _ => None,
        })
        .unwrap();
    assert!(selection.1.arms.is_empty());
    let ranges = selection
        .1
        .ranges(selection.0 as u32, resource.instructions.len() as u32)
        .unwrap();
    assert_eq!(ranges.continuation(), selection.0 as u32 + 1);
    assert_eq!(
        ViewProgramResource::decode_canonical_section(
            &resource.encode_canonical_section().unwrap()
        )
        .unwrap(),
        *resource
    );
    let awbc = AwbcLowerer::new(
        &compiled.runtime_plan().plan,
        &compiled.runtime_plan().dialogue_content_catalog,
        "main.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let awbc = arcweft_bundle::standard_view::install_dialogue_handler_awbc(awbc).unwrap();
    resource
        .validate_awbc_programs(&awbc, compiled.view_product().text())
        .unwrap();
}

#[test]
fn nonreturning_empty_match_keeps_frame_failure_atomic_after_restore() {
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
fn spin() -> Never { loop {} }
view Main() { Text("prefix"); match spin() {} }
"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://view-never-match")
        .compile()
        .unwrap();
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().cloned();
    let awbc = Arc::new(
        arcweft_bundle::standard_view::install_dialogue_handler_awbc(
            AwbcLowerer::new(
                &compiled.runtime_plan().plan,
                &compiled.runtime_plan().dialogue_content_catalog,
                "main.arcw",
            )
            .lower()
            .unwrap()
            .program,
        )
        .unwrap(),
    );
    let handle = PresentationHandleRecord::new(
        PresentationHandleId::try_new("view.empty.loop").unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let mut runtime =
        BundleViewRuntime::try_new_with_awbc(product.clone(), text.clone(), Arc::clone(&awbc))
            .unwrap();
    let failed = runtime.evaluate(std::slice::from_ref(&handle), &[], false);
    assert!(
        !failed.diagnostics.is_empty(),
        "a non-returning selector reaches the fixed evaluation budget"
    );
    assert!(
        failed.mounts.is_empty(),
        "prefix output is never partially published"
    );
    let saved = runtime.snapshot().unwrap();
    let mut cold = BundleViewRuntime::try_new_with_awbc(product, text, awbc).unwrap();
    cold.restore(&saved, std::slice::from_ref(&handle)).unwrap();
    assert_eq!(
        failed,
        cold.evaluate(std::slice::from_ref(&handle), &[], false)
    );
}

#[test]
fn authored_keyed_iteration_reaches_the_retained_program() {
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    use arcweft_runtime_driver::view_runtime::BundleViewTextValue;
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
fn items(reverse: bool) -> Vec<(i32, String)> {
    if reverse { [(2, "second"), (1, "first")] } else { [(1, "first"), (2, "second")] }
}

fn item_key(id: i32) -> String { if id == 1 { "one" } else { "two" } }
view Main(reverse: bool) {
    {
        for (id, label) in items(reverse) key = item_key(id) { Text(label) }
        Text("tail")
    }
}
"#;
    let compiled =
        project_view_fixture_with_entry(source, "arcweft-test://compiler-keyed-iteration")
            .compile()
            .expect("checked keyed iteration View");
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().unwrap().clone();
    let resource = product.program().unwrap().resource();
    assert_eq!(
        ViewProgramResource::decode_canonical_section(
            &resource.encode_canonical_section().unwrap()
        )
        .unwrap(),
        *resource
    );
    let repeat = resource
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            ViewProgramInstruction::RepeatKeyed { program, .. } => Some(program),
            _ => None,
        })
        .unwrap();
    assert_eq!(repeat.source.outputs.len(), 2);
    let awbc = Arc::new(
        arcweft_bundle::standard_view::install_dialogue_handler_awbc(
            AwbcLowerer::new(
                &compiled.runtime_plan().plan,
                &compiled.runtime_plan().dialogue_content_catalog,
                "main.arcw",
            )
            .lower()
            .unwrap()
            .program,
        )
        .unwrap(),
    );
    resource.validate_awbc_programs(&awbc, Some(&text)).unwrap();
    for forgery in 0..5 {
        let mut candidate = resource.clone();
        let program = candidate
            .instructions
            .iter_mut()
            .find_map(|instruction| match instruction {
                ViewProgramInstruction::RepeatKeyed { program, .. } => Some(program),
                _ => None,
            })
            .unwrap();
        match forgery {
            0 => program.source.execution.result_type = program.key.result_type,
            1 => program.source.outputs[0].value_type = program.key.result_type,
            2 => program.source.outputs[0].coordinate.output = 1,
            3 => {
                program.key.inputs[0].source = arcweft_view::ViewExecutionInputSource::Local(
                    program.source.outputs[1].coordinate,
                )
            }
            4 => program.body_span = u32::MAX,
            _ => unreachable!(),
        }
        assert!(
            candidate
                .validate_awbc_programs(&awbc, Some(&text))
                .is_err(),
            "forgery {forgery}"
        );
    }

    let native_plan = Arc::new(compiled.runtime_plan().plan.clone());
    let handle = PresentationHandleRecord::new(
        PresentationHandleId::try_new("view.keyed.items").unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let mut runtime = BundleViewRuntime::try_new_with_awbc(
        product.clone(),
        Some(text.clone()),
        Arc::clone(&awbc),
    )
    .unwrap();
    let mut stable_paths = BTreeMap::new();
    for reverse in [false, true, false] {
        let inputs = [RuntimeBinding {
            name: "reverse".to_owned(),
            value: RuntimeValue::Bool(reverse),
        }];
        let core_inputs = [RuntimeValue::Bool(reverse)];
        let mut native = arcweft_core::pure::VmRuntimePureCallBackend::default();
        let values = arcweft_core::pure::evaluate_pure_program_with_backend(
            &native_plan,
            repeat.source.execution.program,
            &core_inputs,
            &mut native,
        )
        .unwrap();
        let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
        assert_eq!(
            values,
            arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
                &awbc,
                repeat.source.execution.program,
                &core_inputs,
                &mut backend
            )
            .unwrap()
        );
        let frame = runtime.evaluate(std::slice::from_ref(&handle), &inputs, false);
        assert!(frame.diagnostics.is_empty(), "{frame:#?}");
        assert_eq!(frame.mounts.len(), 1);
        let expected = if reverse {
            ["second", "first", "tail"]
        } else {
            ["first", "second", "tail"]
        };
        let rendered = frame.mounts[0]
            .text
            .iter()
            .map(|text| match &text.value {
                BundleViewTextValue::Plain { value } => value.as_str(),
                _ => panic!("plain text"),
            })
            .collect::<Vec<_>>();
        assert_eq!(rendered, expected);
        let item_nodes = frame.mounts[0]
            .style_nodes
            .iter()
            .filter(|node| {
                matches!(
                    node.kind,
                    arcweft_runtime_driver::view_runtime::BundleViewStyleNodeKind::Text { .. }
                ) && !node.path.segments().is_empty()
            })
            .collect::<Vec<_>>();
        assert_eq!(item_nodes.len(), 2);
        for (label, node) in rendered.iter().take(2).zip(item_nodes) {
            let words = node.path.style_path_words();
            if let Some(before) = stable_paths.get(*label) {
                assert_eq!(before, &words);
            }
            stable_paths.insert((*label).to_owned(), words);
        }
        assert_eq!(stable_paths.len(), 2);
        let saved = runtime.snapshot().unwrap();
        let mut cold = BundleViewRuntime::try_new_with_awbc(
            product.clone(),
            Some(text.clone()),
            Arc::clone(&awbc),
        )
        .unwrap();
        cold.restore(&saved, std::slice::from_ref(&handle)).unwrap();
        assert_eq!(
            frame,
            cold.evaluate(std::slice::from_ref(&handle), &inputs, false)
        );
    }
}

#[test]
fn keyed_iteration_failure_and_empty_source_are_cache_and_restore_transparent() {
    use arcweft_core::value::{RuntimeBinding, RuntimeInt, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    use arcweft_runtime_driver::view_runtime::BundleViewDiagnosticCode;
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
fn items(denominator: i32, empty: bool) -> Vec<i32> {
    if empty { [] } else { [1 / denominator, 2 / denominator] }
}
fn item_key(id: i32, duplicate: bool, denominator: i32) -> i32 {
    if duplicate { 7 } else { id / denominator }
}
view Main(duplicate: bool, source_den: i32, key_den: i32, empty: bool) {
    Text("prefix")
    { for id in items(source_den, empty) key = item_key(id, duplicate, key_den) { Text("item") } }
    Text("tail")
}
"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://view-keyed-failure")
        .compile()
        .unwrap();
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().cloned();
    let awbc = Arc::new(
        arcweft_bundle::standard_view::install_dialogue_handler_awbc(
            AwbcLowerer::new(
                &compiled.runtime_plan().plan,
                &compiled.runtime_plan().dialogue_content_catalog,
                "main.arcw",
            )
            .lower()
            .unwrap()
            .program,
        )
        .unwrap(),
    );
    let handle = PresentationHandleRecord::new(
        PresentationHandleId::try_new("view.keyed.failure").unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let handles = std::slice::from_ref(&handle);
    let mut runtime =
        BundleViewRuntime::try_new_with_awbc(product.clone(), text.clone(), Arc::clone(&awbc))
            .unwrap();
    for (duplicate, source_den, key_den, empty, expected_error) in [
        (false, 1, 1, false, None),
        (
            true,
            1,
            1,
            false,
            Some(BundleViewDiagnosticCode::DuplicateRepeatKey),
        ),
        (
            false,
            1,
            0,
            false,
            Some(BundleViewDiagnosticCode::InvalidValueProgram),
        ),
        (
            false,
            0,
            1,
            false,
            Some(BundleViewDiagnosticCode::InvalidValueProgram),
        ),
        (false, 0, 0, true, None),
        (false, 1, 1, false, None),
    ] {
        let inputs = [
            RuntimeBinding {
                name: "duplicate".to_owned(),
                value: RuntimeValue::Bool(duplicate),
            },
            RuntimeBinding {
                name: "source_den".to_owned(),
                value: RuntimeValue::Int(RuntimeInt::I32(source_den)),
            },
            RuntimeBinding {
                name: "key_den".to_owned(),
                value: RuntimeValue::Int(RuntimeInt::I32(key_den)),
            },
            RuntimeBinding {
                name: "empty".to_owned(),
                value: RuntimeValue::Bool(empty),
            },
        ];
        let frame = runtime.evaluate(handles, &inputs, false);
        if let Some(code) = expected_error {
            assert!(
                frame.mounts.is_empty(),
                "no prefix or repeated body is published: {frame:#?}"
            );
            assert_eq!(frame.diagnostics.len(), 1, "{frame:#?}");
            assert_eq!(frame.diagnostics[0].code, code);
        } else {
            assert!(frame.diagnostics.is_empty(), "{frame:#?}");
            assert_eq!(frame.mounts.len(), 1);
            assert_eq!(frame.mounts[0].text.len(), if empty { 2 } else { 4 });
        }
        assert_eq!(frame, runtime.evaluate(handles, &inputs, false));
        let saved = runtime.snapshot().unwrap();
        let mut cold =
            BundleViewRuntime::try_new_with_awbc(product.clone(), text.clone(), Arc::clone(&awbc))
                .unwrap();
        cold.restore(&saved, handles).unwrap();
        assert_eq!(frame, cold.evaluate(handles, &inputs, false));
    }
}

#[test]
fn keyed_iteration_uses_checked_ranges_discard_patterns_and_nested_scopes() {
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    for (ordinal, body, expected_texts) in [
        (0, "{ for id in 0..2 key = id { Text(\"item\") } }", 2),
        (1, "{ for _ in [1] key = 7 { Text(\"item\") } }", 1),
        (
            2,
            "{ for (id, _) in [(1, \"a\"), (2, \"b\")] key = (id, \"key\") { Text(\"item\") } }",
            2,
        ),
        (
            3,
            "{ for outer in [1, 2] key = outer { for inner in [1, 2] key = (outer, inner) { Text(\"item\") } } }",
            4,
        ),
        (
            4,
            "{ let values = [1, 2]; for id in values key = id { Text(\"item\") } }",
            2,
        ),
    ] {
        let source = format!(
            "entry cli @entry.main {{ goto @flow.main }}\nflow main() -> String {{ return \"done\" }}\nview Main() {{ {body} }}"
        );
        let compiled = project_view_fixture_with_entry(
            &source,
            &format!("arcweft-test://view-iteration-family-{ordinal}"),
        )
        .compile()
        .unwrap();
        let product = compiled.view_product().product().as_ref().clone();
        let text = compiled.view_product().text().cloned();
        let awbc = Arc::new(
            arcweft_bundle::standard_view::install_dialogue_handler_awbc(
                AwbcLowerer::new(
                    &compiled.runtime_plan().plan,
                    &compiled.runtime_plan().dialogue_content_catalog,
                    "main.arcw",
                )
                .lower()
                .unwrap()
                .program,
            )
            .unwrap(),
        );
        let handle = PresentationHandleRecord::new(
            PresentationHandleId::try_new(format!("view.iteration.family.{ordinal}")).unwrap(),
            PresentationHandleKind::View,
            "view.Main".to_owned(),
            None,
            PresentationResourceState::Mounted,
            None,
            0,
        );
        let mut runtime =
            BundleViewRuntime::try_new_with_awbc(product.clone(), text.clone(), Arc::clone(&awbc))
                .unwrap();
        let frame = runtime.evaluate(std::slice::from_ref(&handle), &[], false);
        assert!(frame.diagnostics.is_empty(), "case {ordinal}: {frame:#?}");
        assert_eq!(frame.mounts.len(), 1);
        assert_eq!(frame.mounts[0].text.len(), expected_texts);
        let saved = runtime.snapshot().unwrap();
        let mut cold = BundleViewRuntime::try_new_with_awbc(product, text, awbc).unwrap();
        cold.restore(&saved, std::slice::from_ref(&handle)).unwrap();
        assert_eq!(
            frame,
            cold.evaluate(std::slice::from_ref(&handle), &[], false)
        );
    }
}

#[test]
fn authored_local_state_initializes_once_per_mount_and_survives_cold_restore() {
    use arcweft_core::value::{RuntimeBinding, RuntimeValue};
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    use arcweft_runtime_driver::view_runtime::BundleViewTextValue;
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main(initial: String) {
    local state caption: String = initial
    Text(caption)
}
"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://view-local-state")
        .compile()
        .expect("authored typed local state");
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().cloned();
    let awbc = Arc::new(
        arcweft_bundle::standard_view::install_dialogue_handler_awbc(
            AwbcLowerer::new(
                &compiled.runtime_plan().plan,
                &compiled.runtime_plan().dialogue_content_catalog,
                "main.arcw",
            )
            .lower()
            .unwrap()
            .program,
        )
        .unwrap(),
    );
    let handle = |id: &str| {
        PresentationHandleRecord::new(
            PresentationHandleId::try_new(id).unwrap(),
            PresentationHandleKind::View,
            "view.Main".to_owned(),
            None,
            PresentationResourceState::Mounted,
            None,
            0,
        )
    };
    let first = handle("view.state.first");
    let second = handle("view.state.second");
    let mut runtime =
        BundleViewRuntime::try_new_with_awbc(product.clone(), text.clone(), Arc::clone(&awbc))
            .unwrap();
    for (handles, supplied, expected) in [
        (vec![first.clone()], "first", vec!["first"]),
        (
            vec![first.clone(), second.clone()],
            "second",
            vec!["first", "second"],
        ),
        (
            vec![first.clone(), second.clone()],
            "third",
            vec!["first", "second"],
        ),
    ] {
        let inputs = [RuntimeBinding {
            name: "initial".to_owned(),
            value: RuntimeValue::String(supplied.to_owned()),
        }];
        let frame = runtime.evaluate(&handles, &inputs, false);
        assert!(frame.diagnostics.is_empty(), "{frame:#?}");
        let labels = frame
            .mounts
            .iter()
            .map(|mount| match &mount.text[0].value {
                BundleViewTextValue::Plain { value } => value.as_str(),
                _ => panic!("plain state String"),
            })
            .collect::<Vec<_>>();
        assert_eq!(labels, expected);
        let saved = runtime.snapshot().unwrap();
        assert!(
            saved
                .mounts
                .iter()
                .all(|mount| mount.local_state.len() == 1)
        );
        let mut forged = saved.clone();
        forged.mounts[0].local_state[0].value = RuntimeValue::Bool(true);
        let before = runtime.snapshot().unwrap();
        assert!(runtime.restore(&forged, &handles).is_err());
        assert_eq!(
            runtime.snapshot().unwrap(),
            before,
            "failed restore is atomic"
        );
        let mut duplicate = saved.clone();
        let duplicate_field = duplicate.mounts[0].local_state[0].clone();
        duplicate.mounts[0].local_state.push(duplicate_field);
        assert!(runtime.restore(&duplicate, &handles).is_err());
        assert_eq!(runtime.snapshot().unwrap(), before);
        let mut cold =
            BundleViewRuntime::try_new_with_awbc(product.clone(), text.clone(), Arc::clone(&awbc))
                .unwrap();
        cold.restore(&saved, &handles).unwrap();
        assert_eq!(frame, cold.evaluate(&handles, &inputs, false));
    }
}

#[test]
fn authored_local_state_identity_uses_name_and_scope_instead_of_initializer_or_position() {
    let source = r#"view Main() { local state value: String = "first"; Text(value) }"#;
    let fields = |source: &str| {
        let compiled = project_view_fixture(source, "arcweft-test://state-identity")
            .compile()
            .unwrap();
        let mut fields = Vec::new();
        for instruction in &compiled
            .view_product()
            .product()
            .program()
            .unwrap()
            .resource()
            .instructions
        {
            if let ViewProgramInstruction::BindLocal { program, .. } = instruction
                && let arcweft_view::ViewBindingLifetime::Retained { fields: identities } =
                    &program.lifetime
            {
                fields.extend(
                    identities
                        .iter()
                        .copied()
                        .zip(program.outputs.iter().map(|output| output.value_type)),
                );
            }
        }
        fields
    };
    let original = fields(source);
    assert_eq!(original.len(), 1);
    assert_eq!(
        original,
        fields(&source.replace("first", "changed initializer"))
    );
    assert_eq!(
        original,
        fields(&source.replace("local state", "let unrelated: i32 = 1; local state"))
    );
    let renamed = fields(&source.replace("value", "renamed"));
    assert_ne!(original[0].0, renamed[0].0);
    let shadowed = fields(&source.replace(
        "Text(value)",
        "local state value: String = \"second\"; Text(value)",
    ));
    assert_eq!(shadowed.len(), 2);
    assert_ne!(shadowed[0].0, shadowed[1].0);
    let changed_type = fields("view Main() { local state value: bool = true; Text(\"text\") }");
    assert_eq!(original[0].0, changed_type[0].0);
    assert_ne!(original[0].1, changed_type[0].1);
}

#[test]
fn authored_local_state_rejects_nonretained_and_latent_scopes() {
    for source in [
        "fn invalid() -> String { local state value: String = \"first\"; value } view Main() { Text(\"text\") }",
        "view Main() { let latent = || { local state value: String = \"first\"; value }; Text(\"text\") }",
        "view Main() { local state () = (); Text(\"text\") }",
    ] {
        assert!(
            project_view_fixture(source, "arcweft-test://state-rejection")
                .compile()
                .is_err(),
            "{source}"
        );
    }
}

#[test]
fn authored_local_state_follows_repeat_keys_and_rolls_back_failed_initialization() {
    use arcweft_core::value::{RuntimeBinding, RuntimeInt, RuntimeValue};
    use arcweft_runtime_driver::{
        presentation_handles::{
            PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
        },
        view_runtime::BundleViewTextValue,
    };
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
fn initialize(label: String, denominator: i64) -> String {
    match (1i64 / denominator) == 0i64 { true => label, false => label }
}
view Main(items: Vec<String>, initial: String, denominator: i64) {
    local state heading: String = initial
    for item in items key = item {
        local state caption: String = initialize(initial, denominator)
        Text(caption)
    }
}
"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://keyed-local-state")
        .compile()
        .unwrap();
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().cloned();
    let awbc = Arc::new(
        arcweft_bundle::standard_view::install_dialogue_handler_awbc(
            AwbcLowerer::new(
                &compiled.runtime_plan().plan,
                &compiled.runtime_plan().dialogue_content_catalog,
                "main.arcw",
            )
            .lower()
            .unwrap()
            .program,
        )
        .unwrap(),
    );
    let handle = PresentationHandleRecord::new(
        PresentationHandleId::try_new("view.keyed.state").unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let handles = std::slice::from_ref(&handle);
    let inputs = |keys: &[&str], initial: &str, denominator| {
        vec![
            RuntimeBinding {
                name: "items".to_owned(),
                value: RuntimeValue::Seq(arcweft_core::value::RuntimeSeq::values(
                    keys.iter()
                        .map(|key| RuntimeValue::String((*key).to_owned()))
                        .collect(),
                )),
            },
            RuntimeBinding {
                name: "initial".to_owned(),
                value: RuntimeValue::String(initial.to_owned()),
            },
            RuntimeBinding {
                name: "denominator".to_owned(),
                value: RuntimeValue::Int(RuntimeInt::i64(denominator)),
            },
        ]
    };
    let mut runtime =
        BundleViewRuntime::try_new_with_awbc(product.clone(), text.clone(), Arc::clone(&awbc))
            .unwrap();
    for (keys, initial, denominator, expected, failed) in [
        (vec!["a", "b"], "first", 1, vec!["first", "first"], false),
        (vec!["b", "a"], "changed", 0, vec!["first", "first"], false),
        (vec!["b", "a", "c"], "failing", 0, vec![], true),
        (
            vec!["b", "a", "c"],
            "third",
            1,
            vec!["first", "first", "third"],
            false,
        ),
        (vec!["b", "d"], "failed removal", 0, vec![], true),
        (vec!["b"], "unchanged", 0, vec!["first"], false),
        (
            vec!["b", "a"],
            "reintroduced",
            1,
            vec!["first", "reintroduced"],
            false,
        ),
        (vec![], "empty", 0, vec![], false),
        (vec!["b"], "after empty", 1, vec!["after empty"], false),
    ] {
        let before = runtime.snapshot().unwrap();
        let inputs = inputs(&keys, initial, denominator);
        let frame = runtime.evaluate(handles, &inputs, false);
        if failed {
            assert!(
                !frame.diagnostics.is_empty(),
                "new key initializer must fail"
            );
            let after = runtime.snapshot().unwrap();
            assert_eq!(
                after.mounts[0].local_state, before.mounts[0].local_state,
                "failed initialization rolls back every retained field"
            );
            continue;
        }
        assert!(frame.diagnostics.is_empty(), "{frame:#?}");
        let labels: Vec<_> = frame.mounts[0]
            .text
            .iter()
            .map(|text| match &text.value {
                BundleViewTextValue::Plain { value } => value.as_str(),
                _ => panic!("plain state label"),
            })
            .collect();
        assert_eq!(labels, expected);
        let snapshot = runtime.snapshot().unwrap();
        assert_eq!(snapshot.mounts[0].local_state.len(), keys.len() + 1);
        let mut cold =
            BundleViewRuntime::try_new_with_awbc(product.clone(), text.clone(), Arc::clone(&awbc))
                .unwrap();
        cold.restore(&snapshot, handles).unwrap();
        assert_eq!(cold.evaluate(handles, &inputs, false), frame);
        let mut forged = snapshot.clone();
        if let Some(field) = forged.mounts[0]
            .local_state
            .iter_mut()
            .find(|field| !field.path.segments().is_empty())
        {
            field.path = Default::default();
            assert!(
                runtime.restore(&forged, handles).is_err(),
                "repeat field cannot be restored outside its keyed occurrence"
            );
            assert_eq!(runtime.snapshot().unwrap(), snapshot);
        }
    }
}

#[test]
fn authored_local_state_replacement_preserves_fields_rejects_types_and_removes_fields() {
    use arcweft_core::value::RuntimeValue;
    use arcweft_runtime_driver::{
        presentation_handles::{
            PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
        },
        view_runtime::{BundleViewTextValue, ViewMountReconcileError, ViewProgramReplacementError},
    };
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main() { local state value: String = "first"; Text(value) }
view Alternate() { let replacement: String = "second"; Text(replacement) }
view Different() { let replacement: bool = true; Text("different type") }
"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://state-replacement")
        .compile()
        .unwrap();
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().cloned();
    let awbc = Arc::new(
        arcweft_bundle::standard_view::install_dialogue_handler_awbc(
            AwbcLowerer::new(
                &compiled.runtime_plan().plan,
                &compiled.runtime_plan().dialogue_content_catalog,
                "main.arcw",
            )
            .lower()
            .unwrap()
            .program,
        )
        .unwrap(),
    );
    let resource = product.program().unwrap().resource();
    let instructions = |view: &str| {
        let definition = resource
            .definitions
            .iter()
            .find(|definition| definition.public_id.as_str() == view)
            .unwrap();
        resource.instructions
            [definition.body.start_instruction as usize..definition.body.end_instruction as usize]
            .iter()
            .enumerate()
            .map(|(index, instruction)| {
                (
                    definition.body.start_instruction as usize + index,
                    instruction,
                )
            })
            .collect::<Vec<_>>()
    };
    let main = instructions("view.Main");
    let (binding_index, original_binding) = main
        .iter()
        .find_map(|(index, instruction)| match instruction {
            ViewProgramInstruction::BindLocal { program, .. } => Some((*index, program.clone())),
            _ => None,
        })
        .unwrap();
    let (text_index, block, original_source) = main
        .iter()
        .find_map(|(index, instruction)| match instruction {
            ViewProgramInstruction::EmitText {
                text_block,
                text_source,
                ..
            } => Some((*index, text_block.clone(), text_source.clone())),
            _ => None,
        })
        .unwrap();
    let candidate = |alternate: &str, retain: bool| {
        let alternate = instructions(alternate);
        let (alternate_binding_index, mut binding) = alternate
            .iter()
            .find_map(|(index, instruction)| match instruction {
                ViewProgramInstruction::BindLocal { program, .. } => {
                    Some((*index, program.clone()))
                }
                _ => None,
            })
            .unwrap();
        binding.lifetime = if retain {
            original_binding.lifetime.clone()
        } else {
            arcweft_view::ViewBindingLifetime::Derived
        };
        let (alternate_text_index, alternate_block, source) = alternate
            .iter()
            .find_map(|(index, instruction)| match instruction {
                ViewProgramInstruction::EmitText {
                    text_source,
                    text_block,
                    ..
                } => Some((*index, text_block.clone(), text_source.clone())),
                _ => None,
            })
            .unwrap();
        let mut changed = resource.clone();
        // Keep every checked text program owned and pair each reader with its
        // exact binding ABI while replacing the mounted declaration's initializer.
        let mut displaced = original_binding.clone();
        displaced.lifetime = arcweft_view::ViewBindingLifetime::Derived;
        changed.instructions[alternate_binding_index] = ViewProgramInstruction::BindLocal {
            program: displaced,
            source: None,
        };
        let ViewProgramInstruction::EmitText { text_source, .. } =
            &mut changed.instructions[alternate_text_index]
        else {
            unreachable!()
        };
        *text_source = original_source.clone();
        changed
            .text_blocks
            .iter_mut()
            .find(|text| text.public_id == alternate_block)
            .unwrap()
            .text_source = original_source.clone();
        changed.instructions[binding_index] = ViewProgramInstruction::BindLocal {
            program: binding,
            source: None,
        };
        let ViewProgramInstruction::EmitText { text_source, .. } =
            &mut changed.instructions[text_index]
        else {
            unreachable!()
        };
        *text_source = source.clone();
        changed
            .text_blocks
            .iter_mut()
            .find(|text| text.public_id == block)
            .unwrap()
            .text_source = source;
        ValidatedViewProduct::try_new(
            Some(product.source_map().clone()),
            Some(changed),
            product.style().map(|style| style.resource().clone()),
            ViewProductValidationLimits::default(),
        )
        .unwrap()
    };
    let handle = |id: &str| {
        PresentationHandleRecord::new(
            PresentationHandleId::try_new(id).unwrap(),
            PresentationHandleKind::View,
            "view.Main".to_owned(),
            None,
            PresentationResourceState::Mounted,
            None,
            0,
        )
    };
    let first = handle("view.state.replacement.first");
    let second = handle("view.state.replacement.second");
    let mut runtime = BundleViewRuntime::try_new_with_awbc(product.clone(), text, awbc).unwrap();
    assert!(
        runtime
            .evaluate(std::slice::from_ref(&first), &[], false)
            .diagnostics
            .is_empty()
    );
    let before = runtime.snapshot().unwrap();
    let rejected = runtime.prepare_view_program_replacement(candidate("view.Different", true));
    assert!(
        matches!(
            &rejected,
            Err(ViewProgramReplacementError::Reconcile(
                ViewMountReconcileError::LocalStateTypeChanged
            ))
        ),
        "{:?}",
        rejected.as_ref().err()
    );
    assert_eq!(
        runtime.snapshot().unwrap(),
        before,
        "type rejection is atomic"
    );
    let prepared = runtime
        .prepare_view_program_replacement(candidate("view.Alternate", true))
        .unwrap();
    runtime.commit_view_program_replacement(prepared).unwrap();
    let frame = runtime.evaluate(&[first.clone(), second.clone()], &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    let labels: Vec<_> = frame
        .mounts
        .iter()
        .map(|mount| match &mount.text[0].value {
            BundleViewTextValue::Plain { value } => value.as_str(),
            _ => panic!("plain retained label"),
        })
        .collect();
    assert_eq!(
        labels,
        ["first", "second"],
        "old mount keeps its value, new mount runs the new initializer"
    );
    let prepared = runtime
        .prepare_view_program_replacement(candidate("view.Alternate", false))
        .unwrap();
    runtime.commit_view_program_replacement(prepared).unwrap();
    let frame = runtime.evaluate(&[first, second], &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    assert!(
        runtime
            .snapshot()
            .unwrap()
            .mounts
            .iter()
            .all(|mount| mount.local_state.is_empty())
    );
    assert!(frame.mounts.iter().all(|mount| matches!(&mount.text[0].value, BundleViewTextValue::Plain { value } if value == "second")));
    assert!(before.mounts.iter().all(|mount| matches!(&mount.local_state[0].value, RuntimeValue::String(value) if value == "first")));
}

#[test]
fn authored_local_state_scope_retirement_keeps_inactive_sources_and_retires_descendants() {
    use arcweft_core::value::{RuntimeBinding, RuntimeSeq, RuntimeValue};
    use arcweft_runtime_driver::{
        presentation_handles::{
            PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
        },
        view_runtime::BundleViewTextValue,
    };
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main(items: Vec<String>, initial: String, show: bool) {
    local state heading: String = initial
    for item in items key = item {
        local state caption: String = initial
        Text(caption)
        if show {
            for child in ["x", "y"] key = child {
                local state child_caption: String = initial
                Text(child_caption)
            }
        }
    }
}
"#;
    let compiled = project_view_fixture_with_entry(source, "arcweft-test://nested-state-scope")
        .compile()
        .unwrap();
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().cloned();
    let awbc = Arc::new(
        arcweft_bundle::standard_view::install_dialogue_handler_awbc(
            AwbcLowerer::new(
                &compiled.runtime_plan().plan,
                &compiled.runtime_plan().dialogue_content_catalog,
                "main.arcw",
            )
            .lower()
            .unwrap()
            .program,
        )
        .unwrap(),
    );
    let handle = PresentationHandleRecord::new(
        PresentationHandleId::try_new("view.nested.state.scope").unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    );
    let handles = std::slice::from_ref(&handle);
    let mut runtime =
        BundleViewRuntime::try_new_with_awbc(product.clone(), text.clone(), Arc::clone(&awbc))
            .unwrap();
    for (keys, initial, show, cells, expected) in [
        (vec!["a", "b"], "first", true, 7, vec!["first"; 6]),
        (vec!["a", "b"], "changed", false, 7, vec!["first"; 2]),
        (vec!["b"], "hidden", false, 4, vec!["first"]),
        (
            vec!["b", "a"],
            "fresh",
            true,
            7,
            vec!["first", "first", "first", "fresh", "fresh", "fresh"],
        ),
    ] {
        let inputs = [
            RuntimeBinding {
                name: "items".to_owned(),
                value: RuntimeValue::Seq(RuntimeSeq::values(
                    keys.iter()
                        .map(|key| RuntimeValue::String((*key).to_owned()))
                        .collect(),
                )),
            },
            RuntimeBinding {
                name: "initial".to_owned(),
                value: RuntimeValue::String(initial.to_owned()),
            },
            RuntimeBinding {
                name: "show".to_owned(),
                value: RuntimeValue::Bool(show),
            },
        ];
        let frame = runtime.evaluate(handles, &inputs, false);
        assert!(frame.diagnostics.is_empty(), "{frame:#?}");
        let labels: Vec<_> = frame.mounts[0]
            .text
            .iter()
            .map(|text| match &text.value {
                BundleViewTextValue::Plain { value } => value.as_str(),
                _ => panic!("plain state text"),
            })
            .collect();
        assert_eq!(labels, expected);
        let saved = runtime.snapshot().unwrap();
        assert_eq!(saved.mounts[0].local_state.len(), cells);
        let mut cold =
            BundleViewRuntime::try_new_with_awbc(product.clone(), text.clone(), Arc::clone(&awbc))
                .unwrap();
        cold.restore(&saved, handles).unwrap();
        assert_eq!(cold.evaluate(handles, &inputs, false), frame);
    }
}

fn retained_transition_runtime(compiled: &CompiledProject) -> BundleViewRuntime {
    let product = compiled.view_product().product().as_ref().clone();
    let text = compiled.view_product().text().cloned();
    let program = arcweft_bundle::standard_view::install_dialogue_handler_awbc(
        AwbcLowerer::new(
            &compiled.runtime_plan().plan,
            &compiled.runtime_plan().dialogue_content_catalog,
            "main.arcw",
        )
        .lower()
        .unwrap()
        .program,
    )
    .unwrap();
    let bytes = program.encode_canonical().unwrap();
    let program = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
        &bytes,
        arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
    )
    .unwrap();
    BundleViewRuntime::try_new_with_awbc(product, text, Arc::new(program)).unwrap()
}

fn retained_transition_handle(
    id: &str,
) -> arcweft_runtime_driver::presentation_handles::PresentationHandleRecord {
    use arcweft_runtime_driver::presentation_handles::{
        PresentationHandleKind, PresentationHandleRecord, PresentationResourceState,
    };
    PresentationHandleRecord::new(
        PresentationHandleId::try_new(id).unwrap(),
        PresentationHandleKind::View,
        "view.Main".to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    )
}

fn retained_transition_invocation(
    binding: &arcweft_runtime_driver::view_runtime::BundleViewEventBinding,
) -> ViewHandlerInvocation {
    ViewHandlerInvocation::from_input(
        &InputEvent::activate(InputEpoch(1), binding.target().clone()),
        binding.event(),
        binding.route(),
    )
    .unwrap()
}

#[test]
fn retained_transition_failure_leaves_every_cell_and_route_unchanged() {
    use arcweft_core::pure::{RuntimePureCallBackend, VmRuntimePureCallBackend};
    use arcweft_runtime_driver::view_runtime::BundleViewEventDispatchError;
    let compiled = project_view_fixture_with_entry(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main() {
    local state mut count: i64 = 3i64
    local state mut caption: String = "first"
    Text(caption)
    Button("fail").on_click(|| {
        caption = "changed"
        let divisor = count - count
        count = 1i64 / divisor
        ()
    })
}
"#,
        "arcweft-test://retained-transition-failure",
    )
    .compile()
    .unwrap();
    let mut runtime = retained_transition_runtime(&compiled);
    let handle = retained_transition_handle("view.transition.failure");
    let frame = runtime.evaluate(std::slice::from_ref(&handle), &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    let invocation = retained_transition_invocation(&frame.mounts[0].events[0]);
    let before = runtime.snapshot().unwrap();
    let mut backend = VmRuntimePureCallBackend::default();
    assert!(matches!(
        runtime.dispatch_invocation_with_backend(&invocation, &mut backend),
        Err(BundleViewEventDispatchError::ProgramExecution { .. })
    ));
    assert_eq!(backend.stats().awbc_pure_program_calls, 1);
    assert_eq!(runtime.snapshot().unwrap(), before);
    assert!(matches!(
        runtime.dispatch_invocation(&invocation),
        Err(BundleViewEventDispatchError::ProgramExecution { .. })
    ));
    assert_eq!(runtime.snapshot().unwrap(), before);
}

#[test]
fn retained_transition_refreshes_unchanged_routes_and_keeps_other_mounts_independent() {
    use arcweft_core::value::RuntimeValue;
    use arcweft_runtime_driver::view_runtime::BundleViewEventDispatchError;
    let compiled = project_view_fixture_with_entry(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main() {
    local state mut count: i64 = 0i64
    Button("increment").on_click(|| { count = count + 1i64; () })
    Button("noop").on_click(|| ())
}
"#,
        "arcweft-test://retained-transition-isolation",
    )
    .compile()
    .unwrap();
    let mut runtime = retained_transition_runtime(&compiled);
    let handles = [
        retained_transition_handle("view.transition.first"),
        retained_transition_handle("view.transition.other"),
    ];
    let frame = runtime.evaluate(&handles, &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    let first = retained_transition_invocation(&frame.mounts[0].events[0]);
    let noop = retained_transition_invocation(&frame.mounts[0].events[1]);
    let other = retained_transition_invocation(&frame.mounts[1].events[0]);
    runtime.dispatch_invocation(&first).unwrap();
    let snapshot = runtime.snapshot().unwrap();
    assert_eq!(
        snapshot.mounts[0].local_state[0].value,
        RuntimeValue::i64(1)
    );
    assert_eq!(
        snapshot.mounts[1].local_state[0].value,
        RuntimeValue::i64(0)
    );
    assert!(matches!(
        runtime.dispatch_invocation(&noop),
        Err(BundleViewEventDispatchError::UnknownBinding)
    ));
    let refreshed = runtime.evaluate(&handles, &[], false);
    assert!(refreshed.diagnostics.is_empty(), "{refreshed:#?}");
    assert_ne!(
        frame.mounts[0].events[1].route(),
        refreshed.mounts[0].events[1].route()
    );
    assert!(matches!(
        runtime.dispatch_invocation(&noop),
        Err(BundleViewEventDispatchError::UnknownBinding)
    ));
    runtime.dispatch_invocation(&other).unwrap();
    let snapshot = runtime.snapshot().unwrap();
    assert_eq!(
        snapshot.mounts[0].local_state[0].value,
        RuntimeValue::i64(1)
    );
    assert_eq!(
        snapshot.mounts[1].local_state[0].value,
        RuntimeValue::i64(1)
    );
}

#[test]
fn retained_keyed_transition_updates_one_cell_and_restores_without_replaying_handler() {
    use arcweft_core::value::RuntimeValue;
    let compiled = project_view_fixture_with_entry(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main() {
    for row in [1i64, 2i64] key = row {
        local state mut count: i64 = row
        Button("increment").on_click(|| { count = count + 1i64; () })
    }
}
"#,
        "arcweft-test://retained-transition-keyed",
    )
    .compile()
    .unwrap();
    let mut runtime = retained_transition_runtime(&compiled);
    let handle = retained_transition_handle("view.transition.keyed");
    let frame = runtime.evaluate(std::slice::from_ref(&handle), &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    assert_eq!(frame.mounts[0].events.len(), 2);
    let before = runtime.snapshot().unwrap();
    let first = before.mounts[0]
        .local_state
        .iter()
        .find(|cell| cell.value == RuntimeValue::i64(1))
        .unwrap();
    runtime
        .dispatch_invocation(&retained_transition_invocation(&frame.mounts[0].events[0]))
        .unwrap();
    let after = runtime.snapshot().unwrap();
    assert_eq!(after.mounts[0].local_state.len(), 2);
    assert_eq!(
        after.mounts[0]
            .local_state
            .iter()
            .find(|cell| cell.field == first.field && cell.path == first.path)
            .unwrap()
            .value,
        RuntimeValue::i64(2)
    );
    assert!(
        after.mounts[0]
            .local_state
            .iter()
            .all(|cell| cell.value == RuntimeValue::i64(2))
    );
    let mut cold = retained_transition_runtime(&compiled);
    cold.restore(&after, std::slice::from_ref(&handle)).unwrap();
    let restored = cold.evaluate(std::slice::from_ref(&handle), &[], false);
    assert!(restored.diagnostics.is_empty(), "{restored:#?}");
    assert!(
        cold.snapshot().unwrap().mounts[0]
            .local_state
            .iter()
            .all(|cell| cell.value == RuntimeValue::i64(2))
    );
    assert!(
        cold.dispatch_invocation(&retained_transition_invocation(&frame.mounts[0].events[0]))
            .is_err()
    );
    cold.dispatch_invocation(&retained_transition_invocation(
        &restored.mounts[0].events[0],
    ))
    .unwrap();
    assert_eq!(
        cold.snapshot().unwrap().mounts[0]
            .local_state
            .iter()
            .filter(|cell| cell.value == RuntimeValue::i64(3))
            .count(),
        1
    );
}

#[test]
fn forged_state_write_cannot_retarget_another_same_typed_retained_field() {
    let compiled = project_view_fixture_with_entry(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
view Main() {
    local state mut first: i64 = 0i64
    local state mut second: i64 = 99i64
    Button("first").on_click(|| { first = 1i64; () })
}
"#,
        "arcweft-test://retained-transition-forged-field",
    )
    .compile()
    .unwrap();
    let mut program = compiled
        .view_product()
        .product()
        .program()
        .unwrap()
        .resource()
        .clone();
    assert!(program.encode_canonical_section().is_ok());
    let fields = program
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            ViewProgramInstruction::BindLocal { program, .. } => match &program.lifetime {
                arcweft_view::ViewBindingLifetime::Retained { fields } => Some(fields.as_ref()),
                arcweft_view::ViewBindingLifetime::Derived => None,
            },
            _ => None,
        })
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 2);
    let handler = program
        .handlers
        .iter_mut()
        .find(|handler| {
            matches!(
                handler.result.role(),
                ViewHandlerResultRole::StateTransition { .. }
            )
        })
        .unwrap();
    let ViewHandlerResultRole::StateTransition { value, writes } = handler.result.role() else {
        unreachable!()
    };
    let input = writes[0].input();
    let other = fields
        .into_iter()
        .find(|field| *field != writes[0].field())
        .unwrap();
    let role = ViewHandlerResultRole::StateTransition {
        value: *value,
        writes: vec![arcweft_view::ViewHandlerStateWrite::try_new(input, other).unwrap()]
            .into_boxed_slice(),
    };
    handler.result = arcweft_view::ViewHandlerResult::new(role, handler.result.value_type());
    assert!(matches!(
        program.encode_canonical_section(),
        Err(
            arcweft_bundle::resource_codec::SectionCodecError::NonCanonicalTable(
                "view_handler_bindings"
            )
        )
    ));
}

#[test]
fn retained_nested_transition_early_return_matches_native_and_decoded_awbc() {
    use arcweft_core::{task::RuntimeProgramOwner, value::AwbcRuntimeValueSnapshot};
    use arcweft_runtime_driver::view_runtime::BundleViewTextValue;
    let compiled = project_view_fixture_with_entry(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String { return "done" }
struct Leaf { count: i64, caption: String }
struct Root { leaf: Leaf }
view Main() {
    local state mut root: Root = Root { leaf = Leaf { count = 0i64, caption = "first" } }
    Text(root.leaf.caption)
    Button("change").on_click(|| {
        root.leaf.count = 40i64
        if root.leaf.count == 40i64 {
            root.leaf.caption = "changed"
            return ()
        }
        root.leaf.count = 0i64
        ()
    })
}
"#,
        "arcweft-test://retained-transition-native-early-return",
    )
    .compile()
    .unwrap();
    let mut runtime = retained_transition_runtime(&compiled);
    let handle = retained_transition_handle("view.transition.native");
    let frame = runtime.evaluate(std::slice::from_ref(&handle), &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    let initial = runtime.snapshot().unwrap().mounts[0].local_state[0]
        .value
        .clone();
    let program = compiled
        .view_product()
        .product()
        .program()
        .unwrap()
        .resource()
        .handlers[0]
        .program;
    let lowered = arcweft_bundle::standard_view::install_dialogue_handler_awbc(
        AwbcLowerer::new(
            &compiled.runtime_plan().plan,
            &compiled.runtime_plan().dialogue_content_catalog,
            "main.arcw",
        )
        .lower()
        .unwrap()
        .program,
    )
    .unwrap();
    let awbc = Arc::new(
        arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
            &lowered.encode_canonical().unwrap(),
            arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
        )
        .unwrap(),
    );
    let awbc_owner = RuntimeProgramOwner::Awbc(Arc::clone(&awbc));
    let image =
        AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&initial, &awbc_owner).unwrap();
    let plan = Arc::new(compiled.runtime_plan().plan.clone());
    let native_owner = RuntimeProgramOwner::Plan(Arc::clone(&plan));
    let native_input = image.into_runtime_value_for_program(&native_owner).unwrap();
    let mut engine =
        arcweft_core::engine::Engine::for_program_invocation(plan, program, vec![native_input])
            .unwrap();
    let mut native_result = None;
    for _ in 0..64 {
        let output = engine.step(Default::default(), Default::default()).output;
        assert!(output.diagnostics.is_empty(), "{output:?}");
        if let Some((returned, value)) = engine.take_program_result().unwrap() {
            assert_eq!(returned, program);
            native_result = Some(value);
            break;
        }
    }
    let native_result = native_result.expect("native transition must terminate");
    let decoded_result = arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
        &awbc,
        program,
        &[initial],
        &mut arcweft_core::pure::VmRuntimePureCallBackend::default(),
    )
    .unwrap();
    let arcweft_core::value::RuntimeValue::Tuple(outputs) = &decoded_result else {
        panic!("transition must return its publication tuple");
    };
    let arcweft_core::value::RuntimeValue::NominalRecord(root) = &outputs[1] else {
        panic!("updated Root must retain its nominal owner");
    };
    let arcweft_core::value::RuntimeValue::NominalRecord(leaf) = &root.fields()[0] else {
        panic!("updated Leaf must retain its nominal owner");
    };
    assert_eq!(
        leaf.fields()[0],
        arcweft_core::value::RuntimeValue::i64(40),
        "the write after early return must not execute"
    );
    assert_eq!(
        AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&native_result, &native_owner)
            .unwrap(),
        AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&decoded_result, &awbc_owner)
            .unwrap(),
    );
    runtime
        .dispatch_invocation(&retained_transition_invocation(&frame.mounts[0].events[0]))
        .unwrap();
    let updated = runtime.evaluate(std::slice::from_ref(&handle), &[], false);
    assert!(updated.diagnostics.is_empty(), "{updated:#?}");
    assert!(
        matches!(&updated.mounts[0].text[0].value, BundleViewTextValue::Plain { value } if value == "changed")
    );
}
