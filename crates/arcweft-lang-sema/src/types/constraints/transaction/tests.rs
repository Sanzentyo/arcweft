//! Candidate completion must stop before issuing source materialization work.

use std::{
    cell::Cell,
    sync::atomic::{AtomicBool, Ordering},
};

use super::*;
use crate::effect_row::{
    EffectConstraintEligibility, EffectConstraintVariable, EffectRow, EffectVar, EffectVarIssuer,
};
use crate::effects::EffectSet;
use crate::types::constraints::{
    NoConstraintClient, TypeConstraintFailureInvariant, TypeConstraintParameterEligibility,
    TypeConstraintParameterScope,
    context::{
        LocalConstraintAccounting, TypeConstraintContextIssuer, TypeConstraintEffectScope,
        TypeConstraintLimits, TypeConstraintWorkReport,
    },
};
use crate::types::{DetachedGenericOwnerId, GenericParameterOwnerId, GenericTypeParameterId};

fn scope() -> TypeConstraintParameterScope {
    TypeConstraintParameterScope::new([(parameter(), TypeConstraintParameterEligibility::Bindable)])
        .expect("one candidate parameter")
}

fn parameter() -> GenericTypeParameterId {
    GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(721)),
        0,
    )
}

fn effect_scope(variable: EffectVar) -> TypeConstraintEffectScope {
    TypeConstraintEffectScope::seal_call_scope(
        [EffectConstraintVariable::new(
            variable,
            EffectConstraintEligibility::Bindable,
        )],
        [],
    )
    .expect("one scoped effect variable")
}

#[test]
fn fixed_effect_evidence_survives_candidate_sealing() {
    let variable = EffectVar::issued(EffectVarIssuer::fresh_prepared().unwrap(), 0);
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scopes(
            TypeConstraintLimits::new(128, 128, 16, 16),
            &cancellation,
            TypeConstraintParameterScope::empty(),
            effect_scope(variable),
        );
    let mut transaction = TypeConstraintTransaction::new();
    transaction.initialize(&mut context, None).unwrap();
    let known = EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap());
    transaction.constrain_effect_equality(
        &mut context,
        &EffectRow::open(EffectSet::new(), variable),
        &known,
    );
    let solved = transaction
        .finish(&mut context)
        .expect("fixed effect solution");
    assert_eq!(
        solved.solution.effect_bindings().collect::<Vec<_>>(),
        vec![(&variable, &known)],
    );
}

#[test]
fn fixed_effect_evidence_rejects_shrinking_and_expanding_function_relations() {
    for shrink in [true, false] {
        let variable = EffectVar::issued(EffectVarIssuer::fresh_prepared().unwrap(), 0);
        let cancellation = AtomicBool::new(false);
        let mut context =
            TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scopes(
                TypeConstraintLimits::new(128, 128, 16, 16),
                &cancellation,
                TypeConstraintParameterScope::empty(),
                effect_scope(variable),
            );
        let mut transaction = TypeConstraintTransaction::new();
        transaction.initialize(&mut context, None).unwrap();
        let projected = EffectRow::open(EffectSet::new(), variable);
        transaction.constrain_effect_equality(
            &mut context,
            &projected,
            &EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap()),
        );
        let projected = TypeKind::function_with_effects([], TypeKind::I64, projected);
        let incompatible = TypeKind::function_with_effects(
            [],
            TypeKind::I64,
            EffectRow::closed(if shrink {
                EffectSet::new()
            } else {
                EffectSet::from_labels(["fs.read", "fs.write"]).unwrap()
            }),
        );
        let (pattern, actual) = if shrink {
            (&incompatible, &projected)
        } else {
            (&projected, &incompatible)
        };
        transaction.constrain(
            &mut context,
            pattern,
            actual,
            ConstraintAcceptance::PatternAcceptsActual,
        );
        assert!(matches!(
            transaction.finish(&mut context),
            Err(TypeConstraintFailure::Rejected(
                TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::Mismatch)
            ))
        ));
    }
}

