use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use arcweft_lang_hir::project::HirProject;
use arcweft_lang_syntax::{
    ast::{common::TextRange, module_path::CanonicalModulePath},
    expr::{CallArg, Expr, ParenthesizedCalleeSyntax, parse_expr},
    types::TypeRefNodePath,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use crate::{
    checker::{
        CandidateEvaluationPass, CandidateExpectedType, PhysicalArgumentEvaluationKind,
        TypeExpressionId, TypeJudgmentRule, TypeJudgmentSubject, analyze_registered_project_types,
        analyze_types, module::analyze_registered_project_types_for_call_facts,
    },
    diagnostics::TypeCheckErrorKind,
    effect_row::EffectRow,
    env::{
        FunctionParam, FunctionSignature, TypeCheckEnv,
        identity::EnvironmentBindingId,
        nominal::{
            AcceptedNominalId, AcceptedNominalOrigin, AcceptedNominalOwnerId,
            AcceptedNominalRecord, AcceptedNominalSemantics,
        },
    },
    nominal::{
        AssociatedReceiverFailure, BuiltinTypeConstructor, DetachedTypeRef, NominalTypeDiagnostic,
        ResolvedAssociatedTypeReceiver, ResolvedTypeNode, ResolvedTypeProduct,
        ResolvedTypeRefOutcome, TypeNameResolution, TypePoisonRecord, TypeResolutionFailure,
        TypeResolutionReport, TypeSourceEvidence,
    },
    registration::{
        CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts,
        RegisteredExternalOwner, RegisteredSemanticWorld, SourceBackedEnvironmentRegistrationInput,
    },
    test_support::character_project::{
        external_fact, one_character_facts, one_character_facts_with_environment, project_path,
        root_project_source, sample_manifest, source_document,
    },
    test_support::environment::source_backed_callable_input,
    traits::TraitCatalog,
    types::{
        AcceptedNominalType, DetachedTypeOwnerId, GenericTypeOwnerId, GenericTypeParameterId,
        TypeKind,
    },
};

use super::facts::CallTargetFact;
use super::{
    AdapterPackageId, AssociatedResolverWorkReport, BuiltinCallableId, CallCallee, CallPoison,
    CallResolverAuthority, CallResolverRequest, CallSourceContext, CallTargetFactError,
    CallableArgumentPolicy, CallableAuthorityRank, CallableCandidateId, CallableDocumentation,
    CallableEffectSchema, CallableFamily, CallableGroupIndex, CallableGroupKind,
    CallableInstantiation, CallableLimits, CallableLookupKey, CallableName, CallableOverloadIndex,
    CallableParameter, CallableParameterGroup, CallableParameterIndex, CallableParameterPassing,
    CallableParameterPresence, CallableParameterType, CallablePath, CallableQueryLimitError,
    CallableSignatureSchema, CallableValidator, CapabilityCallableId, CorruptCallableCatalogReason,
    EnvironmentCallableKind, EnvironmentCallableOwner, EnvironmentCallablePublicationRecord,
    EnvironmentDeclarationOrdinal, LexicalCallableScope, PRODUCTION_CALLABLE_LIMITS,
    ReceiverMethodKey, ResolveCallError, ResolveCallOutcome, ResolvedCallTarget, ResolverWork,
    SignatureOrigin, SignatureQueryStep, SignatureQueryStepControl, SpreadArgumentPolicy,
    StandardEnvironmentId, UnknownNamedArgumentPolicy, resolve_call_target,
};

mod curried_shared_resolver;
mod overload_accounting;

const SOURCE: &str = r#"
use character.akane as hero

fn project_value(value: i32) -> String {
    "project"
}

flow @flow.main main {
    let project: String = project_value(1i32)
    let standard: String = standard_value(2i32)
    let adapter: String = adapter_value(3i32)
    let overloaded: String = overloaded_value(4i32)
    let dotted: String = custom.read(path = "opening.txt")
    let inferred: String = infer.run(value = 4i32)
    let item: Vec<i32> = [1i32, 2i32]
    let item_len: usize = item.len()
}
"#;

#[derive(Clone)]
struct ResolverFixture {
    document: Arc<SourceDocument>,
    project: HirProject,
    world: RegisteredSemanticWorld,
}

struct CorruptFreeCase {
    source: &'static [&'static str],
    alternate: Option<&'static [&'static str]>,
    query: &'static [&'static str],
    reason: CorruptCallableCatalogReason,
}

impl ResolverFixture {
    fn new() -> Self {
        Self::with_profile("callable-resolver")
    }

    fn with_profile(profile: &str) -> Self {
        let base = TypeCheckEnv::standard()
            .with_symbol("infer", TypeKind::I32)
            .with_function_signature(
                "standard_value",
                FunctionSignature::new(
                    TypeKind::String,
                    [FunctionParam::required("value", TypeKind::I32)],
                ),
            )
            .with_function_signature("akane", FunctionSignature::new(TypeKind::Unit, []))
            .with_function_signature(
                "character.akane",
                FunctionSignature::new(TypeKind::Unit, []),
            )
            .with_function_signature("hero", FunctionSignature::new(TypeKind::Unit, []));
        Self::with_environment(profile, base)
    }

    fn with_environment(profile: &str, base: TypeCheckEnv) -> Self {
        Self::with_source_and_environment(profile, SOURCE, base)
    }

