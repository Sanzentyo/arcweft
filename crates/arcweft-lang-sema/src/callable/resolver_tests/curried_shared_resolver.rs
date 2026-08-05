use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use arcweft_lang_hir::project::HirProject;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::SourceDocument;

use crate::{
    callable::{
        AdapterPackageId, CallCallee, CallResolverAuthority, CallResolverRequest,
        CallSourceContext, CallableArgumentPolicy, CallableCandidateId, CallableDocumentation,
        CallableEffectSchema, CallableGroupIndex, CallableGroupKind, CallableInstantiation,
        CallableLookupKey, CallableName, CallableOverloadIndex, CallableParameter,
        CallableParameterGroup, CallableParameterIndex, CallableParameterPassing,
        CallableParameterPresence, CallableParameterType, CallablePath, CallableSignatureSchema,
        CallableValidator, EnvironmentCallableKind, EnvironmentCallableOwner,
        EnvironmentCallablePublicationRecord, EnvironmentDeclarationOrdinal, FunctionValueOrdinal,
        FunctionValueSignatureId, LexicalCallableScope, PRODUCTION_CALLABLE_LIMITS,
        ResolveCallError, ResolveCallOutcome, ResolvedCallTarget, ResolvedCallable,
        ResolvedFunctionValueSeed, ResolverWork, SignatureOrigin, SignatureQueryStep,
        SignatureQueryStepControl, SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
        resolve_call_target,
    },
    checker::{TypeExpressionId, module::analyze_registered_project_types_for_call_facts},
    effect_row::EffectRow,
    env::{FunctionParam, FunctionSignature, TypeCheckEnv},
    registration::{
        CharacterRegistrar, CharacterRegistrationRequest, RegisteredSemanticWorld,
        SourceBackedEnvironmentRegistrationInput,
    },
    test_support::{
        character_project::{
            one_character_facts_with_environment, root_project_source, sample_manifest,
            source_document,
        },
        environment::source_backed_callable_input,
    },
    traits::TraitCatalog,
    types::TypeKind,
};

use super::{CallTargetFact, exact_span};

const SOURCE: &str = r#"
fn project_curried(value: i32)(suffix: String) -> String {
    return suffix
}

flow main {
    let project_next: String -> String = project_curried(1i32)
    let project_result: String = project_next("project")
}
"#;

#[derive(Clone, Copy)]
enum ProviderFamily {
    Project,
    Standard,
    Adapter,
}

struct CurriedResolverFixture {
    document: Arc<SourceDocument>,
    project: HirProject,
    world: RegisteredSemanticWorld,
}

