use crate::ast::{
    Attribute, AwaitBranchKind, AwaitWith, BorrowBlock, CallableItem, ChoiceAction, ChoiceBlock,
    ChoicePlan, ContractClause, DialogueContent, EntityDeclItem, EntityRef, EnumItem,
    ExternModItem, Flow, FlowItem, FlowKind, FunctionItem, FunctionKind, HookItem, IfBlock,
    IfLetBlock, ImplItem, Item, LinePlan, LoopBlock, MatchBlock, MemoFn, ParserItem, Pattern,
    ScopeBlock, ScopeExprBlock, SourceItem, SourceLocaleBlock, SpeakerLine, StateItem, Stmt,
    StructItem, SyntaxTree, TextRange, TraitItem, TypeAliasItem, WhileBlock, WhileLetBlock,
};
use crate::expr::Expr;
use crate::types::FnSignature;
use core::fmt;
use std::collections::HashMap;

/// HIR-facing module produced from parsed surface syntax.
///
/// This is intentionally still close to syntax. Its role is to prove that the
/// parser exposes enough typed structure for later semantic analysis without
/// re-parsing raw strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirModule {
    flows: Vec<HirFlow>,
    functions: Vec<HirFunction>,
    declarations: Vec<HirTopLevelDecl>,
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

/// HIR-facing function body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFunction {
    kind: FunctionKind,
    signature: FnSignature,
    contracts: Vec<ContractClause>,
    statements: Vec<Stmt>,
    value: Option<Expr>,
}

/// HIR-facing top-level declaration preserved for later semantic passes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirTopLevelDecl {
    Attribute(Attribute),
    Callable(CallableItem),
    State(StateItem),
    Trait(TraitItem),
    Impl(ImplItem),
    Enum(EnumItem),
    EntityDecl(EntityDeclItem),
    ExternMod(ExternModItem),
    Struct(StructItem),
    TypeAlias(TypeAliasItem),
    Hook(HookItem),
    MemoFn(MemoFn),
    Parser(ParserItem),
    Source(SourceItem),
}

/// HIR-facing flow item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirFlowItem {
    Stmt(Stmt),
    Dialogue(HirDialogue),
    Choice(HirChoice),
    LetChoice {
        pattern: Pattern,
        choice: HirChoice,
    },
    LetScope {
        pattern: Pattern,
        scope: HirScopeExpr,
    },
    LetLoop {
        pattern: Pattern,
        block: HirLoop,
    },
    LetAwait {
        pattern: Pattern,
        await_with: HirAwait,
    },
    If(HirIf),
    IfLet(HirIfLet),
    Match(HirMatch),
    Loop(HirLoop),
    While(HirWhile),
    WhileLet(HirWhileLet),
    For(HirFor),
    Select(HirSelect),
    Borrow(HirBorrow),
    SourceLocale(HirSourceLocale),
    Scope(HirScope),
    Include(EntityRef),
    Await(HirAwait),
    Scenario {
        name: String,
        args: Vec<Expr>,
    },
}

/// Dialogue call normalized enough for type checking to resolve speaker symbols.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDialogue {
    callee: String,
    args: Option<String>,
    id: Option<EntityRef>,
    text_key: Option<EntityRef>,
    source_locale: Option<String>,
    content: DialogueContent,
    plan: Option<LinePlan>,
}

/// HIR-facing choice block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirChoice {
    id: Option<EntityRef>,
    items: Vec<crate::ast::ChoiceItem>,
    options: Vec<HirChoiceOption>,
    plan: Option<ChoicePlan>,
}

/// HIR-facing choice option.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirChoiceOption {
    id: Option<EntityRef>,
    label: String,
    condition: Option<Expr>,
    action: ChoiceAction,
    value: Option<Expr>,
    label_text_key: Option<EntityRef>,
}

/// HIR-facing source-locale block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSourceLocale {
    locale: String,
    body: Vec<HirFlowItem>,
}

/// HIR-facing lexical scope. Named scopes also affect generated relative IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirScope {
    name: Option<String>,
    body: Vec<HirFlowItem>,
}

/// HIR-facing named scope expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirScopeExpr {
    name: String,
    statements: Vec<Stmt>,
    value: Option<Expr>,
}

#[derive(Clone, Debug, Default)]
struct LowerContext {
    flow_slug: Option<String>,
    scopes: Vec<String>,
    choice_stack: Vec<String>,
    line_counters: HashMap<String, usize>,
}

