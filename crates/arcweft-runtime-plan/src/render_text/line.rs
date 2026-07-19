use arcweft_core::plan::RuntimeLineId;
use arcweft_lang_hir::model::HirDialogue;
use arcweft_render_text::{
    InlineFailurePolicy, LineDisplayArg, LineDisplaySpec, RichTextCascadeLayer, RichTextDocument,
    RichTextSettingSource, RichTextStyle, RichTextStyleContribution,
};

use crate::errors::RuntimePlanLowerError;
use crate::labels::expr_label;

use super::contributions::{
    LineOptionContribution, append_inline_span_contributions, append_line_option_contributions,
};
use super::defaults::{DialogueDisplayDefaults, DialogueSpeakerPreset};
use super::entity_defaults::append_style_contributions;
use super::fx::{
    DialogueFxExpander, FxCatalog, FxInlineAssignment, append_fx_inline_contributions,
};
use super::inline_failure::{inline_default_from_named_expr, lower_default_inline_failure_policy};
use super::raw::{dialogue_option_source, mark_shadowed_style_contributions, source_range};
use super::speaker_preset::{effective_dialogue_view, speaker_preset_chain};
use super::style_expr::{display_styles_from_expr, display_styles_from_named_expr};

#[cfg(test)]
pub(crate) fn lower_dialogue_display(
    line: RuntimeLineId,
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
) -> LineDisplaySpec {
    lower_dialogue_display_with_speaker_presets(line, dialogue, defaults, &[])
        .expect("test dialogue fixture has valid render controls")
}

#[cfg(test)]
pub(crate) fn lower_dialogue_display_with_speaker_presets(
    line: RuntimeLineId,
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
    speaker_presets: &[DialogueSpeakerPreset],
) -> Result<LineDisplaySpec, RuntimePlanLowerError> {
    lower_dialogue_display_with_speaker_presets_and_fx(
        line,
        dialogue,
        defaults,
        speaker_presets,
        &FxCatalog::default(),
    )
}

pub(crate) fn lower_dialogue_display_with_speaker_presets_and_fx(
    line: RuntimeLineId,
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
    speaker_presets: &[DialogueSpeakerPreset],
    fx: &FxCatalog,
) -> Result<LineDisplaySpec, RuntimePlanLowerError> {
    if let Some(diagnostic) = dialogue.content().diagnostics().first() {
        return Err(RuntimePlanLowerError::new(format!(
            "invalid dialogue content: {}; {}",
            diagnostic.message(),
            diagnostic.recovery()
        )));
    }
    let default_inline_failure_policy =
        lower_effective_inline_failure_policy(dialogue, defaults, speaker_presets);
    let preset_chain = speaker_preset_chain(dialogue.callee(), speaker_presets);
    let character_callee = preset_chain
        .first()
        .map_or_else(|| dialogue.callee(), |preset| preset.callee());
    let mut content = Vec::new();
    let mut fx_expander = DialogueFxExpander::new(fx);
    for token in dialogue.content().tokens() {
        content.extend(fx_expander.lower_token(
            token,
            default_inline_failure_policy.as_ref(),
            &defaults.text_proxies,
        )?);
    }
    let fx_assignments = fx_expander.finish()?;
    Ok(LineDisplaySpec {
        line,
        callee: dialogue.callee().to_owned(),
        speaker_label: defaults
            .speaker_label_for_callee(character_callee)
            .map(str::to_owned),
        text_key: dialogue.text_key().map(|id| id.body().to_owned()),
        view: effective_dialogue_view(dialogue, defaults, speaker_presets)?,
        voice: dialogue.voice().map(expr_label),
        look: dialogue.look().map(expr_label),
        style: dialogue.style().map(expr_label),
        base_styles: lower_effective_dialogue_base_styles(dialogue, defaults, speaker_presets),
        default_inline_failure_policy: default_inline_failure_policy.clone(),
        style_contributions: lower_effective_dialogue_style_contributions(
            dialogue,
            defaults,
            speaker_presets,
            &fx_assignments,
        ),
        args: dialogue
            .args()
            .iter()
            .map(|arg| LineDisplayArg {
                name: arg.name().to_owned(),
                value: expr_label(arg.value()),
            })
            .collect(),
        content: RichTextDocument::new(content),
    })
}

fn lower_effective_dialogue_base_styles(
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
    speaker_presets: &[DialogueSpeakerPreset],
) -> Vec<RichTextStyle> {
    let mut styles = defaults.global.base_styles.clone();
    let preset_chain = speaker_preset_chain(dialogue.callee(), speaker_presets);
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
        .or_else(|| defaults.global.default_inline_failure_policy.clone())
}

fn lower_effective_dialogue_style_contributions(
    dialogue: &HirDialogue,
    defaults: &DialogueDisplayDefaults,
    speaker_presets: &[DialogueSpeakerPreset],
    fx_assignments: &[FxInlineAssignment],
) -> Vec<RichTextStyleContribution> {
    let mut contributions = Vec::new();
    let mut base_offset = 0usize;

    append_style_contributions(&mut contributions, &defaults.global, &mut base_offset);
    let preset_chain = speaker_preset_chain(dialogue.callee(), speaker_presets);
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

    let inline_start = contributions.len();
    append_inline_span_contributions(&mut contributions, dialogue);
    append_fx_inline_contributions(&mut contributions, dialogue, fx_assignments);
    // Expanded Fx layers and ordinary tags share one inline cascade. Keep
    // replacement precedence in authored order regardless of which lowering
    // path produced each contribution.
    contributions[inline_start..].sort_by_key(inline_contribution_source_start);
    mark_shadowed_style_contributions(&mut contributions);
    contributions
}

fn inline_contribution_source_start(contribution: &RichTextStyleContribution) -> usize {
    match &contribution.source {
        RichTextSettingSource::SourceFile {
            range: Some(range), ..
        } => range.start,
        RichTextSettingSource::SourceFile { range: None, .. }
        | RichTextSettingSource::EngineDefault { .. } => usize::MAX,
    }
}
