//! Pure helper extraction for VM/AOT/JIT conformance checks.

use crate::expr::lower_runtime_expr_strict;
use arcweft_core::{
    plan::RuntimePureHelperOrigin,
    pure::PureFunctionRequest,
    value::{RuntimeBinding, RuntimeExpr, RuntimeValue},
};
use arcweft_lang_hir::{
    model::{HirFunction, HirModule},
    syntax::{
        ast::{flow::Stmt, items::FunctionKind, pattern::Pattern},
        expr::Expr,
        types::{FnParam, TypeRef},
    },
};
use thiserror::Error;

/// Runtime-ready pure helper candidate lowered from a checked HIR function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PureHelperCandidate {
    name: String,
    input_names: Vec<String>,
    expr: RuntimeExpr,
    origin: RuntimePureHelperOrigin,
}

/// Error produced while selecting or lowering a pure helper function.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
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
    #[error("pure helper `{name}` has unsupported expression: {reason}")]
    UnsupportedExpr { name: String, reason: String },
}

impl PureHelperCandidate {
    /// Function name from the source signature.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Local binding names used as runtime inputs.
    pub fn input_names(&self) -> &[String] {
        &self.input_names
    }

    /// Runtime expression body used by pure helper backends.
    pub const fn expr(&self) -> &RuntimeExpr {
        &self.expr
    }

    /// Whether this helper was explicitly annotated or inferred from a pure body.
    pub const fn origin(&self) -> RuntimePureHelperOrigin {
        self.origin
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
        Ok(PureFunctionRequest::new(
            self.name.clone(),
            self.expr.clone(),
            self.input_names
                .iter()
                .cloned()
                .zip(values)
                .map(|(name, value)| RuntimeBinding {
                    name,
                    value: RuntimeValue::Int(value),
                }),
        ))
    }
}

/// Lowers all `#[pure] fn` declarations that fit the executable helper subset.
pub fn lower_pure_helper_candidates(
    module: &HirModule,
) -> Result<Vec<PureHelperCandidate>, Vec<PureHelperLowerError>> {
    let mut candidates = Vec::new();
    let mut errors = Vec::new();
    for function in module.functions() {
        let annotated = function.has_attribute("pure");
        match lower_pure_helper_candidate(
            function,
            if annotated {
                RuntimePureHelperOrigin::Annotated
            } else {
                RuntimePureHelperOrigin::Inferred
            },
        ) {
            Ok(candidate) => candidates.push(candidate),
            Err(error) if annotated => errors.push(error),
            Err(_) => {}
        }
    }
    if errors.is_empty() {
        Ok(candidates)
    } else {
        Err(errors)
    }
}

fn lower_pure_helper_candidate(
    function: &HirFunction,
    origin: RuntimePureHelperOrigin,
) -> Result<PureHelperCandidate, PureHelperLowerError> {
    if function.kind() != FunctionKind::Function {
        return Err(PureHelperLowerError::UnsupportedFunctionKind {
            name: function.name().to_owned(),
            kind: function.kind(),
        });
    }
    let input_names = pure_helper_input_names(function)?;
    let expr = lower_pure_helper_body(function)?;
    Ok(PureHelperCandidate {
        name: function.name().to_owned(),
        input_names,
        expr,
        origin,
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
        return Ok((function.statements(), value));
    }
    let Some((last, statements)) = function.statements().split_last() else {
        return Err(PureHelperLowerError::UnsupportedBody {
            name: function.name().to_owned(),
        });
    };
    match last {
        Stmt::Return(value) => Ok((statements, value)),
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

fn pure_helper_input_names(function: &HirFunction) -> Result<Vec<String>, PureHelperLowerError> {
    function
        .signature()
        .param_groups()
        .iter()
        .flat_map(arcweft_lang_hir::syntax::types::FnParamGroup::params)
        .map(|param| pure_helper_param_name(function.name(), param))
        .collect()
}

fn pure_helper_param_name(
    function_name: &str,
    param: &FnParam,
) -> Result<String, PureHelperLowerError> {
    let name = binding_pattern_name(function_name, param.pattern()).map_err(|parameter| {
        PureHelperLowerError::UnsupportedParameter {
            name: function_name.to_owned(),
            parameter,
        }
    })?;
    if !is_jit_integer_type(param.ty()) {
        return Err(PureHelperLowerError::UnsupportedParameterType {
            name: function_name.to_owned(),
            parameter: name,
        });
    }
    Ok(name)
}

fn binding_pattern_name(function_name: &str, pattern: &Pattern) -> Result<String, String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            Ok(name.clone())
        }
        pattern => Err(format!("{pattern:?} in `{function_name}`")),
    }
}

fn is_jit_integer_type(ty: &TypeRef) -> bool {
    matches!(
        ty,
        TypeRef::Path(name)
            if matches!(name.as_str(), "i8" | "i16" | "i32" | "i64" | "isize")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_lang_hir::lower::lower_to_hir;
    use arcweft_lang_hir::syntax::parser::parse_source;

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
        let candidates =
            lower_pure_helper_candidates(&hir).expect("pure function lowers to helper candidate");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name(), "score");
        assert_eq!(candidates[0].input_names(), ["base", "bonus"]);
        let request = candidates[0]
            .request_with_i64_inputs([3, 4])
            .expect("request builds with matching inputs");
        assert_eq!(request.name, "score");
        assert_eq!(request.bindings.len(), 2);
    }

    #[test]
    fn pure_function_candidate_rejects_removed_int_alias() {
        let parsed = parse_source(
            r"
#[pure]
fn score(base: Int) -> i64 {
    base
}
",
        );
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let tree = parsed.into_typed_tree();
        let hir = lower_to_hir(&tree).expect("pure function lowers to HIR");
        let errors = lower_pure_helper_candidates(&hir)
            .expect_err("removed Int alias is rejected")
            .into_iter()
            .collect::<Vec<_>>();
        assert!(
            errors
                .iter()
                .any(|err| matches!(err, PureHelperLowerError::UnsupportedParameterType { .. }))
        );
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

        let candidates =
            lower_pure_helper_candidates(&hir).expect("statement body lowers to helper candidate");

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

        let candidates =
            lower_pure_helper_candidates(&hir).expect("tail return lowers to helper candidate");

        assert_eq!(candidates.len(), 1);
        assert!(matches!(
            candidates[0].expr(),
            RuntimeExpr::Let { name, body, .. }
                if name == "boosted" && matches!(body.as_ref(), RuntimeExpr::Binary { .. })
        ));
    }
}
