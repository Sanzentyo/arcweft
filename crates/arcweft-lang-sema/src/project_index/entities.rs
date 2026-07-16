use super::{
    AgentActionSignature, CallArg, CallableDeclarationId, EntityDeclKind, EntityKind, EntityRef,
    EntitySymbol, EntityType, Expr, FunctionParam, FunctionSignature, HirFlowItem, Literal,
    MatchExprArm, Pattern, ProjectCallableSymbol, ProjectSemanticIndex, ProjectSemanticIndexError,
    PublicId, QualifiedName, SemanticHash, SourceAnchor, SourceName, Stmt, SyntaxFnParam,
    SyntaxFnSignature, TypeKind, TypeRef, parse_type_ref, type_ref_kind,
};
use arcweft_lang_hir::model::HirFunction;
use arcweft_lang_syntax::{ast::items::CallableItem, types::parse_fn_signature};

pub(super) fn index_flow_items(
    items: &[HirFlowItem],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for item in items {
        index = index_flow_item_entities(item, index, source_name)?;
        index = index_flow_item_agent_actions(item, index, source_name)?;
    }
    Ok(index)
}

fn index_flow_item_entities(
    item: &HirFlowItem,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match item {
        HirFlowItem::Dialogue(dialogue) => {
            if let Some(id) = dialogue.id() {
                index = index.with_entity(entity_symbol(
                    id,
                    EntityKind::DialogueLine,
                    None,
                    source_name,
                    "dialogue line",
                )?);
            }
            if let Some(text_key) = dialogue.text_key() {
                index = index.with_entity(entity_symbol(
                    text_key,
                    EntityKind::Text,
                    None,
                    source_name,
                    "text",
                )?);
            }
        }
        HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
            if let Some(id) = choice.id() {
                index = index.with_entity(entity_symbol(
                    id,
                    EntityKind::Choice,
                    None,
                    source_name,
                    "choice",
                )?);
            }
            for option in choice.options() {
                if let Some(id) = option.id() {
                    index = index.with_entity(entity_symbol(
                        id,
                        EntityKind::ChoiceOption,
                        None,
                        source_name,
                        "choice option",
                    )?);
                }
            }
        }
        HirFlowItem::If(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
            index = index_flow_items(block.else_body(), index, source_name)?;
        }
        HirFlowItem::IfLet(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
            index = index_flow_items(block.else_body(), index, source_name)?;
        }
        HirFlowItem::Match(block) => {
            for arm in block.arms() {
                index = index_flow_items(arm.body(), index, source_name)?;
            }
        }
        HirFlowItem::Loop(block) | HirFlowItem::LetLoop { block, .. } => {
            index = index_flow_items(block.body(), index, source_name)?;
        }
        HirFlowItem::While(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
        }
        HirFlowItem::WhileLet(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
        }
        HirFlowItem::For(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
        }
        HirFlowItem::SourceLocale(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
        }
        HirFlowItem::Scope(block) => {
            index = index_flow_items(block.body(), index, source_name)?;
        }
        HirFlowItem::Select(block) => {
            for branch in block.branches() {
                index = index_flow_items(branch.body(), index, source_name)?;
            }
        }
        HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
            for branch in await_with.branches() {
                index = index_flow_items(branch.body(), index, source_name)?;
            }
        }
        HirFlowItem::Thread(thread) => {
            index = index_flow_items(thread.body(), index, source_name)?;
        }
        HirFlowItem::Stmt(_) | HirFlowItem::LetScope { .. } | HirFlowItem::Include(_) => {}
    }
    Ok(index)
}

fn index_flow_item_agent_actions(
    item: &HirFlowItem,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match item {
        HirFlowItem::Stmt(stmt) => index = index_stmt_agent_actions(stmt, index, source_name)?,
        HirFlowItem::LetScope { scope, .. } => {
            for stmt in scope.statements() {
                index = index_stmt_agent_actions(stmt, index, source_name)?;
            }
            if let Some(value) = scope.value() {
                index = index_expr_agent_actions(value, index, source_name)?;
            }
        }
        _ => {}
    }
    Ok(index)
}