impl CurriedResolverFixture {
    fn new(profile: &str, reverse_adapter_records: bool) -> Self {
        let (document, project, symbol_world) = root_project_source(profile, SOURCE);
        let (adapter_document, adapter_input) = adapter_input(reverse_adapter_records);
        let facts = one_character_facts_with_environment(
            &document,
            vec![Arc::clone(&document), adapter_document],
            symbol_world,
            &sample_manifest("layers/body.png"),
            vec![adapter_input],
        );
        let standard_signature = FunctionSignature::new(
            TypeKind::function([TypeKind::String], TypeKind::String),
            [FunctionParam::required("value", TypeKind::I32)],
        )
        .with_remaining_param_groups([[FunctionParam::required("suffix", TypeKind::String)]]);
        let environment = TypeCheckEnv::standard()
            .with_function_signature("standard_curried", standard_signature);
        let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
            Arc::new(environment),
            &project,
            &facts,
            None,
        ))
        .expect("registered curried resolver fixture");
        Self {
            document,
            project,
            world,
        }
    }

    fn resolve_base(&self, family: ProviderFamily) -> ResolvedCallable {
        let path = match family {
            ProviderFamily::Project => path(&["project_curried"]),
            ProviderFamily::Standard => path(&["standard_curried"]),
            ProviderFamily::Adapter => path(&["adapter_curried"]),
        };
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
        .expect("accepted base resolver request");
        let ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) =
            resolve_call_target(request)
        else {
            panic!("provider base must resolve through the shared resolver")
        };
        candidates
            .as_slice()
            .iter()
            .find(|candidate| match (family, candidate.id()) {
                (ProviderFamily::Project, CallableCandidateId::Project(_)) => true,
                (ProviderFamily::Standard, CallableCandidateId::Environment(environment)) => {
                    matches!(environment.owner(), EnvironmentCallableOwner::Standard(_))
                }
                (ProviderFamily::Adapter, CallableCandidateId::Environment(environment)) => {
                    matches!(
                        environment.owner(),
                        EnvironmentCallableOwner::Adapter(package)
                            if package.as_str() == "adapter.curried"
                    ) && environment.overload().get() == 0
                }
                _ => false,
            })
            .cloned()
            .expect("requested provider candidate")
    }

    fn resolve_continuation(
        &self,
        base: ResolvedCallable,
        seed_schema: CallableSignatureSchema,
        group: CallableGroupIndex,
        cancellation: &AtomicBool,
        work: &mut ResolverWork,
        control: Option<&dyn SignatureQueryStepControl>,
    ) -> ResolveCallOutcome {
        let seed = ResolvedFunctionValueSeed::new(
            FunctionValueSignatureId::new(
                TypeExpressionId::from_index(40),
                FunctionValueOrdinal::try_from_usize(0).expect("function value ordinal"),
            ),
            TypeKind::function([TypeKind::String], TypeKind::String),
            seed_schema,
            None,
            Some(base),
            group,
        );
        let lexical = LexicalCallableScope::default();
        let module = CanonicalModulePath::crate_root();
        let traits = TraitCatalog::default();
        let request = CallResolverRequest::try_new(
            CallCallee::FunctionValue { value: &seed },
            CallResolverAuthority::accepted(&module, self.world.symbols(), &self.world),
            &lexical,
            None,
            &traits,
            &[],
            CallSourceContext::accepted(self.document.identity(), None, None),
            group,
            TypeExpressionId::from_index(41),
            cancellation,
            work,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("accepted continuation resolver request")
        .with_signature_control(control);
        resolve_call_target(request)
    }
}

struct ResolverStepCounter {
    observed: Cell<usize>,
}

impl ResolverStepCounter {
    const fn new() -> Self {
        Self {
            observed: Cell::new(0),
        }
    }

    fn observed(&self) -> usize {
        self.observed.get()
    }
}

impl SignatureQueryStepControl for ResolverStepCounter {
    fn check_signature_query_step(&self, step: SignatureQueryStep) -> Result<(), ResolveCallError> {
        assert_eq!(step, SignatureQueryStep::Resolver);
        self.observed.set(
            self.observed
                .get()
                .checked_add(1)
                .expect("resolver step counter"),
        );
        Ok(())
    }
}

struct CancelOnSecondResolverStep<'a> {
    cancellation: &'a AtomicBool,
    observed: Cell<usize>,
}

impl SignatureQueryStepControl for CancelOnSecondResolverStep<'_> {
    fn check_signature_query_step(&self, step: SignatureQueryStep) -> Result<(), ResolveCallError> {
        assert_eq!(step, SignatureQueryStep::Resolver);
        let observed = self
            .observed
            .get()
            .checked_add(1)
            .expect("resolver cancellation step counter");
        self.observed.set(observed);
        if observed == 2 {
            self.cancellation.store(true, Ordering::Relaxed);
            return Err(ResolveCallError::Cancelled);
        }
        Ok(())
    }
}

#[test]
fn shared_resolver_rejects_project_curried_one_over() {
    assert_one_over(ProviderFamily::Project);
}

#[test]
fn shared_resolver_rejects_standard_curried_one_over() {
    assert_one_over(ProviderFamily::Standard);
}

#[test]
fn shared_resolver_rejects_adapter_curried_one_over() {
    assert_one_over(ProviderFamily::Adapter);
}

