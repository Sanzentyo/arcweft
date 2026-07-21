use arcweft_lang_hir::model::HirDialogue;
use arcweft_lang_hir::syntax::ast::common::TextRange;
use arcweft_lang_hir::syntax::ast::pattern::Pattern;
use arcweft_lang_hir::syntax::expr::{CallArg, Expr};
use arcweft_render_text::{
    RichTextAssignOp, RichTextCascadeLayer, RichTextSourceRange, RichTextStyleContribution,
};

use crate::labels::expr_label;

use super::defaults::{
    DEFAULT_DIALOGUE_VIEW, DialogueDisplayDefaults, DialogueSpeakerPreset, DialogueStyleDefaults,
};
use super::helpers::entity_ref_label;
use super::inline_failure::inline_default_from_named_expr;
use super::raw::{source_file, speaker_preset_arg_ranges, style_assignment_paths};
use super::style_expr::display_styles_from_named_expr;

impl DialogueSpeakerPreset {
    pub(crate) fn callee(&self) -> &str {
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
    let Expr::Call(call) = expr else {
        return None;
    };
    let defaults = speaker_preset_defaults(name, call.args(), expr_source, expr_range);
    Some((
        name.to_owned(),
        DialogueSpeakerPreset {
            name: name.to_owned(),
            callee: expr_label(call.callee()),
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
    if path == "view" {
        defaults.view = Some(entity_ref_label(value));
    }

    let style_index = defaults.base_styles.len();
    let styles = display_styles_from_named_expr(path, value);
    let active = policy.is_some() || path == "view" || !styles.is_empty();
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

pub(crate) fn speaker_preset_chain<'a>(
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

pub(crate) fn effective_dialogue_view(
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
    speaker_presets: &[DialogueSpeakerPreset],
) -> Result<arcweft_view::ViewId, crate::errors::RuntimePlanLowerError> {
    let selected = dialogue
        .view()
        .map(|id| id.body().to_owned())
        .or_else(|| {
            speaker_preset_chain(dialogue.callee(), speaker_presets)
                .into_iter()
                .rev()
                .find_map(|preset| preset.defaults.view.clone())
        })
        .or_else(|| {
            let preset_chain = speaker_preset_chain(dialogue.callee(), speaker_presets);
            let character_callee = preset_chain
                .first()
                .map_or_else(|| dialogue.callee(), |preset| preset.callee());
            defaults
                .character_for_callee(character_callee)
                .and_then(|character| character.view.clone())
        })
        .or_else(|| defaults.global.view.clone())
        .unwrap_or_else(|| DEFAULT_DIALOGUE_VIEW.to_owned());
    arcweft_view::ViewId::parse_public(selected.clone()).map_err(|error| {
        crate::errors::RuntimePlanLowerError::new(format!(
            "dialogue View `{selected}` is not a valid public View identity: {error}"
        ))
    })
}
