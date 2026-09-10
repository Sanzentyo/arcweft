use crate::{
    callable::CheckedContentRole,
    checked_rich_text::{
        CheckedAttachedContentArgument, CheckedContentEmission, CheckedContentInsertion,
        CheckedDialogueToken, CheckedRichTextReport,
    },
    final_analysis::{
        CheckedCompileTimeScalar, CheckedExpressionExecution, CheckedExpressionResolution,
        CheckedExpressionResult, CheckedNonValueExpressionResult, CheckedRuntimeValueDisposition,
        CheckedTextProxyApplication, CheckedTextProxyValueOrigin, CompileTimeScalarReductionError,
        FinalSemanticAnalysis, FinalSemanticAnalysisError, TextProxyDeclarationDiagnosticCause,
        TextProxyDefaultExpectation, TextProxyMetadataRole,
    },
};

use super::{analyze, fixture};

fn object_insertions(report: &FinalSemanticAnalysis) -> Vec<&CheckedContentInsertion> {
    fn visit<'a>(
        report: &'a CheckedRichTextReport,
        insertions: &mut Vec<&'a CheckedContentInsertion>,
    ) {
        for token in report.content().tokens() {
            let CheckedDialogueToken::ContentInsert(insertion) = token else {
                continue;
            };
            if matches!(insertion.emission(), CheckedContentEmission::ObjectSpan(_)) {
                insertions.push(insertion);
            }
            if let CheckedAttachedContentArgument::Present {
                checked_content, ..
            } = insertion.argument()
            {
                visit(checked_content, insertions);
            }
        }
    }

    let mut insertions = Vec::new();
    for (_, expression) in report.expressions() {
        if let CheckedExpressionResolution::DialogueApplication { rich_text, .. } =
            expression.resolution()
        {
            visit(rich_text, &mut insertions);
        }
    }
    insertions
}

fn object_applications(report: &FinalSemanticAnalysis) -> Vec<&CheckedTextProxyApplication> {
    object_insertions(report)
        .into_iter()
        .filter_map(|insertion| match insertion.emission() {
            CheckedContentEmission::ObjectSpan(application) => Some(application),
            CheckedContentEmission::Modifier(_)
            | CheckedContentEmission::Fx(_)
            | CheckedContentEmission::Ruby(_)
            | CheckedContentEmission::Raw(_)
            | CheckedContentEmission::ContentResult => None,
        })
        .collect()
}

fn object_application(report: &FinalSemanticAnalysis) -> &CheckedTextProxyApplication {
    let applications = object_applications(report);
    if applications.len() != 1 {
        panic!("expected exactly one generic Object application");
    }
    applications
        .into_iter()
        .next()
        .expect("the length was checked above")
}

#[test]
fn generic_object_emits_one_typed_object_span_with_defaults_and_overrides() {
    let fixture = fixture(
        r#"
#[text_proxy(role = "keyword", hit_test = true, channel = "fallback")]
pub struct KeywordHit {
    channel: String
    weight: Option<i64>
}

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = KeywordHit, weight = 3)[typed]]
    return "done"
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("typed explicit object is accepted");
    assert!(
        report.diagnostics().is_empty(),
        "{:#?}",
        report.diagnostics()
    );

    let insertions = object_insertions(&report);
    let [insertion] = insertions.as_slice() else {
        panic!("one object insertion");
    };
    assert_eq!(insertion.argument().role(), Some(CheckedContentRole::Rich));
    assert!(insertion.argument().checked_content().is_some());
    let object_expression = report
        .expression(insertion.site().raw())
        .expect("Object application expression");
    assert_eq!(object_expression.value_type(), None);
    let application = object_application(&report);
    let call = report
        .call(insertion.site().raw())
        .and_then(|facts| facts.selected_application())
        .expect("the insertion owns the selected Object call");
    let identity = call
        .result()
        .content_emission()
        .expect("the selected Object call retains its exact identity");
    assert_eq!(
        object_expression.result(),
        &CheckedExpressionResult::NonValue(CheckedNonValueExpressionResult::ContentEmission(
            identity,
        ))
    );
    assert_eq!(
        application
            .metadata()
            .role()
            .map(|value| value.value().as_str()),
        Some("keyword")
    );
    assert!(application.metadata().hit_test().value());
    assert_eq!(application.id().value().as_str(), "hotspot");
    let view = report
        .checked_text_proxies()
        .application(application)
        .expect("application joins its exact catalog definition");
    assert_eq!(view.definition().diagnostic_name(), "KeywordHit");
    assert_eq!(application.definition_id(), view.definition().id());
    assert_eq!(
        application.definition_digest(),
        view.definition().digest().expect("definition digest")
    );
    let call_digest = application.call_application_digest();
    assert_eq!(call.digest(), call_digest);
    assert!(matches!(
        application.id().origin(),
        CheckedTextProxyValueOrigin::Inline { application, .. }
            if *application == call_digest
    ));
    assert!(application.id().origin().coordinate().is_some());

    let [channel, weight] = application.fields() else {
        panic!("declaration-order fields");
    };
    assert!(matches!(
        channel.origin(),
        CheckedTextProxyValueOrigin::AttributeDefault(_)
    ));
    assert!(channel.origin().coordinate().is_some());
    assert!(matches!(
        channel.value(),
        Some(CheckedCompileTimeScalar::Text(value)) if value == "fallback"
    ));
    assert!(matches!(
        weight.origin(),
        CheckedTextProxyValueOrigin::Inline { application, .. }
            if *application == call_digest
    ));
    assert!(weight.origin().coordinate().is_some());
    assert!(matches!(
        weight.value(),
        Some(CheckedCompileTimeScalar::Int(3))
    ));
}