    fn with_source_and_environment(profile: &str, source: &str, base: TypeCheckEnv) -> Self {
        let (document, project, world) = root_project_source(profile, source);
        let (environment_document, environment_input) = adapter_environment_input();
        let facts = one_character_facts_with_environment(
            &document,
            vec![Arc::clone(&document), environment_document],
            world,
            &sample_manifest("layers/body.png"),
            vec![environment_input],
        );
        let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
            Arc::new(base),
            &project,
            &facts,
            None,
        ))
        .expect("registered callable resolver fixture");
        Self {
            document,
            project,
            world,
        }
    }

    fn resolve(&self, name: &str) -> ResolveCallOutcome {
        self.resolve_path(&[name])
    }

    fn resolve_path(&self, segments: &[&str]) -> ResolveCallOutcome {
        let path = callable_path(segments);
        let lexical = LexicalCallableScope::default();
        let module = CanonicalModulePath::crate_root();
        let cancellation = AtomicBool::new(false);
        let traits = TraitCatalog::default();
        let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
        let request = CallResolverRequest::try_new(
            CallCallee::Free {
                path: &path,
                enum_variant: None,
            },
            CallResolverAuthority::accepted(&module, self.world.symbols(), &self.world),
            &lexical,
            None,
            &traits,
            &[],
            CallSourceContext::accepted(self.document.identity(), None, None),
            CallableGroupIndex::ZERO,
            TypeExpressionId::from_index(0),
            &cancellation,
            &mut work,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("resolver request");
        resolve_call_target(request)
    }

    fn resolve_method(&self, receiver_type: &TypeKind, method_name: &str) -> ResolveCallOutcome {
        let method = CallableName::try_new(method_name).expect("method name");
        let lexical = LexicalCallableScope::default();
        let module = CanonicalModulePath::crate_root();
        let cancellation = AtomicBool::new(false);
        let traits = TraitCatalog::default();
        let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
        let request = CallResolverRequest::try_new(
            CallCallee::Selected {
                receiver_expression: TypeExpressionId::from_index(0),
                receiver_type,
                method: &method,
                arguments: &[],
            },
            CallResolverAuthority::accepted(&module, self.world.symbols(), &self.world),
            &lexical,
            None,
            &traits,
            &[],
            CallSourceContext::accepted(self.document.identity(), None, None),
            CallableGroupIndex::ZERO,
            TypeExpressionId::from_index(1),
            &cancellation,
            &mut work,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("selected resolver request");
        resolve_call_target(request)
    }

    fn resolve_associated(
        &self,
        product: &ResolvedTypeProduct,
        member_name: &str,
        arguments: &[CallArg],
    ) -> ResolveCallOutcome {
        self.resolve_associated_with_traits(
            product,
            member_name,
            arguments,
            &TraitCatalog::default(),
        )
    }

    fn resolve_associated_with_traits(
        &self,
        product: &ResolvedTypeProduct,
        member_name: &str,
        arguments: &[CallArg],
        traits: &TraitCatalog,
    ) -> ResolveCallOutcome {
        self.resolve_associated_with_traits_and_work(product, member_name, arguments, traits)
            .0
    }

    fn resolve_associated_with_traits_and_work(
        &self,
        product: &ResolvedTypeProduct,
        member_name: &str,
        arguments: &[CallArg],
        traits: &TraitCatalog,
    ) -> (ResolveCallOutcome, AssociatedResolverWorkReport) {
        let (outcome, work) = self.resolve_associated_with_work_limit(
            product,
            member_name,
            arguments,
            traits,
            PRODUCTION_CALLABLE_LIMITS.max_query_work(),
        );
        (outcome, work.associated_report())
    }

    fn resolve_associated_with_work_limit(
        &self,
        product: &ResolvedTypeProduct,
        member_name: &str,
        arguments: &[CallArg],
        traits: &TraitCatalog,
        work_limit: u64,
    ) -> (ResolveCallOutcome, ResolverWork) {
        self.resolve_associated_with_limits_and_work(
            product,
            member_name,
            arguments,
            traits,
            work_limit,
            &PRODUCTION_CALLABLE_LIMITS,
        )
    }

    fn resolve_associated_with_limits(
        &self,
        product: &ResolvedTypeProduct,
        member_name: &str,
        arguments: &[CallArg],
        traits: &TraitCatalog,
        limits: &CallableLimits,
    ) -> (ResolveCallOutcome, ResolverWork) {
        self.resolve_associated_with_limits_and_work(
            product,
            member_name,
            arguments,
            traits,
            limits.max_query_work(),
            limits,
        )
    }

    fn resolve_associated_with_limits_and_work(
        &self,
        product: &ResolvedTypeProduct,
        member_name: &str,
        arguments: &[CallArg],
        traits: &TraitCatalog,
        work_limit: u64,
        limits: &CallableLimits,
    ) -> (ResolveCallOutcome, ResolverWork) {
        let receiver = ResolvedAssociatedTypeReceiver::try_from_product(product)
            .expect("complete nominal product");
        let member = CallableName::try_new(member_name).expect("associated member name");
        let lexical = LexicalCallableScope::default();
        let module = CanonicalModulePath::crate_root();
        let cancellation = AtomicBool::new(false);
        let mut work = ResolverWork::new(work_limit);
        let request = CallResolverRequest::try_new(
            CallCallee::AssociatedType {
                receiver,
                member: &member,
                arguments,
            },
            CallResolverAuthority::accepted(&module, self.world.symbols(), &self.world),
            &lexical,
            None,
            traits,
            &[],
            CallSourceContext::accepted(self.document.identity(), None, None),
            CallableGroupIndex::ZERO,
            TypeExpressionId::from_index(2),
            &cancellation,
            &mut work,
            limits,
        )
        .expect("associated resolver request");
        let outcome = resolve_call_target(request);
        (outcome, work)
    }

    fn with_corrupt_free_catalog(
        mut self,
        source: &[&str],
        alternate: Option<&[&str]>,
        reason: CorruptCallableCatalogReason,
    ) -> Self {
        let source = callable_path(source);
        let alternate = alternate.map(callable_path);
        let catalog = self
            .world
            .environment()
            .callable_catalog()
            .with_corrupt_free_set_for_test(&source, alternate.as_ref(), reason);
        Arc::make_mut(&mut self.world.environment).callables = Arc::new(catalog);
        self
    }
}

fn assert_typed_authority_surface(
    syntax: &ParenthesizedCalleeSyntax,
    callee: &CallCallee<'_>,
    authority: CallResolverAuthority<'_>,
) {
    assert!(matches!(syntax, ParenthesizedCalleeSyntax::PathMember(_)));
    assert!(matches!(callee, CallCallee::AssociatedType { .. }));
    match authority {
        CallResolverAuthority::Accepted {
            current_module,
            symbols,
            world,
        } => {
            assert_eq!(current_module, &CanonicalModulePath::crate_root());
            assert_eq!(symbols.world(), world.symbols().world());
            assert_eq!(symbols.revision(), world.symbols().revision());
        }
        CallResolverAuthority::Detached { .. } => {}
    }
}

const fn authority_is_accepted(authority: CallResolverAuthority<'_>) -> bool {
    match authority {
        CallResolverAuthority::Accepted { .. } => true,
        CallResolverAuthority::Detached { .. } => false,
    }
}

const fn exhaustive_type_receiver(instantiation: &CallableInstantiation) -> Option<&TypeKind> {
    match instantiation {
        CallableInstantiation::TypeReceiver { receiver } => Some(receiver.receiver()),
        CallableInstantiation::None
        | CallableInstantiation::ExpectedEnum { .. }
        | CallableInstantiation::Result { .. }
        | CallableInstantiation::Option { .. }
        | CallableInstantiation::Character { .. }
        | CallableInstantiation::Receiver { .. }
        | CallableInstantiation::Curried { .. }
        | CallableInstantiation::DataLast { .. } => None,
    }
}

fn resolve_detached_associated(
    environment: &TypeCheckEnv,
    product: &ResolvedTypeProduct,
    member_name: &str,
    arguments: &[CallArg],
    traits: &TraitCatalog,
) -> ResolveCallOutcome {
    resolve_detached_associated_with_work(environment, product, member_name, arguments, traits).0
}

fn resolve_detached_associated_with_work(
    environment: &TypeCheckEnv,
    product: &ResolvedTypeProduct,
    member_name: &str,
    arguments: &[CallArg],
    traits: &TraitCatalog,
) -> (ResolveCallOutcome, AssociatedResolverWorkReport) {
    let receiver = ResolvedAssociatedTypeReceiver::try_from_product(product)
        .expect("complete detached nominal product");
    let member = CallableName::try_new(member_name).expect("detached associated member name");
    let lexical = LexicalCallableScope::default();
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let request = CallResolverRequest::try_new(
        CallCallee::AssociatedType {
            receiver,
            member: &member,
            arguments,
        },
        CallResolverAuthority::detached(environment),
        &lexical,
        None,
        traits,
        &[],
        CallSourceContext::detached(8, Some(TextRange::new(0, 8)), Some(TextRange::new(0, 4))),
        CallableGroupIndex::ZERO,
        TypeExpressionId::from_index(3),
        &cancellation,
        &mut work,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("detached associated resolver request");
    let outcome = resolve_call_target(request);
    (outcome, work.associated_report())
}

fn associated_trait_catalog() -> TraitCatalog {
    const TRAIT_SOURCE: &str = r"
trait StaticFactory {
    fn with_capacity(self, amount: usize) -> bool
    fn with_capacitx(self, amount: usize) -> bool
    fn reserve(self, amount: usize) -> bool
}

impl StaticFactory for String {
    fn with_capacity(self, amount: usize) -> bool {
        true
    }

    fn with_capacitx(self, amount: usize) -> bool {
        true
    }

    fn reserve(self, amount: usize) -> bool {
        true
    }
}
";
    let (_, project, _) = root_project_source("associated-trait-order", TRAIT_SOURCE);
    let report = analyze_types(&project.linked_module(), &TypeCheckEnv::standard());
    assert!(
        report.diagnostics.is_empty(),
        "associated trait fixture must type check: {:?}",
        report.diagnostics
    );
    report.trait_catalog
}

fn ambiguous_associated_trait_catalog() -> TraitCatalog {
    const TRAIT_SOURCE: &str = r"
trait FirstFactory {
    fn reserve(self, amount: usize) -> bool
}

trait SecondFactory {
    fn reserve(self, amount: usize) -> bool
}

impl FirstFactory for String {
    fn reserve(self, amount: usize) -> bool {
        true
    }
}

impl SecondFactory for String {
    fn reserve(self, amount: usize) -> bool {
        true
    }
}
";
    let (_, project, _) = root_project_source("associated-trait-ambiguity", TRAIT_SOURCE);
    let report = analyze_types(&project.linked_module(), &TypeCheckEnv::standard());
    assert!(
        report.diagnostics.is_empty(),
        "ambiguous trait fixture declarations must type check: {:?}",
        report.diagnostics
    );
    report.trait_catalog
}

#[test]
fn associated_capacity_uses_the_shared_resolver_with_an_exact_type_receiver() {
    let fixture = ResolverFixture::new();
    let receiver_type = TypeKind::Vec(Box::new(TypeKind::I32));
    let product = complete_builtin_product(receiver_type.clone(), BuiltinTypeConstructor::Vec);
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];

    let candidate =
        resolved_candidate(fixture.resolve_associated(&product, "with_capacity", &arguments));
    let CallableCandidateId::CapacityMethod(id) = candidate.id() else {
        panic!("associated capacity must retain the existing capacity identity")
    };
    assert_eq!(id.receiver(), &receiver_type);
    assert_eq!(id.method().as_str(), "with_capacity");
    assert_eq!(id.arity(), 1);
    assert_eq!(candidate.schema().result(), &receiver_type);
    assert_eq!(
        exhaustive_type_receiver(candidate.instantiation()),
        Some(&receiver_type)
    );
    assert!(matches!(
        candidate.origin(),
        SignatureOrigin::Language {
            family: crate::callable::LanguageCallableFamily::CapacityMethod
        }
    ));
}

#[test]
fn associated_capacity_instantiation_rejects_crossed_receiver_roles() {
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let resolved_receiver = ResolvedAssociatedTypeReceiver::try_from_product(&product)
        .expect("complete nominal product");
    let type_receiver = super::TypeReceiverInstantiation::from_resolved(resolved_receiver);
    let member = CallableName::try_new("with_capacity").expect("capacity member");
    let associated =
        crate::callable::CapacityMethodId::resolve_associated(&TypeKind::String, &member, 1)
            .expect("small arity fits identity")
            .expect("String owns associated capacity construction");
    let associated_origin = SignatureOrigin::Language {
        family: crate::callable::LanguageCallableFamily::CapacityMethod,
    };

    assert!(
        super::ResolvedCallable::try_new(
            CallableCandidateId::CapacityMethod(associated.clone()),
            associated_origin.clone(),
            Arc::new(associated.signature_schema()),
            CallableInstantiation::TypeReceiver {
                receiver: type_receiver.clone(),
            },
            Vec::new(),
            None,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .is_ok()
    );
    assert_eq!(
        super::ResolvedCallable::try_new(
            CallableCandidateId::CapacityMethod(associated.clone()),
            associated_origin.clone(),
            Arc::new(associated.signature_schema()),
            CallableInstantiation::Receiver {
                receiver: TypeKind::String,
            },
            Vec::new(),
            None,
            &PRODUCTION_CALLABLE_LIMITS,
        ),
        Err(ResolveCallError::InvalidResolvedCallable)
    );

    let instance = crate::callable::CapacityMethodId::resolve(
        &TypeKind::String,
        &CallableName::try_new("reserve").expect("instance member"),
        1,
    )
    .expect("String owns instance reserve");
    let instance_origin = SignatureOrigin::Language {
        family: crate::callable::LanguageCallableFamily::CapacityMethod,
    };
    assert!(
        super::ResolvedCallable::try_new(
            CallableCandidateId::CapacityMethod(instance.clone()),
            instance_origin.clone(),
            Arc::new(instance.signature_schema()),
            CallableInstantiation::Receiver {
                receiver: TypeKind::String,
            },
            Vec::new(),
            None,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .is_ok()
    );
    assert_eq!(
        super::ResolvedCallable::try_new(
            CallableCandidateId::CapacityMethod(instance.clone()),
            instance_origin,
            Arc::new(instance.signature_schema()),
            CallableInstantiation::TypeReceiver {
                receiver: type_receiver,
            },
            Vec::new(),
            None,
            &PRODUCTION_CALLABLE_LIMITS,
        ),
        Err(ResolveCallError::InvalidResolvedCallable)
    );
}

#[test]
fn associated_capacity_registered_detached_candidate_parity() {
    let environment = TypeCheckEnv::standard();
    let fixture = ResolverFixture::with_environment(
        "associated-capacity-authority-parity",
        environment.clone(),
    );
    let traits = TraitCatalog::default();
    for (receiver, constructor) in [
        (TypeKind::String, BuiltinTypeConstructor::String),
        (TypeKind::Bytes, BuiltinTypeConstructor::Bytes),
        (
            TypeKind::Vec(Box::new(TypeKind::I32)),
            BuiltinTypeConstructor::Vec,
        ),
    ] {
        let product = complete_builtin_product(receiver, constructor);
        for arity in [0usize, 1, 3] {
            let arguments = (0..arity)
                .map(|_| CallArg::Positional(Box::new(Expr::Tuple(Vec::new()))))
                .collect::<Vec<_>>();
            let accepted = resolved_candidate(fixture.resolve_associated_with_traits(
                &product,
                "with_capacity",
                &arguments,
                &traits,
            ));
            let detached = resolved_candidate(resolve_detached_associated(
                &environment,
                &product,
                "with_capacity",
                &arguments,
                &traits,
            ));
            assert_eq!(accepted, detached);
            let CallableCandidateId::CapacityMethod(id) = accepted.id() else {
                panic!("authority parity must retain a CapacityMethod candidate")
            };
            assert_eq!(id.receiver(), product.recovered());
            assert_eq!(id.arity(), arity);
            assert_eq!(accepted.schema().result(), product.recovered());
            assert_eq!(
                exhaustive_type_receiver(accepted.instantiation()),
                Some(product.recovered())
            );
        }
    }

    let generic = complete_generic_vec_product();
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];
    let accepted = resolved_candidate(fixture.resolve_associated_with_traits(
        &generic,
        "with_capacity",
        &arguments,
        &traits,
    ));
    let detached = resolved_candidate(resolve_detached_associated(
        &environment,
        &generic,
        "with_capacity",
        &arguments,
        &traits,
    ));
    assert_eq!(accepted, detached);
    assert_eq!(accepted.schema().result(), generic.recovered());

    let alias_source = "type Alias<T> = Vec<T>\nfn allocate() -> Vec<i32> {\n    Alias<i32>.with_capacity(8usize)\n}\n";
    let (document, project, world_id) =
        root_project_source("associated-capacity-alias-authority-parity", alias_source);
    let facts = one_character_facts(&document, world_id, &sample_manifest("layers/body.png"));
    let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(environment.clone()),
        &project,
        &facts,
        None,
    ))
    .expect("alias parity world registers");
    let report = analyze_registered_project_types(&project.linked_module(), &registered);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let alias = report
        .nominal_resolutions
        .roots()
        .filter_map(|root| report.nominal_resolutions.report(root))
        .map(|resolution| resolution.outcome().product())
        .find(|product| !product.aliases().is_empty())
        .expect("alias receiver retains its expansion product");
    let accepted = resolved_candidate(fixture.resolve_associated_with_traits(
        alias,
        "with_capacity",
        &arguments,
        &traits,
    ));
    let detached = resolved_candidate(resolve_detached_associated(
        &environment,
        alias,
        "with_capacity",
        &arguments,
        &traits,
    ));
    assert_eq!(accepted, detached);
    assert_eq!(accepted.schema().result(), alias.recovered());
}

#[test]
fn associated_registered_detached_counter_parity() {
    let environment = TypeCheckEnv::standard();
    let fixture = ResolverFixture::with_environment(
        "associated-counter-authority-parity",
        environment.clone(),
    );
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];
    let traits = TraitCatalog::default();

    let (accepted, accepted_work) = fixture.resolve_associated_with_traits_and_work(
        &product,
        "with_capacity",
        &arguments,
        &traits,
    );
    let (detached, detached_work) = resolve_detached_associated_with_work(
        &environment,
        &product,
        "with_capacity",
        &arguments,
        &traits,
    );
    assert_eq!(resolved_candidate(accepted), resolved_candidate(detached));
    assert_eq!(accepted_work, detached_work);
    assert_eq!(accepted_work.typed_environment_lookups(), 1);
    assert_eq!(accepted_work.capacity_selectors(), 1);
    assert_eq!(accepted_work.capacity_materializations(), 1);
    assert_eq!(accepted_work.trait_resolutions(), 0);
}

#[test]
fn associated_capacity_source_identity_does_not_change_candidate() {
    let environment = TypeCheckEnv::standard();
    let fixture = ResolverFixture::with_environment(
        "associated-capacity-source-identity-parity",
        environment.clone(),
    );
    let product = complete_builtin_product(
        TypeKind::Vec(Box::new(TypeKind::I32)),
        BuiltinTypeConstructor::Vec,
    );
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];
    let traits = TraitCatalog::default();

    let accepted = resolved_candidate(fixture.resolve_associated_with_traits(
        &product,
        "with_capacity",
        &arguments,
        &traits,
    ));
    let detached = resolved_candidate(resolve_detached_associated(
        &environment,
        &product,
        "with_capacity",
        &arguments,
        &traits,
    ));

    assert_eq!(accepted, detached);
    assert!(accepted.equivalent_sources().is_empty());
    assert!(matches!(
        accepted.id(),
        CallableCandidateId::CapacityMethod(id)
            if id.receiver() == product.recovered() && id.arity() == 1
    ));
}

#[test]
fn associated_capacity_exact_resolver_work_limit() {
    const EXACT_WORK: u64 = 4;
    let fixture = ResolverFixture::new();
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];
    let traits = TraitCatalog::default();

    let (outcome, work) = fixture.resolve_associated_with_work_limit(
        &product,
        "with_capacity",
        &arguments,
        &traits,
        EXACT_WORK,
    );
    let candidate = resolved_candidate(outcome);
    assert!(matches!(
        candidate.id(),
        CallableCandidateId::CapacityMethod(id)
            if id.receiver() == &TypeKind::String && id.arity() == 1
    ));
    assert_eq!(work.consumed(), EXACT_WORK);
    assert_eq!(work.remaining(), 0);
    assert_eq!(work.limit(), EXACT_WORK);
    assert_eq!(work.associated_report().typed_environment_lookups(), 1);
    assert_eq!(work.associated_report().capacity_selectors(), 1);
    assert_eq!(work.associated_report().capacity_materializations(), 1);
    assert_eq!(work.associated_report().trait_resolutions(), 0);
}

#[test]
fn associated_capacity_typed_authority_compiles() {
    let parsed_callee =
        match parse_expr("Vec<i32>.with_capacity(8usize)").expect("typed associated call syntax") {
            Expr::Call(call) => call
                .parenthesized_syntax()
                .expect("parenthesized call syntax")
                .callee()
                .clone(),
            other => panic!("expected typed call expression, found {other:?}"),
        };
    assert!(matches!(
        &parsed_callee,
        ParenthesizedCalleeSyntax::PathMember(_)
    ));

    let fixture = ResolverFixture::new();
    let product = complete_builtin_product(
        TypeKind::Vec(Box::new(TypeKind::I32)),
        BuiltinTypeConstructor::Vec,
    );
    let receiver = ResolvedAssociatedTypeReceiver::try_from_product(&product)
        .expect("nominal product projects to one typed receiver");
    let member = CallableName::try_new("with_capacity").expect("associated member");
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];
    let callee = CallCallee::AssociatedType {
        receiver,
        member: &member,
        arguments: &arguments,
    };
    let module = CanonicalModulePath::crate_root();
    let accepted =
        CallResolverAuthority::accepted(&module, fixture.world.symbols(), &fixture.world);
    let detached_environment = TypeCheckEnv::standard();
    let detached = CallResolverAuthority::detached(&detached_environment);
    assert!(authority_is_accepted(accepted));
    assert!(!authority_is_accepted(detached));
    assert_typed_authority_surface(&parsed_callee, &callee, accepted);
    assert_typed_authority_surface(&parsed_callee, &callee, detached);

    let candidate =
        resolved_candidate(fixture.resolve_associated(&product, "with_capacity", &arguments));
    assert_eq!(
        exhaustive_type_receiver(candidate.instantiation()),
        Some(&TypeKind::Vec(Box::new(TypeKind::I32)))
    );
}

