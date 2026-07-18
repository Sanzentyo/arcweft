//! Typed Agent projections for parser and shared source diagnostics.

use arcweft_lang_syntax::parser::recovery::{ParseError, RecoverySuggestion};
use arcweft_source::{
    Diagnostic, DiagnosticSeverity, SourceDocument, SourceRange, SourceSpan, SourceSpanError,
    SourceSpanValidationError,
};
use serde_json::{Value, json};
use thiserror::Error;

/// Revision-validating adapter for shared source diagnostics.
pub struct AgentDiagnosticProjector<'a> {
    document: &'a SourceDocument,
}

/// One shared diagnostic validated for source-local Agent projection.
pub struct AgentDiagnosticProjection<'a> {
    diagnostic: &'a Diagnostic,
}

/// Source-local projection of one typed parser diagnostic.
pub struct AgentParserDiagnosticProjection<'a> {
    diagnostic: &'a ParseError,
    source_base: usize,
}

/// Failure to bind a parser diagnostic to a source-local coordinate mapping.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AgentParserDiagnosticProjectionError {
    /// A parser range is invalid for the supplied synthetic document bytes.
    #[error(transparent)]
    InvalidSpan(#[from] SourceSpanError),
    /// The mapped synthetic slice differs from the exact authored source.
    #[error("synthetic source mapping does not contain the exact authored source bytes")]
    SourceTextMismatch,
    /// A parser range lies outside the validated authored-source mapping.
    #[error(
        "parser diagnostic range [{start},{end}) lies outside mapped source [{mapped_start},{mapped_end})"
    )]
    OutsideMappedSource {
        start: usize,
        end: usize,
        mapped_start: usize,
        mapped_end: usize,
    },
}

impl<'a> AgentDiagnosticProjector<'a> {
    /// Binds shared projections to one exact source document revision.
    pub const fn new(document: &'a SourceDocument) -> Self {
        Self { document }
    }

    /// Validates and retains one shared diagnostic for Agent projection.
    pub fn project<'diagnostic>(
        &self,
        diagnostic: &'diagnostic Diagnostic,
    ) -> Result<AgentDiagnosticProjection<'diagnostic>, SourceSpanValidationError> {
        diagnostic.validate_source(self.document)?;
        Ok(AgentDiagnosticProjection { diagnostic })
    }
}