#[test]
fn shared_resolver_publishes_exact_curried_schema_group() {
    let fixture = CurriedResolverFixture::new("curried-positive", false);
    let base = fixture.resolve_base(ProviderFamily::Project);
    let base_id = base.id().clone();
    let base_origin = base.origin().clone();
    let base_authority = base.authority();
    let group = group(1);
    let expected_group = base.schema().group(group).expect("project group 1");
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let outcome = fixture.resolve_continuation(
        base.clone(),
        base.schema().clone(),
        group,
        &cancellation,
        &mut work,
        None,
    );
    let candidate = only_candidate(&outcome);
    let CallableCandidateId::Curried(curried) = candidate.id() else {
        panic!("continuation must publish the canonical Curried ID")
    };
    assert_eq!(curried.base(), &base_id);
    assert_eq!(curried.next_group(), group);
    assert_eq!(
        candidate.instantiation(),
        &CallableInstantiation::Curried {
            base: base_id,
            group,
        }
    );
    assert_eq!(candidate.origin(), &base_origin);
    assert_eq!(candidate.authority(), base_authority);
    assert_eq!(candidate.schema().groups().len(), 2);
    assert!(std::ptr::eq(base.schema(), candidate.schema()));
    assert!(std::ptr::eq(
        expected_group,
        candidate
            .schema()
            .group(group)
            .expect("published exact group 1")
    ));
}

#[test]
fn shared_resolver_rejects_initial_curried_group() {
    let fixture = CurriedResolverFixture::new("curried-group-zero", false);
    let base = fixture.resolve_base(ProviderFamily::Project);
    let base_id = base.id().clone();
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let outcome = fixture.resolve_continuation(
        base.clone(),
        base.schema().clone(),
        CallableGroupIndex::ZERO,
        &cancellation,
        &mut work,
        None,
    );
    assert_invalid_group(&outcome, base_id, CallableGroupIndex::ZERO);
}

