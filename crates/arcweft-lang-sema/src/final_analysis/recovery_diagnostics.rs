//! Semantic projection of terminal final-HIR recovery evidence.
//!
//! Recovered HIR never enters executable final analysis.  A small subset of
//! terminal recovery owners nevertheless has a language-level semantic
//! diagnostic whose exact source identity is already frozen in HIR.  This
//! module projects only those owners before executable admission; it does not
//! weaken readiness, rerun semantic analysis over poisoned records, or copy a
//! second recovery vocabulary.

use std::collections::BTreeSet;

use arcweft_lang_hir::{
    diagnostic::{HirDiagnostic, HirRecoveryPrimary},
    expr::{HirExprKind, HirGenericExprIssue, HirPoisonState, HirRecoveryIssue},
    identity::{ExprId, IdResolveError, ItemId, SyntheticOwner},
    item::{HirItemKind, HirPredicateBody, HirProofBody},
    module::HirModule,
    proof_return::HirProofReturnSemanticClass,
    scope::{HirScopeKind, HirScopeOwner},
    source_index::{
        HirCallableSourceRole, HirItemSourceRole, HirSourcePresence, HirSourceQuery,
        HirSourceQueryError, HirSourceSite,
    },
    symbol::{CallableDeclarationOwner, ProjectSymbolTable},
};
use arcweft_source::{
    Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceDocument, SourceRange, SourceSpan,
    SourceSpanError,
};
use thiserror::Error;

const PREDICATE_MISSING_TAIL: &str = "sema.predicate.missing_boolean_tail";
const PROOF_MISSING_TAIL: &str = "sema.proof.missing_value_tail";

/// One semantic diagnostic projected from an exact terminal HIR recovery owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableTailRecoveryDiagnostic {
    item: ItemId,
    tail: ExprId,
    diagnostic: Diagnostic,
}

impl CallableTailRecoveryDiagnostic {
    pub const fn item(&self) -> ItemId {
        self.item
    }

    pub const fn tail(&self) -> ExprId {
        self.tail
    }

    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_diagnostic(self) -> Diagnostic {
        self.diagnostic
    }
}

/// Failure to project typed terminal recovery evidence without guessing.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CallableTailRecoveryProjectionError {
    #[error("source document does not match the recovered HIR module")]
    WrongSourceDocument,
    #[error("failed to resolve recovered HIR owner {owner:?}")]
    OwnerResolution {
        owner: SyntheticOwner,
        #[source]
        error: IdResolveError,
    },
    #[error("callable recovery owner {item:?} has no exact project symbol")]
    MissingCallableSymbol { item: ItemId },
    #[error("callable recovery owner {item:?} has more than one project symbol")]
    AmbiguousCallableSymbol { item: ItemId },
    #[error("callable recovery owner {item:?} has the wrong symbol family or snapshot")]
    WrongCallableSymbol { item: ItemId },
    #[error("callable recovery source query failed")]
    SourceQuery(#[from] HirSourceQueryError),
    #[error("callable recovery source role is unexpectedly absent")]
    MissingSourceRole,
    #[error("callable recovery source site belongs to another source document")]
    WrongSourceIdentity,
    #[error("callable recovery insertion cannot be represented as a source span")]
    SourceSpan(#[from] SourceSpanError),
    #[error("callable tail recovery was projected more than once for {item:?} / {tail:?}")]
    DuplicateDiagnostic { item: ItemId, tail: ExprId },
}

/// Projects missing Predicate/Proof block-tail diagnostics from final HIR.
///
/// Only the terminal synthetic expression diagnostic is admitted.  The
/// item-level poison propagated from that child is deliberately ignored, so
/// one authored omission yields exactly one semantic diagnostic.
pub fn project_callable_tail_recovery_diagnostics(
    module: &HirModule,
    symbols: &ProjectSymbolTable,
    document: &SourceDocument,
) -> Result<Vec<CallableTailRecoveryDiagnostic>, CallableTailRecoveryProjectionError> {
    if module.provenance().source_identity() != document.identity() {
        return Err(CallableTailRecoveryProjectionError::WrongSourceDocument);
    }

    let mut projected = Vec::new();
    let mut seen = BTreeSet::new();
    for diagnostic in module.diagnostics() {
        let HirDiagnostic::Recovery(recovery) = diagnostic else {
            continue;
        };
        let SyntheticOwner::Expr(tail) = recovery.owner() else {
            continue;
        };
        let expression = module.resolve_expr(tail).map_err(|error| {
            CallableTailRecoveryProjectionError::OwnerResolution {
                owner: SyntheticOwner::Expr(tail),
                error,
            }
        })?;
        if !matches!(
            (expression.kind(), expression.state()),
            (
                HirExprKind::Error(error),
                HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail)
            ) if error.issue() == HirGenericExprIssue::TransactionalChildFailure
        ) {
            continue;
        }
        if recovery.primary_role() != HirRecoveryPrimary::owner_whole(SyntheticOwner::Expr(tail)) {
            continue;
        }

        let scope = module.resolve_scope(expression.scope()).map_err(|error| {
            CallableTailRecoveryProjectionError::OwnerResolution {
                owner: SyntheticOwner::Scope(expression.scope()),
                error,
            }
        })?;
        let HirScopeOwner::Item(item) = *scope.owner() else {
            continue;
        };
        let item_payload = module.resolve_item(item).map_err(|error| {
            CallableTailRecoveryProjectionError::OwnerResolution {
                owner: SyntheticOwner::Item(item),
                error,
            }
        })?;

        let Some((code, expected_owner, secondary_role, message, primary_label, secondary_label)) =
            callable_tail_diagnostic_shape(
                item_payload.kind(),
                scope.kind(),
                expression.scope(),
                tail,
            )
        else {
            continue;
        };

        let mut callable_symbols = symbols
            .callable_symbols()
            .filter(|symbol| symbol.source_item() == item);
        let symbol = callable_symbols
            .next()
            .ok_or(CallableTailRecoveryProjectionError::MissingCallableSymbol { item })?;
        if callable_symbols.next().is_some() {
            return Err(CallableTailRecoveryProjectionError::AmbiguousCallableSymbol { item });
        }
        if symbol.owner() != expected_owner || symbol.source_snapshot() != module.snapshot_id() {
            return Err(CallableTailRecoveryProjectionError::WrongCallableSymbol { item });
        }

        let primary = source_span(document, recovery.primary())?;
        let secondary = module.source_site(
            document.identity(),
            HirSourceQuery::Item {
                owner: item,
                role: HirItemSourceRole::Callable(secondary_role(symbol.source_owner())),
            },
        )?;
        let HirSourcePresence::Present(secondary) = secondary.presence() else {
            return Err(CallableTailRecoveryProjectionError::MissingSourceRole);
        };
        let secondary = source_span(document, secondary)?;

        if !seen.insert((item, tail, code)) {
            return Err(CallableTailRecoveryProjectionError::DuplicateDiagnostic { item, tail });
        }
        projected.push(CallableTailRecoveryDiagnostic {
            item,
            tail,
            diagnostic: Diagnostic::new(DiagnosticSeverity::Error, message)
                .with_code(code)
                .with_label(DiagnosticLabel::primary(
                    primary,
                    Some(primary_label.to_owned()),
                ))
                .with_label(DiagnosticLabel::secondary(
                    secondary,
                    Some(secondary_label.to_owned()),
                )),
        });
    }
    Ok(projected)
}

