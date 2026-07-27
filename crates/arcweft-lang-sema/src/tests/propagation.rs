use super::support::*;
use crate::propagation::{
    CheckedReturnType, PropagationBoundaryKind, PropagationTargetEvidence, TryPropagationOperand,
};
use arcweft_source::{DiagnosticLabelStyle, SourceRange};

fn typecheck_errors(
    label: &str,
    source: &str,
    environment: &TypeCheckEnv,
) -> Vec<crate::diagnostics::TypeCheckError> {
    let hir = lower_bound_hir(label, source);
    typecheck_hir(&hir, environment).expect_err("fixture must produce a propagation diagnostic")
}

fn source_range(source: &str, needle: &str) -> SourceRange {
    let start = source.find(needle).expect("source fixture contains needle");
    SourceRange::new(start, start + needle.len())
}

fn assert_diagnostic_ranges(
    error: &crate::diagnostics::TypeCheckError,
    primary: SourceRange,
    secondary: Option<SourceRange>,
) {
    let diagnostic = error.diagnostic();
    let primary_label = diagnostic
        .labels()
        .iter()
        .find(|label| label.style() == DiagnosticLabelStyle::Primary)
        .expect("structured propagation diagnostic has a primary label");
    assert_eq!(primary_label.span().range(), primary);
    let secondary_label = diagnostic
        .labels()
        .iter()
        .find(|label| label.style() == DiagnosticLabelStyle::Secondary);
    assert_eq!(secondary_label.map(|label| label.span().range()), secondary);
}

#[test]
fn propagation_bearing_hir_retains_its_exact_source_document() {
    let source = r"
fn accepted(value: Result<i64, String>) -> Result<i64, String> {
    let inner = value?
    Ok(inner)
}
";
    let parsed = parse_ok(source);
    let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
        .expect("exact source document lowers");
    assert_eq!(hir.source_identity(), Some(parsed.identity()));
    validate_typecheck_ready(&hir).expect("bound propagation is typecheck-ready");
}

#[test]
fn result_try_accepts_directional_error_compatibility_for_both_spellings() {
    for (label, expression) in [("prefix", "try value"), ("postfix", "value?")] {
        let source = format!(
            r"
fn accepted(value: Result<i64, String>) -> Result<i64, String | i64> {{
    let inner = {expression}
    Ok(inner)
}}
"
        );
        let hir = lower_bound_hir(&format!("accepted-{label}-try"), &source);
        typecheck_hir(&hir, &TypeCheckEnv::new())
            .unwrap_or_else(|errors| panic!("{label} Try must typecheck: {errors:#?}"));
    }
}

#[test]
fn function_mismatch_keeps_the_exact_document_result_boundary() {
    let source = "fn demo(value: Result<i64, String>) -> Result<i64, i64> {\n    let 前 = try value\n    Ok(前)\n}\n";
    let errors = typecheck_errors("function-result-boundary", source, &TypeCheckEnv::new());
    let error = errors
        .iter()
        .find(|error| error.stable_code() == "sema.try.error_mismatch")
        .expect("typed Try mismatch diagnostic");

    assert_diagnostic_ranges(
        error,
        SourceRange::new(72, 75),
        Some(SourceRange::new(39, 55)),
    );
}

#[test]
fn result_try_uses_generic_identity_and_concrete_call_substitution() {
    let source = r"
fn generic<E>(value: Result<i64, E>) -> Result<i64, E> {
    let inner = value?
    Ok(inner)
}

fn concrete(value: Result<i64, String>) -> Result<i64, String> {
    let inner = generic(value)?
    Ok(inner)
}
";
    typecheck_registered_source("generic-try-propagation", source, TypeCheckEnv::new())
        .unwrap_or_else(|errors| {
            panic!("generic Try must typecheck after substitution: {errors:#?}")
        });

    let mismatch = r"
fn mismatch<Expected, Actual>(value: Result<i64, Actual>) -> Result<i64, Expected> {
    let inner = value?
    Ok(inner)
}
";
    let errors = typecheck_errors("generic-try-mismatch", mismatch, &TypeCheckEnv::new());
    let error = errors
        .iter()
        .find(|error| error.stable_code() == "sema.try.error_mismatch")
        .expect("different generic identities mismatch");
    assert!(matches!(
        error.kind(),
        TypeCheckErrorKind::TryErrorMismatch {
            expected: TypeKind::GenericParam(expected),
            actual: TypeKind::GenericParam(actual),
            ..
        } if expected != actual
    ));
}

