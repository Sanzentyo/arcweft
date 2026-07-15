use crate::container::BundleDigest;
use arcweft_presentation::fx::FxRuntimeType;
use arcweft_view::{
    ViewValueProgramId, ViewValueProgramInventory,
    style::{
        ViewSpecifiedValue, ViewStyleDeclaration, ViewStylePatch, ViewStylePredicate,
        ViewStyleProgram, ViewStyleRule, ViewStyleSelector, ViewStyleSheet, ViewStyleSourceId,
        ViewStyleToken,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::resource_codec::SourceMapIndex;
use crate::resource_codec::budget::{SectionCodecBudget, check_budget};
use crate::resource_codec::error::SectionCodecError;
use crate::resource_codec::kind::ProductSectionCodecKind;
use crate::resource_codec::table::PublicIdTable;
use crate::resource_codec::types::SourceRangeRef;

use super::compat::ViewResourceCompatibility;
use super::model::{
    ViewInputOptions, ViewInputResource, ViewProgramInstruction, ViewProgramResource,
    ViewStyleApplicationTarget, ViewStyleResource, ViewTextResource, ViewTextSourceKind,
    ViewThemeResource, ViewValueInputNamespace, ViewValueInputSource,
};

mod part;
mod transcript;

pub use part::ViewExportValidationError;

use self::transcript::{
    decode_view_section, encode_view_section, export_json_bytes, validate_canonical_view_transcript,
};

/// Decode limits for migrated View resource families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewResourceBudget {
    pub common: SectionCodecBudget,
    pub value_programs: usize,
    pub value_program_instructions: usize,
    pub value_inputs: usize,
    pub program_instructions: usize,
    pub fx_arguments: usize,
    pub definitions: usize,
    pub definition_parameters: usize,
    pub handlers: usize,
    pub exported_parts: usize,
    pub semantic_targets: usize,
    pub layout_bounds: usize,
    pub scroll_regions: usize,
    pub surfaces: usize,
    pub text_blocks: usize,
    pub action_buttons: usize,
    pub focus_groups: usize,
    pub focus_targets: usize,
    pub focus_edges: usize,
    pub style_rules: usize,
    pub style_tokens: usize,
    pub style_sheets: usize,
    pub style_patches: usize,
    pub style_declarations: usize,
    pub style_token_depth: usize,
    pub selector_depth: usize,
    pub part_count: usize,
    pub environment_predicates: usize,
    pub source_map_refs: usize,
    pub text_sources: usize,
    pub input_options: usize,
    pub palette_entries: usize,
    pub transcript_bytes: usize,
}

/// Human/tool export view generated from compact bytes. This is not accepted as a
/// product resource input format.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewResourceExport<T> {
    pub schema_version: u32,
    pub codec: ProductSectionCodecKind,
    pub codec_name: String,
    pub canonical_digest: BundleDigest,
    pub resource: T,
}

impl Default for ViewResourceBudget {
    fn default() -> Self {
        Self {
            common: SectionCodecBudget {
                records: 262_144,
                items: 262_144,
                public_ids: 262_144,
                strings: 262_144,
                string_bytes: 16 * 1024 * 1024,
                references: 1_000_000,
                depth: 64,
                ..SectionCodecBudget::default()
            },
            value_programs: 65_536,
            value_program_instructions: 1_000_000,
            value_inputs: 512,
            program_instructions: 262_144,
            fx_arguments: 1_000_000,
            definitions: 65_536,
            definition_parameters: 262_144,
            handlers: 65_536,
            exported_parts: 65_536,
            semantic_targets: 262_144,
            layout_bounds: 262_144,
            scroll_regions: 65_536,
            surfaces: 262_144,
            text_blocks: 262_144,
            action_buttons: 65_536,
            focus_groups: 65_536,
            focus_targets: 262_144,
            focus_edges: 1_000_000,
            style_rules: 262_144,
            style_tokens: 65_536,
            style_sheets: 65_536,
            style_patches: 262_144,
            style_declarations: 1_000_000,
            style_token_depth: 64,
            selector_depth: 32,
            part_count: 65_536,
            environment_predicates: 65_536,
            source_map_refs: 262_144,
            text_sources: 262_144,
            input_options: 65_536,
            palette_entries: 4_096,
            transcript_bytes: 16 * 1024 * 1024,
        }
    }
}

impl ViewProgramResource {
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        section.validate(&ViewResourceBudget::default())?;
        encode_view_section(
            ProductSectionCodecKind::ViewProgram,
            "view_program",
            &section,
            section.public_ids(),
            section.record_count(),
            &ViewResourceBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, ViewResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: ViewResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let (mut section, transcript): (Self, _) = decode_view_section(
            bytes,
            ProductSectionCodecKind::ViewProgram,
            "view_program",
            &budget,
            Self::public_ids,
            Self::record_count,
        )?;
        section.canonicalize();
        section.validate(&budget)?;
        validate_canonical_view_transcript(&transcript, &section)?;
        Ok(section)
    }

    pub fn canonical_digest(&self) -> Result<BundleDigest, SectionCodecError> {
        self.encode_canonical_section()
            .map(|bytes| BundleDigest::of(&bytes))
    }

    pub fn export_json_bytes(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        let digest = section.canonical_digest()?;
        export_json_bytes(ProductSectionCodecKind::ViewProgram, &section, digest)
    }

    pub fn compatibility_with(&self, next: &Self) -> ViewResourceCompatibility {
        if self == next {
            return ViewResourceCompatibility::ContentOnly;
        }
        if self.adapter_requirements != next.adapter_requirements {
            return ViewResourceCompatibility::RestartRequired;
        }
        if self.definitions != next.definitions || self.handlers != next.handlers {
            return ViewResourceCompatibility::CodeGenerational;
        }
        ViewResourceCompatibility::ContentOnly
    }

    /// Canonical public-ID table used when independently lowered View programs
    /// are composed and their source references are rebased.
    pub fn public_id_table(&self) -> Result<PublicIdTable, SectionCodecError> {
        PublicIdTable::new(self.public_ids())
    }

    /// Validates mandatory export provenance against the decoded product source map.
    pub fn validate_export_sources(
        &self,
        sources: &SourceMapIndex,
    ) -> Result<(), SectionCodecError> {
        part::validate_export_source_extents(self, sources).map_err(Into::into)
    }

    /// Binds authored export ranges to this section's canonical source-ID table.
    pub fn bind_export_source_refs(&mut self) -> Result<(), SectionCodecError> {
        let table = self.public_id_table()?;
        for exported in &mut self.exported_parts {
            let source = table
                .id_for(exported.source.source_id.as_str())
                .ok_or(ViewExportValidationError::UnknownSource)?;
            for range in exported.source.ranges_mut() {
                range.source = source;
            }
        }
        Ok(())
    }

    fn canonicalize(&mut self) {
        self.value_programs
            .sort_by_key(arcweft_view::ViewValueProgram::id);
        self.value_inputs
            .sort_by_key(|input| (input.namespace, input.slot));
        self.definitions
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        for instruction in &mut self.instructions {
            match instruction {
                ViewProgramInstruction::CallView { arguments, .. } => {
                    arguments.sort_by_key(|argument| argument.ordinal);
                }
                ViewProgramInstruction::ApplyFx { arguments, .. } => {
                    arguments.sort_by(|left, right| left.parameter.cmp(&right.parameter));
                }
                _ => {}
            }
        }
        self.handlers
            .sort_by(|left, right| left.handler_id.cmp(&right.handler_id));
        self.exported_parts.sort_by(|left, right| {
            left.target
                .view
                .cmp(&right.target.view)
                .then(left.public_name.cmp(&right.public_name))
                .then(left.target.part.cmp(&right.target.part))
        });
        self.semantic_targets
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.layout_bounds.sort_by(|left, right| {
            left.public_id
                .cmp(&right.public_id)
                .then(left.kind.cmp(&right.kind))
        });
        self.action_buttons
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.scroll_regions
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.surfaces
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.text_blocks
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.focus_groups
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.focus_navigation
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
    }

    fn validate(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
        self.validate_budgets(budget)?;
        self.validate_identity_contracts()?;
        self.validate_value_programs()?;
        self.validate_definitions()?;
        self.validate_exported_parts()?;
        self.validate_control_flow_spans()?;
        self.validate_unique_ids()?;
        self.validate_layout_bounds()?;
        self.validate_scroll_regions()?;
        self.validate_surfaces()?;
        self.validate_text_blocks()?;
        self.validate_action_buttons()?;
        self.validate_focus_targets()?;
        self.validate_fx_applications()?;
        self.validate_source_refs()
    }

