//! Production limits for first-class character dialogue values.

/// Maximum sizes accepted by the `CharacterDialogue` domain and runtime
/// boundaries.
///
/// The field widths are part of the runtime contract. Consumers should use
/// [`PRODUCTION_CHARACTER_DIALOGUE_LIMITS`] rather than inventing local policy
/// values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CharacterDialogueLimits {
    pub max_patch_fields: u16,
    pub max_patch_work: u32,
    pub max_custom_fields: u16,
    pub max_custom_field_id_bytes: u16,
    pub max_hooks: u16,
    pub max_config_string_bytes: u32,
    pub max_locale_bytes: u16,
    pub max_structured_depth: u8,
    pub max_structured_leaves: u16,
    pub max_fx_applications: u16,
    pub max_field_value_bytes: u32,
    pub max_config_encoded_bytes: u32,
    pub max_values_per_sequence: u32,
    pub max_captured_values_per_function: u16,
    pub max_defaults_entries: u32,
    pub max_line_id_bytes: u16,
}

/// Production `CharacterDialogue` limits fixed by the language/runtime
/// contract.
pub const PRODUCTION_CHARACTER_DIALOGUE_LIMITS: CharacterDialogueLimits = CharacterDialogueLimits {
    max_patch_fields: 64,
    max_patch_work: 1_024,
    max_custom_fields: 32,
    max_custom_field_id_bytes: 128,
    max_hooks: 64,
    max_config_string_bytes: 16_384,
    max_locale_bytes: 64,
    max_structured_depth: 8,
    max_structured_leaves: 256,
    max_fx_applications: 128,
    max_field_value_bytes: 65_536,
    max_config_encoded_bytes: 524_288,
    max_values_per_sequence: 4_096,
    max_captured_values_per_function: 256,
    max_defaults_entries: 4_096,
    max_line_id_bytes: 256,
};

pub(super) const MAX_PUBLIC_ID_BYTES: usize =
    PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_line_id_bytes as usize;
pub(super) const MAX_LOCAL_ID_BYTES: usize =
    PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_custom_field_id_bytes as usize;
pub(super) const MAX_TYPED_AGGREGATE_BYTES: usize =
    (PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_field_value_bytes as usize) * 4;