impl AgentDiagnosticProjection<'_> {
    /// Structured Agent JSON using explicit source-local UTF-8 byte coordinates.
    #[must_use]
    pub fn json(&self) -> Value {
        let diagnostic = self.diagnostic;
        json!({
            "code": diagnostic.code().map(arcweft_source::DiagnosticCode::as_str),
            "message": diagnostic.message(),
            "range": primary_span(diagnostic).map(|span| {
                json!({
                    "coordinate_space": "source_utf8_bytes",
                    "start": span.range().start(),
                    "end": span.range().end(),
                })
            }),
            "notes": diagnostic.notes(),
            "recovery": diagnostic.suggestions().iter().map(|suggestion| {
                json!({
                    "message": suggestion.message(),
                    "applicability": suggestion.applicability().as_str(),
                    "edits": suggestion.edits().iter().map(|edit| {
                        json!({
                            "range": {
                                "coordinate_space": "source_utf8_bytes",
                                "start": edit.span().range().start(),
                                "end": edit.span().range().end(),
                            },
                            "replacement": edit.replacement(),
                        })
                    }).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
        })
    }

    /// Human Agent rendering without parsing code or message text.
    #[must_use]
    pub fn human(&self) -> String {
        let diagnostic = self.diagnostic;
        let code = diagnostic
            .code()
            .map_or("unclassified", arcweft_source::DiagnosticCode::as_str);
        let mut lines = vec![primary_span(diagnostic).map_or_else(
            || {
                format!(
                    "{}[{code}]: {}",
                    severity_name(diagnostic.severity()),
                    diagnostic.message()
                )
            },
            |span| {
                format!(
                    "{}[{code}] source_utf8_bytes {}..{}: {}",
                    severity_name(diagnostic.severity()),
                    span.range().start(),
                    span.range().end(),
                    diagnostic.message()
                )
            },
        )];
        lines.extend(
            diagnostic
                .notes()
                .iter()
                .map(|note| format!("note: {note}")),
        );
        for suggestion in diagnostic.suggestions() {
            lines.push(format!(
                "help[{}]: {}",
                suggestion.applicability().as_str(),
                suggestion.message()
            ));
            lines.extend(suggestion.edits().iter().map(|edit| {
                format!(
                    "edit source_utf8_bytes {}..{}: {:?}",
                    edit.span().range().start(),
                    edit.span().range().end(),
                    edit.replacement()
                )
            }));
        }
        lines.join("\n")
    }
}

impl<'a> AgentParserDiagnosticProjection<'a> {
    /// Validates a parser diagnostic against its direct authored source.
    pub fn source_local(
        diagnostic: &'a ParseError,
        document: &SourceDocument,
    ) -> Result<Self, AgentParserDiagnosticProjectionError> {
        let mapped = SourceRange::new(0, document.text().len());
        validate_parser_ranges(diagnostic, document, mapped)?;
        Ok(Self {
            diagnostic,
            source_base: 0,
        })
    }

    /// Validates and removes an exact synthetic wrapper around authored source bytes.
    pub fn dewrapped(
        diagnostic: &'a ParseError,
        synthetic_document: &SourceDocument,
        source_document: &SourceDocument,
        synthetic_source_range: SourceRange,
    ) -> Result<Self, AgentParserDiagnosticProjectionError> {
        synthetic_document.span(synthetic_source_range)?;
        if &synthetic_document.text()[synthetic_source_range.as_range()] != source_document.text() {
            return Err(AgentParserDiagnosticProjectionError::SourceTextMismatch);
        }
        validate_parser_ranges(diagnostic, synthetic_document, synthetic_source_range)?;
        Ok(Self {
            diagnostic,
            source_base: synthetic_source_range.start(),
        })
    }

    /// Structured Agent JSON preserving the complete typed parser payload.
    #[must_use]
    pub fn json(&self) -> Value {
        let diagnostic = self.diagnostic;
        let (start, end) = self.local_range(diagnostic.range().start(), diagnostic.range().end());
        json!({
            "kind": diagnostic.label(),
            "code": diagnostic.code(),
            "message": diagnostic.message(),
            "range": {
                "coordinate_space": "source_utf8_bytes",
                "start": start,
                "end": end,
            },
            "expected": diagnostic.expected(),
            "found": diagnostic.found(),
            "recovery": diagnostic.recovery().iter().map(|suggestion| {
                json!({
                    "message": suggestion.message(),
                    "applicability": suggestion.applicability().as_str(),
                    "edits": suggestion.edits().iter().map(|edit| {
                        let (start, end) =
                            self.local_range(edit.range().start(), edit.range().end());
                        json!({
                            "range": {
                                "coordinate_space": "source_utf8_bytes",
                                "start": start,
                                "end": end,
                            },
                            "replacement": edit.replacement(),
                        })
                    }).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
        })
    }

    /// Human Agent rendering preserving typed parser fields and coordinates.
    #[must_use]
    pub fn human(&self) -> String {
        let diagnostic = self.diagnostic;
        let (start, end) = self.local_range(diagnostic.range().start(), diagnostic.range().end());
        let mut lines = vec![format!(
            "error[{}] source_utf8_bytes {start}..{end}: {}",
            diagnostic.code(),
            diagnostic.message()
        )];
        if !diagnostic.expected().is_empty() {
            lines.push(format!("expected: {}", diagnostic.expected().join(", ")));
        }
        if let Some(found) = diagnostic.found() {
            lines.push(format!("found: {found}"));
        }
        for suggestion in diagnostic.recovery() {
            lines.push(format!(
                "help[{}]: {}",
                suggestion.applicability().as_str(),
                suggestion.message()
            ));
            lines.extend(suggestion.edits().iter().map(|edit| {
                let (start, end) = self.local_range(edit.range().start(), edit.range().end());
                format!(
                    "edit source_utf8_bytes {start}..{end}: {:?}",
                    edit.replacement()
                )
            }));
        }
        lines.join("\n")
    }

    fn local_range(&self, start: usize, end: usize) -> (usize, usize) {
        (start - self.source_base, end - self.source_base)
    }
}

fn validate_parser_ranges(
    diagnostic: &ParseError,
    document: &SourceDocument,
    mapped: SourceRange,
) -> Result<(), AgentParserDiagnosticProjectionError> {
    validate_parser_range(
        diagnostic.range().start(),
        diagnostic.range().end(),
        document,
        mapped,
    )?;
    diagnostic
        .recovery()
        .iter()
        .flat_map(RecoverySuggestion::edits)
        .try_for_each(|edit| {
            validate_parser_range(edit.range().start(), edit.range().end(), document, mapped)
        })
}

fn validate_parser_range(
    start: usize,
    end: usize,
    document: &SourceDocument,
    mapped: SourceRange,
) -> Result<(), AgentParserDiagnosticProjectionError> {
    document.span(SourceRange::new(start, end))?;
    if start < mapped.start() || end > mapped.end() {
        return Err(AgentParserDiagnosticProjectionError::OutsideMappedSource {
            start,
            end,
            mapped_start: mapped.start(),
            mapped_end: mapped.end(),
        });
    }
    Ok(())
}

fn primary_span(diagnostic: &Diagnostic) -> Option<&SourceSpan> {
    diagnostic.span().or_else(|| {
        diagnostic
            .labels()
            .first()
            .map(arcweft_source::DiagnosticLabel::span)
    })
}

const fn severity_name(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Info => "info",
        DiagnosticSeverity::Hint => "hint",
    }
}

#[cfg(test)]
mod tests {
    use arcweft_lang_syntax::parser::{parse_source, recovery::ParseErrorKind};
    use arcweft_source::{
        Diagnostic, DiagnosticApplicability, DiagnosticLabel, DiagnosticSeverity,
        DiagnosticSuggestion, SourceDocument, SourceDocumentId, SourceEdit, SourceName,
        SourceRange, SourceSpanValidationError,
    };
    use serde_json::{Value, json};

    use super::{AgentDiagnosticProjector, AgentParserDiagnosticProjection};

    const SOURCE: &str = "pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n";

    fn document(id: &str, source: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("fixture source id"),
            SourceName::Generated,
            source,
        )
        .expect("fixture source document")
    }

    #[test]
    fn parser_diagnostic_projection_preserves_the_editless_source_payload() {
        let parsed = parse_source(SOURCE);
        let diagnostic = parsed
            .errors()
            .iter()
            .find(|diagnostic| diagnostic.kind() == ParseErrorKind::ViewExportPartMissingAs)
            .expect("missing-`as` diagnostic");
        let projection =
            AgentParserDiagnosticProjection::source_local(diagnostic, parsed.document())
                .expect("source-local projection");

        assert_eq!(
            projection.json(),
            json!({
                "kind": ParseErrorKind::ViewExportPartMissingAs.label(),
                "code": "view::export_part_missing_as",
                "message": "View part export needs `as` before its public name",
                "range": {
                    "coordinate_space": "source_utf8_bytes",
                    "start": 47,
                    "end": 54,
                },
                "expected": ["as public_name"],
                "found": Value::Null,
                "recovery": [{
                    "message": "use as public_name syntax",
                    "applicability": "unspecified",
                    "edits": [],
                }],
            })
        );
        assert_eq!(
            projection.human(),
            "error[view::export_part_missing_as] source_utf8_bytes 47..54: View part export needs `as` before its public name\nexpected: as public_name\nhelp[unspecified]: use as public_name syntax"
        );
    }

    #[test]
    fn parser_diagnostic_projection_dewraps_an_exact_synthetic_prefix() {
        let prefix = "// synthetic wrapper\n";
        let synthetic_source = format!("{prefix}{SOURCE}");
        let parsed = parse_source(&synthetic_source);
        let diagnostic = parsed
            .errors()
            .iter()
            .find(|diagnostic| diagnostic.kind() == ParseErrorKind::ViewExportPartMissingAs)
            .expect("wrapped missing-`as` diagnostic");
        let authored = document("arcweft-agent://cell/0", SOURCE);
        let projection = AgentParserDiagnosticProjection::dewrapped(
            diagnostic,
            parsed.document(),
            &authored,
            SourceRange::new(prefix.len(), synthetic_source.len()),
        )
        .expect("exact wrapper dewrap");

        assert_eq!(
            projection.json()["range"],
            json!({
                "coordinate_space": "source_utf8_bytes",
                "start": 47,
                "end": 54,
            })
        );
    }

    #[test]
    fn shared_diagnostic_projection_preserves_the_test_only_edit() {
        let document = document("arcweft-agent://diagnostic/0", SOURCE);
        let diagnostic = Diagnostic::new(
            DiagnosticSeverity::Error,
            "View part export needs `as` before its public name",
        )
        .with_code("view::export_part_missing_as")
        .with_label(DiagnosticLabel::primary(
            document
                .span(SourceRange::new(47, 54))
                .expect("diagnostic span"),
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
                    .expect("insertion span"),
                "as ",
            )),
        );
        let projection = AgentDiagnosticProjector::new(&document)
            .project(&diagnostic)
            .expect("exact revision projection");

        assert_eq!(
            projection.json()["recovery"][0]["applicability"],
            "machine_applicable"
        );
        assert_eq!(
            projection.json()["recovery"][0]["edits"][0],
            json!({
                "range": {
                    "coordinate_space": "source_utf8_bytes",
                    "start": 47,
                    "end": 47,
                },
                "replacement": "as ",
            })
        );
    }

    #[test]
    fn shared_diagnostic_projection_rejects_stale_diagnostics_and_edits() {
        let id = "arcweft-agent://diagnostic/0";
        let original = document(id, SOURCE);
        let diagnostic = Diagnostic::new(DiagnosticSeverity::Error, "stale")
            .with_label(DiagnosticLabel::primary(
                original
                    .span(SourceRange::new(47, 54))
                    .expect("diagnostic span"),
                None,
            ))
            .with_suggestion(
                DiagnosticSuggestion::new("stale edit", DiagnosticApplicability::MachineApplicable)
                    .with_edit(SourceEdit::new(
                        original
                            .span(SourceRange::new(47, 47))
                            .expect("insertion span"),
                        "as ",
                    )),
            );
        let current = document(
            id,
            "pub view Card() {\n    export part タイトル as heading\n    Panel()\n}\n",
        );

        assert!(matches!(
            AgentDiagnosticProjector::new(&current).project(&diagnostic),
            Err(SourceSpanValidationError::WrongRevision { expected, actual })
                if expected == current.identity().revision()
                    && actual == original.identity().revision()
        ));

        let current_diagnostic = Diagnostic::new(DiagnosticSeverity::Error, "current diagnostic")
            .with_label(DiagnosticLabel::primary(
                current
                    .span(SourceRange::new(50, 57))
                    .expect("current heading span"),
                None,
            ))
            .with_suggestion(
                DiagnosticSuggestion::new("stale edit", DiagnosticApplicability::MachineApplicable)
                    .with_edit(SourceEdit::new(
                        original
                            .span(SourceRange::new(47, 47))
                            .expect("stale insertion span"),
                        "as ",
                    )),
            );
        assert!(matches!(
            AgentDiagnosticProjector::new(&current).project(&current_diagnostic),
            Err(SourceSpanValidationError::WrongRevision { expected, actual })
                if expected == current.identity().revision()
                    && actual == original.identity().revision()
        ));
    }
}
