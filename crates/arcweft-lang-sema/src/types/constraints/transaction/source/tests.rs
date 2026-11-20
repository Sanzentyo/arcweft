use crate::types::constraints::test_support::ConstraintTestSetup;
use std::sync::atomic::AtomicBool;

use super::*;
use crate::types::constraints::transaction::materialization::MaterializationInput;
use crate::types::constraints::transaction::{ProbeSubmission, TypeConstraintTransaction};
use crate::{
    effect_row::EffectRow,
    effects::EffectSet,
    types::{
        ArrayLength, DetachedGenericOwnerId, GenericBinder, GenericParameterOwnerId, GenericScope,
        GenericTypeParameterId,
        constraints::{
            ConstraintSourceContainerPolicy, NoConstraintClient,
            TypeConstraintParameterEligibility, TypeConstraintParameterScope,
            TypeConstraintRejection,
            context::{LocalConstraintAccounting, TypeConstraintLimits},
        },
    },
};

type TestContext<'a> = ConstraintTestSetup<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

mod retained_preparation {
    use super::*;
    use crate::effect_row::{
        EffectPredicate,
        prepared::{
            PreparedEffectDelta, PreparedEffectPredicate, PreparedEffectReadScope,
            PreparedEffectRow,
        },
    };
    use crate::types::constraints::{SourceProbeResult, TypeConstraintProjectionClosure};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Facts {
        value: u32,
        drops: Arc<AtomicUsize>,
        relation: Option<PreparedEffectDelta>,
        reads: Option<PreparedEffectReadScope>,
        query: Option<TypeKind<PreparedEffects>>,
    }
    impl Drop for Facts {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }
    struct Domain;

    struct ObservedValue {
        original: OriginalProducerContext,
        receipt: crate::types::constraints::ConstraintSourceReceipt<Domain>,
        ty: TypeKind<PreparedEffects>,
    }

    impl ConstraintSourceValueOwner<Domain> for ObservedValue {
        fn source(&self) -> ConstraintSourceId<()> {
            self.receipt.source()
        }

        fn original_context(&self) -> &OriginalProducerContext {
            &self.original
        }

        fn probe_receipt(
            &self,
        ) -> Option<&crate::types::constraints::ConstraintSourceReceipt<Domain>> {
            Some(&self.receipt)
        }

        fn value_type(&self) -> &TypeKind<PreparedEffects> {
            &self.ty
        }
    }

    impl ConstraintDomain for Domain {
        type Application = ();
        type Source = ();
        type AlternativeIndex = ();
        type EvidenceRule = ();
        type ObservedEvidence = ();
        type CheckedEvidence = ();
        type ProbeSemanticBranch = ();
        type SourceValue = ObservedValue;
        type ProbePreparation = Facts;
        type SealedBranchValue = ();
        type Projection = ();
        type SourceErrorCause = ();
        type ClientInvariant = ();

        fn probe_effect_reads(preparation: &Facts) -> Option<&PreparedEffectReadScope> {
            preparation.reads.as_ref()
        }

        fn visit_probe_effect_types<'p, E>(
            preparation: &'p Facts,
            visit: &mut impl FnMut(&'p TypeKind<PreparedEffects>) -> Result<(), E>,
        ) -> Result<(), E> {
            preparation.query.as_ref().map_or(Ok(()), visit)
        }

        fn evidence_accepts((): &(), (): &()) -> bool {
            true
        }
        fn project_checked_evidence((): &(), _: &TypeKind) -> Option<()> {
            Some(())
        }
        fn alternative_ordinal((): &()) -> u32 {
            0
        }
        fn client_invariant_source((): &()) {}
        fn empty_sealed_branch() {}
    }

    #[test]
    fn an_original_value_uses_its_closed_effects_under_the_same_expectation() {
        let cancellation = AtomicBool::new(false);
        let mut graph = crate::effect_row::prepared::PreparedEffectGraph::default();
        let (mut context, parameters) = ConstraintTestSetup::<_, Domain>::new(
            TypeConstraintLimits::new(4096, 2048, 128, 64),
            &cancellation,
        )
        .into_parts();
        let mut transaction = TypeConstraintTransaction::initialize_fixture(
            &mut context,
            &mut graph,
            (),
            parameters,
            None,
        )
        .unwrap();
        let named = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut context)
            .unwrap();
        let body = EffectRow::closed(EffectSet::from_labels(["probe.once"]).unwrap());
        graph
            .prepare_body(
                &named,
                PreparedEffectRow::Canonical(body.clone()),
                vec![],
                &mut context,
            )
            .unwrap();
        let row = graph.application(&named, vec![], &mut context).unwrap();
        let arrow = |effects| {
            TypeKind::function_with_contract(
                GenericBinder::EMPTY,
                PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                [],
                TypeKind::I64,
                effects,
            )
        };

