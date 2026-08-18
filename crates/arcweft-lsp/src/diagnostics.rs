use crate::documents::DocumentSnapshot;
use crate::positions::LineIndex;
use crate::profiles::LspProfile;
use arcweft_lang_syntax::{
    attachment::SyntaxAccessError,
    incremental::{ParsedSource, SyntaxDiagnostic},
    lint::{SyntaxLint, lint_id_policy},
};
use arcweft_source::{
    Diagnostic as ArcDiagnostic, DiagnosticCommand, DiagnosticLabel, DiagnosticLabelStyle,
    DiagnosticSeverity as ArcDiagnosticSeverity, SourceDocument, SourceSpan,
    SourceSpanValidationError,
};
use arcweft_verify::VerificationReport;
use arcweft_verify_lsp::{LspPositionMapper, profile_manifest_conformance_diagnostics};
use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString,
    Position, PublishDiagnosticsParams, Range, Uri,
};
use std::sync::Arc;

/// Analyzed document diagnostics plus source index used by feature handlers.
#[derive(Clone, Debug)]
pub struct DocumentAnalysis {
    diagnostics: Vec<Diagnostic>,
    compiler_commands: Vec<DiagnosticCommand>,
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
    fn analyze_parsed_source(
        parsed: &ParsedSource,
        line_index: LineIndex,
        profile: &LspProfile,
    ) -> Self {
        let document = Arc::clone(parsed.document_lease());
        let source_document = document.as_ref();
        let mut diagnostics = parsed
            .diagnostics()
            .iter()
            .filter_map(|error| {
                lsp_diagnostic_from_syntax_diagnostic(error, &line_index, source_document).ok()
            })
            .collect::<Vec<_>>();

        if parsed.diagnostics().is_empty() {
            match lint_id_policy(parsed) {
                Ok(lints) => {
                    diagnostics.extend(syntax_lint_diagnostics(
                        &lints,
                        &line_index,
                        source_document,
                    ));
                }
                Err(error) => {
                    diagnostics.extend(lsp_lint_projection_diagnostic(parsed, &error, &line_index));
                }
            }
        }
        diagnostics.extend(project_compile_diagnostics(
            profile,
            source_document,
            &line_index,
        ));
        let compiler_commands = project_compile_diagnostic_commands(profile, source_document);

        let verification_report = profile.accepted_environment().and_then(|accepted| {
            let accepted_source = accepted.project().sources().get(document.identity())?;
            if !Arc::ptr_eq(accepted_source.document(), &document) {
                return None;
            }
            let executable = accepted.executable()?;
            diagnostics.extend(executable.final_analysis().diagnostics().iter().filter_map(
                |diagnostic| {
                    lsp_diagnostic_from_arcweft(diagnostic, &line_index, source_document).ok()
                },
            ));
            Some(executable.verification().as_ref().clone())
        });

        Self {
            diagnostics,
            compiler_commands,
            line_index,
            verification_report,
            document,
        }
    }

    /// Runs analysis against the exact source document and negotiated line index of an open
    /// snapshot.
    pub fn analyze_snapshot(snapshot: &DocumentSnapshot, profile: &LspProfile) -> Self {
        Self::analyze_parsed_source(
            snapshot.parsed_source(),
            snapshot.line_index().clone(),
            profile,
        )
    }

