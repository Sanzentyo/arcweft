#![allow(
    clippy::result_large_err,
    reason = "test helpers assert the complete public typed query error without erasing evidence"
)]

use std::{cell::Cell, sync::Arc, sync::atomic::AtomicBool, time::Instant};

use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_syntax::{ast::module_path::CanonicalModulePath, parser::parse_source};
use arcweft_source::SourceDocument;

use crate::{
    callable::{
        AdapterPackageId, CallableArgumentPolicy, CallableCandidateId, CallableDiagnostic,
        CallableDiagnosticCode, CallableDiagnosticSubject, CallableDocumentation,
        CallableEffectSchema, CallableFamily, CallableGroupIndex, CallableGroupKind,
        CallableLookupKey, CallableName, CallableOverloadIndex, CallableParameter,
        CallableParameterGroup, CallableParameterIndex, CallableParameterPassing,
        CallableParameterPresence, CallableParameterType, CallablePath, CallableSignatureSchema,
        CallableValidator, EnvironmentCallableKind, EnvironmentCallableOwner,
        EnvironmentCallablePublicationRecord, EnvironmentDeclarationOrdinal,
        PRODUCTION_CALLABLE_LIMITS, PRODUCTION_SIGNATURE_LIMITS, ResolverWork,
        SemanticSignatureIndex, SignatureOrigin, SignatureQueryLimits, SignatureQueryStep,
        SignatureQueryWorkMeter, SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
    },
    checker::module::{
        SignatureFocusedAnalysis, analyze_registered_project_types_for_signature_call,
    },
    effect_row::EffectRow,
    effects::EffectSet,
    env::{FunctionParam, FunctionSignature, TypeCheckEnv},
    registration::{CharacterRegistrar, CharacterRegistrationRequest, RegisteredSemanticWorld},
    test_support::character_project::{
        PACKAGE, one_character_facts, one_character_facts_with_environment, register,
        root_project_source, sample_manifest, source_document,
    },
    test_support::environment::source_backed_callable_input,
    types::TypeKind,
};

use super::{
    SemanticSignatureHelp, SignatureFamilySupport, SignaturePositionError, SignatureQuery,
    SignatureQueryControl, SignatureQueryError, SignatureQueryOutcome, SignatureRecovery,
    SignatureSemanticStale, query_signature, signature_family_support,
};

const SOURCE: &str = r#"
fn project_int(value: i32) -> i32 {
    value
}

fn curry(value: i32)(suffix: String) -> String {
    suffix
}

fn main() -> Unit {
    let early: String = standard_value(0i32)
    let nested: String = standard_value(project_int(1i32))
    let curried: String = curry(2i32)("ok")
    ()
}
"#;

struct SignatureFixture {
    document: std::sync::Arc<SourceDocument>,
    project: HirProject,
    world: RegisteredSemanticWorld,
}

struct TestPublication {
    owner: EnvironmentCallableOwner,
    records: Vec<EnvironmentCallablePublicationRecord>,
}

impl SignatureFixture {
    fn new(source: &str) -> Self {
        let (document, project, world_id) = root_project_source("signature-query", source);
        let facts = one_character_facts(&document, world_id, &sample_manifest("layers/body.png"));
        let environment = TypeCheckEnv::standard().with_function_signature(
            "standard_value",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("value", TypeKind::I32)],
            ),
        );
        let world =
            register(&project, &facts, environment, None).expect("signature fixture registers");
        Self {
            document,
            project,
            world,
        }
    }

    fn with_publication(source: &str, publication: TestPublication) -> Self {
        Self::with_environment_and_publication(source, TypeCheckEnv::standard(), publication)
    }

    fn with_environment_and_publication(
        source: &str,
        environment: TypeCheckEnv,
        publication: TestPublication,
    ) -> Self {
        let (document, project, world_id) =
            root_project_source("signature-query-publication", source);
        let environment_document = source_document(
            "arcweft-generated://signature-query/publication",
            "signature query environment publication",
        );
        let environment_input = source_backed_callable_input(
            publication.owner,
            &environment_document,
            publication.records,
        );
        let facts = one_character_facts_with_environment(
            &document,
            vec![Arc::clone(&document), environment_document],
            world_id,
            &sample_manifest("layers/body.png"),
            vec![environment_input],
        );
        let world = CharacterRegistrar::register(CharacterRegistrationRequest::new(
            Arc::new(environment),
            &project,
            &facts,
            None,
        ))
        .expect("signature publication fixture registers");
        Self {
            document,
            project,
            world,
        }
    }

    fn recovered(source: &str) -> Self {
        let document = source_document(
            "arcweft-project://registration-tests/src/recovered-signature.arcw",
            source,
        );
        let parsed = parse_source(source);
        assert!(
            !parsed.errors().is_empty(),
            "recovery fixture must retain a parser diagnostic"
        );
        let hir = lower_document_to_hir(&document, parsed.typed_tree())
            .expect("recovered source still lowers to typed HIR");
        let project = HirProject::new(
            PACKAGE,
            [HirProjectModule::try_new(
                CanonicalModulePath::crate_root(),
                document.identity().clone(),
                hir,
            )
            .expect("recovered module binding")],
        )
        .expect("recovered HIR project");
        let world_id = ProjectSymbolWorldId::try_new(
            CallablePackageId::try_new(PACKAGE).expect("package"),
            document.identity().id().clone(),
            "signature-query-recovery",
        )
        .expect("recovered world id");
        let facts = one_character_facts(&document, world_id, &sample_manifest("layers/body.png"));
        let world = register(&project, &facts, TypeCheckEnv::standard(), None)
            .expect("recovered fixture registers");
        Self {
            document,
            project,
            world,
        }
    }

    fn query(&self, byte_offset: usize) -> Result<SignatureQueryOutcome, SignatureQueryError> {
        let cancelled = AtomicBool::new(false);
        let hir = self.project.linked_module();
        query_signature(
            SignatureQuery::production(
                &self.world,
                &self.document,
                &hir,
                byte_offset,
                SignatureQueryControl::new(&cancelled, None),
            )
            .expect("fixture keeps one accepted document/HIR/world lease"),
        )
    }

    fn query_with_control(
        &self,
        byte_offset: usize,
        control: SignatureQueryControl<'_>,
    ) -> Result<SignatureQueryOutcome, SignatureQueryError> {
        let hir = self.project.linked_module();
        query_signature(SignatureQuery::production(
            &self.world,
            &self.document,
            &hir,
            byte_offset,
            control,
        )?)
    }

    fn query_with_limits(
        &self,
        byte_offset: usize,
        limits: SignatureQueryLimits,
    ) -> Result<SignatureQueryOutcome, SignatureQueryError> {
        let cancelled = AtomicBool::new(false);
        let hir = self.project.linked_module();
        query_signature(SignatureQuery::try_new(
            &self.world,
            &self.document,
            &hir,
            byte_offset,
            limits,
            SignatureQueryControl::new(&cancelled, None),
        )?)
    }

    fn query_in(
        &self,
        unique_call: &str,
        cursor_needle: &str,
    ) -> Result<SignatureQueryOutcome, SignatureQueryError> {
        let call_start = unique_offset(self.document.text(), unique_call);
        let relative = unique_call
            .find(cursor_needle)
            .expect("cursor needle belongs to call");
        self.query(call_start + relative)
    }
}

#[test]
fn native_query_projects_project_and_environment_signatures() {
    let fixture = SignatureFixture::new(SOURCE);

    let SignatureQueryOutcome::Help(project) = fixture
        .query_in("project_int(1i32)", "1i32")
        .expect("project signature query")
    else {
        panic!("project call must produce signature help")
    };
    let project_active = project.active_signature();
    assert!(matches!(
        project.signatures()[project_active.get()].candidate(),
        CallableCandidateId::Project(_)
    ));
    assert_eq!(project.current_group(), CallableGroupIndex::ZERO);
    assert_eq!(
        project
            .active_parameter()
            .expect("mapped project parameter")
            .parameter(),
        CallableParameterIndex::try_from_usize(0).expect("parameter zero")
    );

    let SignatureQueryOutcome::Help(environment) = fixture
        .query_in("standard_value(0i32)", "0i32")
        .expect("environment signature query")
    else {
        panic!("environment call must produce signature help")
    };
    let environment_active = environment.active_signature();
    assert!(matches!(
        environment.signatures()[environment_active.get()].candidate(),
        CallableCandidateId::Environment(_)
    ));
}

