use crate::documents::DocumentSnapshot;
use crate::positions::{LineIndex, PositionEncoding};
use crate::profiles::LspProfile;
use arcweft_lang_hir::lower::lower_document_to_hir;
use arcweft_lang_sema::{
    check::{analyze_types, validate_typecheck_ready},
    diagnostics::{TypeCheckError, TypeCheckReadinessError, TypeCheckWarning},
    resolve::{NameResolutionError, registry_from_hir, validate_hir_references},
};
use arcweft_lang_syntax::{
    lint::{SyntaxLint, lint_id_policy},
    parser::{ParseOptions, parse_document_with_source, recovery::ParseError},
};
use arcweft_source::{
    Diagnostic as ArcDiagnostic, DiagnosticLabelStyle, DiagnosticSeverity as ArcDiagnosticSeverity,
    SourceDocument, SourceSpan, SourceSpanValidationError,
};
use arcweft_verify::{
    BackendKind, VerificationMode, VerificationPolicy, VerificationReport, verify_module_with_env,
};
use arcweft_verify_lsp::{
    LspPositionMapper, diagnostics_from_report_with_mapper,
    profile_manifest_conformance_diagnostics,
};
use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString,
    Position, PublishDiagnosticsParams, Range, Uri,
};
use std::sync::Arc;

/// Analyzed document diagnostics plus source index used by feature handlers.
#[derive(Clone, Debug)]
pub struct DocumentAnalysis {
    diagnostics: Vec<Diagnostic>,
    line_index: LineIndex,
    verification_report: Option<VerificationReport>,
    document: Arc<SourceDocument>,
}

/// Revision-validating adapter from shared Arcweft diagnostics to LSP values.
pub struct DiagnosticProjector<'a> {
    document: &'a SourceDocument,
    line_index: &'a LineIndex,
}

impl<'a> DiagnosticProjector<'a> {
    /// Binds projection to one exact source document and negotiated line index.
    pub const fn new(document: &'a SourceDocument, line_index: &'a LineIndex) -> Self {
        Self {
            document,
            line_index,
        }
    }

    /// Projects one shared diagnostic after exact document/revision validation.
    pub fn project(
        &self,
        diagnostic: &ArcDiagnostic,
    ) -> Result<Diagnostic, SourceSpanValidationError> {
        lsp_diagnostic_from_arcweft_with_source(
            diagnostic,
            self.line_index,
            self.document,
            lsp_source_name(diagnostic),
        )
    }
}

impl DocumentAnalysis {
    fn analyze_document(
        document: Arc<SourceDocument>,
        line_index: LineIndex,
        profile: &LspProfile,
    ) -> Self {
        let source_document = document.as_ref();
        let mut verification_report = None;
        let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
        let mut diagnostics = parsed
            .errors()
            .iter()
            .filter_map(|error| {
                lsp_diagnostic_from_parse_error(error, &line_index, source_document).ok()
            })
            .collect::<Vec<_>>();

        if parsed.errors().is_empty() {
            diagnostics.extend(syntax_lint_diagnostics(
                &lint_id_policy(parsed.typed_tree()),
                &line_index,
                source_document,
            ));
            match lower_document_to_hir(source_document, parsed.typed_tree()) {
                Ok(hir) => {
                    let env = profile.typecheck_env();
                    let resolve = resolve_diagnostics(&hir, &line_index, source_document);
                    if resolve.is_empty() {
                        let readiness = readiness_diagnostics(&hir, &line_index, source_document);
                        if readiness.is_empty() {
                            let typecheck_report = analyze_types(&hir, &env);
                            diagnostics.extend(typecheck_diagnostics(
                                &typecheck_report.diagnostics,
                                &line_index,
                                source_document,
                            ));
                            diagnostics.extend(typecheck_warnings(
                                &typecheck_report.warnings,
                                source_document,
                            ));
                            if typecheck_report.diagnostics.is_empty() {
                                let report = verify_module_with_env(
                                    &hir,
                                    &env,
                                    VerificationPolicy {
                                        mode: VerificationMode::Dev,
                                        backend: BackendKind::Emit,
                                        allow_trusted_proofs: true,
                                    },
                                );
                                diagnostics.extend(diagnostics_from_report_with_mapper(
                                    &report,
                                    &line_index,
                                ));
                                verification_report = Some(report);
                            }
                        } else {
                            diagnostics.extend(readiness);
                        }
                    } else {
                        diagnostics.extend(resolve);
                    }
                }
                Err(errors) => {
                    diagnostics.extend(errors.into_iter().filter_map(|error| {
                        lsp_diagnostic_from_arcweft(
                            &error.diagnostic(source_document),
                            &line_index,
                            source_document,
                        )
                        .ok()
                    }));
                }
            }
        }

        Self {
            diagnostics,
            line_index,
            verification_report,
            document,
        }
    }