        transaction
            .begin_prepared_probe(
                &mut context,
                super::super::super::PreparedSourceConstraint::unchecked(
                    (),
                    PreparedConstraintSourceProjection::Scalar,
                ),
                super::super::super::ConstraintAcceptance::PatternAcceptsActual,
            )
            .unwrap();
        let mut ticket = transaction
            .next_probe(&mut context, &mut graph)
            .unwrap()
            .unwrap();
        let actual = Arc::new(ObservedValue {
            original: ticket.original_context().unwrap().clone(),
            receipt: ticket.receipt(),
            ty: arrow(row.clone()),
        });
        let observed = ticket
            .observe(SourceProbeResult::unchecked_value(Arc::clone(&actual), ()))
            .unwrap();
        let contribution = observed.close_preparation(Facts {
            value: 1,
            drops: Arc::new(AtomicUsize::new(0)),
            relation: None,
            reads: None,
            query: None,
        });
        transaction
            .submit_probe(
                &mut context,
                &mut graph,
                ticket.input(),
                ProbeSubmission::Accepted(contribution),
            )
            .unwrap();
        assert!(
            transaction
                .next_probe(&mut context, &mut graph)
                .unwrap()
                .is_none()
        );
        let ticket = transaction
            .next_materialization_ticket(&mut context, &mut graph)
            .unwrap()
            .unwrap();
        let request = ticket.requests().next().unwrap();
        let completed = request
            .complete_retained_type(None, &mut context)
            .unwrap()
            .unwrap();
        let canonical = arrow(PreparedEffectRow::Canonical(body));
        assert_eq!(
            completed
                .project_declaration_type(&actual.ty, &mut context)
                .unwrap()
                .unwrap()
                .into_replacement(),
            Some(canonical.clone()),
        );
        let foreign_scope = GenericScope::default().with_binder(GenericBinder::new(1, 0, 0));
        let foreign = TypeKind::GenericParam(foreign_scope.bound_type(0, 0).unwrap());
        assert!(
            completed
                .project_declaration_type(&foreign, &mut context)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            completed.into_value().into_replacement(),
            Some(canonical.clone())
        );
        assert_eq!(actual.ty, arrow(row));
        assert!(
            request
                .complete_retained_type(Some(&canonical), &mut context)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn forked_and_suspended_paths_share_the_original_non_clone_preparation() {
        let cancellation = AtomicBool::new(false);
        let mut graph = crate::effect_row::prepared::PreparedEffectGraph::default();
        let (mut context, parameters) = ConstraintTestSetup::<_, Domain>::new(
            TypeConstraintLimits::new(4096, 2048, 128, 64),
            &cancellation,
        )
        .into_parts();
        let mut transaction = TypeConstraintTransaction::initialize_fixture(
            &mut context,
            &mut graph,
            (),
            parameters,
            None,
        )
        .unwrap();
        transaction
            .begin_prepared_probe(
                &mut context,
                super::super::super::PreparedSourceConstraint::unchecked(
                    (),
                    PreparedConstraintSourceProjection::Scalar,
                ),
                super::super::super::ConstraintAcceptance::PatternAcceptsActual,
            )
            .unwrap();
        let mut ticket = transaction
            .next_probe(&mut context, &mut graph)
            .unwrap()
            .unwrap();
        let mut left = ticket
            .fork(&mut context)
            .unwrap()
            .observe(SourceProbeResult::unchecked(TypeKind::I32, ()))
            .unwrap();
        let right = ticket
            .observe(SourceProbeResult::unchecked(TypeKind::String, ()))
            .unwrap();
        left.append(right).unwrap();
        let mut source_base = graph.checkpoint(&mut context).unwrap();
        let mut callback = graph.checkpoint(&mut context).unwrap();
        let original_function = graph
            .declare(Vec::new(), EffectPredicate::unconstrained(), &mut context)
            .unwrap();
        let row = graph
            .application(&original_function, Vec::new(), &mut context)
            .unwrap();
        let body = EffectRow::closed(EffectSet::from_labels(["probe.once"]).unwrap());
        graph
            .prepare_body(
                &original_function,
                PreparedEffectRow::Canonical(body.clone()),
                Vec::new(),
                &mut context,
            )
            .unwrap();
        let relation = graph.extract_delta(&mut callback, &mut context).unwrap();
        let reads = graph.read_scope([&relation], &mut context).unwrap();
        assert!(graph.read().retain_row(&row, &mut context).is_err());
        // The actual argument and its expected type have no effect terms.
        // A local or nested expression in its preparation can still query C.
        let query = TypeKind::function_with_contract(
            GenericBinder::EMPTY,
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            [],
            TypeKind::I64,
            row.clone(),
        );
        let drops = Arc::new(AtomicUsize::new(0));
        let contribution = left.close_preparation(Facts {
            value: 913,
            drops: Arc::clone(&drops),
            relation: Some(relation),
            reads: Some(reads),
            query: Some(query),
        });
        transaction
            .submit_probe(
                &mut context,
                &mut graph,
                ticket.input(),
                ProbeSubmission::Accepted(contribution),
            )
            .unwrap();
        assert!(
            transaction
                .next_probe(&mut context, &mut graph)
                .unwrap()
                .is_none()
        );
        transaction.request_projection(
            &mut context,
            &mut graph,
            (),
            &TypeKind::Unit,
            TypeConstraintProjectionClosure::Closed,
        );
        let suspended = transaction.suspend_result(&mut context, ()).unwrap();
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        let resumed = suspended.resume();
        assert_eq!(resumed.frontier.len(), 2);
        let graph_owner = graph.owner(&mut context).unwrap();
        let mut materialization =
            crate::types::constraints::transaction::ConstraintMaterialization::new(
                graph_owner,
                &mut context,
            )
            .unwrap();
        for path in &resumed.frontier {
            let (input, _) = context
                .with_effect_reads(
                    &mut graph,
                    path.lineage.effect_read_scope(),
                    |context, graph| {
                        let original = path.prepare_original_path_effects(graph, context)?;
                        let effects = ConstraintClosedEffects::for_original_path(
                            path,
                            original,
                            &[],
                            graph,
                            context,
                        )?;
                        let query = path.lineage.probe_trace[0]
                            .branch
                            .preparation()
                            .unwrap()
                            .query
                            .as_ref()
                            .unwrap();
                        let projected =
                            effects.canonicalize(query, path.lineage.scope.shape(), context)?;
                        let TypeKind::Function { effects: row, .. } = projected else {
                            panic!("expected a canonical function type");
                        };
                        assert_eq!(row, body);
                        MaterializationInput::from_closed_original(
                            path,
                            path.root_id(),
                            path,
                            &mut effects.projection(),
                            graph.read(),
                            context,
                        )
                        .map_err(TypeConstraintError::from)
                    },
                )
                .unwrap();
            assert!(graph.read().retain_row(&row, &mut context).is_err());
            materialization.push_component(input, &mut context).unwrap();
        }
        let mut observations = Vec::new();
        while let Some(mut ticket) = materialization
            .next_materialization_ticket(&mut context)
            .unwrap()
        {
            let request = ticket.requests().next().unwrap();
            assert_eq!(request.preparation().value, 913);
            observations.push((
                std::ptr::from_ref(request.preparation()),
                request.actual().clone(),
            ));
            ticket.bind_callback().unwrap();
            let submission = ticket
                .bind_closed_submission(
                    crate::types::constraints::ClosedMaterializationSubmission::Sealed(()),
                )
                .unwrap();
            materialization
                .submit_closed_materialization(ticket, submission)
                .unwrap();
        }
        assert_eq!(observations.len(), 2);
        assert_eq!(observations[0].0, observations[1].0);
        assert_ne!(observations[0].1, observations[1].1);
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        let selected = materialization
            .finish(&mut context, |_, _, _| Ok(true))
            .unwrap()
            .into_index_with(0, |_| Ok::<_, std::convert::Infallible>(()))
            .unwrap()
            .unwrap();
        let crate::types::constraints::SolvedCandidate {
            component,
            source_preparations,
            ..
        } = selected;
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        let failure = source_preparations
            .into_owned(&mut context)
            .err()
            .expect("the original forest still owns the preparation");
        assert!(matches!(
            failure.error,
            crate::types::constraints::transaction::SourcePreparationTakeError::Shared
        ));
        assert_eq!(failure.source, component.sources().all()[0].source());
        assert!(std::ptr::eq(
            failure
                .retained
                .requests(&component)
                .next()
                .unwrap()
                .preparation(),
            observations[0].0,
        ));
        drop(resumed);
        cancellation.store(true, Ordering::Release);
        let failure = failure
            .retained
            .into_owned(&mut context)
            .err()
            .expect("the move is cancellable");
        assert!(matches!(
            failure.error,
            crate::types::constraints::transaction::SourcePreparationTakeError::Control(
                TypeConstraintError::Abort(
                    crate::types::constraints::TypeConstraintAbort::Cancelled
                )
            )
        ));
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        cancellation.store(false, Ordering::Release);
        let mut owned = failure
            .retained
            .into_owned(&mut context)
            .unwrap_or_else(|_| panic!("selected callback preparation"));
        assert_eq!(owned.len(), 1);
        assert_eq!(owned[0].source, component.sources().all()[0].source());
        assert_eq!(owned[0].preparation.value, 913);
        graph.rollback(&mut source_base).unwrap();
        let original_delta = owned[0].preparation.relation.take().unwrap();
        graph.restore_delta(original_delta, &mut context).unwrap();
        assert_eq!(graph.read().retain_row(&row, &mut context).unwrap(), row);
        drop(owned);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert_eq!(component.sources().all()[0].actual(), &observations[1].1);
    }

    #[test]
    fn cancelled_preparation_admission_keeps_the_affine_input_unpublished() {
        let cancellation = AtomicBool::new(false);
        let mut graph = crate::effect_row::prepared::PreparedEffectGraph::default();
        let (mut context, parameters) = ConstraintTestSetup::<_, Domain>::new(
            TypeConstraintLimits::new(4096, 2048, 128, 64),
            &cancellation,
        )
        .into_parts();
        let mut transaction = TypeConstraintTransaction::initialize_fixture(
            &mut context,
            &mut graph,
            (),
            parameters,
            None,
        )
        .unwrap();
        transaction
            .begin_prepared_probe(
                &mut context,
                super::super::super::PreparedSourceConstraint::unchecked(
                    (),
                    PreparedConstraintSourceProjection::Scalar,
                ),
                super::super::super::ConstraintAcceptance::PatternAcceptsActual,
            )
            .unwrap();
        let mut ticket = transaction
            .next_probe(&mut context, &mut graph)
            .unwrap()
            .unwrap();
        let drops = Arc::new(AtomicUsize::new(0));
        let contribution = ticket
            .observe(SourceProbeResult::unchecked(TypeKind::I32, ()))
            .unwrap()
            .close_preparation(Facts {
                value: 7,
                drops: Arc::clone(&drops),
                relation: None,
                reads: None,
                query: None,
            });
        cancellation.store(true, Ordering::Release);
        assert!(matches!(
            transaction.submit_probe(
                &mut context,
                &mut graph,
                ticket.input(),
                ProbeSubmission::Accepted(contribution)
            ),
            Err(TypeConstraintError::Abort(
                crate::types::constraints::TypeConstraintAbort::Cancelled
            ))
        ));
        assert!(transaction.probe.as_ref().unwrap().active_input.is_some());
        assert!(transaction.frontier.is_empty());
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn a_scalar_alternative_rejects_foreign_preparation_before_binding_observations() {
        use crate::effect_row::prepared::{PreparedEffectGraph, PreparedEffectInvariant};

        let cancellation = AtomicBool::new(false);
        let mut graph = PreparedEffectGraph::default();
        let (mut context, parameters) = ConstraintTestSetup::<_, Domain>::new(
            TypeConstraintLimits::new(4096, 2048, 128, 64),
            &cancellation,
        )
        .into_parts();
        let mut transaction = TypeConstraintTransaction::initialize_fixture(
            &mut context,
            &mut graph,
            (),
            parameters,
            None,
        )
        .unwrap();
        transaction
            .begin_prepared_probe(
                &mut context,
                super::super::super::PreparedSourceConstraint::unchecked(
                    (),
                    PreparedConstraintSourceProjection::Scalar,
                ),
                super::super::super::ConstraintAcceptance::PatternAcceptsActual,
            )
            .unwrap();
        let mut ticket = transaction
            .next_probe(&mut context, &mut graph)
            .unwrap()
            .unwrap();
        let result = SourceProbeResult::unchecked(TypeKind::I32, ());
        let observation = Arc::clone(result.observation());
        let foreign = PreparedEffectGraph::default()
            .read_scope([], &mut context)
            .unwrap();
        let drops = Arc::new(AtomicUsize::new(0));
        let contribution = ticket.observe(result).unwrap().close_preparation(Facts {
            value: 7,
            drops: Arc::clone(&drops),
            relation: None,
            reads: Some(foreign),
            query: None,
        });

        assert_eq!(
            transaction
                .submit_probe(
                    &mut context,
                    &mut graph,
                    ticket.input(),
                    ProbeSubmission::Accepted(contribution),
                )
                .err(),
            Some(TypeConstraintError::from(
                PreparedEffectInvariant::ForeignIssuer
            )),
        );
        assert!(observation.preparation().is_none());
        assert!(transaction.probe.as_ref().unwrap().active_input.is_some());
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }
}

fn future_parameter() -> GenericTypeParameterId {
    GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(719)),
        0,
    )
}

