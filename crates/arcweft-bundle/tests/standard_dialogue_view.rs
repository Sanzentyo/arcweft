use arcweft_bundle::resource_codec::view::{
    RgbaColor, ViewActionButtonActionResource, ViewDefinitionResource, ViewInstructionSpan,
    ViewProgramResource, ViewRuntimeButtonBounds, ViewRuntimeSurfaceBounds, ViewStyleValue,
    ViewTextBlockBounds, ViewTextSourceKind,
};
use arcweft_bundle::standard_view::{
    DIALOGUE_VIEW_ID, dialogue_program, dialogue_style, dialogue_text,
};

#[test]
fn standard_dialogue_view_is_a_complete_encodable_authored_resource() {
    let program = dialogue_program();
    let text = dialogue_text();
    let style = dialogue_style();

    assert_eq!(program.definitions[0].public_id, DIALOGUE_VIEW_ID);
    assert_eq!(program.definitions[0].parameters[0].name, "dialogue");
    assert!(matches!(
        &program.action_buttons[0].action,
        ViewActionButtonActionResource::DialoguePrimaryAction { parameter }
            if parameter == "dialogue"
    ));
    assert_eq!(program.surfaces.len(), 1);
    assert_eq!(program.text_blocks.len(), 2);
    assert_eq!(text.sources.len(), 3);
    assert_eq!(style.rules.len(), 4);
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
    assert!(style.rules.iter().any(|rule| {
        rule.declarations.iter().any(|declaration| {
            declaration.property == "background"
                && declaration.value
                    == ViewStyleValue::Rgba(RgbaColor {
                        red: 17,
                        green: 18,
                        blue: 16,
                        alpha: 242,
                    })
        })
    }));

    program
        .encode_canonical_section()
        .expect("standard View program encodes");
    text.encode_canonical_section()
        .expect("standard View text encodes");
    style
        .encode_canonical_section()
        .expect("standard View style encodes");
}

#[test]
fn authored_program_is_merged_without_replacing_the_reserved_standard_definition() {
    let authored = ViewProgramResource {
        program_id: "view.project".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: "view.CustomDialogue".to_owned(),
            body: ViewInstructionSpan::new(0, 0),
            parameters: Vec::new(),
            state_schema_hash: 7,
        }],
        ..ViewProgramResource::default()
    };
    let bundle = test_bundle().with_view_program(authored);
    let program = bundle.view_program.expect("bundle retains View program");

    assert_eq!(program.program_id, "view.project");
    assert!(
        program
            .definitions
            .iter()
            .any(|definition| definition.public_id == DIALOGUE_VIEW_ID)
    );
    assert!(
        program
            .definitions
            .iter()
            .any(|definition| definition.public_id == "view.CustomDialogue")
    );
}

#[test]
fn reserved_standard_dialogue_view_id_cannot_be_overridden() {
    let authored = ViewProgramResource {
        program_id: "view.project".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: DIALOGUE_VIEW_ID.to_owned(),
            body: ViewInstructionSpan::new(0, 0),
            parameters: Vec::new(),
            state_schema_hash: 99,
        }],
        ..ViewProgramResource::default()
    };
    let program = test_bundle()
        .with_view_program(authored)
        .view_program
        .expect("bundle retains merged View program");

    assert!(program.encode_canonical_section().is_err());
}

#[test]
fn dialogue_primary_action_requires_a_declared_typed_parameter() {
    let mut program = dialogue_program();
    program.definitions[0].parameters.clear();

    assert!(program.encode_canonical_section().is_err());
}

fn test_bundle() -> arcweft_bundle::ArcweftBundle {
    use arcweft_bundle::{BundleManifest, BundleRuntimeSummary, BundleSource};
    use arcweft_core::bytecode::BytecodeProgram;
    use arcweft_render_text::LineDisplayCatalog;

    arcweft_bundle::ArcweftBundle::new(
        BundleManifest {
            source_label: "standard-dialogue-view-test".to_owned(),
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
        BundleSource {
            label: "test.arcw".to_owned(),
            text: String::new(),
        },
        BytecodeProgram::default(),
        LineDisplayCatalog::default(),
    )
}