#[test]
fn shared_resolver_corrupt_world_has_no_fallback() {
    let fixture = CurriedResolverFixture::new("curried-corrupt-world", false);
    let published = fixture.resolve_base(ProviderFamily::Project);
    let base_id = published.id().clone();
    let group = group(1);
    let corrupt_base = ResolvedCallable::try_new(
        base_id.clone(),
        published.origin().clone(),
        Arc::new(single_group_schema()),
        CallableInstantiation::None,
        published.equivalent_sources().to_vec(),
        published.authority(),
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("typed corrupt-world base omits only the continuation group");
    assert!(corrupt_base.schema().group(group).is_none());
    assert!(
        published.schema().group(group).is_some(),
        "the seed schema provides an alternate success shape that must not be used"
    );
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let outcome = fixture.resolve_continuation(
        corrupt_base,
        published.schema().clone(),
        group,
        &cancellation,
        &mut work,
        None,
    );
    assert_invalid_group(&outcome, base_id, group);
}

#[test]
fn shared_resolver_curried_candidate_matches_checker_target_fact() {
    let fixture = CurriedResolverFixture::new("curried-checker-target", false);
    let base = fixture.resolve_base(ProviderFamily::Project);
    let group = group(1);
    let cancellation = AtomicBool::new(false);
    let mut resolver_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let expected = fixture.resolve_continuation(
        base.clone(),
        base.schema().clone(),
        group,
        &cancellation,
        &mut resolver_work,
        None,
    );
    let expected = only_candidate(&expected);
    let call = exact_span(&fixture.document, "project_next(\"project\")");
    let mut checker_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let focused = analyze_registered_project_types_for_call_facts(
        &fixture.project.linked_module(),
        &fixture.world,
        call,
        &cancellation,
        &mut checker_work,
    )
    .expect("accepted focused curried call");
    assert!(
        focused.report().diagnostics.is_empty(),
        "curried shared-resolver diagnostics: {:?}",
        focused.report().diagnostics
    );
    let facts = focused
        .focused_call_target_facts()
        .expect("focused curried checker facts");
    let CallTargetFact::Selected {
        selected,
        considered,
    } = facts.target()
    else {
        panic!("curried checker target must be selected")
    };
    assert_eq!(selected.id(), expected.id());
    assert_eq!(selected.instantiation(), expected.instantiation());
    assert_eq!(considered.as_ref(), std::slice::from_ref(selected.as_ref()));
    assert_eq!(facts.current_group(), group);
    assert_eq!(facts.result(), Some(&TypeKind::String));
    assert_eq!(focused.report().stats.registered_call_expressions, 2);
    assert_eq!(focused.report().stats.shared_resolver_invocations, 2);
    assert_eq!(focused.report().stats.old_dispatch_calls, 0);
}

#[test]
fn shared_resolver_curried_result_is_insertion_order_invariant() {
    let forward = CurriedResolverFixture::new("curried-order", false);
    let reversed = CurriedResolverFixture::new("curried-order", true);
    let forward_base = forward.resolve_base(ProviderFamily::Adapter);
    let reversed_base = reversed.resolve_base(ProviderFamily::Adapter);
    assert_eq!(forward_base, reversed_base);
    let group = group(1);
    let cancellation = AtomicBool::new(false);
    let mut forward_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let mut reversed_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let forward_outcome = forward.resolve_continuation(
        forward_base.clone(),
        forward_base.schema().clone(),
        group,
        &cancellation,
        &mut forward_work,
        None,
    );
    let reversed_outcome = reversed.resolve_continuation(
        reversed_base.clone(),
        reversed_base.schema().clone(),
        group,
        &cancellation,
        &mut reversed_work,
        None,
    );
    assert_eq!(forward_outcome, reversed_outcome);
    let candidate = only_candidate(&forward_outcome);
    assert!(matches!(candidate.id(), CallableCandidateId::Curried(_)));
    assert!(candidate.schema().group(group).is_some());
    assert!(matches!(
        candidate.origin(),
        SignatureOrigin::Adapter { .. }
    ));
    assert_eq!(candidate.authority(), forward_base.authority());
}

#[test]
fn curried_group_error_does_not_resolve_base_candidate() {
    let fixture = CurriedResolverFixture::new("curried-no-base-fallback", false);
    let base = fixture.resolve_base(ProviderFamily::Project);
    let base_id = base.id().clone();
    let missing = one_over(base.schema());
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let outcome = fixture.resolve_continuation(
        base.clone(),
        base.schema().clone(),
        missing,
        &cancellation,
        &mut work,
        None,
    );
    assert_invalid_group(&outcome, base_id, missing);
}

#[test]
fn curried_group_error_does_not_retry_old_resolver() {
    let fixture = CurriedResolverFixture::new("curried-no-old-retry", false);
    let base = fixture.resolve_base(ProviderFamily::Project);
    let base_id = base.id().clone();
    let missing = one_over(base.schema());
    let counter = ResolverStepCounter::new();
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());

    let outcome = fixture.resolve_continuation(
        base.clone(),
        base.schema().clone(),
        missing,
        &cancellation,
        &mut work,
        Some(&counter),
    );
    assert_invalid_group(&outcome, base_id, missing);
    assert_eq!(counter.observed(), 2);
    assert_eq!(work.consumed(), 2);

    let call = exact_span(&fixture.document, "project_next(\"project\")");
    let mut checker_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let focused = analyze_registered_project_types_for_call_facts(
        &fixture.project.linked_module(),
        &fixture.world,
        call,
        &cancellation,
        &mut checker_work,
    )
    .expect("accepted curried dispatcher instrumentation");
    assert_eq!(focused.report().stats.registered_call_expressions, 2);
    assert_eq!(focused.report().stats.shared_resolver_invocations, 2);
    assert_eq!(focused.report().stats.old_dispatch_calls, 0);
}

