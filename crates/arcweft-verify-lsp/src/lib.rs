//! Sans I/O LSP helpers for Arcweft verifier diagnostics.
//!
//! This crate does not open sockets or own a language-server transport. It
//! converts verifier reports into `lsp-types` values that a future server,
//! editor plugin, or tests can reuse.

use arcweft_verify::{
    Severity as VerifySeverity, ToolActionKind, VerificationDiagnostic, VerificationReport,
};
use lsp_types::{
    CodeAction, CodeActionKind, Diagnostic, DiagnosticSeverity, InlayHint, InlayHintKind,
    NumberOrString, Position, Range, Uri,
};

/// Converts a verifier report into LSP diagnostics for a document.
pub fn diagnostics_from_report(report: &VerificationReport) -> Vec<Diagnostic> {
    report
        .diagnostics
        .iter()
        .map(diagnostic_from_verify)
        .collect()
}

/// Converts verifier tool actions into LSP code actions.
pub fn code_actions_from_report(uri: &Uri, report: &VerificationReport) -> Vec<CodeAction> {
    report
        .diagnostics
        .iter()
        .flat_map(|diagnostic| {
            diagnostic
                .actions
                .iter()
                .map(|action| CodeAction {
                    title: action.label.clone(),
                    kind: Some(match action.kind {
                        ToolActionKind::GenerateProofStub | ToolActionKind::GenerateUnsafeAudit => {
                            CodeActionKind::QUICKFIX
                        }
                        ToolActionKind::ShowObligation
                        | ToolActionKind::NavigateToProof
                        | ToolActionKind::NavigateToUnsafeAudit => CodeActionKind::REFACTOR,
                    }),
                    diagnostics: Some(vec![diagnostic_from_verify(diagnostic)]),
                    command: Some(lsp_types::Command {
                        title: action.label.clone(),
                        command: format!("arcweft.{}", action.id),
                        arguments: Some(vec![
                            serde_json::json!(uri.to_string()),
                            serde_json::json!(diagnostic.obligation),
                        ]),
                    }),
                    ..CodeAction::default()
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Converts source-level Arcweft tooling actions into LSP code actions.
pub fn source_code_actions(uri: &Uri, source: &str) -> Vec<CodeAction> {
    arcweft_tooling::source_code_actions(source)
        .into_iter()
        .map(|action| CodeAction {
            title: action.label,
            kind: Some(CodeActionKind::REFACTOR_REWRITE),
            command: Some(lsp_types::Command {
                title: action.id.clone(),
                command: action.id,
                arguments: Some(vec![
                    serde_json::json!(uri.to_string()),
                    serde_json::json!(action.edit),
                ]),
            }),
            ..CodeAction::default()
        })
        .collect()
}

/// Converts inferred Arcweft IDs into LSP inlay hints.
pub fn inferred_id_inlay_hints(source: &str) -> Vec<InlayHint> {
    arcweft_tooling::inferred_id_hints(source)
        .into_iter()
        .map(|hint| InlayHint {
            position: offset_position(hint.position),
            label: lsp_types::InlayHintLabel::String(hint.label),
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            tooltip: None,
            padding_left: None,
            padding_right: None,
            data: None,
        })
        .collect()
}

fn diagnostic_from_verify(diagnostic: &VerificationDiagnostic) -> Diagnostic {
    Diagnostic {
        range: diagnostic.source.map_or_else(default_range, |span| Range {
            start: offset_position(span.start),
            end: offset_position(span.end),
        }),
        severity: Some(match diagnostic.severity {
            VerifySeverity::Info => DiagnosticSeverity::INFORMATION,
            VerifySeverity::Warning => DiagnosticSeverity::WARNING,
            VerifySeverity::Error => DiagnosticSeverity::ERROR,
        }),
        code: diagnostic.obligation.clone().map(NumberOrString::String),
        source: Some("arcweft-verify".to_owned()),
        message: diagnostic.message.clone(),
        ..Diagnostic::default()
    }
}

fn default_range() -> Range {
    Range {
        start: Position::new(0, 0),
        end: Position::new(0, 0),
    }
}

fn offset_position(offset: usize) -> Position {
    let character = u32::try_from(offset).unwrap_or(u32::MAX);
    Position::new(0, character)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_verify::{VerificationDiagnostic, VerificationPolicy, VerificationReport};

    #[test]
    fn converts_report_diagnostic() {
        let report = VerificationReport {
            policy: VerificationPolicy::default(),
            diagnostics: vec![VerificationDiagnostic {
                id: "d1".to_owned(),
                severity: VerifySeverity::Error,
                message: "missing proof".to_owned(),
                source: None,
                obligation: Some("obligation.0001".to_owned()),
                related_ids: Vec::new(),
                actions: Vec::new(),
            }],
            ..VerificationReport::default()
        };
        let diagnostics = diagnostics_from_report(&report);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn exposes_source_actions_and_inlay_hints() {
        let uri = "file:///game/routes/opening.awft"
            .parse::<Uri>()
            .expect("uri");
        let source = "flow @.opening opening {\n    alice: hi[p]\n}\n";
        let actions = source_code_actions(&uri, source);
        assert!(
            actions
                .iter()
                .any(|action| action.title == "Expand Arcweft sugar")
        );
        assert!(
            actions
                .iter()
                .any(|action| action.title == "Materialize inferred Arcweft ID")
        );
        let hints = inferred_id_inlay_hints(source);
        assert!(hints.iter().any(|hint| {
            matches!(&hint.label, lsp_types::InlayHintLabel::String(label) if label == "@flow.opening")
        }));
        assert!(hints.iter().any(|hint| {
            matches!(&hint.label, lsp_types::InlayHintLabel::String(label) if label.contains("id=@say.opening.alice.001"))
        }));
    }
}