#[test]
fn native_query_uses_the_shared_builtin_candidate() {
    let fixture = SignatureFixture::new(
        r"
fn main() -> Unit {
    let value: f32 = sin(1.0f32)
    ()
}
",
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("sin(1.0f32)", "1.0f32")
        .expect("builtin signature query")
    else {
        panic!("builtin call must produce signature help")
    };
    let selected = &help.signatures()[help.active_signature().get()];
    assert_eq!(
        selected.candidate(),
        &CallableCandidateId::Builtin(crate::callable::BuiltinCallableId::Sin)
    );
    assert_eq!(
        help.active_parameter()
            .expect("builtin argument maps to parameter zero")
            .parameter(),
        CallableParameterIndex::try_from_usize(0).expect("parameter zero")
    );
}

#[test]
fn authored_alias_labels_preserve_source_spelling_and_document_canonical_owner() {
    let fixture = SignatureFixture::new(
        r"
pub fn canonical(value: i32) -> i32 {
    value
}

use crate.canonical as alias

fn main() -> Unit {
    let value: i32 = alias(1i32)
    ()
}
        ",
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("alias(1i32)", "1i32")
        .expect("project alias signature query")
    else {
        panic!("accepted project alias must produce signature help")
    };
    let selected = help
        .signatures()
        .get(help.active_signature().get())
        .expect("project alias has one selected signature");

    assert_eq!(selected.authored_callee(), "alias");
    assert_eq!(selected.canonical_callee(), "canonical");
    assert!(
        selected
            .documentation()
            .details()
            .is_some_and(|details| details.contains("Canonical owner: `canonical`."))
    );
}

#[test]
fn nested_cursor_selects_the_innermost_parenthesized_call() {
    let fixture = SignatureFixture::new(SOURCE);
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("standard_value(project_int(1i32))", "1i32")
        .expect("nested signature query")
    else {
        panic!("nested cursor must select one call")
    };
    let active = help.active_signature();

    assert!(matches!(
        help.signatures()[active.get()].candidate(),
        CallableCandidateId::Project(_)
    ));
    assert_eq!(
        source_text(&fixture.document, help.call_span()),
        "project_int(1i32)"
    );
    assert_eq!(
        help.signatures()[active.get()].authored_callee(),
        "project_int"
    );
}

#[test]
fn adjacent_and_utf8_calls_keep_their_exact_parser_owned_carriers() {
    let fixture = SignatureFixture::new(
        r#"
fn project_int(value: i32) -> i32 {
    value
}

fn project_string(value: String) -> String {
    value
}

fn main() -> Unit {
    let adjacent: i32 = project_int(1i32) + project_int(2i32)
    let unicode: String = project_string("あ")
    ()
}
"#,
    );

    let SignatureQueryOutcome::Help(adjacent) = fixture
        .query_in("project_int(2i32)", "2i32")
        .expect("second adjacent call query")
    else {
        panic!("the second adjacent call must retain its own carrier")
    };
    assert_eq!(
        source_text(&fixture.document, adjacent.call_span()),
        "project_int(2i32)"
    );

    let unicode_call = "project_string(\"あ\")";
    let unicode_start = unique_offset(fixture.document.text(), unicode_call);
    let after_unicode =
        unicode_start + unicode_call.find('あ').expect("unicode argument") + 'あ'.len_utf8();
    let SignatureQueryOutcome::Help(unicode) =
        fixture.query(after_unicode).expect("UTF-8 boundary query")
    else {
        panic!("a valid UTF-8 boundary must select the typed call carrier")
    };
    assert_eq!(
        source_text(&fixture.document, unicode.call_span()),
        unicode_call
    );
    let selected = unicode
        .signatures()
        .get(unicode.active_signature().get())
        .expect("UTF-8 call has one selected signature");
    assert_eq!(selected.authored_callee(), "project_string");
}

#[test]
fn late_target_does_not_inherit_work_from_earlier_calls() {
    let late = SignatureFixture::new(
        r"
fn project_int(value: i32) -> i32 {
    value
}

fn earlier() -> String {
    standard_value(project_int(0i32))
}

fn main() -> Unit {
    let value: i32 = project_int(1i32)
    ()
}
",
    );
    let minimal = SignatureFixture::new(
        r#"
fn project_int(value: i32) -> i32 {
    value
}

fn earlier() -> String {
    "no earlier calls"
}

fn main() -> Unit {
    let value: i32 = project_int(1i32)
    ()
}
"#,
    );

    let SignatureQueryOutcome::Help(late) = late
        .query_in("project_int(1i32)", "1i32")
        .expect("late signature query")
    else {
        panic!("late target must produce signature help")
    };
    let SignatureQueryOutcome::Help(minimal) = minimal
        .query_in("project_int(1i32)", "1i32")
        .expect("minimal signature query")
    else {
        panic!("minimal target must produce signature help")
    };

    assert_eq!(
        late.work(),
        minimal.work(),
        "only the selected call may consume caller-owned query work"
    );
}

#[test]
fn curried_first_group_reports_current_next_and_active_parameter() {
    let fixture = SignatureFixture::new(SOURCE);
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("curry(2i32)(\"ok\")", "2i32")
        .expect("curried signature query")
    else {
        panic!("curried first group must produce signature help")
    };

    assert_eq!(help.current_group(), CallableGroupIndex::ZERO);
    assert_eq!(
        help.next_group(),
        Some(CallableGroupIndex::try_from_usize(1).expect("group one"))
    );
    assert_eq!(
        help.active_parameter()
            .expect("first curried parameter")
            .parameter(),
        CallableParameterIndex::try_from_usize(0).expect("parameter zero")
    );

    let later = fixture
        .query_in("curry(2i32)(\"ok\")", "\"ok\"")
        .expect("later curried signature query");
    let SignatureQueryOutcome::Help(help) = later else {
        panic!("curried later group must produce signature help: {later:?}")
    };
    assert_eq!(
        help.current_group(),
        CallableGroupIndex::try_from_usize(1).expect("group one")
    );
    assert_eq!(help.next_group(), None);
    assert_eq!(
        help.active_parameter()
            .expect("later curried parameter")
            .parameter(),
        CallableParameterIndex::try_from_usize(0).expect("parameter zero")
    );
}

#[test]
fn query_control_and_positions_fail_before_semantic_projection() {
    let fixture = SignatureFixture::new(SOURCE);
    let hir = fixture.project.linked_module();
    let cancelled = AtomicBool::new(false);
    assert!(matches!(
        SignatureQuery::production(
            &fixture.world,
            &fixture.document,
            &hir,
            fixture.document.text().len() + 1,
            SignatureQueryControl::new(&cancelled, None),
        ),
        Err(SignatureQueryError::InvalidPosition(
            SignaturePositionError::OutOfBounds { .. }
        ))
    ));

    let cancelled = AtomicBool::new(true);
    let query = SignatureQuery::production(
        &fixture.world,
        &fixture.document,
        &hir,
        unique_offset(fixture.document.text(), "1i32"),
        SignatureQueryControl::new(&cancelled, None),
    )
    .expect("cancelled request still has a valid accepted lease");
    assert_eq!(query_signature(query), Err(SignatureQueryError::Cancelled));

    let cancelled = AtomicBool::new(false);
    let query = SignatureQuery::production(
        &fixture.world,
        &fixture.document,
        &hir,
        unique_offset(fixture.document.text(), "1i32"),
        SignatureQueryControl::new(&cancelled, Some(Instant::now())),
    )
    .expect("expired request still has a valid accepted lease");
    assert_eq!(
        query_signature(query),
        Err(SignatureQueryError::DeadlineExceeded)
    );

    let cancelled = AtomicBool::new(false);
    let remaining_steps = Cell::new(1);
    let control =
        SignatureQueryControl::new(&cancelled, None).with_remaining_steps(&remaining_steps);
    assert_eq!(
        fixture.query_with_control(unique_offset(fixture.document.text(), "1i32"), control),
        Err(SignatureQueryError::DeadlineExceeded),
        "the preflight succeeds and the first focused resolver step expires"
    );
}

#[test]
fn deadline_during_candidate_probe_or_selected_replay_returns_no_partial_help() {
    let ambiguous = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = ambiguous_value(1i32)
    ()
}
",
        ambiguous_publication(),
    );
    let cancelled = AtomicBool::new(false);
    let control = SignatureQueryControl::new(&cancelled, None)
        .with_deadline_step(SignatureQueryStep::CandidateProbe);
    assert_eq!(
        ambiguous.query_with_control(unique_offset(ambiguous.document.text(), "1i32"), control,),
        Err(SignatureQueryError::DeadlineExceeded)
    );
    let control = SignatureQueryControl::new(&cancelled, None)
        .with_deadline_step(SignatureQueryStep::CandidateArgumentProbe);
    assert_eq!(
        ambiguous.query_with_control(unique_offset(ambiguous.document.text(), "1i32"), control,),
        Err(SignatureQueryError::DeadlineExceeded),
        "candidate arguments are polled at their evaluation boundary"
    );
    let control = SignatureQueryControl::new(&cancelled, None)
        .with_deadline_step(SignatureQueryStep::CandidateComparison);
    assert_eq!(
        ambiguous.query_with_control(unique_offset(ambiguous.document.text(), "1i32"), control,),
        Err(SignatureQueryError::DeadlineExceeded),
        "candidate comparisons are polled at their comparison boundary"
    );

    let selected = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = selected_overload(1i32)
    ()
}
",
        selected_overload_publication(),
    );
    let control = SignatureQueryControl::new(&cancelled, None)
        .with_deadline_step(SignatureQueryStep::SelectedReplay);
    assert_eq!(
        selected.query_with_control(unique_offset(selected.document.text(), "1i32"), control,),
        Err(SignatureQueryError::DeadlineExceeded)
    );
}

