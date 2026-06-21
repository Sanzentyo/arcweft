use crate::profiles::LspProfile;
use arcweft_lang_sema::types::TypeKind;
use arcweft_verify_lsp::profile_completions;
use lsp_types::{CompletionItem, CompletionItemKind, Documentation};

/// Computes completion items from resolved adapter and runtime-host facts.
pub fn completions(profile: &LspProfile) -> Vec<CompletionItem> {
    let mut items = profile_completions(&profile.context());
    items.extend(enum_variant_completions(profile));
    items
}

fn enum_variant_completions(profile: &LspProfile) -> Vec<CompletionItem> {
    profile
        .typecheck_env()
        .enum_variant_sets()
        .into_iter()
        .flat_map(|(ty, variants)| {
            let ty_label = type_kind_label(&ty);
            variants.into_iter().map(move |variant| {
                let label = format!(".{variant}");
                let qualified = format!("{ty_label}.{variant}");
                CompletionItem {
                    label: label.clone(),
                    kind: Some(CompletionItemKind::ENUM_MEMBER),
                    detail: Some(qualified.clone()),
                    documentation: Some(Documentation::String(format!(
                        "Short enum variant for `{qualified}` when `{ty_label}` is expected."
                    ))),
                    filter_text: Some(label.clone()),
                    insert_text: Some(label),
                    ..CompletionItem::default()
                }
            })
        })
        .collect()
}

fn type_kind_label(ty: &TypeKind) -> String {
    match ty {
        TypeKind::Named(name) => name.clone(),
        _ => format!("{ty:?}"),
    }
}
