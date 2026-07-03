use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::{check::analyze_types, env::TypeCheckEnv};
use arcweft_lang_syntax::parser::parse_source;

fn analyze_fixture(source: &str) -> arcweft_lang_sema::check::TypeCheckReport {
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_to_hir(parsed.typed_tree()).expect("fixture lowers to HIR");
    analyze_types(&hir, &TypeCheckEnv::default())
}

#[test]
fn full_iterator_method_body_typechecks() {
    let report = analyze_fixture(include_str!(
        "../../../fixtures/iterator-witness/user-defined.arcw"
    ));
    assert!(
        report.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report
            .for_iteration_evidence
            .iter()
            .any(|evidence| matches!(
                evidence.family,
                arcweft_lang_sema::check::ForIterationEvidenceFamily::Witness { .. }
            ))
    );
}

#[test]
fn nested_assignment_target_is_structured_diagnostic() {
    let report = analyze_fixture(include_str!(
        "../../../fixtures/iterator-witness/invalid-nested-assignment.arcw"
    ));
    assert!(report.diagnostics.iter().any(
        |diagnostic| diagnostic.stable_code() == "sema.typecheck.unsupported_assignment_target"
    ));
}

#[test]
fn branch_mismatch_under_option_return_is_rejected() {
    let report = analyze_fixture(include_str!(
        "../../../fixtures/iterator-witness/branch-mismatch.arcw"
    ));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("if expression else branch"))
    );
}
