use super::{
    CallArg, ChoiceAction, EntityRef, EntryItem, Expr, HirFlowItem, HirModule, HirTopLevelDecl,
    MatchExprArm, ProjectGraphDependencyRelation, ProjectGraphDependencyRelationKind,
    ProjectGraphRelation, ProjectGraphRelationKind, ProjectGraphSymbolRef, ProjectSemanticIndex,
    ProjectSemanticIndexError, PublicId, QualifiedName, Stmt,
};

pub(super) fn index_entry_relations(
    entry_id: &EntityRef,
    items: &[EntryItem],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for item in items {
        match item {
            EntryItem::Goto(target) => {
                index = index_entity_relation(
                    entry_id,
                    target,
                    ProjectGraphRelationKind::EntryGoto,
                    index,
                )?;
            }
            EntryItem::Route { target, .. } => {
                index = index_entity_relation(
                    entry_id,
                    target,
                    ProjectGraphRelationKind::EntryRoute,
                    index,
                )?;
            }
            EntryItem::Option { .. } | EntryItem::Raw(_) => {}
        }
    }
    Ok(index)
}

pub(super) fn index_content_root_relations(
    content_id: &EntityRef,
    roots: &[EntityRef],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for root in roots {
        index = index_entity_relation(
            content_id,
            root,
            ProjectGraphRelationKind::ContentRoot,
            index,
        )?;
    }
    Ok(index)
}

pub(super) fn index_project_symbol_dependency_relations(
    module: &HirModule,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for flow in module.flows() {
        let Some(id) = flow.id() else {
            continue;
        };
        let parent =
            ProjectGraphSymbolRef::Entity(public_id_for_relation(id, "flow dependency source")?);
        index = index_flow_items_symbol_dependency_relations(&parent, flow.body(), index)?;
    }
    for declaration in module.declarations() {
        if let HirTopLevelDecl::Callable(item) = declaration {
            let parent = ProjectGraphSymbolRef::Callable(QualifiedName::new(item.name()));
            index = index_stmt_body_symbol_dependency_relations(
                &parent,
                item.body_statements(),
                index,
            )?;
            if let Some(value) = item.body_value() {
                index = index_expr_symbol_dependency_relations(&parent, value, index)?;
            }
        }
    }
    Ok(index)
}

fn index_flow_items_symbol_dependency_relations(
    parent: &ProjectGraphSymbolRef,
    items: &[HirFlowItem],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for item in items {
        index = match item {
            HirFlowItem::Dialogue(dialogue) => {
                index_dialogue_symbol_dependency_relations(parent, dialogue, index)?
            }
            HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
                index_choice_symbol_dependency_relations(parent, choice, index)?
            }
            HirFlowItem::If(block) => {
                let index =
                    index_flow_items_symbol_dependency_relations(parent, block.body(), index)?;
                index_flow_items_symbol_dependency_relations(parent, block.else_body(), index)?
            }
            HirFlowItem::IfLet(block) => {
                let index =
                    index_flow_items_symbol_dependency_relations(parent, block.body(), index)?;
                index_flow_items_symbol_dependency_relations(parent, block.else_body(), index)?
            }
            HirFlowItem::Match(block) => {
                let mut next = index;
                for arm in block.arms() {
                    next = index_flow_items_symbol_dependency_relations(parent, arm.body(), next)?;
                }
                next
            }
            HirFlowItem::Loop(block) | HirFlowItem::LetLoop { block, .. } => {
                index_flow_items_symbol_dependency_relations(parent, block.body(), index)?
            }
            HirFlowItem::While(block) => {
                index_flow_items_symbol_dependency_relations(parent, block.body(), index)?
            }
            HirFlowItem::WhileLet(block) => {
                index_flow_items_symbol_dependency_relations(parent, block.body(), index)?
            }
            HirFlowItem::For(block) => {
                index_flow_items_symbol_dependency_relations(parent, block.body(), index)?
            }
            HirFlowItem::SourceLocale(block) => {
                index_flow_items_symbol_dependency_relations(parent, block.body(), index)?
            }
            HirFlowItem::Scope(block) => {
                index_flow_items_symbol_dependency_relations(parent, block.body(), index)?
            }
            HirFlowItem::Select(block) => {
                let mut next = index;
                for branch in block.branches() {
                    next =
                        index_flow_items_symbol_dependency_relations(parent, branch.body(), next)?;
                }
                next
            }
            HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
                let mut next = index;
                for branch in await_with.branches() {
                    next =
                        index_flow_items_symbol_dependency_relations(parent, branch.body(), next)?;
                }
                next
            }
            HirFlowItem::Thread(thread) => {
                index_flow_items_symbol_dependency_relations(parent, thread.body(), index)?
            }
            HirFlowItem::LetScope { scope, .. } => {
                let index =
                    index_stmt_body_symbol_dependency_relations(parent, scope.statements(), index)?;
                if let Some(value) = scope.value() {
                    index_expr_symbol_dependency_relations(parent, value, index)?
                } else {
                    index
                }
            }
            HirFlowItem::Stmt(stmt) => index_stmt_symbol_dependency_relations(parent, stmt, index)?,
            HirFlowItem::Include(_) => index,
        };
    }
    Ok(index)
}

