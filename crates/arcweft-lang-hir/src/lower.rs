use crate::model::{
    HirAwait, HirAwaitBranch, HirBorrow, HirChoice, HirChoiceOption, HirDialogue, HirFlow,
    HirFlowItem, HirFor, HirFunction, HirIf, HirIfLet, HirLoop, HirLowerError, HirMatch,
    HirMatchArm, HirModule, HirScope, HirScopeExpr, HirSelect, HirSelectBranch, HirSourceLocale,
    HirTopLevelDecl, HirWhile, HirWhileLet,
};
use arcweft_lang_syntax::{
    AwaitWith, BorrowBlock, ChoiceAction, ChoiceBlock, EntityRef, EntityRefSyntax, Flow, FlowItem,
    FlowKind, FunctionItem, IdRef, IfBlock, IfLetBlock, Item, LoopBlock, MatchBlock, RelativeId,
    ScopeBlock, ScopeExprBlock, SourceLocaleBlock, SpeakerLine, Stmt, TextRange, TypedSyntaxTree,
    WhileBlock, WhileLetBlock,
};
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
struct LowerContext {
    flow_slug: Option<String>,
    scopes: Vec<String>,
    choice_stack: Vec<String>,
    line_counters: HashMap<String, usize>,
}

// TODO(lint): Module paths and named `scope` blocks should both be available to
// the ID policy checker. Today lowering derives relative IDs from the current
// flow ID and named scopes only; a later lint pass should compare generated IDs
// against the source module path and report IDs that break the project hierarchy.