#[test]
fn option_try_accepts_only_an_option_boundary() {
    let source = r"
fn accepted(maybe: Option<i64>) -> Option<i64> {
    let inner = maybe?
    Some(inner)
}
";
    let hir = lower_bound_hir("option-try-propagation", source);
    typecheck_hir(&hir, &TypeCheckEnv::new())
        .unwrap_or_else(|errors| panic!("Option Try must typecheck: {errors:#?}"));
}

#[test]
fn only_an_explicit_typed_conversion_changes_the_propagated_error() {
    let source = r"
fn converted(value: Result<i64, String>) -> Result<i64, i64> {
    let inner = value.context()?
    Ok(inner)
}
";
    let input = TypeKind::Result {
        ok: Box::new(TypeKind::I64),
        error: Box::new(TypeKind::String),
    };
    let converted = TypeKind::Result {
        ok: Box::new(TypeKind::I64),
        error: Box::new(TypeKind::I64),
    };
    let environment = TypeCheckEnv::new().with_method(input, "context", converted);
    let hir = lower_bound_hir("explicit-try-conversion", source);
    typecheck_hir(&hir, &environment)
        .unwrap_or_else(|errors| panic!("explicit conversion must typecheck: {errors:#?}"));

    let unchanged = source.replace("value.context()?", "value?");
    let errors = typecheck_errors(
        "implicit-try-conversion-prohibited",
        &unchanged,
        &environment,
    );
    assert!(
        errors
            .iter()
            .any(|error| error.stable_code() == "sema.try.error_mismatch")
    );
}

#[test]
fn nested_postfix_try_checks_each_operator_independently() {
    let source = r"
fn nested(value: Result<Result<i64, String>, i64>) -> Result<i64, String> {
    let inner = value??
    Ok(inner)
}
";
    let errors = typecheck_errors("nested-try-propagation", source, &TypeCheckEnv::new());
    let mismatches = errors
        .iter()
        .filter(|error| error.stable_code() == "sema.try.error_mismatch")
        .collect::<Vec<_>>();
    assert_eq!(mismatches.len(), 1);
    let first_question = source.find("??").expect("nested operators");
    assert_diagnostic_ranges(
        mismatches[0],
        SourceRange::new(first_question, first_question + 1),
        Some({
            let first = source_range(source, "Result<i64, String>");
            let start = source[first.end()..]
                .find("Result<i64, String>")
                .expect("return Result")
                + first.end();
            SourceRange::new(start, start + "Result<i64, String>".len())
        }),
    );
}

#[test]
fn result_try_mismatch_retains_typed_boundary_and_smallest_operator_span() {
    for (label, expression, operator) in
        [("prefix", "try value", "try"), ("postfix", "value?", "?")]
    {
        let source = format!(
            r"
fn mismatch(value: Result<i64, String>) -> Result<i64, i64> {{
    let inner = {expression}
    Ok(inner)
}}
"
        );
        let errors = typecheck_errors(
            &format!("{label}-try-mismatch"),
            &source,
            &TypeCheckEnv::new(),
        );
        let error = errors
            .iter()
            .find(|error| error.stable_code() == "sema.try.error_mismatch")
            .expect("typed Try mismatch diagnostic");
        let TypeCheckErrorKind::TryErrorMismatch {
            expected,
            actual,
            boundary,
            ..
        } = error.kind()
        else {
            panic!("unexpected diagnostic payload: {error:#?}");
        };
        assert_eq!(expected, &TypeKind::I64);
        assert_eq!(actual, &TypeKind::String);
        assert_eq!(boundary.kind(), PropagationBoundaryKind::Function);
        assert!(matches!(
            boundary.checked_return(),
            CheckedReturnType::Known(TypeKind::Result { error, .. })
                if error.as_ref() == &TypeKind::I64
        ));
        let operator_start = source.find(expression).expect("expression exists")
            + expression
                .find(operator)
                .expect("operator exists in expression");
        assert_diagnostic_ranges(
            error,
            SourceRange::new(operator_start, operator_start + operator.len()),
            Some(source_range(&source, "Result<i64, i64>")),
        );
    }
}

