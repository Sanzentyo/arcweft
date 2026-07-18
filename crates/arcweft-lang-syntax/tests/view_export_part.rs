use arcweft_lang_syntax::{
    ast::{
        items::Item,
        view::{ViewBody, ViewExpr, ViewModifier},
    },
    parser::{parse_source, recovery::ParseErrorKind},
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
    assert_eq!(declaration.local_name().text(), "header.title");
    assert_eq!(declaration.public_name().text(), "card.heading");
    let local = declaration.local_operand_span().range();
    let public = declaration.public_operand_span().range();
    assert_eq!(&source[local.start()..local.end()], "header.title");
    assert_eq!(&source[public.start()..public.end()], "card.heading");
    let export_keyword = declaration.export_keyword_span().range();
    assert_eq!(
        &source[export_keyword.start()..export_keyword.end()],
        "export"
    );
    let part_keyword = declaration.part_keyword_span().range();
    assert_eq!(&source[part_keyword.start()..part_keyword.end()], "part");
    let as_keyword = declaration.as_keyword_span().range();
    assert_eq!(&source[as_keyword.start()..as_keyword.end()], "as");
    assert_eq!(declaration.declaration_span().source(), parsed.identity());
}

#[test]
fn export_part_excludes_trailing_comment_from_its_declaration_span() {
    let source = "pub view Card() {\n    export part header-row as card.heading // public API\n    Panel().part(header-row)\n}\n";
    let parsed = parse_source(source);
    assert_eq!(parsed.errors(), &[]);

    let declaration = &view_body(&parsed).exports()[0];
    let range = declaration.declaration_span().range();
    assert_eq!(
        &source[range.start()..range.end()],
        "export part header-row as card.heading"
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
    let error = parsed
        .errors()
        .iter()
        .find(|error| error.kind() == ParseErrorKind::ViewExportPartMissingLocal)
        .expect("missing local diagnostic");
    assert_eq!(error.code(), "view::export_part_missing_local");
    assert_eq!(&source[error.range().as_range()], "as");
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
    let error = parsed
        .errors()
        .iter()
        .find(|error| error.kind() == ParseErrorKind::ViewExportPartMisplaced)
        .expect("misplaced export diagnostic");
    assert_eq!(error.code(), "view::export_part_misplaced");
    assert_eq!(
        &source[error.range().as_range()],
        "export part header as heading"
    );
    assert!(view_body(&parsed).exports().is_empty());
}

#[test]
fn malformed_export_families_have_structured_recovery() {
    for (line, kind, range_text) in [
        (
            "export part title heading",
            ParseErrorKind::ViewExportPartMissingAs,
            Some("heading"),
        ),
        (
            "export part title as",
            ParseErrorKind::ViewExportPartMissingPublic,
            None,
        ),
        (
            "export part \"title\" as heading",
            ParseErrorKind::ViewExportPartInvalidLocalName,
            Some("\"title\""),
        ),
        (
            "export part title as \"heading\"",
            ParseErrorKind::ViewExportPartInvalidPublicName,
            Some("\"heading\""),
        ),
        (
            "export part title as heading as other",
            ParseErrorKind::ViewExportPartDuplicateAs,
            Some("as other"),
        ),
        (
            "export part title as heading extra",
            ParseErrorKind::ViewExportPartTrailingSyntax,
            Some("extra"),
        ),
        (
            "export title as heading",
            ParseErrorKind::ViewExportPartMissingPart,
            Some("export title as heading"),
        ),
    ] {
        let source = format!("pub view Card() {{\n    {line}\n    Panel()\n}}\n");
        let parsed = parse_source(&source);
        let error = parsed
            .errors()
            .iter()
            .find(|error| error.kind() == kind)
            .unwrap_or_else(|| panic!("missing `{kind:?}` for `{line}`: {:?}", parsed.errors()));
        assert_eq!(error.code(), kind.code());
        if let Some(range_text) = range_text {
            assert_eq!(&source[error.range().as_range()], range_text, "{kind:?}");
        } else {
            assert_eq!(error.range().start(), error.range().end(), "{kind:?}");
            assert_eq!(
                error.range().start(),
                source.find('\n').unwrap() + 1 + 4 + line.len()
            );
        }
        assert!(view_body(&parsed).exports().is_empty());
    }
}

#[test]
fn malformed_and_duplicate_part_modifiers_do_not_create_partial_labels() {
    for (modifier, kind, range_text) in [
        (".part()", ParseErrorKind::ViewPartMissingName, None),
        (
            ".part(title extra)",
            ParseErrorKind::ViewPartTrailingSyntax,
            Some("extra"),
        ),
        (
            ".part(\"title\")",
            ParseErrorKind::ViewPartInvalidLocalName,
            Some("\"title\""),
        ),
    ] {
        let source = format!("pub view Card() {{\n    Panel()\n        {modifier}\n}}\n");
        let parsed = parse_source(&source);
        let error = parsed
            .errors()
            .iter()
            .find(|error| error.kind() == kind)
            .unwrap_or_else(|| {
                panic!("missing `{kind:?}` for `{modifier}`: {:?}", parsed.errors())
            });
        assert_eq!(error.code(), kind.code());
        if let Some(range_text) = range_text {
            assert_eq!(&source[error.range().as_range()], range_text);
        } else {
            assert_eq!(error.range().start(), error.range().end());
            assert_eq!(
                error.range().start(),
                source.find(".part()").unwrap() + ".part(".len()
            );
        }
    }

    let parsed = parse_source(
        "pub view Card() {\n    Panel()\n        .part(first)\n        .part(second)\n}\n",
    );
    let error = parsed
        .errors()
        .iter()
        .find(|error| error.kind() == ParseErrorKind::ViewDuplicatePartModifier)
        .expect("duplicate part diagnostic");
    assert_eq!(error.code(), "view::duplicate_part_modifier");
    let source = "pub view Card() {\n    Panel()\n        .part(first)\n        .part(second)\n}\n";
    assert_eq!(&source[error.range().as_range()], ".part(second)");
    let ViewExpr::Element(element) = view_body(&parsed).value() else {
        panic!("expected recovered ordinary element");
    };
    assert!(
        element
            .modifiers()
            .iter()
            .all(|modifier| !matches!(modifier, ViewModifier::Part(_)))
    );
}