#[test]
fn accepted_tuple_rejects_changed_bytes_world_mismatch_and_utf8_midpoint() {
    let fixture = SignatureFixture::new(SOURCE);
    let hir = fixture.project.linked_module();
    let cancelled = AtomicBool::new(false);
    let changed = SourceDocument::try_new(
        fixture.document.identity().id().clone(),
        fixture.document.display_name().clone(),
        format!("{}\n// changed accepted bytes", fixture.document.text()),
    )
    .expect("changed logical document");
    assert!(matches!(
        SignatureQuery::production(
            &fixture.world,
            &changed,
            &hir,
            unique_offset(changed.text(), "1i32"),
            SignatureQueryControl::new(&cancelled, None),
        ),
        Err(SignatureQueryError::Stale(stale))
            if matches!(*stale, SignatureSemanticStale::HirDocumentIdentity { .. })
    ));

    let other = SignatureFixture::new(&format!("{SOURCE}\n// another world"));
    assert!(matches!(
        SignatureQuery::production(
            &other.world,
            &fixture.document,
            &hir,
            unique_offset(fixture.document.text(), "1i32"),
            SignatureQueryControl::new(&cancelled, None),
        ),
        Err(SignatureQueryError::Stale(stale))
            if matches!(*stale, SignatureSemanticStale::WorldDocumentIdentity { .. })
    ));

    let unicode = SignatureFixture::new(
        r#"
fn main() -> Unit {
    let label: String = "あ"
    ()
}
"#,
    );
    let unicode_hir = unicode.project.linked_module();
    let middle = unique_offset(unicode.document.text(), "あ") + 1;
    assert_eq!(
        SignatureQuery::production(
            &unicode.world,
            &unicode.document,
            &unicode_hir,
            middle,
            SignatureQueryControl::new(&cancelled, None),
        )
        .err(),
        Some(SignatureQueryError::InvalidPosition(
            SignaturePositionError::NotUtf8Boundary {
                byte_offset: middle
            }
        ))
    );
}

#[test]
fn unsupported_callback_and_non_callable_targets_are_typed_outcomes() {
    let callback = SignatureFixture::new(
        r"
fn main() -> Unit {
    let values: Vec<i32> = [1i32]
    let mapped = values.map { value => value }
    ()
}
",
    );
    assert_eq!(
        callback
            .query_in("values.map { value => value }", "value =>")
            .expect("callback query"),
        SignatureQueryOutcome::NotApplicable(super::SignatureNotApplicable::UnsupportedSurface)
    );

    let non_callable = SignatureFixture::new(
        r"
fn main() -> Unit {
    let value: i32 = 1i32
    let invalid = value(2i32)
    ()
}
",
    );
    let outcome = non_callable
        .query_in("value(2i32)", "2i32")
        .expect("non-callable query");
    assert!(matches!(
        outcome,
        SignatureQueryOutcome::NotApplicable(super::SignatureNotApplicable::NonCallableCallee)
    ));
}

#[test]
fn unknown_callee_and_non_call_dialogue_surfaces_are_typed_outcomes() {
    let unknown = SignatureFixture::new(
        r"
fn main() -> Unit {
    missing_callable(2i32)
    ()
}
",
    );
    assert_eq!(
        unknown
            .query_in("missing_callable(2i32)", "2i32")
            .expect("unknown-callee query"),
        SignatureQueryOutcome::NotApplicable(super::SignatureNotApplicable::UnknownCallee)
    );

    let dialogue_tag = SignatureFixture::new(
        r"
flow @flow.main main {
    alice: Moving.[move at=.left]
}
",
    );
    assert_eq!(
        dialogue_tag
            .query_in("[move at=.left]", "at=.left")
            .expect("dialogue-tag query"),
        SignatureQueryOutcome::NotApplicable(super::SignatureNotApplicable::UnsupportedSurface)
    );

    let goto = SignatureFixture::new(
        r"
flow @flow.main main {
    goto @flow.target
}

flow @flow.target target {
    return ()
}
",
    );
    assert_eq!(
        goto.query_in("goto @flow.target", "@flow.target")
            .expect("goto query"),
        SignatureQueryOutcome::NotApplicable(super::SignatureNotApplicable::UnsupportedSurface)
    );
}

#[test]
fn recovered_missing_close_retains_active_parameter_and_recovery_shape() {
    let fixture = SignatureFixture::recovered(
        r"
fn project_int(value: i32) -> i32 {
    value
}

fn main() -> Unit {
    let value: i32 = project_int(1i32
    ()
}
",
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("project_int(1i32", "1i32")
        .expect("recovered call query")
    else {
        panic!("recovered parenthesized call must retain signature help")
    };
    assert_eq!(
        help.recovery(),
        SignatureRecovery::Recovered {
            missing_close_delimiter: true,
            nodes: 1,
        }
    );
    assert_eq!(
        help.active_parameter()
            .expect("recovered argument still maps")
            .parameter(),
        CallableParameterIndex::try_from_usize(0).expect("parameter zero")
    );
}

#[test]
fn exact_query_budget_succeeds_and_one_under_fails_without_a_partial_result() {
    let fixture = SignatureFixture::new(SOURCE);
    let cursor = unique_offset(fixture.document.text(), "1i32");
    let SignatureQueryOutcome::Help(production) =
        fixture.query(cursor).expect("production budget query")
    else {
        panic!("production budget must produce help")
    };
    let exact_work = production.query_work().total_work();
    let exact = test_limits(exact_work);
    assert!(matches!(
        fixture.query_with_limits(cursor, exact),
        Ok(SignatureQueryOutcome::Help(_))
    ));
    let one_under = test_limits(exact_work.saturating_sub(1));
    assert!(matches!(
        fixture.query_with_limits(cursor, one_under),
        Err(SignatureQueryError::LimitExceeded(_))
    ));
}

#[test]
fn candidate_call_and_nested_path_limits_are_inclusive() {
    let exact_candidates = nested_call_fixture(3);
    let cursor = unique_offset(exact_candidates.document.text(), "1i32");
    let exact = custom_limits(3, 64, 128, 3, 512, 8_388_608, 32, 262_144);
    let SignatureQueryOutcome::Help(help) = exact_candidates
        .query_with_limits(cursor, exact)
        .expect("three cursor-containing calls are accepted")
    else {
        panic!("nested target must produce signature help")
    };
    assert_eq!(help.query_work().search().candidate_calls(), 3);
    assert_eq!(help.query_work().search().nested_calls(), 3);

    let one_under = custom_limits(2, 64, 128, 64, 512, 8_388_608, 32, 262_144);
    assert!(matches!(
        exact_candidates.query_with_limits(cursor, one_under),
        Err(SignatureQueryError::LimitExceeded(
            crate::callable::SignatureLimitExceeded {
                kind: crate::callable::SignatureLimitKind::CandidateCalls,
                observed: 3,
                maximum: 2,
            }
        ))
    ));

    let nested_one_under = custom_limits(4_096, 64, 128, 2, 512, 8_388_608, 32, 262_144);
    assert!(matches!(
        exact_candidates.query_with_limits(cursor, nested_one_under),
        Err(SignatureQueryError::LimitExceeded(
            crate::callable::SignatureLimitExceeded {
                kind: crate::callable::SignatureLimitKind::NestedCalls,
                observed: 3,
                maximum: 2,
            }
        ))
    ));
}

#[test]
fn overload_limit_fails_before_candidate_probe() {
    let fixture = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    ambiguous_value(1i32)
    ()
}
",
        ambiguous_publication(),
    );
    let cancelled = AtomicBool::new(false);
    let control = SignatureQueryControl::new(&cancelled, None)
        .with_deadline_step(SignatureQueryStep::CandidateProbe);
    let hir = fixture.project.linked_module();
    let request = SignatureQuery::try_new(
        &fixture.world,
        &fixture.document,
        &hir,
        unique_offset(fixture.document.text(), "1i32"),
        custom_limits(4_096, 1, 128, 64, 512, 8_388_608, 32, 262_144),
        control,
    )
    .expect("the public overload limit is validated during semantic work");
    assert!(matches!(
        query_signature(request),
        Err(SignatureQueryError::LimitExceeded(
            crate::callable::SignatureLimitExceeded {
                kind: crate::callable::SignatureLimitKind::Overloads,
                observed: 2,
                maximum: 1,
            }
        ))
    ));
}

