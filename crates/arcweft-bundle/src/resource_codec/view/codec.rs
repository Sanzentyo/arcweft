use crate::container::BundleDigest;
use arcweft_presentation::fx::FxRuntimeType;
use arcweft_view::{ViewValueProgramId, ViewValueProgramInventory};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::resource_codec::budget::{SectionCodecBudget, check_budget};
use crate::resource_codec::error::SectionCodecError;
use crate::resource_codec::field::{
    FieldId, FieldRegistry, FieldRequirement, FieldSpec, ResourceField, ResourceWireType,
};
use crate::resource_codec::header::PRODUCT_SECTION_SCHEMA_VERSION;
use crate::resource_codec::kind::ProductSectionCodecKind;
use crate::resource_codec::table::{EnumRegistry, EnumSymbol, PublicIdTable, StringTable};
use crate::resource_codec::wire::ProductResourceEnvelope;

use super::compat::ViewResourceCompatibility;
use super::model::{
    ViewInputOptions, ViewInputResource, ViewProgramInstruction, ViewProgramResource,
    ViewStyleApplyRef, ViewStyleResource, ViewStyleRule, ViewStyleSelector, ViewStyleSelectorPart,
    ViewStyleValue, ViewTextResource, ViewTextSourceKind, ViewThemeResource,
    ViewValueInputNamespace, ViewValueInputSource,
};

const FIELD_VIEW_TRANSCRIPT: FieldId = FieldId(1);

