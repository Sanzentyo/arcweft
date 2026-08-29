//! Exhaustive late construction of the 31 final-HIR statement families.
//!
//! Specialized early checks retain only private preparation. This module is
//! the one producer that consumes those rows, the statement ingress proofs,
//! and affine coordinate evidence after every checked child is complete.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    identity::{ExprId, LocalId, StmtId},
    module::HirModule,
    project::{HirControlTransferKind, HirExecutableProjectView},
    stmt::{
        HirSelectBranchHead, HirSelectStmt, HirSourceLocaleValue, HirStmtKind, HirTrigger,
        HirUnsafeAuditIdentity, HirUnsafeLifetimeBody,
    },
};

use crate::{
    callable::{CheckedCallableCatalog, CheckedCallableDeclaration},
    semantic_coordinate::SemanticCoordinateIndex,
    types::TypeKind,
};

use super::{
    CheckedAssignment, CheckedAssignmentPlace, CheckedBinding, CheckedExpression,
    CheckedExpressionResolution, CheckedIncludeFlowTarget, CheckedScopeIdentity,
    CheckedSelectBranchHead, CheckedSelectResolution, CheckedSelectStatement, CheckedStatement,
    CheckedStatementPayload, CheckedTrigger, CheckedUnsafeAudit, FinalSemanticAnalysisError,
    PreparedSelectBranchHeadProof, PreparedSelectScrutineeProof, PreparedStatementIngressSeal,
    PreparedStatementPayload, PreparedStatementScrutineeProof, PreparedTriggerScrutineeProof,
    statement_effects::CheckedStatementPayloadSealer,
};

/// Move-only all-statement producer used by the final effect transaction.
pub(crate) struct CheckedStatementSeal<'a, 'project, 'coordinate> {
    prepared: BTreeMap<StmtId, PreparedStatementPayload>,
    includes: BTreeMap<StmtId, super::PreparedIncludeFlowProof>,
    scrutinees: BTreeMap<StmtId, PreparedStatementScrutineeProof>,
    locals: &'a BTreeMap<LocalId, CheckedBinding>,
    callables: &'a CheckedCallableCatalog,
    coordinates: &'coordinate SemanticCoordinateIndex<'coordinate, 'coordinate>,
    project: HirExecutableProjectView<'project>,
}

impl<'a, 'project, 'coordinate> CheckedStatementSeal<'a, 'project, 'coordinate> {
    pub(crate) fn new(
        prepared: BTreeMap<StmtId, PreparedStatementPayload>,
        ingress: PreparedStatementIngressSeal,
        locals: &'a BTreeMap<LocalId, CheckedBinding>,
        callables: &'a CheckedCallableCatalog,
        coordinates: &'coordinate SemanticCoordinateIndex<'coordinate, 'coordinate>,
        project: HirExecutableProjectView<'project>,
    ) -> Self {
        let (includes, scrutinees) = ingress.into_parts();
        Self {
            prepared,
            includes,
            scrutinees,
            locals,
            callables,
            coordinates,
            project,
        }
    }

    fn take_prepared(
        &mut self,
        owner: StmtId,
    ) -> Result<PreparedStatementPayload, FinalSemanticAnalysisError> {
        self.prepared
            .remove(&owner)
            .ok_or(FinalSemanticAnalysisError::MissingFact {
                family: super::SemanticFactFamily::Statement,
            })
    }