#[test]
fn runtime_disposition_omits_object_span_application_result() {
    let fixture = fixture(
        r#"
#[text_proxy]
pub struct KeywordHit { channel: Option<String> }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = KeywordHit)[typed]]
    return "done"
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("typed Object analysis");
    let authority = report.execution_projection();
    let insertion = object_insertions(&report)
        .into_iter()
        .next()
        .expect("one Object insertion");
    assert_eq!(
        authority
            .expression(insertion.site().raw())
            .expect("Object runtime execution projection"),
        CheckedExpressionExecution::Structural {
            value: CheckedRuntimeValueDisposition::Omit,
        }
    );
}

#[test]
fn generic_object_requires_an_attached_rich_body() {
    let fixture = fixture(
        r#"
#[text_proxy]
pub struct KeywordHit { channel: Option<String> }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = KeywordHit)]
    return "done"
}
"#,
        None,
    );
    assert!(
        analyze(&fixture).is_err(),
        "Object without a body is not a value call"
    );
}

#[test]
fn object_requires_definition_backed_schema() {
    let fixture = fixture(
        r#"
pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, role = keyword, hit_test = true)[explicit]]
    return "done"
}
"#,
        None,
    );
    // The exact Object grammar requires both `id` and `type`; this malformed
    // metadata-only spelling is rejected while lowering, before sema can
    // publish a call or a compatibility success path.
    assert!(fixture.project.analysis_view().is_err());
}

#[test]
fn generic_object_requires_exact_id_and_type_arguments() {
    let missing_id = fixture(
        r#"
#[text_proxy]
pub struct KeywordHit { channel: Option<String> }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(type = KeywordHit)[missing]]
    return "done"
}
"#,
        None,
    );
    assert!(missing_id.project.analysis_view().is_ok());
    assert!(
        analyze(&missing_id).is_err(),
        "the sema Object owner must reject a clean-HIR call without id"
    );

    let missing_type = fixture(
        r#"
#[text_proxy]
pub struct KeywordHit { channel: Option<String> }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot)[missing]]
    return "done"
}
"#,
        None,
    );
    assert!(missing_type.project.analysis_view().is_err());
}

#[test]
fn text_proxy_declarations_cannot_redeclare_the_id_coordinate() {
    let fixture = fixture(
        r#"
#[text_proxy]
pub struct InvalidProxy { id: String }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = InvalidProxy)[invalid]]
    return "done"
}
"#,
        None,
    );
    assert!(analyze(&fixture).is_err());
}

#[test]
fn unknown_or_unmarked_object_types_are_rejected() {
    let unmarked = fixture(
        r#"
pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = String)[unmarked]]
    return "done"
}
"#,
        None,
    );
    assert!(analyze(&unmarked).is_err());

    let unknown = fixture(
        r#"
pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = MissingProxy)[unknown]]
    return "done"
}
"#,
        None,
    );
    assert!(analyze(&unknown).is_err());
}

#[test]
fn required_and_unknown_custom_fields_are_rejected_by_the_dependent_schema() {
    let missing = fixture(
        r#"
#[text_proxy(role = "keyword")]
pub struct KeywordHit { channel: String }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = KeywordHit)[missing]]
    return "done"
}
"#,
        None,
    );
    assert!(analyze(&missing).is_err());

    let unknown = fixture(
        r#"
#[text_proxy(role = "keyword")]
pub struct KeywordHit { channel: Option<String> }

pub character alice { display = "Alice" }
flow main() -> String {
        alice[#object(id = @.hotspot, type = KeywordHit, channel = "first", typo = "third")[invalid]]
    return "done"
}
"#,
        None,
    );
    assert!(unknown.project.analysis_view().is_ok());
    assert!(analyze(&unknown).is_err());
}

