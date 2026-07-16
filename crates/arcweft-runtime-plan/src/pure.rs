//! Pure helper extraction for VM/AOT/JIT conformance checks.

use crate::expr::lower_runtime_expr_strict;
use arcweft_core::{
    plan::{
        RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin, RuntimePureInputType,
        RuntimePureOutputType,
    },
    pure::PureFunctionRequest,
    value::{RuntimeBinding, RuntimeExpr, RuntimeValue},
};
use arcweft_lang_hir::{
    model::{HirFunction, HirModule},
    symbol::CallableDeclarationId,
    syntax::{
        ast::{flow::Stmt, items::FunctionKind, pattern::Pattern},
        expr::Expr,
        types::{FnParam, TypeRef},
    },
};
use thiserror::Error;

/// Runtime-ready pure helper candidate lowered from a checked HIR function.
#[derive(Clone, Debug, PartialEq)]
pub struct PureHelperCandidate {
    module: Option<arcweft_lang_hir::syntax::ast::module_path::CanonicalModulePath>,
    name: String,
    input_names: Vec<String>,
    input_types: Vec<RuntimePureInputType>,
    output_type: RuntimePureOutputType,
    expr: RuntimeExpr,
    shape: PureHelperShape,
    origin: RuntimePureHelperOrigin,
}

/// Lowered pure helper candidates plus discovery counters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PureHelperCandidateReport {
    pub candidates: Vec<PureHelperCandidate>,
    pub stats: PureHelperCandidateStats,
}

/// Counters for pure-helper candidate discovery and expression lowering.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PureHelperCandidateStats {
    pub functions_seen: usize,
    pub lower_attempts: usize,
    pub lower_failures_inferred: usize,
    pub expr_lowered_nodes: usize,
}

/// Shape summary reused by runtime-plan lowering and backend selection.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PureHelperShape {
    pub input_arity: usize,
    pub supports_scalar_eval: bool,
    pub contains_call: bool,
    pub contains_branch: bool,
    pub expr_weight: usize,
}

/// Error produced while selecting or lowering a pure helper function.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum PureHelperLowerError {
    #[error("pure helper `{name}` uses unsupported function kind `{kind:?}`")]
    UnsupportedFunctionKind { name: String, kind: FunctionKind },
    #[error("pure helper `{name}` must have a final value expression")]
    UnsupportedBody { name: String },
    #[error("pure helper `{name}` has unsupported statement `{statement}`")]
    UnsupportedStatement { name: String, statement: String },
    #[error("pure helper `{name}` has unsupported parameter `{parameter}`")]
    UnsupportedParameter { name: String, parameter: String },
    #[error("pure helper `{name}` has unsupported parameter type `{parameter}`")]
    UnsupportedParameterType { name: String, parameter: String },
    #[error(
        "selected entry callable `{declaration}` resolved to {matches} ordinary functions during runtime lowering"
    )]
    EntryCallableCardinality { declaration: String, matches: usize },
    #[error("pure helper `{name}` has unsupported expression: {reason}")]
    UnsupportedExpr { name: String, reason: String },
}

impl PureHelperCandidate {
    /// Canonical source module retained for checked entry-role projection.
    pub const fn module(
        &self,
    ) -> Option<&arcweft_lang_hir::syntax::ast::module_path::CanonicalModulePath> {
        self.module.as_ref()
    }

    /// Function name from the source signature.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Local binding names used as runtime inputs.
    pub fn input_names(&self) -> &[String] {
        &self.input_names
    }

    /// Runtime ABI input types preserved from the source signature.
    pub fn input_types(&self) -> &[RuntimePureInputType] {
        &self.input_types
    }

    /// Runtime ABI output type preserved from the source signature.
    pub const fn output_type(&self) -> RuntimePureOutputType {
        self.output_type
    }

    /// Runtime expression body used by pure helper backends.
    pub const fn expr(&self) -> &RuntimeExpr {
        &self.expr
    }

    /// Cached expression shape used to avoid repeated body scans.
    pub const fn shape(&self) -> PureHelperShape {
        self.shape
    }