fn context(cancellation: &AtomicBool) -> TestContext<'_> {
    let scope = TypeConstraintParameterScope::new([(
        future_parameter(),
        TypeConstraintParameterEligibility::FutureEligible,
    )])
    .expect("one parameter for a later group");
    TestContext::with_scope(
        TypeConstraintLimits::new(4096, 2048, 128, 64),
        cancellation,
        scope,
    )
}

fn source(
    context: &TypeConstraintContext<'_, LocalConstraintAccounting<'_>, NoConstraintClient>,
    application: ConstraintApplicationId,
    actual: TypeKind<PreparedEffects>,
) -> ActiveConstraintProbe<NoConstraintClient> {
    ActiveConstraintProbe {
        source: ConstraintSourceId::new(application, ()),
        source_ordinal: PreparedSourceOrdinal {
            application,
            ordinal: 0,
        },
        branch: SourceProbeObservation::closed_for_test((), ()),
        selection: StoredSourceSelection::Unchecked,
        prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
        value_expected: None,
        actual: context.fixture_operand(actual),
    }
}

#[test]
fn an_unclosed_probe_preparation_cannot_issue_a_completed_source() {
    let cancellation = AtomicBool::new(false);
    let mut graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let (mut context, path) = context(&cancellation).into_path(&mut graph);
    let mut probe = source(&context, path.root_id(), TypeKind::I32);
    probe.branch = SourceProbeObservation::new(());
    let original = path
        .prepare_original_path_effects(&mut graph, &mut context)
        .unwrap();
    let effects =
        ConstraintClosedEffects::for_original_path(&path, original, &[], &mut graph, &mut context)
            .unwrap();
    assert!(matches!(
        probe.close(&path, &mut effects.projection(), graph.read(), &mut context),
        Err(TypeConstraintError::Invariant(
            super::super::super::TypeConstraintInvariant::SourceProtocol(
                TypeConstraintSourceProtocolInvariant::Outcome
            )
        ))
    ));
}

