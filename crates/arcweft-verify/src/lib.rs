//! Sans I/O verification model for Arcweft HIR.
//!
//! This crate turns structured HIR into proof obligations, diagnostics, and
//! solver-neutral proof problems. It does not read files, spawn processes, or
//! depend on a concrete runtime backend; those responsibilities belong to CLI
//! and solver adapter crates.

use crate::smt::{SmtCheck, SmtError, SmtOutcome, SmtProblem};
use arcweft_compiler::lower::lower_source_line_tasks;
use arcweft_lang_hir::model::{
    HirAwait, HirChoice, HirFlowItem, HirFor, HirFunction, HirIf, HirIfLet, HirLoop, HirMatch,
    HirModule, HirScope, HirScopeExpr, HirSelect, HirTopLevelDecl, HirWhile, HirWhileLet,
};
use arcweft_lang_hir::syntax::{
    ast::{
        choice::{ChoiceBlock, ChoicePlanItem},
        common::TextRange,
        flow::{AwaitWith, FlowItem, ScopeExprBlock, Stmt, ThreadBlock, WaitTarget},
        ids::{EntityRefSyntax, IdRef},
        line_plan::{LinePlan, LinePlanItem, TriggerPattern},
    },
    expr::{CallArg, Expr, LifetimeKey, LifetimeScopeKind},
};
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    semantic::{
        SemanticDiagnostic, SemanticDischarge, SemanticMode, SemanticObligation,
        SemanticObligationKind, SemanticPolicy, SemanticProofTrust, SemanticReport,
        SemanticSeverity, analyze_semantics,
    },
};
use arcweft_source::{
    Diagnostic as SourceDiagnostic, DiagnosticApplicability, DiagnosticCommand, DiagnosticLabel,
    DiagnosticSeverity, DiagnosticSuggestion, SourceDocument, SourceEdit, SourceRange,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

mod assertion;
mod contract_smt;
mod insertion;
pub mod runtime_type;
pub mod smt;

use insertion::{VerifierInsertionInventory, proof_stub_edit, unsafe_audit_edit};
pub use insertion::{VerifierInsertionPolicy, VerifierInsertionTarget};
pub use runtime_type::{
    RuntimeTypeDiagnostic, RuntimeTypeValidationReport, RuntimeTypeValidationStats,
    validate_runtime_plan_types,
};

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

impl BackendKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Emit => "emit",
            Self::Oxiz => "oxiz",
            Self::Z3 => "z3",
        }
    }
}

/// Verifier policy with mode and backend selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerificationPolicy {
    pub mode: VerificationMode,
    pub backend: BackendKind,
    pub allow_trusted_proofs: bool,
}

impl Default for VerificationPolicy {
    fn default() -> Self {
        Self {
            mode: VerificationMode::Dev,
            backend: BackendKind::Emit,
            allow_trusted_proofs: true,
        }
    }
}

/// Verification obligation families understood by Phase 1.5 tooling.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofObligationKind {
    FunctionContract,
    AssertionProof,
    LifetimePromotion,
    UnsafeLifetimeAudit,
    MustDropDischarge,
    ThreadCapture,
    ThreadJoinTyping,
    UpperLifetimeWrite,
    EffectCapability,
    ProofBody,
    TrustedProof,
    TrustedAssumption,
    RawSyntax,
    RuntimeConflict,
}

impl ProofObligationKind {
    const fn is_semantic_owned(self) -> bool {
        matches!(
            self,
            Self::LifetimePromotion
                | Self::UnsafeLifetimeAudit
                | Self::MustDropDischarge
                | Self::ThreadCapture
                | Self::ThreadJoinTyping
                | Self::UpperLifetimeWrite
                | Self::EffectCapability
                | Self::ProofBody
                | Self::TrustedProof
                | Self::TrustedAssumption
                | Self::RawSyntax
                | Self::RuntimeConflict
        )
    }

    pub(crate) const fn owns_proof_insertion_span(self) -> bool {
        matches!(
            self,
            Self::LifetimePromotion
                | Self::MustDropDischarge
                | Self::ThreadCapture
                | Self::ThreadJoinTyping
                | Self::UpperLifetimeWrite
                | Self::EffectCapability
                | Self::ProofBody
        )
    }

    fn actions(self, obligation: &ProofObligation) -> Vec<ToolAction> {
        let discharge = &obligation.discharge;
        if !discharge.is_missing() {
            return Vec::new();
        }
        match self {
            Self::FunctionContract | Self::AssertionProof => vec![ToolAction::show_obligation()],
            Self::LifetimePromotion
            | Self::MustDropDischarge
            | Self::ThreadCapture
            | Self::ThreadJoinTyping
            | Self::UpperLifetimeWrite
            | Self::EffectCapability
            | Self::ProofBody => vec![
                ToolAction::generate_proof_stub(obligation),
                ToolAction::show_obligation(),
            ],
            Self::UnsafeLifetimeAudit => vec![ToolAction::generate_unsafe_audit(obligation)],
            Self::TrustedProof
            | Self::TrustedAssumption
            | Self::RawSyntax
            | Self::RuntimeConflict => Vec::new(),
        }
    }
}

