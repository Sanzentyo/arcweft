//! Counterexample queries for the propositional fragment of checked assertions.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    expr::{HirBinaryOp, HirExprKind, HirUnaryOp},
    identity::{ExprId, LocalId},
    leaf::HirLiteral,
    module::HirModule,
};
use arcweft_lang_sema::{
    final_analysis::{CheckedExpressionResolution, FinalSemanticAnalysis},
    types::TypeKind,
};

use crate::smt::{ProofExpr, SmtProblem, SmtSort, SmtSymbol, SmtSymbolId};

// Bound both recursive query traversal and solver input size. An expression
// outside this fragment or budget remains an unresolved proof obligation.
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 512;

pub(crate) fn counterexample(
    name: &str,
    module: &HirModule,
    semantics: &FinalSemanticAnalysis,
    condition: ExprId,
) -> Option<SmtProblem> {
    let mut projection = AssertionProjection {
        module,
        semantics,
        bindings: BTreeMap::new(),
        symbols: Vec::new(),
        remaining: MAX_NODES,
    };
    let claim = projection.expression(condition, MAX_DEPTH)?;
    let symbols = projection.symbols;
    let model = symbols.iter().map(|symbol| symbol.id.clone()).collect();
    Some(SmtProblem::counterexample(name, symbols, [], claim, model))
}

struct AssertionProjection<'a> {
    module: &'a HirModule,
    semantics: &'a FinalSemanticAnalysis,
    bindings: BTreeMap<LocalId, SmtSymbolId>,
    symbols: Vec<SmtSymbol>,
    remaining: usize,
}

impl AssertionProjection<'_> {
    fn expression(&mut self, owner: ExprId, depth: usize) -> Option<ProofExpr> {
        let child_depth = depth.checked_sub(1)?;
        self.remaining = self.remaining.checked_sub(1)?;
        let checked = self.semantics.expression(owner)?;
        if checked.value_type() != Some(&TypeKind::Bool) || !checked.effects().is_empty() {
            return None;
        }

        let expression = self.module.resolve_expr(owner).ok()?;
        match (expression.kind(), checked.resolution()) {
            (
                HirExprKind::Literal(HirLiteral::Boolean(authored)),
                CheckedExpressionResolution::Literal(HirLiteral::Boolean(value)),
            ) if authored == value => Some(ProofExpr::bool(*value)),
            (HirExprKind::Path(_), _) => self.local(checked.execution_local_use()?),
            (HirExprKind::Unary(unary), CheckedExpressionResolution::Structural)
                if unary.operator() == HirUnaryOp::Not =>
            {
                Some(ProofExpr::negated(
                    self.expression(unary.operand(), child_depth)?,
                ))
            }
            (HirExprKind::Binary(binary), CheckedExpressionResolution::Structural) => {
                let operation: fn(ProofExpr, ProofExpr) -> ProofExpr = match binary.operator() {
                    HirBinaryOp::And => |lhs, rhs| ProofExpr::conjunction([lhs, rhs]),
                    HirBinaryOp::Or => |lhs, rhs| ProofExpr::disjunction([lhs, rhs]),
                    HirBinaryOp::Implies => ProofExpr::implies,
                    HirBinaryOp::Equal => ProofExpr::equal,
                    HirBinaryOp::NotEqual => {
                        |lhs, rhs| ProofExpr::negated(ProofExpr::equal(lhs, rhs))
                    }
                    _ => return None,
                };
                let lhs = self.expression(binary.left(), child_depth)?;
                let rhs = self.expression(binary.right(), child_depth)?;
                Some(operation(lhs, rhs))
            }
            _ => None,
        }
    }

    fn local(&mut self, owner: LocalId) -> Option<ProofExpr> {
        if self.semantics.local(owner)?.ty() != &TypeKind::Bool {
            return None;
        }
        if let Some(symbol) = self.bindings.get(&owner) {
            return Some(ProofExpr::var(symbol.clone()));
        }
        let local = self.module.resolve_local(owner).ok()?;
        let symbol = SmtSymbolId::new(format!("bool_{}", self.symbols.len()));
        self.symbols.push(
            SmtSymbol::new(symbol.clone(), SmtSort::Bool).with_source_label(local.name().as_str()),
        );
        self.bindings.insert(owner, symbol.clone());
        Some(ProofExpr::var(symbol))
    }
}