/// HIR-facing if block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirIf {
    condition: Expr,
    body: Vec<HirFlowItem>,
}

/// HIR-facing if-let block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirIfLet {
    pattern: Pattern,
    expr: Expr,
    guard: Option<Expr>,
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
    guard: Option<Expr>,
    body: Vec<HirFlowItem>,
}

/// HIR-facing value-capable `loop` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLoop {
    label: Option<String>,
    body: Vec<HirFlowItem>,
}

/// HIR-facing sequence loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFor {
    pattern: Pattern,
    source: Expr,
    body: Vec<HirFlowItem>,
}

/// HIR-facing `while` statement loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirWhile {
    condition: Expr,
    body: Vec<HirFlowItem>,
}

/// HIR-facing `while let` statement loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirWhileLet {
    pattern: Pattern,
    expr: Expr,
    guard: Option<Expr>,
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
    applies_try: bool,
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
    let mut functions = Vec::new();
    let mut declarations = Vec::new();
    let mut top_level_items = Vec::new();
    let mut errors = Vec::new();

    for item in tree.items() {
        match item {
            Item::Flow(flow) => match lower_flow(flow) {
                Ok(flow) => flows.push(flow),
                Err(err) => errors.push(err),
            },
            Item::Function(function) => functions.push(lower_function(function)),
            Item::FlowItem(item) => match lower_flow_item(item) {
                Ok(item) => top_level_items.push(item),
                Err(err) => errors.push(err),
            },
            Item::Attribute(item) => {
                declarations.push(HirTopLevelDecl::Attribute(item.clone()));
            }
            Item::Callable(item) => {
                declarations.push(HirTopLevelDecl::Callable(item.clone()));
            }
            Item::Enum(item) => {
                declarations.push(HirTopLevelDecl::Enum(item.clone()));
            }
            Item::EntityDecl(item) => {
                declarations.push(HirTopLevelDecl::EntityDecl(item.clone()));
            }
            Item::ExternMod(item) => {
                declarations.push(HirTopLevelDecl::ExternMod(item.clone()));
            }
            Item::Hook(item) => {
                declarations.push(HirTopLevelDecl::Hook(item.clone()));
            }
            Item::Impl(item) => {
                declarations.push(HirTopLevelDecl::Impl(item.clone()));
            }
            Item::MemoFn(item) => {
                declarations.push(HirTopLevelDecl::MemoFn(item.clone()));
            }
            Item::Parser(item) => {
                declarations.push(HirTopLevelDecl::Parser(item.clone()));
            }
            Item::Source(item) => {
                declarations.push(HirTopLevelDecl::Source(item.clone()));
            }
            Item::State(item) => {
                declarations.push(HirTopLevelDecl::State(item.clone()));
            }
            Item::Struct(item) => {
                declarations.push(HirTopLevelDecl::Struct(item.clone()));
            }
            Item::Trait(item) => {
                declarations.push(HirTopLevelDecl::Trait(item.clone()));
            }
            Item::TypeAlias(item) => {
                declarations.push(HirTopLevelDecl::TypeAlias(item.clone()));
            }
            Item::Raw(raw) => errors.push(HirLowerError::new(
                format!("raw top-level item cannot be lowered: {}", raw.head()),
                Some(*raw.range()),
            )),
        }
    }

    if errors.is_empty() {
        Ok(HirModule {
            flows,
            functions,
            declarations,
            top_level_items,
        })
    } else {
        Err(errors)
    }
}

fn lower_function(function: &FunctionItem) -> HirFunction {
    HirFunction {
        kind: function.kind(),
        signature: function.signature().clone(),
        contracts: function.contracts().to_vec(),
        statements: function.body_statements().to_vec(),
        value: function.body_value().cloned(),
    }
}

fn lower_flow(flow: &Flow) -> Result<HirFlow, HirLowerError> {
    let mut context = LowerContext {
        flow_slug: flow.id().map(flow_slug_from_entity),
        ..LowerContext::default()
    };
    let body = flow
        .body()
        .iter()
        .map(|item| lower_flow_item_with_context(item, &mut context))
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
    lower_flow_item_with_context(item, &mut LowerContext::default())
}

