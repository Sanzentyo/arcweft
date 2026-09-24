use std::{
    convert::Infallible,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use super::*;
use crate::{
    callable::{CallableConstraintApplication, PRODUCTION_CALLABLE_LIMITS, limits::ResolverWork},
    effect_row::{DecisionControl, DecisionWork, EffectFormula, EffectPredicate, EffectRow},
    effects::EffectSet,
    types::constraints::{
        PreparedConstraintSourceProjection, PreparedSourceAlternative, SourceProbeResult,
        TypeConstraintParameterEligibility, TypeConstraintParameterScope,
    },
    types::{ArrayLength, GenericBinder, TypeKind},
};
use arcweft_lang_hir::identity::ExprId;

#[derive(Debug)]
struct Domain;

impl ConstraintDomain for Domain {
    type Application = CallableConstraintApplication;
    type Source = u8;
    type AlternativeIndex = u8;
    type EvidenceRule = ();
    type ObservedEvidence = ();
    type CheckedEvidence = ();
    type ProbeSemanticBranch = u8;
    type SealedBranchValue = ();
    type Projection = u8;
    type SourceErrorCause = ();
    type ClientInvariant = u8;

    fn evidence_accepts(_: &(), _: &()) -> bool {
        true
    }
    fn project_checked_evidence(_: &(), _: &TypeKind) -> Option<()> {
        Some(())
    }
    fn alternative_ordinal(index: &u8) -> u32 {
        u32::from(*index)
    }
    fn client_invariant_source(source: &u8) -> u8 {
        *source
    }
    fn empty_sealed_branch() {}
}

enum Source {
    Scheme(TypeKind),
    ReturnedScheme(TypeKind),
    Value(TypeKind),
}

struct Client<'a> {
    graph: &'a PreparedCallGraph<()>,
    application_owners: Vec<ExprId>,
    sources: &'a [Source],
    observations: Arc<Mutex<Vec<(u8, TypeKind, Option<CallableConstraintApplication>)>>>,
    cancellation: &'a AtomicBool,
    cancel_during_probe: bool,
    check_foreign_graph: bool,
}

fn enclosing() -> EnclosingGenericParameterScope {
    EnclosingGenericParameterScope::sealed(
        std::iter::empty::<crate::types::GenericTypeParameterId>(),
        std::iter::empty::<crate::types::GenericConstParameterId>(),
        std::iter::empty::<crate::types::GenericEffectParameterId>(),
    )
    .unwrap()
}

