//! Sans I/O verification model for Arcweft HIR.
//!
//! This crate turns structured HIR into proof obligations, diagnostics, and
//! solver-neutral proof problems. It does not read files, spawn processes, or
//! depend on a concrete runtime backend; those responsibilities belong to CLI
//! and solver adapter crates.

use arcweft_lang_hir::{
    EntityRefSyntax, Expr, HirAwait, HirBorrow, HirChoice, HirFlowItem, HirFor, HirFunction, HirIf,
    HirIfLet, HirLoop, HirMatch, HirModule, HirScope, HirScopeExpr, HirSelect, HirTopLevelDecl,
    HirWhile, HirWhileLet, IdRef, LifetimeKey, LifetimeScopeKind, LinePlan, LinePlanItem, Stmt,
    TextRange, ThreadBlock, TriggerPattern,
};
use arcweft_lang_sema::{
    SemanticDiagnostic, SemanticDischarge, SemanticMode, SemanticObligation,
    SemanticObligationKind, SemanticPolicy, SemanticReport, SemanticSeverity, TypeCheckEnv,
    analyze_semantics,
};
use arcweft_runtime_plan::lower_line_task_groups;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use thiserror::Error;

/// Stable source span used by verifier diagnostics and Agent/LSP tooling.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

/// Tool-facing diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Verification mode selected by CLI, LSP, or build policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMode {
    /// Collect everything, but keep incomplete formal work as warnings.
    #[default]
    Dev,
    /// Require formal proof for non-trivial verifier obligations.
    Test,
    /// Release policy: reject audited unsafe and missing formal proof.
    Release,
}

/// Solver family selected by tooling.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    /// Emit obligations and SMT-LIB without solving.
    #[default]
    Emit,
    /// Pure-Rust `OxiZ` adapter.
    Oxiz,
    /// External Z3 process adapter.
    Z3,
}

/// Verifier policy with mode and backend selection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerificationPolicy {
    pub mode: VerificationMode,
    pub backend: BackendKind,
}

/// Verification obligation families understood by Phase 1.5 tooling.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofObligationKind {
    LifetimePromotion,
    UnsafeLifetimeAudit,
    MustDropDischarge,
    ThreadCapture,
    ThreadJoinTyping,
    UpperLifetimeWrite,
    EffectCapability,
    ProofBody,
    TrustedAssumption,
    RawSyntax,
    RuntimeConflict,
}

/// How an obligation is discharged, or why it still needs attention.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProofDischarge {
    Automatic,
    FormalProof { id: String },
    AuditedUnsafe { id: String },
    TrustedAxiom { id: String },
    Missing,
}

/// One proof obligation produced by semantic analysis.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProofObligation {
    pub id: String,
    pub kind: ProofObligationKind,
    pub message: String,
    pub subject: Option<String>,
    pub source: Option<SourceSpan>,
    pub discharge: ProofDischarge,
    pub smt: Option<SmtProblem>,
}

/// JSON diagnostic shared by CLI, LSP, and future Agent tools.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerificationDiagnostic {
    pub id: String,
    pub severity: Severity,
    pub message: String,
    pub source: Option<SourceSpan>,
    pub obligation: Option<String>,
    pub related_ids: Vec<String>,
    pub actions: Vec<ToolAction>,
}

/// Stable action descriptor consumed by LSP code actions and Agent tooling.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolAction {
    pub id: String,
    pub label: String,
    pub kind: ToolActionKind,
}

/// Action kind for verifier-assisted edits or navigation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolActionKind {
    GenerateProofStub,
    GenerateUnsafeAudit,
    ShowObligation,
    NavigateToProof,
    NavigateToUnsafeAudit,
}

/// Proof item summary carried into manifests and LSP hovers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProofSummary {
    pub id: String,
    pub source: SourceSpan,
}

/// Trusted axiom summary carried into release review manifests.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrustedAxiomSummary {
    pub id: String,
    pub source: SourceSpan,
}

/// Unsafe lifetime audit metadata carried into manifests.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnsafeAuditSummary {
    pub id: String,
    pub source: Option<SourceSpan>,
    pub has_reason: bool,
    pub has_safety_doc: bool,
}

/// Full verifier report. This is the canonical tool-facing schema.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerificationReport {
    pub policy: VerificationPolicy,
    pub diagnostics: Vec<VerificationDiagnostic>,
    pub obligations: Vec<ProofObligation>,
    pub proofs: Vec<ProofSummary>,
    pub trusted_axioms: Vec<TrustedAxiomSummary>,
    pub unsafe_audits: Vec<UnsafeAuditSummary>,
}

/// Minimal proof expression IR for SMT emission and adapters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ProofExpr {
    Bool(bool),
    Var(String),
    Not {
        expr: Box<ProofExpr>,
    },
    And {
        exprs: Vec<ProofExpr>,
    },
    Or {
        exprs: Vec<ProofExpr>,
    },
    Eq {
        lhs: Box<ProofExpr>,
        rhs: Box<ProofExpr>,
    },
    App {
        name: String,
        args: Vec<ProofExpr>,
    },
}

/// Solver-neutral SMT problem.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SmtProblem {
    pub name: String,
    pub assertions: Vec<ProofExpr>,
}

/// Solver outcome normalized across adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmtOutcome {
    Sat,
    Unsat,
    Unknown,
}

/// Error returned by a solver adapter.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct SmtError {
    message: String,
}

/// Sans-I/O-facing solver trait. Concrete adapters may perform I/O internally.
pub trait SmtBackend {
    fn name(&self) -> &'static str;
    fn check(&self, problem: &SmtProblem) -> Result<SmtOutcome, SmtError>;
}