    fn validate_identity_contracts(&self) -> Result<(), SectionCodecError> {
        if !valid_resource_identity(&self.program_id)
            || self
                .definitions
                .iter()
                .any(|definition| !valid_resource_identity(&definition.public_id))
        {
            return Err(SectionCodecError::NonCanonicalTable(
                "view_program_identities",
            ));
        }
        Ok(())
    }

    fn validate_budgets(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
        self.validate_execution_budgets(budget)?;
        self.validate_metadata_budgets(budget)
    }

    fn validate_execution_budgets(
        &self,
        budget: &ViewResourceBudget,
    ) -> Result<(), SectionCodecError> {
        check_budget(
            self.value_programs.len(),
            budget.value_programs,
            "view_value_programs",
        )?;
        check_budget(
            self.value_inputs.len(),
            budget.value_inputs,
            "view_value_inputs",
        )?;
        check_budget(
            self.value_programs
                .iter()
                .map(|program| program.program().instructions().len())
                .sum::<usize>(),
            budget.value_program_instructions,
            "view_value_program_instructions",
        )?;
        check_budget(
            self.instructions.len(),
            budget.program_instructions,
            "view_program_instructions",
        )?;
        check_budget(
            self.instructions
                .iter()
                .map(|instruction| match instruction {
                    ViewProgramInstruction::ApplyFx { arguments, .. } => arguments.len(),
                    _ => 0,
                })
                .sum::<usize>(),
            budget.fx_arguments,
            "view_fx_arguments",
        )?;
        check_budget(
            self.definitions.len(),
            budget.definitions,
            "view_definitions",
        )?;
        check_budget(
            self.definitions
                .iter()
                .map(|definition| definition.parameters.len())
                .sum::<usize>(),
            budget.definition_parameters,
            "view_definition_parameters",
        )?;
        Ok(())
    }

    fn validate_metadata_budgets(
        &self,
        budget: &ViewResourceBudget,
    ) -> Result<(), SectionCodecError> {
        check_budget(self.handlers.len(), budget.handlers, "view_handlers")?;
        check_budget(
            self.exported_parts.len(),
            budget.exported_parts,
            "view_exported_parts",
        )?;
        check_budget(
            self.semantic_targets.len(),
            budget.semantic_targets,
            "view_semantic_targets",
        )?;
        check_budget(
            self.layout_bounds.len(),
            budget.layout_bounds,
            "view_layout_bounds",
        )?;
        check_budget(
            self.action_buttons.len(),
            budget.action_buttons,
            "view_action_buttons",
        )?;
        check_budget(
            self.scroll_regions.len(),
            budget.scroll_regions,
            "view_scroll_regions",
        )?;
        check_budget(self.surfaces.len(), budget.surfaces, "view_surfaces")?;
        check_budget(
            self.text_blocks.len(),
            budget.text_blocks,
            "view_text_blocks",
        )?;
        check_budget(
            self.focus_groups.len(),
            budget.focus_groups,
            "view_focus_groups",
        )?;
        check_budget(
            self.focus_navigation.len(),
            budget.focus_targets,
            "view_focus_navigation",
        )?;
        check_budget(
            self.focus_navigation
                .iter()
                .map(|target| target.edges.len())
                .sum::<usize>(),
            budget.focus_edges,
            "view_focus_edges",
        )?;
        check_budget(
            self.source_refs().count(),
            budget.source_map_refs,
            "view_program_source_ranges",
        )?;
        Ok(())
    }