impl TypeConstraintClient<Domain> for Client<'_> {
    type ProbeCheckpoint = ();
    type MaterializationCheckpoint = ();
    type PreparedSealedBranchValue = ();

    fn probe_source(
        &mut self,
        _: &mut (),
        probe: &mut CandidateConstraintSourceContext<'_, '_, Domain>,
    ) -> Result<SourceProbeOutcome<Domain>, SourceCallbackFailure<Domain>> {
        let source = probe.source().local();
        match &self.sources[usize::from(source)] {
            Source::Value(value) => {
                probe.observe(SourceProbeResult::checked(value.clone(), 0, 0, ()))
            }
            Source::Scheme(scheme) | Source::ReturnedScheme(scheme) => {
                if self.check_foreign_graph {
                    let foreign = PreparedCallGraph::<()>::new();
                    assert!(matches!(
                        probe.specialize_function_value(
                            &foreign,
                            CallableConstraintApplication::Specialize(
                                self.application_owners[usize::from(source)]
                            ),
                            scheme,
                            &enclosing(),
                            &PRODUCTION_CALLABLE_LIMITS,
                            0,
                        ),
                        Err(FunctionSpecializationFailure::Prepared(
                            CallConstraintInvariant::ForeignPreparedIssuer
                        ))
                    ));
                }
                if self.cancel_during_probe {
                    self.cancellation.store(true, Ordering::Release);
                }
                let specialized =
                    if matches!(self.sources[usize::from(source)], Source::ReturnedScheme(_)) {
                        let owner = self.application_owners[1];
                        let parameter = super::super::tests::accepted_type(900, 0);
                        let template = TypeKind::generic_parameter(parameter.clone());
                        let parameters = TypeConstraintParameterScope::new([(
                            parameter,
                            TypeConstraintParameterEligibility::Bindable,
                        )])
                        .unwrap();
                        let nested = probe.ticket.fork_for_child(probe.context).unwrap();
                        let mut child = TypeConstraintTransaction::initialize_from_nested_path(
                            probe.context,
                            CallableConstraintApplication::Call(owner),
                            parameters,
                            None,
                            nested,
                        )
                        .unwrap();
                        child.constrain(
                            probe.context,
                            &template,
                            scheme,
                            ConstraintAcceptance::PatternAcceptsActual,
                        );
                        child.request_projection(
                            probe.context,
                            0,
                            &template,
                            crate::types::constraints::TypeConstraintProjectionClosure::Closed,
                        );
                        let pending = child.defer_child_result(0).unwrap().with_probe_branch(9);
                        probe.specialize_pending_function_value(
                            self.graph,
                            CallableConstraintApplication::Specialize(owner),
                            pending,
                            &enclosing(),
                            &PRODUCTION_CALLABLE_LIMITS,
                            0,
                        )
                    } else {
                        probe.specialize_function_value(
                            self.graph,
                            CallableConstraintApplication::Specialize(
                                self.application_owners[usize::from(source)],
                            ),
                            scheme,
                            &enclosing(),
                            &PRODUCTION_CALLABLE_LIMITS,
                            0,
                        )
                    };
                let pending = specialized.map_err(|failure| match failure {
                    FunctionSpecializationFailure::Prepared(error) => {
                        panic!("valid scheme preparation: {error:?}")
                    }
                    FunctionSpecializationFailure::Constraint(TypeConstraintFailure::Abort(
                        error,
                    )) => SourceCallbackFailure::Abort(error),
                    FunctionSpecializationFailure::Constraint(error) => {
                        panic!("valid deferred scheme: {error:?}")
                    }
                })?;
                probe.observe_child(
                    pending,
                    0,
                    SourceProbeSelection::Checked {
                        alternative: 0,
                        evidence: Arc::new(()),
                    },
                )
            }
        }
    }

    fn open_probe_checkpoint(
        &mut self,
        _: ConstraintSourceId<u8>,
    ) -> Result<(), SourceCheckpointFailure<Domain>> {
        Ok(())
    }
    fn close_probe_checkpoint(&mut self, _: ()) -> Result<(), SourceCheckpointFailure<Domain>> {
        Ok(())
    }
    fn open_materialization_checkpoint(
        &mut self,
        _: &[ConstraintSourceId<u8>],
    ) -> Result<(), SourceCheckpointFailure<Domain>> {
        Ok(())
    }

    fn materialize_sources<'h, I>(
        &mut self,
        sources: I,
        _: &mut (),
        _: &mut CandidateConstraintWorkSession<'_>,
    ) -> Result<MaterializationOutcome<ConstraintSourceId<u8>, (), ()>, SourceCallbackFailure<Domain>>
    where
        I: IntoIterator<Item = MaterializedSourceRequest<'h, Domain>>,
    {
        for request in sources {
            let result = request.result_projection();
            if let Some(result) = &result {
                assert_eq!(result.projection().value().value(), request.actual());
                assert!(result.projection().value().scope().binders().is_empty());
                let (Source::Scheme(source) | Source::ReturnedScheme(source)) =
                    &self.sources[usize::from(request.source().local())]
                else {
                    panic!("only function scheme uses have specialization results");
                };
                let witness = crate::callable::CheckedFunctionSpecialization::seal(
                    result,
                    &mut crate::types::UnmeteredTypeProjection,
                )
                .unwrap();
                let source_fact = crate::final_analysis::CheckedExpression::value(
                    source.clone(),
                    crate::final_analysis::CheckedTypeSelection::Inferred,
                    EffectSet::from_labels(["fs.read"]).unwrap(),
                    crate::final_analysis::CheckedExpressionResolution::Call,
                );
                let checked = source_fact
                    .clone()
                    .with_function_specialization(witness.owner(), witness)
                    .unwrap();
                assert_eq!(checked.value_type(), Some(request.actual()));
                assert_eq!(checked.source_value_type(), Some(source));
                assert_eq!(checked.effects(), source_fact.effects());
                assert_eq!(checked.resolution(), source_fact.resolution());
                assert_eq!(checked.execution_plan(), source_fact.execution_plan());
                if let Source::ReturnedScheme(source) =
                    &self.sources[usize::from(request.source().local())]
                {
                    assert_eq!(
                        *request.canonical_branch(),
                        9,
                        "the child choice survives specialization"
                    );
                    let origin = result
                        .source()
                        .expect("specialization retains the producing call port");
                    assert_eq!(
                        origin.application_id(),
                        CallableConstraintApplication::Call(result.application_id().expression())
                    );
                    assert_eq!(origin.projection().value().value(), source);
                    assert!(origin.source().is_none());
                }
            }
            self.observations.lock().unwrap().push((
                request.source().local(),
                request.actual().clone(),
                result.map(|result| result.application_id()),
            ));
        }
        Ok(MaterializationOutcome::Sealed(()))
    }

    fn close_materialization_checkpoint(
        &mut self,
        _: (),
        sealed: Option<()>,
    ) -> Result<Option<()>, SourceCheckpointFailure<Domain>> {
        Ok(sealed)
    }
    fn finish(self) -> Result<(), SourceCheckpointFailure<Domain>> {
        Ok(())
    }
}

