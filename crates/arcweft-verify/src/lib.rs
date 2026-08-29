//! Sans-I/O verification over the accepted final-HIR project generation.
//!
//! Verification consumes one executable [`HirExecutableProjectView`], its
//! exact [`ProjectSymbolTable`], and the matching [`FinalSemanticAnalysis`].
//! It never lowers syntax, links or clones HIR modules, reparses source text,
//! or rebuilds semantic facts from presentation labels.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    body_edges::{HirBodyChild, HirBodyChildRole},
    identity::{ExprId, ItemId, StmtId},
    item::{HirItemKind, HirPredicateBody, HirProofBody},
    module::HirModule,
    project::HirExecutableProjectView,
    source_index::{
        HirDeclarationSourceRole, HirItemSourceRole, HirSourcePresence, HirSourceQuery,
        HirSourceQueryError, HirSourceSite, HirStmtSourceRole,
    },
    stmt::{HirStatementBodyRole, HirStatementChild, HirStatementChildRole, HirStmtKind},
    symbol::{
        CallableDeclarationKey, CallableDeclarationOwner, ProjectSymbolTable, ProofArtifactId,
        ProofArtifactIdentityError,
    },
};
use arcweft_lang_sema::final_analysis::{
    CheckedAssertionDisposition, CheckedItemRole, CheckedStatementPayload, CheckedUnsafeAudit,
    FinalSemanticAnalysis, FinalSemanticAnalysisError,
};
use arcweft_lang_sema::types::TypeKind;
use arcweft_source::{
    Diagnostic as SourceDiagnostic, DiagnosticApplicability, DiagnosticCommand, DiagnosticLabel,
    DiagnosticSeverity, DiagnosticSuggestion, SourceDocument, SourceDocumentIdentity, SourceEdit,
    SourceRange,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::insertion::{VerifierInsertionInventory, proof_stub_edit};
use crate::smt::{SmtCheck, SmtError, SmtOutcome, SmtProblem};

mod call_witness;
mod insertion;
pub mod smt;

pub use call_witness::ProofCallWitnessProjection;
pub use insertion::{VerifierInsertionPolicy, VerifierInsertionTarget};

/// Revision-bound source span used by verifier diagnostics and edit actions.
///
/// This is a transport projection of `arcweft_source::SourceSpan`. The exact
/// document identity is mandatory so a consumer cannot apply offsets to a
/// different document or source revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub source: SourceDocumentIdentity,
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    pub fn from_exact(span: &arcweft_source::SourceSpan) -> Self {
        let range = span.range();
        Self {
            source: span.source().clone(),
            start: range.start(),
            end: range.end(),
        }
    }

    fn from_site(site: &HirSourceSite) -> Self {
        match site {
            HirSourceSite::Span(span) => Self::from_exact(span),
            HirSourceSite::Insertion(insertion) => Self {
                source: insertion.source_identity().clone(),
                start: insertion.offset(),
                end: insertion.offset(),
            },
        }
    }

    pub fn validate_for(&self, document: &SourceDocument) -> bool {
        &self.source == document.identity()
            && self.start <= self.end
            && self.end <= document.text().len()
            && document.text().is_char_boundary(self.start)
            && document.text().is_char_boundary(self.end)
    }
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
    #[default]
    Dev,
    Test,
    Release,
}

/// Solver family selected by tooling.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    #[default]
    Emit,
    Oxiz,
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

/// Verification obligation families understood by verifier tooling.
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
    RuntimeConflict,
}