#[test]
fn associated_capacity_one_over_resolver_work_limit() {
    const ONE_UNDER_REQUIRED: u64 = 3;
    let fixture = ResolverFixture::new();
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];
    let traits = TraitCatalog::default();

    let (outcome, work) = fixture.resolve_associated_with_work_limit(
        &product,
        "with_capacity",
        &arguments,
        &traits,
        ONE_UNDER_REQUIRED,
    );
    assert_eq!(
        outcome,
        ResolveCallOutcome::Rejected(ResolveCallError::Work(CallableQueryLimitError::Work {
            requested: 1,
            consumed: ONE_UNDER_REQUIRED,
            limit: ONE_UNDER_REQUIRED,
        }))
    );
    assert_eq!(work.consumed(), ONE_UNDER_REQUIRED);
    assert_eq!(work.remaining(), 0);
    assert_eq!(work.limit(), ONE_UNDER_REQUIRED);
}

#[test]
fn associated_capacity_candidate_limit_does_not_partially_publish() {
    let fixture = ResolverFixture::new();
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];
    let traits = TraitCatalog::default();

    let exact_limits = associated_candidate_limits(2);
    let (exact, exact_work) = fixture.resolve_associated_with_limits(
        &product,
        "candidate_limit_probe",
        &arguments,
        &traits,
        &exact_limits,
    );
    let ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(exact)) = exact else {
        panic!("exact environment candidate boundary must publish one complete set")
    };
    assert_eq!(exact.len().get(), 2);
    assert!(exact.as_slice().iter().all(|candidate| {
        matches!(candidate.id(), CallableCandidateId::Environment(_))
            && exhaustive_type_receiver(candidate.instantiation()) == Some(&TypeKind::String)
    }));
    assert_eq!(
        exact_work.associated_report().typed_environment_lookups(),
        1
    );
    assert_eq!(exact_work.associated_report().capacity_selectors(), 0);
    assert_eq!(
        exact_work.associated_report().capacity_materializations(),
        0
    );
    assert_eq!(exact_work.associated_report().trait_resolutions(), 0);

    let one_over_limits = associated_candidate_limits(1);
    let (one_over, one_over_work) = fixture.resolve_associated_with_limits(
        &product,
        "candidate_limit_probe",
        &arguments,
        &traits,
        &one_over_limits,
    );
    assert_eq!(
        one_over,
        ResolveCallOutcome::Rejected(ResolveCallError::CandidateLimit {
            actual: 2,
            limit: 1,
        })
    );
    assert_eq!(
        one_over_work
            .associated_report()
            .typed_environment_lookups(),
        1
    );
    assert_eq!(one_over_work.associated_report().capacity_selectors(), 0);
    assert_eq!(
        one_over_work
            .associated_report()
            .capacity_materializations(),
        0
    );
    assert_eq!(one_over_work.associated_report().trait_resolutions(), 0);
}

#[test]
fn associated_typed_environment_beats_capacity() {
    let environment = TypeCheckEnv::standard().with_method_signature(
        TypeKind::String,
        "with_capacity",
        FunctionSignature::new(
            TypeKind::Bool,
            [FunctionParam::required("capacity", TypeKind::USize)],
        ),
    );
    let fixture = ResolverFixture::with_environment(
        "associated-environment-authority-parity",
        environment.clone(),
    );
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];
    let traits = TraitCatalog::default();

    let accepted = resolved_candidate(fixture.resolve_associated_with_traits(
        &product,
        "with_capacity",
        &arguments,
        &traits,
    ));
    let detached = resolved_candidate(resolve_detached_associated(
        &environment,
        &product,
        "with_capacity",
        &arguments,
        &traits,
    ));

    assert_eq!(accepted, detached);
    let CallableCandidateId::Environment(id) = accepted.id() else {
        panic!("typed environment method must stop capacity lookup")
    };
    assert_eq!(id.kind(), EnvironmentCallableKind::Method);
    assert_eq!(
        id.owner(),
        &EnvironmentCallableOwner::Standard(StandardEnvironmentId::Core)
    );
    assert_eq!(
        id.key(),
        &CallableLookupKey::Method(ReceiverMethodKey::new(
            TypeKind::String,
            CallableName::try_new("with_capacity").expect("method name"),
        ))
    );
    assert_eq!(id.overload().get(), 0);
    assert_eq!(accepted.schema().validator(), &CallableValidator::Ordinary);
    assert_eq!(accepted.schema().result(), &TypeKind::Bool);
    assert_eq!(
        exhaustive_type_receiver(accepted.instantiation()),
        Some(&TypeKind::String)
    );
    assert_eq!(accepted.authority(), Some(CallableAuthorityRank::Standard));
    assert!(matches!(
        accepted.origin(),
        SignatureOrigin::Standard {
            owner: StandardEnvironmentId::Core,
            id: origin,
        } if origin == id
    ));
}

#[test]
fn associated_untyped_method_fallback_is_ineligible() {
    let environment =
        TypeCheckEnv::standard().with_method(TypeKind::String, "with_capacity", TypeKind::Bool);
    let fixture = ResolverFixture::with_environment(
        "associated-untyped-environment-exclusion",
        environment.clone(),
    );
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let traits = TraitCatalog::default();

    let accepted = resolved_candidate(fixture.resolve_associated_with_traits(
        &product,
        "with_capacity",
        &[],
        &traits,
    ));
    let detached = resolved_candidate(resolve_detached_associated(
        &environment,
        &product,
        "with_capacity",
        &[],
        &traits,
    ));

    assert_eq!(accepted, detached);
    assert!(matches!(
        accepted.id(),
        CallableCandidateId::CapacityMethod(_)
    ));
}

#[test]
fn associated_capacity_beats_trait() {
    let fixture = ResolverFixture::new();
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];
    let traits = associated_trait_catalog();

    let capacity = resolved_candidate(fixture.resolve_associated_with_traits(
        &product,
        "with_capacity",
        &arguments,
        &traits,
    ));
    assert!(matches!(
        capacity.id(),
        CallableCandidateId::CapacityMethod(_)
    ));

    let trait_candidate = resolved_candidate(
        fixture.resolve_associated_with_traits(&product, "reserve", &arguments, &traits),
    );
    assert!(matches!(
        trait_candidate.id(),
        CallableCandidateId::TraitMethod(_)
    ));
    assert_eq!(trait_candidate.schema().result(), &TypeKind::Bool);
    assert_eq!(
        exhaustive_type_receiver(trait_candidate.instantiation()),
        Some(&TypeKind::String)
    );
}

#[test]
fn associated_typed_environment_beats_trait() {
    let environment = TypeCheckEnv::standard().with_method_signature(
        TypeKind::String,
        "reserve",
        FunctionSignature::new(
            TypeKind::I32,
            [FunctionParam::required("amount", TypeKind::USize)],
        ),
    );
    let fixture =
        ResolverFixture::with_environment("associated-environment-before-trait", environment);
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];

    let selected = resolved_candidate(fixture.resolve_associated_with_traits(
        &product,
        "reserve",
        &arguments,
        &associated_trait_catalog(),
    ));

    assert!(matches!(selected.id(), CallableCandidateId::Environment(_)));
    assert_eq!(selected.schema().result(), &TypeKind::I32);
    assert_eq!(
        exhaustive_type_receiver(selected.instantiation()),
        Some(&TypeKind::String)
    );
}

#[test]
fn associated_unique_trait_after_capacity_miss() {
    let fixture = ResolverFixture::new();
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];

    let selected = resolved_candidate(fixture.resolve_associated_with_traits(
        &product,
        "reserve",
        &arguments,
        &associated_trait_catalog(),
    ));

    assert!(matches!(selected.id(), CallableCandidateId::TraitMethod(_)));
    assert_eq!(selected.schema().result(), &TypeKind::Bool);
}

#[test]
fn associated_trait_ambiguity_is_terminal() {
    let fixture = ResolverFixture::new();
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];

    let outcome = fixture.resolve_associated_with_traits(
        &product,
        "reserve",
        &arguments,
        &ambiguous_associated_trait_catalog(),
    );

    let ResolveCallOutcome::Rejected(ResolveCallError::AmbiguousTraitMethod { candidates }) =
        outcome
    else {
        panic!("ambiguous associated trait lookup must terminate: {outcome:?}")
    };
    assert_eq!(candidates.len(), 2);
}

#[test]
fn associated_data_last_is_inapplicable() {
    let environment = TypeCheckEnv::standard().with_function_signature(
        "reserve",
        FunctionSignature::new(
            TypeKind::Bool,
            [
                FunctionParam::required("amount", TypeKind::USize),
                FunctionParam::required("receiver", TypeKind::String),
            ],
        ),
    );
    let fixture =
        ResolverFixture::with_environment("associated-data-last-inapplicable", environment);
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];

    let ResolveCallOutcome::Missing(unknown) =
        fixture.resolve_associated(&product, "reserve", &arguments)
    else {
        panic!("associated receiver must not enter the data-last family")
    };
    assert_eq!(unknown.kind(), super::UnknownCallKind::AssociatedType);
    assert_eq!(unknown.receiver(), Some(&TypeKind::String));
    assert_eq!(unknown.method().map(CallableName::as_str), Some("reserve"));
}

#[test]
fn associated_near_miss_trait_can_resolve() {
    let fixture = ResolverFixture::new();
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];

    let selected = resolved_candidate(fixture.resolve_associated_with_traits(
        &product,
        "with_capacitx",
        &arguments,
        &associated_trait_catalog(),
    ));

    assert!(matches!(selected.id(), CallableCandidateId::TraitMethod(_)));
    assert_eq!(selected.schema().result(), &TypeKind::Bool);
}

#[test]
fn capacity_near_miss_member_not_selected() {
    let fixture = ResolverFixture::new();
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let arguments = [CallArg::Positional(Box::new(Expr::Tuple(Vec::new())))];
    let member = CallableName::try_new("with_capacitx").expect("near-miss member");
    assert_eq!(
        super::CapacityMethodId::resolve_associated(&TypeKind::String, &member, 1)
            .expect("small arity fits capacity identity"),
        None
    );

    let (trait_outcome, trait_work) = fixture.resolve_associated_with_traits_and_work(
        &product,
        member.as_str(),
        &arguments,
        &associated_trait_catalog(),
    );
    assert!(matches!(
        resolved_candidate(trait_outcome).id(),
        CallableCandidateId::TraitMethod(_)
    ));
    assert_eq!(trait_work.capacity_selectors(), 1);
    assert_eq!(trait_work.capacity_materializations(), 0);
    assert_eq!(trait_work.trait_resolutions(), 1);

    let (unknown, unknown_work) = fixture.resolve_associated_with_traits_and_work(
        &product,
        member.as_str(),
        &arguments,
        &TraitCatalog::default(),
    );
    let ResolveCallOutcome::Missing(unknown) = unknown else {
        panic!("near-miss without a trait owner must remain unknown")
    };
    assert_eq!(unknown.kind(), super::UnknownCallKind::AssociatedType);
    assert_eq!(unknown.method(), Some(&member));
    assert_eq!(unknown_work.capacity_selectors(), 1);
    assert_eq!(unknown_work.capacity_materializations(), 0);
    assert_eq!(unknown_work.trait_resolutions(), 1);
}

#[test]
fn associated_near_miss_without_trait_is_unknown() {
    let fixture = ResolverFixture::new();
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);

    let ResolveCallOutcome::Missing(unknown) = fixture.resolve_associated(&product, "reserve", &[])
    else {
        panic!("near-miss associated member must remain missing")
    };
    assert_eq!(unknown.kind(), super::UnknownCallKind::AssociatedType);
    assert_eq!(unknown.receiver(), Some(&TypeKind::String));
    assert_eq!(unknown.method().map(CallableName::as_str), Some("reserve"));
}

