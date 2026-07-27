//! Lowering from ordinary Arcweft function contracts to solver obligations.
//!
//! This first complete cut deliberately targets pure, expression-bodied scalar
//! functions. Each `ensures prove ...` clause is checked as a counterexample
//! query under the function's proof-mode `requires` assumptions and its body relation
//! `result == body`. Proof-mode invariants are rejected until pre/post-state lowering exists.

use crate::{
    BackendKind, ProofDischarge, ProofObligation, ProofObligationKind, Severity, ToolAction,
    VerificationDiagnostic, VerificationMode, VerificationReport,
    smt::{ProofExpr, SmtError, SmtProblem, SmtSort, SmtSymbol, SmtSymbolId},
};
use arcweft_lang_hir::model::{HirFunction, HirModule};
use arcweft_lang_syntax::{
    ast::{flow::ContractClause, module_path::ModulePathRoot},
    expr::{BinaryOp, CallArg, Expr, Literal, UnaryOp},
    types::{FnParamGroup, TypeRef},
};
use std::collections::BTreeMap;
use thiserror::Error;

/// Adds concrete SMT problems for every solver-enabled function postcondition.
pub(crate) fn collect_function_contract_obligations(
    module: &HirModule,
    report: &mut VerificationReport,
) {
    for function in module.functions() {
        let claims = function
            .contracts()
            .iter()
            .filter_map(ContractClause::solver_claim)
            .collect::<Vec<_>>();
        if claims.is_empty() {
            continue;
        }

        match FunctionContractLowerer::new(function) {
            Ok(lowerer) => {
                for (claim_index, claim) in claims.into_iter().enumerate() {
                    match lowerer.problem_for(claim_index, claim) {
                        Ok(problem) => push_problem(report, function, claim_index, problem),
                        Err(error) => {
                            let detail = error.to_string();
                            push_lowering_failure(report, function, claim_index, &detail);
                        }
                    }
                }
            }
            Err(error) => {
                let detail = error.to_string();
                for claim_index in 0..claims.len() {
                    push_lowering_failure(report, function, claim_index, &detail);
                }
            }
        }
    }
}

fn push_problem(
    report: &mut VerificationReport,
    function: &HirFunction,
    claim_index: usize,
    problem: SmtProblem,
) {
    let id = format!("obligation.{:04}", report.obligations.len() + 1);
    let subject = contract_subject(function, claim_index);
    report.obligations.push(ProofObligation {
        id: id.clone(),
        kind: ProofObligationKind::FunctionContract,
        message: format!(
            "function `{}` postcondition #{} must hold for every input satisfying its preconditions",
            function.name(),
            claim_index + 1
        ),
        subject: Some(subject.clone()),
        source: None,
        insertion_target: None,
        discharge: ProofDischarge::Missing,
        smt: Some(problem),
    });

    if report.policy.backend == BackendKind::Emit
        && matches!(
            report.policy.mode,
            VerificationMode::Test | VerificationMode::Release
        )
    {
        report.diagnostics.push(VerificationDiagnostic {
            id: format!("diagnostic.{id}"),
            severity: Severity::Error,
            message: format!(
                "solver-backed contract `{subject}` is pending; select `--backend oxiz` or `--backend z3`"
            ),
            source: None,
            obligation: Some(id),
            related_ids: vec![subject],
            actions: vec![ToolAction::show_obligation()],
        });
    }
}

fn push_lowering_failure(
    report: &mut VerificationReport,
    function: &HirFunction,
    claim_index: usize,
    detail: &str,
) {
    let id = format!("obligation.{:04}", report.obligations.len() + 1);
    let subject = contract_subject(function, claim_index);
    let severity = match report.policy.mode {
        VerificationMode::Dev => Severity::Warning,
        VerificationMode::Test | VerificationMode::Release => Severity::Error,
    };
    let message = format!("cannot lower solver contract `{subject}`: {detail}");
    report.obligations.push(ProofObligation {
        id: id.clone(),
        kind: ProofObligationKind::FunctionContract,
        message: message.clone(),
        subject: Some(subject.clone()),
        source: None,
        insertion_target: None,
        discharge: ProofDischarge::Missing,
        smt: None,
    });
    report.diagnostics.push(VerificationDiagnostic {
        id: format!("diagnostic.{id}"),
        severity,
        message,
        source: None,
        obligation: Some(id),
        related_ids: vec![subject],
        actions: vec![ToolAction::show_obligation()],
    });
}