    /// Diagnostics emitted for the analyzed document.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Compiler/verifier commands retained from exact source-backed project diagnostics.
    pub(crate) fn compiler_commands(&self) -> &[DiagnosticCommand] {
        &self.compiler_commands
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

fn lsp_lint_projection_diagnostic(
    parsed: &ParsedSource,
    error: &SyntaxAccessError,
    line_index: &LineIndex,
) -> Option<Diagnostic> {
    let diagnostic = ArcDiagnostic::new(ArcDiagnosticSeverity::Error, error.to_string())
        .with_code("syntax.lint.projection")
        .with_label(DiagnosticLabel::primary(
            parsed.root_syntax().source_span(),
            Some("attached syntax lint projection failed".to_owned()),
        ));
    lsp_diagnostic_from_arcweft(&diagnostic, line_index, parsed.document()).ok()
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
    diagnostics.extend(profile_diagnostics(profile));
    PublishDiagnosticsParams::new(
        snapshot.uri().clone(),
        diagnostics,
        Some(snapshot.version()),
    )
}

fn profile_diagnostics(profile: &LspProfile) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for diagnostic in profile.diagnostics() {
        let compiler = diagnostic.project_compile_diagnostics();
        if compiler.is_empty() {
            diagnostics.push(profile_diagnostic(diagnostic));
            continue;
        }
        if compiler.iter().all(|diagnostic| {
            let source_diagnostic = diagnostic.syntax_diagnostic().map_or_else(
                || diagnostic.diagnostic().clone(),
                SyntaxDiagnostic::diagnostic,
            );
            primary_span(&source_diagnostic).is_none()
        }) {
            diagnostics.push(profile_diagnostic(diagnostic));
        }
    }
    diagnostics.extend(profile_manifest_conformance_diagnostics(
        &profile.context(),
        profile.declared_manifests(),
    ));
    diagnostics
}

fn project_compile_diagnostics(
    profile: &LspProfile,
    document: &SourceDocument,
    line_index: &LineIndex,
) -> Vec<Diagnostic> {
    let projector = DiagnosticProjector::new(document, line_index);
    profile
        .diagnostics()
        .iter()
        .flat_map(crate::profiles::LspProfileDiagnostic::project_compile_diagnostics)
        .filter_map(|diagnostic| {
            let source_diagnostic = diagnostic.syntax_diagnostic().map_or_else(
                || diagnostic.diagnostic().clone(),
                SyntaxDiagnostic::diagnostic,
            );
            projector.project(&source_diagnostic).ok()
        })
        .collect()
}

fn project_compile_diagnostic_commands(
    profile: &LspProfile,
    document: &SourceDocument,
) -> Vec<DiagnosticCommand> {
    profile
        .diagnostics()
        .iter()
        .flat_map(crate::profiles::LspProfileDiagnostic::project_compile_diagnostics)
        .filter_map(|diagnostic| {
            let source_diagnostic = diagnostic.syntax_diagnostic().map_or_else(
                || diagnostic.diagnostic().clone(),
                SyntaxDiagnostic::diagnostic,
            );
            (primary_span(&source_diagnostic).is_some()
                && source_diagnostic.validate_source(document).is_ok())
            .then_some(source_diagnostic)
        })
        .flat_map(|diagnostic| diagnostic.commands().to_vec())
        .collect()
}

fn profile_diagnostic(diagnostic: &crate::profiles::LspProfileDiagnostic) -> Diagnostic {
    Diagnostic {
        range: start_range(),
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String(diagnostic.kind().code().to_owned())),
        source: Some("arcweft-lsp-profile".to_owned()),
        message: diagnostic.message().to_owned(),
        ..Diagnostic::default()
    }
}

fn lsp_diagnostic_from_arcweft(
    diagnostic: &ArcDiagnostic,
    line_index: &LineIndex,
    document: &SourceDocument,
) -> Result<Diagnostic, SourceSpanValidationError> {
    DiagnosticProjector::new(document, line_index).project(diagnostic)
}