#[test]
fn source_operand_cannot_retain_an_unowned_future_type_reference() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let (mut context, path) = context(&cancellation).into_path(&mut effect_graph);
    let future = path
        .root_scope()
        .parameters()
        .type_reference(&(future_parameter()).clone().into())
        .expect("future slot");
    let probe = source(
        &context,
        path.root_id(),
        TypeKind::GenericParam(future.clone()),
    );
    let effects = path
        .close_original_path_effects(&mut effect_graph, &mut context)
        .unwrap();
    assert_eq!(
        probe
            .close(
                &path,
                &mut effects.projection(),
                effect_graph.read(),
                &mut context
            )
            .err()
            .expect("the already evaluated operand must close before materialization"),
        TypeConstraintError::Rejected(TypeConstraintRejection::IncompleteInstantiation {
            parameter: future.into(),
        }),
    );
}

#[test]
fn source_function_value_retains_its_own_type_and_constant_binder() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let binder = GenericBinder::new(1, 1, 0);
    let scope = GenericScope::default().with_binder(binder);
    let parameter = TypeKind::GenericParam(scope.bound_type(0, 0).expect("function type slot"));
    let function = TypeKind::function_with_binder(
        binder,
        [TypeKind::Array {
            item: Box::new(parameter.clone()),
            len: ArrayLength::Generic(scope.bound_const(0, 0).expect("function length slot")),
        }],
        parameter,
        EffectRow::closed(EffectSet::new()),
    );
    let cancellation = AtomicBool::new(false);
    let (mut context, path) = context(&cancellation).into_path(&mut effect_graph);
    let prepared = context.prepare_fixture_type(&function).unwrap();
    let effects = path
        .close_original_path_effects(&mut effect_graph, &mut context)
        .unwrap();
    let closed = source(&context, path.root_id(), prepared)
        .close(
            &path,
            &mut effects.projection(),
            effect_graph.read(),
            &mut context,
        )
        .unwrap_or_else(|error| panic!("an operand's own quantifiers must remain valid: {error}"));
    assert_eq!(closed.actual(), &function);
    assert!(closed.actual().semantic_identity_digest().is_ok());
    assert!(closed.final_expected().is_none());
    assert!(closed.selection().is_unchecked());
    assert_eq!(
        closed.source_projection(),
        &CheckedConstraintSourceProjection::Scalar
    );
    assert!(context.lexical_scope().binders().is_empty());
}