fn contract_subject(function: &HirFunction, claim_index: usize) -> String {
    format!("function.{}.ensures.{}", function.name(), claim_index + 1)
}

#[derive(Clone, Debug)]
struct FunctionContractLowerer {
    function_name: String,
    symbols: Vec<SmtSymbol>,
    sorts: BTreeMap<SmtSymbolId, SmtSort>,
    assumptions: Vec<ProofExpr>,
    body_relation: ProofExpr,
    model_symbols: Vec<SmtSymbolId>,
}

impl FunctionContractLowerer {
    fn new(function: &HirFunction) -> Result<Self, ContractLoweringError> {
        if function
            .contracts()
            .iter()
            .any(|clause| clause.solver_invariant().is_some())
        {
            return Err(ContractLoweringError::InvariantClause {
                function: function.name().to_owned(),
            });
        }
        if !function.statements().is_empty() {
            return Err(ContractLoweringError::StatementBody {
                function: function.name().to_owned(),
            });
        }
        let body = function
            .value()
            .ok_or_else(|| ContractLoweringError::MissingBodyValue {
                function: function.name().to_owned(),
            })?;
        let return_type = function.signature().return_type().ok_or_else(|| {
            ContractLoweringError::MissingReturnType {
                function: function.name().to_owned(),
            }
        })?;
        let result_sort = SmtSort::from_arcweft_type(return_type.value()).ok_or_else(|| {
            ContractLoweringError::UnsupportedType {
                label: "result".to_owned(),
                ty: format!("{return_type:?}"),
            }
        })?;

        let mut symbols = Vec::new();
        let mut sorts = BTreeMap::new();
        for parameter in function
            .signature()
            .param_groups()
            .iter()
            .flat_map(FnParamGroup::params)
        {
            let name = parameter.pattern().simple_binding_name().ok_or_else(|| {
                ContractLoweringError::UnsupportedParameterPattern {
                    pattern: format!("{:?}", parameter.pattern()),
                }
            })?;
            let ty = parameter
                .ty()
                .ok_or_else(|| ContractLoweringError::UnsupportedType {
                    label: name.to_owned(),
                    ty: "<missing>".to_owned(),
                })?;
            let sort = SmtSort::from_arcweft_type(ty.value()).ok_or_else(|| {
                ContractLoweringError::UnsupportedType {
                    label: name.to_owned(),
                    ty: format!("{:?}", ty.value()),
                }
            })?;
            let id = SmtSymbolId::new(name);
            if sorts.insert(id.clone(), sort).is_some() {
                return Err(ContractLoweringError::DuplicateSymbol(name.to_owned()));
            }
            symbols.push(SmtSymbol::new(id, sort).with_source_label(name));
        }

        let result_id = SmtSymbolId::new("result");
        if sorts.insert(result_id.clone(), result_sort).is_some() {
            return Err(ContractLoweringError::DuplicateSymbol("result".to_owned()));
        }
        symbols.push(SmtSymbol::new(result_id.clone(), result_sort).with_source_label("result"));

        let body = ProofExpr::from_arcweft(body.expr(), &sorts)?;
        require_sort("function body", body.sort, result_sort)?;
        let body_relation = ProofExpr::equal(ProofExpr::var(result_id), body.expr);

        let assumptions = function
            .contracts()
            .iter()
            .filter_map(ContractClause::solver_assumption)
            .map(|expr| ProofExpr::from_arcweft(expr, &sorts))
            .map(|lowered| {
                lowered.and_then(|lowered| {
                    require_sort("contract assumption", lowered.sort, SmtSort::Bool)?;
                    Ok(lowered.expr)
                })
            })
            .collect::<Result<Vec<_>, ContractLoweringError>>()?;
        let model_symbols = symbols.iter().map(|symbol| symbol.id.clone()).collect();

        Ok(Self {
            function_name: function.name().to_owned(),
            symbols,
            sorts,
            assumptions,
            body_relation,
            model_symbols,
        })
    }

