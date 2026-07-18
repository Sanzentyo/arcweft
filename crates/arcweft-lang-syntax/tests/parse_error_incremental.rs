use std::sync::Arc;

use arcweft_lang_syntax::{incremental::SyntaxDatabase, parser::recovery::ParseErrorKind};
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange,
    identity::SourceSnapshotId,
};

#[test]
fn incremental_snapshots_retain_typed_parse_error_kinds() {
    let name = SourceName::path("view.arcw");
    let source = "pub view Card() {\n    export part as heading\n    Panel()\n}\n";
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-project://test/view.arcw")
            .expect("valid test document id"),
        name.clone(),
        Arc::<str>::from(source),
    )
    .expect("test source document");
    let mut database = SyntaxDatabase::default();
    let initial = database
        .parse_initial(SourceSnapshotId::initial(name), document)
        .expect("recovered source commits");

    assert_eq!(
        initial.diagnostics()[0].kind(),
        ParseErrorKind::ViewExportPartMissingLocal
    );

    let insertion = source.find("Panel").expect("fixture expression");
    let edit = SourceEdit::new(
        initial
            .document()
            .span(SourceRange::new(insertion, insertion))
            .expect("edit belongs to current source revision"),
        "  ",
    );
    let reparsed = database
        .reparse(&initial, &[edit])
        .expect("trivia edit commits");

    assert_eq!(
        reparsed.diagnostics()[0].kind(),
        ParseErrorKind::ViewExportPartMissingLocal
    );
    assert_eq!(initial.diagnostics(), reparsed.diagnostics());
}