#[test]
fn wrong_try_envelopes_are_target_missing_not_error_mismatches() {
    let cases = [
        (
            "option-to-result",
            "maybe?",
            "maybe: Option<i64>",
            "Result<i64, String>",
            TryPropagationOperand::Option,
        ),
        (
            "result-to-option",
            "value?",
            "value: Result<i64, String>",
            "Option<i64>",
            TryPropagationOperand::Result {
                actual_error: TypeKind::String,
            },
        ),
    ];
    for (label, expression, parameter, result, operand) in cases {
        let source = format!(
            r"
fn wrong({parameter}) -> {result} {{
    let inner = {expression}
    None
}}
"
        );
        let errors = typecheck_errors(label, &source, &TypeCheckEnv::new());
        let error = errors
            .iter()
            .find(|error| error.stable_code() == "sema.try.propagation_target_missing")
            .expect("typed Try target-missing diagnostic");
        assert!(matches!(
            error.kind(),
            TypeCheckErrorKind::TryPropagationTargetMissing {
                operand: actual,
                target: Some(PropagationTargetEvidence::Boundary(_)),
                ..
            } if actual == &operand
        ));
        assert!(
            !errors
                .iter()
                .any(|error| error.stable_code() == "sema.try.error_mismatch")
        );
    }
}

#[test]
fn non_propagatable_try_operand_uses_the_ordinary_operand_error() {
    let source = r"
fn wrong() -> Result<i64, String> {
    let value = 42i64?
    Ok(value)
}
";
    let errors = typecheck_errors("non-propagatable-try", source, &TypeCheckEnv::new());
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("requires Result<T, E> or Option<T>")
    }));
    assert!(!errors.iter().any(|error| matches!(
        error.kind(),
        TypeCheckErrorKind::TryPropagationTargetMissing { .. }
            | TypeCheckErrorKind::TryErrorMismatch { .. }
    )));
}

#[test]
fn propagating_await_uses_result_boundary_and_preserving_await_does_not() {
    let accepted = r"
fn accepted() -> Result<i64, String> {
    let value = try await load()
    Ok(value)
}

fn preserved() -> Result<i64, String> {
    await load()
}
";
    let environment = TypeCheckEnv::new().with_function(
        "load",
        TypeKind::Need {
            ready: Box::new(TypeKind::I64),
            error: Box::new(TypeKind::String),
        },
    );
    let hir = lower_bound_hir("accepted-await-propagation", accepted);
    typecheck_hir(&hir, &environment)
        .unwrap_or_else(|errors| panic!("matching/preserving Await must typecheck: {errors:#?}"));

    for (label, expression, operator) in [
        ("prefix", "try await load()", "try"),
        ("attached", "await? load()", "?"),
    ] {
        let source = format!(
            r"
fn mismatch() -> Result<i64, i64> {{
    let value = {expression}
    Ok(value)
}}
"
        );
        let errors = typecheck_errors(&format!("{label}-await-mismatch"), &source, &environment);
        let error = errors
            .iter()
            .find(|error| error.stable_code() == "sema.await.error_mismatch")
            .expect("typed Await mismatch diagnostic");
        assert!(matches!(
            error.kind(),
            TypeCheckErrorKind::AwaitErrorMismatch {
                expected: TypeKind::I64,
                actual: TypeKind::String,
                boundary,
                ..
            } if boundary.kind() == PropagationBoundaryKind::Function
        ));
        let operator_start = source.find(expression).expect("expression exists")
            + expression
                .find(operator)
                .expect("operator exists in expression");
        assert_diagnostic_ranges(
            error,
            SourceRange::new(operator_start, operator_start + operator.len()),
            Some(source_range(&source, "Result<i64, i64>")),
        );
    }
}