    fn validate_value_programs(&self) -> Result<(), SectionCodecError> {
        let inventory = ViewValueProgramInventory::from_programs(self.value_programs.clone())
            .map_err(|_| SectionCodecError::NonCanonicalTable("view_value_program_inventory"))?;
        self.validate_value_inputs(&inventory)?;
        for instruction in &self.instructions {
            match instruction {
                ViewProgramInstruction::CallView {
                    view, arguments, ..
                } => {
                    let target = self
                        .definitions
                        .iter()
                        .find(|definition| definition.public_id == *view);
                    for argument in arguments {
                        let expected = target
                            .and_then(|definition| {
                                definition.parameters.get(usize::from(argument.ordinal))
                            })
                            .and_then(|parameter| parameter.value_type);
                        validate_program(&inventory, argument.value_program, expected)?;
                    }
                }
                ViewProgramInstruction::Branch {
                    condition_program, ..
                } => validate_program(&inventory, *condition_program, Some(FxRuntimeType::Bool))?,
                ViewProgramInstruction::RepeatKeyed {
                    source_program,
                    key_program,
                    ..
                } => {
                    validate_program(&inventory, *source_program, Some(FxRuntimeType::I32))?;
                    validate_program(&inventory, *key_program, Some(FxRuntimeType::I32))?;
                }
                ViewProgramInstruction::Await { source_program, .. } => {
                    validate_program(&inventory, *source_program, None)?;
                }
                ViewProgramInstruction::BindLocal {
                    binding,
                    value_program,
                    ..
                } => {
                    if !valid_identifier(binding) {
                        return Err(SectionCodecError::NonCanonicalTable("view_local_binding"));
                    }
                    validate_program(&inventory, *value_program, None)?;
                }
                ViewProgramInstruction::ApplyFx {
                    arguments,
                    key_program,
                    ..
                } => {
                    validate_optional_program(&inventory, *key_program, Some(FxRuntimeType::I32))?;
                    for argument in arguments {
                        validate_program(&inventory, argument.value_program, None)?;
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
        }
        Ok(())
    }

    fn validate_value_inputs(
        &self,
        inventory: &ViewValueProgramInventory,
    ) -> Result<(), SectionCodecError> {
        let mut parameters = BTreeSet::new();
        let mut state = BTreeSet::new();
        for input in &self.value_inputs {
            let (types, slots) = match input.namespace {
                ViewValueInputNamespace::Parameter => {
                    (inventory.parameter_types(), &mut parameters)
                }
                ViewValueInputNamespace::State => (inventory.state_types(), &mut state),
            };
            let source_matches_namespace = matches!(
                (input.namespace, &input.source),
                (
                    ViewValueInputNamespace::Parameter,
                    ViewValueInputSource::DefinitionParameter { .. }
                ) | (
                    ViewValueInputNamespace::State,
                    ViewValueInputSource::Projection { .. }
                        | ViewValueInputSource::LifetimeProjection { .. }
                        | ViewValueInputSource::Local { .. }
                        | ViewValueInputSource::RepeatOrdinal { .. }
                )
            );
            if !slots.insert(input.slot)
                || types.get(usize::from(input.slot)).copied() != Some(input.value_type)
                || !valid_value_input_source(&input.source)
                || !source_matches_namespace
            {
                return Err(SectionCodecError::NonCanonicalTable("view_value_inputs"));
            }
        }
        if parameters.len() != inventory.parameter_types().len()
            || state.len() != inventory.state_types().len()
        {
            return Err(SectionCodecError::NonCanonicalTable(
                "view_value_input_coverage",
            ));
        }
        Ok(())
    }

    fn validate_definitions(&self) -> Result<(), SectionCodecError> {
        reject_duplicates(
            self.definitions
                .iter()
                .map(|definition| definition.public_id.clone()),
            "view_definitions",
        )?;
        let mut spans = self
            .definitions
            .iter()
            .map(|definition| definition.body)
            .collect::<Vec<_>>();
        spans.sort_by_key(|span| (span.start_instruction, span.end_instruction));
        let mut cursor = 0_u32;
        for span in spans {
            if span.start_instruction != cursor
                || span.start_instruction > span.end_instruction
                || span.end_instruction as usize > self.instructions.len()
            {
                return Err(SectionCodecError::NonCanonicalTable(
                    "view_definition_spans",
                ));
            }
            cursor = span.end_instruction;
        }
        if cursor as usize != self.instructions.len() {
            return Err(SectionCodecError::NonCanonicalTable(
                "view_definition_coverage",
            ));
        }
        let inventory = ViewValueProgramInventory::from_programs(self.value_programs.clone())
            .map_err(|_| SectionCodecError::NonCanonicalTable("view_value_program_inventory"))?;
        for definition in &self.definitions {
            let mut names = BTreeSet::new();
            for (ordinal, parameter) in definition.parameters.iter().enumerate() {
                if usize::from(parameter.ordinal) != ordinal
                    || !valid_identifier(&parameter.name)
                    || !names.insert(parameter.name.as_str())
                {
                    return Err(SectionCodecError::NonCanonicalTable(
                        "view_definition_parameters",
                    ));
                }
                validate_optional_program(
                    &inventory,
                    parameter.default_program,
                    parameter.value_type,
                )?;
                if parameter.role == super::model::ViewParameterRole::Dialogue
                    && (parameter.value_type.is_some()
                        || parameter.value_slot.is_some()
                        || parameter.default_program.is_some())
                {
                    return Err(SectionCodecError::NonCanonicalTable(
                        "view_dialogue_parameter_schema",
                    ));
                }
                match (parameter.value_type, parameter.value_slot) {
                    (Some(value_type), Some(value_slot)) => {
                        let expected_source = ViewValueInputSource::DefinitionParameter {
                            view: definition.public_id.clone(),
                            name: parameter.name.clone(),
                        };
                        if !self.value_inputs.iter().any(|input| {
                            input.namespace == ViewValueInputNamespace::Parameter
                                && input.slot == value_slot
                                && input.value_type == value_type
                                && input.source == expected_source
                        }) {
                            return Err(SectionCodecError::NonCanonicalTable(
                                "view_definition_parameter_slots",
                            ));
                        }
                    }
                    (None, None) => {}
                    (Some(_), None) | (None, Some(_)) => {
                        return Err(SectionCodecError::NonCanonicalTable(
                            "view_definition_parameter_slots",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_exported_parts(&self) -> Result<(), SectionCodecError> {
        part::validate_exports(self).map_err(Into::into)
    }

    fn validate_control_flow_spans(&self) -> Result<(), SectionCodecError> {
        for (index, instruction) in self.instructions.iter().enumerate() {
            let definition_end = self
                .definitions
                .iter()
                .find(|definition| {
                    definition.body.start_instruction as usize <= index
                        && index < definition.body.end_instruction as usize
                })
                .map(|definition| definition.body.end_instruction as usize)
                .ok_or(SectionCodecError::NonCanonicalTable(
                    "view_definition_coverage",
                ))?;
            let body_start = index.saturating_add(1);
            let valid_end = |offset: u32, span: u32| {
                usize::try_from(offset)
                    .ok()
                    .and_then(|offset| body_start.checked_add(offset))
                    .and_then(|start| {
                        usize::try_from(span)
                            .ok()
                            .and_then(|span| start.checked_add(span))
                    })
                    .is_some_and(|end| end <= definition_end)
            };
            let valid = match instruction {
                ViewProgramInstruction::Branch {
                    then_span,
                    else_span,
                    ..
                } => {
                    valid_end(0, *then_span)
                        && else_span.is_none_or(|else_span| valid_end(*then_span, else_span))
                }
                ViewProgramInstruction::RepeatKeyed { body_span, .. } => valid_end(0, *body_span),
                ViewProgramInstruction::Await {
                    pending_branch,
                    ready_branch,
                    error_branch,
                    denied_branch,
                    ..
                } => [pending_branch, ready_branch, error_branch, denied_branch]
                    .into_iter()
                    .flatten()
                    .all(|branch| valid_end(branch.start_offset, branch.body_span)),
                _ => true,
            };
            if !valid {
                return Err(SectionCodecError::NonCanonicalTable(
                    "view_control_flow_spans",
                ));
            }
        }
        Ok(())
    }

    fn validate_unique_ids(&self) -> Result<(), SectionCodecError> {
        reject_duplicates(
            self.handlers
                .iter()
                .map(|handler| handler.handler_id.clone()),
            "view_handlers",
        )?;
        reject_duplicate_keys(
            self.exported_parts
                .iter()
                .map(|part| (&part.target.view, &part.target.part)),
            "view_exported_part_targets",
        )?;
        reject_duplicate_keys(
            self.exported_parts
                .iter()
                .map(|part| (&part.target.view, &part.public_name)),
            "view_exported_part_public_names",
        )?;
        reject_duplicates(
            self.semantic_targets
                .iter()
                .map(|target| target.public_id.clone()),
            "view_semantic_targets",
        )?;
        reject_duplicates(
            self.layout_bounds
                .iter()
                .map(super::model::ViewLayoutBoundsResource::identity_key),
            "view_layout_bounds",
        )?;
        reject_duplicates(
            self.action_buttons
                .iter()
                .map(|button| button.public_id.clone()),
            "view_action_buttons",
        )?;
        reject_duplicates(
            self.scroll_regions
                .iter()
                .map(|region| region.public_id.clone()),
            "view_scroll_regions",
        )?;
        reject_duplicates(
            self.surfaces
                .iter()
                .map(|surface| surface.public_id.clone()),
            "view_surfaces",
        )?;
        reject_duplicates(
            self.text_blocks.iter().map(|block| block.public_id.clone()),
            "view_text_blocks",
        )?;
        reject_duplicates(
            self.focus_groups
                .iter()
                .map(|group| group.public_id.clone()),
            "view_focus_groups",
        )?;
        reject_duplicates(
            self.focus_navigation
                .iter()
                .map(|target| target.public_id.clone()),
            "view_focus_navigation",
        )
    }

    fn validate_layout_bounds(&self) -> Result<(), SectionCodecError> {
        if self
            .layout_bounds
            .iter()
            .all(super::model::ViewLayoutBoundsResource::is_valid)
        {
            Ok(())
        } else {
            Err(SectionCodecError::NonCanonicalTable("view_layout_bounds"))
        }
    }

    fn validate_scroll_regions(&self) -> Result<(), SectionCodecError> {
        if self
            .scroll_regions
            .iter()
            .all(super::model::ViewScrollRegionResource::is_valid)
        {
            Ok(())
        } else {
            Err(SectionCodecError::NonCanonicalTable("view_scroll_regions"))
        }
    }

    fn validate_text_blocks(&self) -> Result<(), SectionCodecError> {
        if !self
            .text_blocks
            .iter()
            .all(super::model::ViewTextBlockResource::is_valid)
        {
            return Err(SectionCodecError::NonCanonicalTable("view_text_blocks"));
        }
        let text_blocks = self
            .text_blocks
            .iter()
            .map(|block| (block.public_id.as_str(), block))
            .collect::<BTreeMap<_, _>>();
        let mut referenced = BTreeSet::new();
        for definition in &self.definitions {
            let instructions = &self.instructions[definition.body.start_instruction as usize
                ..definition.body.end_instruction as usize];
            for instruction in instructions {
                let ViewProgramInstruction::EmitText {
                    text_source,
                    text_block,
                    ..
                } = instruction
                else {
                    continue;
                };
                let Some(block) = text_blocks.get(text_block.as_str()) else {
                    return Err(SectionCodecError::NonCanonicalTable(
                        "view_emit_text_block_refs",
                    ));
                };
                if !referenced.insert(text_block.as_str()) {
                    return Err(SectionCodecError::NonCanonicalTable(
                        "view_emit_text_block_duplicate_refs",
                    ));
                }
                if block.text_source != *text_source {
                    return Err(SectionCodecError::NonCanonicalTable(
                        "view_emit_text_block_sources",
                    ));
                }
                if block.view.as_deref() != Some(definition.public_id.as_str()) {
                    return Err(SectionCodecError::NonCanonicalTable(
                        "view_emit_text_block_owners",
                    ));
                }
            }
        }
        if referenced.len() != text_blocks.len() {
            return Err(SectionCodecError::NonCanonicalTable(
                "view_emit_text_block_coverage",
            ));
        }
        Ok(())
    }

    fn validate_surfaces(&self) -> Result<(), SectionCodecError> {
        if self
            .surfaces
            .iter()
            .all(super::model::ViewSurfaceResource::is_valid)
        {
            Ok(())
        } else {
            Err(SectionCodecError::NonCanonicalTable("view_surfaces"))
        }
    }

    fn validate_action_buttons(&self) -> Result<(), SectionCodecError> {
        for button in &self.action_buttons {
            let super::model::ViewActionButtonActionResource::DialoguePrimaryAction { parameter } =
                &button.action
            else {
                continue;
            };
            let Some(definition) = button.view.as_deref().and_then(|view| {
                self.definitions
                    .iter()
                    .find(|definition| definition.public_id == view)
            }) else {
                return Err(SectionCodecError::NonCanonicalTable(
                    "view_dialogue_primary_action_owner",
                ));
            };
            if !valid_identifier(parameter)
                || !definition.parameters.iter().any(|candidate| {
                    candidate.name == *parameter
                        && candidate.role == super::model::ViewParameterRole::Dialogue
                })
            {
                return Err(SectionCodecError::NonCanonicalTable(
                    "view_dialogue_primary_action_parameter",
                ));
            }
        }
        Ok(())
    }

    fn validate_focus_targets(&self) -> Result<(), SectionCodecError> {
        let authored_targets = self
            .focus_navigation
            .iter()
            .map(|target| target.public_id.as_str())
            .chain(
                self.semantic_targets
                    .iter()
                    .map(|target| target.public_id.as_str()),
            )
            .chain(
                self.action_buttons
                    .iter()
                    .map(|button| button.public_id.as_str()),
            )
            .collect::<BTreeSet<_>>();
        for explicit in self
            .focus_groups
            .iter()
            .filter_map(|group| group.initial.explicit_target())
            .chain(self.focus_navigation.iter().flat_map(|target| {
                target
                    .edges
                    .iter()
                    .filter_map(|edge| edge.target.explicit_target())
            }))
        {
            if !authored_targets.contains(explicit) {
                return Err(SectionCodecError::NonCanonicalTable(
                    "view_focus_missing_target",
                ));
            }
        }
        Ok(())
    }

    fn validate_fx_applications(&self) -> Result<(), SectionCodecError> {
        let definitions = self
            .definitions
            .iter()
            .map(|definition| definition.public_id.as_str())
            .collect::<BTreeSet<_>>();
        for instruction in &self.instructions {
            if let ViewProgramInstruction::CallView {
                view, arguments, ..
            } = instruction
            {
                if !definitions.contains(view.as_str()) {
                    return Err(SectionCodecError::NonCanonicalTable("view_call_definition"));
                }
                let target = self
                    .definitions
                    .iter()
                    .find(|definition| definition.public_id == *view)
                    .expect("definition membership checked above");
                let mut ordinals = BTreeSet::new();
                let mut names = BTreeSet::new();
                for argument in arguments {
                    if usize::from(argument.ordinal) >= target.parameters.len()
                        || !ordinals.insert(argument.ordinal)
                        || argument
                            .name
                            .as_deref()
                            .is_some_and(|name| !names.insert(name) || !valid_identifier(name))
                    {
                        return Err(SectionCodecError::NonCanonicalTable("view_call_arguments"));
                    }
                    if let Some(name) = argument.name.as_deref()
                        && target.parameters[usize::from(argument.ordinal)].name != name
                    {
                        return Err(SectionCodecError::NonCanonicalTable(
                            "view_call_argument_names",
                        ));
                    }
                }
                if target.parameters.iter().any(|parameter| {
                    parameter.default_program.is_none() && !ordinals.contains(&parameter.ordinal)
                }) {
                    return Err(SectionCodecError::NonCanonicalTable(
                        "view_call_required_arguments",
                    ));
                }
            }
            let ViewProgramInstruction::ApplyFx { arguments, .. } = instruction else {
                continue;
            };
            let mut parameters = BTreeSet::new();
            for argument in arguments {
                if !valid_identifier(&argument.parameter) {
                    return Err(SectionCodecError::NonCanonicalTable(
                        "view_fx_argument_names",
                    ));
                }
                if !parameters.insert(argument.parameter.as_str()) {
                    return Err(SectionCodecError::NonCanonicalTable(
                        "view_fx_argument_bindings",
                    ));
                }
            }
        }
        Ok(())
    }

    fn source_refs(&self) -> impl Iterator<Item = &SourceRangeRef> {
        self.instructions
            .iter()
            .filter_map(ViewProgramInstruction::source)
            .chain(
                self.semantic_targets
                    .iter()
                    .filter_map(|item| item.source.as_ref()),
            )
            .chain(
                self.layout_bounds
                    .iter()
                    .filter_map(|item| item.source.as_ref()),
            )
            .chain(
                self.scroll_regions
                    .iter()
                    .filter_map(|item| item.source.as_ref()),
            )
            .chain(self.surfaces.iter().filter_map(|item| item.source.as_ref()))
            .chain(
                self.text_blocks
                    .iter()
                    .filter_map(|item| item.source.as_ref()),
            )
            .chain(
                self.action_buttons
                    .iter()
                    .filter_map(|item| item.source.as_ref()),
            )
            .chain(
                self.focus_groups
                    .iter()
                    .filter_map(|item| item.source.as_ref()),
            )
            .chain(self.focus_navigation.iter().flat_map(|item| {
                item.source
                    .iter()
                    .chain(item.edges.iter().filter_map(|edge| edge.source.as_ref()))
            }))
            .chain(
                self.exported_parts
                    .iter()
                    .flat_map(|part| part.source.ranges()),
            )
    }

    fn validate_source_refs(&self) -> Result<(), SectionCodecError> {
        let public_ids = self.public_id_table()?;
        self.source_refs()
            .try_for_each(|source| public_ids.get(source.source).map(|_| ()))
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(
            [self.program_id.clone()]
                .into_iter()
                .chain(self.definitions.iter().flat_map(|definition| {
                    std::iter::once(definition.public_id.clone())
                        .chain(
                            definition
                                .parameters
                                .iter()
                                .map(|parameter| parameter.name.clone()),
                        )
                        .chain(style_apply_public_ids(&definition.styles))
                }))
                .chain(self.instructions.iter().flat_map(instruction_public_ids))
                .chain(
                    self.handlers
                        .iter()
                        .flat_map(|handler| [handler.handler_id.clone(), handler.event.clone()]),
                )
                .chain(self.exported_parts.iter().flat_map(|part| {
                    [
                        part.target.view.public_id().as_str().to_owned(),
                        part.target.part.public_id().as_str().to_owned(),
                        part.public_name.public_id().as_str().to_owned(),
                        part.source.source_id.as_str().to_owned(),
                    ]
                }))
                .chain(
                    self.semantic_targets
                        .iter()
                        .flat_map(semantic_target_public_ids),
                )
                .chain(
                    self.layout_bounds
                        .iter()
                        .map(|bounds| bounds.public_id.clone()),
                )
                .chain(
                    self.action_buttons
                        .iter()
                        .flat_map(action_button_public_ids),
                )
                .chain(
                    self.scroll_regions
                        .iter()
                        .flat_map(scroll_region_public_ids),
                )
                .chain(self.surfaces.iter().flat_map(surface_public_ids))
                .chain(self.text_blocks.iter().flat_map(text_block_public_ids))
                .chain(self.focus_groups.iter().flat_map(focus_group_public_ids))
                .chain(
                    self.focus_navigation
                        .iter()
                        .flat_map(focus_navigation_public_ids),
                ),
        )
    }

    fn record_count(&self) -> u32 {
        saturating_u32(self.value_programs.len())
            .saturating_add(saturating_u32(self.definitions.len()))
            .saturating_add(saturating_u32(
                self.definitions
                    .iter()
                    .map(|definition| definition.parameters.len())
                    .sum::<usize>(),
            ))
            .saturating_add(saturating_u32(self.value_inputs.len()))
            .saturating_add(saturating_u32(self.instructions.len()))
            .saturating_add(saturating_u32(self.layout_bounds.len()))
            .saturating_add(saturating_u32(self.action_buttons.len()))
            .saturating_add(saturating_u32(self.scroll_regions.len()))
            .saturating_add(saturating_u32(self.surfaces.len()))
            .saturating_add(saturating_u32(self.text_blocks.len()))
            .saturating_add(saturating_u32(self.focus_groups.len()))
            .saturating_add(saturating_u32(self.focus_navigation.len()))
    }
}

fn semantic_target_public_ids(target: &super::model::ViewSemanticTarget) -> Vec<String> {
    [
        Some(target.public_id.clone()),
        Some(target.target.clone()),
        target.view.clone(),
        target.label_text_source.clone(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn action_button_public_ids(button: &super::model::ViewActionButtonResource) -> Vec<String> {
    let action_ids = match &button.action {
        super::model::ViewActionButtonActionResource::Noop
        | super::model::ViewActionButtonActionResource::DialoguePrimaryAction { .. } => Vec::new(),
        super::model::ViewActionButtonActionResource::ActionInvoke { action, payload } => {
            std::iter::once(action.clone())
                .chain(action_payload_refs(payload.as_ref()))
                .collect()
        }
    };
    [
        Some(button.public_id.clone()),
        button.view.clone(),
        button.containing_scroll_region.clone(),
        Some(button.label_text_source.clone()),
    ]
    .into_iter()
    .flatten()
    .chain(action_ids)
    .collect()
}

fn scroll_region_public_ids(region: &super::model::ViewScrollRegionResource) -> Vec<String> {
    [Some(region.public_id.clone()), region.view.clone()]
        .into_iter()
        .flatten()
        .collect()
}

fn surface_public_ids(surface: &super::model::ViewSurfaceResource) -> Vec<String> {
    [
        Some(surface.public_id.clone()),
        surface.view.clone(),
        surface.containing_scroll_region.clone(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn text_block_public_ids(block: &super::model::ViewTextBlockResource) -> Vec<String> {
    [
        Some(block.public_id.clone()),
        block.view.clone(),
        block.containing_scroll_region.clone(),
        Some(block.text_source.clone()),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn focus_group_public_ids(group: &super::model::ViewFocusGroupResource) -> Vec<String> {
    [
        Some(group.public_id.clone()),
        group.view.clone(),
        group.parent.clone(),
        group.initial.explicit_target().map(ToOwned::to_owned),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn focus_navigation_public_ids(target: &super::model::ViewFocusNavigationResource) -> Vec<String> {
    [
        Some(target.public_id.clone()),
        target.view.clone(),
        target.group.clone(),
    ]
    .into_iter()
    .flatten()
    .chain(
        target
            .edges
            .iter()
            .filter_map(|edge| edge.target.explicit_target().map(ToOwned::to_owned)),
    )
    .collect()
}

impl ViewStyleResource {
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize()?;
        section.validate(&ViewResourceBudget::default())?;
        encode_view_section(
            ProductSectionCodecKind::ViewStyle,
            "view_style",
            &section,
            section.public_ids(),
            section.record_count(),
            &ViewResourceBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, ViewResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: ViewResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let (section, transcript): (Self, _) = decode_view_section(
            bytes,
            ProductSectionCodecKind::ViewStyle,
            "view_style",
            &budget,
            Self::public_ids,
            Self::record_count,
        )?;
        if !section.is_canonical_order() {
            return Err(SectionCodecError::NonCanonicalTable(
                "view_style_inventory_order",
            ));
        }
        section.validate(&budget)?;
        validate_canonical_view_transcript(&transcript, &section)?;
        Ok(section)
    }

    pub fn canonical_digest(&self) -> Result<BundleDigest, SectionCodecError> {
        self.encode_canonical_section()
            .map(|bytes| BundleDigest::of(&bytes))
    }

    pub fn export_json_bytes(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize()?;
        let digest = section.canonical_digest()?;
        export_json_bytes(ProductSectionCodecKind::ViewStyle, &section, digest)
    }

    pub fn compatibility_with(&self, next: &Self) -> ViewResourceCompatibility {
        if self == next {
            return ViewResourceCompatibility::ContentOnly;
        }
        if self.adapter_requirements != next.adapter_requirements {
            return ViewResourceCompatibility::RestartRequired;
        }
        ViewResourceCompatibility::ContentOnly
    }

    /// Canonical public-ID table used by compiler source-map lowering and
    /// section encoding. Callers must not reproduce this inventory manually.
    pub fn public_id_table(&self) -> Result<PublicIdTable, SectionCodecError> {
        PublicIdTable::new(self.public_ids())
    }

    pub(super) fn canonicalize(&mut self) -> Result<(), SectionCodecError> {
        let mut source_order = (0..self.source_map_refs.len()).collect::<Vec<_>>();
        source_order.sort_by_key(|index| {
            let range = self.source_map_refs[*index];
            (range.source, range.start_byte, range.end_byte)
        });

        if source_order
            .iter()
            .enumerate()
            .any(|(new_index, old_index)| new_index != *old_index)
        {
            let mut source_rebase = vec![ViewStyleSourceId::new(0); source_order.len()];
            let mut canonical_ranges = Vec::with_capacity(source_order.len());
            for (new_index, old_index) in source_order.into_iter().enumerate() {
                let new_index =
                    u32::try_from(new_index).map_err(|_| SectionCodecError::LengthOverflow)?;
                source_rebase[old_index] = ViewStyleSourceId::new(new_index);
                canonical_ranges.push(self.source_map_refs[old_index]);
            }
            self.rebase_source_ids(&source_rebase)?;
            self.source_map_refs = canonical_ranges;
        }

        self.adapter_requirements.sort_by_key(|reference| {
            (
                reference.section_kind,
                reference.section_id,
                reference.content_digest,
                reference.public_id,
            )
        });
        Ok(())
    }

    fn rebase_source_ids(
        &mut self,
        source_rebase: &[ViewStyleSourceId],
    ) -> Result<(), SectionCodecError> {
        let source = |id: ViewStyleSourceId| {
            source_rebase.get(id.value() as usize).copied().ok_or(
                SectionCodecError::NonCanonicalTable("view_style_source_ids"),
            )
        };
        let sheets = self
            .program
            .sheets()
            .iter()
            .map(|sheet| {
                let tokens = sheet
                    .tokens()
                    .iter()
                    .map(|token| {
                        ViewStyleToken::new(
                            token.id().clone(),
                            token.value_kind(),
                            token.value().clone(),
                            source(token.source())?,
                        )
                        .map_err(|_| SectionCodecError::NonCanonicalTable("view_style_program"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let rules = sheet
                    .rules()
                    .iter()
                    .map(|rule| {
                        let declarations = rule
                            .declarations()
                            .iter()
                            .map(|declaration| {
                                ViewStyleDeclaration::new(
                                    declaration.property(),
                                    declaration.value().clone(),
                                    declaration.op(),
                                    source(declaration.source())?,
                                )
                                .map_err(|_| {
                                    SectionCodecError::NonCanonicalTable("view_style_program")
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        ViewStyleRule::new(
                            rule.selector().clone(),
                            declarations,
                            rule.source_order(),
                            source(rule.source())?,
                        )
                        .map_err(|_| SectionCodecError::NonCanonicalTable("view_style_program"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                ViewStyleSheet::new(sheet.id().clone(), tokens, rules)
                    .map_err(|_| SectionCodecError::NonCanonicalTable("view_style_program"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let patches = self
            .program
            .patches()
            .iter()
            .map(|patch| {
                patch
                    .declarations()
                    .iter()
                    .map(|declaration| {
                        ViewStyleDeclaration::new(
                            declaration.property(),
                            declaration.value().clone(),
                            declaration.op(),
                            source(declaration.source())?,
                        )
                        .map_err(|_| SectionCodecError::NonCanonicalTable("view_style_program"))
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(|declarations| ViewStylePatch::new(patch.id(), declarations))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.program = ViewStyleProgram::try_new(sheets, patches)
            .map_err(|_| SectionCodecError::NonCanonicalTable("view_style_program"))?;
        Ok(())
    }

    fn is_canonical_order(&self) -> bool {
        self.program
            .sheets()
            .windows(2)
            .all(|pair| pair[0].id() < pair[1].id())
            && self
                .program
                .patches()
                .windows(2)
                .all(|pair| pair[0].id() < pair[1].id())
            && self.source_map_refs.windows(2).all(|pair| {
                (pair[0].source, pair[0].start_byte, pair[0].end_byte)
                    <= (pair[1].source, pair[1].start_byte, pair[1].end_byte)
            })
            && self.adapter_requirements.windows(2).all(|pair| {
                (
                    pair[0].section_kind,
                    pair[0].section_id,
                    pair[0].content_digest,
                    pair[0].public_id,
                ) <= (
                    pair[1].section_kind,
                    pair[1].section_id,
                    pair[1].content_digest,
                    pair[1].public_id,
                )
            })
    }

    fn validate(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
        self.validate_budgets(budget)?;
        self.validate_identity_contracts()?;
        self.validate_source_maps()?;
        self.program.sheets().iter().try_for_each(|sheet| {
            sheet.tokens().iter().try_for_each(|token| {
                check_budget(
                    style_token_depth(sheet, token),
                    budget.style_token_depth,
                    "view_style_token_depth",
                )
            })
        })
    }

    fn validate_budgets(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
        check_budget(
            self.program.sheets().len(),
            budget.style_sheets,
            "view_style_sheets",
        )?;
        check_budget(
            self.program.patches().len(),
            budget.style_patches,
            "view_style_inline_patches",
        )?;
        let token_count = self
            .program
            .sheets()
            .iter()
            .map(|sheet| sheet.tokens().len())
            .sum();
        let rule_count = self
            .program
            .sheets()
            .iter()
            .map(|sheet| sheet.rules().len())
            .sum();
        let declaration_count = self
            .program
            .sheets()
            .iter()
            .flat_map(ViewStyleSheet::rules)
            .map(|rule| rule.declarations().len())
            .chain(
                self.program
                    .patches()
                    .iter()
                    .map(|patch| patch.declarations().len()),
            )
            .sum();
        check_budget(token_count, budget.style_tokens, "view_style_tokens")?;
        check_budget(rule_count, budget.style_rules, "view_style_rules")?;
        check_budget(
            declaration_count,
            budget.style_declarations,
            "view_style_declarations",
        )?;
        check_budget(
            self.source_map_refs.len(),
            budget.source_map_refs,
            "view_style_source_map_refs",
        )?;
        self.program
            .sheets()
            .iter()
            .flat_map(ViewStyleSheet::rules)
            .map(arcweft_view::style::ViewStyleRule::selector)
            .try_for_each(|selector| {
                check_budget(
                    selector.max_depth(),
                    budget.selector_depth,
                    "view_selector_depth",
                )
            })?;
        let environment_predicate_count = self
            .program
            .sheets()
            .iter()
            .flat_map(ViewStyleSheet::rules)
            .flat_map(|rule| rule.selector().sequences())
            .flat_map(arcweft_view::style::ViewStyleSelectorSequence::predicates)
            .filter(|predicate| matches!(predicate, ViewStylePredicate::Environment(_)))
            .count();
        check_budget(
            environment_predicate_count,
            budget.environment_predicates,
            "view_style_environment_predicates",
        )?;
        let part_count = self
            .program
            .sheets()
            .iter()
            .flat_map(ViewStyleSheet::rules)
            .flat_map(|rule| rule.selector().sequences())
            .filter_map(|sequence| sequence.part())
            .collect::<BTreeSet<_>>()
            .len();
        check_budget(part_count, budget.part_count, "view_style_part_count")
    }

    fn validate_identity_contracts(&self) -> Result<(), SectionCodecError> {
        if !valid_resource_identity(&self.style_program_id) {
            return Err(SectionCodecError::NonCanonicalTable(
                "view_style_program_identity",
            ));
        }
        reject_duplicates(
            std::iter::once(self.style_program_id.clone()).chain(
                self.program
                    .sheets()
                    .iter()
                    .map(|sheet| sheet.id().public_id().as_str().to_owned()),
            ),
            "view_style_product_identities",
        )
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(
            std::iter::once(self.style_program_id.clone())
                .chain(self.program.sheets().iter().flat_map(|sheet| {
                    std::iter::once(sheet.id().public_id().as_str().to_owned())
                        .chain(sheet.tokens().iter().flat_map(|token| {
                            std::iter::once(token.id().public_id().as_str().to_owned())
                                .chain(style_value_public_ids(token.value()))
                        }))
                        .chain(sheet.rules().iter().flat_map(style_rule_public_ids))
                }))
                .chain(self.program.patches().iter().flat_map(|patch| {
                    patch
                        .declarations()
                        .iter()
                        .flat_map(|declaration| style_value_public_ids(declaration.value()))
                })),
        )
    }

    fn record_count(&self) -> u32 {
        let records = self
            .program
            .sheets()
            .len()
            .saturating_add(self.program.patches().len())
            .saturating_add(
                self.program
                    .sheets()
                    .iter()
                    .map(|sheet| sheet.rules().len())
                    .sum(),
            );
        saturating_u32(records)
    }

    fn validate_source_maps(&self) -> Result<(), SectionCodecError> {
        let public_ids = self.public_id_table()?;
        let valid_owners = std::iter::once(self.style_program_id.as_str())
            .chain(
                self.program
                    .sheets()
                    .iter()
                    .map(|sheet| sheet.id().public_id().as_str()),
            )
            .collect::<BTreeSet<_>>();

        for range in &self.source_map_refs {
            if range.start_byte > range.end_byte {
                return Err(SectionCodecError::NonCanonicalTable(
                    "view_style_source_range_order",
                ));
            }
            if !valid_owners.contains(public_ids.get(range.source)?) {
                return Err(SectionCodecError::NonCanonicalTable(
                    "view_style_source_range_owners",
                ));
            }
        }

        for sheet in self.program.sheets() {
            let owner = sheet.id().public_id().as_str();
            let sources = sheet.tokens().iter().map(ViewStyleToken::source).chain(
                sheet.rules().iter().flat_map(|rule| {
                    std::iter::once(rule.source()).chain(
                        rule.declarations()
                            .iter()
                            .map(arcweft_view::style::ViewStyleDeclaration::source),
                    )
                }),
            );
            for source in sources {
                if self.source_owner(&public_ids, source)? != owner {
                    return Err(SectionCodecError::NonCanonicalTable(
                        "view_style_sheet_source_map_owner",
                    ));
                }
            }
        }

        for patch in self.program.patches() {
            for source in patch
                .declarations()
                .iter()
                .map(arcweft_view::style::ViewStyleDeclaration::source)
            {
                if self.source_owner(&public_ids, source)? != self.style_program_id {
                    return Err(SectionCodecError::NonCanonicalTable(
                        "view_style_patch_source_map_owner",
                    ));
                }
            }
        }
        Ok(())
    }

    fn source_owner<'a>(
        &self,
        public_ids: &'a PublicIdTable,
        source: ViewStyleSourceId,
    ) -> Result<&'a str, SectionCodecError> {
        let range = self.source_map_refs.get(source.value() as usize).ok_or(
            SectionCodecError::NonCanonicalTable("view_style_source_ids"),
        )?;
        public_ids.get(range.source)
    }
}

impl ViewTextResource {
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        section.validate(&ViewResourceBudget::default())?;
        encode_view_section(
            ProductSectionCodecKind::ViewText,
            "view_text",
            &section,
            section.public_ids(),
            section.record_count(),
            &ViewResourceBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, ViewResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: ViewResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let (mut section, transcript): (Self, _) = decode_view_section(
            bytes,
            ProductSectionCodecKind::ViewText,
            "view_text",
            &budget,
            Self::public_ids,
            Self::record_count,
        )?;
        section.canonicalize();
        section.validate(&budget)?;
        validate_canonical_view_transcript(&transcript, &section)?;
        Ok(section)
    }

    pub fn canonical_digest(&self) -> Result<BundleDigest, SectionCodecError> {
        self.encode_canonical_section()
            .map(|bytes| BundleDigest::of(&bytes))
    }

    pub fn export_json_bytes(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        let digest = section.canonical_digest()?;
        export_json_bytes(ProductSectionCodecKind::ViewText, &section, digest)
    }

    pub fn compatibility_with(&self, next: &Self) -> ViewResourceCompatibility {
        if self == next {
            return ViewResourceCompatibility::ContentOnly;
        }
        if self.redactions != next.redactions {
            return ViewResourceCompatibility::RestartRequired;
        }
        ViewResourceCompatibility::ContentOnly
    }

    fn canonicalize(&mut self) {
        self.sources
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.localized
            .sort_by(|left, right| (&left.key, &left.locale).cmp(&(&right.key, &right.locale)));
        self.rich_text_documents
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.display_frames
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.reveal_policies
            .sort_by(|left, right| left.text_source.cmp(&right.text_source));
        self.cursor_policies
            .sort_by(|left, right| left.text_source.cmp(&right.text_source));
        self.redactions
            .sort_by(|left, right| left.text_source.cmp(&right.text_source));
    }

    fn validate(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
        let text_record_count = self
            .sources
            .len()
            .saturating_add(self.localized.len())
            .saturating_add(self.rich_text_documents.len())
            .saturating_add(self.display_frames.len());
        check_budget(text_record_count, budget.text_sources, "view_text_sources")?;
        check_budget(
            self.source_ranges.len(),
            budget.source_map_refs,
            "view_text_source_ranges",
        )?;
        reject_duplicates(
            self.sources.iter().map(|source| source.public_id.clone()),
            "view_text_sources",
        )?;
        reject_duplicates(
            self.localized.iter().map(|entry| {
                format!(
                    "{}\u{0}{}",
                    entry.key,
                    entry.locale.as_deref().unwrap_or_default()
                )
            }),
            "view_localized_text",
        )?;
        reject_duplicates(
            self.rich_text_documents
                .iter()
                .map(|document| document.public_id.clone()),
            "view_rich_text_documents",
        )?;
        reject_duplicates(
            self.display_frames
                .iter()
                .map(|frame| frame.public_id.clone()),
            "view_display_frames",
        )?;
        reject_duplicates(
            self.redactions
                .iter()
                .map(|redaction| redaction.text_source.clone()),
            "view_text_redactions",
        )?;
        let valid_sources = self.sources.iter().all(|source| match &source.kind {
            ViewTextSourceKind::Projection { path } => {
                !path.is_empty() && path.iter().all(|segment| valid_identifier(segment))
            }
            ViewTextSourceKind::Local { name } => valid_identifier(name),
            ViewTextSourceKind::RichTextDocument { document } => self
                .rich_text_documents
                .iter()
                .any(|entry| entry.public_id == *document),
            ViewTextSourceKind::DisplayFrame { frame } => self
                .display_frames
                .iter()
                .any(|entry| entry.public_id == *frame),
            ViewTextSourceKind::Dialogue { parameter, .. } => valid_identifier(parameter),
            ViewTextSourceKind::Literal { .. } | ViewTextSourceKind::Localized { .. } => true,
        });
        let valid_display_frames = self.display_frames.iter().all(|entry| {
            usize::try_from(entry.stage_index)
                .ok()
                .and_then(|index| entry.frame.stage(index))
                .is_some()
                && entry.frame.validate().is_ok()
        });
        if valid_sources && valid_display_frames {
            Ok(())
        } else {
            Err(SectionCodecError::NonCanonicalTable("view_text_projection"))
        }
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(
            self.sources
                .iter()
                .flat_map(|source| {
                    [Some(source.public_id.clone())]
                        .into_iter()
                        .chain(text_source_kind_public_ids(&source.kind).map(Some))
                        .flatten()
                })
                .chain(self.localized.iter().flat_map(|entry| {
                    [Some(entry.key.clone()), entry.locale.clone()]
                        .into_iter()
                        .flatten()
                }))
                .chain(
                    self.rich_text_documents
                        .iter()
                        .map(|document| document.public_id.clone()),
                )
                .chain(
                    self.display_frames
                        .iter()
                        .map(|frame| frame.public_id.clone()),
                )
                .chain(
                    self.reveal_policies
                        .iter()
                        .map(|policy| policy.text_source.clone()),
                )
                .chain(
                    self.cursor_policies
                        .iter()
                        .map(|policy| policy.text_source.clone()),
                )
                .chain(
                    self.redactions
                        .iter()
                        .map(|redaction| redaction.text_source.clone()),
                ),
        )
    }

    fn record_count(&self) -> u32 {
        saturating_u32(
            self.sources
                .len()
                .saturating_add(self.localized.len())
                .saturating_add(self.rich_text_documents.len())
                .saturating_add(self.display_frames.len()),
        )
    }
}

impl ViewInputResource {
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        section.validate(&ViewResourceBudget::default())?;
        encode_view_section(
            ProductSectionCodecKind::ViewInput,
            "view_input",
            &section,
            section.public_ids(),
            section.record_count(),
            &ViewResourceBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, ViewResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: ViewResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let (mut section, transcript): (Self, _) = decode_view_section(
            bytes,
            ProductSectionCodecKind::ViewInput,
            "view_input",
            &budget,
            Self::public_ids,
            Self::record_count,
        )?;
        section.canonicalize();
        section.validate(&budget)?;
        validate_canonical_view_transcript(&transcript, &section)?;
        Ok(section)
    }

    pub fn canonical_digest(&self) -> Result<BundleDigest, SectionCodecError> {
        self.encode_canonical_section()
            .map(|bytes| BundleDigest::of(&bytes))
    }

    pub fn export_json_bytes(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        let digest = section.canonical_digest()?;
        export_json_bytes(ProductSectionCodecKind::ViewInput, &section, digest)
    }

    pub fn compatibility_with(&self, next: &Self) -> ViewResourceCompatibility {
        if self == next {
            return ViewResourceCompatibility::ContentOnly;
        }
        if self.adapter_requirements != next.adapter_requirements {
            return ViewResourceCompatibility::RestartRequired;
        }
        self.options
            .iter()
            .fold(
                ViewResourceCompatibility::ContentOnly,
                |compatibility, old| {
                    let next_option = next
                        .options
                        .iter()
                        .find(|candidate| candidate.public_id == old.public_id);
                    compatibility.max(
                        next_option.map_or(ViewResourceCompatibility::RestartRequired, |new| {
                            old.compatibility_with(new)
                        }),
                    )
                },
            )
            .max(if self.options.len() == next.options.len() {
                ViewResourceCompatibility::ContentOnly
            } else {
                ViewResourceCompatibility::RestartRequired
            })
    }

    fn canonicalize(&mut self) {
        self.options
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
    }

    fn validate(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
        check_budget(
            self.options.len(),
            budget.input_options,
            "view_input_options",
        )?;
        reject_duplicates(
            self.options.iter().map(|option| option.public_id.clone()),
            "view_input_options",
        )
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(self.options.iter().flat_map(ViewInputOptions::public_ids))
    }

    fn record_count(&self) -> u32 {
        saturating_u32(self.options.len())
    }
}

impl ViewInputOptions {
    fn compatibility_with(&self, next: &Self) -> ViewResourceCompatibility {
        if self.kind != next.kind
            || self.purpose != next.purpose
            || self.multiline != next.multiline
            || self.selection_policy != next.selection_policy
            || self.shortcut_policy != next.shortcut_policy
            || self.tab_policy != next.tab_policy
            || self.vertical_navigation_policy != next.vertical_navigation_policy
            || self.secure_policy != next.secure_policy
            || self.adapter_requirements != next.adapter_requirements
        {
            return ViewResourceCompatibility::RestartRequired;
        }
        ViewResourceCompatibility::ContentOnly
    }

    fn public_ids(&self) -> Vec<String> {
        [
            Some(self.public_id.clone()),
            self.view.clone(),
            self.containing_scroll_region.clone(),
            Some(self.value_text_source.clone()),
            self.placeholder_text_source.clone(),
            self.submit_handler.clone(),
            self.change_handler.clone(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ViewThemeResource {
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        section.validate(&ViewResourceBudget::default())?;
        encode_view_section(
            ProductSectionCodecKind::ViewTheme,
            "view_theme",
            &section,
            section.public_ids(),
            section.record_count(),
            &ViewResourceBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, ViewResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: ViewResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let (mut section, transcript): (Self, _) = decode_view_section(
            bytes,
            ProductSectionCodecKind::ViewTheme,
            "view_theme",
            &budget,
            Self::public_ids,
            Self::record_count,
        )?;
        section.canonicalize();
        section.validate(&budget)?;
        validate_canonical_view_transcript(&transcript, &section)?;
        Ok(section)
    }

    pub fn canonical_digest(&self) -> Result<BundleDigest, SectionCodecError> {
        self.encode_canonical_section()
            .map(|bytes| BundleDigest::of(&bytes))
    }

    pub fn export_json_bytes(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        let digest = section.canonical_digest()?;
        export_json_bytes(ProductSectionCodecKind::ViewTheme, &section, digest)
    }

    pub fn compatibility_with(&self, _next: &Self) -> ViewResourceCompatibility {
        ViewResourceCompatibility::ContentOnly
    }

    fn canonicalize(&mut self) {
        self.palette_overrides
            .sort_by(|left, right| left.color.source_name().cmp(right.color.source_name()));
        self.dark_mode_visual_golden_ids.sort();
    }

    fn validate(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
        check_budget(
            self.palette_overrides.len(),
            budget.palette_entries,
            "view_theme_palette_entries",
        )?;
        reject_duplicates(
            self.palette_overrides
                .iter()
                .map(|override_entry| format!("{:?}", override_entry.color)),
            "view_theme_palette_entries",
        )
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(self.dark_mode_visual_golden_ids.clone())
    }

    fn record_count(&self) -> u32 {
        saturating_u32(self.palette_overrides.len())
    }
}

fn validate_optional_program(
    inventory: &ViewValueProgramInventory,
    id: Option<ViewValueProgramId>,
    expected: Option<FxRuntimeType>,
) -> Result<(), SectionCodecError> {
    id.map_or(Ok(()), |id| validate_program(inventory, id, expected))
}

fn validate_program(
    inventory: &ViewValueProgramInventory,
    id: ViewValueProgramId,
    expected: Option<FxRuntimeType>,
) -> Result<(), SectionCodecError> {
    let program = inventory
        .get(id)
        .ok_or(SectionCodecError::NonCanonicalTable(
            "view_value_program_reference",
        ))?;
    if expected.is_some_and(|expected| expected != program.return_type()) {
        return Err(SectionCodecError::NonCanonicalTable(
            "view_value_program_return_type",
        ));
    }
    Ok(())
}

fn reject_duplicates(
    values: impl IntoIterator<Item = String>,
    table: &'static str,
) -> Result<(), SectionCodecError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.clone()) {
            return Err(SectionCodecError::DuplicatePublicId(format!(
                "{table}:{value}"
            )));
        }
    }
    Ok(())
}

fn reject_duplicate_keys<T: Ord>(
    values: impl IntoIterator<Item = T>,
    table: &'static str,
) -> Result<(), SectionCodecError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(SectionCodecError::DuplicatePublicId(table.to_owned()));
        }
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

fn valid_resource_identity(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('#')
        && !value.chars().any(char::is_whitespace)
        && !value.chars().any(char::is_control)
}

fn valid_value_input_source(source: &ViewValueInputSource) -> bool {
    match source {
        ViewValueInputSource::DefinitionParameter { view, name } => {
            !view.is_empty() && valid_identifier(name)
        }
        ViewValueInputSource::Projection { path } => {
            !path.is_empty() && path.iter().all(|segment| valid_identifier(segment))
        }
        ViewValueInputSource::LifetimeProjection { scope, path } => {
            valid_identifier(scope)
                && !path.is_empty()
                && path.iter().all(|segment| valid_identifier(segment))
        }
        ViewValueInputSource::Local { view, name } => !view.is_empty() && valid_identifier(name),
        ViewValueInputSource::RepeatOrdinal { view, binding } => {
            !view.is_empty() && valid_identifier(binding)
        }
    }
}

fn unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn instruction_public_ids(instruction: &ViewProgramInstruction) -> Vec<String> {
    match instruction {
        ViewProgramInstruction::OpenElement {
            target,
            styles,
            part,
            ..
        } => target
            .iter()
            .cloned()
            .chain(part.iter().map(|part| part.public_id().as_str().to_owned()))
            .chain(style_apply_public_ids(styles))
            .collect(),
        ViewProgramInstruction::CloseElement
        | ViewProgramInstruction::Branch { .. }
        | ViewProgramInstruction::RepeatKeyed { .. }
        | ViewProgramInstruction::Await { .. }
        | ViewProgramInstruction::BindLocal { .. }
        | ViewProgramInstruction::ApplyFx { .. } => Vec::new(),
        ViewProgramInstruction::EmitText {
            text_source,
            text_block,
            styles,
            part,
            ..
        } => [
            Some(text_source.clone()),
            Some(text_block.clone()),
            part.as_ref()
                .map(|part| part.public_id().as_str().to_owned()),
        ]
        .into_iter()
        .flatten()
        .chain(style_apply_public_ids(styles))
        .collect(),
        ViewProgramInstruction::EmitImage {
            image,
            target,
            styles,
            part,
            ..
        } => [
            Some(image.clone()),
            target.clone(),
            part.as_ref()
                .map(|part| part.public_id().as_str().to_owned()),
        ]
        .into_iter()
        .flatten()
        .chain(style_apply_public_ids(styles))
        .collect(),
        ViewProgramInstruction::EmitCustom {
            element,
            styles,
            part,
            ..
        } => [
            Some(element.clone()),
            part.as_ref()
                .map(|part| part.public_id().as_str().to_owned()),
        ]
        .into_iter()
        .flatten()
        .chain(style_apply_public_ids(styles))
        .collect(),
        ViewProgramInstruction::CallView {
            view, styles, part, ..
        } => [
            Some(view.clone()),
            part.as_ref()
                .map(|part| part.public_id().as_str().to_owned()),
        ]
        .into_iter()
        .flatten()
        .chain(style_apply_public_ids(styles))
        .collect(),
        ViewProgramInstruction::BindHandler { event, handler, .. } => {
            vec![event.clone(), handler.clone()]
        }
        ViewProgramInstruction::AttachSemantic {
            target,
            label_text_source,
            ..
        } => [Some(target.clone()), label_text_source.clone()]
            .into_iter()
            .flatten()
            .collect(),
    }
}

fn style_apply_public_ids(
    styles: &[ViewStyleApplicationTarget],
) -> impl Iterator<Item = String> + '_ {
    styles.iter().filter_map(|style| match style {
        ViewStyleApplicationTarget::Named { sheet } => Some(sheet.public_id().as_str().to_owned()),
        ViewStyleApplicationTarget::Inline { .. } => None,
    })
}

fn action_payload_refs(
    payload: Option<&super::model::ViewActionPayloadResource>,
) -> impl Iterator<Item = String> + '_ {
    payload.into_iter().filter_map(|payload| match payload {
        super::model::ViewActionPayloadResource::LiteralString { .. } => None,
        super::model::ViewActionPayloadResource::TextControlProjection { input, .. } => {
            Some(input.clone())
        }
    })
}

fn style_rule_public_ids(rule: &arcweft_view::style::ViewStyleRule) -> Vec<String> {
    style_selector_public_ids(rule.selector())
        .chain(
            rule.declarations()
                .iter()
                .flat_map(|declaration| style_value_public_ids(declaration.value())),
        )
        .collect()
}

fn style_selector_public_ids(selector: &ViewStyleSelector) -> impl Iterator<Item = String> + '_ {
    selector.sequences().iter().filter_map(|sequence| {
        sequence
            .part()
            .map(|part| part.public_id().as_str().to_owned())
    })
}

fn style_value_public_ids(value: &ViewSpecifiedValue) -> Vec<String> {
    match value {
        ViewSpecifiedValue::Token { token, .. } => {
            vec![token.public_id().as_str().to_owned()]
        }
        ViewSpecifiedValue::Resource { value } => vec![value.as_str().to_owned()],
        ViewSpecifiedValue::BoxAxes { .. }
        | ViewSpecifiedValue::Bool { .. }
        | ViewSpecifiedValue::Integer { .. }
        | ViewSpecifiedValue::Ratio { .. }
        | ViewSpecifiedValue::Scalar { .. }
        | ViewSpecifiedValue::Length { .. }
        | ViewSpecifiedValue::Angle { .. }
        | ViewSpecifiedValue::Color { .. }
        | ViewSpecifiedValue::FontFamilyList { .. }
        | ViewSpecifiedValue::FontWeight { .. }
        | ViewSpecifiedValue::FontStyle { .. }
        | ViewSpecifiedValue::Display { .. }
        | ViewSpecifiedValue::Position { .. }
        | ViewSpecifiedValue::Overflow { .. }
        | ViewSpecifiedValue::FlexDirection { .. }
        | ViewSpecifiedValue::FlexWrap { .. }
        | ViewSpecifiedValue::Alignment { .. }
        | ViewSpecifiedValue::BorderRadii { .. }
        | ViewSpecifiedValue::ShadowList { .. }
        | ViewSpecifiedValue::FilterList { .. }
        | ViewSpecifiedValue::Clip { .. }
        | ViewSpecifiedValue::Mask { .. }
        | ViewSpecifiedValue::BlendMode { .. }
        | ViewSpecifiedValue::Transition { .. } => Vec::new(),
    }
}

fn style_token_depth(sheet: &ViewStyleSheet, token: &ViewStyleToken) -> usize {
    let mut depth = 1_usize;
    let mut current = token;
    while let Some((referenced, _)) = current.value().token_reference() {
        let Some(next) = sheet.token(referenced) else {
            break;
        };
        depth = depth.saturating_add(1);
        current = next;
    }
    depth
}

fn text_source_kind_public_ids(kind: &ViewTextSourceKind) -> impl Iterator<Item = String> + '_ {
    match kind {
        ViewTextSourceKind::Literal { .. }
        | ViewTextSourceKind::Projection { .. }
        | ViewTextSourceKind::Local { .. }
        | ViewTextSourceKind::Dialogue { .. } => Vec::new(),
        ViewTextSourceKind::RichTextDocument { document } => vec![document.clone()],
        ViewTextSourceKind::DisplayFrame { frame } => vec![frame.clone()],
        ViewTextSourceKind::Localized { key, locale } => [Some(key.clone()), locale.clone()]
            .into_iter()
            .flatten()
            .collect(),
    }
    .into_iter()
}
