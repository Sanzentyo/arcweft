//! Linked standard authored View resources supplied to every product bundle.

use crate::resource_codec::view::{
    DialogueTextProjection, RgbaColor, StyleAssignOp, ViewActionButtonActionResource,
    ViewActionButtonResource, ViewDefinitionResource, ViewElementKind, ViewInstructionSpan,
    ViewParameterResource, ViewProgramInstruction, ViewProgramResource, ViewRuntimeButtonBounds,
    ViewRuntimeSurfaceBounds, ViewStyleDeclaration, ViewStyleResource, ViewStyleRule,
    ViewStyleSelector, ViewStyleSelectorPart, ViewStyleValue, ViewSurfaceResource,
    ViewTextBlockBounds, ViewTextBlockResource, ViewTextResource, ViewTextSourceKind,
    ViewTextSourceRecord,
};

pub const DIALOGUE_VIEW_ID: &str = "std.view.dialogue";
pub const DIALOGUE_PARAMETER: &str = "dialogue";

const PANEL_PART: &str = "std.dialogue.panel";
const SPEAKER_PART: &str = "std.dialogue.speaker";
const CONTENT_PART: &str = "std.dialogue.content";
const ACTION_PART: &str = "std.dialogue.primary_action";
const SPEAKER_SOURCE: &str = "std.dialogue.text.speaker";
const CONTENT_SOURCE: &str = "std.dialogue.text.content";
const ACTION_LABEL_SOURCE: &str = "std.dialogue.text.primary_action";

/// Minimal default dialogue View program linked through the normal View runtime.
#[must_use]
pub fn dialogue_program() -> ViewProgramResource {
    ViewProgramResource {
        program_id: "std.view.program".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: DIALOGUE_VIEW_ID.to_owned(),
            body: ViewInstructionSpan::new(0, 6),
            parameters: vec![ViewParameterResource {
                ordinal: 0,
                name: DIALOGUE_PARAMETER.to_owned(),
                value_type: None,
                value_slot: None,
                default_program: None,
            }],
            state_schema_hash: 0x5354_4444_4941_4c47,
        }],
        instructions: vec![
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Panel,
                target: Some(PANEL_PART.to_owned()),
                style: Some(PANEL_PART.to_owned()),
                part: Some(PANEL_PART.to_owned()),
                key: Some(0),
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: SPEAKER_SOURCE.to_owned(),
                style: Some(SPEAKER_PART.to_owned()),
                part: Some(SPEAKER_PART.to_owned()),
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: CONTENT_SOURCE.to_owned(),
                style: Some(CONTENT_PART.to_owned()),
                part: Some(CONTENT_PART.to_owned()),
                source: None,
            },
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Button,
                target: Some(ACTION_PART.to_owned()),
                style: Some(ACTION_PART.to_owned()),
                part: Some(ACTION_PART.to_owned()),
                key: Some(1),
                source: None,
            },
            ViewProgramInstruction::CloseElement,
            ViewProgramInstruction::CloseElement,
        ],
        text_blocks: vec![
            text_block_milli(
                CONTENT_PART,
                CONTENT_SOURCE,
                85_600,
                518_800,
                1_108_800,
                125_600,
            ),
            text_block_milli(
                SPEAKER_PART,
                SPEAKER_SOURCE,
                85_600,
                480_800,
                1_108_800,
                28_000,
            ),
        ],
        surfaces: vec![ViewSurfaceResource {
            public_id: PANEL_PART.to_owned(),
            view: Some(DIALOGUE_VIEW_ID.to_owned()),
            containing_scroll_region: None,
            element: ViewElementKind::Panel,
            bounds: ViewRuntimeSurfaceBounds::new(57_600, 460_800, 1_164_800, 201_600),
            style: Some(PANEL_PART.to_owned()),
            source: None,
        }],
        action_buttons: vec![ViewActionButtonResource {
            public_id: ACTION_PART.to_owned(),
            view: Some(DIALOGUE_VIEW_ID.to_owned()),
            containing_scroll_region: None,
            label_text_source: ACTION_LABEL_SOURCE.to_owned(),
            enabled: true,
            action: ViewActionButtonActionResource::DialoguePrimaryAction {
                parameter: DIALOGUE_PARAMETER.to_owned(),
            },
            bounds: ViewRuntimeButtonBounds::new(57_600, 460_800, 1_164_800, 201_600),
            style: Some(ACTION_PART.to_owned()),
            source: None,
        }],
        ..ViewProgramResource::default()
    }
}

/// Typed speaker/content sources and the default primary-action label.
#[must_use]
pub fn dialogue_text() -> ViewTextResource {
    ViewTextResource {
        sources: vec![
            ViewTextSourceRecord {
                public_id: CONTENT_SOURCE.to_owned(),
                kind: ViewTextSourceKind::Dialogue {
                    parameter: DIALOGUE_PARAMETER.to_owned(),
                    projection: DialogueTextProjection::Content,
                },
                source: None,
            },
            ViewTextSourceRecord {
                public_id: ACTION_LABEL_SOURCE.to_owned(),
                kind: ViewTextSourceKind::Literal {
                    value: String::new(),
                },
                source: None,
            },
            ViewTextSourceRecord {
                public_id: SPEAKER_SOURCE.to_owned(),
                kind: ViewTextSourceKind::Dialogue {
                    parameter: DIALOGUE_PARAMETER.to_owned(),
                    projection: DialogueTextProjection::Speaker,
                },
                source: None,
            },
        ],
        ..ViewTextResource::default()
    }
}

