use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use arcweft_lang_hir::project::HirProject;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::{SourceDocument, SourceRange};

use crate::{
    checker::{TypeExpressionId, analyze_registered_project_types},
    effect_row::EffectRow,
    env::{FunctionParam, FunctionSignature, TypeCheckEnv},
    registration::{CharacterRegistrar, CharacterRegistrationRequest, RegisteredSemanticWorld},
    test_support::character_project::{
        one_character_facts, root_project_source, sample_manifest, source_document,
    },
    traits::TraitCatalog,
    types::TypeKind,
};

use super::{
    AdapterPackageId, CallCallee, CallResolverRequest, CallSourceContext, CallableArgumentPolicy,
    CallableCandidateId, CallableDocumentation, CallableEffectSchema, CallableGroupIndex,
    CallableGroupKind, CallableLookupKey, CallableName, CallableOverloadIndex, CallableParameter,
    CallableParameterGroup, CallableParameterIndex, CallableParameterPassing,
    CallableParameterPresence, CallableParameterType, CallablePath, CallableQueryLimitError,
    CallableSignatureSchema, CallableValidator, EnvironmentCallableKind, EnvironmentCallableOwner,
    EnvironmentCallablePublication, EnvironmentCallablePublicationRecord,
    EnvironmentDeclarationOrdinal, LexicalCallableScope, PRODUCTION_CALLABLE_LIMITS,
    ResolveCallError, ResolveCallOutcome, ResolvedCallTarget, ResolverWork, SignatureOrigin,
    SpreadArgumentPolicy, StandardEnvironmentId, UnknownNamedArgumentPolicy, resolve_call_target,
};

const SOURCE: &str = r#"
fn project_value(value: i32) -> String {
    "project"
}

flow @flow.main main {
    let project: String = project_value(1i32)
    let standard: String = standard_value(2i32)
    let adapter: String = adapter_value(3i32)
    let dotted: String = custom.read(path = "opening.txt")
    let item: Vec<i32> = [1i32, 2i32]
    let item_len: usize = item.len()
}
"#;

struct ResolverFixture {
    document: Arc<SourceDocument>,
    project: HirProject,
    world: RegisteredSemanticWorld,
}

impl ResolverFixture {
    fn new() -> Self {
        Self::with_profile("callable-resolver")
    }

    fn with_profile(profile: &str) -> Self {
        let (document, project, world) = root_project_source(profile, SOURCE);
        let facts = one_character_facts(&document, world, &sample_manifest("layers/body.png"));
        let base = TypeCheckEnv::standard()
            .with_function_signature(
                "standard_value",
                FunctionSignature::new(
                    TypeKind::String,
                    [FunctionParam::required("value", TypeKind::I32)],
                ),
            )
            .with_function_signature("akane", FunctionSignature::new(TypeKind::Unit, []));
        let world = CharacterRegistrar::register(
            CharacterRegistrationRequest::new(Arc::new(base), &project, &facts, None)
                .with_callable_publication(adapter_publication()),
        )
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
            CallCallee::Free { path: &path },
            &lexical,
            None,
            &module,
            self.world.symbols(),
            &self.world,
            &traits,
            CallSourceContext::new(self.document.identity(), None, None),
            CallableGroupIndex::ZERO,
            TypeExpressionId::from_index(0),
            &cancellation,
            &mut work,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("resolver request");
        resolve_call_target(request)
    }
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
fn project_non_callable_binding_stops_environment_fallback() {
    let fixture = ResolverFixture::new();
    let ResolveCallOutcome::Resolved(ResolvedCallTarget::NonCallable(target)) =
        fixture.resolve("akane")
    else {
        panic!("project character alias must terminate as non-callable")
    };
    assert_eq!(
        target.ty(),
        &TypeKind::entity_ref(crate::types::EntityKind::Character)
    );
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
        CallCallee::Free { path: &path },
        &lexical,
        None,
        &module,
        fixture.world.symbols(),
        &fixture.world,
        &traits,
        CallSourceContext::new(wrong.identity(), None, None),
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
        CallCallee::Free { path: &path },
        &lexical,
        None,
        &module,
        fixture.world.symbols(),
        &fixture.world,
        &traits,
        CallSourceContext::new(fixture.document.identity(), Some(&wrong_span), None),
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
        CallCallee::Free { path: &path },
        &lexical,
        None,
        &module,
        fixture.world.symbols(),
        &fixture.world,
        &traits,
        CallSourceContext::new(fixture.document.identity(), None, None),
        CallableGroupIndex::ZERO,
        TypeExpressionId::from_index(0),
        &cancellation,
        &mut work,
        &PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(matches!(request, Err(ResolveCallError::Cancelled)));

    cancellation.store(false, Ordering::Relaxed);
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
        CallCallee::Free { path: &path },
        &lexical,
        None,
        &module,
        other.world.symbols(),
        &fixture.world,
        &traits,
        CallSourceContext::new(fixture.document.identity(), None, None),
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
        CallCallee::Free { path: &path },
        &lexical,
        None,
        &module,
        fixture.world.symbols(),
        &fixture.world,
        &traits,
        CallSourceContext::new(fixture.document.identity(), None, None),
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

fn callable_path(segments: &[&str]) -> CallablePath {
    CallablePath::try_new(
        segments
            .iter()
            .map(|name| CallableName::try_new(*name).expect("callable name")),
    )
    .expect("callable path")
}

fn adapter_publication() -> EnvironmentCallablePublication {
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
    EnvironmentCallablePublication::try_new(
        owner,
        vec![single, dotted, receiver_collision],
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("adapter publication")
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