/// How an obligation is discharged, or why it still needs attention.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProofDischarge {
    Automatic,
    FormalProof {
        id: String,
    },
    AuditedUnsafe {
        id: String,
    },
    TrustedProof {
        id: String,
        trusted_dependencies: Vec<String>,
    },
    Solver {
        backend: BackendKind,
    },
    Missing,
}

impl ProofDischarge {
    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }

    pub const fn is_machine_proven(&self) -> bool {
        matches!(
            self,
            Self::Automatic | Self::FormalProof { .. } | Self::Solver { .. }
        )
    }
}

/// One proof obligation produced by semantic analysis.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProofObligation {
    pub id: String,
    pub kind: ProofObligationKind,
    pub message: String,
    pub subject: Option<String>,
    pub source: Option<SourceSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insertion_target: Option<VerifierInsertionTarget>,
    pub discharge: ProofDischarge,
    pub smt: Option<SmtProblem>,
}

impl ProofObligation {
    fn actions(&self) -> Vec<ToolAction> {
        self.kind.actions(self)
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_edit: Option<ToolActionSourceEdit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<ToolActionCommand>,
}

/// Action kind for verifier-assisted edits or navigation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolActionKind {
    GenerateProofStub,
    GenerateUnsafeAudit,
    ShowObligation,
    NavigateToProof,
    NavigateToUnsafeAudit,
}

/// Optional source rewrite attached to an otherwise stable verifier action.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolActionSourceEdit {
    pub span: SourceSpan,
    pub replacement: String,
    pub applicability: ToolActionApplicability,
}

/// Applicability of verifier-provided source edits.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolActionApplicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Unspecified,
}

/// Host/tool command attached to a verifier action.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolActionCommand {
    pub id: String,
    pub title: String,
}

impl ToolAction {
    pub(crate) fn generate_proof_stub(obligation: &ProofObligation) -> Self {
        let mut action = Self {
            id: "action.generate_proof_stub".to_owned(),
            label: "Generate proof stub".to_owned(),
            kind: ToolActionKind::GenerateProofStub,
            source_edit: None,
            command: Some(ToolActionCommand::new(
                "arcweft.verify.generateProofStub",
                "Generate proof stub",
            )),
        };
        if let Some((span, replacement, applicability)) = proof_stub_edit(obligation) {
            action = action.with_source_edit(span, replacement, applicability);
        }
        action
    }

    pub(crate) fn show_obligation() -> Self {
        Self {
            id: "action.show_obligation".to_owned(),
            label: "Show proof obligation".to_owned(),
            kind: ToolActionKind::ShowObligation,
            source_edit: None,
            command: Some(ToolActionCommand::new(
                "arcweft.verify.showObligation",
                "Show proof obligation",
            )),
        }
    }

    pub(crate) fn generate_unsafe_audit(obligation: &ProofObligation) -> Self {
        let mut action = Self {
            id: "action.generate_unsafe_audit".to_owned(),
            label: "Generate unsafe lifetime audit metadata".to_owned(),
            kind: ToolActionKind::GenerateUnsafeAudit,
            source_edit: None,
            command: Some(ToolActionCommand::new(
                "arcweft.verify.generateUnsafeAudit",
                "Generate unsafe lifetime audit metadata",
            )),
        };
        if let Some((span, replacement, applicability)) = unsafe_audit_edit(obligation) {
            action = action.with_source_edit(span, replacement, applicability);
        }
        action
    }

    #[must_use]
    pub fn with_source_edit(
        mut self,
        span: SourceSpan,
        replacement: impl Into<String>,
        applicability: ToolActionApplicability,
    ) -> Self {
        self.source_edit = Some(ToolActionSourceEdit {
            span,
            replacement: replacement.into(),
            applicability,
        });
        self
    }

    pub fn source_edit(&self) -> Option<&ToolActionSourceEdit> {
        self.source_edit.as_ref()
    }

    pub fn host_command(&self) -> ToolActionCommand {
        self.command.clone().unwrap_or_else(|| {
            ToolActionCommand::new(format!("arcweft.{}", self.id), self.label.clone())
        })
    }

    pub fn diagnostic_suggestion(&self, document: &SourceDocument) -> Option<DiagnosticSuggestion> {
        let edit = self.source_edit.as_ref()?;
        let span = document
            .span(SourceRange::new(edit.span.start, edit.span.end))
            .ok()?;
        Some(
            DiagnosticSuggestion::new(self.label.clone(), edit.applicability.into())
                .with_edit(SourceEdit::new(span, edit.replacement.clone())),
        )
    }

