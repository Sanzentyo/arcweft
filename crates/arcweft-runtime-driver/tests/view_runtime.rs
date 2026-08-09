use arcweft_bundle::resource_codec::view::{
    DialogueTextProjection, ViewDefinitionRef, ViewElementKind, ViewExportedPart,
    ViewObserveClassification, ViewOwnedPartRef, ViewPartExportSourceRef, ViewProgramInstruction,
    ViewRuntimeGeometryOwner, ViewSecureRedactionMetadata, ViewSemanticTarget,
    ViewStyleApplicationTarget, ViewStylePatchId, ViewStyleSheetId, ViewTextSourceKind,
    ViewTextSourceRecord,
};
use arcweft_bundle::resource_codec::{
    SourceMapSection, SourceRangeRef, ValidatedViewProduct, ViewCallArgumentBindingRef,
    ViewDefinitionResource, ViewDisplayFrameResource, ViewInstructionSpan,
    ViewLocalizedTextResource, ViewParameterResource, ViewProductValidationLimits,
    ViewProgramResource, ViewRichTextDocumentResource, ViewStyleResource, ViewTextBlockBounds,
    ViewTextBlockResource, ViewTextResource, ViewValueInputNamespace, ViewValueInputResource,
    ViewValueInputSource,
};
use arcweft_character::id::CharacterId;
use arcweft_core::value::{RuntimeBinding, RuntimeInt, RuntimeValue};
use arcweft_core::{entry::RuntimeValueDigest, plan::RuntimeLineId};
use arcweft_dialogue::InlineFailurePolicy;
use arcweft_id::TextKey;
use arcweft_presentation::fx::{
    FxContextSlot, FxRuntimeType, FxRuntimeValue, ValueInstruction, ValueProgramSchema,
};
use arcweft_render_text::{RuntimeLineContext, resolve_frame};
use arcweft_runtime_driver::dialogue::{
    DialoguePageIndex, DialoguePresentationOperation, DialoguePresentationStore,
    DialogueViewDefinition, DialogueViewInput, DialogueViewOccurrence, DialogueViewPrimaryAction,
    DialogueViewReveal, DialogueViewStage, DialogueViewState,
};
use arcweft_runtime_driver::presentation_handles::{
    PresentationHandleId, PresentationHandleKind, PresentationHandleRecord,
    PresentationResourceState,
};
use arcweft_runtime_driver::view_runtime::{
    BundleViewDiagnosticCode, BundleViewInstancePath, BundleViewInstancePathSegment,
    BundleViewMountOutput, BundleViewPaintItem, BundleViewRuntime as AcceptedBundleViewRuntime,
    BundleViewRuntimeError, BundleViewStyleNode, BundleViewStyleNodeId, BundleViewStyleNodeKind,
    BundleViewTextValue, SavedViewOwner, ViewOwnerEvidence, ViewProgramReplacementError,
    ViewProgramReplacementOutcome,
};
use arcweft_source::{ProductSourceRef, SourceDocument, SourceDocumentId, SourceName};
use arcweft_text_model::{
    CharacterDialoguePresentationConfig, DialogueContentSpec, DialoguePresentationCharacter,
    LineDisplayFrame, RichTextDocument, RichTextNode,
};
use arcweft_view::{
    AcceptedViewProgramRevision, DialogueEntryId, DialogueInstanceId, DialoguePresentationId,
    DialogueStageIndex, EventKind, RustViewId, ViewDescriptor, ViewId, ViewImplementation,
    ViewInstruction, ViewMountId, ViewPartLocalName, ViewPartName, ViewProgramId, ViewRegistry,
    ViewRegistryError, ViewSchemaId,
};
use arcweft_view::{ViewValueProgram, ViewValueProgramId};
use std::collections::{BTreeMap, BTreeSet};

struct BundleViewRuntime;

impl BundleViewRuntime {
    fn try_new(
        program: Option<ViewProgramResource>,
        text: Option<ViewTextResource>,
        style: Option<&ViewStyleResource>,
    ) -> Result<AcceptedBundleViewRuntime, BundleViewRuntimeError> {
        let source_map = if style.is_some() {
            let source = arcweft_bundle::standard_view::dialogue_style_source_document();
            Some(
                SourceMapSection::try_from_documents(&[&source])
                    .expect("standard dialogue Style source map"),
            )
        } else {
            program
                .as_ref()
                .is_some_and(|program| !program.source_refs.is_empty())
                .then(view_source_map)
        };
        let product = ValidatedViewProduct::try_new(
            source_map,
            program,
            style.cloned(),
            ViewProductValidationLimits::default(),
        )?;
        AcceptedBundleViewRuntime::try_new(product, text)
    }
}

fn program_id(value: &str) -> ViewProgramId {
    ViewProgramId::try_new(value).unwrap()
}

fn view_id(value: &str) -> ViewId {
    ViewId::parse_public(value).unwrap()
}

fn dialogue_source_ref() -> ProductSourceRef {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("runtime-driver-view-runtime-dialogue-test")
            .expect("document ID"),
        SourceName::Memory,
        "dialogue frame",
    )
    .expect("source document");
    ProductSourceRef::try_for_identity(source.identity()).expect("product source identity")
}

