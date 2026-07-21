//! Runtime function-value extraction for source-local top-level `fn` bodies.

use crate::expr::{
    RuntimePureHelperLookup, lower_runtime_expr_strict_with_function_locals_and_pure,
};
use arcweft_core::value::RuntimeExpr;
use arcweft_lang_hir::{
    model::{HirFunction, HirModule},
    syntax::{
        ast::{
            flow::Stmt,
            items::FunctionKind,
            pattern::{Pattern, VariantPatternPayload},
        },
        expr::{CallArg, CallExpr, ClosureParam, Expr, MatchExprArm},
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
    pure_helpers: RuntimePureHelperLookup<'_, 'static, 'static>,
) -> Vec<RuntimeFunctionValueCandidate> {
    let mut candidates = BTreeMap::new();
    loop {
        let before = candidates.len();
        for function in module.functions() {
            if candidates.contains_key(function.name()) {
                continue;
            }
            let candidate = {
                let lookup = pure_helpers.with_runtime_function_values(&candidates);
                lower_runtime_function_value_candidate(function, lookup)
            };
            if let Some(candidate) = candidate {
                candidates.insert(candidate.name.clone(), candidate);
            }
        }
        if candidates.len() == before {
            break;
        }
    }
    module
        .functions()
        .iter()
        .filter_map(|function| candidates.remove(function.name()))
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

#[derive(Clone)]
struct RuntimeFunctionValueContext<'helpers, 'functions> {
    function_locals: BTreeMap<String, FunctionLocalSignature>,
    pure_helpers: RuntimePureHelperLookup<'helpers, 'functions, 'static>,
    pipe_binding_depth: u32,
}

#[derive(Clone, Debug)]
struct FunctionLocalSignature {
    arity: usize,
    return_type: Option<TypeRef>,
}

impl FunctionLocalSignature {
    fn from_input_names(input_names: &[String]) -> Self {
        Self {
            arity: input_names.len(),
            return_type: None,
        }
    }
}

impl RuntimeFunctionValueContext<'_, '_> {
    fn function_local_arity(&self, name: &str) -> Option<usize> {
        self.function_locals
            .get(name)
            .map(|signature| signature.arity)
    }

    fn function_local_signature(&self, name: &str) -> Option<&FunctionLocalSignature> {
        self.function_locals.get(name)
    }

    fn top_level_function_signature(&self, name: &str) -> Option<FunctionLocalSignature> {
        self.pure_helpers
            .pure_helper_input_names(name)
            .map(FunctionLocalSignature::from_input_names)
            .or_else(|| {
                self.pure_helpers
                    .function_value_candidate(name)
                    .map(|candidate| {
                        FunctionLocalSignature::from_input_names(candidate.input_names())
                    })
            })
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
    pure_helpers: RuntimePureHelperLookup<'_, '_, 'static>,
) -> Option<RuntimeFunctionValueCandidate> {
    if function.kind() != FunctionKind::Function {
        return None;
    }
    let param_groups = runtime_function_value_param_groups(function)?;
    let context = runtime_function_value_context(function, pure_helpers);
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

fn runtime_function_value_context<'helpers, 'functions>(
    function: &HirFunction,
    pure_helpers: RuntimePureHelperLookup<'helpers, 'functions, 'static>,
) -> RuntimeFunctionValueContext<'helpers, 'functions> {
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
    RuntimeFunctionValueContext {
        function_locals,
        pure_helpers,
        pipe_binding_depth: 0,
    }
}

fn function_local_signature_from_type(ty: &TypeRef) -> Option<FunctionLocalSignature> {
    match ty {
        TypeRef::Function {
            params,
            return_type,
            ..
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

fn runtime_function_value_supported_context<'helpers, 'functions>(
    mut context: RuntimeFunctionValueContext<'helpers, 'functions>,
    statements: &[Stmt],
    value: &Expr,
) -> Option<RuntimeFunctionValueContext<'helpers, 'functions>> {
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
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> Option<RuntimeExpr> {
    let function_local_arities = context.function_local_arities();
    let body = lower_runtime_expr_strict_with_function_locals_and_pure(
        value,
        &function_local_arities,
        context.pure_helpers,
    )
    .ok()?;
    statements.iter().rev().try_fold(body, |body, stmt| {
        let Stmt::Let { pattern, expr, .. } = stmt else {
            return None;
        };
        let name = binding_pattern_name(pattern)?;
        let expr = lower_runtime_expr_strict_with_function_locals_and_pure(
            expr,
            &function_local_arities,
            context.pure_helpers,
        )
        .ok()?;
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

fn pattern_binds_function_local(
    pattern: &Pattern,
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
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
    context: &RuntimeFunctionValueContext<'_, '_>,
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
    context: &RuntimeFunctionValueContext<'_, '_>,
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
    context: &RuntimeFunctionValueContext<'_, '_>,
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
        Expr::Call(call) => runtime_function_value_call_supported(call, context),
        Expr::Pipe { lhs, rhs } => runtime_function_value_pipe_supported(lhs, rhs, context),
        Expr::Placeholder(arcweft_lang_hir::syntax::expr::Placeholder::PipeLeft) => {
            context.pipe_binding_depth > 0
        }
        Expr::LifetimePath { .. }
        | Expr::Borrow(_)
        | Expr::Deref(_)
        | Expr::Placeholder(arcweft_lang_hir::syntax::expr::Placeholder::Partial)
        | Expr::DialogueCall { .. }
        | Expr::Try(_)
        | Expr::Await(_)
        | Expr::Thread { .. }
        | Expr::Raw(_) => false,
        Expr::Closure { params, body, .. } => {
            runtime_function_value_closure_supported(params, body, context)
        }
    }
}

fn runtime_function_value_call_supported(
    call: &CallExpr,
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
    runtime_function_value_local_function_call_supported(call.callee(), call.args(), context)
        || runtime_function_value_pure_helper_call_supported(call.callee(), call.args(), context)
        || runtime_function_value_source_function_call_supported(
            call.callee(),
            call.args(),
            context,
        )
}

fn runtime_function_value_closure_supported(
    params: &[ClosureParam],
    body: &Expr,
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
    params
        .iter()
        .all(|param| !pattern_binds_function_local(param.pattern(), context))
        && runtime_function_value_expr_supported(body, context)
}

fn runtime_function_value_local_function_call_supported(
    callee: &Expr,
    args: &[CallArg],
    context: &RuntimeFunctionValueContext<'_, '_>,
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

fn runtime_function_value_pure_helper_call_supported(
    callee: &Expr,
    args: &[CallArg],
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
    let Expr::Path(path) = callee else {
        return false;
    };
    context
        .pure_helpers
        .pure_helper_input_names(path.as_label())
        .is_some_and(|input_names| {
            runtime_function_value_exact_callable_args_supported(args, input_names, context)
        })
}

fn runtime_function_value_source_function_call_supported(
    callee: &Expr,
    args: &[CallArg],
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
    let Expr::Path(path) = callee else {
        return false;
    };
    context
        .pure_helpers
        .function_value_candidate(path.as_label())
        .is_some_and(|candidate| {
            runtime_function_value_exact_callable_args_supported(
                args,
                candidate.input_names(),
                context,
            )
        })
}

fn runtime_function_value_pipe_supported(
    lhs: &Expr,
    rhs: &Expr,
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
    if rhs.contains_pipe_left() {
        if !runtime_function_value_expr_supported(lhs, context) {
            return false;
        }
        let mut scoped_context = context.clone();
        scoped_context.pipe_binding_depth = scoped_context.pipe_binding_depth.saturating_add(1);
        return runtime_function_value_expr_supported(rhs, &scoped_context);
    }
    runtime_function_value_expr_supported(lhs, context)
        && runtime_function_value_data_last_pipe_rhs_supported(lhs, rhs, context)
}

fn runtime_function_value_data_last_pipe_rhs_supported(
    lhs: &Expr,
    rhs: &Expr,
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
    match rhs {
        Expr::Path(path) => runtime_function_value_data_last_callable_supported(
            path.as_label(),
            &[CallArg::Positional(lhs.clone())],
            context,
        ),
        Expr::Call(call) => {
            let Expr::Path(path) = call.callee() else {
                return false;
            };
            let mut pipe_args = call.args().to_vec();
            pipe_args.push(CallArg::Positional(lhs.clone()));
            runtime_function_value_data_last_callable_supported(
                path.as_label(),
                &pipe_args,
                context,
            )
        }
        _ => false,
    }
}

fn runtime_function_value_data_last_callable_supported(
    callee: &str,
    args: &[CallArg],
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
    if context.function_local_arity(callee).is_some() {
        return runtime_function_value_local_callable_args_supported(callee, args, context);
    }
    if let Some(input_names) = context.pure_helpers.pure_helper_input_names(callee) {
        return runtime_function_value_callable_args_supported(args, input_names, false, context);
    }
    context
        .pure_helpers
        .function_value_candidate(callee)
        .is_some_and(|candidate| {
            runtime_function_value_callable_args_supported(
                args,
                candidate.input_names(),
                false,
                context,
            )
        })
}

fn runtime_function_value_exact_callable_args_supported(
    args: &[CallArg],
    input_names: &[String],
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
    runtime_function_value_callable_args_supported(args, input_names, true, context)
}

fn runtime_function_value_callable_args_supported(
    args: &[CallArg],
    input_names: &[String],
    require_exact: bool,
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
    let mut filled = vec![false; input_names.len()];
    let mut positional_index = 0usize;

    for arg in args {
        match arg {
            CallArg::Positional(value) => {
                while positional_index < filled.len() && filled[positional_index] {
                    positional_index += 1;
                }
                let Some(slot) = filled.get_mut(positional_index) else {
                    return false;
                };
                if !runtime_function_value_expr_supported(value, context) {
                    return false;
                }
                *slot = true;
                positional_index += 1;
            }
            CallArg::Named { name, value } => {
                let Some(index) = input_names.iter().position(|input| input == name) else {
                    return false;
                };
                if filled[index] || !runtime_function_value_expr_supported(value, context) {
                    return false;
                }
                filled[index] = true;
            }
            CallArg::Spread { .. } => return false,
        }
    }

    !require_exact || filled.into_iter().all(std::convert::identity)
}

fn runtime_function_value_local_callable_args_supported(
    callee: &str,
    args: &[CallArg],
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
    context
        .function_local_arity(callee)
        .is_some_and(|arity| arity >= args.len())
        && args.iter().all(|arg| {
            !arg.is_spread()
                && arg.name().is_none()
                && runtime_function_value_expr_supported(arg.value(), context)
        })
}

fn runtime_function_value_expr_function_signature(
    expr: &Expr,
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> Option<FunctionLocalSignature> {
    match expr {
        Expr::Path(path) => context
            .function_local_signature(path.as_label())
            .cloned()
            .or_else(|| context.top_level_function_signature(path.as_label())),
        Expr::Closure {
            params,
            return_type,
            body,
        } => runtime_function_value_closure_supported(params, body, context)
            .then(|| function_local_signature_from_closure(params, return_type.as_ref())),
        Expr::Call(call)
            if runtime_function_value_local_function_call_supported(
                call.callee(),
                call.args(),
                context,
            ) =>
        {
            let Expr::Path(path) = call.callee() else {
                return None;
            };
            context
                .function_local_signature(path.as_label())
                .and_then(|signature| function_local_result_signature(signature, call.args().len()))
        }
        Expr::Pipe { lhs, rhs } => {
            runtime_function_value_pipe_function_signature(lhs, rhs, context)
        }
        _ => None,
    }
}

fn runtime_function_value_pipe_function_signature(
    lhs: &Expr,
    rhs: &Expr,
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> Option<FunctionLocalSignature> {
    if rhs.contains_pipe_left() {
        if !runtime_function_value_expr_supported(lhs, context) {
            return None;
        }
        let mut scoped_context = context.clone();
        scoped_context.pipe_binding_depth = scoped_context.pipe_binding_depth.saturating_add(1);
        return runtime_function_value_expr_function_signature(rhs, &scoped_context);
    }
    if !runtime_function_value_expr_supported(lhs, context) {
        return None;
    }
    match rhs {
        Expr::Path(path) => runtime_function_value_data_last_signature(
            path.as_label(),
            &[CallArg::Positional(lhs.clone())],
            context,
        ),
        Expr::Call(call) => {
            let Expr::Path(path) = call.callee() else {
                return None;
            };
            let mut pipe_args = call.args().to_vec();
            pipe_args.push(CallArg::Positional(lhs.clone()));
            runtime_function_value_data_last_signature(path.as_label(), &pipe_args, context)
        }
        _ => None,
    }
}

fn runtime_function_value_data_last_signature(
    callee: &str,
    args: &[CallArg],
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> Option<FunctionLocalSignature> {
    if let Some(signature) = context.function_local_signature(callee) {
        return runtime_function_value_local_callable_args_supported(callee, args, context)
            .then(|| function_local_result_signature(signature, args.len()))
            .flatten();
    }
    if let Some(input_names) = context.pure_helpers.pure_helper_input_names(callee) {
        return runtime_function_value_partial_signature(args, input_names, context);
    }
    context
        .pure_helpers
        .function_value_candidate(callee)
        .and_then(|candidate| {
            runtime_function_value_partial_signature(args, candidate.input_names(), context)
        })
}

fn runtime_function_value_partial_signature(
    args: &[CallArg],
    input_names: &[String],
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> Option<FunctionLocalSignature> {
    let missing = runtime_function_value_missing_callable_inputs(args, input_names, context)?;
    (!missing.is_empty()).then_some(FunctionLocalSignature {
        arity: missing.len(),
        return_type: None,
    })
}

fn runtime_function_value_missing_callable_inputs(
    args: &[CallArg],
    input_names: &[String],
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> Option<Vec<String>> {
    let mut filled = vec![false; input_names.len()];
    let mut positional_index = 0usize;

    for arg in args {
        match arg {
            CallArg::Positional(value) => {
                while positional_index < filled.len() && filled[positional_index] {
                    positional_index += 1;
                }
                let slot = filled.get_mut(positional_index)?;
                if !runtime_function_value_expr_supported(value, context) {
                    return None;
                }
                *slot = true;
                positional_index += 1;
            }
            CallArg::Named { name, value } => {
                let index = input_names.iter().position(|input| input == name)?;
                if filled[index] || !runtime_function_value_expr_supported(value, context) {
                    return None;
                }
                filled[index] = true;
            }
            CallArg::Spread { .. } => return None,
        }
    }

    Some(
        input_names
            .iter()
            .zip(filled)
            .filter(|(_, filled)| !filled)
            .map(|(name, _)| name.clone())
            .collect(),
    )
}

fn runtime_function_value_block_supported(
    statements: &[Stmt],
    value: Option<&Expr>,
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
    statements
        .iter()
        .all(|stmt| runtime_function_value_statement_supported(stmt, context))
        && value.is_some_and(|value| runtime_function_value_expr_supported(value, context))
}

fn runtime_function_value_if_supported(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    context: &RuntimeFunctionValueContext<'_, '_>,
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
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
    !pattern_binds_function_local(pattern, context)
        && runtime_function_value_expr_supported(expr, context)
        && guard.is_none_or(|guard| runtime_function_value_expr_supported(guard, context))
        && runtime_function_value_expr_supported(then_branch, context)
        && else_branch.is_some_and(|expr| runtime_function_value_expr_supported(expr, context))
}

fn runtime_function_value_match_arm_supported(
    arm: &MatchExprArm,
    context: &RuntimeFunctionValueContext<'_, '_>,
) -> bool {
    !pattern_binds_function_local(arm.pattern(), context)
        && arm
            .guard()
            .is_none_or(|guard| runtime_function_value_expr_supported(guard, context))
        && runtime_function_value_expr_supported(arm.value(), context)
}