    /// Runs analysis against the exact source document and negotiated line index of an open
    /// snapshot.
    pub fn analyze_snapshot(snapshot: &DocumentSnapshot, profile: &LspProfile) -> Self {
        Self::analyze_document(
            Arc::clone(snapshot.source_document()),
            snapshot.line_index().clone(),
            profile,
        )
    }

    /// Diagnostics emitted for the analyzed document.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Line index used for source-aware LSP feature conversion.
    pub const fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    /// Verifier report retained for typed verifier code actions.
    pub const fn verification_report(&self) -> Option<&VerificationReport> {
        self.verification_report.as_ref()
    }

    /// Exact source-document lease retained by this analysis.
    pub(crate) const fn source_document(&self) -> &Arc<SourceDocument> {
        &self.document
    }
}

fn syntax_lint_diagnostics(
    lints: &[SyntaxLint],
    line_index: &LineIndex,
    document: &SourceDocument,
) -> Vec<Diagnostic> {
    lints
        .iter()
        .filter_map(|lint| {
            lsp_diagnostic_from_arcweft(&lint.diagnostic(document), line_index, document).ok()
        })
        .collect()
}

/// Builds diagnostics from the exact analysis shared by the current session cache.
pub fn publish_diagnostics_from_analysis(
    snapshot: &DocumentSnapshot,
    profile: &LspProfile,
    analysis: &DocumentAnalysis,
) -> PublishDiagnosticsParams {
    let mut diagnostics = analysis.diagnostics.clone();
    reconcile_accepted_nominal_diagnostics(snapshot, profile, &mut diagnostics);
    diagnostics.extend(profile_diagnostics(profile));
    PublishDiagnosticsParams::new(
        snapshot.uri().clone(),
        diagnostics,
        Some(snapshot.version()),
    )
}

fn reconcile_accepted_nominal_diagnostics(
    snapshot: &DocumentSnapshot,
    profile: &LspProfile,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(accepted) = profile.accepted_environment() else {
        return;
    };
    let project = accepted.project();
    let Some(source) = project.sources().by_uri(snapshot.uri()) else {
        return;
    };
    if source.document().text() != snapshot.text() {
        return;
    }

    diagnostics.retain(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code.starts_with("sema.nominal.")
        )
    });
    let projector = DiagnosticProjector::new(source.document(), snapshot.line_index());
    diagnostics.extend(
        project
            .typecheck()
            .nominal_resolutions
            .diagnostics()
            .iter()
            .filter_map(arcweft_lang_sema::nominal::NominalTypeDiagnostic::to_source_diagnostic)
            .filter_map(|diagnostic| projector.project(&diagnostic).ok()),
    );
}

fn resolve_diagnostics(
    hir: &arcweft_lang_hir::model::HirModule,
    line_index: &LineIndex,
    document: &SourceDocument,
) -> Vec<Diagnostic> {
    let registry = registry_from_hir(hir);
    validate_hir_references(hir, &registry).map_or_else(
        |errors| {
            errors
                .iter()
                .enumerate()
                .filter_map(|(index, error)| {
                    name_resolution_diagnostic(error, index + 1, line_index, document).ok()
                })
                .collect()
        },
        |()| Vec::new(),
    )
}

fn readiness_diagnostics(
    hir: &arcweft_lang_hir::model::HirModule,
    line_index: &LineIndex,
    document: &SourceDocument,
) -> Vec<Diagnostic> {
    validate_typecheck_ready(hir).map_or_else(
        |errors| {
            errors
                .iter()
                .enumerate()
                .filter_map(|(index, error)| {
                    readiness_diagnostic(error, index + 1, line_index, document).ok()
                })
                .collect()
        },
        |()| Vec::new(),
    )
}

fn name_resolution_diagnostic(
    error: &NameResolutionError,
    _index: usize,
    line_index: &LineIndex,
    document: &SourceDocument,
) -> Result<Diagnostic, SourceSpanValidationError> {
    lsp_diagnostic_from_arcweft(&error.diagnostic(), line_index, document)
}

fn readiness_diagnostic(
    error: &TypeCheckReadinessError,
    _index: usize,
    line_index: &LineIndex,
    document: &SourceDocument,
) -> Result<Diagnostic, SourceSpanValidationError> {
    lsp_diagnostic_from_arcweft(&error.diagnostic(), line_index, document)
}

