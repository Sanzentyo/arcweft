use arcweft_bundle::resource_codec::view::{
    DialogueTextProjection, ViewDefinitionRef, ViewElementKind, ViewExportedPart,
    ViewObserveClassification, ViewOwnedPartRef, ViewPartExportSourceRef, ViewProgramInstruction,
    ViewSecureRedactionMetadata, ViewStyleApplicationTarget, ViewStylePatchId, ViewStyleSheetId,
    ViewTextSourceKind, ViewTextSourceRecord,
};
use arcweft_bundle::resource_codec::{
    PublicIdRef, SourceMapSourceId, SourceRangeRef, ViewCallArgumentBindingRef,
    ViewDefinitionResource, ViewDisplayFrameResource, ViewInstructionSpan,
    ViewLocalizedTextResource, ViewParameterResource, ViewProgramResource,
    ViewRichTextDocumentResource, ViewTextBlockBounds, ViewTextBlockResource, ViewTextResource,
    ViewValueInputNamespace, ViewValueInputResource, ViewValueInputSource,
};
use arcweft_core::plan::RuntimeLineId;
use arcweft_core::value::{RuntimeBinding, RuntimeInt, RuntimeValue};
use arcweft_presentation::fx::{
    FxContextSlot, FxRuntimeType, FxRuntimeValue, ValueInstruction, ValueProgramSchema,
};
use arcweft_render_text::{LineDisplaySpec, RichTextDocument, RichTextNode, RuntimeLineContext};
use arcweft_runtime_driver::dialogue::{
    DialoguePageIndex, DialoguePresentationOperation, DialoguePresentationStore, DialogueViewInput,
    DialogueViewOccurrence, DialogueViewPrimaryAction, DialogueViewReveal, DialogueViewStage,
    DialogueViewState,
};
use arcweft_runtime_driver::presentation_handles::{
    PresentationHandleId, PresentationHandleKind, PresentationHandleRecord,
    PresentationResourceState,
};
use arcweft_runtime_driver::view_runtime::{
    BundleViewDiagnosticCode, BundleViewInstancePathSegment, BundleViewMountOutput,
    BundleViewPaintItem, BundleViewRuntime, BundleViewStyleNode, BundleViewStyleNodeKind,
    BundleViewTextValue,
};
use arcweft_view::{
    DialogueEntryId, DialogueInstanceId, DialoguePresentationId, DialogueStageIndex,
    ViewPartLocalName, ViewPartName,
};
use arcweft_view::{ViewValueProgram, ViewValueProgramId};

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
        1,
        "the corrected unpublished payload remains the initial save schema"
    );
    let runtime = BundleViewRuntime::try_new(None, None, None).unwrap();
    let snapshot = runtime.snapshot();
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

fn local_part(value: &str) -> ViewPartLocalName {
    ViewPartLocalName::try_new(value).expect("valid local part identity")
}

fn public_part(value: &str) -> ViewPartName {
    ViewPartName::try_new(value).expect("valid public part identity")
}