    /// Whether this helper was explicitly annotated or inferred from a pure body.
    pub const fn origin(&self) -> RuntimePureHelperOrigin {
        self.origin
    }

    /// Converts this compiler-side candidate into a runtime-ready helper.
    pub fn to_runtime_helper(&self, id: RuntimePureHelperId) -> RuntimePureHelper {
        RuntimePureHelper {
            id,
            name: self.name.clone(),
            input_names: self.input_names.clone(),
            input_types: self.input_types.clone(),
            output_type: self.output_type,
            expr: self.expr.clone(),
            scalar_eval_supported: self.shape.supports_scalar_eval,
            origin: self.origin,
        }
    }

    /// Builds a concrete VM/JIT request using integer input values.
    pub fn request_with_i64_inputs(
        &self,
        values: impl IntoIterator<Item = i64>,
    ) -> Result<PureFunctionRequest, PureHelperLowerError> {
        let values = values.into_iter().collect::<Vec<_>>();
        if values.len() != self.input_names.len() {
            return Err(PureHelperLowerError::UnsupportedParameter {
                name: self.name.clone(),
                parameter: format!(
                    "expected {} input value(s), got {}",
                    self.input_names.len(),
                    values.len()
                ),
            });
        }
        if !self
            .input_types
            .iter()
            .all(|ty| matches!(ty, RuntimePureInputType::I64))
        {
            return Err(PureHelperLowerError::UnsupportedParameterType {
                name: self.name.clone(),
                parameter: "request_with_i64_inputs requires i64 helper inputs".to_owned(),
            });
        }
        Ok(PureFunctionRequest::new(
            self.name.clone(),
            self.expr.clone(),
            self.input_names
                .iter()
                .cloned()
                .zip(values)
                .map(|(name, value)| RuntimeBinding {
                    name,
                    value: RuntimeValue::i64(value),
                }),
        ))
    }
}

/// Lowers all `#[pure] fn` declarations that fit the executable helper subset.
pub fn lower_pure_helper_candidates(
    module: &HirModule,
) -> Result<PureHelperCandidateReport, Vec<PureHelperLowerError>> {
    lower_pure_helper_candidates_for_entry_callables(module, &[])
}

/// Lowers ordinary inferred helpers plus the exact entry-bound callables that
/// require opaque runtime values for nominal state/event boundaries.
pub(crate) fn lower_pure_helper_candidates_for_entry_callables(
    module: &HirModule,
    entry_callables: &[CallableDeclarationId],
) -> Result<PureHelperCandidateReport, Vec<PureHelperLowerError>> {
    let mut stats = PureHelperCandidateStats::default();
    let mut candidates = Vec::new();
    let mut errors = Vec::new();
    for declaration in entry_callables {
        let matches = module
            .functions()
            .iter()
            .filter(|function| {
                CallableDeclarationId::for_function(declaration.package(), function)
                    .is_ok_and(|candidate| candidate == *declaration)
            })
            .count();
        if matches != 1 {
            errors.push(PureHelperLowerError::EntryCallableCardinality {
                declaration: declaration.to_string(),
                matches,
            });
        }
    }
    for function in module.functions() {
        stats.functions_seen += 1;
        stats.lower_attempts += 1;
        let annotated = function.has_attribute("pure");
        let entry_callable = entry_callables.iter().any(|declaration| {
            CallableDeclarationId::for_function(declaration.package(), function)
                .is_ok_and(|candidate| candidate == *declaration)
        });
        let origin = if annotated {
            RuntimePureHelperOrigin::Annotated
        } else {
            RuntimePureHelperOrigin::Inferred
        };
        let lowered = if entry_callable {
            lower_pure_helper_candidate_with_input_policy(
                function,
                origin,
                PureHelperInputPolicy::EntryOpaqueValues,
            )
        } else {
            lower_pure_helper_candidate(function, origin)
        };
        match lowered {
            Ok(candidate) => {
                stats.expr_lowered_nodes += candidate.shape().expr_weight;
                candidates.push(candidate);
            }
            Err(error) if annotated || entry_callable => errors.push(error),
            Err(_) => stats.lower_failures_inferred += 1,
        }
    }
    if errors.is_empty() {
        Ok(PureHelperCandidateReport { candidates, stats })
    } else {
        Err(errors)
    }
}

