//! Typed Agent projections for parser and shared source diagnostics.

use arcweft_agent_protocol::ids::{AgentRunId, SessionId, StableHash};
use arcweft_core::effect::{RuntimeArtifactFingerprint, RuntimeAssertionFailure};
use arcweft_debug_model::diagnostic::DebugDiagnostic;
use arcweft_lang_syntax::incremental::SyntaxDiagnostic;
use arcweft_source::{
    Diagnostic, DiagnosticLabelStyle, DiagnosticSeverity, SourceDocument, SourceRange, SourceSpan,
    SourceSpanError, SourceSpanValidationError,
};
use arcweft_tooling::runtime_diagnostic::RuntimeAssertionDiagnostic;
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
pub struct AgentSyntaxDiagnosticProjection<'a> {
    diagnostic: &'a SyntaxDiagnostic,
    source_base: usize,
}

/// Debug-record metadata supplied by the Agent/debug session owner.
///
/// This context identifies the existing debug program/record and Agent run
/// only. The runtime-plan artifact fingerprint is supplied separately and
/// retained in the typed failure payload. This context never owns a HIR
/// statement, syntax node, assertion condition index, or runtime-plan session
/// identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAssertionDebugContext {
    diagnostic_id: String,
    program_hash: Option<StableHash>,
    session_id: Option<SessionId>,
    run_id: Option<AgentRunId>,
    sequence: Option<u64>,
    created_unix_ms: i64,
}

impl RuntimeAssertionDebugContext {
    #[must_use]
    pub fn new(
        diagnostic_id: impl Into<String>,
        program_hash: Option<StableHash>,
        session_id: Option<SessionId>,
        run_id: Option<AgentRunId>,
        sequence: Option<u64>,
        created_unix_ms: i64,
    ) -> Self {
        Self {
            diagnostic_id: diagnostic_id.into(),
            program_hash,
            session_id,
            run_id,
            sequence,
            created_unix_ms,
        }
    }
}

/// Failure to bind a parser diagnostic to a source-local coordinate mapping.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AgentSyntaxDiagnosticProjectionError {
    /// A parser range is invalid for the supplied synthetic document bytes.
    #[error(transparent)]
    InvalidSpan(#[from] SourceSpanError),
    /// A bound diagnostic belongs to a different immutable source identity.
    #[error(transparent)]
    InvalidSource(#[from] SourceSpanValidationError),
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
            "labels": diagnostic.labels().iter().map(|label| {
                json!({
                    "style": diagnostic_label_style_name(label.style()),
                    "message": label.message(),
                    "range": {
                        "coordinate_space": "source_utf8_bytes",
                        "start": label.span().range().start(),
                        "end": label.span().range().end(),
                    },
                })
            }).collect::<Vec<_>>(),
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
                .labels()
                .iter()
                .filter(|label| label.style() == DiagnosticLabelStyle::Secondary)
                .map(|label| {
                    let range = label.span().range();
                    label.message().map_or_else(
                        || {
                            format!(
                                "related source_utf8_bytes {}..{}",
                                range.start(),
                                range.end()
                            )
                        },
                        |message| {
                            format!(
                                "related source_utf8_bytes {}..{}: {message}",
                                range.start(),
                                range.end()
                            )
                        },
                    )
                }),
        );
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

/// Projects one shared runtime-assertion diagnostic into the existing debug
/// diagnostic record.
///
/// Persisted identity remains the typed core artifact fingerprint and failure
/// payload. Source labels are presentation evidence only. The session-only
/// identity carried by `RuntimeAssertionDiagnostic` is deliberately not copied
/// into the debug payload, so the record cannot claim a `StmtId`, HIR snapshot,
/// assertion mode, or condition index after reload.
#[must_use]
pub fn project_runtime_assertion_debug_diagnostic(
    context: RuntimeAssertionDebugContext,
    artifact: RuntimeArtifactFingerprint,
    failure: &RuntimeAssertionFailure,
    diagnostic: &RuntimeAssertionDiagnostic,
) -> DebugDiagnostic {
    let primary = diagnostic.primary();
    let source_path = primary.map(|label| label.span().source().id().as_str().to_owned());
    let start_byte = primary.map(|label| source_offset(label.span().range().start()));
    let end_byte = primary.map(|label| source_offset(label.span().range().end()));
    let primary_evidence = primary.map(|label| runtime_assertion_debug_label("primary", label));
    let secondary_evidence = diagnostic
        .secondary()
        .iter()
        .map(|label| runtime_assertion_debug_label("secondary", label))
        .collect::<Vec<_>>();

    DebugDiagnostic {
        diagnostic_id: context.diagnostic_id,
        program_hash: context.program_hash,
        session_id: context.session_id,
        run_id: context.run_id,
        sequence: context.sequence,
        code: Some(diagnostic.code().to_owned()),
        severity: "error".to_owned(),
        phase: "runtime".to_owned(),
        message: diagnostic.message().to_owned(),
        source_path,
        start_byte,
        end_byte,
        related_ids: Vec::new(),
        payload: json!({
            "artifact_fingerprint": artifact,
            "failure": failure,
            "source_evidence": {
                "primary": primary_evidence,
                "secondary": secondary_evidence,
            },
        }),
        created_unix_ms: context.created_unix_ms,
    }
}

fn runtime_assertion_debug_label(
    role: &'static str,
    label: &arcweft_tooling::runtime_diagnostic::RuntimeDiagnosticLabel,
) -> Value {
    let span = label.span();
    json!({
        "role": role,
        "source_id": span.source().id().as_str(),
        "source_revision": span.source().revision().to_hex(),
        "source_len": span.source().source_len(),
        "start_byte": source_offset(span.range().start()),
        "end_byte": source_offset(span.range().end()),
        "message": label.message(),
    })
}

fn source_offset(offset: usize) -> u64 {
    u64::try_from(offset).expect("validated source offsets fit their u64 document length")
}

const fn diagnostic_label_style_name(style: DiagnosticLabelStyle) -> &'static str {
    match style {
        DiagnosticLabelStyle::Primary => "primary",
        DiagnosticLabelStyle::Secondary => "secondary",
    }
}

