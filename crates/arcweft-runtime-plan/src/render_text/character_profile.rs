use arcweft_lang_syntax::ast::items::EntityDeclItem;
use arcweft_lang_syntax::expr::{Expr, Literal, parse_expr};
use arcweft_render_text::{
    RichTextAssignOp, RichTextCascadeLayer, RichTextSettingSource, RichTextStyleContribution,
};

use super::dialogue_context::DialogueStyleDefaults;
use super::helpers::entity_ref_label;
use super::inline_failure::inline_default_from_named_expr;
use super::raw::style_assignment_source;
use super::style_block::{named_style_block, style_block_assignments};
use super::style_expr::display_styles_from_named_expr;

impl DialogueStyleDefaults {
    pub(crate) fn is_empty(&self) -> bool {
        self.base_styles.is_empty()
            && self.style_contributions.is_empty()
            && self.default_inline_failure_policy.is_none()
            && self.view.is_none()
    }
}

pub(crate) fn character_style_defaults(item: &EntityDeclItem) -> DialogueStyleDefaults {
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

pub(crate) fn character_style_keys(item: &EntityDeclItem) -> Vec<String> {
    entity_style_keys(item)
}

pub(crate) fn character_display_label(item: &EntityDeclItem) -> Option<String> {
    item.body()
        .and_then(character_display_assignment)
        .or_else(|| item.name().map(str::to_owned))
        .filter(|label| !label.trim().is_empty())
}

fn character_display_assignment(body: &str) -> Option<String> {
    style_block_assignments(body, None)
        .into_iter()
        .rev()
        .find(|assignment| assignment.name == "display")
        .and_then(|assignment| match parse_expr(assignment.value).ok()? {
            Expr::Literal(Literal::String(value)) => Some(value),
            _ => None,
        })
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

pub(crate) fn character_callee_keys(callee: &str) -> Vec<String> {
    let mut keys = Vec::new();
    push_character_callee_key(&mut keys, callee.trim());
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
    if path == "view" {
        defaults.view = Some(entity_ref_label(expr));
    }

    let style_index = defaults.base_styles.len();
    let styles = display_styles_from_named_expr(&path, expr);
    let active = policy.is_some() || path == "view" || !styles.is_empty();
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

pub(crate) fn append_style_contributions(
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
