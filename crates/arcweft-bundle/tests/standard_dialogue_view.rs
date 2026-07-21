use arcweft_bundle::BundleCodecError;
use arcweft_bundle::resource_codec::view::{
    ViewActionButtonActionResource, ViewDefinitionResource, ViewInstructionSpan,
    ViewProgramResource, ViewResourceMergeError, ViewRuntimeButtonBounds, ViewRuntimeSurfaceBounds,
    ViewStyleResource, ViewTextBlockBounds, ViewTextSourceKind,
};
use arcweft_bundle::resource_codec::{
    MAX_SOURCE_MAP_DOCUMENTS, SectionCodecError, SourceMapBuildError, SourceMapSection,
};
use arcweft_bundle::standard_view::{
    DIALOGUE_STYLE_ID, DIALOGUE_STYLE_SOURCE_ID, DIALOGUE_VIEW_ID, dialogue_program,
    dialogue_style, dialogue_text,
};
use arcweft_core::plan::RuntimeLineId;
use arcweft_presentation::appearance::PresentationColor;
use arcweft_render_text::{LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use arcweft_view::ViewId;
use arcweft_view::style::{
    ViewColorValue, ViewLengthMilli, ViewPosition, ViewPropertyKind, ViewSpecifiedValue,
    ViewStyleApplicationTarget, ViewStyleDeclaration,
};

#[test]
fn standard_dialogue_view_is_a_complete_encodable_authored_resource() {
    let program = dialogue_program();
    let text = dialogue_text();
    let style = dialogue_style();

    assert_eq!(program.definitions[0].public_id.as_str(), DIALOGUE_VIEW_ID);
    assert_eq!(program.definitions[0].parameters[0].name, "dialogue");
    assert!(matches!(
        &program.action_buttons[0].action,
        ViewActionButtonActionResource::DialoguePrimaryAction { parameter }
            if parameter == "dialogue"
    ));
    assert_eq!(program.surfaces.len(), 1);
    assert_eq!(program.text_blocks.len(), 2);
    assert_eq!(text.sources.len(), 3);
    assert_eq!(style.program.sheets().len(), 1);
    assert_eq!(style.program.sheets()[0].rules().len(), 4);
    assert!(matches!(
        program.definitions[0].styles.as_slice(),
        [ViewStyleApplicationTarget::Named { sheet }]
            if sheet.public_id().as_str() == DIALOGUE_STYLE_ID
    ));
    assert_eq!(
        program.surfaces[0].bounds,
        ViewRuntimeSurfaceBounds::new(57_600, 460_800, 1_164_800, 201_600)
    );
    assert_eq!(
        program.text_blocks[0].bounds,
        ViewTextBlockBounds::new(85_600, 518_800, 1_108_800, 125_600)
    );
    assert_eq!(
        program.text_blocks[1].bounds,
        ViewTextBlockBounds::new(85_600, 480_800, 1_108_800, 28_000)
    );
    assert_eq!(
        program.action_buttons[0].bounds,
        ViewRuntimeButtonBounds::new(57_600, 460_800, 1_164_800, 201_600)
    );
    assert!(text.sources.iter().any(|source| matches!(
        &source.kind,
        ViewTextSourceKind::Literal { value } if value.is_empty()
    )));
    assert_standard_style_declarations(&style);

    program
        .encode_canonical_section()
        .expect("standard View program encodes");
    text.encode_canonical_section()
        .expect("standard View text encodes");
    style
        .encode_canonical_section()
        .expect("standard View style encodes");
}

fn assert_standard_style_declarations(style: &ViewStyleResource) {
    let rules = style.program.sheets()[0].rules();
    assert!(rules.iter().any(|rule| {
        rule.declarations().iter().any(|declaration| {
            declaration.property() == ViewPropertyKind::BackgroundColor
                && declaration.value()
                    == &ViewSpecifiedValue::Color {
                        value: ViewColorValue::Literal {
                            color: PresentationColor::rgba(17, 18, 16, 242),
                        },
                    }
        })
    }));
    assert_declaration(
        rules[0].declarations(),
        ViewPropertyKind::Position,
        &position(ViewPosition::Absolute),
    );
    assert_declaration(
        rules[0].declarations(),
        ViewPropertyKind::Left,
        &length(57_600),
    );
    assert_declaration(
        rules[0].declarations(),
        ViewPropertyKind::Top,
        &length(460_800),
    );
    assert_declaration(
        rules[1].declarations(),
        ViewPropertyKind::Position,
        &position(ViewPosition::Absolute),
    );
    assert_declaration(
        rules[1].declarations(),
        ViewPropertyKind::Left,
        &length(28_000),
    );
    assert_declaration(
        rules[1].declarations(),
        ViewPropertyKind::Top,
        &length(20_000),
    );
    assert_declaration(
        rules[2].declarations(),
        ViewPropertyKind::Position,
        &position(ViewPosition::Absolute),
    );
    assert_declaration(
        rules[2].declarations(),
        ViewPropertyKind::Left,
        &length(28_000),
    );
    assert_declaration(
        rules[2].declarations(),
        ViewPropertyKind::Top,
        &length(58_000),
    );
    assert_declaration(
        rules[3].declarations(),
        ViewPropertyKind::Position,
        &position(ViewPosition::Absolute),
    );
    assert_declaration(rules[3].declarations(), ViewPropertyKind::Left, &length(0));
    assert_declaration(rules[3].declarations(), ViewPropertyKind::Top, &length(0));
}

#[test]
fn authored_program_is_merged_without_replacing_the_reserved_standard_definition() {
    let authored = ViewProgramResource {
        program_id: arcweft_view::ViewProgramId::try_new("view.project").unwrap(),
        definitions: vec![ViewDefinitionResource {
            public_id: arcweft_bundle::resource_codec::view::ViewDefinitionRef::new(
                ViewId::try_new("view.CustomDialogue").unwrap(),
            ),
            body: ViewInstructionSpan::new(0, 0),
            styles: Vec::new(),
            parameters: Vec::new(),
            state_schema_hash: 7,
        }],
        ..ViewProgramResource::default()
    };
    let bundle = test_bundle()
        .with_view_resources(Some(authored), None)
        .expect("authored View resources merge");
    let program = bundle.view_program.expect("bundle retains View program");

    assert_eq!(program.program_id.as_str(), "view.project");
    assert!(
        program
            .definitions
            .iter()
            .any(|definition| definition.public_id.as_str() == DIALOGUE_VIEW_ID)
    );
    assert!(
        program
            .definitions
            .iter()
            .any(|definition| definition.public_id.as_str() == "view.CustomDialogue")
    );
}

#[test]
fn bundle_source_map_owns_the_standard_dialogue_style_source() {
    let bundle = test_bundle();

    assert!(
        bundle
            .source_map
            .documents()
            .any(|source| { source.document_id().as_str() == DIALOGUE_STYLE_SOURCE_ID })
    );
    assert_eq!(bundle.source_display_name(), "test.arcw");
    assert_eq!(
        bundle
            .primary_source_document()
            .expect("authored primary source")
            .document_id()
            .as_str(),
        "test.arcw"
    );
}

#[test]
fn reserved_standard_dialogue_view_id_cannot_be_overridden() {
    let authored = ViewProgramResource {
        program_id: arcweft_view::ViewProgramId::try_new("view.project").unwrap(),
        definitions: vec![ViewDefinitionResource {
            public_id: arcweft_bundle::resource_codec::view::ViewDefinitionRef::new(
                ViewId::try_new_engine_owned(DIALOGUE_VIEW_ID).unwrap(),
            ),
            body: ViewInstructionSpan::new(0, 0),
            styles: Vec::new(),
            parameters: Vec::new(),
            state_schema_hash: 99,
        }],
        ..ViewProgramResource::default()
    };
    let error = test_bundle()
        .with_view_resources(Some(authored), None)
        .expect_err("reserved standard View identity rejects during atomic merge");

    assert_eq!(
        error,
        ViewResourceMergeError::Section(SectionCodecError::DuplicatePublicId(
            "view_definitions:std.view.dialogue".to_owned(),
        )),
    );
}

#[test]
fn dialogue_primary_action_requires_a_declared_typed_parameter() {
    let mut program = dialogue_program();
    program.definitions[0].parameters.clear();

    assert!(program.encode_canonical_section().is_err());
}

#[test]
fn reserved_standard_source_collision_is_a_typed_bundle_build_error() {
    let conflicting = SourceDocument::try_new(
        SourceDocumentId::try_new(DIALOGUE_STYLE_SOURCE_ID).expect("reserved source ID"),
        SourceName::path("user-controlled.arcw"),
        "user-controlled text",
    )
    .expect("conflicting source document");
    let source_map =
        SourceMapSection::try_from_documents(&[&conflicting]).expect("conflicting source map");

    let error = try_test_bundle(source_map).expect_err("reserved source collision must reject");

    assert!(matches!(
        error,
        SourceMapBuildError::DuplicateDocument(id)
            if id.as_str() == DIALOGUE_STYLE_SOURCE_ID
    ));
}

#[test]
fn full_user_source_map_cannot_panic_when_standard_source_is_reserved() {
    let documents = (0..MAX_SOURCE_MAP_DOCUMENTS)
        .map(|index| {
            let id = format!("user/{index}.arcw");
            SourceDocument::try_new(
                SourceDocumentId::try_new(id.clone()).expect("source ID"),
                SourceName::path(id),
                "",
            )
            .expect("source document")
        })
        .collect::<Vec<_>>();
    let source_map = SourceMapSection::try_from_documents(&documents.iter().collect::<Vec<_>>())
        .expect("full source map");

    let error = try_test_bundle(source_map).expect_err("standard source needs one reserved slot");

    assert_eq!(
        error,
        SourceMapBuildError::TooManyDocuments {
            actual: MAX_SOURCE_MAP_DOCUMENTS + 1,
            limit: MAX_SOURCE_MAP_DOCUMENTS,
        }
    );
}

#[test]
fn dialogue_view_id_round_trips_as_the_accepted_public_owner() {
    let mut bundle = test_bundle();
    bundle.display = LineDisplayCatalog::new(vec![display_spec(
        RuntimeLineId::from_runtime_line_value("say.accepted").expect("line ID"),
        ViewId::try_new_engine_owned(DIALOGUE_VIEW_ID).expect("standard View ID"),
    )]);

    let encoded = bundle.to_json_bytes().expect("accepted owner encodes");
    let decoded =
        arcweft_bundle::ArcweftBundle::from_json_slice(&encoded).expect("accepted owner decodes");

    assert_eq!(decoded.display.lines()[0].view.as_str(), DIALOGUE_VIEW_ID);
}

#[test]
fn dialogue_view_id_rejects_malformed_wire_identity() {
    let mut bundle = test_bundle();
    bundle.display = LineDisplayCatalog::new(vec![display_spec(
        RuntimeLineId::from_runtime_line_value("say.malformed").expect("line ID"),
        ViewId::try_new_engine_owned(DIALOGUE_VIEW_ID).expect("standard View ID"),
    )]);
    let mut payload: serde_json::Value =
        serde_json::from_slice(&bundle.to_json_bytes().expect("fixture encodes"))
            .expect("fixture JSON");
    payload["display"]["lines"][0]["view"] = serde_json::json!("not a public View id");

    assert!(matches!(
        arcweft_bundle::ArcweftBundle::from_json_slice(
            &serde_json::to_vec(&payload).expect("tampered JSON encodes")
        ),
        Err(BundleCodecError::Decode(_))
    ));
}

fn assert_declaration(
    declarations: &[ViewStyleDeclaration],
    property: ViewPropertyKind,
    value: &ViewSpecifiedValue,
) {
    assert!(
        declarations
            .iter()
            .any(|declaration| declaration.property() == property && declaration.value() == value),
        "missing {property:?} = {value:?}",
    );
}

fn length(value: i32) -> ViewSpecifiedValue {
    ViewSpecifiedValue::Length {
        value: ViewLengthMilli::new(value),
    }
}

const fn position(value: ViewPosition) -> ViewSpecifiedValue {
    ViewSpecifiedValue::Position { value }
}

#[test]
fn dialogue_view_id_is_required_and_rejects_null_wire_identity() {
    let mut bundle = test_bundle();
    bundle.display = LineDisplayCatalog::new(vec![display_spec(
        RuntimeLineId::from_runtime_line_value("say.required").expect("line ID"),
        ViewId::try_new_engine_owned(DIALOGUE_VIEW_ID).expect("standard View ID"),
    )]);
    let payload: serde_json::Value =
        serde_json::from_slice(&bundle.to_json_bytes().expect("fixture encodes"))
            .expect("fixture JSON");

    let mut missing = payload.clone();
    missing["display"]["lines"][0]
        .as_object_mut()
        .expect("display line object")
        .remove("view");
    assert!(matches!(
        arcweft_bundle::ArcweftBundle::from_json_slice(
            &serde_json::to_vec(&missing).expect("missing-field JSON encodes")
        ),
        Err(BundleCodecError::Decode(_))
    ));

    let mut null = payload;
    null["display"]["lines"][0]["view"] = serde_json::Value::Null;
    assert!(matches!(
        arcweft_bundle::ArcweftBundle::from_json_slice(
            &serde_json::to_vec(&null).expect("null-field JSON encodes")
        ),
        Err(BundleCodecError::Decode(_))
    ));
}

#[test]
fn dialogue_view_id_rejects_unknown_public_owner() {
    let line = RuntimeLineId::from_runtime_line_value("say.unknown").expect("line ID");
    let unknown = ViewId::try_new("view.UnknownDialogue").expect("well-formed View ID");
    let mut bundle = test_bundle();
    bundle.display = LineDisplayCatalog::new(vec![display_spec(line.clone(), unknown.clone())]);

    assert!(matches!(
        bundle.to_json_bytes(),
        Err(BundleCodecError::MissingDialogueViewDefinition { line: actual, view })
            if actual == line && view == unknown
    ));
}

#[test]
fn dialogue_view_id_rejects_registered_owner_without_dialogue_role() {
    let line = RuntimeLineId::from_runtime_line_value("say.wrong-role").expect("line ID");
    let owner = ViewId::try_new("view.NotDialogue").expect("well-formed View ID");
    let authored = ViewProgramResource {
        program_id: arcweft_view::ViewProgramId::try_new("view.project.role").unwrap(),
        definitions: vec![ViewDefinitionResource {
            public_id: arcweft_bundle::resource_codec::view::ViewDefinitionRef::new(owner.clone()),
            body: ViewInstructionSpan::new(0, 0),
            styles: Vec::new(),
            parameters: Vec::new(),
            state_schema_hash: 7,
        }],
        ..ViewProgramResource::default()
    };
    let mut bundle = test_bundle()
        .with_view_resources(Some(authored), None)
        .expect("authored View resources merge");
    bundle.display = LineDisplayCatalog::new(vec![display_spec(line.clone(), owner.clone())]);

    assert!(matches!(
        bundle.to_json_bytes(),
        Err(BundleCodecError::DialogueViewDefinitionMissingRole {
            line: actual,
            view,
        }) if actual == line && view == owner
    ));
}

#[test]
fn dialogue_view_validation_rejects_duplicate_owners_before_role_selection() {
    let owner = ViewId::try_new_engine_owned(DIALOGUE_VIEW_ID).expect("standard View ID");
    let mut bundle = test_bundle();
    let program = bundle.view_program.as_mut().expect("standard View program");
    let mut duplicate = program.definitions[0].clone();
    duplicate.parameters.clear();
    program.definitions.push(duplicate);
    bundle.display = LineDisplayCatalog::new(vec![display_spec(
        RuntimeLineId::from_runtime_line_value("say.duplicate-view").expect("line ID"),
        owner.clone(),
    )]);

    assert!(matches!(
        bundle.to_json_bytes(),
        Err(BundleCodecError::DuplicateViewDefinition { view }) if view == owner
    ));
}

fn display_spec(line: RuntimeLineId, view: ViewId) -> LineDisplaySpec {
    LineDisplaySpec {
        line,
        callee: "narrator".to_owned(),
        speaker_label: None,
        text_key: None,
        view,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![RichTextNode::Text {
            text: "dialogue".to_owned(),
        }]),
    }
}

fn test_bundle() -> arcweft_bundle::ArcweftBundle {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("test.arcw").expect("source ID"),
        SourceName::path("test.arcw"),
        "",
    )
    .expect("source document");

    try_test_bundle(SourceMapSection::try_from_documents(&[&document]).expect("source map"))
        .expect("standard dialogue source joins source map")
}

fn try_test_bundle(
    source_map: SourceMapSection,
) -> Result<arcweft_bundle::ArcweftBundle, SourceMapBuildError> {
    use arcweft_bundle::{BundleManifest, BundleRuntimeSummary};
    use arcweft_core::bytecode::BytecodeProgram;
    use arcweft_render_text::LineDisplayCatalog;

    arcweft_bundle::ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: None,
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                entry_flow: None,
                flows: 0,
                bytecode_instructions: 0,
                line_task_groups: 0,
                stream_plans: 0,
                source_plans: 0,
            },
        },
        source_map,
        BytecodeProgram::default(),
        LineDisplayCatalog::default(),
    )
}
