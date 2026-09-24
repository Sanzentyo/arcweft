use std::{
    collections::BTreeMap,
    convert::Infallible,
    sync::{Arc, atomic::AtomicBool},
};

use crate::{
    callable::{CallableGroupIndex, CallableSignatureSchema, PRODUCTION_CALLABLE_LIMITS},
    effect_row::{
        DecisionControl, DecisionWork, EffectConstraintEligibility, EffectConstraintVariable,
        EffectFormula, EffectPredicate, EffectRow,
    },
    effects::EffectSet,
    types::{
        DetachedGenericOwnerId, GenericBinder, GenericEffectParameterId, GenericEffectReference,
        GenericParameterOwnerId, GenericScope, GenericTypeParameterId, TypeKind,
        TypeProjectionControl, TypeProjectionNodeKind,
    },
};

use super::super::{
    ConstraintClosurePolicy, NoConstraintClient, TypeConstraintEffectScope,
    TypeConstraintParameterScope,
    application::ConstraintApplicationScope,
    context::{LocalConstraintAccounting, TypeConstraintContext, TypeConstraintLimits},
};
use super::TypeConstraintSolution;

struct Decisions;

impl DecisionControl for Decisions {
    type Error = Infallible;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn subset(reference: GenericEffectReference, labels: &[&str]) -> EffectPredicate {
    EffectFormula::literal(EffectSet::new(), Some(reference))
        .subset(
            &EffectFormula::literal(
                EffectSet::from_labels(labels.iter().copied()).unwrap(),
                None,
            ),
            &mut Decisions,
        )
        .unwrap()
}

fn scheme() -> TypeKind {
    let binder = GenericBinder::new(0, 0, 1);
    let scope = GenericScope::default().with_binder(binder);
    TypeKind::function_with_contract(
        binder,
        subset(scope.bound_effect(0, 0).unwrap(), &["fs.read"]),
        [],
        TypeKind::Unit,
        EffectRow::closed(EffectSet::new()),
    )
}

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

fn context(cancellation: &AtomicBool) -> Context<'_> {
    Context::with_accounting(LocalConstraintAccounting::new(
        TypeConstraintLimits::new(65_536, 32_768, 1024, 1024),
        cancellation,
    ))
}

fn parameters(
    binder: GenericBinder,
    reference: GenericEffectReference,
    role: EffectConstraintEligibility,
    predicate: EffectPredicate,
) -> TypeConstraintParameterScope {
    TypeConstraintParameterScope::seal_call_scope(
        binder,
        [],
        [],
        TypeConstraintEffectScope::seal_call_scope_with_predicate(
            [EffectConstraintVariable::new(reference, role)],
            [],
            predicate,
        )
        .unwrap(),
        [],
        [],
    )
    .unwrap()
}

#[test]
fn function_predicate_is_part_of_type_identity_rigid_compatibility_and_schema_inventory() {
    let restricted = scheme();
    let mut unrestricted = restricted.clone();
    let TypeKind::Function { predicate, .. } = &mut unrestricted else {
        unreachable!()
    };
    *predicate = EffectPredicate::unconstrained();
    assert!(restricted.accepts(&restricted.clone()));
    assert!(!restricted.accepts(&unrestricted));
    assert!(!unrestricted.accepts(&restricted));
    assert_ne!(
        restricted.stable_ordering(&unrestricted),
        std::cmp::Ordering::Equal
    );
    assert_ne!(
        restricted.semantic_identity_digest().unwrap(),
        unrestricted.semantic_identity_digest().unwrap()
    );

    let signature =
        CallableSignatureSchema::for_function_value(&restricted, &PRODUCTION_CALLABLE_LIMITS)
            .unwrap();
    assert_eq!(
        signature.generic_inventory().effects().len(),
        1,
        "predicate-only slots remain owned"
    );
    let TypeKind::Function { predicate, .. } = &restricted else {
        unreachable!()
    };
    assert_eq!(signature.effect_predicate(), predicate);
    let projected = signature
        .declared_function_type_from_group(
            CallableGroupIndex::ZERO,
            &EffectRow::closed(EffectSet::new()),
        )
        .unwrap();
    let TypeKind::Function {
        predicate: projected,
        ..
    } = projected
    else {
        unreachable!()
    };
    assert_eq!(&projected, predicate);

    let wider = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let mut malformed = restricted;
    let TypeKind::Function { predicate, .. } = &mut malformed else {
        unreachable!()
    };
    *predicate = subset(wider.bound_effect(0, 1).unwrap(), &["fs.read"]);
    assert!(malformed.semantic_identity_digest().is_err());
    assert!(
        CallableSignatureSchema::for_function_value(&malformed, &PRODUCTION_CALLABLE_LIMITS)
            .is_err()
    );
}

