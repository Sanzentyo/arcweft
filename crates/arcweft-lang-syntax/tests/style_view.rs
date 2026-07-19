use arcweft_lang_syntax::{
    ast::{
        items::Item,
        view::{
            ViewAction, ViewActionPayload, ViewAwaitBranchKind, ViewExpr, ViewModifier,
            ViewStyleModifier, ViewTextControlPayloadField,
        },
    },
    expr::{Expr, Literal, UnitNumberSuffix},
    parser::{parse_source, recovery::ParseErrorKind},
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
        z-index = 920
        border-radius = 12px
    }
}

pub style danger_button {
    Button:hover { background-color = rgba(210, 64, 92, 255) }
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
    let active_declarations = styles[1].sheet().body()[0]
        .as_rule()
        .expect("top-level rule")
        .declarations();
    assert!(matches!(
        active_declarations[0].value().expr(),
        Expr::Literal(Literal::UnitNumber { raw, suffix: UnitNumberSuffix::Milli })
            if raw == "920milli"
    ));
    assert!(matches!(
        active_declarations[1].value().expr(),
        Expr::Literal(Literal::Int(value)) if value.raw() == "920"
    ));
    assert!(matches!(
        active_declarations[2].value().expr(),
        Expr::Literal(Literal::UnitNumber { raw, suffix: UnitNumberSuffix::Px })
            if raw == "12px"
    ));
    assert_eq!(
        styles[2].sheet().body()[0]
            .as_rule()
            .expect("top-level rule")
            .declarations()
            .len(),
        1
    );
}

#[test]
fn native_style_multiline_values_keep_expression_and_source_ranges() {
    let source = r"pub style control {
    token shadow.control: ShadowList = [
        shadow(
            x = 0px,
            y = 12px,
            blur = 28px,
            spread = 0px,
            color = rgba(0, 0, 0, 92),
        ),
    ]

    Button:hover {
        box-shadow = token(shadow.control)
    }
}
";
    let parsed = parse_source(source);
    assert_eq!(parsed.errors(), &[]);
    let style = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Style(style) => Some(style),
            _ => None,
        })
        .expect("style declaration");
    let sheet = style.sheet();
    let token = &sheet.tokens()[0];
    assert!(matches!(token.value().expr(), Expr::BracketSeq(values) if values.len() == 1));
    assert_eq!(
        &source[token.value().range().as_range()],
        token.value().source()
    );
    let declaration = &sheet.body()[0]
        .as_rule()
        .expect("top-level rule")
        .declarations()[0];
    assert!(matches!(declaration.value().expr(), Expr::Call(_)));
    assert_eq!(
        &source[declaration.value().range().as_range()],
        declaration.value().source()
    );
}

#[test]
fn style_parser_reports_missing_equals_and_malformed_combinators_with_ranges() {
    let missing = parse_source("pub style broken {\n token color.text Color\n}\n");
    let missing_error = missing
        .errors()
        .iter()
        .find(|error| error.message().contains("needs `=`"))
        .expect("missing equals diagnostic");
    assert!(missing_error.range().end() > missing_error.range().start());

    let selector = parse_source(
        "pub style broken {\n Panel > > Button {\n color = rgba(0, 0, 0, 255)\n }\n}\n",
    );
    let selector_error = selector
        .errors()
        .iter()
        .find(|error| error.message().contains("child combinator"))
        .expect("malformed selector diagnostic");
    assert_eq!(
        selector_error.range().end() - selector_error.range().start(),
        1
    );
}

#[test]
fn unexpected_named_style_head_suffixes_use_ordinary_parser_recovery() {
    let source = r"pub style imported: .ForeignDialect { Button { color = red; } }
pub style explicit: .UnknownDialect { Button { color = rgba(1, 2, 3, 255) } }
";
    let parsed = parse_source(source);
    let head_errors = parsed
        .errors()
        .iter()
        .filter(|error| error.message() == "unexpected text after style declaration head")
        .collect::<Vec<_>>();
    assert_eq!(head_errors.len(), 2);
    assert!(head_errors.iter().all(|error| error.found().is_none()));
    assert!(
        parsed
            .errors()
            .iter()
            .all(|error| error.code() == "syntax.parse")
    );
}

