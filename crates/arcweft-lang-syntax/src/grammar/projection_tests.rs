use arcweft_source::SourceRange;

use super::build::{GrammarBuildError, build_grammar_text};
use super::event::SyntaxEvent;
use super::keyword_statement_projection::PendingKeywordStatementProjection;
use super::{SyntaxKind, SyntaxRole};
use crate::name::SyntaxName;

fn keyword_start(kind: SyntaxKind, projection: PendingKeywordStatementProjection) -> SyntaxEvent {
    let mut event = SyntaxEvent::start(kind, SyntaxRole::Statement(0));
    let SyntaxEvent::StartNode {
        keyword_statement_projection,
        ..
    } = &mut event
    else {
        unreachable!("SyntaxEvent::start always returns a start event")
    };
    *keyword_statement_projection = Some(projection);
    event
}

fn transaction(statement: SyntaxEvent) -> [SyntaxEvent; 4] {
    [
        SyntaxEvent::start(SyntaxKind::SourceFile, SyntaxRole::Root),
        statement,
        SyntaxEvent::FinishNode,
        SyntaxEvent::FinishNode,
    ]
}

#[test]
fn every_keyword_statement_requires_its_exact_projection_family() {
    let cases = [
        (
            SyntaxKind::OutStatement,
            PendingKeywordStatementProjection::Out { label: None },
        ),
        (
            SyntaxKind::GotoStatement,
            PendingKeywordStatementProjection::Goto,
        ),
        (
            SyntaxKind::DeferStatement,
            PendingKeywordStatementProjection::Defer,
        ),
        (
            SyntaxKind::SignalStatement,
            PendingKeywordStatementProjection::Signal,
        ),
        (
            SyntaxKind::BreakStatement,
            PendingKeywordStatementProjection::Break { label: None },
        ),
        (
            SyntaxKind::ContinueStatement,
            PendingKeywordStatementProjection::Continue { label: None },
        ),
    ];

    for (kind, projection) in cases {
        assert_eq!(
            build_grammar_text(
                "",
                &transaction(SyntaxEvent::start(kind, SyntaxRole::Statement(0))),
                0
            )
            .unwrap_err(),
            GrammarBuildError::MissingKeywordStatementProjection { event: 1, kind }
        );
        build_grammar_text("", &transaction(keyword_start(kind, projection.clone())), 0)
            .expect("matching keyword projection transaction");

        let wrong = if kind == SyntaxKind::GotoStatement {
            PendingKeywordStatementProjection::Defer
        } else {
            PendingKeywordStatementProjection::Goto
        };
        assert_eq!(
            build_grammar_text("", &transaction(keyword_start(kind, wrong)), 0).unwrap_err(),
            GrammarBuildError::InvalidKeywordStatementProjection { event: 1, kind }
        );
    }

    let kind = SyntaxKind::ExpressionList;
    assert_eq!(
        build_grammar_text(
            "",
            &transaction(keyword_start(kind, PendingKeywordStatementProjection::Goto,)),
            0,
        )
        .unwrap_err(),
        GrammarBuildError::InvalidKeywordStatementProjection { event: 1, kind }
    );
}

#[test]
fn keyword_statement_projection_rebase_preserves_typed_label_identity() {
    let label = SyntaxName::try_new("exit").expect("valid control label");
    let event = keyword_start(
        SyntaxKind::OutStatement,
        PendingKeywordStatementProjection::Out {
            label: Some(Ok(label)),
        },
    );
    assert_eq!(event.rebased(8), Some(event));
    assert_eq!(
        SyntaxEvent::token(SyntaxKind::IdentifierToken, SourceRange::new(0, 4)).rebased(8),
        Some(SyntaxEvent::token(
            SyntaxKind::IdentifierToken,
            SourceRange::new(8, 12),
        ))
    );
}
