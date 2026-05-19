use crate::ast::choice::{ChoiceAction, ChoiceItem};
use crate::ast::common::TextRange;
use crate::ast::flow::FlowItem;
use crate::ast::ids::{FamilyRelativeEntityRef, IdRef, RelativeId, RelativeIdSpelling};
use crate::ast::items::{Item, TypedSyntaxTree};

/// Syntax-level lint emitted before full name resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxLint {
    code: SyntaxLintCode,
    message: String,
    range: TextRange,
}

/// Stable categories for editor and CLI filtering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxLintCode {
    DeepDotRunRelativeId,
    FlowIdModuleMismatch,
}

/// Lints ID policy choices that are parseable but discouraged.
pub fn lint_id_policy(tree: &TypedSyntaxTree) -> Vec<SyntaxLint> {
    let mut lints = Vec::new();
    for item in tree.items() {
        lint_item_ids(item, tree, &mut lints);
    }
    lints
}

fn lint_item_ids(item: &Item, tree: &TypedSyntaxTree, lints: &mut Vec<SyntaxLint>) {
    match item {
        Item::Flow(flow) => {
            if let (Some(module), Some(id)) = (tree.module(), flow.id()) {
                let module_tail = module.path().rsplit("::").next();
                let id_tail = id.body().rsplit('.').next();
                if module_tail != id_tail {
                    lints.push(SyntaxLint::new(
                        SyntaxLintCode::FlowIdModuleMismatch,
                        format!(
                            "flow id `{}` does not follow module tail `{}`",
                            id.body(),
                            module_tail.unwrap_or_default()
                        ),
                        *id.range(),
                    ));
                }
            }
            for item in flow.body() {
                lint_flow_item_ids(item, lints);
            }
        }
        Item::FlowItem(item) => lint_flow_item_ids(item, lints),
        _ => {}
    }
}

fn lint_flow_item_ids(item: &FlowItem, lints: &mut Vec<SyntaxLint>) {
    match item {
        FlowItem::Stmt(_)
        | FlowItem::ScenarioCommand(_)
        | FlowItem::Include(_)
        | FlowItem::Raw(_) => {}
        FlowItem::SpeakerLine(line) => {
            lint_optional_id(line.options().id(), lints);
            lint_optional_id(line.options().text_key(), lints);
        }
        FlowItem::ContentCall(call) => {
            lint_optional_id(call.options().id(), lints);
            lint_optional_id(call.options().text_key(), lints);
        }
        FlowItem::Choice(choice) => {
            lint_optional_id(choice.id(), lints);
            for item in choice.items() {
                lint_choice_item_ids(item, lints);
            }
        }
        FlowItem::If(block) => lint_flow_items(block.body(), lints),
        FlowItem::IfLet(block) => lint_flow_items(block.body(), lints),
        FlowItem::Match(block) => {
            for arm in block.arms() {
                lint_flow_items(arm.body(), lints);
            }
        }
        FlowItem::Loop(block) => lint_flow_items(block.body(), lints),
        FlowItem::While(block) => lint_flow_items(block.body(), lints),
        FlowItem::WhileLet(block) => lint_flow_items(block.body(), lints),
        FlowItem::For(block) => lint_flow_items(block.body(), lints),
        FlowItem::Select(block) => {
            for branch in block.branches() {
                lint_flow_items(branch.body(), lints);
            }
        }
        FlowItem::BorrowBlock(block) => lint_flow_items(block.body(), lints),
        FlowItem::SourceLocale(block) => lint_flow_items(block.body(), lints),
        FlowItem::Scope(block) => lint_flow_items(block.body(), lints),
        FlowItem::AwaitWith(await_with) => {
            for branch in await_with.branches() {
                lint_flow_items(branch.body(), lints);
            }
        }
    }
}

fn lint_flow_items(items: &[FlowItem], lints: &mut Vec<SyntaxLint>) {
    for item in items {
        lint_flow_item_ids(item, lints);
    }
}

fn lint_choice_item_ids(item: &ChoiceItem, lints: &mut Vec<SyntaxLint>) {
    match item {
        ChoiceItem::Option(option) => {
            lint_optional_id(option.id(), lints);
            lint_optional_id(option.label_text_key(), lints);
            if let ChoiceAction::Goto(target) = option.action() {
                if let Some(relative) = target
                    .family_relative_ref()
                    .map(FamilyRelativeEntityRef::relative)
                {
                    lint_relative_id(relative, lints);
                }
            }
        }
        ChoiceItem::If { items, .. } | ChoiceItem::For { items, .. } => {
            for item in items {
                lint_choice_item_ids(item, lints);
            }
        }
        ChoiceItem::Match { arms, .. } => {
            for arm in arms {
                for item in arm.items() {
                    lint_choice_item_ids(item, lints);
                }
            }
        }
        ChoiceItem::Let { .. } | ChoiceItem::Raw(_) => {}
    }
}

fn lint_optional_id(id: Option<&IdRef>, lints: &mut Vec<SyntaxLint>) {
    if let Some(relative) = id.and_then(IdRef::relative_id) {
        lint_relative_id(relative, lints);
    }
}

fn lint_relative_id(relative: &RelativeId, lints: &mut Vec<SyntaxLint>) {
    if relative.spelling() == RelativeIdSpelling::DotRun && relative.parent_depth() >= 2 {
        lints.push(SyntaxLint::new(
            SyntaxLintCode::DeepDotRunRelativeId,
            format!(
                "`@...{}` is accepted but hand-written source should prefer explicit `@super.super.{}`",
                relative.suffix(),
                relative.suffix()
            ),
            *relative.range(),
        ));
    }
}

impl SyntaxLint {
    fn new(code: SyntaxLintCode, message: String, range: TextRange) -> Self {
        Self {
            code,
            message,
            range,
        }
    }

    pub const fn code(&self) -> SyntaxLintCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}
