use super::support::*;
use std::sync::atomic::AtomicBool;

use arcweft_lang_hir::symbol::ProjectValueLookup;
use arcweft_lang_syntax::{
    ast::{common::TextRange, module_path::CanonicalModulePath, symbol_path::SymbolPath},
    types::parse_type_ref,
};

use crate::{
    callable::{
        CallPoison, CallTargetFact, CallTargetFactError, CallTargetFacts, CallableCandidateId,
        CallableInstantiation, CallableName, CallableQueryLimitError, CapacityMethodId,
        LanguageCallableFamily, PRODUCTION_CALLABLE_LIMITS, ResolveCallError, ResolvedCallable,
        ResolverWork, SignatureOrigin, UnknownCallKind,
    },
    check::TypeCheckReport,
    checker::{
        CandidateEvaluationPass,
        module::{
            analyze_detached_types_for_call_facts, analyze_registered_project_types_for_call_facts,
        },
    },
    diagnostics::TypeCheckErrorKind,
    nominal::{NominalTypeDiagnosticCode, NominalTypeDiagnosticKind},
};
use arcweft_source::SourceRange;

fn analyze_capacity_source(profile: &str, body: &str) -> TypeCheckReport {
    let source = format!(
        "flow @flow.{profile} {profile} {{\n{body}\n}}\n",
        profile = profile.replace('-', "_")
    );
    analyze_registered_source(profile, &source)
}

fn analyze_registered_source(profile: &str, source: &str) -> TypeCheckReport {
    analyze_registered_source_with_environment(profile, source, TypeCheckEnv::standard())
}

