use arcweft_lang_syntax::{
    ast::{
        items::Item,
        style::StyleSyntax,
        view::{ViewExpr, ViewModifier, ViewStyleModifier},
    },
    parser::parse_source,
};

#[test]
fn style_declarations_are_module_scoped() {
    let parsed = parse_source(
        r"
mod hoge

pub style primary_button {
    Button:hover {
        background-color = rgba(54, 190, 170, 255)
    }
}

pub style @style:.secondary_button {
    Button:active {
        opacity = milli(920)
    }
}

pub style danger_button: .Css {
    Button:hover { background-color: rgb(210 64 92); }
}
",
    );

    assert_eq!(parsed.errors(), &[]);
    let styles = parsed
        .typed_tree()
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Style(style) => Some(style),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(styles.len(), 3);
    assert_eq!(styles[0].id().body(), "style.hoge.primary_button");
    assert_eq!(styles[1].id().body(), "style.hoge.secondary_button");
    assert_eq!(styles[2].id().body(), "style.hoge.danger_button");
    assert_eq!(styles[2].syntax(), StyleSyntax::Css);
    assert!(
        styles[2]
            .inline_source()
            .is_some_and(|source| { source.contains("background-color") })
    );
}

#[test]
fn component_view_style_references_are_module_scoped() {
    let parsed = parse_source(
        r#"
mod hoge

pub style primary_button {
    Button:hover {
        background-color = rgba(54, 190, 170, 255)
    }
}

pub component ButtonRow() -> View {
    Button("Confirm")
        .style(@.primary_button)
        .style(@style:.primary_button)
        .style {
            padding-x = milli(24000)
        }
        .style(.Css) {
            color: white;
        }
        .part(confirm)
        .on_click {
            true
        }
}
"#,
    );

    assert_eq!(parsed.errors(), &[]);
    let view = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) => item.component_body()?.view(),
            _ => None,
        })
        .expect("component View body");

    let ViewExpr::Element(element) = view.value() else {
        panic!("expected root Button element");
    };
    let named_styles = element
        .modifiers()
        .iter()
        .filter_map(|modifier| match modifier {
            ViewModifier::Style(ViewStyleModifier::Named(reference)) => reference
                .as_absolute()
                .map(arcweft_lang_syntax::ast::ids::EntityRef::body),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        named_styles,
        ["style.hoge.primary_button", "style.hoge.primary_button"]
    );
    assert!(element.modifiers().iter().any(|modifier| matches!(
        modifier,
        ViewModifier::Style(ViewStyleModifier::InlineArcweft(_))
    )));
    assert!(element.modifiers().iter().any(|modifier| matches!(
        modifier,
        ViewModifier::Style(ViewStyleModifier::InlineCss(_))
    )));
}