#[test]
fn resolver_authority_rejects_crossed_source_carriers_and_detached_non_associated_queries() {
    let fixture = ResolverFixture::new();
    let environment = TypeCheckEnv::standard();
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let receiver = ResolvedAssociatedTypeReceiver::try_from_product(&product)
        .expect("complete associated receiver");
    let member = CallableName::try_new("with_capacity").expect("associated member");
    let lexical = LexicalCallableScope::default();
    let traits = TraitCatalog::default();
    let cancellation = AtomicBool::new(false);
    let module = CanonicalModulePath::crate_root();
    let source_len = fixture.document.text().len();

    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let accepted_with_detached_source = CallResolverRequest::try_new(
        CallCallee::AssociatedType {
            receiver,
            member: &member,
            arguments: &[],
        },
        CallResolverAuthority::accepted(&module, fixture.world.symbols(), &fixture.world),
        &lexical,
        None,
        &traits,
        &[],
        CallSourceContext::detached(source_len, None, None),
        CallableGroupIndex::ZERO,
        TypeExpressionId::from_index(4),
        &cancellation,
        &mut work,
        &PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(matches!(
        accepted_with_detached_source,
        Err(ResolveCallError::SourceIdentityMismatch)
    ));

    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let detached_with_accepted_source = CallResolverRequest::try_new(
        CallCallee::AssociatedType {
            receiver,
            member: &member,
            arguments: &[],
        },
        CallResolverAuthority::detached(&environment),
        &lexical,
        None,
        &traits,
        &[],
        CallSourceContext::accepted(fixture.document.identity(), None, None),
        CallableGroupIndex::ZERO,
        TypeExpressionId::from_index(5),
        &cancellation,
        &mut work,
        &PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(matches!(
        detached_with_accepted_source,
        Err(ResolveCallError::SourceIdentityMismatch)
    ));

    let path = callable_path(&["project_value"]);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let detached_free_call = CallResolverRequest::try_new(
        CallCallee::Free {
            path: &path,
            enum_variant: None,
        },
        CallResolverAuthority::detached(&environment),
        &lexical,
        None,
        &traits,
        &[],
        CallSourceContext::detached(source_len, None, None),
        CallableGroupIndex::ZERO,
        TypeExpressionId::from_index(6),
        &cancellation,
        &mut work,
        &PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(matches!(
        detached_free_call,
        Err(ResolveCallError::InvalidResolvedCallable)
    ));
}

#[test]
fn detached_resolver_rejects_out_of_bounds_source_ranges() {
    let environment = TypeCheckEnv::standard();
    let product = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let receiver = ResolvedAssociatedTypeReceiver::try_from_product(&product)
        .expect("complete associated receiver");
    let member = CallableName::try_new("with_capacity").expect("associated member");
    let lexical = LexicalCallableScope::default();
    let traits = TraitCatalog::default();
    let cancellation = AtomicBool::new(false);
    let source_len = 8;
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let request = CallResolverRequest::try_new(
        CallCallee::AssociatedType {
            receiver,
            member: &member,
            arguments: &[],
        },
        CallResolverAuthority::detached(&environment),
        &lexical,
        None,
        &traits,
        &[],
        CallSourceContext::detached(source_len, Some(TextRange::new(0, source_len + 1)), None),
        CallableGroupIndex::ZERO,
        TypeExpressionId::from_index(7),
        &cancellation,
        &mut work,
        &PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(matches!(request, Err(ResolveCallError::InvalidSourceSpan)));
}

#[test]
fn associated_receiver_projection_rejects_incomplete_and_detached_nominal_outcomes() {
    let incomplete = ResolvedTypeProduct::new(
        TypeKind::Unit,
        [ResolvedTypeNode::new(
            TypeRefNodePath::root(),
            TypeSourceEvidence::detached(TextRange::new(0, 3)),
            None,
            None,
            None,
            TypeNameResolution::Failed(TypeResolutionFailure::SelfUnavailable),
        )],
        [],
    );
    assert_eq!(
        ResolvedAssociatedTypeReceiver::try_from_product(&incomplete),
        Err(AssociatedReceiverFailure::IncompleteNode {
            node: TypeRefNodePath::root(),
        })
    );

    let complete = complete_builtin_product(TypeKind::String, BuiltinTypeConstructor::String);
    let receiver = ResolvedAssociatedTypeReceiver::try_from_product(&complete)
        .expect("complete builtin receiver");
    assert!(std::ptr::eq(
        std::ptr::from_ref(receiver.product()),
        std::ptr::from_ref(&complete),
    ));
    assert_eq!(receiver.root().node(), &TypeRefNodePath::root());
    let report = TypeResolutionReport::new(
        ResolvedTypeRefOutcome::Detached(DetachedTypeRef::new(
            complete,
            [TypeRefNodePath::root()],
            Vec::new(),
        )),
        Vec::<NominalTypeDiagnostic>::new(),
        Vec::<TypePoisonRecord>::new(),
        0,
        0,
    );
    assert_eq!(
        ResolvedAssociatedTypeReceiver::try_from_report(&report),
        Err(AssociatedReceiverFailure::DetachedOutcome)
    );
}

#[test]
fn free_resolver_returns_project_standard_and_adapter_candidates() {
    let fixture = ResolverFixture::new();

    let project = resolved_candidate(fixture.resolve("project_value"));
    assert!(matches!(
        project.origin(),
        SignatureOrigin::Project { path, .. }
            if path.path() == &callable_path(&["project_value"])
    ));
    assert!(matches!(project.id(), CallableCandidateId::Project(_)));

    let standard = resolved_candidate(fixture.resolve("standard_value"));
    assert!(matches!(
        standard.origin(),
        SignatureOrigin::Standard {
            owner: StandardEnvironmentId::Core,
            ..
        }
    ));
    assert!(matches!(standard.id(), CallableCandidateId::Environment(_)));

    let adapter = resolved_candidate(fixture.resolve("adapter_value"));
    assert!(matches!(
        adapter.origin(),
        SignatureOrigin::Adapter { package, .. } if package.as_str() == "adapter.resolver"
    ));
    assert!(matches!(adapter.id(), CallableCandidateId::Environment(_)));

    let dotted = resolved_candidate(fixture.resolve_path(&["custom", "read"]));
    assert!(matches!(
        dotted.origin(),
        SignatureOrigin::Adapter { package, .. } if package.as_str() == "adapter.resolver"
    ));
    assert!(matches!(dotted.id(), CallableCandidateId::Environment(_)));
}

#[test]
fn focused_registered_call_facts_retain_exact_source_and_checked_mapping() {
    let fixture = ResolverFixture::new();
    let call = exact_span(&fixture.document, "standard_value(2i32)");
    let module = fixture.project.linked_module();
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let report = analyze_registered_project_types_for_call_facts(
        &module,
        &fixture.world,
        call.clone(),
        &cancellation,
        &mut work,
    )
    .expect("accepted focused call source");
    assert!(
        report.report().diagnostics.is_empty(),
        "focused analysis retains the ordinary checker report"
    );
    let facts = report
        .focused_call_target_facts()
        .expect("focused call facts");

    assert_eq!(facts.call_span(), &call);
    assert_eq!(facts.document(), fixture.document.identity());
    assert_eq!(facts.result(), Some(&TypeKind::String));
    assert_eq!(facts.current_group(), CallableGroupIndex::ZERO);
    assert_eq!(facts.next_group(), None);
    assert_eq!(facts.function_value_type(), None);
    assert_eq!(facts.poison(), CallPoison::Clean);
    assert!(facts.diagnostics().is_empty());
    let CallTargetFact::Selected {
        selected,
        considered,
    } = facts.target()
    else {
        panic!("focused standard call must retain the selected resolver product")
    };
    assert!(matches!(selected.id(), CallableCandidateId::Environment(_)));
    assert_eq!(considered.as_ref(), std::slice::from_ref(selected.as_ref()));

    let [argument] = facts.arguments() else {
        panic!("standard call must retain one authored argument")
    };
    assert_eq!(argument.index().get(), 0);
    assert_eq!(argument.authored_name(), None);
    assert!(!argument.spread());
    assert_eq!(argument.poison(), CallPoison::Clean);
    let [slot] = argument.slots() else {
        panic!("ordinary argument must retain one checked slot")
    };
    assert_eq!(slot.slot().get(), 0);
    assert_eq!(
        slot.mapped().expect("mapped parameter").group(),
        CallableGroupIndex::ZERO
    );
    assert_eq!(
        slot.mapped().expect("mapped parameter").parameter().get(),
        0
    );
    assert_eq!(slot.inferred(), Some(&TypeKind::I32));
    assert_eq!(slot.expected(), Some(&TypeKind::I32));
    assert_eq!(slot.poison(), CallPoison::Clean);
    assert_eq!(
        source_text(&fixture.document, slot.source().expect("slot source")),
        "2i32"
    );
    assert!(work.consumed() > 0);
    let evaluations = report.report().physical_candidate_argument_evaluations();
    assert_eq!(evaluations.len(), 1);
    assert_eq!(evaluations[0].call_expression, facts.expression());
    assert_eq!(&evaluations[0].candidate, selected.id());
    assert_eq!(
        evaluations[0].pass,
        CandidateEvaluationPass::DirectCommitted
    );
    assert_eq!(evaluations[0].argument.get(), 0);
    assert_eq!(evaluations[0].slot.get(), 0);
    assert_eq!(
        evaluations[0].kind,
        PhysicalArgumentEvaluationKind::Authored
    );
    assert_eq!(
        evaluations[0].expected,
        CandidateExpectedType::Exact(TypeKind::I32)
    );
    assert_eq!(
        report.report().retained_argument_inference_facts().count(),
        1
    );
    assert!(
        !report
            .report()
            .physical_candidate_argument_evaluations_overflowed()
    );
}

#[test]
fn unique_overload_records_probe_and_selected_replay_separately_from_retained_facts() {
    let fixture = ResolverFixture::new();
    let call = exact_span(&fixture.document, "overloaded_value(4i32)");
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let report = analyze_registered_project_types_for_call_facts(
        &fixture.project.linked_module(),
        &fixture.world,
        call,
        &cancellation,
        &mut work,
    )
    .expect("accepted focused overload source");
    let target = report
        .focused_call_target_facts()
        .expect("focused overload facts");
    let CallTargetFact::Selected {
        selected,
        considered,
    } = target.target()
    else {
        panic!("one contextual overload must be selected")
    };
    assert_eq!(considered.len(), 2);
    assert_eq!(selected.id(), considered[0].id());

    let evaluations = report.report().physical_candidate_argument_evaluations();
    assert_eq!(evaluations.len(), 3);
    assert_eq!(
        evaluations
            .iter()
            .map(|evaluation| evaluation.pass)
            .collect::<Vec<_>>(),
        vec![
            CandidateEvaluationPass::Probe,
            CandidateEvaluationPass::Probe,
            CandidateEvaluationPass::SelectedReplay,
        ]
    );
    assert_eq!(
        evaluations
            .iter()
            .map(|evaluation| evaluation.expected.clone())
            .collect::<Vec<_>>(),
        vec![
            CandidateExpectedType::Exact(TypeKind::I32),
            CandidateExpectedType::Exact(TypeKind::String),
            CandidateExpectedType::Exact(TypeKind::I32),
        ]
    );
    assert!(evaluations.iter().all(|evaluation| {
        evaluation.call_expression == target.expression()
            && evaluation.argument.get() == 0
            && evaluation.slot.get() == 0
            && evaluation.kind == PhysicalArgumentEvaluationKind::Authored
    }));
    let retained = report
        .report()
        .retained_argument_inference_facts()
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].0, target.expression());
    assert_eq!(retained[0].1.get(), 0);
    assert_eq!(retained[0].2.slot().get(), 0);
    assert!(
        !report
            .report()
            .physical_candidate_argument_evaluations_overflowed()
    );
}

#[test]
fn singleton_rejection_records_probe_and_rejected_recovery_replay() {
    const SOURCE: &str = r#"
flow @flow.main main {
    let rejected: String = standard_value("wrong")
}
"#;
    let fixture = ResolverFixture::with_source_and_environment(
        "singleton-rejected-accounting",
        SOURCE,
        TypeCheckEnv::standard().with_function_signature(
            "standard_value",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("value", TypeKind::I32)],
            ),
        ),
    );
    let call = exact_span(&fixture.document, r#"standard_value("wrong")"#);
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let report = analyze_registered_project_types_for_call_facts(
        &fixture.project.linked_module(),
        &fixture.world,
        call,
        &cancellation,
        &mut work,
    )
    .expect("rejected focused call retains typed facts");
    let target = report
        .focused_call_target_facts()
        .expect("focused rejected facts");
    assert!(matches!(target.target(), CallTargetFact::Rejected { .. }));
    let evaluations = report.report().physical_candidate_argument_evaluations();
    assert_eq!(evaluations.len(), 2);
    assert_eq!(
        evaluations
            .iter()
            .map(|evaluation| evaluation.pass)
            .collect::<Vec<_>>(),
        vec![
            CandidateEvaluationPass::Probe,
            CandidateEvaluationPass::RejectedRecoveryReplay,
        ]
    );
    assert!(evaluations.iter().all(|evaluation| {
        evaluation.call_expression == target.expression()
            && evaluation.expected == CandidateExpectedType::Exact(TypeKind::I32)
            && evaluation.kind == PhysicalArgumentEvaluationKind::Authored
    }));
    let retained = report
        .report()
        .retained_argument_inference_facts()
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].2.inferred(), Some(&TypeKind::String));
    assert_eq!(retained[0].2.expected(), Some(&TypeKind::I32));
    assert_eq!(retained[0].2.poison(), CallPoison::Rejected);
    let checked = report.report();
    assert_eq!(checked.diagnostics.len(), 1);
    assert!(matches!(
        checked.diagnostics[0].kind(),
        TypeCheckErrorKind::ArgumentTypeMismatch {
            function,
            argument,
            expected: TypeKind::I32,
            actual: TypeKind::String,
        } if function == "standard_value" && argument == "value"
    ));
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message().contains("no viable signature"))
    );
    let slot_expression = retained[0].2.expression();
    let slot_judgments = checked
        .judgments
        .iter()
        .filter(|judgment| {
            matches!(
                &judgment.subject,
                TypeJudgmentSubject::Expr { id, .. } if *id == slot_expression
            )
        })
        .collect::<Vec<_>>();
    let [slot_judgment] = slot_judgments.as_slice() else {
        panic!("rejected recovery must retain exactly one slot judgment")
    };
    assert_eq!(slot_judgment.rule, TypeJudgmentRule::Expected);
    assert_eq!(slot_judgment.ty, TypeKind::String);
    assert_eq!(slot_judgment.expected_type(), Some(&TypeKind::I32));
    assert_eq!(checked.stats.judgments, checked.judgments.len());
    assert!(slot_expression.index() < checked.stats.expressions);
    assert!(
        !report
            .report()
            .physical_candidate_argument_evaluations_overflowed()
    );
}