/// Verifies an HIR module according to the selected policy.
pub fn verify_module(module: &HirModule, policy: VerificationPolicy) -> VerificationReport {
    let mut collector = ObligationCollector::new(policy);
    collector.collect_module(module);
    let semantic_report = analyze_semantics(
        module,
        &TypeCheckEnv::new(),
        SemanticPolicy {
            mode: semantic_mode(policy.mode),
        },
    );
    let mut report = collector.finish();
    merge_semantic_report(&mut report, semantic_report);
    report
}

/// Emits a compact SMT-LIB 2 script for a solver-neutral problem.
pub fn emit_smt_lib(problem: &SmtProblem) -> String {
    let mut out = String::from("(set-logic ALL)\n");
    for assertion in &problem.assertions {
        out.push_str("(assert ");
        out.push_str(&emit_expr(assertion));
        out.push_str(")\n");
    }
    out.push_str("(check-sat)\n");
    out
}

impl SmtError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl VerificationReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }

    pub fn unsafe_audit_count(&self) -> usize {
        self.unsafe_audits.len()
    }
}

fn merge_semantic_report(report: &mut VerificationReport, semantic: SemanticReport) {
    remove_collector_semantic_obligations(report);

    for proof in semantic.proofs {
        if !report.proofs.iter().any(|existing| existing.id == proof.id) {
            report.proofs.push(ProofSummary {
                id: proof.id,
                source: SourceSpan {
                    start: proof.source.start,
                    end: proof.source.end,
                },
            });
        }
    }
    for axiom in semantic.trusted_axioms {
        if !report
            .trusted_axioms
            .iter()
            .any(|existing| existing.id == axiom.id)
        {
            report.trusted_axioms.push(TrustedAxiomSummary {
                id: axiom.id,
                source: SourceSpan {
                    start: axiom.source.start,
                    end: axiom.source.end,
                },
            });
        }
    }
    for audit in semantic.unsafe_audits {
        if !report
            .unsafe_audits
            .iter()
            .any(|existing| existing.id == audit.id)
        {
            report.unsafe_audits.push(UnsafeAuditSummary {
                id: audit.id,
                source: audit.source.map(|source| SourceSpan {
                    start: source.start,
                    end: source.end,
                }),
                has_reason: audit.has_reason,
                has_safety_doc: audit.has_safety_doc,
            });
        }
    }
    let mut id_map = BTreeMap::new();
    for obligation in semantic.obligations {
        let (semantic_id, verification_id) = merge_semantic_obligation(report, obligation);
        id_map.insert(semantic_id, verification_id);
    }
    for diagnostic in semantic.diagnostics {
        merge_semantic_diagnostic(report, diagnostic, &id_map);
    }
    report
        .diagnostics
        .sort_by(|left, right| left.id.cmp(&right.id));
}

fn remove_collector_semantic_obligations(report: &mut VerificationReport) {
    let removed: BTreeSet<String> = report
        .obligations
        .iter()
        .filter(|obligation| is_semantic_owned_kind(obligation.kind))
        .map(|obligation| obligation.id.clone())
        .collect();
    if removed.is_empty() {
        return;
    }
    report
        .obligations
        .retain(|obligation| !removed.contains(&obligation.id));
    report.diagnostics.retain(|diagnostic| {
        diagnostic
            .obligation
            .as_ref()
            .is_none_or(|id| !removed.contains(id))
    });
}

fn is_semantic_owned_kind(kind: ProofObligationKind) -> bool {
    matches!(
        kind,
        ProofObligationKind::LifetimePromotion
            | ProofObligationKind::UnsafeLifetimeAudit
            | ProofObligationKind::MustDropDischarge
            | ProofObligationKind::ThreadCapture
            | ProofObligationKind::ThreadJoinTyping
            | ProofObligationKind::UpperLifetimeWrite
            | ProofObligationKind::EffectCapability
            | ProofObligationKind::ProofBody
            | ProofObligationKind::TrustedAssumption
            | ProofObligationKind::RawSyntax
            | ProofObligationKind::RuntimeConflict
    )
}

fn merge_semantic_obligation(
    report: &mut VerificationReport,
    obligation: SemanticObligation,
) -> (String, String) {
    let semantic_id = obligation.id.clone();
    let kind = proof_kind(obligation.kind);
    if let Some(existing) = report.obligations.iter().find(|existing| {
        existing.kind == kind
            && existing.subject == obligation.subject
            && existing.message == obligation.message
    }) {
        return (semantic_id, existing.id.clone());
    }
    let id = format!("obligation.{:04}", report.obligations.len() + 1);
    let verification_id = id.clone();
    let discharge = proof_discharge(obligation.discharge);
    let subject = obligation.subject;
    let smt = Some(SmtProblem {
        name: id.clone(),
        assertions: vec![ProofExpr::App {
            name: obligation_predicate(kind).to_owned(),
            args: subject
                .as_ref()
                .map(|subject| vec![ProofExpr::Var(subject.clone())])
                .unwrap_or_default(),
        }],
    });
    report.obligations.push(ProofObligation {
        id,
        kind,
        message: obligation.message,
        subject,
        source: obligation.source.map(|source| SourceSpan {
            start: source.start,
            end: source.end,
        }),
        discharge,
        smt,
    });

    (semantic_id, verification_id)
}

fn merge_semantic_diagnostic(
    report: &mut VerificationReport,
    diagnostic: SemanticDiagnostic,
    id_map: &BTreeMap<String, String>,
) {
    if report
        .diagnostics
        .iter()
        .any(|existing| existing.message == diagnostic.message)
    {
        return;
    }
    let obligation = diagnostic
        .obligation
        .and_then(|id| id_map.get(&id).cloned());
    report.diagnostics.push(VerificationDiagnostic {
        id: format!("diagnostic.{}", diagnostic.id),
        severity: severity(diagnostic.severity),
        message: diagnostic.message,
        source: diagnostic.source.map(|source| SourceSpan {
            start: source.start,
            end: source.end,
        }),
        obligation,
        related_ids: diagnostic.related_ids,
        actions: Vec::new(),
    });
}

