use std::sync::Arc;

use arcweft_lang_syntax::attachment::AttachmentFailure;
use arcweft_lang_syntax::incremental::{ParseFailure, ParseStatus, SyntaxDatabase};
use arcweft_lang_syntax::parser::{
    FragmentKind, ParseCompletion, ParseOptions, UnboundFragment, parse_expression_fragment,
    parse_pattern_fragment, parse_statement_fragment, parse_type_fragment,
};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange, SourceSpan};

fn source_document(name: &SourceName, text: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-test://syntax/public-parser/{}",
                name.display_name()
            ))
            .expect("valid test source identity"),
            name.clone(),
            text,
        )
        .expect("valid test source document"),
    )
}

fn attach_exact<K: FragmentKind>(
    database: &mut SyntaxDatabase,
    snapshot: &SourceSnapshotId,
    document: &Arc<SourceDocument>,
    text: &str,
    fragment: UnboundFragment<K>,
) -> arcweft_lang_syntax::parser::AttachedFragment<K> {
    let start = document.text().find(text).expect("fragment in test source");
    let span = document
        .span(SourceRange::new(start, start + text.len()))
        .expect("valid exact fragment span");
    database
        .attach_fragment(snapshot.clone(), Arc::clone(document), span, fragment)
        .expect("complete exact fragment attaches")
}

#[test]
fn whole_source_publication_retains_one_exact_document_and_snapshot() {
    let name = SourceName::path("public-whole-source.arcw");
    let snapshot = SourceSnapshotId::initial(name.clone());
    let document = source_document(&name, "proof valid() = true\n");
    let parsed = SyntaxDatabase::try_new()
        .expect("syntax database")
        .parse_initial(
            snapshot.clone(),
            Arc::clone(&document),
            ParseOptions::default(),
        )
        .expect("whole source transaction");

    assert_eq!(parsed.status(), ParseStatus::Clean);
    assert!(parsed.diagnostics().is_empty());
    assert_eq!(parsed.source_snapshot_id(), &snapshot);
    assert!(Arc::ptr_eq(parsed.document_lease(), &document));
    assert_eq!(parsed.root_syntax().rowan().to_string(), document.text());
}

#[test]
fn every_public_fragment_family_attaches_without_reparse() {
    let name = SourceName::path("public-fragments.arcw");
    let snapshot = SourceSnapshotId::initial(name.clone());
    let source = "value?\nResult<Value>\nSome(value)\nlet value = source;\n";
    let document = source_document(&name, source);
    let mut database = SyntaxDatabase::try_new().expect("syntax database");

    let expression_text = "value?";
    let expression = parse_expression_fragment(expression_text, ParseOptions::default());
    assert_eq!(expression.completion(), &ParseCompletion::Complete);
    assert!(expression.diagnostics().is_empty());
    let expression = attach_exact(
        &mut database,
        &snapshot,
        &document,
        expression_text,
        expression,
    );
    assert_eq!(expression.root().source_text(), expression_text);

    let type_text = "Result<Value>";
    let type_ref = attach_exact(
        &mut database,
        &snapshot,
        &document,
        type_text,
        parse_type_fragment(type_text, ParseOptions::default()),
    );
    assert_eq!(type_ref.root().source_text(), type_text);

    let pattern_text = "Some(value)";
    let pattern = attach_exact(
        &mut database,
        &snapshot,
        &document,
        pattern_text,
        parse_pattern_fragment(pattern_text, ParseOptions::default()),
    );
    assert_eq!(pattern.root().source_text(), pattern_text);

    let statement_text = "let value = source;";
    let statement = attach_exact(
        &mut database,
        &snapshot,
        &document,
        statement_text,
        parse_statement_fragment(statement_text, ParseOptions::default()),
    );
    assert_eq!(statement.root().source_text(), statement_text);

    let lineages = [
        expression.snapshot_id().lineage(),
        type_ref.snapshot_id().lineage(),
        pattern.snapshot_id().lineage(),
        statement.snapshot_id().lineage(),
    ];
    assert!(lineages.windows(2).all(|pair| pair[0] != pair[1]));
}

#[test]
fn fragment_attachment_rejects_incomplete_and_mismatched_products_structurally() {
    let name = SourceName::path("public-fragment-failures.arcw");
    let snapshot = SourceSnapshotId::initial(name.clone());
    let document = source_document(&name, "call(");
    let span = document
        .span(SourceRange::new(0, document.text().len()))
        .expect("valid whole-document span");
    let incomplete = parse_expression_fragment(document.text(), ParseOptions::default());
    assert!(matches!(
        incomplete.completion(),
        ParseCompletion::Incomplete { .. }
    ));
    let mut database = SyntaxDatabase::try_new().expect("syntax database");
    assert!(matches!(
        database.attach_fragment(snapshot.clone(), Arc::clone(&document), span, incomplete,),
        Err(ParseFailure::Attachment(
            AttachmentFailure::FragmentNotComplete {
                completion: ParseCompletion::Incomplete { .. }
            }
        ))
    ));

    let mismatch_document = source_document(&name, "other");
    let mismatch_span: SourceSpan = mismatch_document
        .span(SourceRange::new(0, mismatch_document.text().len()))
        .expect("valid mismatch span");
    assert!(matches!(
        database.attach_fragment(
            snapshot,
            mismatch_document,
            mismatch_span,
            parse_expression_fragment("value", ParseOptions::default()),
        ),
        Err(ParseFailure::Attachment(
            AttachmentFailure::FragmentTextMismatch
        ))
    ));
}
