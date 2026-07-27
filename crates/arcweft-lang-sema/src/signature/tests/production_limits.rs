use std::{fmt::Write as _, sync::Arc, sync::atomic::AtomicBool};

use arcweft_lang_syntax::{
    expr::MAX_NESTED_CALLS,
    parser::{ParseOptions, parse_document_with_source},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use crate::{
    callable::{
        CallableGroupIndex, CallableGroupKind, CallableName, CallableParameter,
        CallableParameterGroup, CallableParameterIndex, CallableParameterPassing,
        CallableParameterPresence, CallableParameterType, CallableSchemaError,
        PRODUCTION_CALLABLE_LIMITS, PRODUCTION_SIGNATURE_LIMITS, SignatureAccountingError,
        SignatureLimitExceeded, SignatureLimitKind, SignatureQueryWorkMeter, SignatureWorkKind,
    },
    env::TypeCheckEnv,
    registration::ProjectRegistrationFacts,
    test_support::character_project::{register, root_project_source},
    types::TypeKind,
};

use super::{
    SignatureFixture, SignatureQuery, SignatureQueryControl, SignatureQueryError,
    SignatureQueryOutcome, no_parameter_schema, publication, schema_with_parameters, unique_offset,
};

const SOURCE_LIMIT_BASE: &str = r"
fn id(value: i32) -> i32 { value }
fn main() -> Unit {
    id(1i32)
    ()
}
";

#[test]
fn production_candidate_call_limit_accepts_exact_and_rejects_one_over() {
    let exact = SignatureFixture::new(&sibling_call_source(4_096));
    let SignatureQueryOutcome::Help(help) = exact
        .query_in("id(999999i32)", "999999i32")
        .expect("exact production candidate-call boundary")
    else {
        panic!("the final sibling call must produce signature help")
    };
    assert_eq!(help.query_work().search().candidate_calls(), 4_096);

    let one_over = SignatureFixture::new(&sibling_call_source(4_097));
    assert!(matches!(
        one_over.query_in("id(999999i32)", "999999i32"),
        Err(SignatureQueryError::LimitExceeded(SignatureLimitExceeded {
            kind: SignatureLimitKind::CandidateCalls,
            observed: 4_097,
            maximum: 4_096,
        }))
    ));
}

#[test]
fn accepted_syntax_nesting_maximum_is_below_the_defensive_query_limit() {
    assert_eq!(MAX_NESTED_CALLS, 32);
    assert_eq!(PRODUCTION_SIGNATURE_LIMITS.nested_calls(), 64);

    let exact_source = low_overload_nested_call_source(MAX_NESTED_CALLS);
    let exact = SignatureFixture::new(&exact_source);
    let SignatureQueryOutcome::Help(help) = exact
        .query_in("id(1i32)", "1i32")
        .expect("all accepted nested call surfaces fit the production query budgets")
    else {
        panic!("the innermost accepted project call must produce help")
    };
    assert_eq!(help.query_work().search().candidate_calls(), 32);
    assert_eq!(help.query_work().search().nested_calls(), 32);

    let expression = format!(
        "{}\"stop\"{}",
        "panic(".repeat(MAX_NESTED_CALLS),
        ")".repeat(MAX_NESTED_CALLS)
    );
    let semantic_depth = SignatureFixture::new(&format!(
        "fn main() -> Unit {{\n    {expression}\n    ()\n}}\n"
    ));
    let outer_close =
        unique_offset(semantic_depth.document.text(), &expression) + expression.len() - 1;
    let semantic_result = semantic_depth.query(outer_close);
    let Ok(SignatureQueryOutcome::Help(semantic_help)) = semantic_result else {
        panic!("{semantic_result:?}")
    };
    let exact_calls = u64::try_from(MAX_NESTED_CALLS).expect("accepted nesting fits u64");
    assert_eq!(semantic_help.work().argument_mapping(), exact_calls);
    assert_eq!(semantic_help.work().type_checks(), exact_calls);
    assert_eq!(
        semantic_help.query_work().resolution().argument_bindings(),
        exact_calls
    );
    assert_eq!(
        semantic_help.query_work().resolution().specificity_checks(),
        exact_calls
    );
    assert_eq!(
        semantic_help.query_work().projection().overloads(),
        exact_calls
    );
    assert!(
        semantic_help
            .work()
            .total_work()
            .expect("semantic work total")
            <= PRODUCTION_CALLABLE_LIMITS.max_query_work()
    );

    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://sema/signature/nested-call-limit.arcw")
                .expect("test document ID"),
            SourceName::Generated,
            low_overload_nested_call_source(MAX_NESTED_CALLS + 1),
        )
        .expect("test source document"),
    );
    let parsed = parse_document_with_source(document, ParseOptions::default());
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.message().contains("inclusive limit of 32")),
        "{:?}",
        parsed.errors()
    );
}