fn semantic_mode(mode: VerificationMode) -> SemanticMode {
    match mode {
        VerificationMode::Dev => SemanticMode::Dev,
        VerificationMode::Test => SemanticMode::Test,
        VerificationMode::Release => SemanticMode::Release,
    }
}

fn proof_kind(kind: SemanticObligationKind) -> ProofObligationKind {
    match kind {
        SemanticObligationKind::LifetimePromotion => ProofObligationKind::LifetimePromotion,
        SemanticObligationKind::UnsafeLifetimeAudit => ProofObligationKind::UnsafeLifetimeAudit,
        SemanticObligationKind::MustDropDischarge => ProofObligationKind::MustDropDischarge,
        SemanticObligationKind::ThreadCapture => ProofObligationKind::ThreadCapture,
        SemanticObligationKind::ThreadJoinTyping => ProofObligationKind::ThreadJoinTyping,
        SemanticObligationKind::UpperLifetimeWrite => ProofObligationKind::UpperLifetimeWrite,
        SemanticObligationKind::EffectCapability => ProofObligationKind::EffectCapability,
        SemanticObligationKind::ProofBody => ProofObligationKind::ProofBody,
        SemanticObligationKind::TrustedAssumption => ProofObligationKind::TrustedAssumption,
        SemanticObligationKind::RawSyntax => ProofObligationKind::RawSyntax,
        SemanticObligationKind::RuntimeConflict => ProofObligationKind::RuntimeConflict,
    }
}

fn proof_discharge(discharge: SemanticDischarge) -> ProofDischarge {
    match discharge {
        SemanticDischarge::Automatic => ProofDischarge::Automatic,
        SemanticDischarge::FormalProof { id } => ProofDischarge::FormalProof { id },
        SemanticDischarge::AuditedUnsafe { id } => ProofDischarge::AuditedUnsafe { id },
        SemanticDischarge::TrustedAxiom { id } => ProofDischarge::TrustedAxiom { id },
        SemanticDischarge::Missing => ProofDischarge::Missing,
    }
}

fn severity(severity: SemanticSeverity) -> Severity {
    match severity {
        SemanticSeverity::Info => Severity::Info,
        SemanticSeverity::Warning => Severity::Warning,
        SemanticSeverity::Error => Severity::Error,
    }
}

struct ObligationCollector {
    policy: VerificationPolicy,
    report: VerificationReport,
    next_obligation: usize,
    unsafe_stack: Vec<String>,
    known_proofs: BTreeSet<String>,
    known_axioms: BTreeSet<String>,
    lifetime_reads: Vec<LifetimeKey>,
    lifetime_drops: HashSet<LifetimeKey>,
}

impl ObligationCollector {
    fn new(policy: VerificationPolicy) -> Self {
        Self {
            policy,
            report: VerificationReport {
                policy,
                ..VerificationReport::default()
            },
            next_obligation: 0,
            unsafe_stack: Vec::new(),
            known_proofs: BTreeSet::new(),
            known_axioms: BTreeSet::new(),
            lifetime_reads: Vec::new(),
            lifetime_drops: HashSet::new(),
        }
    }

    fn collect_module(&mut self, module: &HirModule) {
        self.collect_declarations(module);
        for flow in module.flows() {
            self.collect_flow_items(flow.body());
        }
        for function in module.functions() {
            self.collect_function(function);
        }
        self.collect_flow_items(module.top_level_items());
        self.collect_must_drop_obligations();
        self.collect_runtime_plan_obligations(module);
    }

    fn collect_runtime_plan_obligations(&mut self, module: &HirModule) {
        if let Err(errors) = lower_line_task_groups(module) {
            for error in errors {
                self.add_obligation(
                    ProofObligationKind::RuntimeConflict,
                    format!("runtime plan conflict: {}", error.message()),
                    None,
                    &ProofDischarge::Missing,
                );
            }
        }
    }

    fn collect_declarations(&mut self, module: &HirModule) {
        for declaration in module.declarations() {
            match declaration {
                HirTopLevelDecl::Proof(proof) => {
                    let id = id_ref_label(proof.id(), "proof");
                    self.known_proofs.insert(id.clone());
                    self.report.proofs.push(ProofSummary {
                        id,
                        source: span_from_range(proof.range()),
                    });
                }
                HirTopLevelDecl::TrustedAxiom(axiom) => {
                    let id = id_ref_label(axiom.id(), "axiom");
                    self.known_axioms.insert(id.clone());
                    self.report.trusted_axioms.push(TrustedAxiomSummary {
                        id,
                        source: span_from_range(axiom.range()),
                    });
                }
                HirTopLevelDecl::Hook(hook) => self.collect_stmts(hook.body_statements()),
                HirTopLevelDecl::MemoFn(item) => self.collect_stmts(item.body_statements()),
                HirTopLevelDecl::Parser(item) => self.collect_stmts(item.body_statements()),
                HirTopLevelDecl::Source(item) => self.collect_stmts(item.body_statements()),
                _ => {}
            }
        }
    }

    fn collect_function(&mut self, function: &HirFunction) {
        self.collect_stmts(function.statements());
        if let Some(value) = function.value() {
            self.collect_expr(value);
        }
    }

    fn collect_flow_items(&mut self, items: &[HirFlowItem]) {
        for item in items {
            self.collect_flow_item(item);
        }
    }

