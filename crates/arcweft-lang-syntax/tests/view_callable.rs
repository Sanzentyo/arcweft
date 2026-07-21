use arcweft_lang_syntax::{
    ast::items::{EntityDeclKind, Item},
    ast::view::{ViewBody, ViewExpr, ViewModifier, ViewStyleModifier},
    parser::parse_source,
};

#[test]
fn typed_view_declaration_is_the_only_view_callable_owner() {
    let parsed = parse_source("pub view Card() {\n    Panel()\n}\n");
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let view = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) if item.kind() == EntityDeclKind::View => Some(item),
            _ => None,
        })
        .expect("typed View declaration");

    assert_eq!(view.local_binding_name(), Some("Card"));
    assert_eq!(view.signature_tail(), "()");
    let body = view.view_body().expect("typed View body");
    let signature = body.signature().expect("typed View signature");
    assert_eq!(signature.name(), "view");
    assert!(signature.return_type().is_none());
    assert!(body.view().is_some());
}

#[test]
fn malformed_nested_view_values_remain_recovery_not_executable_syntax() {
    for source in [
        "pub view Broken(value:) {\n    Text(\"x\")\n}\n",
        "pub view Broken() {\n    Text(@@@)\n}\n",
        "pub view Broken() {\n    Panel(width = @@@)\n}\n",
        "pub view Broken() {\n    Scroll(axis = @@@) { Text(\"x\") }\n}\n",
        "pub view Broken() {\n    if @@@ { Text(\"x\") }\n}\n",
        "pub view Broken(value: i32) {\n    match value {\n        ??? => Text(\"x\")\n    }\n}\n",
        "pub view Broken(value: i32) {\n    match value {\n        .MissingArrow Text(\"x\")\n    }\n}\n",
        "pub view Broken() {\n    Button(\"x\").unknown_modifier(@@@)\n}\n",
        "pub view Broken() {\n    Button(\"x\").on_focus { wait(@@@) }\n}\n",
        "pub view Broken(items: Vec<Item>) {\n    for item in items key item.id {\n        Text(\"x\")\n    }\n}\n",
        "pub view Broken() {\n    Button(\"x\").nav(sideways: auto)\n}\n",
        "pub view Broken() {\n    Button(\"x\").nav(right: nowhere)\n}\n",
        "pub view Broken() {\n    Button(\"x\").nav(@button:.next)\n}\n",
        "pub view Broken() {\n    Button(\"x\").nav(right: auto, right: none)\n}\n",
    ] {
        let parsed = parse_source(source);
        let body = parsed
            .typed_tree()
            .items()
            .iter()
            .find_map(|item| match item {
                Item::EntityDecl(item) if item.kind() == EntityDeclKind::View => item.view_body(),
                _ => None,
            })
            .expect("recovered View declaration");
        assert!(
            body.has_recovery() || body.view().is_some_and(ViewBody::contains_recovered_syntax),
            "malformed View must retain non-executable recovery: {source}; body={:?}; errors={:?}",
            body.view().map(ViewBody::value),
            parsed.errors(),
        );
    }
}

#[test]
fn same_shaped_view_nodes_and_style_modifiers_keep_distinct_authored_ranges() {
    let source = r#"pub view SameShape() {
    Column {
        Button("Same").style(@style.primary)
        Button("Same").style(@style.second)
    }
}
"#;
    let parsed = parse_source(source);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let body = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) if item.kind() == EntityDeclKind::View => {
                item.view_body()?.view()
            }
            _ => None,
        })
        .expect("typed View body");
    let ViewExpr::Element(column) = body.value() else {
        panic!("expected root Column");
    };
    let [ViewExpr::Button(first), ViewExpr::Button(second)] = column.children() else {
        panic!("expected two Button children");
    };

    assert_ne!(first.range(), second.range());
    assert_eq!(
        &source[first.range().as_range()],
        "Button(\"Same\").style(@style.primary)"
    );
    assert_eq!(
        &source[second.range().as_range()],
        "Button(\"Same\").style(@style.second)"
    );

    let named_style_range = |modifiers: &[ViewModifier]| {
        modifiers.iter().find_map(|modifier| match modifier {
            ViewModifier::Style(ViewStyleModifier::Named(reference)) => Some(*reference.range()),
            _ => None,
        })
    };
    let first_style = named_style_range(first.modifiers()).expect("first style range");
    let second_style = named_style_range(second.modifiers()).expect("second style range");
    assert_ne!(first_style, second_style);
    assert_eq!(&source[first_style.as_range()], "@style.primary");
    assert_eq!(&source[second_style.as_range()], "@style.second");
}

#[test]
fn malformed_for_keys_and_navigation_arguments_remain_non_executable_recovery() {
    for (source, expected_message) in [
        (
            "pub view Broken(items: Vec<Item>) {\n    for item in items key item.id {\n        Text(\"x\")\n    }\n}\n",
            "key needs `=`",
        ),
        (
            "pub view Broken() {\n    Button(\"x\").nav(sideways: auto)\n}\n",
            "unknown View navigation direction",
        ),
        (
            "pub view Broken() {\n    Button(\"x\").nav(right: nowhere)\n}\n",
            "invalid View navigation target",
        ),
        (
            "pub view Broken() {\n    Button(\"x\").nav(@button:.next)\n}\n",
            "must name a direction",
        ),
    ] {
        let parsed = parse_source(source);
        assert!(
            parsed
                .errors()
                .iter()
                .any(|error| error.message().contains(expected_message)),
            "missing `{expected_message}` for {source}: {:?}",
            parsed.errors()
        );
        let body = parsed
            .typed_tree()
            .items()
            .iter()
            .find_map(|item| match item {
                Item::EntityDecl(item) if item.kind() == EntityDeclKind::View => {
                    item.view_body()?.view()
                }
                _ => None,
            })
            .expect("recovered typed View body");
        assert!(body.contains_recovered_syntax());
    }
}

#[test]
fn view_without_a_body_retains_a_non_executable_typed_owner() {
    let parsed = parse_source("pub view Broken()\n");
    assert!(!parsed.errors().is_empty());
    let body = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) if item.kind() == EntityDeclKind::View => item.view_body(),
            _ => None,
        })
        .expect("missing-body View retains its typed declaration owner");
    assert!(body.signature().is_some());
    assert!(body.view().is_none());
    assert!(body.has_recovery());
}