fn lower_flow_item_with_context(
    item: &FlowItem,
    context: &mut LowerContext,
) -> Result<HirFlowItem, HirLowerError> {
    match item {
        FlowItem::Stmt(Stmt::LetChoice { pattern, choice }) => Ok(HirFlowItem::LetChoice {
            pattern: pattern.clone(),
            choice: lower_choice(choice, context),
        }),
        FlowItem::Stmt(Stmt::LetScope { pattern, scope }) => Ok(HirFlowItem::LetScope {
            pattern: pattern.clone(),
            scope: lower_scope_expr(scope, context),
        }),
        FlowItem::Stmt(Stmt::LetLoop { pattern, block }) => Ok(HirFlowItem::LetLoop {
            pattern: pattern.clone(),
            block: lower_loop(block, context)?,
        }),
        FlowItem::Stmt(Stmt::LetAwait {
            pattern,
            await_with,
        }) => Ok(HirFlowItem::LetAwait {
            pattern: pattern.clone(),
            await_with: lower_await_with(await_with, context)?,
        }),
        FlowItem::Stmt(stmt) => Ok(HirFlowItem::Stmt(stmt.clone())),
        FlowItem::ScenarioCommand(command) => Ok(HirFlowItem::Scenario {
            name: command.name().to_owned(),
            args: command.args().to_vec(),
        }),
        FlowItem::SpeakerLine(line) => Ok(HirFlowItem::Dialogue(lower_speaker_line(line, context))),
        FlowItem::ContentCall(call) => Ok(HirFlowItem::Dialogue(lower_content_call(call, context))),
        FlowItem::Choice(choice) => Ok(HirFlowItem::Choice(lower_choice(choice, context))),
        FlowItem::If(block) => lower_if(block, context).map(HirFlowItem::If),
        FlowItem::IfLet(block) => lower_if_let(block, context).map(HirFlowItem::IfLet),
        FlowItem::Match(block) => lower_match(block, context).map(HirFlowItem::Match),
        FlowItem::Loop(block) => lower_loop(block, context).map(HirFlowItem::Loop),
        FlowItem::While(block) => lower_while(block, context).map(HirFlowItem::While),
        FlowItem::WhileLet(block) => lower_while_let(block, context).map(HirFlowItem::WhileLet),
        FlowItem::For(block) => lower_for(block, context).map(HirFlowItem::For),
        FlowItem::Select(block) => lower_select(block, context).map(HirFlowItem::Select),
        FlowItem::BorrowBlock(block) => lower_borrow(block, context).map(HirFlowItem::Borrow),
        FlowItem::SourceLocale(block) => {
            lower_source_locale(block, context).map(HirFlowItem::SourceLocale)
        }
        FlowItem::Scope(block) => lower_scope(block, context).map(HirFlowItem::Scope),
        FlowItem::Include(entity) => Ok(HirFlowItem::Include(entity.clone())),
        FlowItem::AwaitWith(await_with) => {
            lower_await_with(await_with, context).map(HirFlowItem::Await)
        }
        FlowItem::Raw(raw) => Err(HirLowerError::new(
            format!("raw flow item cannot be lowered: {raw}"),
            None,
        )),
    }
}

fn lower_await_with(
    await_with: &AwaitWith,
    context: &mut LowerContext,
) -> Result<HirAwait, HirLowerError> {
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
                    .map(|item| lower_flow_item_with_context(item, context))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
        .collect::<Result<Vec<_>, HirLowerError>>()?;
    Ok(HirAwait {
        expr: await_with.expr().clone(),
        applies_try: await_with.applies_try(),
        branches,
    })
}

fn lower_scope_expr(scope: &ScopeExprBlock, context: &mut LowerContext) -> HirScopeExpr {
    context.scopes.push(scope.name().to_owned());
    let lowered = HirScopeExpr {
        name: scope.name().to_owned(),
        statements: scope.statements().to_vec(),
        value: scope.value().cloned(),
    };
    context.scopes.pop();
    lowered
}