    fn collect_flow_item(&mut self, item: &HirFlowItem) {
        match item {
            HirFlowItem::Stmt(stmt) => self.collect_stmt(stmt),
            HirFlowItem::Dialogue(dialogue) => {
                for arg in dialogue.args() {
                    self.collect_expr(arg.value());
                }
                if let Some(plan) = dialogue.plan() {
                    self.collect_line_plan(plan);
                }
            }
            HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
                self.collect_choice(choice);
            }
            HirFlowItem::LetScope { scope, .. } => self.collect_scope_expr(scope),
            HirFlowItem::LetLoop { block, .. } | HirFlowItem::Loop(block) => {
                self.collect_loop(block);
            }
            HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
                self.collect_await(await_with);
            }
            HirFlowItem::If(block) => self.collect_if(block),
            HirFlowItem::IfLet(block) => self.collect_if_let(block),
            HirFlowItem::Match(block) => self.collect_match(block),
            HirFlowItem::While(block) => self.collect_while(block),
            HirFlowItem::WhileLet(block) => self.collect_while_let(block),
            HirFlowItem::For(block) => self.collect_for(block),
            HirFlowItem::Select(block) => self.collect_select(block),
            HirFlowItem::Borrow(block) => self.collect_borrow(block),
            HirFlowItem::SourceLocale(block) => self.collect_flow_items(block.body()),
            HirFlowItem::Scope(block) => self.collect_scope(block),
            HirFlowItem::Scenario { args, .. } => {
                for arg in args {
                    self.collect_expr(arg);
                }
            }
            HirFlowItem::Include(_) => {}
        }
    }

    fn collect_choice(&mut self, choice: &HirChoice) {
        for option in choice.options() {
            if let Some(condition) = option.condition() {
                self.collect_expr(condition);
            }
            if let Some(value) = option.value() {
                self.collect_expr(value);
            }
        }
        if let Some(plan) = choice.plan() {
            for item in plan.items() {
                self.collect_choice_plan_item(item);
            }
        }
    }

    fn collect_choice_plan_item(&mut self, item: &arcweft_lang_hir::ChoicePlanItem) {
        match item {
            arcweft_lang_hir::ChoicePlanItem::Option { value, .. } => self.collect_expr(value),
            arcweft_lang_hir::ChoicePlanItem::Timeout { duration, body } => {
                self.collect_expr(duration);
                self.collect_stmts(body);
            }
            arcweft_lang_hir::ChoicePlanItem::Cancel { trigger, body } => {
                self.collect_trigger(trigger);
                self.collect_stmts(body);
            }
            arcweft_lang_hir::ChoicePlanItem::OnSelect { body, .. } => {
                self.collect_stmts(body);
            }
            arcweft_lang_hir::ChoicePlanItem::Raw(raw) => {
                self.add_raw_obligation(format!("raw choice plan item: {raw}"), None);
            }
        }
    }

    fn collect_line_plan(&mut self, plan: &LinePlan) {
        for item in plan.items() {
            self.collect_line_plan_item(item);
        }
    }

    fn collect_line_plan_item(&mut self, item: &LinePlanItem) {
        match item {
            LinePlanItem::Init(stmts) => self.collect_stmts(stmts),
            LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
                for item in items {
                    self.collect_line_plan_item(item);
                }
            }
            LinePlanItem::Thread(thread) => self.collect_thread(thread),
            LinePlanItem::On { body, .. } => self.collect_stmts(body),
            LinePlanItem::Option { value, .. }
            | LinePlanItem::Let { expr: value, .. }
            | LinePlanItem::Out(value)
            | LinePlanItem::Assert { expr: value, .. }
            | LinePlanItem::Expr(value) => self.collect_expr(value),
            LinePlanItem::Stmt(stmt) => self.collect_stmt(stmt),
            LinePlanItem::CancelRule(rule) => self.collect_stmts(rule.action()),
            LinePlanItem::TimedCue { anchor, body } => {
                self.collect_expr(anchor);
                self.collect_expr(body);
            }
            LinePlanItem::Memo { options, .. } => {
                for (_, value) in options {
                    self.collect_expr(value);
                }
            }
            LinePlanItem::Raw(raw) => {
                self.add_raw_obligation(format!("raw line plan item: {raw}"), None);
            }
        }
    }

    fn collect_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::LetElse {
                expr, else_body, ..
            } => {
                self.collect_expr(expr);
                self.collect_stmts(else_body);
            }
            Stmt::LetChoice { choice, .. } => self.collect_choice_syntax(choice),
            Stmt::LetScope { scope, .. } => self.collect_scope_expr_syntax(scope),
            Stmt::LetLoop { block, .. } => self.collect_flow_items_syntax(block.body()),
            Stmt::LetAwait { await_with, .. } => self.collect_await_syntax(await_with),
            Stmt::Let { expr, .. }
            | Stmt::Return(expr)
            | Stmt::Out { expr, .. }
            | Stmt::Goto(expr)
            | Stmt::Defer { expr, .. }
            | Stmt::Yield(expr)
            | Stmt::Panic(expr)
            | Stmt::Fail(expr)
            | Stmt::Bail(expr)
            | Stmt::Close(expr)
            | Stmt::Select(expr)
            | Stmt::Expr(expr) => self.collect_expr(expr),
            Stmt::Thread(thread) => self.collect_thread(thread),
            Stmt::DeferBlock { statements, .. } => self.collect_stmts(statements),
            Stmt::Ensure { condition, message }
            | Stmt::Signal {
                target: condition,
                value: message,
            } => {
                self.collect_expr(condition);
                self.collect_expr(message);
            }
            Stmt::LifetimeSet { target, expr } => {
                self.collect_lifetime_write(target);
                self.collect_expr(expr);
            }
            Stmt::Wait(target) => self.collect_wait(target),
            Stmt::On { trigger, body } => {
                self.collect_trigger(trigger);
                self.collect_stmts(body);
            }
            Stmt::UnsafeLifetime {
                id,
                reason,
                has_safety_doc,
                body,
            } => self.collect_unsafe_lifetime(id, reason.as_ref(), *has_safety_doc, body),
            Stmt::Command(command) => {
                for arg in command.args() {
                    self.collect_expr(arg);
                }
            }
            Stmt::If { condition, body } | Stmt::While { condition, body } => {
                self.collect_expr(condition);
                self.collect_stmts(body);
            }
            Stmt::Loop { body } => self.collect_stmts(body),
            Stmt::WhileLet {
                expr, guard, body, ..
            } => {
                self.collect_expr(expr);
                if let Some(guard) = guard {
                    self.collect_expr(guard);
                }
                self.collect_stmts(body);
            }
            Stmt::For { source, body, .. } => {
                self.collect_expr(source);
                self.collect_stmts(body);
            }
            Stmt::Match { expr, arms } => {
                self.collect_expr(expr);
                for arm in arms {
                    if let Some(guard) = arm.guard() {
                        self.collect_expr(guard);
                    }
                    self.collect_stmts(arm.body());
                }
            }
            Stmt::Break { expr, .. } => {
                if let Some(expr) = expr {
                    self.collect_expr(expr);
                }
            }
            Stmt::Continue { .. } => {}
            Stmt::Raw(raw) => self.add_raw_obligation(format!("raw statement: {raw}"), None),
        }
    }

    fn collect_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.collect_stmt(stmt);
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "expression traversal mirrors the public Expr enum so verifier coverage is auditable"
    )]
    fn collect_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(_) | Expr::Path(_) | Expr::Placeholder(_) | Expr::Raw(_) => {
                if let Expr::Raw(raw) = expr {
                    self.add_raw_obligation(format!("raw expression: {raw}"), None);
                }
            }
            Expr::EntityRef(_) => {}
            Expr::LifetimePath { key, .. } => {
                self.lifetime_reads.push(key.clone());
            }
            Expr::Tuple(items) | Expr::List(items) => {
                for item in items {
                    self.collect_expr(item);
                }
            }
            Expr::Call { callee, args } => {
                self.collect_call(callee, args);
            }
            Expr::NamedArg { value, .. } => self.collect_expr(value),
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => self.collect_method_call(receiver, method, args),
            Expr::Field { target, .. } => self.collect_expr(target),
            Expr::DialogueCall { callee, plan, .. } => {
                self.collect_expr(callee);
                if let Some(plan) = plan {
                    self.collect_line_plan(plan);
                }
            }
            Expr::Index { target, index } => {
                self.collect_expr(target);
                self.collect_expr(index);
            }
            Expr::Pipe { lhs, rhs } => {
                self.collect_lifetime_drop_pipe(lhs, rhs);
                self.collect_expr(lhs);
                self.collect_expr(rhs);
            }
            Expr::Try { expr } | Expr::Await { expr, .. } | Expr::Unary { expr, .. } => {
                self.collect_expr(expr);
            }
            Expr::Thread { block } => self.collect_thread(block),
            Expr::Range { start, end, .. } => {
                if let Some(start) = start {
                    self.collect_expr(start);
                }
                if let Some(end) = end {
                    self.collect_expr(end);
                }
            }
            Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
                for (_, value) in fields {
                    self.collect_expr(value);
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.collect_expr(lhs);
                self.collect_expr(rhs);
            }
            Expr::Closure { body, .. } => self.collect_expr(body),
            Expr::Block { statements, value }
            | Expr::ComputationBlock {
                statements, value, ..
            }
            | Expr::NamedBlock {
                statements, value, ..
            } => {
                self.collect_stmts(statements);
                if let Some(value) = value {
                    self.collect_expr(value);
                }
            }
            Expr::MemoBlock {
                options,
                statements,
                value,
            } => {
                for (_, value) in options {
                    self.collect_expr(value);
                }
                self.collect_stmts(statements);
                if let Some(value) = value {
                    self.collect_expr(value);
                }
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_expr(condition);
                self.collect_expr(then_branch);
                if let Some(else_branch) = else_branch {
                    self.collect_expr(else_branch);
                }
            }
            Expr::IfLet {
                expr,
                guard,
                then_branch,
                else_branch,
                ..
            } => {
                self.collect_expr(expr);
                if let Some(guard) = guard {
                    self.collect_expr(guard);
                }
                self.collect_expr(then_branch);
                if let Some(else_branch) = else_branch {
                    self.collect_expr(else_branch);
                }
            }
            Expr::Match { scrutinee, arms } => {
                self.collect_expr(scrutinee);
                for arm in arms {
                    if let Some(guard) = arm.guard() {
                        self.collect_expr(guard);
                    }
                    self.collect_expr(arm.value());
                }
            }
        }
    }

    fn collect_call(&mut self, callee: &Expr, args: &[Expr]) {
        if let Expr::Path(name) = callee {
            match name.as_str() {
                "promote" => self.add_promote_obligation(args, false),
                "promote_unchecked" => self.add_promote_obligation(args, true),
                "drop" | "drop_optional" | "on_drop" => self.collect_drop_args(args),
                "assume" => self.add_assume_obligation(args),
                _ => {}
            }
        }
        self.collect_expr(callee);
        for arg in args {
            self.collect_expr(arg);
        }
    }

    fn collect_method_call(&mut self, receiver: &Expr, method: &str, args: &[Expr]) {
        match method {
            "promote" => self.add_promote_obligation(args, false),
            "promote_unchecked" => self.add_promote_obligation(args, true),
            "drop" | "drop_optional" | "on_drop" => {
                if let Expr::LifetimePath { key, .. } = receiver {
                    self.lifetime_drops.insert(key.clone());
                }
            }
            _ => {}
        }
        self.collect_expr(receiver);
        for arg in args {
            self.collect_expr(arg);
        }
    }

    fn add_promote_obligation(&mut self, args: &[Expr], unchecked: bool) {
        let proof = proof_arg(args);
        let target = args.first().and_then(lifetime_label_arg);
        let discharge = if unchecked {
            self.unsafe_stack
                .last()
                .cloned()
                .map_or(ProofDischarge::Missing, |id| {
                    ProofDischarge::AuditedUnsafe { id }
                })
        } else if let Some(id) = proof {
            ProofDischarge::FormalProof { id }
        } else {
            ProofDischarge::Missing
        };
        let message = target.as_ref().map_or_else(
            || "lifetime promotion requires proof or audit".to_owned(),
            |target| format!("lifetime promotion to `{target}` requires proof or audit"),
        );
        self.add_obligation(
            ProofObligationKind::LifetimePromotion,
            message,
            target,
            &discharge,
        );
    }

    fn add_assume_obligation(&mut self, args: &[Expr]) {
        let discharge = axiom_arg(args)
            .filter(|id| self.known_axioms.contains(id))
            .map_or(ProofDischarge::Missing, |id| ProofDischarge::TrustedAxiom {
                id,
            });
        self.add_obligation(
            ProofObligationKind::TrustedAssumption,
            "assume requires a reason or trusted axiom".to_owned(),
            None,
            &discharge,
        );
    }

    fn collect_thread(&mut self, thread: &ThreadBlock) {
        let discharge = if thread.is_detached() {
            ProofDischarge::Missing
        } else {
            ProofDischarge::Automatic
        };
        self.add_obligation(
            ProofObligationKind::ThreadCapture,
            "thread capture must not move borrowed or MustDrop state across its scope".to_owned(),
            thread.name().map(str::to_owned),
            &discharge,
        );
        self.collect_stmts(thread.body());
    }

    fn collect_unsafe_lifetime(
        &mut self,
        id: &IdRef,
        reason: Option<&Expr>,
        has_safety_doc: bool,
        body: &[Stmt],
    ) {
        let id = id_ref_label(id, "unsafe");
        if let Some(reason) = reason {
            self.collect_expr(reason);
        }
        self.report.unsafe_audits.push(UnsafeAuditSummary {
            id: id.clone(),
            source: None,
            has_reason: reason.is_some(),
            has_safety_doc,
        });
        let discharge = if reason.is_some() && has_safety_doc {
            ProofDischarge::AuditedUnsafe { id: id.clone() }
        } else {
            ProofDischarge::Missing
        };
        self.add_obligation(
            ProofObligationKind::UnsafeLifetimeAudit,
            format!("unsafe lifetime audit `{id}` must include reason and SAFETY docs"),
            Some(id.clone()),
            &discharge,
        );
        self.unsafe_stack.push(id);
        self.collect_stmts(body);
        self.unsafe_stack.pop();
    }

    fn collect_lifetime_write(&mut self, target: &Expr) {
        if let Expr::LifetimePath { key, .. } = target {
            if is_upper_lifetime(key.scope()) {
                self.add_obligation(
                    ProofObligationKind::UpperLifetimeWrite,
                    format!(
                        "upper lifetime write to `{}` needs effect capability or proof",
                        key.as_dotted()
                    ),
                    Some(key.as_dotted()),
                    &ProofDischarge::Missing,
                );
            }
        }
        self.collect_expr(target);
    }

    fn collect_lifetime_drop_pipe(&mut self, lhs: &Expr, rhs: &Expr) {
        let Expr::LifetimePath { key, .. } = lhs else {
            return;
        };
        if is_drop_expr(rhs) {
            self.lifetime_drops.insert(key.clone());
        }
    }

    fn collect_drop_args(&mut self, args: &[Expr]) {
        for arg in args {
            if let Expr::LifetimePath { key, .. } = arg {
                self.lifetime_drops.insert(key.clone());
            }
        }
    }

    fn collect_must_drop_obligations(&mut self) {
        let reads = std::mem::take(&mut self.lifetime_reads);
        for key in reads {
            if is_must_drop_key(&key) && !self.lifetime_drops.contains(&key) {
                self.add_obligation(
                    ProofObligationKind::MustDropDischarge,
                    format!(
                        "MustDrop lifetime value `{}` must be explicitly dropped or transferred",
                        key.as_dotted()
                    ),
                    Some(key.as_dotted()),
                    &ProofDischarge::Missing,
                );
            }
        }
    }

    fn collect_wait(&mut self, target: &arcweft_lang_hir::WaitTarget) {
        match target {
            arcweft_lang_hir::WaitTarget::Duration(expr)
            | arcweft_lang_hir::WaitTarget::Expr(expr) => self.collect_expr(expr),
            arcweft_lang_hir::WaitTarget::Mark(_) => {}
        }
    }

    fn collect_trigger(&mut self, trigger: &TriggerPattern) {
        match trigger {
            TriggerPattern::Signal { target, .. }
            | TriggerPattern::Timeout(target)
            | TriggerPattern::Expr(target) => {
                self.collect_expr(target);
            }
            TriggerPattern::Input(_)
            | TriggerPattern::Event(_)
            | TriggerPattern::Mark(_)
            | TriggerPattern::Select(_)
            | TriggerPattern::Task(_)
            | TriggerPattern::Scope(_) => {}
        }
    }

    fn collect_scope_expr(&mut self, scope: &HirScopeExpr) {
        self.collect_stmts(scope.statements());
        if let Some(value) = scope.value() {
            self.collect_expr(value);
        }
    }

    fn collect_scope_expr_syntax(&mut self, scope: &arcweft_lang_hir::ScopeExprBlock) {
        self.collect_stmts(scope.statements());
        if let Some(value) = scope.value() {
            self.collect_expr(value);
        }
    }

    fn collect_choice_syntax(&mut self, choice: &arcweft_lang_hir::ChoiceBlock) {
        for option in choice.options() {
            if let Some(condition) = option.condition() {
                self.collect_expr(condition);
            }
            if let Some(value) = option.value() {
                self.collect_expr(value);
            }
        }
    }

    fn collect_await_syntax(&mut self, await_with: &arcweft_lang_hir::AwaitWith) {
        self.collect_expr(await_with.expr());
        for branch in await_with.branches() {
            self.collect_flow_items_syntax(branch.body());
        }
    }

    fn collect_flow_items_syntax(&mut self, items: &[arcweft_lang_hir::FlowItem]) {
        for item in items {
            match item {
                arcweft_lang_hir::FlowItem::Stmt(stmt) => self.collect_stmt(stmt),
                arcweft_lang_hir::FlowItem::Raw(raw) => {
                    self.add_raw_obligation(format!("raw flow item: {raw}"), None);
                }
                _ => {}
            }
        }
    }

    fn collect_loop(&mut self, block: &HirLoop) {
        self.collect_flow_items(block.body());
    }

    fn collect_if(&mut self, block: &HirIf) {
        self.collect_expr(block.condition());
        self.collect_flow_items(block.body());
    }

    fn collect_if_let(&mut self, block: &HirIfLet) {
        self.collect_expr(block.expr());
        if let Some(guard) = block.guard() {
            self.collect_expr(guard);
        }
        self.collect_flow_items(block.body());
    }

    fn collect_match(&mut self, block: &HirMatch) {
        self.collect_expr(block.expr());
        for arm in block.arms() {
            if let Some(guard) = arm.guard() {
                self.collect_expr(guard);
            }
            self.collect_flow_items(arm.body());
        }
    }

    fn collect_while(&mut self, block: &HirWhile) {
        self.collect_expr(block.condition());
        self.collect_flow_items(block.body());
    }

    fn collect_while_let(&mut self, block: &HirWhileLet) {
        self.collect_expr(block.expr());
        if let Some(guard) = block.guard() {
            self.collect_expr(guard);
        }
        self.collect_flow_items(block.body());
    }

    fn collect_for(&mut self, block: &HirFor) {
        self.collect_expr(block.source());
        self.collect_flow_items(block.body());
    }

    fn collect_select(&mut self, block: &HirSelect) {
        for branch in block.branches() {
            self.collect_flow_items(branch.body());
        }
    }

    fn collect_borrow(&mut self, block: &HirBorrow) {
        self.collect_expr(block.source());
        self.collect_flow_items(block.body());
    }

    fn collect_await(&mut self, await_with: &HirAwait) {
        self.collect_expr(await_with.expr());
        for branch in await_with.branches() {
            self.collect_flow_items(branch.body());
        }
    }

    fn collect_scope(&mut self, block: &HirScope) {
        self.collect_flow_items(block.body());
    }

    fn add_raw_obligation(&mut self, message: String, subject: Option<String>) {
        self.add_obligation(
            ProofObligationKind::RawSyntax,
            message,
            subject,
            &ProofDischarge::Missing,
        );
    }

    fn add_obligation(
        &mut self,
        kind: ProofObligationKind,
        message: String,
        subject: Option<String>,
        discharge: &ProofDischarge,
    ) {
        self.next_obligation += 1;
        let id = format!("obligation.{:04}", self.next_obligation);
        let smt = Some(SmtProblem {
            name: id.clone(),
            assertions: vec![ProofExpr::App {
                name: obligation_predicate(kind).to_owned(),
                args: subject
                    .as_ref()
                    .map(|subject| vec![ProofExpr::Var(subject.clone())])
                    .unwrap_or_default(),
            }],
        });
        self.report.obligations.push(ProofObligation {
            id: id.clone(),
            kind,
            message: message.clone(),
            subject: subject.clone(),
            source: None,
            discharge: discharge.clone(),
            smt,
        });
        let severity = self.severity_for(kind, discharge);
        if severity != Severity::Info || *discharge == ProofDischarge::Missing {
            self.report.diagnostics.push(VerificationDiagnostic {
                id: format!("diagnostic.{id}"),
                severity,
                message,
                source: None,
                obligation: Some(id),
                related_ids: subject.into_iter().collect(),
                actions: actions_for(kind, discharge),
            });
        }
    }

    fn severity_for(&self, kind: ProofObligationKind, discharge: &ProofDischarge) -> Severity {
        if matches!(
            discharge,
            ProofDischarge::Automatic | ProofDischarge::FormalProof { .. }
        ) {
            return Severity::Info;
        }
        if kind == ProofObligationKind::RawSyntax {
            return Severity::Error;
        }
        if kind == ProofObligationKind::RuntimeConflict {
            return Severity::Error;
        }
        match self.policy.mode {
            VerificationMode::Dev => Severity::Warning,
            VerificationMode::Test
                if matches!(
                    (kind, discharge),
                    (
                        ProofObligationKind::UnsafeLifetimeAudit,
                        ProofDischarge::AuditedUnsafe { .. }
                    ) | (_, ProofDischarge::TrustedAxiom { .. })
                ) =>
            {
                Severity::Warning
            }
            VerificationMode::Test | VerificationMode::Release => Severity::Error,
        }
    }

    fn finish(mut self) -> VerificationReport {
        self.report
            .diagnostics
            .sort_by(|left, right| left.id.cmp(&right.id));
        self.report
    }
}