#[test]
fn closed_source_owns_the_normalized_actual_expectation_and_container() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let (mut context, mut path) = context(&cancellation).into_path(&mut effect_graph);
    let parameter = path
        .root_scope()
        .parameters()
        .type_reference(&(future_parameter()).clone().into())
        .expect("template slot");
    let item = TypeKind::GenericParam(parameter.clone());
    let probe = ActiveConstraintProbe {
        source: ConstraintSourceId::new(path.root_id(), ()),
        source_ordinal: PreparedSourceOrdinal {
            application: path.root_id(),
            ordinal: 3,
        },
        branch: SourceProbeObservation::closed_for_test((), ()),
        selection: StoredSourceSelection::Checked {
            alternative: (),
            evidence: Arc::new(()),
        },
        prepared_source_projection: PreparedConstraintSourceProjection::InferSpreadContainer {
            policy: ConstraintSourceContainerPolicy::Positional,
        },
        value_expected: Some(context.fixture_operand(item.clone())),
        actual: context.fixture_operand(TypeKind::Array {
            item: Box::new(item),
            len: ArrayLength::Const(4),
        }),
    };
    path.bindings
        .insert(parameter, context.fixture_operand(TypeKind::I64));
    let effects = path
        .close_original_path_effects(&mut effect_graph, &mut context)
        .unwrap();
    let closed = probe
        .close(
            &path,
            &mut effects.projection(),
            effect_graph.read(),
            &mut context,
        )
        .unwrap_or_else(|error| panic!("normalized source: {error}"));
    let expected = TypeKind::Array {
        item: Box::new(TypeKind::I64),
        len: ArrayLength::Const(4),
    };
    assert_eq!(closed.actual(), &expected);
    assert_eq!(closed.final_expected(), Some(&expected));
    assert_eq!(closed.ordinal().ordinal, 3);
    assert_eq!(closed.selection().alternative(), Some(()));
    assert_eq!(closed.selection().evidence(), Some(&()));
    assert!(closed.source_projection().matches_actual(closed.actual()));
}