#[test]
fn surface_scan_charges_every_visited_sibling_call_list() {
    let fixture = SignatureFixture::new(
        r"
fn id(value: i32) -> i32 { value }
fn combine(first: i32, second: i32, third: i32) -> i32 { third }
fn main() -> Unit {
    let value = combine(id(id(1i32)), id(id(2i32)), id(3i32))
    ()
}
",
    );
    let cursor = unique_offset(fixture.document.text(), "3i32");
    let SignatureQueryOutcome::Help(help) = fixture
        .query_with_limits(
            cursor,
            custom_limits(6, 64, 128, 2, 512, 8_388_608, 32, 262_144),
        )
        .expect("all six visited call lists fit the exact search limit")
    else {
        panic!("target id call must produce signature help")
    };
    assert_eq!(help.query_work().search().candidate_calls(), 6);
    assert_eq!(help.query_work().search().nested_calls(), 2);
}

#[test]
fn overload_probes_do_not_recharge_nested_cursor_path_syntax() {
    let fixture = SignatureFixture::with_publication(
        r"
fn id(value: i32) -> i32 { value }
fn main() -> Unit {
    let value = ambiguous_value(id(1i32))
    ()
}
",
        ambiguous_publication(),
    );
    let cursor = unique_offset(fixture.document.text(), "1i32");
    let SignatureQueryOutcome::Help(help) = fixture
        .query_with_limits(
            cursor,
            custom_limits(2, 64, 128, 2, 512, 8_388_608, 32, 262_144),
        )
        .expect("semantic overload probes do not add to two syntax call lists")
    else {
        panic!("nested id target must produce signature help")
    };
    assert_eq!(help.query_work().search().candidate_calls(), 2);
    assert_eq!(help.query_work().search().nested_calls(), 2);
}

#[test]
fn zero_argument_overloads_charge_each_actual_candidate_comparison() {
    let fixture = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = zero_choice()
    ()
}
",
        publication(
            "adapter.signature-zero-comparisons",
            "zero_choice",
            [
                no_parameter_schema(TypeKind::String),
                no_parameter_schema(TypeKind::Bool),
                no_parameter_schema(TypeKind::I32),
            ],
        ),
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("zero_choice()", ")")
        .expect("zero-argument overload query")
    else {
        panic!("ambiguous zero-argument target must produce help")
    };
    assert_eq!(help.active_signature().get(), 0);
    assert_eq!(help.query_work().resolution().specificity_checks(), 0);
    assert!(
        help.work().resolver() >= 2,
        "three candidates require two internally charged pair comparisons"
    );
}

#[test]
fn source_byte_limit_accepts_exact_bytes_and_rejects_one_over() {
    let fixture = SignatureFixture::new(SOURCE);
    let cursor = unique_offset(fixture.document.text(), "1i32");
    let source_bytes = u64::try_from(fixture.document.text().len()).expect("source length");
    assert!(matches!(
        fixture.query_with_limits(
            cursor,
            custom_limits(4_096, 64, 128, 64, 512, source_bytes, 32, 262_144),
        ),
        Ok(SignatureQueryOutcome::Help(_))
    ));
    assert!(matches!(
        fixture.query_with_limits(
            cursor,
            custom_limits(
                4_096,
                64,
                128,
                64,
                512,
                source_bytes - 1,
                32,
                262_144,
            ),
        ),
        Err(SignatureQueryError::LimitExceeded(
            crate::callable::SignatureLimitExceeded {
                kind: crate::callable::SignatureLimitKind::SourceBytes,
                observed,
                maximum,
            }
        )) if observed == source_bytes && maximum == source_bytes - 1
    ));
}

#[test]
fn ambiguous_help_keeps_all_viable_candidates_and_focuses_the_first() {
    let fixture = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = ambiguous_value(1i32)
    ()
}
",
        ambiguous_publication(),
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("ambiguous_value(1i32)", "1i32")
        .expect("ambiguous signature query")
    else {
        panic!("ambiguous target must retain semantic signatures")
    };

    assert_eq!(help.signatures().len(), 2);
    assert_eq!(help.active_signature().get(), 0);
    assert!(help.active_parameter().is_some());
    assert_eq!(
        help.diagnostics()
            .iter()
            .map(crate::callable::CallableDiagnostic::code)
            .collect::<Vec<_>>(),
        vec![CallableDiagnosticCode::AmbiguousOverload]
    );
    assert_ne!(
        help.signatures()[0].candidate(),
        help.signatures()[1].candidate()
    );
}

#[test]
fn selected_nonzero_overload_retains_its_actual_active_signature_index() {
    let fixture = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = selected_overload(1i32)
    ()
}
",
        selected_overload_publication(),
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("selected_overload(1i32)", "1i32")
        .expect("selected overload signature query")
    else {
        panic!("selected overload must retain semantic signatures")
    };

    assert_eq!(help.signatures().len(), 2);
    assert_eq!(
        help.active_signature(),
        SemanticSignatureIndex::try_from_usize(1).expect("signature one")
    );
    assert!(help.active_parameter().is_some());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one ranking test keeps the exact, fixed/rest, and omission precedence cases together"
)]
fn overload_selection_prefers_exact_types_fixed_parameters_and_fewer_omissions() {
    let exact = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = exact_choice(1i32)
    ()
}
",
        publication(
            "adapter.signature-exact",
            "exact_choice",
            [
                one_parameter_schema(
                    CallableParameterType::Unchecked,
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                    TypeKind::Bool,
                ),
                one_parameter_schema(
                    CallableParameterType::Exact(TypeKind::I32),
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                    TypeKind::String,
                ),
            ],
        ),
    );
    let SignatureQueryOutcome::Help(help) = exact
        .query_in("exact_choice(1i32)", "1i32")
        .expect("exact overload query")
    else {
        panic!("exact overload must produce help")
    };
    let active = help.active_signature();
    assert_eq!(help.signatures()[active.get()].result(), &TypeKind::String);

    let fixed = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = fixed_choice(1i32)
    ()
}
",
        publication(
            "adapter.signature-fixed",
            "fixed_choice",
            [
                one_parameter_schema(
                    CallableParameterType::Exact(TypeKind::I32),
                    CallableParameterPassing::RestPositional,
                    CallableParameterPresence::Required,
                    TypeKind::Bool,
                ),
                one_parameter_schema(
                    CallableParameterType::Exact(TypeKind::I32),
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                    TypeKind::String,
                ),
            ],
        ),
    );
    let SignatureQueryOutcome::Help(help) = fixed
        .query_in("fixed_choice(1i32)", "1i32")
        .expect("fixed overload query")
    else {
        panic!("fixed overload must produce help")
    };
    let active = help.active_signature();
    assert_eq!(active.get(), 0);
    assert_eq!(help.signatures()[active.get()].result(), &TypeKind::Bool);
    assert_eq!(
        help.diagnostics()
            .iter()
            .map(crate::callable::CallableDiagnostic::code)
            .collect::<Vec<_>>(),
        vec![CallableDiagnosticCode::AmbiguousOverload],
        "fixed/rest binding counts are not later-contract tie-breakers"
    );

    let omissions = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = omission_choice(1i32)
    ()
}
",
        publication(
            "adapter.signature-omissions",
            "omission_choice",
            [
                two_parameter_mapping_schema(false, TypeKind::Bool),
                one_parameter_schema(
                    CallableParameterType::Exact(TypeKind::I32),
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                    TypeKind::String,
                ),
            ],
        ),
    );
    let SignatureQueryOutcome::Help(help) = omissions
        .query_in("omission_choice(1i32)", "1i32")
        .expect("omission overload query")
    else {
        panic!("omission overload must produce help")
    };
    let active = help.active_signature();
    assert_eq!(help.signatures()[active.get()].result(), &TypeKind::String);
}

#[test]
fn unchecked_slots_are_equally_non_specific() {
    let fixture = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = open_choice(1i32)
    ()
}
",
        publication(
            "adapter.signature-open-specificity",
            "open_choice",
            [
                one_parameter_schema(
                    CallableParameterType::Unchecked,
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                    TypeKind::String,
                ),
                one_parameter_schema(
                    CallableParameterType::Unchecked,
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                    TypeKind::Bool,
                ),
            ],
        ),
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("open_choice(1i32)", "1i32")
        .expect("open specificity query")
    else {
        panic!("open specificity target must produce help")
    };
    assert_eq!(help.active_signature().get(), 0);
    assert_eq!(
        help.diagnostics()
            .iter()
            .map(crate::callable::CallableDiagnostic::code)
            .collect::<Vec<_>>(),
        vec![CallableDiagnosticCode::AmbiguousOverload]
    );
}

