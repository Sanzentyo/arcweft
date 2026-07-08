//! Runtime function-value extraction for source-local top-level `fn` bodies.

use crate::expr::lower_runtime_expr_strict_with_function_locals;
use arcweft_core::value::RuntimeExpr;
use arcweft_lang_hir::{
    model::{HirFunction, HirModule},
    syntax::{
        ast::{
            flow::Stmt,
            items::FunctionKind,
            pattern::{Pattern, VariantPatternPayload},
        },
        expr::{CallArg, ClosureParam, Expr, MatchExprArm},
        types::TypeRef,
    },
};
use std::collections::BTreeMap;

/// Top-level source function body accepted as a runtime function value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RuntimeFunctionValueCandidate {
    name: String,
    input_names: Vec<String>,
    value: RuntimeExpr,
}

impl RuntimeFunctionValueCandidate {
    pub(crate) fn input_names(&self) -> &[String] {
        &self.input_names
    }

    pub(crate) fn value(&self) -> RuntimeExpr {
        self.value.clone()
    }
}

pub(crate) fn lower_runtime_function_value_candidates(
    module: &HirModule,
) -> Vec<RuntimeFunctionValueCandidate> {
    module
        .functions()
        .iter()
        .filter_map(lower_runtime_function_value_candidate)
        .collect()
}

pub(crate) fn runtime_function_value_map(
    candidates: &[RuntimeFunctionValueCandidate],
) -> BTreeMap<String, RuntimeFunctionValueCandidate> {
    candidates
        .iter()
        .cloned()
        .map(|candidate| (candidate.name.clone(), candidate))
        .collect()
}

#[derive(Clone, Debug, Default)]
struct RuntimeFunctionValueContext {
    function_locals: BTreeMap<String, FunctionLocalSignature>,
}

#[derive(Clone, Debug)]
struct FunctionLocalSignature {
    arity: usize,
    return_type: Option<TypeRef>,
}

impl RuntimeFunctionValueContext {
    fn function_local_arity(&self, name: &str) -> Option<usize> {
        self.function_locals
            .get(name)
            .map(|signature| signature.arity)
    }

    fn function_local_signature(&self, name: &str) -> Option<&FunctionLocalSignature> {
        self.function_locals.get(name)
    }

    fn shadows_function_local(&self, name: &str) -> bool {
        self.function_locals.contains_key(name)
    }

    fn insert_function_local(&mut self, name: String, signature: FunctionLocalSignature) {
        self.function_locals.insert(name, signature);
    }

    fn function_local_arities(&self) -> BTreeMap<String, usize> {
        self.function_locals
            .iter()
            .map(|(name, signature)| (name.clone(), signature.arity))
            .collect()
    }
}

fn lower_runtime_function_value_candidate(
    function: &HirFunction,
) -> Option<RuntimeFunctionValueCandidate> {
    if function.kind() != FunctionKind::Function {
        return None;
    }
    let param_groups = runtime_function_value_param_groups(function)?;
    let context = runtime_function_value_context(function);
    let input_names = param_groups.first()?.clone();
    let (statements, value) = runtime_function_value_body_parts(function)?;
    let context = runtime_function_value_supported_context(context, statements, value)?;
    let body = lower_runtime_function_value_body(statements, value, &context)?;
    let value = param_groups
        .into_iter()
        .rev()
        .fold(body, |body, params| RuntimeExpr::Function {
            params,
            body: Box::new(body),
        });
    Some(RuntimeFunctionValueCandidate {
        name: function.name().to_owned(),
        input_names,
        value,
    })
}

