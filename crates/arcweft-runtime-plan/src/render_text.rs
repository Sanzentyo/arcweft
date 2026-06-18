//! Rich-text display lowering for runtime-plan sidecars.

use crate::labels::expr_label;
use arcweft_core::plan::RuntimeLineId;
use arcweft_lang_hir::model::{HirDialogue, HirModule, HirTopLevelDecl};
use arcweft_lang_hir::syntax::ast::common::{TextRange, Visibility};
use arcweft_lang_hir::syntax::ast::dialogue::{
    DialogueDefaultAssignOp, DialogueDefaultAssignment, DialogueDefaultsItem, DialogueTag,
    DialogueToken, LineArg,
};
use arcweft_lang_hir::syntax::ast::items::{Attribute, EntityDeclItem, EntityDeclKind, StructItem};
use arcweft_lang_hir::syntax::ast::pattern::Pattern;
use arcweft_lang_hir::syntax::expr::{CallArg, Expr, Literal, parse_expr};
use arcweft_render_text::{
    DialogueHostEvent, FallbackStylePolicy, InlineFailurePolicy, InlineFallback, LineDisplayArg,
    LineDisplaySpec, Milli, RichTextAngle, RichTextAssignOp, RichTextCascadeLayer, RichTextColor,
    RichTextControl, RichTextDocument, RichTextEffectDescriptor, RichTextEffectPhase,
    RichTextEffectTarget, RichTextFontFamily, RichTextInlineDirection, RichTextJlreqStrictness,
    RichTextLayout, RichTextNode, RichTextObjectProxy, RichTextObjectProxyDeclaration,
    RichTextParam, RichTextPresentationStyle, RichTextRubyPosition, RichTextSettingSource,
    RichTextShaderRef, RichTextSourceRange, RichTextStateScope, RichTextStyle,
    RichTextStyleContribution, RichTextTransform, RichTextTransformOrigin, RichTextVec2,
    RichTextVerticalLatinMode, RichTextWritingMode, parse_decimal_milli, parse_milli_token,
    parse_z_index_token,
};
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;

