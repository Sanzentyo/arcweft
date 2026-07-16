//! Linked standard authored View resources supplied to every product bundle.

use crate::resource_codec::view::{
    DialogueTextProjection, ViewActionButtonActionResource, ViewActionButtonResource,
    ViewDefinitionResource, ViewElementKind, ViewInstructionSpan, ViewParameterResource,
    ViewParameterRole, ViewProgramInstruction, ViewProgramResource, ViewRuntimeButtonBounds,
    ViewRuntimeSurfaceBounds, ViewStyleResource, ViewSurfaceResource, ViewTextBlockBounds,
    ViewTextBlockResource, ViewTextResource, ViewTextSourceKind, ViewTextSourceRecord,
    ViewTextSurface,
};
use crate::resource_codec::{SourceRangeRef, table::PublicIdRef};
use arcweft_presentation::appearance::PresentationColor;
use arcweft_view::style::{
    ViewColorValue, ViewLengthMilli, ViewPropertyKind, ViewSpecifiedValue,
    ViewStyleApplicationTarget, ViewStyleAssignOp, ViewStyleDeclaration, ViewStyleProgram,
    ViewStyleRule, ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheet, ViewStyleSheetId,
    ViewStyleSourceId,
};
use arcweft_view::{ViewPartLocalName, ViewPartName};

pub const DIALOGUE_VIEW_ID: &str = "std.view.dialogue";
pub const DIALOGUE_PARAMETER: &str = "dialogue";
pub const DIALOGUE_STYLE_ID: &str = "style.dialogue.standard";

const PANEL_PART: &str = "part.dialogue.panel";
const SPEAKER_PART: &str = "part.dialogue.speaker";
const CONTENT_PART: &str = "part.dialogue.content";
const ACTION_PART: &str = "part.dialogue.primary_action";
const SPEAKER_SOURCE: &str = "std.dialogue.text.speaker";
const CONTENT_SOURCE: &str = "std.dialogue.text.content";

fn local_part(value: &str) -> ViewPartLocalName {
    ViewPartLocalName::try_new(value).expect("standard View part identities are canonical")
}
const ACTION_LABEL_SOURCE: &str = "std.dialogue.text.primary_action";
const DIALOGUE_STYLE_SOURCE: &str = "standard dialogue style";