fn dialogue_frame(
    line: &str,
    view: &str,
    display_name: &str,
    nodes: Vec<RichTextNode>,
) -> LineDisplayFrame {
    let spec = DialogueContentSpec::new(
        RuntimeLineId::from_runtime_line_value(line).expect("runtime line identity"),
        TextKey::try_new(line.replacen("say.", "text.", 1)).expect("text key"),
        RichTextDocument::new(nodes),
        Vec::new(),
        dialogue_source_ref(),
    );
    resolve_frame(
        &spec,
        &RuntimeLineContext::new(
            Vec::new(),
            DialoguePresentationCharacter {
                id: CharacterId::try_new("character.test").expect("character identity"),
                display_name: display_name.to_owned(),
            },
            CharacterDialoguePresentationConfig {
                view: view_id(view),
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
            Vec::new(),
            Vec::new(),
        ),
    )
    .expect("final dialogue content resolves with explicit runtime context")
}

fn definition_ref(value: &str) -> ViewDefinitionRef {
    ViewDefinitionRef::new(view_id(value))
}

fn minimal_program(program: &str, view: &str, schema: u64) -> ViewProgramResource {
    ViewProgramResource {
        program_id: program_id(program),
        definitions: vec![ViewDefinitionResource {
            public_id: definition_ref(view),
            body: ViewInstructionSpan::new(0, 0),
            styles: Vec::new(),
            parameters: Vec::new(),
            state_schema_hash: schema,
        }],
        ..ViewProgramResource::default()
    }
}

fn handle(id: &str, view: &str) -> PresentationHandleRecord {
    PresentationHandleRecord::new(
        PresentationHandleId::try_new(id).unwrap(),
        PresentationHandleKind::View,
        view.to_owned(),
        None,
        PresentationResourceState::Mounted,
        None,
        0,
    )
}

#[test]
fn runtime_snapshot_requires_the_strict_axis_seed_registry_field() {
    assert_eq!(
        arcweft_runtime_driver::session_save::BUNDLE_SESSION_SAVE_SCHEMA_VERSION,
        2,
        "the final dialogue-content generation identity is a breaking unpublished save schema"
    );
    let runtime = BundleViewRuntime::try_new(None, None, None).unwrap();
    let snapshot = runtime.snapshot().unwrap();
    let mut missing = serde_json::to_value(&snapshot).unwrap();
    missing.as_object_mut().unwrap().remove("axis_seeds");
    assert!(
        serde_json::from_value::<arcweft_runtime_driver::view_runtime::BundleViewRuntimeSnapshot>(
            missing
        )
        .is_err()
    );

    let mut unknown = serde_json::to_value(snapshot).unwrap();
    unknown["axis_seeds"]["unknown"] = serde_json::json!(true);
    assert!(
        serde_json::from_value::<arcweft_runtime_driver::view_runtime::BundleViewRuntimeSnapshot>(
            unknown
        )
        .is_err()
    );
}

#[test]
fn view_identity_catalog_preserves_host_views_and_registers_arcweft_definitions() {
    let host = ViewId::try_new("view.host.public").unwrap();
    let authored = ViewId::try_new("view.Authored").unwrap();
    let mut registry = ViewRegistry::default();
    let anonymous_slot = registry
        .register(ViewDescriptor::anonymous_rust(
            ViewSchemaId(6),
            RustViewId(2),
        ))
        .unwrap();
    let host_slot = registry
        .register(ViewDescriptor::public_rust(
            host.clone(),
            ViewSchemaId(7),
            RustViewId(3),
        ))
        .unwrap();
    let product = ValidatedViewProduct::try_new(
        None,
        Some(minimal_program("view.program.catalog", "view.Authored", 11)),
        None,
        ViewProductValidationLimits::default(),
    )
    .unwrap();

    let runtime =
        AcceptedBundleViewRuntime::try_new_with_registry(product, None, registry).unwrap();
    assert_eq!(
        runtime.registry_owner_evidence(anonymous_slot),
        Some(ViewOwnerEvidence::AnonymousHost)
    );
    assert_eq!(
        runtime.registry_owner_evidence(host_slot),
        Some(ViewOwnerEvidence::Public { view: host.clone() })
    );
    assert_eq!(
        serde_json::to_string(&runtime.registry_owner_evidence(host_slot).unwrap()).unwrap(),
        r#"{"kind":"public","view":"view.host.public"}"#
    );
    let host_descriptor = runtime
        .registry()
        .get(runtime.registry().resolve(&host).unwrap())
        .unwrap();
    assert_eq!(
        host_descriptor.implementation(),
        &ViewImplementation::Rust(RustViewId(3))
    );
    let authored_descriptor = runtime
        .registry()
        .get(runtime.registry().resolve(&authored).unwrap())
        .unwrap();
    assert_eq!(authored_descriptor.schema(), ViewSchemaId(11));
    assert!(matches!(
        authored_descriptor.implementation(),
        ViewImplementation::Arcweft { program }
            if program == &program_id("view.program.catalog")
    ));
}

#[test]
fn accepted_catalog_rejects_host_owner_collision_before_publication() {
    let collision = ViewId::try_new("view.Collision").unwrap();
    let mut registry = ViewRegistry::default();
    registry
        .register(ViewDescriptor::public_rust(
            collision.clone(),
            ViewSchemaId(1),
            RustViewId(4),
        ))
        .unwrap();
    let product = ValidatedViewProduct::try_new(
        None,
        Some(minimal_program(
            "view.program.collision",
            "view.Collision",
            2,
        )),
        None,
        ViewProductValidationLimits::default(),
    )
    .unwrap();

    assert!(matches!(
        AcceptedBundleViewRuntime::try_new_with_registry(product, None, registry),
        Err(BundleViewRuntimeError::Registry(
            ViewRegistryError::DuplicateViewId(id)
        )) if id == collision
    ));
}

#[test]
fn authored_click_handler_enters_the_catalog_as_control_activation() {
    let mut program = minimal_program("view.program.click", "view.Clickable", 1);
    program.definitions[0].body = ViewInstructionSpan::new(0, 1);
    program.instructions = vec![ViewProgramInstruction::BindHandler {
        event: "click".to_owned(),
        handler: "handler.click".to_owned(),
        source: None,
    }];
    program.handlers = vec![arcweft_bundle::resource_codec::view::ViewHandlerRef {
        handler_id: "handler.click".to_owned(),
        event: "click".to_owned(),
        awbc_function_index: 0,
        handler_abi: arcweft_bundle::container::BundleDigest::of(b"handler.click"),
        function_binding: None,
    }];

    let product = validated_product(program);
    let runtime = AcceptedBundleViewRuntime::try_new(product, None).unwrap();
    let definition = runtime
        .catalog()
        .unwrap()
        .definition(&ViewId::try_new("view.Clickable").unwrap())
        .unwrap();
    assert!(matches!(
        definition.instructions(),
        [ViewInstruction::BindEvent(binding)] if binding.event == EventKind::Activate
    ));
}

#[test]
fn hot_reload_unchanged_and_source_only_candidates_preserve_runtime_generation() {
    let first = sourced_product("first.arcw", "first source text");
    let second = sourced_product("renamed.arcw", "different source text and length");
    assert_eq!(
        first.program().unwrap().accepted_revision(),
        second.program().unwrap().accepted_revision(),
    );
    assert_ne!(
        first.program().unwrap().source_set_revision(),
        second.program().unwrap().source_set_revision(),
    );
    let mut runtime = AcceptedBundleViewRuntime::try_new(first.clone(), None).unwrap();
    let initial_generation = runtime.accepted_generation();
    let initial_frame = runtime.frame_revision();

    let unchanged = runtime
        .prepare_view_program_replacement(first)
        .expect("equal candidate prepares");
    assert_eq!(
        runtime.commit_view_program_replacement(unchanged),
        Ok(ViewProgramReplacementOutcome::Unchanged),
    );
    assert_eq!(runtime.accepted_generation(), initial_generation);
    assert_eq!(runtime.frame_revision(), initial_frame);

    let source_only = runtime
        .prepare_view_program_replacement(second)
        .expect("source-only candidate prepares");
    assert_eq!(
        runtime.commit_view_program_replacement(source_only),
        Ok(ViewProgramReplacementOutcome::SourceOnly),
    );
    assert_eq!(runtime.accepted_generation(), initial_generation);
    assert_eq!(runtime.frame_revision(), initial_frame);
    assert!(runtime.last_invalidation().is_none());
}

#[test]
fn hot_reload_treats_style_provenance_as_source_only_and_rejects_style_semantic_changes() {
    let program = minimal_program("view.program.style-reload", "view.Styled", 1);
    let initial_source = arcweft_bundle::standard_view::dialogue_style_source_document();
    let initial = styled_product(
        program.clone(),
        &initial_source,
        arcweft_bundle::standard_view::dialogue_style(),
    );
    let mut runtime = AcceptedBundleViewRuntime::try_new(initial, None).unwrap();
    let initial_generation = runtime.accepted_generation();
    let initial_frame = runtime.frame_revision();

    let changed_text = format!(
        "{}\n// source-only revision\n",
        arcweft_bundle::standard_view::dialogue_style_source_document().text()
    );
    let changed_source = SourceDocument::try_new(
        SourceDocumentId::try_new(arcweft_bundle::standard_view::DIALOGUE_STYLE_SOURCE_ID).unwrap(),
        SourceName::Generated,
        changed_text,
    )
    .unwrap();
    let provenance_only = styled_product(
        program.clone(),
        &changed_source,
        arcweft_bundle::standard_view::dialogue_style(),
    );
    let prepared = runtime
        .prepare_view_program_replacement(provenance_only)
        .expect("Style provenance-only candidate prepares");
    assert_eq!(
        runtime.commit_view_program_replacement(prepared),
        Ok(ViewProgramReplacementOutcome::SourceOnly),
    );
    assert_eq!(runtime.accepted_generation(), initial_generation);
    assert_eq!(runtime.frame_revision(), initial_frame);

    let before = runtime.snapshot().unwrap();
    let mut changed_style = arcweft_bundle::standard_view::dialogue_style();
    changed_style.style_program_id = "std.view.style.program.changed".to_owned();
    let changed = styled_product(program, &changed_source, changed_style);
    assert!(matches!(
        runtime.prepare_view_program_replacement(changed),
        Err(ViewProgramReplacementError::StyleProgramChanged)
    ));
    assert_eq!(runtime.snapshot().unwrap(), before);
    assert_eq!(runtime.accepted_generation(), initial_generation);
    assert_eq!(runtime.frame_revision(), initial_frame);
}

#[test]
fn hot_reload_semantic_replacement_reconciles_multiple_mounts_and_reintroduction_is_fresh() {
    let initial = validated_product(minimal_program("view.program.hot-reload", "view.Hot", 1));
    let mut runtime = AcceptedBundleViewRuntime::try_new(initial, None).unwrap();
    let handles = [
        handle("handle.hot.first", "view.Hot"),
        handle("handle.hot.second", "view.Hot"),
    ];
    let initial_frame = runtime.evaluate(&handles, &[], false);
    let initial_mounts = initial_frame
        .mounts
        .iter()
        .map(|mount| mount.mount)
        .collect::<BTreeSet<_>>();
    assert_eq!(initial_mounts.len(), 2);

    let schema_change =
        validated_product(minimal_program("view.program.hot-reload", "view.Hot", 2));
    let prepared = runtime
        .prepare_view_program_replacement(schema_change)
        .expect("semantic candidate prepares");
    assert!(matches!(
        runtime.commit_view_program_replacement(prepared),
        Ok(ViewProgramReplacementOutcome::Semantic { generation, .. })
            if generation.get() == 2
    ));
    assert_eq!(runtime.frame_revision(), 1);
    let after_schema = runtime.evaluate(&handles, &[], false);
    assert_eq!(
        after_schema
            .mounts
            .iter()
            .map(|mount| mount.mount)
            .collect::<BTreeSet<_>>(),
        initial_mounts,
    );

    let removed = validated_product(ViewProgramResource {
        program_id: program_id("view.program.hot-reload"),
        ..ViewProgramResource::default()
    });
    let prepared = runtime
        .prepare_view_program_replacement(removed)
        .expect("definition removal prepares");
    runtime
        .commit_view_program_replacement(prepared)
        .expect("definition removal commits");
    assert_eq!(
        runtime.last_invalidation().unwrap().retired_mounts(),
        &initial_mounts,
    );
    assert!(runtime.evaluate(&handles, &[], false).mounts.is_empty());

    let reintroduced = validated_product(minimal_program("view.program.hot-reload", "view.Hot", 2));
    let prepared = runtime
        .prepare_view_program_replacement(reintroduced)
        .expect("reintroduction prepares");
    runtime
        .commit_view_program_replacement(prepared)
        .expect("reintroduction commits");
    let reintroduced_frame = runtime.evaluate(&handles[..1], &[], false);
    assert_eq!(reintroduced_frame.mounts.len(), 1);
    assert!(!initial_mounts.contains(&reintroduced_frame.mounts[0].mount));
}

#[test]
fn hot_reload_prepared_candidate_rejects_stale_runtime_without_mutation() {
    let initial = validated_product(minimal_program("view.program.stale", "view.Stale", 1));
    let mut runtime = AcceptedBundleViewRuntime::try_new(initial, None).unwrap();
    let candidate = validated_product(minimal_program("view.program.stale", "view.Stale", 2));
    let prepared = runtime
        .prepare_view_program_replacement(candidate)
        .expect("candidate prepares");
    runtime.evaluate(&[handle("handle.stale", "view.Stale")], &[], false);
    let before = runtime.snapshot().unwrap();

    assert_eq!(
        runtime.commit_view_program_replacement(prepared),
        Err(ViewProgramReplacementError::StalePreparedState),
    );
    assert_eq!(runtime.snapshot().unwrap(), before);
    assert_eq!(runtime.accepted_generation().get(), 1);
    assert_eq!(runtime.frame_revision(), 0);
}

#[test]
fn hot_reload_invalid_catalog_and_program_identity_leave_runtime_unchanged() {
    let initial = validated_product(minimal_program("view.program.atomic", "view.Atomic", 1));
    let mut runtime = AcceptedBundleViewRuntime::try_new(initial, None).unwrap();
    runtime.evaluate(&[handle("handle.atomic", "view.Atomic")], &[], false);
    let before = runtime.snapshot().unwrap();
    let mut invalid = minimal_program("view.program.atomic", "view.Atomic", 1);
    invalid.definitions[0].body = ViewInstructionSpan::new(0, 1);
    invalid.instructions = vec![ViewProgramInstruction::BindHandler {
        event: "unsupported_event".to_owned(),
        handler: "handler.invalid".to_owned(),
        source: None,
    }];
    invalid.handlers = vec![arcweft_bundle::resource_codec::view::ViewHandlerRef {
        handler_id: "handler.invalid".to_owned(),
        event: "unsupported_event".to_owned(),
        awbc_function_index: 0,
        handler_abi: arcweft_bundle::container::BundleDigest::of(b"handler.invalid"),
        function_binding: None,
    }];

    assert!(matches!(
        runtime.prepare_view_program_replacement(validated_product(invalid)),
        Err(ViewProgramReplacementError::Catalog(_))
    ));
    assert_eq!(runtime.snapshot().unwrap(), before);
    assert_eq!(runtime.accepted_generation().get(), 1);

    let other = validated_product(minimal_program("view.program.other", "view.Atomic", 2));
    assert!(matches!(
        runtime.prepare_view_program_replacement(other),
        Err(ViewProgramReplacementError::ProgramIdentityMismatch)
    ));
    assert_eq!(runtime.snapshot().unwrap(), before);
}

#[test]
fn hot_reload_exported_part_change_invalidates_owner_and_direct_caller_only() {
    let initial = validated_product(replacement_graph_program(
        "ChildElement",
        "part.public",
        true,
    ));
    let mut runtime = AcceptedBundleViewRuntime::try_new(initial, None).unwrap();
    let candidate = validated_product(replacement_graph_program(
        "ChildElement",
        "part.renamed",
        true,
    ));
    let prepared = runtime
        .prepare_view_program_replacement(candidate)
        .expect("export change prepares");
    runtime
        .commit_view_program_replacement(prepared)
        .expect("export change commits");
    let invalidation = runtime.last_invalidation().expect("semantic invalidation");

    assert_eq!(
        invalidation.export_owners(),
        &BTreeSet::from([ViewId::try_new("view.Child").unwrap()]),
    );
    assert_eq!(
        invalidation.direct_callers(),
        &BTreeSet::from([ViewId::try_new("view.Parent").unwrap()]),
    );
    assert!(
        !invalidation
            .owners()
            .contains(&ViewId::try_new("view.Unrelated").unwrap())
    );
}

#[test]
fn hot_reload_unexported_local_edit_does_not_invalidate_direct_caller() {
    let initial = validated_product(replacement_graph_program(
        "ChildElement",
        "part.public",
        true,
    ));
    let mut runtime = AcceptedBundleViewRuntime::try_new(initial, None).unwrap();
    let candidate = validated_product(replacement_graph_program(
        "ChangedChildElement",
        "part.public",
        true,
    ));
    let prepared = runtime
        .prepare_view_program_replacement(candidate)
        .expect("local edit prepares");
    runtime
        .commit_view_program_replacement(prepared)
        .expect("local edit commits");
    let invalidation = runtime.last_invalidation().expect("semantic invalidation");

    assert_eq!(
        invalidation.owners(),
        &BTreeSet::from([ViewId::try_new("view.Child").unwrap()]),
    );
    assert!(invalidation.export_owners().is_empty());
    assert!(invalidation.direct_callers().is_empty());
}

#[test]
fn hot_reload_removed_nested_call_retires_only_the_child_mount() {
    let initial = validated_product(replacement_graph_program(
        "ChildElement",
        "part.public",
        true,
    ));
    let mut runtime = AcceptedBundleViewRuntime::try_new(initial, None).unwrap();
    let mounted = handle("handle.nested-reload", "view.Parent");
    let initial_frame = runtime.evaluate(std::slice::from_ref(&mounted), &[], false);
    assert_eq!(initial_frame.mounts.len(), 2);
    let root_mount = initial_frame
        .mounts
        .iter()
        .find(|mount| mount.path.segments().is_empty())
        .unwrap()
        .mount;
    let child_mount = initial_frame
        .mounts
        .iter()
        .find(|mount| !mount.path.segments().is_empty())
        .unwrap()
        .mount;
    let candidate = validated_product(replacement_graph_program(
        "ChildElement",
        "part.public",
        false,
    ));
    let prepared = runtime
        .prepare_view_program_replacement(candidate)
        .expect("call removal prepares");
    runtime
        .commit_view_program_replacement(prepared)
        .expect("call removal commits");

    assert_eq!(
        runtime.last_invalidation().unwrap().retired_mounts(),
        &BTreeSet::from([child_mount]),
    );
    let frame = runtime.evaluate(std::slice::from_ref(&mounted), &[], false);
    assert_eq!(frame.mounts.len(), 1);
    assert_eq!(frame.mounts[0].mount, root_mount);
}

#[test]
fn hot_reload_definition_removal_retires_repeat_nested_mounts_atomically() {
    let initial = validated_product(replacement_repeat_graph_program());
    let mut runtime = AcceptedBundleViewRuntime::try_new(initial, None).unwrap();
    let mounted = handle("handle.repeat-reload", "view.RepeatRoot");
    let initial_frame = runtime.evaluate(std::slice::from_ref(&mounted), &[], false);
    assert!(initial_frame.diagnostics.is_empty(), "{initial_frame:#?}");
    assert_eq!(initial_frame.mounts.len(), 3);
    let root_mount = initial_frame
        .mounts
        .iter()
        .find(|mount| mount.path.segments().is_empty())
        .unwrap()
        .mount;
    let repeated_children = initial_frame
        .mounts
        .iter()
        .filter(|mount| !mount.path.segments().is_empty())
        .map(|mount| mount.mount)
        .collect::<BTreeSet<_>>();
    assert_eq!(repeated_children.len(), 2);
    assert!(
        initial_frame
            .mounts
            .iter()
            .filter(|mount| {
                matches!(
                    mount.path.segments(),
                    [
                        BundleViewInstancePathSegment::Repeat { instruction: 0, .. },
                        BundleViewInstancePathSegment::Call { instruction: 1, .. }
                    ]
                )
            })
            .count()
            == 2
    );

    let candidate = validated_product(minimal_program(
        "view.program.repeat-replacement",
        "view.RepeatRoot",
        1,
    ));
    let prepared = runtime
        .prepare_view_program_replacement(candidate)
        .expect("repeat child removal prepares");
    runtime
        .commit_view_program_replacement(prepared)
        .expect("repeat child removal commits");

    assert_eq!(
        runtime.last_invalidation().unwrap().retired_mounts(),
        &repeated_children,
    );
    let frame = runtime.evaluate(std::slice::from_ref(&mounted), &[], false);
    assert_eq!(frame.mounts.len(), 1);
    assert_eq!(frame.mounts[0].mount, root_mount);
}

fn local_part(value: &str) -> ViewPartLocalName {
    ViewPartLocalName::try_new(value).expect("valid local part identity")
}

fn public_part(value: &str) -> ViewPartName {
    ViewPartName::try_new(value).expect("valid public part identity")
}

fn exported_part(
    owner: &str,
    local: &str,
    public: &str,
    source_refs: &[ProductSourceRef],
) -> ViewExportedPart {
    let source = &source_refs[0];
    ViewExportedPart {
        target: ViewOwnedPartRef::new(definition_ref(owner), local_part(local)),
        public_name: public_part(public),
        source: ViewPartExportSourceRef {
            declaration: source_range(source_refs, source, 0, 32),
            local_name: source_range(source_refs, source, 12, 20),
            public_name: source_range(source_refs, source, 24, 31),
        },
    }
}

fn view_source_refs() -> Vec<ProductSourceRef> {
    view_source_map()
        .documents()
        .map(arcweft_bundle::resource_codec::SourceMapDocument::product_source_ref)
        .collect()
}

fn view_source_map() -> SourceMapSection {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("view-runtime.arcw").expect("source ID"),
        SourceName::path("view-runtime.arcw"),
        "x".repeat(64),
    )
    .expect("source document");
    SourceMapSection::try_from_documents(&[&document]).expect("source map")
}