#[test]
fn a_closed_source_keeps_its_exact_completion_owner() {
    let cancellation = AtomicBool::new(false);
    let mut graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let (mut context, path) = context(&cancellation).into_path(&mut graph);
    let receipt = path
        .prepare_effect_constraint(&mut graph, &mut context)
        .unwrap();
    let frozen = graph
        .read()
        .freeze_scoped(&[], &[], &[&receipt], &mut context)
        .unwrap();
    let original = graph
        .bind_original_scoped(&frozen, &mut context)
        .unwrap()
        .freeze(graph.read(), &mut context)
        .unwrap();
    let effects =
        ConstraintClosedEffects::for_path(&path, &original, &[], &mut graph, &mut context).unwrap();
    let actual = context.fixture_operand(TypeKind::<crate::types::PreparedEffects>::I64);
    let active = ActiveConstraintProbe {
        source: ConstraintSourceId::new(path.root_id(), ()),
        source_ordinal: PreparedSourceOrdinal {
            application: path.root_id(),
            ordinal: 0,
        },
        branch: SourceProbeObservation::closed_for_test((), ()),
        selection: StoredSourceSelection::Unchecked,
        prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
        value_expected: None,
        actual,
    };
    let closed = active
        .close(&path, &mut effects.projection(), graph.read(), &mut context)
        .unwrap();
    closed.require_completion(&effects).unwrap();
    let foreign =
        ConstraintClosedEffects::for_path(&path, &original, &[], &mut graph, &mut context).unwrap();
    assert_eq!(
        closed.require_completion(&foreign),
        Err(super::super::super::TypeConstraintInvariant::OriginalProducer.into())
    );
    let trace = ClosedConstraintSourceTrace::new(
        path.root_id(),
        Arc::clone(&path.applications),
        Box::new([closed]),
    );
    assert_eq!(trace.all()[0].actual(), &TypeKind::I64);
    trace.all()[0].require_completion(&effects).unwrap();
}

