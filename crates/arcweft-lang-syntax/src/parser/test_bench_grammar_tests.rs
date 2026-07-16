use arcweft_source::SourceRange;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::event::PendingSyntaxDiagnostic;
use crate::grammar::kinds::SyntaxKind;

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/test-bench-shadow").unwrap(),
        SourceName::path("test-bench-shadow.arcw"),
        text,
    )
    .unwrap()
}

fn kind_count(entries: &[UnattachedGrammarEntry], kind: SyntaxKind) -> usize {
    entries.iter().filter(|entry| entry.kind() == kind).count()
}

#[test]
fn test_and_bench_plans_are_structured_and_lossless() {
    let source = r#"/// Headless route check.
#[generated]
test @test.opening scenario {
    goto @flow.opening
    expect.signal(@signal.current_flow, @flow.opening)
}

bench @bench.score {
    setup { let input = fixture("score.json") }
    measure { pure(score) }
    assert(metric.allocations <= 2)
}
"#;
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();

    for expected in [
        SyntaxKind::TestItem,
        SyntaxKind::BenchItem,
        SyntaxKind::DocBlock,
        SyntaxKind::OuterAttribute,
        SyntaxKind::NameReference,
        SyntaxKind::Block,
        SyntaxKind::OpenBraceNode,
        SyntaxKind::CloseBraceNode,
        SyntaxKind::GotoStatement,
        SyntaxKind::ExpressionStatement,
        SyntaxKind::CallExpression,
        SyntaxKind::NamedBlockExpression,
    ] {
        assert!(
            entries.iter().any(|entry| entry.kind() == expected),
            "missing {expected:?}: kinds={:?}, diagnostics={:?}",
            entries
                .iter()
                .map(UnattachedGrammarEntry::kind)
                .collect::<Vec<_>>(),
            built.diagnostics(),
        );
    }
    assert_eq!(kind_count(entries, SyntaxKind::Block), 4);
    // The nested `setup` expression block owns its ordinary omitted tail; the
    // two outer plan blocks do not reinterpret their last row as a tail.
    assert_eq!(kind_count(entries, SyntaxKind::OmittedBlockTail), 1);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_plan_header_parts_and_bodies_recover_before_following_proofs() {
    let source = concat!(
        "test scenario {}\n",
        "test @test.no_kind {}\n",
        "test @test.no_body scenario\n",
        "bench {}\n",
        "bench @bench.no_body\n",
        "proof next() = ()\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();
    let codes = built
        .diagnostics()
        .iter()
        .map(PendingSyntaxDiagnostic::code)
        .collect::<Vec<_>>();

    assert_eq!(kind_count(entries, SyntaxKind::TestItem), 3);
    assert_eq!(kind_count(entries, SyntaxKind::BenchItem), 2);
    assert_eq!(kind_count(entries, SyntaxKind::MissingName), 2);
    assert_eq!(kind_count(entries, SyntaxKind::MissingBody), 2);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    for expected in [
        "syntax.test.missing_id",
        "syntax.test.missing_kind",
        "syntax.test.missing_body",
        "syntax.bench.missing_id",
        "syntax.bench.missing_body",
    ] {
        assert!(codes.contains(&expected), "missing {expected}: {codes:?}");
    }
    assert!(built.diagnostics().iter().all(|diagnostic| {
        !diagnostic.code().starts_with("syntax.test.missing_")
            && !diagnostic.code().starts_with("syntax.bench.missing_")
            || diagnostic.range().is_empty()
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unclosed_plan_body_synchronizes_before_the_next_declaration() {
    let source = concat!(
        "test @test.broken scenario {\n",
        "    expect.signal(@signal.ready, true)\n",
        "proof after_test() = ()\n",
        "bench @bench.broken {\n",
        "    measure { pure(score) }\n",
        "proof after_bench() = ()\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::TestItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::BenchItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 2);
    assert_eq!(kind_count(entries, SyntaxKind::CloseBraceNode), 3);
    assert_eq!(
        built.missing_tokens().len(),
        2,
        "{:?}",
        built.missing_tokens()
    );
    assert_eq!(
        built
            .missing_tokens()
            .iter()
            .map(crate::grammar::build::MissingTokenSite::at)
            .collect::<Vec<_>>(),
        [68, 142]
    );
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .map(PendingSyntaxDiagnostic::code)
            .collect::<Vec<_>>(),
        ["syntax.test.missing_body", "syntax.bench.missing_body"]
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unexpected_plan_header_tokens_are_recovered_without_hiding_the_body() {
    let source = concat!(
        "test @test.extra scenario unexpected { expect.no_failures() }\n",
        "bench @bench.extra unexpected { report { cpu_time } }\n",
        "proof next() = ()\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::TestItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::BenchItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ErrorNode), 2);
    assert_eq!(kind_count(entries, SyntaxKind::Block), 3);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .map(PendingSyntaxDiagnostic::code)
            .collect::<Vec<_>>(),
        [
            "syntax.item.unexpected_token",
            "syntax.item.unexpected_token"
        ]
    );
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .map(PendingSyntaxDiagnostic::range)
            .collect::<Vec<_>>(),
        [SourceRange::new(26, 36), SourceRange::new(81, 91)]
    );
    assert_eq!(built.green().to_string(), source);
}
