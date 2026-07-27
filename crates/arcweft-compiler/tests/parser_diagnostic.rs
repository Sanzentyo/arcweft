use arcweft_compiler::{project::ProjectCompileStage, source::compile_source};
use arcweft_lang_syntax::parser::{
    ParseOptions, parse_document_with_source, recovery::ParseErrorKind,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::sync::Arc;

const MISSING_AS_SOURCE: &str =
    "pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n";

#[test]
fn parser_diagnostic_compiler_forwarding_preserves_the_complete_logical_value() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/parser/missing-as.arcw")
                .expect("missing-as fixture source ID"),
            SourceName::path("compiler/parser/missing-as.arcw"),
            MISSING_AS_SOURCE,
        )
        .expect("missing-as fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let syntax_diagnostics = parsed.errors().to_vec();
    assert_eq!(syntax_diagnostics.len(), 1);
    assert_eq!(
        syntax_diagnostics[0].kind(),
        ParseErrorKind::ViewExportPartMissingAs
    );

    let compiler_error =
        compile_source(MISSING_AS_SOURCE).expect_err("missing `as` must stop compilation");
    let project = compiler_error.project();
    assert_eq!(project.stage(), ProjectCompileStage::Parse.as_str());
    let compiler_diagnostics = project
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            diagnostic
                .parse_error()
                .expect("parse-stage diagnostics preserve their typed parser payload")
                .clone()
        })
        .collect::<Vec<_>>();

    assert_eq!(compiler_diagnostics, syntax_diagnostics);
}