#[test]
fn freshly_opened_scheme_predicates_reject_forbidden_effects_without_mutating_the_source() {
    let source = scheme();
    let source_digest = source.semantic_identity_digest().unwrap();
    let signature =
        CallableSignatureSchema::for_function_value(&source, &PRODUCTION_CALLABLE_LIMITS).unwrap();
    let binder = signature.generic_inventory().template_binder();
    let predicate = signature.effect_predicate();
    let lexical = GenericScope::default().with_binder(binder);
    let reference = lexical.bound_effect(0, 0).unwrap();
    let cancellation = AtomicBool::new(false);
    let mut openings = Vec::new();
    for (labels, accepted) in [
        (&[][..], true),
        (&["fs.read"][..], true),
        (&["fs.write"][..], false),
    ] {
        let mut context = context(&cancellation);
        let application = ConstraintApplicationScope::new(
            (),
            parameters(
                binder,
                reference.clone(),
                EffectConstraintEligibility::Bindable,
                predicate.clone(),
            ),
        );
        let mut path = context.start_path(application).unwrap();
        let opening = path
            .applications
            .root_scope()
            .parameters()
            .effect_reference(&reference)
            .unwrap();
        openings.push(opening.clone());
        let actual = EffectRow::closed(EffectSet::from_labels(labels.iter().copied()).unwrap());
        let admitted = path.effects.constrain_subset(
            &actual,
            &EffectRow::open(EffectSet::new(), opening),
            &mut context,
        );
        assert_eq!(admitted.is_ok(), accepted, "{labels:?}");
        if accepted {
            let solution = TypeConstraintSolution::complete_application(
                &path,
                path.applications.root_id(),
                &mut context,
            )
            .unwrap();
            let value = solution
                .apply_result_template(&TypeKind::function_with_effects(
                    [],
                    TypeKind::Unit,
                    EffectRow::open(EffectSet::new(), reference.clone()),
                ))
                .unwrap();
            let TypeKind::Function {
                binder,
                predicate,
                effects,
                ..
            } = value
            else {
                unreachable!()
            };
            assert!(binder.is_empty());
            assert!(predicate.is_unconstrained());
            assert_eq!(effects, actual);
        }
    }
    assert!(openings.windows(2).all(|pair| pair[0] != pair[1]));
    assert_eq!(source.semantic_identity_digest().unwrap(), source_digest);
}

