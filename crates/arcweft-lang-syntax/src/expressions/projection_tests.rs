use arcweft_source::SourceRange;

use super::dialogue::{
    PendingCandidateGraph, PendingCandidateGraphError, PendingCandidateNode,
    PendingCandidateSemantic,
};
use crate::grammar::keyword_statement_projection::PendingKeywordStatementProjection;
use crate::grammar::{SyntaxKind, SyntaxRole};

fn node(kind: SyntaxKind, projection: PendingKeywordStatementProjection) -> PendingCandidateNode {
    PendingCandidateNode::new(
        kind,
        SyntaxRole::Statement(0),
        None,
        SourceRange::new(0, 0),
        PendingCandidateSemantic::KeywordStatement(projection),
    )
}

#[test]
fn candidate_graph_rejects_every_keyword_projection_family_substitution() {
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
        PendingCandidateGraph::try_new(vec![node(kind, projection)])
            .expect("matching candidate keyword projection");
        let wrong = if kind == SyntaxKind::GotoStatement {
            PendingKeywordStatementProjection::Defer
        } else {
            PendingKeywordStatementProjection::Goto
        };
        assert_eq!(
            PendingCandidateGraph::try_new(vec![node(kind, wrong)]).unwrap_err(),
            PendingCandidateGraphError::InvalidKeywordStatementProjection
        );
    }

    assert_eq!(
        PendingCandidateGraph::try_new(vec![node(
            SyntaxKind::ExpressionList,
            PendingKeywordStatementProjection::Goto,
        )])
        .unwrap_err(),
        PendingCandidateGraphError::InvalidKeywordStatementProjection
    );
}
