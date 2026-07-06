use arcweft_lang_syntax::{
    ast::{
        items::Item,
        style::StyleSyntax,
        view::{
            ViewAction, ViewActionPayload, ViewExpr, ViewModifier, ViewStyleModifier,
            ViewTextControlPayloadField,
        },
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
fn component_view_button_on_click_text_submit_parses() {
    let parsed = parse_source(
        r#"
pub component FeedbackForm() {
  Column {
    TextField(@input:.feedback, value: "", enter_key: send)
      .label("Message")
      .placeholder("Type text")

    Button(@button:.feedback_send)
      .label("Send")
      .style(@style:.primary_button)
      .enabled(true)
      .focusable(true)
      .on_click(|| text_submit @input:.feedback)
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

    let button = find_button(view.value()).expect("button parsed");
    assert!(matches!(
        button.activation(),
        Some(ViewAction::TextSubmit(_))
    ));
    let field = find_text_field(view.value()).expect("text field parsed");
    assert_eq!(
        field
            .input()
            .map(arcweft_lang_syntax::ast::ids::EntityRefSyntax::canonical_body),
        Some("input.feedback".to_owned())
    );
}

#[test]
fn component_view_button_on_click_action_invoke_block_parses() {
    let parsed = parse_source(
        r#"
pub action feedback.submit(value: String)

pub component FeedbackForm() {
  Button("Continue")
    .on_click {
      action.invoke(@action:.feedback.submit, value = visitor_name.text)
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

    let button = find_button(view.value()).expect("button parsed");
    let Some(ViewAction::ActionInvoke(action)) = button.activation() else {
        panic!("expected action.invoke activation");
    };
    assert_eq!(action.action().canonical_body(), "action.feedback.submit");
    assert_eq!(action.payload_name(), Some("value"));
    assert_eq!(
        action.payload(),
        Some(&ViewActionPayload::TextControlProjection {
            input: "visitor_name".to_owned(),
            field: ViewTextControlPayloadField::Text,
        })
    );
}

#[test]
fn component_view_box_and_scroll_parse_as_canonical_elements() {
    let parsed = parse_source(
        r#"
pub style glass_shell {
  Box {
    background-color = rgba(20, 24, 32, 180)
  }

  Scroll {
    opacity = milli(920)
  }
}

pub component FeedbackForm() {
  Box {
    Scroll {
      Text("Message")
    }
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

    assert!(find_element(view.value(), "Box").is_some());
    assert!(find_element(view.value(), "Scroll").is_some());
}

#[test]
fn component_view_removed_return_annotation_is_rejected() {
    let parsed = parse_source(
        r#"
pub component FeedbackForm() -> View {
  Panel {
    Text("Message")
  }
}
"#,
    );

    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.message().contains("remove the `-> View`"))
    );
}

#[test]
fn removed_view_element_names_are_rejected() {
    let parsed = parse_source(
        r#"
pub component FeedbackForm() {
  Surface {
    Text("Message")
  }
}

pub component ListForm() {
  VStack {
    Text("Message")
  }
}

pub component RowForm() {
  HStack {
    Text("Message")
  }
}
"#,
    );

    let messages = parsed
        .errors()
        .iter()
        .map(arcweft_lang_syntax::parser::recovery::ParseError::message)
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("`Surface` was removed"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("`VStack` was removed"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("`HStack` was removed"))
    );
}

#[test]
fn top_level_ui_text_input_is_rejected() {
    let parsed = parse_source(
        r#"
ui text_input @input.feedback {
  label = "Message"
}
"#,
    );

    assert!(parsed.errors().iter().any(|error| {
        error
            .message()
            .contains("were removed from top-level Arcweft syntax")
    }));
}

#[test]
fn ui_action_button_is_rejected() {
    let parsed = parse_source(
        r#"
ui action_button @button.send {
  label = "Send"
  text_submit = @input.feedback
}
"#,
    );

    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.message().contains("ui action_button"))
    );
}

fn find_button(
    expr: &arcweft_lang_syntax::ast::view::ViewExpr,
) -> Option<&arcweft_lang_syntax::ast::view::ViewButton> {
    match expr {
        ViewExpr::Button(button) => Some(button),
        ViewExpr::Fragment(children) => children.iter().find_map(find_button),
        ViewExpr::Element(element) => element.children().iter().find_map(find_button),
        _ => None,
    }
}

fn find_text_field(
    expr: &arcweft_lang_syntax::ast::view::ViewExpr,
) -> Option<&arcweft_lang_syntax::ast::view::ViewTextField> {
    match expr {
        ViewExpr::TextField(field) => Some(field),
        ViewExpr::Fragment(children) => children.iter().find_map(find_text_field),
        ViewExpr::Element(element) => element.children().iter().find_map(find_text_field),
        _ => None,
    }
}

fn find_element<'a>(
    expr: &'a arcweft_lang_syntax::ast::view::ViewExpr,
    callee: &str,
) -> Option<&'a arcweft_lang_syntax::ast::view::ViewElement> {
    match expr {
        ViewExpr::Element(element) if element.callee() == callee => Some(element),
        ViewExpr::Fragment(children) => children
            .iter()
            .find_map(|child| find_element(child, callee)),
        ViewExpr::Element(element) => element
            .children()
            .iter()
            .find_map(|child| find_element(child, callee)),
        _ => None,
    }
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

pub component ButtonRow() {
    Button(@button:.confirm)
        .label("Confirm")
        .style(@.primary_button)
        .style(@style:.primary_button)
        .style {
            padding-x = milli(24000)
        }
        .style(.Css) {
            color: white;
        }
        .part(confirm)
        .on_click(|| noop)
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

    let button = find_button(view.value()).expect("expected root Button");
    let named_styles = button
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
    assert!(button.modifiers().iter().any(|modifier| matches!(
        modifier,
        ViewModifier::Style(ViewStyleModifier::InlineArcweft(_))
    )));
    assert!(button.modifiers().iter().any(|modifier| matches!(
        modifier,
        ViewModifier::Style(ViewStyleModifier::InlineCss(_))
    )));
}
