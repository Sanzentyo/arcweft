//! Reachable inline-builtin definition inventory derived from typed HIR dialogue tokens.

use std::collections::BTreeMap;

use arcweft_dialogue::rich_text::{RichTextTagFamily, inferred_tag_family};
use arcweft_lang_hir::{
    model::{HirFlowItem, HirModule},
    syntax::ast::dialogue::{DialogueContent, DialogueTag, DialogueToken},
};
use arcweft_presentation::fx::{FxDefinition, FxId};

use crate::{errors::RuntimePlanLowerError, render_text::tag::split_selector_attrs};

use super::{CompiledBuiltinRichTextFx, compile_builtin_rich_text_fx, fx_error};

pub(crate) fn builtin_rich_text_fx_definitions(
    module: &HirModule,
) -> Result<Vec<FxDefinition>, RuntimePlanLowerError> {
    let mut definitions = BTreeMap::<FxId, FxDefinition>::new();
    visit_items(module.top_level_items(), &mut definitions)?;
    for flow in module.flows() {
        visit_items(flow.body(), &mut definitions)?;
    }
    Ok(definitions.into_values().collect())
}

fn visit_items(
    items: &[HirFlowItem],
    definitions: &mut BTreeMap<FxId, FxDefinition>,
) -> Result<(), RuntimePlanLowerError> {
    for item in items {
        match item {
            HirFlowItem::Dialogue(dialogue) => visit_dialogue(dialogue.content(), definitions)?,
            HirFlowItem::Thread(thread) => visit_items(thread.body(), definitions)?,
            HirFlowItem::If(branch) => {
                visit_items(branch.body(), definitions)?;
                visit_items(branch.else_body(), definitions)?;
            }
            HirFlowItem::IfLet(branch) => {
                visit_items(branch.body(), definitions)?;
                visit_items(branch.else_body(), definitions)?;
            }
            HirFlowItem::Match(branch) => {
                for arm in branch.arms() {
                    visit_items(arm.body(), definitions)?;
                }
            }
            HirFlowItem::LetLoop { block, .. } | HirFlowItem::Loop(block) => {
                visit_items(block.body(), definitions)?;
            }
            HirFlowItem::While(block) => visit_items(block.body(), definitions)?,
            HirFlowItem::WhileLet(block) => visit_items(block.body(), definitions)?,
            HirFlowItem::For(block) => visit_items(block.body(), definitions)?,
            HirFlowItem::Select(select) => {
                for branch in select.branches() {
                    visit_items(branch.body(), definitions)?;
                }
            }
            HirFlowItem::Borrow(block) => visit_items(block.body(), definitions)?,
            HirFlowItem::SourceLocale(block) => visit_items(block.body(), definitions)?,
            HirFlowItem::Scope(block) => visit_items(block.body(), definitions)?,
            HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
                for branch in await_with.branches() {
                    visit_items(branch.body(), definitions)?;
                }
            }
            HirFlowItem::Stmt(_)
            | HirFlowItem::Choice(_)
            | HirFlowItem::LetChoice { .. }
            | HirFlowItem::LetScope { .. }
            | HirFlowItem::Include(_) => {}
        }
    }
    Ok(())
}

fn visit_dialogue(
    content: &DialogueContent,
    definitions: &mut BTreeMap<FxId, FxDefinition>,
) -> Result<(), RuntimePlanLowerError> {
    for token in content.tokens() {
        let Some((selector, attrs)) = builtin_selector(token) else {
            continue;
        };
        if let CompiledBuiltinRichTextFx::Definition(definition) =
            compile_builtin_rich_text_fx(selector, attrs)?
        {
            let id = definition.id().clone();
            if let Some(previous) = definitions.insert(id.clone(), definition.clone())
                && previous != definition
            {
                return Err(fx_error(format!(
                    "inline builtin `{id}` produced inconsistent definitions"
                )));
            }
        }
    }
    Ok(())
}

pub(crate) fn builtin_selector(token: &DialogueToken) -> Option<(&str, &str)> {
    match token {
        DialogueToken::Tag(tag) if tag.name() == "effect" => {
            let (selector, attrs) = split_selector_attrs(tag.attrs());
            Some((selector.trim_start_matches('.'), attrs))
        }
        DialogueToken::Tag(tag) if tag.name() == "shader" => Some(("shader", tag.attrs())),
        DialogueToken::InferredTag(tag) if inferred_tag_is_effect(tag) => {
            Some((tag.name().trim_start_matches('.'), tag.attrs()))
        }
        _ => None,
    }
}

fn inferred_tag_is_effect(tag: &DialogueTag) -> bool {
    inferred_tag_family(tag.name().trim_start_matches('.'), tag.attrs())
        == Some(RichTextTagFamily::Effect)
}
