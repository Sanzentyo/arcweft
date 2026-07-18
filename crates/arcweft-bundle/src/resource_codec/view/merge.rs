//! Atomic composition of executable View programs and their Style catalogs.

use super::model::{
    ViewProgramInstruction, ViewProgramResource, ViewStyleApplicationTarget, ViewStyleResource,
    ViewValueInputNamespace,
};
use super::{ViewResourceBudget, ViewStyleContractError};
use crate::resource_codec::budget::check_budget;
use crate::resource_codec::{SectionCodecError, ViewProductBuildError};
use arcweft_presentation::fx::{FxRuntimeType, ValueInstruction, ValueProgramSchema};
use arcweft_view::{
    ViewValueProgram, ViewValueProgramId,
    style::{
        ViewStyleDeclaration, ViewStyleModelError, ViewStylePatch, ViewStylePatchId,
        ViewStyleProgram, ViewStyleRule, ViewStyleSheet, ViewStyleSourceId, ViewStyleToken,
    },
};
use std::collections::{BTreeMap, BTreeSet};
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
    #[error("invalid product source reference during View resource merge: {0}")]
    ProductSource(#[from] ViewProductBuildError),
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
        self.merge_with_budget(other, ViewResourceBudget::default())
    }

    /// Merges with explicit candidate limits.
    ///
    /// Both inputs are validated and their combined inventories and dense-ID
    /// offsets are checked before an unpublished candidate is allocated. The
    /// candidate becomes observable only after a canonical encode/decode
    /// round trip succeeds under the same budget.
    pub fn merge_with_budget(
        self,
        other: Self,
        budget: ViewResourceBudget,
    ) -> Result<Self, ViewResourceMergeError> {
        self.validate_with_budget(&budget)?;
        other.validate_with_budget(&budget)?;
        MergePreflight::for_inputs(&self, &other)?.validate(&budget)?;
        let (style, patch_rebase) = merge_styles(self.style, other.style)?;
        let program = merge_programs(self.program, other.program, &patch_rebase)?;
        Self { program, style }.accept_candidate(&budget)
    }

    pub fn validate(&self) -> Result<(), ViewResourceMergeError> {
        self.validate_with_budget(&ViewResourceBudget::default())
    }

    fn validate_with_budget(
        &self,
        budget: &ViewResourceBudget,
    ) -> Result<(), ViewResourceMergeError> {
        if let Some(program) = &self.program {
            program.encode_canonical_section_with_budget(budget)?;
        }
        if let Some(style) = &self.style {
            style.encode_canonical_section_with_budget(budget)?;
        }
        if let Some(program) = &self.program {
            program.validate_style_references(self.style.as_ref())?;
        }
        Ok(())
    }

    fn accept_candidate(self, budget: &ViewResourceBudget) -> Result<Self, ViewResourceMergeError> {
        let style = self
            .style
            .map(|mut style| {
                style.canonicalize()?;
                let bytes = style.encode_canonical_section_with_budget(budget)?;
                let decoded =
                    ViewStyleResource::decode_canonical_section_with_budget(&bytes, *budget)?;
                if decoded != style {
                    return Err(ViewResourceMergeError::Section(
                        SectionCodecError::NonCanonicalTable("merged_view_style"),
                    ));
                }
                Ok(decoded)
            })
            .transpose()?;
        let program = self
            .program
            .map(|mut program| {
                program.canonicalize();
                let bytes = program.encode_canonical_section_with_budget(budget)?;
                let decoded =
                    ViewProgramResource::decode_canonical_section_with_budget(&bytes, *budget)?;
                if decoded != program {
                    return Err(ViewResourceMergeError::Section(
                        SectionCodecError::NonCanonicalTable("merged_view_program"),
                    ));
                }
                Ok(decoded)
            })
            .transpose()?;
        let candidate = Self { program, style };
        if let Some(program) = candidate.program.as_ref() {
            program.validate_style_references(candidate.style.as_ref())?;
        }
        Ok(candidate)
    }
}