fn index_dialogue_symbol_dependency_relations(
    parent: &ProjectGraphSymbolRef,
    dialogue: &arcweft_lang_hir::model::HirDialogue,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for arg in dialogue.args() {
        index = index_expr_symbol_dependency_relations(parent, arg.value(), index)?;
    }
    if let Some(rich_text) = dialogue.rich_text() {
        index = index_expr_symbol_dependency_relations(parent, rich_text, index)?;
    }
    Ok(index)
}

fn index_choice_symbol_dependency_relations(
    parent: &ProjectGraphSymbolRef,
    choice: &arcweft_lang_hir::model::HirChoice,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for option in choice.options() {
        if let Some(condition) = option.condition() {
            index = index_expr_symbol_dependency_relations(parent, condition, index)?;
        }
        if let Some(value) = option.value() {
            index = index_expr_symbol_dependency_relations(parent, value, index)?;
        }
        index = match option.action() {
            ChoiceAction::Out(expr) => index_expr_symbol_dependency_relations(parent, expr, index)?,
            ChoiceAction::SelectBlock(statements) => {
                index_stmt_body_symbol_dependency_relations(parent, statements, index)?
            }
            ChoiceAction::Goto(_) | ChoiceAction::None => index,
        };
    }
    Ok(index)
}

fn index_stmt_symbol_dependency_relations(
    parent: &ProjectGraphSymbolRef,
    stmt: &Stmt,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match stmt {
        Stmt::Let { expr, .. } | Stmt::Return { expr, .. } | Stmt::Expr { expr, .. } => {
            index = index_expr_symbol_dependency_relations(parent, expr, index)?;
        }
        Stmt::LifetimeSet { expr, .. }
        | Stmt::Out { expr, .. }
        | Stmt::Defer { expr, .. }
        | Stmt::Yield(expr)
        | Stmt::Close(expr)
        | Stmt::Select(expr)
        | Stmt::Goto(expr) => {
            index = index_expr_symbol_dependency_relations(parent, expr.expr(), index)?;
        }
        Stmt::Assign { target, expr }
        | Stmt::Signal {
            target,
            value: expr,
        } => {
            index = index_expr_symbol_dependency_relations(parent, target.expr(), index)?;
            index = index_expr_symbol_dependency_relations(parent, expr.expr(), index)?;
        }
        Stmt::LetElse {
            expr, else_body, ..
        } => {
            index = index_expr_symbol_dependency_relations(parent, expr.expr(), index)?;
            index = index_stmt_body_symbol_dependency_relations(parent, else_body, index)?;
        }
        Stmt::LetActionReceive { action, .. } => {
            index = index_expr_symbol_dependency_relations(parent, action.expr(), index)?;
        }
        Stmt::DeferBlock { statements, .. } => {
            index = index_stmt_body_symbol_dependency_relations(parent, statements, index)?;
        }
        Stmt::On { body, .. } | Stmt::UnsafeLifetime { body, .. } | Stmt::Loop { body } => {
            index = index_stmt_body_symbol_dependency_relations(parent, body, index)?;
        }
        Stmt::If {
            condition,
            body,
            else_body,
        } => {
            index = index_expr_symbol_dependency_relations(parent, condition.expr(), index)?;
            index = index_stmt_body_symbol_dependency_relations(parent, body, index)?;
            index = index_stmt_body_symbol_dependency_relations(parent, else_body, index)?;
        }
        Stmt::While { condition, body } => {
            index = index_expr_symbol_dependency_relations(parent, condition.expr(), index)?;
            index = index_stmt_body_symbol_dependency_relations(parent, body, index)?;
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            index = index_expr_symbol_dependency_relations(parent, expr.expr(), index)?;
            if let Some(guard) = guard {
                index = index_expr_symbol_dependency_relations(parent, guard.expr(), index)?;
            }
            index = index_stmt_body_symbol_dependency_relations(parent, body, index)?;
        }
        Stmt::For { source, body, .. } => {
            index = index_expr_symbol_dependency_relations(parent, source.expr(), index)?;
            index = index_stmt_body_symbol_dependency_relations(parent, body, index)?;
        }
        Stmt::Match { arms, .. } => {
            for arm in arms {
                if let Some(guard) = arm.guard() {
                    index = index_expr_symbol_dependency_relations(parent, guard, index)?;
                }
                index = index_stmt_body_symbol_dependency_relations(parent, arm.body(), index)?;
            }
        }
        Stmt::LetChoice { .. }
        | Stmt::LetScope { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::Thread(_)
        | Stmt::Wait(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => {}
    }
    Ok(index)
}

fn index_stmt_body_symbol_dependency_relations(
    parent: &ProjectGraphSymbolRef,
    statements: &[Stmt],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for stmt in statements {
        index = index_stmt_symbol_dependency_relations(parent, stmt, index)?;
    }
    Ok(index)
}

fn index_expr_symbol_dependency_relations(
    parent: &ProjectGraphSymbolRef,
    expr: &Expr,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match expr {
        Expr::EntityRef(target) => {
            if let (ProjectGraphSymbolRef::Callable(_), Some(target)) =
                (parent, target.as_absolute())
            {
                index = index_symbol_dependency_relation(
                    parent,
                    ProjectGraphSymbolRef::Entity(public_id_for_relation(
                        target,
                        "callable entity reference",
                    )?),
                    ProjectGraphDependencyRelationKind::ReferencesEntity,
                    index,
                );
            }
        }
        Expr::Call { callee, args } => {
            index = index_call_expr_symbol_dependency_relations(parent, callee, args, index)?;
        }
        Expr::Select(select) => {
            index = index_expr_symbol_dependency_relations(parent, select.target(), index)?;
        }
        Expr::Try { expr: target }
        | Expr::Await { expr: target, .. }
        | Expr::Unary { expr: target, .. }
        | Expr::DialogueCall { callee: target, .. }
        | Expr::Closure { body: target, .. } => {
            index = index_expr_symbol_dependency_relations(parent, target, index)?;
        }
        Expr::Index {
            target,
            index: item,
        }
        | Expr::Pipe {
            lhs: target,
            rhs: item,
        }
        | Expr::Binary {
            lhs: target,
            rhs: item,
            ..
        }
        | Expr::ArrayRepeat {
            value: target,
            len: item,
        } => {
            index = index_two_expr_symbol_dependency_relations(parent, target, item, index)?;
        }
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            index = index_expr_list_symbol_dependency_relations(parent, items, index)?;
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            for (_, value) in fields {
                index = index_expr_symbol_dependency_relations(parent, value, index)?;
            }
        }
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => {
            index = index_expr_block_symbol_dependency_relations(
                parent,
                statements,
                value.as_deref(),
                index,
            )?;
        }
        Expr::MemoBlock {
            options,
            statements,
            value,
        } => {
            for (_, value) in options {
                index = index_expr_symbol_dependency_relations(parent, value, index)?;
            }
            index = index_expr_block_symbol_dependency_relations(
                parent,
                statements,
                value.as_deref(),
                index,
            )?;
        }
        Expr::If { .. } | Expr::IfLet { .. } | Expr::Match { .. } | Expr::Range { .. } => {
            index = index_control_expr_symbol_dependency_relations(parent, expr, index)?;
        }
        Expr::Thread { .. }
        | Expr::Literal(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::ShortVariant(_)
        | Expr::Placeholder(_)
        | Expr::NumericBracketSeq(_)
        | Expr::Raw(_) => {}
    }
    Ok(index)
}

fn index_call_expr_symbol_dependency_relations(
    parent: &ProjectGraphSymbolRef,
    callee: &Expr,
    args: &[CallArg],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    if let Some(name) = project_callable_callee(callee, &index) {
        index = index_symbol_dependency_relation(
            parent,
            ProjectGraphSymbolRef::Callable(name),
            ProjectGraphDependencyRelationKind::CallsCallable,
            index,
        );
    }
    index = index_expr_symbol_dependency_relations(parent, callee, index)?;
    index_call_arg_symbol_dependency_relations(parent, args, index)
}

fn project_callable_callee(callee: &Expr, index: &ProjectSemanticIndex) -> Option<QualifiedName> {
    let Expr::Path(path) = callee else {
        return None;
    };
    let name = QualifiedName::new(path.as_label());
    index
        .project_callables()
        .contains_key(&name)
        .then_some(name)
}

fn index_two_expr_symbol_dependency_relations(
    parent: &ProjectGraphSymbolRef,
    first: &Expr,
    second: &Expr,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_symbol_dependency_relations(parent, first, index)?;
    index_expr_symbol_dependency_relations(parent, second, index)
}

fn index_expr_list_symbol_dependency_relations(
    parent: &ProjectGraphSymbolRef,
    items: &[Expr],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for item in items {
        index = index_expr_symbol_dependency_relations(parent, item, index)?;
    }
    Ok(index)
}

fn index_call_arg_symbol_dependency_relations(
    parent: &ProjectGraphSymbolRef,
    args: &[CallArg],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for arg in args {
        let value = match arg {
            CallArg::Positional(value) => value,
            CallArg::Named { value, .. } | CallArg::Spread { value } => value,
        };
        index = index_expr_symbol_dependency_relations(parent, value, index)?;
    }
    Ok(index)
}

fn index_expr_block_symbol_dependency_relations(
    parent: &ProjectGraphSymbolRef,
    statements: &[Stmt],
    value: Option<&Expr>,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_stmt_body_symbol_dependency_relations(parent, statements, index)?;
    if let Some(value) = value {
        index = index_expr_symbol_dependency_relations(parent, value, index)?;
    }
    Ok(index)
}

fn index_control_expr_symbol_dependency_relations(
    parent: &ProjectGraphSymbolRef,
    expr: &Expr,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match expr {
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            index = index_expr_symbol_dependency_relations(parent, condition, index)?;
            index = index_expr_symbol_dependency_relations(parent, then_branch, index)?;
            if let Some(else_branch) = else_branch {
                index = index_expr_symbol_dependency_relations(parent, else_branch, index)?;
            }
        }
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => {
            index = index_expr_symbol_dependency_relations(parent, expr, index)?;
            if let Some(guard) = guard {
                index = index_expr_symbol_dependency_relations(parent, guard, index)?;
            }
            index = index_expr_symbol_dependency_relations(parent, then_branch, index)?;
            if let Some(else_branch) = else_branch {
                index = index_expr_symbol_dependency_relations(parent, else_branch, index)?;
            }
        }
        Expr::Match { scrutinee, arms } => {
            index = index_expr_symbol_dependency_relations(parent, scrutinee, index)?;
            for arm in arms {
                if let Some(guard) = arm.guard() {
                    index = index_expr_symbol_dependency_relations(parent, guard, index)?;
                }
                index = index_expr_symbol_dependency_relations(parent, arm.value(), index)?;
            }
        }
        Expr::Range { start, end, .. } => {
            if let Some(start) = start {
                index = index_expr_symbol_dependency_relations(parent, start, index)?;
            }
            if let Some(end) = end {
                index = index_expr_symbol_dependency_relations(parent, end, index)?;
            }
        }
        _ => {}
    }
    Ok(index)
}

fn index_symbol_dependency_relation(
    from: &ProjectGraphSymbolRef,
    to: ProjectGraphSymbolRef,
    edge_kind: ProjectGraphDependencyRelationKind,
    index: ProjectSemanticIndex,
) -> ProjectSemanticIndex {
    index.with_dependency_relation(ProjectGraphDependencyRelation::new(
        from.clone(),
        to,
        edge_kind,
    ))
}

pub(super) fn index_flow_item_relations(
    parent: Option<&EntityRef>,
    items: &[HirFlowItem],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for item in items {
        match item {
            HirFlowItem::Dialogue(dialogue) => {
                if let (Some(parent), Some(id)) = (parent, dialogue.id()) {
                    index = index_entity_relation(
                        parent,
                        id,
                        ProjectGraphRelationKind::ContainsDialogue,
                        index,
                    )?;
                }
                index = index_dialogue_dependency_relations(parent, dialogue, index)?;
            }
            HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
                if let Some(choice_id) = choice.id() {
                    if let Some(parent) = parent {
                        index = index_entity_relation(
                            parent,
                            choice_id,
                            ProjectGraphRelationKind::ContainsChoice,
                            index,
                        )?;
                    }
                    index = index_choice_relations(choice_id, choice.options(), index)?;
                }
                index = index_choice_dependency_relations(parent, choice, index)?;
            }
            HirFlowItem::If(block) => {
                index = index_flow_item_relations(parent, block.body(), index)?;
                index = index_flow_item_relations(parent, block.else_body(), index)?;
            }
            HirFlowItem::IfLet(block) => {
                index = index_flow_item_relations(parent, block.body(), index)?;
                index = index_flow_item_relations(parent, block.else_body(), index)?;
            }
            HirFlowItem::Match(block) => {
                for arm in block.arms() {
                    index = index_flow_item_relations(parent, arm.body(), index)?;
                }
            }
            HirFlowItem::Loop(block) | HirFlowItem::LetLoop { block, .. } => {
                index = index_flow_item_relations(parent, block.body(), index)?;
            }
            HirFlowItem::While(block) => {
                index = index_flow_item_relations(parent, block.body(), index)?;
            }
            HirFlowItem::WhileLet(block) => {
                index = index_flow_item_relations(parent, block.body(), index)?;
            }
            HirFlowItem::For(block) => {
                index = index_flow_item_relations(parent, block.body(), index)?;
            }
            HirFlowItem::SourceLocale(block) => {
                index = index_flow_item_relations(parent, block.body(), index)?;
            }
            HirFlowItem::Scope(block) => {
                index = index_flow_item_relations(parent, block.body(), index)?;
            }
            HirFlowItem::Select(block) => {
                for branch in block.branches() {
                    index = index_flow_item_relations(parent, branch.body(), index)?;
                }
            }
            HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
                for branch in await_with.branches() {
                    index = index_flow_item_relations(parent, branch.body(), index)?;
                }
            }
            HirFlowItem::Thread(thread) => {
                index = index_flow_item_relations(parent, thread.body(), index)?;
            }
            HirFlowItem::Stmt(stmt) => {
                index = index_stmt_relations(parent, stmt, index)?;
            }
            HirFlowItem::Include(target) => {
                if let Some(parent) = parent {
                    index = index_entity_relation(
                        parent,
                        target,
                        ProjectGraphRelationKind::FlowInclude,
                        index,
                    )?;
                }
            }
            HirFlowItem::LetScope { scope, .. } => {
                index = index_stmt_body_relations(parent, scope.statements(), index)?;
                if let Some(value) = scope.value() {
                    index = index_expr_dependency_relations(parent, value, index)?;
                }
            }
        }
    }
    Ok(index)
}