#[test]
fn otherwise_equal_standard_candidate_precedes_adapter_candidate() {
    let fixture = SignatureFixture::with_environment_and_publication(
        r"
fn main() -> Unit {
    let value = authority_choice(1i32)
    ()
}
",
        TypeCheckEnv::standard().with_function_signature(
            "authority_choice",
            FunctionSignature::new(
                TypeKind::String,
                [FunctionParam::required("value", TypeKind::I32)],
            ),
        ),
        publication(
            "adapter.signature-standard-precedence",
            "authority_choice",
            [one_parameter_schema(
                CallableParameterType::Exact(TypeKind::I32),
                CallableParameterPassing::PositionalOrNamed,
                CallableParameterPresence::Required,
                TypeKind::Bool,
            )],
        ),
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("authority_choice(1i32)", "1i32")
        .expect("standard-adapter precedence query")
    else {
        panic!("standard-adapter target must produce help")
    };
    let active = help.active_signature();
    assert!(matches!(
        help.signatures()[active.get()].origin(),
        SignatureOrigin::Standard { .. }
    ));
    assert_eq!(help.signatures()[active.get()].result(), &TypeKind::String);
}

#[test]
fn rejected_overload_probes_do_not_duplicate_selected_closure_capture_inventory() {
    let source = r"
fn main() -> Unit {
    let captured = 1i32
    let callback = closure_choice(|value: i32| -> i32 { captured })
    ()
}
";
    let fixture = SignatureFixture::with_publication(
        source,
        publication(
            "adapter.signature-transaction",
            "closure_choice",
            [
                one_parameter_schema(
                    CallableParameterType::Unchecked,
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                    TypeKind::Bool,
                ),
                one_parameter_schema(
                    CallableParameterType::Exact(TypeKind::I32),
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                    TypeKind::String,
                ),
            ],
        ),
    );
    let cancellation = AtomicBool::new(false);
    let mut resolver_work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    let mut signature_work = SignatureQueryWorkMeter::new(PRODUCTION_SIGNATURE_LIMITS);
    let control = SignatureQueryControl::new(&cancellation, None);
    let linked = fixture.project.linked_module();
    let byte_offset = unique_offset(source, "captured })");
    let site = super::surface::select_signature_surface(
        &linked,
        &fixture.document,
        byte_offset,
        control,
        &mut signature_work,
    )
    .expect("surface search succeeds")
    .site
    .expect("focused call surface");
    let report = analyze_registered_project_types_for_signature_call(SignatureFocusedAnalysis {
        module: &linked,
        registered: &fixture.world,
        site,
        cancellation: &cancellation,
        work: &mut resolver_work,
        signature_work: &mut signature_work,
        signature_control: &control,
    })
    .expect("focused semantic analysis succeeds");

    assert_eq!(report.report().closure_captures.len(), 1);
    assert_eq!(
        report.report().closure_captures[0]
            .captures
            .iter()
            .map(|capture| capture.name.as_str())
            .collect::<Vec<_>>(),
        vec!["captured"]
    );
}

#[test]
fn no_viable_overload_is_unselected_and_retains_typed_diagnostic() {
    let fixture = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = impossible_choice(1i32)
    ()
}
",
        publication(
            "adapter.signature-no-viable",
            "impossible_choice",
            [
                one_parameter_schema(
                    CallableParameterType::Exact(TypeKind::String),
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                    TypeKind::String,
                ),
                one_parameter_schema(
                    CallableParameterType::Exact(TypeKind::Bool),
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                    TypeKind::Bool,
                ),
            ],
        ),
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("impossible_choice(1i32)", "1i32")
        .expect("no-viable query still returns candidate signatures")
    else {
        panic!("no-viable target must produce help")
    };
    assert_eq!(help.active_signature().get(), 0);
    assert_eq!(
        help.active_parameter(),
        Some(crate::callable::CallableParameterCoordinate::new(
            CallableGroupIndex::ZERO,
            CallableParameterIndex::try_from_usize(0).expect("parameter zero"),
        ))
    );
    assert_eq!(
        help.diagnostics()
            .iter()
            .map(crate::callable::CallableDiagnostic::code)
            .collect::<Vec<_>>(),
        vec![CallableDiagnosticCode::NoViableSignature]
    );
}

#[test]
fn rejected_candidates_are_filtered_and_singletons_keep_no_viable_facts() {
    let mixed = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    mixed_choice(1i32)
    ()
}
",
        publication(
            "adapter.signature-mixed-viability",
            "mixed_choice",
            [
                single_parameter_schema(TypeKind::String),
                single_parameter_schema(TypeKind::Bool),
                one_parameter_schema(
                    CallableParameterType::Exact(TypeKind::String),
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                    TypeKind::Unit,
                ),
            ],
        ),
    );
    let SignatureQueryOutcome::Help(help) = mixed
        .query_in("mixed_choice(1i32)", "1i32")
        .expect("mixed-viability query")
    else {
        panic!("two tied viable candidates must retain help")
    };
    assert_eq!(help.signatures().len(), 2);
    assert_eq!(help.active_signature().get(), 0);
    assert_eq!(
        help.diagnostics()
            .iter()
            .map(crate::callable::CallableDiagnostic::code)
            .collect::<Vec<_>>(),
        vec![CallableDiagnosticCode::AmbiguousOverload]
    );

    let singleton = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    singleton_choice(1i32)
    ()
}
",
        publication(
            "adapter.signature-singleton-rejection",
            "singleton_choice",
            [one_parameter_schema(
                CallableParameterType::Exact(TypeKind::String),
                CallableParameterPassing::PositionalOrNamed,
                CallableParameterPresence::Required,
                TypeKind::String,
            )],
        ),
    );
    let SignatureQueryOutcome::Help(help) = singleton
        .query_in("singleton_choice(1i32)", "1i32")
        .expect("rejected singleton query")
    else {
        panic!("a rejected singleton still projects its one signature")
    };
    assert_eq!(help.signatures().len(), 1);
    assert_eq!(help.active_signature().get(), 0);
    assert_eq!(
        help.diagnostics()
            .iter()
            .map(crate::callable::CallableDiagnostic::code)
            .collect::<Vec<_>>(),
        vec![CallableDiagnosticCode::NoViableSignature]
    );
}

#[test]
fn candidate_zero_owns_unselected_mapping_and_exact_slots_rank_over_unchecked() {
    let no_viable = SignatureFixture::with_publication(
        r#"
fn main() -> Unit {
    mapped_rejection("wrong")
    ()
}
"#,
        publication(
            "adapter.signature-rejected-mapping",
            "mapped_rejection",
            [
                two_parameter_mapping_schema(true, TypeKind::String),
                two_parameter_mapping_schema(false, TypeKind::Bool),
            ],
        ),
    );
    let SignatureQueryOutcome::Help(help) = no_viable
        .query_in("mapped_rejection(\"wrong\")", "wrong")
        .expect("candidate-zero rejected mapping query")
    else {
        panic!("no-viable candidates still project help")
    };
    assert_eq!(help.active_signature().get(), 0);
    assert_eq!(
        help.active_parameter()
            .expect("candidate zero supplies the unselected mapping")
            .parameter()
            .get(),
        1
    );

    let exact = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    exact_choice(1i32)
    ()
}
",
        publication(
            "adapter.signature-exact-slot",
            "exact_choice",
            [
                one_parameter_schema(
                    CallableParameterType::Unchecked,
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                    TypeKind::Bool,
                ),
                one_parameter_schema(
                    CallableParameterType::Exact(TypeKind::I32),
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                    TypeKind::String,
                ),
            ],
        ),
    );
    let SignatureQueryOutcome::Help(help) = exact
        .query_in("exact_choice(1i32)", "1i32")
        .expect("exact-slot query")
    else {
        panic!("exact slot must select the exact overload")
    };
    assert_eq!(help.active_signature().get(), 1);
    assert_eq!(help.signatures()[1].result(), &TypeKind::String);
}

#[test]
fn fixed_expression_spread_focuses_the_exact_element_parameter() {
    let fixture = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    spread_choice([1i32 + 0i32, 2i32 + 0i32]...)
    ()
}
",
        publication(
            "adapter.signature-fixed-spread-focus",
            "spread_choice",
            [two_positional_parameter_schema_with_spread(
                TypeKind::String,
            )],
        ),
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("spread_choice([1i32 + 0i32, 2i32 + 0i32]...)", "2i32")
        .expect("fixed expression-spread query")
    else {
        panic!("fixed expression spread must produce help")
    };
    assert_eq!(
        help.active_parameter()
            .expect("the second fixed element has exact source evidence")
            .parameter()
            .get(),
        1
    );
}

