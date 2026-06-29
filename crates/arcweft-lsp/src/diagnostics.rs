use crate::documents::DocumentSnapshot;
use crate::positions::{LineIndex, PositionEncoding};
use crate::profiles::LspProfile;
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::{
    check::{analyze_types, validate_typecheck_ready},
    diagnostics::{TypeCheckError, TypeCheckReadinessError, TypeCheckWarning},
    resolve::{NameResolutionError, registry_from_hir, validate_hir_references},
};
use arcweft_lang_syntax::{
    lint::{SyntaxLint, lint_id_policy},
    parser::parse_source,
};
use arcweft_source::{
    Diagnostic as ArcDiagnostic, DiagnosticApplicability, DiagnosticLabelStyle,
    DiagnosticSeverity as ArcDiagnosticSeverity, SourceName, SourceSpan,
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
}

impl DocumentAnalysis {
    /// Runs syntax, HIR lowering, profile-aware type checking, and verifier diagnostics.
    pub fn analyze(source: &str, encoding: PositionEncoding, profile: &LspProfile) -> Self {
        let line_index = LineIndex::new(source.to_owned(), encoding);
        let source_name = SourceName::path("<memory>");
        let mut verification_report = None;
        let parsed = parse_source(source.to_owned());
        let mut diagnostics = parsed
            .errors()
            .iter()
            .map(|error| lsp_diagnostic_from_arcweft(&error.diagnostic(&source_name), &line_index))
            .collect::<Vec<_>>();

        if parsed.errors().is_empty() {
            diagnostics.extend(syntax_lint_diagnostics(
                &lint_id_policy(parsed.typed_tree()),
                &line_index,
                &source_name,
            ));
            match lower_to_hir(parsed.typed_tree()) {
                Ok(hir) => {
                    let env = profile.typecheck_env();
                    let resolve = resolve_diagnostics(&hir, &line_index);
                    if resolve.is_empty() {
                        let readiness = readiness_diagnostics(&hir, &line_index);
                        if readiness.is_empty() {
                            let typecheck_report = analyze_types(&hir, &env);
                            diagnostics.extend(typecheck_diagnostics(
                                &typecheck_report.diagnostics,
                                &line_index,
                            ));
                            diagnostics.extend(typecheck_warnings(&typecheck_report.warnings));
                            if typecheck_report.diagnostics.is_empty() {
                                let report = verify_module_with_env(
                                    &hir,
                                    &env,
                                    VerificationPolicy {
                                        mode: VerificationMode::Dev,
                                        backend: BackendKind::Emit,
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
                    diagnostics.extend(errors.into_iter().map(|error| {
                        lsp_diagnostic_from_arcweft(&error.diagnostic(&source_name), &line_index)
                    }));
                }
            }
        }

        Self {
            diagnostics,
            line_index,
            verification_report,
        }
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
}

fn syntax_lint_diagnostics(
    lints: &[SyntaxLint],
    line_index: &LineIndex,
    source_name: &SourceName,
) -> Vec<Diagnostic> {
    lints
        .iter()
        .map(|lint| lsp_diagnostic_from_arcweft(&lint.diagnostic(source_name), line_index))
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
    let mut diagnostics = analysis.diagnostics;
    diagnostics.extend(profile_diagnostics(profile));
    PublishDiagnosticsParams::new(snapshot.uri().clone(), diagnostics, snapshot.version())
}

fn resolve_diagnostics(
    hir: &arcweft_lang_hir::model::HirModule,
    line_index: &LineIndex,
) -> Vec<Diagnostic> {
    let registry = registry_from_hir(hir);
    validate_hir_references(hir, &registry).map_or_else(
        |errors| {
            errors
                .iter()
                .enumerate()
                .map(|(index, error)| name_resolution_diagnostic(error, index + 1, line_index))
                .collect()
        },
        |()| Vec::new(),
    )
}

fn readiness_diagnostics(
    hir: &arcweft_lang_hir::model::HirModule,
    line_index: &LineIndex,
) -> Vec<Diagnostic> {
    validate_typecheck_ready(hir).map_or_else(
        |errors| {
            errors
                .iter()
                .enumerate()
                .map(|(index, error)| readiness_diagnostic(error, index + 1, line_index))
                .collect()
        },
        |()| Vec::new(),
    )
}

fn name_resolution_diagnostic(
    error: &NameResolutionError,
    _index: usize,
    line_index: &LineIndex,
) -> Diagnostic {
    lsp_diagnostic_from_arcweft(&error.diagnostic(), line_index)
}

fn readiness_diagnostic(
    error: &TypeCheckReadinessError,
    _index: usize,
    line_index: &LineIndex,
) -> Diagnostic {
    lsp_diagnostic_from_arcweft(&error.diagnostic(), line_index)
}

fn typecheck_diagnostics(errors: &[TypeCheckError], line_index: &LineIndex) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|error| lsp_diagnostic_from_arcweft(&error.diagnostic(), line_index))
        .collect()
}

fn typecheck_warnings(warnings: &[TypeCheckWarning]) -> Vec<Diagnostic> {
    let line_index = LineIndex::new(String::new(), PositionEncoding::Utf16);
    warnings
        .iter()
        .map(|warning| lsp_diagnostic_from_arcweft(&warning.diagnostic(), &line_index))
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

fn lsp_diagnostic_from_arcweft(diagnostic: &ArcDiagnostic, line_index: &LineIndex) -> Diagnostic {
    let span = primary_span(diagnostic);
    let range = span.map_or_else(start_range, |span| range_for_span(span, line_index));
    Diagnostic {
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
    }
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
mod route::opening

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