const DEFAULT_DIALOGUE_WINDOW: &str = "textbox.main";

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DialogueDisplayDefaults {
    global: DialogueStyleDefaults,
    textboxes: BTreeMap<String, DialogueStyleDefaults>,
    characters: BTreeMap<String, DialogueStyleDefaults>,
    text_proxies: BTreeMap<String, TextProxyTypeDefaults>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DialogueStyleDefaults {
    base_styles: Vec<RichTextStyle>,
    style_contributions: Vec<RichTextStyleContribution>,
    default_inline_failure_policy: Option<InlineFailurePolicy>,
    window: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DialogueSpeakerPreset {
    name: String,
    callee: String,
    defaults: DialogueStyleDefaults,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct TextProxyTypeDefaults {
    declaration: RichTextObjectProxyDeclaration,
    type_name: String,
    role: Option<String>,
    layer: Option<String>,
    depth: Option<Milli>,
    default_hit: Option<bool>,
    params: BTreeMap<String, RichTextParam>,
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

    fn character_for_callee(&self, callee: &str) -> Option<&DialogueStyleDefaults> {
        character_callee_keys(callee)
            .into_iter()
            .find_map(|key| self.characters.get(&key))
    }

    fn textbox_for_window(&self, window: &str) -> Option<&DialogueStyleDefaults> {
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

#[cfg(test)]
pub(crate) fn lower_dialogue_display(
    line: RuntimeLineId,
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
) -> LineDisplaySpec {
    lower_dialogue_display_with_speaker_presets(line, dialogue, defaults, &[])
}

pub(crate) fn lower_dialogue_display_with_speaker_presets(
    line: RuntimeLineId,
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
    speaker_presets: &[DialogueSpeakerPreset],
) -> LineDisplaySpec {
    let default_inline_failure_policy =
        lower_effective_inline_failure_policy(dialogue, defaults, speaker_presets);
    LineDisplaySpec {
        line,
        callee: dialogue.callee().to_owned(),
        text_key: dialogue.text_key().map(|id| id.body().to_owned()),
        window: effective_dialogue_window(dialogue, defaults, speaker_presets),
        voice: dialogue.voice().map(expr_label),
        look: dialogue.look().map(expr_label),
        style: dialogue.style().map(expr_label),
        base_styles: lower_effective_dialogue_base_styles(dialogue, defaults, speaker_presets),
        default_inline_failure_policy: default_inline_failure_policy.clone(),
        style_contributions: lower_effective_dialogue_style_contributions(
            dialogue,
            defaults,
            speaker_presets,
        ),
        args: dialogue
            .args()
            .iter()
            .map(|arg| LineDisplayArg {
                name: arg.name().to_owned(),
                value: expr_label(arg.value()),
            })
            .collect(),
        content: RichTextDocument::new(
            dialogue
                .content()
                .tokens()
                .iter()
                .flat_map(|token| {
                    lower_dialogue_token(
                        token,
                        default_inline_failure_policy.as_ref(),
                        &defaults.text_proxies,
                    )
                })
                .collect(),
        ),
    }
}

fn lower_effective_dialogue_base_styles(
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
    speaker_presets: &[DialogueSpeakerPreset],
) -> Vec<RichTextStyle> {
    let mut styles = defaults.global.base_styles.clone();
    let preset_chain = speaker_preset_chain(dialogue.callee(), speaker_presets);
    if let Some(textbox) = effective_dialogue_window(dialogue, defaults, speaker_presets)
        .as_deref()
        .and_then(|window| defaults.textbox_for_window(window))
    {
        styles.extend(textbox.base_styles.clone());
    }
    let character_callee = preset_chain
        .first()
        .map_or_else(|| dialogue.callee(), |preset| preset.callee());
    if let Some(character) = defaults.character_for_callee(character_callee) {
        styles.extend(character.base_styles.clone());
    }
    styles.extend(
        preset_chain
            .iter()
            .flat_map(|preset| preset.defaults.base_styles.clone()),
    );
    styles.extend(
        dialogue
            .style()
            .into_iter()
            .flat_map(display_styles_from_expr),
    );
    styles.extend(
        dialogue
            .rich_text()
            .into_iter()
            .flat_map(display_styles_from_expr),
    );
    styles.extend(
        dialogue
            .args()
            .iter()
            .flat_map(|arg| display_styles_from_named_expr(arg.name(), arg.value())),
    );
    styles
}

fn lower_effective_inline_failure_policy(
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
    speaker_presets: &[DialogueSpeakerPreset],
) -> Option<InlineFailurePolicy> {
    let preset_chain = speaker_preset_chain(dialogue.callee(), speaker_presets);
    let character_callee = preset_chain
        .first()
        .map_or_else(|| dialogue.callee(), |preset| preset.callee());
    let textbox_policy = effective_dialogue_window(dialogue, defaults, speaker_presets)
        .as_deref()
        .and_then(|window| defaults.textbox_for_window(window))
        .and_then(|textbox| textbox.default_inline_failure_policy.clone());
    lower_default_inline_failure_policy(dialogue.args())
        .or_else(|| {
            preset_chain
                .into_iter()
                .rev()
                .find_map(|preset| preset.defaults.default_inline_failure_policy.clone())
        })
        .or_else(|| {
            defaults
                .character_for_callee(character_callee)
                .and_then(|character| character.default_inline_failure_policy.clone())
        })
        .or(textbox_policy)
        .or_else(|| defaults.global.default_inline_failure_policy.clone())
}

fn lower_effective_dialogue_style_contributions(
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
    speaker_presets: &[DialogueSpeakerPreset],
) -> Vec<RichTextStyleContribution> {
    let mut contributions = Vec::new();
    let mut base_offset = 0usize;

    append_style_contributions(&mut contributions, &defaults.global, &mut base_offset);
    let preset_chain = speaker_preset_chain(dialogue.callee(), speaker_presets);
    if let Some(textbox) = effective_dialogue_window(dialogue, defaults, speaker_presets)
        .as_deref()
        .and_then(|window| defaults.textbox_for_window(window))
    {
        append_style_contributions(&mut contributions, textbox, &mut base_offset);
    }
    let character_callee = preset_chain
        .first()
        .map_or_else(|| dialogue.callee(), |preset| preset.callee());
    if let Some(character) = defaults.character_for_callee(character_callee) {
        append_style_contributions(&mut contributions, character, &mut base_offset);
    }
    for preset in preset_chain {
        append_style_contributions(&mut contributions, &preset.defaults, &mut base_offset);
    }

    if let Some(style) = dialogue.style() {
        let styles = display_styles_from_expr(style);
        let has_policy = inline_default_from_named_expr("style", style).is_some();
        let source = dialogue_option_source(dialogue, dialogue.style_range().map(source_range));
        append_line_option_contributions(
            &mut contributions,
            &mut base_offset,
            &LineOptionContribution {
                path: "style",
                expr: style,
                raw: dialogue.style_raw(),
                styles: &styles,
                has_policy,
                source,
                layer: RichTextCascadeLayer::LineOptions,
            },
        );
    }
    if let Some(rich_text) = dialogue.rich_text() {
        let styles = display_styles_from_expr(rich_text);
        let has_policy = inline_default_from_named_expr("rich_text", rich_text).is_some();
        let source = dialogue_option_source(dialogue, dialogue.rich_text_range().map(source_range));
        append_line_option_contributions(
            &mut contributions,
            &mut base_offset,
            &LineOptionContribution {
                path: "rich_text",
                expr: rich_text,
                raw: dialogue.rich_text_raw(),
                styles: &styles,
                has_policy,
                source,
                layer: RichTextCascadeLayer::LineOptions,
            },
        );
    }
    for arg in dialogue.args() {
        let styles = display_styles_from_named_expr(arg.name(), arg.value());
        let has_policy = inline_default_from_named_expr(arg.name(), arg.value()).is_some();
        let source = dialogue_option_source(dialogue, Some(source_range(arg.value_range())));
        append_line_option_contributions(
            &mut contributions,
            &mut base_offset,
            &LineOptionContribution {
                path: arg.name(),
                expr: arg.value(),
                raw: Some(arg.raw_value()),
                styles: &styles,
                has_policy,
                source,
                layer: RichTextCascadeLayer::LineOptions,
            },
        );
    }

    append_inline_span_contributions(&mut contributions, dialogue);
    mark_shadowed_style_contributions(&mut contributions);
    contributions
}

impl DialogueSpeakerPreset {
    fn callee(&self) -> &str {
        &self.callee
    }
}

pub(crate) fn speaker_preset_from_let(
    pattern: &Pattern,
    expr: &Expr,
    expr_source: Option<&str>,
    expr_range: Option<&TextRange>,
) -> Option<(String, DialogueSpeakerPreset)> {
    let name = pattern_ident(pattern)?;
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    let defaults = speaker_preset_defaults(name, args, expr_source, expr_range);
    Some((
        name.to_owned(),
        DialogueSpeakerPreset {
            name: name.to_owned(),
            callee: expr_label(callee),
            defaults,
        },
    ))
}

fn pattern_ident(pattern: &Pattern) -> Option<&str> {
    match pattern {
        Pattern::Ident(name)
        | Pattern::MutIdent(name)
        | Pattern::Typed { name, .. }
        | Pattern::Whole { name, .. } => Some(name),
        Pattern::Literal(_)
        | Pattern::Entity(_)
        | Pattern::Variant { .. }
        | Pattern::Discard
        | Pattern::Tuple(_)
        | Pattern::Record { .. }
        | Pattern::BracketSeq { .. }
        | Pattern::Raw(_) => None,
    }
}

fn speaker_preset_defaults(
    name: &str,
    args: &[CallArg],
    expr_source: Option<&str>,
    expr_range: Option<&TextRange>,
) -> DialogueStyleDefaults {
    let mut defaults = DialogueStyleDefaults::default();
    let arg_ranges = speaker_preset_arg_ranges(expr_source, expr_range);
    for arg in args {
        if let CallArg::Named { name: path, value } = arg {
            append_speaker_preset_arg(
                &mut defaults,
                name,
                path,
                value,
                arg_ranges.get(path).copied(),
            );
        }
    }
    defaults
}

fn append_speaker_preset_arg(
    defaults: &mut DialogueStyleDefaults,
    preset_name: &str,
    path: &str,
    value: &Expr,
    range: Option<RichTextSourceRange>,
) {
    let policy = inline_default_from_named_expr(path, value);
    if let Some(policy) = policy.clone() {
        defaults.default_inline_failure_policy = Some(policy);
    }
    if path == "window" {
        defaults.window = Some(entity_ref_label(value));
    }

    let style_index = defaults.base_styles.len();
    let styles = display_styles_from_named_expr(path, value);
    let active = policy.is_some() || path == "window" || !styles.is_empty();
    let style_index = (!styles.is_empty()).then_some(style_index);
    defaults.base_styles.extend(styles);
    defaults
        .style_contributions
        .extend(
            style_assignment_paths(path, value)
                .into_iter()
                .map(|(path, value)| RichTextStyleContribution {
                    path,
                    layer: RichTextCascadeLayer::SpeakerPreset,
                    source: source_file(Some(preset_name.to_owned()), range),
                    op: RichTextAssignOp::Replace,
                    value,
                    style_index,
                    active,
                    shadowed_by: None,
                }),
        );
}

fn speaker_preset_chain<'a>(
    callee: &str,
    speaker_presets: &'a [DialogueSpeakerPreset],
) -> Vec<&'a DialogueSpeakerPreset> {
    let mut chain = Vec::new();
    push_speaker_preset_chain(callee, speaker_presets, &mut chain, &mut Vec::new());
    chain.reverse();
    chain
}

fn push_speaker_preset_chain<'a>(
    callee: &str,
    speaker_presets: &'a [DialogueSpeakerPreset],
    chain: &mut Vec<&'a DialogueSpeakerPreset>,
    seen: &mut Vec<String>,
) {
    if seen.iter().any(|item| item == callee) {
        return;
    }
    seen.push(callee.to_owned());
    let Some(preset) = speaker_presets.iter().rev().find(|preset| {
        preset.callee() != callee && preset_names(callee).any(|name| name == preset.name)
    }) else {
        return;
    };
    chain.push(preset);
    push_speaker_preset_chain(preset.callee(), speaker_presets, chain, seen);
}

fn preset_names(callee: &str) -> impl Iterator<Item = &str> {
    std::iter::once(callee.trim()).chain(
        callee
            .trim()
            .strip_suffix(".say")
            .into_iter()
            .map(str::trim),
    )
}

fn effective_dialogue_window(
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
    speaker_presets: &[DialogueSpeakerPreset],
) -> Option<String> {
    dialogue
        .window()
        .map(|id| id.body().to_owned())
        .or_else(|| {
            speaker_preset_chain(dialogue.callee(), speaker_presets)
                .into_iter()
                .rev()
                .find_map(|preset| preset.defaults.window.clone())
        })
        .or_else(|| {
            let preset_chain = speaker_preset_chain(dialogue.callee(), speaker_presets);
            let character_callee = preset_chain
                .first()
                .map_or_else(|| dialogue.callee(), |preset| preset.callee());
            defaults
                .character_for_callee(character_callee)
                .and_then(|character| character.window.clone())
        })
        .or_else(|| defaults.global.window.clone())
        .or_else(|| Some(DEFAULT_DIALOGUE_WINDOW.to_owned()))
}

impl DialogueStyleDefaults {
    fn is_empty(&self) -> bool {
        self.base_styles.is_empty()
            && self.style_contributions.is_empty()
            && self.default_inline_failure_policy.is_none()
            && self.window.is_none()
    }

    fn merge(&mut self, other: Self) {
        self.base_styles.extend(other.base_styles);
        self.style_contributions.extend(other.style_contributions);
        self.default_inline_failure_policy = other
            .default_inline_failure_policy
            .or_else(|| self.default_inline_failure_policy.clone());
        self.window = other.window.or_else(|| self.window.clone());
    }
}

fn character_style_defaults(item: &EntityDeclItem) -> DialogueStyleDefaults {
    item.body()
        .and_then(|body| named_style_block(body, item.body_range(), "dialogue_style"))
        .map(|body| {
            style_defaults_from_body(
                body.source,
                None,
                RichTextCascadeLayer::CharacterDialogueStyle,
                Some(item.id().body()),
                body.absolute_start,
            )
        })
        .unwrap_or_default()
}

fn character_style_keys(item: &EntityDeclItem) -> Vec<String> {
    entity_style_keys(item)
}

fn textbox_style_defaults(item: &EntityDeclItem) -> DialogueStyleDefaults {
    let mut defaults = DialogueStyleDefaults::default();
    if let Some(body) = item.body() {
        if let Some(block) = named_style_block(body, item.body_range(), "dialogue_style") {
            defaults.merge(style_defaults_from_body(
                block.source,
                None,
                RichTextCascadeLayer::TextBoxTheme,
                Some(item.id().body()),
                block.absolute_start,
            ));
        }
        if let Some(block) = named_style_block(body, item.body_range(), "rich_text") {
            defaults.merge(style_defaults_from_body(
                block.source,
                Some("rich_text"),
                RichTextCascadeLayer::TextBoxTheme,
                Some(item.id().body()),
                block.absolute_start,
            ));
        }
    }
    defaults
}

fn entity_style_keys(item: &EntityDeclItem) -> Vec<String> {
    [
        item.surface_alias().map(str::to_owned),
        item.name().map(str::to_owned),
        Some(item.id().body().to_owned()),
        item.id()
            .body()
            .rsplit_once('.')
            .map(|(_, suffix)| suffix.to_owned()),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn entity_ref_keys(raw: &str) -> Vec<String> {
    let mut keys = Vec::new();
    push_character_callee_key(&mut keys, raw);
    keys
}

fn character_callee_keys(callee: &str) -> Vec<String> {
    let mut keys = Vec::new();
    push_character_callee_key(&mut keys, callee.trim());
    if let Some(receiver) = callee.trim().strip_suffix(".say") {
        push_character_callee_key(&mut keys, receiver);
    }
    if let Some((speaker, _)) = callee.trim().split_once('.') {
        push_character_callee_key(&mut keys, speaker);
    }
    keys
}

fn push_character_callee_key(keys: &mut Vec<String>, raw: &str) {
    let normalized = raw
        .trim()
        .strip_prefix("@<")
        .and_then(|inner| inner.strip_suffix('>'))
        .or_else(|| raw.trim().strip_prefix('@'))
        .unwrap_or(raw)
        .trim();
    if normalized.is_empty() {
        return;
    }
    push_unique_string(keys, normalized);
    if let Some((_, suffix)) = normalized.rsplit_once(['.', ':']) {
        push_unique_string(keys, suffix);
    }
}

fn push_unique_string(keys: &mut Vec<String>, value: &str) {
    if !keys.iter().any(|key| key == value) {
        keys.push(value.to_owned());
    }
}

fn style_defaults_from_body(
    body: &str,
    path_prefix: Option<&str>,
    layer: RichTextCascadeLayer,
    item_id: Option<&str>,
    body_absolute_start: Option<usize>,
) -> DialogueStyleDefaults {
    let mut defaults = DialogueStyleDefaults::default();
    for assignment in style_block_assignments(body, path_prefix) {
        if let Ok(expr) = parse_expr(assignment.value) {
            append_style_default(
                &mut defaults,
                assignment.name.clone(),
                RichTextAssignOp::Replace,
                assignment.value.to_owned(),
                &expr,
                layer,
                style_assignment_source(item_id, body_absolute_start, assignment.value_range),
            );
        }
    }
    defaults
}

fn style_defaults_from_dialogue_defaults(item: &DialogueDefaultsItem) -> DialogueStyleDefaults {
    let mut defaults = DialogueStyleDefaults::default();
    let item_id = item.id().map(|id| id.body().to_owned());
    for assignment in item.assignments() {
        append_dialogue_default_assignment(&mut defaults, assignment, item_id.clone());
    }
    defaults
}

fn append_dialogue_default_assignment(
    defaults: &mut DialogueStyleDefaults,
    assignment: &DialogueDefaultAssignment,
    item_id: Option<String>,
) {
    append_style_default(
        defaults,
        assignment.path().dotted(),
        rich_text_assign_op(assignment.op()),
        assignment.raw_value().to_owned(),
        assignment.value(),
        RichTextCascadeLayer::DialogueDefaults,
        source_file(item_id, Some(source_range(assignment.value_range()))),
    );
}

fn append_style_default(
    defaults: &mut DialogueStyleDefaults,
    path: String,
    op: RichTextAssignOp,
    value: String,
    expr: &Expr,
    layer: RichTextCascadeLayer,
    source: RichTextSettingSource,
) {
    let policy = inline_default_from_named_expr(&path, expr);
    if let Some(policy) = policy.clone() {
        defaults.default_inline_failure_policy = Some(policy);
    }
    if path == "window" {
        defaults.window = Some(entity_ref_label(expr));
    }

    let style_index = defaults.base_styles.len();
    let styles = display_styles_from_named_expr(&path, expr);
    let active = policy.is_some() || path == "window" || !styles.is_empty();
    let style_index = (!styles.is_empty()).then_some(style_index);
    defaults.base_styles.extend(styles);
    defaults
        .style_contributions
        .push(RichTextStyleContribution {
            path,
            layer,
            source,
            op,
            value,
            style_index,
            active,
            shadowed_by: None,
        });
}

fn append_style_contributions(
    target: &mut Vec<RichTextStyleContribution>,
    defaults: &DialogueStyleDefaults,
    base_offset: &mut usize,
) {
    target.extend(
        defaults
            .style_contributions
            .iter()
            .cloned()
            .map(|mut contribution| {
                if let Some(style_index) = contribution.style_index {
                    contribution.style_index = Some(*base_offset + style_index);
                }
                contribution
            }),
    );
    *base_offset += defaults.base_styles.len();
}

#[derive(Clone)]
struct LineOptionContribution<'a> {
    path: &'a str,
    expr: &'a Expr,
    raw: Option<&'a str>,
    styles: &'a [RichTextStyle],
    has_policy: bool,
    source: RichTextSettingSource,
    layer: RichTextCascadeLayer,
}

fn append_line_option_contributions(
    target: &mut Vec<RichTextStyleContribution>,
    base_offset: &mut usize,
    input: &LineOptionContribution<'_>,
) {
    let style_index = (!input.styles.is_empty()).then_some(*base_offset);
    let active = input.has_policy || !input.styles.is_empty();
    if let Some(assignments) = input
        .raw
        .map(|raw| style_assignments_from_raw(input.path, raw))
        .filter(|assignments| !assignments.is_empty())
    {
        target.extend(
            assignments
                .into_iter()
                .map(|assignment| RichTextStyleContribution {
                    path: assignment.path,
                    layer: input.layer,
                    source: source_with_relative_range(&input.source, assignment.value_range),
                    op: RichTextAssignOp::Replace,
                    value: assignment.value,
                    style_index,
                    active,
                    shadowed_by: None,
                }),
        );
    } else {
        target.extend(
            style_assignment_paths(input.path, input.expr)
                .into_iter()
                .map(|(path, value)| RichTextStyleContribution {
                    path,
                    layer: input.layer,
                    source: input.source.clone(),
                    op: RichTextAssignOp::Replace,
                    value,
                    style_index,
                    active,
                    shadowed_by: None,
                }),
        );
    }
    *base_offset += input.styles.len();
}

fn append_inline_span_contributions(
    target: &mut Vec<RichTextStyleContribution>,
    dialogue: &HirDialogue,
) {
    target.extend(
        inline_style_assignments(dialogue.content().raw(), dialogue.content().range().start())
            .into_iter()
            .map(|assignment| RichTextStyleContribution {
                path: assignment.path,
                layer: RichTextCascadeLayer::InlineSpan,
                source: RichTextSettingSource::SourceFile {
                    item_id: dialogue.id().map(|id| id.body().to_owned()),
                    public_id: dialogue.text_key().map(|id| id.body().to_owned()),
                    range: Some(RichTextSourceRange {
                        start: assignment.value_range.start,
                        end: assignment.value_range.end,
                    }),
                },
                op: RichTextAssignOp::Replace,
                value: assignment.value,
                style_index: None,
                active: true,
                shadowed_by: None,
            }),
    );
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InlineStyleAssignment {
    path: String,
    value: String,
    value_range: Range<usize>,
}

fn inline_style_assignments(raw: &str, absolute_start: usize) -> Vec<InlineStyleAssignment> {
    inline_tag_ranges(raw)
        .into_iter()
        .flat_map(|tag_range| {
            let inside = &raw[tag_range.start + '['.len_utf8()..tag_range.end - ']'.len_utf8()];
            inline_assignments_from_tag(inside, absolute_start + tag_range.start + '['.len_utf8())
        })
        .collect()
}

fn inline_tag_ranges(raw: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut cursor = 0usize;
    while let Some(open_relative) = raw[cursor..].find('[') {
        let open = cursor + open_relative;
        let Some(close_relative) = raw[open + '['.len_utf8()..].find(']') else {
            break;
        };
        let close = open + '['.len_utf8() + close_relative + ']'.len_utf8();
        ranges.push(open..close);
        cursor = close;
    }
    ranges
}

fn inline_assignments_from_tag(
    inside: &str,
    inside_absolute_start: usize,
) -> Vec<InlineStyleAssignment> {
    let leading = inside.len() - inside.trim_start().len();
    let trimmed = inside.trim();
    if trimmed.is_empty() || trimmed.starts_with('/') || trimmed.starts_with('!') {
        return Vec::new();
    }
    let trimmed_start = inside_absolute_start + leading;
    if trimmed.starts_with('.') {
        let (selector, attrs) = split_tag_name_attrs_for_inline(trimmed);
        let attrs_start = inline_attrs_start(trimmed, selector, trimmed_start);
        return inferred_inline_assignments(
            selector.trim_start_matches('.'),
            attrs,
            trimmed_start,
            attrs_start,
        );
    }

    let (name, attrs) = split_tag_name_attrs_for_inline(trimmed);
    let attrs_start = inline_attrs_start(trimmed, name, trimmed_start);
    match name {
        "style" => {
            selector_inline_assignments(attrs, attrs_start, style_selector_inline_assignments)
        }
        "layout" => {
            selector_inline_assignments(attrs, attrs_start, layout_selector_inline_assignments)
        }
        "transform" => {
            selector_inline_assignments(attrs, attrs_start, transform_selector_inline_assignments)
        }
        "effect" | "fx" => {
            selector_inline_assignments(attrs, attrs_start, effect_selector_inline_assignments)
        }
        "color" | "font" | "size" | "em" | "strong" | "i" | "italic" | "oblique" | "slant" => {
            direct_inline_assignments(name, attrs, trimmed_start, attrs_start)
        }
        _ => Vec::new(),
    }
}

fn split_tag_name_attrs_for_inline(source: &str) -> (&str, &str) {
    let mut parts = source.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or_default();
    let attrs = parts.next().unwrap_or_default().trim();
    (name, attrs)
}

fn inline_attrs_start(trimmed: &str, name: &str, trimmed_start: usize) -> usize {
    trimmed_start + name.len() + trimmed[name.len()..].len()
        - trimmed[name.len()..].trim_start().len()
}

fn selector_inline_assignments(
    attrs: &str,
    attrs_start: usize,
    build: fn(&str, &str, usize, usize) -> Vec<InlineStyleAssignment>,
) -> Vec<InlineStyleAssignment> {
    let (selector, selector_attrs) = split_selector_attrs(attrs);
    let selector_offset = attrs.find(selector).unwrap_or(0);
    let selector_start = attrs_start + selector_offset;
    let selector_attrs_start =
        inline_attrs_start(&attrs[selector_offset..], selector, selector_start);
    build(
        selector.trim_start_matches('.'),
        selector_attrs,
        selector_start,
        selector_attrs_start,
    )
}

fn inferred_inline_assignments(
    selector: &str,
    attrs: &str,
    selector_start: usize,
    attrs_start: usize,
) -> Vec<InlineStyleAssignment> {
    match inferred_tag_family(selector, attrs) {
        Some(InferredTagFamily::Style) => {
            style_selector_inline_assignments(selector, attrs, selector_start, attrs_start)
        }
        Some(InferredTagFamily::Layout) => {
            layout_selector_inline_assignments(selector, attrs, selector_start, attrs_start)
        }
        Some(InferredTagFamily::Transform) => {
            transform_selector_inline_assignments(selector, attrs, selector_start, attrs_start)
        }
        Some(InferredTagFamily::Effect) => {
            effect_selector_inline_assignments(selector, attrs, selector_start, attrs_start)
        }
        Some(InferredTagFamily::Marker) | None => Vec::new(),
    }
}

fn direct_inline_assignments(
    name: &str,
    attrs: &str,
    name_start: usize,
    attrs_start: usize,
) -> Vec<InlineStyleAssignment> {
    let value_range = if attrs.is_empty() {
        name_start..name_start + name.len()
    } else {
        attrs_start..attrs_start + attrs.len()
    };
    let value = if attrs.is_empty() { name } else { attrs }
        .trim()
        .to_owned();
    let path = match name {
        "color" => "rich_text.text.color",
        "font" => "rich_text.text.font",
        "size" => "rich_text.text.size",
        "em" | "strong" | "i" | "italic" | "oblique" | "slant" => "rich_text.text.style",
        _ => return Vec::new(),
    };
    vec![InlineStyleAssignment {
        path: path.to_owned(),
        value,
        value_range,
    }]
}

fn style_selector_inline_assignments(
    selector: &str,
    attrs: &str,
    selector_start: usize,
    attrs_start: usize,
) -> Vec<InlineStyleAssignment> {
    let value = if attrs.is_empty() { selector } else { attrs }
        .trim()
        .to_owned();
    let value_range = if attrs.is_empty() {
        selector_start..selector_start + selector.len()
    } else {
        attrs_start..attrs_start + attrs.len()
    };
    vec![InlineStyleAssignment {
        path: "rich_text.text.style".to_owned(),
        value,
        value_range,
    }]
}

fn layout_selector_inline_assignments(
    selector: &str,
    attrs: &str,
    selector_start: usize,
    attrs_start: usize,
) -> Vec<InlineStyleAssignment> {
    let mut assignments = Vec::new();
    match selector {
        "vertical_rl" | "vertical" | "vertical_lr" | "horizontal_tb" => {
            assignments.push(InlineStyleAssignment {
                path: "rich_text.layout.writing_mode".to_owned(),
                value: selector.to_owned(),
                value_range: selector_start..selector_start + selector.len(),
            });
        }
        "ruby_over" | "ruby_under" | "ruby_inter_character" => {
            assignments.push(InlineStyleAssignment {
                path: "rich_text.ruby.position".to_owned(),
                value: selector.trim_start_matches("ruby_").to_owned(),
                value_range: selector_start..selector_start + selector.len(),
            });
        }
        _ => {}
    }
    assignments.extend(
        inline_attr_assignments(attrs, attrs_start)
            .into_iter()
            .filter_map(|attr| {
                let path = match attr.name.as_str() {
                    "ruby_size" | "size" if selector.starts_with("ruby_") => "rich_text.ruby.size",
                    "ruby_gap" | "gap" if selector.starts_with("ruby_") => "rich_text.ruby.gap",
                    "ruby_overhang" | "overhang" => "rich_text.ruby.overhang",
                    "ruby_collision_gap" | "collision_gap" => "rich_text.ruby.collision_gap",
                    "jlreq" | "strictness" | "kinsoku" => "rich_text.layout.jlreq",
                    "latin" | "vertical_latin" => "rich_text.layout.vertical_latin",
                    "dir" | "direction" => "rich_text.layout.direction",
                    "column_gap" | "gap" => "rich_text.layout.column_gap",
                    _ => return None,
                };
                Some(InlineStyleAssignment {
                    path: path.to_owned(),
                    value: attr.value,
                    value_range: attr.value_range,
                })
            }),
    );
    assignments
}

fn transform_selector_inline_assignments(
    selector: &str,
    attrs: &str,
    selector_start: usize,
    attrs_start: usize,
) -> Vec<InlineStyleAssignment> {
    let mut assignments = vec![InlineStyleAssignment {
        path: "rich_text.transform.kind".to_owned(),
        value: selector.to_owned(),
        value_range: selector_start..selector_start + selector.len(),
    }];
    assignments.extend(
        inline_attr_assignments(attrs, attrs_start)
            .into_iter()
            .map(|attr| InlineStyleAssignment {
                path: format!("rich_text.transform.{}", attr.name),
                value: attr.value,
                value_range: attr.value_range,
            }),
    );
    assignments
}

fn effect_selector_inline_assignments(
    selector: &str,
    attrs: &str,
    selector_start: usize,
    attrs_start: usize,
) -> Vec<InlineStyleAssignment> {
    let mut assignments = vec![InlineStyleAssignment {
        path: "rich_text.effect".to_owned(),
        value: selector.to_owned(),
        value_range: selector_start..selector_start + selector.len(),
    }];
    assignments.extend(
        inline_attr_assignments(attrs, attrs_start)
            .into_iter()
            .map(|attr| InlineStyleAssignment {
                path: format!("rich_text.effect.{selector}.{}", attr.name),
                value: attr.value,
                value_range: attr.value_range,
            }),
    );
    assignments
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InlineAttrAssignment {
    name: String,
    value: String,
    value_range: Range<usize>,
}

fn inline_attr_assignments(attrs: &str, attrs_start: usize) -> Vec<InlineAttrAssignment> {
    let mut assignments = Vec::new();
    let mut cursor = 0usize;
    for part in attrs.split_whitespace() {
        let part_start = attrs[cursor..]
            .find(part)
            .map_or(cursor, |relative| cursor + relative);
        cursor = part_start + part.len();
        let Some((name, value)) = part.split_once('=') else {
            continue;
        };
        let value_start = attrs_start + part_start + name.len() + '='.len_utf8();
        assignments.push(InlineAttrAssignment {
            name: name.to_owned(),
            value: trim_quotes(value).to_owned(),
            value_range: value_start..value_start + value.len(),
        });
    }
    assignments
}

fn style_assignment_paths(path: &str, expr: &Expr) -> Vec<(String, String)> {
    let nested = nested_style_assignment_paths(path, expr);
    if nested.is_empty() {
        vec![(path.to_owned(), expr_label(expr))]
    } else {
        nested
    }
}

struct RawStyleAssignment {
    path: String,
    value: String,
    value_range: Option<Range<usize>>,
}

fn style_assignments_from_raw(path: &str, raw: &str) -> Vec<RawStyleAssignment> {
    let raw = raw.trim();
    style_assignments_from_trimmed_raw(path, raw, 0)
}

fn style_assignments_from_trimmed_raw(
    path: &str,
    raw: &str,
    raw_offset: usize,
) -> Vec<RawStyleAssignment> {
    let Some((callee, _args)) = raw_call_parts(raw) else {
        return vec![RawStyleAssignment {
            path: path.to_owned(),
            value: raw.to_owned(),
            value_range: Some(raw_offset..raw_offset + raw.len()),
        }];
    };
    let Some(call_args_range) = raw_call_args_source_range(raw) else {
        return vec![RawStyleAssignment {
            path: path.to_owned(),
            value: raw.to_owned(),
            value_range: Some(raw_offset..raw_offset + raw.len()),
        }];
    };
    match callee.rsplit('.').next().unwrap_or(callee) {
        "text_style" | "dialogue_style" | "style" | "rich_text_style" => {
            raw_call_arg_ranges(&raw[call_args_range.clone()])
                .into_iter()
                .flat_map(|arg| {
                    let raw_arg_range =
                        call_args_range.start + arg.start..call_args_range.start + arg.end;
                    let trimmed_arg_range = trim_raw_range(raw, raw_arg_range);
                    if let Some((name, value)) =
                        split_named_raw_range(raw, trimmed_arg_range.clone())
                    {
                        let child = format!("{path}.{name}");
                        style_assignments_from_trimmed_raw(
                            &child,
                            &raw[value.clone()],
                            raw_offset + value.start,
                        )
                    } else {
                        style_assignments_from_trimmed_raw(
                            path,
                            &raw[trimmed_arg_range.clone()],
                            raw_offset + trimmed_arg_range.start,
                        )
                    }
                })
                .collect()
        }
        "ruby_style" | "layout_style" => raw_call_arg_ranges(&raw[call_args_range.clone()])
            .into_iter()
            .filter_map(|arg| {
                let raw_arg_range =
                    call_args_range.start + arg.start..call_args_range.start + arg.end;
                let trimmed_arg_range = trim_raw_range(raw, raw_arg_range);
                let (name, value) = split_named_raw_range(raw, trimmed_arg_range)?;
                Some(RawStyleAssignment {
                    path: format!("{path}.{name}"),
                    value: raw[value.clone()].trim().to_owned(),
                    value_range: Some(raw_offset + value.start..raw_offset + value.end),
                })
            })
            .collect(),
        _ => vec![RawStyleAssignment {
            path: path.to_owned(),
            value: raw.to_owned(),
            value_range: Some(raw_offset..raw_offset + raw.len()),
        }],
    }
}

fn trim_raw_range(source: &str, range: Range<usize>) -> Range<usize> {
    let raw = &source[range.clone()];
    let leading = raw.len() - raw.trim_start().len();
    let trailing = raw.len() - raw.trim_end().len();
    range.start + leading..range.end - trailing
}

fn split_named_raw_range(source: &str, range: Range<usize>) -> Option<(&str, Range<usize>)> {
    let raw = &source[range.clone()];
    let equals = find_top_level_raw_punctuation(raw, '=')?;
    let name = raw[..equals].trim();
    if name.is_empty() {
        return None;
    }
    let value = &raw[equals + '='.len_utf8()..];
    let value_leading = value.len() - value.trim_start().len();
    let value_trailing = value.len() - value.trim_end().len();
    Some((
        name,
        range.start + equals + '='.len_utf8() + value_leading..range.end - value_trailing,
    ))
}

fn source_with_relative_range(
    source: &RichTextSettingSource,
    value_range: Option<Range<usize>>,
) -> RichTextSettingSource {
    match (source, value_range) {
        (
            RichTextSettingSource::SourceFile {
                item_id,
                public_id,
                range: Some(source_range),
            },
            Some(value_range),
        ) => RichTextSettingSource::SourceFile {
            item_id: item_id.clone(),
            public_id: public_id.clone(),
            range: Some(RichTextSourceRange {
                start: source_range.start + value_range.start,
                end: source_range.start + value_range.end,
            }),
        },
        _ => source.clone(),
    }
}

fn raw_call_parts(raw: &str) -> Option<(&str, &str)> {
    let open = find_top_level_raw_punctuation(raw, '(')?;
    let close = raw.rfind(')')?;
    (close > open && raw[close + ')'.len_utf8()..].trim().is_empty())
        .then(|| (raw[..open].trim(), raw[open + '('.len_utf8()..close].trim()))
}

fn speaker_preset_arg_ranges(
    expr_source: Option<&str>,
    expr_range: Option<&TextRange>,
) -> BTreeMap<String, RichTextSourceRange> {
    let (Some(expr_source), Some(expr_range)) = (expr_source, expr_range) else {
        return BTreeMap::new();
    };
    let Some(args_range) = raw_call_args_source_range(expr_source) else {
        return BTreeMap::new();
    };
    let raw_args_source = &expr_source[args_range.clone()];
    raw_call_arg_ranges(raw_args_source)
        .into_iter()
        .filter_map(|arg_range| {
            let arg_text = &raw_args_source[arg_range.clone()];
            let leading = arg_text.len() - arg_text.trim_start().len();
            let trimmed = arg_text.trim();
            if trimmed.is_empty() {
                return None;
            }
            let equals = find_top_level_raw_punctuation(trimmed, '=')?;
            let name = trimmed[..equals].trim();
            let value = &trimmed[equals + '='.len_utf8()..];
            let value_leading = value.len() - value.trim_start().len();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                return None;
            }
            let value_start = args_range.start
                + arg_range.start
                + leading
                + equals
                + '='.len_utf8()
                + value_leading;
            Some((
                name.to_owned(),
                RichTextSourceRange {
                    start: expr_range.start() + value_start,
                    end: expr_range.start() + value_start + value.len(),
                },
            ))
        })
        .collect()
}

fn raw_call_args_source_range(raw: &str) -> Option<Range<usize>> {
    let open = find_top_level_raw_punctuation(raw, '(')?;
    let close = raw.rfind(')')?;
    (close > open && raw[close + ')'.len_utf8()..].trim().is_empty())
        .then(|| open + '('.len_utf8()..close)
}

fn raw_call_arg_ranges(source: &str) -> Vec<Range<usize>> {
    split_top_level_raw_ranges(source, ',')
        .into_iter()
        .filter(|range| !source[range.clone()].trim().is_empty())
        .collect()
}

fn split_top_level_raw_ranges(source: &str, delimiter: char) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth = depth.saturating_sub(1),
            _ if ch == delimiter && !in_string && depth == 0 => {
                ranges.push(start..offset);
                start = offset + ch.len_utf8();
            }
            _ => {}
        }
    }
    ranges.push(start..source.len());
    ranges
}

fn find_top_level_raw_punctuation(source: &str, needle: char) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            _ if ch == needle && !in_string && depth == 0 => return Some(offset),
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn nested_style_assignment_paths(path: &str, expr: &Expr) -> Vec<(String, String)> {
    let Expr::Call { callee, args } = expr else {
        return Vec::new();
    };
    match style_call_name(callee) {
        Some("text_style" | "dialogue_style" | "style" | "rich_text_style") => args
            .iter()
            .flat_map(|arg| match arg {
                CallArg::Named { name, value } => {
                    let child = format!("{path}.{name}");
                    style_assignment_paths(&child, value)
                }
                CallArg::Positional(value) => style_assignment_paths(path, value),
                CallArg::Spread { .. } => Vec::new(),
            })
            .collect(),
        Some("ruby_style" | "layout_style") => args
            .iter()
            .filter_map(|arg| match arg {
                CallArg::Named { name, value } => {
                    Some((format!("{path}.{name}"), expr_label(value)))
                }
                CallArg::Positional(_) | CallArg::Spread { .. } => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn mark_shadowed_style_contributions(contributions: &mut [RichTextStyleContribution]) {
    let mut latest_by_path = BTreeMap::<String, usize>::new();
    for index in 0..contributions.len() {
        if !contributions[index].active || contributions[index].op != RichTextAssignOp::Replace {
            continue;
        }
        if let Some(previous) = latest_by_path.insert(contributions[index].path.clone(), index) {
            contributions[previous].active = false;
            contributions[previous].shadowed_by = Some(index);
        }
    }
}

fn rich_text_assign_op(op: DialogueDefaultAssignOp) -> RichTextAssignOp {
    match op {
        DialogueDefaultAssignOp::Replace => RichTextAssignOp::Replace,
        DialogueDefaultAssignOp::Append => RichTextAssignOp::Append,
    }
}

fn dialogue_option_source(
    dialogue: &HirDialogue,
    range: Option<RichTextSourceRange>,
) -> RichTextSettingSource {
    RichTextSettingSource::SourceFile {
        item_id: dialogue.id().map(|id| id.body().to_owned()),
        public_id: dialogue.text_key().map(|id| id.body().to_owned()),
        range,
    }
}

fn source_file(
    item_id: Option<String>,
    range: Option<RichTextSourceRange>,
) -> RichTextSettingSource {
    RichTextSettingSource::SourceFile {
        public_id: item_id.clone(),
        item_id,
        range,
    }
}

fn style_assignment_source(
    item_id: Option<&str>,
    body_absolute_start: Option<usize>,
    body_relative_range: Range<usize>,
) -> RichTextSettingSource {
    let range = body_absolute_start.map(|start| RichTextSourceRange {
        start: start + body_relative_range.start,
        end: start + body_relative_range.end,
    });
    source_file(item_id.map(str::to_owned), range)
}

fn source_range(range: &arcweft_lang_hir::syntax::ast::common::TextRange) -> RichTextSourceRange {
    RichTextSourceRange {
        start: range.start(),
        end: range.end(),
    }
}

fn display_styles_from_expr(expr: &Expr) -> Vec<RichTextStyle> {
    let Expr::Call { callee, args } = expr else {
        return Vec::new();
    };
    let Some(name) = style_call_name(callee) else {
        return Vec::new();
    };
    match name {
        "font" => first_positional_value(args)
            .map(|attrs| RichTextStyle::Font {
                family: RichTextFontFamily::from_attrs(&attrs),
            })
            .into_iter()
            .collect(),
        "color" | "rgb" => first_positional_expr(args)
            .map(color_from_expr)
            .or_else(|| first_positional_value(args).map(|attrs| RichTextColor::from_attrs(&attrs)))
            .map(|value| RichTextStyle::Color { value })
            .into_iter()
            .collect(),
        "size" => first_positional_value(args)
            .map(|attrs| RichTextStyle::from_tag("size", &attrs))
            .into_iter()
            .collect(),
        "text_style" | "dialogue_style" | "style" | "rich_text_style" => args
            .iter()
            .flat_map(display_styles_from_style_arg)
            .collect(),
        "ruby_style" => ruby_layout_from_args(args)
            .map(|layout| RichTextStyle::Layout { layout })
            .into_iter()
            .collect(),
        "layout_style" => text_layout_from_args(args)
            .map(|layout| RichTextStyle::Layout { layout })
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn display_styles_from_style_arg(arg: &CallArg) -> Vec<RichTextStyle> {
    match arg {
        CallArg::Positional(expr) => display_styles_from_expr(expr),
        CallArg::Named { name, value } => display_styles_from_named_expr(name, value),
        CallArg::Spread { .. } => Vec::new(),
    }
}

fn display_styles_from_named_expr(name: &str, value: &Expr) -> Vec<RichTextStyle> {
    if let Some(field) = name.strip_prefix("rich_text.text.") {
        return display_styles_from_named_expr(field, value);
    }
    if let Some(field) = name.strip_prefix("rich_text.ruby.") {
        return ruby_layout_from_field(field, value)
            .map(|layout| RichTextStyle::Layout { layout })
            .into_iter()
            .collect();
    }
    if let Some(field) = name.strip_prefix("rich_text.layout.") {
        return text_layout_from_field(field, value)
            .map(|layout| RichTextStyle::Layout { layout })
            .into_iter()
            .collect();
    }
    let attrs = expr_style_value(value);
    match name {
        "style" | "text_style" | "dialogue_style" | "rich_text" | "text" | "layout" | "ruby" => {
            display_styles_from_expr(value)
        }
        "font" | "font_family" | "text_font" => vec![RichTextStyle::Font {
            family: RichTextFontFamily::from_attrs(&attrs),
        }],
        "color" | "text_color" | "read_text_color" | "unread_text_color" => {
            vec![RichTextStyle::Color {
                value: color_from_expr(value),
            }]
        }
        "size" | "text_size" => vec![RichTextStyle::from_tag("size", &attrs)],
        _ => Vec::new(),
    }
}

fn ruby_layout_from_args(args: &[CallArg]) -> Option<RichTextLayout> {
    let mut layout = RichTextLayout::default();
    let mut changed = false;
    for arg in args {
        if let CallArg::Named { name, value } = arg
            && apply_ruby_layout_field(&mut layout, name, value)
        {
            changed = true;
        }
    }
    changed.then_some(layout)
}

fn ruby_layout_from_field(field: &str, value: &Expr) -> Option<RichTextLayout> {
    let mut layout = RichTextLayout::default();
    apply_ruby_layout_field(&mut layout, field, value).then_some(layout)
}

fn apply_ruby_layout_field(layout: &mut RichTextLayout, field: &str, value: &Expr) -> bool {
    match field {
        "position" => {
            layout.ruby_position = ruby_position_from_value(&expr_style_value(value));
            true
        }
        "size" | "font_size" => {
            layout.ruby_font_size = Some(parse_milli_token(&expr_style_value(value)));
            true
        }
        "gap" => {
            layout.ruby_gap = Some(parse_milli_token(&expr_style_value(value)));
            true
        }
        "overhang" => {
            layout.ruby_overhang = Some(parse_milli_token(&expr_style_value(value)));
            true
        }
        "collision_gap" => {
            layout.ruby_collision_gap = Some(parse_milli_token(&expr_style_value(value)));
            true
        }
        _ => false,
    }
}

fn text_layout_from_args(args: &[CallArg]) -> Option<RichTextLayout> {
    let mut layout = RichTextLayout::default();
    let mut changed = false;
    for arg in args {
        if let CallArg::Named { name, value } = arg
            && apply_text_layout_field(&mut layout, name, value)
        {
            changed = true;
        }
    }
    changed.then_some(layout)
}

fn text_layout_from_field(field: &str, value: &Expr) -> Option<RichTextLayout> {
    let mut layout = RichTextLayout::default();
    apply_text_layout_field(&mut layout, field, value).then_some(layout)
}

fn apply_text_layout_field(layout: &mut RichTextLayout, field: &str, value: &Expr) -> bool {
    match field {
        "writing_mode" => {
            layout.writing_mode = writing_mode_from_value(&expr_style_value(value));
            true
        }
        "direction" | "dir" => {
            layout.direction = direction_from_value(&expr_style_value(value));
            true
        }
        "vertical_latin" | "latin" => {
            layout.vertical_latin = vertical_latin_from_value(&expr_style_value(value));
            true
        }
        "jlreq" | "jlreq_strictness" => {
            layout.jlreq_strictness = jlreq_from_value(&expr_style_value(value));
            true
        }
        "column_gap" => {
            layout.column_gap = parse_milli_token(&expr_style_value(value));
            true
        }
        _ => false,
    }
}

fn ruby_position_from_value(value: &str) -> RichTextRubyPosition {
    match value {
        "over" => RichTextRubyPosition::Over,
        "under" => RichTextRubyPosition::Under,
        "inter_character" => RichTextRubyPosition::InterCharacter,
        _ => RichTextRubyPosition::Auto,
    }
}

fn writing_mode_from_value(value: &str) -> RichTextWritingMode {
    match value {
        "vertical_rl" | "vertical" | "rl" => RichTextWritingMode::VerticalRl,
        "vertical_lr" | "lr" => RichTextWritingMode::VerticalLr,
        _ => RichTextWritingMode::HorizontalTb,
    }
}

fn direction_from_value(value: &str) -> RichTextInlineDirection {
    match value {
        "ltr" => RichTextInlineDirection::Ltr,
        "rtl" => RichTextInlineDirection::Rtl,
        _ => RichTextInlineDirection::Auto,
    }
}

fn vertical_latin_from_value(value: &str) -> RichTextVerticalLatinMode {
    match value {
        "upright" => RichTextVerticalLatinMode::Upright,
        "sideways" => RichTextVerticalLatinMode::Sideways,
        _ => RichTextVerticalLatinMode::Mixed,
    }
}

fn jlreq_from_value(value: &str) -> RichTextJlreqStrictness {
    match value {
        "loose" => RichTextJlreqStrictness::Loose,
        "normal" => RichTextJlreqStrictness::Normal,
        "strict" => RichTextJlreqStrictness::Strict,
        _ => RichTextJlreqStrictness::Auto,
    }
}

fn style_call_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Path(path) => Some(path.as_str()),
        Expr::Field { field, .. } => Some(field.as_str()),
        _ => None,
    }
}

fn first_positional_expr(args: &[CallArg]) -> Option<&Expr> {
    args.iter().find_map(|arg| match arg {
        CallArg::Positional(expr) => Some(expr),
        CallArg::Named { name, value } if name == "family" || name == "value" => Some(value),
        CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn first_positional_value(args: &[CallArg]) -> Option<String> {
    args.iter().find_map(|arg| match arg {
        CallArg::Positional(expr) => Some(expr_style_value(expr)),
        CallArg::Named { name, value } if name == "family" || name == "value" => {
            Some(expr_style_value(value))
        }
        CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn color_from_expr(expr: &Expr) -> RichTextColor {
    match expr {
        Expr::Call { callee, args } if matches!(style_call_name(callee), Some("rgb" | "color")) => {
            first_positional_expr(args)
                .map(expr_style_value)
                .map_or_else(
                    || RichTextColor::from_attrs(&expr_label(expr)),
                    |attrs| RichTextColor::from_attrs(&attrs),
                )
        }
        _ => RichTextColor::from_attrs(&expr_style_value(expr)),
    }
}

fn entity_ref_label(expr: &Expr) -> String {
    match expr {
        Expr::EntityRef(entity) => entity.body().to_owned(),
        _ => expr_style_value(expr).trim_start_matches('@').to_owned(),
    }
}

fn expr_style_value(expr: &Expr) -> String {
    match expr {
        Expr::Literal(Literal::String(value)) | Expr::Path(value) => value.clone(),
        Expr::Literal(literal) => crate::labels::literal_label(literal),
        Expr::EntityRef(entity) => format!("@{}", entity.body()),
        _ => expr_label(expr),
    }
}

fn lower_default_inline_failure_policy(args: &[LineArg]) -> Option<InlineFailurePolicy> {
    args.iter().find_map(|arg| match arg.name() {
        "inline_fallback" => Some(inline_fallback_from_expr(arg.value())),
        "inline_error" | "inline_error_policy" => {
            Some(inline_failure_policy_from_expr(arg.value()))
        }
        _ => None,
    })
}

fn inline_default_from_named_expr(name: &str, value: &Expr) -> Option<InlineFailurePolicy> {
    match name {
        "inline_fallback" => Some(inline_fallback_from_expr(value)),
        "inline_error" | "inline_error_policy" => Some(inline_failure_policy_from_expr(value)),
        _ => None,
    }
}

struct DialogueStyleBlock<'a> {
    source: &'a str,
    absolute_start: Option<usize>,
}

fn named_style_block<'a>(
    body: &'a str,
    body_range: Option<&arcweft_lang_hir::syntax::ast::common::TextRange>,
    name: &str,
) -> Option<DialogueStyleBlock<'a>> {
    let start = body.find(name)?;
    let open = body[start..].find('{')? + start;
    let close = matching_brace(body, open)?;
    let raw_block = &body[open + 1..close];
    let leading = raw_block.len() - raw_block.trim_start().len();
    let source = raw_block.trim();
    Some(DialogueStyleBlock {
        source,
        absolute_start: body_range.map(|range| range.start() + open + 1 + leading),
    })
}

fn matching_brace(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in source[open..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

struct StyleBlockAssignment<'a> {
    name: String,
    value: &'a str,
    value_range: Range<usize>,
}

struct LogicalStyleItem<'a> {
    source: &'a str,
    range: Range<usize>,
}

fn style_block_assignments<'a>(
    body: &'a str,
    path_prefix: Option<&str>,
) -> Vec<StyleBlockAssignment<'a>> {
    nested_style_block_assignments(body, path_prefix)
}

fn nested_style_block_assignments<'a>(
    body: &'a str,
    path_prefix: Option<&str>,
) -> Vec<StyleBlockAssignment<'a>> {
    logical_style_items(body)
        .iter()
        .flat_map(|item| style_item_assignments(body, item, path_prefix))
        .collect()
}

fn style_item_assignments<'a>(
    body: &'a str,
    item: &LogicalStyleItem<'a>,
    path_prefix: Option<&str>,
) -> Vec<StyleBlockAssignment<'a>> {
    if let Some(assignment) = split_assignment(item, path_prefix) {
        return vec![assignment];
    }
    let Some((head, nested_body, nested_start)) = split_nested_style_block(body, item) else {
        return Vec::new();
    };
    let next_prefix =
        path_prefix.map_or_else(|| head.to_owned(), |prefix| format!("{prefix}.{head}"));
    nested_style_block_assignments(nested_body, Some(&next_prefix))
        .into_iter()
        .map(|assignment| StyleBlockAssignment {
            name: assignment.name,
            value: assignment.value,
            value_range: assignment.value_range.start + nested_start
                ..assignment.value_range.end + nested_start,
        })
        .collect()
}

fn logical_style_items(body: &str) -> Vec<LogicalStyleItem<'_>> {
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in body.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth = depth.saturating_sub(1),
            '\n' if !in_string && depth == 0 => {
                let item = trim_logical_style_item(body, start, offset);
                if !item.source.is_empty() && !item.source.starts_with("//") {
                    items.push(item);
                }
                start = offset + '\n'.len_utf8();
            }
            _ => {}
        }
    }
    let tail = trim_logical_style_item(body, start, body.len());
    if !tail.source.is_empty() && !tail.source.starts_with("//") {
        items.push(tail);
    }
    items
}