struct Decisions;
impl DecisionControl for Decisions {
    type Error = Infallible;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Infallible> {
        Ok(())
    }
}

fn scheme() -> TypeKind {
    let binder = GenericBinder::new(1, 1, 1);
    let scope = GenericScope::default().with_binder(binder);
    let array = TypeKind::Array {
        item: Box::new(TypeKind::GenericParam(scope.bound_type(0, 0).unwrap())),
        len: ArrayLength::Generic(scope.bound_const(0, 0).unwrap()),
    };
    let effect = scope.bound_effect(0, 0).unwrap();
    let row = EffectRow::open(EffectSet::new(), effect.clone());
    let predicate = EffectFormula::literal(EffectSet::new(), Some(effect))
        .subset(
            &EffectFormula::literal(EffectSet::from_labels(["fs.read"]).unwrap(), None),
            &mut Decisions,
        )
        .unwrap();
    TypeKind::function_with_contract(
        binder,
        predicate,
        [
            array.clone(),
            TypeKind::function_with_effects([], TypeKind::Unit, row.clone()),
        ],
        array,
        row,
    )
}

fn concrete(item: TypeKind, length: usize, effects: &[&str]) -> TypeKind {
    let array = TypeKind::Array {
        item: Box::new(item),
        len: ArrayLength::Const(length),
    };
    let row = EffectRow::closed(EffectSet::from_labels(effects.iter().copied()).unwrap());
    TypeKind::function_with_effects(
        [
            array.clone(),
            TypeKind::function_with_effects([], TypeKind::Unit, row.clone()),
        ],
        array,
        row,
    )
}

fn drive(
    graph: &PreparedCallGraph<()>,
    mut client: Client<'_>,
    scope: TypeConstraintParameterScope,
    expected: &[TypeKind],
    limits: CallableLimits,
) -> Result<SolvedCandidate<Domain>, TypeConstraintFailure<Domain>> {
    let fixture = crate::final_analysis::tests::fixture(
        "fn identity<T>(value: T) -> T { value }\nfn caller() { identity(1i64); identity(2i64); }\n",
        None,
    );
    client.application_owners = fixture
        .project
        .analysis_view()
        .unwrap()
        .modules()
        .flat_map(|(_, module)| module.expressions())
        .filter_map(|(owner, expression)| {
            matches!(
                expression.kind(),
                arcweft_lang_hir::expr::HirExprKind::Call(_)
            )
            .then_some(owner)
        })
        .collect();
    assert!(client.application_owners.len() >= client.sources.len());
    let application = CallableConstraintApplication::Call(client.application_owners[0]);
    let mut work = ResolverWork::new(1_048_576);
    let session = work
        .begin_candidate_constraint_session(limits, client.cancellation)
        .unwrap();
    let initialization = super::super::tests::initialization_from_graph(graph, scope);
    session
        .with_driver::<Domain, _, _>(application, initialization, client, |mut driver| {
            for (index, expected) in expected.iter().enumerate() {
                driver.probe_prepared_source(
                    PreparedSourceConstraint::checked(
                        u8::try_from(index).unwrap(),
                        PreparedConstraintSourceProjection::Scalar,
                        [],
                        PreparedSourceAlternative::new(0, (), expected.clone()),
                    )
                    .unwrap(),
                    ConstraintAcceptance::PatternAcceptsActual,
                )?;
            }
            driver.finish()
        })
        .unwrap()
}