pub fn lower_pure_helper_candidate(
    function: &HirFunction,
    origin: RuntimePureHelperOrigin,
) -> Result<PureHelperCandidate, PureHelperLowerError> {
    lower_pure_helper_candidate_with_input_policy(
        function,
        origin,
        PureHelperInputPolicy::ScalarOnly,
    )
}

#[derive(Clone, Copy)]
enum PureHelperInputPolicy {
    ScalarOnly,
    EntryOpaqueValues,
}

fn lower_pure_helper_candidate_with_input_policy(
    function: &HirFunction,
    origin: RuntimePureHelperOrigin,
    input_policy: PureHelperInputPolicy,
) -> Result<PureHelperCandidate, PureHelperLowerError> {
    if function.kind() != FunctionKind::Function {
        return Err(PureHelperLowerError::UnsupportedFunctionKind {
            name: function.name().to_owned(),
            kind: function.kind(),
        });
    }
    let inputs = pure_helper_inputs(function, input_policy)?;
    let (input_names, input_types): (Vec<_>, Vec<_>) = inputs.into_iter().unzip();
    let expr = lower_pure_helper_body(function)?;
    let shape = pure_helper_shape(&expr, input_names.len());
    Ok(PureHelperCandidate {
        module: function.module_path().cloned(),
        name: function.name().to_owned(),
        input_names,
        input_types,
        output_type: pure_helper_output_type(function.signature().return_type()),
        expr,
        shape,
        origin,
    })
}

fn pure_helper_shape(expr: &RuntimeExpr, input_arity: usize) -> PureHelperShape {
    let mut shape = summarize_runtime_expr(expr);
    shape.input_arity = input_arity;
    shape.supports_scalar_eval = expr.supports_scalar_pure_eval();
    shape
}

fn summarize_runtime_expr(expr: &RuntimeExpr) -> PureHelperShape {
    match expr {
        RuntimeExpr::Let { expr, body, .. } => {
            merge_shape_summaries([summarize_runtime_expr(expr), summarize_runtime_expr(body)])
        }
        RuntimeExpr::AssignField {
            target, expr, body, ..
        } => merge_shape_summaries([
            summarize_runtime_expr(target),
            summarize_runtime_expr(expr),
            summarize_runtime_expr(body),
        ]),
        RuntimeExpr::Tuple(items) | RuntimeExpr::BracketSeq(items) => {
            merge_shape_summaries(items.iter().map(summarize_runtime_expr))
        }
        RuntimeExpr::RepeatSeq { value, .. }
        | RuntimeExpr::Field { target: value, .. }
        | RuntimeExpr::ProjectTuple { target: value, .. }
        | RuntimeExpr::ProjectRecord { target: value, .. }
        | RuntimeExpr::SpreadArg(value)
        | RuntimeExpr::Sum { source: value }
        | RuntimeExpr::Unary { expr: value, .. } => {
            merge_shape_summaries([summarize_runtime_expr(value)])
        }
        RuntimeExpr::Range { start, end, .. } => merge_shape_summaries(
            start
                .as_deref()
                .into_iter()
                .chain(end.as_deref())
                .map(summarize_runtime_expr),
        ),
        RuntimeExpr::Record(fields) => merge_shape_summaries(
            fields
                .iter()
                .map(|field| summarize_runtime_expr(&field.value)),
        ),
        RuntimeExpr::Variant { payload, .. } => {
            merge_shape_summaries(payload.as_deref().map(summarize_runtime_expr))
        }
        RuntimeExpr::Call { args, .. } | RuntimeExpr::PureCall { args, .. } => {
            let mut shape = merge_shape_summaries(args.iter().map(summarize_runtime_expr));
            shape.contains_call = true;
            shape
        }
        RuntimeExpr::Function { body, .. } => summarize_runtime_expr(body),
        RuntimeExpr::Apply { callee, args } => {
            let mut shape = merge_shape_summaries(
                std::iter::once(callee.as_ref())
                    .chain(args.iter())
                    .map(summarize_runtime_expr),
            );
            shape.contains_call = true;
            shape
        }
        RuntimeExpr::MethodCall { receiver, args, .. }
        | RuntimeExpr::TraitCall { receiver, args, .. } => {
            let mut shape = merge_shape_summaries(
                std::iter::once(receiver.as_ref())
                    .chain(args.iter())
                    .map(summarize_runtime_expr),
            );
            shape.contains_call = true;
            shape
        }
        RuntimeExpr::Map { source, body, .. }
        | RuntimeExpr::Filter { source, body, .. }
        | RuntimeExpr::Binary {
            lhs: source,
            rhs: body,
            ..
        } => merge_shape_summaries([summarize_runtime_expr(source), summarize_runtime_expr(body)]),
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => summarize_if_expr(condition, then_expr, else_expr),
        RuntimeExpr::IfLet {
            expr,
            guard,
            then_expr,
            else_expr,
            ..
        } => {
            let mut shape = merge_shape_summaries(
                std::iter::once(expr.as_ref())
                    .chain(guard.as_deref())
                    .chain([then_expr.as_ref(), else_expr.as_ref()])
                    .map(summarize_runtime_expr),
            );
            shape.contains_branch = true;
            shape
        }
        RuntimeExpr::Match { scrutinee, arms } => summarize_match_expr(scrutinee, arms),
        RuntimeExpr::Value(_) | RuntimeExpr::Local(_) | RuntimeExpr::EntityRef(_) => {
            single_runtime_expr_shape()
        }
    }
}