impl<'a> AgentSyntaxDiagnosticProjection<'a> {
    /// Validates an attached diagnostic against its exact authored source.
    pub fn source_local(
        diagnostic: &'a SyntaxDiagnostic,
        document: &SourceDocument,
    ) -> Result<Self, AgentSyntaxDiagnosticProjectionError> {
        validate_syntax_diagnostic(
            diagnostic,
            document,
            SourceRange::new(0, document.text().len()),
        )?;
        Ok(Self {
            diagnostic,
            source_base: 0,
        })
    }

    /// Validates and removes an exact synthetic wrapper around authored source bytes.
    pub fn dewrapped(
        diagnostic: &'a SyntaxDiagnostic,
        synthetic_document: &SourceDocument,
        source_document: &SourceDocument,
        synthetic_source_range: SourceRange,
    ) -> Result<Self, AgentSyntaxDiagnosticProjectionError> {
        synthetic_document.span(synthetic_source_range)?;
        if &synthetic_document.text()[synthetic_source_range.as_range()] != source_document.text() {
            return Err(AgentSyntaxDiagnosticProjectionError::SourceTextMismatch);
        }
        validate_syntax_diagnostic(diagnostic, synthetic_document, synthetic_source_range)?;
        Ok(Self {
            diagnostic,
            source_base: synthetic_source_range.start(),
        })
    }

    /// Structured Agent JSON retaining the accepted attached diagnostic identity.
    #[must_use]
    pub fn json(&self) -> Value {
        let diagnostic = self.diagnostic;
        let primary = diagnostic.primary().range();
        let (start, end) = self.local_range(primary.start(), primary.end());
        json!({
            "kind": "attached_source",
            "code": diagnostic.code(),
            "message": diagnostic.message(),
            "range": {
                "coordinate_space": "source_utf8_bytes",
                "start": start,
                "end": end,
            },
            "related": diagnostic.related().into_iter().map(|related| {
                let range = related.range();
                let (start, end) = self.local_range(range.start(), range.end());
                json!({
                    "range": {
                        "coordinate_space": "source_utf8_bytes",
                        "start": start,
                        "end": end,
                    },
                    "message": "related syntax recovery",
                })
            }).collect::<Vec<_>>(),
            "expected": [],
            "found": Value::Null,
            "recovery": [],
        })
    }