#[test]
fn propagating_await_reports_exact_target_missing_evidence_for_both_spellings() {
    let environment = TypeCheckEnv::new().with_function(
        "load",
        TypeKind::Need {
            ready: Box::new(TypeKind::I64),
            error: Box::new(TypeKind::String),
        },
    );
    for (label, expression, operator) in [
        ("prefix", "try await load()", "try"),
        ("attached", "await? load()", "?"),
    ] {
        let source = format!(
            r"
fn wrong() -> Unit {{
    let value = {expression}
}}
"
        );
        let errors = typecheck_errors(
            &format!("{label}-await-target-missing"),
            &source,
            &environment,
        );
        let error = errors
            .iter()
            .find(|error| error.stable_code() == "sema.await.propagation_target_missing")
            .expect("typed Await target-missing diagnostic");
        assert!(matches!(
            error.kind(),
            TypeCheckErrorKind::AwaitPropagationTargetMissing {
                actual_error: TypeKind::String,
                target: Some(PropagationTargetEvidence::Boundary(boundary)),
                ..
            } if boundary.kind() == PropagationBoundaryKind::Function
                && boundary.checked_return() == &CheckedReturnType::Known(TypeKind::Unit)
        ));
        let operator_start = source.find(expression).expect("expression exists")
            + expression.find(operator).expect("operator exists");
        assert_diagnostic_ranges(
            error,
            SourceRange::new(operator_start, operator_start + operator.len()),
            Some(source_range(&source, "Unit")),
        );
    }
}

#[test]
fn default_expression_propagation_has_an_empty_boundary_stack() {
    let try_source = r"
#[fx]
fn wrong(value: i64 = input?) -> Unit {}
";
    let try_errors = typecheck_errors(
        "default-try-without-boundary",
        try_source,
        &TypeCheckEnv::new().with_symbol(
            "input",
            TypeKind::Result {
                ok: Box::new(TypeKind::I64),
                error: Box::new(TypeKind::String),
            },
        ),
    );
    let try_error = try_errors
        .iter()
        .find(|error| error.stable_code() == "sema.try.propagation_target_missing")
        .expect("default Try has no callable boundary");
    assert!(matches!(
        try_error.kind(),
        TypeCheckErrorKind::TryPropagationTargetMissing { target: None, .. }
    ));
    assert_diagnostic_ranges(try_error, source_range(try_source, "?"), None);

    for (label, expression, operator) in [
        ("prefix", "try await input", "try"),
        ("attached", "await? input", "?"),
    ] {
        let source = format!(
            r"
#[fx]
fn wrong(value: i64 = {expression}) -> Unit {{}}
"
        );
        let errors = typecheck_errors(
            &format!("default-{label}-await-without-boundary"),
            &source,
            &TypeCheckEnv::new().with_symbol(
                "input",
                TypeKind::Need {
                    ready: Box::new(TypeKind::I64),
                    error: Box::new(TypeKind::String),
                },
            ),
        );
        let error = errors
            .iter()
            .find(|error| error.stable_code() == "sema.await.propagation_target_missing")
            .expect("default Await has no callable boundary");
        assert!(matches!(
            error.kind(),
            TypeCheckErrorKind::AwaitPropagationTargetMissing { target: None, .. }
        ));
        let operator_start = source.find(expression).expect("expression exists")
            + expression.find(operator).expect("operator exists");
        assert_diagnostic_ranges(
            error,
            SourceRange::new(operator_start, operator_start + operator.len()),
            None,
        );
    }
}

