use std::collections::BTreeMap;
use std::fmt;

use arcweft_lang_hir::model::{HirModule, HirTopLevelDecl};
use arcweft_lang_hir::syntax::ast::common::Visibility;
use arcweft_lang_hir::syntax::ast::dialogue::DialogueDefaultsItem;
use arcweft_lang_hir::syntax::ast::items::{Attribute, EntityDeclKind, StructItem};
use arcweft_render_text::{
    InlineFailurePolicy, Milli, RichTextObjectProxyDeclaration, RichTextParam, RichTextStyle,
    RichTextStyleContribution, parse_milli_token,
};

use super::attrs::{param_from_value, parse_attr_args, trim_quotes, truthy_attr};
use super::entity_defaults::{
    character_callee_keys, character_display_label, character_style_defaults, character_style_keys,
    entity_ref_keys, entity_style_keys, style_defaults_from_dialogue_defaults,
    textbox_style_defaults,
};

pub(crate) const DEFAULT_DIALOGUE_WINDOW: &str = "textbox.main";

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DialogueDisplayDefaults {
    pub(crate) global: DialogueStyleDefaults,
    pub(crate) textboxes: BTreeMap<String, DialogueStyleDefaults>,
    pub(crate) characters: BTreeMap<String, DialogueStyleDefaults>,
    pub(crate) character_labels: BTreeMap<String, String>,
    pub(crate) text_proxies: BTreeMap<String, TextProxyTypeDefaults>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DialogueStyleDefaults {
    pub(crate) base_styles: Vec<RichTextStyle>,
    pub(crate) style_contributions: Vec<RichTextStyleContribution>,
    pub(crate) default_inline_failure_policy: Option<InlineFailurePolicy>,
    pub(crate) window: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DialogueSpeakerPreset {
    pub(crate) name: String,
    pub(crate) callee: String,
    pub(crate) defaults: DialogueStyleDefaults,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TextProxyTypeDefaults {
    pub(crate) declaration: RichTextObjectProxyDeclaration,
    pub(crate) type_name: String,
    pub(crate) role: Option<String>,
    pub(crate) layer: Option<String>,
    pub(crate) depth: Option<Milli>,
    pub(crate) default_hit: Option<bool>,
    pub(crate) params: BTreeMap<String, RichTextParam>,
}

impl DialogueDisplayDefaults {
    #[cfg(test)]
    pub(crate) fn from_module(module: &HirModule) -> Self {
        Self::try_from_module_with_selection(module, None).unwrap_or_default()
    }

    pub(crate) fn try_from_module_with_selection(
        module: &HirModule,
        selected_profile: Option<&str>,
    ) -> Result<Self, DialogueDefaultsSelectionError> {
        let mut defaults = Self::default();
        if let Some(item) = selected_dialogue_defaults(module, selected_profile)? {
            defaults.global = style_defaults_from_dialogue_defaults(item);
        }
        for declaration in module.declarations() {
            match declaration {
                HirTopLevelDecl::EntityDecl(item) if item.kind() == EntityDeclKind::Character => {
                    if let Some(label) = character_display_label(item) {
                        for key in character_style_keys(item) {
                            defaults.character_labels.insert(key, label.clone());
                        }
                    }
                    let style = character_style_defaults(item);
                    if !style.is_empty() {
                        for key in character_style_keys(item) {
                            defaults.characters.insert(key, style.clone());
                        }
                    }
                }
                HirTopLevelDecl::EntityDecl(item) if item.kind() == EntityDeclKind::Textbox => {
                    let style = textbox_style_defaults(item);
                    if !style.is_empty() {
                        for key in entity_style_keys(item) {
                            defaults.textboxes.insert(key, style.clone());
                        }
                    }
                }
                HirTopLevelDecl::Struct(item) => {
                    if let Some(proxy_defaults) = text_proxy_defaults_from_struct(item) {
                        defaults
                            .text_proxies
                            .insert(item.name().to_owned(), proxy_defaults);
                    }
                }
                _ => {}
            }
        }
        Ok(defaults)
    }

    pub(crate) fn character_for_callee(&self, callee: &str) -> Option<&DialogueStyleDefaults> {
        character_callee_keys(callee)
            .into_iter()
            .find_map(|key| self.characters.get(&key))
    }

    pub(crate) fn speaker_label_for_callee(&self, callee: &str) -> Option<&str> {
        character_callee_keys(callee)
            .into_iter()
            .find_map(|key| self.character_labels.get(&key).map(String::as_str))
    }

    pub(crate) fn textbox_for_window(&self, window: &str) -> Option<&DialogueStyleDefaults> {
        entity_ref_keys(window)
            .into_iter()
            .find_map(|key| self.textboxes.get(&key))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DialogueDefaultsSelectionError {
    MissingSelected { id: String },
    Ambiguous { profiles: Vec<String> },
}

impl fmt::Display for DialogueDefaultsSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSelected { id } => {
                write!(f, "selected dialogue defaults profile `{id}` was not found")
            }
            Self::Ambiguous { profiles } => write!(
                f,
                "multiple dialogue defaults profiles are visible but none was selected: {}",
                profiles.join(", ")
            ),
        }
    }
}

fn selected_dialogue_defaults<'a>(
    module: &'a HirModule,
    selected_profile: Option<&str>,
) -> Result<Option<&'a DialogueDefaultsItem>, DialogueDefaultsSelectionError> {
    let items = module
        .declarations()
        .iter()
        .filter_map(|declaration| match declaration {
            HirTopLevelDecl::DialogueDefaults(item) => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    if let Some(selected_profile) = selected_profile {
        return items
            .iter()
            .copied()
            .find(|item| item.id().is_some_and(|id| id.body() == selected_profile))
            .map(Some)
            .ok_or_else(|| DialogueDefaultsSelectionError::MissingSelected {
                id: selected_profile.to_owned(),
            });
    }
    if let Some(item) = items
        .iter()
        .copied()
        .find(|item| item.id().is_some_and(|id| id.body() == "dialogue.defaults"))
    {
        return Ok(Some(item));
    }
    let public = items
        .iter()
        .copied()
        .filter(|item| item.visibility() == Some(Visibility::Public))
        .collect::<Vec<_>>();
    if public.len() == 1 {
        return Ok(Some(public[0]));
    }
    if public.len() > 1 {
        return Err(DialogueDefaultsSelectionError::Ambiguous {
            profiles: public
                .iter()
                .map(|item| dialogue_defaults_label(item))
                .collect(),
        });
    }
    if items.len() == 1 {
        return Ok(Some(items[0]));
    }
    if items.len() > 1 {
        return Err(DialogueDefaultsSelectionError::Ambiguous {
            profiles: items
                .iter()
                .map(|item| dialogue_defaults_label(item))
                .collect(),
        });
    }
    Ok(None)
}

fn dialogue_defaults_label(item: &DialogueDefaultsItem) -> String {
    item.id()
        .map_or_else(|| "<anonymous>".to_owned(), |id| format!("@{}", id.body()))
}

fn text_proxy_defaults_from_struct(item: &StructItem) -> Option<TextProxyTypeDefaults> {
    let attr = item
        .attrs()
        .iter()
        .find(|attr| is_text_proxy_attribute(attr))?;
    let declaration = RichTextObjectProxyDeclaration {
        struct_name: item.name().to_owned(),
        attribute: attr.name().to_owned(),
    };
    let attrs = parse_attr_args(attr.args().unwrap_or_default());
    let type_name = attrs
        .get("type")
        .or_else(|| attrs.get("proxy"))
        .or_else(|| attrs.get("name"))
        .map(|value| trim_quotes(value).to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| item.name().to_owned());
    let role = attrs
        .get("role")
        .or_else(|| attrs.get("kind"))
        .map(|value| trim_quotes(value).to_owned())
        .filter(|value| !value.is_empty());
    let layer = attrs
        .get("layer")
        .or_else(|| attrs.get("object_layer"))
        .map(|value| trim_quotes(value).trim_start_matches('.').to_owned())
        .filter(|value| !value.is_empty());
    let depth = attrs
        .get("depth")
        .or_else(|| attrs.get("z"))
        .or_else(|| attrs.get("z_index"))
        .map(|value| parse_milli_token(value));
    let default_hit = attrs
        .get("default_hit")
        .or_else(|| attrs.get("hit"))
        .or_else(|| attrs.get("hit_test"))
        .map(|value| truthy_attr(value));
    let params = attrs
        .iter()
        .filter(|(key, _)| !is_text_proxy_attribute_metadata_attr(key))
        .map(|(key, value)| (key.clone(), param_from_value(value)))
        .collect();

    Some(TextProxyTypeDefaults {
        declaration,
        type_name,
        role,
        layer,
        depth,
        default_hit,
        params,
    })
}

fn is_text_proxy_attribute(attr: &Attribute) -> bool {
    matches!(attr.name(), "text_proxy" | "rich_text_proxy")
}

fn is_text_proxy_attribute_metadata_attr(key: &str) -> bool {
    matches!(
        key,
        "type"
            | "proxy"
            | "name"
            | "role"
            | "kind"
            | "layer"
            | "object_layer"
            | "depth"
            | "z"
            | "z_index"
            | "default_hit"
            | "hit"
            | "hit_test"
    )
}