#[test]
fn fixed_effect_evidence_abort_cannot_publish_a_partial_equality() {
    let variable = EffectVar::issued(EffectVarIssuer::fresh_prepared().unwrap(), 0);
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scopes(
            TypeConstraintLimits::new(128, 1, 16, 16),
            &cancellation,
            TypeConstraintParameterScope::empty(),
            effect_scope(variable),
        );
    let mut transaction = TypeConstraintTransaction::new();
    transaction.initialize(&mut context, None).unwrap();
    transaction.constrain_effect_equality(
        &mut context,
        &EffectRow::open(EffectSet::new(), variable),
        &EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap()),
    );
    assert!(transaction.frontier.is_empty());
    assert!(transaction.materialization.is_empty());
    assert!(transaction.materialized.is_empty());
    assert!(matches!(
        transaction.finish(&mut context),
        Err(TypeConstraintFailure::Abort(
            TypeConstraintAbort::NodeLimit {
                actual: 2,
                limit: 1,
            }
        ))
    ));
}

fn source_frontier<A: TypeConstraintAccounting>(
    context: &mut TypeConstraintContext<'_, A, NoConstraintClient>,
) -> TypeConstraintTransaction<NoConstraintClient> {
    let parameter = context
        .parameter_scope
        .type_reference(&parameter().into())
        .expect("opened candidate parameter");
    let mut transaction = TypeConstraintTransaction::new();
    for _ in 0..2 {
        let mut path = context.start_path().expect("admitted frontier row");
        path.bindings.insert(parameter.clone(), TypeKind::I64);
        path.probe_trace.push(source::ConstraintProbe::Active(
            source::ActiveConstraintProbe {
                source: (),
                source_ordinal: 0,
                branch: Arc::new(()),
                selection: StoredSourceSelection::Unchecked,
                prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
                value_expected: None,
                actual: TypeKind::I64,
            },
        ));
        transaction.frontier.push(path);
    }
    transaction
}

#[test]
fn advancing_materialization_preserves_the_queue_until_the_current_ticket_is_submitted() {
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(4096, 2048, 128, 128),
            &cancellation,
            scope(),
        );
    let mut transaction = source_frontier(&mut context);
    for remaining in [1, 0] {
        let mut ticket = transaction
            .next_materialization_ticket(&mut context)
            .unwrap()
            .expect("every retained path is materialized");
        assert!(matches!(
            transaction.next_materialization_ticket(&mut context),
            Err(MaterializationImmediateFailure::Invariant(
                TypeConstraintFailureInvariant::Constraint(
                    TypeConstraintInvariant::SourceProtocol(
                        TypeConstraintSourceProtocolInvariant::Ticket
                    )
                )
            ))
        ));
        assert_eq!(transaction.materialization.len(), remaining);
        transaction
            .validate_materialization_callback_begin(&ticket)
            .unwrap();
        let binding = ticket.bind_callback().unwrap();
        ticket.validate_callback_binding(&binding).unwrap();
        let closed = ticket
            .bind_closed_submission(ClosedMaterializationSubmission::Sealed(()))
            .unwrap();
        transaction
            .submit_closed_materialization(ticket, closed)
            .unwrap();
    }
    assert!(
        transaction
            .next_materialization_ticket(&mut context)
            .unwrap()
            .is_none()
    );
    transaction
        .finish(&mut context)
        .expect("both equivalent paths completed");
}

#[test]
fn a_completed_alternative_cannot_hide_an_outstanding_materialization() {
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(4096, 2048, 128, 128),
            &cancellation,
            scope(),
        );
    let mut transaction = source_frontier(&mut context);
    let mut first = transaction
        .next_materialization_ticket(&mut context)
        .unwrap()
        .unwrap();
    first.bind_callback().unwrap();
    let closed = first
        .bind_closed_submission(ClosedMaterializationSubmission::Sealed(()))
        .unwrap();
    transaction
        .submit_closed_materialization(first, closed)
        .unwrap();
    let _outstanding = transaction
        .next_materialization_ticket(&mut context)
        .unwrap()
        .unwrap();
    assert!(matches!(
        transaction.finish(&mut context),
        Err(TypeConstraintFailure::Invariant(
            TypeConstraintFailureInvariant::Constraint(TypeConstraintInvariant::SourceProtocol(
                TypeConstraintSourceProtocolInvariant::Outcome
            ))
        ))
    ));
}

#[test]
fn comparison_limit_prevents_materialization_ticket_issuance() {
    let cancellation = AtomicBool::new(false);
    for limit in [4, 5] {
        let mut context =
            TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
                TypeConstraintLimits::new(128, limit, 16, 16),
                &cancellation,
                scope(),
            );
        let mut transaction = source_frontier(&mut context);
        let result = transaction.close(&mut context);
        if limit == 4 {
            assert_eq!(
                result,
                Err(TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit {
                    actual: 5,
                    limit: 4,
                }))
            );
            assert!(transaction.materialization.is_empty());
            assert!(transaction.materialized.is_empty());
        } else {
            assert_eq!(result, Ok(()));
            assert_eq!(transaction.materialization.len(), 2);
            assert!(transaction.materialized.is_empty());
        }
    }
}