fn index_stmt_agent_actions(
    stmt: &Stmt,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match stmt {
        Stmt::Assertion(assertion) => {
            for condition in assertion.conditions() {
                index = index_expr_agent_actions(condition, index, source_name)?;
            }
        }
        Stmt::Let { expr, .. } | Stmt::Return { expr, .. } | Stmt::Expr { expr, .. } => {
            index = index_expr_agent_actions(expr, index, source_name)?;
        }
        Stmt::LifetimeSet { expr, .. }
        | Stmt::Out { expr, .. }
        | Stmt::Defer { expr, .. }
        | Stmt::Goto(expr)
        | Stmt::Yield(expr)
        | Stmt::Close(expr)
        | Stmt::Select(expr) => {
            index = index_expr_agent_actions(expr.expr(), index, source_name)?;
        }
        Stmt::Assign { target, expr }
        | Stmt::Signal {
            target,
            value: expr,
        } => {
            index = index_expr_agent_actions(target.expr(), index, source_name)?;
            index = index_expr_agent_actions(expr.expr(), index, source_name)?;
        }
        Stmt::LetElse {
            expr, else_body, ..
        } => {
            index = index_expr_agent_actions(expr.expr(), index, source_name)?;
            index = index_stmt_body_agent_actions(else_body, index, source_name)?;
        }
        Stmt::LetActionReceive { action, .. } => {
            index = index_expr_agent_actions(action.expr(), index, source_name)?;
        }
        Stmt::DeferBlock { statements, .. } => {
            index = index_stmt_body_agent_actions(statements, index, source_name)?;
        }
        Stmt::On { body, .. } | Stmt::UnsafeLifetime { body, .. } | Stmt::Loop { body } => {
            index = index_stmt_body_agent_actions(body, index, source_name)?;
        }
        Stmt::If {
            condition,
            body,
            else_body,
        } => {
            index = index_expr_agent_actions(condition.expr(), index, source_name)?;
            index = index_stmt_body_agent_actions(body, index, source_name)?;
            index = index_stmt_body_agent_actions(else_body, index, source_name)?;
        }
        Stmt::While { condition, body } => {
            index = index_expr_agent_actions(condition.expr(), index, source_name)?;
            index = index_stmt_body_agent_actions(body, index, source_name)?;
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            index = index_expr_agent_actions(expr.expr(), index, source_name)?;
            if let Some(guard) = guard {
                index = index_expr_agent_actions(guard.expr(), index, source_name)?;
            }
            index = index_stmt_body_agent_actions(body, index, source_name)?;
        }
        Stmt::For { source, body, .. } => {
            index = index_expr_agent_actions(source.expr(), index, source_name)?;
            index = index_stmt_body_agent_actions(body, index, source_name)?;
        }
        Stmt::Match { expr, arms } => {
            index = index_expr_agent_actions(expr.expr(), index, source_name)?;
            for arm in arms {
                if let Some(guard) = arm.guard() {
                    index = index_expr_agent_actions(guard, index, source_name)?;
                }
                index = index_stmt_body_agent_actions(arm.body(), index, source_name)?;
            }
        }
        Stmt::LetScope { scope, .. } => {
            index = index_stmt_body_agent_actions(scope.statements(), index, source_name)?;
            if let Some(value) = scope.value() {
                index = index_expr_agent_actions(value, index, source_name)?;
            }
        }
        Stmt::Wait(_)
        | Stmt::LetChoice { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::Thread(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => {}
    }
    Ok(index)
}

fn index_stmt_body_agent_actions(
    body: &[Stmt],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for stmt in body {
        index = index_stmt_agent_actions(stmt, index, source_name)?;
    }
    Ok(index)
}

fn index_expr_agent_actions(
    expr: &Expr,
    index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    match expr {
        Expr::Call { callee, args } => {
            index_call_expr_agent_actions(callee, args, index, source_name)
        }
        Expr::Select(select) => index_expr_agent_actions(select.target(), index, source_name),
        Expr::Try { expr: target }
        | Expr::Await { expr: target, .. }
        | Expr::Unary { expr: target, .. } => index_expr_agent_actions(target, index, source_name),
        Expr::Borrow(borrow) => index_expr_agent_actions(borrow.operand(), index, source_name),
        Expr::Deref(deref) => index_expr_agent_actions(deref.operand(), index, source_name),
        Expr::DialogueCall { callee, .. } | Expr::Closure { body: callee, .. } => {
            index_expr_agent_actions(callee, index, source_name)
        }
        Expr::Index {
            target,
            index: item,
        } => index_two_expr_agent_actions(target, item, index, source_name),
        Expr::Pipe { lhs, rhs } | Expr::Binary { lhs, rhs, .. } => {
            index_two_expr_agent_actions(lhs, rhs, index, source_name)
        }
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            index_expr_list_agent_actions(items, index, source_name)
        }
        Expr::ArrayRepeat { value, len } => {
            index_two_expr_agent_actions(value, len, index, source_name)
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            index_record_expr_agent_actions(fields, index, source_name)
        }
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => index_expr_block_agent_actions(statements, value.as_deref(), index, source_name),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => index_if_expr_agent_actions(
            condition,
            then_branch,
            else_branch.as_deref(),
            index,
            source_name,
        ),
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => index_if_let_expr_agent_actions(
            expr,
            guard.as_deref(),
            then_branch,
            else_branch.as_deref(),
            index,
            source_name,
        ),
        Expr::Match { scrutinee, arms } => {
            index_match_expr_agent_actions(scrutinee, arms, index, source_name)
        }
        Expr::Range {
            start,
            end,
            inclusive: _,
        } => index_range_expr_agent_actions(start.as_deref(), end.as_deref(), index, source_name),
        Expr::Thread { .. }
        | Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::ShortVariant(_)
        | Expr::Placeholder(_)
        | Expr::NumericBracketSeq(_)
        | Expr::Raw(_) => Ok(index),
    }
}

fn index_two_expr_agent_actions(
    first: &Expr,
    second: &Expr,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_agent_actions(first, index, source_name)?;
    index_expr_agent_actions(second, index, source_name)
}

fn index_expr_list_agent_actions(
    items: &[Expr],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for item in items {
        index = index_expr_agent_actions(item, index, source_name)?;
    }
    Ok(index)
}

fn index_record_expr_agent_actions(
    fields: &[(String, Expr)],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for (_, value) in fields {
        index = index_expr_agent_actions(value, index, source_name)?;
    }
    Ok(index)
}

fn index_range_expr_agent_actions(
    start: Option<&Expr>,
    end: Option<&Expr>,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    if let Some(start) = start {
        index = index_expr_agent_actions(start, index, source_name)?;
    }
    if let Some(end) = end {
        index = index_expr_agent_actions(end, index, source_name)?;
    }
    Ok(index)
}

fn index_call_expr_agent_actions(
    callee: &Expr,
    args: &[CallArg],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    if matches!(callee, Expr::Path(path) if path == "image") {
        index = index_image_call_agent_actions(args, index, source_name)?;
    }
    index = index_expr_agent_actions(callee, index, source_name)?;
    index_call_arg_agent_actions(args, index, source_name)
}

fn index_expr_block_agent_actions(
    statements: &[Stmt],
    value: Option<&Expr>,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for stmt in statements {
        index = index_stmt_agent_actions(stmt, index, source_name)?;
    }
    if let Some(value) = value {
        index = index_expr_agent_actions(value, index, source_name)?;
    }
    Ok(index)
}

fn index_if_expr_agent_actions(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_agent_actions(condition, index, source_name)?;
    index = index_expr_agent_actions(then_branch, index, source_name)?;
    if let Some(else_branch) = else_branch {
        index = index_expr_agent_actions(else_branch, index, source_name)?;
    }
    Ok(index)
}

fn index_if_let_expr_agent_actions(
    expr: &Expr,
    guard: Option<&Expr>,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_agent_actions(expr, index, source_name)?;
    if let Some(guard) = guard {
        index = index_expr_agent_actions(guard, index, source_name)?;
    }
    index = index_expr_agent_actions(then_branch, index, source_name)?;
    if let Some(else_branch) = else_branch {
        index = index_expr_agent_actions(else_branch, index, source_name)?;
    }
    Ok(index)
}

fn index_match_expr_agent_actions(
    scrutinee: &Expr,
    arms: &[MatchExprArm],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    index = index_expr_agent_actions(scrutinee, index, source_name)?;
    for arm in arms {
        if let Some(guard) = arm.guard() {
            index = index_expr_agent_actions(guard, index, source_name)?;
        }
        index = index_expr_agent_actions(arm.value(), index, source_name)?;
    }
    Ok(index)
}

fn index_call_arg_agent_actions(
    args: &[CallArg],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    for arg in args {
        match arg {
            CallArg::Positional(value) => {
                index = index_expr_agent_actions(value, index, source_name)?;
            }
            CallArg::Named { value, .. } | CallArg::Spread { value } => {
                index = index_expr_agent_actions(value.as_ref(), index, source_name)?;
            }
        }
    }
    Ok(index)
}

fn index_image_call_agent_actions(
    args: &[CallArg],
    mut index: ProjectSemanticIndex,
    source_name: &SourceName,
) -> Result<ProjectSemanticIndex, ProjectSemanticIndexError> {
    let Some(target) = image_call_target(args) else {
        return Ok(index);
    };
    let actions = image_call_actions(args)?;
    if actions.is_empty() {
        return Ok(index);
    }

    let target_id = PublicId::try_new(target.clone()).map_err(|error| {
        ProjectSemanticIndexError::InvalidPublicId {
            id: target.clone(),
            kind: "image action target",
            message: error.to_string(),
        }
    })?;
    let symbol = index.entities.entry(target_id.clone()).or_insert_with(|| {
        EntitySymbol::new(
            target_id,
            EntityType::new(EntityKind::Target, None),
            SourceAnchor::from_span(
                source_name
                    .span(arcweft_source::SourceRange::new(0, 0))
                    .expect("the start of a source document is a valid synthetic site"),
            ),
            SemanticHash::new(format!("hir:image-target:{target}")),
        )
    });
    for action in actions {
        if symbol
            .agent_actions
            .iter()
            .any(|candidate| candidate.action().as_str() == action)
        {
            continue;
        }
        symbol.agent_actions.push(AgentActionSignature::new(
            QualifiedName::new(action),
            [],
            TypeKind::ActionResult,
        ));
    }
    Ok(index)
}

fn image_call_target(args: &[CallArg]) -> Option<String> {
    for arg in args {
        if let CallArg::Named { name, value } = arg
            && name == "target"
            && let Some(value) = image_call_id_value(value)
        {
            return Some(value);
        }
    }
    None
}

fn image_call_actions(args: &[CallArg]) -> Result<Vec<String>, ProjectSemanticIndexError> {
    args.iter()
        .filter_map(|arg| match arg {
            CallArg::Named { name, value } if name == "action" || name == "actions" => {
                Some(image_call_action_values(value))
            }
            _ => None,
        })
        .flatten()
        .collect()
}

fn image_call_action_values(value: &Expr) -> Vec<Result<String, ProjectSemanticIndexError>> {
    match value {
        Expr::Literal(Literal::String(value)) => value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| Ok(value.to_owned()))
            .collect(),
        Expr::Path(path) => vec![Ok(path.as_label().to_owned())],
        Expr::ShortVariant(name) => vec![Ok(name.to_string())],
        Expr::EntityRef(entity) => vec![Ok(entity.body().to_owned())],
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            items.iter().flat_map(image_call_action_values).collect()
        }
        _ => Vec::new(),
    }
}