fn analyze_registered_source_with_environment(
    profile: &str,
    source: &str,
    environment: TypeCheckEnv,
) -> TypeCheckReport {
    let (document, project, world) =
        crate::test_support::character_project::root_project_source(profile, source);
    let facts = crate::registration::ProjectRegistrationFacts::try_new(
        world,
        vec![document],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("associated capacity registration facts");
    let registered =
        crate::test_support::character_project::register(&project, &facts, environment, None)
            .expect("associated capacity registered world");
    crate::checker::analyze_registered_project_types(&project.linked_module(), &registered)
}

fn analyze_registered_modules(profile: &str, sources: &[(&str, &str)]) -> TypeCheckReport {
    let (documents, project, world) =
        crate::test_support::character_project::project_modules(profile, sources);
    let facts = crate::registration::ProjectRegistrationFacts::try_new(
        world,
        documents,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("associated capacity multi-module registration facts");
    let registered = crate::test_support::character_project::register(
        &project,
        &facts,
        TypeCheckEnv::standard(),
        None,
    )
    .expect("associated capacity multi-module registered world");
    crate::checker::analyze_registered_project_types(&project.linked_module(), &registered)
}

fn sole_capacity_facts(report: &TypeCheckReport) -> &CallTargetFacts {
    assert!(
        report.diagnostics.is_empty(),
        "associated capacity fixture must be accepted: {:?}",
        report.diagnostics
    );
    let facts = report.retained_call_target_facts().collect::<Vec<_>>();
    let [facts] = facts.as_slice() else {
        panic!("expected one retained capacity call, got {facts:?}")
    };
    facts
}

fn selected_capacity(facts: &CallTargetFacts) -> (&ResolvedCallable, &CapacityMethodId) {
    let CallTargetFact::Selected {
        selected,
        considered,
    } = facts.target()
    else {
        panic!(
            "capacity call must select one candidate: {:?}",
            facts.target()
        )
    };
    assert_eq!(considered.as_ref(), std::slice::from_ref(selected.as_ref()));
    let CallableCandidateId::CapacityMethod(id) = selected.id() else {
        panic!("associated call selected a non-capacity candidate: {selected:?}")
    };
    (selected, id)
}

fn assert_type_receiver(instantiation: &CallableInstantiation, expected: &TypeKind) {
    let CallableInstantiation::TypeReceiver { receiver } = instantiation else {
        panic!("associated candidate must retain a type receiver: {instantiation:?}")
    };
    assert_eq!(receiver.receiver(), expected);
}

fn assert_terminal_associated_receiver_failure(
    report: &TypeCheckReport,
    expected_diagnostic: NominalTypeDiagnosticCode,
    expected_arguments: usize,
) {
    assert!(
        report.diagnostics.iter().any(|error| matches!(
            error.kind(),
            TypeCheckErrorKind::Nominal { diagnostic }
                if diagnostic.kind().code() == expected_diagnostic
        )),
        "missing {expected_diagnostic:?} diagnostic: {:?}",
        report.diagnostics
    );
    assert_eq!(report.stats.associated_nominal_receiver_resolutions, 1);
    assert_eq!(report.stats.shared_resolver_invocations, 0);
    assert_eq!(report.stats.associated_typed_environment_lookups, 0);
    assert_eq!(report.stats.associated_capacity_selectors, 0);
    assert_eq!(report.stats.associated_capacity_materializations, 0);
    assert_eq!(report.stats.associated_trait_resolutions, 0);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    assert_eq!(
        report.stats.registered_argument_expression_checks,
        expected_arguments
    );
    assert!(report.physical_candidate_argument_evaluations().is_empty());

    let retained = report.retained_call_target_facts().collect::<Vec<_>>();
    let [facts] = retained.as_slice() else {
        panic!("failed associated receiver must retain one call fact: {retained:?}")
    };
    assert!(matches!(
        facts.target(),
        CallTargetFact::Missing {
            kind: UnknownCallKind::AssociatedType
        }
    ));
    assert_eq!(facts.arguments().len(), expected_arguments);
    assert!(
        facts.arguments().iter().all(
            |argument| argument.slots().len() == 1 && argument.poison() == CallPoison::Rejected
        )
    );
    assert_eq!(facts.result(), None);
    assert_eq!(facts.function_value_type(), None);
}

fn assert_ordinary_callee_recovery(report: &TypeCheckReport, facts: &CallTargetFacts) {
    assert_eq!(report.stats.associated_nominal_receiver_resolutions, 0);
    assert_eq!(report.stats.shared_resolver_invocations, 0);
    assert_eq!(report.stats.associated_typed_environment_lookups, 0);
    assert_eq!(report.stats.associated_capacity_selectors, 0);
    assert_eq!(report.stats.associated_capacity_materializations, 0);
    assert_eq!(report.stats.associated_trait_resolutions, 0);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    assert_eq!(report.stats.registered_argument_expression_checks, 1);
    assert!(report.physical_candidate_argument_evaluations().is_empty());
    assert_eq!(report.retained_argument_inference_facts().count(), 0);

    assert_eq!(report.retained_call_target_facts().count(), 1);
    assert!(matches!(
        facts.target(),
        CallTargetFact::Missing {
            kind: UnknownCallKind::Free
        }
    ));
    assert_eq!(facts.arguments().len(), 1);
    assert_eq!(facts.arguments()[0].slots().len(), 1);
    assert_eq!(facts.arguments()[0].poison(), CallPoison::Recovered);
    assert_eq!(facts.result(), None);
    assert_eq!(facts.function_value_type(), None);
}

fn unknown_associated_type_report(profile: &str) -> TypeCheckReport {
    analyze_registered_source(
        profile,
        r"
fn main() -> Unit {
    let values: Vec<usize> = [3usize]
    let _ = Missing<i32>.with_capacity(1usize, capacity = 2usize, values...)
    ()
}
",
    )
}

fn ambiguous_associated_type_report(profile: &str) -> TypeCheckReport {
    analyze_registered_modules(
        profile,
        &[
            (
                "",
                r"
use crate.left.*
use crate.right.*

fn main() -> Unit {
    let values: Vec<usize> = [3usize]
    let _ = Widget.with_capacity(1usize, capacity = 2usize, values...)
    ()
}
",
            ),
            ("left", "pub struct Widget { left: i32 }\n"),
            ("right", "pub struct Widget { right: i32 }\n"),
        ],
    )
}

fn assert_unknown_associated_call_recovery(report: &TypeCheckReport, expected_arguments: usize) {
    assert_eq!(report.stats.registered_call_expressions, 1);
    assert_eq!(report.stats.associated_nominal_receiver_resolutions, 1);
    assert_eq!(report.stats.shared_resolver_invocations, 1);
    assert_eq!(report.stats.associated_typed_environment_lookups, 1);
    assert_eq!(report.stats.associated_capacity_selectors, 1);
    assert_eq!(report.stats.associated_capacity_materializations, 0);
    assert_eq!(report.stats.associated_trait_resolutions, 1);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    assert_eq!(
        report.stats.registered_argument_expression_checks,
        expected_arguments
    );
    assert!(report.physical_candidate_argument_evaluations().is_empty());

    let retained = report.retained_call_target_facts().collect::<Vec<_>>();
    let [facts] = retained.as_slice() else {
        panic!("unknown associated member must retain one call fact: {retained:?}")
    };
    assert!(matches!(
        facts.target(),
        CallTargetFact::Missing {
            kind: UnknownCallKind::AssociatedType
        }
    ));
    assert_eq!(facts.arguments().len(), expected_arguments);
    assert!(facts.arguments().iter().all(|argument| {
        argument.slots().len() == 1 && argument.poison() == CallPoison::Rejected
    }));
}

fn assert_capacity_identity(
    profile: &str,
    body: &str,
    receiver: &TypeKind,
    arity: usize,
) -> TypeCheckReport {
    let report = analyze_capacity_source(profile, body);
    let facts = sole_capacity_facts(&report);
    let (candidate, id) = selected_capacity(facts);
    assert_eq!(id.receiver(), receiver);
    assert_eq!(id.method().as_str(), "with_capacity");
    assert_eq!(id.arity(), arity);
    assert_eq!(candidate.schema().result(), receiver);
    assert_eq!(facts.result(), Some(receiver));
    assert_type_receiver(candidate.instantiation(), receiver);
    assert!(matches!(
        candidate.origin(),
        SignatureOrigin::Language {
            family: LanguageCallableFamily::CapacityMethod
        }
    ));
    assert_eq!(facts.poison(), CallPoison::Clean);
    report
}

#[test]
fn associated_string_capacity_identity() {
    let report = assert_capacity_identity(
        "associated-string-capacity-identity",
        "    let _ = String.with_capacity(64usize)",
        &TypeKind::String,
        1,
    );
    assert_eq!(report.stats.registered_argument_expression_checks, 1);
}

#[test]
fn associated_capacity_success_exact_counters() {
    let report = assert_capacity_identity(
        "associated-capacity-success-exact-counters",
        "    let _ = String.with_capacity(64usize)",
        &TypeKind::String,
        1,
    );
    assert_eq!(report.stats.registered_call_expressions, 1);
    assert_eq!(report.stats.associated_nominal_receiver_resolutions, 1);
    assert_eq!(report.stats.shared_resolver_invocations, 1);
    assert_eq!(report.stats.associated_typed_environment_lookups, 1);
    assert_eq!(report.stats.associated_capacity_selectors, 1);
    assert_eq!(report.stats.associated_capacity_materializations, 1);
    assert_eq!(report.stats.associated_trait_resolutions, 0);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    assert_eq!(report.stats.registered_argument_expression_checks, 1);
    let evaluations = report.physical_candidate_argument_evaluations();
    let [evaluation] = evaluations else {
        panic!("one singular capacity candidate must evaluate one argument: {evaluations:?}")
    };
    assert_eq!(evaluation.pass, CandidateEvaluationPass::DirectCommitted);
    assert_eq!(report.retained_call_target_facts().count(), 1);
    assert_eq!(report.retained_argument_inference_facts().count(), 1);
}

#[test]
fn associated_environment_override_exact_counters() {
    let environment = TypeCheckEnv::standard().with_method_signature(
        TypeKind::String,
        "with_capacity",
        FunctionSignature::new(
            TypeKind::Bool,
            [FunctionParam::required("capacity", TypeKind::USize)],
        ),
    );
    let report = analyze_registered_source_with_environment(
        "associated-environment-override-exact-counters",
        "flow @flow.associated_environment_override associated_environment_override {\n    let _: bool = String.with_capacity(64usize)\n}\n",
        environment,
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let retained = report.retained_call_target_facts().collect::<Vec<_>>();
    let [facts] = retained.as_slice() else {
        panic!("environment override must retain one call fact")
    };
    assert!(matches!(
        facts.target(),
        CallTargetFact::Selected { selected, .. }
            if matches!(selected.id(), CallableCandidateId::Environment(_))
    ));
    assert_eq!(report.stats.shared_resolver_invocations, 1);
    assert_eq!(report.stats.associated_typed_environment_lookups, 1);
    assert_eq!(report.stats.associated_capacity_selectors, 0);
    assert_eq!(report.stats.associated_capacity_materializations, 0);
    assert_eq!(report.stats.associated_trait_resolutions, 0);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    assert_eq!(report.stats.registered_argument_expression_checks, 1);
}

#[test]
fn associated_trait_fallback_exact_counters() {
    let report = analyze_registered_source(
        "associated-trait-fallback-exact-counters",
        r"
trait StaticFactory {
    fn reserve(self, amount: usize) -> bool
}

impl StaticFactory for String {
    fn reserve(self, amount: usize) -> bool {
        true
    }
}

fn main() -> bool {
    String.reserve(8usize)
}
",
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let retained = report.retained_call_target_facts().collect::<Vec<_>>();
    let [facts] = retained.as_slice() else {
        panic!("associated trait fallback must retain one call fact")
    };
    assert!(matches!(
        facts.target(),
        CallTargetFact::Selected { selected, .. }
            if matches!(selected.id(), CallableCandidateId::TraitMethod(_))
    ));
    assert_eq!(report.stats.shared_resolver_invocations, 1);
    assert_eq!(report.stats.associated_typed_environment_lookups, 1);
    assert_eq!(report.stats.associated_capacity_selectors, 1);
    assert_eq!(report.stats.associated_capacity_materializations, 0);
    assert_eq!(report.stats.associated_trait_resolutions, 1);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    assert_eq!(report.stats.registered_argument_expression_checks, 1);
}

#[test]
fn associated_bytes_capacity_identity() {
    let report = assert_capacity_identity(
        "associated-bytes-capacity-identity",
        "    let _ = Bytes.with_capacity(4096usize)",
        &TypeKind::Bytes,
        1,
    );
    assert_eq!(report.stats.registered_argument_expression_checks, 1);
}

#[test]
fn associated_vec_capacity_identity() {
    let receiver = TypeKind::Vec(Box::new(TypeKind::I32));
    let report = assert_capacity_identity(
        "associated-vec-capacity-identity",
        "    let _ = Vec<i32>.with_capacity(8usize)",
        &receiver,
        1,
    );
    assert_eq!(report.stats.registered_argument_expression_checks, 1);
}

#[test]
fn associated_vec_i32_resolves_structurally() {
    let report = analyze_capacity_source(
        "associated-vec-i32-structural-spellings",
        "    let _ = Vec<i32>.with_capacity(8usize)\n    let _ = Vec<i32>::with_capacity(8usize)\n    let _ = Vec::<i32>.with_capacity(8usize)\n    let _ = Vec::<i32>::with_capacity(8usize)",
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let candidates = report
        .retained_call_target_facts()
        .map(|facts| selected_capacity(facts).0.clone())
        .collect::<Vec<_>>();
    let [first, second, third, fourth] = candidates.as_slice() else {
        panic!("all four associated spellings must retain a candidate: {candidates:?}")
    };
    assert_eq!(first, second);
    assert_eq!(first, third);
    assert_eq!(first, fourth);
    let expected = TypeKind::Vec(Box::new(TypeKind::I32));
    let CallableCandidateId::CapacityMethod(id) = first.id() else {
        panic!("all spellings must retain the CapacityMethod identity")
    };
    assert_eq!(id.receiver(), &expected);
    assert_eq!(first.schema().result(), &expected);
    assert_eq!(report.stats.associated_nominal_receiver_resolutions, 4);
    assert_eq!(report.stats.shared_resolver_invocations, 4);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    assert_eq!(report.stats.registered_argument_expression_checks, 4);
}

#[test]
fn associated_generic_vec_capacity_identity() {
    let report = analyze_registered_source(
        "associated-generic-vec-capacity-identity",
        "fn allocate<T>() -> Vec<T> {\n    Vec<T>.with_capacity(8usize)\n}\n",
    );
    let facts = sole_capacity_facts(&report);
    let (candidate, id) = selected_capacity(facts);
    let TypeKind::Vec(item) = id.receiver() else {
        panic!(
            "generic capacity receiver must remain Vec<T>: {:?}",
            id.receiver()
        )
    };
    let TypeKind::GenericParam(parameter) = item.as_ref() else {
        panic!("generic capacity item must retain a typed parameter identity: {item:?}")
    };
    assert_eq!(parameter.ordinal(), 0);
    assert!(matches!(
        parameter.owner(),
        crate::types::GenericTypeOwnerId::Callable(owner) if owner.name() == "allocate"
    ));
    assert_eq!(candidate.schema().result(), id.receiver());
    assert_eq!(facts.result(), Some(id.receiver()));
    assert_type_receiver(candidate.instantiation(), id.receiver());
}

#[test]
fn associated_generic_parameter_preserves_id() {
    let report = analyze_registered_source(
        "associated-generic-parameter-spellings",
        "fn allocate<T>() -> Unit {\n    let _ = Vec<T>.with_capacity(8usize)\n    let _ = Vec<T>::with_capacity(8usize)\n    let _ = Vec::<T>.with_capacity(8usize)\n    let _ = Vec::<T>::with_capacity(8usize)\n    ()\n}\n",
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let candidates = report
        .retained_call_target_facts()
        .map(|facts| selected_capacity(facts).0.clone())
        .collect::<Vec<_>>();
    let [first, second, third, fourth] = candidates.as_slice() else {
        panic!("all generic spellings must retain a candidate: {candidates:?}")
    };
    assert_eq!(first, second);
    assert_eq!(first, third);
    assert_eq!(first, fourth);
    let CallableCandidateId::CapacityMethod(id) = first.id() else {
        panic!("all generic spellings must retain CapacityMethod")
    };
    let TypeKind::Vec(item) = id.receiver() else {
        panic!("generic receiver must remain Vec<T>: {:?}", id.receiver())
    };
    let TypeKind::GenericParam(parameter) = item.as_ref() else {
        panic!("generic receiver child must retain a semantic parameter: {item:?}")
    };
    assert_eq!(parameter.ordinal(), 0);
    assert!(matches!(
        parameter.owner(),
        crate::types::GenericTypeOwnerId::Callable(owner) if owner.name() == "allocate"
    ));
    assert_eq!(report.stats.associated_nominal_receiver_resolutions, 4);
    assert_eq!(report.stats.shared_resolver_invocations, 4);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    assert_eq!(report.stats.registered_argument_expression_checks, 4);
}

#[test]
fn associated_shadowed_generic_parameters_keep_scope_identity() {
    let report = analyze_registered_source(
        "associated-shadowed-generic-scope-identity",
        r"
trait ShadowFactory {
    fn outer() -> Unit
    fn inner<T>() -> Unit
}

impl<T> ShadowFactory for Option<T> {
    fn outer() -> Unit {
        let _ = Vec<T>::with_capacity(1usize)
        ()
    }

    fn inner<T>() -> Unit {
        let _ = Vec<T>::with_capacity(2usize)
        ()
    }
}
",
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let parameters = report
        .retained_call_target_facts()
        .map(|facts| {
            let (_, id) = selected_capacity(facts);
            let TypeKind::Vec(item) = id.receiver() else {
                panic!("shadowed generic receiver must remain Vec<T>")
            };
            let TypeKind::GenericParam(parameter) = item.as_ref() else {
                panic!("shadowed receiver child must retain a generic ID: {item:?}")
            };
            parameter.clone()
        })
        .collect::<Vec<_>>();
    let [outer, inner] = parameters.as_slice() else {
        panic!("outer and inner generic calls must both retain facts: {parameters:?}")
    };
    assert_ne!(outer, inner);
    assert_eq!(outer.ordinal(), 0);
    assert_eq!(inner.ordinal(), 0);
    let (
        crate::types::GenericTypeOwnerId::AcceptedSource(outer_owner),
        crate::types::GenericTypeOwnerId::AcceptedSource(inner_owner),
    ) = (outer.owner(), inner.owner())
    else {
        panic!("accepted shadowed generics must retain source-qualified owners")
    };
    assert_eq!(outer_owner.source(), inner_owner.source());
    assert!(outer_owner.range().start() <= inner_owner.range().start());
    assert!(outer_owner.range().end() >= inner_owner.range().end());
    assert_ne!(outer_owner.range(), inner_owner.range());
}

#[test]
fn associated_qualified_type_preserves_declaration_identity() {
    let report = analyze_registered_modules(
        "associated-qualified-declaration-identity",
        &[
            (
                "",
                r"
fn main() -> Unit {
    let _ = crate.left.Buffer.with_capacity(1usize)
    let _ = crate.right.Buffer.with_capacity(2usize)
    ()
}
",
            ),
            ("left", "pub type Buffer = String\n"),
            ("right", "pub type Buffer = String\n"),
        ],
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let aliases = report
        .nominal_resolutions
        .roots()
        .filter_map(|root| report.nominal_resolutions.report(root))
        .flat_map(|resolution| resolution.outcome().product().aliases())
        .map(|alias| alias.alias().clone())
        .collect::<Vec<_>>();
    let [left, right] = aliases.as_slice() else {
        panic!("both qualified aliases must retain declaration IDs: {aliases:?}")
    };
    assert_eq!(left.name().as_str(), "Buffer");
    assert_eq!(right.name().as_str(), "Buffer");
    assert_ne!(left.module(), right.module());
    assert_ne!(left, right);

    let candidates = report
        .retained_call_target_facts()
        .map(|facts| selected_capacity(facts).0.clone())
        .collect::<Vec<_>>();
    let [first, second] = candidates.as_slice() else {
        panic!("both qualified capacity calls must select a candidate")
    };
    assert_eq!(first, second);
    assert_eq!(first.schema().result(), &TypeKind::String);
}

#[test]
fn associated_alias_capacity_uses_normalized_receiver() {
    let report = analyze_registered_source(
        "associated-alias-capacity-normalization",
        "type Alias<T> = Vec<T>\nfn allocate() -> Vec<i32> {\n    Alias<i32>.with_capacity(8usize)\n}\n",
    );
    let receiver = TypeKind::Vec(Box::new(TypeKind::I32));
    let facts = sole_capacity_facts(&report);
    let (candidate, id) = selected_capacity(facts);
    assert_eq!(id.receiver(), &receiver);
    assert_eq!(candidate.schema().result(), &receiver);
    assert_eq!(facts.result(), Some(&receiver));
    assert!(
        report
            .nominal_resolutions
            .roots()
            .filter_map(|root| report.nominal_resolutions.report(root))
            .any(|resolution| !resolution.outcome().product().aliases().is_empty()),
        "the normalized capacity identity must coexist with retained alias facts"
    );
}

#[test]
fn capacity_arity_identity_zero() {
    let report = assert_capacity_identity(
        "capacity-arity-identity-zero",
        "    let _ = String.with_capacity()",
        &TypeKind::String,
        0,
    );
    assert!(sole_capacity_facts(&report).arguments().is_empty());
    assert_eq!(report.stats.registered_argument_expression_checks, 0);
}

#[test]
fn capacity_arity_identity_one() {
    let report = assert_capacity_identity(
        "capacity-arity-identity-one",
        "    let _ = String.with_capacity(1usize)",
        &TypeKind::String,
        1,
    );
    let [argument] = sole_capacity_facts(&report).arguments() else {
        panic!("one authored capacity argument must retain one fact")
    };
    assert_eq!(argument.slots().len(), 1);
    assert_eq!(report.stats.registered_argument_expression_checks, 1);
}

#[test]
fn capacity_arity_identity_multiple() {
    let report = assert_capacity_identity(
        "capacity-arity-identity-multiple",
        "    let _ = String.with_capacity(1usize, 2usize, 3usize)",
        &TypeKind::String,
        3,
    );
    let facts = sole_capacity_facts(&report);
    assert_eq!(facts.arguments().len(), 3);
    assert!(
        facts
            .arguments()
            .iter()
            .all(|argument| argument.slots().len() == 1)
    );
    assert_eq!(report.stats.registered_argument_expression_checks, 3);
}

#[test]
fn capacity_arity_identity_named() {
    let report = assert_capacity_identity(
        "capacity-arity-identity-named",
        "    let n: usize = 8usize\n    let _ = String.with_capacity(capacity = n)",
        &TypeKind::String,
        1,
    );
    let [argument] = sole_capacity_facts(&report).arguments() else {
        panic!("named capacity argument must retain one fact")
    };
    assert_eq!(
        argument.authored_name().map(CallableName::as_str),
        Some("capacity")
    );
    assert!(!argument.spread());
    let [slot] = argument.slots() else {
        panic!("named capacity argument must retain one checked slot")
    };
    assert_eq!(slot.inferred(), Some(&TypeKind::USize));
    assert_eq!(slot.expected(), None);
    assert_eq!(slot.poison(), CallPoison::Clean);
    assert_eq!(report.stats.registered_argument_expression_checks, 1);
}

#[test]
fn capacity_arity_identity_spread() {
    let report = assert_capacity_identity(
        "capacity-arity-identity-spread",
        "    let values: Vec<usize> = [1usize, 2usize]\n    let _ = String.with_capacity(values...)",
        &TypeKind::String,
        1,
    );
    let [argument] = sole_capacity_facts(&report).arguments() else {
        panic!("spread capacity argument must retain one fact")
    };
    assert!(argument.spread());
    let [slot] = argument.slots() else {
        panic!("unchecked spread must check its authored container once")
    };
    assert_eq!(
        slot.inferred(),
        Some(&TypeKind::Vec(Box::new(TypeKind::USize)))
    );
    assert_eq!(slot.expected(), None);
    assert_eq!(slot.poison(), CallPoison::Clean);
    assert_eq!(report.stats.registered_argument_expression_checks, 1);
}

#[test]
fn capacity_arity_identity_mixed() {
    let report = assert_capacity_identity(
        "capacity-arity-identity-mixed",
        "    let n: usize = 8usize\n    let values: Vec<usize> = [2usize]\n    let _ = String.with_capacity(1usize, capacity = n, values...)",
        &TypeKind::String,
        3,
    );
    let arguments = sole_capacity_facts(&report).arguments();
    assert_eq!(arguments.len(), 3);
    assert_eq!(arguments[0].authored_name(), None);
    assert_eq!(
        arguments[1].authored_name().map(CallableName::as_str),
        Some("capacity")
    );
    assert!(arguments[2].spread());
    assert!(arguments.iter().all(|argument| argument.slots().len() == 1));
    assert_eq!(report.stats.registered_argument_expression_checks, 3);
}

fn assert_capacity_authority_facts_equal(
    accepted: &CallTargetFacts,
    detached: &CallTargetFacts,
    argument_checks: usize,
) {
    let (
        CallTargetFact::Selected {
            selected: accepted_selected,
            considered: accepted_considered,
        },
        CallTargetFact::Selected {
            selected: detached_selected,
            considered: detached_considered,
        },
    ) = (accepted.target(), detached.target())
    else {
        panic!("both authorities must select the Capacity candidate")
    };
    assert_eq!(accepted_selected, detached_selected);
    assert_eq!(accepted_considered, detached_considered);
    assert_eq!(accepted.document(), detached.document());
    assert_eq!(accepted.call_span(), detached.call_span());
    assert_eq!(accepted.arguments(), detached.arguments());
    assert_eq!(accepted.result(), detached.result());
    assert_eq!(accepted.effects(), detached.effects());
    assert_eq!(accepted.current_group(), detached.current_group());
    assert_eq!(accepted.next_group(), detached.next_group());
    assert_eq!(
        accepted.function_value_type(),
        detached.function_value_type()
    );
    assert_eq!(accepted.poison(), detached.poison());
    assert_eq!(accepted.diagnostics(), detached.diagnostics());
    assert_eq!(accepted.active_parameter(), detached.active_parameter());
    assert_eq!(accepted.arguments().len(), argument_checks);
    assert_eq!(accepted.poison(), CallPoison::Clean);
    assert!(accepted.diagnostics().is_empty());
}

fn assert_capacity_authority_work_equal(accepted: &ResolverWork, detached: &ResolverWork) {
    assert_eq!(accepted.associated_report(), detached.associated_report());
    assert_eq!(accepted.associated_report().typed_environment_lookups(), 1);
    assert_eq!(accepted.associated_report().capacity_selectors(), 1);
    assert_eq!(accepted.associated_report().capacity_materializations(), 1);
    assert_eq!(accepted.associated_report().trait_resolutions(), 0);
}

fn assert_capacity_authority_reports_equal(
    accepted: &TypeCheckReport,
    detached: &TypeCheckReport,
    argument_checks: usize,
) {
    assert_eq!(accepted.stats.shared_resolver_invocations, 1);
    assert_eq!(detached.stats.shared_resolver_invocations, 1);
    assert_eq!(accepted.stats.old_dispatch_calls, 0);
    assert_eq!(detached.stats.old_dispatch_calls, 0);
    assert_eq!(
        accepted.stats.registered_argument_expression_checks,
        argument_checks
    );
    assert_eq!(
        detached.stats.registered_argument_expression_checks,
        argument_checks
    );
    let accepted_evaluations = accepted.physical_candidate_argument_evaluations();
    assert_eq!(
        accepted_evaluations,
        detached.physical_candidate_argument_evaluations()
    );
    assert_eq!(accepted_evaluations.len(), argument_checks);
    assert!(
        accepted_evaluations
            .iter()
            .all(|evaluation| evaluation.pass == CandidateEvaluationPass::DirectCommitted)
    );
}

fn assert_associated_capacity_argument_parity(
    profile: &str,
    preamble: &str,
    call: &str,
    argument_checks: usize,
) {
    let source = format!("fn main() -> String {{\n{preamble}    {call}\n}}\n");
    let (document, project, world_id) =
        crate::test_support::character_project::root_project_source(profile, &source);
    let call_start = source.find(call).expect("parity call is unique");
    let call_span = document
        .span(SourceRange::new(call_start, call_start + call.len()))
        .expect("parity call belongs to the accepted source");
    let environment = TypeCheckEnv::standard();
    let facts = crate::registration::ProjectRegistrationFacts::try_new(
        world_id,
        vec![document.clone()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("argument parity registration facts");
    let registered = crate::test_support::character_project::register(
        &project,
        &facts,
        environment.clone(),
        None,
    )
    .expect("argument parity registered world");
    let module = project.linked_module();
    let cancellation = AtomicBool::new(false);
    let mut accepted_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let accepted = analyze_registered_project_types_for_call_facts(
        &module,
        &registered,
        call_span.clone(),
        &cancellation,
        &mut accepted_work,
    )
    .expect("accepted argument parity analysis");
    let mut detached_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let detached = analyze_detached_types_for_call_facts(
        &module,
        &environment,
        call_span,
        &cancellation,
        &mut detached_work,
    )
    .expect("detached argument parity analysis");
    assert_capacity_authority_facts_equal(
        accepted
            .focused_call_target_facts()
            .expect("accepted argument facts"),
        detached
            .focused_call_target_facts()
            .expect("detached argument facts"),
        argument_checks,
    );
    assert_capacity_authority_work_equal(&accepted_work, &detached_work);
    assert_capacity_authority_reports_equal(accepted.report(), detached.report(), argument_checks);
}

#[test]
fn associated_capacity_registered_detached_argument_parity() {
    for (profile, preamble, call, argument_checks) in [
        (
            "associated-capacity-argument-parity-zero",
            "",
            "String.with_capacity()",
            0,
        ),
        (
            "associated-capacity-argument-parity-multiple",
            "",
            "String.with_capacity(1usize, 2usize, 3usize)",
            3,
        ),
        (
            "associated-capacity-argument-parity-named",
            "",
            "String.with_capacity(capacity = 1usize)",
            1,
        ),
        (
            "associated-capacity-argument-parity-spread",
            "    let values: Vec<usize> = [1usize, 2usize]\n",
            "String.with_capacity(values...)",
            1,
        ),
    ] {
        assert_associated_capacity_argument_parity(profile, preamble, call, argument_checks);
    }
}

#[test]
fn associated_string_resolves_builtin_type() {
    let report = analyze_capacity_source(
        "associated-string-builtin-type",
        "    let _: String = String.with_capacity(8usize)",
    );
    assert!(
        report
            .nominal_resolutions
            .roots()
            .filter_map(|root| report.nominal_resolutions.report(root))
            .any(|resolution| resolution.outcome().product().recovered() == &TypeKind::String)
    );
    let (candidate, id) = selected_capacity(sole_capacity_facts(&report));
    assert_eq!(id.receiver(), &TypeKind::String);
    assert_eq!(candidate.schema().result(), &TypeKind::String);
}

#[test]
fn associated_bytes_resolves_builtin_type() {
    let report = analyze_capacity_source(
        "associated-bytes-builtin-type",
        "    let _: Bytes = Bytes.with_capacity(8usize)",
    );
    assert!(
        report
            .nominal_resolutions
            .roots()
            .filter_map(|root| report.nominal_resolutions.report(root))
            .any(|resolution| resolution.outcome().product().recovered() == &TypeKind::Bytes)
    );
    let (candidate, id) = selected_capacity(sole_capacity_facts(&report));
    assert_eq!(id.receiver(), &TypeKind::Bytes);
    assert_eq!(candidate.schema().result(), &TypeKind::Bytes);
}

#[test]
fn associated_alias_normalizes_target_and_retains_alias_facts() {
    let report = analyze_registered_source(
        "associated-alias-normalized-target-facts",
        "type Alias<T> = Vec<T>\nfn allocate() -> Vec<i32> {\n    Alias<i32>.with_capacity(8usize)\n}\n",
    );
    let normalized = TypeKind::Vec(Box::new(TypeKind::I32));
    let (candidate, id) = selected_capacity(sole_capacity_facts(&report));
    assert_eq!(id.receiver(), &normalized);
    assert_eq!(candidate.schema().result(), &normalized);
    assert!(
        report
            .nominal_resolutions
            .roots()
            .filter_map(|root| report.nominal_resolutions.report(root))
            .any(|resolution| !resolution.outcome().product().aliases().is_empty())
    );
}

#[test]
fn associated_unknown_type_is_terminal() {
    let report = unknown_associated_type_report("associated-unknown-type-terminal");
    assert_terminal_associated_receiver_failure(&report, NominalTypeDiagnosticCode::UnknownType, 3);
}

#[test]
fn associated_unknown_type_checks_arguments_once() {
    let report = unknown_associated_type_report("associated-unknown-type-argument-recovery");
    assert_terminal_associated_receiver_failure(&report, NominalTypeDiagnosticCode::UnknownType, 3);
}

#[test]
fn associated_type_failure_exact_counters() {
    let report = unknown_associated_type_report("associated-type-failure-exact-counters");
    assert_terminal_associated_receiver_failure(&report, NominalTypeDiagnosticCode::UnknownType, 3);
    assert_eq!(report.stats.registered_call_expressions, 1);
    assert_eq!(report.stats.associated_value_namespace_lookups, 1);
}

#[test]
fn associated_ambiguous_type_is_terminal() {
    let report = ambiguous_associated_type_report("associated-ambiguous-type-terminal");
    assert_terminal_associated_receiver_failure(
        &report,
        NominalTypeDiagnosticCode::AmbiguousType,
        3,
    );
}

#[test]
fn associated_ambiguous_type_checks_arguments_once() {
    let report = ambiguous_associated_type_report("associated-ambiguous-type-argument-recovery");
    assert_terminal_associated_receiver_failure(
        &report,
        NominalTypeDiagnosticCode::AmbiguousType,
        3,
    );
}

#[test]
fn associated_wrong_kind_type_is_terminal() {
    let report = analyze_registered_source(
        "associated-wrong-kind-type-terminal",
        r"
fn NotAType() -> Unit { () }

fn main() -> Unit {
    let _ = NotAType<i32>::with_capacity(1usize)
    ()
}
",
    );
    assert_terminal_associated_receiver_failure(&report, NominalTypeDiagnosticCode::WrongKind, 1);
    assert_eq!(report.stats.associated_value_namespace_lookups, 0);
}

#[test]
fn associated_unresolved_generic_argument_is_structural_failure() {
    const SOURCE: &str = r"
fn main() -> Unit {
    let _ = Vec<Missing>.with_capacity(8usize)
    ()
}
";
    let report = analyze_registered_source("associated-unresolved-generic-argument", SOURCE);
    assert_terminal_associated_receiver_failure(&report, NominalTypeDiagnosticCode::UnknownType, 1);

    let missing_start = SOURCE
        .find("Missing")
        .expect("nested missing type spelling");
    let diagnostic = report
        .diagnostics
        .iter()
        .find_map(|error| match error.kind() {
            TypeCheckErrorKind::Nominal { diagnostic }
                if matches!(diagnostic.kind(), NominalTypeDiagnosticKind::Unknown { .. }) =>
            {
                Some(diagnostic)
            }
            _ => None,
        })
        .expect("nested unknown type diagnostic");
    assert_eq!(
        diagnostic.primary().local(),
        TextRange::new(missing_start, missing_start + "Missing".len())
    );
    assert!(
        report
            .nominal_resolutions
            .roots()
            .filter_map(|root| report.nominal_resolutions.report(root))
            .all(
                |resolution| match resolution.outcome().product().recovered() {
                    TypeKind::Named(name) => name != "Missing",
                    TypeKind::Vec(item) => {
                        !matches!(item.as_ref(), TypeKind::Named(name) if name == "Missing")
                    }
                    _ => true,
                }
            )
    );
}

#[test]
fn associated_alias_cycle_is_terminal() {
    let report = analyze_registered_source(
        "associated-alias-cycle-terminal",
        r"
type First = Second
type Second = First

fn main() -> Unit {
    let _ = First.with_capacity(1usize)
    ()
}
",
    );
    assert_terminal_associated_receiver_failure(&report, NominalTypeDiagnosticCode::CyclicAlias, 1);
}

#[test]
fn associated_invalid_member_checks_arguments_once() {
    let report = analyze_registered_source(
        "associated-invalid-member-argument-recovery",
        r"
fn main() -> Unit {
    let values: Vec<usize> = [3usize]
    let _ = String.reserve(1usize, amount = 2usize, values...)
    ()
}
",
    );
    assert_unknown_associated_call_recovery(&report, 3);
}

#[test]
fn associated_recovered_argument_has_one_slot() {
    let report = analyze_registered_source(
        "associated-recovered-argument-one-slot",
        r"
fn main() -> Unit {
    let _ = String.reserve(8usize)
    ()
}
",
    );
    assert_unknown_associated_call_recovery(&report, 1);
    let facts = report
        .retained_call_target_facts()
        .next()
        .expect("unknown associated call fact");
    let [argument] = facts.arguments() else {
        panic!("one authored value must retain one argument fact")
    };
    let [slot] = argument.slots() else {
        panic!("one recovered value must retain one argument slot")
    };
    assert_eq!(slot.inferred(), Some(&TypeKind::USize));
    assert_eq!(slot.expected(), None);
    assert_eq!(slot.poison(), CallPoison::Rejected);
}

#[test]
fn associated_trait_ambiguity_checks_arguments_once() {
    let report = analyze_registered_source(
        "associated-trait-ambiguity-argument-recovery",
        r"
trait FirstFactory {
    fn reserve(self, amount: usize) -> bool
}

trait SecondFactory {
    fn reserve(self, amount: usize) -> bool
}

impl FirstFactory for String {
    fn reserve(self, amount: usize) -> bool { true }
}

impl SecondFactory for String {
    fn reserve(self, amount: usize) -> bool { true }
}

fn main() -> Unit {
    let values: Vec<usize> = [3usize]
    let _ = String.reserve(1usize, amount = 2usize, values...)
    ()
}
",
    );
    assert!(report.diagnostics.iter().any(|error| matches!(
        error.kind(),
        TypeCheckErrorKind::Trait { diagnostic }
            if diagnostic.code() == "sema.trait.ambiguous_method"
    )));
    assert_eq!(report.stats.registered_call_expressions, 1);
    assert_eq!(report.stats.shared_resolver_invocations, 1);
    assert_eq!(report.stats.associated_typed_environment_lookups, 1);
    assert_eq!(report.stats.associated_capacity_selectors, 1);
    assert_eq!(report.stats.associated_capacity_materializations, 0);
    assert_eq!(report.stats.associated_trait_resolutions, 1);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    assert_eq!(report.stats.registered_argument_expression_checks, 3);
    assert!(report.physical_candidate_argument_evaluations().is_empty());
    assert_eq!(report.retained_call_target_facts().count(), 0);
}

#[test]
fn associated_work_exhaustion_is_atomic() {
    const SOURCE: &str = r"
fn main() -> Vec<i32> {
    Vec<i32>.with_capacity(1usize, 2usize)
}
";
    const CALL: &str = "Vec<i32>.with_capacity(1usize, 2usize)";
    let (document, project, world_id) = crate::test_support::character_project::root_project_source(
        "associated-work-exhaustion-atomicity",
        SOURCE,
    );
    let facts = crate::registration::ProjectRegistrationFacts::try_new(
        world_id,
        vec![document.clone()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("associated work fixture registration facts");
    let registered = crate::test_support::character_project::register(
        &project,
        &facts,
        TypeCheckEnv::standard(),
        None,
    )
    .expect("associated work fixture registered world");
    let call_start = SOURCE.find(CALL).expect("associated work call spelling");
    let call_span = document
        .span(SourceRange::new(call_start, call_start + CALL.len()))
        .expect("associated work call span");
    let linked = project.linked_module();
    let cancellation = AtomicBool::new(false);

    let mut accepted_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let accepted = analyze_registered_project_types_for_call_facts(
        &linked,
        &registered,
        call_span.clone(),
        &cancellation,
        &mut accepted_work,
    )
    .expect("accepted associated work query");
    accepted
        .focused_call_target_facts()
        .expect("accepted associated work query publishes one target");
    let required = accepted_work.consumed();
    assert!(required > 0);

    for limit in 0..required {
        let mut limited_work = ResolverWork::new(limit);
        let limited = analyze_registered_project_types_for_call_facts(
            &linked,
            &registered,
            call_span.clone(),
            &cancellation,
            &mut limited_work,
        )
        .expect("work exhaustion remains in the focused report");
        assert!(matches!(
            limited.focused_call_target_facts(),
            Err(CallTargetFactError::Resolve { reason, .. })
                if matches!(
                    reason.as_ref(),
                    ResolveCallError::Work(CallableQueryLimitError::Work {
                        requested: 1,
                        consumed,
                        limit: failed_limit,
                    }) if *consumed == limit && *failed_limit == limit
                )
        ));
        assert_eq!(limited_work.consumed(), limit);
        assert_eq!(limited.report().retained_call_target_facts().count(), 0);
        assert_eq!(
            limited.report().stats.registered_argument_expression_checks,
            0
        );
        assert_eq!(
            limited.report().retained_argument_inference_facts().count(),
            0
        );
    }
}

#[test]
fn associated_stale_source_is_noncacheable() {
    const ACCEPTED: &str = r"
fn main() -> String {
    String.with_capacity(1usize)
}
";
    const STALE: &str = r"
fn main() -> String {
    String.with_capacity(2usize)
}
";
    const CALL: &str = "String.with_capacity(2usize)";
    let (accepted_document, project, world_id) =
        crate::test_support::character_project::root_project_source(
            "associated-stale-source-noncacheable",
            ACCEPTED,
        );
    let facts = crate::registration::ProjectRegistrationFacts::try_new(
        world_id,
        vec![accepted_document],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("stale-source registration facts");
    let registered = crate::test_support::character_project::register(
        &project,
        &facts,
        TypeCheckEnv::standard(),
        None,
    )
    .expect("stale-source registered world");
    let stale = crate::test_support::character_project::source_document(
        "arcweft-project://registration-tests/src/main.arcw",
        STALE,
    );
    let start = STALE.find(CALL).expect("stale associated call spelling");
    let span = stale
        .span(SourceRange::new(start, start + CALL.len()))
        .expect("stale associated call span");
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    assert!(matches!(
        analyze_registered_project_types_for_call_facts(
            &project.linked_module(),
            &registered,
            span,
            &cancellation,
            &mut work,
        ),
        Err(CallTargetFactError::FocusedSourceUnavailable { document })
            if document == *stale.identity()
    ));
    assert_eq!(work.consumed(), 0);
    assert_eq!(work.associated_report().typed_environment_lookups(), 0);
    assert_eq!(work.associated_report().capacity_selectors(), 0);
    assert_eq!(work.associated_report().capacity_materializations(), 0);
    assert_eq!(work.associated_report().trait_resolutions(), 0);
}

#[test]
fn associated_dot_project_value_beats_imported_type() {
    const ROOT: &str = r"
use crate.values.Item as Collision
use crate.types.Item as Collision

fn main() -> Unit {
    let _ = Collision.with_capacity(1usize)
    ()
}
";
    let (documents, project, world) = crate::test_support::character_project::project_modules(
        "associated-project-value-before-type",
        &[
            ("", ROOT),
            ("values", "pub fn Item() -> Unit { () }\n"),
            ("types", "pub type Item = String\n"),
        ],
    );
    let facts = crate::registration::ProjectRegistrationFacts::try_new(
        world,
        documents.clone(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("project value/type collision facts");
    let registered = crate::test_support::character_project::register(
        &project,
        &facts,
        TypeCheckEnv::standard(),
        None,
    )
    .expect("project value/type collision registration");
    let receiver = parse_type_ref("Collision").expect("typed receiver path");
    let reference = SymbolPath::try_from(
        receiver
            .value()
            .nominal_path()
            .expect("nominal receiver path")
            .path(),
    )
    .expect("project value reference");
    let receiver_start = ROOT
        .find("Collision.with_capacity")
        .expect("receiver source");
    let source = documents[0]
        .span(SourceRange::new(
            receiver_start,
            receiver_start + "Collision".len(),
        ))
        .expect("accepted receiver span");
    assert!(matches!(
        registered.symbols().resolve_value_target(
            &CanonicalModulePath::crate_root(),
            &reference,
            source,
        ),
        Ok(ProjectValueLookup::Present(callable))
            if callable.declaration().name() == "Item"
                && callable.declaration().module().to_string() == "crate.values"
    ));

    let report =
        crate::checker::analyze_registered_project_types(&project.linked_module(), &registered);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert_eq!(report.stats.associated_value_namespace_lookups, 1);
    assert_eq!(report.stats.associated_nominal_receiver_resolutions, 0);
    assert_eq!(report.stats.shared_resolver_invocations, 1);
    assert_eq!(report.stats.registered_argument_expression_checks, 1);
    assert_eq!(report.stats.associated_typed_environment_lookups, 0);
    assert_eq!(report.stats.associated_capacity_selectors, 0);
    assert_eq!(report.stats.associated_capacity_materializations, 0);
    assert_eq!(report.stats.associated_trait_resolutions, 0);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    assert!(
        report
            .typed_lowering_evidence
            .iter()
            .any(|evidence| matches!(
                &evidence.kind,
                TypedLoweringEvidenceKind::FunctionValueReference { callee, .. }
                    if callee == "Collision"
            ))
    );
    let facts = report
        .retained_call_target_facts()
        .next()
        .expect("project value method-miss facts");
    assert!(matches!(
        facts.target(),
        CallTargetFact::Missing {
            kind: UnknownCallKind::Method,
        }
    ));
    assert_eq!(facts.arguments().len(), 1);
}

#[test]
fn associated_dot_environment_value_beats_type() {
    let environment = TypeCheckEnv::standard()
        .with_symbol("EnvironmentBuffer", TypeKind::Bool)
        .with_method_signature(
            TypeKind::Bool,
            "with_capacity",
            FunctionSignature::new(
                TypeKind::Bool,
                [FunctionParam::required("capacity", TypeKind::USize)],
            ),
        );
    let report = analyze_registered_source_with_environment(
        "associated-environment-value-before-type",
        r"
type EnvironmentBuffer = String

fn main() -> bool {
    EnvironmentBuffer.with_capacity(1usize)
}
",
        environment,
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert_eq!(report.stats.associated_value_namespace_lookups, 1);
    assert_eq!(report.stats.associated_nominal_receiver_resolutions, 0);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    let facts = report
        .retained_call_target_facts()
        .next()
        .expect("environment value call fact");
    assert!(matches!(
        facts.target(),
        CallTargetFact::Selected { selected, .. }
            if matches!(selected.id(), CallableCandidateId::Environment(_))
    ));
    assert_eq!(facts.result(), Some(&TypeKind::Bool));
}

#[test]
fn associated_dot_value_ambiguity_is_terminal() {
    let report = analyze_registered_modules(
        "associated-value-ambiguity-terminal",
        &[
            (
                "",
                r"
use crate.left.Item as Collision
use crate.right.Item as Collision
use crate.types.Item as Collision

fn main() -> Unit {
    let _ = Collision.with_capacity(1usize)
    ()
}
",
            ),
            ("left", "pub fn Item() -> Unit { () }\n"),
            ("right", "pub fn Item() -> Unit { () }\n"),
            ("types", "pub type Item = String\n"),
        ],
    );
    assert!(report.diagnostics.iter().any(|error| matches!(
        error.kind(),
        TypeCheckErrorKind::ProjectValueLookup {
            error: arcweft_lang_hir::symbol::ProjectValueLookupError::Ambiguous {
                candidates,
                ..
            }
        } if candidates.len() == 2
    )));
    assert_eq!(report.stats.associated_value_namespace_lookups, 1);
    assert_eq!(report.stats.associated_nominal_receiver_resolutions, 0);
    assert_eq!(report.stats.shared_resolver_invocations, 0);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    assert_eq!(report.retained_call_target_facts().count(), 0);
}

#[test]
fn associated_dot_value_access_error_is_terminal() {
    let report = analyze_registered_modules(
        "associated-value-access-terminal",
        &[
            (
                "",
                r"
fn main() -> Unit {
    let _ = crate.values.Item.with_capacity(1usize)
    ()
}
",
            ),
            (
                "values",
                "fn Item() -> Unit { () }\npub use crate.types.Item\n",
            ),
            ("types", "pub type Item = String\n"),
        ],
    );
    assert!(
        report.diagnostics.iter().any(|error| matches!(
            error.kind(),
            TypeCheckErrorKind::ProjectValueLookup {
                error: arcweft_lang_hir::symbol::ProjectValueLookupError::Inaccessible {
                    candidates,
                    ..
                }
            } if candidates.len() == 1
        )),
        "expected terminal inaccessible value error: {:#?}",
        report.diagnostics
    );
    assert_eq!(report.stats.associated_value_namespace_lookups, 1);
    assert_eq!(report.stats.associated_nominal_receiver_resolutions, 0);
    assert_eq!(report.stats.shared_resolver_invocations, 0);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    assert_eq!(report.retained_call_target_facts().count(), 0);
}

#[test]
fn associated_malformed_receiver_checks_retained_arguments_once() {
    for malformed in [
        "Vec::<T::>().with_capacity(8usize)",
        "Vec<,T>.with_capacity(8usize)",
    ] {
        let source = format!("fn main<T>() -> Unit {{\n    let _ = {malformed}\n    ()\n}}\n");
        let parsed = parse_source(&source);
        assert!(
            !parsed.errors().is_empty(),
            "malformed receiver must remain ordinary syntax failure: {malformed}"
        );
        let call_start = source.find(malformed).expect("recovered call is unique");
        let call_span = parsed
            .document()
            .span(SourceRange::new(call_start, call_start + malformed.len()))
            .expect("recovered call belongs to the parsed source");
        let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("malformed receiver recovery still lowers the retained module");
        let cancellation = AtomicBool::new(false);
        let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
        let focused = analyze_detached_types_for_call_facts(
            &hir,
            &TypeCheckEnv::standard(),
            call_span,
            &cancellation,
            &mut work,
        )
        .expect("malformed receiver call facts");
        let facts = focused
            .focused_call_target_facts()
            .expect("malformed receiver recovery facts");
        assert_ordinary_callee_recovery(focused.report(), facts);
    }
}

#[test]
fn associated_missing_member_checks_arguments_once() {
    for malformed in ["Vec<i32>.(8usize)", "Vec<i32>::(8usize)"] {
        let source = format!("fn main() -> Unit {{\n    let _ = {malformed}\n    ()\n}}\n");
        let parsed = parse_source(&source);
        assert!(
            !parsed.errors().is_empty(),
            "missing associated member must remain ordinary syntax failure: {malformed}"
        );
        let call_start = source.find(malformed).expect("recovered call is unique");
        let call_span = parsed
            .document()
            .span(SourceRange::new(call_start, call_start + malformed.len()))
            .expect("recovered call belongs to the parsed source");
        let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("missing-member recovery still lowers the retained module");
        let cancellation = AtomicBool::new(false);
        let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
        let focused = analyze_detached_types_for_call_facts(
            &hir,
            &TypeCheckEnv::standard(),
            call_span,
            &cancellation,
            &mut work,
        )
        .expect("missing-member call facts");
        let facts = focused
            .focused_call_target_facts()
            .expect("missing-member recovery facts");
        assert_ordinary_callee_recovery(focused.report(), facts);
    }
}

#[test]
fn associated_spelling_forms_have_equal_candidate() {
    let report = analyze_capacity_source(
        "associated-capacity-equal-spelling-candidates",
        "    let _ = Vec<i32>.with_capacity(8usize)\n    let _ = Vec<i32>::with_capacity(8usize)\n    let _ = Vec::<i32>.with_capacity(8usize)\n    let _ = Vec::<i32>::with_capacity(8usize)",
    );
    let candidates = report
        .retained_call_target_facts()
        .map(|facts| selected_capacity(facts).0.clone())
        .collect::<Vec<_>>();
    let [first, second, third, fourth] = candidates.as_slice() else {
        panic!("all associated spellings must retain a candidate")
    };
    assert_eq!(first, second);
    assert_eq!(first, third);
    assert_eq!(first, fourth);
}

#[test]
fn value_with_capacity_never_static_capacity() {
    let report = analyze_registered_source(
        "value-with-capacity-never-static-capacity",
        "fn main() -> Unit {\n    let value: String = \"value\"\n    let _ = value.with_capacity(8usize)\n    ()\n}\n",
    );
    assert_eq!(report.stats.associated_value_namespace_lookups, 1);
    assert_eq!(report.stats.associated_nominal_receiver_resolutions, 0);
    assert_eq!(report.stats.shared_resolver_invocations, 1);
    assert_eq!(report.stats.associated_typed_environment_lookups, 0);
    assert_eq!(report.stats.associated_capacity_selectors, 0);
    assert_eq!(report.stats.associated_capacity_materializations, 0);
    assert_eq!(report.stats.associated_trait_resolutions, 0);
    let facts = report
        .retained_call_target_facts()
        .next()
        .expect("value-selected missing method fact");
    assert!(matches!(
        facts.target(),
        CallTargetFact::Missing {
            kind: UnknownCallKind::Method,
        }
    ));
    assert_eq!(report.retained_call_target_facts().count(), 1);
    assert_eq!(facts.arguments().len(), 1);
    assert_eq!(facts.arguments()[0].slots().len(), 1);
}

#[test]
fn associated_capacity_old_dispatch_counter_is_zero() {
    let report = analyze_capacity_source(
        "associated-capacity-zero-old-dispatch",
        "    let _ = String.with_capacity(1usize)\n    let _ = Bytes.with_capacity(2usize)\n    let _ = Vec<i32>.with_capacity(3usize)\n    let _ = Vec::<i32>::with_capacity(4usize)",
    );
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert_eq!(report.stats.registered_call_expressions, 4);
    assert_eq!(report.stats.shared_resolver_invocations, 4);
    assert_eq!(report.stats.old_dispatch_calls, 0);
    assert_eq!(report.stats.registered_argument_expression_checks, 4);
}
