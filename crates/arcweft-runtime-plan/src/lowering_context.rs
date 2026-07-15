//! Structured authored locations for executable runtime-plan lowering.

use crate::errors::{RuntimePlanLowerContext, RuntimePlanLowerError};
use arcweft_lang_hir::{
    model::HirModule,
    syntax::ast::{common::TextRange, flow::Stmt, module_path::CanonicalModulePath},
};
use arcweft_source::SourceSpan;

#[derive(Clone, Debug)]
pub(crate) struct ExecutableLoweringLocation<'hir> {
    owner: String,
    path: Vec<String>,
    source: Option<ExecutableSource<'hir>>,
}

#[derive(Clone, Debug)]
struct ExecutableSource<'hir> {
    module: &'hir HirModule,
    module_path: Option<CanonicalModulePath>,
}

impl<'hir> ExecutableLoweringLocation<'hir> {
    pub(crate) fn in_module(
        owner: impl Into<String>,
        module: &'hir HirModule,
        module_path: Option<&CanonicalModulePath>,
    ) -> Self {
        Self {
            owner: owner.into(),
            path: Vec::new(),
            source: Some(ExecutableSource {
                module,
                module_path: module_path.cloned(),
            }),
        }
    }

    pub(crate) fn statement(&self, index: usize) -> Self {
        self.child(index.to_string())
    }

    pub(crate) fn with_owner(&self, owner: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            path: Vec::new(),
            source: self.source.clone(),
        }
    }

    pub(crate) fn child(&self, segment: impl Into<String>) -> Self {
        let mut path = self.path.clone();
        path.push(segment.into());
        Self {
            owner: self.owner.clone(),
            path,
            source: self.source.clone(),
        }
    }

    pub(crate) fn owner(&self) -> &str {
        &self.owner
    }

    pub(crate) fn path(&self) -> &[String] {
        &self.path
    }

    pub(crate) fn unsupported_statement(&self, statement: &Stmt) -> RuntimePlanLowerError {
        let kind = statement_kind(statement);
        let source_range = statement_range(statement);
        RuntimePlanLowerError::in_context(
            RuntimePlanLowerContext::statement(&self.owner, self.path.clone(), kind, source_range),
            format!("`{kind}` is not executable in this runtime-plan owner"),
        )
        .with_source(self.source_span(source_range))
    }

    pub(crate) fn expression_error(
        &self,
        statement: &Stmt,
        role: &'static str,
        source_range: Option<TextRange>,
        reason: impl Into<String>,
    ) -> RuntimePlanLowerError {
        self.named_expression_error(statement_kind(statement), role, source_range, reason)
    }

    pub(crate) fn named_expression_error(
        &self,
        statement_kind: &'static str,
        role: &'static str,
        source_range: Option<TextRange>,
        reason: impl Into<String>,
    ) -> RuntimePlanLowerError {
        RuntimePlanLowerError::in_context(
            RuntimePlanLowerContext::expression(
                &self.owner,
                self.path.clone(),
                statement_kind,
                role,
                source_range,
            ),
            reason,
        )
        .with_source(self.source_span(source_range))
    }

    pub(crate) fn pattern_error(
        &self,
        statement: &Stmt,
        role: &'static str,
        reason: impl Into<String>,
    ) -> RuntimePlanLowerError {
        self.named_pattern_error(
            statement_kind(statement),
            role,
            statement_range(statement),
            reason,
        )
    }

    pub(crate) fn named_pattern_error(
        &self,
        statement_kind: &'static str,
        role: &'static str,
        source_range: Option<TextRange>,
        reason: impl Into<String>,
    ) -> RuntimePlanLowerError {
        RuntimePlanLowerError::in_context(
            RuntimePlanLowerContext::pattern(
                &self.owner,
                self.path.clone(),
                statement_kind,
                role,
                source_range,
            ),
            reason,
        )
        .with_source(self.source_span(source_range))
    }

    pub(crate) fn bind_error(&self, error: RuntimePlanLowerError) -> RuntimePlanLowerError {
        let range = error
            .context()
            .and_then(RuntimePlanLowerContext::source_range);
        error.with_source(self.source_span(range))
    }

    fn source_span(&self, range: Option<TextRange>) -> Option<SourceSpan> {
        let source = self.source.as_ref()?;
        let range = range?;
        source
            .module_path
            .as_ref()
            .and_then(|module| source.module.project_source_span(module, range))
            .or_else(|| source.module.source_span(range))
    }
}

fn statement_range(statement: &Stmt) -> Option<TextRange> {
    match statement {
        Stmt::Let { expr_range, .. }
        | Stmt::Return { expr_range, .. }
        | Stmt::Expr { expr_range, .. } => *expr_range,
        Stmt::LetElse { expr, .. } | Stmt::Goto(expr) | Stmt::Yield(expr) | Stmt::Close(expr) => {
            expr.range()
        }
        Stmt::Assign { target, expr } => target.range().or(expr.range()),
        Stmt::Signal { target, value } => target.range().or(value.range()),
        Stmt::If { condition, .. }
        | Stmt::While { condition, .. }
        | Stmt::For {
            source: condition, ..
        }
        | Stmt::Match {
            expr: condition, ..
        } => condition.range(),
        Stmt::WhileLet { expr, .. } => expr.range(),
        Stmt::LetScope { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::LetActionReceive { .. }
        | Stmt::LetChoice { .. }
        | Stmt::Out { .. }
        | Stmt::Defer { .. }
        | Stmt::Select(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::LifetimeSet { .. }
        | Stmt::Wait(_)
        | Stmt::Thread(_)
        | Stmt::DeferBlock { .. }
        | Stmt::On { .. }
        | Stmt::Loop { .. }
        | Stmt::UnsafeLifetime { .. }
        | Stmt::Raw(_) => None,
    }
}

pub(crate) const fn statement_kind(statement: &Stmt) -> &'static str {
    match statement {
        Stmt::Let { .. } => "let",
        Stmt::LetElse { .. } => "let-else",
        Stmt::LetScope { .. } => "let-scope",
        Stmt::LetLoop { .. } => "let-loop",
        Stmt::LetAwait { .. } => "let-await",
        Stmt::LetActionReceive { .. } => "let-action-receive",
        Stmt::LetChoice { .. } => "let-choice",
        Stmt::Return { .. } => "return",
        Stmt::Expr { .. } => "expression",
        Stmt::Out { .. } => "out",
        Stmt::Defer { .. } => "defer",
        Stmt::Goto(_) => "goto",
        Stmt::Yield(_) => "yield",
        Stmt::Close(_) => "close",
        Stmt::Select(_) => "select",
        Stmt::Break { .. } => "break",
        Stmt::Continue { .. } => "continue",
        Stmt::Assign { .. } => "assign",
        Stmt::Signal { .. } => "signal",
        Stmt::LifetimeSet { .. } => "lifetime-set",
        Stmt::Wait(_) => "wait",
        Stmt::Thread(_) => "thread",
        Stmt::DeferBlock { .. } => "defer-block",
        Stmt::On { .. } => "on",
        Stmt::Loop { .. } => "loop",
        Stmt::UnsafeLifetime { .. } => "unsafe-lifetime",
        Stmt::If { .. } => "if",
        Stmt::While { .. } => "while",
        Stmt::WhileLet { .. } => "while-let",
        Stmt::For { .. } => "for",
        Stmt::Match { .. } => "match",
        Stmt::Raw(_) => "raw",
    }
}