fn span_from_range(range: &TextRange) -> SourceSpan {
    SourceSpan {
        start: range.start(),
        end: range.end(),
    }
}

fn id_ref_label(id: &IdRef, default_family: &str) -> String {
    match id {
        IdRef::Absolute(entity) => entity.body().to_owned(),
        IdRef::Relative(relative) => format!("{default_family}.{}", relative.suffix()),
        IdRef::FamilyRelative(relative) => {
            format!("{}.{}", relative.family(), relative.relative().suffix())
        }
    }
}

fn proof_arg(args: &[Expr]) -> Option<String> {
    named_entity_arg(args, "proof")
}

fn axiom_arg(args: &[Expr]) -> Option<String> {
    named_entity_arg(args, "axiom").or_else(|| named_entity_arg(args, "trusted_axiom"))
}

fn named_entity_arg(args: &[Expr], name: &str) -> Option<String> {
    args.iter().find_map(|arg| match arg {
        Expr::NamedArg {
            name: arg_name,
            value,
        } if arg_name == name => match value.as_ref() {
            Expr::EntityRef(entity) => entity_label(entity),
            Expr::Path(path) => Some(path.clone()),
            _ => None,
        },
        _ => None,
    })
}

fn entity_label(entity: &EntityRefSyntax) -> Option<String> {
    entity
        .as_absolute()
        .map(|absolute| absolute.body().to_owned())
        .or_else(|| Some(entity.body().to_owned()))
}