#[test]
fn known_callback_predicates_distinguish_callable_digests_and_selected_call_candidates() {
    let reading = scheme();
    let mut writing = reading.clone();
    let TypeKind::Function {
        binder, predicate, ..
    } = &mut writing
    else {
        unreachable!()
    };
    *predicate = subset(
        GenericScope::default()
            .with_binder(*binder)
            .bound_effect(0, 0)
            .unwrap(),
        &["fs.write"],
    );
    let callbacks = [reading.clone(), writing];
    let signatures = callbacks
        .iter()
        .map(|callback| {
            CallableSignatureSchema::for_function_value(
                &TypeKind::function_with_effects(
                    [callback.clone()],
                    TypeKind::I64,
                    EffectRow::closed(EffectSet::new()),
                ),
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_ne!(
        signatures[0].semantic_digest(),
        signatures[1].semantic_digest()
    );

    let cancellation = AtomicBool::new(false);
    let admitted = callbacks
        .iter()
        .map(|expected| {
            let mut context = context(&cancellation);
            let path = context
                .start_path(ConstraintApplicationScope::new(
                    (),
                    TypeConstraintParameterScope::empty(),
                ))
                .unwrap();
            super::super::relate_selected_call(
                expected,
                &reading,
                path,
                &mut context,
                super::super::ConstraintAcceptance::PatternAcceptsActual,
            )
            .unwrap()
            .len()
        })
        .collect::<Vec<_>>();
    assert_eq!(admitted, [1, 0]);
}

#[test]
fn returned_scheme_fuses_residual_and_local_predicates_in_templates_and_completed_projections() {
    let external = GenericEffectReference::Free(GenericEffectParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(87_531)),
        0,
    ));
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let path = context
        .start_path(ConstraintApplicationScope::new(
            (),
            parameters(
                GenericBinder::EMPTY,
                external.clone(),
                EffectConstraintEligibility::FutureEligible,
                subset(external.clone(), &["fs.read"]),
            ),
        ))
        .unwrap();
    let application = path.applications.root_id();
    let solution =
        TypeConstraintSolution::complete_application(&path, application, &mut context).unwrap();
    let own = GenericBinder::new(0, 0, 1);
    let local = GenericScope::default().with_binder(own);
    let template = TypeKind::function_with_contract(
        own,
        subset(local.bound_effect(0, 0).unwrap(), &["ui.write"]),
        [TypeKind::function_with_effects(
            [],
            TypeKind::Unit,
            EffectRow::open(EffectSet::new(), external),
        )],
        TypeKind::Unit,
        EffectRow::open(EffectSet::new(), local.bound_effect(0, 0).unwrap()),
    );
    let result = solution.apply_result_template(&template).unwrap();
    let merged = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let expected = subset(merged.bound_effect(0, 0).unwrap(), &["fs.read"])
        .and(
            &subset(merged.bound_effect(0, 1).unwrap(), &["ui.write"]),
            &mut Decisions,
        )
        .unwrap();
    let TypeKind::Function {
        binder, predicate, ..
    } = &result
    else {
        unreachable!()
    };
    assert_eq!(*binder, GenericBinder::new(0, 0, 2));
    assert_eq!(predicate, &expected);
    assert!(result.semantic_identity_digest().is_ok());

    let opened = context
        .open_template_type(&template, &path, application)
        .unwrap();
    let projection = solution
        .reify_projection(
            Arc::new(()),
            &opened,
            &path,
            application,
            ConstraintClosurePolicy::ProjectionFuture,
            &mut context,
        )
        .unwrap();
    assert_eq!(projection.value().to_quantified_type().unwrap(), result);
}

#[derive(Debug, thiserror::Error)]
#[error("predicate projection budget exhausted")]
struct Stop;

struct ProjectionBudget(usize);

impl TypeProjectionControl for ProjectionBudget {
    type Error = Stop;
    fn check(&mut self) -> Result<(), Stop> {
        Ok(())
    }
    fn visit_node(&mut self, _: TypeProjectionNodeKind, _: u64) -> Result<(), Stop> {
        self.0 = self.0.checked_sub(1).ok_or(Stop)?;
        Ok(())
    }
    fn visit_binding(&mut self) -> Result<(), Stop> {
        Ok(())
    }
}

#[test]
fn predicate_binders_survive_known_scheme_substitution_and_bounded_copy() {
    let replacement = scheme();
    let parameter = GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(87_532)),
        0,
    );
    let owner = GenericBinder::new(0, 0, 1);
    let outer = GenericScope::default().with_binder(owner);
    let template = TypeKind::function_with_binder(
        owner,
        [TypeKind::generic_parameter(parameter.clone())],
        TypeKind::Unit,
        EffectRow::open(EffectSet::new(), outer.bound_effect(0, 0).unwrap()),
    );
    let bindings = BTreeMap::from([(&parameter, &replacement)]);
    let mut budget = ProjectionBudget(65_536);
    let projected = template
        .instantiate_type_parameters_with_control(&bindings, &mut budget)
        .unwrap();
    let TypeKind::Function { params, .. } = &projected else {
        unreachable!()
    };
    assert_eq!(params, &[replacement.clone()]);
    assert!(projected.semantic_identity_digest().is_ok());
    for budget in 0..65_536 - budget.0 {
        assert!(
            template
                .instantiate_type_parameters_with_control(&bindings, &mut ProjectionBudget(budget))
                .is_err()
        );
    }
    assert_eq!(replacement, scheme());
}
