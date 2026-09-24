use super::super::{
    BundleEntryStart, BundleSession, BundleSessionOptions, BundleSessionSaveError, BundleStepInput,
    GenerationId, ProgramGeneration, RuntimeClockStep, SwapCompatibility,
};
use arcweft_bundle::{
    ArcweftBundle, BundleManifest, BundleRuntimeSummary,
    resource_codec::{
        SourceMapSection, SourceRangeRef,
        view::{ViewStyleResource, ViewThemeResource},
    },
};
use arcweft_core::{
    effect::RuntimeArtifactFingerprint,
    entry::{
        EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
        RuntimeFlowSchema,
    },
    plan::{
        EntryRuntimeId, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
        RuntimeFlowOpSeed, RuntimeFlowSeed, RuntimePlanBuilder,
    },
};
use arcweft_presentation::appearance::{
    ColorScheme, PresentationColor, PresentationEnvironmentOverrides, PresentationEnvironmentValue,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_text_model::DialogueContentCatalog;
use arcweft_view::{
    ViewPartName,
    style::{
        ViewColorValue, ViewPropertyKind, ViewSpecifiedValue, ViewStyleAssignOp,
        ViewStyleDeclaration, ViewStyleProgram, ViewStyleRule, ViewStyleSelector,
        ViewStyleSelectorSequence, ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId,
    },
};

fn bundle(red: u8, scheme: ColorScheme) -> ArcweftBundle {
    bundle_with_source_note(red, scheme, "")
}

fn bundle_with_source_note(red: u8, scheme: ColorScheme, note: &str) -> ArcweftBundle {
    let mut builder = RuntimePlanBuilder::new();
    let flow = FlowRuntimeId::from_checked_declaration_digest([0x51; 32], "flow.main").unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            [],
            arcweft_core::plan::RuntimeEffectSet::empty(),
            vec![RuntimeFlowOpSeed::Return("done".to_owned())],
        ))
        .unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: Vec::new(),
        })
        .unwrap();
    builder
        .push_flow_executable(RuntimeFlowExecutable {
            flow: flow.clone(),
            contract: FlowContractHash::from_bytes([0x52; 32]),
            controller: None,
        })
        .unwrap();
    builder
        .push_entry(RuntimeEntrySpec {
            id: EntryRuntimeId::from_source_entity_body("entry.main").unwrap(),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([0x53; 32]),
            target: RuntimeEntryTarget::Flow(flow),
            roles: RuntimeEntryRoles::None,
        })
        .unwrap();
    let plan = builder.finish().unwrap();
    let dialogue = DialogueContentCatalog::new();
    let awbc = AwbcLowerer::new(&plan, &dialogue, "presentation-generation.arcw")
        .lower()
        .unwrap()
        .program;
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("presentation-generation-fixture").unwrap(),
        SourceName::Memory,
        format!(
            "style dialogue_lease {{ content {{ color = rgb(\"#{red:02x}1020\") }} }} // {note}"
        ),
    )
    .unwrap();
    let sources = SourceMapSection::try_from_documents(&[&source]).unwrap();
    let style = style_resource(red, &sources);
    let mut environment = PresentationEnvironmentOverrides::empty();
    environment.insert(PresentationEnvironmentValue::ColorScheme(scheme));
    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some("entry.main".to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                artifact_fingerprint: RuntimeArtifactFingerprint::try_from_bytes([0x54; 32])
                    .unwrap(),
                entry_flow: Some("flow.main".to_owned()),
                flows: 1,
                bytecode_instructions: awbc.instructions.len(),
                line_task_groups: 0,
                stream_plans: 0,
            },
        },
        sources,
        awbc,
        dialogue,
    )
    .unwrap()
    .with_view_resources(None, Some(style))
    .unwrap()
    .with_view_theme(ViewThemeResource {
        environment,
        ..ViewThemeResource::default()
    })
}

fn style_resource(red: u8, sources: &SourceMapSection) -> ViewStyleResource {
    let source_ref = sources.documents().next().unwrap().product_source_ref();
    let source_refs = vec![source_ref.clone()];
    let source_range = SourceRangeRef::try_for_source(&source_refs, &source_ref, 0, 1).unwrap();
    let style_source = ViewStyleSourceId::new(0);
    let selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(
            None,
            None,
            Some(ViewPartName::try_new("content").unwrap()),
            Vec::new(),
        )
        .unwrap(),
    ])
    .unwrap();
    let declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::Color,
        ViewSpecifiedValue::Color {
            value: ViewColorValue::Literal {
                color: PresentationColor::rgba(red, 16, 32, 255),
            },
        },
        ViewStyleAssignOp::Replace,
        style_source,
    )
    .unwrap();
    let sheet = ViewStyleSheet::new(
        ViewStyleSheetId::try_new("style.dialogue_lease").unwrap(),
        Vec::new(),
        vec![ViewStyleRule::new(selector, None, vec![declaration], 0, style_source).unwrap()],
    )
    .unwrap();
    ViewStyleResource {
        style_program_id: "view_style.presentation_generation".to_owned(),
        program: ViewStyleProgram::try_new(vec![sheet], Vec::new()).unwrap(),
        source_refs,
        source_map_refs: vec![source_range],
        adapter_requirements: Vec::new(),
    }
}

fn commit_generational(session: &mut BundleSession, bundle: &ArcweftBundle) {
    let report = session.hot_swap_bundle(bundle).unwrap();
    assert_eq!(report.compatibility, SwapCompatibility::CodeGenerational);
}