fn runtime_function_value_param_groups(function: &HirFunction) -> Option<Vec<Vec<String>>> {
    let groups = function
        .signature()
        .param_groups()
        .iter()
        .map(|group| {
            group
                .params()
                .iter()
                .map(|param| {
                    if param.is_rest()
                        || param.default().is_some()
                        || param.receiver_kind().is_some()
                    {
                        return None;
                    }
                    binding_pattern_name(param.pattern())
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()?;
    (!groups.is_empty()).then_some(groups)
}

fn runtime_function_value_context(function: &HirFunction) -> RuntimeFunctionValueContext {
    let function_locals = function
        .signature()
        .param_groups()
        .iter()
        .flat_map(|group| group.params().iter())
        .filter_map(|param| {
            let signature = function_local_signature_from_type(param.ty())?;
            let name = binding_pattern_name(param.pattern())?;
            Some((name, signature))
        })
        .collect();
    RuntimeFunctionValueContext { function_locals }
}

fn function_local_signature_from_type(ty: &TypeRef) -> Option<FunctionLocalSignature> {
    match ty {
        TypeRef::Function {
            params,
            return_type,
        } => Some(FunctionLocalSignature {
            arity: params.len(),
            return_type: Some((**return_type).clone()),
        }),
        _ => None,
    }
}

fn function_local_signature_from_closure(
    params: &[ClosureParam],
    return_type: Option<&TypeRef>,
) -> FunctionLocalSignature {
    FunctionLocalSignature {
        arity: params.len(),
        return_type: return_type.cloned(),
    }
}

fn function_local_result_signature(
    signature: &FunctionLocalSignature,
    arg_count: usize,
) -> Option<FunctionLocalSignature> {
    if arg_count < signature.arity {
        return Some(FunctionLocalSignature {
            arity: signature.arity - arg_count,
            return_type: signature.return_type.clone(),
        });
    }
    if arg_count == signature.arity {
        return signature
            .return_type
            .as_ref()
            .and_then(function_local_signature_from_type);
    }
    None
}

fn runtime_function_value_supported_context(
    mut context: RuntimeFunctionValueContext,
    statements: &[Stmt],
    value: &Expr,
) -> Option<RuntimeFunctionValueContext> {
    for stmt in statements {
        if !runtime_function_value_statement_supported(stmt, &context) {
            return None;
        }
        let Stmt::Let { pattern, expr, .. } = stmt else {
            return None;
        };
        let name = binding_pattern_name(pattern)?;
        if let Some(signature) = runtime_function_value_expr_function_signature(expr, &context) {
            context.insert_function_local(name, signature);
        }
    }
    runtime_function_value_expr_supported(value, &context).then_some(context)
}

fn runtime_function_value_body_parts(function: &HirFunction) -> Option<(&[Stmt], &Expr)> {
    if let Some(value) = function.value() {
        return Some((function.statements(), value.expr()));
    }
    let (last, statements) = function.statements().split_last()?;
    match last {
        Stmt::Return { expr, .. } => Some((statements, expr)),
        _ => None,
    }
}

fn lower_runtime_function_value_body(
    statements: &[Stmt],
    value: &Expr,
    context: &RuntimeFunctionValueContext,
) -> Option<RuntimeExpr> {
    let function_local_arities = context.function_local_arities();
    let body =
        lower_runtime_expr_strict_with_function_locals(value, &function_local_arities).ok()?;
    statements.iter().rev().try_fold(body, |body, stmt| {
        let Stmt::Let { pattern, expr, .. } = stmt else {
            return None;
        };
        let name = binding_pattern_name(pattern)?;
        let expr =
            lower_runtime_expr_strict_with_function_locals(expr, &function_local_arities).ok()?;
        Some(RuntimeExpr::Let {
            name,
            expr: Box::new(expr),
            body: Box::new(body),
        })
    })
}

fn binding_pattern_name(pattern: &Pattern) -> Option<String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            Some(name.clone())
        }
        _ => None,
    }
}

fn pattern_binds_function_local(pattern: &Pattern, context: &RuntimeFunctionValueContext) -> bool {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            context.shadows_function_local(name)
        }
        Pattern::Whole { name, pattern } => {
            context.shadows_function_local(name) || pattern_binds_function_local(pattern, context)
        }
        Pattern::Tuple(items) | Pattern::BracketSeq { items, .. } => items
            .iter()
            .any(|pattern| pattern_binds_function_local(pattern, context)),
        Pattern::Record { fields, .. } => fields
            .iter()
            .any(|field| pattern_binds_function_local(field.pattern(), context)),
        Pattern::Variant { payload, .. } => payload
            .as_ref()
            .is_some_and(|payload| variant_payload_binds_function_local(payload, context)),
        Pattern::Literal(_) | Pattern::Entity(_) | Pattern::Discard | Pattern::Raw(_) => false,
    }
}

