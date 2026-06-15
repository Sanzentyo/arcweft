use crate::documents::DocumentSnapshot;
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_hir::model::{HirDialogue, HirFlowItem, HirModule};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_render_text::{LineDisplaySpec, RichTextSettingSource, RichTextSourceRange};
use arcweft_runtime_plan::flow::lower_runtime_plan_with_stats;

/// Effective dialogue display context at a document byte offset.
#[derive(Clone, Debug)]
pub(crate) struct EffectiveDialogueCascade {
    pub(crate) spec: LineDisplaySpec,
}

pub(crate) fn effective_dialogue_cascade_at(
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<EffectiveDialogueCascade> {
    let parsed = parse_source(document.text());
    if !parsed.errors().is_empty() {
        return None;
    }
    let hir = lower_to_hir(parsed.typed_tree()).ok()?;
    let dialogues = collect_dialogues(&hir);
    let dialogue_index = dialogues
        .iter()
        .position(|dialogue| dialogue_content_contains_offset(dialogue, offset))?;
    let report = lower_runtime_plan_with_stats(&hir).ok()?;
    let spec = report.line_display_catalog.lines().get(dialogue_index)?;
    Some(EffectiveDialogueCascade { spec: spec.clone() })
}

fn collect_dialogues(module: &HirModule) -> Vec<&HirDialogue> {
    let mut dialogues = Vec::new();
    for flow in module.flows() {
        collect_flow_item_dialogues(flow.body(), &mut dialogues);
    }
    collect_flow_item_dialogues(module.top_level_items(), &mut dialogues);
    dialogues
}

fn collect_flow_item_dialogues<'a>(items: &'a [HirFlowItem], dialogues: &mut Vec<&'a HirDialogue>) {
    for item in items {
        match item {
            HirFlowItem::Dialogue(dialogue) => dialogues.push(dialogue),
            HirFlowItem::LetLoop { block, .. } | HirFlowItem::Loop(block) => {
                collect_flow_item_dialogues(block.body(), dialogues);
            }
            HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
                for branch in await_with.branches() {
                    collect_flow_item_dialogues(branch.body(), dialogues);
                }
            }
            HirFlowItem::Thread(thread) => collect_flow_item_dialogues(thread.body(), dialogues),
            HirFlowItem::If(block) => {
                collect_flow_item_dialogues(block.body(), dialogues);
                collect_flow_item_dialogues(block.else_body(), dialogues);
            }
            HirFlowItem::IfLet(block) => {
                collect_flow_item_dialogues(block.body(), dialogues);
                collect_flow_item_dialogues(block.else_body(), dialogues);
            }
            HirFlowItem::Match(block) => {
                for arm in block.arms() {
                    collect_flow_item_dialogues(arm.body(), dialogues);
                }
            }
            HirFlowItem::While(block) => collect_flow_item_dialogues(block.body(), dialogues),
            HirFlowItem::WhileLet(block) => collect_flow_item_dialogues(block.body(), dialogues),
            HirFlowItem::For(block) => collect_flow_item_dialogues(block.body(), dialogues),
            HirFlowItem::Select(block) => {
                for branch in block.branches() {
                    collect_flow_item_dialogues(branch.body(), dialogues);
                }
            }
            HirFlowItem::Borrow(block) => collect_flow_item_dialogues(block.body(), dialogues),
            HirFlowItem::SourceLocale(block) => {
                collect_flow_item_dialogues(block.body(), dialogues);
            }
            HirFlowItem::Scope(block) => collect_flow_item_dialogues(block.body(), dialogues),
            HirFlowItem::Stmt(_)
            | HirFlowItem::Choice(_)
            | HirFlowItem::LetChoice { .. }
            | HirFlowItem::LetScope { .. }
            | HirFlowItem::Include(_) => {}
        }
    }
}

fn dialogue_content_contains_offset(dialogue: &HirDialogue, offset: usize) -> bool {
    let range = dialogue.content().range();
    range.start() <= offset && offset <= range.end()
}

pub(crate) fn source_range(source: &RichTextSettingSource) -> Option<RichTextSourceRange> {
    match source {
        RichTextSettingSource::SourceFile { range, .. } => *range,
        RichTextSettingSource::EngineDefault { .. } => None,
    }
}