fn validated_product(program: ViewProgramResource) -> ValidatedViewProduct {
    let source_map = (!program.source_refs.is_empty()).then(view_source_map);
    ValidatedViewProduct::try_new(
        source_map,
        Some(program),
        None,
        ViewProductValidationLimits::default(),
    )
    .expect("test View product validates")
}

fn styled_product(
    program: ViewProgramResource,
    source: &SourceDocument,
    mut style: ViewStyleResource,
) -> ValidatedViewProduct {
    let source_map = SourceMapSection::try_from_documents(&[source]).unwrap();
    style.source_refs = vec![
        source_map
            .documents()
            .next()
            .expect("Style source map is non-empty")
            .product_source_ref(),
    ];
    ValidatedViewProduct::try_new(
        Some(source_map),
        Some(program),
        Some(style),
        ViewProductValidationLimits::default(),
    )
    .expect("styled product validates")
}

fn sourced_product(label: &str, text: &str) -> ValidatedViewProduct {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new(label).expect("source ID"),
        SourceName::path(label),
        text,
    )
    .expect("source document");
    let source_map = SourceMapSection::try_from_documents(&[&document]).expect("source map");
    let source_refs = source_map
        .documents()
        .map(arcweft_bundle::resource_codec::SourceMapDocument::product_source_ref)
        .collect::<Vec<_>>();
    let source = source_refs[0].clone();
    let mut program = minimal_program("view.program.source-only", "view.SourceOnly", 1);
    program.source_refs.clone_from(&source_refs);
    program.semantic_targets = vec![ViewSemanticTarget {
        public_id: "target.source-only".to_owned(),
        target: "target.source-only".to_owned(),
        view: Some("view.SourceOnly".to_owned()),
        label_text_source: None,
        source: Some(source_range(
            &source_refs,
            &source,
            0,
            u32::try_from(text.len()).expect("source length"),
        )),
    }];
    ValidatedViewProduct::try_new(
        Some(source_map),
        Some(program),
        None,
        ViewProductValidationLimits::default(),
    )
    .expect("sourced product validates")
}

fn replacement_graph_program(
    child_element: &str,
    exported_name: &str,
    parent_calls_child: bool,
) -> ViewProgramResource {
    let source_refs = view_source_refs();
    let parent = if parent_calls_child {
        ViewProgramInstruction::CallView {
            view: definition_ref("view.Child"),
            arguments: Vec::new(),
            styles: Vec::new(),
            part: None,
            key: Some(7),
            source: None,
        }
    } else {
        ViewProgramInstruction::EmitCustom {
            element: "ParentWithoutCall".to_owned(),
            styles: Vec::new(),
            part: None,
            source: None,
        }
    };
    ViewProgramResource {
        program_id: program_id("view.program.replacement-graph"),
        source_refs: source_refs.clone(),
        definitions: vec![
            ViewDefinitionResource {
                public_id: definition_ref("view.Parent"),
                body: ViewInstructionSpan::new(0, 1),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 1,
            },
            ViewDefinitionResource {
                public_id: definition_ref("view.Child"),
                body: ViewInstructionSpan::new(1, 2),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 2,
            },
            ViewDefinitionResource {
                public_id: definition_ref("view.Unrelated"),
                body: ViewInstructionSpan::new(2, 3),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 3,
            },
        ],
        instructions: vec![
            parent,
            ViewProgramInstruction::EmitCustom {
                element: child_element.to_owned(),
                styles: Vec::new(),
                part: Some(local_part("part.local")),
                source: None,
            },
            ViewProgramInstruction::EmitCustom {
                element: "UnrelatedElement".to_owned(),
                styles: Vec::new(),
                part: None,
                source: None,
            },
        ],
        exported_parts: vec![exported_part(
            "view.Child",
            "part.local",
            exported_name,
            &source_refs,
        )],
        ..ViewProgramResource::default()
    }
}