#[test]
fn curried_group_validation_preserves_accepted_world_guard() {
    let fixture = CurriedResolverFixture::new("curried-world-guard", false);
    let other = CurriedResolverFixture::new("curried-world-guard-other", false);
    let base = fixture.resolve_base(ProviderFamily::Project);
    let group = group(1);
    let seed = ResolvedFunctionValueSeed::new(
        FunctionValueSignatureId::new(
            TypeExpressionId::from_index(70),
            FunctionValueOrdinal::try_from_usize(0).expect("function value ordinal"),
        ),
        TypeKind::function([TypeKind::String], TypeKind::String),
        base.schema().clone(),
        None,
        Some(base),
        group,
    );
    let lexical = LexicalCallableScope::default();
    let module = CanonicalModulePath::crate_root();
    let traits = TraitCatalog::default();
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let request = CallResolverRequest::try_new(
        CallCallee::FunctionValue { value: &seed },
        CallResolverAuthority::accepted(&module, other.world.symbols(), &fixture.world),
        &lexical,
        None,
        &traits,
        &[],
        CallSourceContext::accepted(fixture.document.identity(), None, None),
        group,
        TypeExpressionId::from_index(71),
        &cancellation,
        &mut work,
        &PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(matches!(request, Err(ResolveCallError::WorldMismatch)));
    assert_eq!(work.consumed(), 0);
}

#[test]
fn curried_group_validation_preserves_cancellation() {
    let fixture = CurriedResolverFixture::new("curried-cancellation", false);
    let base = fixture.resolve_base(ProviderFamily::Project);
    let cancellation = AtomicBool::new(false);
    let control = CancelOnSecondResolverStep {
        cancellation: &cancellation,
        observed: Cell::new(0),
    };
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let outcome = fixture.resolve_continuation(
        base.clone(),
        base.schema().clone(),
        group(1),
        &cancellation,
        &mut work,
        Some(&control),
    );
    assert_eq!(
        outcome,
        ResolveCallOutcome::Rejected(ResolveCallError::Cancelled)
    );
    assert!(cancellation.load(Ordering::Relaxed));
    assert_eq!(control.observed.get(), 2);
    assert_eq!(work.consumed(), 1);
}

#[test]
fn curried_group_validation_charges_existing_work_budget() {
    let fixture = CurriedResolverFixture::new("curried-work", false);
    let base = fixture.resolve_base(ProviderFamily::Project);
    let base_id = base.id().clone();
    let missing = one_over(base.schema());
    let cancellation = AtomicBool::new(false);
    let counter = ResolverStepCounter::new();
    let mut exact_work = ResolverWork::new(2);
    let outcome = fixture.resolve_continuation(
        base.clone(),
        base.schema().clone(),
        missing,
        &cancellation,
        &mut exact_work,
        Some(&counter),
    );
    assert_invalid_group(&outcome, base_id, missing);
    assert_eq!(counter.observed(), 2);
    assert_eq!(exact_work.consumed(), 2);

    let mut one_under = ResolverWork::new(1);
    let outcome = fixture.resolve_continuation(
        base.clone(),
        base.schema().clone(),
        missing,
        &cancellation,
        &mut one_under,
        None,
    );
    assert_eq!(
        outcome,
        ResolveCallOutcome::Rejected(ResolveCallError::Work(
            crate::callable::CallableQueryLimitError::Work {
                requested: 1,
                consumed: 1,
                limit: 1,
            }
        ))
    );
    assert_eq!(one_under.consumed(), 1);
}

fn assert_one_over(family: ProviderFamily) {
    let fixture = CurriedResolverFixture::new("curried-provider-one-over", false);
    let base = fixture.resolve_base(family);
    let base_id = base.id().clone();
    let missing = one_over(base.schema());
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let outcome = fixture.resolve_continuation(
        base.clone(),
        base.schema().clone(),
        missing,
        &cancellation,
        &mut work,
        None,
    );
    assert_invalid_group(&outcome, base_id, missing);
}

fn assert_invalid_group(
    outcome: &ResolveCallOutcome,
    base: CallableCandidateId,
    group: CallableGroupIndex,
) {
    assert_eq!(
        outcome,
        &ResolveCallOutcome::Rejected(ResolveCallError::InvalidCallGroup {
            candidate: Box::new(base),
            group,
        })
    );
}

fn only_candidate(outcome: &ResolveCallOutcome) -> &ResolvedCallable {
    let ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) = outcome else {
        panic!("expected one resolved continuation candidate: {outcome:?}")
    };
    assert_eq!(candidates.len().get(), 1);
    candidates.first()
}