fn summarize_if_expr(
    condition: &RuntimeExpr,
    then_expr: &RuntimeExpr,
    else_expr: &RuntimeExpr,
) -> PureHelperShape {
    let mut shape = merge_shape_summaries([
        summarize_runtime_expr(condition),
        summarize_runtime_expr(then_expr),
        summarize_runtime_expr(else_expr),
    ]);
    shape.contains_branch = true;
    shape
}

fn summarize_match_expr(
    scrutinee: &RuntimeExpr,
    arms: &[arcweft_core::value::RuntimeExprMatchArm],
) -> PureHelperShape {
    let mut shape = merge_shape_summaries(
        std::iter::once(summarize_runtime_expr(scrutinee)).chain(arms.iter().map(|arm| {
            merge_shape_summaries(
                arm.guard
                    .as_ref()
                    .map(summarize_runtime_expr)
                    .into_iter()
                    .chain(std::iter::once(summarize_runtime_expr(&arm.value))),
            )
        })),
    );
    shape.contains_branch = true;
    shape
}

fn single_runtime_expr_shape() -> PureHelperShape {
    PureHelperShape {
        expr_weight: 1,
        ..PureHelperShape::default()
    }
}

fn merge_shape_summaries(summaries: impl IntoIterator<Item = PureHelperShape>) -> PureHelperShape {
    summaries
        .into_iter()
        .fold(single_runtime_expr_shape(), |mut total, shape| {
            total.contains_call |= shape.contains_call;
            total.contains_branch |= shape.contains_branch;
            total.expr_weight += shape.expr_weight;
            total
        })
}

fn lower_pure_helper_body(function: &HirFunction) -> Result<RuntimeExpr, PureHelperLowerError> {
    let name = function.name();
    let (statements, value) = pure_helper_body_parts(function)?;
    let body = lower_runtime_expr_strict(value).map_err(|reason| {
        PureHelperLowerError::UnsupportedExpr {
            name: name.to_owned(),
            reason,
        }
    })?;
    statements.iter().rev().try_fold(body, |body, stmt| {
        lower_pure_helper_let_stmt(name, stmt).map(|(let_name, expr)| RuntimeExpr::Let {
            name: let_name,
            expr: Box::new(expr),
            body: Box::new(body),
        })
    })
}

fn pure_helper_body_parts(
    function: &HirFunction,
) -> Result<(&[Stmt], &Expr), PureHelperLowerError> {
    if let Some(value) = function.value() {
        return Ok((function.statements(), value.expr()));
    }
    let Some((last, statements)) = function.statements().split_last() else {
        return Err(PureHelperLowerError::UnsupportedBody {
            name: function.name().to_owned(),
        });
    };
    match last {
        Stmt::Return { expr: value, .. } => Ok((statements, value)),
        _ => Err(PureHelperLowerError::UnsupportedBody {
            name: function.name().to_owned(),
        }),
    }
}

