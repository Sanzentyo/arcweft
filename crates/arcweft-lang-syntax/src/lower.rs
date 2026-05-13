use crate::ast::{
    AwaitBranchKind, BorrowBlock, ChoiceBlock, ContractClause, DialogueContent, EntityRef, Flow,
    FlowItem, FlowKind, IfBlock, Item, LinePlan, MatchBlock, Pattern, SpeakerLine, Stmt,
    SyntaxTree, TextRange,
};
use crate::expr::Expr;
use core::fmt;

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
    kind: FlowKind,
    id: Option<EntityRef>,
    name: Option<String>,
    contracts: Vec<ContractClause>,
    body: Vec<HirFlowItem>,
}

/// HIR-facing flow item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirFlowItem {
    Stmt(Stmt),
    Dialogue(HirDialogue),
    Choice(HirChoice),
    If(HirIf),
    Match(HirMatch),
    For(HirFor),
    Select(HirSelect),
    Borrow(HirBorrow),
    Include(EntityRef),
    Await(HirAwait),
    Scenario { name: String, args: Vec<Expr> },
}

/// Dialogue call normalized enough for type checking to resolve speaker symbols.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDialogue {
    callee: String,
    args: Option<String>,
    content: DialogueContent,
    plan: Option<LinePlan>,
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

/// HIR-facing if block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirIf {
    condition: Expr,
    body: Vec<HirFlowItem>,
}

/// HIR-facing match block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMatch {
    expr: Expr,
    arms: Vec<HirMatchArm>,
}

/// HIR-facing match arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMatchArm {
    pattern: Pattern,
    body: Vec<HirFlowItem>,
}

/// HIR-facing sequence loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFor {
    pattern: Pattern,
    source: Expr,
    body: Vec<HirFlowItem>,
}

/// HIR-facing source select block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSelect {
    branches: Vec<HirSelectBranch>,
}

/// HIR-facing select branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSelectBranch {
    head: crate::ast::SelectBranchHead,
    body: Vec<HirFlowItem>,
}

/// HIR-facing zero-copy borrow block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBorrow {
    source: Expr,
    binding: Pattern,
    body: Vec<HirFlowItem>,
}

/// HIR-facing await-with block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirAwait {
    expr: Expr,
    propagates_error: bool,
    branches: Vec<HirAwaitBranch>,
}

/// HIR-facing wait-view branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirAwaitBranch {
    kind: AwaitBranchKind,
    pattern: Pattern,
    body: Vec<HirFlowItem>,
}

/// Lowering failure for syntax that is still too raw for HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
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
            Item::Attribute(_)
            | Item::Callable(_)
            | Item::Enum(_)
            | Item::Function(_)
            | Item::Hook(_)
            | Item::Impl(_)
            | Item::MemoFn(_)
            | Item::Parser(_)
            | Item::State(_)
            | Item::Struct(_)
            | Item::Trait(_)
            | Item::TypeAlias(_) => {}
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
        kind: flow.kind(),
        id: flow.id().cloned(),
        name: flow.name().map(str::to_owned),
        contracts: flow.contracts().to_vec(),
        body,
    })
}