#[test]
fn ordinary_function_uses_the_declared_boundary_model() {
    let source = r"
fn wrong(value: Result<i64, String>) -> Result<i64, i64> {
    let inner = value?
    Ok(inner)
}
";
    let errors = typecheck_registered_source(
        "ordinary-function-propagation-boundary",
        source,
        TypeCheckEnv::new(),
    )
    .expect_err("mismatched error types must reject");
    let error = errors
        .iter()
        .find(|error| error.stable_code() == "sema.try.error_mismatch")
        .expect("typed mismatch is retained");
    assert!(matches!(
        error.kind(),
        TypeCheckErrorKind::TryErrorMismatch { boundary, .. }
            if boundary.kind() == PropagationBoundaryKind::Function
                && boundary.declaration().is_some()
    ));
    assert_diagnostic_ranges(
        error,
        source_range(source, "?"),
        Some(source_range(source, "Result<i64, i64>")),
    );
}

#[test]
fn declared_flow_and_method_boundaries_keep_exact_result_sources() {
    let cases = [
        (
            "flow",
            r"
flow @flow.wrong wrong(value: Result<i64, String>) -> Result<i64, i64> {
    let inner = value?
    return Ok(inner)
}
",
            PropagationBoundaryKind::Flow,
        ),
        (
            "method",
            r"
struct Owner {}

impl Owner {
    fn wrong(value: Result<i64, String>) -> Result<i64, i64> {
        let inner = value?
        Ok(inner)
    }
}
",
            PropagationBoundaryKind::Method,
        ),
    ];
    for (label, source, kind) in cases {
        let errors = typecheck_errors(label, source, &TypeCheckEnv::new());
        let error = errors
            .iter()
            .find(|error| error.stable_code() == "sema.try.error_mismatch")
            .expect("typed mismatch is retained");
        assert!(matches!(
            error.kind(),
            TypeCheckErrorKind::TryErrorMismatch { boundary, .. }
                if boundary.kind() == kind
        ));
        assert_diagnostic_ranges(
            error,
            source_range(source, "?"),
            Some(source_range(source, "Result<i64, i64>")),
        );
    }
}

#[test]
fn expected_closure_return_supplies_a_known_inner_boundary() {
    let source = r"
fn outer(value: Result<i64, String>) -> Unit {
    let later: i64 -> Result<i64, String> = |_ignored: i64| Ok(value?)
}
";
    let hir = lower_bound_hir("expected-closure-propagation", source);
    typecheck_hir(&hir, &TypeCheckEnv::new())
        .unwrap_or_else(|errors| panic!("expected closure return must propagate: {errors:#?}"));
}

#[test]
fn ordinary_nested_blocks_do_not_replace_the_function_boundary() {
    let cases = [
        (
            "scope",
            r"
flow @flow.nested nested(value: Result<i64, String>) -> Result<i64, i64> {
    let scoped = scope {
        let inner = value?
        inner
    }
    return Ok(scoped)
}
",
            PropagationBoundaryKind::Flow,
        ),
        (
            "if",
            r"
flow @flow.if_nested nested(value: Result<i64, String>) -> Result<i64, i64> {
    let branched = if true {
        let inner = value?
        inner
    } else {
        0i64
    }
    return Ok(branched)
}
",
            PropagationBoundaryKind::Flow,
        ),
        (
            "loop",
            r"
flow @flow.loop_nested nested(value: Result<i64, String>) -> Result<i64, i64> {
    let repeated = loop {
        break value?
    }
    return Ok(repeated)
}
",
            PropagationBoundaryKind::Flow,
        ),
    ];
    for (label, source, boundary_kind) in cases {
        let errors = typecheck_errors(
            &format!("nested-{label}-propagation"),
            source,
            &TypeCheckEnv::new(),
        );
        let mismatches = errors
            .iter()
            .filter(|error| error.stable_code() == "sema.try.error_mismatch")
            .collect::<Vec<_>>();
        assert_eq!(mismatches.len(), 1, "{label}: {errors:#?}");
        assert!(matches!(
            mismatches[0].kind(),
            TypeCheckErrorKind::TryErrorMismatch { boundary, .. }
                if boundary.kind() == boundary_kind
        ));
    }
}

