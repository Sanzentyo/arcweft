use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::bounded_diagnostics;
use crate::callable::{
    CallableDiagnostic, CallableDiagnosticCode, CallableDiagnosticSeverity,
    CallableDiagnosticSubject, PRODUCTION_CALLABLE_LIMITS, PRODUCTION_SIGNATURE_LIMITS,
    SignatureQueryWorkMeter,
};

fn diagnostics(count: usize) -> (SourceDocument, Vec<CallableDiagnostic>) {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("signature-diagnostic-truncation").expect("document id"),
        SourceName::Memory,
        "target(value)",
    )
    .expect("source document");
    let span = document
        .span(SourceRange::new(0, document.text().len()))
        .expect("call span");
    let diagnostics = (0..count)
        .map(|_| {
            CallableDiagnostic::try_new(
                CallableDiagnosticCode::ArgumentTypeMismatch,
                CallableDiagnosticSeverity::Error,
                Some(span.clone()),
                CallableDiagnosticSubject::None,
                Vec::new(),
                Some(document.identity()),
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .expect("diagnostic")
        })
        .collect();
    (document, diagnostics)
}

#[test]
fn exactly_thirty_two_diagnostics_are_retained_unchanged() {
    let (document, diagnostics) = diagnostics(32);
    let call = document
        .span(SourceRange::new(0, document.text().len()))
        .expect("call span");
    let mut work = SignatureQueryWorkMeter::new(PRODUCTION_SIGNATURE_LIMITS);
    let (bounded, omitted) = bounded_diagnostics(
        &diagnostics,
        document.identity(),
        &call,
        &PRODUCTION_CALLABLE_LIMITS,
        &PRODUCTION_SIGNATURE_LIMITS,
        &mut work,
    )
    .expect("exact diagnostic boundary");

    assert_eq!(bounded, diagnostics);
    assert_eq!(omitted, 0);
    assert_eq!(work.report().projection().diagnostic_considerations(), 32);
}

#[test]
fn thirty_three_diagnostics_keep_first_thirty_one_and_one_marker() {
    let (document, diagnostics) = diagnostics(33);
    let call = document
        .span(SourceRange::new(0, document.text().len()))
        .expect("call span");
    let mut work = SignatureQueryWorkMeter::new(PRODUCTION_SIGNATURE_LIMITS);
    let (bounded, omitted) = bounded_diagnostics(
        &diagnostics,
        document.identity(),
        &call,
        &PRODUCTION_CALLABLE_LIMITS,
        &PRODUCTION_SIGNATURE_LIMITS,
        &mut work,
    )
    .expect("one-over diagnostic boundary truncates");

    assert_eq!(&bounded[..31], &diagnostics[..31]);
    assert_eq!(bounded.len(), 32);
    assert_eq!(
        bounded.last().map(CallableDiagnostic::code),
        Some(CallableDiagnosticCode::DiagnosticsTruncated)
    );
    assert_eq!(omitted, 2);
    assert_eq!(work.report().projection().diagnostic_considerations(), 33);
}
