//! Source-independent semantic transcript for accepted View programs.

use arcweft_view::{AcceptedViewProgramRevision, ViewPartName};
use serde::Serialize;

use crate::resource_codec::{CrossSectionRef, SectionCodecError};

use super::{
    ViewActionButtonResource, ViewDefinitionResource, ViewFocusGroupResource,
    ViewFocusNavigationResource, ViewHandlerRef, ViewLayoutBoundsResource, ViewOwnedPartRef,
    ViewProgramInstruction, ViewProgramResource, ViewScrollRegionResource, ViewSemanticTarget,
    ViewSurfaceResource, ViewTextBlockResource, ViewValueInputResource,
};

#[derive(Serialize)]
struct SemanticViewProgramTranscript<'a> {
    schema: &'static str,
    program_id: &'a arcweft_view::ViewProgramId,
    definitions: &'a [ViewDefinitionResource],
    value_programs: &'a [arcweft_view::ViewValueProgram],
    value_inputs: &'a [ViewValueInputResource],
    instructions: Vec<ViewProgramInstruction>,
    handlers: &'a [ViewHandlerRef],
    exported_parts: Vec<SemanticViewExportedPart>,
    semantic_targets: Vec<ViewSemanticTarget>,
    layout_bounds: Vec<ViewLayoutBoundsResource>,
    scroll_regions: Vec<ViewScrollRegionResource>,
    surfaces: Vec<ViewSurfaceResource>,
    text_blocks: Vec<ViewTextBlockResource>,
    action_buttons: Vec<ViewActionButtonResource>,
    focus_groups: Vec<ViewFocusGroupResource>,
    focus_navigation: Vec<ViewFocusNavigationResource>,
    adapter_requirements: &'a [CrossSectionRef],
}

#[derive(Serialize)]
struct SemanticViewExportedPart {
    target: ViewOwnedPartRef,
    public_name: ViewPartName,
}

pub(super) fn accepted_revision(
    resource: &ViewProgramResource,
) -> Result<AcceptedViewProgramRevision, SectionCodecError> {
    let transcript = SemanticViewProgramTranscript {
        schema: "arcweft.view.semantic-program.v1",
        program_id: &resource.program_id,
        definitions: &resource.definitions,
        value_programs: &resource.value_programs,
        value_inputs: &resource.value_inputs,
        instructions: resource
            .instructions
            .iter()
            .cloned()
            .map(without_instruction_source)
            .collect(),
        handlers: &resource.handlers,
        exported_parts: resource
            .exported_parts
            .iter()
            .map(|part| SemanticViewExportedPart {
                target: part.target.clone(),
                public_name: part.public_name.clone(),
            })
            .collect(),
        semantic_targets: without_source(&resource.semantic_targets, |item| {
            item.source = None;
        }),
        layout_bounds: without_source(&resource.layout_bounds, |item| item.source = None),
        scroll_regions: without_source(&resource.scroll_regions, |item| item.source = None),
        surfaces: without_source(&resource.surfaces, |item| item.source = None),
        text_blocks: without_source(&resource.text_blocks, |item| item.source = None),
        action_buttons: without_source(&resource.action_buttons, |item| item.source = None),
        focus_groups: without_source(&resource.focus_groups, |item| item.source = None),
        focus_navigation: without_source(&resource.focus_navigation, |item| {
            item.source = None;
            item.edges.iter_mut().for_each(|edge| edge.source = None);
        }),
        adapter_requirements: &resource.adapter_requirements,
    };
    let bytes = serde_json::to_vec(&transcript)
        .map_err(|_| SectionCodecError::NonCanonicalTable("view_semantic_transcript"))?;
    AcceptedViewProgramRevision::try_for_semantic_transcript(&bytes)
        .map_err(|_| SectionCodecError::NonCanonicalTable("view_semantic_transcript"))
}

fn without_instruction_source(mut instruction: ViewProgramInstruction) -> ViewProgramInstruction {
    match &mut instruction {
        ViewProgramInstruction::OpenElement { source, .. }
        | ViewProgramInstruction::EmitText { source, .. }
        | ViewProgramInstruction::EmitImage { source, .. }
        | ViewProgramInstruction::EmitCustom { source, .. }
        | ViewProgramInstruction::CallView { source, .. }
        | ViewProgramInstruction::Branch { source, .. }
        | ViewProgramInstruction::RepeatKeyed { source, .. }
        | ViewProgramInstruction::Await { source, .. }
        | ViewProgramInstruction::BindLocal { source, .. }
        | ViewProgramInstruction::ApplyFx { source, .. }
        | ViewProgramInstruction::BindHandler { source, .. }
        | ViewProgramInstruction::AttachSemantic { source, .. } => *source = None,
        ViewProgramInstruction::CloseElement => {}
    }
    instruction
}

fn without_source<T: Clone>(values: &[T], clear: impl Fn(&mut T)) -> Vec<T> {
    values
        .iter()
        .cloned()
        .map(|mut value| {
            clear(&mut value);
            value
        })
        .collect()
}