fn lower_flow_item(item: &FlowItem) -> Result<HirFlowItem, HirLowerError> {
    match item {
        FlowItem::Stmt(stmt) => Ok(HirFlowItem::Stmt(stmt.clone())),
        FlowItem::ScenarioCommand(command) => Ok(HirFlowItem::Scenario {
            name: command.name().to_owned(),
            args: command.args().to_vec(),
        }),
        FlowItem::SpeakerLine(line) => Ok(HirFlowItem::Dialogue(lower_speaker_line(line))),
        FlowItem::ContentCall(call) => Ok(HirFlowItem::Dialogue(HirDialogue {
            callee: call.callee().to_owned(),
            args: call.args().map(str::to_owned),
            content: call.content().clone(),
            plan: call.plan().cloned(),
        })),
        FlowItem::Choice(choice) => Ok(HirFlowItem::Choice(lower_choice(choice))),
        FlowItem::If(block) => lower_if(block).map(HirFlowItem::If),
        FlowItem::Match(block) => lower_match(block).map(HirFlowItem::Match),
        FlowItem::For(block) => lower_for(block).map(HirFlowItem::For),
        FlowItem::Select(block) => lower_select(block).map(HirFlowItem::Select),
        FlowItem::BorrowBlock(block) => lower_borrow(block).map(HirFlowItem::Borrow),
        FlowItem::Include(entity) => Ok(HirFlowItem::Include(entity.clone())),
        FlowItem::AwaitWith(await_with) => {
            let branches = await_with
                .branches()
                .iter()
                .map(|branch| {
                    Ok(HirAwaitBranch {
                        kind: branch.kind(),
                        pattern: branch.pattern().clone(),
                        body: branch
                            .body()
                            .iter()
                            .map(lower_flow_item)
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                })
                .collect::<Result<Vec<_>, HirLowerError>>()?;
            Ok(HirFlowItem::Await(HirAwait {
                expr: await_with.expr().clone(),
                propagates_error: await_with.propagates_error(),
                branches,
            }))
        }
        FlowItem::Raw(raw) => Err(HirLowerError::new(
            format!("raw flow item cannot be lowered: {raw}"),
            None,
        )),
    }
}

fn lower_borrow(block: &BorrowBlock) -> Result<HirBorrow, HirLowerError> {
    Ok(HirBorrow {
        source: block.source().clone(),
        binding: block.binding().clone(),
        body: block
            .body()
            .iter()
            .map(lower_flow_item)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_for(block: &crate::ast::ForBlock) -> Result<HirFor, HirLowerError> {
    Ok(HirFor {
        pattern: block.pattern().clone(),
        source: block.source().clone(),
        body: block
            .body()
            .iter()
            .map(lower_flow_item)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_select(block: &crate::ast::SelectBlock) -> Result<HirSelect, HirLowerError> {
    Ok(HirSelect {
        branches: block
            .branches()
            .iter()
            .map(|branch| {
                Ok(HirSelectBranch {
                    head: branch.head().clone(),
                    body: branch
                        .body()
                        .iter()
                        .map(lower_flow_item)
                        .collect::<Result<Vec<_>, _>>()?,
                })
            })
            .collect::<Result<Vec<_>, HirLowerError>>()?,
    })
}

fn lower_if(block: &IfBlock) -> Result<HirIf, HirLowerError> {
    Ok(HirIf {
        condition: block.condition().clone(),
        body: block
            .body()
            .iter()
            .map(lower_flow_item)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_match(block: &MatchBlock) -> Result<HirMatch, HirLowerError> {
    Ok(HirMatch {
        expr: block.expr().clone(),
        arms: block
            .arms()
            .iter()
            .map(|arm| {
                Ok(HirMatchArm {
                    pattern: arm.pattern().clone(),
                    body: arm
                        .body()
                        .iter()
                        .map(lower_flow_item)
                        .collect::<Result<Vec<_>, _>>()?,
                })
            })
            .collect::<Result<Vec<_>, HirLowerError>>()?,
    })
}

fn lower_speaker_line(line: &SpeakerLine) -> HirDialogue {
    HirDialogue {
        callee: line.speaker().to_owned(),
        args: line.args().map(str::to_owned),
        content: line.content().clone(),
        plan: line.plan().cloned(),
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
    pub const fn kind(&self) -> FlowKind {
        self.kind
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }

    pub fn contracts(&self) -> &[ContractClause] {
        &self.contracts
    }
}

impl HirDialogue {
    pub fn callee(&self) -> &str {
        &self.callee
    }

    pub fn args(&self) -> Option<&str> {
        self.args.as_deref()
    }

    pub const fn content(&self) -> &DialogueContent {
        &self.content
    }

    pub const fn plan(&self) -> Option<&LinePlan> {
        self.plan.as_ref()
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

impl HirIf {
    pub const fn condition(&self) -> &Expr {
        &self.condition
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirMatch {
    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    pub fn arms(&self) -> &[HirMatchArm] {
        &self.arms
    }
}

impl HirMatchArm {
    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirFor {
    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn source(&self) -> &Expr {
        &self.source
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirSelect {
    pub fn branches(&self) -> &[HirSelectBranch] {
        &self.branches
    }
}

impl HirSelectBranch {
    pub const fn head(&self) -> &crate::ast::SelectBranchHead {
        &self.head
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirBorrow {
    pub const fn source(&self) -> &Expr {
        &self.source
    }

    pub const fn binding(&self) -> &Pattern {
        &self.binding
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirAwait {
    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    pub const fn propagates_error(&self) -> bool {
        self.propagates_error
    }

    pub fn branches(&self) -> &[HirAwaitBranch] {
        &self.branches
    }
}

impl HirAwaitBranch {
    pub const fn kind(&self) -> AwaitBranchKind {
        self.kind
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
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

impl fmt::Display for HirLowerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for HirLowerError {}
