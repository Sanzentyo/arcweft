use crate::container::BundleDigest;
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

use super::compat::UiResourceCompatibility;
use super::model::{
    UiInputOptions, UiInputResource, UiProgramInstruction, UiProgramResource, UiStyleApplyRef,
    UiStyleResource, UiStyleRule, UiStyleSelector, UiStyleSelectorPart, UiStyleValue,
    UiTextResource, UiTextSourceKind, UiThemeResource,
};

const FIELD_UI_TRANSCRIPT: FieldId = FieldId(1);

/// Decode limits for migrated UI resource families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiResourceBudget {
    pub common: SectionCodecBudget,
    pub program_instructions: usize,
    pub child_spans: usize,
    pub handlers: usize,
    pub state_schema_hashes: usize,
    pub exported_parts: usize,
    pub semantic_targets: usize,
    pub layout_bounds: usize,
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
pub struct UiResourceExport<T> {
    pub schema_version: u32,
    pub codec: ProductSectionCodecKind,
    pub codec_name: String,
    pub canonical_digest: BundleDigest,
    pub resource: T,
}

impl Default for UiResourceBudget {
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
            program_instructions: 262_144,
            child_spans: 262_144,
            handlers: 65_536,
            state_schema_hashes: 65_536,
            exported_parts: 65_536,
            semantic_targets: 262_144,
            layout_bounds: 262_144,
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

impl UiProgramResource {
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        section.validate(UiResourceBudget::default())?;
        encode_ui_section(
            ProductSectionCodecKind::UiProgram,
            "ui_program",
            &section,
            section.public_ids(),
            section.record_count(),
            UiResourceBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, UiResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: UiResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let mut section: Self = decode_ui_section(
            bytes,
            ProductSectionCodecKind::UiProgram,
            "ui_program",
            budget,
        )?;
        section.canonicalize();
        section.validate(budget)?;
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
        export_json_bytes(ProductSectionCodecKind::UiProgram, &section, digest)
    }

    pub fn compatibility_with(&self, next: &Self) -> UiResourceCompatibility {
        if self == next {
            return UiResourceCompatibility::ContentOnly;
        }
        if self.adapter_requirements != next.adapter_requirements {
            return UiResourceCompatibility::RestartRequired;
        }
        if self.state_schema_hashes != next.state_schema_hashes || self.handlers != next.handlers {
            return UiResourceCompatibility::CodeGenerational;
        }
        UiResourceCompatibility::ContentOnly
    }

    fn canonicalize(&mut self) {
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
        self.focus_groups
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
        self.focus_navigation
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
    }

    fn validate(&self, budget: UiResourceBudget) -> Result<(), SectionCodecError> {
        self.validate_budgets(budget)?;
        self.validate_child_spans()?;
        self.validate_unique_ids()?;
        self.validate_layout_bounds()?;
        self.validate_focus_targets()
    }

    fn validate_budgets(&self, budget: UiResourceBudget) -> Result<(), SectionCodecError> {
        check_budget(
            self.instructions.len(),
            budget.program_instructions,
            "ui_program_instructions",
        )?;
        check_budget(self.child_spans.len(), budget.child_spans, "ui_child_spans")?;
        check_budget(self.handlers.len(), budget.handlers, "ui_handlers")?;
        check_budget(
            self.state_schema_hashes.len(),
            budget.state_schema_hashes,
            "ui_state_schema_hashes",
        )?;
        check_budget(
            self.exported_parts.len(),
            budget.exported_parts,
            "ui_exported_parts",
        )?;
        check_budget(
            self.semantic_targets.len(),
            budget.semantic_targets,
            "ui_semantic_targets",
        )?;
        check_budget(
            self.layout_bounds.len(),
            budget.layout_bounds,
            "ui_layout_bounds",
        )?;
        check_budget(
            self.action_buttons.len(),
            budget.action_buttons,
            "ui_action_buttons",
        )?;
        check_budget(
            self.focus_groups.len(),
            budget.focus_groups,
            "ui_focus_groups",
        )?;
        check_budget(
            self.focus_navigation.len(),
            budget.focus_targets,
            "ui_focus_navigation",
        )?;
        check_budget(
            self.focus_navigation
                .iter()
                .map(|target| target.edges.len())
                .sum::<usize>(),
            budget.focus_edges,
            "ui_focus_edges",
        )?;
        Ok(())
    }