fn typecheck_diagnostics(
    errors: &[TypeCheckError],
    line_index: &LineIndex,
    document: &SourceDocument,
) -> Vec<Diagnostic> {
    errors
        .iter()
        .filter_map(|error| {
            lsp_diagnostic_from_arcweft(&error.diagnostic(), line_index, document).ok()
        })
        .collect()
}

fn typecheck_warnings(warnings: &[TypeCheckWarning], document: &SourceDocument) -> Vec<Diagnostic> {
    let line_index = LineIndex::new(String::new(), PositionEncoding::Utf16);
    warnings
        .iter()
        .filter_map(|warning| {
            lsp_diagnostic_from_arcweft(&warning.diagnostic(), &line_index, document).ok()
        })
        .collect()
}

fn profile_diagnostics(profile: &LspProfile) -> Vec<Diagnostic> {
    let mut diagnostics = profile
        .diagnostics()
        .iter()
        .map(|diagnostic| Diagnostic {
            range: start_range(),
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(diagnostic.kind().code().to_owned())),
            source: Some("arcweft-lsp-profile".to_owned()),
            message: diagnostic.message().to_owned(),
            ..Diagnostic::default()
        })
        .collect::<Vec<_>>();
    diagnostics.extend(profile_manifest_conformance_diagnostics(
        &profile.context(),
        profile.declared_manifests(),
    ));
    diagnostics
}

fn lsp_diagnostic_from_arcweft(
    diagnostic: &ArcDiagnostic,
    line_index: &LineIndex,
    document: &SourceDocument,
) -> Result<Diagnostic, SourceSpanValidationError> {
    DiagnosticProjector::new(document, line_index).project(diagnostic)
}

fn lsp_diagnostic_from_parse_error(
    error: &ParseError,
    line_index: &LineIndex,
    document: &SourceDocument,
) -> Result<Diagnostic, SourceSpanValidationError> {
    lsp_diagnostic_from_arcweft(&error.diagnostic(document), line_index, document)
}

fn lsp_diagnostic_from_arcweft_with_source(
    diagnostic: &ArcDiagnostic,
    line_index: &LineIndex,
    document: &SourceDocument,
    source: &'static str,
) -> Result<Diagnostic, SourceSpanValidationError> {
    diagnostic.validate_source(document)?;
    let span = primary_span(diagnostic);
    let range = span.map_or_else(start_range, |span| range_for_span(span, line_index));
    Ok(Diagnostic {
        range,
        severity: Some(lsp_severity(diagnostic.severity())),
        code: diagnostic
            .code()
            .map(|code| NumberOrString::String(code.as_str().to_owned())),
        source: Some(source.to_owned()),
        message: diagnostic.message().to_owned(),
        related_information: related_information(diagnostic, line_index),
        data: suggestions_data(diagnostic, line_index),
        ..Diagnostic::default()
    })
}

fn primary_span(diagnostic: &ArcDiagnostic) -> Option<&SourceSpan> {
    diagnostic.span().or_else(|| {
        diagnostic
            .labels()
            .first()
            .map(arcweft_source::DiagnosticLabel::span)
    })
}

fn range_for_span(span: &SourceSpan, line_index: &LineIndex) -> Range {
    line_index.range_from_byte_span(span.range().start(), span.range().end())
}

fn lsp_severity(severity: ArcDiagnosticSeverity) -> DiagnosticSeverity {
    match severity {
        ArcDiagnosticSeverity::Error => DiagnosticSeverity::ERROR,
        ArcDiagnosticSeverity::Warning => DiagnosticSeverity::WARNING,
        ArcDiagnosticSeverity::Info => DiagnosticSeverity::INFORMATION,
        ArcDiagnosticSeverity::Hint => DiagnosticSeverity::HINT,
    }
}

fn lsp_source_name(diagnostic: &ArcDiagnostic) -> &'static str {
    match diagnostic
        .code()
        .map(arcweft_source::DiagnosticCode::as_str)
    {
        Some(code) if code.starts_with("syntax.") || code.starts_with("AWF0") => "arcweft-syntax",
        Some(code) if code.starts_with("hir.") => "arcweft-hir",
        Some(code) if code.starts_with("sema.") || code.starts_with("AWF-EFX") => "arcweft-sema",
        Some(code) if code.starts_with("runtime.") => "arcweft-runtime",
        _ => "arcweft",
    }
}