fn replacement_repeat_graph_program() -> ViewProgramResource {
    let state_types = vec![FxRuntimeType::I32];
    ViewProgramResource {
        program_id: program_id("view.program.repeat-replacement"),
        definitions: vec![
            ViewDefinitionResource {
                public_id: definition_ref("view.RepeatRoot"),
                body: ViewInstructionSpan::new(0, 2),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 1,
            },
            ViewDefinitionResource {
                public_id: definition_ref("view.RepeatChild"),
                body: ViewInstructionSpan::new(2, 3),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 2,
            },
        ],
        value_programs: vec![
            value_program(
                0,
                Vec::new(),
                state_types.clone(),
                FxRuntimeType::I32,
                vec![
                    ValueInstruction::Constant {
                        value: FxRuntimeValue::I32(2),
                    },
                    ValueInstruction::Return,
                ],
            ),
            value_program(
                1,
                Vec::new(),
                state_types,
                FxRuntimeType::I32,
                vec![
                    ValueInstruction::LoadState {
                        slot: 0,
                        ty: FxRuntimeType::I32,
                    },
                    ValueInstruction::Return,
                ],
            ),
        ],
        value_inputs: vec![ViewValueInputResource {
            namespace: ViewValueInputNamespace::State,
            slot: 0,
            value_type: FxRuntimeType::I32,
            source: ViewValueInputSource::RepeatOrdinal {
                view: "view.RepeatRoot".to_owned(),
                binding: "item".to_owned(),
            },
        }],
        instructions: vec![
            ViewProgramInstruction::RepeatKeyed {
                source_program: ViewValueProgramId(0),
                key_program: ViewValueProgramId(1),
                body_span: 1,
                source: None,
            },
            ViewProgramInstruction::CallView {
                view: definition_ref("view.RepeatChild"),
                arguments: Vec::new(),
                styles: Vec::new(),
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::EmitCustom {
                element: "RepeatChild".to_owned(),
                styles: Vec::new(),
                part: None,
                source: None,
            },
        ],
        ..ViewProgramResource::default()
    }
}

fn source_range(
    source_refs: &[ProductSourceRef],
    source: &ProductSourceRef,
    start_byte: u32,
    end_byte: u32,
) -> SourceRangeRef {
    SourceRangeRef::try_for_source(source_refs, source, start_byte, end_byte).expect("source range")
}

fn value_program(
    id: u32,
    parameter_types: Vec<FxRuntimeType>,
    state_types: Vec<FxRuntimeType>,
    return_type: FxRuntimeType,
    instructions: Vec<ValueInstruction>,
) -> ViewValueProgram {
    ViewValueProgram::validate(
        ViewValueProgramId(id),
        ValueProgramSchema::new(parameter_types, state_types, return_type),
        instructions,
    )
    .unwrap()
}

fn text_resource(
    records: impl IntoIterator<Item = (&'static str, ViewTextSourceKind)>,
) -> ViewTextResource {
    ViewTextResource {
        sources: records
            .into_iter()
            .map(|(public_id, kind)| ViewTextSourceRecord {
                public_id: public_id.to_owned(),
                kind,
                source: None,
            })
            .collect(),
        ..ViewTextResource::default()
    }
}

fn plain_text(frame: &arcweft_runtime_driver::view_runtime::BundleViewFrame) -> &str {
    let BundleViewTextValue::Plain { value } = &frame.mounts[0].text[0].value else {
        panic!("expected plain text")
    };
    value
}

#[test]
fn style_scope_follows_subtrees_without_leaking_to_siblings() {
    let root_sheet = ViewStyleSheetId::try_new("style.root").unwrap();
    let first_sheet = ViewStyleSheetId::try_new("style.first").unwrap();
    let second_sheet = ViewStyleSheetId::try_new("style.second").unwrap();
    let inline_patch = ViewStylePatchId::new(7);
    let program = ViewProgramResource {
        program_id: program_id("view.program.style-subtrees"),
        definitions: vec![ViewDefinitionResource {
            public_id: definition_ref("view.Root"),
            body: ViewInstructionSpan::new(0, 7),
            styles: vec![ViewStyleApplicationTarget::named(root_sheet.clone())],
            parameters: Vec::new(),
            state_schema_hash: 1,
        }],
        instructions: vec![
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Panel,
                target: None,
                styles: vec![ViewStyleApplicationTarget::named(first_sheet.clone())],
                part: Some(local_part("part.first-root")),
                key: None,
                source: None,
            },
            ViewProgramInstruction::EmitCustom {
                element: "FirstChild".to_owned(),
                styles: Vec::new(),
                part: Some(local_part("part.first-child")),
                source: None,
            },
            ViewProgramInstruction::CloseElement,
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Panel,
                target: None,
                styles: vec![
                    ViewStyleApplicationTarget::named(second_sheet.clone()),
                    ViewStyleApplicationTarget::inline(inline_patch),
                ],
                part: Some(local_part("part.second-root")),
                key: None,
                source: None,
            },
            ViewProgramInstruction::EmitCustom {
                element: "SecondChild".to_owned(),
                styles: Vec::new(),
                part: Some(local_part("part.second-child")),
                source: None,
            },
            ViewProgramInstruction::CloseElement,
            ViewProgramInstruction::EmitCustom {
                element: "UnaffectedSibling".to_owned(),
                styles: Vec::new(),
                part: Some(local_part("part.sibling")),
                source: None,
            },
        ],
        ..ViewProgramResource::default()
    };
    let mut runtime = BundleViewRuntime::try_new(Some(program), None, None).unwrap();
    let frame = runtime.evaluate(&[handle("handle.styles", "view.Root")], &[], false);
    assert!(frame.diagnostics.is_empty());

    let nodes = &frame.mounts[0].style_nodes;
    assert_eq!(
        nodes
            .iter()
            .map(|node| node.instruction)
            .collect::<Vec<_>>(),
        [0, 1, 3, 4, 6]
    );
    assert!(nodes.iter().all(|node| matches!(
        node.applications[0].target(),
        ViewStyleApplicationTarget::Named { sheet } if sheet == &root_sheet
    )));
    assert!(matches!(
        nodes[0].applications[1].target(),
        ViewStyleApplicationTarget::Named { sheet } if sheet == &first_sheet
    ));
    assert_eq!(
        nodes[0].applications[1].scope(),
        nodes[1].applications[1].scope()
    );
    assert!(matches!(
        nodes[2].applications[1].target(),
        ViewStyleApplicationTarget::Named { sheet } if sheet == &second_sheet
    ));
    assert!(matches!(
        nodes[2].applications[2].target(),
        ViewStyleApplicationTarget::Inline { patch } if *patch == inline_patch
    ));
    assert_eq!(
        nodes[2].applications[1].scope(),
        nodes[3].applications[1].scope()
    );
    assert_eq!(nodes[0].applications[0].application_order(), 0);
    assert_eq!(nodes[0].applications[1].application_order(), 1);
    assert_eq!(nodes[2].applications[1].application_order(), 2);
    assert_eq!(nodes[2].applications[2].application_order(), 3);
    assert_eq!(nodes[3].applications.len(), 2);
    assert_eq!(nodes[4].applications.len(), 1);
}

fn call_boundary_style_program(
    external_sheet: &ViewStyleSheetId,
    child_sheet: &ViewStyleSheetId,
    inline_patch: ViewStylePatchId,
) -> ViewProgramResource {
    let source_refs = view_source_refs();
    ViewProgramResource {
        program_id: program_id("view.program.style-call-boundary"),
        source_refs: source_refs.clone(),
        definitions: vec![
            ViewDefinitionResource {
                public_id: definition_ref("view.Parent"),
                body: ViewInstructionSpan::new(0, 2),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 1,
            },
            ViewDefinitionResource {
                public_id: definition_ref("view.Child"),
                body: ViewInstructionSpan::new(2, 6),
                styles: vec![ViewStyleApplicationTarget::named(child_sheet.clone())],
                parameters: Vec::new(),
                state_schema_hash: 2,
            },
        ],
        instructions: vec![
            ViewProgramInstruction::CallView {
                view: definition_ref("view.Child"),
                arguments: Vec::new(),
                styles: vec![
                    ViewStyleApplicationTarget::named(external_sheet.clone()),
                    ViewStyleApplicationTarget::inline(inline_patch),
                ],
                part: Some(local_part("part.call")),
                key: None,
                source: None,
            },
            ViewProgramInstruction::EmitCustom {
                element: "ParentSibling".to_owned(),
                styles: Vec::new(),
                part: Some(local_part("part.parent-sibling")),
                source: None,
            },
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Panel,
                target: None,
                styles: Vec::new(),
                part: Some(local_part("part.child-root")),
                key: None,
                source: None,
            },
            ViewProgramInstruction::EmitCustom {
                element: "PrivateChild".to_owned(),
                styles: Vec::new(),
                part: Some(local_part("part.child-private")),
                source: None,
            },
            ViewProgramInstruction::EmitCustom {
                element: "ExportedChild".to_owned(),
                styles: Vec::new(),
                part: Some(local_part("part.child-exported")),
                source: None,
            },
            ViewProgramInstruction::CloseElement,
        ],
        exported_parts: vec![exported_part(
            "view.Child",
            "part.child-exported",
            "part.public-child",
            &source_refs,
        )],
        ..ViewProgramResource::default()
    }
}

fn call_boundary_parent_node(parent: &BundleViewMountOutput) -> &BundleViewStyleNode {
    assert_eq!(parent.style_nodes.len(), 2);
    let call = parent
        .style_nodes
        .iter()
        .find(|node| {
            node.instruction == 0 && matches!(node.kind, BundleViewStyleNodeKind::CallView { .. })
        })
        .unwrap();
    let sibling = parent
        .style_nodes
        .iter()
        .find(|node| node.instruction == 1)
        .unwrap();
    assert!(matches!(
        &sibling.kind,
        BundleViewStyleNodeKind::Custom { element } if element == "ParentSibling"
    ));
    assert!(sibling.applications.is_empty());
    call
}

#[test]
fn style_scope_enters_call_view_before_recursion_and_protects_private_parts() {
    let external_sheet = ViewStyleSheetId::try_new("style.external").unwrap();
    let child_sheet = ViewStyleSheetId::try_new("style.child").unwrap();
    let inline_patch = ViewStylePatchId::new(11);
    let program = call_boundary_style_program(&external_sheet, &child_sheet, inline_patch);
    let mut runtime = BundleViewRuntime::try_new(Some(program), None, None).unwrap();
    let frame = runtime.evaluate(&[handle("handle.parent", "view.Parent")], &[], false);
    assert!(frame.diagnostics.is_empty());

    let parent = frame
        .mounts
        .iter()
        .find(|mount| mount.view.as_str() == "view.Parent")
        .unwrap();
    let call = call_boundary_parent_node(parent);
    assert_eq!(call.applications.len(), 2);
    assert!(matches!(
        call.applications[0].target(),
        ViewStyleApplicationTarget::Named { sheet } if sheet == &external_sheet
    ));
    assert!(matches!(
        call.applications[1].target(),
        ViewStyleApplicationTarget::Inline { patch } if *patch == inline_patch
    ));

    let child = frame
        .mounts
        .iter()
        .find(|mount| mount.view.as_str() == "view.Child")
        .unwrap();
    assert_eq!(
        child
            .style_nodes
            .iter()
            .map(|node| node.instruction)
            .collect::<Vec<_>>(),
        [2, 3, 4]
    );
    assert!(
        child
            .style_nodes
            .iter()
            .all(|node| node.applications.len() == 2)
    );
    assert!(child.style_nodes.iter().all(|node| matches!(
        node.applications[0].target(),
        ViewStyleApplicationTarget::Named { sheet } if sheet == &external_sheet
    )));
    assert!(child.style_nodes.iter().all(|node| matches!(
        node.applications[1].target(),
        ViewStyleApplicationTarget::Named { sheet } if sheet == &child_sheet
    )));
    assert!(
        child
            .style_nodes
            .iter()
            .all(|node| !node.applications[1].boundary().is_nested_view_boundary())
    );
    assert_eq!(
        child.style_nodes[0].applications[0].scope(),
        call.applications[0].scope()
    );

    let root_boundary = child.style_nodes[0].applications[0].boundary();
    assert!(root_boundary.is_nested_view_boundary());
    assert!(root_boundary.allows_inherited_root());
    assert!(!root_boundary.allows_selector_traversal());

    let private_boundary = child.style_nodes[1].applications[0].boundary();
    assert_eq!(child.style_nodes[1].exported_part, None);
    assert!(private_boundary.is_nested_view_boundary());
    assert!(!private_boundary.allows_inherited_root());
    assert!(!private_boundary.allows_selector_traversal());
    assert!(!private_boundary.matches_part(
        &public_part("part.public-child"),
        child.style_nodes[1].part.as_ref(),
        child.style_nodes[1].exported_part.as_ref(),
    ));

    let exported_boundary = child.style_nodes[2].applications[0].boundary();
    assert_eq!(
        child.style_nodes[2]
            .exported_part
            .as_ref()
            .map(|part| part.as_public_id().as_str()),
        Some("part.public-child")
    );
    assert!(exported_boundary.is_nested_view_boundary());
    assert!(exported_boundary.is_exported_part());
    assert!(!exported_boundary.allows_inherited_root());
    assert!(exported_boundary.allows_selector_traversal());
    assert!(exported_boundary.matches_part(
        &public_part("part.public-child"),
        child.style_nodes[2].part.as_ref(),
        child.style_nodes[2].exported_part.as_ref(),
    ));
    let evidence = child.exported_part_evidence().collect::<Vec<_>>();
    assert_eq!(evidence.len(), 1);
    let public_json = serde_json::to_string(&evidence[0]).unwrap();
    assert_eq!(
        public_json,
        r#"{"owner":{"kind":"public","view":"view.Child"},"part":"part.public-child"}"#
    );
    assert!(!public_json.contains("part.child-exported"));
    assert!(!public_json.contains("registry"));
    assert!(!public_json.contains("definition"));
}