fn lifetime_label_arg(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) if path.starts_with('\'') => Some(path.clone()),
        Expr::LifetimePath { key, .. } => Some(format!("'{}", key.as_dotted())),
        _ => None,
    }
}

fn is_upper_lifetime(scope: &LifetimeScopeKind) -> bool {
    matches!(
        scope,
        LifetimeScopeKind::Flow
            | LifetimeScopeKind::Session
            | LifetimeScopeKind::Global
            | LifetimeScopeKind::Persistent
    )
}

fn is_must_drop_key(key: &LifetimeKey) -> bool {
    key.scope() == &LifetimeScopeKind::Line
        && key.path().first().is_some_and(|part| part == "focus")
}

fn is_drop_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call { callee, .. }
            if matches!(callee.as_ref(), Expr::Path(path) if matches!(path.as_str(), "drop" | "drop_optional" | "on_drop"))
    )
}

fn obligation_predicate(kind: ProofObligationKind) -> &'static str {
    match kind {
        ProofObligationKind::LifetimePromotion => "safe_promote",
        ProofObligationKind::UnsafeLifetimeAudit => "unsafe_audit_complete",
        ProofObligationKind::MustDropDischarge => "must_drop_discharged",
        ProofObligationKind::ThreadCapture => "thread_capture_safe",
        ProofObligationKind::ThreadJoinTyping => "thread_join_result_typed",
        ProofObligationKind::UpperLifetimeWrite => "upper_lifetime_write_safe",
        ProofObligationKind::EffectCapability => "effect_capability_available",
        ProofObligationKind::ProofBody => "proof_body_valid",
        ProofObligationKind::TrustedAssumption => "trusted_assumption",
        ProofObligationKind::RawSyntax => "raw_syntax_absent",
        ProofObligationKind::RuntimeConflict => "runtime_conflict_absent",
    }
}

