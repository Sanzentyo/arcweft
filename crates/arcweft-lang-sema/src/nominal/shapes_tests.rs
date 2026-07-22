use crate::{
    checker::analyze_registered_project_types,
    env::TypeCheckEnv,
    registration::ProjectRegistrationFacts,
    test_support::character_project::{project_modules, register, root_project_source},
};

fn registered_report(profile: &str, source: &str) -> crate::checker::TypeCheckReport {
    let (document, project, world) = root_project_source(profile, source);
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![document],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("project nominal shape registration facts");
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("project nominal shape semantic world");
    analyze_registered_project_types(&project.linked_module(), &registered)
}

#[test]
fn generic_record_constructor_substitutes_the_expected_project_instantiation() {
    let report = registered_report(
        "generic-record-constructor",
        r"
struct Boxed<T> {
    value: T
}

fn boxed_i64() -> Boxed<i64>
effects {}
{
    Boxed { value: 1i64 }
}
",
    );
    assert!(
        report.diagnostics.is_empty(),
        "generic project record fields must use substituted types: {:?}",
        report.diagnostics
    );
}

#[test]
fn generic_record_constructor_rejects_a_value_after_substitution() {
    let report = registered_report(
        "generic-record-constructor-mismatch",
        r#"
struct Boxed<T> {
    value: T
}

fn boxed_i64() -> Boxed<i64>
effects {}
{
    Boxed { value: "wrong" }
}
"#,
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|error| error.message().contains("record field `Boxed.value`")),
        "substituted project record mismatch must be diagnosed: {:?}",
        report.diagnostics
    );
}

#[test]
fn same_terminal_record_names_use_their_exact_module_declarations() {
    let root = r#"
use crate.numeric.Payload as NumericPayload
use crate.textual.Payload as TextualPayload

fn make_numeric() -> crate.numeric.Payload
effects {}
{
    NumericPayload { value: 1i64 }
}

fn make_textual() -> crate.textual.Payload
effects {}
{
    TextualPayload { value: "ok" }
}
"#;
    let (documents, project, world) = project_modules(
        "qualified-record-constructors",
        &[
            ("", root),
            ("numeric", "pub struct Payload { value: i64 }\n"),
            ("textual", "pub struct Payload { value: String }\n"),
        ],
    );
    let facts =
        ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new(), Vec::new())
            .expect("qualified project nominal registration facts");
    let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
        .expect("qualified project nominal semantic world");
    let report = analyze_registered_project_types(&project.linked_module(), &registered);
    assert!(
        report.diagnostics.is_empty(),
        "record lookup must not use a terminal-name field map: {:?}",
        report.diagnostics
    );
}

#[test]
fn detached_record_checking_does_not_fabricate_a_named_project_type() {
    let parsed = arcweft_lang_syntax::parser::parse_source(
        "struct Local { value: i64 }\nfn make() -> Local\neffects {}\n{ Local { value: 1i64 } }\n",
    );
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = arcweft_lang_hir::lower::lower_to_hir(parsed.typed_tree())
        .expect("detached project record fixture lowers");
    let report = crate::checker::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        report.diagnostics.iter().any(|error| error
            .message()
            .contains("unknown record constructor `Local`")),
        "detached checking must fail closed instead of returning Named(\"Local\"): {:?}",
        report.diagnostics
    );
}
