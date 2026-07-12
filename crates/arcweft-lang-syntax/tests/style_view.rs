use arcweft_lang_syntax::{
    ast::{
        items::{Item, ViewStyleValueDecl},
        style::StyleSyntax,
        view::{
            ViewAction, ViewActionPayload, ViewAwaitBranchKind, ViewExpr, ViewModifier,
            ViewStyleModifier, ViewTextControlPayloadField,
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
        opacity = 920milli
        z-index = milli(920)
        border-radius = 12px
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
    let active_declarations = styles[1].rules()[0].declarations();
    assert_eq!(
        active_declarations[0].value(),
        active_declarations[1].value()
    );
    assert_eq!(
        active_declarations[0].value(),
        &ViewStyleValueDecl::Milli(920)
    );
    assert_eq!(
        active_declarations[2].value(),
        &ViewStyleValueDecl::Text("12px".to_owned())
    );
    assert!(
        styles[2]
            .inline_source()
            .is_some_and(|source| { source.contains("background-color") })
    );
}

#[test]
fn view_button_on_click_action_invoke_parses() {
    let parsed = parse_source(
        r#"
pub action feedback.submit(value: String)

pub view FeedbackForm() {
  Column {
    TextField(@input:.feedback, value: "", enter_key: send)
      .label("Message")
      .placeholder("Type text")

    Button(@button:.feedback_send)
      .label("Send")
      .style(@style:.primary_button)
      .enabled(true)
      .focusable(true)
      .on_click(|| action.invoke(@action:.feedback.submit, value = @input:.feedback.text))
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
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("view View body");

    let button = find_button(view.value()).expect("button parsed");
    assert!(matches!(
        button.activation(),
        Some(ViewAction::ActionInvoke(_))
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
fn view_button_on_click_action_invoke_block_parses() {
    let parsed = parse_source(
        r#"
pub action feedback.submit(value: String)

pub view FeedbackForm() {
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
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("view View body");

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
fn view_text_field_on_submit_action_invoke_block_parses() {
    let parsed = parse_source(
        r#"
pub action feedback.submit(value: String)

pub view FeedbackForm() {
  let feedback = input.text(@input:.feedback, initial = "")

  TextField(feedback)
    .purpose(.text)
    .enter_key(.send)
    .on_submit {
      action.invoke(@action:.feedback.submit, value = feedback.text)
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
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("view body");
    let invokes = view.action_invokes();
    assert_eq!(invokes.len(), 1);
    assert_eq!(
        invokes[0].action().canonical_body(),
        "action.feedback.submit"
    );
}

#[test]
fn view_generic_callback_block_modifier_parses() {
    let parsed = parse_source(
        r#"
pub action feedback.focus(value: String)

pub view FeedbackForm() {
  Button("Continue")
    .on_focus {
      action.invoke(@action:.feedback.focus, value = "focused")
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
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("view body");
    let button = find_button(view.value()).expect("button parsed");
    assert!(button.modifiers().iter().any(|modifier| {
        matches!(
            modifier,
            ViewModifier::OnEvent { name, body }
                if name == "focus" && matches!(body, arcweft_lang_syntax::expr::Expr::Block { .. })
        )
    }));
    assert!(button.activation().is_none());
    let invokes = view.action_invokes();
    assert_eq!(invokes.len(), 1);
    assert_eq!(
        invokes[0].action().canonical_body(),
        "action.feedback.focus"
    );
}

#[test]
fn view_button_on_click_multi_statement_block_uses_final_action() {
    let parsed = parse_source(
        r#"
pub action feedback.submit(value: String)

pub view FeedbackForm() {
  Button("Continue")
    .on_click {
      let value = visitor_name.text
      action.invoke(@action:.feedback.submit, value = "ready")
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
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("view View body");

    let button = find_button(view.value()).expect("button parsed");
    let Some(ViewAction::ActionInvoke(action)) = button.activation() else {
        panic!("expected action.invoke activation");
    };
    assert_eq!(action.action().canonical_body(), "action.feedback.submit");
    assert_eq!(action.payload_name(), Some("value"));
    assert_eq!(
        action.payload(),
        Some(&ViewActionPayload::LiteralString("ready".to_owned()))
    );
}

#[test]
fn view_local_let_input_handle_parses() {
    let parsed = parse_source(
        r#"
pub view FeedbackForm() {
  let visitor_name = input.text(@input:.visitor_name, initial = "")
  Column {
    TextField(visitor_name)
      .placeholder("Your name")
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
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("view View body");

    let ViewExpr::Fragment(items) = view.value() else {
        panic!("expected root View fragment");
    };
    let Some(ViewExpr::Let(binding)) = items.first() else {
        panic!("expected View-local let binding");
    };
    assert_eq!(
        binding.pattern().simple_binding_name(),
        Some("visitor_name")
    );
    assert_eq!(
        view.text_control_inputs()
            .into_iter()
            .map(arcweft_lang_syntax::ast::ids::EntityRefSyntax::canonical_body)
            .collect::<Vec<_>>(),
        vec!["input.visitor_name".to_owned()]
    );
}

#[test]
fn view_reactive_if_match_for_parse_to_structured_view_exprs() {
    let parsed = parse_source(
        r"
pub view ReactivePanel() {
  Column {
    if true {
      TextField(@input:.empty)
    } else {
      TextField(@input:.available)
    }

    for choice in [1, 2] key = choice {
      TextField(@input:.choice)
    }

    match .Debug {
      .Normal => TextField(@input:.normal)
      .Debug => TextField(@input:.debug)
    }
  }
}
",
    );

    assert_eq!(parsed.errors(), &[]);
    let view = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("view View body");

    let column = find_element(view.value(), "Column").expect("column parsed");
    assert!(matches!(column.children().first(), Some(ViewExpr::If(_))));
    assert!(matches!(
        column.children().get(1),
        Some(ViewExpr::ForEach(_))
    ));
    assert!(matches!(column.children().get(2), Some(ViewExpr::Match(_))));
    let inputs = view
        .text_control_inputs()
        .into_iter()
        .map(arcweft_lang_syntax::ast::ids::EntityRefSyntax::canonical_body)
        .collect::<Vec<_>>();
    assert_eq!(
        inputs,
        vec![
            "input.empty".to_owned(),
            "input.available".to_owned(),
            "input.choice".to_owned(),
            "input.normal".to_owned(),
            "input.debug".to_owned()
        ]
    );
}

#[test]
fn view_await_parse_to_structured_branches() {
    let parsed = parse_source(
        r"
pub view AvatarPanel() {
  Column {
    AwaitView(load_avatar(user)) {
      pending _ => TextField(@input:.loading)
      ready img => Image(img)
      error _ => TextField(@input:.fallback)
    }
  }
}
",
    );

    assert_eq!(parsed.errors(), &[]);
    let view = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("view View body");

    let column = find_element(view.value(), "Column").expect("column parsed");
    let Some(ViewExpr::Await(view_await)) = column.children().first() else {
        panic!("expected View await expression");
    };
    let kinds = view_await
        .branches()
        .iter()
        .map(arcweft_lang_syntax::ast::view::ViewAwaitBranch::kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            ViewAwaitBranchKind::Pending,
            ViewAwaitBranchKind::Ready,
            ViewAwaitBranchKind::Error
        ]
    );
    let inputs = view
        .text_control_inputs()
        .into_iter()
        .map(arcweft_lang_syntax::ast::ids::EntityRefSyntax::canonical_body)
        .collect::<Vec<_>>();
    assert_eq!(
        inputs,
        vec!["input.loading".to_owned(), "input.fallback".to_owned()]
    );
}

#[test]
fn view_box_and_scroll_parse_as_canonical_elements() {
    let parsed = parse_source(
        r#"
pub style glass_shell {
  Box {
    background-color = rgba(20, 24, 32, 180)
  }

  Scroll {
    axis = text("vertical")
    opacity = milli(920)
  }
}

pub view FeedbackForm() {
  Box {
    Scroll(id = @scroll:.feedback_body, axis = .vertical, width = 360px, height = 120px, overflow = .hidden) {
      Text("Message")
    }

    Button(@button:.send)
      .width(220px)
      .clip(false)
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
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("view View body");

    assert!(find_element(view.value(), "Box").is_some());
    let scroll = find_element(view.value(), "Scroll").expect("scroll parsed");
    assert_eq!(scroll.args().len(), 5);
    let button = find_button(view.value()).expect("button parsed");
    assert!(button.modifiers().iter().any(
        |modifier| matches!(modifier, ViewModifier::Property { name, .. } if name == "width")
    ));
    assert!(
        button.modifiers().iter().any(
            |modifier| matches!(modifier, ViewModifier::Property { name, .. } if name == "clip")
        )
    );
}

#[test]
fn view_fx_modifiers_keep_typed_calls_keys_and_authored_ordinals() {
    let parsed = parse_source(
        r#"
pub view Warning(state: WarningState) {
  Text("WARNING")
    .fx(
      notice(
        accent = state.warning_color,
        amplitude = 2px,
      ),
      key = state.warning_id,
    )
    .fx(pulse(speed = 1.5))
}
"#,
    );

    assert_eq!(parsed.errors(), &[]);
    let view = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("view View body");
    let applications = view.fx_applications();

    assert_eq!(applications.len(), 2);
    assert_eq!(applications[0].ordinal().get(), 0);
    assert!(applications[0].key().is_some());
    assert_eq!(applications[1].ordinal().get(), 1);
    assert!(applications[1].key().is_none());
    assert!(matches!(
        applications[0].call(),
        arcweft_lang_syntax::expr::Expr::Call { args, .. }
            if args.len() == 2
                && args.iter().all(|arg| matches!(
                    arg,
                    arcweft_lang_syntax::expr::CallArg::Named { .. }
                ))
    ));
}

#[test]
fn view_fx_rejects_positional_function_arguments_and_open_modifier_options() {
    let parsed = parse_source(
        r#"
pub view InvalidFx() {
  Text("WARNING")
    .fx(notice("red"))
    .fx(notice(), seed = 4)
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
            .any(|message| message.contains("named-only"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("unknown View `.fx` option `seed`"))
    );
}

#[test]
fn unsupported_view_block_element_names_are_rejected() {
    let parsed = parse_source(
        r#"
pub view FeedbackForm() {
  Card {
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
            .any(|message| message.contains("unsupported View element `Card`"))
    );
}

#[test]
fn non_intrinsic_view_calls_are_preserved_for_typed_resolution() {
    let parsed = parse_source(
        r#"
mod game.opening

pub view Child(label: String) {
  Text(label)
}

pub view Parent() {
  Child(label = "Message")
}

pub view ParentRelative() {
  @view:.Child(label = "Relative")
}
"#,
    );

    assert_eq!(parsed.errors(), &[]);
    let parent = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(view) if view.id().body() == "view.game.opening.Parent" => {
                view.view_body()?.view()
            }
            _ => None,
        })
        .expect("parent View");
    assert!(matches!(
        parent.value(),
        ViewExpr::ViewCall(call)
            if call.args().len() == 1
                && matches!(call.view(), arcweft_lang_syntax::expr::Expr::EntityRef(reference) if reference.canonical_body() == "view.game.opening.Child")
    ));
    let relative = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(view) if view.id().body() == "view.game.opening.ParentRelative" => {
                view.view_body()?.view()
            }
            _ => None,
        })
        .expect("relative parent View");
    assert!(matches!(
        relative.value(),
        ViewExpr::ViewCall(call)
            if matches!(call.view(), arcweft_lang_syntax::expr::Expr::EntityRef(reference) if reference.canonical_body() == "view.game.opening.Child")
    ));
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
fn view_style_references_are_module_scoped() {
    let parsed = parse_source(
        r#"
mod hoge

pub style primary_button {
    Button:hover {
        background-color = rgba(54, 190, 170, 255)
    }
}

pub view ButtonRow() {
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
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("view View body");

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

#[test]
fn view_container_accepts_a_trailing_modifier_chain() {
    let parsed = parse_source(
        r#"
pub view DialoguePanel() {
    Panel(width = 400px, height = 160px) {
        Text("Hello")
    }
        .part(dialogue_panel)
}
"#,
    );

    assert_eq!(parsed.errors(), &[]);
    let view = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("view View body");
    let panel = find_element(view.value(), "Panel").expect("expected root Panel");
    assert!(matches!(
        panel.modifiers(),
        [ViewModifier::Part(part)] if part == "dialogue_panel"
    ));
}
