//! Runtime function-value extraction for source-local top-level `fn` bodies.

use crate::expr::lower_runtime_expr_strict;
use arcweft_core::value::RuntimeExpr;
use arcweft_lang_hir::{
    model::{HirFunction, HirModule},
    syntax::{
        ast::{flow::Stmt, items::FunctionKind, pattern::Pattern},
        expr::{Expr, MatchExprArm},
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

fn lower_runtime_function_value_candidate(
    function: &HirFunction,
) -> Option<RuntimeFunctionValueCandidate> {
    if function.kind() != FunctionKind::Function {
        return None;
    }
    let param_groups = runtime_function_value_param_groups(function)?;
    let input_names = param_groups.first()?.clone();
    let (statements, value) = runtime_function_value_body_parts(function)?;
    if !statements
        .iter()
        .all(runtime_function_value_statement_supported)
        || !runtime_function_value_expr_supported(value)
    {
        return None;
    }
    let body = lower_runtime_function_value_body(statements, value)?;
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

fn lower_runtime_function_value_body(statements: &[Stmt], value: &Expr) -> Option<RuntimeExpr> {
    let body = lower_runtime_expr_strict(value).ok()?;
    statements.iter().rev().try_fold(body, |body, stmt| {
        let Stmt::Let { pattern, expr, .. } = stmt else {
            return None;
        };
        let name = binding_pattern_name(pattern)?;
        let expr = lower_runtime_expr_strict(expr).ok()?;
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

fn runtime_function_value_statement_supported(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { pattern, expr, .. } => {
            binding_pattern_name(pattern).is_some() && runtime_function_value_expr_supported(expr)
        }
        _ => false,
    }
}

fn runtime_function_value_expr_supported(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::Path(_)
        | Expr::ShortVariant(_)
        | Expr::NumericBracketSeq(_) => true,
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            items.iter().all(runtime_function_value_expr_supported)
        }
        Expr::ArrayRepeat { value, len }
        | Expr::Index {
            target: value,
            index: len,
        } => {
            runtime_function_value_expr_supported(value)
                && runtime_function_value_expr_supported(len)
        }
        Expr::Select(select) => runtime_function_value_expr_supported(select.target()),
        Expr::Range { start, end, .. } => start
            .as_deref()
            .into_iter()
            .chain(end.as_deref())
            .all(runtime_function_value_expr_supported),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
            .iter()
            .all(|(_, value)| runtime_function_value_expr_supported(value)),
        Expr::Binary { lhs, rhs, .. } => {
            runtime_function_value_expr_supported(lhs) && runtime_function_value_expr_supported(rhs)
        }
        Expr::Unary { expr, .. } => runtime_function_value_expr_supported(expr),
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => runtime_function_value_block_supported(statements, value.as_deref()),
        Expr::MemoBlock {
            options,
            statements,
            value,
        } => runtime_function_value_memo_block_supported(options, statements, value.as_deref()),
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => runtime_function_value_if_supported(condition, then_branch, else_branch.as_deref()),
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => runtime_function_value_if_let_supported(
            expr,
            guard.as_deref(),
            then_branch,
            else_branch.as_deref(),
        ),
        Expr::Match { scrutinee, arms } => {
            runtime_function_value_expr_supported(scrutinee)
                && arms.iter().all(runtime_function_value_match_arm_supported)
        }
        Expr::LifetimePath { .. }
        | Expr::Placeholder(_)
        | Expr::Call { .. }
        | Expr::DialogueCall { .. }
        | Expr::Pipe { .. }
        | Expr::Try { .. }
        | Expr::Await { .. }
        | Expr::Thread { .. }
        | Expr::Closure { .. }
        | Expr::Raw(_) => false,
    }
}

fn runtime_function_value_block_supported(statements: &[Stmt], value: Option<&Expr>) -> bool {
    statements
        .iter()
        .all(runtime_function_value_statement_supported)
        && value.is_some_and(runtime_function_value_expr_supported)
}

fn runtime_function_value_memo_block_supported(
    options: &[(String, Expr)],
    statements: &[Stmt],
    value: Option<&Expr>,
) -> bool {
    options
        .iter()
        .all(|(_, value)| runtime_function_value_expr_supported(value))
        && runtime_function_value_block_supported(statements, value)
}

fn runtime_function_value_if_supported(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
) -> bool {
    runtime_function_value_expr_supported(condition)
        && runtime_function_value_expr_supported(then_branch)
        && else_branch.is_some_and(runtime_function_value_expr_supported)
}

fn runtime_function_value_if_let_supported(
    expr: &Expr,
    guard: Option<&Expr>,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
) -> bool {
    runtime_function_value_expr_supported(expr)
        && guard.is_none_or(runtime_function_value_expr_supported)
        && runtime_function_value_expr_supported(then_branch)
        && else_branch.is_some_and(runtime_function_value_expr_supported)
}

fn runtime_function_value_match_arm_supported(arm: &MatchExprArm) -> bool {
    arm.guard()
        .is_none_or(runtime_function_value_expr_supported)
        && runtime_function_value_expr_supported(arm.value())
}