fn image_call_id_value(value: &Expr) -> Option<String> {
    match value {
        Expr::Literal(Literal::String(value)) => Some(value.clone()),
        Expr::EntityRef(entity) => Some(entity.body().to_owned()),
        Expr::Path(path) => Some(path.as_label().to_owned()),
        Expr::ShortVariant(name) => Some(name.to_string()),
        _ => None,
    }
}

pub(super) fn entity_symbol(
    id: &EntityRef,
    kind: EntityKind,
    value: Option<TypeKind>,
    source_name: &SourceName,
    kind_label: &'static str,
) -> Result<EntitySymbol, ProjectSemanticIndexError> {
    let public_id = PublicId::try_new(id.body()).map_err(|error| {
        ProjectSemanticIndexError::InvalidPublicId {
            id: id.body().to_owned(),
            kind: kind_label,
            message: error.to_string(),
        }
    })?;
    let source = SourceAnchor::from_span(
        source_name
            .span(arcweft_source::SourceRange::new(
                id.range().start(),
                id.range().end(),
            ))
            .expect("an entity range belongs to the source document that was lowered"),
    );
    let semantic_hash = SemanticHash::new(format!(
        "hir:{kind_label}:{}:{}",
        id.body(),
        value
            .as_ref()
            .map_or_else(|| "_".to_owned(), type_kind_stable_label)
    ));
    Ok(EntitySymbol::new(
        public_id,
        EntityType::new(kind, value),
        source,
        semantic_hash,
    ))
}

