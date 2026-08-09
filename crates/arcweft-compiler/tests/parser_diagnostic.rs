use arcweft_compiler::{project::ProjectCompileStage, source::compile_source};

const MISSING_AS_SOURCE: &str =
    "pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n";

#[test]
fn compiler_retains_the_exact_parse_diagnostic_in_a_readiness_failure() {
    let compiler_error =
        compile_source(MISSING_AS_SOURCE).expect_err("missing `as` must stop compilation");
    let project = compiler_error.project();
    assert_eq!(project.stage(), ProjectCompileStage::Readiness.as_str());
    let diagnostics = project.diagnostics();
    let owned = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .syntax_diagnostic()
                .is_some_and(|diagnostic| diagnostic.code() == "view::export_part_missing_as")
        })
        .expect("missing-as compiler diagnostic");
    assert_eq!(owned.stage(), ProjectCompileStage::Parse);
    let diagnostic = owned
        .syntax_diagnostic()
        .expect("parse-stage diagnostic retains attached-source payload");
    assert_eq!(diagnostic.code(), "view::export_part_missing_as");
    diagnostic
        .primary()
        .validate_for(owned.source().expect("diagnostic source").document())
        .expect("diagnostic remains bound to compiler source revision");
}