fn index_dialogue_dependency_relations(
    parent: Option<&EntityRef>,
    dialogue: &arcweft_lang_hir::model::HirDialogue,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for arg in dialogue.args() {
        index = index_expr_dependency_relations(parent, arg.value(), index)?;
    }
    if let Some(rich_text) = dialogue.rich_text() {
        index = index_expr_dependency_relations(parent, rich_text, index)?;
    }
    Ok(index)
}

fn index_choice_dependency_relations(
    parent: Option<&EntityRef>,
    choice: &arcweft_lang_hir::model::HirChoice,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for option in choice.options() {
        if let Some(condition) = option.condition() {
            index = index_expr_dependency_relations(parent, condition, index)?;
        }
        if let Some(value) = option.value() {
            index = index_expr_dependency_relations(parent, value, index)?;
        }
        match option.action() {
            ChoiceAction::Out(expr) => {
                index = index_expr_dependency_relations(parent, expr, index)?;
            }
            ChoiceAction::SelectBlock(statements) => {
                index = index_stmt_body_relations(parent, statements, index)?;
            }
            ChoiceAction::Goto(_) | ChoiceAction::None => {}
        }
    }
    Ok(index)
}

fn index_stmt_relations(
    parent: Option<&EntityRef>,
    stmt: &Stmt,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match stmt {
        Stmt::Goto(expr) => {
            if let Expr::EntityRef(target) = expr.expr()
                && let (Some(parent), Some(target)) = (parent, target.as_absolute())
            {
                index = index_entity_relation(
                    parent,
                    target,
                    ProjectGraphRelationKind::FlowGoto,
                    index,
                )?;
            }
            index = index_expr_dependency_relations(parent, expr.expr(), index)?;
        }
        Stmt::Let { expr, .. } | Stmt::Return { expr, .. } | Stmt::Expr { expr, .. } => {
            index = index_expr_dependency_relations(parent, expr, index)?;
        }
        Stmt::LifetimeSet { expr, .. }
        | Stmt::Out { expr, .. }
        | Stmt::Defer { expr, .. }
        | Stmt::Yield(expr)
        | Stmt::Close(expr)
        | Stmt::Select(expr) => {
            index = index_expr_dependency_relations(parent, expr.expr(), index)?;
        }
        Stmt::Assign { target, expr }
        | Stmt::Signal {
            target,
            value: expr,
        } => {
            index = index_expr_dependency_relations(parent, target.expr(), index)?;
            index = index_expr_dependency_relations(parent, expr.expr(), index)?;
        }
        Stmt::LetElse {
            expr, else_body, ..
        } => {
            index = index_expr_dependency_relations(parent, expr.expr(), index)?;
            index = index_stmt_body_relations(parent, else_body, index)?;
        }
        Stmt::LetActionReceive { action, .. } => {
            index = index_expr_dependency_relations(parent, action.expr(), index)?;
        }
        Stmt::DeferBlock { statements, .. } => {
            index = index_stmt_body_relations(parent, statements, index)?;
        }
        Stmt::On { body, .. } | Stmt::UnsafeLifetime { body, .. } | Stmt::Loop { body } => {
            index = index_stmt_body_relations(parent, body, index)?;
        }
        Stmt::If {
            condition,
            body,
            else_body,
        } => {
            index = index_expr_dependency_relations(parent, condition.expr(), index)?;
            index = index_stmt_body_relations(parent, body, index)?;
            index = index_stmt_body_relations(parent, else_body, index)?;
        }
        Stmt::While { condition, body } => {
            index = index_expr_dependency_relations(parent, condition.expr(), index)?;
            index = index_stmt_body_relations(parent, body, index)?;
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            index = index_expr_dependency_relations(parent, expr.expr(), index)?;
            if let Some(guard) = guard {
                index = index_expr_dependency_relations(parent, guard.expr(), index)?;
            }
            index = index_stmt_body_relations(parent, body, index)?;
        }
        Stmt::For { source, body, .. } => {
            index = index_expr_dependency_relations(parent, source.expr(), index)?;
            index = index_stmt_body_relations(parent, body, index)?;
        }
        Stmt::Match { arms, .. } => {
            for arm in arms {
                if let Some(guard) = arm.guard() {
                    index = index_expr_dependency_relations(parent, guard, index)?;
                }
                index = index_stmt_body_relations(parent, arm.body(), index)?;
            }
        }
        Stmt::LetChoice { .. }
        | Stmt::LetScope { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::Thread(_)
        | Stmt::Wait(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => {}
    }
    Ok(index)
}

fn index_stmt_body_relations(
    parent: Option<&EntityRef>,
    statements: &[Stmt],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for stmt in statements {
        index = index_stmt_relations(parent, stmt, index)?;
    }
    Ok(index)
}

fn index_expr_dependency_relations(
    parent: Option<&EntityRef>,
    expr: &Expr,
    index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match expr {
        Expr::EntityRef(target) => {
            let mut index = index;
            if let (Some(parent), Some(target)) = (parent, target.as_absolute()) {
                index = index_entity_relation(
                    parent,
                    target,
                    ProjectGraphRelationKind::ReferencesEntity,
                    index,
                )?;
            }
            Ok(index)
        }
        Expr::Thread { .. }
        | Expr::Literal(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::ShortVariant(_)
        | Expr::Placeholder(_)
        | Expr::NumericBracketSeq(_)
        | Expr::Raw(_) => Ok(index),
        _ => index_compound_expr_dependency_relations(parent, expr, index),
    }
}

fn index_compound_expr_dependency_relations(
    parent: Option<&EntityRef>,
    expr: &Expr,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match expr {
        Expr::EntityRef(_)
        | Expr::Thread { .. }
        | Expr::Literal(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::ShortVariant(_)
        | Expr::Placeholder(_)
        | Expr::NumericBracketSeq(_)
        | Expr::Raw(_) => {}
        Expr::Call { callee, args } => {
            index = index_call_expr_dependency_relations(parent, callee, args, index)?;
        }
        Expr::Select(select) => {
            index = index_expr_dependency_relations(parent, select.target(), index)?;
        }
        Expr::Try { expr: target }
        | Expr::Await { expr: target, .. }
        | Expr::Unary { expr: target, .. } => {
            index = index_expr_dependency_relations(parent, target, index)?;
        }
        Expr::DialogueCall { callee, .. } | Expr::Closure { body: callee, .. } => {
            index = index_expr_dependency_relations(parent, callee, index)?;
        }
        Expr::Index {
            target,
            index: item,
        }
        | Expr::Pipe {
            lhs: target,
            rhs: item,
        }
        | Expr::Binary {
            lhs: target,
            rhs: item,
            ..
        }
        | Expr::ArrayRepeat {
            value: target,
            len: item,
        } => {
            index = index_two_expr_dependency_relations(parent, target, item, index)?;
        }
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            index = index_expr_list_dependency_relations(parent, items, index)?;
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            index = index_record_expr_dependency_relations(parent, fields, index)?;
        }
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => {
            index =
                index_expr_block_dependency_relations(parent, statements, value.as_deref(), index)?;
        }
        Expr::MemoBlock {
            options,
            statements,
            value,
        } => {
            index = index_memo_expr_dependency_relations(
                parent,
                options,
                statements,
                value.as_deref(),
                index,
            )?;
        }
        Expr::If { .. } | Expr::IfLet { .. } | Expr::Match { .. } | Expr::Range { .. } => {
            index = index_control_expr_dependency_relations(parent, expr, index)?;
        }
    }
    Ok(index)
}

fn index_control_expr_dependency_relations(
    parent: Option<&EntityRef>,
    expr: &Expr,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match expr {
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            index = index_if_expr_dependency_relations(
                parent,
                condition,
                then_branch,
                else_branch.as_deref(),
                index,
            )?;
        }
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => {
            index = index_if_let_expr_dependency_relations(
                parent,
                expr,
                guard.as_deref(),
                then_branch,
                else_branch.as_deref(),
                index,
            )?;
        }
        Expr::Match { scrutinee, arms } => {
            index = index_match_expr_dependency_relations(parent, scrutinee, arms, index)?;
        }
        Expr::Range { start, end, .. } => {
            index = index_range_expr_dependency_relations(
                parent,
                start.as_deref(),
                end.as_deref(),
                index,
            )?;
        }
        _ => {}
    }
    Ok(index)
}

fn index_call_expr_dependency_relations(
    parent: Option<&EntityRef>,
    callee: &Expr,
    args: &[CallArg],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_dependency_relations(parent, callee, index)?;
    index_call_arg_dependency_relations(parent, args, index)
}

fn index_two_expr_dependency_relations(
    parent: Option<&EntityRef>,
    first: &Expr,
    second: &Expr,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_dependency_relations(parent, first, index)?;
    index_expr_dependency_relations(parent, second, index)
}

fn index_record_expr_dependency_relations(
    parent: Option<&EntityRef>,
    fields: &[(String, Expr)],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for (_, value) in fields {
        index = index_expr_dependency_relations(parent, value, index)?;
    }
    Ok(index)
}

fn index_expr_block_dependency_relations(
    parent: Option<&EntityRef>,
    statements: &[Stmt],
    value: Option<&Expr>,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_stmt_body_relations(parent, statements, index)?;
    if let Some(value) = value {
        index = index_expr_dependency_relations(parent, value, index)?;
    }
    Ok(index)
}

fn index_memo_expr_dependency_relations(
    parent: Option<&EntityRef>,
    options: &[(String, Expr)],
    statements: &[Stmt],
    value: Option<&Expr>,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for (_, value) in options {
        index = index_expr_dependency_relations(parent, value, index)?;
    }
    index_expr_block_dependency_relations(parent, statements, value, index)
}

fn index_if_expr_dependency_relations(
    parent: Option<&EntityRef>,
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_dependency_relations(parent, condition, index)?;
    index = index_expr_dependency_relations(parent, then_branch, index)?;
    if let Some(else_branch) = else_branch {
        index = index_expr_dependency_relations(parent, else_branch, index)?;
    }
    Ok(index)
}

fn index_if_let_expr_dependency_relations(
    parent: Option<&EntityRef>,
    expr: &Expr,
    guard: Option<&Expr>,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_dependency_relations(parent, expr, index)?;
    if let Some(guard) = guard {
        index = index_expr_dependency_relations(parent, guard, index)?;
    }
    index = index_expr_dependency_relations(parent, then_branch, index)?;
    if let Some(else_branch) = else_branch {
        index = index_expr_dependency_relations(parent, else_branch, index)?;
    }
    Ok(index)
}

fn index_match_expr_dependency_relations(
    parent: Option<&EntityRef>,
    scrutinee: &Expr,
    arms: &[MatchExprArm],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_dependency_relations(parent, scrutinee, index)?;
    for arm in arms {
        if let Some(guard) = arm.guard() {
            index = index_expr_dependency_relations(parent, guard, index)?;
        }
        index = index_expr_dependency_relations(parent, arm.value(), index)?;
    }
    Ok(index)
}

fn index_range_expr_dependency_relations(
    parent: Option<&EntityRef>,
    start: Option<&Expr>,
    end: Option<&Expr>,
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    if let Some(start) = start {
        index = index_expr_dependency_relations(parent, start, index)?;
    }
    if let Some(end) = end {
        index = index_expr_dependency_relations(parent, end, index)?;
    }
    Ok(index)
}

fn index_call_arg_dependency_relations(
    parent: Option<&EntityRef>,
    args: &[CallArg],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for arg in args {
        index = index_expr_dependency_relations(parent, arg.value(), index)?;
    }
    Ok(index)
}

fn index_expr_list_dependency_relations(
    parent: Option<&EntityRef>,
    items: &[Expr],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for item in items {
        index = index_expr_dependency_relations(parent, item, index)?;
    }
    Ok(index)
}

fn index_choice_relations(
    choice_id: &EntityRef,
    options: &[arcweft_lang_hir::model::HirChoiceOption],
    mut index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for option in options {
        if let Some(option_id) = option.id() {
            index = index_entity_relation(
                choice_id,
                option_id,
                ProjectGraphRelationKind::ContainsChoiceOption,
                index,
            )?;
            if let Some(target) = option.target() {
                index = index_entity_relation(
                    option_id,
                    target,
                    ProjectGraphRelationKind::ChoiceOptionGoto,
                    index,
                )?;
            }
        }
    }
    Ok(index)
}

fn index_entity_relation(
    from: &EntityRef,
    to: &EntityRef,
    edge_kind: ProjectGraphRelationKind,
    index: ProjectSemanticIndex,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    let from = public_id_for_relation(from, edge_kind.as_str())?;
    let to = public_id_for_relation(to, edge_kind.as_str())?;
    Ok(index.with_relation(ProjectGraphRelation::new(from, to, edge_kind)))
}

pub(super) fn public_id_for_relation(
    id: &EntityRef,
    kind: &'static str,
) -> Result<PublicId, ProjectSemanticIndexError> {
    PublicId::try_new(id.body().to_owned()).map_err(|error| {
        ProjectSemanticIndexError::InvalidPublicId {
            id: id.body().to_owned(),
            kind,
            message: error.to_string(),
        }
    })
}