impl ProofObligationKind {
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
        if !obligation.discharge.is_missing() {
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
            Self::UnsafeLifetimeAudit => vec![ToolAction::generate_unsafe_audit()],
            Self::TrustedProof | Self::TrustedAssumption | Self::RuntimeConflict => Vec::new(),
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

/// One proof obligation derived from typed HIR and exact semantic facts.
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
    /// Session-only proof identity. Serialized labels never replace it.
    #[serde(skip)]
    pub proof_artifact: Option<ProofArtifactId>,
    /// Session-only statement identity for assertion and unsafe-audit rows.
    #[serde(skip)]
    pub statement: Option<StmtId>,
}

impl ProofObligation {
    fn actions(&self) -> Vec<ToolAction> {
        self.kind.actions(self)
    }
}

/// JSON diagnostic shared by CLI, LSP, and Agent tooling.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolActionKind {
    GenerateProofStub,
    GenerateUnsafeAudit,
    ShowObligation,
    NavigateToProof,
    NavigateToUnsafeAudit,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolActionSourceEdit {
    pub span: SourceSpan,
    pub replacement: String,
    pub applicability: ToolActionApplicability,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolActionApplicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Unspecified,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolActionCommand {
    pub id: String,
    pub title: String,
}

impl ToolAction {
    fn generate_proof_stub(obligation: &ProofObligation) -> Self {
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

    fn show_obligation() -> Self {
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

    fn generate_unsafe_audit() -> Self {
        Self {
            id: "action.generate_unsafe_audit".to_owned(),
            label: "Generate unsafe lifetime audit metadata".to_owned(),
            kind: ToolActionKind::GenerateUnsafeAudit,
            source_edit: None,
            command: Some(ToolActionCommand::new(
                "arcweft.verify.generateUnsafeAudit",
                "Generate unsafe lifetime audit metadata",
            )),
        }
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
        if !edit.span.validate_for(document) {
            return None;
        }
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
    pub const fn span(&self) -> &SourceSpan {
        &self.span
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
        if let Some(span) = &self.source
            && span.validate_for(document)
            && let Ok(exact) = document.span(SourceRange::new(span.start, span.end))
        {
            diagnostic = diagnostic.with_label(DiagnosticLabel::primary(
                exact,
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
///
/// `id` is presentation only. [`VerificationReport::proof_artifacts`] is the
/// sole identity inventory used for semantic joins.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProofSummary {
    pub id: String,
    pub source: SourceSpan,
    pub trust: ProofTrustSummary,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_dependencies: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProofTrustSummary {
    Pending,
    Verified,
    Trusted { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnsafeAuditSummary {
    pub id: String,
    pub source: Option<SourceSpan>,
    pub has_reason: bool,
    pub has_safety_doc: bool,
    /// Typed owner used for all in-process joins.
    #[serde(skip)]
    pub statement: Option<StmtId>,
}

/// Complete verifier report for one accepted project generation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerificationReport {
    pub policy: VerificationPolicy,
    pub diagnostics: Vec<VerificationDiagnostic>,
    pub obligations: Vec<ProofObligation>,
    pub solver_checks: Vec<SolverCheck>,
    pub proofs: Vec<ProofSummary>,
    pub unsafe_audits: Vec<UnsafeAuditSummary>,
    /// Session-only identities matching `proofs` in the same source order.
    #[serde(skip)]
    pub proof_artifacts: Vec<ProofArtifactId>,
    /// Session-only, bounded Call evidence projected from complete semantic facts.
    #[serde(skip)]
    pub call_witnesses: Vec<ProofCallWitnessProjection>,
}

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

/// Rejection of a verifier input before any report is published.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum VerificationInputError {
    #[error(transparent)]
    SemanticGeneration(#[from] FinalSemanticAnalysisError),
    #[error(transparent)]
    SourceQuery(#[from] HirSourceQueryError),
    #[error(transparent)]
    ProofIdentity(#[from] ProofArtifactIdentityError),
    #[error("final semantic analysis is missing item fact for {owner:?}")]
    MissingItemFact { owner: ItemId },
    #[error("final semantic analysis is missing statement fact for {owner:?}")]
    MissingStatementFact { owner: StmtId },
    #[error("final semantic item fact disagrees with HIR item {owner:?}")]
    ItemRoleMismatch { owner: ItemId },
    #[error("final semantic statement fact disagrees with HIR statement {owner:?}")]
    StatementRoleMismatch { owner: StmtId },
    #[error("final semantic expression fact is missing for {owner:?}")]
    MissingExpressionFact { owner: ExprId },
    #[error("proof item {owner:?} is absent from the registered callable authority")]
    MissingProofSymbol { owner: ItemId },
    #[error("typed HIR source role for {owner:?} has no present source site")]
    MissingSourceSite { owner: VerificationOwner },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationOwner {
    Item(ItemId),
    Statement(StmtId),
}

/// Verifies one exact accepted final-HIR generation.
pub fn verify_project(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    semantics: &FinalSemanticAnalysis,
    policy: VerificationPolicy,
) -> Result<VerificationReport, VerificationInputError> {
    semantics.validate_generation(project, symbols)?;
    let insertion = VerifierInsertionInventory::from_project(project);
    let mut verifier = ProjectVerifier {
        project,
        symbols,
        semantics,
        insertion,
        report: VerificationReport {
            policy,
            ..VerificationReport::default()
        },
    };
    verifier.verify()?;
    Ok(verifier.finish())
}

struct ProjectVerifier<'project, 'catalog> {
    project: HirExecutableProjectView<'project>,
    symbols: &'catalog ProjectSymbolTable,
    semantics: &'catalog FinalSemanticAnalysis,
    insertion: VerifierInsertionInventory,
    report: VerificationReport,
}

impl ProjectVerifier<'_, '_> {
    fn verify(&mut self) -> Result<(), VerificationInputError> {
        for item in self.project.items() {
            let owner = item.id();
            let checked = self
                .semantics
                .item(owner)
                .ok_or(VerificationInputError::MissingItemFact { owner })?;
            if checked.role().family() != item.item().kind().family() {
                return Err(VerificationInputError::ItemRoleMismatch { owner });
            }
            match item.item().kind() {
                HirItemKind::Predicate(predicate) => {
                    self.validate_predicate(item.module(), predicate)?;
                }
                HirItemKind::Proof(proof) => {
                    if !matches!(checked.role(), CheckedItemRole::Proof) {
                        return Err(VerificationInputError::ItemRoleMismatch { owner });
                    }
                    self.collect_proof(item.module(), owner, proof)?;
                }
                _ => {}
            }
        }
        for (_, module) in self.project.modules() {
            for (owner, statement) in module.statements() {
                self.collect_statement(module, owner, statement.kind())?;
            }
        }
        Ok(())
    }

    fn validate_predicate(
        &self,
        module: &HirModule,
        predicate: &arcweft_lang_hir::item::HirPredicate,
    ) -> Result<(), VerificationInputError> {
        self.validate_expressions(predicate.requires())?;
        self.validate_expressions(predicate.ensures())?;
        match predicate.body() {
            HirPredicateBody::Expression { expression, .. } => {
                self.validate_expression(*expression)
            }
            HirPredicateBody::Block {
                statements, tail, ..
            } => {
                self.validate_statements(module, statements)?;
                self.validate_expression(*tail)
            }
            HirPredicateBody::Error { expression, .. } => {
                Err(VerificationInputError::MissingExpressionFact { owner: *expression })
            }
        }
    }

    fn collect_proof(
        &mut self,
        module: &HirModule,
        owner: ItemId,
        proof: &arcweft_lang_hir::item::HirProof,
    ) -> Result<(), VerificationInputError> {
        self.validate_expressions(proof.requires())?;
        self.validate_expressions(proof.ensures())?;
        match proof.body() {
            HirProofBody::Expression { expression, .. } => {
                self.validate_expression(*expression)?;
            }
            HirProofBody::Block {
                statements, tail, ..
            } => {
                self.validate_statements(module, statements)?;
                self.validate_expression(*tail)?;
            }
            HirProofBody::Error { expression, .. } => {
                return Err(VerificationInputError::MissingExpressionFact { owner: *expression });
            }
        }

        let symbol = self
            .symbols
            .callable_symbols()
            .find(|symbol| {
                symbol.source_item() == owner && symbol.owner() == CallableDeclarationOwner::Proof
            })
            .ok_or(VerificationInputError::MissingProofSymbol { owner })?;
        let CallableDeclarationKey::Existing(declaration) = symbol.declaration() else {
            return Err(VerificationInputError::MissingProofSymbol { owner });
        };
        let artifact = self
            .symbols
            .proof_artifact(self.project.project_view(), declaration)?;
        let source = item_source(module, owner)?;
        let label = declaration.qualified_name();
        let obligation_id = format!("proof.{label}.body");
        let obligation = ProofObligation {
            id: obligation_id.clone(),
            kind: ProofObligationKind::ProofBody,
            message: format!("proof `{label}` awaits typed proof discharge"),
            subject: Some(label.clone()),
            source: Some(source.clone()),
            insertion_target: self
                .insertion
                .proof_target_for_kind(module.module_id(), ProofObligationKind::ProofBody),
            discharge: ProofDischarge::Missing,
            smt: None,
            proof_artifact: Some(artifact.clone()),
            statement: None,
        };
        self.report.diagnostics.push(missing_obligation_diagnostic(
            &obligation,
            self.report.policy.mode,
        ));
        self.report.obligations.push(obligation);
        self.report.proofs.push(ProofSummary {
            id: label,
            source,
            trust: ProofTrustSummary::Pending,
            trusted_dependencies: Vec::new(),
        });
        self.report.proof_artifacts.push(artifact);
        Ok(())
    }

    fn collect_statement(
        &mut self,
        module: &HirModule,
        owner: StmtId,
        statement: &HirStmtKind,
    ) -> Result<(), VerificationInputError> {
        let checked = self
            .semantics
            .statement(owner)
            .ok_or(VerificationInputError::MissingStatementFact { owner })?;
        match checked.payload() {
            CheckedStatementPayload::Assertion(disposition) => {
                self.collect_assertion(module, owner, statement, *disposition)
            }
            CheckedStatementPayload::UnsafeAudit(audit) => {
                let audit = audit.clone();
                self.collect_unsafe_audit(module, owner, statement, &audit)
            }
            _ if matches!(
                statement,
                HirStmtKind::Assertion { .. } | HirStmtKind::UnsafeLifetime { .. }
            ) =>
            {
                Err(VerificationInputError::StatementRoleMismatch { owner })
            }
            _ => Ok(()),
        }
    }

    fn collect_assertion(
        &mut self,
        module: &HirModule,
        owner: StmtId,
        statement: &HirStmtKind,
        disposition: CheckedAssertionDisposition,
    ) -> Result<(), VerificationInputError> {
        if !matches!(statement, HirStmtKind::Assertion { .. }) {
            return Err(VerificationInputError::StatementRoleMismatch { owner });
        }
        let conditions = self.checked_assertion_conditions(owner, statement)?;
        if !matches!(disposition, CheckedAssertionDisposition::PendingProof) {
            return Ok(());
        }
        let source = statement_source(module, owner)?;
        for (index, _) in conditions.iter().enumerate() {
            let obligation = ProofObligation {
                id: format!("assertion.{owner:?}.condition.{index}"),
                kind: ProofObligationKind::AssertionProof,
                message: format!("assert.prove condition {index} requires compile-time discharge"),
                subject: Some(format!("condition.{index}")),
                source: Some(source.clone()),
                insertion_target: None,
                discharge: ProofDischarge::Missing,
                smt: None,
                proof_artifact: None,
                statement: Some(owner),
            };
            self.report
                .diagnostics
                .push(unresolved_prove_diagnostic(&obligation));
            self.report.obligations.push(obligation);
        }
        Ok(())
    }

    fn checked_assertion_conditions(
        &self,
        owner: StmtId,
        statement: &HirStmtKind,
    ) -> Result<Vec<ExprId>, VerificationInputError> {
        let edges = statement
            .try_child_edges()
            .map_err(|_| VerificationInputError::StatementRoleMismatch { owner })?;
        if edges.is_empty() {
            return Err(VerificationInputError::StatementRoleMismatch { owner });
        }
        edges
            .into_iter()
            .enumerate()
            .map(|(index, edge)| {
                let expected = u32::try_from(index)
                    .map_err(|_| VerificationInputError::StatementRoleMismatch { owner })?;
                let (
                    HirStatementChildRole::AssertionCondition { ordinal },
                    HirStatementChild::Expression(expression),
                ) = (edge.role(), edge.child())
                else {
                    return Err(VerificationInputError::StatementRoleMismatch { owner });
                };
                if ordinal != expected {
                    return Err(VerificationInputError::StatementRoleMismatch { owner });
                }
                let checked = self
                    .semantics
                    .expression(expression)
                    .ok_or(VerificationInputError::MissingExpressionFact { owner: expression })?;
                if checked.ty() != &TypeKind::Bool || !checked.effects().is_empty() {
                    return Err(VerificationInputError::StatementRoleMismatch { owner });
                }
                Ok(expression)
            })
            .collect()
    }

    fn collect_unsafe_audit(
        &mut self,
        module: &HirModule,
        owner: StmtId,
        statement: &HirStmtKind,
        audit: &CheckedUnsafeAudit,
    ) -> Result<(), VerificationInputError> {
        let has_reason = self.checked_unsafe_audit_children(owner, statement)?;
        let source = statement_source(module, owner)?;
        let id = audit.id().as_public_id().as_str().to_owned();
        let complete = has_reason && audit.has_safety_doc();
        self.report.unsafe_audits.push(UnsafeAuditSummary {
            id: id.clone(),
            source: Some(source.clone()),
            has_reason,
            has_safety_doc: audit.has_safety_doc(),
            statement: Some(owner),
        });
        if !complete {
            let obligation = ProofObligation {
                id: format!("unsafe_audit.{owner:?}"),
                kind: ProofObligationKind::UnsafeLifetimeAudit,
                message: format!(
                    "unsafe lifetime audit `{id}` requires reason and SAFETY documentation"
                ),
                subject: Some(id),
                source: Some(source),
                insertion_target: None,
                discharge: ProofDischarge::Missing,
                smt: None,
                proof_artifact: None,
                statement: Some(owner),
            };
            self.report.diagnostics.push(missing_obligation_diagnostic(
                &obligation,
                self.report.policy.mode,
            ));
            self.report.obligations.push(obligation);
        }
        Ok(())
    }

    fn checked_unsafe_audit_children(
        &self,
        owner: StmtId,
        statement: &HirStmtKind,
    ) -> Result<bool, VerificationInputError> {
        let bodies = statement
            .body_projections()
            .map_err(|_| VerificationInputError::StatementRoleMismatch { owner })?;
        let [body] = bodies.as_slice() else {
            return Err(VerificationInputError::StatementRoleMismatch { owner });
        };
        if body.role() != &HirStatementBodyRole::UnsafeLifetime {
            return Err(VerificationInputError::StatementRoleMismatch { owner });
        }
        let body_statements = body
            .children()
            .iter()
            .enumerate()
            .map(|(index, edge)| {
                let expected = u32::try_from(index)
                    .map_err(|_| VerificationInputError::StatementRoleMismatch { owner })?;
                let (HirBodyChildRole::Statement { ordinal }, HirBodyChild::Statement(child)) =
                    (edge.role(), edge.child())
                else {
                    return Err(VerificationInputError::StatementRoleMismatch { owner });
                };
                if ordinal != expected {
                    return Err(VerificationInputError::StatementRoleMismatch { owner });
                }
                self.semantics
                    .statement(child)
                    .ok_or(VerificationInputError::MissingStatementFact { owner: child })?;
                Ok(child)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut reason = None;
        let mut next_body = 0_usize;
        for edge in statement
            .try_child_edges()
            .map_err(|_| VerificationInputError::StatementRoleMismatch { owner })?
        {
            match (edge.role(), edge.child()) {
                (
                    HirStatementChildRole::UnsafeReason,
                    HirStatementChild::Expression(expression),
                ) if reason.is_none() => {
                    let checked = self.semantics.expression(expression).ok_or(
                        VerificationInputError::MissingExpressionFact { owner: expression },
                    )?;
                    if checked.ty() != &TypeKind::String || !checked.effects().is_empty() {
                        return Err(VerificationInputError::StatementRoleMismatch { owner });
                    }
                    reason = Some(expression);
                }
                (
                    HirStatementChildRole::BodyItem {
                        body: HirStatementBodyRole::UnsafeLifetime,
                        ordinal,
                    },
                    HirStatementChild::Statement(child),
                ) if u32::try_from(next_body).ok() == Some(ordinal)
                    && body_statements.get(next_body) == Some(&child) =>
                {
                    next_body += 1;
                }
                _ => return Err(VerificationInputError::StatementRoleMismatch { owner }),
            }
        }
        if next_body != body_statements.len() {
            return Err(VerificationInputError::StatementRoleMismatch { owner });
        }
        Ok(reason.is_some())
    }

    fn validate_statements(
        &self,
        module: &HirModule,
        statements: &[StmtId],
    ) -> Result<(), VerificationInputError> {
        for &owner in statements {
            module
                .resolve_stmt(owner)
                .map_err(|_| VerificationInputError::MissingStatementFact { owner })?;
            self.semantics
                .statement(owner)
                .ok_or(VerificationInputError::MissingStatementFact { owner })?;
        }
        Ok(())
    }

    fn validate_expressions(&self, expressions: &[ExprId]) -> Result<(), VerificationInputError> {
        for &owner in expressions {
            self.validate_expression(owner)?;
        }
        Ok(())
    }

    fn validate_expression(&self, owner: ExprId) -> Result<(), VerificationInputError> {
        self.semantics
            .expression(owner)
            .map(|_| ())
            .ok_or(VerificationInputError::MissingExpressionFact { owner })
    }

    fn finish(mut self) -> VerificationReport {
        self.report.call_witnesses = self
            .semantics
            .calls()
            .map(|(_, facts)| ProofCallWitnessProjection::from_facts(facts))
            .collect();
        self.report.diagnostics.sort_by(|left, right| {
            source_sort_key(left.source.as_ref())
                .cmp(&source_sort_key(right.source.as_ref()))
                .then_with(|| left.id.cmp(&right.id))
                .then_with(|| left.message.cmp(&right.message))
        });
        self.report
    }
}

fn item_source(module: &HirModule, owner: ItemId) -> Result<SourceSpan, VerificationInputError> {
    source_lookup(
        module,
        HirSourceQuery::Item {
            owner,
            role: HirItemSourceRole::Declaration(HirDeclarationSourceRole::Whole),
        },
        VerificationOwner::Item(owner),
    )
}

fn statement_source(
    module: &HirModule,
    owner: StmtId,
) -> Result<SourceSpan, VerificationInputError> {
    source_lookup(
        module,
        HirSourceQuery::Stmt {
            owner,
            role: HirStmtSourceRole::Whole,
        },
        VerificationOwner::Statement(owner),
    )
}

fn source_lookup(
    module: &HirModule,
    query: HirSourceQuery,
    owner: VerificationOwner,
) -> Result<SourceSpan, VerificationInputError> {
    let lookup = module.source_site(module.provenance().source_identity(), query)?;
    match lookup.presence() {
        HirSourcePresence::Present(site) => Ok(SourceSpan::from_site(site)),
        HirSourcePresence::AbsentOptional => {
            Err(VerificationInputError::MissingSourceSite { owner })
        }
    }
}

fn missing_obligation_diagnostic(
    obligation: &ProofObligation,
    mode: VerificationMode,
) -> VerificationDiagnostic {
    VerificationDiagnostic {
        id: format!("verify.pending.{}", obligation.id),
        severity: match mode {
            VerificationMode::Dev => Severity::Warning,
            VerificationMode::Test | VerificationMode::Release => Severity::Error,
        },
        message: obligation.message.clone(),
        source: obligation.source.clone(),
        obligation: Some(obligation.id.clone()),
        related_ids: obligation.subject.iter().cloned().collect(),
        actions: obligation.actions(),
    }
}

fn unresolved_prove_diagnostic(obligation: &ProofObligation) -> VerificationDiagnostic {
    VerificationDiagnostic {
        id: "verify.proof.unresolved".to_owned(),
        severity: Severity::Error,
        message: obligation.message.clone(),
        source: obligation.source.clone(),
        obligation: Some(obligation.id.clone()),
        related_ids: obligation.subject.iter().cloned().collect(),
        actions: obligation.actions(),
    }
}

fn source_sort_key(source: Option<&SourceSpan>) -> (&str, usize, usize) {
    source.map_or(("", 0, 0), |source| {
        (source.source.id().as_str(), source.start, source.end)
    })
}

impl VerificationReport {
    /// Returns final-HIR-identity-ordered bounded Call evidence for this generation.
    pub fn call_witnesses(&self) -> &[ProofCallWitnessProjection] {
        &self.call_witnesses
    }

    /// Returns bounded Proof evidence for one exact final-HIR Call identity.
    pub fn call_witness(&self, expression: ExprId) -> Option<&ProofCallWitnessProjection> {
        self.call_witnesses
            .iter()
            .find(|projection| projection.expression() == expression)
    }

    pub fn trusted_proofs(&self) -> impl Iterator<Item = &ProofSummary> {
        self.proofs
            .iter()
            .filter(|proof| !proof.trusted_dependencies.is_empty())
    }

    pub fn source_diagnostics(&self, document: &SourceDocument) -> Vec<SourceDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic
                    .source
                    .as_ref()
                    .is_none_or(|source| &source.source == document.identity())
            })
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
                && obligation.discharge.is_missing()
        })
    }

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
            .is_some_and(|item| item.discharge.is_missing())
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