    /// Human Agent rendering of the attached diagnostic and exact coordinates.
    #[must_use]
    pub fn human(&self) -> String {
        let diagnostic = self.diagnostic;
        let primary = diagnostic.primary().range();
        let (start, end) = self.local_range(primary.start(), primary.end());
        let mut lines = vec![format!(
            "error[{}] source_utf8_bytes {start}..{end}: {}",
            diagnostic.code(),
            diagnostic.message()
        )];
        lines.extend(diagnostic.related().into_iter().map(|related| {
            let range = related.range();
            let (start, end) = self.local_range(range.start(), range.end());
            format!("related source_utf8_bytes {start}..{end}: related syntax recovery")
        }));
        lines.join("\n")
    }

    fn local_range(&self, start: usize, end: usize) -> (usize, usize) {
        (start - self.source_base, end - self.source_base)
    }
}

fn validate_syntax_diagnostic(
    diagnostic: &SyntaxDiagnostic,
    document: &SourceDocument,
    mapped: SourceRange,
) -> Result<(), AgentSyntaxDiagnosticProjectionError> {
    diagnostic.primary().validate_for(document)?;
    validate_syntax_range(diagnostic.primary().range(), mapped)?;
    if let Some(related) = diagnostic.related() {
        related.validate_for(document)?;
        validate_syntax_range(related.range(), mapped)?;
    }
    Ok(())
}

