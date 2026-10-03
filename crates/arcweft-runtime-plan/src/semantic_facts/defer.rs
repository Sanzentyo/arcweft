//! Checked deferred executable body and its registration-time capture ABI.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_core::line_task::RuntimeDeferOutcomeFilter;
use arcweft_lang_hir::expr::HirExprKind;
use arcweft_lang_hir::identity::{ExprId, HirModuleId, LocalId, StmtId};
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::stmt::HirStmtKind;
use arcweft_lang_sema::effects::EffectSet;
use arcweft_lang_syntax::ast::line_plan::DeferOutcome;

use super::{
    RuntimeExecutableCaptureFact, RuntimeNormalizedType, RuntimeSemanticFactFamily,
    RuntimeSemanticFactsError, RuntimeSemanticOwnerSet, RuntimeTypeShape, require_stmt_family,
    resolve_expr, resolve_stmt,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDeferFact {
    outcome: RuntimeDeferOutcomeFilter,
    body: ExprId,
    effects: EffectSet,
    captures: Box<[RuntimeExecutableCaptureFact]>,
}

impl RuntimeDeferFact {
    pub fn new(
        outcome: RuntimeDeferOutcomeFilter,
        body: ExprId,
        effects: EffectSet,
        captures: impl Into<Box<[RuntimeExecutableCaptureFact]>>,
    ) -> Self {
        Self {
            outcome,
            body,
            effects,
            captures: captures.into(),
        }
    }

    pub const fn outcome(&self) -> RuntimeDeferOutcomeFilter {
        self.outcome
    }
    pub const fn body(&self) -> ExprId {
        self.body
    }
    pub const fn effects(&self) -> &EffectSet {
        &self.effects
    }
    pub const fn captures(&self) -> &[RuntimeExecutableCaptureFact] {
        &self.captures
    }
}

pub(super) fn validate_defer<'types>(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owners: RuntimeSemanticOwnerSet<'_>,
    local_type: impl Fn(&LocalId) -> Option<&'types RuntimeNormalizedType>,
    expressions: &BTreeMap<ExprId, RuntimeNormalizedType>,
    statement: StmtId,
    fact: &RuntimeDeferFact,
) -> Result<(), RuntimeSemanticFactsError> {
    require_stmt_family(
        modules,
        owners,
        statement,
        RuntimeSemanticFactFamily::Defer,
        |kind| matches!(kind, HirStmtKind::Defer { .. }),
    )?;
    validate_defer_payload(modules, local_type, expressions, statement, fact)
}

pub(super) fn validate_defer_payload<'types>(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    local_type: impl Fn(&LocalId) -> Option<&'types RuntimeNormalizedType>,
    expressions: &BTreeMap<ExprId, RuntimeNormalizedType>,
    statement: StmtId,
    fact: &RuntimeDeferFact,
) -> Result<(), RuntimeSemanticFactsError> {
    let HirStmtKind::Defer {
        outcome,
        expression,
    } = resolve_stmt(modules, statement)?
    else {
        return Err(RuntimeSemanticFactsError::WrongStatementFamily {
            statement,
            expected: RuntimeSemanticFactFamily::Defer,
        });
    };
    let expected = match outcome {
        DeferOutcome::Always => RuntimeDeferOutcomeFilter::Always,
        DeferOutcome::Completed => RuntimeDeferOutcomeFilter::Completed,
        DeferOutcome::Cancelled => RuntimeDeferOutcomeFilter::Cancelled,
        DeferOutcome::Failed => RuntimeDeferOutcomeFilter::Failed,
    };
    if fact.outcome != expected
        || fact.body != *expression
        || fact.body.module() != statement.module()
    {
        return Err(RuntimeSemanticFactsError::InvalidDeferFact { statement });
    }
    if !matches!(resolve_expr(modules, fact.body)?, HirExprKind::Block(_)) {
        return Err(RuntimeSemanticFactsError::InvalidDeferFact { statement });
    }
    if !matches!(
        expressions
            .get(&fact.body)
            .map(RuntimeNormalizedType::shape),
        Some(RuntimeTypeShape::Unit)
    ) {
        return Err(RuntimeSemanticFactsError::InvalidDeferFact { statement });
    }
    let mut seen = BTreeSet::new();
    let mut origins = BTreeSet::new();
    for capture in fact.captures() {
        if !seen.insert(capture.local())
            || !origins.insert(capture.origin().clone())
            || local_type(&capture.local()) != Some(capture.ty())
        {
            return Err(RuntimeSemanticFactsError::InvalidDeferFact { statement });
        }
    }
    Ok(())
}