fn variant_payload_binds_function_local(
    payload: &VariantPatternPayload,
    context: &RuntimeFunctionValueContext,
) -> bool {
    match payload {
        VariantPatternPayload::Tuple(items) => items
            .iter()
            .any(|pattern| pattern_binds_function_local(pattern, context)),
        VariantPatternPayload::Record { fields, .. } => fields
            .iter()
            .any(|field| pattern_binds_function_local(field.pattern(), context)),
    }
}

fn runtime_function_value_statement_supported(
    stmt: &Stmt,
    context: &RuntimeFunctionValueContext,
) -> bool {
    match stmt {
        Stmt::Let { pattern, expr, .. } => binding_pattern_name(pattern).is_some_and(|name| {
            !context.shadows_function_local(&name)
                && runtime_function_value_expr_supported(expr, context)
        }),
        _ => false,
    }
}

fn runtime_function_value_expr_supported(
    expr: &Expr,
    context: &RuntimeFunctionValueContext,
) -> bool {
    match expr {
        Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::Path(_)
        | Expr::ShortVariant(_)
        | Expr::NumericBracketSeq(_) => true,
        Expr::Tuple(items) | Expr::BracketSeq(items) => items
            .iter()
            .all(|item| runtime_function_value_expr_supported(item, context)),
        Expr::ArrayRepeat { value, len }
        | Expr::Index {
            target: value,
            index: len,
        } => {
            runtime_function_value_expr_supported(value, context)
                && runtime_function_value_expr_supported(len, context)
        }
        Expr::Select(select) => runtime_function_value_expr_supported(select.target(), context),
        Expr::Range { start, end, .. } => start
            .as_deref()
            .into_iter()
            .chain(end.as_deref())
            .all(|expr| runtime_function_value_expr_supported(expr, context)),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
            .iter()
            .all(|(_, value)| runtime_function_value_expr_supported(value, context)),
        Expr::Binary { lhs, rhs, .. } => {
            runtime_function_value_expr_supported(lhs, context)
                && runtime_function_value_expr_supported(rhs, context)
        }
        Expr::Unary { expr, .. } => runtime_function_value_expr_supported(expr, context),
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => runtime_function_value_block_supported(statements, value.as_deref(), context),
        Expr::MemoBlock {
            options,
            statements,
            value,
        } => runtime_function_value_memo_block_supported(
            options,
            statements,
            value.as_deref(),
            context,
        ),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => runtime_function_value_if_supported(
            condition,
            then_branch,
            else_branch.as_deref(),
            context,
        ),
        Expr::IfLet {
            pattern,
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => runtime_function_value_if_let_supported(
            pattern,
            expr,
            guard.as_deref(),
            then_branch,
            else_branch.as_deref(),
            context,
        ),
        Expr::Match { scrutinee, arms } => {
            runtime_function_value_expr_supported(scrutinee, context)
                && arms
                    .iter()
                    .all(|arm| runtime_function_value_match_arm_supported(arm, context))
        }
        Expr::Call { callee, args } => {
            runtime_function_value_local_function_call_supported(callee, args, context)
        }
        Expr::LifetimePath { .. }
        | Expr::Placeholder(_)
        | Expr::DialogueCall { .. }
        | Expr::Pipe { .. }
        | Expr::Try { .. }
        | Expr::Await { .. }
        | Expr::Thread { .. }
        | Expr::Raw(_) => false,
        Expr::Closure { params, body, .. } => {
            runtime_function_value_closure_supported(params, body, context)
        }
    }
}

fn runtime_function_value_closure_supported(
    params: &[ClosureParam],
    body: &Expr,
    context: &RuntimeFunctionValueContext,
) -> bool {
    params
        .iter()
        .all(|param| !pattern_binds_function_local(param.pattern(), context))
        && runtime_function_value_expr_supported(body, context)
}

fn runtime_function_value_local_function_call_supported(
    callee: &Expr,
    args: &[CallArg],
    context: &RuntimeFunctionValueContext,
) -> bool {
    let Expr::Path(path) = callee else {
        return false;
    };
    context
        .function_local_arity(path.as_label())
        .is_some_and(|arity| arity >= args.len())
        && args.iter().all(|arg| {
            !arg.is_spread()
                && arg.name().is_none()
                && runtime_function_value_expr_supported(arg.value(), context)
        })
}

fn runtime_function_value_expr_function_signature(
    expr: &Expr,
    context: &RuntimeFunctionValueContext,
) -> Option<FunctionLocalSignature> {
    match expr {
        Expr::Path(path) => context.function_local_signature(path.as_label()).cloned(),
        Expr::Closure {
            params,
            return_type,
            body,
        } => runtime_function_value_closure_supported(params, body, context)
            .then(|| function_local_signature_from_closure(params, return_type.as_ref())),
        Expr::Call { callee, args }
            if runtime_function_value_local_function_call_supported(callee, args, context) =>
        {
            let Expr::Path(path) = callee.as_ref() else {
                return None;
            };
            context
                .function_local_signature(path.as_label())
                .and_then(|signature| function_local_result_signature(signature, args.len()))
        }
        _ => None,
    }
}

fn runtime_function_value_block_supported(
    statements: &[Stmt],
    value: Option<&Expr>,
    context: &RuntimeFunctionValueContext,
) -> bool {
    statements
        .iter()
        .all(|stmt| runtime_function_value_statement_supported(stmt, context))
        && value.is_some_and(|value| runtime_function_value_expr_supported(value, context))
}

fn runtime_function_value_memo_block_supported(
    options: &[(String, Expr)],
    statements: &[Stmt],
    value: Option<&Expr>,
    context: &RuntimeFunctionValueContext,
) -> bool {
    options
        .iter()
        .all(|(_, value)| runtime_function_value_expr_supported(value, context))
        && runtime_function_value_block_supported(statements, value, context)
}

fn runtime_function_value_if_supported(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    context: &RuntimeFunctionValueContext,
) -> bool {
    runtime_function_value_expr_supported(condition, context)
        && runtime_function_value_expr_supported(then_branch, context)
        && else_branch.is_some_and(|expr| runtime_function_value_expr_supported(expr, context))
}

fn runtime_function_value_if_let_supported(
    pattern: &Pattern,
    expr: &Expr,
    guard: Option<&Expr>,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    context: &RuntimeFunctionValueContext,
) -> bool {
    !pattern_binds_function_local(pattern, context)
        && runtime_function_value_expr_supported(expr, context)
        && guard.is_none_or(|guard| runtime_function_value_expr_supported(guard, context))
        && runtime_function_value_expr_supported(then_branch, context)
        && else_branch.is_some_and(|expr| runtime_function_value_expr_supported(expr, context))
}

fn runtime_function_value_match_arm_supported(
    arm: &MatchExprArm,
    context: &RuntimeFunctionValueContext,
) -> bool {
    !pattern_binds_function_local(arm.pattern(), context)
        && arm
            .guard()
            .is_none_or(|guard| runtime_function_value_expr_supported(guard, context))
        && runtime_function_value_expr_supported(arm.value(), context)
}
