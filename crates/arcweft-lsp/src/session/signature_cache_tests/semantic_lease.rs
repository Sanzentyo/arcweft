use lsp_server::{ErrorCode, Notification, Request};
use lsp_types::{
    DidChangeTextDocumentParams, SignatureHelp, TextDocumentContentChangeEvent,
    VersionedTextDocumentIdentifier,
    notification::{DidChangeTextDocument, Notification as LspNotification},
    request::{HoverRequest, InlayHintRequest, Request as LspRequest},
};

use super::{SIGNATURE_REQUEST_DEADLINE, SOURCE, SignatureCacheFixture, position_after};

const REJECTED: &str = "fn sum(lhs: i64, rhs: i64) -> i64 { lhs + rhs }\nfn evaluate(value: i64) { sum(value, value, value); }\nentry server @entry.server.main { goto @flow.main }\nflow @flow.main main { let count = 42 }\n";

fn replace_source(fixture: &SignatureCacheFixture, version: i32, source: &str) {
    fixture
        .session
        .write()
        .unwrap()
        .handle_notification(Notification::new(
            DidChangeTextDocument::METHOD.to_owned(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: fixture.uri.clone(),
                    version,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: source.to_owned(),
                }],
            },
        ))
        .unwrap();
}

#[test]
fn semantic_lease_serves_signature_help_for_rejected_source() {
    let fixture = SignatureCacheFixture::new_with_source_tree(
        "lsp-rejected-semantic-lease",
        REJECTED,
        &[],
        SIGNATURE_REQUEST_DEADLINE,
    );
    let accepted = fixture.accepted();
    assert!(accepted.executable().is_none());
    let analysis = accepted
        .analysis()
        .expect("complete rejected-call semantics are retained");
    assert_eq!(analysis.final_analysis().call_diagnostics().count(), 1);
    let prepared = fixture.prepare(401, position_after(REJECTED, "sum(value,"));
    assert!(std::ptr::eq(
        prepared.lease().final_analysis(),
        analysis.final_analysis().as_ref()
    ));
    let response = fixture.publish(&prepared, fixture.execute(&prepared));
    assert!(response.error.is_none(), "{:?}", response.error);
    let help: SignatureHelp = serde_json::from_value(response.result.unwrap()).unwrap();
    assert_eq!(help.signatures.len(), 1);
    assert!(help.signatures[0].label.contains("sum"));
    assert_eq!(help.signatures[0].parameters.as_ref().unwrap().len(), 2);
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 1);
}

#[test]
fn semantic_lease_serves_hover_and_inlays_for_rejected_source() {
    let fixture = SignatureCacheFixture::new_with_source_tree(
        "lsp-rejected-semantic-features",
        REJECTED,
        &[],
        SIGNATURE_REQUEST_DEADLINE,
    );
    assert!(fixture.accepted().executable().is_none());
    assert!(fixture.accepted().analysis().is_some());
    let mut session = fixture.session.write().unwrap();
    let response = session.handle_request(Request {
        id: 405.into(),
        method: HoverRequest::METHOD.to_owned(),
        params: serde_json::json!({
            "textDocument": { "uri": fixture.uri },
            "position": position_after(REJECTED, "fn su"),
        }),
    });
    assert!(response.error.is_none(), "{:?}", response.error);
    let hover: lsp_types::Hover = serde_json::from_value(response.result.unwrap()).unwrap();
    let lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(text)) = hover.contents
    else {
        panic!("semantic callable hover must expose its checked effects");
    };
    assert!(text.contains("checked effects for `sum`"), "{text}");
    assert!(text.contains("effects: { }"), "{text}");
    let response = session.handle_request(Request {
        id: 406.into(),
        method: InlayHintRequest::METHOD.to_owned(),
        params: serde_json::json!({
            "textDocument": { "uri": fixture.uri },
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 4, "character": 0 } },
        }),
    });
    assert!(response.error.is_none(), "{:?}", response.error);
    let hints: Vec<lsp_types::InlayHint> =
        serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(
        hints.iter().any(|hint| {
            matches!(&hint.label, lsp_types::InlayHintLabel::String(label) if label == ": i32")
        }),
        "{hints:?}"
    );
}

#[test]
fn semantic_lease_edit_cycle_rejects_stale_results_and_restores_execution() {
    let fixture = SignatureCacheFixture::new("lsp-semantic-lease-edit-cycle");
    assert!(fixture.accepted().executable().is_some());
    let old = fixture.prepare(402, position_after(SOURCE, "sum(value,"));
    let old_result = fixture.execute(&old).unwrap();
    replace_source(&fixture, 2, REJECTED);
    let rejected = fixture.accepted();
    assert!(rejected.executable().is_none());
    assert!(rejected.analysis().is_some());
    assert_eq!(rejected.signature_cache_snapshot_for_test().entries, 0);
    let response = fixture.publish(&old, Ok(old_result));
    assert_eq!(
        response.error.unwrap().code,
        ErrorCode::ContentModified as i32
    );
    assert_eq!(rejected.signature_cache_snapshot_for_test().entries, 0);
    let request = fixture.prepare(403, position_after(REJECTED, "sum(value,"));
    let response = fixture.publish(&request, fixture.execute(&request));
    assert!(response.error.is_none(), "{:?}", response.error);
    assert!(response.result.is_some_and(|value| !value.is_null()));
    replace_source(&fixture, 3, SOURCE);
    let repaired = fixture.accepted();
    assert!(repaired.executable().is_some());
    assert_eq!(repaired.signature_cache_snapshot_for_test().entries, 0);
    let request = fixture.prepare(404, position_after(SOURCE, "sum(value,"));
    let response = fixture.publish(&request, fixture.execute(&request));
    assert!(response.error.is_none(), "{:?}", response.error);
    assert!(response.result.is_some_and(|value| !value.is_null()));
}
