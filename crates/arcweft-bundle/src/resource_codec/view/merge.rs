//! Atomic composition of executable View programs and their Style catalogs.

use super::ViewStyleContractError;
use super::model::{
    ViewProgramInstruction, ViewProgramResource, ViewStyleApplicationTarget, ViewStyleResource,
    ViewValueInputNamespace,
};
use crate::resource_codec::{PublicIdTable, SectionCodecError, SourceRangeRef};
use arcweft_presentation::fx::{FxRuntimeType, ValueInstruction, ValueProgramSchema};
use arcweft_view::{
    ViewValueProgram, ViewValueProgramId,
    style::{
        ViewStyleDeclaration, ViewStyleModelError, ViewStylePatch, ViewStylePatchId,
        ViewStyleProgram, ViewStyleRule, ViewStyleSheet, ViewStyleSourceId, ViewStyleToken,
    },
};
use std::collections::BTreeMap;
use thiserror::Error;

/// Program and Style sections that must be rebased as one executable unit.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewProgramStyleResources {
    pub program: Option<ViewProgramResource>,
    pub style: Option<ViewStyleResource>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewResourceMergeError {
    #[error("invalid View resource section during merge: {0}")]
    Section(#[from] SectionCodecError),
    #[error("invalid canonical View Style model during merge: {0}")]
    StyleModel(#[from] ViewStyleModelError),
    #[error("invalid merged View Style contract: {0}")]
    StyleContract(#[from] ViewStyleContractError),
    #[error("View resource merge overflowed {0}")]
    Overflow(&'static str),
    #[error(
        "right View program references inline patch {0:?} that its Style resource does not own"
    )]
    UnownedInlinePatch(ViewStylePatchId),
    #[error("View value-program merge failed: {0}")]
    ValueProgram(String),
}

impl ViewProgramStyleResources {
    pub const fn new(
        program: Option<ViewProgramResource>,
        style: Option<ViewStyleResource>,
    ) -> Self {
        Self { program, style }
    }

    /// Merges an independently compiled resource set, rebasing every coupled
    /// program, patch, source-range, and public-table reference atomically.
    pub fn merge(self, other: Self) -> Result<Self, ViewResourceMergeError> {
        let (style, patch_rebase) = merge_styles(self.style, other.style)?;
        let program = merge_programs(self.program, other.program, &patch_rebase)?;
        let merged = Self { program, style };
        merged.validate()?;
        Ok(merged)
    }

    pub fn validate(&self) -> Result<(), ViewResourceMergeError> {
        match (&self.program, &self.style) {
            (Some(program), style) => {
                program.encode_canonical_section()?;
                program.validate_style_contract(style.as_ref())?;
            }
            (None, Some(style)) => {
                style.encode_canonical_section()?;
            }
            (None, None) => {}
        }
        Ok(())
    }
}

fn merge_styles(
    left: Option<ViewStyleResource>,
    right: Option<ViewStyleResource>,
) -> Result<
    (
        Option<ViewStyleResource>,
        BTreeMap<ViewStylePatchId, ViewStylePatchId>,
    ),
    ViewResourceMergeError,
> {
    match (left, right) {
        (None, None) => Ok((None, BTreeMap::new())),
        (Some(mut style), None) => {
            style.canonicalize()?;
            Ok((Some(style), BTreeMap::new()))
        }
        (None, Some(mut style)) => {
            style.canonicalize()?;
            let patch_rebase = style
                .program
                .patches()
                .iter()
                .map(|patch| (patch.id(), patch.id()))
                .collect();
            Ok((Some(style), patch_rebase))
        }
        (Some(mut left), Some(mut right)) => {
            left.canonicalize()?;
            right.canonicalize()?;
            left.encode_canonical_section()?;
            right.encode_canonical_section()?;
            let left_table = left.public_id_table()?;
            let right_table = right.public_id_table()?;
            let source_offset = u32::try_from(left.source_map_refs.len())
                .map_err(|_| ViewResourceMergeError::Overflow("Style source IDs"))?;
            let patch_offset = left
                .program
                .patches()
                .iter()
                .map(|patch| patch.id().value())
                .max()
                .map_or(Ok(0), |id| {
                    id.checked_add(1)
                        .ok_or(ViewResourceMergeError::Overflow("Style patch IDs"))
                })?;
            let mut patch_rebase = BTreeMap::new();
            let mut sheets = left.program.sheets().to_vec();
            sheets.extend(
                right
                    .program
                    .sheets()
                    .iter()
                    .map(|sheet| rebase_sheet(sheet, source_offset))
                    .collect::<Result<Vec<_>, _>>()?,
            );
            let mut patches = left.program.patches().to_vec();
            patches.extend(
                right
                    .program
                    .patches()
                    .iter()
                    .map(|patch| {
                        rebase_patch(patch, source_offset, patch_offset, &mut patch_rebase)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            let left_range_count = left.source_map_refs.len();
            left.program = ViewStyleProgram::try_new(sheets, patches)?;
            left.source_map_refs.append(&mut right.source_map_refs);
            left.adapter_requirements.extend(right.adapter_requirements);
            let merged_table = left.public_id_table()?;
            let no_identity_rebase = BTreeMap::new();
            remap_range_public_ids(
                &mut left.source_map_refs[..left_range_count],
                &left_table,
                &merged_table,
                &no_identity_rebase,
            )?;
            let right_program_rebase =
                BTreeMap::from([(right.style_program_id, left.style_program_id.clone())]);
            remap_range_public_ids(
                &mut left.source_map_refs[left_range_count..],
                &right_table,
                &merged_table,
                &right_program_rebase,
            )?;
            left.canonicalize()?;
            left.encode_canonical_section()?;
            Ok((Some(left), patch_rebase))
        }
    }
}

fn rebase_sheet(
    sheet: &ViewStyleSheet,
    source_offset: u32,
) -> Result<ViewStyleSheet, ViewResourceMergeError> {
    let tokens = sheet
        .tokens()
        .iter()
        .map(|token| {
            ViewStyleToken::new(
                token.id().clone(),
                token.value_kind(),
                token.value().clone(),
                rebase_source(token.source(), source_offset)?,
            )
            .map_err(ViewResourceMergeError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let rules = sheet
        .rules()
        .iter()
        .map(|rule| {
            ViewStyleRule::new(
                rule.selector().clone(),
                rule.environment()
                    .map(|condition| {
                        condition.try_map_sources(|source| rebase_source(source, source_offset))
                    })
                    .transpose()?,
                rebase_declarations(rule.declarations(), source_offset)?,
                rule.source_order(),
                rebase_source(rule.source(), source_offset)?,
            )
            .map_err(ViewResourceMergeError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    ViewStyleSheet::new(sheet.id().clone(), tokens, rules).map_err(ViewResourceMergeError::from)
}

fn rebase_patch(
    patch: &ViewStylePatch,
    source_offset: u32,
    patch_offset: u32,
    patch_rebase: &mut BTreeMap<ViewStylePatchId, ViewStylePatchId>,
) -> Result<ViewStylePatch, ViewResourceMergeError> {
    let old = patch.id();
    let id = ViewStylePatchId::new(
        old.value()
            .checked_add(patch_offset)
            .ok_or(ViewResourceMergeError::Overflow("Style patch IDs"))?,
    );
    if patch_rebase.insert(old, id).is_some() {
        return Err(ViewResourceMergeError::UnownedInlinePatch(old));
    }
    Ok(ViewStylePatch::new(
        id,
        rebase_declarations(patch.declarations(), source_offset)?,
    ))
}

fn rebase_declarations(
    declarations: &[ViewStyleDeclaration],
    source_offset: u32,
) -> Result<Vec<ViewStyleDeclaration>, ViewResourceMergeError> {
    declarations
        .iter()
        .map(|declaration| {
            ViewStyleDeclaration::new(
                declaration.property(),
                declaration.value().clone(),
                declaration.op(),
                rebase_source(declaration.source(), source_offset)?,
            )
            .map_err(ViewResourceMergeError::from)
        })
        .collect()
}

fn rebase_source(
    source: ViewStyleSourceId,
    offset: u32,
) -> Result<ViewStyleSourceId, ViewResourceMergeError> {
    source
        .value()
        .checked_add(offset)
        .map(ViewStyleSourceId::new)
        .ok_or(ViewResourceMergeError::Overflow("Style source IDs"))
}

fn remap_range_public_ids(
    ranges: &mut [SourceRangeRef],
    old: &PublicIdTable,
    merged: &PublicIdTable,
    identity_rebase: &BTreeMap<String, String>,
) -> Result<(), ViewResourceMergeError> {
    for range in ranges {
        remap_source_ref_with_identity_rebase(range, old, merged, identity_rebase)?;
    }
    Ok(())
}

fn remap_source_ref(
    source: &mut SourceRangeRef,
    old: &PublicIdTable,
    merged: &PublicIdTable,
) -> Result<(), ViewResourceMergeError> {
    remap_source_ref_with_identity_rebase(source, old, merged, &BTreeMap::new())
}

fn remap_source_ref_with_identity_rebase(
    source: &mut SourceRangeRef,
    old: &PublicIdTable,
    merged: &PublicIdTable,
    identity_rebase: &BTreeMap<String, String>,
) -> Result<(), ViewResourceMergeError> {
    let public_id = old.get(source.source)?;
    let public_id = identity_rebase
        .get(public_id)
        .map_or(public_id, String::as_str);
    source.source = merged
        .id_for(public_id)
        .ok_or(SectionCodecError::NonCanonicalTable(
            "view_resource_source_public_ids",
        ))?;
    Ok(())
}

fn merge_programs(
    left: Option<ViewProgramResource>,
    right: Option<ViewProgramResource>,
    patch_rebase: &BTreeMap<ViewStylePatchId, ViewStylePatchId>,
) -> Result<Option<ViewProgramResource>, ViewResourceMergeError> {
    match (left, right) {
        (None, None) => Ok(None),
        (Some(program), None) => Ok(Some(program)),
        (None, Some(mut program)) => {
            for definition in &mut program.definitions {
                rebase_style_list(&mut definition.styles, patch_rebase)?;
            }
            rebase_style_references(&mut program.instructions, patch_rebase)?;
            Ok(Some(program))
        }
        (Some(mut left), Some(mut right)) => {
            left.encode_canonical_section()?;
            right.encode_canonical_section()?;
            let left_table = left.public_id_table()?;
            let right_table = right.public_id_table()?;
            let source_splits = ProgramSourceSplits::capture(&left);
            for definition in &mut right.definitions {
                rebase_style_list(&mut definition.styles, patch_rebase)?;
            }
            rebase_style_references(&mut right.instructions, patch_rebase)?;
            merge_value_inventories(&mut left, &mut right)?;
            let instruction_offset = u32::try_from(left.instructions.len())
                .map_err(|_| ViewResourceMergeError::Overflow("View instruction spans"))?;
            for definition in &mut right.definitions {
                definition.body.start_instruction = definition
                    .body
                    .start_instruction
                    .checked_add(instruction_offset)
                    .ok_or(ViewResourceMergeError::Overflow(
                        "View definition span start",
                    ))?;
                definition.body.end_instruction = definition
                    .body
                    .end_instruction
                    .checked_add(instruction_offset)
                    .ok_or(ViewResourceMergeError::Overflow("View definition span end"))?;
            }
            left.instructions.extend(right.instructions);
            left.definitions.extend(right.definitions);
            left.handlers.extend(right.handlers);
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
            let merged_table = left.public_id_table()?;
            remap_program_source_refs(
                &mut left,
                source_splits,
                &left_table,
                &right_table,
                &merged_table,
            )?;
            left.encode_canonical_section()?;
            Ok(Some(left))
        }
    }
}

#[derive(Clone, Copy)]
struct ProgramSourceSplits {
    instructions: usize,
    exported_parts: usize,
    semantic_targets: usize,
    layout_bounds: usize,
    scroll_regions: usize,
    surfaces: usize,
    text_blocks: usize,
    action_buttons: usize,
    focus_groups: usize,
    focus_navigation: usize,
}

impl ProgramSourceSplits {
    fn capture(program: &ViewProgramResource) -> Self {
        Self {
            instructions: program.instructions.len(),
            exported_parts: program.exported_parts.len(),
            semantic_targets: program.semantic_targets.len(),
            layout_bounds: program.layout_bounds.len(),
            scroll_regions: program.scroll_regions.len(),
            surfaces: program.surfaces.len(),
            text_blocks: program.text_blocks.len(),
            action_buttons: program.action_buttons.len(),
            focus_groups: program.focus_groups.len(),
            focus_navigation: program.focus_navigation.len(),
        }
    }
}

fn remap_program_source_refs(
    program: &mut ViewProgramResource,
    splits: ProgramSourceSplits,
    left: &PublicIdTable,
    right: &PublicIdTable,
    merged: &PublicIdTable,
) -> Result<(), ViewResourceMergeError> {
    remap_instruction_sources(
        &mut program.instructions[..splits.instructions],
        left,
        merged,
    )?;
    let (left_exports, right_exports) = program.exported_parts.split_at_mut(splits.exported_parts);
    remap_export_sources(left_exports, left, merged)?;
    remap_export_sources(right_exports, right, merged)?;
    remap_instruction_sources(
        &mut program.instructions[splits.instructions..],
        right,
        merged,
    )?;
    remap_partitioned_sources(
        &mut program.semantic_targets,
        splits.semantic_targets,
        |item| &mut item.source,
        left,
        right,
        merged,
    )?;
    remap_partitioned_sources(
        &mut program.layout_bounds,
        splits.layout_bounds,
        |item| &mut item.source,
        left,
        right,
        merged,
    )?;
    remap_partitioned_sources(
        &mut program.scroll_regions,
        splits.scroll_regions,
        |item| &mut item.source,
        left,
        right,
        merged,
    )?;
    remap_partitioned_sources(
        &mut program.surfaces,
        splits.surfaces,
        |item| &mut item.source,
        left,
        right,
        merged,
    )?;
    remap_partitioned_sources(
        &mut program.text_blocks,
        splits.text_blocks,
        |item| &mut item.source,
        left,
        right,
        merged,
    )?;
    remap_partitioned_sources(
        &mut program.action_buttons,
        splits.action_buttons,
        |item| &mut item.source,
        left,
        right,
        merged,
    )?;
    remap_partitioned_sources(
        &mut program.focus_groups,
        splits.focus_groups,
        |item| &mut item.source,
        left,
        right,
        merged,
    )?;
    let (left_navigation, right_navigation) = program
        .focus_navigation
        .split_at_mut(splits.focus_navigation);
    remap_focus_navigation_sources(left_navigation, left, merged)?;
    remap_focus_navigation_sources(right_navigation, right, merged)
}

fn remap_export_sources(
    exports: &mut [super::model::ViewExportedPart],
    old: &PublicIdTable,
    merged: &PublicIdTable,
) -> Result<(), ViewResourceMergeError> {
    for source in exports
        .iter_mut()
        .flat_map(|export| export.source.ranges_mut())
    {
        remap_source_ref(source, old, merged)?;
    }
    Ok(())
}

fn remap_partitioned_sources<T>(
    items: &mut [T],
    split: usize,
    source: impl for<'a> Fn(&'a mut T) -> &'a mut Option<SourceRangeRef> + Copy,
    left: &PublicIdTable,
    right: &PublicIdTable,
    merged: &PublicIdTable,
) -> Result<(), ViewResourceMergeError> {
    let (left_items, right_items) = items.split_at_mut(split);
    remap_optional_sources(left_items.iter_mut().map(source), left, merged)?;
    remap_optional_sources(right_items.iter_mut().map(source), right, merged)
}

fn remap_instruction_sources(
    instructions: &mut [ViewProgramInstruction],
    old: &PublicIdTable,
    merged: &PublicIdTable,
) -> Result<(), ViewResourceMergeError> {
    for source in instructions
        .iter_mut()
        .filter_map(ViewProgramInstruction::source_mut)
    {
        remap_source_ref(source, old, merged)?;
    }
    Ok(())
}

fn remap_optional_sources<'a>(
    sources: impl Iterator<Item = &'a mut Option<SourceRangeRef>>,
    old: &PublicIdTable,
    merged: &PublicIdTable,
) -> Result<(), ViewResourceMergeError> {
    for source in sources.flatten() {
        remap_source_ref(source, old, merged)?;
    }
    Ok(())
}

fn remap_focus_navigation_sources(
    navigation: &mut [super::model::ViewFocusNavigationResource],
    old: &PublicIdTable,
    merged: &PublicIdTable,
) -> Result<(), ViewResourceMergeError> {
    for item in navigation {
        if let Some(source) = &mut item.source {
            remap_source_ref(source, old, merged)?;
        }
        remap_optional_sources(
            item.edges.iter_mut().map(|edge| &mut edge.source),
            old,
            merged,
        )?;
    }
    Ok(())
}

fn rebase_style_references(
    instructions: &mut [ViewProgramInstruction],
    patch_rebase: &BTreeMap<ViewStylePatchId, ViewStylePatchId>,
) -> Result<(), ViewResourceMergeError> {
    for instruction in instructions {
        let Some(styles) = instruction.styles_mut() else {
            continue;
        };
        rebase_style_list(styles, patch_rebase)?;
    }
    Ok(())
}

fn rebase_style_list(
    styles: &mut [ViewStyleApplicationTarget],
    patch_rebase: &BTreeMap<ViewStylePatchId, ViewStylePatchId>,
) -> Result<(), ViewResourceMergeError> {
    for reference in styles {
        let ViewStyleApplicationTarget::Inline { patch } = reference else {
            continue;
        };
        *patch = patch_rebase
            .get(patch)
            .copied()
            .ok_or(ViewResourceMergeError::UnownedInlinePatch(*patch))?;
    }
    Ok(())
}

fn merge_value_inventories(
    left: &mut ViewProgramResource,
    right: &mut ViewProgramResource,
) -> Result<(), ViewResourceMergeError> {
    let (left_parameters, left_state) = value_schema(&left.value_programs)?;
    let (right_parameters, right_state) = value_schema(&right.value_programs)?;
    let parameter_offset = u16::try_from(left_parameters.len())
        .map_err(|_| ViewResourceMergeError::Overflow("View parameter schema"))?;
    let state_offset = u16::try_from(left_state.len())
        .map_err(|_| ViewResourceMergeError::Overflow("View state schema"))?;
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
                .ok_or(ViewResourceMergeError::Overflow("View value-program IDs"))
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
                    .ok_or(ViewResourceMergeError::Overflow("View value-program IDs"))?,
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
            .ok_or(ViewResourceMergeError::Overflow("View value-input slots"))?;
    }
    for instruction in &mut right.instructions {
        remap_program_references(instruction, program_offset)?;
    }
    for definition in &mut right.definitions {
        for parameter in &mut definition.parameters {
            if let Some(value_slot) = &mut parameter.value_slot {
                *value_slot = value_slot.checked_add(parameter_offset).ok_or(
                    ViewResourceMergeError::Overflow("View definition parameter slots"),
                )?;
            }
            if let Some(default_program) = &mut parameter.default_program {
                remap_program(default_program, program_offset)?;
            }
        }
    }
    left.value_programs.append(&mut right.value_programs);
    left.value_inputs.append(&mut right.value_inputs);
    Ok(())
}

fn value_schema(
    programs: &[ViewValueProgram],
) -> Result<(Vec<FxRuntimeType>, Vec<FxRuntimeType>), ViewResourceMergeError> {
    let Some(first) = programs.first() else {
        return Ok((Vec::new(), Vec::new()));
    };
    let parameters = first.program().schema().parameter_types().to_vec();
    let state = first.program().schema().state_types().to_vec();
    if programs.iter().any(|program| {
        program.program().schema().parameter_types() != parameters
            || program.program().schema().state_types() != state
    }) {
        return Err(ViewResourceMergeError::ValueProgram(
            "one View resource contains inconsistent value-program schemas".to_owned(),
        ));
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
) -> Result<ViewValueProgram, ViewResourceMergeError> {
    let instructions = program
        .program()
        .instructions()
        .iter()
        .cloned()
        .map(|instruction| match instruction {
            ValueInstruction::LoadParameter { slot, ty } => Ok(ValueInstruction::LoadParameter {
                slot: slot
                    .checked_add(parameter_offset)
                    .ok_or(ViewResourceMergeError::Overflow("View parameter slots"))?,
                ty,
            }),
            ValueInstruction::LoadState { slot, ty } => Ok(ValueInstruction::LoadState {
                slot: slot
                    .checked_add(state_offset)
                    .ok_or(ViewResourceMergeError::Overflow("View state slots"))?,
                ty,
            }),
            instruction => Ok(instruction),
        })
        .collect::<Result<Vec<_>, ViewResourceMergeError>>()?;
    ViewValueProgram::validate(
        id,
        ValueProgramSchema::new(parameters.to_vec(), state.to_vec(), program.return_type()),
        instructions,
    )
    .map_err(|error| ViewResourceMergeError::ValueProgram(error.to_string()))
}

fn remap_program_references(
    instruction: &mut ViewProgramInstruction,
    offset: u32,
) -> Result<(), ViewResourceMergeError> {
    match instruction {
        ViewProgramInstruction::CallView { arguments, .. } => {
            for argument in arguments {
                remap_program(&mut argument.value_program, offset)?;
            }
        }
        ViewProgramInstruction::Branch {
            condition_program, ..
        } => remap_program(condition_program, offset)?,
        ViewProgramInstruction::RepeatKeyed {
            source_program,
            key_program,
            ..
        } => {
            remap_program(source_program, offset)?;
            remap_program(key_program, offset)?;
        }
        ViewProgramInstruction::Await { source_program, .. } => {
            remap_program(source_program, offset)?;
        }
        ViewProgramInstruction::BindLocal { value_program, .. } => {
            remap_program(value_program, offset)?;
        }
        ViewProgramInstruction::ApplyFx {
            arguments,
            key_program,
            ..
        } => {
            for argument in arguments {
                remap_program(&mut argument.value_program, offset)?;
            }
            if let Some(key_program) = key_program {
                remap_program(key_program, offset)?;
            }
        }
        ViewProgramInstruction::OpenElement { .. }
        | ViewProgramInstruction::CloseElement
        | ViewProgramInstruction::EmitText { .. }
        | ViewProgramInstruction::EmitImage { .. }
        | ViewProgramInstruction::EmitCustom { .. }
        | ViewProgramInstruction::BindHandler { .. }
        | ViewProgramInstruction::AttachSemantic { .. } => {}
    }
    Ok(())
}

fn remap_program(id: &mut ViewValueProgramId, offset: u32) -> Result<(), ViewResourceMergeError> {
    id.0 =
        id.0.checked_add(offset)
            .ok_or(ViewResourceMergeError::Overflow(
                "View value-program references",
            ))?;
    Ok(())
}
