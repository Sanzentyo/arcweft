//! Candidate completion must stop before issuing source materialization work.

use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use crate::effect_row::{
    EffectConstraintEligibility, EffectConstraintVariable, EffectRow, EffectVar, EffectVarIssuer,
};
use crate::effects::EffectSet;
use crate::types::constraints::{
    NoConstraintClient, TypeConstraintParameterEligibility, TypeConstraintParameterScope,
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
        .finish(context)
        .complete()
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
            transaction.finish(context).complete(),
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
        transaction.finish(context).complete(),
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