#[test]
fn exported_part_access_does_not_cross_two_nested_view_boundaries() {
    let external_sheet = ViewStyleSheetId::try_new("style.external.owner").unwrap();
    let source_refs = view_source_refs();
    let program = ViewProgramResource {
        program_id: program_id("view.program.non-transitive-export"),
        source_refs: source_refs.clone(),
        definitions: vec![
            ViewDefinitionResource {
                public_id: definition_ref("view.A"),
                body: ViewInstructionSpan::new(0, 1),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 1,
            },
            ViewDefinitionResource {
                public_id: definition_ref("view.B"),
                body: ViewInstructionSpan::new(1, 2),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 2,
            },
            ViewDefinitionResource {
                public_id: definition_ref("view.C"),
                body: ViewInstructionSpan::new(2, 3),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 3,
            },
        ],
        instructions: vec![
            ViewProgramInstruction::CallView {
                view: definition_ref("view.B"),
                arguments: Vec::new(),
                styles: vec![ViewStyleApplicationTarget::named(external_sheet.clone())],
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::CallView {
                view: definition_ref("view.C"),
                arguments: Vec::new(),
                styles: Vec::new(),
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::EmitCustom {
                element: "DeepExport".to_owned(),
                styles: Vec::new(),
                part: Some(local_part("part.c.exported")),
                source: None,
            },
        ],
        exported_parts: vec![exported_part(
            "view.C",
            "part.c.exported",
            "part.public-c",
            &source_refs,
        )],
        ..ViewProgramResource::default()
    };
    let mut runtime = BundleViewRuntime::try_new(Some(program), None, None).unwrap();
    let frame = runtime.evaluate(&[handle("handle.a", "view.A")], &[], false);
    assert!(frame.diagnostics.is_empty());

    let deep = frame
        .mounts
        .iter()
        .find(|mount| mount.view.as_str() == "view.C")
        .and_then(|mount| mount.style_nodes.first())
        .expect("the deep exported node retains the ancestor application");
    let boundary = deep.applications[0].boundary();
    assert_eq!(
        deep.exported_part
            .as_ref()
            .map(|part| part.as_public_id().as_str()),
        Some("part.public-c")
    );
    assert!(matches!(
        deep.applications[0].target(),
        ViewStyleApplicationTarget::Named { sheet } if sheet == &external_sheet
    ));
    assert_eq!(boundary.crossed_view_boundaries(), 2);
    assert!(boundary.is_exported_part());
    assert!(!boundary.allows_selector_traversal());
    assert!(!boundary.matches_part(
        &public_part("part.public-c"),
        deep.part.as_ref(),
        deep.exported_part.as_ref(),
    ));
}

#[test]
fn style_scope_rejects_inline_patch_on_non_rendered_definition_root() {
    let program = ViewProgramResource {
        program_id: program_id("view.program.invalid-root-inline"),
        definitions: vec![ViewDefinitionResource {
            public_id: definition_ref("view.Root"),
            body: ViewInstructionSpan::new(0, 0),
            styles: vec![ViewStyleApplicationTarget::inline(ViewStylePatchId::new(
                17,
            ))],
            parameters: Vec::new(),
            state_schema_hash: 1,
        }],
        ..ViewProgramResource::default()
    };
    let mut runtime = BundleViewRuntime::try_new(Some(program), None, None).unwrap();
    let frame = runtime.evaluate(&[handle("handle.invalid-style", "view.Root")], &[], false);
    assert!(frame.mounts.is_empty());
    assert_eq!(
        frame.diagnostics[0].code,
        BundleViewDiagnosticCode::InvalidControlFlow
    );
    assert_eq!(frame.diagnostics[0].instruction, None);
    assert!(
        frame.diagnostics[0]
            .message
            .contains("only establish named Style sheets")
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the complete typed IR fixture is kept beside all three frame assertions"
)]
fn branch_reacts_per_mount_and_missing_input_never_uses_placeholder() {
    let branch_style = ViewStyleSheetId::try_new("style.branch.inventory").unwrap();
    let program = ViewProgramResource {
        program_id: program_id("view.program.branch"),
        definitions: vec![ViewDefinitionResource {
            public_id: definition_ref("view.Root"),
            body: ViewInstructionSpan::new(0, 3),
            styles: vec![ViewStyleApplicationTarget::named(branch_style)],
            parameters: vec![ViewParameterResource {
                ordinal: 0,
                name: "active".to_owned(),
                role: arcweft_bundle::resource_codec::view::ViewParameterRole::Value,
                value_type: Some(FxRuntimeType::Bool),
                value_slot: Some(0),
                default_program: None,
            }],
            state_schema_hash: 11,
        }],
        value_programs: vec![value_program(
            0,
            vec![FxRuntimeType::Bool],
            Vec::new(),
            FxRuntimeType::Bool,
            vec![
                ValueInstruction::LoadParameter {
                    slot: 0,
                    ty: FxRuntimeType::Bool,
                },
                ValueInstruction::Return,
            ],
        )],
        value_inputs: vec![ViewValueInputResource {
            namespace: ViewValueInputNamespace::Parameter,
            slot: 0,
            value_type: FxRuntimeType::Bool,
            source: ViewValueInputSource::DefinitionParameter {
                view: "view.Root".to_owned(),
                name: "active".to_owned(),
            },
        }],
        instructions: vec![
            ViewProgramInstruction::Branch {
                condition_program: ViewValueProgramId(0),
                then_span: 1,
                else_span: Some(1),
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.yes".to_owned(),
                text_block: "text.block.yes".to_owned(),
                styles: Vec::new(),
                part: None,
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.no".to_owned(),
                text_block: "text.block.no".to_owned(),
                styles: Vec::new(),
                part: None,
                source: None,
            },
        ],
        text_blocks: vec![
            ViewTextBlockResource::new(
                "text.block.yes",
                Some("view.Root".to_owned()),
                None,
                "text.yes",
                ViewTextBlockBounds::from_px(0, 0, 100, 20),
            ),
            ViewTextBlockResource::new(
                "text.block.no",
                Some("view.Root".to_owned()),
                None,
                "text.no",
                ViewTextBlockBounds::from_px(0, 0, 100, 20),
            ),
        ],
        ..ViewProgramResource::default()
    };
    let mut text = text_resource([
        (
            "text.yes",
            ViewTextSourceKind::Literal {
                value: "yes".to_owned(),
            },
        ),
        (
            "text.no",
            ViewTextSourceKind::Literal {
                value: "no".to_owned(),
            },
        ),
    ]);
    text.redactions.push(ViewSecureRedactionMetadata {
        text_source: "text.yes".to_owned(),
        classification: ViewObserveClassification::AgentMasked,
        replacement: Some("masked".to_owned()),
    });
    let mounted = handle("handle.root", "view.Root");

    let mut missing =
        BundleViewRuntime::try_new(Some(program.clone()), Some(text.clone()), None).unwrap();
    let frame = missing.evaluate(std::slice::from_ref(&mounted), &[], false);
    assert!(frame.mounts.is_empty());
    assert_eq!(
        frame.diagnostics[0].code,
        BundleViewDiagnosticCode::MissingInput
    );

    let mut runtime = BundleViewRuntime::try_new(Some(program), Some(text), None).unwrap();
    let active = runtime.evaluate(
        std::slice::from_ref(&mounted),
        &[RuntimeBinding {
            name: "active".to_owned(),
            value: RuntimeValue::Bool(true),
        }],
        false,
    );
    assert!(active.diagnostics.is_empty());
    assert_eq!(plain_text(&active), "yes");
    assert_eq!(active.mounts[0].style_nodes.len(), 1);
    assert_eq!(active.mounts[0].style_nodes[0].instruction, 1);
    assert_eq!(plain_text(&active.redacted_for_observation()), "masked");
    let mount_id = active.mounts[0].mount;

    let inactive = runtime.evaluate(
        std::slice::from_ref(&mounted),
        &[RuntimeBinding {
            name: "active".to_owned(),
            value: RuntimeValue::Bool(false),
        }],
        false,
    );
    assert!(inactive.diagnostics.is_empty());
    assert_eq!(plain_text(&inactive), "no");
    assert_eq!(inactive.mounts[0].style_nodes.len(), 1);
    assert_eq!(inactive.mounts[0].style_nodes[0].instruction, 2);
    assert_eq!(inactive.mounts[0].mount, mount_id);

    let retained = runtime.evaluate(std::slice::from_ref(&mounted), &[], false);
    assert_eq!(plain_text(&retained), "no");
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the parent/child IR fixture and exact restore assertions describe one persistence scenario"
)]
fn view_save_round_trips_stable_nested_owners_and_allocator_stays_fresh() {
    let common_parameters = vec![FxRuntimeType::I32];
    let constant = value_program(
        0,
        common_parameters.clone(),
        Vec::new(),
        FxRuntimeType::I32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::I32(5),
            },
            ValueInstruction::Return,
        ],
    );
    let program = ViewProgramResource {
        program_id: program_id("view.program.nested-runtime"),
        definitions: vec![
            ViewDefinitionResource {
                public_id: definition_ref("view.Parent"),
                body: ViewInstructionSpan::new(0, 3),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 21,
            },
            ViewDefinitionResource {
                public_id: definition_ref("view.Child"),
                body: ViewInstructionSpan::new(3, 4),
                styles: Vec::new(),
                parameters: vec![ViewParameterResource {
                    ordinal: 0,
                    name: "count".to_owned(),
                    role: arcweft_bundle::resource_codec::view::ViewParameterRole::Value,
                    value_type: Some(FxRuntimeType::I32),
                    value_slot: Some(0),
                    default_program: None,
                }],
                state_schema_hash: 22,
            },
        ],
        value_programs: vec![constant],
        value_inputs: vec![ViewValueInputResource {
            namespace: ViewValueInputNamespace::Parameter,
            slot: 0,
            value_type: FxRuntimeType::I32,
            source: ViewValueInputSource::DefinitionParameter {
                view: "view.Child".to_owned(),
                name: "count".to_owned(),
            },
        }],
        instructions: vec![
            ViewProgramInstruction::EmitText {
                text_source: "text.parent.before".to_owned(),
                text_block: "text.parent.before.target".to_owned(),
                styles: Vec::new(),
                part: None,
                source: None,
            },
            ViewProgramInstruction::CallView {
                view: definition_ref("view.Child"),
                arguments: vec![ViewCallArgumentBindingRef {
                    ordinal: 0,
                    name: Some("count".to_owned()),
                    value_program: ViewValueProgramId(0),
                }],
                styles: Vec::new(),
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.parent.after".to_owned(),
                text_block: "text.parent.after.target".to_owned(),
                styles: Vec::new(),
                part: None,
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.child.count".to_owned(),
                text_block: "text.child.count.target".to_owned(),
                styles: Vec::new(),
                part: None,
                source: None,
            },
        ],
        text_blocks: vec![
            ViewTextBlockResource::new(
                "text.parent.before.target",
                Some("view.Parent".to_owned()),
                None,
                "text.parent.before",
                ViewTextBlockBounds::from_px(0, 0, 100, 20),
            ),
            ViewTextBlockResource::new(
                "text.parent.after.target",
                Some("view.Parent".to_owned()),
                None,
                "text.parent.after",
                ViewTextBlockBounds::from_px(0, 40, 100, 20),
            ),
            ViewTextBlockResource::new(
                "text.child.count.target",
                Some("view.Child".to_owned()),
                None,
                "text.child.count",
                ViewTextBlockBounds::from_px(0, 20, 100, 20),
            ),
        ],
        ..ViewProgramResource::default()
    };
    let text = text_resource([
        (
            "text.parent.before",
            ViewTextSourceKind::Literal {
                value: "before".to_owned(),
            },
        ),
        (
            "text.parent.after",
            ViewTextSourceKind::Literal {
                value: "after".to_owned(),
            },
        ),
        (
            "text.child.count",
            ViewTextSourceKind::Projection {
                path: vec!["count".to_owned()],
            },
        ),
    ]);
    let first_handle = handle("handle.first", "view.Parent");
    let mut runtime =
        BundleViewRuntime::try_new(Some(program.clone()), Some(text.clone()), None).unwrap();
    runtime.advance_millis(1_250).unwrap();
    let first = runtime.evaluate(std::slice::from_ref(&first_handle), &[], false);
    assert!(first.diagnostics.is_empty());
    assert_eq!(first.mounts.len(), 2);
    assert_eq!(first.mounts[1].view.as_str(), "view.Child");
    assert_eq!(
        first.mounts[0].paint,
        [
            BundleViewPaintItem::Text {
                source_id: "text.parent.before".to_owned(),
                target: "text.parent.before.target".to_owned(),
            },
            BundleViewPaintItem::Mount {
                mount: first.mounts[1].mount,
            },
            BundleViewPaintItem::Text {
                source_id: "text.parent.after".to_owned(),
                target: "text.parent.after.target".to_owned(),
            },
        ]
    );
    assert_eq!(
        first.mounts[1].text[0].value,
        BundleViewTextValue::Plain {
            value: "5".to_owned()
        }
    );

    let snapshot = runtime.snapshot().unwrap();
    assert_eq!(snapshot.mounts.len(), 2);
    assert!(snapshot.mounts.iter().all(|mount| matches!(
        &mount.owner,
        SavedViewOwner::Arcweft { view, program, .. }
            if (view.as_str() == "view.Parent" || view.as_str() == "view.Child")
                && program.as_str() == "view.program.nested-runtime"
    )));
    let SavedViewOwner::Arcweft {
        revision: accepted_revision,
        ..
    } = &snapshot.mounts[0].owner
    else {
        unreachable!("bundle-authored mount has an Arcweft owner")
    };
    assert_ne!(accepted_revision.as_bytes(), &[0; 32]);
    assert!(snapshot.mounts.iter().all(|mount| matches!(
        &mount.owner,
        SavedViewOwner::Arcweft { revision, .. } if revision == accepted_revision
    )));
    let persisted = serde_json::to_string(&snapshot).unwrap();
    assert!(!persisted.contains("\"registry\""));
    assert!(!persisted.contains("\"definition\""));
    assert!(!persisted.contains("\"rust\""));

    let mut restored =
        BundleViewRuntime::try_new(Some(program.clone()), Some(text.clone()), None).unwrap();
    restored
        .restore(&snapshot, std::slice::from_ref(&first_handle))
        .unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);

    let before_tamper = restored.snapshot().unwrap();
    let mut wrong_program = snapshot.clone();
    let SavedViewOwner::Arcweft { program, .. } = &mut wrong_program.mounts[0].owner else {
        unreachable!("bundle-authored mount has an Arcweft owner")
    };
    *program = program_id("view.program.forged");
    assert!(matches!(
        restored.restore(&wrong_program, std::slice::from_ref(&first_handle)),
        Err(BundleViewRuntimeError::Save(_))
    ));
    assert_eq!(restored.snapshot().unwrap(), before_tamper);

    let mut wrong_revision = snapshot.clone();
    let SavedViewOwner::Arcweft { revision, .. } = &mut wrong_revision.mounts[0].owner else {
        unreachable!("bundle-authored mount has an Arcweft owner")
    };
    *revision = AcceptedViewProgramRevision::try_from_bytes([0x5a; 32]).unwrap();
    assert!(matches!(
        restored.restore(&wrong_revision, std::slice::from_ref(&first_handle)),
        Err(BundleViewRuntimeError::Save(_))
    ));
    assert_eq!(restored.snapshot().unwrap(), before_tamper);

    let mut wrong_implementation = snapshot.clone();
    let saved_view = wrong_implementation.mounts[0].owner.view().clone();
    wrong_implementation.mounts[0].owner = SavedViewOwner::Rust {
        view: saved_view,
        schema: ViewSchemaId(11),
    };
    assert!(matches!(
        restored.restore(&wrong_implementation, std::slice::from_ref(&first_handle)),
        Err(BundleViewRuntimeError::Save(_))
    ));
    assert_eq!(restored.snapshot().unwrap(), before_tamper);
    let after_restore = restored.evaluate(std::slice::from_ref(&first_handle), &[], false);
    assert_eq!(
        after_restore
            .mounts
            .iter()
            .map(|mount| mount.mount)
            .collect::<Vec<_>>(),
        first
            .mounts
            .iter()
            .map(|mount| mount.mount)
            .collect::<Vec<_>>()
    );

    let second_handle = handle("handle.second", "view.Parent");
    let both = restored.evaluate(&[first_handle, second_handle], &[], false);
    assert!(both.diagnostics.is_empty());
    assert_eq!(both.mounts.len(), 4);
    let unique = both
        .mounts
        .iter()
        .map(|mount| mount.mount)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), 4);
    assert!(
        both.mounts
            .iter()
            .map(|mount| mount.mount.get())
            .max()
            .unwrap()
            >= 3
    );
}

