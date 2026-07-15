use arcweft_lang_syntax::{
    ast::{items::Item, view::ViewBody},
    parser::parse_source,
    source::ParsedSource,
};

fn first_view(source: &str) -> (ParsedSource, &str) {
    (parse_source(source), source)
}

fn view_body(parsed: &ParsedSource) -> &ViewBody {
    parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("typed View body")
}

#[test]
fn view_export_part_parses_canonical_declaration_with_exact_ranges() {
    let source = r"pub view Card() {
    export part header.title as card.heading
    Panel().part(header.title)
}
";
    let (parsed, source) = first_view(source);
    assert_eq!(parsed.errors(), &[]);

    let declaration = &view_body(&parsed).exports()[0];
    assert_eq!(declaration.local().text(), "header.title");
    assert_eq!(declaration.public().text(), "card.heading");
    assert_eq!(
        &source[declaration.local().range().start()..declaration.local().range().end()],
        "header.title"
    );
    assert_eq!(
        &source[declaration.public().range().start()..declaration.public().range().end()],
        "card.heading"
    );
    assert_eq!(
        &source
            [declaration.export_keyword_range().start()..declaration.export_keyword_range().end()],
        "export"
    );
    assert_eq!(
        &source[declaration.part_keyword_range().start()..declaration.part_keyword_range().end()],
        "part"
    );
    assert_eq!(
        &source[declaration.as_keyword_range().start()..declaration.as_keyword_range().end()],
        "as"
    );
}

#[test]
fn malformed_export_recovers_without_creating_partial_declaration() {
    let source = r"pub view Card() {
    export part as card.heading
    Panel().part(header)
}
";
    let parsed = parse_source(source);
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.code() == "view::export_part_missing_local")
    );
    assert!(view_body(&parsed).exports().is_empty());
}

#[test]
fn export_after_view_expression_is_misplaced_and_does_not_lower() {
    let source = r"pub view Card() {
    Panel().part(header)
    export part header as heading
}
";
    let parsed = parse_source(source);
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.code() == "view::export_part_misplaced")
    );
    assert!(view_body(&parsed).exports().is_empty());
}

#[test]
fn malformed_export_families_have_structured_recovery() {
    for (line, code) in [
        ("export part title heading", "view::export_part_missing_as"),
        ("export part title as", "view::export_part_missing_public"),
        (
            "export part \"title\" as heading",
            "view::export_part_invalid_local_name",
        ),
        (
            "export part title as \"heading\"",
            "view::export_part_invalid_public_name",
        ),
        (
            "export part title as heading as other",
            "view::export_part_duplicate_as",
        ),
        (
            "export part title as heading extra",
            "view::export_part_trailing_syntax",
        ),
        (
            "export title as heading",
            "view::unsupported_export_spelling",
        ),
        (
            "exportparts title as heading",
            "view::unsupported_export_spelling",
        ),
    ] {
        let source = format!("pub view Card() {{\n    {line}\n    Panel()\n}}\n");
        let parsed = parse_source(&source);
        assert!(
            parsed.errors().iter().any(|error| error.code() == code),
            "missing `{code}` for `{line}`: {:?}",
            parsed.errors(),
        );
        assert!(view_body(&parsed).exports().is_empty());
    }
}

#[test]
fn malformed_and_duplicate_part_modifiers_do_not_create_partial_labels() {
    for (modifier, code) in [
        (".part()", "view::part_missing_name"),
        (".part(title extra)", "view::part_trailing_syntax"),
        (".part(\"title\")", "view::part_invalid_local_name"),
        (".export_part(heading)", "view::unsupported_export_spelling"),
    ] {
        let source = format!("pub view Card() {{\n    Panel()\n        {modifier}\n}}\n");
        let parsed = parse_source(&source);
        assert!(
            parsed.errors().iter().any(|error| error.code() == code),
            "missing `{code}` for `{modifier}`: {:?}",
            parsed.errors(),
        );
    }

    let parsed = parse_source(
        "pub view Card() {\n    Panel()\n        .part(first)\n        .part(second)\n}\n",
    );
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.code() == "view::duplicate_part_modifier")
    );
}