#[test]
fn production_recovery_limit_accepts_exact_and_rejects_one_over() {
    let exact = SignatureFixture::recovered(&recovered_call_source(512));
    let SignatureQueryOutcome::Help(help) = exact
        .query_in("target(999999i32)", "999999i32")
        .expect("exact production recovery boundary")
    else {
        panic!("the valid target after recovered calls must produce help")
    };
    assert_eq!(help.query_work().search().recovery_nodes(), 512);

    let one_over = SignatureFixture::recovered(&recovered_call_source(513));
    assert!(matches!(
        one_over.query_in("target(999999i32)", "999999i32"),
        Err(SignatureQueryError::LimitExceeded(SignatureLimitExceeded {
            kind: SignatureLimitKind::RecoveryNodes,
            observed: 513,
            maximum: 512,
        }))
    ));
}

#[test]
fn production_source_byte_limit_accepts_exact_and_rejects_one_over_first() {
    let maximum = usize::try_from(PRODUCTION_SIGNATURE_LIMITS.source_bytes())
        .expect("production source bound fits usize");
    let exact_source = padded_source(maximum);
    let (document, project, world_id) =
        root_project_source("signature-production-source-limit", &exact_source);
    let facts = ProjectRegistrationFacts::try_new(
        world_id,
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("manifest-free registration facts");
    let world = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("manifest-free accepted world");
    let exact = SignatureFixture {
        document,
        project,
        world,
    };
    let cursor = unique_offset(exact.document.text(), "1i32");
    assert!(matches!(
        exact.query(cursor),
        Ok(SignatureQueryOutcome::Help(_))
    ));

    let oversized = SourceDocument::try_new(
        SourceDocumentId::try_new("signature-production-source-one-over").expect("document id"),
        SourceName::Memory,
        padded_source(maximum + 1),
    )
    .expect("one-over source document");
    let cancelled = AtomicBool::new(false);
    let hir = exact.project.linked_module();
    assert!(matches!(
        SignatureQuery::production(
            &exact.world,
            &oversized,
            &hir,
            cursor,
            SignatureQueryControl::new(&cancelled, None),
        ),
        Err(SignatureQueryError::LimitExceeded(SignatureLimitExceeded {
            kind: SignatureLimitKind::SourceBytes,
            observed: 8_388_609,
            maximum: 8_388_608,
        }))
    ));
}

#[test]
fn production_work_limit_accepts_exact_and_rejects_next_operation() {
    let mut meter = SignatureQueryWorkMeter::new(PRODUCTION_SIGNATURE_LIMITS);
    meter
        .charge(
            SignatureWorkKind::NodeVisits,
            PRODUCTION_SIGNATURE_LIMITS.work_units(),
        )
        .expect("exact production work boundary");
    assert_eq!(meter.report().total_work(), 262_144);
    assert_eq!(
        meter.charge(SignatureWorkKind::NodeVisits, 1),
        Err(SignatureAccountingError::Limit(SignatureLimitExceeded {
            kind: SignatureLimitKind::WorkUnits,
            observed: 262_145,
            maximum: 262_144,
        }))
    );
    assert_eq!(meter.report().total_work(), 262_144);
}

#[test]
fn accepted_catalog_maximum_projects_thirty_two_overloads() {
    let schemas = (0..PRODUCTION_CALLABLE_LIMITS.max_overloads_per_key())
        .map(|_| no_parameter_schema(TypeKind::String));
    let fixture = SignatureFixture::with_publication(
        "fn main() -> Unit { many()\n() }",
        publication("adapter.signature-production-overloads", "many", schemas),
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("many()", ")")
        .expect("accepted catalog maximum signature query")
    else {
        panic!("the accepted overload set must produce signature help")
    };
    assert_eq!(help.signatures().len(), 32);
    assert_eq!(help.query_work().projection().overloads(), 32);
}

#[test]
fn production_parameter_limit_projects_exact_and_rejects_one_over_schema() {
    let fixture = SignatureFixture::with_publication(
        "fn main() -> Unit { wide()\n() }",
        publication(
            "adapter.signature-production-parameters",
            "wide",
            [schema_with_parameters(parameters(128), TypeKind::String)],
        ),
    );
    let SignatureQueryOutcome::Help(help) = fixture
        .query_in("wide()", ")")
        .expect("exact production parameter boundary")
    else {
        panic!("the exact-width signature must produce help")
    };
    assert_eq!(help.signatures()[0].groups()[0].parameters().len(), 128);
    assert_eq!(help.query_work().projection().parameters(), 128);

    assert_eq!(
        CallableParameterGroup::try_new(
            CallableGroupIndex::ZERO,
            CallableGroupKind::Initial,
            parameters(129),
            &PRODUCTION_CALLABLE_LIMITS,
        ),
        Err(CallableSchemaError::ParameterLimit {
            actual: 129,
            limit: 128,
        })
    );
}

fn sibling_call_source(count: usize) -> String {
    assert!(count > 0);
    let mut source = String::from("fn id(value: i32) -> i32 { value }\nfn main() -> Unit {\n");
    for _ in 1..count {
        writeln!(source, "    id(0i32)").expect("write sibling call");
    }
    source.push_str("    id(999999i32)\n    ()\n}\n");
    source
}

fn recovered_call_source(count: usize) -> String {
    let mut source = String::from(
        "fn recover(value: i32) -> i32 { value }\n\
         fn target(value: i32) -> i32 { value }\n\
         fn main() -> Unit {\n",
    );
    for _ in 0..count {
        source.push_str("    recover(@@@)\n");
    }
    source.push_str("    target(999999i32)\n    ()\n}\n");
    source
}

fn padded_source(length: usize) -> String {
    assert!(length >= SOURCE_LIMIT_BASE.len() + 2);
    let mut source = String::with_capacity(length);
    source.push_str(SOURCE_LIMIT_BASE);
    source.push_str("//");
    source.extend(std::iter::repeat_n('x', length - source.len()));
    assert_eq!(source.len(), length);
    source
}

fn low_overload_nested_call_source(call_count: usize) -> String {
    assert!(call_count > 0);
    let mut expression = "id(1i32)".to_owned();
    for _ in 1..call_count {
        expression = format!("unknown({expression})");
    }
    format!(
        "fn id(value: i32) -> i32 {{ value }}\n\
         fn main() -> Unit {{\n    let unknown: i32 = 0i32\n    let value: i32 = {expression}\n    ()\n}}\n"
    )
}

fn parameters(count: usize) -> Vec<CallableParameter> {
    (0..count)
        .map(|index| {
            CallableParameter::try_new(
                CallableParameterIndex::try_from_usize(index).expect("parameter index"),
                Some(
                    CallableName::try_new(format!("value{index}")).expect("unique parameter name"),
                ),
                CallableParameterType::Exact(TypeKind::I32),
                CallableParameterPassing::PositionalOrNamed,
                CallableParameterPresence::Required,
                None,
                None,
            )
            .expect("production parameter")
        })
        .collect()
}
