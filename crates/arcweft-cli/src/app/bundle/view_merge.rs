//! Deterministic composition of independent executable View resources.

use arcweft_bundle::resource_codec::view::{
    ViewProgramInstruction, ViewProgramResource, ViewValueInputNamespace,
};
use arcweft_presentation::fx::{FxRuntimeType, ValueInstruction, ValueProgramSchema};
use arcweft_view::{ViewValueProgram, ViewValueProgramId};

pub(super) fn merge_view_programs(
    mut left: ViewProgramResource,
    mut right: ViewProgramResource,
) -> Result<ViewProgramResource, String> {
    merge_value_inventories(&mut left, &mut right)?;
    let instruction_offset = u32::try_from(left.instructions.len())
        .map_err(|_| "left instruction inventory exceeds u32::MAX".to_owned())?;
    let child_span_offset = u32::try_from(left.child_spans.len())
        .map_err(|_| "left child-span inventory exceeds u32::MAX".to_owned())?;
    for span in &mut right.child_spans {
        span.start_instruction = span
            .start_instruction
            .checked_add(instruction_offset)
            .ok_or_else(|| "View child-span start overflow".to_owned())?;
        span.end_instruction = span
            .end_instruction
            .checked_add(instruction_offset)
            .ok_or_else(|| "View child-span end overflow".to_owned())?;
    }
    for instruction in &mut right.instructions {
        if let ViewProgramInstruction::CallView { child_span, .. } = instruction {
            *child_span = child_span
                .checked_add(child_span_offset)
                .ok_or_else(|| "View call child-span index overflow".to_owned())?;
        }
    }
    left.instructions.extend(right.instructions);
    left.child_spans.extend(right.child_spans);
    left.handlers.extend(right.handlers);
    left.state_schema_hashes.extend(right.state_schema_hashes);
    left.exported_parts.extend(right.exported_parts);
    left.semantic_targets.extend(right.semantic_targets);
    left.layout_bounds.extend(right.layout_bounds);
    left.scroll_regions.extend(right.scroll_regions);
    left.surfaces.extend(right.surfaces);
    left.text_blocks.extend(right.text_blocks);
    left.action_buttons.extend(right.action_buttons);
    left.focus_groups.extend(right.focus_groups);
    left.focus_navigation.extend(right.focus_navigation);
    left.adapter_requirements.extend(right.adapter_requirements);
    Ok(left)
}