fn related_information(
    diagnostic: &ArcDiagnostic,
    line_index: &LineIndex,
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let uri = memory_uri();
    let mut related = diagnostic
        .labels()
        .iter()
        .filter_map(|label| {
            let message = label
                .message()
                .map(str::to_owned)
                .or_else(|| match label.style() {
                    DiagnosticLabelStyle::Primary => Some("primary location".to_owned()),
                    DiagnosticLabelStyle::Secondary => Some("related location".to_owned()),
                })?;
            Some(DiagnosticRelatedInformation {
                location: Location::new(uri.clone(), range_for_span(label.span(), line_index)),
                message,
            })
        })
        .collect::<Vec<_>>();
    let note_range =
        primary_span(diagnostic).map_or_else(start_range, |span| range_for_span(span, line_index));
    related.extend(
        diagnostic
            .notes()
            .iter()
            .map(|note| DiagnosticRelatedInformation {
                location: Location::new(uri.clone(), note_range),
                message: note.clone(),
            }),
    );
    (!related.is_empty()).then_some(related)
}

fn suggestions_data(
    diagnostic: &ArcDiagnostic,
    line_index: &LineIndex,
) -> Option<serde_json::Value> {
    if diagnostic.suggestions().is_empty() {
        return None;
    }
    Some(serde_json::json!({
        "suggestions": diagnostic.suggestions().iter().map(|suggestion| {
            serde_json::json!({
                "message": suggestion.message(),
                "applicability": suggestion.applicability().as_str(),
                "edits": suggestion.edits().iter().map(|edit| {
                    serde_json::json!({
                        "range": range_for_span(edit.span(), line_index),
                        "replacement": edit.replacement(),
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
    }))
}

fn memory_uri() -> Uri {
    "file:///__arcweft_memory__.arcw"
        .parse()
        .expect("memory URI is valid")
}

fn start_range() -> Range {
    Range::new(Position::new(0, 0), Position::new(0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_adapter_context::manifest::{
        AdapterCallableGroupIndex, AdapterCallableName, AdapterCallableOverloadIndex,
        AdapterCallableParameterIndex, AdapterCallablePath, AdapterFunctionParam,
        AdapterFunctionSignature, AdapterManifest, AdapterParameterGroup, AdapterParameterPassing,
        AdapterParameterPresence, AdapterTypeKind,
    };
    use arcweft_runtime_host::RuntimeHostRunnerKind;
    use arcweft_source::{
        DiagnosticApplicability, DiagnosticLabel, DiagnosticSuggestion, SourceDocumentId,
        SourceEdit, SourceName, SourceRange,
    };

    fn analyze_fixture(
        source: &str,
        encoding: PositionEncoding,
        profile: &LspProfile,
    ) -> DocumentAnalysis {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-generated://lsp-document-analysis/fixture")
                    .expect("fixture document ID"),
                SourceName::Generated,
                source,
            )
            .expect("fixture source document"),
        );
        let line_index = LineIndex::new(document.text(), encoding);
        DocumentAnalysis::analyze_document(document, line_index, profile)
    }

    #[test]
    fn document_analysis_retains_the_exact_source_lease() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("file:///workspace/exact-analysis.arcw")
                    .expect("fixture document ID"),
                SourceName::path("exact-analysis.arcw"),
                "fn main() {}\n",
            )
            .expect("fixture source document"),
        );

        let analysis = DocumentAnalysis::analyze_document(
            Arc::clone(&document),
            LineIndex::new(document.text(), PositionEncoding::Utf16),
            &LspProfile::default_for_runner(RuntimeHostRunnerKind::Native),
        );

        assert!(Arc::ptr_eq(analysis.source_document(), &document));
    }

    #[test]
    fn stale_span_is_not_published() {
        let id = SourceDocumentId::try_new("file:///workspace/main.arcw").expect("document id");
        let stale = SourceDocument::try_new(id.clone(), SourceName::path("main.arcw"), "old")
            .expect("stale document");
        let current = SourceDocument::try_new(id, SourceName::path("main.arcw"), "new")
            .expect("current document");
        let stale_span = stale
            .span(arcweft_source::SourceRange::new(0, 3))
            .expect("stale span");
        let diagnostic = ArcDiagnostic::new(ArcDiagnosticSeverity::Error, "stale")
            .with_span(stale_span.clone())
            .with_suggestion(
                arcweft_source::DiagnosticSuggestion::new(
                    "replace",
                    DiagnosticApplicability::MachineApplicable,
                )
                .with_edit(arcweft_source::SourceEdit::new(stale_span, "current")),
            );
        let line_index = LineIndex::new("new".to_owned(), PositionEncoding::Utf16);

        assert!(matches!(
            lsp_diagnostic_from_arcweft(&diagnostic, &line_index, &current),
            Err(SourceSpanValidationError::WrongRevision { expected, actual })
                if expected == current.identity().revision()
                    && actual == stale.identity().revision()
        ));
        let published = [diagnostic]
            .iter()
            .filter_map(|diagnostic| {
                lsp_diagnostic_from_arcweft(diagnostic, &line_index, &current).ok()
            })
            .collect::<Vec<_>>();
        assert!(published.is_empty());
    }

    #[test]
    fn stale_edit_is_not_published_from_a_current_diagnostic() {
        let id = SourceDocumentId::try_new("file:///workspace/view.arcw").expect("document id");
        let original = SourceDocument::try_new(
            id.clone(),
            SourceName::path("view.arcw"),
            "pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n",
        )
        .expect("original document");
        let current = SourceDocument::try_new(
            id,
            SourceName::path("view.arcw"),
            "pub view Card() {\n    export part タイトル as heading\n    Panel()\n}\n",
        )
        .expect("current document");
        let diagnostic = ArcDiagnostic::new(ArcDiagnosticSeverity::Error, "current diagnostic")
            .with_label(DiagnosticLabel::primary(
                current
                    .span(SourceRange::new(50, 57))
                    .expect("current heading span"),
                None,
            ))
            .with_suggestion(
                DiagnosticSuggestion::new(
                    "stale insertion",
                    DiagnosticApplicability::MachineApplicable,
                )
                .with_edit(SourceEdit::new(
                    original
                        .span(SourceRange::new(47, 47))
                        .expect("stale insertion span"),
                    "as ",
                )),
            );
        let line_index = LineIndex::new(current.text(), PositionEncoding::Utf16);

        assert!(matches!(
            DiagnosticProjector::new(&current, &line_index).project(&diagnostic),
            Err(SourceSpanValidationError::WrongRevision { expected, actual })
                if expected == current.identity().revision()
                    && actual == original.identity().revision()
        ));
    }

    #[test]
    fn parser_diagnostic_test_only_edit_maps_in_both_position_encodings() {
        const SOURCE: &str =
            "pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n";
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-generated://lsp-parser-edit/0")
                .expect("fixture source id"),
            SourceName::Generated,
            SOURCE,
        )
        .expect("fixture source document");
        let diagnostic = ArcDiagnostic::new(
            ArcDiagnosticSeverity::Error,
            "View part export needs `as` before its public name",
        )
        .with_code("view::export_part_missing_as")
        .with_label(DiagnosticLabel::primary(
            document
                .span(SourceRange::new(47, 54))
                .expect("fixture diagnostic span"),
            None,
        ))
        .with_suggestion(
            DiagnosticSuggestion::new(
                "insert missing `as` keyword",
                DiagnosticApplicability::MachineApplicable,
            )
            .with_edit(SourceEdit::new(
                document
                    .span(SourceRange::new(47, 47))
                    .expect("fixture insertion span"),
                "as ",
            )),
        );

        for (encoding, expected_character) in
            [(PositionEncoding::Utf16, 21), (PositionEncoding::Utf8, 29)]
        {
            let line_index = LineIndex::new(SOURCE, encoding);
            let projected = DiagnosticProjector::new(&document, &line_index)
                .project(&diagnostic)
                .expect("exact revision projects");
            let edit =
                &projected.data.as_ref().expect("suggestion data")["suggestions"][0]["edits"][0];

            assert_eq!(edit["range"]["start"]["line"], 1);
            assert_eq!(edit["range"]["start"]["character"], expected_character);
            assert_eq!(edit["range"]["end"], edit["range"]["start"]);
            assert_eq!(edit["replacement"], "as ");
            assert_eq!(
                projected.data.as_ref().expect("suggestion data")["suggestions"][0]["applicability"],
                "machine_applicable"
            );
        }
    }

    #[test]
    fn propagation_diagnostics_project_exact_try_and_await_operators() {
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        for (parameter_type, expression, code, utf16, utf8) in [
            (
                "Result<i64, String>",
                "try value",
                "sema.try.error_mismatch",
                12..15,
                14..17,
            ),
            (
                "Result<i64, String>",
                "value?",
                "sema.try.error_mismatch",
                17..18,
                19..20,
            ),
            (
                "Need<i64, String>",
                "try await value",
                "sema.await.error_mismatch",
                12..15,
                14..17,
            ),
            (
                "Need<i64, String>",
                "await? value",
                "sema.await.error_mismatch",
                17..18,
                19..20,
            ),
        ] {
            let source = format!(
                "fn demo(value: {parameter_type}) -> Result<i64, i64> {{\n    let 前 = {expression}\n    Ok(前)\n}}\n"
            );
            let result_start = source
                .find("Result<i64, i64>")
                .expect("fixture declares a Result return");
            let result_end = result_start + "Result<i64, i64>".len();
            let result_start = u32::try_from(result_start).expect("fixture position fits LSP");
            let result_end = u32::try_from(result_end).expect("fixture position fits LSP");
            for (encoding, expected) in [
                (PositionEncoding::Utf16, utf16.clone()),
                (PositionEncoding::Utf8, utf8.clone()),
            ] {
                let analysis = analyze_fixture(&source, encoding, &profile);
                let diagnostic = analysis
                    .diagnostics()
                    .iter()
                    .find(|diagnostic| {
                        diagnostic.code == Some(NumberOrString::String(code.to_owned()))
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "missing {code} for {expression} under {encoding:?}: {:#?}",
                            analysis.diagnostics()
                        )
                    });

                assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
                assert_eq!(
                    diagnostic.range,
                    Range::new(
                        Position::new(1, expected.start),
                        Position::new(1, expected.end),
                    )
                );
                let related = diagnostic
                    .related_information
                    .as_ref()
                    .expect("return boundary is projected as related information");
                assert!(
                    related.iter().any(|information| {
                        information.message == "enclosing return error is declared here"
                            && information.location.range
                                == Range::new(
                                    Position::new(0, result_start),
                                    Position::new(0, result_end),
                                )
                    }),
                    "related propagation boundary was {related:#?}"
                );
            }
        }
    }

    #[test]
    fn diagnostics_do_not_bypass_registered_callable_catalog() {
        let source = r#"
flow @.main main {
    let value = custom_echo("hello")
}
"#;
        let default_profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let default_analysis = analyze_fixture(source, PositionEncoding::Utf16, &default_profile);
        assert!(default_analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unknown function `custom_echo`")
        }));

        let adapter = AdapterManifest::new("custom", "Custom").with_function_signature(
            AdapterCallablePath::single(
                AdapterCallableName::try_new("custom_echo").expect("valid callable name"),
            ),
            AdapterCallableOverloadIndex::try_from_usize(0).expect("zero overload fits"),
            AdapterFunctionSignature::try_new(
                vec![
                    AdapterParameterGroup::try_new(
                        AdapterCallableGroupIndex::try_from_usize(0).expect("initial group fits"),
                        vec![
                            AdapterFunctionParam::try_new(
                                AdapterCallableParameterIndex::try_from_usize(0)
                                    .expect("parameter index fits"),
                                Some(
                                    AdapterCallableName::try_new("value")
                                        .expect("valid parameter name"),
                                ),
                                AdapterTypeKind::String,
                                AdapterParameterPassing::PositionalOrNamed,
                                AdapterParameterPresence::Required,
                            )
                            .expect("valid parameter"),
                        ],
                    )
                    .expect("valid initial group"),
                ],
                AdapterTypeKind::String,
            )
            .expect("valid callable signature"),
            [],
        );
        let profile = LspProfile::new(adapter, RuntimeHostRunnerKind::Native);
        let profile_analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);

        assert!(
            profile_analysis.diagnostics().iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("unknown function `custom_echo`")
            }),
            "an unregistered manifest must not mutate the checker environment"
        );
    }

    #[test]
    fn diagnostics_accept_standard_enum_variant_shorthand() {
        let source = r#"
flow @flow.opening opening {
    let payload = ["hello"]
    let bytes: Bytes = data.encode(payload, .Json)
    let decoded: AgentValue = data.decode(bytes, .Json)
    let shape: DataShape = data.shape(decoded)
}
"#;
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);

        assert!(
            !analysis.diagnostics().iter().any(|diagnostic| {
                diagnostic.message.contains("data.encode")
                    || diagnostic.message.contains("data.decode")
                    || diagnostic.message.contains(".Json")
                    || diagnostic.message.contains("DataFormat")
            }),
            "unexpected data enum diagnostic: {:?}",
            analysis.diagnostics()
        );
    }

    #[test]
    fn diagnostics_publish_numeric_fallback_warning() {
        let source = r"
flow @flow.closure_numeric_fallback closure_numeric_fallback {
    let fallback = || 1
}
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);
        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.code
                    == Some(NumberOrString::String(
                        "sema.numeric.fallback_in_inferred_closure".into(),
                    ))
            })
            .expect("numeric fallback warning diagnostic");

        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(diagnostic.source.as_deref(), Some("arcweft-sema"));
        assert!(diagnostic.message.contains("defaults to I32"));
    }

    #[test]
    fn diagnostics_include_stable_syntax_lint_codes() {
        let source = r"
flow @flow.opening start {
}
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);
        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == Some(NumberOrString::String("AWF0102".into())))
            .expect("identity mismatch diagnostic");

        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diagnostic.source.as_deref(), Some("arcweft-syntax"));
    }

    #[test]
    fn parser_diagnostic_missing_as_preserves_utf16_payload_without_an_edit() {
        let source = "pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);
        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.code
                    == Some(NumberOrString::String(
                        "view::export_part_missing_as".to_owned(),
                    ))
            })
            .expect("typed parser diagnostic");

        assert_eq!(diagnostic.source.as_deref(), Some("arcweft"));
        assert_eq!(diagnostic.range.start, Position::new(1, 21));
        assert_eq!(diagnostic.range.end, Position::new(1, 28));
        assert_eq!(
            diagnostic.message,
            "View part export needs `as` before its public name"
        );
        assert!(
            diagnostic
                .related_information
                .as_ref()
                .is_some_and(|related| related
                    .iter()
                    .any(|item| item.message == "expected: as public_name"))
        );
        assert_eq!(
            diagnostic.data,
            Some(serde_json::json!({
                "suggestions": [{
                    "message": "use as public_name syntax",
                    "applicability": "unspecified",
                    "edits": [],
                }],
            }))
        );
    }

    #[test]
    fn parser_diagnostic_missing_as_maps_the_same_payload_to_utf8() {
        let source = "pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf8, &profile);
        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.code
                    == Some(NumberOrString::String(
                        "view::export_part_missing_as".to_owned(),
                    ))
            })
            .expect("typed parser diagnostic");

        assert_eq!(diagnostic.source.as_deref(), Some("arcweft"));
        assert_eq!(diagnostic.range.start, Position::new(1, 29));
        assert_eq!(diagnostic.range.end, Position::new(1, 36));
        assert_eq!(
            diagnostic.message,
            "View part export needs `as` before its public name"
        );
        assert_eq!(
            diagnostic.data,
            Some(serde_json::json!({
                "suggestions": [{
                    "message": "use as public_name syntax",
                    "applicability": "unspecified",
                    "edits": [],
                }],
            }))
        );
    }

    #[test]
    fn statement_unknown_mode_projects_as_parser_diagnostic() {
        let source = "flow demo {\n    assert.assume(true)\n}\n";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);
        let [diagnostic] = analysis.diagnostics() else {
            panic!(
                "expected one parser diagnostic, got {:?}",
                analysis.diagnostics()
            );
        };

        assert_eq!(
            diagnostic.code,
            Some(NumberOrString::String(
                "syntax.assert.unknown_mode".to_owned()
            ))
        );
        assert_eq!(diagnostic.message, "unknown assertion mode");
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diagnostic.source.as_deref(), Some("arcweft-syntax"));
        assert_eq!(diagnostic.range.start, Position::new(1, 11));
        assert_eq!(diagnostic.range.end, Position::new(1, 17));
    }

    #[test]
    fn bare_flow_item_uses_generic_declaration_only_syntax_diagnostics() {
        let source = "alice: hello\npub character bob {}\n";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);
        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.code == Some(NumberOrString::String("syntax.parse".to_owned()))
            })
            .expect("generic syntax diagnostic is published");

        assert_eq!(diagnostic.source.as_deref(), Some("arcweft-syntax"));
        assert_eq!(diagnostic.message, "unexpected top-level item");
        assert_eq!(diagnostic.range.start, Position::new(0, 0));
        assert_eq!(diagnostic.range.end, Position::new(0, 12));
        let data = diagnostic.data.as_ref().expect("generic recovery data");
        assert_eq!(
            data["suggestions"][0]["message"],
            "use a current Arcweft declaration form"
        );
        assert!(
            !diagnostic.message.to_ascii_lowercase().contains("removed")
                && !diagnostic
                    .message
                    .to_ascii_lowercase()
                    .contains("deprecated")
        );
    }

    #[test]
    fn diagnostics_map_explicit_decl_id_to_hint() {
        let source = r"
flow @flow.opening {
}
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);
        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == Some(NumberOrString::String("AWF0103".into())))
            .expect("explicit declaration id diagnostic");

        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::HINT));
        assert_eq!(diagnostic.source.as_deref(), Some("arcweft-syntax"));
    }

    #[test]
    fn diagnostics_respect_allow_attribute_for_flow_module_mismatch() {
        let source = r"
mod route.opening

#[allow(id::flow_module_mismatch)]
flow @flow.prologue {
}
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);

        assert!(!analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == Some(NumberOrString::String("AWF0002".into()))
        }));
    }

    #[test]
    fn diagnostics_respect_source_generated_attribute_for_decl_identity() {
        let source = r"
#![generated(tool)]
flow @flow.opening opening {
}

flow @flow.opening start {
}
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);

        assert!(analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == Some(NumberOrString::String("AWF0104".into()))
                && diagnostic.severity == Some(DiagnosticSeverity::INFORMATION)
        }));
        assert!(!analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == Some(NumberOrString::String("AWF0101".into()))
        }));
        assert!(analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == Some(NumberOrString::String("AWF0102".into()))
                && diagnostic.severity == Some(DiagnosticSeverity::ERROR)
        }));
    }

    #[test]
    fn diagnostics_surface_public_abi_anonymous_sum_warning() {
        let source = r"
pub type Payload = String | Bytes
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);

        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.code
                    == Some(NumberOrString::String(
                        "sema.public_abi.anonymous_sum".to_owned(),
                    ))
            })
            .expect("public ABI anonymous sum warning is surfaced");
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
        assert!(diagnostic.message.contains("public type alias `Payload`"));
    }

    #[test]
    fn diagnostics_surface_upper_bound_exceeded_effect_error() {
        let source = r"
flow @flow.opening opening
effects {}
{
    'flow.flags.seen <- 1i32
}
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);

        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.code == Some(NumberOrString::String("AWF-EFX-001".to_owned()))
            })
            .expect("upper-bound effect error is surfaced");
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostic.message.contains("state.write(flow)"));
    }

    #[test]
    fn diagnostics_surface_returned_closure_effect_trace() {
        let source = r#"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
}