#[derive(Debug, Default)]
struct MergePreflight {
    style: StyleMergeInventory,
    program: ProgramMergeInventory,
    offsets: MergeOffsetInventory,
}

impl MergePreflight {
    fn for_inputs(
        left: &ViewProgramStyleResources,
        right: &ViewProgramStyleResources,
    ) -> Result<Self, ViewResourceMergeError> {
        let mut style = StyleMergeInventory::default();
        left.style
            .iter()
            .chain(right.style.iter())
            .try_for_each(|resource| style.include(resource))?;

        let mut program = ProgramMergeInventory::default();
        left.program
            .iter()
            .chain(right.program.iter())
            .try_for_each(|resource| program.include(resource))?;

        Ok(Self {
            style,
            program,
            offsets: MergeOffsetInventory::for_inputs(left, right),
        })
    }

    fn validate(&self, budget: &ViewResourceBudget) -> Result<(), ViewResourceMergeError> {
        self.style.validate(budget)?;
        self.program.validate(budget)?;
        self.offsets.validate()
    }
}

#[derive(Debug, Default)]
struct StyleMergeInventory {
    source_refs: usize,
    source_ranges: usize,
    public_ids: BTreeSet<String>,
    sheets: usize,
    rules: usize,
    patches: usize,
    declarations: usize,
    conditions: usize,
    wrappers: usize,
    clauses: usize,
}

impl StyleMergeInventory {
    fn include(&mut self, resource: &ViewStyleResource) -> Result<(), ViewResourceMergeError> {
        checked_accumulate(
            &mut self.source_refs,
            resource.source_refs.len(),
            "Style source references",
        )?;
        checked_accumulate(
            &mut self.source_ranges,
            resource.source_map_refs.len(),
            "Style source ranges",
        )?;
        self.public_ids.extend(resource.public_ids());
        checked_accumulate(
            &mut self.sheets,
            resource.program.sheets().len(),
            "Style sheets",
        )?;
        checked_accumulate(
            &mut self.patches,
            resource.program.patches().len(),
            "Style patches",
        )?;
        for sheet in resource.program.sheets() {
            checked_accumulate(&mut self.rules, sheet.rules().len(), "Style rules")?;
            for rule in sheet.rules() {
                checked_accumulate(
                    &mut self.declarations,
                    rule.declarations().len(),
                    "Style declarations",
                )?;
                if let Some(condition) = rule.environment() {
                    checked_accumulate(&mut self.conditions, 1, "Style environment conditions")?;
                    checked_accumulate(
                        &mut self.wrappers,
                        condition.wrappers().len(),
                        "Style environment wrappers",
                    )?;
                    checked_accumulate(
                        &mut self.clauses,
                        condition.clauses().len(),
                        "Style environment clauses",
                    )?;
                }
            }
        }
        for patch in resource.program.patches() {
            checked_accumulate(
                &mut self.declarations,
                patch.declarations().len(),
                "Style declarations",
            )?;
        }
        Ok(())
    }

    fn validate(&self, budget: &ViewResourceBudget) -> Result<(), ViewResourceMergeError> {
        check_budget(
            self.source_refs,
            budget.common.public_ids,
            "view_style_source_refs",
        )?;
        check_budget(
            self.source_ranges,
            budget.source_map_refs,
            "view_style_source_map_refs",
        )?;
        check_budget(
            self.public_ids.len(),
            budget.common.public_ids,
            "view_style_public_ids",
        )?;
        check_budget(self.sheets, budget.style_sheets, "view_style_sheets")?;
        check_budget(self.rules, budget.style_rules, "view_style_rules")?;
        check_budget(
            self.patches,
            budget.style_patches,
            "view_style_inline_patches",
        )?;
        check_budget(
            self.declarations,
            budget.style_declarations,
            "view_style_declarations",
        )?;
        check_budget(
            self.conditions,
            budget.environment_conditions,
            "view_style_environment_conditions",
        )?;
        check_budget(
            self.wrappers,
            budget.environment_wrappers,
            "view_style_environment_wrappers",
        )?;
        check_budget(
            self.clauses,
            budget.environment_clauses,
            "view_style_environment_clauses",
        )?;
        ensure_u32_count(self.source_refs, "Style product-source table indices")?;
        ensure_u32_count(self.source_ranges, "Style source IDs")?;
        ensure_u32_count(self.public_ids.len(), "Style public-ID table indices")
    }
}

