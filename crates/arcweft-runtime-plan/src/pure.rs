//! Pure helper extraction for VM/AOT/JIT conformance checks.

use crate::expr::lower_runtime_expr_strict;
use arcweft_core::{
    pure::PureFunctionRequest,
    value::{RuntimeBinding, RuntimeExpr, RuntimeValue},
};
use arcweft_lang_hir::{
    model::{HirFunction, HirModule},
    syntax::{
        ast::{items::FunctionKind, pattern::Pattern},
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
}

/// Error produced while selecting or lowering a pure helper function.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PureHelperLowerError {
    #[error("pure helper `{name}` uses unsupported function kind `{kind:?}`")]
    UnsupportedFunctionKind { name: String, kind: FunctionKind },
    #[error("pure helper `{name}` must have a single expression body")]
    UnsupportedBody { name: String },
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
    let (candidates, errors): (Vec<_>, Vec<_>) = module
        .functions()
        .iter()
        .filter(|function| function.has_attribute("pure"))
        .map(lower_pure_helper_candidate)
        .partition(Result::is_ok);
    let errors = errors
        .into_iter()
        .filter_map(Result::err)
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(candidates.into_iter().filter_map(Result::ok).collect())
    } else {
        Err(errors)
    }
}

fn lower_pure_helper_candidate(
    function: &HirFunction,
) -> Result<PureHelperCandidate, PureHelperLowerError> {
    if function.kind() != FunctionKind::Function {
        return Err(PureHelperLowerError::UnsupportedFunctionKind {
            name: function.name().to_owned(),
            kind: function.kind(),
        });
    }
    if !function.statements().is_empty() {
        return Err(PureHelperLowerError::UnsupportedBody {
            name: function.name().to_owned(),
        });
    }
    let Some(value) = function.value() else {
        return Err(PureHelperLowerError::UnsupportedBody {
            name: function.name().to_owned(),
        });
    };
    let input_names = pure_helper_input_names(function)?;
    let expr = lower_runtime_expr_strict(value).map_err(|reason| {
        PureHelperLowerError::UnsupportedExpr {
            name: function.name().to_owned(),
            reason,
        }
    })?;
    Ok(PureHelperCandidate {
        name: function.name().to_owned(),
        input_names,
        expr,
    })
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
    let name = match param.pattern() {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            name.clone()
        }
        pattern => {
            return Err(PureHelperLowerError::UnsupportedParameter {
                name: function_name.to_owned(),
                parameter: format!("{pattern:?}"),
            });
        }
    };
    if !is_jit_integer_type(param.ty()) {
        return Err(PureHelperLowerError::UnsupportedParameterType {
            name: function_name.to_owned(),
            parameter: name,
        });
    }
    Ok(name)
}

fn is_jit_integer_type(ty: &TypeRef) -> bool {
    matches!(
        ty,
        TypeRef::Path(name)
            if matches!(name.as_str(), "i8" | "i16" | "i32" | "i64" | "isize" | "Int")
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
    fn rejects_statement_body_pure_helper_candidate() {
        let parsed = parse_source(
            r"
#[pure]
fn score(base: i64) -> i64 {
    let doubled = base * 2
    doubled
}
",
        );
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let tree = parsed.into_typed_tree();
        let hir = lower_to_hir(&tree).expect("pure function lowers to HIR");

        let errors = lower_pure_helper_candidates(&hir)
            .expect_err("statement body is outside the executable helper subset");

        assert!(matches!(
            errors.as_slice(),
            [PureHelperLowerError::UnsupportedBody { name }] if name == "score"
        ));
    }
}