#[test]
fn source_scheme_uses_open_types_lengths_and_effects_independently_in_one_component() {
    let graph = PreparedCallGraph::<()>::new();
    let source = scheme();
    let original = source.semantic_identity_digest().unwrap();
    let sources = [
        Source::Scheme(source.clone()),
        Source::Scheme(source.clone()),
    ];
    let expected = [
        concrete(TypeKind::I64, 2, &[]),
        concrete(TypeKind::String, 3, &["fs.read"]),
    ];
    let observations = Arc::new(Mutex::new(Vec::new()));
    let cancellation = AtomicBool::new(false);
    let solved = drive(
        &graph,
        Client {
            graph: &graph,
            application_owners: Vec::new(),
            sources: &sources,
            observations: Arc::clone(&observations),
            cancellation: &cancellation,
            cancel_during_probe: false,
            check_foreign_graph: true,
        },
        TypeConstraintParameterScope::empty(),
        &expected,
        PRODUCTION_CALLABLE_LIMITS,
    )
    .unwrap();
    assert_eq!(solved.component.applications().len(), 3);
    let call_owner = solved
        .component
        .applications()
        .find_map(|(application, _)| application.require_call().ok())
        .unwrap();
    let observations = observations.lock().unwrap();
    assert_eq!(
        observations[0],
        (
            0,
            expected[0].clone(),
            Some(CallableConstraintApplication::Specialize(call_owner))
        )
    );
    assert_eq!(observations[1].0, 1);
    assert_eq!(observations[1].1, expected[1]);
    assert_ne!(observations[1].2.unwrap().expression(), call_owner);
    assert!(
        observations
            .iter()
            .all(|(_, _, application)| application.unwrap().require_call().is_err())
    );
    assert_eq!(source.semantic_identity_digest().unwrap(), original);
    assert!(
        !expected[0].accepts(&source),
        "ordinary compatibility remains rigid"
    );
}

mod nested;
mod witness;

#[test]
fn pending_call_result_specializes_on_its_own_path_and_retains_same_expression_origin() {
    let graph = PreparedCallGraph::<()>::new();
    let source = scheme();
    let sources = [Source::ReturnedScheme(source.clone())];
    let expected = concrete(TypeKind::I64, 4, &["fs.read"]);
    let observations = Arc::new(Mutex::new(Vec::new()));
    let cancellation = AtomicBool::new(false);
    let solved = drive(
        &graph,
        Client {
            graph: &graph,
            application_owners: Vec::new(),
            sources: &sources,
            observations: Arc::clone(&observations),
            cancellation: &cancellation,
            cancel_during_probe: false,
            check_foreign_graph: false,
        },
        TypeConstraintParameterScope::empty(),
        std::slice::from_ref(&expected),
        PRODUCTION_CALLABLE_LIMITS,
    )
    .unwrap();
    assert_eq!(solved.component.applications().len(), 3);
    assert_eq!(observations.lock().unwrap()[0].1, expected);
    assert_eq!(source, scheme());
}