#[test]
fn ambiguous_overloads_retain_only_the_stable_primary_probe() {
    let fixture = overload_recovery_accounting_fixture();
    let cancellation = AtomicBool::new(false);
    let mut ambiguous_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let ambiguous = analyze_registered_project_types_for_call_facts(
        &fixture.project.linked_module(),
        &fixture.world,
        exact_span(&fixture.document, "ambiguous_value(1i32)"),
        &cancellation,
        &mut ambiguous_work,
    )
    .expect("ambiguous focused call facts");
    let ambiguous_target = ambiguous
        .focused_call_target_facts()
        .expect("ambiguous target");
    assert!(matches!(
        ambiguous_target.target(),
        CallTargetFact::Ambiguous { .. }
    ));
    let ambiguous_evaluations = ambiguous.report().physical_candidate_argument_evaluations();
    assert_eq!(ambiguous_evaluations.len(), 2);
    assert!(ambiguous_evaluations.iter().all(|evaluation| {
        evaluation.pass == CandidateEvaluationPass::Probe
            && evaluation.expected == CandidateExpectedType::Exact(TypeKind::I32)
    }));
    assert_eq!(
        ambiguous
            .report()
            .retained_argument_inference_facts()
            .count(),
        1
    );
}

#[test]
fn multi_rejected_overloads_retain_only_the_stable_primary_probe() {
    let fixture = overload_recovery_accounting_fixture();
    let cancellation = AtomicBool::new(false);
    let mut rejected_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let rejected = analyze_registered_project_types_for_call_facts(
        &fixture.project.linked_module(),
        &fixture.world,
        exact_span(&fixture.document, "rejected_value(true)"),
        &cancellation,
        &mut rejected_work,
    )
    .expect("multi-rejected focused call facts");
    let rejected_target = rejected
        .focused_call_target_facts()
        .expect("multi-rejected target");
    assert!(matches!(
        rejected_target.target(),
        CallTargetFact::Rejected { .. }
    ));
    let rejected_evaluations = rejected.report().physical_candidate_argument_evaluations();
    assert_eq!(rejected_evaluations.len(), 2);
    assert_eq!(
        rejected_evaluations
            .iter()
            .map(|evaluation| (evaluation.pass, evaluation.expected.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(TypeKind::I32),
            ),
            (
                CandidateEvaluationPass::Probe,
                CandidateExpectedType::Exact(TypeKind::String),
            ),
        ]
    );
    let rejected_retained = rejected
        .report()
        .retained_argument_inference_facts()
        .collect::<Vec<_>>();
    assert_eq!(rejected_retained.len(), 1);
    assert_eq!(rejected_retained[0].2.inferred(), Some(&TypeKind::Bool));
    assert_eq!(rejected_retained[0].2.expected(), Some(&TypeKind::I32));
}

fn overload_recovery_accounting_fixture() -> ResolverFixture {
    const SOURCE: &str = r"
flow @flow.main main {
    let ambiguous: String = ambiguous_value(1i32)
    let rejected: String = rejected_value(true)
}
";
    let (document, project, symbol_world) =
        root_project_source("overload-recovery-accounting", SOURCE);
    let environment_document = source_document(
        "arcweft-generated://overload-recovery-accounting/adapter",
        "overload recovery adapter callables",
    );
    let owner = EnvironmentCallableOwner::Adapter(
        AdapterPackageId::try_new("adapter.overload.accounting").expect("adapter id"),
    );
    let records = [
        ("ambiguous_value", 0, TypeKind::I32),
        ("ambiguous_value", 1, TypeKind::I32),
        ("rejected_value", 0, TypeKind::I32),
        ("rejected_value", 1, TypeKind::String),
    ]
    .into_iter()
    .enumerate()
    .map(|(ordinal, (name, overload, parameter))| {
        EnvironmentCallablePublicationRecord::try_new(
            EnvironmentCallableKind::Function,
            CallableLookupKey::Free(callable_path(&[name])),
            CallableOverloadIndex::try_from_usize(overload).expect("overload"),
            ordinary_single_parameter_schema("value", parameter, TypeKind::String),
            CallableDocumentation::missing(),
            None,
            None,
            EnvironmentDeclarationOrdinal::try_from_usize(ordinal).expect("declaration ordinal"),
        )
        .expect("overload accounting record")
    })
    .collect::<Vec<_>>();
    let environment_input = source_backed_callable_input(owner, &environment_document, records);
    let facts = one_character_facts_with_environment(
        &document,
        vec![Arc::clone(&document), environment_document],
        symbol_world,
        &sample_manifest("layers/body.png"),
        vec![environment_input],
    );
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        &project,
        &facts,
        None,
    ))
    .expect("overload recovery accounting fixture");
    ResolverFixture {
        document,
        project,
        world,
    }
}

#[test]
fn typed_rest_spread_records_one_unchecked_container_evaluation() {
    const SOURCE: &str = r"
flow @flow.main main {
    let values: Vec<i32> = [1i32, 2i32]
    let result: String = typed_rest_values(values...)
}
";
    let fixture = ResolverFixture::with_source_and_environment(
        "typed-rest-accounting",
        SOURCE,
        TypeCheckEnv::standard(),
    );
    let call = exact_span(&fixture.document, "typed_rest_values(values...)");
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let report = analyze_registered_project_types_for_call_facts(
        &fixture.project.linked_module(),
        &fixture.world,
        call,
        &cancellation,
        &mut work,
    )
    .expect("typed-rest focused facts");
    let target = report
        .focused_call_target_facts()
        .expect("typed-rest target facts");
    let [evaluation] = report.report().physical_candidate_argument_evaluations() else {
        panic!("typed-rest container must be evaluated once")
    };
    assert_eq!(evaluation.call_expression, target.expression());
    assert_eq!(evaluation.pass, CandidateEvaluationPass::DirectCommitted);
    assert_eq!(
        evaluation.kind,
        PhysicalArgumentEvaluationKind::TypedRestSpread
    );
    assert_eq!(evaluation.expected, CandidateExpectedType::Unchecked);
    assert_eq!(evaluation.argument.get(), 0);
    assert_eq!(evaluation.slot.get(), 0);
    let retained = report
        .report()
        .retained_argument_inference_facts()
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 1);
    assert_eq!(
        retained[0].2.inferred(),
        Some(&TypeKind::Vec(Box::new(TypeKind::I32)))
    );
}

#[test]
fn missing_target_recovery_is_outside_candidate_physical_and_retained_accounting() {
    const SOURCE: &str = r"
flow @flow.main main {
    let non_callable: i32 = 1i32
    non_callable(2i32)
    missing_target(1i32)
}
";
    let fixture = ResolverFixture::with_source_and_environment(
        "missing-target-accounting",
        SOURCE,
        TypeCheckEnv::standard(),
    );
    let call = exact_span(&fixture.document, "missing_target(1i32)");
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let report = analyze_registered_project_types_for_call_facts(
        &fixture.project.linked_module(),
        &fixture.world,
        call,
        &cancellation,
        &mut work,
    )
    .expect("missing target focused recovery facts");
    let target = report
        .focused_call_target_facts()
        .expect("missing target fact");
    assert!(matches!(target.target(), CallTargetFact::Missing { .. }));
    assert_eq!(target.arguments().len(), 1);
    assert_eq!(target.arguments()[0].slots().len(), 1);
    assert_eq!(
        target.arguments()[0].slots()[0].inferred(),
        Some(&TypeKind::I32)
    );
    assert_eq!(target.arguments()[0].slots()[0].expected(), None);
    assert_eq!(
        target.arguments()[0].slots()[0].poison(),
        CallPoison::Rejected
    );
    assert!(
        report
            .report()
            .physical_candidate_argument_evaluations()
            .is_empty()
    );
    assert_eq!(
        report.report().retained_argument_inference_facts().count(),
        0
    );

    let mut non_callable_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let non_callable = analyze_registered_project_types_for_call_facts(
        &fixture.project.linked_module(),
        &fixture.world,
        exact_span(&fixture.document, "non_callable(2i32)"),
        &cancellation,
        &mut non_callable_work,
    )
    .expect("non-callable focused recovery facts");
    let non_callable_target = non_callable
        .focused_call_target_facts()
        .expect("non-callable target fact");
    assert!(matches!(
        non_callable_target.target(),
        CallTargetFact::NonCallable { .. }
    ));
    assert_eq!(non_callable_target.arguments().len(), 1);
    assert_eq!(non_callable_target.arguments()[0].slots().len(), 1);
    assert_eq!(
        non_callable_target.arguments()[0].slots()[0].inferred(),
        Some(&TypeKind::I32)
    );
    assert_eq!(
        non_callable_target.arguments()[0].slots()[0].expected(),
        None
    );
    assert_eq!(
        non_callable_target.arguments()[0].slots()[0].poison(),
        CallPoison::Rejected
    );
    assert!(
        non_callable
            .report()
            .physical_candidate_argument_evaluations()
            .is_empty()
    );
    assert_eq!(
        non_callable
            .report()
            .retained_argument_inference_facts()
            .count(),
        0
    );
}

#[test]
fn focused_registered_function_value_facts_retain_the_exact_callable_type() {
    const SOURCE: &str = r"
fn apply_once(f: i64 -> i64, value: i64) -> i64 {
    return f(value)
}

flow @flow.main main {
    let result: i64 = apply_once(|value: i64| -> i64 { value }, 2i64)
}
";
    let (document, project, symbol_world) =
        root_project_source("registered-function-value", SOURCE);
    let facts = one_character_facts(&document, symbol_world, &sample_manifest("layers/body.png"));
    let function_type = TypeKind::function([TypeKind::I64], TypeKind::I64);
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        &project,
        &facts,
        None,
    ))
    .expect("registered function-value fixture");
    let call = exact_span(&document, "f(value)");
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let report = analyze_registered_project_types_for_call_facts(
        &project.linked_module(),
        &world,
        call,
        &cancellation,
        &mut work,
    )
    .expect("accepted function-value call source");
    assert!(
        report.report().diagnostics.is_empty(),
        "registered function-value checking must use the shared resolver: {:?}",
        report.report().diagnostics
    );
    let target = report
        .focused_call_target_facts()
        .expect("focused function-value facts");
    let CallTargetFact::Selected {
        selected,
        considered,
    } = target.target()
    else {
        panic!("function-value call must retain its selected callable")
    };
    assert_eq!(selected.id().family(), CallableFamily::FunctionValue);
    assert_eq!(considered.as_ref(), std::slice::from_ref(selected.as_ref()));
    assert_eq!(target.function_value_type(), Some(&function_type));
    assert_eq!(target.result(), Some(&TypeKind::I64));
}

#[test]
fn registered_data_last_uses_the_shared_candidate_and_exact_checked_facts() {
    const SOURCE: &str = r"
fn above(min: i64, value: i64) -> bool {
    value > min
}

flow @flow.main main {
    let score: i64 = 90i64
    let accepted: bool = score.above(80i64)
}
";
    let (document, project, symbol_world) = root_project_source("registered-data-last", SOURCE);
    let facts = one_character_facts(&document, symbol_world, &sample_manifest("layers/body.png"));
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        &project,
        &facts,
        None,
    ))
    .expect("registered data-last fixture");
    let call = exact_span(&document, "score.above(80i64)");
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let report = analyze_registered_project_types_for_call_facts(
        &project.linked_module(),
        &world,
        call,
        &cancellation,
        &mut work,
    )
    .expect("accepted data-last call source");
    assert!(
        report.report().diagnostics.is_empty(),
        "registered data-last checking must use the shared resolver: {:?}",
        report.report().diagnostics
    );
    let target = report
        .focused_call_target_facts()
        .expect("focused data-last facts");
    let CallTargetFact::Selected {
        selected,
        considered,
    } = target.target()
    else {
        panic!("data-last fallback must retain one selected candidate")
    };
    assert_eq!(selected.id().family(), CallableFamily::DataLast);
    assert_eq!(considered.as_ref(), std::slice::from_ref(selected.as_ref()));
    assert!(matches!(
        selected.instantiation(),
        CallableInstantiation::DataLast {
            receiver: TypeKind::I64,
            group,
            parameter,
        } if group.get() == 0 && parameter.get() == 1
    ));
    assert_eq!(target.result(), Some(&TypeKind::Bool));
    let [argument] = target.arguments() else {
        panic!("data-last call must retain its one authored argument")
    };
    let [slot] = argument.slots() else {
        panic!("data-last argument must retain one checked slot")
    };
    let mapped = slot.mapped().expect("mapped data-last argument");
    assert_eq!(mapped.group().get(), 0);
    assert_eq!(mapped.parameter().get(), 0);
}

#[test]
fn registered_data_last_can_continue_into_the_next_curried_group() {
    const SOURCE: &str = r#"
fn surround(prefix: String)(value: i64)(suffix: String) -> String {
    return prefix
}

flow @flow.main main {
    let score: i64 = 90i64
    let suffixer: String -> String = score.surround("prefix")
    let result: String = suffixer("suffix")
}
"#;
    let (document, project, symbol_world) =
        root_project_source("registered-data-last-curried", SOURCE);
    let facts = one_character_facts(&document, symbol_world, &sample_manifest("layers/body.png"));
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        &project,
        &facts,
        None,
    ))
    .expect("registered curried data-last fixture");
    let call = exact_span(&document, "score.surround(\"prefix\")");
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let report = analyze_registered_project_types_for_call_facts(
        &project.linked_module(),
        &world,
        call,
        &cancellation,
        &mut work,
    )
    .expect("accepted curried data-last call source");
    assert!(
        report.report().diagnostics.is_empty(),
        "registered data-last checking must retain the next curried group: {:?}",
        report.report().diagnostics
    );
    let target = report
        .focused_call_target_facts()
        .expect("focused curried data-last facts");
    let CallTargetFact::Selected { selected, .. } = target.target() else {
        panic!("curried data-last fallback must retain its selected candidate")
    };
    assert_eq!(selected.id().family(), CallableFamily::DataLast);
    assert_eq!(
        target.result().and_then(TypeKind::function_arity),
        Some(1),
        "the receiver completes its own group and leaves the suffix group callable"
    );
}