#[test]
fn duplicate_repeat_keys_fail_structurally_instead_of_reusing_one_child() {
    let source = value_program(
        0,
        Vec::new(),
        Vec::new(),
        FxRuntimeType::I32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::I32(2),
            },
            ValueInstruction::Return,
        ],
    );
    let duplicate_key = value_program(
        1,
        Vec::new(),
        Vec::new(),
        FxRuntimeType::I32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::I32(7),
            },
            ValueInstruction::Return,
        ],
    );
    let program = ViewProgramResource {
        program_id: program_id("view.program.repeat"),
        definitions: vec![ViewDefinitionResource {
            public_id: definition_ref("view.Repeat"),
            body: ViewInstructionSpan::new(0, 2),
            styles: Vec::new(),
            parameters: Vec::new(),
            state_schema_hash: 31,
        }],
        value_programs: vec![source, duplicate_key],
        instructions: vec![
            ViewProgramInstruction::RepeatKeyed {
                source_program: ViewValueProgramId(0),
                key_program: ViewValueProgramId(1),
                body_span: 1,
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.item".to_owned(),
                text_block: "text.block.item".to_owned(),
                styles: Vec::new(),
                part: None,
                source: None,
            },
        ],
        text_blocks: vec![ViewTextBlockResource::new(
            "text.block.item",
            Some("view.Repeat".to_owned()),
            None,
            "text.item",
            ViewTextBlockBounds::from_px(0, 0, 100, 20),
        )],
        ..ViewProgramResource::default()
    };
    let text = text_resource([(
        "text.item",
        ViewTextSourceKind::Literal {
            value: "item".to_owned(),
        },
    )]);
    let mut runtime = BundleViewRuntime::try_new(Some(program), Some(text), None).unwrap();
    let frame = runtime.evaluate(&[handle("handle.repeat", "view.Repeat")], &[], false);
    assert!(frame.mounts.is_empty());
    assert_eq!(
        frame.diagnostics[0].code,
        BundleViewDiagnosticCode::DuplicateRepeatKey
    );
}

