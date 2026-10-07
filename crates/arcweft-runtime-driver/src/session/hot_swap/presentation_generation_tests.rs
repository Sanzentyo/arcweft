use super::super::{
    BundleEntryStart, BundleHotSwapError, BundleSession, BundleSessionArtifactIdentity,
    BundleSessionOptions, BundleSessionSaveError, BundleStepInput, GenerationId, ProgramGeneration,
    RuntimeClockStep, SwapCompatibility,
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
    task::{
        CancelScopeId, HostTaskRequest, LogicalEpoch, TaskClass, TaskPolicy, TaskPriority,
        TaskSequence, TaskSpec,
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

#[test]
fn generation_counter_exhaustion_rejects_swap_before_commit() {
    let first = bundle(10, ColorScheme::Light);
    let second = bundle(20, ColorScheme::Dark);
    let mut session = BundleSession::new(&first, BundleSessionOptions::default()).unwrap();
    session.next_generation_id = u64::MAX;

    assert!(matches!(
        session.hot_swap_bundle(&second),
        Err(BundleHotSwapError::GenerationIdExhausted)
    ));
    assert_eq!(session.active_generation().id, GenerationId::new(0));
    assert_eq!(session.next_generation_id, u64::MAX);
}

#[test]
fn restore_rejects_reused_generation_counter() {
    let first = bundle(10, ColorScheme::Light);
    let mut session = BundleSession::new(&first, BundleSessionOptions::default()).unwrap();
    let step = session.step_with_clock(
        RuntimeClockStep::from_millis(1, 16).unwrap(),
        BundleStepInput::default(),
    );
    assert!(step.finished, "{:?}", step.diagnostics);
    let mut snapshot = session.snapshot_session().unwrap();
    snapshot.runtime.next_generation_id = session.active_generation().id.get();

    assert!(matches!(
        session.restore_session_snapshot(snapshot),
        Err(BundleSessionSaveError::GenerationMismatch {
            field: "next_generation_id",
            ..
        })
    ));
}

#[test]
fn local_task_cancellation_reaches_runtime_and_releases_its_generation_pin() {
    let mut session = BundleSession::new(
        &bundle(10, ColorScheme::Light),
        BundleSessionOptions::default(),
    )
    .unwrap();
    let sequence = TaskSequence(7);
    let dispatch = crate::task::HostTaskDispatch {
        logical_epoch: LogicalEpoch(1),
        sequence,
        last_publication_revision: None,
        bundle_asset_context: None,
        task: admitted_test_task("task.cancelled"),
    };
    session.tasks.register_dispatch(&dispatch);
    session
        .task_generation_pins
        .insert(sequence, session.swap.pin_active_generation());
    session.cancel_runtime_tasks(&crate::task::RuntimeTaskCancelTarget::All);

    let prepared = session.prepare_step_input(
        RuntimeClockStep::from_millis(1, 16).unwrap(),
        BundleStepInput::default(),
    );
    assert!(matches!(
        prepared.runtime.task_events.as_slice(),
        [arcweft_core::task::TaskEvent {
            correlation,
            kind: arcweft_core::task::TaskEventKind::Cancelled,
            ..
        }] if correlation == &dispatch.task.handle().correlation
    ));
    assert!(session.task_generation_pins.is_empty());
}

#[test]
fn task_sequence_uses_last_available_identity_once() {
    let mut session = BundleSession::new(
        &bundle(10, ColorScheme::Light),
        BundleSessionOptions::default(),
    )
    .unwrap();
    session.next_task_sequence = u64::MAX - 1;
    let task = admitted_test_task("task.last");

    let dispatch =
        session.dispatch_requested_tasks(RuntimeClockStep::from_millis(1, 16).unwrap(), vec![task]);
    assert_eq!(dispatch.len(), 1);
    assert_eq!(dispatch[0].sequence, TaskSequence(u64::MAX - 1));
    assert_eq!(session.next_task_sequence, u64::MAX);
}

fn bundle(red: u8, scheme: ColorScheme) -> ArcweftBundle {
    bundle_with_source_note(red, scheme, "")
}

fn bundle_with_source_note(red: u8, scheme: ColorScheme, note: &str) -> ArcweftBundle {
    let mut builder = RuntimePlanBuilder::new();
    let flow = FlowRuntimeId::from_checked_declaration_digest([0x51; 32], "flow.main").unwrap();
    builder
        .admit_type_batch(
            [arcweft_core::plan::RuntimePlanTypeSeed::new(
                arcweft_core::pattern::RuntimeCheckedType::String.semantic_identity_digest(),
                arcweft_core::plan::RuntimePlanTypeProjection::String,
            )],
            [],
        )
        .expect("Flow result type admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [61; 32],
                ),
                None,
                Box::new([]),
                arcweft_core::pattern::RuntimeCheckedType::String.semantic_identity_digest(),
                arcweft_core::plan::RuntimeEffectSet::empty(),
            ),
            arcweft_core::plan::RuntimeExecutableBodySeed {
                effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                ops: (vec![RuntimeFlowOpSeed::Return("done".to_owned())]).into_boxed_slice(),
            },
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
            locale: arcweft_manifest_model::ProjectLocaleSpec::default(),
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
fn retained_runtime_image_keeps_its_original_artifact_identity() {
    let first = bundle(10, ColorScheme::Light);
    let second = bundle(20, ColorScheme::Dark);
    let mut session = BundleSession::new(&first, BundleSessionOptions::default()).unwrap();
    let first_identity = session.active_generation().artifact_identity;
    let second_identity = BundleSessionArtifactIdentity::LogicalBundle {
        identity: second.logical_identity().unwrap(),
    };

    commit_generational(&mut session, &second);

    assert_eq!(
        session.active_generation().artifact_identity,
        second_identity
    );
    assert_eq!(
        session.artifact_identity_for_generation(GenerationId::new(0)),
        Some(first_identity)
    );
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
    let candidate = ProgramGeneration::from_bundle(
        GenerationId::new(2),
        BundleSessionArtifactIdentity::LogicalBundle {
            identity: third.logical_identity().unwrap(),
        },
        &third,
    )
    .unwrap();
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

fn admitted_test_task(label: &str) -> arcweft_core::task::TaskSubmission {
    use arcweft_core::{
        pattern::RuntimeCheckedType,
        task::{
            NeedProducerContractDigest, NeedProducerFamily, NeedProducerInstance,
            NeedProducerSiteDigest, NeedProducerSpec, RuntimeTypeSemanticDigest,
            TaskAdmissionJournal, TaskOutcomeContract, TaskPlanSemanticDigest,
        },
        value::{RuntimePayload, RuntimeValue},
    };
    let outcome = TaskOutcomeContract::new(RuntimeCheckedType::Unit);
    let producer = NeedProducerSpec::new(
        NeedProducerFamily::HostAdapterTask,
        NeedProducerContractDigest::from_bytes([1; 32]),
        TaskPlanSemanticDigest::from_bytes([2; 32]),
        NeedProducerSiteDigest::from_bytes([3; 32]),
        RuntimeTypeSemanticDigest::from_bytes(*outcome.payload_semantic_identity().as_bytes()),
        RuntimeValue::Tuple(vec![RuntimeValue::String(label.into())])
            .try_digest(1024)
            .unwrap(),
    );
    let spec = TaskSpec {
        generation: GenerationId::new(0),
        producer: NeedProducerInstance::try_from(&producer).unwrap(),
        class: TaskClass::Background,
        priority: TaskPriority(0),
        cancel_scope: CancelScopeId("test".into()),
        policy: TaskPolicy::AlwaysStart,
        outcome,
        request: HostTaskRequest::custom("test", "unit", [RuntimePayload::from(label)]),
        debug_label: label.into(),
    };
    let mut journal = TaskAdmissionJournal::default();
    let handle = journal.ensure_task(spec).unwrap();
    journal.submission(handle).unwrap()
}