fn exported_part(owner: &str, local: &str, public: &str) -> ViewExportedPart {
    ViewExportedPart {
        target: ViewOwnedPartRef::new(
            ViewDefinitionRef::try_new(owner).expect("valid View owner"),
            local_part(local),
        ),
        public_name: public_part(public),
        source: ViewPartExportSourceRef {
            source_id: SourceMapSourceId::try_new("view-runtime.arcw")
                .expect("valid source identity"),
            declaration: SourceRangeRef {
                source: PublicIdRef(0),
                start_byte: 0,
                end_byte: 32,
            },
            local_name: SourceRangeRef {
                source: PublicIdRef(0),
                start_byte: 12,
                end_byte: 20,
            },
            public_name: SourceRangeRef {
                source: PublicIdRef(0),
                start_byte: 24,
                end_byte: 31,
            },
        },
    }
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
        program_id: "view.program.style-subtrees".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.Root".to_owned(),
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
    ViewProgramResource {
        program_id: "view.program.style-call-boundary".to_owned(),
        definitions: vec![
            ViewDefinitionResource {
                public_id: "view.Parent".to_owned(),
                body: ViewInstructionSpan::new(0, 2),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 1,
            },
            ViewDefinitionResource {
                public_id: "view.Child".to_owned(),
                body: ViewInstructionSpan::new(2, 6),
                styles: vec![ViewStyleApplicationTarget::named(child_sheet.clone())],
                parameters: Vec::new(),
                state_schema_hash: 2,
            },
        ],
        instructions: vec![
            ViewProgramInstruction::CallView {
                view: "view.Child".to_owned(),
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
        .find(|mount| mount.view == "view.Parent")
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
        .find(|mount| mount.view == "view.Child")
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
}

#[test]
fn exported_part_access_does_not_cross_two_nested_view_boundaries() {
    let external_sheet = ViewStyleSheetId::try_new("style.external.owner").unwrap();
    let program = ViewProgramResource {
        program_id: "view.program.non-transitive-export".to_owned(),
        definitions: vec![
            ViewDefinitionResource {
                public_id: "view.A".to_owned(),
                body: ViewInstructionSpan::new(0, 1),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 1,
            },
            ViewDefinitionResource {
                public_id: "view.B".to_owned(),
                body: ViewInstructionSpan::new(1, 2),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 2,
            },
            ViewDefinitionResource {
                public_id: "view.C".to_owned(),
                body: ViewInstructionSpan::new(2, 3),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 3,
            },
        ],
        instructions: vec![
            ViewProgramInstruction::CallView {
                view: "view.B".to_owned(),
                arguments: Vec::new(),
                styles: vec![ViewStyleApplicationTarget::named(external_sheet.clone())],
                part: None,
                key: None,
                source: None,
            },
            ViewProgramInstruction::CallView {
                view: "view.C".to_owned(),
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
        exported_parts: vec![exported_part("view.C", "part.c.exported", "part.public-c")],
        ..ViewProgramResource::default()
    };
    let mut runtime = BundleViewRuntime::try_new(Some(program), None, None).unwrap();
    let frame = runtime.evaluate(&[handle("handle.a", "view.A")], &[], false);
    assert!(frame.diagnostics.is_empty());

    let deep = frame
        .mounts
        .iter()
        .find(|mount| mount.view == "view.C")
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
        program_id: "view.program.invalid-root-inline".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.Root".to_owned(),
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
        program_id: "view.program.branch".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.Root".to_owned(),
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
fn nested_mounts_round_trip_exactly_and_allocator_stays_fresh() {
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
        program_id: "view.program.nested-runtime".to_owned(),
        definitions: vec![
            ViewDefinitionResource {
                public_id: "view.Parent".to_owned(),
                body: ViewInstructionSpan::new(0, 3),
                styles: Vec::new(),
                parameters: Vec::new(),
                state_schema_hash: 21,
            },
            ViewDefinitionResource {
                public_id: "view.Child".to_owned(),
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
                view: "view.Child".to_owned(),
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
    assert_eq!(first.mounts[1].view, "view.Child");
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

    let snapshot = runtime.snapshot();
    let mut restored = BundleViewRuntime::try_new(Some(program), Some(text), None).unwrap();
    restored
        .restore(&snapshot, std::slice::from_ref(&first_handle))
        .unwrap();
    assert_eq!(restored.snapshot(), snapshot);
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
        program_id: "view.program.repeat".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.Repeat".to_owned(),
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
        program_id: "view.program.repeat-style-inventory".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.RepeatStyle".to_owned(),
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
fn logical_time_updates_context_cache_and_reduce_motion_freezes_it() {
    let program = ViewProgramResource {
        program_id: "view.program.time".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.Time".to_owned(),
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
        program_id: "view.program.i32".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.Exact".to_owned(),
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
        program_id: "view.program.typed_text".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.TypedText".to_owned(),
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
    let display_document = RichTextDocument::new(vec![RichTextNode::Text {
        text: "Display stage".to_owned(),
    }]);
    let display_frame = LineDisplaySpec {
        line: RuntimeLineId::from_runtime_line_value("say.typed_text.display").unwrap(),
        callee: "narrator".to_owned(),
        speaker_label: None,
        text_key: None,
        view: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: display_document,
    }
    .resolve_frame(&RuntimeLineContext::new(Vec::new()))
    .unwrap();
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
    let first_handle = PresentationHandleId::try_new("dialogue.40").unwrap();
    let first_inputs = [DialogueViewInput {
        handle: first_handle.clone(),
        view: "view.Dialogue",
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
        BundleViewTextValue::DialogueSpeaker { label, frame }
            if label == "Hero" && frame.as_ref() == &display_frame
    ));
    assert!(matches!(
        &first.mounts[0].text[1].value,
        BundleViewTextValue::DisplayFrame { frame, stage_index: 0 }
            if frame.as_ref() == &display_frame
    ));
    let first_mount = first.mounts[0].mount;

    let snapshot = runtime.snapshot();
    let mut restored = BundleViewRuntime::try_new(Some(program), Some(text), None).unwrap();
    let restored_handle = handle("dialogue.40", "view.Dialogue");
    restored
        .restore(&snapshot, std::slice::from_ref(&restored_handle))
        .expect("mount graph restores");
    let second_handle = PresentationHandleId::try_new("dialogue.41").unwrap();
    let two_inputs = [
        DialogueViewInput {
            handle: first_inputs[0].handle.clone(),
            view: "view.Dialogue",
            frame: &display_frame,
            state: dialogue_view_state(40),
        },
        DialogueViewInput {
            handle: second_handle,
            view: "view.Dialogue",
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
        program_id: "view.program.dialogue".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.Dialogue".to_owned(),
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
                projection: DialogueTextProjection::Speaker,
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

fn typed_dialogue_display_frame() -> arcweft_render_text::LineDisplayFrame {
    LineDisplaySpec {
        line: RuntimeLineId::from_runtime_line_value("say.dialogue.typed").unwrap(),
        callee: "character.hero".to_owned(),
        speaker_label: Some("Hero".to_owned()),
        text_key: None,
        view: Some("view.Dialogue".to_owned()),
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![RichTextNode::Ruby {
            base: "夢".to_owned(),
            ruby: "ゆめ".to_owned(),
        }]),
    }
    .resolve_frame(&RuntimeLineContext::new(Vec::new()))
    .unwrap()
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
    let frame = LineDisplaySpec {
        line: RuntimeLineId::from_runtime_line_value("say.standard.dialogue").unwrap(),
        callee: "narrator".to_owned(),
        speaker_label: Some("Narrator".to_owned()),
        text_key: None,
        view: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![RichTextNode::Text {
            text: "Standard authored View".to_owned(),
        }]),
    }
    .resolve_frame(&RuntimeLineContext::new(Vec::new()))
    .unwrap();
    let mut dialogue = DialoguePresentationStore::default();
    dialogue
        .apply_operations(&[DialoguePresentationOperation::append(
            arcweft_bundle::standard_view::DIALOGUE_VIEW_ID,
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
        output.mounts[0].view,
        arcweft_bundle::standard_view::DIALOGUE_VIEW_ID
    );
    assert_eq!(output.mounts[0].text.len(), 2);
    assert!(
        output.mounts[0]
            .dialogue
            .is_some_and(|state| state.primary_action.target.is_some())
    );
}
