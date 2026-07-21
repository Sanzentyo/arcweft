//! Rich-text display lowering for runtime-plan sidecars.

mod attrs;
mod character_profile;
mod contributions;
mod dialogue_context;
mod fx;
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

pub(crate) use dialogue_context::{DialogueDisplayDefaults, DialogueSpeakerPreset};
pub(crate) use fx::FxCatalog;
pub(crate) use fx::builtins::builtin_rich_text_fx_definitions;
pub(crate) use line::lower_dialogue_display_with_speaker_presets_and_fx;
pub(crate) use speaker_preset::speaker_preset_from_let;

#[cfg(test)]
pub(crate) use line::{lower_dialogue_display, lower_dialogue_display_with_speaker_presets};