#[test]
fn source_only_style_change_retains_content_only_compatibility() {
    let first = bundle(10, ColorScheme::Light);
    let second = bundle_with_source_note(10, ColorScheme::Light, "source-only change");
    let mut session = BundleSession::new(&first, BundleSessionOptions::default()).unwrap();
    let before = session.view_style_program().unwrap().clone();
    let report = session.hot_swap_bundle(&second).unwrap();
    assert_eq!(report.compatibility, SwapCompatibility::ContentOnly);
    assert_eq!(session.presentation_generation.id, GenerationId::new(1));
    assert_eq!(session.view_style_program(), Some(&before));
}

#[test]
fn retired_presentation_survives_chained_content_only_swap_and_fiber_completion() {
    let first = bundle(10, ColorScheme::Light);
    let second = bundle(20, ColorScheme::Dark);
    let mut session = BundleSession::new(&first, BundleSessionOptions::default()).unwrap();
    let original_style = session.view_style_program().unwrap().clone();
    let original_environment = session.presentation_environment();
    commit_generational(&mut session, &second);
    let mut third = second.clone();
    third.manifest.profile_id = Some("profile.next_content".to_owned());
    let candidate = ProgramGeneration::from_bundle(GenerationId::new(2), &third).unwrap();
    assert_eq!(
        crate::swap::classify_swap(session.active_generation(), &candidate),
        SwapCompatibility::ContentOnly
    );
    let report = session.hot_swap_bundle(&third).unwrap();
    assert_eq!(report.compatibility, SwapCompatibility::CodeGenerational);
    assert_eq!(session.presentation_generation.id, GenerationId::new(0));
    assert_eq!(session.view_style_program(), Some(&original_style));
    assert_eq!(session.presentation_environment(), original_environment);

    let step = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).unwrap(),
        BundleStepInput::default(),
    );
    assert!(step.finished, "{:?}", step.diagnostics);
    assert_eq!(session.current_fiber_generation(), None);
    session.retire_unused_generations();
    assert!(session.has_runtime_image(GenerationId::new(0)));
    assert!(matches!(
        session.snapshot_session(),
        Err(BundleSessionSaveError::GenerationMismatch {
            field: "presentation_generation",
            ..
        })
    ));

    session
        .start_foreground_entry_on_current_generation(BundleEntryStart::SessionDefault)
        .unwrap();
    assert_eq!(session.presentation_generation.id, GenerationId::new(2));
    assert_eq!(
        session.current_fiber_generation(),
        Some(GenerationId::new(2))
    );
    assert_eq!(
        session.presentation_environment().color_scheme(),
        ColorScheme::Dark
    );
    assert_ne!(session.view_style_program(), Some(&original_style));
    assert!(!session.has_runtime_image(GenerationId::new(0)));
    let snapshot = session.snapshot_session().unwrap();
    session.restore_session_snapshot(snapshot.clone()).unwrap();
    assert_eq!(session.snapshot_session().unwrap(), snapshot);
}

#[test]
fn unchanged_active_bundle_is_a_noop_while_a_retired_presentation_is_installed() {
    let first = bundle(10, ColorScheme::Light);
    let second = bundle(20, ColorScheme::Dark);
    let mut session = BundleSession::new(&first, BundleSessionOptions::default()).unwrap();
    commit_generational(&mut session, &second);
    let old_owner = session.presentation_generation.clone();
    let old_view = session.view_runtime.snapshot().unwrap();
    let next_id = session.next_generation_id;
    let report = session.hot_swap_bundle(&second).unwrap();
    assert_eq!(report.generation, GenerationId::new(1));
    assert_eq!(report.compatibility, SwapCompatibility::ContentOnly);
    assert!(std::sync::Arc::ptr_eq(
        &session.presentation_generation,
        &old_owner
    ));
    assert_eq!(session.view_runtime.snapshot().unwrap(), old_view);
    assert_eq!(session.next_generation_id, next_id);
}

#[test]
fn content_only_replacement_switches_presentation_owner_without_relabeling_the_fiber() {
    let first = bundle(10, ColorScheme::Light);
    let mut second = first.clone();
    second.manifest.profile_id = Some("profile.next_content".to_owned());
    let mut session = BundleSession::new(&first, BundleSessionOptions::default()).unwrap();
    let report = session.hot_swap_bundle(&second).unwrap();
    assert_eq!(report.compatibility, SwapCompatibility::ContentOnly);
    assert_eq!(session.presentation_generation.id, GenerationId::new(1));
    assert_eq!(
        session.current_fiber_generation(),
        Some(GenerationId::new(0))
    );
    assert!(matches!(
        session.snapshot_session(),
        Err(BundleSessionSaveError::GenerationMismatch {
            field: "runtime_generation_pin",
            ..
        })
    ));
    let step = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).unwrap(),
        BundleStepInput::default(),
    );
    assert!(step.finished, "{:?}", step.diagnostics);
    let snapshot = session.snapshot_session().unwrap();
    session.restore_session_snapshot(snapshot.clone()).unwrap();
    assert_eq!(session.snapshot_session().unwrap(), snapshot);
    assert!(!session.has_runtime_image(GenerationId::new(0)));
}

#[test]
fn restore_rejects_a_retired_installed_presentation_before_reinterpreting_its_view_state() {
    let first = bundle(10, ColorScheme::Light);
    let second = bundle(20, ColorScheme::Dark);
    let mut session = BundleSession::new(&first, BundleSessionOptions::default()).unwrap();
    let snapshot = session.snapshot_session().unwrap();
    commit_generational(&mut session, &second);
    let original_style = session.view_style_program().unwrap().clone();
    assert!(matches!(
        session.restore_session_snapshot(snapshot),
        Err(BundleSessionSaveError::GenerationMismatch {
            field: "presentation_generation",
            ..
        })
    ));
    assert_eq!(session.presentation_generation.id, GenerationId::new(0));
    assert_eq!(session.view_style_program(), Some(&original_style));
}