#[test]
fn named_and_inline_style_assignments_without_equals_use_parser_recovery() {
    let source = r#"pub style broken {
    Button { color red }
}
pub view Example() {
    Button("OK").style { opacity 0.9 }
}
"#;
    let parsed = parse_source(source);
    let diagnostics = parsed
        .errors()
        .iter()
        .filter(|error| error.message() == "style declaration needs `=`")
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 2);
    for diagnostic in diagnostics {
        assert_eq!(diagnostic.code(), "syntax.parse");
        assert!(matches!(
            &source[diagnostic.range().as_range()],
            "color red" | "opacity 0.9"
        ));
    }
}

#[test]
fn native_style_braces_in_values_strings_and_comments_are_not_selector_rules() {
    let source = r#"pub style authored_values {
    Button {
        custom-data = {
            label: "{literal brace}",
            nested: { amount: 1 },
        }
        content = "an unmatched { inside a string"
        opacity = 900milli // selector-looking { in a comment
        // Another selector-looking { comment is trivia.
    }
}
pub view Example() {
    Button("OK").style {
        custom-data = {
            label: "inline record",
            nested: { amount: 2 },
        }
    }
}
"#;
    let parsed = parse_source(source);
    assert_eq!(parsed.errors(), &[], "syntax errors: {:?}", parsed.errors());

    let named = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Style(style) => Some(style),
            _ => None,
        })
        .expect("named style");
    let declarations = named.sheet().body()[0]
        .as_rule()
        .expect("top-level rule")
        .declarations();
    assert!(matches!(
        declarations[0].value().expr(),
        Expr::RecordLiteral(fields) if fields.len() == 2
    ));
    assert!(matches!(
        declarations[1].value().expr(),
        Expr::Literal(Literal::String(value)) if value.contains('{')
    ));

    let inline = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) => item.view_body()?.view()?.style_patches().first().copied(),
            _ => None,
        })
        .expect("inline style");
    assert!(matches!(
        inline.declarations()[0].value().expr(),
        Expr::RecordLiteral(fields) if fields.len() == 2
    ));
}

#[test]
fn inline_native_style_rejects_only_a_top_level_selector_rule() {
    let source = r#"pub view Example() {
    Button("OK").style {
        custom-data = { label: "{" }
        Button:hover { opacity = 900milli }
    }
}
"#;
    let parsed = parse_source(source);
    let error = parsed
        .errors()
        .iter()
        .find(|error| error.kind() == ParseErrorKind::StyleInlineSelectorNotSupported)
        .expect("inline selector diagnostic");
    assert_eq!(error.code(), "style::inline_selector_not_supported");
    assert_eq!(
        error.range().end() - error.range().start(),
        "Button:hover { opacity = 900milli }".len()
    );
}

#[test]
fn named_style_rejects_nested_selector_with_typed_recovery() {
    let source = r"pub style broken {
    Panel {
        Button:hover { opacity = 900milli }
    }
}
";
    let parsed = parse_source(source);
    let error = parsed
        .errors()
        .iter()
        .find(|error| error.kind() == ParseErrorKind::StyleMalformedSelector)
        .expect("nested selector diagnostic");

    assert_eq!(error.code(), "style::malformed_selector");
    assert_eq!(
        &source[error.range().as_range()],
        "Button:hover { opacity = 900milli }"
    );
    assert_eq!(
        error.recovery()[0].message(),
        "use extract the selector into a named `style` declaration syntax"
    );
}

#[test]
fn unknown_inline_style_head_uses_ordinary_view_modifier_recovery() {
    let source = r#"pub view ExactRanges() {
    Button("One")
        .style {
            opacity = 900milli
        }
    Button("Two")
        .style {
            opacity = 900milli
        }
        .style(.UnknownDialect) {
            opacity = 500milli
        }
}
"#;
    let parsed = parse_source(source);
    let rejected = parsed
        .errors()
        .iter()
        .find(|error| error.message() == "unsupported View modifier")
        .expect("ordinary View modifier diagnostic");
    assert_eq!(rejected.code(), "syntax.parse");
    let patches = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) => item.view_body()?.view().map(|view| view.style_patches()),
            _ => None,
        })
        .expect("View style patches");
    assert_eq!(patches.len(), 2);

    let native_bodies = source
        .match_indices("\n            opacity = 900milli\n        ")
        .map(|(start, body)| (start, start + body.len()))
        .collect::<Vec<_>>();
    for (patch, (expected_start, expected_end)) in patches[..2].iter().zip(native_bodies) {
        assert_eq!(patch.range().start(), expected_start);
        assert_eq!(patch.range().end(), expected_end);
        assert_eq!(
            &source[patch.range().as_range()],
            "\n            opacity = 900milli\n        "
        );
        let declaration = &patch.declarations()[0];
        assert_eq!(
            &source[declaration.range().as_range()],
            "opacity = 900milli"
        );
        assert_eq!(
            &source[declaration.property().range().as_range()],
            "opacity"
        );
        assert_eq!(
            &source[declaration.value().range().as_range()],
            declaration.value().source()
        );
    }
}