pub(super) fn project_function_symbol(
    declaration: CallableDeclarationId,
    function: &HirFunction,
    source_name: &SourceName,
) -> ProjectCallableSymbol {
    let signature = function_signature_from_syntax(function.signature());
    let source = SourceAnchor::from_span(
        source_name
            .span(arcweft_source::SourceRange::new(
                function.range().start(),
                function.range().end(),
            ))
            .expect("a function range belongs to the project source document that was lowered"),
    );
    let semantic_hash =
        SemanticHash::new(project_function_semantic_label(&declaration, &signature));
    ProjectCallableSymbol::function(declaration, signature, source, semantic_hash)
}

pub(super) fn project_view_callable_symbol(
    declaration: CallableDeclarationId,
    item: &CallableItem,
    source_name: &SourceName,
) -> Result<ProjectCallableSymbol, ProjectSemanticIndexError> {
    let signature_source = format!("fn {}{}", item.name(), item.signature_tail());
    let signature = parse_fn_signature(&signature_source).map_err(|error| {
        ProjectSemanticIndexError::InvalidCallableSignature {
            name: item.name().to_owned(),
            message: error.to_string(),
        }
    })?;
    let signature = function_signature_from_syntax(&signature);
    let source = SourceAnchor::from_span(
        source_name
            .span(arcweft_source::SourceRange::new(
                item.range().start(),
                item.range().end(),
            ))
            .expect("a View callable range belongs to the source document that was lowered"),
    );
    let semantic_hash = SemanticHash::new(format!(
        "hir:callable:view:{}:{}",
        declaration.qualified_name(),
        item.signature_tail().trim()
    ));
    Ok(ProjectCallableSymbol::view(
        declaration,
        signature,
        source,
        semantic_hash,
    ))
}