fn lsp_diagnostic_from_syntax_diagnostic(
    error: &SyntaxDiagnostic,
    line_index: &LineIndex,
    document: &SourceDocument,
) -> Result<Diagnostic, SourceSpanValidationError> {
    lsp_diagnostic_from_arcweft(&error.diagnostic(), line_index, document)
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
    use crate::documents::{AcceptedOpenDocument, DocumentStore};
    use crate::positions::PositionEncoding;
    use crate::profiles::{LspProfileResolver, LspProfileTestHarness};
    use arcweft_adapter_context::manifest::{
        AdapterCallableGroupIndex, AdapterCallableName, AdapterCallableOverloadIndex,
        AdapterCallableParameterIndex, AdapterCallablePath, AdapterFunctionParam,
        AdapterFunctionSignature, AdapterManifest, AdapterParameterGroup, AdapterParameterPassing,
        AdapterParameterPresence, AdapterTypeKind,
    };
    use arcweft_core::effect::{
        RuntimeAssertion, RuntimeAssertionFailure, RuntimeAssertionGuardId, RuntimeAssertionProfile,
    };
    use arcweft_lang_syntax::{incremental::SyntaxDatabase, parser::ParseOptions};
    use arcweft_runtime_host::RuntimeHostRunnerKind;
    use arcweft_source::{
        DiagnosticApplicability, DiagnosticLabel, DiagnosticSuggestion, SourceDocumentId,
        SourceEdit, SourceName, SourceRange, identity::SourceSnapshotId,
    };
    use arcweft_tooling::runtime_diagnostic::project_persisted_assertion_failure;
    use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem};
    use std::{
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn parse_fixture(document: Arc<SourceDocument>) -> ParsedSource {
        let mut database = SyntaxDatabase::try_new().expect("test syntax database");
        database
            .parse_initial(
                SourceSnapshotId::initial(document.display_name().clone()),
                document,
                ParseOptions::default(),
            )
            .expect("attached LSP syntax fixture")
    }

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
        let parsed = parse_fixture(document);
        DocumentAnalysis::analyze_parsed_source(&parsed, line_index, profile)
    }

    fn analyze_project_fixture(source: &str, encoding: PositionEncoding) -> DocumentAnalysis {
        analyze_project_fixture_with_adapter(source, encoding, None)
    }

    fn analyze_project_fixture_with_adapter(
        source: &str,
        encoding: PositionEncoding,
        adapter: Option<AdapterManifest>,
    ) -> DocumentAnalysis {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("arcweft-lsp-diagnostics-{unique}"));
        let source_path = root.join("src/main.arcw");
        fs::create_dir_all(source_path.parent().expect("source parent")).expect("project root");
        fs::write(
            root.join("arcw.toml"),
            r#"schema = 1

[package]
id = "org.arcweft.tests.lsp-diagnostics"
version = "0.1.0"

[profiles.agent]
kind = "agent"
entry = "@entry.agent.main"
source = "src/main.arcw"
"#,
        )
        .expect("manifest");
        let source = format!(
            "{source}\nfn lsp_test_controller() -> Result<Unit, AgentError>\neffects {{}}\n{{\n    Ok(())\n}}\n\nentry agent @entry.agent.main {{\n    controller = lsp_test_controller\n}}\n"
        );
        fs::write(&source_path, &source).expect("source");
        let profile = LspProfileTestHarness::new(LspProfileResolver::new(
            RuntimeHostRunnerKind::Native,
            Some("agent".to_owned()),
        ))
        .resolve_for_document_path(&source_path)
        .expect("profile construction")
        .publish_for_test();
        let profile = match adapter {
            Some(adapter) => profile.with_adapter_for_test(adapter),
            None => profile,
        };
        let uri = file_uri(&source_path);
        let accepted = profile.accepted_environment().expect("accepted profile");
        let accepted_source = accepted
            .project()
            .sources()
            .by_uri(&uri)
            .expect("accepted source");
        let authority = AcceptedOpenDocument::new(Arc::clone(accepted_source.document()), None);
        let mut store = DocumentStore::default();
        let document = store
            .open_with_authority(
                DidOpenTextDocumentParams {
                    text_document: TextDocumentItem::new(uri, "arcweft".to_owned(), 1, source),
                },
                encoding,
                Some(&authority),
            )
            .expect("accepted document open");
        let analysis = DocumentAnalysis::analyze_snapshot(&document, &profile);
        let _ = fs::remove_dir_all(root);
        analysis
    }

    fn file_uri(path: &Path) -> Uri {
        format!(
            "file:///{}",
            path.to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches('/')
        )
        .parse()
        .expect("file URI")
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

        let parsed = parse_fixture(Arc::clone(&document));
        let analysis = DocumentAnalysis::analyze_parsed_source(
            &parsed,
            LineIndex::new(document.text(), PositionEncoding::Utf16),
            &LspProfile::default_for_runner(RuntimeHostRunnerKind::Native),
        );

        assert!(Arc::ptr_eq(analysis.source_document(), &document));
    }

    #[test]
    fn lsp_projector_consumes_shared_runtime_assertion_diagnostic() {
        let source = "flow checks { assert.check(ready) }\n";
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("file:///workspace/runtime-assertion.arcw")
                .expect("fixture document ID"),
            SourceName::path("runtime-assertion.arcw"),
            source,
        )
        .expect("fixture document");
        let condition_start = source.find("ready").expect("condition source");
        let condition_span = document
            .span(SourceRange::new(
                condition_start,
                condition_start + "ready".len(),
            ))
            .expect("condition span");
        let failure = RuntimeAssertionFailure::new(RuntimeAssertion::new(
            RuntimeAssertionGuardId::try_from_bytes([0x51; 16]).expect("fixture guard"),
            "ready".to_owned(),
            "runtime condition failed".to_owned(),
            RuntimeAssertionProfile::Always,
        ));
        let diagnostic = project_persisted_assertion_failure(&failure, Some(condition_span))
            .to_source_diagnostic();
        let line_index = LineIndex::new(source, PositionEncoding::Utf16);
        let projected = DiagnosticProjector::new(&document, &line_index)
            .project(&diagnostic)
            .expect("runtime diagnostic belongs to exact source revision");

        assert_eq!(
            projected.code,
            Some(NumberOrString::String(
                "runtime.assertion_failed".to_owned()
            ))
        );
        assert_eq!(projected.source.as_deref(), Some("arcweft-runtime"));
        assert_eq!(projected.range.start.line, 0);
        assert_eq!(
            projected.range.end.character - projected.range.start.character,
            5
        );
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
    fn propagation_diagnostics_project_exact_try_operator() {
        for (parameter_type, expression, code, utf16, utf8) in [
            (
                "Result<i64, String>",
                "try value",
                "sema.try.error_mismatch",
                12..15,
                14..17,
            ),
            (
                "Need<i64, String>",
                "try await value",
                "sema.try.error_mismatch",
                12..15,
                14..17,
            ),
        ] {
            let source = format!(
                "fn demo(value: {parameter_type}) -> Result<i64, i64> {{\n    /* 前 */ {expression}\n}}\n"
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
                let analysis = analyze_project_fixture(&source, encoding);
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
flow @flow.main main {
    let value = custom_echo("hello")
}
"#;
        let default_analysis = analyze_project_fixture(source, PositionEncoding::Utf16);
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
        let profile_analysis =
            analyze_project_fixture_with_adapter(source, PositionEncoding::Utf16, Some(adapter));

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
        let analysis = analyze_project_fixture(source, PositionEncoding::Utf16);

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
        let analysis = analyze_project_fixture(source, PositionEncoding::Utf16);
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
        assert!(diagnostic.message.contains("defaults to i32"));
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
        let analysis = analyze_project_fixture(source, PositionEncoding::Utf16);
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
        let source = r#"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
    fn read_metadata(path: String) -> String effects { fs.read }
    fn read_dormant(path: String) -> String effects { fs.read }
}

fn unrelated_read() -> String effects { fs.read } {
    fs.read_metadata(path = "unrelated.arcw")
}

fn unused_factory() -> ((Unit) -> String effects { fs.read }) {
    |_unit: Unit| -> String { fs.read_dormant(path = "dormant.arcw") }
}

fn load_story() -> String
effects {}
{
    fs.read_text(path = "story.arcw")
}
"#;
        let analysis = analyze_project_fixture(source, PositionEncoding::Utf16);

        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.code == Some(NumberOrString::String("AWF-EFX-001".to_owned()))
            })
            .expect("upper-bound effect error is surfaced");
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostic.message.contains("fs.read"));
    }

    #[test]
    fn diagnostics_surface_returned_closure_effect_trace() {
        let source = r#"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
}

fn make_loader(
    load: (String) -> String effects { fs.read }
) -> ((Unit) -> String effects { fs.read }) {
    |_unit: Unit| -> String { load("story.arcw") }
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
        let analysis = analyze_project_fixture(source, PositionEncoding::Utf16);

        let diagnostic = analysis
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.code == Some(NumberOrString::String("AWF-EFX-001".to_owned()))
            })
            .unwrap_or_else(|| {
                panic!(
                    "returned closure effect error is surfaced: {:#?}",
                    analysis.diagnostics()
                )
            });
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
        assert!(!rendered.contains("fs.read_metadata"), "{rendered}");
        assert!(!rendered.contains("fs.read_dormant"), "{rendered}");
    }

    #[test]
    fn diagnostics_surface_performed_effect_trace() {
        let source = r"
fn await_avatar(avatar: Need<String, String>) -> Result<String, String>
effects { }
{
    await avatar
}
";
        let analysis = analyze_project_fixture(source, PositionEncoding::Utf16);

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
            rendered.contains("`await_avatar` performs `control.suspend`"),
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
