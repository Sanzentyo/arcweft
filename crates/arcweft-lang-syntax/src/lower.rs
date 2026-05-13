use crate::ast::{
    ChoiceBlock, DialogueContent, EntityRef, Flow, FlowItem, Item, SpeakerLine, Stmt, SyntaxTree,
    TextRange,
};
use crate::expr::Expr;
use thiserror::Error;

/// HIR-facing module produced from parsed surface syntax.
///
/// This is intentionally still close to syntax. Its role is to prove that the
/// parser exposes enough typed structure for later semantic analysis without
/// re-parsing raw strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirModule {
    flows: Vec<HirFlow>,
    top_level_items: Vec<HirFlowItem>,
}

/// HIR-facing flow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFlow {
    id: Option<EntityRef>,
    name: Option<String>,
    body: Vec<HirFlowItem>,
}

/// HIR-facing flow item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirFlowItem {
    Stmt(Stmt),
    Dialogue(HirDialogue),
    Choice(HirChoice),
    Include(EntityRef),
    Await { expr: Expr, propagates_error: bool },
    Scenario { name: String, args: String },
}

/// Dialogue call normalized enough for type checking to resolve speaker symbols.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDialogue {
    callee: String,
    content: DialogueContent,
}

/// HIR-facing choice block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirChoice {
    id: Option<EntityRef>,
    options: Vec<HirChoiceOption>,
}

/// HIR-facing choice option.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirChoiceOption {
    id: Option<EntityRef>,
    label: String,
    condition: Option<Expr>,
    target: EntityRef,
}

/// Lowering failure for syntax that is still too raw for HIR.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct HirLowerError {
    message: String,
    range: Option<TextRange>,
}

/// Lowers a parsed syntax tree into HIR-facing structures.
pub fn lower_to_hir(tree: &SyntaxTree) -> Result<HirModule, Vec<HirLowerError>> {
    let mut flows = Vec::new();
    let mut top_level_items = Vec::new();
    let mut errors = Vec::new();

    for item in tree.items() {
        match item {
            Item::Flow(flow) => match lower_flow(flow) {
                Ok(flow) => flows.push(flow),
                Err(err) => errors.push(err),
            },
            Item::FlowItem(item) => match lower_flow_item(item) {
                Ok(item) => top_level_items.push(item),
                Err(err) => errors.push(err),
            },
            Item::Attribute(_) | Item::Hook(_) | Item::MemoFn(_) | Item::Parser(_) => {}
            Item::Raw(raw) => errors.push(HirLowerError::new(
                format!("raw top-level item cannot be lowered: {}", raw.head()),
                Some(raw.range().clone()),
            )),
        }
    }

    if errors.is_empty() {
        Ok(HirModule {
            flows,
            top_level_items,
        })
    } else {
        Err(errors)
    }
}

fn lower_flow(flow: &Flow) -> Result<HirFlow, HirLowerError> {
    let body = flow
        .body()
        .iter()
        .map(lower_flow_item)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HirFlow {
        id: flow.id().cloned(),
        name: flow.name().map(str::to_owned),
        body,
    })
}

fn lower_flow_item(item: &FlowItem) -> Result<HirFlowItem, HirLowerError> {
    match item {
        FlowItem::Stmt(stmt) => Ok(HirFlowItem::Stmt(stmt.clone())),
        FlowItem::ScenarioCommand(command) => Ok(HirFlowItem::Scenario {
            name: command.name().to_owned(),
            args: command.args().to_owned(),
        }),
        FlowItem::SpeakerLine(line) => Ok(HirFlowItem::Dialogue(lower_speaker_line(line))),
        FlowItem::ContentCall(call) => Ok(HirFlowItem::Dialogue(HirDialogue {
            callee: call.callee().to_owned(),
            content: call.content().clone(),
        })),
        FlowItem::Choice(choice) => Ok(HirFlowItem::Choice(lower_choice(choice))),
        FlowItem::Include(entity) => Ok(HirFlowItem::Include(entity.clone())),
        FlowItem::AwaitWith(await_with) => Ok(HirFlowItem::Await {
            expr: await_with.expr().clone(),
            propagates_error: await_with.propagates_error(),
        }),
        FlowItem::Raw(raw) => Err(HirLowerError::new(
            format!("raw flow item cannot be lowered: {raw}"),
            None,
        )),
    }
}

fn lower_speaker_line(line: &SpeakerLine) -> HirDialogue {
    HirDialogue {
        callee: line.speaker().to_owned(),
        content: line.content().clone(),
    }
}

fn lower_choice(choice: &ChoiceBlock) -> HirChoice {
    HirChoice {
        id: choice.id().cloned(),
        options: choice
            .options()
            .iter()
            .map(|option| HirChoiceOption {
                id: option.id().cloned(),
                label: option.label().to_owned(),
                condition: option.condition().cloned(),
                target: option.target().clone(),
            })
            .collect(),
    }
}

impl HirModule {
    pub fn flows(&self) -> &[HirFlow] {
        &self.flows
    }

    pub fn top_level_items(&self) -> &[HirFlowItem] {
        &self.top_level_items
    }
}

impl HirFlow {
    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirDialogue {
    pub fn callee(&self) -> &str {
        &self.callee
    }

    pub const fn content(&self) -> &DialogueContent {
        &self.content
    }
}

impl HirChoice {
    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn options(&self) -> &[HirChoiceOption] {
        &self.options
    }
}

impl HirChoiceOption {
    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn condition(&self) -> Option<&Expr> {
        self.condition.as_ref()
    }

    pub const fn target(&self) -> &EntityRef {
        &self.target
    }
}

impl HirLowerError {
    fn new(message: String, range: Option<TextRange>) -> Self {
        Self { message, range }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn range(&self) -> Option<&TextRange> {
        self.range.as_ref()
    }
}