struct CancelBeforeComparison<'a> {
    cancellation: &'a AtomicBool,
    nodes: u64,
}

#[derive(Default)]
struct AccountingReceipt {
    commits: Cell<u32>,
    branches: Cell<u64>,
}

struct ContextLifetimeAccounting<'a> {
    cancellation: &'a AtomicBool,
    receipt: &'a AccountingReceipt,
    proposed: TypeConstraintWorkReport,
    committed: bool,
}

impl TypeConstraintAccounting for ContextLifetimeAccounting<'_> {
    fn charge_constraint(
        &mut self,
        delta: &TypeConstraintWorkReport,
        _limits: TypeConstraintLimits,
    ) -> Result<(), TypeConstraintError> {
        self.proposed = self.proposed.checked_add(delta)?;
        Ok(())
    }

    fn commit(&mut self) {
        if !self.committed {
            self.receipt.branches.set(self.proposed.branches);
            self.receipt.commits.set(self.receipt.commits.get() + 1);
            self.committed = true;
        }
    }
}

impl<'a> TypeConstraintContextIssuer<'a> for ContextLifetimeAccounting<'a> {
    fn context_limits(&self) -> TypeConstraintLimits {
        TypeConstraintLimits::new(4096, 2048, 1, 128)
    }

    fn context_cancellation(&self) -> &'a AtomicBool {
        self.cancellation
    }
}

#[test]
fn transaction_completion_cannot_release_or_reset_the_surrounding_context() {
    let cancellation = AtomicBool::new(false);
    for complete in [false, true] {
        let receipt = AccountingReceipt::default();
        let mut context = TypeConstraintContext::with_accounting(
            ContextLifetimeAccounting {
                cancellation: &cancellation,
                receipt: &receipt,
                proposed: TypeConstraintWorkReport::default(),
                committed: false,
            },
            TypeConstraintParameterScope::empty(),
            TypeConstraintEffectScope::seal_call_scope([], []).unwrap(),
        );
        let mut first = TypeConstraintTransaction::<NoConstraintClient>::new();
        first.initialize(&mut context, None).unwrap();
        if complete {
            first.finish(&mut context).unwrap();
        } else {
            drop(first);
        }
        assert_eq!(receipt.commits.get(), 0);

        let mut second = TypeConstraintTransaction::<NoConstraintClient>::new();
        assert!(matches!(
            second.initialize(&mut context, None),
            Err(super::super::TypeConstraintInitializationFailure::Abort(
                TypeConstraintAbort::BranchLimit {
                    actual: 2,
                    limit: 1
                }
            ))
        ));
        assert_eq!(receipt.commits.get(), 0);
        drop(context);
        assert_eq!(receipt.commits.get(), 1);
        assert_eq!(receipt.branches.get(), 1);
    }
}

impl TypeConstraintAccounting for CancelBeforeComparison<'_> {
    fn charge_constraint(
        &mut self,
        delta: &TypeConstraintWorkReport,
        _limits: TypeConstraintLimits,
    ) -> Result<(), TypeConstraintError> {
        self.nodes += delta.nodes();
        if self.nodes == 4 {
            self.cancellation.store(true, Ordering::Release);
        }
        Ok(())
    }

    fn commit(&mut self) {}
}

impl<'a> TypeConstraintContextIssuer<'a> for CancelBeforeComparison<'a> {
    fn context_limits(&self) -> TypeConstraintLimits {
        TypeConstraintLimits::new(128, 128, 16, 16)
    }

    fn context_cancellation(&self) -> &'a AtomicBool {
        self.cancellation
    }
}

#[test]
fn comparison_cancellation_prevents_materialization_ticket_issuance() {
    let cancellation = AtomicBool::new(false);
    let mut context = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
        CancelBeforeComparison {
            cancellation: &cancellation,
            nodes: 0,
        },
        scope(),
        TypeConstraintEffectScope::seal_call_scope([], []).expect("empty effect scope"),
    );
    let mut transaction = source_frontier(&mut context);
    assert_eq!(
        transaction.close(&mut context),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
    );
    assert!(transaction.materialization.is_empty());
    assert!(transaction.materialized.is_empty());
}