/// Minimal default dialogue View program linked through the normal View runtime.
#[must_use]
pub fn dialogue_program() -> ViewProgramResource {
    ViewProgramResource {
        program_id: "std.view.program".to_owned(),
        definitions: vec![ViewDefinitionResource {
            public_id: DIALOGUE_VIEW_ID.to_owned(),
            body: ViewInstructionSpan::new(0, 6),
            styles: vec![dialogue_style_ref()],
            parameters: vec![ViewParameterResource {
                ordinal: 0,
                name: DIALOGUE_PARAMETER.to_owned(),
                role: ViewParameterRole::Dialogue,
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
                styles: Vec::new(),
                part: Some(local_part(PANEL_PART)),
                key: Some(0),
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: SPEAKER_SOURCE.to_owned(),
                text_block: SPEAKER_PART.to_owned(),
                styles: Vec::new(),
                part: Some(local_part(SPEAKER_PART)),
                source: None,
            },
            ViewProgramInstruction::EmitText {
                text_source: CONTENT_SOURCE.to_owned(),
                text_block: CONTENT_PART.to_owned(),
                styles: Vec::new(),
                part: Some(local_part(CONTENT_PART)),
                source: None,
            },
            ViewProgramInstruction::OpenElement {
                element: ViewElementKind::Button,
                target: Some(ACTION_PART.to_owned()),
                styles: Vec::new(),
                part: Some(local_part(ACTION_PART)),
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
                ViewTextSurface::RichText,
            ),
            text_block_milli(
                SPEAKER_PART,
                SPEAKER_SOURCE,
                85_600,
                480_800,
                1_108_800,
                28_000,
                ViewTextSurface::Text,
            ),
        ],
        surfaces: vec![ViewSurfaceResource {
            public_id: PANEL_PART.to_owned(),
            view: Some(DIALOGUE_VIEW_ID.to_owned()),
            containing_scroll_region: None,
            element: ViewElementKind::Panel,
            bounds: ViewRuntimeSurfaceBounds::new(57_600, 460_800, 1_164_800, 201_600),
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
///
/// # Panics
///
/// Panics only if the checked-in standard Style identifiers, source range, or
/// typed declarations violate their compile-time invariants.
#[must_use]
pub fn dialogue_style() -> ViewStyleResource {
    let source = ViewStyleSourceId::new(0);
    let sheet = ViewStyleSheet::new(
        style_sheet_id(),
        Vec::new(),
        vec![
            rule(
                PANEL_PART,
                vec![declaration(
                    ViewPropertyKind::BackgroundColor,
                    rgba(17, 18, 16, 242),
                    source,
                )],
                0,
                source,
            ),
            rule(
                SPEAKER_PART,
                vec![
                    declaration(ViewPropertyKind::Color, rgba(174, 226, 142, 255), source),
                    declaration(ViewPropertyKind::FontSize, length(25_000), source),
                    declaration(ViewPropertyKind::LineHeight, length(34_000), source),
                ],
                1,
                source,
            ),
            rule(
                CONTENT_PART,
                vec![
                    declaration(ViewPropertyKind::Color, rgba(248, 246, 234, 255), source),
                    declaration(ViewPropertyKind::FontSize, length(25_000), source),
                    declaration(ViewPropertyKind::LineHeight, length(34_000), source),
                ],
                2,
                source,
            ),
            rule(
                ACTION_PART,
                vec![declaration(
                    ViewPropertyKind::BackgroundColor,
                    rgba(0, 0, 0, 0),
                    source,
                )],
                3,
                source,
            ),
        ],
    )
    .expect("standard dialogue Style sheet is statically valid");
    let mut resource = ViewStyleResource {
        style_program_id: "std.view.style.program".to_owned(),
        program: ViewStyleProgram::try_new(vec![sheet], Vec::new())
            .expect("standard dialogue Style program is statically valid"),
        source_map_refs: vec![SourceRangeRef {
            source: PublicIdRef::default(),
            start_byte: 0,
            end_byte: u32::try_from(DIALOGUE_STYLE_SOURCE.len())
                .expect("standard dialogue Style source length fits u32"),
        }],
        adapter_requirements: Vec::new(),
    };
    let source_ref = resource
        .public_id_table()
        .expect("standard dialogue Style public IDs are valid")
        .id_for(DIALOGUE_STYLE_ID)
        .expect("standard dialogue Style sheet is in its public-ID table");
    resource.source_map_refs[0].source = source_ref;
    resource
}

fn dialogue_style_ref() -> ViewStyleApplicationTarget {
    ViewStyleApplicationTarget::named(style_sheet_id())
}

fn style_sheet_id() -> ViewStyleSheetId {
    ViewStyleSheetId::try_new(DIALOGUE_STYLE_ID)
        .expect("standard dialogue Style ID is statically valid")
}

fn length(value: i32) -> ViewSpecifiedValue {
    ViewSpecifiedValue::Length {
        value: ViewLengthMilli::new(value),
    }
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
    surface: ViewTextSurface,
) -> ViewTextBlockResource {
    ViewTextBlockResource::new(
        public_id,
        Some(DIALOGUE_VIEW_ID.to_owned()),
        None,
        source,
        ViewTextBlockBounds::new(x_milli, y_milli, width_milli, height_milli),
    )
    .with_surface(surface)
}

fn rule(
    part: &str,
    declarations: Vec<ViewStyleDeclaration>,
    source_order: u32,
    source: ViewStyleSourceId,
) -> ViewStyleRule {
    let part = ViewPartName::try_new(part).expect("standard dialogue part ID is valid");
    let sequence = ViewStyleSelectorSequence::new(None, None, Some(part), Vec::new())
        .expect("standard dialogue selector sequence is non-empty");
    let selector = ViewStyleSelector::new(vec![sequence])
        .expect("standard dialogue selector has a valid relation shape");
    ViewStyleRule::new(selector, None, declarations, source_order, source)
        .expect("standard dialogue rule is statically valid")
}

fn declaration(
    property: ViewPropertyKind,
    value: ViewSpecifiedValue,
    source: ViewStyleSourceId,
) -> ViewStyleDeclaration {
    ViewStyleDeclaration::new(property, value, ViewStyleAssignOp::Replace, source)
        .expect("standard dialogue declaration matches its property kind")
}

const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> ViewSpecifiedValue {
    ViewSpecifiedValue::Color {
        value: ViewColorValue::Literal {
            color: PresentationColor::rgba(red, green, blue, alpha),
        },
    }
}