/// Decode limits for migrated View resource families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewResourceBudget {
    pub common: SectionCodecBudget,
    pub value_programs: usize,
    pub value_program_instructions: usize,
    pub value_inputs: usize,
    pub program_instructions: usize,
    pub fx_arguments: usize,
    pub child_spans: usize,
    pub handlers: usize,
    pub state_schema_hashes: usize,
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
    pub selector_depth: usize,
    pub part_count: usize,
    pub environment_predicates: usize,
    pub source_map_refs: usize,
    pub external_css_descriptors: usize,
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
            child_spans: 262_144,
            handlers: 65_536,
            state_schema_hashes: 65_536,
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
            selector_depth: 32,
            part_count: 65_536,
            environment_predicates: 65_536,
            source_map_refs: 262_144,
            external_css_descriptors: 65_536,
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
        let mut section: Self = decode_view_section(
            bytes,
            ProductSectionCodecKind::ViewProgram,
            "view_program",
            &budget,
        )?;
        section.canonicalize();
        section.validate(&budget)?;
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
        if self.state_schema_hashes != next.state_schema_hashes || self.handlers != next.handlers {
            return ViewResourceCompatibility::CodeGenerational;
        }
        ViewResourceCompatibility::ContentOnly
    }

    fn canonicalize(&mut self) {
        self.value_programs
            .sort_by_key(arcweft_view::ViewValueProgram::id);
        self.value_inputs
            .sort_by_key(|input| (input.namespace, input.slot));
        for instruction in &mut self.instructions {
            if let ViewProgramInstruction::ApplyFx { arguments, .. } = instruction {
                arguments.sort_by(|left, right| left.parameter.cmp(&right.parameter));
            }
        }
        self.handlers
            .sort_by(|left, right| left.handler_id.cmp(&right.handler_id));
        self.state_schema_hashes.sort_by(|left, right| {
            left.public_id
                .cmp(&right.public_id)
                .then_with(|| left.hash.as_bytes().cmp(&right.hash.as_bytes()))
        });
        self.exported_parts
            .sort_by(|left, right| left.part_id.cmp(&right.part_id));
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
        self.validate_value_programs()?;
        self.validate_child_spans()?;
        self.validate_control_flow_spans()?;
        self.validate_unique_ids()?;
        self.validate_layout_bounds()?;
        self.validate_scroll_regions()?;
        self.validate_surfaces()?;
        self.validate_text_blocks()?;
        self.validate_focus_targets()?;
        self.validate_fx_applications()
    }

    fn validate_budgets(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
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
            self.child_spans.len(),
            budget.child_spans,
            "view_child_spans",
        )?;
        check_budget(self.handlers.len(), budget.handlers, "view_handlers")?;
        check_budget(
            self.state_schema_hashes.len(),
            budget.state_schema_hashes,
            "view_state_schema_hashes",
        )?;
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
        Ok(())
    }

    fn validate_value_programs(&self) -> Result<(), SectionCodecError> {
        let inventory = ViewValueProgramInventory::from_programs(self.value_programs.clone())
            .map_err(|_| SectionCodecError::NonCanonicalTable("view_value_program_inventory"))?;
        self.validate_value_inputs(&inventory)?;
        for instruction in &self.instructions {
            match instruction {
                ViewProgramInstruction::CallView { arguments, .. } => {
                    for argument in arguments {
                        validate_program(&inventory, argument.value_program, None)?;
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
                | ViewProgramInstruction::ApplyStyle { .. }
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
            if !slots.insert(input.slot)
                || types.get(usize::from(input.slot)).copied() != Some(input.value_type)
                || !valid_value_input_source(&input.source)
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

    fn validate_child_spans(&self) -> Result<(), SectionCodecError> {
        self.child_spans.iter().try_for_each(|span| {
            if span.start_instruction > span.end_instruction
                || span.end_instruction as usize > self.instructions.len()
            {
                Err(SectionCodecError::NonCanonicalTable("view_child_spans"))
            } else {
                Ok(())
            }
        })
    }

    fn validate_control_flow_spans(&self) -> Result<(), SectionCodecError> {
        for (index, instruction) in self.instructions.iter().enumerate() {
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
                    .is_some_and(|end| end <= self.instructions.len())
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
        reject_duplicates(
            self.exported_parts.iter().map(|part| part.part_id.clone()),
            "view_exported_parts",
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
        if self
            .text_blocks
            .iter()
            .all(super::model::ViewTextBlockResource::is_valid)
        {
            Ok(())
        } else {
            Err(SectionCodecError::NonCanonicalTable("view_text_blocks"))
        }
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
        for instruction in &self.instructions {
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

    fn public_ids(&self) -> Vec<String> {
        unique_strings(
            [self.program_id.clone(), self.root_view.clone()]
                .into_iter()
                .chain(self.instructions.iter().flat_map(instruction_public_ids))
                .chain(
                    self.handlers
                        .iter()
                        .flat_map(|handler| [handler.handler_id.clone(), handler.event.clone()]),
                )
                .chain(
                    self.state_schema_hashes
                        .iter()
                        .filter_map(|schema| schema.public_id.clone()),
                )
                .chain(
                    self.exported_parts
                        .iter()
                        .flat_map(|part| [part.part_id.clone(), part.public_name.clone()]),
                )
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
        super::model::ViewActionButtonActionResource::Noop => Vec::new(),
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
        surface.style.clone(),
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
        section.canonicalize();
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
        let mut section: Self = decode_view_section(
            bytes,
            ProductSectionCodecKind::ViewStyle,
            "view_style",
            &budget,
        )?;
        section.canonicalize();
        section.validate(&budget)?;
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

    fn canonicalize(&mut self) {
        self.arcweft_sources
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.css_sources
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.tokens
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.external_css_descriptors
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.part_rules
            .sort_by(|left, right| left.part.cmp(&right.part));
    }

    fn validate(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
        check_budget(self.rules.len(), budget.style_rules, "view_style_rules")?;
        check_budget(
            self.part_rules.len(),
            budget.style_rules,
            "view_style_part_rules",
        )?;
        check_budget(self.tokens.len(), budget.style_tokens, "view_style_tokens")?;
        check_budget(
            self.environment_predicates.len(),
            budget.environment_predicates,
            "view_style_environment_predicates",
        )?;
        check_budget(
            self.source_map_refs.len(),
            budget.source_map_refs,
            "view_style_source_map_refs",
        )?;
        check_budget(
            self.external_css_descriptors.len(),
            budget.external_css_descriptors,
            "view_external_css_descriptors",
        )?;
        self.rules
            .iter()
            .map(|rule| &rule.selector)
            .chain(self.part_rules.iter().map(|rule| &rule.selector))
            .try_for_each(|selector| {
                check_budget(
                    selector.max_depth(),
                    budget.selector_depth,
                    "view_selector_depth",
                )
            })?;
        let part_count = self
            .part_rules
            .iter()
            .map(|rule| rule.part.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        check_budget(part_count, budget.part_count, "view_style_part_count")?;
        reject_duplicates(
            self.tokens.iter().map(|token| token.public_id.clone()),
            "view_style_tokens",
        )?;
        reject_duplicates(
            self.external_css_descriptors
                .iter()
                .map(|descriptor| descriptor.public_id.clone()),
            "view_external_css_descriptors",
        )
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(
            [self.style_program_id.clone()]
                .into_iter()
                .chain(
                    self.arcweft_sources
                        .iter()
                        .chain(self.css_sources.iter())
                        .map(|source| source.public_id.clone()),
                )
                .chain(self.tokens.iter().flat_map(|token| {
                    [Some(token.public_id.clone())]
                        .into_iter()
                        .chain(style_value_public_ids(&token.value).into_iter().map(Some))
                        .flatten()
                }))
                .chain(self.rules.iter().flat_map(style_rule_public_ids))
                .chain(self.part_rules.iter().flat_map(|rule| {
                    [Some(rule.part.clone())]
                        .into_iter()
                        .chain(style_selector_public_ids(&rule.selector).map(Some))
                        .chain(rule.declarations.iter().flat_map(|declaration| {
                            [Some(declaration.property.clone())].into_iter().chain(
                                style_value_public_ids(&declaration.value)
                                    .into_iter()
                                    .map(Some),
                            )
                        }))
                        .flatten()
                }))
                .chain(
                    self.external_css_descriptors
                        .iter()
                        .map(|descriptor| descriptor.public_id.clone()),
                ),
        )
    }

    fn record_count(&self) -> u32 {
        saturating_u32(self.rules.len().saturating_add(self.part_rules.len()))
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
        let mut section: Self = decode_view_section(
            bytes,
            ProductSectionCodecKind::ViewText,
            "view_text",
            &budget,
        )?;
        section.canonicalize();
        section.validate(&budget)?;
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
        self.reveal_policies
            .sort_by(|left, right| left.text_source.cmp(&right.text_source));
        self.cursor_policies
            .sort_by(|left, right| left.text_source.cmp(&right.text_source));
        self.redactions
            .sort_by(|left, right| left.text_source.cmp(&right.text_source));
    }

    fn validate(&self, budget: &ViewResourceBudget) -> Result<(), SectionCodecError> {
        check_budget(self.sources.len(), budget.text_sources, "view_text_sources")?;
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
            self.redactions
                .iter()
                .map(|redaction| redaction.text_source.clone()),
            "view_text_redactions",
        )?;
        if self.sources.iter().all(|source| match &source.kind {
            ViewTextSourceKind::Projection { path } => {
                !path.is_empty() && path.iter().all(|segment| valid_identifier(segment))
            }
            ViewTextSourceKind::Local { name } => valid_identifier(name),
            ViewTextSourceKind::Literal { .. }
            | ViewTextSourceKind::Localized { .. }
            | ViewTextSourceKind::RichTextDocument { .. }
            | ViewTextSourceKind::DisplayFrame { .. } => true,
        }) {
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
        saturating_u32(self.sources.len())
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
        let mut section: Self = decode_view_section(
            bytes,
            ProductSectionCodecKind::ViewInput,
            "view_input",
            &budget,
        )?;
        section.canonicalize();
        section.validate(&budget)?;
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
        let mut section: Self = decode_view_section(
            bytes,
            ProductSectionCodecKind::ViewTheme,
            "view_theme",
            &budget,
        )?;
        section.canonicalize();
        section.validate(&budget)?;
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
            .sort_by_key(|override_| override_.color);
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

fn encode_view_section<T>(
    codec: ProductSectionCodecKind,
    family_label: &'static str,
    value: &T,
    public_ids: impl IntoIterator<Item = String>,
    record_count: u32,
    budget: &ViewResourceBudget,
) -> Result<Vec<u8>, SectionCodecError>
where
    T: Serialize,
{
    let transcript = serde_json::to_vec(value)
        .map_err(|_| SectionCodecError::NonCanonicalTable(family_label))?;
    check_budget(
        transcript.len(),
        budget.transcript_bytes,
        "view_transcript_bytes",
    )?;
    let strings = StringTable::with_budget(
        [
            family_label.to_owned(),
            "canonical_view_resource_transcript_v1".to_owned(),
        ],
        budget.common,
    )?;
    let public_ids = PublicIdTable::with_budget(unique_strings(public_ids), budget.common)?;
    let enums = EnumRegistry::with_budget(
        [EnumSymbol {
            code: 1,
            name: strings
                .id_for(family_label)
                .ok_or(SectionCodecError::NonCanonicalTable(family_label))?,
        }],
        &strings,
        budget.common,
    )?;
    let field = ResourceField::new(
        FIELD_VIEW_TRANSCRIPT,
        FieldRequirement::Required,
        ResourceWireType::Bytes,
        1,
        u16::try_from(public_ids.len()).map_err(|_| SectionCodecError::LengthOverflow)?,
        transcript,
    );
    ProductResourceEnvelope::with_budget(
        codec,
        strings,
        public_ids,
        enums,
        [field],
        record_count,
        budget.common,
    )?
    .encode_canonical()
}

fn decode_view_section<T>(
    bytes: &[u8],
    codec: ProductSectionCodecKind,
    family_label: &'static str,
    budget: &ViewResourceBudget,
) -> Result<T, SectionCodecError>
where
    T: for<'de> Deserialize<'de>,
{
    let decoded = ProductResourceEnvelope::decode_with_registry(
        bytes,
        codec,
        &view_registry()?,
        budget.common,
    )?;
    let field = decoded
        .envelope
        .fields
        .iter()
        .find(|field| field.id == FIELD_VIEW_TRANSCRIPT)
        .ok_or(SectionCodecError::MissingRequiredField(
            FIELD_VIEW_TRANSCRIPT,
        ))?;
    check_budget(
        field.payload.len(),
        budget.transcript_bytes,
        "view_transcript_bytes",
    )?;
    serde_json::from_slice(&field.payload)
        .map_err(|_| SectionCodecError::NonCanonicalTable(family_label))
}

fn export_json_bytes<T>(
    codec: ProductSectionCodecKind,
    resource: &T,
    canonical_digest: BundleDigest,
) -> Result<Vec<u8>, SectionCodecError>
where
    T: Clone + Serialize,
{
    let export = ViewResourceExport {
        schema_version: PRODUCT_SECTION_SCHEMA_VERSION,
        codec,
        codec_name: codec.as_str().to_owned(),
        canonical_digest,
        resource: resource.clone(),
    };
    serde_json::to_vec_pretty(&export)
        .map_err(|_| SectionCodecError::NonCanonicalTable("view_export_json"))
}

fn view_registry() -> Result<FieldRegistry, SectionCodecError> {
    FieldRegistry::new([FieldSpec::required(
        FIELD_VIEW_TRANSCRIPT,
        ResourceWireType::Bytes,
    )])
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

fn valid_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

fn valid_value_input_source(source: &ViewValueInputSource) -> bool {
    match source {
        ViewValueInputSource::Projection { path } => {
            !path.is_empty() && path.iter().all(|segment| valid_identifier(segment))
        }
        ViewValueInputSource::LifetimeProjection { scope, path } => {
            valid_identifier(scope)
                && !path.is_empty()
                && path.iter().all(|segment| valid_identifier(segment))
        }
        ViewValueInputSource::Local { name } => valid_identifier(name),
        ViewValueInputSource::RepeatOrdinal { binding } => valid_identifier(binding),
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
            style,
            part,
            ..
        } => option_ids([target, style, part]),
        ViewProgramInstruction::CloseElement
        | ViewProgramInstruction::Branch { .. }
        | ViewProgramInstruction::RepeatKeyed { .. }
        | ViewProgramInstruction::Await { .. }
        | ViewProgramInstruction::BindLocal { .. }
        | ViewProgramInstruction::ApplyFx { .. } => Vec::new(),
        ViewProgramInstruction::EmitText {
            text_source,
            style,
            part,
            ..
        } => [Some(text_source.clone()), style.clone(), part.clone()]
            .into_iter()
            .flatten()
            .collect(),
        ViewProgramInstruction::EmitImage {
            image, style, part, ..
        } => [Some(image.clone()), style.clone(), part.clone()]
            .into_iter()
            .flatten()
            .collect(),
        ViewProgramInstruction::EmitCustom {
            element,
            style,
            part,
            ..
        } => [Some(element.clone()), style.clone(), part.clone()]
            .into_iter()
            .flatten()
            .collect(),
        ViewProgramInstruction::CallView {
            view, style, part, ..
        } => [Some(view.clone()), style.clone(), part.clone()]
            .into_iter()
            .flatten()
            .collect(),
        ViewProgramInstruction::ApplyStyle { style, .. } => match style {
            ViewStyleApplyRef::Named(id) => vec![id.clone()],
            ViewStyleApplyRef::InlineArcweft { .. } | ViewStyleApplyRef::InlineCss { .. } => {
                Vec::new()
            }
        },
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

fn option_ids<const N: usize>(values: [&Option<String>; N]) -> Vec<String> {
    values.iter().filter_map(|value| (*value).clone()).collect()
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

fn style_rule_public_ids(rule: &ViewStyleRule) -> Vec<String> {
    style_selector_public_ids(&rule.selector)
        .chain(rule.declarations.iter().flat_map(|declaration| {
            [Some(declaration.property.clone())]
                .into_iter()
                .chain(
                    style_value_public_ids(&declaration.value)
                        .into_iter()
                        .map(Some),
                )
                .flatten()
        }))
        .collect()
}

fn style_selector_public_ids(selector: &ViewStyleSelector) -> impl Iterator<Item = String> + '_ {
    selector.parts.iter().filter_map(|part| match part {
        ViewStyleSelectorPart::Part(id) => Some(id.clone()),
        _ => None,
    })
}

fn style_value_public_ids(value: &ViewStyleValue) -> Vec<String> {
    match value {
        ViewStyleValue::Token(id) | ViewStyleValue::Resource(id) => vec![id.clone()],
        ViewStyleValue::List(values) => values.iter().flat_map(style_value_public_ids).collect(),
        ViewStyleValue::SystemColor(_)
        | ViewStyleValue::Rgba(_)
        | ViewStyleValue::Milli(_)
        | ViewStyleValue::Text(_)
        | ViewStyleValue::Digest(_) => Vec::new(),
    }
}

fn text_source_kind_public_ids(kind: &ViewTextSourceKind) -> impl Iterator<Item = String> + '_ {
    match kind {
        ViewTextSourceKind::Literal { .. }
        | ViewTextSourceKind::Projection { .. }
        | ViewTextSourceKind::Local { .. }
        | ViewTextSourceKind::RichTextDocument { .. }
        | ViewTextSourceKind::DisplayFrame { .. } => Vec::new(),
        ViewTextSourceKind::Localized { key, locale } => [Some(key.clone()), locale.clone()]
            .into_iter()
            .flatten()
            .collect(),
    }
    .into_iter()
}
