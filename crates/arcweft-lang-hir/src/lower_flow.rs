use crate::lower_choice::lower_choice;
use crate::lower_context::LowerContext;
use crate::lower_dialogue::{lower_content_call, lower_speaker_line};
use crate::lower_ids::{
    flow_slug_from_entity, normalize_entity_ref_syntax, normalize_flow_decl_id,
};
use crate::model::{
    HirAwait, HirAwaitBranch, HirBorrow, HirFlow, HirFlowItem, HirFor, HirIf, HirIfLet, HirLoop,
    HirLowerError, HirMatch, HirMatchArm, HirScope, HirScopeExpr, HirSelect, HirSelectBranch,
    HirSourceLocale, HirWhile, HirWhileLet,
};
use arcweft_lang_syntax::ast::flow::{
    AwaitWith, BorrowBlock, Flow, FlowItem, ForBlock, IfBlock, IfLetBlock, LoopBlock, MatchBlock,
    ScopeBlock, ScopeExprBlock, SelectBlock, SourceLocaleBlock, Stmt, WhileBlock, WhileLetBlock,
};

pub(crate) fn lower_flow(flow: &Flow) -> Result<HirFlow, HirLowerError> {
    let id = normalize_flow_decl_id(flow)?;
    let mut context = LowerContext::with_flow_slug(id.as_ref().map(flow_slug_from_entity));
    let body = flow
        .body()
        .iter()
        .map(|item| lower_flow_item_with_context(item, &mut context))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HirFlow {
        kind: flow.kind(),
        id,
        name: flow.name().map(str::to_owned),
        signature: flow.signature().cloned(),
        contracts: flow.contracts().to_vec(),
        body,
    })
}

pub(crate) fn lower_flow_item(item: &FlowItem) -> Result<HirFlowItem, HirLowerError> {
    lower_flow_item_with_context(item, &mut LowerContext::default())
}

pub(crate) fn lower_flow_item_with_context(
    item: &FlowItem,
    context: &mut LowerContext,
) -> Result<HirFlowItem, HirLowerError> {
    match item {
        FlowItem::Stmt(Stmt::LetChoice { pattern, choice }) => Ok(HirFlowItem::LetChoice {
            pattern: pattern.clone(),
            choice: lower_choice(choice, context)?,
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
        FlowItem::SpeakerLine(line) => Ok(HirFlowItem::Dialogue(Box::new(lower_speaker_line(
            line, context,
        )?))),
        FlowItem::ContentCall(call) => Ok(HirFlowItem::Dialogue(Box::new(lower_content_call(
            call, context,
        )?))),
        FlowItem::Choice(choice) => lower_choice(choice, context).map(HirFlowItem::Choice),
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
        FlowItem::Include(entity) => {
            normalize_entity_ref_syntax(entity, context).map(HirFlowItem::Include)
        }
        FlowItem::AwaitWith(await_with) => {
            lower_await_with(await_with, context).map(HirFlowItem::Await)
        }
        FlowItem::Raw(raw) => Err(HirLowerError::new(
            format!(
                "raw {:?} recovery node cannot be lowered: {}",
                raw.family(),
                raw.source()
            ),
            raw.range(),
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
    if let Some(name) = scope.name() {
        context.scopes.push(name.to_owned());
    }
    let lowered = HirScopeExpr {
        name: scope.name().map(str::to_owned),
        statements: scope.statements().to_vec(),
        value: scope.value().cloned(),
    };
    if scope.name().is_some() {
        context.scopes.pop();
    }
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

fn lower_for(block: &ForBlock, context: &mut LowerContext) -> Result<HirFor, HirLowerError> {
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
    block: &SelectBlock,
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
        else_body: block
            .else_body()
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
        else_body: block
            .else_body()
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