fn lower_borrow(
    block: &BorrowBlock,
    context: &mut LowerContext,
) -> Result<HirBorrow, HirLowerError> {
    Ok(HirBorrow {
        source: block.source().clone(),
        binding: block.binding().clone(),
        body: block
            .body()
            .iter()
            .map(|item| lower_flow_item_with_context(item, context))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_loop(block: &LoopBlock, context: &mut LowerContext) -> Result<HirLoop, HirLowerError> {
    Ok(HirLoop {
        label: block.label().map(str::to_owned),
        body: block
            .body()
            .iter()
            .map(|item| lower_flow_item_with_context(item, context))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_while(block: &WhileBlock, context: &mut LowerContext) -> Result<HirWhile, HirLowerError> {
    Ok(HirWhile {
        condition: block.condition().clone(),
        body: block
            .body()
            .iter()
            .map(|item| lower_flow_item_with_context(item, context))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_while_let(
    block: &WhileLetBlock,
    context: &mut LowerContext,
) -> Result<HirWhileLet, HirLowerError> {
    Ok(HirWhileLet {
        pattern: block.pattern().clone(),
        expr: block.expr().clone(),
        guard: block.guard().cloned(),
        body: block
            .body()
            .iter()
            .map(|item| lower_flow_item_with_context(item, context))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_for(
    block: &crate::ast::ForBlock,
    context: &mut LowerContext,
) -> Result<HirFor, HirLowerError> {
    Ok(HirFor {
        pattern: block.pattern().clone(),
        source: block.source().clone(),
        body: block
            .body()
            .iter()
            .map(|item| lower_flow_item_with_context(item, context))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_select(
    block: &crate::ast::SelectBlock,
    context: &mut LowerContext,
) -> Result<HirSelect, HirLowerError> {
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
                        .map(|item| lower_flow_item_with_context(item, context))
                        .collect::<Result<Vec<_>, _>>()?,
                })
            })
            .collect::<Result<Vec<_>, HirLowerError>>()?,
    })
}

fn lower_if(block: &IfBlock, context: &mut LowerContext) -> Result<HirIf, HirLowerError> {
    Ok(HirIf {
        condition: block.condition().clone(),
        body: block
            .body()
            .iter()
            .map(|item| lower_flow_item_with_context(item, context))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_if_let(block: &IfLetBlock, context: &mut LowerContext) -> Result<HirIfLet, HirLowerError> {
    Ok(HirIfLet {
        pattern: block.pattern().clone(),
        expr: block.expr().clone(),
        guard: block.guard().cloned(),
        body: block
            .body()
            .iter()
            .map(|item| lower_flow_item_with_context(item, context))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_match(block: &MatchBlock, context: &mut LowerContext) -> Result<HirMatch, HirLowerError> {
    Ok(HirMatch {
        expr: block.expr().clone(),
        arms: block
            .arms()
            .iter()
            .map(|arm| {
                Ok(HirMatchArm {
                    pattern: arm.pattern().clone(),
                    guard: arm.guard().cloned(),
                    body: arm
                        .body()
                        .iter()
                        .map(|item| lower_flow_item_with_context(item, context))
                        .collect::<Result<Vec<_>, _>>()?,
                })
            })
            .collect::<Result<Vec<_>, HirLowerError>>()?,
    })
}

fn lower_speaker_line(line: &SpeakerLine, context: &mut LowerContext) -> HirDialogue {
    let speaker = speaker_slug(line.speaker());
    let id = normalize_line_id(line.options().id(), speaker.clone(), context, *line.range());
    HirDialogue {
        callee: line.speaker().to_owned(),
        args: line.args().map(str::to_owned),
        text_key: normalize_line_text_key(line.options().text_key(), id.as_ref(), speaker, context),
        id,
        source_locale: line.options().source_locale().map(str::to_owned),
        content: line.content().clone(),
        plan: line.plan().cloned(),
    }
}

fn lower_content_call(call: &crate::ast::ContentCall, context: &mut LowerContext) -> HirDialogue {
    let speaker = content_callee_slug(call.callee());
    let id = normalize_line_id(call.options().id(), speaker.clone(), context, *call.range());
    HirDialogue {
        callee: call.callee().to_owned(),
        args: call.args().map(str::to_owned),
        text_key: normalize_line_text_key(call.options().text_key(), id.as_ref(), speaker, context),
        id,
        source_locale: call.options().source_locale().map(str::to_owned),
        content: call.content().clone(),
        plan: call.plan().cloned(),
    }
}

fn lower_choice(choice: &ChoiceBlock, context: &mut LowerContext) -> HirChoice {
    let id = choice
        .id()
        .map(|id| normalize_choice_id(id, context))
        .or_else(|| choice.id().cloned());
    if let Some(id) = &id {
        context.choice_stack.push(id.body().to_owned());
    }
    let options = choice
        .options()
        .iter()
        .map(|option| HirChoiceOption {
            id: option.id().map(|id| normalize_option_id(id, context)),
            label: option.label().to_owned(),
            condition: option.condition().cloned(),
            action: option.action().clone(),
            value: option.value().cloned(),
            label_text_key: option
                .label_text_key()
                .map(|id| normalize_text_key_id(id, context)),
        })
        .collect();
    if choice.id().is_some() {
        context.choice_stack.pop();
    }
    HirChoice {
        id,
        items: choice.items().to_vec(),
        plan: choice.plan().cloned(),
        options,
    }
}

fn lower_source_locale(
    block: &SourceLocaleBlock,
    context: &mut LowerContext,
) -> Result<HirSourceLocale, HirLowerError> {
    Ok(HirSourceLocale {
        locale: block.locale().to_owned(),
        body: block
            .body()
            .iter()
            .map(|item| lower_flow_item_with_context(item, context))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_scope(block: &ScopeBlock, context: &mut LowerContext) -> Result<HirScope, HirLowerError> {
    if let Some(name) = block.name() {
        context.scopes.push(name.to_owned());
    }
    let body = block
        .body()
        .iter()
        .map(|item| lower_flow_item_with_context(item, context))
        .collect::<Result<Vec<_>, _>>()?;
    if block.name().is_some() {
        context.scopes.pop();
    }
    Ok(HirScope {
        name: block.name().map(str::to_owned),
        body,
    })
}

fn normalize_choice_id(id: &EntityRef, context: &LowerContext) -> EntityRef {
    if !id.is_relative() {
        return id.clone();
    }
    let Some(flow_slug) = &context.flow_slug else {
        return id.clone();
    };
    let mut parts = vec!["choice".to_owned(), flow_slug.clone()];
    parts.extend(context.scopes.iter().cloned());
    parts.push(id.body().to_owned());
    EntityRef::new(parts.join("."), false, *id.range())
}

fn normalize_option_id(id: &EntityRef, context: &LowerContext) -> EntityRef {
    if !id.is_relative() {
        return id.clone();
    }
    context.choice_stack.last().map_or_else(
        || id.clone(),
        |choice| EntityRef::new(format!("{choice}.{}", id.body()), false, *id.range()),
    )
}

fn normalize_text_key_id(id: &EntityRef, context: &LowerContext) -> EntityRef {
    if !id.is_relative() {
        return id.clone();
    }
    context.choice_stack.last().map_or_else(
        || id.clone(),
        |choice| {
            let choice_path = choice.strip_prefix("choice.").unwrap_or(choice);
            EntityRef::new(
                format!("text.choice.{choice_path}.{}", id.body()),
                false,
                *id.range(),
            )
        },
    )
}

fn normalize_line_id(
    id: Option<&EntityRef>,
    speaker: String,
    context: &mut LowerContext,
    range: TextRange,
) -> Option<EntityRef> {
    match id {
        Some(id) if !id.is_relative() => Some(id.clone()),
        Some(id) => Some(build_line_entity_ref(
            speaker,
            Some(id.body()),
            context,
            *id.range(),
        )?),
        None => Some(build_line_entity_ref(speaker, None, context, range)?),
    }
}

fn normalize_line_text_key(
    text_key: Option<&EntityRef>,
    line_id: Option<&EntityRef>,
    speaker: String,
    context: &LowerContext,
) -> Option<EntityRef> {
    if let Some(text_key) = text_key {
        if !text_key.is_relative() {
            return Some(text_key.clone());
        }
        let Some(flow_slug) = &context.flow_slug else {
            return Some(text_key.clone());
        };
        let mut parts = vec!["text".to_owned(), flow_slug.clone(), speaker];
        parts.extend(context.scopes.iter().cloned());
        parts.push(text_key.body().to_owned());
        return Some(EntityRef::new(parts.join("."), false, *text_key.range()));
    }
    line_id.map(|id| EntityRef::new(line_id_to_text_key(id.body()), false, *id.range()))
}

fn build_line_entity_ref(
    speaker: String,
    explicit_suffix: Option<&str>,
    context: &mut LowerContext,
    range: TextRange,
) -> Option<EntityRef> {
    let flow_slug = context.flow_slug.as_ref()?;
    let mut parts = vec!["say".to_owned(), flow_slug.clone(), speaker];
    parts.extend(context.scopes.iter().cloned());
    let prefix = parts.join(".");
    let suffix = explicit_suffix.map_or_else(
        || {
            let next = context.line_counters.entry(prefix.clone()).or_insert(0);
            *next += 1;
            format!("{next:03}")
        },
        str::to_owned,
    );
    Some(EntityRef::new(format!("{prefix}.{suffix}"), false, range))
}

fn line_id_to_text_key(line_id: &str) -> String {
    line_id
        .strip_prefix("say.")
        .map_or_else(|| format!("text.{line_id}"), |tail| format!("text.{tail}"))
}

fn speaker_slug(speaker: &str) -> String {
    match speaker {
        "地の文" | "narrator" => "narrator".to_owned(),
        other => {
            let source = other
                .trim()
                .strip_prefix("#<")
                .and_then(|inner| inner.strip_suffix('>'))
                .or_else(|| other.trim().strip_prefix('#'))
                .unwrap_or(other)
                .trim_end_matches(".say");
            source
                .rsplit(['.', ':'])
                .next()
                .unwrap_or(source)
                .to_owned()
        }
    }
}

fn content_callee_slug(callee: &str) -> String {
    callee
        .strip_suffix(".say")
        .map_or_else(|| speaker_slug(callee), speaker_slug)
}

fn flow_slug_from_entity(id: &EntityRef) -> String {
    id.body()
        .strip_prefix("flow.")
        .or_else(|| id.body().strip_prefix("fragment."))
        .unwrap_or(id.body())
        .to_owned()
}

impl HirModule {
    pub fn flows(&self) -> &[HirFlow] {
        &self.flows
    }

    pub fn functions(&self) -> &[HirFunction] {
        &self.functions
    }

    pub fn declarations(&self) -> &[HirTopLevelDecl] {
        &self.declarations
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

impl HirFunction {
    pub const fn kind(&self) -> FunctionKind {
        self.kind
    }

    pub fn name(&self) -> &str {
        self.signature.name()
    }

    pub const fn signature(&self) -> &FnSignature {
        &self.signature
    }

    pub fn contracts(&self) -> &[ContractClause] {
        &self.contracts
    }

    pub fn statements(&self) -> &[Stmt] {
        &self.statements
    }

    pub const fn value(&self) -> Option<&Expr> {
        self.value.as_ref()
    }
}

impl HirDialogue {
    pub fn callee(&self) -> &str {
        &self.callee
    }

    pub fn args(&self) -> Option<&str> {
        self.args.as_deref()
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub const fn text_key(&self) -> Option<&EntityRef> {
        self.text_key.as_ref()
    }

    pub fn source_locale(&self) -> Option<&str> {
        self.source_locale.as_deref()
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

    pub fn items(&self) -> &[crate::ast::ChoiceItem] {
        &self.items
    }

    pub const fn plan(&self) -> Option<&ChoicePlan> {
        self.plan.as_ref()
    }
}

impl HirScope {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirScopeExpr {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn statements(&self) -> &[Stmt] {
        &self.statements
    }

    pub const fn value(&self) -> Option<&Expr> {
        self.value.as_ref()
    }
}

impl HirLoop {
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
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

    pub const fn target(&self) -> Option<&EntityRef> {
        match &self.action {
            ChoiceAction::Goto(target) => Some(target),
            _ => None,
        }
    }

    pub const fn action(&self) -> &ChoiceAction {
        &self.action
    }

    pub const fn value(&self) -> Option<&Expr> {
        self.value.as_ref()
    }

    pub const fn label_text_key(&self) -> Option<&EntityRef> {
        self.label_text_key.as_ref()
    }
}

impl HirSourceLocale {
    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
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

impl HirIfLet {
    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    pub const fn guard(&self) -> Option<&Expr> {
        self.guard.as_ref()
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

    pub const fn guard(&self) -> Option<&Expr> {
        self.guard.as_ref()
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

impl HirWhile {
    pub const fn condition(&self) -> &Expr {
        &self.condition
    }

    pub fn body(&self) -> &[HirFlowItem] {
        &self.body
    }
}

impl HirWhileLet {
    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    pub const fn guard(&self) -> Option<&Expr> {
        self.guard.as_ref()
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

    pub const fn applies_try(&self) -> bool {
        self.applies_try
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