    fn structural(
        &mut self,
        owner: StmtId,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError> {
        match self.take_prepared(owner)? {
            PreparedStatementPayload::HirOwned => Ok(CheckedStatementPayload::Structural),
            PreparedStatementPayload::Assignment(_)
            | PreparedStatementPayload::Assertion(_)
            | PreparedStatementPayload::Iteration(_)
            | PreparedStatementPayload::Suspension(_)
            | PreparedStatementPayload::Yield
            | PreparedStatementPayload::EvaluatedEffect(_)
            | PreparedStatementPayload::SealedEvaluatedEffect(_) => {
                Err(FinalSemanticAnalysisError::WrongPayloadFamily)
            }
        }
    }

    fn assertion(
        &mut self,
        owner: StmtId,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError> {
        match self.take_prepared(owner)? {
            PreparedStatementPayload::Assertion(assertion) => {
                Ok(CheckedStatementPayload::Assertion(assertion))
            }
            PreparedStatementPayload::HirOwned
            | PreparedStatementPayload::Assignment(_)
            | PreparedStatementPayload::Iteration(_)
            | PreparedStatementPayload::Suspension(_)
            | PreparedStatementPayload::Yield
            | PreparedStatementPayload::EvaluatedEffect(_)
            | PreparedStatementPayload::SealedEvaluatedEffect(_) => {
                Err(FinalSemanticAnalysisError::WrongPayloadFamily)
            }
        }
    }

    fn iteration(
        &mut self,
        owner: StmtId,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError> {
        match self.take_prepared(owner)? {
            PreparedStatementPayload::Iteration(iteration) => {
                Ok(CheckedStatementPayload::Iteration(iteration))
            }
            PreparedStatementPayload::HirOwned
            | PreparedStatementPayload::Assignment(_)
            | PreparedStatementPayload::Assertion(_)
            | PreparedStatementPayload::Suspension(_)
            | PreparedStatementPayload::Yield
            | PreparedStatementPayload::EvaluatedEffect(_)
            | PreparedStatementPayload::SealedEvaluatedEffect(_) => {
                Err(FinalSemanticAnalysisError::WrongPayloadFamily)
            }
        }
    }

    fn suspension(
        &mut self,
        owner: StmtId,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError> {
        match self.take_prepared(owner)? {
            PreparedStatementPayload::Suspension(suspension) => {
                Ok(CheckedStatementPayload::Suspension(suspension))
            }
            PreparedStatementPayload::HirOwned
            | PreparedStatementPayload::Assignment(_)
            | PreparedStatementPayload::Assertion(_)
            | PreparedStatementPayload::Iteration(_)
            | PreparedStatementPayload::Yield
            | PreparedStatementPayload::EvaluatedEffect(_)
            | PreparedStatementPayload::SealedEvaluatedEffect(_) => {
                Err(FinalSemanticAnalysisError::WrongPayloadFamily)
            }
        }
    }

    fn yield_statement(
        &mut self,
        owner: StmtId,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError> {
        match self.take_prepared(owner)? {
            PreparedStatementPayload::Yield => Ok(CheckedStatementPayload::Yield),
            PreparedStatementPayload::HirOwned
            | PreparedStatementPayload::Assignment(_)
            | PreparedStatementPayload::Assertion(_)
            | PreparedStatementPayload::Iteration(_)
            | PreparedStatementPayload::Suspension(_)
            | PreparedStatementPayload::EvaluatedEffect(_)
            | PreparedStatementPayload::SealedEvaluatedEffect(_) => {
                Err(FinalSemanticAnalysisError::WrongPayloadFamily)
            }
        }
    }

    fn assignment(
        &mut self,
        owner: StmtId,
        expressions: &BTreeMap<ExprId, CheckedExpression>,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError> {
        let PreparedStatementPayload::Assignment(prepared) = self.take_prepared(owner)? else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let (local, nominal, target, value, field_type) = prepared.into_parts();
        if self.locals.get(&local).map(CheckedBinding::ty) != Some(&nominal.ty()) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let target = expressions
            .get(&target)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: target })?;
        let CheckedExpressionResolution::Select(CheckedSelectResolution::Field(selection)) =
            target.resolution()
        else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let value = expressions
            .get(&value)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: value })?;
        if target.ty() != &field_type || value.ty() != &field_type {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let place = CheckedAssignmentPlace::try_new(local, nominal, selection.clone(), field_type)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        Ok(CheckedStatementPayload::Assignment(Box::new(
            CheckedAssignment::new(place, value.ty().clone()),
        )))
    }

    fn control_transfer(
        &mut self,
        owner: StmtId,
        expected: HirControlTransferKind,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError> {
        let prepared = self.take_prepared(owner)?;
        if !matches!(prepared, PreparedStatementPayload::HirOwned) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let evidence = self
            .coordinates
            .control_transfer_evidence(owner)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if evidence.owner() != owner || evidence.kind() != expected {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(CheckedStatementPayload::ControlTransfer(
            evidence.into_target(),
        ))
    }

    fn trigger(
        &mut self,
        owner: StmtId,
        hir: &HirTrigger,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError> {
        if !matches!(
            self.take_prepared(owner)?,
            PreparedStatementPayload::HirOwned
        ) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let proof = self
            .scrutinees
            .remove(&owner)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let PreparedStatementScrutineeProof::Trigger(proof) = proof else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let checked = match (hir, proof) {
            (HirTrigger::Input(_), PreparedTriggerScrutineeProof::Input) => CheckedTrigger::input(),
            (HirTrigger::Event(_), PreparedTriggerScrutineeProof::Event) => CheckedTrigger::event(),
            (HirTrigger::Signal { .. }, PreparedTriggerScrutineeProof::Signal) => {
                CheckedTrigger::signal()
            }
            (HirTrigger::Timeout(_), PreparedTriggerScrutineeProof::Timeout) => {
                CheckedTrigger::timeout()
            }
            (HirTrigger::Mark(hir), PreparedTriggerScrutineeProof::Mark(proof))
                if *hir == proof =>
            {
                CheckedTrigger::mark(
                    self.coordinates
                        .dialogue_mark(self.project, proof)
                        .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?,
                )
            }
            (HirTrigger::Select(_), PreparedTriggerScrutineeProof::Select) => {
                CheckedTrigger::select()
            }
            (HirTrigger::Task(_), PreparedTriggerScrutineeProof::Task) => CheckedTrigger::task(),
            (HirTrigger::Scope(_), PreparedTriggerScrutineeProof::Scope) => CheckedTrigger::scope(),
            (HirTrigger::Expression(_), PreparedTriggerScrutineeProof::Expression) => {
                CheckedTrigger::expression()
            }
            (HirTrigger::Recovered(_), _)
            | (HirTrigger::Input(_), _)
            | (HirTrigger::Event(_), _)
            | (HirTrigger::Signal { .. }, _)
            | (HirTrigger::Timeout(_), _)
            | (HirTrigger::Mark(_), _)
            | (HirTrigger::Select(_), _)
            | (HirTrigger::Task(_), _)
            | (HirTrigger::Scope(_), _)
            | (HirTrigger::Expression(_), _) => {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        };
        Ok(CheckedStatementPayload::Trigger(checked))
    }

    fn select(
        &mut self,
        owner: StmtId,
        hir: &HirSelectStmt,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError> {
        if !matches!(
            self.take_prepared(owner)?,
            PreparedStatementPayload::HirOwned
        ) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let proof = self
            .scrutinees
            .remove(&owner)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let PreparedStatementScrutineeProof::Select(proof) = proof else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let checked = match (hir, proof) {
            (HirSelectStmt::Operand(_), PreparedSelectScrutineeProof::Operand) => {
                CheckedSelectStatement::operand()
            }
            (
                HirSelectStmt::Branches { branches, .. },
                PreparedSelectScrutineeProof::Branches(proofs),
            ) if branches.len() == proofs.len() => {
                let heads = branches
                    .iter()
                    .zip(proofs.into_vec())
                    .map(|(branch, proof)| match (branch.head(), proof) {
                        (HirSelectBranchHead::Bind { .. }, PreparedSelectBranchHeadProof::Bind) => {
                            Ok(CheckedSelectBranchHead::Bind)
                        }
                        (
                            HirSelectBranchHead::Frame { .. },
                            PreparedSelectBranchHeadProof::Frame,
                        ) => Ok(CheckedSelectBranchHead::Frame),
                        (
                            HirSelectBranchHead::Event { .. },
                            PreparedSelectBranchHeadProof::Event,
                        ) => Ok(CheckedSelectBranchHead::Event),
                        (HirSelectBranchHead::Recovered, _)
                        | (HirSelectBranchHead::Bind { .. }, _)
                        | (HirSelectBranchHead::Frame { .. }, _)
                        | (HirSelectBranchHead::Event { .. }, _) => {
                            Err(FinalSemanticAnalysisError::WrongPayloadFamily)
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice();
                CheckedSelectStatement::branches(heads)
            }
            (HirSelectStmt::Operand(_), PreparedSelectScrutineeProof::Branches(_))
            | (HirSelectStmt::Branches { .. }, PreparedSelectScrutineeProof::Operand)
            | (HirSelectStmt::Branches { .. }, PreparedSelectScrutineeProof::Branches(_)) => {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        };
        Ok(CheckedStatementPayload::Select(checked))
    }

    fn unsafe_audit(
        &mut self,
        owner: StmtId,
        audit: &arcweft_lang_hir::stmt::HirUnsafeAudit,
        body: &HirUnsafeLifetimeBody,
        expressions: &BTreeMap<ExprId, CheckedExpression>,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError> {
        if !matches!(
            self.take_prepared(owner)?,
            PreparedStatementPayload::HirOwned
        ) || !matches!(body, HirUnsafeLifetimeBody::Block { .. })
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let HirUnsafeAuditIdentity::Accepted(id) = audit.identity() else {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        };
        if let Some(reason) = audit.reason() {
            let reason = expressions
                .get(&reason)
                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: reason })?;
            if reason.ty() != &TypeKind::String || !reason.effects().is_empty() {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
        Ok(CheckedStatementPayload::UnsafeAudit(
            CheckedUnsafeAudit::new(id.clone(), audit.has_safety_doc()),
        ))
    }

    fn include(
        &mut self,
        owner: StmtId,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError> {
        if !matches!(
            self.take_prepared(owner)?,
            PreparedStatementPayload::HirOwned
        ) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let proof = self
            .includes
            .remove(&owner)
            .filter(|proof| proof.statement() == owner)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        self.callables
            .project_callable(proof.source())
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let target = proof.into_target();
        let declaration = arcweft_lang_hir::symbol::CallableDeclarationKey::Flow(target.clone());
        let checked = self
            .callables
            .project_callable(&declaration)
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        if !matches!(
            checked.id().declaration(),
            CheckedCallableDeclaration::Project(retained) if retained == &declaration
        ) {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
        Ok(CheckedStatementPayload::Include(
            CheckedIncludeFlowTarget::new(target.semantic_digest()),
        ))
    }

    fn expression_statement(
        &mut self,
        owner: StmtId,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError> {
        match self.take_prepared(owner)? {
            PreparedStatementPayload::HirOwned => Ok(CheckedStatementPayload::Structural),
            PreparedStatementPayload::SealedEvaluatedEffect(effect) => {
                Ok(CheckedStatementPayload::EvaluatedEffect(effect))
            }
            PreparedStatementPayload::Assignment(_)
            | PreparedStatementPayload::Assertion(_)
            | PreparedStatementPayload::Iteration(_)
            | PreparedStatementPayload::Suspension(_)
            | PreparedStatementPayload::Yield
            | PreparedStatementPayload::EvaluatedEffect(_) => {
                Err(FinalSemanticAnalysisError::WrongPayloadFamily)
            }
        }
    }
}

impl CheckedStatementPayloadSealer for CheckedStatementSeal<'_, '_, '_> {
    #[allow(
        clippy::too_many_lines,
        reason = "the accepted contract requires one explicit producer arm for all 31 HIR families"
    )]
    fn seal_payload(
        &mut self,
        _module: &HirModule,
        owner: StmtId,
        statement: &HirStmtKind,
        expressions: &BTreeMap<ExprId, CheckedExpression>,
        _statements: &BTreeMap<StmtId, CheckedStatement>,
    ) -> Result<CheckedStatementPayload, FinalSemanticAnalysisError> {
        match statement {
            HirStmtKind::Assertion { .. } => self.assertion(owner),
            HirStmtKind::Let { .. } => self.structural(owner),
            HirStmtKind::Assign { .. } => self.assignment(owner, expressions),
            HirStmtKind::LetElse { .. } => self.structural(owner),
            HirStmtKind::Return { .. } => self.structural(owner),
            HirStmtKind::Out { .. } => self.control_transfer(owner, HirControlTransferKind::Out),
            HirStmtKind::Goto { .. } => self.structural(owner),
            HirStmtKind::Defer { outcome, .. } => {
                if !matches!(
                    self.take_prepared(owner)?,
                    PreparedStatementPayload::HirOwned
                ) {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                Ok(CheckedStatementPayload::Defer(*outcome))
            }
            HirStmtKind::Yield { .. } => self.yield_statement(owner),
            HirStmtKind::Signal { .. } => self.structural(owner),
            HirStmtKind::LifetimeSet { .. } => self.structural(owner),
            HirStmtKind::Wait { .. } => self.suspension(owner),
            HirStmtKind::On { trigger, .. } => self.trigger(owner, trigger),
            HirStmtKind::UnsafeLifetime { audit, body } => {
                self.unsafe_audit(owner, audit, body, expressions)
            }
            HirStmtKind::Choice { .. } => self.structural(owner),
            HirStmtKind::If(_) => self.structural(owner),
            HirStmtKind::IfLet(_) => self.structural(owner),
            HirStmtKind::Match(_) => self.structural(owner),
            HirStmtKind::While(_) => self.structural(owner),
            HirStmtKind::WhileLet(_) => self.structural(owner),
            HirStmtKind::For(_) => self.iteration(owner),
            HirStmtKind::Close { .. } => self.structural(owner),
            HirStmtKind::Select(select) => self.select(owner, select),
            HirStmtKind::SourceLocale(locale) => {
                if !matches!(
                    self.take_prepared(owner)?,
                    PreparedStatementPayload::HirOwned
                ) {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                let HirSourceLocaleValue::Resolved(locale) = locale.locale() else {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                };
                Ok(CheckedStatementPayload::SourceLocale(locale.clone()))
            }
            HirStmtKind::Scope(scope) => {
                if !matches!(
                    self.take_prepared(owner)?,
                    PreparedStatementPayload::HirOwned
                ) {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                Ok(CheckedStatementPayload::Scope(if scope.name().is_some() {
                    CheckedScopeIdentity::Named
                } else {
                    CheckedScopeIdentity::Anonymous
                }))
            }
            HirStmtKind::Include(_) => self.include(owner),
            HirStmtKind::Break { .. } => {
                self.control_transfer(owner, HirControlTransferKind::Break)
            }
            HirStmtKind::Continue { .. } => {
                self.control_transfer(owner, HirControlTransferKind::Continue)
            }
            HirStmtKind::Expression { .. } => self.expression_statement(owner),
            HirStmtKind::ProofCall { .. } => self.structural(owner),
            HirStmtKind::Error => Err(FinalSemanticAnalysisError::RecoveredOwner),
        }
    }

    fn finish(self) -> Result<(), FinalSemanticAnalysisError> {
        if self.prepared.is_empty() && self.includes.is_empty() && self.scrutinees.is_empty() {
            Ok(())
        } else {
            Err(FinalSemanticAnalysisError::WrongPayloadFamily)
        }
    }
}