#[test]
fn inline_style_ranges_survive_same_line_view_chain_expansion() {
    let source = r#"pub view ExpandedRange() {
    Button("One").style { opacity = 900milli }
    Button("Two").style { outline-width = 2px }
}
"#;
    let parsed = parse_source(source);
    assert_eq!(parsed.errors(), &[]);
    let patches = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) => item.view_body()?.view().map(|view| view.style_patches()),
            _ => None,
        })
        .expect("expanded inline style patches");
    let patch = patches[0];
    assert_eq!(&source[patch.range().as_range()], " opacity = 900milli ");
    assert_eq!(
        &source[patch.declarations()[0].range().as_range()],
        "opacity = 900milli"
    );
    let second = patches[1];
    assert_eq!(&source[second.range().as_range()], " outline-width = 2px ");
    assert_eq!(
        &source[second.declarations()[0].range().as_range()],
        "outline-width = 2px"
    );
}

#[test]
fn inline_native_style_diagnostic_starts_at_the_original_repeated_modifier() {
    let source = r#"pub view ExactDiagnostic() {
    Button("One")
        .style {
            opacity = 900milli
        }
    Button("Two")
        .style {
            opacity 900milli
        }
}
"#;
    let parsed = parse_source(source);
    let error = parsed
        .errors()
        .iter()
        .find(|error| error.message().contains("style declaration needs `=`"))
        .expect("missing equals diagnostic");
    let expected = source
        .find("opacity 900milli")
        .expect("malformed declaration source");
    assert_eq!(error.range().start(), expected);
    assert_eq!(&source[error.range().as_range()], "opacity 900milli");
}

#[test]
fn style_parser_reports_an_unclosed_style_block() {
    let parsed = parse_source("pub style broken {\n Button { opacity = 900milli }\n");
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.message().contains("unclosed block"))
    );
}

#[test]
fn inline_and_named_native_styles_share_expression_ast() {
    let parsed = parse_source(
        r#"pub style named {
    Button { opacity = 900milli }
}
pub view Example() {
    Button("OK").style { opacity = 900milli }
}
"#,
    );
    assert_eq!(parsed.errors(), &[]);
    let named = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::Style(style) => Some(style),
            _ => None,
        })
        .expect("named style");
    let named_expr = named.sheet().body()[0]
        .as_rule()
        .expect("top-level rule")
        .declarations()[0]
        .value()
        .expr();
    let inline_expr = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) => item.view_body()?.view()?.style_patches().first().copied(),
            _ => None,
        })
        .expect("inline style")
        .declarations()[0]
        .value()
        .expr();
    assert_eq!(named_expr, inline_expr);
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
fn view_text_argument_retains_recovered_call_at_the_authored_owner_boundary() {
    let source = r"
pub view RecoveredText() {
  Text(format(α, β)
}
";
    let parsed = parse_source(source);
    let view = parsed
        .typed_tree()
        .items()
        .iter()
        .find_map(|item| match item {
            Item::EntityDecl(item) => item.view_body()?.view(),
            _ => None,
        })
        .expect("view View body");
    let ViewExpr::Text(text) = view.value() else {
        panic!("expected Text View expression");
    };
    assert!(
        matches!(text.source(), Expr::Call(_)),
        "recovered argument: {:?}",
        text.source()
    );

    let boundary = source
        .find("β)")
        .map(|offset| offset + "β".len())
        .expect("authored outer View call boundary");
    let diagnostic = parsed
        .errors()
        .iter()
        .find(|error| error.message().contains("missing closing `)`"))
        .expect("missing call close diagnostic");
    assert_eq!(
        *diagnostic.range(),
        arcweft_lang_syntax::ast::common::TextRange::new(boundary, boundary)
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
        arcweft_lang_syntax::expr::Expr::Call(call)
            if call.args().len() == 2
                && call.args().iter().all(|arg| matches!(
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
    assert_eq!(
        button
            .modifiers()
            .iter()
            .filter(|modifier| matches!(
                modifier,
                ViewModifier::Style(ViewStyleModifier::Inline(_))
            ))
            .count(),
        1
    );
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
        [ViewModifier::Part(part)] if part.local_name().text() == "dialogue_panel"
    ));
}
