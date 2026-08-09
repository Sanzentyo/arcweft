use crate::diagnostics::DocumentAnalysis;
use crate::documents::DocumentSnapshot;
use crate::profiles::LspProfile;
use arcweft_tooling::model::ToolingError;
use arcweft_verify_lsp::{code_actions_from_report_with_mapper, source_code_actions_with_mapper};
use lsp_types::{CodeAction, CodeActionKind, Command, Position, Uri};

/// Computes code actions for one open Arcweft document.
pub fn actions(
    _profile: &LspProfile,
    uri: &Uri,
    document: &DocumentSnapshot,
    analysis: &DocumentAnalysis,
    _position: Position,
) -> Result<Vec<CodeAction>, ToolingError> {
    let mut actions =
        source_code_actions_with_mapper(uri, document.source_document(), document.line_index())?;
    actions.extend(analysis.compiler_commands().iter().map(|command| {
        CodeAction {
            title: command.title().to_owned(),
            kind: Some(if command.id() == "arcweft.verify.generateProofStub" {
                CodeActionKind::QUICKFIX
            } else {
                CodeActionKind::REFACTOR
            }),
            command: Some(Command {
                title: command.title().to_owned(),
                command: command.id().to_owned(),
                arguments: Some(
                    std::iter::once(serde_json::json!(uri.to_string()))
                        .chain(
                            command
                                .arguments()
                                .iter()
                                .map(|argument| serde_json::json!(argument)),
                        )
                        .collect(),
                ),
            }),
            ..CodeAction::default()
        }
    }));
    if let Some(report) = analysis.verification_report() {
        actions.extend(code_actions_from_report_with_mapper(
            uri,
            document.source_document(),
            report,
            document.line_index(),
        ));
    }
    Ok(actions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::{AcceptedOpenDocument, DocumentStore};
    use crate::positions::PositionEncoding;
    use crate::profiles::{LspProfileResolver, LspProfileTestHarness};
    use arcweft_runtime_host::RuntimeHostRunnerKind;
    use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem};
    use std::{
        fs,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn verifier_report_actions_are_included_in_code_actions() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("arcweft-lsp-actions-{unique}"));
        let source_path = root.join("src/main.arcw");
        fs::create_dir_all(source_path.parent().expect("source parent")).expect("project root");
        fs::write(
            root.join("arcw.toml"),
            r#"schema = 1

[package]
id = "org.arcweft.tests.lsp-actions"
version = "0.1.0"

[profiles.agent]
kind = "agent"
entry = "@entry.agent.main"
source = "src/main.arcw"
"#,
        )
        .expect("manifest");
        let source = "proof pending() { assert.prove(true) }\n\nfn controller() -> Result<Unit, AgentError>\neffects {}\n{\n    Ok(())\n}\n\nentry agent @entry.agent.main {\n    controller = controller\n}\n";
        fs::write(&source_path, source).expect("source");
        let uri = format!(
            "file:///{}",
            source_path
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches('/')
        )
        .parse::<Uri>()
        .expect("uri");
        let profile = LspProfileTestHarness::new(LspProfileResolver::new(
            RuntimeHostRunnerKind::Native,
            Some("agent".to_owned()),
        ))
        .resolve_for_document_path(&source_path)
        .expect("accepted profile construction")
        .publish_for_test();
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
                    text_document: TextDocumentItem::new(
                        uri.clone(),
                        "arcweft".to_owned(),
                        1,
                        source.to_owned(),
                    ),
                },
                PositionEncoding::Utf16,
                Some(&authority),
            )
            .expect("document parse");
        let analysis = DocumentAnalysis::analyze_snapshot(&document, &profile);

        let code_actions = actions(&profile, &uri, &document, &analysis, Position::new(1, 4))
            .expect("code actions");

        assert!(code_actions.iter().any(|action| {
            action.command.as_ref().is_some_and(|command| {
                command.command == "arcweft.verify.generateProofStub"
                    || command.command == "arcweft.verify.showObligation"
            })
        }));
        let _ = fs::remove_dir_all(root);
    }
}