#[test]
fn compact_numeric_spread_focuses_the_exact_element_parameter() {
    let fixture = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    spread_choice([1i32, 22i32]...)
    ()
}
",
        publication(
            "adapter.signature-compact-numeric-spread-focus",
            "spread_choice",
            [two_positional_parameter_schema_with_spread(
                TypeKind::String,
            )],
        ),
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("spread_choice([1i32, 22i32]...)", "22i32")
        .expect("compact numeric-spread query")
    else {
        panic!("compact numeric spread must produce help")
    };

    assert_eq!(
        help.active_parameter()
            .expect("the second compact literal has exact source evidence")
            .parameter()
            .get(),
        1
    );
}

#[test]
fn ambiguous_candidates_use_the_first_committed_mapping_for_ui_focus() {
    let fixture = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = mapped_differently(1i32)
    ()
}
",
        publication(
            "adapter.signature-active-parameter",
            "mapped_differently",
            [
                two_parameter_mapping_schema(true, TypeKind::String),
                two_parameter_mapping_schema(false, TypeKind::Bool),
            ],
        ),
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("mapped_differently(1i32)", "1i32")
        .expect("candidate-specific mapping query")
    else {
        panic!("ambiguous target must produce help")
    };
    assert_eq!(help.active_signature().get(), 0);
    assert_eq!(
        help.active_parameter()
            .expect("the first viable candidate supplies committed focus")
            .parameter()
            .get(),
        1
    );
}

#[test]
fn unselected_candidates_bind_active_slots_after_prior_arguments() {
    let fixture = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let trailing = pair_choice(1i32, )
    let reordered = pair_choice(first = 1i32, 2i32)
    ()
}
",
        publication(
            "adapter.signature-next-active-parameter",
            "pair_choice",
            [
                two_positional_parameter_schema(TypeKind::String),
                two_positional_parameter_schema(TypeKind::Bool),
            ],
        ),
    );
    for (call, cursor) in [
        ("pair_choice(1i32, )", ")"),
        ("pair_choice(first = 1i32, 2i32)", "2i32"),
    ] {
        let SignatureQueryOutcome::Help(help) = fixture
            .query_in(call, cursor)
            .expect("candidate-local active binding query")
        else {
            panic!("unselected pair target must produce help")
        };
        assert_eq!(help.active_signature().get(), 0);
        assert_eq!(
            help.active_parameter()
                .expect("both candidates agree on the next parameter")
                .parameter()
                .get(),
            1
        );
    }
}