    pub fn diagnostic_command(&self, obligation: Option<&str>) -> Option<DiagnosticCommand> {
        if self.source_edit.is_some() {
            return None;
        }
        let command = self.host_command();
        let mut diagnostic = DiagnosticCommand::new(command.id, command.title);
        if let Some(obligation) = obligation {
            diagnostic = diagnostic.with_argument(obligation.to_owned());
        }
        Some(diagnostic)
    }
}

impl ToolActionSourceEdit {
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    pub const fn applicability(&self) -> ToolActionApplicability {
        self.applicability
    }
}

impl ToolActionCommand {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }
}

impl From<ToolActionApplicability> for DiagnosticApplicability {
    fn from(value: ToolActionApplicability) -> Self {
        match value {
            ToolActionApplicability::MachineApplicable => Self::MachineApplicable,
            ToolActionApplicability::MaybeIncorrect => Self::MaybeIncorrect,
            ToolActionApplicability::HasPlaceholders => Self::HasPlaceholders,
            ToolActionApplicability::Unspecified => Self::Unspecified,
        }
    }
}

impl From<Severity> for DiagnosticSeverity {
    fn from(value: Severity) -> Self {
        match value {
            Severity::Info => Self::Info,
            Severity::Warning => Self::Warning,
            Severity::Error => Self::Error,
        }
    }
}

impl VerificationDiagnostic {
    pub fn source_diagnostic(&self, document: &SourceDocument) -> SourceDiagnostic {
        let mut diagnostic = SourceDiagnostic::new(self.severity.into(), self.message.clone())
            .with_code(self.id.clone());
        if let Some(span) = self.source
            && let Ok(span) = document.span(SourceRange::new(span.start, span.end))
        {
            diagnostic = diagnostic.with_label(DiagnosticLabel::primary(
                span,
                Some("verifier diagnostic".to_owned()),
            ));
        }
        if let Some(obligation) = &self.obligation {
            diagnostic = diagnostic.with_note(format!("obligation: {obligation}"));
        }
        for related in &self.related_ids {
            diagnostic = diagnostic.with_note(format!("related: {related}"));
        }
        for action in &self.actions {
            if let Some(suggestion) = action.diagnostic_suggestion(document) {
                diagnostic = diagnostic.with_suggestion(suggestion);
            }
            if let Some(command) = action.diagnostic_command(self.obligation.as_deref()) {
                diagnostic = diagnostic.with_command(command);
            }
        }
        diagnostic
    }
}

/// Proof item summary carried into manifests and LSP hovers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProofSummary {
    pub id: String,
    pub source: SourceSpan,
    pub trust: ProofTrustSummary,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_dependencies: Vec<String>,
}

/// Typed proof trust metadata carried into release review manifests.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProofTrustSummary {
    Verified,
    Trusted { reason: String },
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
    pub solver_checks: Vec<SolverCheck>,
    pub proofs: Vec<ProofSummary>,
    pub unsafe_audits: Vec<UnsafeAuditSummary>,
}

/// Result of checking one proof obligation with a solver backend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SolverCheck {
    pub obligation: String,
    pub backend: BackendKind,
    pub outcome: Option<SmtOutcome>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub model: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<String>,
    pub error: Option<String>,
    pub required: bool,
}

/// Verifies an HIR module according to the selected policy.
pub fn verify_module(module: &HirModule, policy: VerificationPolicy) -> VerificationReport {
    verify_module_with_env(module, &TypeCheckEnv::new(), policy)
}

/// Verifies an HIR module with the same type-check environment used by the caller.
pub fn verify_module_with_env(
    module: &HirModule,
    env: &TypeCheckEnv,
    policy: VerificationPolicy,
) -> VerificationReport {
    let insertion_inventory = VerifierInsertionInventory::from_module(module);
    let mut collector = ObligationCollector::new(policy, insertion_inventory);
    collector.collect_module(module);
    let semantic_report = analyze_semantics(
        module,
        env,
        SemanticPolicy {
            mode: semantic_mode(policy.mode),
            allow_trusted_proofs: policy.allow_trusted_proofs,
        },
    );
    let mut report = collector.finish();
    merge_semantic_report(&mut report, semantic_report, insertion_inventory);
    contract_smt::collect_function_contract_obligations(module, &mut report);
    report
        .diagnostics
        .sort_by(|left, right| left.id.cmp(&right.id));
    report
}

impl VerificationReport {
    /// Returns every proof whose evidence is directly or transitively trusted.
    pub fn trusted_proofs(&self) -> impl Iterator<Item = &ProofSummary> {
        self.proofs
            .iter()
            .filter(|proof| !proof.trusted_dependencies.is_empty())
    }

    pub fn source_diagnostics(&self, document: &SourceDocument) -> Vec<SourceDiagnostic> {
        self.diagnostics
            .iter()
            .map(|diagnostic| diagnostic.source_diagnostic(document))
            .collect()
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }

    pub fn has_missing_unsafe_audit_metadata(&self) -> bool {
        self.obligations.iter().any(|obligation| {
            obligation.kind == ProofObligationKind::UnsafeLifetimeAudit
                && matches!(&obligation.discharge, ProofDischarge::Missing)
        })
    }

    /// Runtime-producing flows must not proceed with an unaudited unsafe
    /// lifetime block even when the caller selected a dev verifier policy where
    /// ordinary proof obligations are advisory warnings.
    pub fn has_blocking_runtime_safety_gaps(&self) -> bool {
        self.has_errors() || self.has_missing_unsafe_audit_metadata()
    }

    pub fn has_solver_failures(&self) -> bool {
        self.solver_checks.iter().any(SolverCheck::is_failure)
    }

    pub fn unsafe_audit_count(&self) -> usize {
        self.unsafe_audits.len()
    }

    pub fn record_solver_check(
        &mut self,
        obligation: &str,
        backend: BackendKind,
        result: Result<SmtCheck, SmtError>,
    ) {
        let required = self.solver_check_required(obligation);
        let (outcome, model, raw_output, error) = match result {
            Ok(check) => (Some(check.outcome), check.model, check.raw_output, None),
            Err(error) => (
                None,
                BTreeMap::new(),
                None,
                Some(error.message().to_owned()),
            ),
        };
        let check = SolverCheck {
            obligation: obligation.to_owned(),
            backend,
            outcome,
            model,
            raw_output,
            error,
            required,
        };
        if check.proves_claim()
            && let Some(item) = self
                .obligations
                .iter_mut()
                .find(|item| item.id == obligation)
        {
            item.discharge = ProofDischarge::Solver { backend };
        }
        if check.is_failure() {
            self.diagnostics.push(VerificationDiagnostic {
                id: format!("diagnostic.solver.{}", self.solver_checks.len() + 1),
                severity: Severity::Error,
                message: check.failure_message(),
                source: None,
                obligation: Some(obligation.to_owned()),
                related_ids: vec![obligation.to_owned()],
                actions: vec![ToolAction::show_obligation()],
            });
        }
        self.solver_checks.push(check);
    }

    fn solver_check_required(&self, obligation: &str) -> bool {
        matches!(
            self.policy.mode,
            VerificationMode::Test | VerificationMode::Release
        ) && self
            .obligations
            .iter()
            .find(|item| item.id == obligation)
            .is_some_and(|item| matches!(item.discharge, ProofDischarge::Missing))
    }
}

impl SolverCheck {
    pub fn is_failure(&self) -> bool {
        self.required && !self.proves_claim()
    }

    pub fn proves_claim(&self) -> bool {
        self.outcome.is_some_and(SmtOutcome::proves_claim)
    }

    fn failure_message(&self) -> String {
        if let Some(error) = &self.error {
            return format!(
                "required solver check `{}` failed on {}: {error}",
                self.obligation,
                self.backend.label()
            );
        }
        format!(
            "required solver check `{}` on {} returned {}",
            self.obligation,
            self.backend.label(),
            self.outcome.unwrap_or(SmtOutcome::Unknown).as_str()
        )
    }
}