#[test]
fn duplicate_object_named_argument_is_rejected_during_hir_recovery() {
    let duplicate = fixture(
        r#"
#[text_proxy(role = "keyword")]
pub struct KeywordHit { channel: Option<String> }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = KeywordHit, channel = "first", channel = "second")[invalid]]
    return "done"
}
"#,
        None,
    );
    assert!(
        duplicate.project.analysis_view().is_err(),
        "duplicate Object named arguments must remain HIR recovery"
    );
}

#[test]
fn optional_custom_fields_are_preserved_as_absent_values() {
    let fixture = fixture(
        r#"
#[text_proxy(role = "keyword")]
pub struct KeywordHit {
    channel: Option<String>
    weight: Option<i64>
}

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = KeywordHit)[optional]]
    return "done"
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("optional text-proxy fields may be absent");
    let application = object_application(&report);
    let [channel, weight] = application.fields() else {
        panic!("declaration-order fields");
    };
    assert_eq!(channel.origin(), &CheckedTextProxyValueOrigin::Absent);
    assert!(channel.value().is_none());
    assert_eq!(weight.origin(), &CheckedTextProxyValueOrigin::Absent);
    assert!(weight.value().is_none());
}

#[test]
fn equal_shape_proxy_declarations_keep_distinct_definition_identity() {
    let fixture = fixture(
        r#"
use crate.child.KeywordHit as ChildHit

#[text_proxy(role = "root")]
pub struct KeywordHit { channel: Option<String> }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = ChildHit, channel = "child")[qualified]]
    return "done"
}
"#,
        Some(
            r#"
#[rich_text_proxy(role = "child")]
pub struct KeywordHit { channel: Option<String> }
"#,
        ),
    );
    let report = analyze(&fixture).expect("qualified proxy type is accepted");
    assert_eq!(report.checked_text_proxies().definitions().count(), 2);
    let definitions = report
        .checked_text_proxies()
        .definitions()
        .collect::<Vec<_>>();
    assert_ne!(definitions[0].id(), definitions[1].id());
    assert_ne!(
        definitions[0].digest().expect("definition digest"),
        definitions[1].digest().expect("definition digest")
    );
    let application = object_application(&report);
    let view = report
        .checked_text_proxies()
        .application(application)
        .expect("application joins exact declaration");
    assert_eq!(
        view.definition().attribute(),
        crate::final_analysis::CheckedTextProxyAttributeFamily::RichTextProxy
    );
    assert!(
        view.definition()
            .declaration()
            .qualified_name()
            .contains("child")
    );
}

#[test]
fn referenced_invalid_declaration_default_remains_a_typed_diagnostic() {
    let fixture = fixture(
        r#"
#[text_proxy(depth = 2pt)]
pub struct KeywordHit { channel: Option<String> }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = KeywordHit)[invalid]]
    return "done"
}
"#,
        None,
    );
    let Err(FinalSemanticAnalysisError::InvalidTextProxyDeclarations { diagnostics }) =
        analyze(&fixture)
    else {
        panic!("referenced invalid proxy must retain its typed declaration diagnostic");
    };
    let [diagnostic] = diagnostics.as_ref() else {
        panic!("one typed declaration-default diagnostic");
    };
    assert_eq!(diagnostic.name(), Some("depth"));
    assert_eq!(
        diagnostic.expected(),
        &TextProxyDefaultExpectation::Metadata(TextProxyMetadataRole::Depth)
    );
    assert_eq!(
        diagnostic.cause(),
        &TextProxyDeclarationDiagnosticCause::Reduction(CompileTimeScalarReductionError::WrongUnit)
    );
}

#[test]
fn malformed_rich_text_inside_object_is_rejected_before_insertion() {
    let fixture = fixture(
        r#"
#[text_proxy]
pub struct KeywordHit { channel: Option<String> }

pub character alice { display = "Alice" }
flow main() -> String {
    alice[#object(id = @.hotspot, type = KeywordHit, depth = 2em)[invalid]]
    return "done"
}
"#,
        None,
    );
    assert!(
        fixture.project.analysis_view().is_err(),
        "invalid Object metadata unit must be rejected before semantic analysis"
    );
}

#[test]
fn object_application_digest_is_owner_stable_and_typed_atom_sensitive() {
    let build = |weight: i64| {
        let source = format!(
            r#"
#[text_proxy]
pub struct KeywordHit {{ weight: Option<i64> }}

pub character alice {{ display = "Alice" }}
flow main() -> String {{
    alice[#object(id = @.hotspot, type = KeywordHit, weight = {weight})[body]]
    return "done"
}}
"#
        );
        let fixture = fixture(&source, None);
        let report = analyze(&fixture).expect("typed Object application");
        object_application(&report).semantic_digest()
    };

    let stable = build(3);
    assert_eq!(
        stable,
        build(3),
        "the same checked owner inputs issue the same digest"
    );
    assert_ne!(
        stable,
        build(4),
        "a checked scalar field value belongs to the application identity"
    );
}