fn trim_logical_style_item(body: &str, start: usize, end: usize) -> LogicalStyleItem<'_> {
    let raw = &body[start..end];
    let leading = raw.len() - raw.trim_start().len();
    let source = raw.trim();
    LogicalStyleItem {
        source,
        range: start + leading..start + leading + source.len(),
    }
}

fn split_assignment<'a>(
    item: &LogicalStyleItem<'a>,
    path_prefix: Option<&str>,
) -> Option<StyleBlockAssignment<'a>> {
    let equals = find_top_level_raw_punctuation(item.source, '=')?;
    let name = item.source[..equals].trim();
    let value_source = &item.source[equals + '='.len_utf8()..];
    let value_trimmed_start = value_source.trim_start();
    let leading = value_source.len() - value_trimmed_start.len();
    let value = value_trimmed_start.trim_end_matches(',').trim_end();
    let value_start = item.range.start + equals + '='.len_utf8() + leading;
    (!name.is_empty() && !value.is_empty()).then_some(StyleBlockAssignment {
        name: path_prefix.map_or_else(|| name.to_owned(), |prefix| format!("{prefix}.{name}")),
        value,
        value_range: value_start..value_start + value.len(),
    })
}

fn split_nested_style_block<'a>(
    body: &'a str,
    item: &LogicalStyleItem<'a>,
) -> Option<(&'a str, &'a str, usize)> {
    let open_in_item = item.source.find('{')?;
    let close_in_item = matching_brace(item.source, open_in_item)?;
    if !item.source[close_in_item + '}'.len_utf8()..]
        .trim()
        .is_empty()
    {
        return None;
    }
    let head = item.source[..open_in_item].trim();
    if head.is_empty() || head.contains(char::is_whitespace) {
        return None;
    }
    let inner_start = item.range.start + open_in_item + '{'.len_utf8();
    let inner_end = item.range.start + close_in_item;
    Some((head, &body[inner_start..inner_end], inner_start))
}