#[test]
fn registered_trait_method_precedes_data_last_and_retains_shared_facts() {
    const SOURCE: &str = r#"
struct Score {}

trait Threshold {
    fn above(self, min: i64) -> String
}

impl Threshold for Score {
    fn above(self, min: i64) -> String {
        "trait"
    }
}

fn above(min: i64, value: Score) -> bool {
    true
}

flow @flow.main main(score: Score) {
    let accepted: String = score.above(80i64)
}
"#;
    let (document, project, symbol_world) = root_project_source("registered-trait-method", SOURCE);
    let facts = one_character_facts(&document, symbol_world, &sample_manifest("layers/body.png"));
    let base = TypeCheckEnv::standard();
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(base),
        &project,
        &facts,
        None,
    ))
    .expect("registered trait-method fixture");
    let call = exact_span(&document, "score.above(80i64)");
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let report = analyze_registered_project_types_for_call_facts(
        &project.linked_module(),
        &world,
        call,
        &cancellation,
        &mut work,
    )
    .expect("accepted trait-method call source");
    assert!(
        report.report().diagnostics.is_empty(),
        "registered trait checking must use the shared resolver: {:?}",
        report.report().diagnostics
    );
    let target = report
        .focused_call_target_facts()
        .expect("focused trait-method facts");
    let CallTargetFact::Selected { selected, .. } = target.target() else {
        panic!("trait method must retain one selected candidate")
    };
    assert_eq!(selected.id().family(), CallableFamily::TraitMethod);
    assert_eq!(target.result(), Some(&TypeKind::String));
}

#[test]
fn focused_registered_call_requires_the_exact_complete_call_span() {
    let fixture = ResolverFixture::new();
    let callee_only = exact_span(&fixture.document, "standard_value");
    let module = fixture.project.linked_module();
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let report = analyze_registered_project_types_for_call_facts(
        &module,
        &fixture.world,
        callee_only.clone(),
        &cancellation,
        &mut work,
    )
    .expect("accepted focused call source");

    assert!(matches!(
        report.focused_call_target_facts(),
        Err(CallTargetFactError::FocusedTargetMissing { call }) if call == callee_only
    ));
}

#[test]
fn focused_registered_call_uses_caller_owned_cancellation_and_work() {
    let fixture = ResolverFixture::new();
    let call = exact_span(&fixture.document, "project_value(1i32)");
    let module = fixture.project.linked_module();
    let cancellation = AtomicBool::new(true);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let report = analyze_registered_project_types_for_call_facts(
        &module,
        &fixture.world,
        call.clone(),
        &cancellation,
        &mut work,
    )
    .expect("accepted focused call source");

    assert!(matches!(
        report.focused_call_target_facts(),
        Err(CallTargetFactError::Resolve { call: actual, reason })
            if actual == call && reason.as_ref() == &ResolveCallError::Cancelled
    ));
    assert_eq!(work.consumed(), 0);

    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(0);
    let report = analyze_registered_project_types_for_call_facts(
        &module,
        &fixture.world,
        call.clone(),
        &cancellation,
        &mut work,
    )
    .expect("accepted focused call source");
    assert!(matches!(
        report.focused_call_target_facts(),
        Err(CallTargetFactError::Resolve {
            call: actual,
            reason
        }) if actual == call
            && matches!(
                reason.as_ref(),
                ResolveCallError::Work(CallableQueryLimitError::Work {
                    requested: 1,
                    consumed: 0,
                    limit: 0,
                })
            )
    ));
    assert_eq!(work.consumed(), 0);
}

#[test]
fn focused_registered_call_rejects_a_nonaccepted_source_identity() {
    let fixture = ResolverFixture::new();
    let foreign = SourceDocument::try_new(
        SourceDocumentId::try_new("foreign-call").expect("foreign id"),
        SourceName::Memory,
        "standard_value(2i32)",
    )
    .expect("foreign document");
    let call = exact_span(&foreign, "standard_value(2i32)");
    let module = fixture.project.linked_module();
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let result = analyze_registered_project_types_for_call_facts(
        &module,
        &fixture.world,
        call,
        &cancellation,
        &mut work,
    );

    assert!(matches!(
        result,
        Err(CallTargetFactError::FocusedSourceUnavailable { document })
            if document == foreign.identity().clone()
    ));
    assert_eq!(
        work.consumed(),
        0,
        "foreign source rejection must not perform semantic work"
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one fixture proves parity across capability, standard, and standalone checking"
)]
fn capability_and_standard_calls_share_registered_and_standalone_checking() {
    const SOURCE: &str = r#"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
}

flow @flow.main main effects { fs.read } {
    let text: String = fs.read_text("opening.txt")
    let display = fmt(text)
    log.info(text)
    event.emit("AppStarted", flow = @flow.main)
}
"#;
    let (document, project, symbol_world) =
        root_project_source("capability-callable-resolver", SOURCE);
    let facts = one_character_facts(&document, symbol_world, &sample_manifest("layers/body.png"));
    let base = TypeCheckEnv::standard();
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(base.clone()),
        &project,
        &facts,
        None,
    ))
    .expect("capability callable resolver fixture");
    let fixture = ResolverFixture {
        document,
        project,
        world,
    };

    let capability = resolved_candidate(fixture.resolve_path(&["fs", "read_text"]));
    assert!(matches!(
        capability.origin(),
        SignatureOrigin::Project { path, .. }
            if path.path() == &callable_path(&["fs", "read_text"])
    ));
    assert!(matches!(
        capability.id(),
        CallableCandidateId::Project(declaration)
            if declaration.owner()
                == arcweft_lang_hir::symbol::CallableDeclarationOwner::ExternCapability
    ));
    assert!(
        capability
            .schema()
            .effects()
            .project_declaration()
            .is_none(),
        "extern capabilities own fixed external effects, not a local call-graph row"
    );
    assert!(
        capability
            .schema()
            .effects()
            .declared()
            .concrete()
            .iter()
            .any(|effect| effect.as_str() == "fs.read")
    );

    let standard = resolved_candidate(fixture.resolve_path(&["log", "info"]));
    assert!(matches!(
        standard.origin(),
        SignatureOrigin::Standard {
            owner: StandardEnvironmentId::Core,
            ..
        }
    ));
    let untyped_standard = resolved_candidate(fixture.resolve_path(&["fmt"]));
    assert!(matches!(
        untyped_standard.origin(),
        SignatureOrigin::Standard {
            owner: StandardEnvironmentId::Core,
            ..
        }
    ));
    assert_eq!(untyped_standard.schema().result(), &TypeKind::DisplayText);
    assert_eq!(
        untyped_standard.schema().validator(),
        &CallableValidator::Untyped
    );
    assert_generic_untyped_schema(untyped_standard.schema());
    let event_emit = resolved_candidate(fixture.resolve_path(&["event", "emit"]));
    assert_eq!(
        event_emit.id(),
        &CallableCandidateId::Builtin(BuiltinCallableId::Capability(
            CapabilityCallableId::EventEmit
        ))
    );
    assert_eq!(
        event_emit.schema().validator(),
        &CallableValidator::Builtin(BuiltinCallableId::Capability(
            CapabilityCallableId::EventEmit
        ))
    );
    assert_eq!(event_emit.schema().result(), &TypeKind::Unit);

    let registered =
        analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    assert!(
        registered.diagnostics.is_empty(),
        "registered checking must resolve capability, standard, and unchecked capability calls: {:?}",
        registered.diagnostics
    );
    let standalone = analyze_types(&fixture.project.linked_module(), &base);
    assert!(
        standalone.diagnostics.is_empty(),
        "standalone checking must use the same unchecked capability argument policy: {:?}",
        standalone.diagnostics
    );
}

#[test]
fn untyped_calls_check_every_authored_argument_without_name_special_cases() {
    const SOURCE: &str = r"
flow @flow.main main {
    event.emit(missing_event, payload = missing_payload)
}
";
    let (document, project, symbol_world) = root_project_source("untyped-all-arguments", SOURCE);
    let facts = one_character_facts(&document, symbol_world, &sample_manifest("layers/body.png"));
    let base = TypeCheckEnv::standard();
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(base.clone()),
        &project,
        &facts,
        None,
    ))
    .expect("untyped all-arguments fixture");

    for report in [
        analyze_registered_project_types(&project.linked_module(), &world),
        analyze_types(&project.linked_module(), &base),
    ] {
        for missing in ["missing_event", "missing_payload"] {
            assert!(
                report.diagnostics.iter().any(|error| error
                    .message()
                    .contains(&format!("unknown symbol `{missing}`"))),
                "untyped checking must retain `{missing}`: {:?}",
                report.diagnostics
            );
        }
    }
}

#[test]
fn non_event_untyped_callable_accepts_open_named_and_spread_arguments() {
    const SOURCE: &str = r#"
flow @flow.main main {
    let values: Vec<i32> = [3i32, 4i32]
    custom_untyped(1i32, label = "open", [2i32]..., values..., 5i32...)
}
"#;
    let (document, project, symbol_world) = root_project_source("generic-untyped-callable", SOURCE);
    let facts = one_character_facts(&document, symbol_world, &sample_manifest("layers/body.png"));
    let base = TypeCheckEnv::standard().with_function_signature(
        "custom_untyped",
        FunctionSignature::return_only(TypeKind::Unit),
    );
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(base.clone()),
        &project,
        &facts,
        None,
    ))
    .expect("generic untyped callable fixture");
    let fixture = ResolverFixture {
        document,
        project,
        world,
    };
    let candidate = resolved_candidate(fixture.resolve("custom_untyped"));
    assert_eq!(
        candidate.schema().argument_policy(),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::OpenUnchecked,
            SpreadArgumentPolicy::Unchecked,
        )
    );

    let registered =
        analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    assert!(
        registered.diagnostics.is_empty(),
        "registered generic untyped calls accept open named and unchecked spread arguments: {:?}",
        registered.diagnostics
    );
    let standalone = analyze_types(&fixture.project.linked_module(), &base);
    assert!(
        standalone.diagnostics.is_empty(),
        "standalone generic untyped calls retain the same policy: {:?}",
        standalone.diagnostics
    );

    let call = exact_span(
        &fixture.document,
        r#"custom_untyped(1i32, label = "open", [2i32]..., values..., 5i32...)"#,
    );
    let module = fixture.project.linked_module();
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let focused = analyze_registered_project_types_for_call_facts(
        &module,
        &fixture.world,
        call,
        &cancellation,
        &mut work,
    )
    .expect("accepted focused call source");
    let facts = focused
        .focused_call_target_facts()
        .expect("focused untyped facts");
    assert_eq!(facts.arguments().len(), 5);
    assert_eq!(
        facts.arguments()[0]
            .slots()
            .first()
            .and_then(super::facts::CheckedCallArgumentSlotFact::mapped)
            .map(|coordinate| coordinate.parameter().get()),
        Some(0)
    );
    assert_eq!(
        facts.arguments()[1]
            .authored_name()
            .map(CallableName::as_str),
        Some("label")
    );
    assert_eq!(facts.arguments()[1].slots()[0].mapped(), None);
    for argument in &facts.arguments()[2..] {
        assert!(argument.spread());
        assert_eq!(
            argument.slots().len(),
            1,
            "unchecked spread retains one authored untyped slot"
        );
        assert_eq!(argument.slots()[0].mapped(), None);
        assert_eq!(argument.poison(), CallPoison::Clean);
    }
}

#[test]
fn registered_open_checked_named_arguments_still_check_their_values() {
    const SOURCE: &str = r"
flow @flow.main main {
    open_checked(extra = missing_open_checked_value)
}
";
    let (document, project, symbol_world) = root_project_source("registered-open-checked", SOURCE);
    let (environment_document, environment_input) = adapter_environment_input();
    let facts = one_character_facts_with_environment(
        &document,
        vec![Arc::clone(&document), environment_document],
        symbol_world,
        &sample_manifest("layers/body.png"),
        vec![environment_input],
    );
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        &project,
        &facts,
        None,
    ))
    .expect("registered open-checked fixture");
    let report = analyze_registered_project_types(&project.linked_module(), &world);
    assert!(
        report.diagnostics.iter().any(|error| error
            .message()
            .contains("unknown symbol `missing_open_checked_value`")),
        "OpenChecked must check the authored value: {:?}",
        report.diagnostics
    );
    assert!(
        report
            .diagnostics
            .iter()
            .all(|error| !error.message().contains("has no named parameter `extra`")),
        "OpenChecked must not reject the authored name: {:?}",
        report.diagnostics
    );
}

