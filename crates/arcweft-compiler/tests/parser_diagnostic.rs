use arcweft_compiler::{error::CompileSourceError, source::compile_source};
use arcweft_lang_syntax::parser::{parse_source, recovery::ParseErrorKind};

const MISSING_AS_SOURCE: &str =
    "pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n";

#[test]
fn parser_diagnostic_compiler_forwarding_preserves_the_complete_logical_value() {
    let parsed = parse_source(MISSING_AS_SOURCE);
    let syntax_diagnostics = parsed.errors().to_vec();
    assert_eq!(syntax_diagnostics.len(), 1);
    assert_eq!(
        syntax_diagnostics[0].kind(),
        ParseErrorKind::ViewExportPartMissingAs
    );

    let CompileSourceError::Parse(compiler_diagnostics) =
        compile_source(MISSING_AS_SOURCE).expect_err("missing `as` must stop compilation")
    else {
        panic!("expected typed parser diagnostics");
    };

    assert_eq!(compiler_diagnostics, syntax_diagnostics);
}
