use super::*;
use crate::expressions::PendingCandidateGraph;
use crate::expressions::{
    ExpressionProjection, SyntaxPostfixBracketProjection, SyntaxPostfixIndexCandidate,
};
use crate::grammar::build::{GrammarBuildError, build_grammar};
use crate::incremental::SyntaxLimit;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

fn document() -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-test://retained-recovery").unwrap(),
        SourceName::path("retained-recovery.arcw"),
        format!(
            "//夢\npredicate leaf() = alice()[|[夢](ゆめ)]\n//{}",
            "x".repeat(2_048)
        ),
    )
    .unwrap()
}

fn retained_events(
    document: &SourceDocument,
    diagnostics: Vec<PendingSyntaxDiagnostic>,
    missing: Vec<(ExpectedToken, usize)>,
) -> Vec<SyntaxEvent> {
    let mut events =
        crate::parser::parse_document(document, crate::parser::ParseOptions::default())
            .unwrap()
            .events()
            .to_vec();
    let pending = events
        .iter_mut()
        .find_map(|event| match event {
            SyntaxEvent::StartNode {
                projection: PendingStartProjection::Expression(projection),
                ..
            } if matches!(
                projection.projection(),
                ExpressionProjection::PostfixBracket(_)
            ) =>
            {
                Some(projection)
            }
            _ => None,
        })
        .unwrap();
    let ExpressionProjection::PostfixBracket(SyntaxPostfixBracketProjection::Ambiguous {
        index,
        dialogue,
    }) = pending.projection()
    else {
        panic!("retained interpretations")
    };
    let graph =
        PendingCandidateGraph::try_new(index.graph().nodes().to_vec(), missing, diagnostics)
            .unwrap();
    **pending = PendingExpressionProjection::new(
        ExpressionProjection::PostfixBracket(SyntaxPostfixBracketProjection::Ambiguous {
            index: Box::new(SyntaxPostfixIndexCandidate::new(graph)),
            dialogue: dialogue.clone(),
        }),
        pending.components().to_vec(),
    );
    events
}

#[test]
fn retained_candidate_recovery_validates_primary_related_edit_and_missing_ranges() {
    let document = document();
    let valid = PendingSyntaxDiagnostic::new(
        "syntax.retained.test",
        SourceRange::new(8, 9),
        "retained diagnostic",
    );
    for invalid in [
        SourceRange::new(3, 4),
        SourceRange::new(document.text().len() + 1, document.text().len() + 1),
    ] {
        for diagnostic in [
            PendingSyntaxDiagnostic::new("syntax.retained.test", invalid, "invalid primary"),
            valid.clone().with_related_range(invalid),
            valid.clone().with_suggestions([PendingSyntaxSuggestion {
                edits: vec![PendingSyntaxEdit::new(invalid, "replacement")].into_boxed_slice(),
                ..PendingSyntaxSuggestion::new("invalid edit")
            }]),
        ] {
            assert!(matches!(
                build_grammar(
                    &document,
                    &retained_events(&document, vec![diagnostic], vec![])
                ),
                Err(GrammarBuildError::InvalidDiagnosticRange { .. })
            ));
        }
        let expected = ExpectedToken::try_with_spelling(SyntaxKind::PunctuationToken, ")").unwrap();
        assert!(matches!(
            build_grammar(
                &document,
                &retained_events(&document, vec![], vec![(expected, invalid.start())])
            ),
            Err(GrammarBuildError::InvalidDiagnosticRange { .. })
        ));
    }
    let diagnostic = valid.with_suggestions([PendingSyntaxSuggestion {
        edits: vec![PendingSyntaxEdit::new(SourceRange::new(8, 9), "new")].into_boxed_slice(),
        ..PendingSyntaxSuggestion::new("valid edit")
    }]);
    let events = retained_events(&document, vec![diagnostic], vec![]);
    build_grammar(&document, &events).expect("valid candidate recovery remains attachable");
    let rebased = events.iter().find(|event| matches!(event, SyntaxEvent::StartNode { projection: PendingStartProjection::Expression(projection), .. } if matches!(projection.projection(), ExpressionProjection::PostfixBracket(_)))).unwrap().rebased(11).unwrap();
    let SyntaxEvent::StartNode {
        projection: PendingStartProjection::Expression(projection),
        ..
    } = rebased
    else {
        panic!("rebased expression")
    };
    let diagnostic = projection
        .projection()
        .candidate_graphs()
        .next()
        .unwrap()
        .diagnostics()
        .first()
        .unwrap();
    assert_eq!(diagnostic.range(), SourceRange::new(19, 20));
    assert_eq!(
        diagnostic.suggestions()[0].edits()[0].range(),
        SourceRange::new(19, 20)
    );
}

#[test]
fn retained_candidate_diagnostics_share_the_exact_document_budget() {
    let document = document();
    let padding_start = document.text().len() - 2_048;
    let diagnostics = (0..SyntaxLimit::Diagnostics.maximum())
        .map(|ordinal| {
            PendingSyntaxDiagnostic::new(
                "syntax.retained.limit",
                SourceRange::new(padding_start + ordinal, padding_start + ordinal + 1),
                "retained",
            )
        })
        .collect::<Vec<_>>();
    let mut events = retained_events(&document, diagnostics.clone(), vec![]);
    build_grammar(&document, &events).expect("inclusive retained diagnostic limit");
    events.insert(
        events.len() - 1,
        SyntaxEvent::Diagnostic(diagnostics[0].clone()),
    );
    build_grammar(&document, &events)
        .expect("one identity referenced in ordinary and candidate evidence is charged once");
    events.insert(
        events.len() - 1,
        SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.retained.extra",
            SourceRange::new(8, 9),
            "one more identity",
        )),
    );
    assert!(matches!(
        build_grammar(&document, &events),
        Err(GrammarBuildError::LimitExceeded(SyntaxLimit::Diagnostics))
    ));
}