#[test]
fn registered_spread_policy_distinguishes_fixed_literal_and_rejected_spreads() {
    const SOURCE: &str = r"
flow @flow.main main {
    let values: Vec<i32> = [3i32, 4i32]
    let accepted: String = fixed_literal_only([1i32, 2i32]...)
    let dynamic: String = fixed_literal_only(values...)
    let rejected: String = adapter_value([5i32]...)
}
";
    let (document, project, symbol_world) = root_project_source("registered-spread-policy", SOURCE);
    let (environment_document, environment_input) = adapter_environment_input();
    let facts = one_character_facts_with_environment(
        &document,
        vec![Arc::clone(&document), environment_document],
        symbol_world,
        &sample_manifest("layers/body.png"),
        vec![environment_input],
    );
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        &project,
        &facts,
        None,
    ))
    .expect("registered spread-policy fixture");
    let report = analyze_registered_project_types(&project.linked_module(), &world);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|error| error.message().contains(
                "function `fixed_literal_only` does not accept non-literal spread arguments"
            )),
        "FixedLiteralOnly must reject dynamic spread: {:?}",
        report.diagnostics
    );
    assert!(
        report.diagnostics.iter().any(|error| error
            .message()
            .contains("function `adapter_value` does not accept spread arguments")),
        "Reject must reject fixed literal spread too: {:?}",
        report.diagnostics
    );
    assert_eq!(
        report.diagnostics.len(),
        2,
        "the accepted fixed literal spread must not add a diagnostic: {:?}",
        report.diagnostics
    );

    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let focused = analyze_registered_project_types_for_call_facts(
        &project.linked_module(),
        &world,
        exact_span(&document, "fixed_literal_only([1i32, 2i32]...)"),
        &cancellation,
        &mut work,
    )
    .expect("accepted fixed literal spread facts");
    let target = focused
        .focused_call_target_facts()
        .expect("focused fixed literal spread target");
    let evaluations = focused.report().physical_candidate_argument_evaluations();
    assert_eq!(evaluations.len(), 2);
    assert!(evaluations.iter().enumerate().all(|(slot, evaluation)| {
        evaluation.call_expression == target.expression()
            && evaluation.pass == CandidateEvaluationPass::DirectCommitted
            && evaluation.argument.get() == 0
            && evaluation.slot.get() == slot
            && evaluation.kind == PhysicalArgumentEvaluationKind::FixedLiteralSpread
            && evaluation.expected == CandidateExpectedType::Exact(TypeKind::I32)
    }));
    assert_eq!(
        focused.report().retained_argument_inference_facts().count(),
        2
    );
}

#[test]
fn selected_resolver_returns_adapter_method_candidate() {
    let fixture = ResolverFixture::new();
    let method = resolved_candidate(fixture.resolve_method(&TypeKind::I32, "run"));
    assert!(matches!(
        method.origin(),
        SignatureOrigin::Adapter { package, .. } if package.as_str() == "adapter.resolver"
    ));
    assert!(matches!(method.id(), CallableCandidateId::Environment(_)));
}

#[test]
fn project_non_callable_binding_stops_environment_fallback() {
    let fixture = ResolverFixture::new();
    for path in [&["akane"][..], &["character", "akane"][..], &["hero"][..]] {
        let ResolveCallOutcome::Resolved(ResolvedCallTarget::NonCallable(target)) =
            fixture.resolve_path(path)
        else {
            panic!("project character binding {path:?} must terminate as non-callable")
        };
        assert_eq!(
            target.ty(),
            &TypeKind::entity_ref(crate::types::EntityKind::Character)
        );
    }
}

#[test]
fn compact_project_binding_does_not_shadow_a_qualified_environment_callable() {
    let (document, project, symbol_world) =
        root_project_source("segmented-binding-shadowing", "fn main() -> Unit { () }\n");
    let generated = source_document(
        "arcweft-generated://registration-tests/compact-akane",
        "adapter.akane",
    );
    let declaration = generated
        .span(SourceRange::new(0, "adapter.akane".len()))
        .expect("environment declaration span");
    let environment = EnvironmentBindingId::try_new("adapter.akane").expect("environment id");
    let fact = external_fact(
        environment.as_str(),
        &[project_path(["akane"])],
        RegisteredExternalOwner::environment(environment.clone(), environment.clone()),
        declaration,
    );
    let record = EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::Function,
        CallableLookupKey::Free(callable_path(&["character", "akane"])),
        CallableOverloadIndex::try_from_usize(0).expect("overload"),
        ordinary_single_parameter_schema("value", TypeKind::I32, TypeKind::String),
        CallableDocumentation::missing(),
        None,
        None,
        EnvironmentDeclarationOrdinal::try_from_usize(0).expect("declaration ordinal"),
    )
    .expect("qualified environment callable");
    let environment_input = source_backed_callable_input(
        EnvironmentCallableOwner::Adapter(
            AdapterPackageId::try_new("adapter.segmented-shadowing").expect("adapter id"),
        ),
        &generated,
        [record],
    );
    let facts = ProjectRegistrationFacts::try_new(
        symbol_world,
        vec![Arc::clone(&document), generated],
        vec![fact],
        Vec::new(),
        vec![environment_input],
    )
    .expect("compact typed project binding");
    let base = TypeCheckEnv::standard().with_symbol(environment.as_str(), TypeKind::I32);
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(base),
        &project,
        &facts,
        None,
    ))
    .expect("segmented resolver fixture");
    let fixture = ResolverFixture {
        document,
        project,
        world,
    };

    assert!(matches!(
        fixture.resolve("akane"),
        ResolveCallOutcome::Resolved(ResolvedCallTarget::NonCallable(_))
    ));
    let qualified = resolved_candidate(fixture.resolve_path(&["character", "akane"]));
    assert!(matches!(
        qualified.origin(),
        SignatureOrigin::Adapter { package, .. }
            if package.as_str() == "adapter.segmented-shadowing"
    ));
}

#[test]
fn resolver_request_rejects_wrong_source_and_span() {
    let fixture = ResolverFixture::new();
    let path = callable_path(&["project_value"]);
    let lexical = LexicalCallableScope::default();
    let module = CanonicalModulePath::crate_root();
    let cancellation = AtomicBool::new(false);
    let traits = TraitCatalog::default();
    let wrong = source_document("arcweft-project://wrong.arcw", "wrong");
    let wrong_span = wrong
        .span(SourceRange::new(0, 1))
        .expect("wrong source span");
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let wrong_document = CallResolverRequest::try_new(
        CallCallee::Free {
            path: &path,
            enum_variant: None,
        },
        CallResolverAuthority::accepted(&module, fixture.world.symbols(), &fixture.world),
        &lexical,
        None,
        &traits,
        &[],
        CallSourceContext::accepted(wrong.identity(), None, None),
        CallableGroupIndex::ZERO,
        TypeExpressionId::from_index(0),
        &cancellation,
        &mut work,
        &PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(matches!(
        wrong_document,
        Err(ResolveCallError::SourceIdentityMismatch)
    ));

    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let wrong_span = CallResolverRequest::try_new(
        CallCallee::Free {
            path: &path,
            enum_variant: None,
        },
        CallResolverAuthority::accepted(&module, fixture.world.symbols(), &fixture.world),
        &lexical,
        None,
        &traits,
        &[],
        CallSourceContext::accepted(fixture.document.identity(), Some(&wrong_span), None),
        CallableGroupIndex::ZERO,
        TypeExpressionId::from_index(0),
        &cancellation,
        &mut work,
        &PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(matches!(
        wrong_span,
        Err(ResolveCallError::InvalidSourceSpan)
    ));
}

#[test]
fn resolver_cancellation_is_fail_closed() {
    let fixture = ResolverFixture::new();
    let path = callable_path(&["adapter_value"]);
    let lexical = LexicalCallableScope::default();
    let module = CanonicalModulePath::crate_root();
    let cancellation = AtomicBool::new(true);
    let traits = TraitCatalog::default();
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let request = CallResolverRequest::try_new(
        CallCallee::Free {
            path: &path,
            enum_variant: None,
        },
        CallResolverAuthority::accepted(&module, fixture.world.symbols(), &fixture.world),
        &lexical,
        None,
        &traits,
        &[],
        CallSourceContext::accepted(fixture.document.identity(), None, None),
        CallableGroupIndex::ZERO,
        TypeExpressionId::from_index(0),
        &cancellation,
        &mut work,
        &PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(matches!(request, Err(ResolveCallError::Cancelled)));

    cancellation.store(false, Ordering::Release);
}

struct CancelDuringResolverLoop<'a> {
    cancellation: &'a AtomicBool,
    polls_before_cancel: Cell<usize>,
    observed_polls: Cell<usize>,
}

impl SignatureQueryStepControl for CancelDuringResolverLoop<'_> {
    fn check_signature_query_step(&self, step: SignatureQueryStep) -> Result<(), ResolveCallError> {
        assert_eq!(step, SignatureQueryStep::Resolver);
        self.observed_polls.set(
            self.observed_polls
                .get()
                .checked_add(1)
                .expect("poll count"),
        );
        let remaining = self.polls_before_cancel.get();
        if remaining == 0 {
            self.cancellation.store(true, Ordering::Release);
        } else {
            self.polls_before_cancel.set(remaining - 1);
        }
        if self.cancellation.load(Ordering::Acquire) {
            Err(ResolveCallError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[test]
fn resolver_observes_caller_cancellation_during_its_bounded_loop() {
    let fixture = ResolverFixture::new();
    let path = callable_path(&["adapter_value"]);
    let lexical = LexicalCallableScope::default();
    let module = CanonicalModulePath::crate_root();
    let cancellation = AtomicBool::new(false);
    let control = CancelDuringResolverLoop {
        cancellation: &cancellation,
        polls_before_cancel: Cell::new(1),
        observed_polls: Cell::new(0),
    };
    let traits = TraitCatalog::default();
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let request = CallResolverRequest::try_new(
        CallCallee::Free {
            path: &path,
            enum_variant: None,
        },
        CallResolverAuthority::accepted(&module, fixture.world.symbols(), &fixture.world),
        &lexical,
        None,
        &traits,
        &[],
        CallSourceContext::accepted(fixture.document.identity(), None, None),
        CallableGroupIndex::ZERO,
        TypeExpressionId::from_index(0),
        &cancellation,
        &mut work,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("request starts before caller cancellation")
    .with_signature_control(Some(&control));

    assert_eq!(
        resolve_call_target(request),
        ResolveCallOutcome::Rejected(ResolveCallError::Cancelled)
    );
    assert!(cancellation.load(Ordering::Acquire));
    assert_eq!(control.observed_polls.get(), 2);
}

#[test]
fn corrupt_environment_candidate_sets_fail_closed_without_a_guessed_target() {
    let fixture = ResolverFixture::new();
    let cases = [
        CorruptFreeCase {
            source: &["adapter_value"],
            alternate: None,
            query: &["adapter_value"],
            reason: CorruptCallableCatalogReason::EmptySet,
        },
        CorruptFreeCase {
            source: &["adapter_value"],
            alternate: Some(&["corrupt_value"]),
            query: &["corrupt_value"],
            reason: CorruptCallableCatalogReason::KeyMismatch,
        },
        CorruptFreeCase {
            source: &["overloaded_value"],
            alternate: None,
            query: &["overloaded_value"],
            reason: CorruptCallableCatalogReason::DuplicateId,
        },
        CorruptFreeCase {
            source: &["adapter_value"],
            alternate: None,
            query: &["adapter_value"],
            reason: CorruptCallableCatalogReason::WrongAuthority,
        },
        CorruptFreeCase {
            source: &["adapter_value"],
            alternate: None,
            query: &["adapter_value"],
            reason: CorruptCallableCatalogReason::MissingRecord,
        },
        CorruptFreeCase {
            source: &["adapter_value"],
            alternate: Some(&["custom", "read"]),
            query: &["adapter_value"],
            reason: CorruptCallableCatalogReason::InvalidEquivalent,
        },
        CorruptFreeCase {
            source: &["overloaded_value"],
            alternate: None,
            query: &["overloaded_value"],
            reason: CorruptCallableCatalogReason::Unsorted,
        },
    ];

    for case in cases {
        let corrupted =
            fixture
                .clone()
                .with_corrupt_free_catalog(case.source, case.alternate, case.reason);
        assert_eq!(
            corrupted.resolve_path(case.query),
            ResolveCallOutcome::Rejected(ResolveCallError::CorruptCatalog {
                key: CallableLookupKey::Free(callable_path(case.query)),
                reason: case.reason,
            }),
            "corrupt catalog reason {:?} must be terminal",
            case.reason,
        );
    }
}

#[test]
fn resolver_request_rejects_symbols_from_another_accepted_world() {
    let fixture = ResolverFixture::new();
    let other = ResolverFixture::with_profile("callable-resolver-other-world");
    let path = callable_path(&["project_value"]);
    let lexical = LexicalCallableScope::default();
    let module = CanonicalModulePath::crate_root();
    let cancellation = AtomicBool::new(false);
    let traits = TraitCatalog::default();
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let request = CallResolverRequest::try_new(
        CallCallee::Free {
            path: &path,
            enum_variant: None,
        },
        CallResolverAuthority::accepted(&module, other.world.symbols(), &fixture.world),
        &lexical,
        None,
        &traits,
        &[],
        CallSourceContext::accepted(fixture.document.identity(), None, None),
        CallableGroupIndex::ZERO,
        TypeExpressionId::from_index(0),
        &cancellation,
        &mut work,
        &PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(matches!(request, Err(ResolveCallError::WorldMismatch)));
}

#[test]
fn resolver_zero_work_limit_rejects_before_returning_candidates() {
    let fixture = ResolverFixture::new();
    let path = callable_path(&["project_value"]);
    let lexical = LexicalCallableScope::default();
    let module = CanonicalModulePath::crate_root();
    let cancellation = AtomicBool::new(false);
    let traits = TraitCatalog::default();
    let mut work = ResolverWork::new(0);
    let request = CallResolverRequest::try_new(
        CallCallee::Free {
            path: &path,
            enum_variant: None,
        },
        CallResolverAuthority::accepted(&module, fixture.world.symbols(), &fixture.world),
        &lexical,
        None,
        &traits,
        &[],
        CallSourceContext::accepted(fixture.document.identity(), None, None),
        CallableGroupIndex::ZERO,
        TypeExpressionId::from_index(0),
        &cancellation,
        &mut work,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("zero work is validated by the resolver step");

    assert!(matches!(
        resolve_call_target(request),
        ResolveCallOutcome::Rejected(ResolveCallError::Work(CallableQueryLimitError::Work {
            requested: 1,
            consumed: 0,
            limit: 0,
        }))
    ));
    assert_eq!(work.consumed(), 0);
}

#[test]
fn registered_checker_accepts_project_standard_single_and_dotted_adapter_calls() {
    let fixture = ResolverFixture::new();
    let report = analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    assert!(
        report.diagnostics.is_empty(),
        "unexpected registered-call diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn registered_checker_keeps_local_receiver_over_same_spelled_dotted_free_candidate() {
    let fixture = ResolverFixture::new();
    let candidate = resolved_candidate(fixture.resolve_path(&["item", "len"]));
    assert!(matches!(
        candidate.origin(),
        SignatureOrigin::Adapter { package, .. } if package.as_str() == "adapter.resolver"
    ));

    let report = analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world);
    assert!(
        report.diagnostics.is_empty(),
        "local Vec receiver must retain selected-call ownership: {:?}",
        report.diagnostics
    );
}

fn resolved_candidate(outcome: ResolveCallOutcome) -> super::ResolvedCallable {
    let ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) = outcome else {
        panic!("expected resolved candidates")
    };
    assert_eq!(candidates.len().get(), 1);
    candidates.first().clone()
}

fn exact_span(document: &SourceDocument, needle: &str) -> arcweft_source::SourceSpan {
    let start = document.text().find(needle).expect("unique source needle");
    assert_eq!(
        document.text()[start + needle.len()..].find(needle),
        None,
        "source needle must identify one call"
    );
    document
        .span(SourceRange::new(start, start + needle.len()))
        .expect("exact source span")
}

fn source_text<'a>(document: &'a SourceDocument, span: &arcweft_source::SourceSpan) -> &'a str {
    &document.text()[span.range().as_range()]
}

fn callable_path(segments: &[&str]) -> CallablePath {
    CallablePath::try_new(
        segments
            .iter()
            .map(|name| CallableName::try_new(*name).expect("callable name")),
    )
    .expect("callable path")
}

fn complete_builtin_product(
    ty: TypeKind,
    constructor: BuiltinTypeConstructor,
) -> ResolvedTypeProduct {
    ResolvedTypeProduct::new(
        ty.clone(),
        [ResolvedTypeNode::new(
            TypeRefNodePath::root(),
            TypeSourceEvidence::detached(TextRange::new(0, 1)),
            None,
            None,
            Some(ty),
            TypeNameResolution::Builtin(constructor),
        )],
        [],
    )
}

fn complete_generic_vec_product() -> ResolvedTypeProduct {
    let parameter = GenericTypeParameterId::new(
        GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(73)),
        0,
    );
    let ty = TypeKind::Vec(Box::new(TypeKind::GenericParam(parameter)));
    ResolvedTypeProduct::new(
        ty.clone(),
        [ResolvedTypeNode::new(
            TypeRefNodePath::root(),
            TypeSourceEvidence::detached(TextRange::new(0, 6)),
            None,
            None,
            Some(ty),
            TypeNameResolution::Builtin(BuiltinTypeConstructor::Vec),
        )],
        [],
    )
}

fn assert_generic_untyped_schema(schema: &CallableSignatureSchema) {
    let group = schema
        .group(CallableGroupIndex::ZERO)
        .expect("untyped callable initial group");
    let [parameter] = group.parameters() else {
        panic!("untyped callable must publish one variadic parameter")
    };
    assert_eq!(parameter.name().map(CallableName::as_str), Some("args"));
    assert_eq!(parameter.ty(), &CallableParameterType::Unchecked);
    assert_eq!(
        parameter.passing(),
        CallableParameterPassing::RestPositional
    );
    assert_eq!(parameter.presence(), CallableParameterPresence::Optional);
    assert_eq!(
        schema.argument_policy(),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::OpenUnchecked,
            SpreadArgumentPolicy::Unchecked,
        )
    );
    assert_eq!(schema.validator(), &CallableValidator::Untyped);
}

fn adapter_environment_input() -> (
    Arc<SourceDocument>,
    SourceBackedEnvironmentRegistrationInput,
) {
    let owner = EnvironmentCallableOwner::Adapter(
        AdapterPackageId::try_new("adapter.resolver").expect("adapter id"),
    );
    let schema = ordinary_single_parameter_schema("value", TypeKind::I32, TypeKind::String);
    let single = EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::Function,
        CallableLookupKey::Free(callable_path(&["adapter_value"])),
        CallableOverloadIndex::try_from_usize(0).expect("overload"),
        schema.clone(),
        CallableDocumentation::missing(),
        None,
        None,
        EnvironmentDeclarationOrdinal::try_from_usize(0).expect("declaration ordinal"),
    )
    .expect("adapter record");
    let dotted = EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::Function,
        CallableLookupKey::Free(callable_path(&["custom", "read"])),
        CallableOverloadIndex::try_from_usize(0).expect("overload"),
        ordinary_single_parameter_schema("path", TypeKind::String, TypeKind::String),
        CallableDocumentation::missing(),
        None,
        None,
        EnvironmentDeclarationOrdinal::try_from_usize(1).expect("declaration ordinal"),
    )
    .expect("dotted adapter record");
    let receiver_collision = EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::Function,
        CallableLookupKey::Free(callable_path(&["item", "len"])),
        CallableOverloadIndex::try_from_usize(0).expect("overload"),
        schema,
        CallableDocumentation::missing(),
        None,
        None,
        EnvironmentDeclarationOrdinal::try_from_usize(2).expect("declaration ordinal"),
    )
    .expect("receiver collision adapter record");
    let method = EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::Method,
        CallableLookupKey::Method(ReceiverMethodKey::new(
            TypeKind::I32,
            CallableName::try_new("run").expect("method name"),
        )),
        CallableOverloadIndex::try_from_usize(0).expect("overload"),
        ordinary_single_parameter_schema("value", TypeKind::I32, TypeKind::String),
        CallableDocumentation::missing(),
        None,
        None,
        EnvironmentDeclarationOrdinal::try_from_usize(3).expect("declaration ordinal"),
    )
    .expect("adapter method record");
    let open_checked = EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::Function,
        CallableLookupKey::Free(callable_path(&["open_checked"])),
        CallableOverloadIndex::try_from_usize(0).expect("overload"),
        open_checked_schema(),
        CallableDocumentation::missing(),
        None,
        None,
        EnvironmentDeclarationOrdinal::try_from_usize(4).expect("declaration ordinal"),
    )
    .expect("open-checked adapter record");
    let fixed_literal_only = EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::Function,
        CallableLookupKey::Free(callable_path(&["fixed_literal_only"])),
        CallableOverloadIndex::try_from_usize(0).expect("overload"),
        fixed_literal_only_schema(),
        CallableDocumentation::missing(),
        None,
        None,
        EnvironmentDeclarationOrdinal::try_from_usize(5).expect("declaration ordinal"),
    )
    .expect("fixed-literal-only adapter record");
    let typed_rest = typed_rest_environment_record();
    let [overloaded_zero, overloaded_one] = overloaded_environment_records();
    let [candidate_limit_zero, candidate_limit_one] = candidate_limit_method_records();
    let document = source_document(
        "arcweft-generated://callable-resolver/adapter",
        "adapter resolver callables",
    );
    let input = source_backed_callable_input(
        owner,
        &document,
        [
            single,
            dotted,
            receiver_collision,
            method,
            open_checked,
            fixed_literal_only,
            overloaded_zero,
            overloaded_one,
            typed_rest,
            candidate_limit_zero,
            candidate_limit_one,
        ],
    );
    (document, input)
}

