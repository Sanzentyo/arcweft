use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use arcweft_lang_hir::project::HirProject;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use crate::{
    checker::{
        TypeExpressionId, analyze_registered_project_types, analyze_types,
        module::analyze_registered_project_types_for_call_facts,
    },
    effect_row::EffectRow,
    env::{
        FunctionParam, FunctionSignature, TypeCheckEnv,
        identity::EnvironmentBindingId,
        nominal::{
            AcceptedNominalId, AcceptedNominalOrigin, AcceptedNominalOwnerId,
            AcceptedNominalRecord, AcceptedNominalSemantics, RustPackageId,
        },
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
    types::{AcceptedNominalType, TypeKind},
};

use super::facts::CallTargetFact;
use super::{
    AdapterPackageId, BuiltinCallableId, CallCallee, CallPoison, CallResolverRequest,
    CallSourceContext, CallTargetFactError, CallableArgumentPolicy, CallableCandidateId,
    CallableDocumentation, CallableEffectSchema, CallableFamily, CallableGroupIndex,
    CallableGroupKind, CallableInstantiation, CallableLookupKey, CallableName,
    CallableOverloadIndex, CallableParameter, CallableParameterGroup, CallableParameterIndex,
    CallableParameterPassing, CallableParameterPresence, CallableParameterType, CallablePath,
    CallableQueryLimitError, CallableSignatureSchema, CallableValidator, CapabilityCallableId,
    EnvironmentCallableKind, EnvironmentCallableOwner, EnvironmentCallablePublicationRecord,
    EnvironmentDeclarationOrdinal, LexicalCallableScope, PRODUCTION_CALLABLE_LIMITS,
    ReceiverMethodKey, ResolveCallError, ResolveCallOutcome, ResolvedCallTarget, ResolverWork,
    RustCallableProvenance, RustCallablePurity, RustItemPath, RustPackageProvenance,
    SignatureOrigin, SignatureQueryStep, SignatureQueryStepControl, SpreadArgumentPolicy,
    StandardEnvironmentId, UnknownNamedArgumentPolicy, resolve_call_target,
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
        let (environment_document, environment_input) = adapter_environment_input();
        let facts = one_character_facts_with_environment(
            &document,
            vec![Arc::clone(&document), environment_document],
            world,
            &sample_manifest("layers/body.png"),
            vec![environment_input],
        );
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
            &lexical,
            None,
            &module,
            self.world.symbols(),
            &self.world,
            &traits,
            &[],
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
                arguments: &[],
            },
            &lexical,
            None,
            &module,
            self.world.symbols(),
            &self.world,
            &traits,
            &[],
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
    let rank_path: arcweft_lang_syntax::types::TypePath = project_path(["Rank"]).into();
    let base = TypeCheckEnv::standard()
        .try_with_nominal_record(
            AcceptedNominalRecord::try_new(
                AcceptedNominalId::new(
                    AcceptedNominalOwnerId::RustPackage(
                        RustPackageId::try_new("truck_game").expect("package id"),
                    ),
                    rank_path.clone(),
                ),
                0,
                AcceptedNominalSemantics::Opaque,
                AcceptedNominalOrigin::RustExport,
                None,
            )
            .expect("accepted Rust nominal"),
        )
        .expect("Rust type export");
    let rank = base
        .nominal_catalog()
        .exact(&rank_path)
        .expect("typed Rust rank export");
    let rank_type = TypeKind::AcceptedNominal(AcceptedNominalType::new(
        rank.id().clone(),
        Box::<[TypeKind]>::default(),
    ));
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
        ordinary_single_parameter_schema("score", TypeKind::I32, rank_type),
        CallableDocumentation::missing(),
        None,
        Some(rust),
        EnvironmentDeclarationOrdinal::try_from_usize(0).expect("declaration ordinal"),
    )
    .expect("Rust publication record");
    let environment_document = source_document(
        "arcweft-generated://callable-resolver/rust-alias",
        "rust score_to_rank callable",
    );
    let environment_input = source_backed_callable_input(
        EnvironmentCallableOwner::Adapter(adapter),
        &environment_document,
        [record],
    );
    let facts = one_character_facts_with_environment(
        &document,
        vec![Arc::clone(&document), environment_document],
        symbol_world,
        &sample_manifest("layers/body.png"),
        vec![environment_input],
    );
    let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(base),
        &project,
        &facts,
        None,
    ))
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
        &lexical,
        None,
        &module,
        fixture.world.symbols(),
        &fixture.world,
        &traits,
        &[],
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
        CallCallee::Free {
            path: &path,
            enum_variant: None,
        },
        &lexical,
        None,
        &module,
        fixture.world.symbols(),
        &fixture.world,
        &traits,
        &[],
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
        CallCallee::Free {
            path: &path,
            enum_variant: None,
        },
        &lexical,
        None,
        &module,
        fixture.world.symbols(),
        &fixture.world,
        &traits,
        &[],
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
            self.cancellation.store(true, Ordering::Relaxed);
        } else {
            self.polls_before_cancel.set(remaining - 1);
        }
        if self.cancellation.load(Ordering::Relaxed) {
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
        &lexical,
        None,
        &module,
        fixture.world.symbols(),
        &fixture.world,
        &traits,
        &[],
        CallSourceContext::new(fixture.document.identity(), None, None),
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
    assert!(cancellation.load(Ordering::Relaxed));
    assert_eq!(control.observed_polls.get(), 2);
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
        &lexical,
        None,
        &module,
        other.world.symbols(),
        &fixture.world,
        &traits,
        &[],
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
        CallCallee::Free {
            path: &path,
            enum_variant: None,
        },
        &lexical,
        None,
        &module,
        fixture.world.symbols(),
        &fixture.world,
        &traits,
        &[],
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
        ],
    );
    (document, input)
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