#[test]
fn repeat_style_inventory_retains_one_collision_free_path_per_executed_item() {
    let source = value_program(
        0,
        Vec::new(),
        vec![FxRuntimeType::I32],
        FxRuntimeType::I32,
        vec![
            ValueInstruction::Constant {
                value: FxRuntimeValue::I32(2),
            },
            ValueInstruction::Return,
        ],
    );
    let key = value_program(
        1,
        Vec::new(),
        vec![FxRuntimeType::I32],
        FxRuntimeType::I32,
        vec![
            ValueInstruction::LoadState {
                slot: 0,
                ty: FxRuntimeType::I32,
            },
            ValueInstruction::Return,
        ],
    );
    let sheet = ViewStyleSheetId::try_new("style.repeat.inventory").unwrap();
    let program = ViewProgramResource {
        program_id: program_id("view.program.repeat-style-inventory"),
        definitions: vec![ViewDefinitionResource {
            public_id: definition_ref("view.RepeatStyle"),
            body: ViewInstructionSpan::new(0, 2),
            styles: vec![ViewStyleApplicationTarget::named(sheet)],
            parameters: Vec::new(),
            state_schema_hash: 32,
        }],
        value_programs: vec![source, key],
        value_inputs: vec![ViewValueInputResource {
            namespace: ViewValueInputNamespace::State,
            slot: 0,
            value_type: FxRuntimeType::I32,
            source: ViewValueInputSource::RepeatOrdinal {
                view: "view.RepeatStyle".to_owned(),
                binding: "item".to_owned(),
            },
        }],
        instructions: vec![
            ViewProgramInstruction::RepeatKeyed {
                source_program: ViewValueProgramId(0),
                key_program: ViewValueProgramId(1),
                body_span: 1,
                source: None,
            },
            ViewProgramInstruction::EmitCustom {
                element: "RepeatedItem".to_owned(),
                styles: Vec::new(),
                part: None,
                source: None,
            },
        ],
        ..ViewProgramResource::default()
    };
    let mut runtime = BundleViewRuntime::try_new(Some(program), None, None).unwrap();

    let frame = runtime.evaluate(
        &[handle("handle.repeat-style", "view.RepeatStyle")],
        &[],
        false,
    );

    assert!(frame.diagnostics.is_empty());
    let nodes = &frame.mounts[0].style_nodes;
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].instruction, 1);
    assert_eq!(nodes[1].instruction, 1);
    assert_ne!(nodes[0].path, nodes[1].path);
    assert!(matches!(
        nodes[0].path.segments(),
        [BundleViewInstancePathSegment::Repeat {
            instruction: 0,
            key: 0
        }]
    ));
    assert!(matches!(
        nodes[1].path.segments(),
        [BundleViewInstancePathSegment::Repeat {
            instruction: 0,
            key: 1
        }]
    ));
}

#[test]
fn style_path_words_are_little_endian_injective_and_feed_the_single_node_key() {
    let path: BundleViewInstancePath = serde_json::from_value(serde_json::json!([
        {
            "kind": "call",
            "instruction": 16_909_060,
            "authored_key": 18_446_744_073_709_551_615_u64
        },
        { "kind": "repeat", "instruction": 9, "key": -2 }
    ]))
    .unwrap();
    assert_eq!(
        path.style_path_words(),
        vec![
            0,
            16_909_060,
            1,
            u64::MAX,
            1,
            9,
            u64::from(u32::from_le_bytes((-2_i32).to_le_bytes())),
            0,
        ]
    );

    let id = BundleViewStyleNodeId {
        path,
        instruction: 11,
    };
    let key = id.style_node_key(ViewMountId::from_raw(7));
    assert_eq!(key.mount(), ViewMountId::from_raw(7));
    assert_eq!(key.instruction(), 11);
    assert_eq!(key.path().len(), 8);

    assert_eq!(
        BundleViewStyleNodeKind::Element {
            element: ViewElementKind::Row,
            target: None,
        }
        .runtime_geometry_owner(),
        ViewRuntimeGeometryOwner::Element(ViewElementKind::Row)
    );
    assert_eq!(
        BundleViewStyleNodeKind::Text {
            text_source: "text.main".to_owned(),
        }
        .runtime_geometry_owner(),
        ViewRuntimeGeometryOwner::Text
    );
}

#[test]
fn logical_time_updates_context_cache_and_reduce_motion_freezes_it() {
    let program = ViewProgramResource {
        program_id: program_id("view.program.time"),
        definitions: vec![ViewDefinitionResource {
            public_id: definition_ref("view.Time"),
            body: ViewInstructionSpan::new(0, 2),
            styles: Vec::new(),
            parameters: Vec::new(),
            state_schema_hash: 41,
        }],
        value_programs: vec![value_program(
            0,
            Vec::new(),
            vec![FxRuntimeType::F32],
            FxRuntimeType::F32,
            vec![
                ValueInstruction::LoadContext {
                    slot: FxContextSlot::Time,
                },
                ValueInstruction::Return,
            ],
        )],
        value_inputs: vec![ViewValueInputResource {
            namespace: ViewValueInputNamespace::State,
            slot: 0,
            value_type: FxRuntimeType::F32,
            source: ViewValueInputSource::Local {
                view: "view.Time".to_owned(),
                name: "elapsed".to_owned(),
            },
        }],
        instructions: vec![
            ViewProgramInstruction::BindLocal {
                binding: "elapsed".to_owned(),
                value_program: ViewValueProgramId(0),
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: "text.elapsed".to_owned(),
                text_block: "text.block.elapsed".to_owned(),
                styles: Vec::new(),
                part: None,
                source: None,
            },
        ],
        text_blocks: vec![ViewTextBlockResource::new(
            "text.block.elapsed",
            Some("view.Time".to_owned()),
            None,
            "text.elapsed",
            ViewTextBlockBounds::from_px(0, 0, 100, 20),
        )],
        ..ViewProgramResource::default()
    };
    let text = text_resource([(
        "text.elapsed",
        ViewTextSourceKind::Local {
            name: "elapsed".to_owned(),
        },
    )]);
    let mounted = handle("handle.time", "view.Time");
    let mut runtime = BundleViewRuntime::try_new(Some(program), Some(text), None).unwrap();
    let initial = runtime.evaluate(std::slice::from_ref(&mounted), &[], false);
    assert_eq!(plain_text(&initial), "0");
    runtime.advance_millis(1_000).unwrap();
    let advanced = runtime.evaluate(std::slice::from_ref(&mounted), &[], false);
    assert_eq!(plain_text(&advanced), "1");
    let reduced = runtime.evaluate(std::slice::from_ref(&mounted), &[], true);
    assert_eq!(plain_text(&reduced), "0");
}

