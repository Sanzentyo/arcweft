//! Rich-text display lowering for runtime-plan sidecars.

mod attrs;
mod contributions;
mod defaults;
mod entity_defaults;
mod helpers;
mod inline_failure;
mod line;
mod raw;
mod speaker_preset;
mod style_block;
mod style_expr;
mod tag;

#[cfg(test)]
mod tests;

pub(crate) use defaults::{DialogueDisplayDefaults, DialogueSpeakerPreset};
pub(crate) use line::lower_dialogue_display_with_speaker_presets;
pub(crate) use speaker_preset::speaker_preset_from_let;

#[cfg(test)]
pub(crate) use line::lower_dialogue_display;