#[derive(Eq, PartialEq)]
struct ObservedCase {
    owner: TypeKind,
    tag: u8,
}

#[derive(Debug, Eq, PartialEq)]
struct CheckedCase {
    owner: crate::types::SemanticTypeDigest,
    tag: u8,
}

struct EvidenceDomain;

impl ConstraintDomain for EvidenceDomain {
    type Application = ();
    type Source = u8;
    type AlternativeIndex = u8;
    type EvidenceRule = ();
    type ObservedEvidence = ObservedCase;
    type CheckedEvidence = CheckedCase;
    type ProbeSemanticBranch = ();
    type SourceValue = std::convert::Infallible;
    type ProbePreparation = ();
    type SealedBranchValue = ();
    type Projection = u8;
    type SourceErrorCause = ();
    type ClientInvariant = u8;

    fn evidence_accepts(_: &(), _: &ObservedCase) -> bool {
        true
    }
    fn project_checked_evidence(observed: &ObservedCase, actual: &TypeKind) -> Option<CheckedCase> {
        Some(CheckedCase {
            owner: actual.semantic_identity_digest().ok()?,
            tag: observed.tag,
        })
    }
    fn alternative_ordinal(index: &u8) -> u32 {
        u32::from(*index)
    }
    fn client_invariant_source(source: &u8) -> u8 {
        *source
    }
    fn empty_sealed_branch() {}
}

#[test]
fn completion_transforms_observed_evidence_into_its_distinct_checked_type() {
    let mut effect_graph = crate::effect_row::prepared::PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let parameter = future_parameter();
    let scope = TypeConstraintParameterScope::new([(
        parameter.clone(),
        TypeConstraintParameterEligibility::Bindable,
    )])
    .expect("application scope");
    let reference = scope
        .type_reference(&(parameter).clone().into())
        .expect("opened parameter");
    let (mut context, mut path) =
        ConstraintTestSetup::<LocalConstraintAccounting<'_>, EvidenceDomain>::with_scope(
            TypeConstraintLimits::new(4096, 2048, 128, 64),
            &cancellation,
            scope,
        )
        .into_path(&mut effect_graph);
    let actual = TypeKind::Option(Box::new(TypeKind::GenericParam(reference.clone())));
    let observed = ObservedCase {
        owner: actual.clone(),
        tag: 7,
    };
    assert!(observed.owner.semantic_identity_digest().is_err());
    let actual = context.prepare_fixture_type(&actual).unwrap();
    let probe = ActiveConstraintProbe {
        source: ConstraintSourceId::new(path.root_id(), 2),
        source_ordinal: PreparedSourceOrdinal {
            application: path.root_id(),
            ordinal: 0,
        },
        branch: SourceProbeObservation::closed_for_test((), ()),
        selection: StoredSourceSelection::Checked {
            alternative: 0,
            evidence: Arc::new(observed),
        },
        prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
        value_expected: Some(context.fixture_operand(actual.clone())),
        actual: context.fixture_operand(actual),
    };
    path.bindings
        .insert(reference, context.fixture_operand(TypeKind::I64));
    let effects = path
        .close_original_path_effects(&mut effect_graph, &mut context)
        .unwrap();
    let closed = probe
        .close(
            &path,
            &mut effects.projection(),
            effect_graph.read(),
            &mut context,
        )
        .unwrap_or_else(|error| panic!("complete source evidence: {error}"));
    let expected = TypeKind::Option(Box::new(TypeKind::I64));
    assert_eq!(closed.actual(), &expected);
    assert_eq!(closed.final_expected(), Some(&expected));
    assert_eq!(
        closed.selection().evidence(),
        Some(&CheckedCase {
            owner: expected.semantic_identity_digest().expect("closed owner"),
            tag: 7,
        })
    );
}
