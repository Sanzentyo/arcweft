#![allow(
    clippy::result_large_err,
    reason = "test helpers assert the complete public typed query error without erasing evidence"
)]

use std::{sync::Arc, sync::atomic::AtomicBool, time::Instant};

use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_syntax::{ast::module_path::CanonicalModulePath, parser::parse_source};
use arcweft_source::SourceDocument;

use crate::{
    callable::{
        AdapterPackageId, CallableArgumentPolicy, CallableCandidateId, CallableDiagnosticCode,
        CallableDocumentation, CallableEffectSchema, CallableFamily, CallableGroupIndex,
        CallableGroupKind, CallableLimits, CallableLookupKey, CallableName, CallableOverloadIndex,
        CallableParameter, CallableParameterGroup, CallableParameterIndex,
        CallableParameterPassing, CallableParameterPresence, CallableParameterType, CallablePath,
        CallableSignatureSchema, CallableValidator, EnvironmentCallableKind,
        EnvironmentCallableOwner, EnvironmentCallablePublication,
        EnvironmentCallablePublicationRecord, EnvironmentDeclarationOrdinal,
        PRODUCTION_CALLABLE_LIMITS, SemanticSignatureIndex, SpreadArgumentPolicy,
        UnknownNamedArgumentPolicy,
    },
    effect_row::EffectRow,
    effects::EffectSet,
    env::{FunctionParam, FunctionSignature, TypeCheckEnv},
    registration::{CharacterRegistrar, CharacterRegistrationRequest, RegisteredSemanticWorld},
    test_support::character_project::{
        PACKAGE, one_character_facts, register, root_project_source, sample_manifest,
        source_document,
    },
    types::TypeKind,
};

use super::{
    SignatureFamilySupport, SignaturePositionError, SignatureQuery, SignatureQueryControl,
    SignatureQueryError, SignatureQueryOutcome, SignatureRecovery, SignatureSemanticStale,
    query_signature, signature_family_support,
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

    fn with_publication(source: &str, publication: EnvironmentCallablePublication) -> Self {
        let (document, project, world_id) =
            root_project_source("signature-query-publication", source);
        let facts = one_character_facts(&document, world_id, &sample_manifest("layers/body.png"));
        let world = CharacterRegistrar::register(
            CharacterRegistrationRequest::new(
                Arc::new(TypeCheckEnv::standard()),
                &project,
                &facts,
                None,
            )
            .with_callable_publication(publication),
        )
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

    fn query_with_limits(
        &self,
        byte_offset: usize,
        limits: CallableLimits,
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
        Err(SignatureQueryError::Stale(
            SignatureSemanticStale::HirDocumentIdentity { .. }
        ))
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
        Err(SignatureQueryError::Stale(
            SignatureSemanticStale::WorldDocumentIdentity { .. }
        ))
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
        SignatureQueryOutcome::NotApplicable(
            super::SignatureNotApplicable::NonCallableCallee { .. }
        )
    ));
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
    let exact_work = production.work().total_work().expect("bounded work");
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
fn ambiguous_help_keeps_all_candidates_and_focuses_deterministic_first() {
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

fn selected_overload_publication() -> EnvironmentCallablePublication {
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
    EnvironmentCallablePublication::try_new(owner, records, &PRODUCTION_CALLABLE_LIMITS)
        .expect("selected overload publication")
}

fn ambiguous_publication() -> EnvironmentCallablePublication {
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
    EnvironmentCallablePublication::try_new(owner, records, &PRODUCTION_CALLABLE_LIMITS)
        .expect("ambiguous publication")
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

fn test_limits(max_query_work: u64) -> CallableLimits {
    CallableLimits::for_test(32, 16, 128, 32, 256, 256, 128, 1_048_576, max_query_work)
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