fn lower_pure_helper_let_stmt(
    function_name: &str,
    stmt: &Stmt,
) -> Result<(String, RuntimeExpr), PureHelperLowerError> {
    let Stmt::Let { pattern, expr, .. } = stmt else {
        return Err(PureHelperLowerError::UnsupportedStatement {
            name: function_name.to_owned(),
            statement: format!("{stmt:?}"),
        });
    };
    let name = binding_pattern_name(function_name, pattern).map_err(|parameter| {
        PureHelperLowerError::UnsupportedStatement {
            name: function_name.to_owned(),
            statement: format!("let {parameter}"),
        }
    })?;
    let expr = lower_runtime_expr_strict(expr).map_err(|reason| {
        PureHelperLowerError::UnsupportedExpr {
            name: function_name.to_owned(),
            reason,
        }
    })?;
    Ok((name, expr))
}

fn pure_helper_inputs(
    function: &HirFunction,
    input_policy: PureHelperInputPolicy,
) -> Result<Vec<(String, RuntimePureInputType)>, PureHelperLowerError> {
    function
        .signature()
        .param_groups()
        .iter()
        .flat_map(arcweft_lang_hir::syntax::types::FnParamGroup::params)
        .map(|param| pure_helper_param(function.name(), param, input_policy))
        .collect()
}

fn pure_helper_param(
    function_name: &str,
    param: &FnParam,
    input_policy: PureHelperInputPolicy,
) -> Result<(String, RuntimePureInputType), PureHelperLowerError> {
    let name = binding_pattern_name(function_name, param.pattern()).map_err(|parameter| {
        PureHelperLowerError::UnsupportedParameter {
            name: function_name.to_owned(),
            parameter,
        }
    })?;
    pure_helper_input_type(param.ty(), input_policy)
        .map(|ty| (name.clone(), ty))
        .ok_or_else(|| PureHelperLowerError::UnsupportedParameterType {
            name: function_name.to_owned(),
            parameter: name,
        })
}

fn binding_pattern_name(function_name: &str, pattern: &Pattern) -> Result<String, String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            Ok(name.clone())
        }
        pattern => Err(format!("{pattern:?} in `{function_name}`")),
    }
}

fn pure_helper_input_type(
    ty: &TypeRef,
    input_policy: PureHelperInputPolicy,
) -> Option<RuntimePureInputType> {
    match ty {
        TypeRef::Path(name) => match name.as_str() {
            "i8" => Some(RuntimePureInputType::I8),
            "i16" => Some(RuntimePureInputType::I16),
            "i32" => Some(RuntimePureInputType::I32),
            "i64" => Some(RuntimePureInputType::I64),
            "i128" => Some(RuntimePureInputType::I128),
            "isize" => Some(RuntimePureInputType::ISize),
            "u8" => Some(RuntimePureInputType::U8),
            "u16" => Some(RuntimePureInputType::U16),
            "u32" => Some(RuntimePureInputType::U32),
            "u64" => Some(RuntimePureInputType::U64),
            "u128" => Some(RuntimePureInputType::U128),
            "usize" => Some(RuntimePureInputType::USize),
            "f32" => Some(RuntimePureInputType::F32),
            "f64" => Some(RuntimePureInputType::F64),
            _ if matches!(input_policy, PureHelperInputPolicy::EntryOpaqueValues) => {
                Some(RuntimePureInputType::Value)
            }
            _ => None,
        },
        TypeRef::Reference(_)
            if matches!(input_policy, PureHelperInputPolicy::EntryOpaqueValues) =>
        {
            Some(RuntimePureInputType::Value)
        }
        _ => None,
    }
}