#[test]
fn comma_start_focuses_the_following_semantic_parameter() {
    let fixture = SignatureFixture::with_publication(
        r"
fn main() -> Unit {
    let value = pair_choice(1i32, 2i32)
    ()
}
",
        publication(
            "adapter.signature-comma-boundary",
            "pair_choice",
            [two_positional_parameter_schema(TypeKind::String)],
        ),
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("pair_choice(1i32, 2i32)", ",")
        .expect("comma-boundary signature query")
    else {
        panic!("comma boundary must produce signature help")
    };

    assert_eq!(help.active_signature().get(), 0);
    assert_eq!(
        help.active_parameter()
            .expect("comma start belongs to the following parameter")
            .parameter()
            .get(),
        1
    );
}

#[test]
fn a05_duplicate_named_argument_retains_parameter_and_both_exact_spans() {
    let call = "argument_target(first = 1i32, first = 2i32)";
    let (fixture, help) = argument_diagnostic_help(
        call,
        "2i32",
        two_positional_parameter_schema(TypeKind::String),
    );
    let diagnostic = diagnostic(&help, CallableDiagnosticCode::DuplicateArgument);

    assert_eq!(
        help.active_parameter()
            .expect("duplicate remains mapped to its parameter")
            .parameter()
            .get(),
        0
    );
    assert!(matches!(
        diagnostic.subject(),
        CallableDiagnosticSubject::Parameter(coordinate) if coordinate.parameter().get() == 0
    ));
    assert_eq!(
        source_text(
            &fixture.document,
            diagnostic.span().expect("duplicate primary span")
        ),
        "first"
    );
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(
        source_text(
            &fixture.document,
            diagnostic.related()[0]
                .span()
                .expect("first binding related span")
        ),
        "first"
    );
    assert_eq!(
        diagnostic_codes(&help),
        vec![CallableDiagnosticCode::DuplicateArgument]
    );
}

#[test]
fn a05_positional_then_named_duplicate_retains_the_first_argument_span() {
    let call = "argument_target(1i32, first = 2i32)";
    let (fixture, help) = argument_diagnostic_help(
        call,
        "2i32",
        two_positional_parameter_schema(TypeKind::String),
    );
    let diagnostic = diagnostic(&help, CallableDiagnosticCode::DuplicateArgument);

    assert_eq!(
        source_text(
            &fixture.document,
            diagnostic.span().expect("duplicate primary span")
        ),
        "first"
    );
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(
        source_text(
            &fixture.document,
            diagnostic.related()[0]
                .span()
                .expect("first positional binding span")
        ),
        "1i32"
    );
    assert_eq!(
        diagnostic_codes(&help),
        vec![CallableDiagnosticCode::DuplicateArgument]
    );
}

#[test]
fn a05_show_positional_then_named_look_duplicate_uses_the_typed_show_mapping() {
    let call = "show(@character.akane, .normal, look = .normal)";
    let fixture = SignatureFixture::new(&format!("fn main() -> Unit {{\n    {call}\n    ()\n}}\n"));
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in(call, "look =")
        .expect("show duplicate query")
    else {
        panic!("typed show duplicate must retain signature help")
    };
    let diagnostic = diagnostic(&help, CallableDiagnosticCode::DuplicateArgument);

    assert_eq!(
        source_text(
            &fixture.document,
            diagnostic.span().expect("duplicate look name span")
        ),
        "look"
    );
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(
        source_text(
            &fixture.document,
            diagnostic.related()[0]
                .span()
                .expect("first positional look span")
        ),
        ".normal"
    );
    assert_eq!(
        diagnostic_codes(&help),
        vec![CallableDiagnosticCode::DuplicateArgument]
    );
}

#[test]
fn a08_unknown_named_argument_is_unmapped_with_an_exact_argument_span() {
    let call = "argument_target(unknown = 1i32)";
    let (fixture, help) = argument_diagnostic_help(
        call,
        "1i32",
        one_parameter_schema(
            CallableParameterType::Exact(TypeKind::I32),
            CallableParameterPassing::PositionalOrNamed,
            CallableParameterPresence::Optional,
            TypeKind::String,
        ),
    );
    let diagnostic = diagnostic(&help, CallableDiagnosticCode::UnknownNamedArgument);

    assert_eq!(help.active_parameter(), None);
    assert!(matches!(
        diagnostic.subject(),
        CallableDiagnosticSubject::Argument(_)
    ));
    assert_eq!(
        source_text(
            &fixture.document,
            diagnostic.span().expect("unknown named primary span")
        ),
        "unknown"
    );
    assert!(diagnostic.related().is_empty());
    assert_eq!(
        diagnostic_codes(&help),
        vec![CallableDiagnosticCode::UnknownNamedArgument]
    );
}

#[test]
fn a10_unsupported_spread_stops_later_mapping_with_an_exact_argument_span() {
    let call = "argument_target([1i32]..., 2i32)";
    let (fixture, help) = argument_diagnostic_help(
        call,
        "2i32",
        two_positional_parameter_schema(TypeKind::String),
    );
    let diagnostic = diagnostic(&help, CallableDiagnosticCode::UnsupportedSpread);

    assert_eq!(help.active_parameter(), None);
    assert!(matches!(
        diagnostic.subject(),
        CallableDiagnosticSubject::Argument(_)
    ));
    assert_eq!(
        source_text(
            &fixture.document,
            diagnostic.span().expect("spread primary span")
        ),
        "[1i32]..."
    );
    assert!(diagnostic.related().is_empty());
    assert_eq!(
        diagnostic_codes(&help),
        vec![CallableDiagnosticCode::UnsupportedSpread]
    );
}

#[test]
fn a11_extra_positional_argument_is_unmapped_with_an_exact_argument_span() {
    let call = "argument_target(1i32)";
    let (fixture, help) =
        argument_diagnostic_help(call, "1i32", no_parameter_schema(TypeKind::String));
    let diagnostic = diagnostic(&help, CallableDiagnosticCode::TooManyPositionalArguments);

    assert_eq!(help.active_parameter(), None);
    assert!(matches!(
        diagnostic.subject(),
        CallableDiagnosticSubject::Argument(_)
    ));
    assert_eq!(
        source_text(
            &fixture.document,
            diagnostic.span().expect("extra positional primary span")
        ),
        "1i32"
    );
    assert!(diagnostic.related().is_empty());
    assert_eq!(
        diagnostic_codes(&help),
        vec![CallableDiagnosticCode::TooManyPositionalArguments]
    );
}

#[test]
fn a11_fixed_literal_spread_overflow_uses_each_unmapped_element_span() {
    let empty_call = "argument_target([1i32]...)";
    let (empty_fixture, empty) = argument_diagnostic_help(
        empty_call,
        "1i32",
        fixed_literal_spread_schema(0, TypeKind::String),
    );
    let empty_diagnostic = diagnostic(&empty, CallableDiagnosticCode::TooManyPositionalArguments);
    assert_eq!(
        source_text(
            &empty_fixture.document,
            empty_diagnostic
                .span()
                .expect("unmapped spread element span")
        ),
        "1i32"
    );
    assert_eq!(
        diagnostic_codes(&empty),
        vec![CallableDiagnosticCode::TooManyPositionalArguments]
    );

    let partial_call = "argument_target([1i32, 2i32]...)";
    let partial_fixture = SignatureFixture::with_publication(
        &format!("fn main() -> Unit {{\n    {partial_call}\n    ()\n}}\n"),
        publication(
            "adapter.signature-fixed-spread-overflow",
            "argument_target",
            [fixed_literal_spread_schema(1, TypeKind::String)],
        ),
    );
    let SignatureQueryOutcome::Help(first) = partial_fixture
        .query_in(partial_call, "1i32")
        .expect("first spread element query")
    else {
        panic!("first fixed-spread element must retain signature help")
    };
    assert_eq!(
        first
            .active_parameter()
            .expect("first fixed-spread element remains mapped")
            .parameter()
            .get(),
        0
    );
    let SignatureQueryOutcome::Help(second) = partial_fixture
        .query_in(partial_call, "2i32")
        .expect("overflow spread element query")
    else {
        panic!("overflow fixed-spread element must retain signature help")
    };
    let diagnostic = diagnostic(&second, CallableDiagnosticCode::TooManyPositionalArguments);
    assert_eq!(
        source_text(
            &partial_fixture.document,
            diagnostic.span().expect("overflow element span")
        ),
        "2i32"
    );
    assert_eq!(
        diagnostic_codes(&second),
        vec![CallableDiagnosticCode::TooManyPositionalArguments]
    );
}

#[test]
fn a12_missing_required_argument_uses_the_exact_insertion_span() {
    let call = "argument_target(1i32, )";
    let (fixture, help) = argument_diagnostic_help(
        call,
        ")",
        two_required_positional_parameter_schema(TypeKind::String),
    );
    let diagnostic = diagnostic(&help, CallableDiagnosticCode::MissingArgument);
    let span = diagnostic.span().expect("missing argument insertion span");
    let call_start = unique_offset(fixture.document.text(), call);
    let close = call.find(')').expect("call close");

    assert_eq!(
        help.active_parameter()
            .expect("trailing comma focuses the missing parameter")
            .parameter()
            .get(),
        1
    );
    assert!(matches!(
        diagnostic.subject(),
        CallableDiagnosticSubject::Parameter(coordinate) if coordinate.parameter().get() == 1
    ));
    assert_eq!(span.range().start(), call_start + close);
    assert_eq!(span.range().end(), call_start + close);
    assert!(diagnostic.related().is_empty());
    assert_eq!(
        diagnostic_codes(&help),
        vec![CallableDiagnosticCode::MissingArgument]
    );
}

#[test]
fn a14_positional_after_named_reports_the_skipped_binding_and_advances() {
    let call = "argument_target(first = 1i32, 2i32)";
    let (fixture, help) = argument_diagnostic_help(
        call,
        "2i32",
        two_positional_parameter_schema(TypeKind::String),
    );
    let diagnostic = diagnostic(&help, CallableDiagnosticCode::ParameterAlreadyBound);

    assert_eq!(
        help.active_parameter()
            .expect("positional value advances to the next unbound parameter")
            .parameter()
            .get(),
        1
    );
    assert!(matches!(
        diagnostic.subject(),
        CallableDiagnosticSubject::Parameter(coordinate) if coordinate.parameter().get() == 0
    ));
    assert_eq!(
        source_text(
            &fixture.document,
            diagnostic.span().expect("positional primary span")
        ),
        "2i32"
    );
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(
        source_text(
            &fixture.document,
            diagnostic.related()[0]
                .span()
                .expect("named binding related span")
        ),
        "first"
    );
    assert_eq!(
        diagnostic_codes(&help),
        vec![CallableDiagnosticCode::ParameterAlreadyBound]
    );
}

#[test]
fn every_callable_family_has_an_explicit_native_query_audit_state() {
    let audited = CallableFamily::ALL.map(|family| (family, signature_family_support(family)));
    assert_eq!(audited.len(), CallableFamily::ALL.len());
    assert!(
        audited
            .iter()
            .all(|(_, support)| support == &SignatureFamilySupport::NativeFacts)
    );
    assert_eq!(
        PRODUCTION_CALLABLE_LIMITS.max_candidates_per_call(),
        256,
        "the audit covers the fixed production query policy"
    );
}

fn argument_diagnostic_help(
    call: &str,
    cursor: &str,
    schema: CallableSignatureSchema,
) -> (SignatureFixture, SemanticSignatureHelp) {
    let source = format!(
        r"
fn main() -> Unit {{
    let value = {call}
    ()
}}
"
    );
    let fixture = SignatureFixture::with_publication(
        &source,
        publication(
            "adapter.signature-argument-diagnostic",
            "argument_target",
            [schema],
        ),
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in(call, cursor)
        .expect("argument diagnostic signature query")
    else {
        panic!("argument diagnostic target must produce signature help")
    };
    (fixture, help)
}

fn diagnostic(help: &SemanticSignatureHelp, code: CallableDiagnosticCode) -> &CallableDiagnostic {
    help.diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == code)
        .unwrap_or_else(|| panic!("missing callable diagnostic {code:?}"))
}

fn diagnostic_codes(help: &SemanticSignatureHelp) -> Vec<CallableDiagnosticCode> {
    help.diagnostics()
        .iter()
        .map(CallableDiagnostic::code)
        .collect()
}

fn selected_overload_publication() -> TestPublication {
    let owner = EnvironmentCallableOwner::Adapter(
        AdapterPackageId::try_new("adapter.signature-selection").expect("adapter id"),
    );
    let key = CallableLookupKey::Free(callable_path(&["selected_overload"]));
    let records = [
        no_parameter_schema(TypeKind::String),
        single_parameter_schema(TypeKind::Bool),
    ]
    .into_iter()
    .enumerate()
    .map(|(overload, schema)| {
        EnvironmentCallablePublicationRecord::try_new(
            EnvironmentCallableKind::Function,
            key.clone(),
            CallableOverloadIndex::try_from_usize(overload).expect("overload"),
            schema,
            CallableDocumentation::missing(),
            None,
            None,
            EnvironmentDeclarationOrdinal::try_from_usize(overload).expect("declaration ordinal"),
        )
        .expect("selected overload publication record")
    })
    .collect::<Vec<_>>();
    TestPublication { owner, records }
}

fn ambiguous_publication() -> TestPublication {
    let owner = EnvironmentCallableOwner::Adapter(
        AdapterPackageId::try_new("adapter.signature-ambiguity").expect("adapter id"),
    );
    let key = CallableLookupKey::Free(callable_path(&["ambiguous_value"]));
    let records = [TypeKind::String, TypeKind::Bool]
        .into_iter()
        .enumerate()
        .map(|(overload, result)| {
            EnvironmentCallablePublicationRecord::try_new(
                EnvironmentCallableKind::Function,
                key.clone(),
                CallableOverloadIndex::try_from_usize(overload).expect("overload"),
                single_parameter_schema(result),
                CallableDocumentation::missing(),
                None,
                None,
                EnvironmentDeclarationOrdinal::try_from_usize(overload)
                    .expect("declaration ordinal"),
            )
            .expect("ambiguous publication record")
        })
        .collect::<Vec<_>>();
    TestPublication { owner, records }
}

fn publication(
    owner: &str,
    callable: &str,
    schemas: impl IntoIterator<Item = CallableSignatureSchema>,
) -> TestPublication {
    let owner =
        EnvironmentCallableOwner::Adapter(AdapterPackageId::try_new(owner).expect("adapter id"));
    let segments = callable.split('.').collect::<Vec<_>>();
    let key = CallableLookupKey::Free(callable_path(&segments));
    let records = schemas
        .into_iter()
        .enumerate()
        .map(|(overload, schema)| {
            EnvironmentCallablePublicationRecord::try_new(
                EnvironmentCallableKind::Function,
                key.clone(),
                CallableOverloadIndex::try_from_usize(overload).expect("overload"),
                schema,
                CallableDocumentation::missing(),
                None,
                None,
                EnvironmentDeclarationOrdinal::try_from_usize(overload)
                    .expect("declaration ordinal"),
            )
            .expect("overload publication record")
        })
        .collect::<Vec<_>>();
    TestPublication { owner, records }
}

fn one_parameter_schema(
    ty: CallableParameterType,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
    result: TypeKind,
) -> CallableSignatureSchema {
    schema_with_parameters(
        vec![
            CallableParameter::try_new(
                CallableParameterIndex::try_from_usize(0).expect("parameter zero"),
                Some(CallableName::try_new("value").expect("parameter name")),
                ty,
                passing,
                presence,
                None,
                None,
            )
            .expect("parameter"),
        ],
        result,
    )
}

fn two_parameter_mapping_schema(
    named_parameter_first: bool,
    result: TypeKind,
) -> CallableSignatureSchema {
    let make = |index: usize,
                name: &str,
                passing: CallableParameterPassing,
                presence: CallableParameterPresence| {
        CallableParameter::try_new(
            CallableParameterIndex::try_from_usize(index).expect("parameter index"),
            Some(CallableName::try_new(name).expect("parameter name")),
            CallableParameterType::Exact(TypeKind::I32),
            passing,
            presence,
            None,
            None,
        )
        .expect("parameter")
    };
    let parameters = if named_parameter_first {
        vec![
            make(
                0,
                "named",
                CallableParameterPassing::NamedOnly,
                CallableParameterPresence::Optional,
            ),
            make(
                1,
                "value",
                CallableParameterPassing::PositionalOnly,
                CallableParameterPresence::Required,
            ),
        ]
    } else {
        vec![
            make(
                0,
                "value",
                CallableParameterPassing::PositionalOnly,
                CallableParameterPresence::Required,
            ),
            make(
                1,
                "named",
                CallableParameterPassing::NamedOnly,
                CallableParameterPresence::Optional,
            ),
        ]
    };
    schema_with_parameters(parameters, result)
}

fn two_positional_parameter_schema(result: TypeKind) -> CallableSignatureSchema {
    let parameters = [
        ("first", CallableParameterPresence::Required),
        ("second", CallableParameterPresence::Optional),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (name, presence))| {
        CallableParameter::try_new(
            CallableParameterIndex::try_from_usize(index).expect("parameter index"),
            Some(CallableName::try_new(name).expect("parameter name")),
            CallableParameterType::Exact(TypeKind::I32),
            CallableParameterPassing::PositionalOrNamed,
            presence,
            None,
            None,
        )
        .expect("positional parameter")
    })
    .collect();
    schema_with_parameters(parameters, result)
}