fn make_loader(load: String -> String) -> Unit -> String {
    return |_unit: Unit| -> String { load("story.arcw") }
}

flow @flow.returned_closure_callback_call returned_closure_callback_call
effects { }
{
    let loader = make_loader(|path: String| -> String {
        fs.read_text(path = path)
    })
    let body = loader(())
}
"#;
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);

        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.code == Some(NumberOrString::String("AWF-EFX-001".to_owned()))
            })
            .expect("returned closure effect error is surfaced");
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert!(
            diagnostic
                .message
                .contains("flow.returned_closure_callback_call")
        );
        assert!(diagnostic.message.contains("fs.read"));

        let related = diagnostic
            .related_information
            .as_ref()
            .expect("effect trace is surfaced as related information");
        let rendered = related
            .iter()
            .map(|item| item.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rendered.contains("effect trace for `fs.read`"),
            "trace related information was:\n{rendered}"
        );
        assert!(
            rendered.contains("function value call `loader`"),
            "trace related information was:\n{rendered}"
        );
        assert!(
            rendered.contains("returned function value from `make_loader`"),
            "trace related information was:\n{rendered}"
        );
        assert!(
            rendered.contains("higher-order argument `load` captured by returned closure"),
            "trace related information was:\n{rendered}"
        );
        assert!(
            rendered.contains("call `fs.read_text`"),
            "trace related information was:\n{rendered}"
        );
    }

    #[test]
    fn diagnostics_surface_performed_effect_trace() {
        let source = r"
extern capability assets {
    fn load_avatar() -> Need<String, AssetError>
}

flow @flow.await_avatar await_avatar
effects { }
{
    let avatar = await assets.load_avatar()
}
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);

        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.code == Some(NumberOrString::String("AWF-EFX-001".to_owned()))
                    && diagnostic.message.contains("control.suspend")
            })
            .expect("performed await effect error is surfaced");
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));

        let related = diagnostic
            .related_information
            .as_ref()
            .expect("performed effect trace is surfaced as related information");
        let rendered = related
            .iter()
            .map(|item| item.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rendered.contains("effect trace for `control.suspend`"),
            "trace related information was:\n{rendered}"
        );
        assert!(
            rendered.contains("`flow.await_avatar` performs `control.suspend`"),
            "trace related information was:\n{rendered}"
        );
        assert!(
            rendered.contains("via await"),
            "trace related information was:\n{rendered}"
        );
    }

    #[test]
    fn diagnostics_include_machine_applicable_suggestions_in_lsp_data() {
        let source = r"
flow @flow.opening {
}
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = analyze_fixture(source, PositionEncoding::Utf16, &profile);
        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == Some(NumberOrString::String("AWF0103".into())))
            .expect("explicit declaration id diagnostic");
        let data = diagnostic.data.as_ref().expect("suggestion data");
        let rendered = data.to_string();
        assert!(rendered.contains("machine_applicable"));
        assert!(rendered.contains("opening"));
    }
}