fn validate_syntax_range(
    range: SourceRange,
    mapped: SourceRange,
) -> Result<(), AgentSyntaxDiagnosticProjectionError> {
    if range.start() < mapped.start() || range.end() > mapped.end() {
        return Err(AgentSyntaxDiagnosticProjectionError::OutsideMappedSource {
            start: range.start(),
            end: range.end(),
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
    use arcweft_core::effect::{
        RuntimeArtifactFingerprint, RuntimeAssertion, RuntimeAssertionFailure,
        RuntimeAssertionGuardId, RuntimeAssertionProfile,
    };
    use arcweft_lang_syntax::{
        incremental::{ParsedSource, SyntaxDatabase},
        parser::ParseOptions,
    };
    use arcweft_source::{
        Diagnostic, DiagnosticApplicability, DiagnosticLabel, DiagnosticSeverity,
        DiagnosticSuggestion, SourceDocument, SourceDocumentId, SourceEdit, SourceName,
        SourceRange, SourceSpanValidationError, identity::SourceSnapshotId,
    };
    use arcweft_tooling::runtime_diagnostic::project_persisted_assertion_failure;
    use serde_json::json;
    use std::sync::Arc;

    use super::{
        AgentDiagnosticProjector, AgentSyntaxDiagnosticProjection, RuntimeAssertionDebugContext,
        project_runtime_assertion_debug_diagnostic,
    };

    const SOURCE: &str = "pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n";

    fn document(id: &str, source: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("fixture source id"),
            SourceName::Generated,
            source,
        )
        .expect("fixture source document")
    }

    fn attached(id: &str, source: &str) -> (Arc<SourceDocument>, ParsedSource) {
        let document = Arc::new(document(id, source));
        let mut syntax = SyntaxDatabase::try_new().expect("fixture syntax database");
        let parsed = syntax
            .parse_initial(
                SourceSnapshotId::initial(document.display_name().clone()),
                Arc::clone(&document),
                ParseOptions::default(),
            )
            .expect("attached fixture source");
        (document, parsed)
    }

    #[test]
    fn syntax_diagnostic_projection_preserves_the_attached_source_payload() {
        let (document, parsed) = attached(
            "arcweft-test://agent-repl/diagnostics/attached-source-payload",
            SOURCE,
        );
        let diagnostic = parsed
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.message() == "View part export needs `as` before its public name"
            })
            .expect("missing-`as` diagnostic");
        let projection = AgentSyntaxDiagnosticProjection::source_local(diagnostic, &document)
            .expect("source-local projection");
        let range = diagnostic.primary().range();

        assert_eq!(
            projection.json(),
            json!({
                "kind": "attached_source",
                "code": diagnostic.code(),
                "message": "View part export needs `as` before its public name",
                "range": {
                    "coordinate_space": "source_utf8_bytes",
                    "start": range.start(),
                    "end": range.end(),
                },
                "related": [],
                "expected": [],
                "found": null,
                "recovery": [],
            })
        );
        assert_eq!(
            projection.human(),
            format!(
                "error[{}] source_utf8_bytes {}..{}: View part export needs `as` before its public name",
                diagnostic.code(),
                range.start(),
                range.end()
            )
        );
    }

    #[test]
    fn syntax_diagnostic_projection_dewraps_an_exact_synthetic_prefix() {
        let prefix = "// synthetic wrapper\n";
        let synthetic_source = format!("{prefix}{SOURCE}");
        let (synthetic_document, parsed) = attached(
            "arcweft-test://agent-repl/diagnostics/synthetic-prefix",
            &synthetic_source,
        );
        let diagnostic = parsed
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.message() == "View part export needs `as` before its public name"
            })
            .expect("wrapped missing-`as` diagnostic");
        let authored = document("arcweft-agent://cell/0", SOURCE);
        let projection = AgentSyntaxDiagnosticProjection::dewrapped(
            diagnostic,
            &synthetic_document,
            &authored,
            SourceRange::new(prefix.len(), synthetic_source.len()),
        )
        .expect("exact wrapper dewrap");
        let primary = diagnostic.primary().range();

        assert_eq!(
            projection.json()["range"],
            json!({
                "coordinate_space": "source_utf8_bytes",
                "start": primary.start() - prefix.len(),
                "end": primary.end() - prefix.len(),
            })
        );
    }

    #[test]
    fn syntax_diagnostic_projection_preserves_and_validates_related_ranges() {
        let source = concat!(
            "character Alice {\n",
            "    display_name = \"Alice\"\n",
            "    display_name = \"Other\"\n",
            "}\n",
        );
        let (document, parsed) = attached(
            "arcweft-test://agent-repl/diagnostics/related-ranges",
            source,
        );
        let diagnostic = parsed
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code() == "syntax.character.duplicate_member")
            .expect("duplicate-member diagnostic");
        let projection = AgentSyntaxDiagnosticProjection::source_local(diagnostic, &document)
            .expect("source-local projection");
        let related = &projection.json()["related"][0];
        let first = diagnostic.related().expect("first role source").range();

        assert_eq!(related["range"]["coordinate_space"], "source_utf8_bytes");
        assert_eq!(related["range"]["start"], first.start());
        assert_eq!(related["range"]["end"], first.end());
        assert_eq!(related["message"], "related syntax recovery");
        assert!(projection.human().contains(&format!(
            "related source_utf8_bytes {}..{}: related syntax recovery",
            first.start(),
            first.end()
        )));
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
    fn runtime_assertion_projection_preserves_condition_and_statement_labels() {
        let source = "flow checks { assert.check(ready) }\n";
        let document = document("arcweft-agent://runtime-assertion/0", source);
        let condition_start = source.find("ready").expect("condition source");
        let statement_start = source.find("assert.check").expect("statement source");
        let failure = RuntimeAssertionFailure::new(RuntimeAssertion::new(
            RuntimeAssertionGuardId::try_from_bytes([0x31; 16]).expect("fixture guard"),
            "ready".to_owned(),
            "runtime condition failed".to_owned(),
            RuntimeAssertionProfile::Always,
        ));
        let mut diagnostic = project_persisted_assertion_failure(
            &failure,
            Some(
                document
                    .span(SourceRange::new(
                        condition_start,
                        condition_start + "ready".len(),
                    ))
                    .expect("condition span"),
            ),
        )
        .to_source_diagnostic();
        let statement_end = statement_start + "assert.check(ready)".len();
        diagnostic = diagnostic.with_label(DiagnosticLabel::secondary(
            document
                .span(SourceRange::new(statement_start, statement_end))
                .expect("statement span"),
            Some("assertion statement".to_owned()),
        ));

        let projection = AgentDiagnosticProjector::new(&document)
            .project(&diagnostic)
            .expect("runtime diagnostic belongs to exact source revision");
        let json = projection.json();

        assert_eq!(json["code"], "runtime.assertion_failed");
        assert_eq!(json["labels"][0]["style"], "primary");
        assert_eq!(json["labels"][0]["message"], "ready");
        assert_eq!(json["labels"][1]["style"], "secondary");
        assert_eq!(json["labels"][1]["message"], "assertion statement");
        assert!(projection.human().contains(&format!(
            "related source_utf8_bytes {statement_start}..{statement_end}: assertion statement"
        )));
    }

    #[test]
    fn runtime_assertion_debug_projection_persists_only_core_identity_and_source_evidence() {
        let source = "flow checks { assert.check(ready) }\n";
        let document = document("arcweft-agent://runtime-assertion/debug", source);
        let condition_start = source.find("ready").expect("condition source");
        let condition_span = document
            .span(SourceRange::new(
                condition_start,
                condition_start + "ready".len(),
            ))
            .expect("condition span");
        let guard = RuntimeAssertionGuardId::try_from_bytes([0x61; 16]).expect("fixture guard");
        let artifact =
            RuntimeArtifactFingerprint::try_from_bytes([0x71; 32]).expect("fixture artifact");
        let failure = RuntimeAssertionFailure::new(RuntimeAssertion::new(
            guard,
            "ready".to_owned(),
            "runtime condition failed".to_owned(),
            RuntimeAssertionProfile::Always,
        ));
        let diagnostic =
            project_persisted_assertion_failure(&failure, Some(condition_span.clone()));

        let projected = project_runtime_assertion_debug_diagnostic(
            RuntimeAssertionDebugContext::new(
                "diagnostic.runtime-assertion.1",
                Some(arcweft_agent_protocol::ids::StableHash::from_blake3_bytes(
                    [0x81; 32],
                )),
                None,
                None,
                Some(3),
                17,
            ),
            artifact,
            &failure,
            &diagnostic,
        );

        assert_eq!(projected.code.as_deref(), Some("runtime.assertion_failed"));
        assert_eq!(projected.phase, "runtime");
        assert_eq!(projected.message, "runtime condition failed");
        assert_eq!(projected.sequence, Some(3));
        assert_eq!(
            projected.start_byte,
            Some(u64::try_from(condition_start).expect("fixture offset fits u64"))
        );
        assert_eq!(
            projected.end_byte,
            Some(u64::try_from(condition_start + "ready".len()).expect("fixture offset fits u64"),)
        );
        assert_eq!(
            projected
                .program_hash
                .as_ref()
                .expect("caller-owned debug program hash")
                .as_str(),
            arcweft_agent_protocol::ids::StableHash::from_blake3_bytes([0x81; 32]).as_str()
        );

        let payload = projected
            .payload
            .as_object()
            .expect("runtime assertion debug payload is an object");
        assert_eq!(payload.len(), 3);
        assert!(payload.contains_key("artifact_fingerprint"));
        assert!(payload.contains_key("failure"));
        assert!(payload.contains_key("source_evidence"));
        let decoded_artifact: RuntimeArtifactFingerprint =
            serde_json::from_value(payload["artifact_fingerprint"].clone())
                .expect("debug payload retains the typed core artifact fingerprint");
        let decoded_failure: RuntimeAssertionFailure =
            serde_json::from_value(payload["failure"].clone())
                .expect("debug payload retains the typed core assertion failure");
        assert_eq!(decoded_artifact, artifact);
        assert_eq!(decoded_failure, failure);

        let evidence = payload["source_evidence"]
            .as_object()
            .expect("typed projection owns source presentation separately");
        assert_eq!(evidence.len(), 2);
        assert_eq!(evidence["primary"]["role"], "primary");
        assert_eq!(
            evidence["primary"]["source_id"],
            condition_span.source().id().as_str()
        );
        assert_eq!(
            evidence["primary"]["start_byte"],
            u64::try_from(condition_start).expect("fixture offset fits u64")
        );
        assert_eq!(
            evidence["primary"]["end_byte"],
            u64::try_from(condition_start + 5).expect("fixture offset fits u64")
        );
        assert_eq!(evidence["secondary"], json!([]));
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