#[test]
fn source_predicate_rejects_forbidden_specialization_before_materialization() {
    let graph = PreparedCallGraph::<()>::new();
    let source = scheme();
    let sources = [Source::Scheme(source.clone())];
    let observations = Arc::new(Mutex::new(Vec::new()));
    let cancellation = AtomicBool::new(false);
    let result = drive(
        &graph,
        Client {
            graph: &graph,
            application_owners: Vec::new(),
            sources: &sources,
            observations: Arc::clone(&observations),
            cancellation: &cancellation,
            cancel_during_probe: false,
            check_foreign_graph: false,
        },
        TypeConstraintParameterScope::empty(),
        &[concrete(TypeKind::I64, 2, &["fs.write"])],
        PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(
        matches!(result, Err(TypeConstraintFailure::Rejected(_))),
        "{result:?}"
    );
    assert!(observations.lock().unwrap().is_empty());
    assert!(source.accepts(&source.clone()));
}

#[test]
fn specialization_waits_for_later_parent_operands_to_close_its_result() {
    let graph = PreparedCallGraph::<()>::new();
    let binder = GenericBinder::new(1, 0, 0);
    let scope = GenericScope::default().with_binder(binder);
    let value = TypeKind::GenericParam(scope.bound_type(0, 0).unwrap());
    let source = TypeKind::function_with_contract(
        binder,
        EffectPredicate::unconstrained(),
        [value.clone()],
        value,
        EffectRow::closed(EffectSet::new()),
    );
    let parameter = super::super::tests::accepted_type(512, 0);
    let parent = TypeKind::generic_parameter(parameter.clone());
    let parameters = TypeConstraintParameterScope::new([(
        parameter,
        TypeConstraintParameterEligibility::Bindable,
    )])
    .unwrap();
    let expected = [
        TypeKind::function_with_effects(
            [parent.clone()],
            parent.clone(),
            EffectRow::closed(EffectSet::new()),
        ),
        parent,
    ];
    let sources = [Source::Scheme(source), Source::Value(TypeKind::I64)];
    let observations = Arc::new(Mutex::new(Vec::new()));
    let cancellation = AtomicBool::new(false);
    let solved = drive(
        &graph,
        Client {
            graph: &graph,
            application_owners: Vec::new(),
            sources: &sources,
            observations: Arc::clone(&observations),
            cancellation: &cancellation,
            cancel_during_probe: false,
            check_foreign_graph: false,
        },
        parameters,
        &expected,
        PRODUCTION_CALLABLE_LIMITS,
    )
    .unwrap();
    assert_eq!(solved.component.applications().len(), 2);
    assert_eq!(
        observations.lock().unwrap()[0].1,
        TypeKind::function_with_effects(
            [TypeKind::I64],
            TypeKind::I64,
            EffectRow::closed(EffectSet::new())
        )
    );
}

#[test]
fn specialization_cannot_open_a_known_expected_scheme() {
    let graph = PreparedCallGraph::<()>::new();
    let source = scheme();
    let sources = [Source::Scheme(source.clone())];
    let observations = Arc::new(Mutex::new(Vec::new()));
    let cancellation = AtomicBool::new(false);
    let result = drive(
        &graph,
        Client {
            graph: &graph,
            application_owners: Vec::new(),
            sources: &sources,
            observations: Arc::clone(&observations),
            cancellation: &cancellation,
            cancel_during_probe: false,
            check_foreign_graph: false,
        },
        TypeConstraintParameterScope::empty(),
        &[source.clone()],
        PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(
        matches!(result, Err(TypeConstraintFailure::Rejected(_))),
        "{result:?}"
    );
    assert!(observations.lock().unwrap().is_empty());
    assert!(
        source.accepts(&source),
        "known schemes use the existing rigid relation"
    );
}

#[test]
fn specialization_uses_parent_cancellation_and_structural_budget() {
    let graph = PreparedCallGraph::<()>::new();
    let sources = [Source::Scheme(scheme())];
    for cancelled in [false, true] {
        let observations = Arc::new(Mutex::new(Vec::new()));
        let cancellation = AtomicBool::new(false);
        let limits = if cancelled {
            PRODUCTION_CALLABLE_LIMITS
        } else {
            PRODUCTION_CALLABLE_LIMITS.with_type_constraint_limits(1024, 20, 1024)
        };
        let result = drive(
            &graph,
            Client {
                graph: &graph,
                application_owners: Vec::new(),
                sources: &sources,
                observations: Arc::clone(&observations),
                cancellation: &cancellation,
                cancel_during_probe: cancelled,
                check_foreign_graph: false,
            },
            TypeConstraintParameterScope::empty(),
            &[concrete(TypeKind::I64, 2, &[])],
            limits,
        );
        assert!(
            matches!(result, Err(TypeConstraintFailure::Abort(_))),
            "{result:?}"
        );
        assert!(observations.lock().unwrap().is_empty());
    }
}
