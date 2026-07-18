use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use arcweft_lang_hir::project::HirProject;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::{SourceDocument, SourceRange};

use crate::{
    checker::{TypeExpressionId, analyze_registered_project_types, analyze_types},
    effect_row::EffectRow,
    env::{FunctionParam, FunctionSignature, TypeCheckEnv},
    registration::{
        CharacterRegistrar, CharacterRegistrationRequest, EnvironmentBindingId,
        ProjectRegistrationFacts, RegisteredExternalOwner, RegisteredSemanticWorld,
    },
    test_support::character_project::{
        external_fact, one_character_facts, project_path, root_project_source, sample_manifest,
        source_document,
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
    ReceiverMethodKey, ResolveCallError, ResolveCallOutcome, ResolvedCallTarget, ResolverWork,
    RustCallableProvenance, RustCallablePurity, RustItemPath, RustPackageProvenance,
    SignatureOrigin, SpreadArgumentPolicy, StandardEnvironmentId, UnknownNamedArgumentPolicy,
    resolve_call_target,
};

const SOURCE: &str = r#"
use character.akane as hero

fn project_value(value: i32) -> String {
    "project"
}

flow @flow.main main {
    let project: String = project_value(1i32)
    let standard: String = standard_value(2i32)
    let adapter: String = adapter_value(3i32)
    let dotted: String = custom.read(path = "opening.txt")
    let inferred: String = infer.run(value = 4i32)
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
            .with_symbol("infer", TypeKind::Named("InferApi".to_owned()))
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
            },
            &lexical,
            None,
            &module,
            self.world.symbols(),
            &self.world,
            &traits,
            CallSourceContext::new(self.document.identity(), None, None),
            CallableGroupIndex::ZERO,
            TypeExpressionId::from_index(1),
            &cancellation,
            &mut work,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("selected resolver request");
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
fn capability_and_standard_calls_share_registered_and_standalone_checking() {
    const SOURCE: &str = r#"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
}

flow @flow.main main effects { fs.read } {
    let text: String = fs.read_text("opening.txt")
    let display = fmt(text)
    log.info(text)
    event.emit(AppStarted, flow = @flow.main)
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
fn extern_rust_alias_resolves_exact_typed_environment_record() {
    const SOURCE: &str = r#"
extern rust mod mini_games.truck from crate "truck_game" {
    pub type Rank
    pub fn score_to_rank(score: i32) -> Rank
}

flow @flow.main main {
    let rank: Rank = mini_games.truck.score_to_rank(score = 42i32)
}
"#;
    let (document, project, symbol_world) = root_project_source("rust-extern-alias", SOURCE);
    let facts = one_character_facts(&document, symbol_world, &sample_manifest("layers/body.png"));
    let adapter = AdapterPackageId::try_new("adapter.rust").expect("adapter id");
    let rust = RustCallableProvenance::try_new(
        adapter.clone(),
        RustPackageProvenance::try_new("truck_game", "1.0.0", None)
            .expect("Rust package provenance"),
        RustItemPath::try_new("truck_game::score_to_rank").expect("Rust item path"),
        RustCallablePurity::Pure,
    )
    .expect("Rust callable provenance");
    let record = EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::RustFunction,
        CallableLookupKey::Free(callable_path(&["score_to_rank"])),
        CallableOverloadIndex::try_from_usize(0).expect("overload"),
        ordinary_single_parameter_schema(
            "score",
            TypeKind::I32,
            TypeKind::Named("Rank".to_owned()),
        ),
        CallableDocumentation::missing(),
        None,
        Some(rust),
        EnvironmentDeclarationOrdinal::try_from_usize(0).expect("declaration ordinal"),
    )
    .expect("Rust publication record");
    let publication = EnvironmentCallablePublication::try_new(
        EnvironmentCallableOwner::Adapter(adapter),
        vec![record],
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("Rust callable publication");
    let base = TypeCheckEnv::standard().with_rust_type_export("truck_game", "Rank");
    let world = CharacterRegistrar::register(
        CharacterRegistrationRequest::new(Arc::new(base), &project, &facts, None)
            .with_callable_publication(publication),
    )
    .expect("registered Rust alias fixture");
    let fixture = ResolverFixture {
        document,
        project,
        world,
    };

    let candidate =
        resolved_candidate(fixture.resolve_path(&["mini_games", "truck", "score_to_rank"]));
    assert!(matches!(
        candidate.origin(),
        SignatureOrigin::Adapter { .. }
    ));
    analyze_registered_project_types(&fixture.project.linked_module(), &fixture.world)
        .into_result()
        .expect("extern Rust alias typechecks through the accepted catalog");
}

#[test]
fn selected_resolver_returns_adapter_method_candidate() {
    let fixture = ResolverFixture::new();
    let method =
        resolved_candidate(fixture.resolve_method(&TypeKind::Named("InferApi".to_owned()), "run"));
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
        RegisteredExternalOwner::Environment(environment.clone()),
        declaration,
    );
    let facts = ProjectRegistrationFacts::try_new(
        symbol_world,
        vec![Arc::clone(&document), generated],
        vec![fact],
        Vec::new(),
    )
    .expect("compact typed project binding");
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
    let publication = EnvironmentCallablePublication::try_new(
        EnvironmentCallableOwner::Adapter(
            AdapterPackageId::try_new("adapter.segmented-shadowing").expect("adapter id"),
        ),
        vec![record],
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("qualified environment publication");
    let base = TypeCheckEnv::standard().with_symbol(environment.as_str(), TypeKind::I32);
    let world = CharacterRegistrar::register(
        CharacterRegistrationRequest::new(Arc::new(base), &project, &facts, None)
            .with_callable_publication(publication),
    )
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
    let method = EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::Method,
        CallableLookupKey::Method(ReceiverMethodKey::new(
            TypeKind::Named("InferApi".to_owned()),
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
    EnvironmentCallablePublication::try_new(
        owner,
        vec![single, dotted, receiver_collision, method],
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
