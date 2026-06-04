use crate::profiles::LspProfile;
use arcweft_verify_lsp::profile_completions;
use lsp_types::CompletionItem;

/// Computes completion items from resolved adapter and runtime-host facts.
pub fn completions(profile: &LspProfile) -> Vec<CompletionItem> {
    profile_completions(&profile.context())
}