#[derive(Debug, Default)]
struct ProgramMergeInventory {
    source_refs: usize,
    source_ranges: usize,
    public_ids: BTreeSet<String>,
    instructions: usize,
}

impl ProgramMergeInventory {
    fn include(&mut self, resource: &ViewProgramResource) -> Result<(), ViewResourceMergeError> {
        checked_accumulate(
            &mut self.source_refs,
            resource.source_refs.len(),
            "View program source references",
        )?;
        checked_accumulate(
            &mut self.source_ranges,
            resource.source_ranges().count(),
            "View program source ranges",
        )?;
        checked_accumulate(
            &mut self.instructions,
            resource.instructions.len(),
            "View instructions",
        )?;
        self.public_ids.extend(resource.public_ids());
        Ok(())
    }

    fn validate(&self, budget: &ViewResourceBudget) -> Result<(), ViewResourceMergeError> {
        check_budget(
            self.source_refs,
            budget.common.public_ids,
            "view_program_source_refs",
        )?;
        check_budget(
            self.source_ranges,
            budget.source_map_refs,
            "view_program_source_ranges",
        )?;
        check_budget(
            self.public_ids.len(),
            budget.common.public_ids,
            "view_program_public_ids",
        )?;
        check_budget(
            self.instructions,
            budget.program_instructions,
            "view_program_instructions",
        )?;
        ensure_u32_count(
            self.source_refs,
            "View program product-source table indices",
        )?;
        ensure_u32_count(self.instructions, "View instruction IDs")?;
        ensure_u32_count(
            self.public_ids.len(),
            "View program public-ID table indices",
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct MergeOffsetInventory {
    left_style_source_count: u64,
    right_style_source_max: Option<u32>,
    left_style_product_source_count: u64,
    right_style_product_source_max: Option<u32>,
    left_patch_max: Option<u32>,
    right_patch_max: Option<u32>,
    left_program_product_source_count: u64,
    right_program_product_source_max: Option<u32>,
    left_instruction_count: u64,
    right_definition_end_max: Option<u32>,
}

impl MergeOffsetInventory {
    fn for_inputs(left: &ViewProgramStyleResources, right: &ViewProgramStyleResources) -> Self {
        Self {
            left_style_source_count: left
                .style
                .as_ref()
                .map_or(0, |style| style.source_map_refs.len() as u64),
            right_style_source_max: right.style.as_ref().and_then(max_style_source_id),
            left_style_product_source_count: left
                .style
                .as_ref()
                .map_or(0, |style| style.source_refs.len() as u64),
            right_style_product_source_max: right.style.as_ref().and_then(|style| {
                style
                    .source_map_refs
                    .iter()
                    .map(|range| range.source().value())
                    .max()
            }),
            left_patch_max: left.style.as_ref().and_then(|style| {
                style
                    .program
                    .patches()
                    .iter()
                    .map(|patch| patch.id().value())
                    .max()
            }),
            right_patch_max: right.style.as_ref().and_then(|style| {
                style
                    .program
                    .patches()
                    .iter()
                    .map(|patch| patch.id().value())
                    .max()
            }),
            left_program_product_source_count: left
                .program
                .as_ref()
                .map_or(0, |program| program.source_refs.len() as u64),
            right_program_product_source_max: right.program.as_ref().and_then(|program| {
                program
                    .source_ranges()
                    .map(|range| range.source().value())
                    .max()
            }),
            left_instruction_count: left
                .program
                .as_ref()
                .map_or(0, |program| program.instructions.len() as u64),
            right_definition_end_max: right.program.as_ref().and_then(|program| {
                program
                    .definitions
                    .iter()
                    .map(|definition| definition.body.end_instruction)
                    .max()
            }),
        }
    }

    fn validate(self) -> Result<(), ViewResourceMergeError> {
        ensure_u32_rebase(
            self.left_style_source_count,
            self.right_style_source_max,
            "Style source IDs",
        )?;
        ensure_u32_rebase(
            self.left_style_product_source_count,
            self.right_style_product_source_max,
            "Style product-source table indices",
        )?;
        if let Some(right_patch_max) = self.right_patch_max {
            let patch_offset = self
                .left_patch_max
                .map_or(0, |left_patch_max| u64::from(left_patch_max) + 1);
            ensure_u32_rebase(patch_offset, Some(right_patch_max), "Style patch IDs")?;
        }
        ensure_u32_rebase(
            self.left_program_product_source_count,
            self.right_program_product_source_max,
            "View program product-source table indices",
        )?;
        ensure_u32_rebase(
            self.left_instruction_count,
            self.right_definition_end_max,
            "View definition instruction spans",
        )
    }
}

fn checked_accumulate(
    total: &mut usize,
    value: usize,
    label: &'static str,
) -> Result<(), ViewResourceMergeError> {
    *total = total
        .checked_add(value)
        .ok_or(ViewResourceMergeError::Overflow(label))?;
    Ok(())
}

fn ensure_u32_count(count: usize, label: &'static str) -> Result<(), ViewResourceMergeError> {
    u32::try_from(count)
        .map(|_| ())
        .map_err(|_| ViewResourceMergeError::Overflow(label))
}

fn ensure_u32_rebase(
    offset: u64,
    right_max: Option<u32>,
    label: &'static str,
) -> Result<(), ViewResourceMergeError> {
    let Some(right_max) = right_max else {
        return Ok(());
    };
    let rebased = offset
        .checked_add(u64::from(right_max))
        .ok_or(ViewResourceMergeError::Overflow(label))?;
    u32::try_from(rebased)
        .map(|_| ())
        .map_err(|_| ViewResourceMergeError::Overflow(label))
}

fn max_style_source_id(style: &ViewStyleResource) -> Option<u32> {
    let mut maximum = None;
    let mut include = |source: ViewStyleSourceId| {
        maximum = Some(maximum.map_or(source.value(), |current: u32| current.max(source.value())));
    };
    for sheet in style.program.sheets() {
        for token in sheet.tokens() {
            include(token.source());
        }
        for rule in sheet.rules() {
            include(rule.source());
            if let Some(condition) = rule.environment() {
                for wrapper in condition.wrappers() {
                    include(wrapper.predicate_source());
                    include(wrapper.body_source());
                    include(wrapper.scope_source());
                }
                for clause in condition.clauses() {
                    include(clause.source());
                }
            }
            for declaration in rule.declarations() {
                include(declaration.source());
            }
        }
    }
    for patch in style.program.patches() {
        for declaration in patch.declarations() {
            include(declaration.source());
        }
    }
    maximum
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
            let source_offset = u32::try_from(left.source_map_refs.len())
                .map_err(|_| ViewResourceMergeError::Overflow("Style source IDs"))?;
            let patch_offset = if right.program.patches().is_empty() {
                0
            } else {
                left.program
                    .patches()
                    .iter()
                    .map(|patch| patch.id().value())
                    .max()
                    .map_or(Ok(0), |id| {
                        id.checked_add(1)
                            .ok_or(ViewResourceMergeError::Overflow("Style patch IDs"))
                    })?
            };
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
            left.program = ViewStyleProgram::try_new(sheets, patches)?;
            right.offset_source_indexes(left.source_refs.len())?;
            left.source_refs.append(&mut right.source_refs);
            left.source_map_refs.append(&mut right.source_map_refs);
            left.adapter_requirements.extend(right.adapter_requirements);
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
            for definition in &mut right.definitions {
                rebase_style_list(&mut definition.styles, patch_rebase)?;
            }
            rebase_style_references(&mut right.instructions, patch_rebase)?;
            merge_value_inventories(&mut left, &mut right)?;
            let right_source_offset = left.source_refs.len();
            right.offset_source_indexes(right_source_offset)?;
            left.source_refs.append(&mut right.source_refs);
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
            left.canonicalize_source_table();
            left.encode_canonical_section()?;
            Ok(Some(left))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{
        MergeOffsetInventory, ViewProgramStyleResources, ViewResourceBudget,
        ViewResourceMergeError, ensure_u32_count,
    };
    use crate::resource_codec::view::model::{ViewProgramResource, ViewStyleResource};
    use crate::resource_codec::{
        ProductSourceRef, SectionCodecError, SourceMapSection, SourceRangeRef,
    };
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    #[test]
    fn program_and_style_preflight_applies_supplied_style_budget() {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("style-budget.arcw").expect("source ID"),
            SourceName::path("style-budget.arcw"),
            "x",
        )
        .expect("source document");
        let source_map =
            SourceMapSection::try_from_documents(&[&document]).expect("source map section");
        let source = ProductSourceRef::from_document(
            source_map.documents().next().expect("source map document"),
        );
        let source_refs = vec![source.clone()];
        let source_map_refs = vec![
            SourceRangeRef::try_for_source(&source_refs, &source, 0, 1).expect("source range"),
        ];
        let style = ViewStyleResource {
            style_program_id: "view.style.budget".to_owned(),
            source_refs,
            source_map_refs,
            ..ViewStyleResource::default()
        };
        let resources =
            ViewProgramStyleResources::new(Some(ViewProgramResource::default()), Some(style));
        let budget = ViewResourceBudget {
            source_map_refs: 0,
            ..ViewResourceBudget::default()
        };

        assert_eq!(
            resources.validate_with_budget(&budget),
            Err(ViewResourceMergeError::Section(
                SectionCodecError::BudgetExceeded("view_style_source_map_refs")
            ))
        );
    }

    #[test]
    fn merge_offset_preflight_rejects_source_id_overflow_without_allocating_tables() {
        let inventory = MergeOffsetInventory {
            left_style_source_count: u64::from(u32::MAX),
            right_style_source_max: Some(1),
            ..MergeOffsetInventory::default()
        };

        assert_eq!(
            inventory.validate(),
            Err(ViewResourceMergeError::Overflow("Style source IDs"))
        );
    }

    #[test]
    fn merge_offset_preflight_rejects_patch_and_instruction_overflow() {
        let patch = MergeOffsetInventory {
            left_patch_max: Some(u32::MAX),
            right_patch_max: Some(0),
            ..MergeOffsetInventory::default()
        };
        assert_eq!(
            patch.validate(),
            Err(ViewResourceMergeError::Overflow("Style patch IDs"))
        );

        let instruction = MergeOffsetInventory {
            left_instruction_count: u64::from(u32::MAX),
            right_definition_end_max: Some(1),
            ..MergeOffsetInventory::default()
        };
        assert_eq!(
            instruction.validate(),
            Err(ViewResourceMergeError::Overflow(
                "View definition instruction spans"
            ))
        );
    }

    #[test]
    fn merge_count_preflight_rejects_public_table_index_overflow_without_allocation() {
        let Some(overflowing_count) = usize::try_from(u64::from(u32::MAX) + 1).ok() else {
            return;
        };

        assert_eq!(
            ensure_u32_count(overflowing_count, "View program public-ID table indices"),
            Err(ViewResourceMergeError::Overflow(
                "View program public-ID table indices"
            ))
        );
    }
}
