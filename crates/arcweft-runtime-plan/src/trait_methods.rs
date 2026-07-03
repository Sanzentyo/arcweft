//! Runtime-plan lowering for executable trait method bodies.

use crate::errors::RuntimePlanLowerError;
use crate::expr::lower_runtime_expr_strict_with_pure;
use crate::labels::expr_label;
use arcweft_core::plan::{
    RuntimePureInputType, RuntimePureOutputType, RuntimeReceiverMode, RuntimeTraitMethod,
    RuntimeTraitMethodId, RuntimeTraitMethodIdentity,
};
use arcweft_core::value::RuntimeExpr;
use arcweft_lang_hir::syntax::ast::flow::Stmt;
use arcweft_lang_hir::syntax::ast::pattern::Pattern;
use arcweft_lang_hir::syntax::expr::Expr;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct TraitMethodLowerInput<'a> {
    pub identity: RuntimeTraitMethodIdentity,
    pub receiver: RuntimeReceiverMode,
    pub input_names: Vec<String>,
    pub input_types: Vec<RuntimePureInputType>,
    pub output_type: RuntimePureOutputType,
    pub statements: &'a [Stmt],
    pub value: Option<&'a Expr>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeTraitMethodInventory {
    pub methods: Vec<RuntimeTraitMethod>,
    pub by_witness_method: BTreeMap<(usize, String), RuntimeTraitMethodId>,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum TraitMethodLowerDiagnostic {
    #[error("trait method `{method}` has no executable body")]
    MissingMethodBody { method: String },
    #[error("trait method `{method}` body is unsupported: {reason}")]
    UnsupportedBody { method: String, reason: String },
    #[error("trait method `{method}` would capture non-deterministic iterator state: {reason}")]
    NonDeterministicIteratorState { method: String, reason: String },
}

pub fn lower_trait_method_inventory<'a>(
    inputs: impl IntoIterator<Item = TraitMethodLowerInput<'a>>,
) -> Result<RuntimeTraitMethodInventory, Vec<RuntimePlanLowerError>> {
    let mut inventory = RuntimeTraitMethodInventory::default();
    let mut errors = Vec::new();
    for input in inputs {
        match lower_trait_method(input, RuntimeTraitMethodId(inventory.methods.len())) {
            Ok(method) => {
                if let Some(witness) = method.identity.witness {
                    inventory
                        .by_witness_method
                        .insert((witness, method.identity.method_name.clone()), method.id);
                }
                inventory.methods.push(method);
            }
            Err(error) => errors.push(RuntimePlanLowerError::new(error.to_string())),
        }
    }
    if errors.is_empty() {
        Ok(inventory)
    } else {
        Err(errors)
    }
}

fn lower_trait_method(
    input: TraitMethodLowerInput<'_>,
    id: RuntimeTraitMethodId,
) -> Result<RuntimeTraitMethod, TraitMethodLowerDiagnostic> {
    let method = input.identity.method_name.clone();
    let (statements, value) = method_body_parts(&method, input.statements, input.value)?;
    let body = lower_runtime_expr_strict_with_pure(value, &BTreeMap::new()).map_err(|reason| {
        TraitMethodLowerDiagnostic::UnsupportedBody {
            method: method.clone(),
            reason,
        }
    })?;
    let body = statements.iter().rev().try_fold(body, |body, statement| {
        lower_prefix_statement(&method, statement, body)
    })?;
    reject_nondeterministic_iterator_state(&method, input.receiver, &body)?;
    Ok(RuntimeTraitMethod {
        id,
        identity: input.identity,
        receiver: input.receiver,
        input_names: input.input_names,
        input_types: input.input_types,
        output_type: input.output_type,
        body,
    })
}

fn method_body_parts<'a>(
    method: &str,
    statements: &'a [Stmt],
    value: Option<&'a Expr>,
) -> Result<(&'a [Stmt], &'a Expr), TraitMethodLowerDiagnostic> {
    if let Some(value) = value {
        return Ok((statements, value));
    }
    let Some((last, statements)) = statements.split_last() else {
        return Err(TraitMethodLowerDiagnostic::MissingMethodBody {
            method: method.to_owned(),
        });
    };
    match last {
        Stmt::Return(value) => Ok((statements, value)),
        _ => Err(TraitMethodLowerDiagnostic::MissingMethodBody {
            method: method.to_owned(),
        }),
    }
}

fn lower_prefix_statement(
    method: &str,
    statement: &Stmt,
    body: RuntimeExpr,
) -> Result<RuntimeExpr, TraitMethodLowerDiagnostic> {
    match statement {
        Stmt::Let { pattern, expr, .. } => {
            let name = simple_pattern_name(pattern).ok_or_else(|| {
                TraitMethodLowerDiagnostic::UnsupportedBody {
                    method: method.to_owned(),
                    reason: format!("unsupported let pattern `{pattern:?}`"),
                }
            })?;
            let expr =
                lower_runtime_expr_strict_with_pure(expr, &BTreeMap::new()).map_err(|reason| {
                    TraitMethodLowerDiagnostic::UnsupportedBody {
                        method: method.to_owned(),
                        reason,
                    }
                })?;
            Ok(RuntimeExpr::Let {
                name,
                expr: Box::new(expr),
                body: Box::new(body),
            })
        }
        Stmt::Assign { target, expr } => {
            let (target, field) = lower_direct_assignment_target(method, target)?;
            let expr =
                lower_runtime_expr_strict_with_pure(expr, &BTreeMap::new()).map_err(|reason| {
                    TraitMethodLowerDiagnostic::UnsupportedBody {
                        method: method.to_owned(),
                        reason,
                    }
                })?;
            Ok(RuntimeExpr::AssignField {
                target: Box::new(target),
                field,
                expr: Box::new(expr),
                body: Box::new(body),
            })
        }
        other => Err(TraitMethodLowerDiagnostic::UnsupportedBody {
            method: method.to_owned(),
            reason: format!("unsupported statement `{other:?}`"),
        }),
    }
}