#[test]
fn exact_i32_width_is_enforced_at_the_runtime_boundary() {
    let program = ViewProgramResource {
        program_id: program_id("view.program.i32"),
        definitions: vec![ViewDefinitionResource {
            public_id: definition_ref("view.Exact"),
            body: ViewInstructionSpan::new(0, 0),
            styles: Vec::new(),
            parameters: vec![ViewParameterResource {
                ordinal: 0,
                name: "count".to_owned(),
                role: arcweft_bundle::resource_codec::view::ViewParameterRole::Value,
                value_type: Some(FxRuntimeType::I32),
                value_slot: Some(0),
                default_program: None,
            }],
            state_schema_hash: 51,
        }],
        value_inputs: vec![ViewValueInputResource {
            namespace: ViewValueInputNamespace::Parameter,
            slot: 0,
            value_type: FxRuntimeType::I32,
            source: ViewValueInputSource::DefinitionParameter {
                view: "view.Exact".to_owned(),
                name: "count".to_owned(),
            },
        }],
        value_programs: vec![value_program(
            0,
            vec![FxRuntimeType::I32],
            Vec::new(),
            FxRuntimeType::I32,
            vec![
                ValueInstruction::LoadParameter {
                    slot: 0,
                    ty: FxRuntimeType::I32,
                },
                ValueInstruction::Return,
            ],
        )],
        ..ViewProgramResource::default()
    };
    let mut runtime = BundleViewRuntime::try_new(Some(program), None, None).unwrap();
    let frame = runtime.evaluate(
        &[handle("handle.exact", "view.Exact")],
        &[RuntimeBinding {
            name: "count".to_owned(),
            value: RuntimeValue::Int(RuntimeInt::i64(4)),
        }],
        false,
    );
    assert!(frame.mounts.is_empty());
    assert_eq!(
        frame.diagnostics[0].code,
        BundleViewDiagnosticCode::InputType
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the end-to-end typed-store fixture keeps all three source families and their missing-store diagnostic together"
)]
fn typed_text_stores_resolve_localized_rich_and_display_sources_without_string_fallback() {
    let program = ViewProgramResource {
        program_id: program_id("view.program.typed_text"),
        definitions: vec![ViewDefinitionResource {
            public_id: definition_ref("view.TypedText"),
            body: ViewInstructionSpan::new(0, 3),
            styles: Vec::new(),
            parameters: Vec::new(),
            state_schema_hash: 61,
        }],
        instructions: ["localized", "rich", "display"]
            .into_iter()
            .map(|suffix| ViewProgramInstruction::EmitText {
                text_source: format!("text.{suffix}"),
                text_block: format!("text.block.{suffix}"),
                styles: Vec::new(),
                part: None,
                source: None,
            })
            .collect(),
        text_blocks: ["localized", "rich", "display"]
            .into_iter()
            .map(|suffix| {
                ViewTextBlockResource::new(
                    format!("text.block.{suffix}"),
                    Some("view.TypedText".to_owned()),
                    None,
                    format!("text.{suffix}"),
                    ViewTextBlockBounds::from_px(0, 0, 240, 48),
                )
            })
            .collect(),
        ..ViewProgramResource::default()
    };
    let localized_document = RichTextDocument::new(vec![RichTextNode::Text {
        text: "こんにちは".to_owned(),
    }]);
    let rich_document = RichTextDocument::new(vec![RichTextNode::Ruby {
        base: "夢".to_owned(),
        ruby: "ゆめ".to_owned(),
    }]);
    let display_frame = dialogue_frame(
        "say.typed_text.display",
        "view.TypedText",
        "Narrator",
        vec![RichTextNode::Text {
            text: "Display stage".to_owned(),
        }],
    );
    let text = ViewTextResource {
        sources: vec![
            ViewTextSourceRecord {
                public_id: "text.localized".to_owned(),
                kind: ViewTextSourceKind::Localized {
                    key: "text.greeting".to_owned(),
                    locale: Some("ja-JP".to_owned()),
                },
                source: None,
            },
            ViewTextSourceRecord {
                public_id: "text.rich".to_owned(),
                kind: ViewTextSourceKind::RichTextDocument {
                    document: "document.dream".to_owned(),
                },
                source: None,
            },
            ViewTextSourceRecord {
                public_id: "text.display".to_owned(),
                kind: ViewTextSourceKind::DisplayFrame {
                    frame: "display.opening".to_owned(),
                },
                source: None,
            },
        ],
        localized: vec![ViewLocalizedTextResource {
            key: "text.greeting".to_owned(),
            locale: Some("ja-JP".to_owned()),
            document: localized_document.clone(),
        }],
        rich_text_documents: vec![ViewRichTextDocumentResource {
            public_id: "document.dream".to_owned(),
            document: rich_document.clone(),
        }],
        display_frames: vec![ViewDisplayFrameResource {
            public_id: "display.opening".to_owned(),
            frame: display_frame.clone(),
            stage_index: 0,
        }],
        ..ViewTextResource::default()
    };

    let mut runtime =
        BundleViewRuntime::try_new(Some(program.clone()), Some(text.clone()), None).unwrap();
    let frame = runtime.evaluate(&[handle("handle.typed", "view.TypedText")], &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    assert_eq!(frame.mounts[0].paint.len(), 3);
    assert_eq!(frame.mounts[0].text.len(), 3);
    assert!(matches!(
        &frame.mounts[0].text[0].value,
        BundleViewTextValue::Localized { document, .. }
            if document.as_ref() == &localized_document
    ));
    assert!(matches!(
        &frame.mounts[0].text[1].value,
        BundleViewTextValue::RichTextDocument { document }
            if document.as_ref() == &rich_document
    ));
    assert!(matches!(
        &frame.mounts[0].text[2].value,
        BundleViewTextValue::DisplayFrame { frame, stage_index: 0 }
            if frame.as_ref() == &display_frame
    ));
    assert_eq!(
        frame.mounts[0].text[0].targets[0].public_id,
        "text.block.localized"
    );

    let mut missing_text = text;
    missing_text.localized.clear();
    let mut missing = BundleViewRuntime::try_new(Some(program), Some(missing_text), None).unwrap();
    let failure = missing.evaluate(&[handle("handle.typed", "view.TypedText")], &[], false);
    assert!(failure.mounts.is_empty());
    assert_eq!(
        failure.diagnostics[0].code,
        BundleViewDiagnosticCode::MissingLocalizedText
    );
}

#[test]
fn typed_dialogue_projection_uses_one_persistent_authored_mount_per_occurrence() {
    let (program, text) = typed_dialogue_view_resources();
    let display_frame = typed_dialogue_display_frame();
    let dialogue_view = view_id("view.Dialogue");
    let first_handle = PresentationHandleId::try_new("dialogue.40").unwrap();
    let first_inputs = [DialogueViewInput {
        handle: first_handle.clone(),
        view: &dialogue_view,
        frame: &display_frame,
        state: dialogue_view_state(40),
    }];
    let mut runtime = BundleViewRuntime::try_new(Some(program.clone()), Some(text.clone()), None)
        .expect("dialogue View runtime builds");
    let first = runtime.evaluate_with_dialogue(&[], &first_inputs, &[], false);
    assert!(first.diagnostics.is_empty(), "{first:#?}");
    assert_eq!(first.mounts.len(), 1);
    assert_eq!(first.mounts[0].handle, first_handle);
    assert_eq!(first.mounts[0].dialogue, Some(dialogue_view_state(40)));
    assert!(matches!(
        &first.mounts[0].text[0].value,
        BundleViewTextValue::DialogueCharacterDisplayName { label, frame }
            if label == "Hero" && frame.as_ref() == &display_frame
    ));
    assert!(matches!(
        &first.mounts[0].text[1].value,
        BundleViewTextValue::DisplayFrame { frame, stage_index: 0 }
            if frame.as_ref() == &display_frame
    ));
    let first_mount = first.mounts[0].mount;

    let snapshot = runtime.snapshot().unwrap();
    let mut restored = BundleViewRuntime::try_new(Some(program), Some(text), None).unwrap();
    let restored_handle = handle("dialogue.40", "view.Dialogue");
    restored
        .restore(&snapshot, std::slice::from_ref(&restored_handle))
        .expect("mount graph restores");
    let second_handle = PresentationHandleId::try_new("dialogue.41").unwrap();
    let two_inputs = [
        DialogueViewInput {
            handle: first_inputs[0].handle.clone(),
            view: &dialogue_view,
            frame: &display_frame,
            state: dialogue_view_state(40),
        },
        DialogueViewInput {
            handle: second_handle,
            view: &dialogue_view,
            frame: &display_frame,
            state: dialogue_view_state(41),
        },
    ];
    let after_restore = restored.evaluate_with_dialogue(&[], &two_inputs, &[], false);
    assert!(after_restore.diagnostics.is_empty(), "{after_restore:#?}");
    assert_eq!(after_restore.mounts.len(), 2);
    assert_eq!(
        after_restore
            .mounts
            .iter()
            .find(|mount| mount.handle == first_inputs[0].handle)
            .expect("first occurrence remains mounted")
            .mount,
        first_mount
    );
    assert_ne!(after_restore.mounts[0].mount, after_restore.mounts[1].mount);
}

#[test]
fn replacement_cannot_remove_or_retype_a_live_dialogue_view_owner() {
    let (program, text) = typed_dialogue_view_resources();
    let display_frame = typed_dialogue_display_frame();
    let mut runtime =
        AcceptedBundleViewRuntime::try_new(validated_product(program.clone()), Some(text))
            .expect("dialogue View runtime builds");
    let dialogue_view = view_id("view.Dialogue");
    let inputs = [DialogueViewInput {
        handle: PresentationHandleId::try_new("dialogue.replacement").unwrap(),
        view: &dialogue_view,
        frame: &display_frame,
        state: dialogue_view_state(70),
    }];
    let frame = runtime.evaluate_with_dialogue(&[], &inputs, &[], false);
    assert!(frame.diagnostics.is_empty(), "{frame:#?}");
    let before = runtime.snapshot().expect("live runtime snapshots");

    let removed = validated_product(ViewProgramResource {
        program_id: program.program_id.clone(),
        ..ViewProgramResource::default()
    });
    assert!(matches!(
        runtime.prepare_view_program_replacement(removed),
        Err(ViewProgramReplacementError::MissingRequiredDialogueView { definition })
            if definition == dialogue_view
    ));
    assert_eq!(runtime.snapshot().unwrap(), before);

    let mut wrong_role = program;
    wrong_role.definitions[0].parameters[0].role =
        arcweft_bundle::resource_codec::view::ViewParameterRole::Value;
    assert!(matches!(
        runtime.prepare_view_program_replacement(validated_product(wrong_role)),
        Err(ViewProgramReplacementError::RequiredDialogueViewMissingRole { definition })
            if definition == dialogue_view
    ));
    assert_eq!(runtime.snapshot().unwrap(), before);
}

#[test]
fn runtime_rejects_handcrafted_dialogue_projection_with_wrong_parameter_role() {
    let (mut program, text) = typed_dialogue_view_resources();
    program.definitions[0].parameters[0].role =
        arcweft_bundle::resource_codec::view::ViewParameterRole::Value;

    let error = BundleViewRuntime::try_new(Some(program), Some(text), None)
        .expect_err("runtime must reject an untyped dialogue projection");

    assert!(matches!(
        error,
        arcweft_runtime_driver::view_runtime::BundleViewRuntimeError::DialogueContract(
            arcweft_bundle::resource_codec::view::DialogueViewContractError::InvalidTextParameterRole { .. }
        )
    ));
}

fn typed_dialogue_view_resources() -> (ViewProgramResource, ViewTextResource) {
    let program = ViewProgramResource {
        program_id: program_id("view.program.dialogue"),
        definitions: vec![ViewDefinitionResource {
            public_id: definition_ref("view.Dialogue"),
            body: ViewInstructionSpan::new(0, 2),
            styles: Vec::new(),
            parameters: vec![ViewParameterResource {
                ordinal: 0,
                name: "dialogue".to_owned(),
                role: arcweft_bundle::resource_codec::view::ViewParameterRole::Dialogue,
                value_type: None,
                value_slot: None,
                default_program: None,
            }],
            state_schema_hash: 91,
        }],
        instructions: ["speaker", "content"]
            .into_iter()
            .map(|suffix| ViewProgramInstruction::EmitText {
                text_source: format!("text.dialogue.{suffix}"),
                text_block: format!("text.block.{suffix}"),
                styles: Vec::new(),
                part: None,
                source: None,
            })
            .collect(),
        text_blocks: ["speaker", "content"]
            .into_iter()
            .map(|suffix| {
                ViewTextBlockResource::new(
                    format!("text.block.{suffix}"),
                    Some("view.Dialogue".to_owned()),
                    None,
                    format!("text.dialogue.{suffix}"),
                    ViewTextBlockBounds::from_px(0, 0, 640, 96),
                )
                .with_surface(if suffix == "content" {
                    arcweft_bundle::resource_codec::view::ViewTextSurface::RichText
                } else {
                    arcweft_bundle::resource_codec::view::ViewTextSurface::Text
                })
            })
            .collect(),
        ..ViewProgramResource::default()
    };
    let text = text_resource([
        (
            "text.dialogue.speaker",
            ViewTextSourceKind::Dialogue {
                parameter: "dialogue".to_owned(),
                projection: DialogueTextProjection::CharacterDisplayName,
            },
        ),
        (
            "text.dialogue.content",
            ViewTextSourceKind::Dialogue {
                parameter: "dialogue".to_owned(),
                projection: DialogueTextProjection::Content,
            },
        ),
    ]);
    (program, text)
}

fn typed_dialogue_display_frame() -> LineDisplayFrame {
    dialogue_frame(
        "say.dialogue.typed",
        "view.Dialogue",
        "Hero",
        vec![RichTextNode::Ruby {
            base: "夢".to_owned(),
            ruby: "ゆめ".to_owned(),
        }],
    )
}

fn dialogue_view_state(identity: u64) -> DialogueViewState {
    DialogueViewState {
        occurrence: DialogueViewOccurrence {
            presentation: DialoguePresentationId::new(identity),
            entry: DialogueEntryId::new(identity),
            instance: DialogueInstanceId::new(identity),
        },
        stage: DialogueViewStage {
            index: DialogueStageIndex::new(0),
            page: DialoguePageIndex::new(0),
            stage_count: 1,
            page_count: 1,
        },
        reveal: DialogueViewReveal::complete(),
        primary_action: DialogueViewPrimaryAction { target: None },
    }
}

#[test]
fn standard_dialogue_resource_uses_the_same_typed_mount_path() {
    let frame = dialogue_frame(
        "say.standard.dialogue",
        arcweft_bundle::standard_view::DIALOGUE_VIEW_ID,
        "Narrator",
        vec![RichTextNode::Text {
            text: "Standard authored View".to_owned(),
        }],
    );
    let mut dialogue = DialoguePresentationStore::default();
    dialogue
        .apply_operations(&[DialoguePresentationOperation::append(
            DialogueViewDefinition::new(view_id(arcweft_bundle::standard_view::DIALOGUE_VIEW_ID)),
            frame,
        )])
        .unwrap();
    dialogue
        .synchronize_waiting_line(Some(
            &RuntimeLineId::from_runtime_line_value("say.standard.dialogue").unwrap(),
        ))
        .unwrap();
    let mut runtime = BundleViewRuntime::try_new(
        Some(arcweft_bundle::standard_view::dialogue_program()),
        Some(arcweft_bundle::standard_view::dialogue_text()),
        Some(&arcweft_bundle::standard_view::dialogue_style()),
    )
    .unwrap();
    let output = runtime.evaluate_with_dialogue(&[], &dialogue.view_inputs(), &[], false);

    assert!(output.diagnostics.is_empty(), "{output:#?}");
    assert_eq!(output.mounts.len(), 1);
    assert_eq!(
        output.mounts[0].view.as_str(),
        arcweft_bundle::standard_view::DIALOGUE_VIEW_ID
    );
    assert_eq!(output.mounts[0].text.len(), 2);
    assert!(
        output.mounts[0]
            .dialogue
            .is_some_and(|state| state.primary_action.target.is_some())
    );
}