fn typed_rest_environment_record() -> EnvironmentCallablePublicationRecord {
    EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::Function,
        CallableLookupKey::Free(callable_path(&["typed_rest_values"])),
        CallableOverloadIndex::try_from_usize(0).expect("overload"),
        typed_rest_schema(),
        CallableDocumentation::missing(),
        None,
        None,
        EnvironmentDeclarationOrdinal::try_from_usize(8).expect("declaration ordinal"),
    )
    .expect("typed-rest adapter record")
}

fn overloaded_environment_records() -> [EnvironmentCallablePublicationRecord; 2] {
    let path = CallableLookupKey::Free(callable_path(&["overloaded_value"]));
    [
        EnvironmentCallablePublicationRecord::try_new(
            EnvironmentCallableKind::Function,
            path.clone(),
            CallableOverloadIndex::try_from_usize(0).expect("overload zero"),
            ordinary_single_parameter_schema("value", TypeKind::I32, TypeKind::String),
            CallableDocumentation::missing(),
            None,
            None,
            EnvironmentDeclarationOrdinal::try_from_usize(6).expect("declaration ordinal"),
        )
        .expect("first overloaded adapter record"),
        EnvironmentCallablePublicationRecord::try_new(
            EnvironmentCallableKind::Function,
            path,
            CallableOverloadIndex::try_from_usize(1).expect("overload one"),
            ordinary_single_parameter_schema("value", TypeKind::String, TypeKind::String),
            CallableDocumentation::missing(),
            None,
            None,
            EnvironmentDeclarationOrdinal::try_from_usize(7).expect("declaration ordinal"),
        )
        .expect("second overloaded adapter record"),
    ]
}

fn candidate_limit_method_records() -> [EnvironmentCallablePublicationRecord; 2] {
    let key = CallableLookupKey::Method(ReceiverMethodKey::new(
        TypeKind::String,
        CallableName::try_new("candidate_limit_probe").expect("method name"),
    ));
    std::array::from_fn(|overload| {
        EnvironmentCallablePublicationRecord::try_new(
            EnvironmentCallableKind::Method,
            key.clone(),
            CallableOverloadIndex::try_from_usize(overload).expect("overload index"),
            ordinary_single_parameter_schema("value", TypeKind::USize, TypeKind::String),
            CallableDocumentation::missing(),
            None,
            None,
            EnvironmentDeclarationOrdinal::try_from_usize(9 + overload)
                .expect("declaration ordinal"),
        )
        .expect("candidate-limit method record")
    })
}

fn associated_candidate_limits(max_candidates_per_call: usize) -> CallableLimits {
    CallableLimits::for_test(
        32,
        16,
        128,
        32,
        max_candidates_per_call,
        256,
        128,
        PRODUCTION_CALLABLE_LIMITS.max_catalog_build_work(),
        PRODUCTION_CALLABLE_LIMITS.max_query_work(),
    )
}

fn ordinary_single_parameter_schema(
    name: &str,
    parameter_type: TypeKind,
    result: TypeKind,
) -> CallableSignatureSchema {
    let parameter = CallableParameter::try_new(
        CallableParameterIndex::try_from_usize(0).expect("parameter index"),
        Some(CallableName::try_new(name).expect("parameter name")),
        CallableParameterType::Exact(parameter_type),
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Required,
        None,
        None,
    )
    .expect("parameter");
    CallableSignatureSchema::try_new(
        vec![
            CallableParameterGroup::try_new(
                CallableGroupIndex::ZERO,
                CallableGroupKind::Initial,
                vec![parameter],
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .expect("parameter group"),
        ],
        result,
        CallableEffectSchema::fixed(EffectRow::closed(crate::effects::EffectSet::new())),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("adapter schema")
}

fn open_checked_schema() -> CallableSignatureSchema {
    CallableSignatureSchema::try_new(
        vec![
            CallableParameterGroup::try_new(
                CallableGroupIndex::ZERO,
                CallableGroupKind::Initial,
                Vec::new(),
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .expect("empty open-checked group"),
        ],
        TypeKind::Unit,
        CallableEffectSchema::fixed(EffectRow::closed(crate::effects::EffectSet::new())),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::OpenChecked,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("open-checked schema")
}

fn fixed_literal_only_schema() -> CallableSignatureSchema {
    let parameter = CallableParameter::try_new(
        CallableParameterIndex::try_from_usize(0).expect("parameter index"),
        Some(CallableName::try_new("values").expect("parameter name")),
        CallableParameterType::Exact(TypeKind::I32),
        CallableParameterPassing::RestPositional,
        CallableParameterPresence::Optional,
        None,
        None,
    )
    .expect("fixed-literal-only parameter");
    CallableSignatureSchema::try_new(
        vec![
            CallableParameterGroup::try_new(
                CallableGroupIndex::ZERO,
                CallableGroupKind::Initial,
                vec![parameter],
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .expect("fixed-literal-only group"),
        ],
        TypeKind::String,
        CallableEffectSchema::fixed(EffectRow::closed(crate::effects::EffectSet::new())),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::FixedLiteralOnly,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("fixed-literal-only schema")
}

fn typed_rest_schema() -> CallableSignatureSchema {
    let parameter = CallableParameter::try_new(
        CallableParameterIndex::try_from_usize(0).expect("parameter index"),
        Some(CallableName::try_new("values").expect("parameter name")),
        CallableParameterType::Exact(TypeKind::I32),
        CallableParameterPassing::RestPositional,
        CallableParameterPresence::Optional,
        None,
        None,
    )
    .expect("typed-rest parameter");
    CallableSignatureSchema::try_new(
        vec![
            CallableParameterGroup::try_new(
                CallableGroupIndex::ZERO,
                CallableGroupKind::Initial,
                vec![parameter],
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .expect("typed-rest group"),
        ],
        TypeKind::String,
        CallableEffectSchema::fixed(EffectRow::closed(crate::effects::EffectSet::new())),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::TypedRest,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("typed-rest schema")
}