fn lower_direct_assignment_target(
    method: &str,
    target: &Expr,
) -> Result<(RuntimeExpr, String), TraitMethodLowerDiagnostic> {
    let Expr::Field { target, field } = target else {
        return Err(TraitMethodLowerDiagnostic::UnsupportedBody {
            method: method.to_owned(),
            reason: format!(
                "unsupported assignment target `{}`; only direct record fields are executable",
                expr_label(target)
            ),
        });
    };
    let receiver =
        lower_runtime_expr_strict_with_pure(target, &BTreeMap::new()).map_err(|reason| {
            TraitMethodLowerDiagnostic::UnsupportedBody {
                method: method.to_owned(),
                reason,
            }
        })?;
    match receiver {
        RuntimeExpr::Local(_) => Ok((receiver, field.clone())),
        RuntimeExpr::Field { .. }
        | RuntimeExpr::ProjectTuple { .. }
        | RuntimeExpr::ProjectRecord { .. } => Err(TraitMethodLowerDiagnostic::UnsupportedBody {
            method: method.to_owned(),
            reason: format!(
                "unsupported nested assignment target `{}`; nested lvalues require a future RuntimeLValue model",
                expr_label(target)
            ),
        }),
        other => Err(TraitMethodLowerDiagnostic::UnsupportedBody {
            method: method.to_owned(),
            reason: format!(
                "unsupported assignment receiver `{other}`; assignment requires a local record value"
            ),
        }),
    }
}

fn simple_pattern_name(pattern: &Pattern) -> Option<String> {
    pattern.simple_binding_name().map(str::to_owned)
}

fn reject_nondeterministic_iterator_state(
    method: &str,
    receiver: RuntimeReceiverMode,
    body: &RuntimeExpr,
) -> Result<(), TraitMethodLowerDiagnostic> {
    if receiver == RuntimeReceiverMode::MutRef && contains_host_or_source_call(body) {
        return Err(TraitMethodLowerDiagnostic::NonDeterministicIteratorState {
            method: method.to_owned(),
            reason: "host/source/stream call in &mut self iterator body".to_owned(),
        });
    }
    Ok(())
}

fn contains_host_or_source_call(expr: &RuntimeExpr) -> bool {
    match expr {
        RuntimeExpr::Call { callee, .. } => {
            let label = callee.as_label();
            label.starts_with("host.")
                || label.starts_with("source.")
                || label.starts_with("stream.")
        }
        RuntimeExpr::Let { expr, body, .. } | RuntimeExpr::AssignField { expr, body, .. } => {
            contains_host_or_source_call(expr) || contains_host_or_source_call(body)
        }
        RuntimeExpr::TraitCall { receiver, args, .. }
        | RuntimeExpr::MethodCall { receiver, args, .. } => {
            contains_host_or_source_call(receiver) || args.iter().any(contains_host_or_source_call)
        }
        RuntimeExpr::Tuple(items) | RuntimeExpr::BracketSeq(items) => {
            items.iter().any(contains_host_or_source_call)
        }
        RuntimeExpr::RepeatSeq { value, .. }
        | RuntimeExpr::Field { target: value, .. }
        | RuntimeExpr::ProjectTuple { target: value, .. }
        | RuntimeExpr::ProjectRecord { target: value, .. }
        | RuntimeExpr::SpreadArg(value)
        | RuntimeExpr::Map { source: value, .. }
        | RuntimeExpr::Sum { source: value }
        | RuntimeExpr::Unary { expr: value, .. } => contains_host_or_source_call(value),
        RuntimeExpr::Range { start, end, .. } => start
            .as_deref()
            .into_iter()
            .chain(end.as_deref())
            .any(contains_host_or_source_call),
        RuntimeExpr::Record(fields) => fields
            .iter()
            .any(|field| contains_host_or_source_call(&field.value)),
        RuntimeExpr::Variant { payload, .. } => {
            payload.as_deref().is_some_and(contains_host_or_source_call)
        }
        RuntimeExpr::PureCall { args, .. } => args.iter().any(contains_host_or_source_call),
        RuntimeExpr::Binary { lhs, rhs, .. } => {
            contains_host_or_source_call(lhs) || contains_host_or_source_call(rhs)
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            contains_host_or_source_call(condition)
                || contains_host_or_source_call(then_expr)
                || contains_host_or_source_call(else_expr)
        }
        RuntimeExpr::IfLet {
            expr,
            guard,
            then_expr,
            else_expr,
            ..
        } => {
            contains_host_or_source_call(expr)
                || guard.as_deref().is_some_and(contains_host_or_source_call)
                || contains_host_or_source_call(then_expr)
                || contains_host_or_source_call(else_expr)
        }
        RuntimeExpr::Match { scrutinee, arms } => {
            contains_host_or_source_call(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(contains_host_or_source_call)
                        || contains_host_or_source_call(&arm.value)
                })
        }
        RuntimeExpr::Value(_) | RuntimeExpr::Local(_) | RuntimeExpr::EntityRef(_) => false,
    }
}