fn one_over(schema: &CallableSignatureSchema) -> CallableGroupIndex {
    let one_over = CallableGroupIndex::try_from_usize(schema.groups().len())
        .expect("one-over group is representable");
    assert!(schema.group(one_over).is_none());
    one_over
}

fn group(index: usize) -> CallableGroupIndex {
    CallableGroupIndex::try_from_usize(index).expect("test group index")
}

fn path(segments: &[&str]) -> CallablePath {
    CallablePath::try_new(
        segments
            .iter()
            .map(|segment| CallableName::try_new(*segment).expect("callable segment")),
    )
    .expect("callable path")
}

fn adapter_input(
    reverse_records: bool,
) -> (
    Arc<SourceDocument>,
    SourceBackedEnvironmentRegistrationInput,
) {
    let owner = EnvironmentCallableOwner::Adapter(
        AdapterPackageId::try_new("adapter.curried").expect("adapter package"),
    );
    let key = CallableLookupKey::Free(path(&["adapter_curried"]));
    let mut records = vec![
        adapter_record(key.clone(), 0, TypeKind::I32),
        adapter_record(key, 1, TypeKind::I64),
    ];
    if reverse_records {
        records.reverse();
    }
    let document = source_document(
        "arcweft-generated://callable-resolver/curried-adapter",
        "curried adapter publication",
    );
    let input = source_backed_callable_input(owner, &document, records);
    (document, input)
}

fn adapter_record(
    key: CallableLookupKey,
    overload: usize,
    initial_type: TypeKind,
) -> EnvironmentCallablePublicationRecord {
    EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::Function,
        key,
        CallableOverloadIndex::try_from_usize(overload).expect("adapter overload"),
        multi_group_schema(initial_type),
        CallableDocumentation::missing(),
        None,
        None,
        EnvironmentDeclarationOrdinal::try_from_usize(overload)
            .expect("adapter declaration ordinal"),
    )
    .expect("curried adapter record")
}

fn multi_group_schema(initial_type: TypeKind) -> CallableSignatureSchema {
    CallableSignatureSchema::try_new(
        vec![
            parameter_group(0, CallableGroupKind::Initial, "value", initial_type),
            parameter_group(1, CallableGroupKind::Curried, "suffix", TypeKind::String),
        ],
        TypeKind::String,
        CallableEffectSchema::fixed(EffectRow::closed(crate::effects::EffectSet::new())),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("multi-group callable schema")
}

fn single_group_schema() -> CallableSignatureSchema {
    CallableSignatureSchema::try_new(
        vec![parameter_group(
            0,
            CallableGroupKind::Initial,
            "value",
            TypeKind::I32,
        )],
        TypeKind::String,
        CallableEffectSchema::fixed(EffectRow::closed(crate::effects::EffectSet::new())),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("single-group callable schema")
}

fn parameter_group(
    group_index: usize,
    kind: CallableGroupKind,
    parameter_name: &str,
    parameter_type: TypeKind,
) -> CallableParameterGroup {
    let parameter = CallableParameter::try_new(
        CallableParameterIndex::try_from_usize(0).expect("parameter index"),
        Some(CallableName::try_new(parameter_name).expect("parameter name")),
        CallableParameterType::Exact(parameter_type),
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Required,
        None,
        None,
    )
    .expect("curried parameter");
    CallableParameterGroup::try_new(
        group(group_index),
        kind,
        vec![parameter],
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("curried parameter group")
}