#[test]
fn generator_owners_stop_propagation_without_routing_to_their_error_type() {
    let cases = [
        (
            "generator-function",
            r"
fn values() -> Stream<i64, String> {
    let inner = input?
    yield inner
}
",
            TypeCheckEnv::new().with_symbol(
                "input",
                TypeKind::Result {
                    ok: Box::new(TypeKind::I64),
                    error: Box::new(TypeKind::String),
                },
            ),
        ),
        (
            "stream-block",
            r"
flow @flow.stream_outer outer(value: Result<i64, String>) -> Result<i64, String> {
    let values = stream {
        let inner = value?
        yield inner
    }
    return Ok(0i64)
}
",
            TypeCheckEnv::new(),
        ),
        (
            "seq-block",
            r"
flow @flow.seq_outer outer(value: Result<i64, String>) -> Result<i64, String> {
    let values = seq {
        let inner = value?
        yield inner
    }
    return Ok(0i64)
}
",
            TypeCheckEnv::new(),
        ),
        (
            "source-declaration",
            r"
source @source.values: Source<i64, String> {
    from value?
    backpressure = latest
    replay = event_only
    privacy = transient

    on item item => yield item
}
",
            TypeCheckEnv::new().with_symbol(
                "value",
                TypeKind::Result {
                    ok: Box::new(TypeKind::I64),
                    error: Box::new(TypeKind::String),
                },
            ),
        ),
    ];
    for (label, source, environment) in cases {
        let parsed = parse_source(source);
        assert!(parsed.errors().is_empty(), "{label}: {:?}", parsed.errors());
        let errors = typecheck_errors(label, source, &environment);
        let error = errors
            .iter()
            .find(|error| error.stable_code() == "sema.try.propagation_target_missing")
            .expect("generator barrier produces target-missing");
        assert!(matches!(
            error.kind(),
            TypeCheckErrorKind::TryPropagationTargetMissing {
                target: Some(PropagationTargetEvidence::GeneratorTerminal(_)),
                ..
            }
        ));
        assert!(
            error
                .diagnostic()
                .labels()
                .iter()
                .any(|label| label.style() == DiagnosticLabelStyle::Secondary)
        );
    }
}

#[test]
fn unresolved_types_do_not_create_a_propagation_cascade() {
    let source = r"
fn wrong() -> Result<i64, String> {
    let inner = missing?
    Ok(inner)
}
";
    let errors = typecheck_errors("unresolved-try-cascade", source, &TypeCheckEnv::new());
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("missing"))
    );
    assert!(!errors.iter().any(|error| matches!(
        error.kind(),
        TypeCheckErrorKind::TryPropagationTargetMissing { .. }
            | TypeCheckErrorKind::TryErrorMismatch { .. }
    )));
}

#[test]
fn utf8_before_postfix_try_keeps_the_one_byte_operator_primary() {
    let source = r#"
fn wrong(value: Result<i64, String>) -> Result<i64, i64> {
    let 日本語 = "前"
    let inner = value?
    Ok(inner)
}
"#;
    let errors = typecheck_errors("utf8-postfix-try-primary", source, &TypeCheckEnv::new());
    let error = errors
        .iter()
        .find(|error| error.stable_code() == "sema.try.error_mismatch")
        .expect("typed mismatch is retained");
    assert_diagnostic_ranges(
        error,
        source_range(source, "?"),
        Some(source_range(source, "Result<i64, i64>")),
    );
}