fn pure_helper_output_type(ty: Option<&TypeRef>) -> RuntimePureOutputType {
    match ty {
        Some(TypeRef::Path(name)) => match name.as_str() {
            "bool" => RuntimePureOutputType::Bool,
            "i8" => RuntimePureOutputType::I8,
            "i16" => RuntimePureOutputType::I16,
            "i32" => RuntimePureOutputType::I32,
            "i64" => RuntimePureOutputType::I64,
            "i128" => RuntimePureOutputType::I128,
            "isize" => RuntimePureOutputType::ISize,
            "u8" => RuntimePureOutputType::U8,
            "u16" => RuntimePureOutputType::U16,
            "u32" => RuntimePureOutputType::U32,
            "u64" => RuntimePureOutputType::U64,
            "u128" => RuntimePureOutputType::U128,
            "usize" => RuntimePureOutputType::USize,
            "f32" => RuntimePureOutputType::F32,
            "f64" => RuntimePureOutputType::F64,
            _ => RuntimePureOutputType::Value,
        },
        _ => RuntimePureOutputType::Value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_lang_hir::lower::lower_to_hir;
    use arcweft_lang_hir::symbol::CallablePackageId;
    use arcweft_lang_hir::syntax::parser::parse_source;

    #[test]
    fn opaque_entry_inputs_do_not_broaden_ordinary_helper_inference() {
        let parsed = parse_source(
            r"
mod game

fn unrelated(value: String) -> String {
    value
}

fn reduce(state: &GameState, event: GameEvent) -> GameEvent {
    event
}
",
        );
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let tree = parsed.into_typed_tree();
        let hir = lower_to_hir(&tree).expect("functions lower to HIR");

        let ordinary =
            lower_pure_helper_candidates(&hir).expect("ordinary helper discovery succeeds");
        assert!(ordinary.candidates.is_empty());

        let package = CallablePackageId::try_new("game").expect("package ID");
        let reducer = CallableDeclarationId::for_function(&package, &hir.functions()[1])
            .expect("reducer declaration ID");
        let selected = lower_pure_helper_candidates_for_entry_callables(&hir, &[reducer])
            .expect("selected entry callable lowers");
        assert_eq!(selected.candidates.len(), 1);
        assert_eq!(selected.candidates[0].name(), "reduce");
        assert_eq!(
            selected.candidates[0].input_types(),
            [RuntimePureInputType::Value, RuntimePureInputType::Value]
        );
    }

    #[test]
    fn lowers_pure_function_candidate_from_hir_attribute() {
        let parsed = parse_source(
            r"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    if base >= 3 { base * add(bonus, 2) } else { 0 }
}
",
        );
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let tree = parsed.into_typed_tree();
        let hir = lower_to_hir(&tree).expect("pure function lowers to HIR");

        assert!(hir.functions()[0].has_attribute("pure"));
        let report =
            lower_pure_helper_candidates(&hir).expect("pure function lowers to helper candidate");
        assert_eq!(report.stats.functions_seen, 1);
        assert_eq!(report.stats.lower_attempts, 1);
        assert_eq!(report.stats.lower_failures_inferred, 0);
        assert!(report.stats.expr_lowered_nodes > 0);
        let candidates = report.candidates;

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name(), "score");
        assert_eq!(candidates[0].input_names(), ["base", "bonus"]);
        assert_eq!(
            candidates[0].input_types(),
            [RuntimePureInputType::I64, RuntimePureInputType::I64]
        );
        assert_eq!(candidates[0].output_type(), RuntimePureOutputType::I64);
        assert_eq!(candidates[0].shape().input_arity, 2);
        assert!(!candidates[0].shape().supports_scalar_eval);
        assert!(candidates[0].shape().contains_branch);
        assert!(candidates[0].shape().contains_call);
        let request = candidates[0]
            .request_with_i64_inputs([3, 4])
            .expect("request builds with matching inputs");
        assert_eq!(request.name, "score");
        assert_eq!(request.bindings.len(), 2);
    }

    #[test]
    fn pure_function_candidate_preserves_non_i64_integer_input_types() {
        let parsed = parse_source(
            r"
#[pure]
fn score(base: i32, bonus: i16) -> i32 {
    base + bonus
}
",
        );
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let tree = parsed.into_typed_tree();
        let hir = lower_to_hir(&tree).expect("pure function lowers to HIR");

        let report =
            lower_pure_helper_candidates(&hir).expect("pure function lowers to helper candidate");
        let candidates = report.candidates;

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].input_names(), ["base", "bonus"]);
        assert_eq!(
            candidates[0].input_types(),
            [RuntimePureInputType::I32, RuntimePureInputType::I16]
        );
        assert_eq!(candidates[0].output_type(), RuntimePureOutputType::I32);
        assert!(matches!(
            candidates[0].request_with_i64_inputs([3, 4]),
            Err(PureHelperLowerError::UnsupportedParameterType { .. })
        ));
    }

    #[test]
    fn pure_function_candidate_preserves_unsigned_integer_input_types() {
        let parsed = parse_source(
            r"
#[pure]
fn pack(byte: u8, index: u32) -> u64 {
    index + byte
}
",
        );
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let tree = parsed.into_typed_tree();
        let hir = lower_to_hir(&tree).expect("pure function lowers to HIR");

        let report =
            lower_pure_helper_candidates(&hir).expect("pure function lowers to helper candidate");
        let candidates = report.candidates;

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].input_names(), ["byte", "index"]);
        assert_eq!(
            candidates[0].input_types(),
            [RuntimePureInputType::U8, RuntimePureInputType::U32]
        );
        assert_eq!(candidates[0].output_type(), RuntimePureOutputType::U64);
    }

    #[test]
    fn pure_function_candidate_preserves_typed_float_input_types() {
        let parsed = parse_source(
            r"
#[pure]
fn blend(base: f32, gain: f32) -> f32 {
    base + gain
}

#[pure]
fn score(base: f64, gain: f64) -> f64 {
    base * gain
}
",
        );
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let tree = parsed.into_typed_tree();
        let hir = lower_to_hir(&tree).expect("pure functions lower to HIR");

        let report =
            lower_pure_helper_candidates(&hir).expect("pure functions lower to helper candidates");
        assert_eq!(report.stats.functions_seen, 2);
        assert_eq!(report.stats.lower_attempts, 2);
        let candidates = report.candidates;

        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates[0].input_types(),
            [RuntimePureInputType::F32, RuntimePureInputType::F32]
        );
        assert_eq!(candidates[0].output_type(), RuntimePureOutputType::F32);
        assert_eq!(
            candidates[1].input_types(),
            [RuntimePureInputType::F64, RuntimePureInputType::F64]
        );
        assert_eq!(candidates[1].output_type(), RuntimePureOutputType::F64);
    }

    #[test]
    fn lowers_simple_statement_body_pure_helper_candidate() {
        let parsed = parse_source(
            r"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    let boosted = add(bonus, 2)
    let weighted = base * boosted
    if base >= 3 { weighted } else { 0 }
}
",
        );
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let tree = parsed.into_typed_tree();
        let hir = lower_to_hir(&tree).expect("pure function lowers to HIR");

        let report =
            lower_pure_helper_candidates(&hir).expect("statement body lowers to helper candidate");
        let candidates = report.candidates;

        assert_eq!(candidates.len(), 1);
        assert!(matches!(
            candidates[0].expr(),
            RuntimeExpr::Let { name, body, .. }
                if name == "boosted" && matches!(body.as_ref(), RuntimeExpr::Let { name, .. } if name == "weighted")
        ));
        let request = candidates[0]
            .request_with_i64_inputs([3, 4])
            .expect("request builds with matching inputs");
        assert_eq!(request.bindings.len(), 2);
    }

    #[test]
    fn lowers_tail_return_pure_helper_candidate() {
        let parsed = parse_source(
            r"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    let boosted = bonus + 2
    return base * boosted
}
",
        );
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let tree = parsed.into_typed_tree();
        let hir = lower_to_hir(&tree).expect("pure function lowers to HIR");

        let report =
            lower_pure_helper_candidates(&hir).expect("tail return lowers to helper candidate");
        let candidates = report.candidates;

        assert_eq!(candidates.len(), 1);
        assert!(matches!(
            candidates[0].expr(),
            RuntimeExpr::Let { name, body, .. }
                if name == "boosted" && matches!(body.as_ref(), RuntimeExpr::Binary { .. })
        ));
    }
}