/// Explicit, renderer-neutral visual defaults for the 1280x720 standard View.
#[must_use]
pub fn dialogue_style() -> ViewStyleResource {
    ViewStyleResource {
        style_program_id: "std.view.style".to_owned(),
        rules: vec![
            rule(
                PANEL_PART,
                vec![declaration("background", rgba(17, 18, 16, 242))],
            ),
            rule(
                SPEAKER_PART,
                vec![
                    declaration("color", rgba(174, 226, 142, 255)),
                    declaration("font-size", ViewStyleValue::Milli(25_000)),
                    declaration("line-height", ViewStyleValue::Milli(34_000)),
                ],
            ),
            rule(
                CONTENT_PART,
                vec![
                    declaration("color", rgba(248, 246, 234, 255)),
                    declaration("font-size", ViewStyleValue::Milli(25_000)),
                    declaration("line-height", ViewStyleValue::Milli(34_000)),
                ],
            ),
            rule(
                ACTION_PART,
                vec![declaration("background", rgba(0, 0, 0, 0))],
            ),
        ],
        ..ViewStyleResource::default()
    }
}

pub(crate) fn merge_program(
    mut standard: ViewProgramResource,
    mut authored: ViewProgramResource,
) -> ViewProgramResource {
    let offset = u32::try_from(standard.instructions.len())
        .expect("standard View instruction inventory fits the u32 bundle contract");
    for definition in &mut authored.definitions {
        definition.body.start_instruction = definition
            .body
            .start_instruction
            .checked_add(offset)
            .expect("merged View definition start fits the u32 bundle contract");
        definition.body.end_instruction = definition
            .body
            .end_instruction
            .checked_add(offset)
            .expect("merged View definition end fits the u32 bundle contract");
    }
    standard.program_id = authored.program_id;
    standard.definitions.extend(authored.definitions);
    standard.value_programs.extend(authored.value_programs);
    standard.value_inputs.extend(authored.value_inputs);
    standard.instructions.extend(authored.instructions);
    standard.handlers.extend(authored.handlers);
    standard.exported_parts.extend(authored.exported_parts);
    standard.semantic_targets.extend(authored.semantic_targets);
    standard.layout_bounds.extend(authored.layout_bounds);
    standard.action_buttons.extend(authored.action_buttons);
    standard.text_blocks.extend(authored.text_blocks);
    standard.surfaces.extend(authored.surfaces);
    standard.scroll_regions.extend(authored.scroll_regions);
    standard.focus_groups.extend(authored.focus_groups);
    standard.focus_navigation.extend(authored.focus_navigation);
    standard
        .adapter_requirements
        .extend(authored.adapter_requirements);
    standard
}

pub(crate) fn merge_style(
    mut standard: ViewStyleResource,
    authored: ViewStyleResource,
) -> ViewStyleResource {
    standard.style_program_id = authored.style_program_id;
    standard.arcweft_sources.extend(authored.arcweft_sources);
    standard.css_sources.extend(authored.css_sources);
    standard.tokens.extend(authored.tokens);
    standard.rules.extend(authored.rules);
    standard.part_rules.extend(authored.part_rules);
    standard
        .environment_predicates
        .extend(authored.environment_predicates);
    standard.source_map_refs.extend(authored.source_map_refs);
    standard
        .external_css_descriptors
        .extend(authored.external_css_descriptors);
    standard
        .adapter_requirements
        .extend(authored.adapter_requirements);
    standard
}

pub(crate) fn merge_text(
    mut standard: ViewTextResource,
    authored: ViewTextResource,
) -> ViewTextResource {
    standard.sources.extend(authored.sources);
    standard.localized.extend(authored.localized);
    standard
        .rich_text_documents
        .extend(authored.rich_text_documents);
    standard.display_frames.extend(authored.display_frames);
    standard.source_ranges.extend(authored.source_ranges);
    standard.reveal_policies.extend(authored.reveal_policies);
    standard.cursor_policies.extend(authored.cursor_policies);
    standard.redactions.extend(authored.redactions);
    standard
}

fn text_block_milli(
    public_id: &str,
    source: &str,
    x_milli: i32,
    y_milli: i32,
    width_milli: u32,
    height_milli: u32,
) -> ViewTextBlockResource {
    let mut block = ViewTextBlockResource::new(
        public_id,
        Some(DIALOGUE_VIEW_ID.to_owned()),
        None,
        source,
        ViewTextBlockBounds::new(x_milli, y_milli, width_milli, height_milli),
    );
    block.style = Some(public_id.to_owned());
    block
}

fn rule(part: &str, declarations: Vec<ViewStyleDeclaration>) -> ViewStyleRule {
    ViewStyleRule {
        selector: ViewStyleSelector {
            parts: vec![ViewStyleSelectorPart::Part(part.to_owned())],
        },
        declarations,
        source: None,
    }
}

fn declaration(property: &str, value: ViewStyleValue) -> ViewStyleDeclaration {
    ViewStyleDeclaration {
        property: property.to_owned(),
        value,
        op: StyleAssignOp::Replace,
    }
}

const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> ViewStyleValue {
    ViewStyleValue::Rgba(RgbaColor {
        red,
        green,
        blue,
        alpha,
    })
}
