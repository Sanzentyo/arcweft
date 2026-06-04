use crate::documents::DocumentSnapshot;
use crate::positions::{LineIndex, PositionEncoding};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_syntax::parser::parse_source;
use arcweft_verify::{BackendKind, VerificationMode, VerificationPolicy, verify_module};
use arcweft_verify_lsp::{LspPositionMapper, diagnostics_from_report_with_mapper};
use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, PublishDiagnosticsParams};

/// Analyzed document diagnostics plus source index used by feature handlers.
#[derive(Clone, Debug)]
pub struct DocumentAnalysis {
    diagnostics: Vec<Diagnostic>,
    line_index: LineIndex,
}

impl DocumentAnalysis {
    /// Runs syntax, HIR lowering, and verifier diagnostics for a source snapshot.
    pub fn analyze(source: &str, encoding: PositionEncoding) -> Self {
        let line_index = LineIndex::new(source.to_owned(), encoding);
        let parsed = parse_source(source.to_owned());
        let mut diagnostics = parsed
            .errors()
            .iter()
            .enumerate()
            .map(|(index, error)| Diagnostic {
                range: line_index.range_from_byte_span(error.range().start(), error.range().end()),
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String(format!(
                    "syntax.parse.{}",
                    index + 1
                ))),
                source: Some("arcweft-syntax".to_owned()),
                message: error.message().to_owned(),
                ..Diagnostic::default()
            })
            .collect::<Vec<_>>();

        if parsed.errors().is_empty() {
            match lower_to_hir(parsed.typed_tree()) {
                Ok(hir) => {
                    let report = verify_module(
                        &hir,
                        VerificationPolicy {
                            mode: VerificationMode::Dev,
                            backend: BackendKind::Emit,
                        },
                    );
                    diagnostics.extend(diagnostics_from_report_with_mapper(&report, &line_index));
                }
                Err(errors) => {
                    diagnostics.extend(errors.into_iter().enumerate().map(|(index, error)| {
                        let range = error.range().map_or_else(
                            || line_index.range_from_byte_span(0, 0),
                            |range| line_index.range_from_byte_span(range.start(), range.end()),
                        );
                        Diagnostic {
                            range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: Some(NumberOrString::String(format!("hir.lower.{}", index + 1))),
                            source: Some("arcweft-hir".to_owned()),
                            message: error.message().to_owned(),
                            ..Diagnostic::default()
                        }
                    }));
                }
            }
        }

        Self {
            diagnostics,
            line_index,
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
}

/// Builds a publishDiagnostics notification payload for one open document.
pub fn publish_diagnostics(snapshot: &DocumentSnapshot) -> PublishDiagnosticsParams {
    let analysis =
        DocumentAnalysis::analyze(snapshot.text(), snapshot.line_index().position_encoding());
    PublishDiagnosticsParams::new(
        snapshot.uri().clone(),
        analysis.diagnostics,
        snapshot.version(),
    )
}