fn project_function_semantic_label(
    declaration: &CallableDeclarationId,
    signature: &FunctionSignature,
) -> String {
    let parameters = signature
        .params()
        .iter()
        .map(|parameter| type_kind_stable_label(parameter.ty()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "hir:callable:function:{}:{}:({parameters})->{}",
        declaration.package().as_str(),
        declaration.qualified_name(),
        type_kind_stable_label(signature.return_type())
    )
}

fn function_signature_from_syntax(signature: &SyntaxFnSignature) -> FunctionSignature {
    let return_type = curried_project_signature_return_type(signature);
    let params = signature
        .param_groups()
        .first()
        .into_iter()
        .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
        .map(project_function_param);
    let remaining_param_groups = signature
        .param_groups()
        .iter()
        .skip(1)
        .map(|group| {
            group
                .params()
                .iter()
                .map(project_function_param)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    FunctionSignature::new(return_type, params).with_remaining_param_groups(remaining_param_groups)
}

fn project_function_param(param: &SyntaxFnParam) -> FunctionParam {
    let name = callable_param_name(param.pattern()).unwrap_or("_");
    let ty = project_type_ref_kind(param.ty());
    if param.is_rest() {
        FunctionParam::rest(name, ty)
    } else if param.default().is_some() {
        FunctionParam::defaulted(name, ty)
    } else {
        FunctionParam::required(name, ty)
    }
}

fn curried_project_signature_return_type(signature: &SyntaxFnSignature) -> TypeKind {
    let return_type = signature
        .return_type()
        .map_or_else(|| TypeKind::Named("_".to_owned()), project_type_ref_kind);
    signature
        .param_groups()
        .iter()
        .skip(1)
        .rev()
        .fold(return_type, |return_type, group| {
            TypeKind::function(
                group
                    .params()
                    .iter()
                    .map(|param| project_type_ref_kind(param.ty())),
                return_type,
            )
        })
}

fn callable_param_name(pattern: &Pattern) -> Option<&str> {
    match pattern {
        Pattern::Ident(name)
        | Pattern::MutIdent(name)
        | Pattern::Typed { name, .. }
        | Pattern::Whole { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

pub(super) fn signal_value_type(
    id: &str,
    signature_tail: &str,
) -> Result<Option<TypeKind>, ProjectSemanticIndexError> {
    let Some(type_source) = signature_tail.trim().strip_prefix(':') else {
        return Ok(None);
    };
    let type_ref = parse_type_ref(type_source.trim()).map_err(|error| {
        ProjectSemanticIndexError::InvalidSignalType {
            id: id.to_owned(),
            message: error.to_string(),
        }
    })?;
    Ok(Some(signal_declared_value_type(&type_ref)))
}

fn signal_declared_value_type(ty: &TypeRef) -> TypeKind {
    match ty {
        TypeRef::Generic { base, args } if base == "Watch" && args.len() == 1 => {
            project_type_ref_kind(&args[0])
        }
        _ => project_type_ref_kind(ty),
    }
}

fn project_type_ref_kind(ty: &TypeRef) -> TypeKind {
    match ty {
        TypeRef::Generic { base, args } if base == "Ref" && args.len() == 1 => {
            if let TypeRef::Path(name) = &args[0] {
                entity_kind_from_type_name(name)
                    .map_or_else(|| type_ref_kind(ty), TypeKind::entity_ref)
            } else {
                type_ref_kind(ty)
            }
        }
        TypeRef::Generic { base, args } if base == "Option" && args.len() == 1 => {
            TypeKind::Option(Box::new(project_type_ref_kind(&args[0])))
        }
        TypeRef::Generic { base, args } if base == "Vec" && args.len() == 1 => {
            TypeKind::Vec(Box::new(project_type_ref_kind(&args[0])))
        }
        _ => type_ref_kind(ty),
    }
}

fn entity_kind_from_type_name(name: &str) -> Option<EntityKind> {
    Some(match name {
        "Agent" => EntityKind::Agent,
        "Entry" => EntityKind::Entry,
        "Flow" => EntityKind::Flow,
        "Choice" => EntityKind::Choice,
        "ChoiceOption" => EntityKind::ChoiceOption,
        "Character" => EntityKind::Character,
        "View" => EntityKind::View,
        "Action" => EntityKind::Action,
        "Activity" => EntityKind::Activity,
        "DialogueLine" => EntityKind::DialogueLine,
        "Text" => EntityKind::Text,
        "Content" => EntityKind::Content,
        "Style" => EntityKind::Style,
        "Asset" => EntityKind::Asset,
        "Image" => EntityKind::Image,
        "Animation" => EntityKind::Animation,
        "Capture" => EntityKind::Capture,
        "Hook" => EntityKind::Hook,
        "Signal" => EntityKind::Signal,
        "Metric" => EntityKind::Metric,
        "Scene" => EntityKind::Scene,
        "Source" => EntityKind::Source,
        "Test" => EntityKind::Test,
        "Bench" => EntityKind::Bench,
        "Layer" => EntityKind::Layer,
        "Voice" => EntityKind::Voice,
        "Se" => EntityKind::Se,
        "Bgm" => EntityKind::Bgm,
        "AudioBus" => EntityKind::AudioBus,
        "MixerSnapshot" => EntityKind::MixerSnapshot,
        "Ducking" => EntityKind::Ducking,
        "Motion" => EntityKind::Motion,
        "Rig" => EntityKind::Rig,
        "Slot" => EntityKind::Slot,
        "Target" => EntityKind::Target,
        _ => return None,
    })
}

pub(super) fn entity_decl_kind(kind: EntityDeclKind) -> EntityKind {
    match kind {
        EntityDeclKind::Asset => EntityKind::Asset,
        EntityDeclKind::Image => EntityKind::Image,
        EntityDeclKind::Character => EntityKind::Character,
        EntityDeclKind::View => EntityKind::View,
        EntityDeclKind::Action => EntityKind::Action,
        EntityDeclKind::Activity => EntityKind::Activity,
        EntityDeclKind::Content => EntityKind::Content,
        EntityDeclKind::Signal => EntityKind::Signal,
        EntityDeclKind::Metric => EntityKind::Metric,
        EntityDeclKind::Layer => EntityKind::Layer,
        EntityDeclKind::Voice => EntityKind::Voice,
        EntityDeclKind::Se => EntityKind::Se,
        EntityDeclKind::Bgm => EntityKind::Bgm,
        EntityDeclKind::AudioBus => EntityKind::AudioBus,
        EntityDeclKind::MixerSnapshot => EntityKind::MixerSnapshot,
        EntityDeclKind::Ducking => EntityKind::Ducking,
        EntityDeclKind::Motion => EntityKind::Motion,
        EntityDeclKind::Rig => EntityKind::Rig,
    }
}

pub(super) fn entity_decl_kind_label(kind: EntityDeclKind) -> &'static str {
    match kind {
        EntityDeclKind::Asset => "asset",
        EntityDeclKind::Image => "image",
        EntityDeclKind::Character => "character",
        EntityDeclKind::View => "view",
        EntityDeclKind::Action => "action",
        EntityDeclKind::Activity => "activity",
        EntityDeclKind::Content => "content",
        EntityDeclKind::Signal => "signal",
        EntityDeclKind::Metric => "metric",
        EntityDeclKind::Layer => "layer",
        EntityDeclKind::Voice => "voice",
        EntityDeclKind::Se => "se",
        EntityDeclKind::Bgm => "bgm",
        EntityDeclKind::AudioBus => "audio bus",
        EntityDeclKind::MixerSnapshot => "mixer snapshot",
        EntityDeclKind::Ducking => "ducking",
        EntityDeclKind::Motion => "motion",
        EntityDeclKind::Rig => "rig",
    }
}

fn type_kind_stable_label(ty: &TypeKind) -> String {
    match ty {
        TypeKind::Bool => "bool".to_owned(),
        TypeKind::Char => "char".to_owned(),
        TypeKind::Ref(entity) => entity.value().map_or_else(
            || format!("Ref<{:?}>", entity.kind()),
            |value| format!("Ref<{:?},{}>", entity.kind(), type_kind_stable_label(value)),
        ),
        TypeKind::Probe(inner) => format!("Probe<{}>", type_kind_stable_label(inner)),
        TypeKind::Vec(inner) => format!("Vec<{}>", type_kind_stable_label(inner)),
        TypeKind::Array { item, len } => {
            format!("Array<{},{}>", type_kind_stable_label(item), len)
        }
        TypeKind::Slice(inner) => format!("Slice<{}>", type_kind_stable_label(inner)),
        TypeKind::Seq(inner) => format!("Seq<{}>", type_kind_stable_label(inner)),
        TypeKind::Map { kind, key, value } => format!(
            "Map<{:?},{},{}>",
            kind,
            type_kind_stable_label(key),
            type_kind_stable_label(value)
        ),
        TypeKind::BorrowRef { kind, inner, .. } => {
            format!(
                "BorrowRef<{},{}>",
                kind.stable_label(),
                type_kind_stable_label(inner)
            )
        }
        TypeKind::Need { ready, error } => format!(
            "Need<{},{}>",
            type_kind_stable_label(ready),
            type_kind_stable_label(error)
        ),
        TypeKind::Stream { item, error } => format!(
            "Stream<{},{}>",
            type_kind_stable_label(item),
            type_kind_stable_label(error)
        ),
        TypeKind::Source { item, error } => format!(
            "Source<{},{}>",
            type_kind_stable_label(item),
            type_kind_stable_label(error)
        ),
        TypeKind::Result { ok, error } => format!(
            "Result<{},{}>",
            type_kind_stable_label(ok),
            type_kind_stable_label(error)
        ),
        TypeKind::Option(inner) => format!("Option<{}>", type_kind_stable_label(inner)),
        TypeKind::Handle { name, .. } => format!("Handle<{name}>"),
        TypeKind::ThreadHandle(inner) => format!("ThreadHandle<{}>", type_kind_stable_label(inner)),
        TypeKind::Shared(inner) => format!("Shared<{}>", type_kind_stable_label(inner)),
        TypeKind::Function {
            params,
            return_type,
            ..
        } => format!(
            "Function<({}),{}>",
            params
                .iter()
                .map(type_kind_stable_label)
                .collect::<Vec<_>>()
                .join(","),
            type_kind_stable_label(return_type)
        ),
        TypeKind::Speaker(kind) => format!("Speaker<{kind:?}>"),
        TypeKind::SpeakerPreset(kind) => format!("SpeakerPreset<{kind:?}>"),
        TypeKind::CharacterPatch(kind) => format!("CharacterPatch<{kind:?}>"),
        TypeKind::Named(name) => name.clone(),
        TypeKind::Tuple(items) => format!(
            "Tuple<{}>",
            items
                .iter()
                .map(type_kind_stable_label)
                .collect::<Vec<_>>()
                .join(",")
        ),
        TypeKind::Choice(items) => format!(
            "Choice<{}>",
            items
                .iter()
                .map(type_kind_stable_label)
                .collect::<Vec<_>>()
                .join("|")
        ),
        other => format!("{other:?}"),
    }
}