type CallableTailDiagnosticShape = (
    &'static str,
    CallableDeclarationOwner,
    fn(arcweft_lang_hir::source_index::HirCallableSourceOwner) -> HirCallableSourceRole,
    &'static str,
    &'static str,
    &'static str,
);

fn callable_tail_diagnostic_shape(
    item: &HirItemKind,
    scope_kind: HirScopeKind,
    scope: arcweft_lang_hir::identity::ScopeId,
    tail: ExprId,
) -> Option<CallableTailDiagnosticShape> {
    match item {
        HirItemKind::Predicate(predicate)
            if scope_kind == HirScopeKind::Predicate
                && matches!(
                    predicate.body(),
                    HirPredicateBody::Block {
                        scope: body_scope,
                        tail: body_tail,
                        ..
                    } if *body_scope == scope && *body_tail == tail
                ) =>
        {
            Some((
                PREDICATE_MISSING_TAIL,
                CallableDeclarationOwner::Predicate,
                |owner| HirCallableSourceRole::Signature { owner },
                "predicate block must end with a boolean value",
                "boolean tail is required here",
                "predicate return contract is declared here",
            ))
        }
        HirItemKind::Proof(proof)
            if scope_kind == HirScopeKind::Proof
                && proof.return_semantic_class() == HirProofReturnSemanticClass::NonUnit
                && matches!(
                    proof.body(),
                    HirProofBody::Block {
                        scope: body_scope,
                        tail: body_tail,
                        ..
                    } if *body_scope == scope && *body_tail == tail
                ) =>
        {
            Some((
                PROOF_MISSING_TAIL,
                CallableDeclarationOwner::Proof,
                |owner| HirCallableSourceRole::Result { owner },
                "proof block must end with a value matching its declared return type",
                "proof value tail is required here",
                "proof return type is declared here",
            ))
        }
        _ => None,
    }
}

fn source_span(
    document: &SourceDocument,
    site: &HirSourceSite,
) -> Result<SourceSpan, CallableTailRecoveryProjectionError> {
    if site.source_identity() != document.identity() {
        return Err(CallableTailRecoveryProjectionError::WrongSourceIdentity);
    }
    match site {
        HirSourceSite::Span(span) => Ok(span.clone()),
        HirSourceSite::Insertion(insertion) => {
            Ok(document.span(SourceRange::new(insertion.offset(), insertion.offset()))?)
        }
    }
}