fn merge_semantic_report(
    report: &mut VerificationReport,
    semantic: SemanticReport,
    insertion_inventory: VerifierInsertionInventory,
) {
    let insertion_context = SemanticInsertionContext::new(&semantic, insertion_inventory);
    remove_collector_semantic_obligations(report);

    for proof in semantic.proofs {
        let summary = ProofSummary {
            id: proof.id,
            source: SourceSpan {
                start: proof.source.start,
                end: proof.source.end,
            },
            trust: match proof.trust {
                SemanticProofTrust::Verified => ProofTrustSummary::Verified,
                SemanticProofTrust::Trusted { reason } => ProofTrustSummary::Trusted { reason },
            },
            trusted_dependencies: proof.trusted_dependencies,
        };
        if let Some(existing) = report
            .proofs
            .iter_mut()
            .find(|existing| existing.id == summary.id)
        {
            *existing = summary;
        } else {
            report.proofs.push(summary);
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
        let (semantic_id, verification_id) =
            merge_semantic_obligation(report, obligation, &insertion_context);
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
        .filter(|obligation| obligation.kind.is_semantic_owned())
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

fn merge_semantic_obligation(
    report: &mut VerificationReport,
    obligation: SemanticObligation,
    insertion_context: &SemanticInsertionContext,
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
    let insertion_target = insertion_context.target_for(kind, subject.as_deref());
    report.obligations.push(ProofObligation {
        id,
        kind,
        message: obligation.message,
        subject,
        source: obligation.source.map(|source| SourceSpan {
            start: source.start,
            end: source.end,
        }),
        insertion_target,
        discharge,
        smt: None,
    });

    (semantic_id, verification_id)
}

struct SemanticInsertionContext {
    proof_inventory: VerifierInsertionInventory,
    unsafe_audits: BTreeMap<String, VerifierInsertionTarget>,
}

impl SemanticInsertionContext {
    fn new(report: &SemanticReport, proof_inventory: VerifierInsertionInventory) -> Self {
        let unsafe_audits = report
            .unsafe_audits
            .iter()
            .filter_map(|audit| {
                let insertion = audit.audit_insertion?;
                Some((
                    audit.id.clone(),
                    VerifierInsertionTarget::unsafe_audit_metadata(
                        SourceSpan {
                            start: insertion.start,
                            end: insertion.end,
                        },
                        audit.has_reason,
                        audit.has_safety_doc,
                    ),
                ))
            })
            .collect();
        Self {
            proof_inventory,
            unsafe_audits,
        }
    }

    fn target_for(
        &self,
        kind: ProofObligationKind,
        subject: Option<&str>,
    ) -> Option<VerifierInsertionTarget> {
        match kind {
            ProofObligationKind::UnsafeLifetimeAudit => {
                subject.and_then(|subject| self.unsafe_audits.get(subject).copied())
            }
            _ => self.proof_inventory.proof_target_for_kind(kind),
        }
    }
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
    let actions = obligation
        .as_ref()
        .and_then(|id| report.obligations.iter().find(|item| &item.id == id))
        .map_or_else(Vec::new, ProofObligation::actions);
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
        actions,
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
        SemanticObligationKind::TrustedProof => ProofObligationKind::TrustedProof,
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
        SemanticDischarge::TrustedProof {
            id,
            trusted_dependencies,
        } => ProofDischarge::TrustedProof {
            id,
            trusted_dependencies,
        },
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
    insertion_inventory: VerifierInsertionInventory,
    unsafe_stack: Vec<String>,
    known_proofs: BTreeSet<String>,
    lifetime_reads: Vec<LifetimeKey>,
    lifetime_drops: HashSet<LifetimeKey>,
}

impl ObligationCollector {
    fn new(policy: VerificationPolicy, insertion_inventory: VerifierInsertionInventory) -> Self {
        Self {
            policy,
            report: VerificationReport {
                policy,
                ..VerificationReport::default()
            },
            next_obligation: 0,
            insertion_inventory,
            unsafe_stack: Vec::new(),
            known_proofs: BTreeSet::new(),
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
        if let Err(errors) = lower_source_line_tasks(module) {
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
                    self.known_proofs.insert(id);
                }
                HirTopLevelDecl::Source(source) => {
                    self.collect_stmts(source.item().body_statements());
                }
                _ => {}
            }
        }
    }

    fn collect_function(&mut self, function: &HirFunction) {
        self.collect_stmts(function.statements());
        if let Some(value) = function.value() {
            self.collect_expr(value.expr());
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
            HirFlowItem::SourceLocale(block) => self.collect_flow_items(block.body()),
            HirFlowItem::Scope(block) => self.collect_scope(block),
            HirFlowItem::Thread(thread) => self.collect_flow_items(thread.body()),
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

    fn collect_choice_plan_item(&mut self, item: &ChoicePlanItem) {
        match item {
            ChoicePlanItem::Option { value, .. } => {
                self.collect_expr(value);
            }
            ChoicePlanItem::Timeout { duration, body } => {
                self.collect_expr(duration);
                self.collect_stmts(body);
            }
            ChoicePlanItem::Cancel { trigger, body } => {
                self.collect_trigger(trigger);
                self.collect_stmts(body);
            }
            ChoicePlanItem::OnSelect { body, .. } => {
                self.collect_stmts(body);
            }
            ChoicePlanItem::Raw(raw) => {
                self.add_raw_obligation(
                    format!("raw {:?} recovery node: {}", raw.family(), raw.source()),
                    raw.range().map(|range| format!("{range:?}")),
                );
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
            | LinePlanItem::Expr(value) => self.collect_expr(value),
            LinePlanItem::TimelineAssert(assertion) => {
                self.collect_expr(assertion.condition());
            }
            LinePlanItem::Stmt(stmt) => self.collect_stmt(stmt),
            LinePlanItem::CancelRule(rule) => self.collect_stmts(rule.action()),
            LinePlanItem::TimedCue { anchor, body } => {
                self.collect_expr(anchor);
                self.collect_expr(body);
            }
            LinePlanItem::Raw(raw) => {
                self.add_raw_obligation(
                    format!("raw {:?} recovery node: {}", raw.family(), raw.source()),
                    raw.range().map(|range| format!("{range:?}")),
                );
            }
        }
    }

    fn collect_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Assertion(assertion) => self.collect_assertion(assertion),
            Stmt::LetElse {
                expr, else_body, ..
            } => {
                self.collect_expr(expr.expr());
                self.collect_stmts(else_body);
            }
            Stmt::LetChoice { choice, .. } => self.collect_choice_syntax(choice),
            Stmt::LetScope { scope, .. } => self.collect_scope_expr_syntax(scope),
            Stmt::LetLoop { block, .. } => self.collect_flow_items_syntax(block.body()),
            Stmt::LetAwait { await_with, .. } => self.collect_await_syntax(await_with),
            Stmt::LetActionReceive { action, .. } => self.collect_expr(action.expr()),
            Stmt::Let { expr, .. } | Stmt::Return { expr, .. } | Stmt::Expr { expr, .. } => {
                self.collect_expr(expr);
            }
            Stmt::Out { expr, .. }
            | Stmt::Defer { expr, .. }
            | Stmt::Goto(expr)
            | Stmt::Yield(expr)
            | Stmt::Close(expr)
            | Stmt::Select(expr) => {
                self.collect_expr(expr.expr());
            }
            Stmt::Assign { target, expr } => {
                self.collect_expr(target.expr());
                self.collect_expr(expr.expr());
            }
            Stmt::Thread(thread) => self.collect_thread(thread),
            Stmt::DeferBlock { statements, .. } => self.collect_stmts(statements),
            Stmt::Signal {
                target: condition,
                value: message,
            } => {
                self.collect_expr(condition.expr());
                self.collect_expr(message.expr());
            }
            Stmt::LifetimeSet { target, expr } => {
                self.collect_lifetime_write(target.expr());
                self.collect_expr(expr.expr());
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
                audit_insertion,
                body,
            } => self.collect_unsafe_lifetime(
                id,
                reason.as_ref(),
                *has_safety_doc,
                audit_insertion.as_ref(),
                body,
            ),
            Stmt::If {
                condition,
                body,
                else_body,
            } => self.collect_if_stmt(condition.expr(), body, else_body),
            Stmt::While { condition, body } => {
                self.collect_expr(condition.expr());
                self.collect_stmts(body);
            }
            Stmt::Loop { body } => self.collect_stmts(body),
            Stmt::WhileLet {
                expr, guard, body, ..
            } => {
                self.collect_expr(expr.expr());
                if let Some(guard) = guard {
                    self.collect_expr(guard.expr());
                }
                self.collect_stmts(body);
            }
            Stmt::For { source, body, .. } => {
                self.collect_expr(source.expr());
                self.collect_stmts(body);
            }
            Stmt::Match { expr, arms } => {
                self.collect_expr(expr.expr());
                for arm in arms {
                    if let Some(guard) = arm.guard() {
                        self.collect_expr(guard);
                    }
                    self.collect_stmts(arm.body());
                }
            }
            Stmt::Break {
                expr: Some(expr), ..
            } => self.collect_expr(expr.expr()),
            Stmt::Break { expr: None, .. } | Stmt::Continue { .. } => {}
            Stmt::Raw(raw) => self.add_raw_obligation(
                format!("raw {:?} recovery node: {}", raw.family(), raw.source()),
                raw.range().map(|range| format!("{range:?}")),
            ),
        }
    }

    fn collect_if_stmt(&mut self, condition: &Expr, body: &[Stmt], else_body: &[Stmt]) {
        self.collect_expr(condition);
        self.collect_stmts(body);
        self.collect_stmts(else_body);
    }

    fn collect_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.collect_stmt(stmt);
        }
    }

    fn collect_syntax_flow_items(&mut self, items: &[FlowItem]) {
        for item in items {
            match item {
                FlowItem::Stmt(stmt) => self.collect_stmt(stmt),
                FlowItem::Choice(choice) => self.collect_choice_syntax(choice),
                FlowItem::If(block) => {
                    self.collect_expr(block.condition());
                    self.collect_syntax_flow_items(block.body());
                    self.collect_syntax_flow_items(block.else_body());
                }
                FlowItem::IfLet(block) => {
                    self.collect_expr(block.expr());
                    if let Some(guard) = block.guard() {
                        self.collect_expr(guard);
                    }
                    self.collect_syntax_flow_items(block.body());
                    self.collect_syntax_flow_items(block.else_body());
                }
                FlowItem::Match(block) => {
                    self.collect_expr(block.expr());
                    for arm in block.arms() {
                        if let Some(guard) = arm.guard() {
                            self.collect_expr(guard);
                        }
                        self.collect_syntax_flow_items(arm.body());
                    }
                }
                FlowItem::Loop(block) => self.collect_syntax_flow_items(block.body()),
                FlowItem::While(block) => {
                    self.collect_expr(block.condition());
                    self.collect_syntax_flow_items(block.body());
                }
                FlowItem::WhileLet(block) => {
                    self.collect_expr(block.expr());
                    if let Some(guard) = block.guard() {
                        self.collect_expr(guard);
                    }
                    self.collect_syntax_flow_items(block.body());
                }
                FlowItem::For(block) => {
                    self.collect_expr(block.source());
                    self.collect_syntax_flow_items(block.body());
                }
                FlowItem::Select(block) => {
                    for branch in block.branches() {
                        self.collect_syntax_flow_items(branch.body());
                    }
                }
                FlowItem::SourceLocale(block) => self.collect_syntax_flow_items(block.body()),
                FlowItem::Scope(block) => self.collect_syntax_flow_items(block.body()),
                FlowItem::AwaitWith(await_with) => self.collect_await_syntax(await_with),
                FlowItem::SpeakerLine(_)
                | FlowItem::ContentCall(_)
                | FlowItem::Include(_)
                | FlowItem::Raw(_) => {}
            }
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "expression traversal mirrors the public Expr enum so verifier coverage is auditable"
    )]
    fn collect_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(_)
            | Expr::Path(_)
            | Expr::ShortVariant(_)
            | Expr::Placeholder(_)
            | Expr::NumericBracketSeq(_)
            | Expr::Raw(_) => {
                if let Expr::Raw(raw) = expr {
                    self.add_raw_obligation(format!("raw expression: {raw}"), None);
                }
            }
            Expr::EntityRef(_) => {}
            Expr::LifetimePath { key, .. } => {
                self.lifetime_reads.push(key.clone());
            }
            Expr::Tuple(items) | Expr::BracketSeq(items) => {
                for item in items {
                    self.collect_expr(item);
                }
            }
            Expr::ArrayRepeat { value, len } => {
                self.collect_expr(value);
                self.collect_expr(len);
            }
            Expr::Call(call) => {
                self.collect_call(call.callee(), call.args());
            }
            Expr::Select(select) => self.collect_expr(select.target()),
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
            Expr::Borrow(borrow) => self.collect_expr(borrow.operand()),
            Expr::Deref(deref) => self.collect_expr(deref.operand()),
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

    fn collect_call(&mut self, callee: &Expr, args: &[CallArg]) {
        if let Expr::Path(name) = callee {
            match name.as_str() {
                "promote" => self.add_promote_obligation(args, false),
                "promote_unchecked" => self.add_promote_obligation(args, true),
                "drop" | "drop_optional" | "on_drop" => self.collect_drop_args(args),
                "assume" => self.add_assume_obligation(args),
                _ => {}
            }
        }
        if let Expr::Select(select) = callee {
            match select.member().as_str() {
                "promote" => self.add_promote_obligation(args, false),
                "promote_unchecked" => self.add_promote_obligation(args, true),
                "drop" | "drop_optional" | "on_drop" => {
                    if let Expr::LifetimePath { key, .. } = select.target() {
                        self.lifetime_drops.insert(key.clone());
                    }
                }
                _ => {}
            }
        }
        self.collect_expr(callee);
        for arg in args {
            self.collect_expr(arg.value());
        }
    }

    fn add_promote_obligation(&mut self, args: &[CallArg], unchecked: bool) {
        let proof = proof_arg(args);
        let target = args.first().and_then(|arg| lifetime_label_arg(arg.value()));
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

    fn add_assume_obligation(&mut self, args: &[CallArg]) {
        let discharge = proof_arg(args)
            .filter(|id| self.known_proofs.contains(id))
            .map_or(ProofDischarge::Missing, |id| ProofDischarge::FormalProof {
                id,
            });
        self.add_obligation(
            ProofObligationKind::TrustedAssumption,
            "assume requires a proof dependency".to_owned(),
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
        self.collect_syntax_flow_items(thread.body());
    }

    fn collect_unsafe_lifetime(
        &mut self,
        id: &IdRef,
        reason: Option<&Expr>,
        has_safety_doc: bool,
        audit_insertion: Option<&arcweft_lang_hir::syntax::ast::flow::UnsafeAuditInsertion>,
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
        let insertion_target = audit_insertion
            .filter(|_| reason.is_none() || !has_safety_doc)
            .map(|insertion| span_from_range(insertion.replacement_range()))
            .map(|span| {
                VerifierInsertionTarget::unsafe_audit_metadata(
                    span,
                    reason.is_some(),
                    has_safety_doc,
                )
            });
        self.add_obligation_with_insertion(
            ProofObligationKind::UnsafeLifetimeAudit,
            format!("unsafe lifetime audit `{id}` must include reason and SAFETY docs"),
            Some(id.clone()),
            &discharge,
            insertion_target,
        );
        self.unsafe_stack.push(id);
        self.collect_stmts(body);
        self.unsafe_stack.pop();
    }

    fn collect_lifetime_write(&mut self, target: &Expr) {
        if let Expr::LifetimePath { key, .. } = target
            && is_upper_lifetime(key.scope())
        {
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

    fn collect_drop_args(&mut self, args: &[CallArg]) {
        for arg in args {
            if let Expr::LifetimePath { key, .. } = arg.value() {
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

    fn collect_wait(&mut self, target: &WaitTarget) {
        match target {
            WaitTarget::Duration(expr) | WaitTarget::Expr(expr) => self.collect_expr(expr.expr()),
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

    fn collect_scope_expr_syntax(&mut self, scope: &ScopeExprBlock) {
        self.collect_stmts(scope.statements());
        if let Some(value) = scope.value() {
            self.collect_expr(value);
        }
    }

    fn collect_choice_syntax(&mut self, choice: &ChoiceBlock) {
        for option in choice.options() {
            if let Some(condition) = option.condition() {
                self.collect_expr(condition);
            }
            if let Some(value) = option.value() {
                self.collect_expr(value);
            }
        }
    }

    fn collect_await_syntax(&mut self, await_with: &AwaitWith) {
        self.collect_expr(await_with.expr());
        for branch in await_with.branches() {
            self.collect_flow_items_syntax(branch.body());
        }
    }

    fn collect_flow_items_syntax(&mut self, items: &[FlowItem]) {
        for item in items {
            match item {
                FlowItem::Stmt(stmt) => self.collect_stmt(stmt),
                FlowItem::Raw(raw) => {
                    self.add_raw_obligation(
                        format!("raw {:?} recovery node: {}", raw.family(), raw.source()),
                        raw.range().map(|range| format!("{range:?}")),
                    );
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
        let insertion_target = self.insertion_inventory.proof_target_for_kind(kind);
        self.add_obligation_with_insertion(kind, message, subject, discharge, insertion_target);
    }

    fn add_obligation_with_insertion(
        &mut self,
        kind: ProofObligationKind,
        message: String,
        subject: Option<String>,
        discharge: &ProofDischarge,
        insertion_target: Option<VerifierInsertionTarget>,
    ) {
        self.record_obligation(
            kind,
            message,
            subject,
            discharge,
            insertion_target,
            None,
            None,
        );
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "one verifier record atomically owns its typed kind, source, discharge, insertion, and diagnostic identity"
    )]
    fn record_obligation(
        &mut self,
        kind: ProofObligationKind,
        message: String,
        subject: Option<String>,
        discharge: &ProofDischarge,
        insertion_target: Option<VerifierInsertionTarget>,
        source: Option<SourceSpan>,
        diagnostic_id: Option<&'static str>,
    ) {
        self.next_obligation += 1;
        let id = format!("obligation.{:04}", self.next_obligation);
        let obligation = ProofObligation {
            id: id.clone(),
            kind,
            message: message.clone(),
            subject: subject.clone(),
            source,
            insertion_target,
            discharge: discharge.clone(),
            smt: None,
        };
        let actions = obligation.actions();
        self.report.obligations.push(obligation);
        let severity = self.severity_for(kind, discharge);
        if severity != Severity::Info || *discharge == ProofDischarge::Missing {
            self.report.diagnostics.push(VerificationDiagnostic {
                id: diagnostic_id.map_or_else(|| format!("diagnostic.{id}"), str::to_owned),
                severity,
                message,
                source,
                obligation: Some(id),
                related_ids: subject.into_iter().collect(),
                actions,
            });
        }
    }

    fn severity_for(&self, kind: ProofObligationKind, discharge: &ProofDischarge) -> Severity {
        if matches!(
            discharge,
            ProofDischarge::Automatic
                | ProofDischarge::FormalProof { .. }
                | ProofDischarge::Solver { .. }
        ) {
            return Severity::Info;
        }
        if matches!(
            kind,
            ProofObligationKind::RawSyntax | ProofObligationKind::RuntimeConflict
        ) {
            return Severity::Error;
        }
        if matches!(discharge, ProofDischarge::TrustedProof { .. })
            || kind == ProofObligationKind::TrustedProof
        {
            return if self.policy.allow_trusted_proofs {
                Severity::Warning
            } else {
                Severity::Error
            };
        }
        match self.policy.mode {
            VerificationMode::Dev => Severity::Warning,
            VerificationMode::Test
                if matches!(
                    (kind, discharge),
                    (
                        ProofObligationKind::UnsafeLifetimeAudit,
                        ProofDischarge::AuditedUnsafe { .. }
                    )
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

fn proof_arg(args: &[CallArg]) -> Option<String> {
    named_entity_arg(args, "proof")
}

fn named_entity_arg(args: &[CallArg], name: &str) -> Option<String> {
    args.iter().find_map(|arg| match arg {
        CallArg::Named {
            name: arg_name,
            value,
        } if arg_name == name => match value.as_ref() {
            Expr::EntityRef(entity) => entity_label(entity),
            Expr::Path(path) => Some(path.as_label().to_owned()),
            Expr::ShortVariant(name) => Some(format!(".{name}")),
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
        Expr::Path(path) if path.starts_with('\'') => Some(path.as_label().to_owned()),
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
        Expr::Call(call)
            if matches!(call.callee(), Expr::Path(path) if matches!(path.as_str(), "drop" | "drop_optional" | "on_drop"))
    )
}

#[cfg(test)]
mod tests;