    fn problem_for(
        &self,
        claim_index: usize,
        claim: &Expr,
    ) -> Result<SmtProblem, ContractLoweringError> {
        let claim = ProofExpr::from_arcweft(claim, &self.sorts)?;
        require_sort("contract postcondition", claim.sort, SmtSort::Bool)?;
        let problem = SmtProblem::counterexample(
            format!("contract.{}.{}", self.function_name, claim_index + 1),
            self.symbols.clone(),
            self.assumptions
                .iter()
                .cloned()
                .chain(std::iter::once(self.body_relation.clone())),
            claim.expr,
            self.model_symbols.clone(),
        );
        problem.validate().map_err(ContractLoweringError::Smt)?;
        Ok(problem)
    }
}

impl SmtSort {
    /// Maps the scalar Arcweft types supported by solver-backed contracts.
    fn from_arcweft_type(ty: &TypeRef) -> Option<Self> {
        match direct_type_name(ty)? {
            "bool" => Some(Self::Bool),
            "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
            | "u128" | "usize" => Some(Self::Int),
            _ => None,
        }
    }
}

fn direct_type_name(ty: &TypeRef) -> Option<&str> {
    let TypeRef::Path(path) = ty else {
        return None;
    };
    let [name] = path.segments() else {
        return None;
    };
    matches!(path.root(), ModulePathRoot::ImplicitCrate).then(|| name.as_str())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoweredExpr {
    expr: ProofExpr,
    sort: SmtSort,
}

impl LoweredExpr {
    fn new(expr: ProofExpr, sort: SmtSort) -> Self {
        Self { expr, sort }
    }

    fn bool(expr: ProofExpr) -> Self {
        Self::new(expr, SmtSort::Bool)
    }

    fn int(expr: ProofExpr) -> Self {
        Self::new(expr, SmtSort::Int)
    }
}

impl ProofExpr {
    /// Lowers the supported Arcweft expression subset without reparsing source text.
    fn from_arcweft(
        expr: &Expr,
        symbols: &BTreeMap<SmtSymbolId, SmtSort>,
    ) -> Result<LoweredExpr, ContractLoweringError> {
        match expr {
            Expr::Literal(Literal::Bool(value)) => Ok(LoweredExpr::bool(Self::bool(*value))),
            Expr::Literal(Literal::Int(literal)) => literal
                .magnitude()
                .map(|value| LoweredExpr::int(Self::natural(value)))
                .map_err(|_| {
                    ContractLoweringError::UnsupportedExpression(format!(
                        "integer literal `{}` is not a valid u128 magnitude",
                        literal.raw()
                    ))
                }),
            Expr::Path(path) => {
                let id = SmtSymbolId::new(path.as_str());
                let sort = symbols.get(&id).copied().ok_or_else(|| {
                    ContractLoweringError::UnknownSymbol(path.as_label().to_owned())
                })?;
                Ok(LoweredExpr::new(Self::var(id), sort))
            }
            Expr::Unary { op, expr } => {
                let expr = Self::from_arcweft(expr, symbols)?;
                match op {
                    UnaryOp::Not => {
                        require_sort("Boolean negation", expr.sort, SmtSort::Bool)?;
                        Ok(LoweredExpr::bool(Self::negated(expr.expr)))
                    }
                    UnaryOp::Neg => {
                        require_sort("integer negation", expr.sort, SmtSort::Int)?;
                        Ok(LoweredExpr::int(Self::arithmetic_negated(expr.expr)))
                    }
                }
            }
            Expr::Binary { lhs, op, rhs } => Self::from_arcweft_binary(lhs, *op, rhs, symbols),
            Expr::Call(call) if matches!(call.callee(), Expr::Path(path) if path == "old") => {
                let [arg] = call.args() else {
                    return Err(ContractLoweringError::InvalidCall(
                        "old(expr) requires exactly one argument".to_owned(),
                    ));
                };
                Self::from_arcweft(arg.value(), symbols)
            }
            Expr::Call(call) if matches!(selected_callee_method(call.callee()), Some("clamp")) => {
                Self::from_arcweft_clamp(
                    selected_callee_receiver(call.callee()).expect("selected callee has receiver"),
                    call.args(),
                    symbols,
                )
            }
            Expr::Call(call)
                if matches!(selected_callee_method(call.callee()), Some("min" | "max")) =>
            {
                let method =
                    selected_callee_method(call.callee()).expect("selected callee has method");
                Self::from_arcweft_min_max(
                    selected_callee_receiver(call.callee()).expect("selected callee has receiver"),
                    method,
                    call.args(),
                    symbols,
                )
            }
            Expr::If {
                condition,
                then_branch,
                else_branch: Some(else_branch),
            } => {
                let condition = Self::from_arcweft(condition, symbols)?;
                let then_expr = Self::from_arcweft(then_branch, symbols)?;
                let else_expr = Self::from_arcweft(else_branch, symbols)?;
                require_sort("if condition", condition.sort, SmtSort::Bool)?;
                require_same_sort("if branches", then_expr.sort, else_expr.sort)?;
                Ok(LoweredExpr::new(
                    Self::if_then_else(condition.expr, then_expr.expr, else_expr.expr),
                    then_expr.sort,
                ))
            }
            _ => Err(ContractLoweringError::UnsupportedExpression(format!(
                "{expr:?}"
            ))),
        }
    }

    fn from_arcweft_binary(
        lhs: &Expr,
        op: BinaryOp,
        rhs: &Expr,
        symbols: &BTreeMap<SmtSymbolId, SmtSort>,
    ) -> Result<LoweredExpr, ContractLoweringError> {
        let lhs = Self::from_arcweft(lhs, symbols)?;
        let rhs = Self::from_arcweft(rhs, symbols)?;
        match op {
            BinaryOp::Implies => {
                require_sort("implication lhs", lhs.sort, SmtSort::Bool)?;
                require_sort("implication rhs", rhs.sort, SmtSort::Bool)?;
                Ok(LoweredExpr::bool(Self::implies(lhs.expr, rhs.expr)))
            }
            BinaryOp::Or => {
                require_sort("Boolean or lhs", lhs.sort, SmtSort::Bool)?;
                require_sort("Boolean or rhs", rhs.sort, SmtSort::Bool)?;
                Ok(LoweredExpr::bool(Self::disjunction([lhs.expr, rhs.expr])))
            }
            BinaryOp::And => {
                require_sort("Boolean and lhs", lhs.sort, SmtSort::Bool)?;
                require_sort("Boolean and rhs", rhs.sort, SmtSort::Bool)?;
                Ok(LoweredExpr::bool(Self::conjunction([lhs.expr, rhs.expr])))
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                require_same_sort("equality operands", lhs.sort, rhs.sort)?;
                let equality = Self::equal(lhs.expr, rhs.expr);
                Ok(LoweredExpr::bool(if op == BinaryOp::NotEq {
                    Self::negated(equality)
                } else {
                    equality
                }))
            }
            BinaryOp::Gte | BinaryOp::Lte | BinaryOp::Gt | BinaryOp::Lt => {
                require_integer_pair("comparison", &lhs, &rhs)?;
                let expr = match op {
                    BinaryOp::Gte => Self::greater_equal(lhs.expr, rhs.expr),
                    BinaryOp::Lte => Self::less_equal(lhs.expr, rhs.expr),
                    BinaryOp::Gt => Self::greater_than(lhs.expr, rhs.expr),
                    BinaryOp::Lt => Self::less_than(lhs.expr, rhs.expr),
                    _ => unreachable!(),
                };
                Ok(LoweredExpr::bool(expr))
            }
            BinaryOp::Add => {
                require_integer_pair("addition", &lhs, &rhs)?;
                Ok(LoweredExpr::int(Self::add([lhs.expr, rhs.expr])))
            }
            BinaryOp::Sub => {
                require_integer_pair("subtraction", &lhs, &rhs)?;
                Ok(LoweredExpr::int(Self::subtract(lhs.expr, rhs.expr)))
            }
            BinaryOp::Mul => {
                require_integer_pair("multiplication", &lhs, &rhs)?;
                if !matches!(&lhs.expr, Self::Int(_)) && !matches!(&rhs.expr, Self::Int(_)) {
                    return Err(ContractLoweringError::NonlinearMultiplication);
                }
                Ok(LoweredExpr::int(Self::multiply(lhs.expr, rhs.expr)))
            }
            BinaryOp::In | BinaryOp::Merge | BinaryOp::Div | BinaryOp::Rem => Err(
                ContractLoweringError::UnsupportedOperator(format!("{op:?}")),
            ),
        }
    }

    fn from_arcweft_clamp(
        receiver: &Expr,
        args: &[CallArg],
        symbols: &BTreeMap<SmtSymbolId, SmtSort>,
    ) -> Result<LoweredExpr, ContractLoweringError> {
        let [lower, upper] = args else {
            return Err(ContractLoweringError::InvalidCall(
                "value.clamp(lower, upper) requires two arguments".to_owned(),
            ));
        };
        let value = Self::from_arcweft(receiver, symbols)?;
        let lower = Self::from_arcweft(lower.value(), symbols)?;
        let upper = Self::from_arcweft(upper.value(), symbols)?;
        require_sort("clamp receiver", value.sort, SmtSort::Int)?;
        require_integer_pair("clamp bounds", &lower, &upper)?;
        let below = Self::less_than(value.expr.clone(), lower.expr.clone());
        let above = Self::greater_than(value.expr.clone(), upper.expr.clone());
        Ok(LoweredExpr::int(Self::if_then_else(
            below,
            lower.expr,
            Self::if_then_else(above, upper.expr, value.expr),
        )))
    }

    fn from_arcweft_min_max(
        receiver: &Expr,
        method: &str,
        args: &[CallArg],
        symbols: &BTreeMap<SmtSymbolId, SmtSort>,
    ) -> Result<LoweredExpr, ContractLoweringError> {
        let [other] = args else {
            return Err(ContractLoweringError::InvalidCall(format!(
                "value.{method}(other) requires one argument"
            )));
        };
        let value = Self::from_arcweft(receiver, symbols)?;
        let other = Self::from_arcweft(other.value(), symbols)?;
        require_integer_pair(method, &value, &other)?;
        let condition = if method == "min" {
            Self::less_equal(value.expr.clone(), other.expr.clone())
        } else {
            Self::greater_equal(value.expr.clone(), other.expr.clone())
        };
        Ok(LoweredExpr::int(Self::if_then_else(
            condition, value.expr, other.expr,
        )))
    }
}

fn require_integer_pair(
    context: &str,
    lhs: &LoweredExpr,
    rhs: &LoweredExpr,
) -> Result<(), ContractLoweringError> {
    require_sort(context, lhs.sort, SmtSort::Int)?;
    require_sort(context, rhs.sort, SmtSort::Int)
}

fn require_same_sort(
    context: &str,
    lhs: SmtSort,
    rhs: SmtSort,
) -> Result<(), ContractLoweringError> {
    if lhs == rhs {
        Ok(())
    } else {
        Err(ContractLoweringError::Sort {
            context: context.to_owned(),
            expected: lhs,
            actual: rhs,
        })
    }
}

fn selected_callee_receiver(expr: &Expr) -> Option<&Expr> {
    let Expr::Select(select) = expr else {
        return None;
    };
    Some(select.target())
}

fn selected_callee_method(expr: &Expr) -> Option<&str> {
    let Expr::Select(select) = expr else {
        return None;
    };
    Some(select.member().as_str())
}

fn require_sort(
    context: &str,
    actual: SmtSort,
    expected: SmtSort,
) -> Result<(), ContractLoweringError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ContractLoweringError::Sort {
            context: context.to_owned(),
            expected,
            actual,
        })
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
enum ContractLoweringError {
    #[error(
        "function `{function}` uses a proof-mode invariant; scalar lowering currently supports `requires` and `ensures prove`, and will not assume an unproven invariant"
    )]
    InvariantClause { function: String },
    #[error(
        "function `{function}` has statements; the current solver lowering requires one expression body"
    )]
    StatementBody { function: String },
    #[error("function `{function}` has no expression body")]
    MissingBodyValue { function: String },
    #[error("function `{function}` has no explicit return type")]
    MissingReturnType { function: String },
    #[error("unsupported scalar type for `{label}`: {ty}")]
    UnsupportedType { label: String, ty: String },
    #[error("unsupported function parameter pattern: {pattern}")]
    UnsupportedParameterPattern { pattern: String },
    #[error("duplicate solver symbol `{0}`")]
    DuplicateSymbol(String),
    #[error("unknown contract symbol `{0}`")]
    UnknownSymbol(String),
    #[error("unsupported contract expression: {0}")]
    UnsupportedExpression(String),
    #[error("unsupported contract operator: {0}")]
    UnsupportedOperator(String),
    #[error("non-linear multiplication is outside QF_LIA; one operand must be an integer literal")]
    NonlinearMultiplication,
    #[error("invalid contract call: {0}")]
    InvalidCall(String),
    #[error("{context} expected {expected}, found {actual}")]
    Sort {
        context: String,
        expected: SmtSort,
        actual: SmtSort,
    },
    #[error(transparent)]
    Smt(#[from] SmtError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_lang_hir::lower::lower_document_to_hir;
    use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use std::sync::Arc;

    fn lowered_report(source: &str) -> VerificationReport {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://verify/contract-smt.arcw")
                    .expect("contract SMT fixture source ID"),
                SourceName::path("verify/contract-smt.arcw"),
                source,
            )
            .expect("contract SMT fixture source document"),
        );
        let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let hir = lower_document_to_hir(parsed.document().as_ref(), parsed.typed_tree())
            .expect("fixture lowers");
        let mut report = VerificationReport::default();
        collect_function_contract_obligations(&hir, &mut report);
        report
    }

    #[test]
    fn lowers_natural_arcweft_contract_to_counterexample_query() {
        let report = lowered_report(
            r"
fn bounded(value: i32, delta: i32) -> i32
requires value >= 0 && value <= 100
ensures prove result >= 0
{
    (value + delta).clamp(0, 100)
}
",
        );
        let problem = report.obligations[0].smt.as_ref().expect("SMT problem");
        let script = problem
            .emit_smt_lib(crate::smt::SmtEmission::CounterexampleValues)
            .expect("SMT emits");
        assert!(script.contains("(declare-const value Int)"));
        assert!(script.contains("(declare-const result Int)"));
        assert!(script.contains("(assert (= result (ite"));
        assert!(script.contains("(assert (< result 0))"));
    }

    #[test]
    fn preserves_u128_literals_as_arbitrary_precision_smt_integers() {
        let report = lowered_report(
            r"
fn bounded(value: u128) -> u128
ensures prove result <= 340282366920938463463374607431768211455
{
    value
}
",
        );
        let script = report.obligations[0]
            .smt
            .as_ref()
            .expect("u128 contract lowers")
            .emit_smt_lib(crate::smt::SmtEmission::CheckOnly)
            .expect("SMT emits");
        assert!(script.contains("340282366920938463463374607431768211455"));
    }

    #[test]
    fn proof_mode_invariant_is_rejected_instead_of_becoming_an_assumption() {
        let report = lowered_report(
            r"
fn guarded(value: i32) -> i32
invariant prove value >= 0
ensures prove result >= 0
{
    value
}
",
        );
        assert!(report.obligations[0].smt.is_none());
        assert!(
            report.diagnostics[0]
                .message
                .contains("will not assume an unproven invariant")
        );
    }

    #[test]
    fn explicit_assume_does_not_silently_strengthen_smt() {
        let report = lowered_report(
            r"
fn assumed(value: i32) -> i32
assume value >= 0
ensures prove result >= 0
{
    value
}
",
        );
        let problem = report.obligations[0].smt.as_ref().expect("SMT problem");
        let script = problem
            .emit_smt_lib(crate::smt::SmtEmission::CheckOnly)
            .expect("SMT emits");
        assert!(!script.contains("(assert (>= value 0))"));
        assert!(script.contains("(assert (= result value))"));
        assert!(script.contains("(assert (< result 0))"));
    }

    #[test]
    fn unsupported_dsl_shape_is_visible_instead_of_becoming_uninterpreted_smt() {
        let report = lowered_report(
            r"
fn unsupported(xs: [i32]) -> i32
ensures prove result >= 0
{
    xs[0]
}
",
        );
        assert!(report.obligations[0].smt.is_none());
        assert!(
            report.diagnostics[0]
                .message
                .contains("unsupported scalar type")
        );
    }
}
