use crate::documents::DocumentSnapshot;
use crate::positions::{LineIndex, PositionEncoding};
use crate::profiles::LspProfile;
use arcweft_lang_hir::lower::lower_document_to_hir;
use arcweft_lang_sema::{
    canonicalization::{CheckedCanonicalizationInventory, SemanticDataUnavailable},
    check::{analyze_types, validate_typecheck_ready},
    diagnostics::{TypeCheckError, TypeCheckReadinessError, TypeCheckWarning},
    resolve::{NameResolutionError, registry_from_hir, validate_hir_references},
};
use arcweft_lang_syntax::{
    lint::{SyntaxLint, lint_id_policy},
    parser::parse_source,
};
use arcweft_source::{
    Diagnostic as ArcDiagnostic, DiagnosticApplicability, DiagnosticLabel, DiagnosticLabelStyle,
    DiagnosticSeverity as ArcDiagnosticSeverity, DiagnosticSuggestion, SourceDocument,
    SourceDocumentId, SourceEdit, SourceName, SourceRevision, SourceSpan,
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

/// Analyzed document diagnostics plus source index used by feature handlers.
#[derive(Clone, Debug)]
pub struct DocumentAnalysis {
    diagnostics: Vec<Diagnostic>,
    line_index: LineIndex,
    verification_report: Option<VerificationReport>,
    canonicalization: Result<CheckedCanonicalizationInventory, SemanticDataUnavailable>,
    source_revision: SourceRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum LspDiagnosticSourceError {
    WrongDocument {
        expected: SourceDocumentId,
        actual: SourceDocumentId,
    },
    WrongRevision {
        expected: SourceRevision,
        actual: SourceRevision,
    },
}

impl DocumentAnalysis {
    /// Runs syntax, HIR lowering, profile-aware type checking, and verifier diagnostics.
    ///
    /// # Panics
    ///
    /// Panics only if the current platform cannot represent the in-memory source length in a
    /// revision-bound source identity.
    pub fn analyze(source: &str, encoding: PositionEncoding, profile: &LspProfile) -> Self {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-generated://lsp-document-analysis/0")
                .expect("generated LSP document id is valid"),
            SourceName::Generated,
            source,
        )
        .expect("an in-memory LSP source fits the source document identity");
        Self::analyze_document(&document, encoding, profile)
    }

    fn analyze_document(
        document: &SourceDocument,
        encoding: PositionEncoding,
        profile: &LspProfile,
    ) -> Self {
        let source = document.text();
        let line_index = LineIndex::new(source.to_owned(), encoding);
        let mut verification_report = None;
        let parsed = parse_source(source.to_owned());
        let mut diagnostics = parsed
            .errors()
            .iter()
            .filter_map(|error| {
                lsp_diagnostic_from_arcweft(&error.diagnostic(document), &line_index, document).ok()
            })
            .collect::<Vec<_>>();

        if parsed.errors().is_empty() {
            diagnostics.extend(syntax_lint_diagnostics(
                &lint_id_policy(parsed.typed_tree()),
                &line_index,
                document,
            ));
            match lower_document_to_hir(document, parsed.typed_tree()) {
                Ok(hir) => {
                    let env = profile.typecheck_env();
                    let resolve = resolve_diagnostics(&hir, &line_index, document);
                    if resolve.is_empty() {
                        let readiness = readiness_diagnostics(&hir, &line_index, document);
                        if readiness.is_empty() {
                            let typecheck_report = analyze_types(&hir, &env);
                            diagnostics.extend(typecheck_diagnostics(
                                &typecheck_report.diagnostics,
                                &line_index,
                                document,
                            ));
                            diagnostics
                                .extend(typecheck_warnings(&typecheck_report.warnings, document));
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
                            &error.diagnostic(document),
                            &line_index,
                            document,
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
            canonicalization: Err(SemanticDataUnavailable::new(
                document.identity().id().clone(),
                "standalone document analysis has no checked project snapshot",
            )),
            source_revision: document.identity().revision(),
        }
    }

    /// Runs analysis against the containing project and exact open-document snapshot.
    ///
    /// # Panics
    ///
    /// Panics if `uri` cannot form a non-empty, control-free source document identity or if the
    /// current platform cannot represent the in-memory source length in that identity.
    pub fn analyze_project(
        source: &str,
        encoding: PositionEncoding,
        profile: &LspProfile,
        uri: &Uri,
    ) -> Self {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new(uri.to_string()).expect("LSP URI is a valid document id"),
            SourceName::path(uri.to_string()),
            source,
        )
        .expect("an in-memory LSP source fits the source document identity");
        let mut analysis = Self::analyze_document(&document, encoding, profile);
        analysis.canonicalization =
            crate::canonicalization::checked_inventory_for_document(uri, source, profile);
        analysis
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

    /// Exact checked inventory or typed unavailability for this snapshot.
    pub const fn canonicalization_input(
        &self,
    ) -> arcweft_tooling::model::CanonicalizationInput<'_> {
        match &self.canonicalization {
            Ok(inventory) => arcweft_tooling::model::CanonicalizationInput::Checked(inventory),
            Err(unavailable) => {
                arcweft_tooling::model::CanonicalizationInput::Unavailable(unavailable)
            }
        }
    }

    /// BLAKE3 revision of the exact UTF-8 source analyzed here.
    pub const fn source_revision(&self) -> SourceRevision {
        self.source_revision
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

/// Builds a publishDiagnostics notification payload for one open document.
pub fn publish_diagnostics(
    snapshot: &DocumentSnapshot,
    profile: &LspProfile,
) -> PublishDiagnosticsParams {
    let analysis = DocumentAnalysis::analyze(
        snapshot.text(),
        snapshot.line_index().position_encoding(),
        profile,
    );
    publish_diagnostics_from_analysis(snapshot, profile, &analysis)
}

/// Builds diagnostics from the exact analysis shared by the current session cache.
pub fn publish_diagnostics_from_analysis(
    snapshot: &DocumentSnapshot,
    profile: &LspProfile,
    analysis: &DocumentAnalysis,
) -> PublishDiagnosticsParams {
    let mut diagnostics = analysis.diagnostics.clone();
    diagnostics.extend(profile_diagnostics(profile));
    PublishDiagnosticsParams::new(
        snapshot.uri().clone(),
        diagnostics,
        Some(snapshot.version()),
    )
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
) -> Result<Diagnostic, LspDiagnosticSourceError> {
    lsp_diagnostic_from_arcweft(&error.diagnostic(), line_index, document)
}

fn readiness_diagnostic(
    error: &TypeCheckReadinessError,
    _index: usize,
    line_index: &LineIndex,
    document: &SourceDocument,
) -> Result<Diagnostic, LspDiagnosticSourceError> {
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
) -> Result<Diagnostic, LspDiagnosticSourceError> {
    validate_diagnostic_sources(diagnostic, document)?;
    let span = primary_span(diagnostic);
    let range = span.map_or_else(start_range, |span| range_for_span(span, line_index));
    Ok(Diagnostic {
        range,
        severity: Some(lsp_severity(diagnostic.severity())),
        code: diagnostic
            .code()
            .map(|code| NumberOrString::String(code.as_str().to_owned())),
        source: Some(lsp_source_name(diagnostic).to_owned()),
        message: diagnostic.message().to_owned(),
        related_information: related_information(diagnostic, line_index),
        data: suggestions_data(diagnostic, line_index),
        ..Diagnostic::default()
    })
}

fn validate_diagnostic_sources(
    diagnostic: &ArcDiagnostic,
    document: &SourceDocument,
) -> Result<(), LspDiagnosticSourceError> {
    primary_span(diagnostic)
        .into_iter()
        .chain(diagnostic.labels().iter().map(DiagnosticLabel::span))
        .chain(
            diagnostic
                .suggestions()
                .iter()
                .flat_map(DiagnosticSuggestion::edits)
                .map(SourceEdit::span),
        )
        .try_for_each(|span| validate_span_source(span, document))
}

fn validate_span_source(
    span: &SourceSpan,
    document: &SourceDocument,
) -> Result<(), LspDiagnosticSourceError> {
    if span.source().id() != document.identity().id() {
        return Err(LspDiagnosticSourceError::WrongDocument {
            expected: document.identity().id().clone(),
            actual: span.source().id().clone(),
        });
    }
    if span.source().revision() != document.identity().revision() {
        return Err(LspDiagnosticSourceError::WrongRevision {
            expected: document.identity().revision(),
            actual: span.source().revision(),
        });
    }
    Ok(())
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
                "applicability": applicability_name(suggestion.applicability()),
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

fn applicability_name(applicability: DiagnosticApplicability) -> &'static str {
    match applicability {
        DiagnosticApplicability::MachineApplicable => "machine_applicable",
        DiagnosticApplicability::MaybeIncorrect => "maybe_incorrect",
        DiagnosticApplicability::HasPlaceholders => "has_placeholders",
        DiagnosticApplicability::Unspecified => "unspecified",
    }
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
        AdapterFunctionParam, AdapterFunctionSignature, AdapterManifest, AdapterTypeKind,
    };
    use arcweft_runtime_host::RuntimeHostRunnerKind;

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
            Err(LspDiagnosticSourceError::WrongRevision { expected, actual })
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
    fn diagnostics_use_profile_selected_adapter_environment() {
        let source = r#"
flow @.main main {
    let value = custom_echo("hello")
}
"#;
        let default_profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let default_analysis =
            DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &default_profile);
        assert!(default_analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unknown function `custom_echo`")
        }));

        let adapter = AdapterManifest::new("custom", "Custom").with_function_signature(
            "custom_echo",
            AdapterFunctionSignature::new(
                AdapterTypeKind::String,
                [AdapterFunctionParam::required(
                    "value",
                    AdapterTypeKind::String,
                )],
            ),
            [],
        );
        let profile = LspProfile::new(adapter, RuntimeHostRunnerKind::Native);
        let profile_analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);

        assert!(!profile_analysis.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unknown function `custom_echo`")
        }));
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
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);

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
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);
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
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);
        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == Some(NumberOrString::String("AWF0102".into())))
            .expect("identity mismatch diagnostic");

        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diagnostic.source.as_deref(), Some("arcweft-syntax"));
    }

    #[test]
    fn diagnostics_map_explicit_decl_id_to_hint() {
        let source = r"
flow @flow.opening {
}
";
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);
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
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);

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
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);

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
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);

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
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);

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
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);

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
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);

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
        let analysis = DocumentAnalysis::analyze(source, PositionEncoding::Utf16, &profile);
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