fn two_required_positional_parameter_schema(result: TypeKind) -> CallableSignatureSchema {
    let parameters = ["first", "second"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            CallableParameter::try_new(
                CallableParameterIndex::try_from_usize(index).expect("parameter index"),
                Some(CallableName::try_new(name).expect("parameter name")),
                CallableParameterType::Exact(TypeKind::I32),
                CallableParameterPassing::PositionalOrNamed,
                CallableParameterPresence::Required,
                None,
                None,
            )
            .expect("required positional parameter")
        })
        .collect();
    schema_with_parameters(parameters, result)
}

fn two_positional_parameter_schema_with_spread(result: TypeKind) -> CallableSignatureSchema {
    let parameters = ["first", "second"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            CallableParameter::try_new(
                CallableParameterIndex::try_from_usize(index).expect("parameter index"),
                Some(CallableName::try_new(name).expect("parameter name")),
                CallableParameterType::Exact(TypeKind::I32),
                CallableParameterPassing::PositionalOrNamed,
                CallableParameterPresence::Required,
                None,
                None,
            )
            .expect("fixed-spread parameter")
        })
        .collect();
    let group = CallableParameterGroup::try_new(
        CallableGroupIndex::ZERO,
        CallableGroupKind::Initial,
        parameters,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("fixed-spread parameter group");
    CallableSignatureSchema::try_new(
        vec![group],
        result,
        CallableEffectSchema::fixed(EffectRow::closed(EffectSet::new())),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::FixedLiteralOnly,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("fixed-spread signature schema")
}

fn fixed_literal_spread_schema(
    parameter_count: usize,
    result: TypeKind,
) -> CallableSignatureSchema {
    let parameters = (0..parameter_count)
        .map(|index| {
            CallableParameter::try_new(
                CallableParameterIndex::try_from_usize(index).expect("parameter index"),
                Some(CallableName::try_new(format!("value{}", index + 1)).expect("parameter name")),
                CallableParameterType::Exact(TypeKind::I32),
                CallableParameterPassing::PositionalOrNamed,
                CallableParameterPresence::Required,
                None,
                None,
            )
            .expect("fixed-spread parameter")
        })
        .collect();
    let group = CallableParameterGroup::try_new(
        CallableGroupIndex::ZERO,
        CallableGroupKind::Initial,
        parameters,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("fixed-spread parameter group");
    CallableSignatureSchema::try_new(
        vec![group],
        result,
        CallableEffectSchema::fixed(EffectRow::closed(EffectSet::new())),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::FixedLiteralOnly,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("fixed-spread signature schema")
}

fn schema_with_parameters(
    parameters: Vec<CallableParameter>,
    result: TypeKind,
) -> CallableSignatureSchema {
    let group = CallableParameterGroup::try_new(
        CallableGroupIndex::ZERO,
        CallableGroupKind::Initial,
        parameters,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("parameter group");
    CallableSignatureSchema::try_new(
        vec![group],
        result,
        CallableEffectSchema::fixed(EffectRow::closed(EffectSet::new())),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("signature schema")
}

fn no_parameter_schema(result: TypeKind) -> CallableSignatureSchema {
    let group = CallableParameterGroup::try_new(
        CallableGroupIndex::ZERO,
        CallableGroupKind::Initial,
        Vec::new(),
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("empty parameter group");
    CallableSignatureSchema::try_new(
        vec![group],
        result,
        CallableEffectSchema::fixed(EffectRow::closed(EffectSet::new())),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("no-parameter signature schema")
}

fn single_parameter_schema(result: TypeKind) -> CallableSignatureSchema {
    let parameter = CallableParameter::try_new(
        CallableParameterIndex::try_from_usize(0).expect("parameter zero"),
        Some(CallableName::try_new("value").expect("parameter name")),
        CallableParameterType::Exact(TypeKind::I32),
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Required,
        None,
        None,
    )
    .expect("parameter");
    let group = CallableParameterGroup::try_new(
        CallableGroupIndex::ZERO,
        CallableGroupKind::Initial,
        vec![parameter],
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("parameter group");
    CallableSignatureSchema::try_new(
        vec![group],
        result,
        CallableEffectSchema::fixed(EffectRow::closed(EffectSet::new())),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("signature schema")
}

fn callable_path(segments: &[&str]) -> CallablePath {
    CallablePath::try_new(
        segments
            .iter()
            .map(|segment| CallableName::try_new(*segment).expect("callable segment")),
    )
    .expect("callable path")
}

fn test_limits(max_query_work: u64) -> SignatureQueryLimits {
    custom_limits(4_096, 64, 128, 64, 512, 8_388_608, 32, max_query_work)
}

#[allow(clippy::too_many_arguments)]
fn custom_limits(
    candidate_calls: u64,
    overloads: u64,
    parameters_per_signature: u64,
    nested_calls: u64,
    recovery_nodes: u64,
    source_bytes: u64,
    diagnostics: u64,
    work_units: u64,
) -> SignatureQueryLimits {
    SignatureQueryLimits::try_for_test(
        candidate_calls,
        overloads,
        parameters_per_signature,
        nested_calls,
        recovery_nodes,
        source_bytes,
        diagnostics,
        work_units,
    )
    .expect("positive signature query limits")
}

fn nested_call_fixture(depth: usize) -> SignatureFixture {
    let expression = format!("{}1i32{}", "id(".repeat(depth), ")".repeat(depth));
    SignatureFixture::new(&format!(
        r"
fn id(value: i32) -> i32 {{ value }}
fn main() -> Unit {{
    let value: i32 = {expression}
    ()
}}
"
    ))
}

fn unique_offset(source: &str, needle: &str) -> usize {
    let start = source.find(needle).expect("source needle");
    assert_eq!(
        source[start + needle.len()..].find(needle),
        None,
        "source needle must be unique"
    );
    start
}

fn source_text<'a>(document: &'a SourceDocument, span: &arcweft_source::SourceSpan) -> &'a str {
    &document.text()[span.range().as_range()]
}