fn merge_value_inventories(
    left: &mut ViewProgramResource,
    right: &mut ViewProgramResource,
) -> Result<(), String> {
    let (left_parameters, left_state) = value_schema(&left.value_programs)?;
    let (right_parameters, right_state) = value_schema(&right.value_programs)?;
    let parameter_offset = u16::try_from(left_parameters.len())
        .map_err(|_| "left View parameter schema exceeds u16::MAX".to_owned())?;
    let state_offset = u16::try_from(left_state.len())
        .map_err(|_| "left View state schema exceeds u16::MAX".to_owned())?;
    let mut parameters = left_parameters;
    parameters.extend(right_parameters);
    let mut state = left_state;
    state.extend(right_state);

    let program_offset = left
        .value_programs
        .iter()
        .map(|program| program.id().0)
        .max()
        .map_or(Ok(0), |id| {
            id.checked_add(1)
                .ok_or_else(|| "View value-program ID overflow".to_owned())
        })?;

    left.value_programs = left
        .value_programs
        .iter()
        .map(|program| rebuild_program(program, program.id(), &parameters, &state, 0, 0))
        .collect::<Result<Vec<_>, _>>()?;
    right.value_programs = right
        .value_programs
        .iter()
        .map(|program| {
            let id = ViewValueProgramId(
                program
                    .id()
                    .0
                    .checked_add(program_offset)
                    .ok_or_else(|| "View value-program ID overflow".to_owned())?,
            );
            rebuild_program(
                program,
                id,
                &parameters,
                &state,
                parameter_offset,
                state_offset,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    for input in &mut right.value_inputs {
        input.slot = input
            .slot
            .checked_add(match input.namespace {
                ViewValueInputNamespace::Parameter => parameter_offset,
                ViewValueInputNamespace::State => state_offset,
            })
            .ok_or_else(|| "View value-input slot overflow".to_owned())?;
    }
    for instruction in &mut right.instructions {
        remap_program_references(instruction, program_offset)?;
    }
    left.value_programs.append(&mut right.value_programs);
    left.value_inputs.append(&mut right.value_inputs);
    Ok(())
}

fn value_schema(
    programs: &[ViewValueProgram],
) -> Result<(Vec<FxRuntimeType>, Vec<FxRuntimeType>), String> {
    let Some(first) = programs.first() else {
        return Ok((Vec::new(), Vec::new()));
    };
    let parameters = first.program().schema().parameter_types().to_vec();
    let state = first.program().schema().state_types().to_vec();
    if programs.iter().any(|program| {
        program.program().schema().parameter_types() != parameters
            || program.program().schema().state_types() != state
    }) {
        return Err("one View resource contains inconsistent value-program schemas".to_owned());
    }
    Ok((parameters, state))
}

fn rebuild_program(
    program: &ViewValueProgram,
    id: ViewValueProgramId,
    parameters: &[FxRuntimeType],
    state: &[FxRuntimeType],
    parameter_offset: u16,
    state_offset: u16,
) -> Result<ViewValueProgram, String> {
    let instructions = program
        .program()
        .instructions()
        .iter()
        .cloned()
        .map(|instruction| match instruction {
            ValueInstruction::LoadParameter { slot, ty } => Ok(ValueInstruction::LoadParameter {
                slot: slot
                    .checked_add(parameter_offset)
                    .ok_or_else(|| "View parameter slot overflow".to_owned())?,
                ty,
            }),
            ValueInstruction::LoadState { slot, ty } => Ok(ValueInstruction::LoadState {
                slot: slot
                    .checked_add(state_offset)
                    .ok_or_else(|| "View state slot overflow".to_owned())?,
                ty,
            }),
            instruction => Ok(instruction),
        })
        .collect::<Result<Vec<_>, String>>()?;
    ViewValueProgram::validate(
        id,
        ValueProgramSchema::new(parameters.to_vec(), state.to_vec(), program.return_type()),
        instructions,
    )
    .map_err(|error| error.to_string())
}

fn remap_program_references(
    instruction: &mut ViewProgramInstruction,
    offset: u32,
) -> Result<(), String> {
    let remap = |id: &mut ViewValueProgramId| -> Result<(), String> {
        id.0 =
            id.0.checked_add(offset)
                .ok_or_else(|| "View value-program reference overflow".to_owned())?;
        Ok(())
    };
    match instruction {
        ViewProgramInstruction::CallView { arguments, .. } => {
            for argument in arguments {
                remap(&mut argument.value_program)?;
            }
        }
        ViewProgramInstruction::Branch {
            condition_program, ..
        } => remap(condition_program)?,
        ViewProgramInstruction::RepeatKeyed {
            source_program,
            key_program,
            ..
        } => {
            remap(source_program)?;
            remap(key_program)?;
        }
        ViewProgramInstruction::Await { source_program, .. } => remap(source_program)?,
        ViewProgramInstruction::BindLocal { value_program, .. } => remap(value_program)?,
        ViewProgramInstruction::ApplyFx {
            arguments,
            key_program,
            ..
        } => {
            for argument in arguments {
                remap(&mut argument.value_program)?;
            }
            if let Some(key_program) = key_program {
                remap(key_program)?;
            }
        }
        ViewProgramInstruction::OpenElement { .. }
        | ViewProgramInstruction::CloseElement
        | ViewProgramInstruction::EmitText { .. }
        | ViewProgramInstruction::EmitImage { .. }
        | ViewProgramInstruction::EmitCustom { .. }
        | ViewProgramInstruction::ApplyStyle { .. }
        | ViewProgramInstruction::BindHandler { .. }
        | ViewProgramInstruction::AttachSemantic { .. } => {}
    }
    Ok(())
}
