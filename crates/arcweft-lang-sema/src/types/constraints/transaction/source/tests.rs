use std::sync::atomic::AtomicBool;

use super::*;
use crate::{
    effect_row::EffectRow,
    effects::EffectSet,
    types::{
        DetachedGenericOwnerId, GenericBinder, GenericParameterOwnerId, GenericScope,
        GenericTypeParameterId,
        constraints::{
            ConstraintSourceContainerPolicy, NoConstraintClient,
            TypeConstraintParameterEligibility, TypeConstraintParameterScope,
            TypeConstraintRejection,
            context::{LocalConstraintAccounting, TypeConstraintLimits},
        },
    },
};

type TestContext<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

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

fn source(actual: TypeKind) -> ConstraintProbe<NoConstraintClient> {
    ConstraintProbe::Active(ActiveConstraintProbe {
        source: (),
        source_ordinal: 0,
        branch: Arc::new(()),
        selection: StoredSourceSelection::Unchecked,
        prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
        value_expected: None,
        actual,
    })
}

#[test]
fn source_operand_cannot_retain_an_unowned_future_type_reference() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let future = context
        .parameter_scope
        .type_reference(&(future_parameter()).clone().into())
        .expect("future slot");
    let probe = source(TypeKind::GenericParam(future.clone()));
    assert_eq!(
        probe
            .close(
                &BTreeMap::new(),
                &BTreeMap::new(),
                &EffectSubstitution::new(),
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
    let mut context = context(&cancellation);
    let closed = source(function.clone())
        .close(
            &BTreeMap::new(),
            &BTreeMap::new(),
            &EffectSubstitution::new(),
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
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let parameter = context
        .parameter_scope
        .type_reference(&(future_parameter()).clone().into())
        .expect("template slot");
    let item = TypeKind::GenericParam(parameter.clone());
    let probe = ConstraintProbe::Active(ActiveConstraintProbe {
        source: (),
        source_ordinal: 3,
        branch: Arc::new(()),
        selection: StoredSourceSelection::Checked {
            alternative: (),
            evidence: Arc::new(()),
        },
        prepared_source_projection: PreparedConstraintSourceProjection::InferSpreadContainer {
            policy: ConstraintSourceContainerPolicy::Positional,
        },
        value_expected: Some(item.clone()),
        actual: TypeKind::Array {
            item: Box::new(item),
            len: ArrayLength::Const(4),
        },
    });
    let closed = probe
        .close(
            &BTreeMap::from([(parameter, TypeKind::I64)]),
            &BTreeMap::new(),
            &EffectSubstitution::new(),
            &mut context,
        )
        .unwrap_or_else(|error| panic!("normalized source: {error}"));
    let expected = TypeKind::Array {
        item: Box::new(TypeKind::I64),
        len: ArrayLength::Const(4),
    };
    assert_eq!(closed.actual(), &expected);
    assert_eq!(closed.final_expected(), Some(&expected));
    assert_eq!(closed.ordinal(), 3);
    assert_eq!(closed.selection().alternative(), Some(()));
    assert_eq!(closed.selection().evidence(), Some(&()));
    assert!(closed.source_projection().matches_actual(closed.actual()));
}

#[test]
fn source_trace_cannot_skip_or_repeat_completion() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let active = source(TypeKind::I64);
    let expected = protocol_error(TypeConstraintSourceProtocolInvariant::Outcome);
    assert_eq!(active.closed().err(), Some(expected.clone()));
    assert_eq!(active.clone().into_closed().err(), Some(expected.clone()));
    let closed = active
        .close(
            &BTreeMap::new(),
            &BTreeMap::new(),
            &EffectSubstitution::new(),
            &mut context,
        )
        .unwrap_or_else(|error| panic!("complete source: {error}"));
    let trace = ConstraintProbe::Closed(closed);
    assert_eq!(
        trace.closed().ok().map(ClosedConstraintProbe::actual),
        Some(&TypeKind::I64)
    );
    assert_eq!(
        trace
            .clone()
            .close(
                &BTreeMap::new(),
                &BTreeMap::new(),
                &EffectSubstitution::new(),
                &mut context
            )
            .err(),
        Some(expected),
    );
    assert!(trace.into_closed().is_ok());
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
    type Source = u8;
    type AlternativeIndex = u8;
    type EvidenceRule = ();
    type ObservedEvidence = ObservedCase;
    type CheckedEvidence = CheckedCase;
    type ProbeSemanticBranch = ();
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
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, EvidenceDomain>::with_scope(
            TypeConstraintLimits::new(4096, 2048, 128, 64),
            &cancellation,
            scope,
        );
    let actual = TypeKind::Option(Box::new(TypeKind::GenericParam(reference.clone())));
    let observed = ObservedCase {
        owner: actual.clone(),
        tag: 7,
    };
    assert!(observed.owner.semantic_identity_digest().is_err());
    let probe = ConstraintProbe::Active(ActiveConstraintProbe {
        source: 2,
        source_ordinal: 0,
        branch: Arc::new(()),
        selection: StoredSourceSelection::Checked {
            alternative: 0,
            evidence: Arc::new(observed),
        },
        prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
        value_expected: Some(actual.clone()),
        actual,
    });
    let closed = probe
        .close(
            &BTreeMap::from([(reference, TypeKind::I64)]),
            &BTreeMap::new(),
            &EffectSubstitution::new(),
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