fn lower_dialogue_token(
    token: &DialogueToken,
    default_inline_failure_policy: Option<&InlineFailurePolicy>,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> Vec<RichTextNode> {
    match token {
        DialogueToken::Text(text) => vec![RichTextNode::Text { text: text.clone() }],
        DialogueToken::Raw(text) => {
            vec![RichTextNode::Control {
                control: RichTextControl::Raw { text: text.clone() },
            }]
        }
        DialogueToken::Tag(tag) => lower_tag(tag, text_proxies),
        DialogueToken::InferredTag(tag) => lower_inferred_tag(tag, text_proxies),
        DialogueToken::Mark(mark) => {
            vec![RichTextNode::Control {
                control: RichTextControl::Mark {
                    name: mark.name().to_owned(),
                },
            }]
        }
        DialogueToken::EndTag(name) => {
            vec![RichTextNode::StyleEnd {
                name: canonical_end_tag(name).to_owned(),
            }]
        }
        DialogueToken::InferredEndTag => {
            vec![RichTextNode::StyleEnd {
                name: "/".to_owned(),
            }]
        }
        DialogueToken::Expr(expr) => {
            vec![RichTextNode::Interpolation {
                expr: expr_label(expr),
                fallback_source: inline_fallback_source_label(expr),
                on_error: inline_failure_policy(expr, default_inline_failure_policy),
            }]
        }
        DialogueToken::Ruby { base, ruby } => {
            vec![RichTextNode::Ruby {
                base: base.clone(),
                ruby: ruby.clone(),
            }]
        }
        DialogueToken::Escape(ch) => vec![RichTextNode::Text {
            text: ch.to_string(),
        }],
    }
}

fn lower_tag(
    tag: &DialogueTag,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> Vec<RichTextNode> {
    match tag.name() {
        "p" | "page" => vec![RichTextNode::Control {
            control: RichTextControl::Page,
        }],
        "l" | "wait" => vec![RichTextNode::Control {
            control: RichTextControl::LineWait,
        }],
        "r" | "br" | "nl" => vec![RichTextNode::Control {
            control: RichTextControl::HardBreak,
        }],
        "w" => vec![RichTextNode::Control {
            control: RichTextControl::TimedWait {
                value: tag.attrs().to_owned(),
            },
        }],
        "clear" | "er" | "cm" => vec![RichTextNode::Control {
            control: RichTextControl::Clear,
        }],
        "reset" => vec![RichTextNode::Control {
            control: RichTextControl::Reset,
        }],
        "em" | "strong" | "color" | "font" | "size" | "speed" | "i" | "italic" | "oblique"
        | "slant" => {
            vec![RichTextNode::StyleStart {
                style: RichTextStyle::from_tag(tag.name(), tag.attrs()),
            }]
        }
        "style" => lower_style_tag(tag),
        "layout" => lower_layout_tag(tag),
        "transform" => lower_transform_tag(tag),
        "object" => lower_object_tag(tag, text_proxies),
        "effect" | "fx" => lower_effect_tag(tag),
        "voice" => host_event(DialogueHostEvent::Voice {
            attrs: tag.attrs().to_owned(),
        }),
        "face" => host_event(DialogueHostEvent::Face {
            attrs: tag.attrs().to_owned(),
        }),
        "pose" => host_event(DialogueHostEvent::Pose {
            attrs: tag.attrs().to_owned(),
        }),
        "show" => host_event(DialogueHostEvent::Show {
            attrs: tag.attrs().to_owned(),
        }),
        "hide" => host_event(DialogueHostEvent::Hide {
            attrs: tag.attrs().to_owned(),
        }),
        "move" => host_event(DialogueHostEvent::Move {
            attrs: tag.attrs().to_owned(),
        }),
        "scale" => host_event(DialogueHostEvent::Scale {
            attrs: tag.attrs().to_owned(),
        }),
        "rotate" => host_event(DialogueHostEvent::Rotate {
            attrs: tag.attrs().to_owned(),
        }),
        "anim" => host_event(DialogueHostEvent::Anim {
            attrs: tag.attrs().to_owned(),
        }),
        "shake" => host_event(DialogueHostEvent::Shake {
            attrs: tag.attrs().to_owned(),
        }),
        "at" => host_event(DialogueHostEvent::TimedCue {
            attrs: tag.attrs().to_owned(),
        }),
        "call" => host_event(DialogueHostEvent::Call {
            attrs: tag.attrs().to_owned(),
        }),
        "signal" => host_event(DialogueHostEvent::Signal {
            attrs: tag.attrs().to_owned(),
        }),
        "if" | "else" | "endif" => host_event(DialogueHostEvent::Conditional {
            name: tag.name().to_owned(),
            attrs: tag.attrs().to_owned(),
        }),
        name => vec![RichTextNode::Control {
            control: RichTextControl::Unknown {
                name: name.to_owned(),
                attrs: tag.attrs().to_owned(),
            },
        }],
    }
}

fn lower_inferred_tag(
    tag: &DialogueTag,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> Vec<RichTextNode> {
    let selector = tag.name().trim_start_matches('.');
    if inferred_text_proxy_type(selector, tag.attrs(), text_proxies) {
        return lower_object_selector(selector, tag.attrs(), text_proxies);
    }
    match inferred_tag_family(selector, tag.attrs()) {
        Some(InferredTagFamily::Style) => lower_style_selector(selector, tag.attrs()),
        Some(InferredTagFamily::Layout) => lower_layout_selector(selector, tag.attrs()),
        Some(InferredTagFamily::Transform) => lower_transform_selector(selector, tag.attrs()),
        Some(InferredTagFamily::Effect) => lower_effect_selector(selector, tag.attrs()),
        Some(InferredTagFamily::Marker) | None => {
            vec![RichTextNode::Control {
                control: RichTextControl::Mark {
                    name: tag.name().to_owned(),
                },
            }]
        }
    }
}

fn inferred_text_proxy_type(
    selector: &str,
    attrs: &str,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> bool {
    let attrs = parse_attrs(attrs);
    object_proxy_type_name_attr(&attrs)
        .is_some_and(|type_name| text_proxies.contains_key(&type_name))
        || text_proxies.contains_key(selector)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InferredTagFamily {
    Style,
    Layout,
    Transform,
    Effect,
    Marker,
}

fn inferred_tag_family(selector: &str, attrs: &str) -> Option<InferredTagFamily> {
    match selector {
        "italic" | "oblique" | "opacity" | "alpha" | "layer" | "object_layer" | "meta"
        | "metadata" | "data" | "z" | "z_index" => Some(InferredTagFamily::Style),
        "horizontal_tb"
        | "vertical_rl"
        | "vertical_lr"
        | "dir"
        | "ruby_over"
        | "ruby_under"
        | "ruby_inter_character" => Some(InferredTagFamily::Layout),
        "offset" | "pos" | "rotate" | "scale" | "skew" => Some(InferredTagFamily::Transform),
        "wave" | "shake" | "arc" | "spin" | "pulse" | "motion" | "typewriter" | "jitter"
        | "shader" | "host" => Some(InferredTagFamily::Effect),
        "mark" => Some(InferredTagFamily::Marker),
        _ if !attrs.trim().is_empty() => Some(InferredTagFamily::Effect),
        _ => None,
    }
}

fn lower_style_tag(tag: &DialogueTag) -> Vec<RichTextNode> {
    let (selector, attrs) = split_selector_attrs(tag.attrs());
    lower_style_selector(selector.trim_start_matches('.'), attrs)
}

fn lower_style_selector(selector: &str, attrs: &str) -> Vec<RichTextNode> {
    let style = match selector {
        "italic" | "i" => RichTextStyle::Italic {
            attrs: attrs.to_owned(),
        },
        "oblique" | "slant" => RichTextStyle::Oblique {
            angle: angle_from_attrs(attrs, "deg").unwrap_or_else(|| RichTextAngle {
                degrees: parse_milli_token(attrs),
            }),
            raw: attrs.to_owned(),
        },
        "opacity" | "alpha" => RichTextStyle::Presentation {
            presentation: RichTextPresentationStyle {
                opacity: Some(parse_milli_token(&style_scalar_attr(attrs, "opacity"))),
                layer: None,
                params: BTreeMap::new(),
                z_index: None,
            },
        },
        "layer" | "object_layer" => RichTextStyle::Presentation {
            presentation: RichTextPresentationStyle {
                opacity: None,
                layer: style_layer_attr(attrs),
                params: BTreeMap::new(),
                z_index: None,
            },
        },
        "meta" | "metadata" | "data" => RichTextStyle::Presentation {
            presentation: RichTextPresentationStyle {
                opacity: None,
                layer: None,
                params: parse_attrs(attrs)
                    .into_iter()
                    .map(|(key, value)| (key, param_from_value(&value)))
                    .collect(),
                z_index: None,
            },
        },
        "z" | "z_index" => RichTextStyle::Presentation {
            presentation: RichTextPresentationStyle {
                opacity: None,
                layer: None,
                params: BTreeMap::new(),
                z_index: parse_z_index_token(&style_scalar_attr(attrs, "z_index")),
            },
        },
        _ => RichTextStyle::Unknown {
            name: "style".to_owned(),
            attrs: attrs.to_owned(),
        },
    };
    vec![RichTextNode::StyleStart { style }]
}

fn style_scalar_attr(attrs: &str, preferred: &str) -> String {
    let parsed = parse_attrs(attrs);
    parsed
        .get(preferred)
        .or_else(|| match preferred {
            "opacity" => parsed.get("alpha"),
            "z_index" => parsed.get("z"),
            "layer" => parsed.get("object_layer"),
            _ => None,
        })
        .or_else(|| parsed.get("value"))
        .or_else(|| parsed.get("amount"))
        .map_or_else(|| attrs.to_owned(), ToOwned::to_owned)
}

fn style_layer_attr(attrs: &str) -> Option<String> {
    let value = style_scalar_attr(attrs, "layer");
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}

fn lower_layout_tag(tag: &DialogueTag) -> Vec<RichTextNode> {
    let (selector, attrs) = split_selector_attrs(tag.attrs());
    lower_layout_selector(selector.trim_start_matches('.'), attrs)
}

fn lower_layout_selector(selector: &str, attrs: &str) -> Vec<RichTextNode> {
    vec![RichTextNode::StyleStart {
        style: RichTextStyle::Layout {
            layout: layout_from_selector(selector, attrs),
        },
    }]
}

fn lower_transform_tag(tag: &DialogueTag) -> Vec<RichTextNode> {
    let (selector, attrs) = split_selector_attrs(tag.attrs());
    lower_transform_selector(selector.trim_start_matches('.'), attrs)
}

fn lower_transform_selector(selector: &str, attrs: &str) -> Vec<RichTextNode> {
    vec![RichTextNode::StyleStart {
        style: RichTextStyle::Transform {
            transform: transform_from_selector(selector, attrs),
        },
    }]
}

fn lower_effect_tag(tag: &DialogueTag) -> Vec<RichTextNode> {
    let (selector, attrs) = split_selector_attrs(tag.attrs());
    lower_effect_selector(selector.trim_start_matches('.'), attrs)
}

fn lower_effect_selector(selector: &str, attrs: &str) -> Vec<RichTextNode> {
    if effect_selector_is_host_event(attrs) {
        return host_event(DialogueHostEvent::Effect {
            id: host_event_effect_id(selector, attrs),
            attrs: attrs.trim().to_owned(),
        });
    }
    if selector == "shader" {
        return vec![RichTextNode::StyleStart {
            style: RichTextStyle::Shader {
                shader: shader_from_attrs(attrs),
            },
        }];
    }
    vec![RichTextNode::StyleStart {
        style: RichTextStyle::Effect {
            effect: effect_from_selector(selector, attrs),
        },
    }]
}

fn effect_selector_is_host_event(attrs: &str) -> bool {
    phase_attr(&parse_attrs(attrs)) == Some(RichTextEffectPhase::HostEvent)
}

fn host_event_effect_id(selector: &str, attrs: &str) -> String {
    let attrs = parse_attrs(attrs);
    match selector {
        "host" => effect_descriptor_id(selector, &attrs),
        "shader" => attrs
            .get("id")
            .map(|value| trim_quotes(value).trim_start_matches('.').to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| selector.to_owned()),
        _ => selector.to_owned(),
    }
}

fn lower_object_tag(
    tag: &DialogueTag,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> Vec<RichTextNode> {
    let (selector, attrs) = split_selector_attrs(tag.attrs());
    lower_object_selector(selector.trim_start_matches('.'), attrs, text_proxies)
}

fn lower_object_selector(
    selector: &str,
    attrs: &str,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> Vec<RichTextNode> {
    vec![RichTextNode::StyleStart {
        style: RichTextStyle::Object {
            proxy: object_proxy_from_selector(selector, attrs, text_proxies),
        },
    }]
}

fn object_proxy_from_selector(
    selector: &str,
    attrs: &str,
    text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
) -> RichTextObjectProxy {
    let attrs = parse_attrs(attrs);
    let explicit_type_name = object_proxy_type_name_attr(&attrs);
    let defaults = explicit_type_name
        .as_deref()
        .and_then(|type_name| text_proxies.get(type_name))
        .or_else(|| text_proxies.get(selector));
    let id = if selector.is_empty() {
        attrs
            .get("id")
            .or_else(|| attrs.get("name"))
            .map(|value| trim_quotes(value).trim_start_matches('.').to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "text_object".to_owned())
    } else {
        selector.to_owned()
    };
    let mut params = defaults.map_or_else(BTreeMap::new, |defaults| defaults.params.clone());
    params.extend(
        attrs
            .iter()
            .filter(|(key, _)| !is_object_proxy_metadata_attr(key))
            .map(|(key, value)| (key.clone(), param_from_value(value))),
    );
    RichTextObjectProxy {
        id,
        declaration: defaults.map(|defaults| defaults.declaration.clone()),
        type_name: explicit_type_name
            .or_else(|| defaults.map(|defaults| defaults.type_name.clone())),
        role: attrs
            .get("role")
            .or_else(|| attrs.get("kind"))
            .map(|value| trim_quotes(value).to_owned())
            .filter(|value| !value.is_empty())
            .or_else(|| defaults.and_then(|defaults| defaults.role.clone())),
        layer: attrs
            .get("layer")
            .or_else(|| attrs.get("object_layer"))
            .map(|value| trim_quotes(value).trim_start_matches('.').to_owned())
            .filter(|value| !value.is_empty())
            .or_else(|| defaults.and_then(|defaults| defaults.layer.clone())),
        depth: attrs
            .get("depth")
            .or_else(|| attrs.get("z"))
            .or_else(|| attrs.get("z_index"))
            .map(|value| parse_milli_token(value))
            .or_else(|| defaults.and_then(|defaults| defaults.depth)),
        hit_test: attrs
            .get("hit")
            .or_else(|| attrs.get("hit_test"))
            .map(|value| truthy_attr(value))
            .or_else(|| defaults.and_then(|defaults| defaults.default_hit))
            .unwrap_or(false),
        params,
    }
}

fn object_proxy_type_name_attr(attrs: &BTreeMap<String, String>) -> Option<String> {
    attrs
        .get("type")
        .or_else(|| attrs.get("struct"))
        .or_else(|| attrs.get("proxy"))
        .map(|value| trim_quotes(value).to_owned())
        .filter(|value| !value.is_empty())
}

fn is_object_proxy_metadata_attr(key: &str) -> bool {
    matches!(
        key,
        "id" | "name"
            | "kind"
            | "type"
            | "struct"
            | "proxy"
            | "role"
            | "layer"
            | "object_layer"
            | "depth"
            | "z"
            | "z_index"
            | "hit"
            | "hit_test"
    )
}

fn canonical_end_tag(name: &str) -> &str {
    match name {
        "style" | "italic" | "i" | "oblique" | "slant" => "style",
        "layout"
        | "vertical"
        | "vertical_rl"
        | "vertical_lr"
        | "horizontal_tb"
        | "ruby_over"
        | "ruby_under"
        | "ruby_inter_character" => "layout",
        "transform" | "offset" | "pos" | "rotate" | "scale" | "skew" => "transform",
        "object" => "object",
        "effect" | "fx" | "wave" | "shake" | "arc" | "spin" | "pulse" | "motion" | "typewriter"
        | "jitter" | "shader" | "host" => "effect",
        other => other,
    }
}

fn split_selector_attrs(attrs: &str) -> (&str, &str) {
    let attrs = attrs.trim();
    let mut parts = attrs.splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or_default();
    if first.starts_with('.') {
        (first, parts.next().unwrap_or_default().trim())
    } else {
        ("", attrs)
    }
}

fn layout_from_selector(selector: &str, attrs: &str) -> RichTextLayout {
    let attrs = parse_attrs(attrs);
    let ruby_selector = matches!(
        selector,
        "ruby_over" | "ruby_under" | "ruby_inter_character"
    );
    RichTextLayout {
        writing_mode: match selector {
            "vertical_rl" | "vertical" => RichTextWritingMode::VerticalRl,
            "vertical_lr" => RichTextWritingMode::VerticalLr,
            "horizontal_tb" => RichTextWritingMode::HorizontalTb,
            _ => match attrs.get("mode").map(String::as_str) {
                Some("vertical_rl" | "vertical" | "rl") => RichTextWritingMode::VerticalRl,
                Some("vertical_lr" | "lr") => RichTextWritingMode::VerticalLr,
                _ => RichTextWritingMode::HorizontalTb,
            },
        },
        direction: match attrs
            .get("dir")
            .or_else(|| attrs.get("value"))
            .map(String::as_str)
        {
            Some("ltr") => RichTextInlineDirection::Ltr,
            Some("rtl") => RichTextInlineDirection::Rtl,
            _ => RichTextInlineDirection::Auto,
        },
        vertical_latin: match attrs.get("latin").map(String::as_str) {
            Some("upright") => RichTextVerticalLatinMode::Upright,
            Some("sideways") => RichTextVerticalLatinMode::Sideways,
            _ => RichTextVerticalLatinMode::Mixed,
        },
        ruby_position: match selector {
            "ruby_over" => RichTextRubyPosition::Over,
            "ruby_under" => RichTextRubyPosition::Under,
            "ruby_inter_character" => RichTextRubyPosition::InterCharacter,
            _ => RichTextRubyPosition::Auto,
        },
        jlreq_strictness: jlreq_strictness_attr(&attrs),
        column_gap: column_gap_attr(&attrs, ruby_selector).unwrap_or(Milli(8000)),
        ruby_font_size: ruby_milli_attr(&attrs, ruby_selector, "ruby_size", "size"),
        ruby_gap: ruby_milli_attr(&attrs, ruby_selector, "ruby_gap", "gap"),
        ruby_overhang: attrs
            .get("ruby_overhang")
            .or_else(|| attrs.get("overhang"))
            .map(|value| parse_milli_token(value)),
        ruby_collision_gap: attrs
            .get("ruby_collision_gap")
            .or_else(|| attrs.get("collision_gap"))
            .map(|value| parse_milli_token(value)),
    }
}

fn column_gap_attr(attrs: &BTreeMap<String, String>, ruby_selector: bool) -> Option<Milli> {
    attrs
        .get("column_gap")
        .or_else(|| {
            if ruby_selector {
                None
            } else {
                attrs.get("gap")
            }
        })
        .map(|value| parse_milli_token(value))
}

fn ruby_milli_attr(
    attrs: &BTreeMap<String, String>,
    ruby_selector: bool,
    explicit_name: &str,
    ruby_selector_short_name: &str,
) -> Option<Milli> {
    attrs
        .get(explicit_name)
        .or_else(|| {
            if ruby_selector {
                attrs.get(ruby_selector_short_name)
            } else {
                None
            }
        })
        .map(|value| parse_milli_token(value))
}

fn jlreq_strictness_attr(attrs: &BTreeMap<String, String>) -> RichTextJlreqStrictness {
    match attrs
        .get("jlreq")
        .or_else(|| attrs.get("strictness"))
        .or_else(|| attrs.get("kinsoku"))
        .map(String::as_str)
    {
        Some("loose") => RichTextJlreqStrictness::Loose,
        Some("normal") => RichTextJlreqStrictness::Normal,
        Some("strict") => RichTextJlreqStrictness::Strict,
        _ => RichTextJlreqStrictness::Auto,
    }
}

fn transform_from_selector(selector: &str, attrs: &str) -> RichTextTransform {
    let raw_attrs = attrs;
    let attrs = parse_attrs(attrs);
    let mut transform = RichTextTransform::default();
    match selector {
        "offset" | "pos" => {
            transform.translate = RichTextVec2::new(
                milli_attr(&attrs, "x").unwrap_or_default(),
                milli_attr(&attrs, "y").unwrap_or_default(),
            );
        }
        "rotate" => {
            transform.rotate = transform_angle_attr(&attrs, raw_attrs).unwrap_or_default();
            transform.origin = RichTextTransformOrigin::Center;
        }
        "scale" => {
            transform.scale = RichTextVec2::new(
                milli_attr(&attrs, "x").unwrap_or(Milli::ONE),
                milli_attr(&attrs, "y").unwrap_or(Milli::ONE),
            );
            transform.origin = RichTextTransformOrigin::Center;
        }
        "skew" => {
            transform.skew = RichTextVec2::new(
                milli_attr(&attrs, "x").unwrap_or_default(),
                milli_attr(&attrs, "y").unwrap_or_default(),
            );
        }
        _ => {}
    }
    transform.target = target_attr(&attrs);
    if let Some(origin) = transform_origin_attr(&attrs) {
        transform.origin = origin;
    }
    transform
}

fn effect_from_selector(selector: &str, attrs: &str) -> RichTextEffectDescriptor {
    let attrs = parse_attrs(attrs);
    let id = effect_descriptor_id(selector, &attrs);
    RichTextEffectDescriptor {
        id,
        params: attrs
            .iter()
            .filter(|(key, _)| !is_effect_descriptor_metadata_attr(selector, key))
            .map(|(key, value)| (key.clone(), param_from_value(value)))
            .collect(),
        target: target_attr(&attrs),
        phase: phase_attr(&attrs).unwrap_or_else(|| default_effect_phase(selector)),
        state_scope: state_scope_attr(&attrs),
    }
}

fn effect_descriptor_id(selector: &str, attrs: &BTreeMap<String, String>) -> String {
    if selector == "host" {
        return attrs
            .get("id")
            .or_else(|| attrs.get("effect"))
            .or_else(|| attrs.get("name"))
            .map(|value| trim_quotes(value).trim_start_matches('.').to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| selector.to_owned());
    }
    selector.to_owned()
}

fn is_effect_descriptor_metadata_attr(selector: &str, key: &str) -> bool {
    matches!(key, "target" | "phase" | "state" | "scope" | "state_scope")
        || (selector == "host" && matches!(key, "id" | "effect" | "name"))
}

fn shader_from_attrs(attrs: &str) -> RichTextShaderRef {
    let attrs = parse_attrs(attrs);
    RichTextShaderRef {
        id: attrs.get("id").cloned().unwrap_or_default(),
        params: attrs
            .iter()
            .filter(|(key, _)| !matches!(key.as_str(), "id" | "phase"))
            .map(|(key, value)| (key.clone(), param_from_value(value)))
            .collect(),
        phase: phase_attr(&attrs).unwrap_or(RichTextEffectPhase::RunOffscreenPass),
    }
}

fn default_effect_phase(selector: &str) -> RichTextEffectPhase {
    match selector {
        "shader" => RichTextEffectPhase::RunOffscreenPass,
        "typewriter" => RichTextEffectPhase::GlyphMask,
        _ => RichTextEffectPhase::GlyphTransform,
    }
}

fn target_attr(attrs: &BTreeMap<String, String>) -> RichTextEffectTarget {
    match attrs.get("target").map(String::as_str) {
        Some("document") => RichTextEffectTarget::Document,
        Some("line") => RichTextEffectTarget::Line,
        Some("sentence") => RichTextEffectTarget::Sentence,
        Some("glyph") => RichTextEffectTarget::Glyph,
        Some("textbox" | "box") => RichTextEffectTarget::TextBox,
        Some("screen") => RichTextEffectTarget::Screen,
        _ => RichTextEffectTarget::Run,
    }
}

fn transform_origin_attr(attrs: &BTreeMap<String, String>) -> Option<RichTextTransformOrigin> {
    match attrs.get("origin").map(String::as_str)? {
        "baseline_start" | "start" => Some(RichTextTransformOrigin::BaselineStart),
        "baseline_center" => Some(RichTextTransformOrigin::BaselineCenter),
        "center" => Some(RichTextTransformOrigin::Center),
        "glyph_center" | "glyph" => Some(RichTextTransformOrigin::GlyphCenter),
        _ => None,
    }
}

fn state_scope_attr(attrs: &BTreeMap<String, String>) -> RichTextStateScope {
    match attrs
        .get("state_scope")
        .or_else(|| attrs.get("state"))
        .or_else(|| attrs.get("scope"))
        .map(String::as_str)
    {
        Some("glyph") => RichTextStateScope::Glyph,
        Some("line") => RichTextStateScope::Line,
        Some("sentence") => RichTextStateScope::Sentence,
        Some("paragraph") => RichTextStateScope::Paragraph,
        Some("document") => RichTextStateScope::Document,
        Some("dialogue_line") => RichTextStateScope::DialogueLine,
        Some("speaker") => RichTextStateScope::Speaker,
        Some("window") => RichTextStateScope::Window,
        Some("global") => RichTextStateScope::Global,
        _ => RichTextStateScope::Run,
    }
}

fn phase_attr(attrs: &BTreeMap<String, String>) -> Option<RichTextEffectPhase> {
    match attrs.get("phase").map(String::as_str)? {
        "before_layout" => Some(RichTextEffectPhase::BeforeLayout),
        "layout_transform" => Some(RichTextEffectPhase::LayoutTransform),
        "glyph_transform" => Some(RichTextEffectPhase::GlyphTransform),
        "glyph_color" => Some(RichTextEffectPhase::GlyphColor),
        "glyph_mask" => Some(RichTextEffectPhase::GlyphMask),
        "run_offscreen" | "run_offscreen_pass" => Some(RichTextEffectPhase::RunOffscreenPass),
        "post_process" => Some(RichTextEffectPhase::PostProcess),
        "host_event" => Some(RichTextEffectPhase::HostEvent),
        _ => None,
    }
}

fn param_from_value(value: &str) -> RichTextParam {
    let value = trim_quotes(value);
    if value == "true" {
        return RichTextParam::Bool { value: true };
    }
    if value == "false" {
        return RichTextParam::Bool { value: false };
    }
    if let Some(selector) = value.strip_prefix('.') {
        return RichTextParam::Selector {
            value: format!(".{selector}"),
        };
    }
    if let Ok(parsed) = value.parse::<i64>() {
        return RichTextParam::Int { value: parsed };
    }
    if let Some(milli) = parse_param_milli(value) {
        return RichTextParam::Milli { value: milli };
    }
    RichTextParam::Raw {
        value: value.to_owned(),
    }
}

fn parse_param_milli(value: &str) -> Option<Milli> {
    let trimmed = value.trim();
    let numeric = trimmed
        .strip_suffix("px")
        .or_else(|| trimmed.strip_suffix("deg"))
        .or_else(|| trimmed.strip_suffix("ch"))
        .unwrap_or(trimmed)
        .trim();
    parse_decimal_milli(numeric)
}

fn angle_from_attrs(attrs: &str, name: &str) -> Option<RichTextAngle> {
    angle_from_attrs_map(&parse_attrs(attrs), name)
}

fn transform_angle_attr(
    attrs: &BTreeMap<String, String>,
    raw_attrs: &str,
) -> Option<RichTextAngle> {
    angle_from_attrs_map(attrs, "angle")
        .or_else(|| angle_from_attrs_map(attrs, "deg"))
        .or_else(|| positional_angle_attr(raw_attrs))
}

fn angle_from_attrs_map(attrs: &BTreeMap<String, String>, name: &str) -> Option<RichTextAngle> {
    attrs.get(name).map(|value| RichTextAngle {
        degrees: parse_milli_token(value),
    })
}

fn positional_angle_attr(raw_attrs: &str) -> Option<RichTextAngle> {
    raw_attrs
        .split_whitespace()
        .find(|item| !item.contains('='))
        .map(|value| RichTextAngle {
            degrees: parse_milli_token(value),
        })
}

fn milli_attr(attrs: &BTreeMap<String, String>, name: &str) -> Option<Milli> {
    attrs.get(name).map(|value| parse_milli_token(value))
}

fn parse_attrs(source: &str) -> BTreeMap<String, String> {
    source
        .split_whitespace()
        .filter_map(|item| {
            let (key, value) = item.split_once('=')?;
            Some((key.to_owned(), trim_quotes(value).to_owned()))
        })
        .collect()
}

fn parse_attr_args(source: &str) -> BTreeMap<String, String> {
    split_attr_items(source)
        .into_iter()
        .filter_map(|item| {
            let (key, value) = item.as_str().split_once('=')?;
            Some((key.to_owned(), trim_quotes(value).to_owned()))
        })
        .collect()
}

fn split_attr_items(source: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in source.chars() {
        match (quote, ch) {
            (Some(active), next) if next == active => {
                quote = None;
                current.push(ch);
            }
            (None, '"' | '\'') => {
                quote = Some(ch);
                current.push(ch);
            }
            (None, ',' | ';') => {
                push_attr_item(&mut items, &mut current);
            }
            (None, next) if next.is_whitespace() => {
                push_attr_item(&mut items, &mut current);
            }
            _ => current.push(ch),
        }
    }
    push_attr_item(&mut items, &mut current);
    items
}

fn push_attr_item(items: &mut Vec<String>, current: &mut String) {
    let item = current.trim();
    if !item.is_empty() {
        items.push(item.to_owned());
    }
    current.clear();
}

fn truthy_attr(value: &str) -> bool {
    matches!(trim_quotes(value), "true" | "yes" | "1" | "on")
}

fn trim_quotes(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn host_event(event: DialogueHostEvent) -> Vec<RichTextNode> {
    vec![RichTextNode::HostEvent { event }]
}

fn inline_failure_policy(
    expr: &Expr,
    default: Option<&InlineFailurePolicy>,
) -> InlineFailurePolicy {
    match expr {
        Expr::Call { args, .. } | Expr::MethodCall { args, .. } => {
            inline_failure_policy_from_args(args)
                .or_else(|| default.cloned())
                .unwrap_or(InlineFailurePolicy::FailLine)
        }
        _ => default.cloned().unwrap_or(InlineFailurePolicy::FailLine),
    }
}

fn inline_fallback_source_label(expr: &Expr) -> String {
    match expr {
        Expr::Call { args, .. } | Expr::MethodCall { args, .. } => args
            .iter()
            .find_map(|arg| match arg {
                CallArg::Positional(value) => Some(expr_label(value)),
                CallArg::Named { name, value } if name == "value" || name == "input" => {
                    Some(expr_label(value))
                }
                CallArg::Named { .. } | CallArg::Spread { .. } => None,
            })
            .unwrap_or_else(|| expr_label(expr)),
        _ => expr_label(expr),
    }
}

fn inline_failure_policy_from_args(args: &[CallArg]) -> Option<InlineFailurePolicy> {
    args.iter().find_map(|arg| match arg {
        CallArg::Named { name, value } if name == "fallback" || name == "none" => {
            Some(inline_fallback_from_expr(value))
        }
        CallArg::Named { name, value } if name == "discard_error" => {
            is_truthy_policy_value(value).then_some(InlineFailurePolicy::Discard)
        }
        CallArg::Named { name, value } if name == "on_error" => {
            Some(inline_failure_policy_from_expr(value))
        }
        CallArg::Positional(_) | CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}

fn is_truthy_policy_value(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(Literal::Bool(true)))
        || matches!(expr, Expr::Path(path) if path == "true")
}

fn inline_failure_policy_from_expr(expr: &Expr) -> InlineFailurePolicy {
    match enum_variant_name(expr) {
        Some((_, "fail" | "line_error")) => InlineFailurePolicy::FailLine,
        Some((_, "discard")) => InlineFailurePolicy::Discard,
        _ => inline_failure_constructor(expr).unwrap_or(InlineFailurePolicy::FailLine),
    }
}

fn inline_failure_constructor(expr: &Expr) -> Option<InlineFailurePolicy> {
    let args = match expr {
        Expr::Call { callee, args } if constructor_name(callee)? == "fallback" => args,
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if matches!(receiver.as_ref(), Expr::Path(namespace) if namespace == "InlineFailure")
            && method == "fallback" =>
        {
            args
        }
        _ => return None,
    };
    let fallback = args
        .iter()
        .find_map(|arg| match arg {
            CallArg::Positional(value) => Some(inline_fallback_value(value)),
            CallArg::Named { name, value } if name == "value" || name == "text" => {
                Some(inline_fallback_value(value))
            }
            CallArg::Named { .. } | CallArg::Spread { .. } => None,
        })
        .unwrap_or_else(|| InlineFallback::Text {
            text: String::new(),
            style: FallbackStylePolicy::Plain,
        });
    Some(InlineFailurePolicy::Fallback { fallback })
}

fn inline_fallback_from_expr(expr: &Expr) -> InlineFailurePolicy {
    InlineFailurePolicy::Fallback {
        fallback: inline_fallback_value(expr),
    }
}

fn inline_fallback_value(expr: &Expr) -> InlineFallback {
    match enum_variant_name(expr) {
        Some((_, "expr_source")) => InlineFallback::ExprSource {
            style: FallbackStylePolicy::Plain,
        },
        Some((_, "call_source")) => InlineFallback::CallSource {
            style: FallbackStylePolicy::Plain,
        },
        Some((_, "value_plain")) => InlineFallback::ValuePlain,
        _ => InlineFallback::Text {
            text: expr_style_value(expr),
            style: FallbackStylePolicy::Plain,
        },
    }
}

fn constructor_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Path(name) if name == "fallback" => Some("fallback"),
        Expr::Field { target, field } if matches!(target.as_ref(), Expr::Path(namespace) if namespace == "InlineFailure") => {
            Some(field)
        }
        _ => None,
    }
}

fn enum_variant_name(expr: &Expr) -> Option<(&str, &str)> {
    match expr {
        Expr::Path(value) => value.strip_prefix('.').map(|variant| ("", variant)),
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if args.is_empty() => match receiver.as_ref() {
            Expr::Path(namespace) => Some((namespace.as_str(), method.as_str())),
            _ => None,
        },
        Expr::Field { target, field } => match target.as_ref() {
            Expr::Path(namespace) => Some((namespace.as_str(), field.as_str())),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_lang_hir::lower::lower_to_hir;
    use arcweft_lang_syntax::parser::parse_source;
    use arcweft_render_text::{
        RichTextCascadeLayer, RichTextColor, RichTextFontFamily, RuntimeLineContext,
    };

    #[test]
    fn lowers_full_tag_families_to_render_text_nodes() {
        let parsed = parse_source(
            r##"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice(style=text_style(font=serif, color="#f7e8ff"), inline_error=InlineFailure.fallback("?")): Hello #[player] |[夢](ゆめ)[r][font monospace][em:quiet][voice auto][face smile][at 0.2s call=flash][signal .seen][p]
}
"##,
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let defaults = DialogueDisplayDefaults::from_module(&hir);
        let spec = lower_dialogue_display(
            RuntimeLineId("say.opening.001".to_owned()),
            dialogue,
            &defaults,
        );

        assert_eq!(spec.window.as_deref(), Some("textbox.main"));
        assert_eq!(
            spec.base_styles,
            vec![
                RichTextStyle::Font {
                    family: RichTextFontFamily::Serif
                },
                RichTextStyle::Color {
                    value: RichTextColor::Rgb {
                        red: 247,
                        green: 232,
                        blue: 255
                    }
                }
            ]
        );
        assert_eq!(
            spec.default_inline_failure_policy,
            Some(InlineFailurePolicy::Fallback {
                fallback: InlineFallback::Text {
                    text: "?".to_owned(),
                    style: FallbackStylePolicy::Plain
                }
            })
        );
        assert!(spec.content.nodes.iter().any(|node| {
            matches!(
                node,
                RichTextNode::Interpolation {
                    expr,
                    fallback_source,
                    on_error: InlineFailurePolicy::Fallback {
                        fallback: InlineFallback::Text { text, .. }
                    },
                } if expr == "player"
                    && fallback_source == "player"
                    && text == "?"
            )
        }));
        assert!(spec.content.nodes.iter().any(|node| {
            matches!(
                node,
                RichTextNode::Ruby { base, ruby } if base == "夢" && ruby == "ゆめ"
            )
        }));
        assert_has_host_event(&spec.content.nodes, |event| {
            matches!(event, DialogueHostEvent::Voice { .. })
        });
        assert_has_host_event(
            &spec.content.nodes,
            |event| matches!(event, DialogueHostEvent::TimedCue { attrs } if attrs == "0.2s call=flash"),
        );
        assert_has_host_event(&spec.content.nodes, |event| {
            matches!(event, DialogueHostEvent::Signal { .. })
        });
        assert!(spec.content.nodes.iter().any(|node| {
            matches!(
                node,
                RichTextNode::StyleStart {
                    style: RichTextStyle::Font {
                        family: RichTextFontFamily::Monospace
                    }
                }
            )
        }));
    }

    fn assert_has_host_event(
        nodes: &[RichTextNode],
        predicate: impl Fn(&DialogueHostEvent) -> bool,
    ) {
        assert!(nodes.iter().any(|node| match node {
            RichTextNode::HostEvent { event } => predicate(event),
            _ => false,
        }));
    }

    #[test]
    fn inferred_dot_rich_text_lowers_custom_attr_selector_to_effect_presentation() {
        let parsed = parse_source(
            r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[.sparkle amp=2px dir=0,1 pattern=a,b,c seed=dialogue target=glyph phase=layout_transform state_scope=global]BC[/]D[p]
}
",
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.001".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("rich text frame resolves");
        let effect_run = frame
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                frame
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "BC")
            })
            .expect("effect text run");
        let effect = effect_run
            .presentation
            .effects
            .first()
            .expect("effect presentation");

        assert_eq!(effect.id, "sparkle");
        assert_eq!(effect.target, RichTextEffectTarget::Glyph);
        assert_eq!(effect.phase, RichTextEffectPhase::LayoutTransform);
        assert_eq!(effect.state_scope, RichTextStateScope::Global);
        assert_eq!(
            effect.params.get("amp"),
            Some(&RichTextParam::Milli { value: Milli(2000) })
        );
        assert_eq!(
            effect.params.get("dir"),
            Some(&RichTextParam::Raw {
                value: "0,1".to_owned()
            })
        );
        assert_eq!(
            effect.params.get("pattern"),
            Some(&RichTextParam::Raw {
                value: "a,b,c".to_owned()
            })
        );
        assert_eq!(
            effect.params.get("seed"),
            Some(&RichTextParam::Raw {
                value: "dialogue".to_owned()
            })
        );
        assert!(
            !effect.params.contains_key("state_scope"),
            "state_scope is descriptor metadata and should not be forwarded as a custom effect param"
        );
        let plain_run = frame
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                frame
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "D")
            })
            .expect("plain text run after inferred close");
        assert!(plain_run.presentation.effects.is_empty());
    }

    #[test]
    fn host_event_phase_effect_lowers_to_typed_host_event() {
        let parsed = parse_source(
            r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[effect .wave phase=host_event amp=4px target=glyph]BC[/effect][.host id=sparkle phase=host_event channel=debug]DE[/][p]
}
",
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.host_event.effect".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("rich text frame resolves");

        assert_eq!(frame.text, "ABCDE");
        assert!(frame.host_events.iter().any(|event| {
            matches!(
                event,
                DialogueHostEvent::Effect { id, attrs }
                    if id == "wave"
                        && attrs.contains("phase=host_event")
                        && attrs.contains("amp=4px")
            )
        }));
        assert!(frame.host_events.iter().any(|event| {
            matches!(
                event,
                DialogueHostEvent::Effect { id, attrs }
                    if id == "sparkle"
                        && attrs.contains("phase=host_event")
                        && attrs.contains("channel=debug")
            )
        }));
        assert!(
            frame
                .display_map
                .text_runs
                .iter()
                .all(|run| run.presentation.effects.is_empty()),
            "host_event phase effects should not become visual presentation effects: {:#?}",
            frame.display_map.text_runs
        );
    }

    #[test]
    fn explicit_object_tag_lowers_text_proxy_metadata_to_presentation() {
        let parsed = parse_source(
            r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[object .hotspot type=KeywordHit role=keyword depth=4 hit=true channel=choice]BC[/object]D[p]
}
",
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.object.proxy".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("rich text frame resolves");
        let object_run = frame
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                frame
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "BC")
            })
            .expect("object proxy text run");
        let proxy = object_run
            .presentation
            .object_proxies
            .first()
            .expect("object proxy presentation");

        assert_eq!(proxy.id, "hotspot");
        assert!(proxy.declaration.is_none());
        assert_eq!(proxy.type_name.as_deref(), Some("KeywordHit"));
        assert_eq!(proxy.role.as_deref(), Some("keyword"));
        assert_eq!(proxy.depth, Some(Milli(4000)));
        assert!(proxy.hit_test);
        assert_eq!(
            proxy.params.get("channel"),
            Some(&RichTextParam::Raw {
                value: "choice".to_owned()
            })
        );
        assert!(
            !proxy.params.contains_key("type") && !proxy.params.contains_key("depth"),
            "proxy metadata keys should not be forwarded as custom params"
        );
        let plain_run = frame
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                frame
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "D")
            })
            .expect("plain text run after object close");
        assert!(plain_run.presentation.object_proxies.is_empty());
    }

    #[test]
    fn text_proxy_struct_attribute_supplies_object_proxy_defaults() {
        let parsed = parse_source(
            r#"
#[text_proxy(kind="keyword", default_hit=true, depth=4, channel=choice)]
pub struct KeywordHit {
    channel: String
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[object .hotspot type=KeywordHit]BC[/object]D[p]
}
"#,
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.object.proxy.defaults".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("rich text frame resolves");
        let proxy = frame
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                frame
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "BC")
            })
            .and_then(|run| run.presentation.object_proxies.first())
            .expect("object proxy presentation");

        assert_eq!(proxy.id, "hotspot");
        assert_eq!(
            proxy.declaration.as_ref().map(|declaration| (
                declaration.struct_name.as_str(),
                declaration.attribute.as_str()
            )),
            Some(("KeywordHit", "text_proxy"))
        );
        assert_eq!(proxy.type_name.as_deref(), Some("KeywordHit"));
        assert_eq!(proxy.role.as_deref(), Some("keyword"));
        assert_eq!(proxy.depth, Some(Milli(4000)));
        assert!(proxy.hit_test);
        assert_eq!(
            proxy.params.get("channel"),
            Some(&RichTextParam::Raw {
                value: "choice".to_owned()
            })
        );
    }

    #[test]
    fn nested_text_proxy_struct_attributes_accumulate_with_inline_overrides() {
        let parsed = parse_source(
            r#"
#[text_proxy(kind="keyword", default_hit=true, depth=4, channel=choice)]
pub struct KeywordHit {
    channel: String
}

#[text_proxy(kind="hover", default_hit=false, depth=2, layer=ui)]
pub struct HoverHit {
    layer: String
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[object .hotspot type=KeywordHit channel=inventory][object .hover type=HoverHit depth=7 hit=true layer=hud tone=alert]BC[/object][/object]D[p]
}
"#,
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.object.proxy.nested".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("rich text frame resolves");
        let object_proxies = frame
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                frame
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "BC")
            })
            .map(|run| run.presentation.object_proxies.as_slice())
            .expect("nested object proxy text run");
        let [keyword, hover] = object_proxies else {
            panic!("nested object run should carry two proxies: {object_proxies:?}");
        };

        assert_eq!(keyword.id, "hotspot");
        assert_eq!(
            keyword.declaration.as_ref().map(|declaration| (
                declaration.struct_name.as_str(),
                declaration.attribute.as_str()
            )),
            Some(("KeywordHit", "text_proxy"))
        );
        assert_eq!(keyword.type_name.as_deref(), Some("KeywordHit"));
        assert_eq!(keyword.role.as_deref(), Some("keyword"));
        assert_eq!(keyword.depth, Some(Milli(4000)));
        assert!(keyword.hit_test);
        assert_eq!(
            keyword.params.get("channel"),
            Some(&RichTextParam::Raw {
                value: "inventory".to_owned()
            })
        );

        assert_eq!(hover.id, "hover");
        assert_eq!(
            hover.declaration.as_ref().map(|declaration| (
                declaration.struct_name.as_str(),
                declaration.attribute.as_str()
            )),
            Some(("HoverHit", "text_proxy"))
        );
        assert_eq!(hover.type_name.as_deref(), Some("HoverHit"));
        assert_eq!(hover.role.as_deref(), Some("hover"));
        assert_eq!(hover.layer.as_deref(), Some("hud"));
        assert_eq!(hover.depth, Some(Milli(7000)));
        assert!(hover.hit_test);
        assert_eq!(
            hover.params.get("tone"),
            Some(&RichTextParam::Raw {
                value: "alert".to_owned()
            })
        );
        assert!(
            !hover.params.contains_key("type")
                && !hover.params.contains_key("hit")
                && !hover.params.contains_key("layer"),
            "proxy metadata keys should not be forwarded as custom params"
        );
    }

    #[test]
    fn inferred_text_proxy_struct_shorthand_lowers_to_object_proxy() {
        let parsed = parse_source(
            r#"
#[text_proxy(kind="keyword", default_hit=true, depth=4, channel=choice)]
pub struct KeywordHit {
    channel: String
}

#[text_proxy(kind="hover", default_hit=false, depth=2, layer=ui)]
pub struct HoverHit {
    layer: String
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[.hotspot type=KeywordHit channel=inventory][.HoverHit depth=7 hit=true tone=alert]BC[/][/][.sparkle amp=2px]FX[/][p]
}
"#,
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.object.proxy.inferred".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("rich text frame resolves");
        let object_proxies = frame
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                frame
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "BC")
            })
            .map(|run| run.presentation.object_proxies.as_slice())
            .expect("inferred object proxy text run");
        let [keyword, hover] = object_proxies else {
            panic!("inferred proxy run should carry two proxies: {object_proxies:?}");
        };

        assert_eq!(keyword.id, "hotspot");
        assert_eq!(
            keyword.declaration.as_ref().map(|declaration| (
                declaration.struct_name.as_str(),
                declaration.attribute.as_str()
            )),
            Some(("KeywordHit", "text_proxy"))
        );
        assert_eq!(keyword.type_name.as_deref(), Some("KeywordHit"));
        assert_eq!(keyword.role.as_deref(), Some("keyword"));
        assert_eq!(keyword.depth, Some(Milli(4000)));
        assert!(keyword.hit_test);
        assert_eq!(
            keyword.params.get("channel"),
            Some(&RichTextParam::Raw {
                value: "inventory".to_owned()
            })
        );

        assert_eq!(hover.id, "HoverHit");
        assert_eq!(
            hover.declaration.as_ref().map(|declaration| (
                declaration.struct_name.as_str(),
                declaration.attribute.as_str()
            )),
            Some(("HoverHit", "text_proxy"))
        );
        assert_eq!(hover.type_name.as_deref(), Some("HoverHit"));
        assert_eq!(hover.role.as_deref(), Some("hover"));
        assert_eq!(hover.layer.as_deref(), Some("ui"));
        assert_eq!(hover.depth, Some(Milli(7000)));
        assert!(hover.hit_test);
        assert_eq!(
            hover.params.get("tone"),
            Some(&RichTextParam::Raw {
                value: "alert".to_owned()
            })
        );

        assert_run_has_effect_without_object_proxy(&frame, "FX", "sparkle");
    }

    fn assert_run_has_effect_without_object_proxy(
        frame: &arcweft_render_text::LineDisplayFrame,
        text: &str,
        effect_id: &str,
    ) {
        let effect_run = frame
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                frame
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|run_text| run_text == text)
            })
            .expect("custom effect run remains effect");
        assert!(effect_run.presentation.object_proxies.is_empty());
        assert!(
            effect_run
                .presentation
                .effects
                .iter()
                .any(|effect| effect.id == effect_id)
        );
    }

    #[test]
    fn rich_text_proxy_struct_attribute_supplies_object_proxy_defaults() {
        let parsed = parse_source(
            r#"
#[rich_text_proxy(kind="quest", default_hit=true, depth=6, layer=hud, channel=quest)]
pub struct QuestHit {
    channel: String
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[.QuestHit state=active]BC[/]D[p]
}
"#,
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.object.proxy.rich_text_attribute".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("rich text frame resolves");
        let proxy = frame
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                frame
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "BC")
            })
            .and_then(|run| run.presentation.object_proxies.first())
            .expect("rich_text_proxy presentation");

        assert_eq!(proxy.id, "QuestHit");
        assert_eq!(
            proxy.declaration.as_ref().map(|declaration| (
                declaration.struct_name.as_str(),
                declaration.attribute.as_str()
            )),
            Some(("QuestHit", "rich_text_proxy"))
        );
        assert_eq!(proxy.type_name.as_deref(), Some("QuestHit"));
        assert_eq!(proxy.role.as_deref(), Some("quest"));
        assert_eq!(proxy.layer.as_deref(), Some("hud"));
        assert_eq!(proxy.depth, Some(Milli(6000)));
        assert!(proxy.hit_test);
        assert_eq!(
            proxy.params.get("channel"),
            Some(&RichTextParam::Raw {
                value: "quest".to_owned()
            })
        );
        assert_eq!(
            proxy.params.get("state"),
            Some(&RichTextParam::Raw {
                value: "active".to_owned()
            })
        );
    }

    #[test]
    fn presentation_scalar_style_sets_opacity_and_z_index() {
        let parsed = parse_source(
            r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[.layer hud][.z_index 7][.opacity 0.5][.meta role=caption hover=true weight=2]BC[/][/][/][/]D[p]
}
",
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.presentation.scalar".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("rich text frame resolves");
        let run = frame
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                frame
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "BC")
            })
            .expect("presentation scalar run");

        assert_eq!(run.presentation.z_index, 7);
        assert_eq!(run.presentation.opacity, Some(Milli(500)));
        assert_eq!(run.presentation.layer.as_deref(), Some("hud"));
        assert_eq!(
            run.presentation.params.get("role"),
            Some(&RichTextParam::Raw {
                value: "caption".to_owned()
            })
        );
        assert_eq!(
            run.presentation.params.get("hover"),
            Some(&RichTextParam::Bool { value: true })
        );
        assert_eq!(
            run.presentation.params.get("weight"),
            Some(&RichTextParam::Int { value: 2 })
        );
    }

    #[test]
    fn host_effect_selector_resolves_registry_id_from_metadata_attrs() {
        let parsed = parse_source(
            r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[.host id=sparkle amp=2px target=glyph]BC[/]D[p]
    alice: X[effect .host name=.nudge amount=3px]YZ[/effect]Q[p]
}
",
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let defaults = DialogueDisplayDefaults::from_module(&hir);
        let dialogues = hir
            .flows()
            .first()
            .expect("flow exists")
            .body()
            .iter()
            .filter_map(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .collect::<Vec<_>>();

        let inferred = lower_dialogue_display(
            RuntimeLineId("say.rich_text.host.inferred".to_owned()),
            dialogues[0],
            &defaults,
        )
        .resolve_frame(&RuntimeLineContext::default())
        .expect("inferred host frame resolves");
        let inferred_effect = inferred
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                inferred
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "BC")
            })
            .and_then(|run| run.presentation.effects.first())
            .expect("inferred host effect");

        assert_eq!(inferred_effect.id, "sparkle");
        assert_eq!(inferred_effect.target, RichTextEffectTarget::Glyph);
        assert_eq!(
            inferred_effect.params.get("amp"),
            Some(&RichTextParam::Milli { value: Milli(2000) })
        );
        assert!(!inferred_effect.params.contains_key("id"));

        let explicit = lower_dialogue_display(
            RuntimeLineId("say.rich_text.host.explicit".to_owned()),
            dialogues[1],
            &defaults,
        )
        .resolve_frame(&RuntimeLineContext::default())
        .expect("explicit host frame resolves");
        let explicit_effect = explicit
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                explicit
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "YZ")
            })
            .and_then(|run| run.presentation.effects.first())
            .expect("explicit host effect");

        assert_eq!(explicit_effect.id, "nudge");
        assert_eq!(
            explicit_effect.params.get("amount"),
            Some(&RichTextParam::Milli { value: Milli(3000) })
        );
        assert!(!explicit_effect.params.contains_key("name"));
    }

    #[test]
    fn explicit_effect_selector_end_tag_closes_effect_span() {
        let parsed = parse_source(
            r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[effect .shake amp=2px]BC[/shake]D[p]
}
",
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");
        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.effect.end".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("rich text frame resolves");
        let plain_run = frame
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                frame
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "D")
            })
            .expect("plain text run after explicit selector end");

        assert!(plain_run.presentation.effects.is_empty());
    }

    #[test]
    fn rotate_transform_selector_accepts_named_and_positional_angles() {
        let parsed = parse_source(
            r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: A[.rotate angle=8deg origin=baseline_start target=glyph]BC[/]D[transform .rotate 10deg origin=glyph_center target=textbox]EF[/transform]G[p]
}
",
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");
        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.transform.rotate".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("rich text frame resolves");

        let named_run = frame
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                frame
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "BC")
            })
            .expect("named rotate run");
        let positional_run = frame
            .display_map
            .text_runs
            .iter()
            .find(|run| {
                frame
                    .text
                    .get(run.range.start..run.range.end)
                    .is_some_and(|text| text == "EF")
            })
            .expect("positional rotate run");

        let named_transform = named_run
            .presentation
            .transform
            .as_ref()
            .expect("named rotate transform");
        let positional_transform = positional_run
            .presentation
            .transform
            .as_ref()
            .expect("positional rotate transform");

        assert_eq!(named_transform.rotate.degrees, Milli(8000));
        assert_eq!(
            named_transform.origin,
            RichTextTransformOrigin::BaselineStart
        );
        assert_eq!(named_transform.target, RichTextEffectTarget::Glyph);
        assert_eq!(positional_transform.rotate.degrees, Milli(10000));
        assert_eq!(
            positional_transform.origin,
            RichTextTransformOrigin::GlyphCenter
        );
        assert_eq!(positional_transform.target, RichTextEffectTarget::TextBox);
    }

    #[test]
    fn rich_text_defaults_and_line_options_lower_to_ruby_layout() {
        let source = r"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        ruby {
            size = 14px
            gap = 2px
        }
    }
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice(rich_text=rich_text_style(ruby=ruby_style(size=11px, gap=1px))): |[夢](ゆめ)[p]
}
";
        let default_ruby_size_start = source.find("14px").expect("default ruby size literal");
        let parsed = parse_source(source);
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");
        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.defaults".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );

        assert!(spec.base_styles.iter().any(|style| {
            matches!(
                style,
                RichTextStyle::Layout {
                    layout: RichTextLayout {
                        ruby_font_size: Some(Milli(14000)),
                        ..
                    }
                }
            )
        }));
        assert!(spec.base_styles.iter().any(|style| {
            matches!(
                style,
                RichTextStyle::Layout {
                    layout: RichTextLayout {
                        ruby_gap: Some(Milli(2000)),
                        ..
                    }
                }
            )
        }));
        assert!(spec.base_styles.iter().any(|style| {
            matches!(
                style,
                RichTextStyle::Layout {
                    layout: RichTextLayout {
                        ruby_font_size: Some(Milli(11000)),
                        ruby_gap: Some(Milli(1000)),
                        ..
                    }
                }
            )
        }));
        assert!(
            spec.style_contributions.iter().any(|contribution| {
                contribution.layer == RichTextCascadeLayer::DialogueDefaults
                    && contribution.path == "rich_text.ruby.size"
                    && contribution.value == "14px"
                    && !contribution.active
                    && contribution.style_index == Some(0)
                    && contribution_source_range(contribution)
                        == Some((default_ruby_size_start, default_ruby_size_start + 4))
            }),
            "{:#?}",
            spec.style_contributions
        );
        assert!(spec.style_contributions.iter().any(|contribution| {
            contribution.layer == RichTextCascadeLayer::LineOptions
                && contribution.path == "rich_text.ruby.size"
                && contribution.value == "11px"
                && contribution.active
                && contribution.style_index == Some(2)
                && matches!(
                    contribution.source,
                    RichTextSettingSource::SourceFile {
                        range: Some(RichTextSourceRange { .. }),
                        ..
                    }
                )
        }));
        assert!(spec.style_contributions.iter().any(|contribution| {
            contribution.layer == RichTextCascadeLayer::DialogueDefaults
                && contribution.path == "rich_text.ruby.size"
                && !contribution.active
                && contribution.shadowed_by.is_some()
        }));
    }

    #[test]
    fn dialogue_display_uses_canonical_defaults_profile_when_multiple_exist() {
        let parsed = parse_source(
            r##"
pub dialogue defaults @dialogue.defaults.debug {
    text_color = rgb("#ff0000")
}

pub dialogue defaults @dialogue.defaults {
    text_color = rgb("#101112")
}

pub dialogue defaults @dialogue.defaults.mobile {
    text_color = rgb("#00ff00")
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Hello[p]
}
"##,
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");
        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.defaults.profile".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );

        assert_eq!(
            spec.base_styles,
            vec![RichTextStyle::Color {
                value: RichTextColor::Rgb {
                    red: 16,
                    green: 17,
                    blue: 18
                }
            }]
        );
        assert!(spec.style_contributions.iter().any(|contribution| {
            contribution.layer == RichTextCascadeLayer::DialogueDefaults
                && contribution.path == "text_color"
                && contribution.value == "rgb(\"#101112\")"
                && contribution.active
        }));
        assert!(!spec.style_contributions.iter().any(|contribution| {
            contribution.value == "rgb(\"#ff0000\")" || contribution.value == "rgb(\"#00ff00\")"
        }));
    }

    #[test]
    fn inferred_layout_selector_lowers_jlreq_strictness_preset() {
        let parsed = parse_source(
            r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=strict]天地。「人[/][p]
}
",
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.002".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("rich text frame resolves");
        let run = frame.display_map.text_runs.first().expect("text run");
        let layout = run.presentation.layout.as_ref().expect("layout");

        assert_eq!(layout.writing_mode, RichTextWritingMode::VerticalRl);
        assert_eq!(layout.jlreq_strictness, RichTextJlreqStrictness::Strict);
    }

    #[test]
    fn inferred_layout_selector_lowers_ruby_under_position() {
        let parsed = parse_source(
            r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl][.ruby_under]|[夢](ゆめ)[/][p]
}
",
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.003".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("rich text frame resolves");
        let ruby = frame
            .display_map
            .ruby_annotations
            .first()
            .expect("ruby annotation");
        let layout = ruby.presentation.layout.as_ref().expect("ruby layout");

        assert_eq!(layout.writing_mode, RichTextWritingMode::VerticalRl);
        assert_eq!(layout.ruby_position, RichTextRubyPosition::Under);
    }

    #[test]
    fn ruby_layout_selector_lowers_typography_attrs() {
        let parsed = parse_source(
            r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.ruby_over ruby_size=11px ruby_gap=1px ruby_overhang=4px ruby_collision_gap=3px]|[夢](ゆめ)[/][p]
}
",
        );
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.004".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("rich text frame resolves");
        let ruby = frame
            .display_map
            .ruby_annotations
            .first()
            .expect("ruby annotation");
        let layout = ruby.presentation.layout.as_ref().expect("ruby layout");

        assert_eq!(layout.ruby_position, RichTextRubyPosition::Over);
        assert_eq!(layout.ruby_font_size, Some(Milli(11000)));
        assert_eq!(layout.ruby_gap, Some(Milli(1000)));
        assert_eq!(layout.ruby_overhang, Some(Milli(4000)));
        assert_eq!(layout.ruby_collision_gap, Some(Milli(3000)));
        assert_eq!(layout.column_gap, Milli(8000));
    }

    #[test]
    fn inline_rich_text_span_contributes_cascade_provenance() {
        let source = r"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        ruby {
            size = 14px
        }
    }
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.ruby_over ruby_size=11px]|[夢](ゆめ)[/][p]
}
";
        let inline_size_start = source.find("11px").expect("inline ruby size literal");
        let parsed = parse_source(source);
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.rich_text.inline".to_owned()),
            dialogue,
            &DialogueDisplayDefaults::from_module(&hir),
        );

        assert!(spec.style_contributions.iter().any(|contribution| {
            contribution.layer == RichTextCascadeLayer::InlineSpan
                && contribution.path == "rich_text.ruby.size"
                && contribution.value == "11px"
                && contribution.active
                && contribution_source_range(contribution)
                    == Some((inline_size_start, inline_size_start + 4))
        }));
        assert!(spec.style_contributions.iter().any(|contribution| {
            contribution.layer == RichTextCascadeLayer::DialogueDefaults
                && contribution.path == "rich_text.ruby.size"
                && contribution.value == "14px"
                && !contribution.active
                && contribution.shadowed_by.is_some()
        }));
    }

    #[test]
    fn dialogue_display_inherits_global_and_character_style_defaults() {
        let source = r##"
pub dialogue defaults @dialogue.defaults {
    font = serif
    text_color = rgb("#101112")
    inline_error = InlineFailure.fallback("global")
}

character @character.alice Alice as alice {
    dialogue_style {
        text_color = rgb("#202122")
        inline_error = InlineFailure.discard
    }
}

flow @flow.main main {
    @<character.alice>.say(color=rgb("#303132"))[Hello #[missing][p]]
}
"##;
        let parsed = parse_source(source);
        let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers");
        let defaults = DialogueDisplayDefaults::from_module(&hir);
        let dialogue = hir
            .flows()
            .first()
            .and_then(|flow| flow.body().first())
            .and_then(|item| match item {
                arcweft_lang_hir::model::HirFlowItem::Dialogue(dialogue) => Some(dialogue),
                _ => None,
            })
            .expect("dialogue item");

        let spec = lower_dialogue_display(
            RuntimeLineId("say.opening.002".to_owned()),
            dialogue,
            &defaults,
        );

        assert_eq!(
            spec.base_styles,
            vec![
                RichTextStyle::Font {
                    family: RichTextFontFamily::Serif
                },
                RichTextStyle::Color {
                    value: RichTextColor::Rgb {
                        red: 16,
                        green: 17,
                        blue: 18
                    }
                },
                RichTextStyle::Color {
                    value: RichTextColor::Rgb {
                        red: 32,
                        green: 33,
                        blue: 34
                    }
                },
                RichTextStyle::Color {
                    value: RichTextColor::Rgb {
                        red: 48,
                        green: 49,
                        blue: 50
                    }
                }
            ]
        );
        assert_eq!(
            spec.default_inline_failure_policy,
            Some(InlineFailurePolicy::Discard)
        );
        let character_text_color = spec
            .style_contributions
            .iter()
            .find(|contribution| {
                contribution.layer == RichTextCascadeLayer::CharacterDialogueStyle
                    && contribution.path == "text_color"
                    && contribution.value == "rgb(\"#202122\")"
            })
            .expect("character text color contribution");
        let RichTextSettingSource::SourceFile {
            range: Some(range), ..
        } = &character_text_color.source
        else {
            panic!("character contribution should preserve its source range");
        };
        assert_eq!(source[range.start..range.end].trim(), "rgb(\"#202122\")");
        assert!(spec.content.nodes.iter().any(|node| {
            matches!(
                node,
                RichTextNode::Interpolation {
                    expr,
                    on_error: InlineFailurePolicy::Discard,
                    ..
                } if expr == "missing"
            )
        }));
    }

    fn contribution_source_range(
        contribution: &RichTextStyleContribution,
    ) -> Option<(usize, usize)> {
        match &contribution.source {
            RichTextSettingSource::SourceFile {
                range: Some(range), ..
            } => Some((range.start, range.end)),
            _ => None,
        }
    }
}