#[test]
fn inner_closure_boundary_is_never_skipped_for_an_outer_compatible_result() {
    let source = r"
fn outer(value: Result<i64, String>) -> Result<i64, String> {
    let inner = || -> Result<i64, i64> {
        let unwrapped = value?
        Ok(unwrapped)
    }
    Ok(0i64)
}
";
    let errors = typecheck_errors("closure-boundary-shadowing", source, &TypeCheckEnv::new());
    let error = errors
        .iter()
        .find(|error| error.stable_code() == "sema.try.error_mismatch")
        .expect("inner closure mismatch is reported");
    assert!(matches!(
        error.kind(),
        TypeCheckErrorKind::TryErrorMismatch { boundary, .. }
            if boundary.kind() == PropagationBoundaryKind::Closure
    ));
    assert_diagnostic_ranges(
        error,
        source_range(source, "?"),
        Some(source_range(source, "Result<i64, i64>")),
    );
}

#[test]
fn unconstrained_closure_stops_outer_propagation() {
    let closure_source = r"
fn outer(value: Result<i64, String>) -> Result<i64, String> {
    let inner = || value?
    Ok(0i64)
}
";
    let closure_errors = typecheck_errors(
        "unconstrained-closure-propagation",
        closure_source,
        &TypeCheckEnv::new(),
    );
    assert!(closure_errors.iter().any(|error| matches!(
        error.kind(),
        TypeCheckErrorKind::TryPropagationTargetMissing {
            target: Some(PropagationTargetEvidence::Boundary(boundary)),
            ..
        } if boundary.kind() == PropagationBoundaryKind::Closure
            && boundary.checked_return() == &CheckedReturnType::Unconstrained
    )));
}

#[test]
fn generator_terminal_stops_outer_propagation() {
    let generator_source = r"
fn outer(value: Result<i64, String>) -> Result<i64, String> {
    let values = || -> Seq<i64> {
        seq {
            let unwrapped = value?
            yield unwrapped
        }
    }
    Ok(0i64)
}
";
    let generator_errors = typecheck_errors(
        "generator-terminal-propagation",
        generator_source,
        &TypeCheckEnv::new(),
    );
    let error = generator_errors
        .iter()
        .find(|error| error.stable_code() == "sema.try.propagation_target_missing")
        .expect("generator terminal stops outer propagation");
    assert!(matches!(
        error.kind(),
        TypeCheckErrorKind::TryPropagationTargetMissing {
            target: Some(PropagationTargetEvidence::GeneratorTerminal(_)),
            ..
        }
    ));
    assert_diagnostic_ranges(
        error,
        source_range(generator_source, "?"),
        Some(source_range(
            generator_source,
            "seq {\n            let unwrapped = value?\n            yield unwrapped\n        }",
        )),
    );
}

#[test]
fn omitted_flow_return_is_a_unit_boundary_with_header_related_evidence() {
    let source = r"
flow @flow.unit unit(value: Result<i64, String>) {
    let unwrapped = value?
}
";
    let errors = typecheck_errors("unit-flow-propagation", source, &TypeCheckEnv::new());
    let error = errors
        .iter()
        .find(|error| error.stable_code() == "sema.try.propagation_target_missing")
        .expect("Unit flow rejects Result propagation");
    let TypeCheckErrorKind::TryPropagationTargetMissing {
        target: Some(PropagationTargetEvidence::Boundary(boundary)),
        ..
    } = error.kind()
    else {
        panic!("unexpected flow propagation payload: {error:#?}");
    };
    assert_eq!(boundary.kind(), PropagationBoundaryKind::Flow);
    assert_eq!(
        boundary.checked_return(),
        &CheckedReturnType::Known(TypeKind::Unit)
    );
    assert!(boundary.result().is_none());
    assert_diagnostic_ranges(
        error,
        source_range(source, "?"),
        Some(source_range(
            source,
            "flow @flow.unit unit(value: Result<i64, String>)",
        )),
    );
}
