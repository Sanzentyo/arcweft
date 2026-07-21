use arcweft_lang_syntax::{
    ast::{common::TextRange, items::Item},
    parser::parse_source,
};

fn text(source: &str, range: TextRange) -> &str {
    &source[range.start()..range.end()]
}

#[test]
fn nominal_declarations_retain_exact_typed_sources_without_payload_reparse() {
    let source = concat!(
        "struct Box<T: Bound> where T: Bound {\n",
        "    value: Result<T, Missing>,\n",
        "}\n",
        "enum Outcome<T> where T: Bound {\n",
        "    Value Result<T, Missing>,\n",
        "    Empty,\n",
        "}\n",
        "type Alias<T> = Result<T, Missing>\n",
        "where T: Bound\n",
    );
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let items = parsed.typed_tree().items();

    let Item::Struct(structure) = &items[0] else {
        panic!("first declaration must be a struct")
    };
    assert_eq!(text(source, *structure.name_range()), "Box");
    assert_eq!(
        structure.generic_range().map(|range| text(source, range)),
        Some("<T: Bound>")
    );
    assert_eq!(structure.generic_params().len(), 1);
    assert_eq!(structure.where_clauses().len(), 1);
    assert_eq!(
        text(source, structure.where_clauses()[0].range()),
        "T: Bound"
    );
    let field = &structure.fields()[0];
    assert_eq!(text(source, field.name_range()), "value");
    assert_eq!(text(source, field.range()), "value: Result<T, Missing>");
    assert_eq!(
        text(source, *field.ty().root_source().whole()),
        "Result<T, Missing>"
    );

    let Item::Enum(enumeration) = &items[1] else {
        panic!("second declaration must be an enum")
    };
    assert_eq!(text(source, *enumeration.name_range()), "Outcome");
    assert_eq!(
        enumeration.generic_range().map(|range| text(source, range)),
        Some("<T>")
    );
    assert_eq!(enumeration.where_clauses().len(), 1);
    let variant = &enumeration.variants()[0];
    assert_eq!(text(source, variant.name_range()), "Value");
    assert_eq!(
        variant.payload_range().map(|range| text(source, range)),
        Some("Result<T, Missing>")
    );
    assert_eq!(
        text(
            source,
            *variant
                .payload()
                .expect("Value has one typed payload")
                .root_source()
                .whole(),
        ),
        "Result<T, Missing>"
    );

    let Item::TypeAlias(alias) = &items[2] else {
        panic!("third declaration must be a type alias")
    };
    assert_eq!(text(source, alias.name_range()), "Alias");
    assert_eq!(
        alias.generic_range().map(|range| text(source, range)),
        Some("<T>")
    );
    assert_eq!(
        text(source, *alias.target().root_source().whole()),
        "Result<T, Missing>"
    );
    assert_eq!(alias.where_clauses().len(), 1);
    assert_eq!(text(source, alias.where_clauses()[0].range()), "T: Bound");
}

#[test]
fn malformed_nominal_payload_is_retained_as_type_recovery() {
    let source = "enum Old {\n    Row { value: Missing },\n}\n";
    let parsed = parse_source(source);
    assert!(!parsed.errors().is_empty());
    let Item::Enum(enumeration) = &parsed.typed_tree().items()[0] else {
        panic!("enum owner remains available during ordinary recovery")
    };
    let payload = enumeration.variants()[0]
        .payload()
        .expect("malformed payload retains one recovery type node");
    assert!(matches!(
        payload.value(),
        arcweft_lang_syntax::types::TypeRef::Recovery(_)
    ));
    assert_eq!(
        text(source, *payload.root_source().whole()),
        "{ value: Missing }"
    );
}