    fn validate_child_spans(&self) -> Result<(), SectionCodecError> {
        self.child_spans.iter().try_for_each(|span| {
            if span.start_instruction > span.end_instruction
                || span.end_instruction as usize > self.instructions.len()
            {
                Err(SectionCodecError::NonCanonicalTable("ui_child_spans"))
            } else {
                Ok(())
            }
        })
    }

    fn validate_unique_ids(&self) -> Result<(), SectionCodecError> {
        reject_duplicates(
            self.handlers
                .iter()
                .map(|handler| handler.handler_id.clone()),
            "ui_handlers",
        )?;
        reject_duplicates(
            self.exported_parts.iter().map(|part| part.part_id.clone()),
            "ui_exported_parts",
        )?;
        reject_duplicates(
            self.semantic_targets
                .iter()
                .map(|target| target.public_id.clone()),
            "ui_semantic_targets",
        )?;
        reject_duplicates(
            self.layout_bounds
                .iter()
                .map(super::model::UiLayoutBoundsResource::identity_key),
            "ui_layout_bounds",
        )?;
        reject_duplicates(
            self.action_buttons
                .iter()
                .map(|button| button.public_id.clone()),
            "ui_action_buttons",
        )?;
        reject_duplicates(
            self.focus_groups
                .iter()
                .map(|group| group.public_id.clone()),
            "ui_focus_groups",
        )?;
        reject_duplicates(
            self.focus_navigation
                .iter()
                .map(|target| target.public_id.clone()),
            "ui_focus_navigation",
        )
    }

    fn validate_layout_bounds(&self) -> Result<(), SectionCodecError> {
        if self
            .layout_bounds
            .iter()
            .all(super::model::UiLayoutBoundsResource::is_valid)
        {
            Ok(())
        } else {
            Err(SectionCodecError::NonCanonicalTable("ui_layout_bounds"))
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
                    "ui_focus_missing_target",
                ));
            }
        }
        Ok(())
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(
            [self.program_id.clone(), self.root_component.clone()]
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
                .chain(self.semantic_targets.iter().flat_map(|target| {
                    [
                        Some(target.public_id.clone()),
                        Some(target.target.clone()),
                        target.label_text_source.clone(),
                    ]
                    .into_iter()
                    .flatten()
                }))
                .chain(
                    self.layout_bounds
                        .iter()
                        .map(|bounds| bounds.public_id.clone()),
                )
                .chain(self.action_buttons.iter().flat_map(|button| {
                    let action_ids = match &button.action {
                        super::model::UiActionButtonActionResource::TextInputSubmit {
                            input,
                            ..
                        } => Some(input.clone()),
                    };
                    [
                        Some(button.public_id.clone()),
                        Some(button.label_text_source.clone()),
                        action_ids,
                    ]
                    .into_iter()
                    .flatten()
                }))
                .chain(self.focus_groups.iter().flat_map(|group| {
                    [
                        Some(group.public_id.clone()),
                        group.parent.clone(),
                        group.initial.explicit_target().map(ToOwned::to_owned),
                    ]
                    .into_iter()
                    .flatten()
                }))
                .chain(self.focus_navigation.iter().flat_map(|target| {
                    [Some(target.public_id.clone()), target.group.clone()]
                        .into_iter()
                        .flatten()
                        .chain(target.edges.iter().filter_map(|edge| {
                            edge.target.explicit_target().map(ToOwned::to_owned)
                        }))
                })),
        )
    }

    fn record_count(&self) -> u32 {
        saturating_u32(self.instructions.len())
            .saturating_add(saturating_u32(self.layout_bounds.len()))
            .saturating_add(saturating_u32(self.action_buttons.len()))
            .saturating_add(saturating_u32(self.focus_groups.len()))
            .saturating_add(saturating_u32(self.focus_navigation.len()))
    }
}

impl UiStyleResource {
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        section.validate(UiResourceBudget::default())?;
        encode_ui_section(
            ProductSectionCodecKind::UiStyle,
            "ui_style",
            &section,
            section.public_ids(),
            section.record_count(),
            UiResourceBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, UiResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: UiResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let mut section: Self =
            decode_ui_section(bytes, ProductSectionCodecKind::UiStyle, "ui_style", budget)?;
        section.canonicalize();
        section.validate(budget)?;
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
        export_json_bytes(ProductSectionCodecKind::UiStyle, &section, digest)
    }