fn actions_for(kind: ProofObligationKind, discharge: &ProofDischarge) -> Vec<ToolAction> {
    if discharge != &ProofDischarge::Missing {
        return Vec::new();
    }
    match kind {
        ProofObligationKind::LifetimePromotion
        | ProofObligationKind::MustDropDischarge
        | ProofObligationKind::ThreadCapture
        | ProofObligationKind::ThreadJoinTyping
        | ProofObligationKind::UpperLifetimeWrite
        | ProofObligationKind::EffectCapability
        | ProofObligationKind::ProofBody => {
            vec![
                ToolAction {
                    id: "action.generate_proof_stub".to_owned(),
                    label: "Generate proof stub".to_owned(),
                    kind: ToolActionKind::GenerateProofStub,
                },
                ToolAction {
                    id: "action.show_obligation".to_owned(),
                    label: "Show proof obligation".to_owned(),
                    kind: ToolActionKind::ShowObligation,
                },
            ]
        }
        ProofObligationKind::UnsafeLifetimeAudit => vec![ToolAction {
            id: "action.generate_unsafe_audit".to_owned(),
            label: "Generate unsafe lifetime audit scaffold".to_owned(),
            kind: ToolActionKind::GenerateUnsafeAudit,
        }],
        ProofObligationKind::TrustedAssumption
        | ProofObligationKind::RawSyntax
        | ProofObligationKind::RuntimeConflict => Vec::new(),
    }
}

fn emit_expr(expr: &ProofExpr) -> String {
    match expr {
        ProofExpr::Bool(value) => value.to_string(),
        ProofExpr::Var(name) => sanitize_symbol(name),
        ProofExpr::Not { expr } => format!("(not {})", emit_expr(expr)),
        ProofExpr::And { exprs } => emit_nary("and", exprs),
        ProofExpr::Or { exprs } => emit_nary("or", exprs),
        ProofExpr::Eq { lhs, rhs } => format!("(= {} {})", emit_expr(lhs), emit_expr(rhs)),
        ProofExpr::App { name, args } => {
            if args.is_empty() {
                sanitize_symbol(name)
            } else {
                format!(
                    "({} {})",
                    sanitize_symbol(name),
                    args.iter().map(emit_expr).collect::<Vec<_>>().join(" ")
                )
            }
        }
    }
}

fn emit_nary(op: &str, exprs: &[ProofExpr]) -> String {
    if exprs.is_empty() {
        return "true".to_owned();
    }
    format!(
        "({op} {})",
        exprs.iter().map(emit_expr).collect::<Vec<_>>().join(" ")
    )
}

fn sanitize_symbol(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '$') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