/// Lowers a parsed syntax tree into HIR-facing structures.
pub fn lower_to_hir(tree: &TypedSyntaxTree) -> Result<HirModule, Vec<HirLowerError>> {
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
            Item::DialogueDefaults(item) => {
                declarations.push(HirTopLevelDecl::DialogueDefaults(item.clone()));
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
            Item::Proof(item) => {
                declarations.push(HirTopLevelDecl::Proof(item.clone()));
            }
            Item::TrustedAxiom(item) => {
                declarations.push(HirTopLevelDecl::TrustedAxiom(item.clone()));
            }
            Item::Test(item) => {
                declarations.push(HirTopLevelDecl::Test(item.clone()));
            }
            Item::Bench(item) => {
                declarations.push(HirTopLevelDecl::Bench(item.clone()));
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
    let id = normalize_flow_decl_id(flow)?;
    let mut context = LowerContext {
        flow_slug: id.as_ref().map(flow_slug_from_entity),
        ..LowerContext::default()
    };
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

fn normalize_flow_decl_id(flow: &Flow) -> Result<Option<EntityRef>, HirLowerError> {
    let family = flow_decl_family(flow.kind());
    match flow.id() {
        Some(IdRef::Absolute(id)) => Ok(Some(id.clone())),
        Some(IdRef::Relative(relative)) => Ok(Some(EntityRef::new(
            format!("{family}.{}", relative.suffix()),
            false,
            *relative.range(),
        ))),
        Some(IdRef::FamilyRelative(relative)) => {
            if !flow_decl_family_matches(flow.kind(), relative.family()) {
                return Err(HirLowerError::new(
                    format!(
                        "{} declaration cannot use `{}` family-relative id",
                        flow_decl_family(flow.kind()),
                        relative.family()
                    ),
                    Some(*relative.range()),
                ));
            }
            Ok(Some(EntityRef::new(
                format!("{family}.{}", relative.relative().suffix()),
                false,
                *relative.range(),
            )))
        }
        None => Ok(flow
            .name()
            .map(|name| EntityRef::new(format!("{family}.{name}"), false, *flow.range()))),
    }
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

fn lower_for(
    block: &arcweft_lang_syntax::ForBlock,
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
    block: &arcweft_lang_syntax::SelectBlock,
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

fn lower_speaker_line(
    line: &SpeakerLine,
    context: &mut LowerContext,
) -> Result<HirDialogue, HirLowerError> {
    let speaker = speaker_slug(line.speaker());
    let id = normalize_line_id(line.options().id(), speaker.clone(), context, *line.range())?;
    let text_key =
        normalize_line_text_key(line.options().text_key(), id.as_ref(), speaker, context)?;
    let window = line
        .options()
        .window()
        .map(|window| normalize_entity_ref_syntax(window, context))
        .transpose()?;
    Ok(HirDialogue {
        callee: line.speaker().to_owned(),
        text_key,
        id,
        voice: line.options().voice().cloned(),
        look: line.options().look().cloned(),
        stage: line.options().stage().cloned(),
        portrait: line.options().portrait().cloned(),
        focus: line.options().focus().cloned(),
        cleanup: line.options().cleanup().cloned(),
        window,
        source_locale: line.options().source_locale().map(str::to_owned),
        hooks: line.options().hooks().to_vec(),
        style: line.options().style().cloned(),
        args: line.options().args().to_vec(),
        content: line.content().clone(),
        plan: line.plan().cloned(),
    })
}

fn lower_content_call(
    call: &arcweft_lang_syntax::ContentCall,
    context: &mut LowerContext,
) -> Result<HirDialogue, HirLowerError> {
    let speaker = content_callee_slug(call.callee());
    let id = normalize_line_id(call.options().id(), speaker.clone(), context, *call.range())?;
    let text_key =
        normalize_line_text_key(call.options().text_key(), id.as_ref(), speaker, context)?;
    let window = call
        .options()
        .window()
        .map(|window| normalize_entity_ref_syntax(window, context))
        .transpose()?;
    Ok(HirDialogue {
        callee: call.callee().to_owned(),
        text_key,
        id,
        voice: call.options().voice().cloned(),
        look: call.options().look().cloned(),
        stage: call.options().stage().cloned(),
        portrait: call.options().portrait().cloned(),
        focus: call.options().focus().cloned(),
        cleanup: call.options().cleanup().cloned(),
        window,
        source_locale: call.options().source_locale().map(str::to_owned),
        hooks: call.options().hooks().to_vec(),
        style: call.options().style().cloned(),
        args: call.options().args().to_vec(),
        content: call.content().clone(),
        plan: call.plan().cloned(),
    })
}

fn lower_choice(
    choice: &ChoiceBlock,
    context: &mut LowerContext,
) -> Result<HirChoice, HirLowerError> {
    let id = choice
        .id()
        .map(|id| normalize_choice_id(id, context))
        .transpose()?;
    if let Some(id) = &id {
        context.choice_stack.push(id.body().to_owned());
    }
    let options = choice
        .options()
        .iter()
        .map(|option| {
            Ok(HirChoiceOption {
                id: option
                    .id()
                    .map(|id| normalize_option_id(id, context))
                    .transpose()?,
                label: option.label().to_owned(),
                condition: option.condition().cloned(),
                action: normalize_choice_action(option.action(), context)?,
                value: option.value().cloned(),
                label_text_key: option
                    .label_text_key()
                    .map(|id| normalize_text_key_id(id, context))
                    .transpose()?,
            })
        })
        .collect::<Result<Vec<_>, HirLowerError>>()?;
    if choice.id().is_some() {
        context.choice_stack.pop();
    }
    Ok(HirChoice {
        id,
        items: choice.items().to_vec(),
        plan: choice.plan().cloned(),
        options,
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

fn normalize_choice_action(
    action: &ChoiceAction,
    context: &LowerContext,
) -> Result<ChoiceAction, HirLowerError> {
    match action {
        ChoiceAction::Goto(target) => normalize_entity_ref_syntax(target, context)
            .map(EntityRefSyntax::absolute)
            .map(ChoiceAction::Goto),
        ChoiceAction::Out(expr) => Ok(ChoiceAction::Out(expr.clone())),
        ChoiceAction::SelectBlock(body) => Ok(ChoiceAction::SelectBlock(body.clone())),
        ChoiceAction::None => Ok(ChoiceAction::None),
    }
}

fn normalize_entity_ref_syntax(
    entity: &EntityRefSyntax,
    context: &LowerContext,
) -> Result<EntityRef, HirLowerError> {
    match entity {
        EntityRefSyntax::Absolute(entity) => Ok(entity.clone()),
        EntityRefSyntax::FamilyRelative(relative) => {
            let Some(flow_slug) = &context.flow_slug else {
                return Err(HirLowerError::new(
                    "relative entity reference requires a flow context",
                    Some(*relative.range()),
                ));
            };
            // Family-relative refs are the recommended spelling for reference
            // positions (`@flow:.next`, `@frag:.intro`) because the family keeps
            // lookup separate from ID-bearing `@.suffix` declaration contexts.
            let mut parts = vec![relative.family().to_owned(), flow_slug.clone()];
            parts.extend(relative_scopes(context, relative.relative())?);
            parts.push(relative.relative().suffix().to_owned());
            Ok(EntityRef::new(parts.join("."), false, *relative.range()))
        }
    }
}

fn normalize_choice_id(id: &IdRef, context: &LowerContext) -> Result<EntityRef, HirLowerError> {
    let relative = match id {
        IdRef::Absolute(id) => return Ok(id.clone()),
        IdRef::Relative(relative) => relative,
        IdRef::FamilyRelative(relative) => {
            ensure_id_family(relative.family(), "choice", relative.range())?;
            relative.relative()
        }
    };
    let Some(flow_slug) = &context.flow_slug else {
        return Err(HirLowerError::new(
            "relative choice ID requires a flow context",
            Some(*id.range()),
        ));
    };
    let mut parts = vec!["choice".to_owned(), flow_slug.clone()];
    parts.extend(relative_scopes(context, relative)?);
    parts.push(relative.suffix().to_owned());
    Ok(EntityRef::new(parts.join("."), false, *id.range()))
}

fn normalize_option_id(id: &IdRef, context: &LowerContext) -> Result<EntityRef, HirLowerError> {
    let relative = match id {
        IdRef::Absolute(id) => return Ok(id.clone()),
        IdRef::Relative(relative) => relative,
        IdRef::FamilyRelative(relative) => {
            ensure_id_family(relative.family(), "choice", relative.range())?;
            relative.relative()
        }
    };
    let Some(choice) = context.choice_stack.last() else {
        return Err(HirLowerError::new(
            "relative option ID requires an enclosing choice",
            Some(*id.range()),
        ));
    };
    Ok(EntityRef::new(
        append_relative_suffix(choice, relative.suffix(), relative.parent_depth())?,
        false,
        *id.range(),
    ))
}

fn normalize_text_key_id(id: &IdRef, context: &LowerContext) -> Result<EntityRef, HirLowerError> {
    let relative = match id {
        IdRef::Absolute(id) => return Ok(id.clone()),
        IdRef::Relative(relative) => relative,
        IdRef::FamilyRelative(relative) => {
            ensure_id_family(relative.family(), "text", relative.range())?;
            relative.relative()
        }
    };
    let Some(choice) = context.choice_stack.last() else {
        return Err(HirLowerError::new(
            "relative choice text key requires an enclosing choice",
            Some(*id.range()),
        ));
    };
    let normalized_choice = append_relative_suffix(choice, "", relative.parent_depth())?;
    let choice_path = normalized_choice
        .trim_end_matches('.')
        .strip_prefix("choice.")
        .unwrap_or(normalized_choice.trim_end_matches('.'));
    Ok(EntityRef::new(
        format!("text.choice.{choice_path}.{}", relative.suffix()),
        false,
        *id.range(),
    ))
}

fn normalize_line_id(
    id: Option<&IdRef>,
    speaker: String,
    context: &mut LowerContext,
    range: TextRange,
) -> Result<Option<EntityRef>, HirLowerError> {
    if context.flow_slug.is_none() && !matches!(id, Some(IdRef::Absolute(_))) {
        return Ok(None);
    }
    match id {
        Some(IdRef::Absolute(id)) => Ok(Some(id.clone())),
        Some(IdRef::Relative(relative)) => Ok(Some(build_line_entity_ref(
            speaker,
            Some(relative),
            context,
            *relative.range(),
        )?)),
        Some(IdRef::FamilyRelative(relative)) => {
            ensure_id_family(relative.family(), "say", relative.range())?;
            Ok(Some(build_line_entity_ref(
                speaker,
                Some(relative.relative()),
                context,
                *relative.range(),
            )?))
        }
        None => Ok(Some(build_line_entity_ref(speaker, None, context, range)?)),
    }
}

fn normalize_line_text_key(
    text_key: Option<&IdRef>,
    line_id: Option<&EntityRef>,
    speaker: String,
    context: &LowerContext,
) -> Result<Option<EntityRef>, HirLowerError> {
    if let Some(text_key) = text_key {
        let relative = match text_key {
            IdRef::Absolute(text_key) => return Ok(Some(text_key.clone())),
            IdRef::Relative(relative) => relative,
            IdRef::FamilyRelative(relative) => {
                ensure_id_family(relative.family(), "text", relative.range())?;
                relative.relative()
            }
        };
        let Some(flow_slug) = &context.flow_slug else {
            return Err(HirLowerError::new(
                "relative text key requires a flow context",
                Some(*text_key.range()),
            ));
        };
        let mut parts = vec!["text".to_owned(), flow_slug.clone(), speaker];
        parts.extend(relative_scopes(context, relative)?);
        parts.push(relative.suffix().to_owned());
        return Ok(Some(EntityRef::new(
            parts.join("."),
            false,
            *text_key.range(),
        )));
    }
    Ok(line_id.map(|id| EntityRef::new(line_id_to_text_key(id.body()), false, *id.range())))
}

fn ensure_id_family(found: &str, expected: &str, range: &TextRange) -> Result<(), HirLowerError> {
    if found == expected {
        Ok(())
    } else {
        Err(HirLowerError::new(
            format!("relative ID family `{found}` is not valid here; expected `{expected}`"),
            Some(*range),
        ))
    }
}

fn build_line_entity_ref(
    speaker: String,
    explicit_id: Option<&RelativeId>,
    context: &mut LowerContext,
    range: TextRange,
) -> Result<EntityRef, HirLowerError> {
    let Some(flow_slug) = context.flow_slug.as_ref() else {
        return Err(HirLowerError::new(
            "dialogue line ID requires a flow context",
            Some(range),
        ));
    };
    let mut parts = vec!["say".to_owned(), flow_slug.clone(), speaker];
    if let Some(id) = explicit_id {
        parts.extend(relative_scopes(context, id)?);
    } else {
        parts.extend(context.scopes.iter().cloned());
    }
    let prefix = parts.join(".");
    let suffix = explicit_id.map_or_else(
        || {
            let next = context.line_counters.entry(prefix.clone()).or_insert(0);
            *next += 1;
            format!("{next:03}")
        },
        |id| id.suffix().to_owned(),
    );
    Ok(EntityRef::new(format!("{prefix}.{suffix}"), false, range))
}

fn relative_scopes(
    context: &LowerContext,
    relative: &RelativeId,
) -> Result<Vec<String>, HirLowerError> {
    // TODO(lint): `@...suffix` is accepted for machine output and compact
    // authoring, but hand-written source should be nudged toward explicit
    // `@super.super.suffix` once a lint/formatter layer exists.
    let Some(take_len) = context.scopes.len().checked_sub(relative.parent_depth()) else {
        return Err(HirLowerError::new(
            "relative ID walks past the available ID scopes",
            Some(*relative.range()),
        ));
    };
    Ok(context.scopes.iter().take(take_len).cloned().collect())
}

fn append_relative_suffix(
    base: &str,
    suffix: &str,
    parent_depth: usize,
) -> Result<String, HirLowerError> {
    let mut parts = base.split('.').map(str::to_owned).collect::<Vec<_>>();
    for _ in 0..parent_depth {
        if parts.len() <= 1 {
            return Err(HirLowerError::new(
                "relative ID walks past the available ID scopes",
                None,
            ));
        }
        parts.pop();
    }
    if !suffix.is_empty() {
        parts.push(suffix.to_owned());
    }
    Ok(parts.join("."))
}

fn line_id_to_text_key(line_id: &str) -> String {
    line_id
        .strip_prefix("say.")
        .map_or_else(|| format!("text.{line_id}"), |tail| format!("text.{tail}"))
}

fn flow_decl_family(kind: FlowKind) -> &'static str {
    match kind {
        FlowKind::Flow => "flow",
        FlowKind::Fragment => "fragment",
    }
}

fn flow_decl_family_matches(kind: FlowKind, family: &str) -> bool {
    match kind {
        FlowKind::Flow => family == "flow",
        FlowKind::Fragment => matches!(family, "fragment" | "frag"),
    }
}

fn speaker_slug(speaker: &str) -> String {
    match speaker.trim() {
        "地の文" | "地文" | "ナレーター" | "ナレータ" | "ナレーション" | "語り" | "語り手"
        | "narrator" | "Narrator" | "NARRATOR" | "VO" | "V.O." | "O.S." | "Offscreen"
        | "Script" | "StageDirection" | "ト書き" | "脚本" => "narrator".to_owned(),
        other => {
            let source = other
                .trim()
                .strip_prefix("@<")
                .and_then(|inner| inner.strip_suffix('>'))
                .or_else(|| other.trim().strip_prefix('@'))
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