    pub fn compatibility_with(&self, next: &Self) -> UiResourceCompatibility {
        if self == next {
            return UiResourceCompatibility::ContentOnly;
        }
        if self.adapter_requirements != next.adapter_requirements {
            return UiResourceCompatibility::RestartRequired;
        }
        UiResourceCompatibility::ContentOnly
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

    fn validate(&self, budget: UiResourceBudget) -> Result<(), SectionCodecError> {
        check_budget(self.rules.len(), budget.style_rules, "ui_style_rules")?;
        check_budget(
            self.part_rules.len(),
            budget.style_rules,
            "ui_style_part_rules",
        )?;
        check_budget(self.tokens.len(), budget.style_tokens, "ui_style_tokens")?;
        check_budget(
            self.environment_predicates.len(),
            budget.environment_predicates,
            "ui_style_environment_predicates",
        )?;
        check_budget(
            self.source_map_refs.len(),
            budget.source_map_refs,
            "ui_style_source_map_refs",
        )?;
        check_budget(
            self.external_css_descriptors.len(),
            budget.external_css_descriptors,
            "ui_external_css_descriptors",
        )?;
        self.rules
            .iter()
            .map(|rule| &rule.selector)
            .chain(self.part_rules.iter().map(|rule| &rule.selector))
            .try_for_each(|selector| {
                check_budget(
                    selector.max_depth(),
                    budget.selector_depth,
                    "ui_selector_depth",
                )
            })?;
        let part_count = self
            .part_rules
            .iter()
            .map(|rule| rule.part.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        check_budget(part_count, budget.part_count, "ui_style_part_count")?;
        reject_duplicates(
            self.tokens.iter().map(|token| token.public_id.clone()),
            "ui_style_tokens",
        )?;
        reject_duplicates(
            self.external_css_descriptors
                .iter()
                .map(|descriptor| descriptor.public_id.clone()),
            "ui_external_css_descriptors",
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
                        .chain(style_value_public_id(&token.value).map(Some))
                        .flatten()
                }))
                .chain(self.rules.iter().flat_map(style_rule_public_ids))
                .chain(self.part_rules.iter().flat_map(|rule| {
                    [Some(rule.part.clone())]
                        .into_iter()
                        .chain(style_selector_public_ids(&rule.selector).map(Some))
                        .chain(rule.declarations.iter().flat_map(|declaration| {
                            [Some(declaration.property.clone())]
                                .into_iter()
                                .chain(style_value_public_id(&declaration.value).map(Some))
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

impl UiTextResource {
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        section.validate(UiResourceBudget::default())?;
        encode_ui_section(
            ProductSectionCodecKind::UiText,
            "ui_text",
            &section,
            section.public_ids(),
            section.record_count(),
            UiResourceBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, UiResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: UiResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let mut section: Self =
            decode_ui_section(bytes, ProductSectionCodecKind::UiText, "ui_text", budget)?;
        section.canonicalize();
        section.validate(budget)?;
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
        export_json_bytes(ProductSectionCodecKind::UiText, &section, digest)
    }

    pub fn compatibility_with(&self, next: &Self) -> UiResourceCompatibility {
        if self == next {
            return UiResourceCompatibility::ContentOnly;
        }
        if self.redactions != next.redactions {
            return UiResourceCompatibility::RestartRequired;
        }
        UiResourceCompatibility::ContentOnly
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

    fn validate(&self, budget: UiResourceBudget) -> Result<(), SectionCodecError> {
        check_budget(self.sources.len(), budget.text_sources, "ui_text_sources")?;
        check_budget(
            self.source_ranges.len(),
            budget.source_map_refs,
            "ui_text_source_ranges",
        )?;
        reject_duplicates(
            self.sources.iter().map(|source| source.public_id.clone()),
            "ui_text_sources",
        )?;
        reject_duplicates(
            self.redactions
                .iter()
                .map(|redaction| redaction.text_source.clone()),
            "ui_text_redactions",
        )
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

impl UiInputResource {
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        section.validate(UiResourceBudget::default())?;
        encode_ui_section(
            ProductSectionCodecKind::UiInput,
            "ui_input",
            &section,
            section.public_ids(),
            section.record_count(),
            UiResourceBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, UiResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: UiResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let mut section: Self =
            decode_ui_section(bytes, ProductSectionCodecKind::UiInput, "ui_input", budget)?;
        section.canonicalize();
        section.validate(budget)?;
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
        export_json_bytes(ProductSectionCodecKind::UiInput, &section, digest)
    }

    pub fn compatibility_with(&self, next: &Self) -> UiResourceCompatibility {
        if self == next {
            return UiResourceCompatibility::ContentOnly;
        }
        if self.adapter_requirements != next.adapter_requirements {
            return UiResourceCompatibility::RestartRequired;
        }
        self.options
            .iter()
            .fold(
                UiResourceCompatibility::ContentOnly,
                |compatibility, old| {
                    let next_option = next
                        .options
                        .iter()
                        .find(|candidate| candidate.public_id == old.public_id);
                    compatibility.max(
                        next_option.map_or(UiResourceCompatibility::RestartRequired, |new| {
                            old.compatibility_with(new)
                        }),
                    )
                },
            )
            .max(if self.options.len() == next.options.len() {
                UiResourceCompatibility::ContentOnly
            } else {
                UiResourceCompatibility::RestartRequired
            })
    }

    fn canonicalize(&mut self) {
        self.options
            .sort_by(|left, right| left.public_id.cmp(&right.public_id));
    }

    fn validate(&self, budget: UiResourceBudget) -> Result<(), SectionCodecError> {
        check_budget(self.options.len(), budget.input_options, "ui_input_options")?;
        reject_duplicates(
            self.options.iter().map(|option| option.public_id.clone()),
            "ui_input_options",
        )
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(self.options.iter().flat_map(UiInputOptions::public_ids))
    }

    fn record_count(&self) -> u32 {
        saturating_u32(self.options.len())
    }
}

impl UiInputOptions {
    fn compatibility_with(&self, next: &Self) -> UiResourceCompatibility {
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
            return UiResourceCompatibility::RestartRequired;
        }
        UiResourceCompatibility::ContentOnly
    }

    fn public_ids(&self) -> Vec<String> {
        [
            Some(self.public_id.clone()),
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

impl UiThemeResource {
    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        let mut section = self.clone();
        section.canonicalize();
        section.validate(UiResourceBudget::default())?;
        encode_ui_section(
            ProductSectionCodecKind::UiTheme,
            "ui_theme",
            &section,
            section.public_ids(),
            section.record_count(),
            UiResourceBudget::default(),
        )
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, UiResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: UiResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let mut section: Self =
            decode_ui_section(bytes, ProductSectionCodecKind::UiTheme, "ui_theme", budget)?;
        section.canonicalize();
        section.validate(budget)?;
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
        export_json_bytes(ProductSectionCodecKind::UiTheme, &section, digest)
    }

    pub fn compatibility_with(&self, _next: &Self) -> UiResourceCompatibility {
        UiResourceCompatibility::ContentOnly
    }

    fn canonicalize(&mut self) {
        self.palette_overrides
            .sort_by_key(|override_| override_.color);
        self.dark_mode_visual_golden_ids.sort();
    }

    fn validate(&self, budget: UiResourceBudget) -> Result<(), SectionCodecError> {
        check_budget(
            self.palette_overrides.len(),
            budget.palette_entries,
            "ui_theme_palette_entries",
        )?;
        reject_duplicates(
            self.palette_overrides
                .iter()
                .map(|override_entry| format!("{:?}", override_entry.color)),
            "ui_theme_palette_entries",
        )
    }

    fn public_ids(&self) -> Vec<String> {
        unique_strings(self.dark_mode_visual_golden_ids.clone())
    }

    fn record_count(&self) -> u32 {
        saturating_u32(self.palette_overrides.len())
    }
}

fn encode_ui_section<T>(
    codec: ProductSectionCodecKind,
    family_label: &'static str,
    value: &T,
    public_ids: impl IntoIterator<Item = String>,
    record_count: u32,
    budget: UiResourceBudget,
) -> Result<Vec<u8>, SectionCodecError>
where
    T: Serialize,
{
    let transcript = serde_json::to_vec(value)
        .map_err(|_| SectionCodecError::NonCanonicalTable(family_label))?;
    check_budget(
        transcript.len(),
        budget.transcript_bytes,
        "ui_transcript_bytes",
    )?;
    let strings = StringTable::with_budget(
        [
            family_label.to_owned(),
            "canonical_ui_resource_transcript_v1".to_owned(),
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
        FIELD_UI_TRANSCRIPT,
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

fn decode_ui_section<T>(
    bytes: &[u8],
    codec: ProductSectionCodecKind,
    family_label: &'static str,
    budget: UiResourceBudget,
) -> Result<T, SectionCodecError>
where
    T: for<'de> Deserialize<'de>,
{
    let decoded = ProductResourceEnvelope::decode_with_registry(
        bytes,
        codec,
        &ui_registry()?,
        budget.common,
    )?;
    let field = decoded
        .envelope
        .fields
        .iter()
        .find(|field| field.id == FIELD_UI_TRANSCRIPT)
        .ok_or(SectionCodecError::MissingRequiredField(FIELD_UI_TRANSCRIPT))?;
    check_budget(
        field.payload.len(),
        budget.transcript_bytes,
        "ui_transcript_bytes",
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
    let export = UiResourceExport {
        schema_version: PRODUCT_SECTION_SCHEMA_VERSION,
        codec,
        codec_name: codec.as_str().to_owned(),
        canonical_digest,
        resource: resource.clone(),
    };
    serde_json::to_vec_pretty(&export)
        .map_err(|_| SectionCodecError::NonCanonicalTable("ui_export_json"))
}

fn ui_registry() -> Result<FieldRegistry, SectionCodecError> {
    FieldRegistry::new([FieldSpec::required(
        FIELD_UI_TRANSCRIPT,
        ResourceWireType::Bytes,
    )])
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

fn instruction_public_ids(instruction: &UiProgramInstruction) -> Vec<String> {
    match instruction {
        UiProgramInstruction::OpenElement { style, part, .. } => option_ids([style, part]),
        UiProgramInstruction::CloseElement => Vec::new(),
        UiProgramInstruction::EmitText {
            text_source,
            style,
            part,
            ..
        } => [Some(text_source.clone()), style.clone(), part.clone()]
            .into_iter()
            .flatten()
            .collect(),
        UiProgramInstruction::EmitImage {
            image, style, part, ..
        } => [Some(image.clone()), style.clone(), part.clone()]
            .into_iter()
            .flatten()
            .collect(),
        UiProgramInstruction::EmitCustom {
            element,
            style,
            part,
            ..
        } => [Some(element.clone()), style.clone(), part.clone()]
            .into_iter()
            .flatten()
            .collect(),
        UiProgramInstruction::CallComponent {
            component,
            style,
            part,
            ..
        } => [Some(component.clone()), style.clone(), part.clone()]
            .into_iter()
            .flatten()
            .collect(),
        UiProgramInstruction::Branch { .. } | UiProgramInstruction::RepeatKeyed { .. } => {
            Vec::new()
        }
        UiProgramInstruction::ApplyStyle { style, .. } => match style {
            UiStyleApplyRef::Named(id) => vec![id.clone()],
            UiStyleApplyRef::InlineArcweft { .. } | UiStyleApplyRef::InlineCss { .. } => Vec::new(),
        },
        UiProgramInstruction::BindHandler { event, handler, .. } => {
            vec![event.clone(), handler.clone()]
        }
        UiProgramInstruction::AttachSemantic {
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

fn style_rule_public_ids(rule: &UiStyleRule) -> Vec<String> {
    style_selector_public_ids(&rule.selector)
        .chain(rule.declarations.iter().flat_map(|declaration| {
            [Some(declaration.property.clone())]
                .into_iter()
                .chain(style_value_public_id(&declaration.value).map(Some))
                .flatten()
        }))
        .collect()
}

fn style_selector_public_ids(selector: &UiStyleSelector) -> impl Iterator<Item = String> + '_ {
    selector.parts.iter().filter_map(|part| match part {
        UiStyleSelectorPart::Part(id) => Some(id.clone()),
        _ => None,
    })
}

fn style_value_public_id(value: &UiStyleValue) -> Option<String> {
    match value {
        UiStyleValue::Token(id) | UiStyleValue::Resource(id) => Some(id.clone()),
        UiStyleValue::SystemColor(_)
        | UiStyleValue::Rgba(_)
        | UiStyleValue::Milli(_)
        | UiStyleValue::Text(_)
        | UiStyleValue::Digest(_) => None,
    }
}

fn text_source_kind_public_ids(kind: &UiTextSourceKind) -> impl Iterator<Item = String> + '_ {
    match kind {
        UiTextSourceKind::Literal { .. }
        | UiTextSourceKind::RichTextDocument { .. }
        | UiTextSourceKind::DisplayFrame { .. } => Vec::new(),
        UiTextSourceKind::Localized { key, locale } => [Some(key.clone()), locale.clone()]
            .into_iter()
            .flatten()
            .collect(),
    }
    .into_iter()
}
